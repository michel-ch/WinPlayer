use crate::domain::{song_id_from_path, Song};
use lofty::file::{AudioFile, TaggedFileExt};
use lofty::tag::{Accessor, ItemKey};
#[cfg(panic = "unwind")]
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::time::Duration;

const UNKNOWN_ARTIST: &str = "Unknown Artist";

pub fn parse_track_prefix(path: &Path) -> Option<u32> {
    let stem = path.file_stem()?.to_string_lossy();
    let digits: String = stem.chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        return None;
    }
    digits.parse().ok()
}

pub fn read_song(path: &Path) -> Option<Song> {
    #[cfg(panic = "unwind")]
    {
        let result = catch_unwind(AssertUnwindSafe(|| read_song_inner(path)));
        match result {
            Ok(Some(s)) => Some(s),
            Ok(None) => None,
            Err(_) => {
                log::warn!("lofty panicked on {}", path.display());
                Some(synthetic_song(path))
            }
        }
    }

    #[cfg(not(panic = "unwind"))]
    {
        read_song_inner(path)
    }
}

fn read_song_inner(path: &Path) -> Option<Song> {
    let tagged = lofty::read_from_path(path).ok()?;
    let properties = tagged.properties();
    let duration = properties.duration();

    let primary_tag = tagged.primary_tag().or_else(|| tagged.first_tag());

    let title = primary_tag
        .and_then(|t| t.title().map(|c| c.to_string()))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| filename_stem(path));

    let artist = primary_tag
        .and_then(|t| t.artist().map(|c| c.to_string()))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| UNKNOWN_ARTIST.to_string());

    let album = primary_tag
        .and_then(|t| t.album().map(|c| c.to_string()))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| parent_folder_name(path));

    let album_artist = primary_tag
        .and_then(|t| t.get_string(&ItemKey::AlbumArtist).map(String::from))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| artist.clone());

    let year = primary_tag.and_then(|t| t.year());
    let genre = primary_tag.and_then(|t| t.genre().map(|c| c.to_string()));
    let composer = primary_tag.and_then(|t| t.get_string(&ItemKey::Composer).map(String::from));

    let track_no = primary_tag
        .and_then(|t| t.track())
        .or_else(|| parse_track_prefix(path));

    let has_embedded_art = primary_tag.is_some_and(|t| t.picture_count() > 0);

    Some(Song {
        id: song_id_from_path(path),
        title,
        artist,
        album,
        album_artist,
        duration,
        year,
        genre,
        composer,
        track_no,
        path: PathBuf::from(path),
        has_embedded_art,
    })
}

fn filename_stem(path: &Path) -> String {
    path.file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "Unknown".to_string())
}

fn parent_folder_name(path: &Path) -> String {
    path.parent()
        .and_then(|p| p.file_name())
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "Unknown Album".to_string())
}

fn synthetic_song(path: &Path) -> Song {
    Song {
        id: song_id_from_path(path),
        title: filename_stem(path),
        artist: UNKNOWN_ARTIST.to_string(),
        album: parent_folder_name(path),
        album_artist: UNKNOWN_ARTIST.to_string(),
        duration: Duration::from_secs(1),
        year: None,
        genre: None,
        composer: None,
        track_no: parse_track_prefix(path),
        path: PathBuf::from(path),
        has_embedded_art: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn track_prefix_simple() {
        assert_eq!(parse_track_prefix(&PathBuf::from("01 - foo.mp3")), Some(1));
        assert_eq!(parse_track_prefix(&PathBuf::from("12.bar.flac")), Some(12));
        assert_eq!(parse_track_prefix(&PathBuf::from("foo.mp3")), None);
    }

    #[test]
    fn parent_folder_name_falls_back_when_orphan() {
        assert_eq!(parent_folder_name(Path::new("foo.mp3")), "Unknown Album");
    }
}
