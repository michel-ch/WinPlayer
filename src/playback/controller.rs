use crate::domain::{sort_songs, PlaybackState, RepeatMode, Song, SortOption};
use crate::engine::{Engine, EngineCmd, EngineEvent};
use crate::playback::queue::Queue;
use parking_lot::RwLock;
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub struct PlaybackController {
    engine: Engine,
    queue: Arc<RwLock<Queue>>,
    state: Arc<RwLock<PlaybackState>>,
    event_tracker: Arc<RwLock<LoadEventTracker>>,
}

#[derive(Debug, Clone)]
pub struct QueueSnapshot {
    pub songs: Vec<Song>,
    pub current: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct StartedDecision {
    accepted: bool,
    pending_seek_ms: Option<u64>,
}

#[derive(Debug, Default)]
struct LoadEventTracker {
    requested_generation: u64,
    requested_path: Option<PathBuf>,
    engine_load_generation: Option<u64>,
    engine_load_path: Option<PathBuf>,
    pending_seek_ms: Option<u64>,
}

impl LoadEventTracker {
    fn new() -> Self {
        Self::default()
    }

    fn request_load(&mut self, path: PathBuf, pending_seek_ms: Option<u64>) {
        self.requested_generation = self.requested_generation.wrapping_add(1);
        self.requested_path = Some(path);
        self.engine_load_generation = None;
        self.pending_seek_ms = pending_seek_ms;
    }

    fn engine_load_started(&mut self, path: &Path) {
        self.engine_load_path = Some(path.to_path_buf());
        self.engine_load_generation = if self.requested_path.as_deref() == Some(path) {
            Some(self.requested_generation)
        } else {
            None
        };
    }

    fn started(&mut self, _duration_ms: u64) -> StartedDecision {
        if !self.timeline_event_is_current() {
            return StartedDecision {
                accepted: false,
                pending_seek_ms: None,
            };
        }
        StartedDecision {
            accepted: true,
            pending_seek_ms: self.pending_seek_ms.take(),
        }
    }

    fn timeline_event_is_current(&self) -> bool {
        matches!(
            (
                self.requested_path.as_deref(),
                self.engine_load_path.as_deref(),
                self.engine_load_generation
            ),
            (Some(requested), Some(engine), Some(engine_generation))
                if requested == engine && engine_generation == self.requested_generation
        )
    }

    fn load_failed_is_current(&self, path: &Path) -> bool {
        self.requested_path.as_deref() == Some(path)
            && self.engine_load_generation == Some(self.requested_generation)
    }
}

impl PlaybackController {
    pub fn new() -> Result<Arc<Self>, String> {
        let engine = Engine::start()?;
        let queue = Arc::new(RwLock::new(Queue::new()));
        let state = Arc::new(RwLock::new(PlaybackState::default()));
        let event_tracker = Arc::new(RwLock::new(LoadEventTracker::new()));
        let controller = Arc::new(Self {
            engine,
            queue,
            state,
            event_tracker,
        });
        Self::spawn_events_thread(controller.clone());
        Ok(controller)
    }

    pub fn state(&self) -> Arc<RwLock<PlaybackState>> {
        self.state.clone()
    }
    pub fn queue(&self) -> Arc<RwLock<Queue>> {
        self.queue.clone()
    }
    pub fn snapshot(&self) -> PlaybackState {
        self.state.read().clone()
    }
    pub fn queue_snapshot(&self) -> QueueSnapshot {
        let q = self.queue.read();
        QueueSnapshot {
            songs: q.songs.clone(),
            current: q.current,
        }
    }

    pub fn play_songs(&self, mut songs: Vec<Song>, start_index: usize, sort: Option<SortOption>) {
        if let Some(opt) = sort {
            sort_songs(&mut songs, opt);
        }
        if songs.is_empty() {
            return;
        }
        {
            let mut q = self.queue.write();
            q.replace(songs, start_index);
        }
        self.start_current();
    }

    pub fn play_pause(&self) {
        let mut s = self.state.write();
        if s.is_playing {
            s.is_playing = false;
            drop(s);
            self.engine.send(EngineCmd::Pause);
        } else if s.current_song.is_some() {
            s.is_playing = true;
            drop(s);
            self.engine.send(EngineCmd::Play);
        }
    }

    pub fn next(&self) {
        let next = { self.queue.write().advance_manual() };
        match next {
            Some(_) => self.start_current(),
            None => self.stop_internal(),
        }
    }

    pub fn previous(&self) {
        let pos_ms = self.state.read().current_position_ms;
        if pos_ms > 3000 {
            self.engine.send(EngineCmd::SeekFraction(0.0));
            return;
        }
        let _ = { self.queue.write().rewind() };
        self.start_current();
    }

    pub fn jump_to(&self, idx: usize) {
        let ok = { self.queue.write().jump_to(idx) };
        if ok {
            self.start_current();
        }
    }

    pub fn remove_from_queue(&self, id: i64) {
        let was_current = self.queue.read().current_song().map(|s| s.id) == Some(id);
        let removed = { self.queue.write().remove_song_id(id) };
        if removed > 0 && was_current {
            if self.queue.read().current_song().is_some() {
                self.start_current();
            } else {
                self.stop_internal();
            }
        }
    }

    pub fn prepare_current_for_delete(&self, id: i64) {
        let is_current = self.queue.read().current_song().map(|s| s.id) == Some(id);
        if !is_current {
            return;
        }
        // Best effort: Stop is asynchronous because the engine currently has
        // no ack for "decoder dropped its file handle". Windows may still
        // reject the delete; callers must leave the queue intact on failure.
        self.engine.send(EngineCmd::Stop);
        self.state.write().is_playing = false;
    }

    pub fn seek_fraction(&self, frac: f32) {
        self.engine.send(EngineCmd::SeekFraction(frac));
    }

    pub fn set_volume(&self, v: f32) {
        self.state.write().volume = v;
        self.engine.send(EngineCmd::SetVolume(v));
    }

    pub fn set_shuffle(&self, on: bool) {
        self.state.write().shuffle_enabled = on;
        self.queue.write().shuffle = on;
    }

    pub fn set_repeat(&self, r: RepeatMode) {
        self.state.write().repeat_mode = r;
        self.queue.write().repeat = r;
    }

    pub fn cycle_repeat(&self) {
        let cur = self.state.read().repeat_mode;
        self.set_repeat(cur.next());
    }

    /// Stop the audio engine and join its threads. Call from the App's Drop
    /// impl on shutdown so cpal's Stream tears down cleanly and `evt_tx`
    /// disconnects (which lets the events thread exit instead of leaking).
    pub fn shutdown(&self) {
        self.engine.send(EngineCmd::Shutdown);
    }

    fn start_current(&self) {
        let song = match self.queue.read().current_song().cloned() {
            Some(s) => s,
            None => return,
        };
        {
            let mut s = self.state.write();
            s.current_song = Some(song.clone());
            s.is_playing = true;
            s.current_position_ms = 0;
            s.duration_ms = song.duration.as_millis() as u64;
        }
        let _ = crate::last_played::save(&crate::last_played::LastPlayed {
            path: song.path.clone(),
            position_ms: 0,
        });
        self.event_tracker
            .write()
            .request_load(song.path.clone(), None);
        self.engine.send(EngineCmd::Load {
            path: song.path,
            autoplay: true,
        });
    }

    /// Restore a song into the queue and load it WITHOUT autoplay. Used at
    /// startup to surface the last-played track from `last_played.toml`.
    /// If `resume_ms > 0` the controller will seek to that position once the
    /// engine reports the track has started decoding.
    pub fn load_paused(&self, song: Song, resume_ms: u64) {
        {
            let mut q = self.queue.write();
            q.replace(vec![song.clone()], 0);
        }
        {
            let mut s = self.state.write();
            s.current_song = Some(song.clone());
            s.is_playing = false;
            s.current_position_ms = resume_ms;
            s.duration_ms = song.duration.as_millis() as u64;
        }
        let pending_seek_ms = (resume_ms > 0).then_some(resume_ms);
        self.event_tracker
            .write()
            .request_load(song.path.clone(), pending_seek_ms);
        self.engine.send(EngineCmd::Load {
            path: song.path,
            autoplay: false,
        });
    }

    fn stop_internal(&self) {
        self.engine.send(EngineCmd::Stop);
        let mut s = self.state.write();
        s.current_song = None;
        s.is_playing = false;
        s.current_position_ms = 0;
        s.duration_ms = 0;
    }

    fn spawn_events_thread(me: Arc<Self>) {
        let evt_rx = me.engine.events();
        std::thread::Builder::new()
            .name("playback-events".into())
            .spawn(move || {
                while let Ok(evt) = evt_rx.recv() {
                    match evt {
                        EngineEvent::Started { duration_ms } => {
                            let started = me.event_tracker.write().started(duration_ms);
                            if !started.accepted {
                                continue;
                            }
                            {
                                let mut s = me.state.write();
                                s.duration_ms = duration_ms;
                                // Don't reset `is_playing` here — the controller
                                // already set it correctly in start_current /
                                // load_paused. Reading the autoplay decision back
                                // from the engine was the source of a desync bug
                                // for paused-load (startup resume).
                            }
                            if let Some(pending) =
                                started.pending_seek_ms.filter(|_| duration_ms > 0)
                            {
                                let frac = (pending as f32 / duration_ms as f32).clamp(0.0, 1.0);
                                me.engine.send(EngineCmd::SeekFraction(frac));
                            }
                        }
                        EngineEvent::Position {
                            current_ms,
                            duration_ms,
                        } => {
                            if !me.event_tracker.read().timeline_event_is_current() {
                                continue;
                            }
                            let mut s = me.state.write();
                            s.current_position_ms = current_ms;
                            if duration_ms != 0 {
                                s.duration_ms = duration_ms;
                            }
                        }
                        EngineEvent::Paused => {
                            me.state.write().is_playing = false;
                        }
                        EngineEvent::Resumed => {
                            me.state.write().is_playing = true;
                        }
                        EngineEvent::EndOfTrack => {
                            if !me.event_tracker.read().timeline_event_is_current() {
                                continue;
                            }
                            let next = { me.queue.write().advance() };
                            match next {
                                Some(_) => me.start_current(),
                                None => me.stop_internal(),
                            }
                        }
                        EngineEvent::LoadFailed { path, error } => {
                            if !me.event_tracker.read().load_failed_is_current(&path) {
                                continue;
                            }
                            log::error!("load failed {}: {}", path.display(), error);
                            me.next();
                        }
                        EngineEvent::LoadStarted(path) => {
                            me.event_tracker.write().engine_load_started(&path);
                        }
                    }
                }
            })
            .expect("spawn playback-events");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};

    #[test]
    fn stale_started_event_for_previous_load_is_ignored() {
        let mut tracker = LoadEventTracker::new();
        let old = PathBuf::from("old.mp3");
        let new = PathBuf::from("new.mp3");

        tracker.request_load(old.clone(), None);
        tracker.engine_load_started(&old);
        tracker.request_load(new.clone(), None);

        assert!(!tracker.started(10_000).accepted);

        tracker.engine_load_started(&new);
        assert!(tracker.started(10_000).accepted);
    }

    #[test]
    fn stale_started_event_does_not_consume_new_pending_seek() {
        let mut tracker = LoadEventTracker::new();
        let old = PathBuf::from("old.mp3");
        let new = PathBuf::from("new.mp3");

        tracker.request_load(old.clone(), None);
        tracker.engine_load_started(&old);
        tracker.request_load(new.clone(), Some(4_000));

        assert_eq!(tracker.started(10_000).pending_seek_ms, None);
        tracker.engine_load_started(&new);
        assert_eq!(tracker.started(10_000).pending_seek_ms, Some(4_000));
    }

    #[test]
    fn stale_timeline_events_for_previous_load_are_ignored() {
        let mut tracker = LoadEventTracker::new();
        let old = Path::new("old.mp3");
        let new = Path::new("new.mp3");

        tracker.request_load(old.to_path_buf(), None);
        tracker.engine_load_started(old);
        tracker.request_load(new.to_path_buf(), None);

        assert!(!tracker.timeline_event_is_current());
        tracker.engine_load_started(new);
        assert!(tracker.timeline_event_is_current());
    }

    #[test]
    fn stale_timeline_events_for_previous_same_path_load_are_ignored() {
        let mut tracker = LoadEventTracker::new();
        let path = Path::new("same.mp3");

        tracker.request_load(path.to_path_buf(), None);
        tracker.engine_load_started(path);
        tracker.request_load(path.to_path_buf(), None);

        assert!(!tracker.timeline_event_is_current());
        tracker.engine_load_started(path);
        assert!(tracker.timeline_event_is_current());
    }
}
