use crate::data::Library;
use crate::domain::{Screen, Song, SortOption};
use crate::playback::PlaybackController;
use crate::ui::components::song_row::{draw_with_options, RowOptions};
use std::sync::Arc;

#[derive(Default)]
pub struct ArtistsCache {
    version: Option<u64>,
    entries: Vec<(String, usize)>,
    detail_version: Option<u64>,
    detail_artist: Option<String>,
    detail_songs: Vec<Song>,
}

impl ArtistsCache {
    fn refresh(&mut self, library: &Arc<Library>) {
        let v = library.version();
        if self.version == Some(v) {
            return;
        }
        let songs = library.songs_snapshot();
        let mut by_artist: std::collections::BTreeMap<String, usize> = Default::default();
        for s in &songs {
            *by_artist.entry(s.artist.clone()).or_insert(0) += 1;
        }
        self.entries = by_artist.into_iter().collect();
        self.version = Some(v);
    }

    fn detail_songs(&mut self, library: &Arc<Library>, artist: &str) -> &[Song] {
        let v = library.version();
        if self.detail_version != Some(v) || self.detail_artist.as_deref() != Some(artist) {
            let mut songs = library.songs_snapshot();
            songs.retain(|s| s.artist == artist);
            crate::domain::sort_songs(&mut songs, SortOption::AlbumAsc);
            self.detail_songs = songs;
            self.detail_artist = Some(artist.to_owned());
            self.detail_version = Some(v);
        }
        &self.detail_songs
    }
}

pub fn draw_list(
    ui: &mut egui::Ui,
    library: &Arc<Library>,
    cache: &mut ArtistsCache,
    screen: &mut Screen,
) {
    crate::ui::components::page_header::page_header(ui, "Library", "Artists");
    cache.refresh(library);
    egui::ScrollArea::vertical().show(ui, |ui| {
        for (artist, count) in &cache.entries {
            if ui
                .selectable_label(false, format!("{artist}  ({count})"))
                .clicked()
            {
                *screen = Screen::ArtistDetail(artist.clone());
            }
        }
    });
}

pub fn draw_detail(
    ui: &mut egui::Ui,
    library: &Arc<Library>,
    playback: &Arc<PlaybackController>,
    cache: &mut ArtistsCache,
    artist: &str,
    screen: &mut Screen,
) {
    if crate::ui::components::page_header::back_link(ui, "Artists") {
        *screen = Screen::ArtistsList;
    }
    crate::ui::components::page_header::page_header(ui, "Artist", artist);
    let songs = cache.detail_songs(library, artist);
    let cur_id = playback.snapshot().current_song.as_ref().map(|s| s.id);
    let row_h = 32.0;
    egui::ScrollArea::vertical().show_rows(ui, row_h, songs.len(), |ui, range| {
        for i in range {
            let s = &songs[i];
            let opts = RowOptions {
                highlighted: cur_id == Some(s.id),
                ..Default::default()
            };
            let act = draw_with_options(ui, s, i, opts);
            if act.clicked || act.play_clicked {
                playback.play_songs(songs.to_vec(), i, None);
            }
        }
    });
}
