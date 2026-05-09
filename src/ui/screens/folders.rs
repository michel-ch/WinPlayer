use crate::data::Library;
use crate::domain::{Screen, SortOption};
use crate::playback::PlaybackController;
use crate::ui::components::song_row::{draw_with_options, RowOptions};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

pub struct FoldersState {
    pub selected: Option<PathBuf>,
}

impl Default for FoldersState {
    fn default() -> Self { Self { selected: None } }
}

pub fn draw(
    ui: &mut egui::Ui,
    library: &Arc<Library>,
    playback: &Arc<PlaybackController>,
    state: &mut FoldersState,
    _screen: &mut Screen,
) {
    let songs = library.songs_snapshot();
    let mut by_folder: BTreeMap<PathBuf, usize> = BTreeMap::new();
    for s in &songs {
        if let Some(parent) = s.path.parent() {
            *by_folder.entry(parent.to_path_buf()).or_insert(0) += 1;
        }
    }

    if state.selected.is_none() {
        egui::ScrollArea::vertical().show(ui, |ui| {
            for (folder, count) in by_folder {
                let label = folder.file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| folder.to_string_lossy().into_owned());
                if ui.selectable_label(false, format!("{label}  ({count})")).clicked() {
                    state.selected = Some(folder);
                }
            }
        });
        return;
    }

    let folder = state.selected.clone().unwrap();
    if ui.button("\u{25C0} Folders").clicked() { state.selected = None; return; }
    ui.heading(folder.display().to_string());

    let mut songs: Vec<_> = songs.into_iter()
        .filter(|s| s.path.parent() == Some(&folder))
        .collect();
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
