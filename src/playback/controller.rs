use crate::data::Library;
use crate::domain::{sort_songs, PlaybackState, RepeatMode, Song, SortOption};
use crate::engine::{Engine, EngineCmd, EngineEvent};
use crate::playback::queue::Queue;
use parking_lot::RwLock;
use std::sync::Arc;

pub struct PlaybackController {
    engine: Engine,
    #[allow(dead_code)]
    library: Arc<Library>,
    queue: Arc<RwLock<Queue>>,
    state: Arc<RwLock<PlaybackState>>,
}

impl PlaybackController {
    pub fn new(library: Arc<Library>) -> Result<Arc<Self>, String> {
        let engine = Engine::start()?;
        let queue = Arc::new(RwLock::new(Queue::new()));
        let state = Arc::new(RwLock::new(PlaybackState::default()));
        let controller = Arc::new(Self { engine, library, queue, state });
        Self::spawn_events_thread(controller.clone());
        Ok(controller)
    }

    pub fn state(&self) -> Arc<RwLock<PlaybackState>> { self.state.clone() }
    pub fn queue(&self) -> Arc<RwLock<Queue>> { self.queue.clone() }
    pub fn snapshot(&self) -> PlaybackState { self.state.read().clone() }

    pub fn play_songs(&self, mut songs: Vec<Song>, start_index: usize, sort: Option<SortOption>) {
        if let Some(opt) = sort { sort_songs(&mut songs, opt); }
        if songs.is_empty() { return; }
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
        let next = { self.queue.write().advance() };
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
        if ok { self.start_current(); }
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
        self.engine.send(EngineCmd::Load { path: song.path, autoplay: true });
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
        std::thread::Builder::new().name("playback-events".into()).spawn(move || {
            while let Ok(evt) = evt_rx.recv() {
                match evt {
                    EngineEvent::Started { duration_ms } => {
                        let mut s = me.state.write();
                        s.duration_ms = duration_ms;
                        s.current_position_ms = 0;
                        s.is_playing = true;
                    }
                    EngineEvent::Position { current_ms, duration_ms } => {
                        let mut s = me.state.write();
                        s.current_position_ms = current_ms;
                        if duration_ms != 0 { s.duration_ms = duration_ms; }
                    }
                    EngineEvent::Paused => { me.state.write().is_playing = false; }
                    EngineEvent::Resumed => { me.state.write().is_playing = true; }
                    EngineEvent::EndOfTrack => { me.next(); }
                    EngineEvent::LoadFailed { path, error } => {
                        log::error!("load failed {}: {}", path.display(), error);
                        me.next();
                    }
                    EngineEvent::LoadStarted(_) => {}
                }
            }
        }).expect("spawn playback-events");
    }
}
