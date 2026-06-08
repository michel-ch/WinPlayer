use crate::domain::{PlaybackState, RepeatMode, Screen};
use crate::playback::PlaybackController;
use crate::ui::components::seek_slider::{draw_seek_slider, fmt_time_ms};
use crate::ui::theme;
use egui::{Color32, FontFamily, FontId, RichText};
use std::sync::Arc;

pub fn draw(ui: &mut egui::Ui, playback: &Arc<PlaybackController>, screen: &mut Screen) {
    let snapshot = playback.snapshot();
    draw_with_state(ui, &snapshot, playback, screen);
}

pub fn draw_with_state(
    ui: &mut egui::Ui,
    state: &PlaybackState,
    playback: &Arc<PlaybackController>,
    screen: &mut Screen,
) {
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        ui.add_space(8.0);
        if ui
            .add(
                egui::Button::new(
                    RichText::new("\u{2190}  Back")
                        .color(theme::TEXT_2)
                        .size(13.0),
                )
                .frame(false),
            )
            .clicked()
        {
            *screen = Screen::AllSongs;
        }
    });

    ui.vertical_centered(|ui| {
        ui.add_space(32.0);

        ui.label(
            RichText::new("NOW PLAYING")
                .font(FontId::new(11.0, FontFamily::Proportional))
                .color(theme::TEXT_3)
                .strong(),
        );
        ui.add_space(20.0);

        if let Some(s) = &state.current_song {
            ui.label(
                RichText::new(&s.title)
                    .font(FontId::new(54.0, theme::serif()))
                    .color(theme::TEXT),
            );
            ui.add_space(8.0);
            ui.label(
                RichText::new(&s.artist)
                    .font(FontId::new(16.0, FontFamily::Proportional))
                    .color(theme::ACCENT),
            );
            ui.label(
                RichText::new(&s.album)
                    .font(FontId::new(13.0, FontFamily::Proportional))
                    .color(theme::TEXT_3),
            );
        } else {
            ui.label(
                RichText::new("Nothing playing")
                    .font(FontId::new(40.0, theme::serif()))
                    .color(theme::TEXT_2),
            );
            return;
        }

        ui.add_space(40.0);

        ui.allocate_ui_with_layout(
            egui::vec2(ui.available_width().min(640.0), 50.0),
            egui::Layout::top_down(egui::Align::Center),
            |ui| {
                if let Some(t) = draw_seek_slider(ui, state) {
                    playback.seek_fraction(t);
                }
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(fmt_time_ms(state.current_position_ms))
                            .monospace()
                            .color(theme::TEXT_3)
                            .small(),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            RichText::new(fmt_time_ms(state.duration_ms))
                                .monospace()
                                .color(theme::TEXT_3)
                                .small(),
                        );
                    });
                });
            },
        );

        ui.add_space(28.0);

        ui.horizontal(|ui| {
            let center_w = 240.0;
            ui.add_space((ui.available_width() - center_w).max(0.0) / 2.0);

            if circular_button(ui, "\u{23EE}", 44.0, theme::SURFACE, theme::TEXT, false).clicked() {
                playback.previous();
            }
            ui.add_space(12.0);

            let pp = if state.is_playing {
                "\u{23F8}"
            } else {
                "\u{25B6}"
            };
            if circular_button(ui, pp, 64.0, theme::ACCENT, theme::ACCENT_INK, true).clicked() {
                playback.play_pause();
            }
            ui.add_space(12.0);

            if circular_button(ui, "\u{23ED}", 44.0, theme::SURFACE, theme::TEXT, false).clicked() {
                playback.next();
            }
        });

        ui.add_space(24.0);

        ui.horizontal(|ui| {
            let center_w = 220.0;
            ui.add_space((ui.available_width() - center_w).max(0.0) / 2.0);

            let shuffle_color = if state.shuffle_enabled {
                theme::ACCENT
            } else {
                theme::TEXT_3
            };
            if ui
                .add(
                    egui::Button::new(
                        RichText::new("\u{1F500} Shuffle")
                            .color(shuffle_color)
                            .size(12.0),
                    )
                    .frame(false),
                )
                .clicked()
            {
                playback.set_shuffle(!state.shuffle_enabled);
            }

            ui.add_space(20.0);

            let (rep_text, rep_color) = match state.repeat_mode {
                RepeatMode::Off => ("\u{1F501} Repeat", theme::TEXT_3),
                RepeatMode::All => ("\u{1F501} Repeat all", theme::ACCENT),
                RepeatMode::One => ("\u{1F502} Repeat one", theme::ACCENT),
            };
            if ui
                .add(
                    egui::Button::new(RichText::new(rep_text).color(rep_color).size(12.0))
                        .frame(false),
                )
                .clicked()
            {
                playback.cycle_repeat();
            }
        });
    });
}

fn circular_button(
    ui: &mut egui::Ui,
    glyph: &str,
    diameter: f32,
    fill: Color32,
    fg: Color32,
    primary: bool,
) -> egui::Response {
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(diameter, diameter), egui::Sense::click());
    let painter = ui.painter();
    let bg = if response.hovered() {
        if primary {
            let [r, g, b, _] = fill.to_array();
            Color32::from_rgb(
                (r as f32 * 0.94) as u8,
                (g as f32 * 0.94) as u8,
                (b as f32 * 0.94) as u8,
            )
        } else {
            theme::HOVER
        }
    } else {
        fill
    };
    painter.circle_filled(rect.center(), diameter * 0.5, bg);
    if !primary {
        painter.circle_stroke(
            rect.center(),
            diameter * 0.5,
            egui::Stroke::new(1.0, theme::BORDER_SOFT),
        );
    }
    let glyph_size = diameter * 0.45;
    painter.text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        glyph,
        FontId::new(glyph_size, FontFamily::Proportional),
        fg,
    );
    response
}
