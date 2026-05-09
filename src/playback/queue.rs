use crate::domain::{RepeatMode, Song};

pub struct Queue {
    pub songs: Vec<Song>,
    pub current: Option<usize>,
    pub shuffle: bool,
    pub repeat: RepeatMode,
}

impl Queue {
    pub fn new() -> Self {
        Self { songs: Vec::new(), current: None, shuffle: false, repeat: RepeatMode::Off }
    }

    pub fn replace(&mut self, songs: Vec<Song>, start: usize) {
        let len = songs.len();
        self.songs = songs;
        self.current = if len == 0 { None } else { Some(start.min(len.saturating_sub(1))) };
    }

    pub fn current_song(&self) -> Option<&Song> {
        self.current.and_then(|i| self.songs.get(i))
    }

    pub fn next_index(&self) -> Option<usize> {
        let cur = self.current?;
        match self.repeat {
            RepeatMode::One => Some(cur),
            RepeatMode::All => {
                if self.songs.is_empty() { None } else { Some((cur + 1) % self.songs.len()) }
            }
            RepeatMode::Off => {
                let nxt = cur + 1;
                if nxt < self.songs.len() { Some(nxt) } else { None }
            }
        }
    }

    pub fn prev_index(&self) -> Option<usize> {
        let cur = self.current?;
        if cur == 0 {
            match self.repeat {
                RepeatMode::All if !self.songs.is_empty() => Some(self.songs.len() - 1),
                _ => Some(0),
            }
        } else {
            Some(cur - 1)
        }
    }

    pub fn advance(&mut self) -> Option<usize> {
        self.current = self.next_index();
        self.current
    }

    pub fn rewind(&mut self) -> Option<usize> {
        self.current = self.prev_index();
        self.current
    }

    pub fn jump_to(&mut self, idx: usize) -> bool {
        if idx < self.songs.len() {
            self.current = Some(idx);
            true
        } else {
            false
        }
    }

    pub fn remove_song_id(&mut self, id: i64) -> usize {
        let removed_before_current = self.current
            .map(|c| self.songs[..c].iter().filter(|s| s.id == id).count())
            .unwrap_or(0);
        let current_dropped = self.current
            .map(|c| self.songs.get(c).map(|s| s.id == id).unwrap_or(false))
            .unwrap_or(false);

        let before = self.songs.len();
        self.songs.retain(|s| s.id != id);
        let removed = before - self.songs.len();

        if removed > 0 {
            if let Some(c) = self.current {
                let new_c = c.saturating_sub(removed_before_current);
                if self.songs.is_empty() {
                    self.current = None;
                } else if current_dropped {
                    self.current = Some(new_c.min(self.songs.len() - 1));
                } else {
                    self.current = Some(new_c.min(self.songs.len() - 1));
                }
            }
        }
        removed
    }
}

impl Default for Queue {
    fn default() -> Self { Self::new() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::song_id_from_path;
    use std::path::{Path, PathBuf};
    use std::time::Duration;

    fn s(p: &str) -> Song {
        Song {
            id: song_id_from_path(Path::new(p)),
            title: p.into(), artist: "".into(), album: "".into(),
            album_artist: "".into(),
            duration: Duration::from_secs(1),
            year: None, genre: None, composer: None, track_no: None,
            path: PathBuf::from(p),
            has_embedded_art: false,
        }
    }

    #[test]
    fn next_off_stops_at_end() {
        let mut q = Queue::new();
        q.replace(vec![s("/a"), s("/b")], 1);
        q.repeat = RepeatMode::Off;
        assert_eq!(q.next_index(), None);
    }

    #[test]
    fn next_all_wraps() {
        let mut q = Queue::new();
        q.replace(vec![s("/a"), s("/b")], 1);
        q.repeat = RepeatMode::All;
        assert_eq!(q.next_index(), Some(0));
    }

    #[test]
    fn next_one_stays() {
        let mut q = Queue::new();
        q.replace(vec![s("/a"), s("/b")], 1);
        q.repeat = RepeatMode::One;
        assert_eq!(q.next_index(), Some(1));
    }

    #[test]
    fn prev_at_zero_off_stays() {
        let mut q = Queue::new();
        q.replace(vec![s("/a"), s("/b")], 0);
        q.repeat = RepeatMode::Off;
        assert_eq!(q.prev_index(), Some(0));
    }

    #[test]
    fn prev_at_zero_all_wraps_to_last() {
        let mut q = Queue::new();
        q.replace(vec![s("/a"), s("/b"), s("/c")], 0);
        q.repeat = RepeatMode::All;
        assert_eq!(q.prev_index(), Some(2));
    }

    #[test]
    fn remove_before_current_decrements_current() {
        let mut q = Queue::new();
        let a = s("/a"); let b = s("/b"); let c = s("/c");
        q.replace(vec![a.clone(), b.clone(), c.clone()], 2);
        q.remove_song_id(a.id);
        assert_eq!(q.current, Some(1));
        assert_eq!(q.current_song().unwrap().id, c.id);
    }

    #[test]
    fn remove_current_keeps_index_stable() {
        let mut q = Queue::new();
        let a = s("/a"); let b = s("/b"); let c = s("/c");
        q.replace(vec![a.clone(), b.clone(), c.clone()], 1);
        q.remove_song_id(b.id);
        assert_eq!(q.current_song().unwrap().id, c.id);
    }

    #[test]
    fn remove_last_clamps_to_end() {
        let mut q = Queue::new();
        let a = s("/a"); let b = s("/b");
        q.replace(vec![a.clone(), b.clone()], 1);
        q.remove_song_id(b.id);
        assert_eq!(q.current_song().unwrap().id, a.id);
    }

    #[test]
    fn remove_all_clears_current() {
        let mut q = Queue::new();
        let a = s("/a");
        q.replace(vec![a.clone()], 0);
        q.remove_song_id(a.id);
        assert_eq!(q.current, None);
    }
}
