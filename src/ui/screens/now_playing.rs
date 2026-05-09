use crate::domain::{RepeatMode, Screen};
use crate::playback::PlaybackController;
use crate::ui::components::seek_slider::{draw_seek_slider, fmt_time_ms};
use std::sync::Arc;

pub fn draw(ui: &mut egui::Ui, playback: &Arc<PlaybackController>, screen: &mut Screen) {
    if ui.button("\u{25C0} Back").clicked() { *screen = Screen::AllSongs; }
    let state = playback.snapshot();
    ui.vertical_centered(|ui| {
        ui.add_space(20.0);
        if let Some(s) = &state.current_song {
            ui.heading(&s.title);
            ui.label(egui::RichText::new(&s.artist).size(18.0));
            ui.label(egui::RichText::new(&s.album).weak());
        } else {
            ui.heading("Nothing playing");
            return;
        }
        ui.add_space(40.0);
        ui.allocate_ui_with_layout(
            egui::vec2(ui.available_width().min(800.0), 60.0),
            egui::Layout::top_down(egui::Align::Center),
            |ui| {
                if let Some(t) = draw_seek_slider(ui, &state) {
                    playback.seek_fraction(t);
                }
                ui.horizontal(|ui| {
                    ui.label(fmt_time_ms(state.current_position_ms));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(fmt_time_ms(state.duration_ms));
                    });
                });
            },
        );
        ui.add_space(20.0);
        ui.horizontal(|ui| {
            ui.add_space(ui.available_width() / 2.0 - 90.0);
            if ui.button(egui::RichText::new("\u{23EE}").size(28.0)).clicked() { playback.previous(); }
            let pp = if state.is_playing { "\u{23F8}" } else { "\u{25B6}" };
            if ui.button(egui::RichText::new(pp).size(28.0)).clicked() { playback.play_pause(); }
            if ui.button(egui::RichText::new("\u{23ED}").size(28.0)).clicked() { playback.next(); }
        });
        ui.add_space(20.0);
        ui.horizontal(|ui| {
            ui.add_space(ui.available_width() / 2.0 - 90.0);
            let shuf = if state.shuffle_enabled { "\u{1F500} ON" } else { "\u{1F500} off" };
            if ui.button(shuf).clicked() { playback.set_shuffle(!state.shuffle_enabled); }
            let rep = match state.repeat_mode {
                RepeatMode::Off => "\u{1F501} off",
                RepeatMode::All => "\u{1F501} all",
                RepeatMode::One => "\u{1F502} one",
            };
            if ui.button(rep).clicked() { playback.cycle_repeat(); }
        });
    });
}
