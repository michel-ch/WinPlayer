use crate::domain::Screen;
use crate::domain::Song;
use crate::playback::PlaybackController;
use crate::ui::components::song_row::{draw_with_options, RowOptions};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

#[derive(Default)]
pub struct QueueState {
    fingerprint: u64,
    current: Option<usize>,
    songs: Vec<Song>,
}

impl QueueState {
    fn sync(&mut self, playback: &Arc<PlaybackController>) {
        let q = playback.queue();
        let r = q.read();
        let fingerprint = queue_fingerprint(&r.songs);
        if self.fingerprint != fingerprint {
            self.songs = r.songs.clone();
            self.fingerprint = fingerprint;
        }
        self.current = r.current;
    }
}

fn queue_fingerprint(songs: &[Song]) -> u64 {
    let mut hasher = DefaultHasher::new();
    songs.len().hash(&mut hasher);
    for song in songs {
        song.id.hash(&mut hasher);
    }
    hasher.finish()
}

pub fn draw(
    ui: &mut egui::Ui,
    playback: &Arc<PlaybackController>,
    state: &mut QueueState,
    _screen: &mut Screen,
) {
    state.sync(playback);
    let songs = &state.songs;
    let cur = state.current;
    let count_label = format!("{} songs", songs.len());
    crate::ui::components::page_header::page_header(ui, &count_label, "Queue");
    let row_h = 32.0;
    egui::ScrollArea::vertical().show_rows(ui, row_h, songs.len(), |ui, range| {
        for i in range {
            let s = &songs[i];
            let opts = RowOptions {
                highlighted: cur == Some(i),
                ..Default::default()
            };
            let act = draw_with_options(ui, s, i, opts);
            if act.clicked || act.play_clicked {
                playback.jump_to(i);
            }
            if act.remove_clicked {
                playback.remove_from_queue(s.id);
                state.fingerprint = 0;
            }
        }
    });
}
