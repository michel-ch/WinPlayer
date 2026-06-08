use crate::data::scanner::scan_root;
use crate::domain::{normalize_path_key, Song};
use parking_lot::RwLock;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LibraryStatus {
    Idle,
    Scanning,
    Ready,
}

pub struct Library {
    songs: RwLock<Vec<Song>>,
    status: RwLock<LibraryStatus>,
    version: AtomicU64,
}

impl Library {
    pub fn new() -> Self {
        Self {
            songs: RwLock::new(Vec::new()),
            status: RwLock::new(LibraryStatus::Idle),
            version: AtomicU64::new(0),
        }
    }

    pub fn version(&self) -> u64 {
        self.version.load(Ordering::Acquire)
    }
    fn bump(&self) {
        self.version.fetch_add(1, Ordering::AcqRel);
    }

    pub fn status(&self) -> LibraryStatus {
        *self.status.read()
    }
    pub fn set_status(&self, s: LibraryStatus) {
        *self.status.write() = s;
    }

    pub fn songs_snapshot(&self) -> Vec<Song> {
        self.songs.read().clone()
    }
    pub fn len(&self) -> usize {
        self.songs.read().len()
    }
    pub fn is_empty(&self) -> bool {
        self.songs.read().is_empty()
    }

    pub fn find_by_id(&self, id: i64) -> Option<Song> {
        self.songs.read().iter().find(|s| s.id == id).cloned()
    }

    pub fn scan(&self, roots: &[PathBuf]) {
        self.set_status(LibraryStatus::Scanning);
        let mut all: Vec<Song> = Vec::new();
        let mut seen: HashSet<String> = HashSet::new();
        for r in roots {
            for song in scan_root(r) {
                let k = normalize_path_key(&song.path);
                if seen.insert(k) {
                    all.push(song);
                }
            }
        }
        {
            let mut songs = self.songs.write();
            *songs = all;
            self.bump();
        }
        self.set_status(LibraryStatus::Ready);
    }

    pub fn refresh_folder(&self, folder: &Path) {
        let new = scan_root(folder);
        let mut songs = self.songs.write();
        songs.retain(|s| !path_is_under_folder(&s.path, folder));
        let mut existing_keys: HashSet<String> =
            songs.iter().map(|s| normalize_path_key(&s.path)).collect();
        for song in new {
            let k = normalize_path_key(&song.path);
            if existing_keys.insert(k) {
                songs.push(song);
            }
        }
        self.bump();
    }

    pub fn remove_song(&self, id: i64) -> bool {
        let mut songs = self.songs.write();
        let before = songs.len();
        songs.retain(|s| s.id != id);
        let removed = songs.len() < before;
        if removed {
            self.bump();
        }
        removed
    }

    pub fn replace_all(&self, new_songs: Vec<Song>) {
        let mut songs = self.songs.write();
        *songs = new_songs;
        self.bump();
    }
}

fn path_is_under_folder(path: &Path, folder: &Path) -> bool {
    let path_key = normalize_path_key(path);
    let mut folder_key = normalize_path_key(folder);
    if !folder_key.ends_with('/') {
        folder_key.push('/');
    }
    path_key.starts_with(&folder_key)
}

impl Default for Library {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::song_id_from_path;
    use std::time::Duration;

    fn fake(path: &str) -> Song {
        Song {
            id: song_id_from_path(Path::new(path)),
            title: "t".into(),
            artist: "a".into(),
            album: "al".into(),
            album_artist: "a".into(),
            duration: Duration::from_secs(1),
            year: None,
            genre: None,
            composer: None,
            track_no: None,
            path: PathBuf::from(path),
            has_embedded_art: false,
        }
    }

    #[test]
    fn version_bumps_on_remove() {
        let lib = Library::new();
        lib.replace_all(vec![fake("/a.mp3"), fake("/b.mp3")]);
        let v = lib.version();
        assert!(lib.remove_song(fake("/a.mp3").id));
        assert!(lib.version() > v);
    }

    #[test]
    fn remove_unknown_id_returns_false() {
        let lib = Library::new();
        lib.replace_all(vec![fake("/a.mp3")]);
        assert!(!lib.remove_song(999_999));
    }

    #[test]
    fn find_by_id_returns_clone() {
        let lib = Library::new();
        let s = fake("/a.mp3");
        lib.replace_all(vec![s.clone()]);
        assert_eq!(lib.find_by_id(s.id).unwrap().path, s.path);
    }

    #[test]
    fn refresh_folder_removes_stale_tracks_under_nested_paths() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("music");
        let nested = root.join("disc1");
        std::fs::create_dir_all(&nested).unwrap();

        let stale_nested = fake(&nested.join("old.mp3").to_string_lossy());
        let outside = fake(&dir.path().join("outside.mp3").to_string_lossy());

        let lib = Library::new();
        lib.replace_all(vec![stale_nested, outside.clone()]);
        lib.refresh_folder(&root);

        let songs = lib.songs_snapshot();
        assert_eq!(songs, vec![outside]);
    }
}
