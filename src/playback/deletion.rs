use crate::data::Library;
use crate::playback::controller::PlaybackController;
use crate::renumberer::renumber_folder;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Debug)]
pub struct DeletionResult {
    pub deleted_path: PathBuf,
    pub renumbered: u32,
}

pub trait PlaybackDeletionHooks {
    fn is_current_song(&self, id: i64) -> bool;
    fn prepare_current_for_delete(&self, id: i64);
    fn remove_from_queue_after_delete(&self, id: i64);
}

impl PlaybackDeletionHooks for PlaybackController {
    fn is_current_song(&self, id: i64) -> bool {
        self.snapshot().current_song.as_ref().map(|s| s.id) == Some(id)
    }

    fn prepare_current_for_delete(&self, id: i64) {
        PlaybackController::prepare_current_for_delete(self, id);
    }

    fn remove_from_queue_after_delete(&self, id: i64) {
        self.remove_from_queue(id);
    }
}

pub fn delete_song_with_playback<P: PlaybackDeletionHooks + ?Sized>(
    library: &Arc<Library>,
    playback: &P,
    id: i64,
    renumber: bool,
    threshold: f32,
) -> Result<DeletionResult, String> {
    let is_current = playback.is_current_song(id);
    if is_current {
        playback.prepare_current_for_delete(id);
    }

    let result = delete_song(library, id, renumber, threshold)?;
    playback.remove_from_queue_after_delete(id);
    Ok(result)
}

pub fn delete_song(
    library: &Arc<Library>,
    id: i64,
    renumber: bool,
    threshold: f32,
) -> Result<DeletionResult, String> {
    let song = library
        .find_by_id(id)
        .ok_or_else(|| format!("song {} not found", id))?;
    let path = song.path.clone();
    let folder = path
        .parent()
        .map(PathBuf::from)
        .ok_or_else(|| format!("song path has no parent: {}", path.display()))?;

    std::fs::remove_file(&path).map_err(|e| format!("remove_file: {e}"))?;
    library.remove_song(id);

    let renumbered = if renumber {
        let count = renumber_folder(&folder, threshold).map_err(|e| format!("renumber: {e}"))?;
        if count > 0 {
            library.refresh_folder(&folder);
        }
        count
    } else {
        0
    };

    Ok(DeletionResult {
        deleted_path: path,
        renumbered,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{song_id_from_path, Song};
    use std::fs::File;
    use std::time::Duration;

    fn fake(path: PathBuf) -> Song {
        Song {
            id: song_id_from_path(&path),
            title: "t".into(),
            artist: "a".into(),
            album: "al".into(),
            album_artist: "a".into(),
            duration: Duration::from_secs(1),
            year: None,
            genre: None,
            composer: None,
            track_no: None,
            path,
            has_embedded_art: false,
        }
    }

    #[test]
    fn missing_song_errors() {
        let lib = Arc::new(Library::new());
        assert!(delete_song(&lib, 12345, false, 0.5).is_err());
    }

    #[test]
    fn deletes_file_and_drops_from_library() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("x.mp3");
        File::create(&path).unwrap();
        let song = fake(path.clone());
        let lib = Arc::new(Library::new());
        lib.replace_all(vec![song.clone()]);
        let res = delete_song(&lib, song.id, false, 0.5).unwrap();
        assert_eq!(res.deleted_path, path);
        assert!(!path.exists());
        assert_eq!(lib.len(), 0);
    }

    #[derive(Default)]
    struct PlaybackHooks {
        current: bool,
        prepared: usize,
        removed: usize,
    }

    impl PlaybackDeletionHooks for parking_lot::Mutex<PlaybackHooks> {
        fn is_current_song(&self, _id: i64) -> bool {
            self.lock().current
        }

        fn prepare_current_for_delete(&self, _id: i64) {
            self.lock().prepared += 1;
        }

        fn remove_from_queue_after_delete(&self, _id: i64) {
            self.lock().removed += 1;
        }
    }

    #[test]
    fn current_delete_failure_prepares_playback_but_keeps_queue() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("missing.mp3");
        let song = fake(path);
        let lib = Arc::new(Library::new());
        lib.replace_all(vec![song.clone()]);
        let playback = parking_lot::Mutex::new(PlaybackHooks {
            current: true,
            ..Default::default()
        });

        let err = delete_song_with_playback(&lib, &playback, song.id, false, 0.5).unwrap_err();

        assert!(err.contains("remove_file"));
        assert!(lib.find_by_id(song.id).is_some());
        let hooks = playback.lock();
        assert_eq!(hooks.prepared, 1);
        assert_eq!(hooks.removed, 0);
    }

    #[test]
    fn successful_delete_removes_from_queue_after_file_delete() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("x.mp3");
        File::create(&path).unwrap();
        let song = fake(path);
        let lib = Arc::new(Library::new());
        lib.replace_all(vec![song.clone()]);
        let playback = parking_lot::Mutex::new(PlaybackHooks {
            current: false,
            ..Default::default()
        });

        delete_song_with_playback(&lib, &playback, song.id, false, 0.5).unwrap();

        let hooks = playback.lock();
        assert_eq!(hooks.prepared, 0);
        assert_eq!(hooks.removed, 1);
    }
}
