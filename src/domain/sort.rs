use crate::domain::Song;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::time::SystemTime;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortOption {
    TitleAsc, TitleDesc,
    ArtistAsc, ArtistDesc,
    AlbumAsc, AlbumDesc,
    DurationAsc, DurationDesc,
    TrackNoAsc, TrackNoDesc,
    FilenameAsc, FilenameDesc,
    Shuffle,
}

impl SortOption {
    pub const ALL: [SortOption; 13] = [
        SortOption::TitleAsc, SortOption::TitleDesc,
        SortOption::ArtistAsc, SortOption::ArtistDesc,
        SortOption::AlbumAsc, SortOption::AlbumDesc,
        SortOption::DurationAsc, SortOption::DurationDesc,
        SortOption::TrackNoAsc, SortOption::TrackNoDesc,
        SortOption::FilenameAsc, SortOption::FilenameDesc,
        SortOption::Shuffle,
    ];

    pub fn label(self) -> &'static str {
        match self {
            SortOption::TitleAsc => "Title \u{2191}",
            SortOption::TitleDesc => "Title \u{2193}",
            SortOption::ArtistAsc => "Artist \u{2191}",
            SortOption::ArtistDesc => "Artist \u{2193}",
            SortOption::AlbumAsc => "Album \u{2191}",
            SortOption::AlbumDesc => "Album \u{2193}",
            SortOption::DurationAsc => "Duration \u{2191}",
            SortOption::DurationDesc => "Duration \u{2193}",
            SortOption::TrackNoAsc => "Track # \u{2191}",
            SortOption::TrackNoDesc => "Track # \u{2193}",
            SortOption::FilenameAsc => "Filename \u{2191}",
            SortOption::FilenameDesc => "Filename \u{2193}",
            SortOption::Shuffle => "Shuffle",
        }
    }
}

fn filename_lower(s: &Song) -> String {
    s.path.file_name()
        .map(|n| n.to_string_lossy().to_lowercase())
        .unwrap_or_default()
}

pub fn sort_songs(songs: &mut [Song], opt: SortOption) {
    let key_lower = |s: &str| s.to_lowercase();
    match opt {
        SortOption::TitleAsc => songs.sort_by_key(|s| key_lower(&s.title)),
        SortOption::TitleDesc => { songs.sort_by_key(|s| key_lower(&s.title)); songs.reverse(); }
        SortOption::ArtistAsc => songs.sort_by_key(|s| key_lower(&s.artist)),
        SortOption::ArtistDesc => { songs.sort_by_key(|s| key_lower(&s.artist)); songs.reverse(); }
        SortOption::AlbumAsc => songs.sort_by_key(|s| key_lower(&s.album)),
        SortOption::AlbumDesc => { songs.sort_by_key(|s| key_lower(&s.album)); songs.reverse(); }
        SortOption::DurationAsc => songs.sort_by_key(|s| s.duration),
        SortOption::DurationDesc => { songs.sort_by_key(|s| s.duration); songs.reverse(); }
        SortOption::TrackNoAsc => songs.sort_by_key(|s| s.track_no.unwrap_or(u32::MAX)),
        SortOption::TrackNoDesc => { songs.sort_by_key(|s| s.track_no.unwrap_or(0)); songs.reverse(); }
        SortOption::FilenameAsc => songs.sort_by_key(filename_lower),
        SortOption::FilenameDesc => { songs.sort_by_key(filename_lower); songs.reverse(); }
        SortOption::Shuffle => {
            let nanos = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .map(|d| d.as_nanos() as u64)
                .unwrap_or(0);
            songs.sort_by_key(|s| {
                let mut h = DefaultHasher::new();
                (s.id, nanos).hash(&mut h);
                h.finish()
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::time::Duration;

    fn s(id: i64, title: &str, artist: &str, dur_secs: u64, track: Option<u32>, name: &str) -> Song {
        Song {
            id, title: title.into(), artist: artist.into(),
            album: String::new(), album_artist: String::new(),
            duration: Duration::from_secs(dur_secs),
            year: None, genre: None, composer: None,
            track_no: track,
            path: PathBuf::from(name),
            has_embedded_art: false,
        }
    }

    #[test]
    fn title_asc_is_case_insensitive() {
        let mut v = vec![s(1, "beta", "", 0, None, "1"), s(2, "Alpha", "", 0, None, "2")];
        sort_songs(&mut v, SortOption::TitleAsc);
        assert_eq!(v[0].id, 2);
    }

    #[test]
    fn track_asc_pushes_missing_to_end() {
        let mut v = vec![
            s(1, "", "", 0, None, "1"),
            s(2, "", "", 0, Some(2), "2"),
            s(3, "", "", 0, Some(1), "3"),
        ];
        sort_songs(&mut v, SortOption::TrackNoAsc);
        assert_eq!(v.iter().map(|x| x.id).collect::<Vec<_>>(), vec![3, 2, 1]);
    }

    #[test]
    fn shuffle_does_not_lose_songs() {
        let mut v: Vec<Song> = (0..50).map(|i| s(i, "", "", 0, None, "x")).collect();
        sort_songs(&mut v, SortOption::Shuffle);
        assert_eq!(v.len(), 50);
    }
}
