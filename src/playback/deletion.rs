use crate::data::Library;
use crate::renumberer::renumber_folder;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Debug)]
pub struct DeletionResult {
    pub deleted_path: PathBuf,
    pub renumbered: u32,
}

pub fn delete_song(
    library: &Arc<Library>,
    id: i64,
    renumber: bool,
    threshold: f32,
) -> Result<DeletionResult, String> {
    let song = library.find_by_id(id)
        .ok_or_else(|| format!("song {} not found", id))?;
    let path = song.path.clone();
    let folder = path.parent().map(PathBuf::from)
        .ok_or_else(|| format!("song path has no parent: {}", path.display()))?;

    std::fs::remove_file(&path)
        .map_err(|e| format!("remove_file: {e}"))?;
    library.remove_song(id);

    let renumbered = if renumber {
        let count = renumber_folder(&folder, threshold)
            .map_err(|e| format!("renumber: {e}"))?;
        if count > 0 {
            library.refresh_folder(&folder);
        }
        count
    } else { 0 };

    Ok(DeletionResult { deleted_path: path, renumbered })
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
            title: "t".into(), artist: "a".into(), album: "al".into(),
            album_artist: "a".into(),
            duration: Duration::from_secs(1),
            year: None, genre: None, composer: None, track_no: None,
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
}
