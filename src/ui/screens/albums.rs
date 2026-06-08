use crate::data::Library;
use crate::domain::{Screen, Song, SortOption};
use crate::playback::PlaybackController;
use crate::ui::components::song_row::{draw_with_options, RowOptions};
use std::sync::Arc;

#[derive(Default)]
pub struct AlbumsCache {
    version: Option<u64>,
    entries: Vec<(String, usize)>,
    detail_version: Option<u64>,
    detail_album: Option<String>,
    detail_songs: Vec<Song>,
}

impl AlbumsCache {
    fn refresh(&mut self, library: &Arc<Library>) {
        let v = library.version();
        if self.version == Some(v) {
            return;
        }
        let songs = library.songs_snapshot();
        let mut by_album: std::collections::BTreeMap<String, usize> = Default::default();
        for s in &songs {
            *by_album.entry(s.album.clone()).or_insert(0) += 1;
        }
        self.entries = by_album.into_iter().collect();
        self.version = Some(v);
    }

    fn detail_songs(&mut self, library: &Arc<Library>, album: &str) -> &[Song] {
        let v = library.version();
        if self.detail_version != Some(v) || self.detail_album.as_deref() != Some(album) {
            let mut songs = library.songs_snapshot();
            songs.retain(|s| s.album == album);
            crate::domain::sort_songs(&mut songs, SortOption::TrackNoAsc);
            self.detail_songs = songs;
            self.detail_album = Some(album.to_owned());
            self.detail_version = Some(v);
        }
        &self.detail_songs
    }
}

pub fn draw_list(
    ui: &mut egui::Ui,
    library: &Arc<Library>,
    cache: &mut AlbumsCache,
    screen: &mut Screen,
) {
    crate::ui::components::page_header::page_header(ui, "Library", "Albums");
    cache.refresh(library);
    egui::ScrollArea::vertical().show(ui, |ui| {
        for (album, count) in &cache.entries {
            if ui
                .selectable_label(false, format!("{album}  ({count})"))
                .clicked()
            {
                *screen = Screen::AlbumDetail(album.clone());
            }
        }
    });
}

pub fn draw_detail(
    ui: &mut egui::Ui,
    library: &Arc<Library>,
    playback: &Arc<PlaybackController>,
    cache: &mut AlbumsCache,
    album: &str,
    screen: &mut Screen,
) {
    if crate::ui::components::page_header::back_link(ui, "Albums") {
        *screen = Screen::AlbumsList;
    }
    crate::ui::components::page_header::page_header(ui, "Album", album);

    let songs = cache.detail_songs(library, album);
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
