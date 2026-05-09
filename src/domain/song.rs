use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Song {
    pub id: i64,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub album_artist: String,
    pub duration: Duration,
    pub year: Option<u32>,
    pub genre: Option<String>,
    pub composer: Option<String>,
    pub track_no: Option<u32>,
    pub path: PathBuf,
    pub has_embedded_art: bool,
}

pub fn normalize_path_key(path: &Path) -> String {
    let s = path.to_string_lossy();
    if cfg!(windows) {
        s.replace('\\', "/").to_lowercase()
    } else {
        s.into_owned()
    }
}

pub fn song_id_from_path(path: &Path) -> i64 {
    let mut h = DefaultHasher::new();
    normalize_path_key(path).hash(&mut h);
    h.finish() as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_paths_normalize_to_same_key() {
        if !cfg!(windows) { return; }
        let a = PathBuf::from(r"C:\Music\foo.mp3");
        let b = PathBuf::from(r"c:\music\foo.mp3");
        let c = PathBuf::from("C:/Music/foo.mp3");
        assert_eq!(normalize_path_key(&a), normalize_path_key(&b));
        assert_eq!(normalize_path_key(&a), normalize_path_key(&c));
        assert_eq!(song_id_from_path(&a), song_id_from_path(&c));
    }

    #[test]
    fn distinct_paths_get_distinct_ids() {
        let a = song_id_from_path(Path::new("/x/a.mp3"));
        let b = song_id_from_path(Path::new("/x/b.mp3"));
        assert_ne!(a, b);
    }
}
