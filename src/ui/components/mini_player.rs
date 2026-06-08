use crate::domain::{PlaybackState, RepeatMode, Screen};
use crate::playback::PlaybackController;
use crate::ui::components::seek_slider::{draw_seek_slider, fmt_time_ms};
use crate::ui::theme;
use egui::{Color32, FontFamily, FontId, RichText};
use std::sync::Arc;

pub fn draw(
    ui: &mut egui::Ui,
    state: &PlaybackState,
    playback: &Arc<PlaybackController>,
    screen: &mut Screen,
) {
    ui.horizontal(|ui| {
        ui.set_height(48.0);

        ui.allocate_ui_with_layout(
            egui::vec2(220.0, 48.0),
            egui::Layout::top_down(egui::Align::LEFT),
            |ui| {
                if let Some(s) = &state.current_song {
                    ui.add(
                        egui::Label::new(
                            RichText::new(&s.title)
                                .color(theme::TEXT)
                                .size(13.0)
                                .strong(),
                        )
                        .truncate(),
                    );
                    ui.add(
                        egui::Label::new(RichText::new(&s.artist).color(theme::TEXT_3).size(12.0))
                            .truncate(),
                    );
                } else {
                    ui.label(
                        RichText::new("Nothing playing")
                            .color(theme::TEXT_3)
                            .size(13.0),
                    );
                }
            },
        );

        ui.add_space(8.0);

        if mini_button(ui, "\u{23EE}", 28.0, "Previous").clicked() {
            playback.previous();
        }
        ui.add_space(4.0);
        let pp = if state.is_playing {
            "\u{23F8}"
        } else {
            "\u{25B6}"
        };
        let pp_label = if state.is_playing { "Pause" } else { "Play" };
        if mini_play(ui, pp, pp_label).clicked() {
            playback.play_pause();
        }
        ui.add_space(4.0);
        if mini_button(ui, "\u{23ED}", 28.0, "Next").clicked() {
            playback.next();
        }

        ui.add_space(12.0);

        ui.allocate_ui_with_layout(
            egui::vec2((ui.available_width() - 260.0).max(80.0), 48.0),
            egui::Layout::top_down(egui::Align::Center),
            |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(fmt_time_ms(state.current_position_ms))
                            .monospace()
                            .color(theme::TEXT_3)
                            .size(11.0),
                    );
                    ui.add_space(8.0);
                    if let Some(target) = draw_seek_slider(ui, state) {
                        playback.seek_fraction(target);
                    }
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new(fmt_time_ms(state.duration_ms))
                            .monospace()
                            .color(theme::TEXT_3)
                            .size(11.0),
                    );
                });
            },
        );

        let mut v = state.volume;
        ui.add_space(8.0);
        if ui
            .add(egui::Slider::new(&mut v, 0.0..=1.0).show_value(false))
            .on_hover_text("Volume")
            .changed()
        {
            playback.set_volume(v);
        }

        let shuffle_color = if state.shuffle_enabled {
            theme::ACCENT
        } else {
            theme::TEXT_3
        };
        if ui
            .add(
                egui::Button::new(RichText::new("\u{1F500}").color(shuffle_color).size(13.0))
                    .frame(false),
            )
            .on_hover_text("Shuffle")
            .clicked()
        {
            playback.set_shuffle(!state.shuffle_enabled);
        }

        let (rep_glyph, rep_color) = match state.repeat_mode {
            RepeatMode::Off => ("\u{1F501}", theme::TEXT_3),
            RepeatMode::All => ("\u{1F501}", theme::ACCENT),
            RepeatMode::One => ("\u{1F502}", theme::ACCENT),
        };
        if ui
            .add(
                egui::Button::new(RichText::new(rep_glyph).color(rep_color).size(13.0))
                    .frame(false),
            )
            .on_hover_text("Repeat")
            .clicked()
        {
            playback.cycle_repeat();
        }

        if ui
            .add(
                egui::Button::new(RichText::new("\u{2922}").color(theme::TEXT_2).size(13.0))
                    .frame(false),
            )
            .on_hover_text("Now Playing")
            .clicked()
        {
            *screen = Screen::NowPlaying;
        }
    });
}

fn mini_button(ui: &mut egui::Ui, glyph: &str, d: f32, label: &'static str) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(d, d), egui::Sense::click());
    if response.hovered() {
        ui.painter()
            .circle_filled(rect.center(), d * 0.5, theme::HOVER);
    }
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        glyph,
        FontId::new(d * 0.45, FontFamily::Proportional),
        theme::TEXT,
    );
    label_button(response, label)
}

fn mini_play(ui: &mut egui::Ui, glyph: &str, label: &'static str) -> egui::Response {
    let d = 34.0;
    let (rect, response) = ui.allocate_exact_size(egui::vec2(d, d), egui::Sense::click());
    let fill = if response.hovered() {
        darken(theme::ACCENT, 0.94)
    } else {
        theme::ACCENT
    };
    ui.painter().circle_filled(rect.center(), d * 0.5, fill);
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        glyph,
        FontId::new(d * 0.45, FontFamily::Proportional),
        theme::ACCENT_INK,
    );
    label_button(response, label)
}

fn label_button(response: egui::Response, label: &'static str) -> egui::Response {
    let response = response
        .on_hover_text(label)
        .on_disabled_hover_text(label)
        .on_hover_cursor(egui::CursorIcon::PointingHand);
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label));
    response
}

fn darken(c: Color32, factor: f32) -> Color32 {
    let [r, g, b, a] = c.to_array();
    Color32::from_rgba_unmultiplied(
        (r as f32 * factor) as u8,
        (g as f32 * factor) as u8,
        (b as f32 * factor) as u8,
        a,
    )
}
