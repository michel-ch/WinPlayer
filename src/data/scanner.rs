use crate::data::tags::read_song;
use crate::domain::Song;
use std::path::Path;
use walkdir::WalkDir;

const EXTS: &[&str] = &["mp3", "flac", "m4a", "ogg", "wav", "aac", "opus"];

pub fn is_audio_path(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| EXTS.iter().any(|x| x.eq_ignore_ascii_case(e)))
        .unwrap_or(false)
}

pub fn scan_root(root: &Path) -> Vec<Song> {
    let mut out = Vec::new();
    if !root.exists() {
        log::info!("scan root missing: {}", root.display());
        return out;
    }
    for entry in WalkDir::new(root).into_iter().filter_map(|e| e.ok()) {
        if !entry.file_type().is_file() { continue; }
        let path = entry.path();
        if !is_audio_path(path) { continue; }
        if let Some(song) = read_song(path) {
            out.push(song);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn extension_check_is_case_insensitive() {
        assert!(is_audio_path(&PathBuf::from("a.MP3")));
        assert!(is_audio_path(&PathBuf::from("a.flac")));
        assert!(is_audio_path(&PathBuf::from("a.OPUS")));
        assert!(!is_audio_path(&PathBuf::from("a.txt")));
        assert!(!is_audio_path(&PathBuf::from("a")));
    }

    #[test]
    fn missing_root_returns_empty_without_panic() {
        let out = scan_root(&PathBuf::from("Z:/no/such/path"));
        assert!(out.is_empty());
    }
}
