use crate::data::Library;
use crate::domain::{Screen, SortOption};
use crate::playback::PlaybackController;
use crate::ui::components::song_row::{draw_with_options, RowOptions};
use std::collections::BTreeMap;
use std::sync::Arc;

pub fn draw_list(ui: &mut egui::Ui, library: &Arc<Library>, screen: &mut Screen) {
    let songs = library.songs_snapshot();
    let mut by_artist: BTreeMap<String, usize> = BTreeMap::new();
    for s in &songs { *by_artist.entry(s.artist.clone()).or_insert(0) += 1; }
    egui::ScrollArea::vertical().show(ui, |ui| {
        for (artist, count) in by_artist {
            if ui.selectable_label(false, format!("{artist}  ({count})")).clicked() {
                *screen = Screen::ArtistDetail(artist);
            }
        }
    });
}

pub fn draw_detail(
    ui: &mut egui::Ui,
    library: &Arc<Library>,
    playback: &Arc<PlaybackController>,
    artist: &str,
    screen: &mut Screen,
) {
    if ui.button("\u{25C0} Artists").clicked() { *screen = Screen::ArtistsList; }
    ui.heading(artist);
    let mut songs = library.songs_snapshot();
    songs.retain(|s| s.artist == artist);
    crate::domain::sort_songs(&mut songs, SortOption::AlbumAsc);
    let cur_id = playback.snapshot().current_song.as_ref().map(|s| s.id);
    let row_h = 32.0;
    egui::ScrollArea::vertical().show_rows(ui, row_h, songs.len(), |ui, range| {
        for i in range {
            let s = &songs[i];
            let opts = RowOptions { highlighted: cur_id == Some(s.id), ..Default::default() };
            let act = draw_with_options(ui, s, i, opts);
            if act.clicked || act.play_clicked {
                playback.play_songs(songs.clone(), i, None);
            }
        }
    });
}
