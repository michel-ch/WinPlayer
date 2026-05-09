use crate::domain::{PlaybackState, RepeatMode, Screen};
use crate::playback::PlaybackController;
use crate::ui::components::seek_slider::{draw_seek_slider, fmt_time_ms};
use std::sync::Arc;

pub fn draw(
    ui: &mut egui::Ui,
    state: &PlaybackState,
    playback: &Arc<PlaybackController>,
    screen: &mut Screen,
) {
    ui.horizontal(|ui| {
        ui.set_height(56.0);

        ui.allocate_ui_with_layout(
            egui::vec2(220.0, 56.0),
            egui::Layout::top_down(egui::Align::LEFT),
            |ui| {
                if let Some(s) = &state.current_song {
                    ui.add(egui::Label::new(egui::RichText::new(&s.title).strong()).truncate());
                    ui.add(egui::Label::new(egui::RichText::new(&s.artist).weak()).truncate());
                } else {
                    ui.label("\u{2014}");
                }
            },
        );

        if ui.button("\u{23EE}").on_hover_text("Previous").clicked() { playback.previous(); }
        let pp = if state.is_playing { "\u{23F8}" } else { "\u{25B6}" };
        if ui.button(pp).on_hover_text("Play / Pause").clicked() { playback.play_pause(); }
        if ui.button("\u{23ED}").on_hover_text("Next").clicked() { playback.next(); }

        ui.allocate_ui_with_layout(
            egui::vec2((ui.available_width() - 240.0).max(80.0), 56.0),
            egui::Layout::top_down(egui::Align::Center),
            |ui| {
                if let Some(target) = draw_seek_slider(ui, state) {
                    playback.seek_fraction(target);
                }
                ui.horizontal(|ui| {
                    ui.label(fmt_time_ms(state.current_position_ms));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(fmt_time_ms(state.duration_ms));
                    });
                });
            },
        );

        let mut v = state.volume;
        if ui.add(egui::Slider::new(&mut v, 0.0..=1.0).show_value(false).text("vol")).changed() {
            playback.set_volume(v);
        }
        let shuffle_label = if state.shuffle_enabled { "\u{1F500}ON" } else { "\u{1F500}" };
        if ui.button(shuffle_label).on_hover_text("Shuffle").clicked() {
            playback.set_shuffle(!state.shuffle_enabled);
        }
        let repeat_label = match state.repeat_mode {
            RepeatMode::Off => "\u{1F501}",
            RepeatMode::All => "\u{1F501}ALL",
            RepeatMode::One => "\u{1F502}",
        };
        if ui.button(repeat_label).on_hover_text("Repeat").clicked() {
            playback.cycle_repeat();
        }
        if ui.button("\u{2922}").on_hover_text("Now Playing").clicked() {
            *screen = Screen::NowPlaying;
        }
    });
}
