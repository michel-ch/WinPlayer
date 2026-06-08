use crate::domain::Screen;
use crate::ui::theme;
use egui::{FontFamily, FontId, RichText};

fn nav(ui: &mut egui::Ui, label: &str, selected: bool) -> egui::Response {
    let text = RichText::new(label)
        .font(FontId::new(13.0, FontFamily::Proportional))
        .color(if selected { theme::TEXT } else { theme::TEXT_2 });
    ui.selectable_label(selected, text)
}

pub fn draw(ui: &mut egui::Ui, current: &mut Screen, song_count: usize, search: &mut String) {
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.add_space(8.0);

        ui.painter()
            .circle_filled(ui.cursor().min + egui::vec2(7.0, 11.0), 5.0, theme::ACCENT);
        ui.add_space(16.0);
        ui.label(
            RichText::new("WinPlayer")
                .font(FontId::new(20.0, theme::serif()))
                .color(theme::TEXT),
        );

        ui.add_space(20.0);

        if nav(ui, "Songs", matches!(current, Screen::AllSongs)).clicked() {
            *current = Screen::AllSongs;
        }
        if nav(
            ui,
            "Albums",
            matches!(current, Screen::AlbumsList | Screen::AlbumDetail(_)),
        )
        .clicked()
        {
            *current = Screen::AlbumsList;
        }
        if nav(
            ui,
            "Artists",
            matches!(current, Screen::ArtistsList | Screen::ArtistDetail(_)),
        )
        .clicked()
        {
            *current = Screen::ArtistsList;
        }
        if nav(ui, "Folders", matches!(current, Screen::Folders)).clicked() {
            *current = Screen::Folders;
        }
        if nav(ui, "Queue", matches!(current, Screen::Queue)).clicked() {
            *current = Screen::Queue;
        }
        if nav(ui, "EQ", matches!(current, Screen::Equalizer)).clicked() {
            *current = Screen::Equalizer;
        }
        if nav(ui, "Settings", matches!(current, Screen::Settings)).clicked() {
            *current = Screen::Settings;
        }

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add_space(8.0);
            ui.label(
                RichText::new(format!("{} songs", song_count))
                    .color(theme::TEXT_3)
                    .small(),
            );
            if matches!(current, Screen::AllSongs) {
                ui.add_space(12.0);
                ui.add(
                    egui::TextEdit::singleline(search)
                        .hint_text("Search\u{2026}")
                        .desired_width(220.0),
                );
            }
        });
    });
    ui.add_space(4.0);
}
