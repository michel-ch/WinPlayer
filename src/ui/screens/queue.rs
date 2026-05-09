use crate::domain::Screen;
use crate::playback::PlaybackController;
use crate::ui::components::song_row::{draw_with_options, RowOptions};
use std::sync::Arc;

pub fn draw(ui: &mut egui::Ui, playback: &Arc<PlaybackController>, _screen: &mut Screen) {
    let q = playback.queue();
    let snapshot = {
        let r = q.read();
        (r.songs.clone(), r.current)
    };
    let (songs, cur) = snapshot;
    ui.heading(format!("Queue ({})", songs.len()));
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
            }
        }
    });
}
