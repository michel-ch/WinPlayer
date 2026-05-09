use crate::data::Library;
use crate::domain::{sort_songs, Screen, Song, SortOption};
use crate::playback::{delete_song, PlaybackController};
use crate::ui::components::song_row::{draw_with_options, RowOptions};
use crate::ui::toasts::Toasts;
use std::sync::Arc;

const PAGE_SIZE: usize = 50;

pub struct AllSongsState {
    pub sort: SortOption,
    pub query: String,
    pub page: usize,
    cache_key: Option<(u64, SortOption, String)>,
    cached: Vec<Song>,
}

impl Default for AllSongsState {
    fn default() -> Self {
        Self {
            sort: SortOption::TitleAsc,
            query: String::new(),
            page: 0,
            cache_key: None,
            cached: Vec::new(),
        }
    }
}

impl AllSongsState {
    fn compute_view(&mut self, library: &Arc<Library>) {
        let key = (library.version(), self.sort, self.query.clone());
        if self.cache_key.as_ref() == Some(&key) { return; }
        let mut songs = library.songs_snapshot();
        let q = self.query.to_lowercase();
        if !q.is_empty() {
            songs.retain(|s|
                s.title.to_lowercase().contains(&q) ||
                s.artist.to_lowercase().contains(&q) ||
                s.album.to_lowercase().contains(&q)
            );
        }
        sort_songs(&mut songs, self.sort);
        self.cached = songs;
        self.cache_key = Some(key);
        if self.page * PAGE_SIZE >= self.cached.len() && self.page > 0 { self.page = 0; }
    }
}

pub fn draw(
    ui: &mut egui::Ui,
    library: &Arc<Library>,
    playback: &Arc<PlaybackController>,
    state: &mut AllSongsState,
    toasts: &mut Toasts,
    renumber_threshold: f32,
    _screen: &mut Screen,
) {
    ui.horizontal(|ui| {
        let mut new_sort = state.sort;
        egui::ComboBox::from_label("Sort")
            .selected_text(state.sort.label())
            .show_ui(ui, |ui| {
                for opt in SortOption::ALL {
                    ui.selectable_value(&mut new_sort, opt, opt.label());
                }
            });
        if new_sort != state.sort {
            state.sort = new_sort;
            state.cache_key = None;
        }
        if ui.add(egui::TextEdit::singleline(&mut state.query).hint_text("Search\u{2026}")).changed() {
            state.cache_key = None;
        }
    });

    state.compute_view(library);
    let total = state.cached.len();
    let pages = (total + PAGE_SIZE - 1) / PAGE_SIZE.max(1);
    let start = state.page * PAGE_SIZE;
    let end = (start + PAGE_SIZE).min(total);

    ui.label(format!("{} songs   page {}/{}", total, state.page + 1, pages.max(1)));
    ui.horizontal(|ui| {
        if ui.button("\u{25C0} Prev").clicked() && state.page > 0 { state.page -= 1; }
        if ui.button("Next \u{25B6}").clicked() && state.page + 1 < pages { state.page += 1; }
    });
    ui.separator();

    let row_h = 32.0;
    let visible_owned: Vec<Song> = state.cached[start..end].to_vec();
    let cur_id = playback.snapshot().current_song.as_ref().map(|s| s.id);

    egui::ScrollArea::vertical().show_rows(ui, row_h, visible_owned.len(), |ui, range| {
        for i in range {
            let song = &visible_owned[i];
            let opts = RowOptions {
                highlighted: cur_id == Some(song.id),
                ..Default::default()
            };
            let action = draw_with_options(ui, song, start + i, opts);
            if action.clicked || action.play_clicked {
                playback.play_songs(state.cached.clone(), start + i, None);
            }
            if action.remove_clicked {
                let id = song.id;
                let lib = library.clone();
                match delete_song(&lib, id, true, renumber_threshold) {
                    Ok(res) => toasts.info(format!(
                        "Deleted {} ({} renumbered)",
                        res.deleted_path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
                        res.renumbered,
                    )),
                    Err(e) => toasts.error(format!("Delete failed: {e}")),
                }
                state.cache_key = None;
                playback.remove_from_queue(id);
            }
        }
    });
}
