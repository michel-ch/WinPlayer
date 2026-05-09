use crate::data::Library;
use crate::domain::{Screen, SortOption};
use crate::playback::PlaybackController;
use crate::ui::components::song_row::{draw_with_options, RowOptions};
use std::collections::BTreeMap;
use std::sync::Arc;

pub fn draw_list(ui: &mut egui::Ui, library: &Arc<Library>, screen: &mut Screen) {
    let songs = library.songs_snapshot();
    let mut by_album: BTreeMap<String, usize> = BTreeMap::new();
    for s in &songs { *by_album.entry(s.album.clone()).or_insert(0) += 1; }

    egui::ScrollArea::vertical().show(ui, |ui| {
        for (album, count) in by_album {
            if ui.selectable_label(false, format!("{album}  ({count})")).clicked() {
                *screen = Screen::AlbumDetail(album);
            }
        }
    });
}

pub fn draw_detail(
    ui: &mut egui::Ui,
    library: &Arc<Library>,
    playback: &Arc<PlaybackController>,
    album: &str,
    screen: &mut Screen,
) {
    if ui.button("\u{25C0} Albums").clicked() { *screen = Screen::AlbumsList; }
    ui.heading(album);

    let mut songs = library.songs_snapshot();
    songs.retain(|s| s.album == album);
    crate::domain::sort_songs(&mut songs, SortOption::TrackNoAsc);

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
