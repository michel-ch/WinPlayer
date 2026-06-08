use crate::data::Library;
use crate::domain::{Screen, Song, SortOption};
use crate::playback::PlaybackController;
use crate::ui::components::song_row::{draw_with_options, RowOptions};
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Default)]
pub struct FoldersState {
    pub selected: Option<PathBuf>,
    version: Option<u64>,
    folder_list: Vec<(PathBuf, usize)>,
    detail_version: Option<u64>,
    detail_folder: Option<PathBuf>,
    detail_songs: Vec<Song>,
}

impl FoldersState {
    fn refresh(&mut self, library: &Arc<Library>) {
        let v = library.version();
        if self.version == Some(v) {
            return;
        }
        let songs = library.songs_snapshot();
        let mut by_folder: std::collections::BTreeMap<PathBuf, usize> = Default::default();
        for s in &songs {
            if let Some(parent) = s.path.parent() {
                *by_folder.entry(parent.to_path_buf()).or_insert(0) += 1;
            }
        }
        self.folder_list = by_folder.into_iter().collect();
        self.version = Some(v);
    }

    fn detail_songs(&mut self, library: &Arc<Library>, folder: &PathBuf) -> &[Song] {
        let v = library.version();
        if self.detail_version != Some(v) || self.detail_folder.as_ref() != Some(folder) {
            let mut songs: Vec<_> = library
                .songs_snapshot()
                .into_iter()
                .filter(|s| s.path.parent() == Some(folder.as_path()))
                .collect();
            crate::domain::sort_songs(&mut songs, SortOption::TrackNoAsc);
            self.detail_songs = songs;
            self.detail_folder = Some(folder.clone());
            self.detail_version = Some(v);
        }
        &self.detail_songs
    }
}

pub fn draw(
    ui: &mut egui::Ui,
    library: &Arc<Library>,
    playback: &Arc<PlaybackController>,
    state: &mut FoldersState,
    _screen: &mut Screen,
) {
    if state.selected.is_none() {
        crate::ui::components::page_header::page_header(ui, "Library", "Folders");
        state.refresh(library);
        egui::ScrollArea::vertical().show(ui, |ui| {
            let mut click: Option<PathBuf> = None;
            for (folder, count) in &state.folder_list {
                let label = folder
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| folder.to_string_lossy().into_owned());
                if ui
                    .selectable_label(false, format!("{label}  ({count})"))
                    .clicked()
                {
                    click = Some(folder.clone());
                }
            }
            if let Some(f) = click {
                state.selected = Some(f);
            }
        });
        return;
    }

    let folder = state.selected.clone().unwrap();
    if crate::ui::components::page_header::back_link(ui, "Folders") {
        state.selected = None;
        return;
    }
    let folder_name = folder
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| folder.to_string_lossy().into_owned());
    crate::ui::components::page_header::page_header(ui, "Folder", &folder_name);

    let songs = state.detail_songs(library, &folder);
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
