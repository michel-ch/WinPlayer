use crate::domain::Screen;

pub fn draw(ui: &mut egui::Ui, current: &mut Screen, song_count: usize, search: &mut String) {
    ui.horizontal(|ui| {
        ui.heading("WinPlayer");
        ui.separator();
        if ui.selectable_label(matches!(current, Screen::AllSongs), "Songs").clicked() {
            *current = Screen::AllSongs;
        }
        if ui.selectable_label(matches!(current, Screen::AlbumsList | Screen::AlbumDetail(_)), "Albums").clicked() {
            *current = Screen::AlbumsList;
        }
        if ui.selectable_label(matches!(current, Screen::ArtistsList | Screen::ArtistDetail(_)), "Artists").clicked() {
            *current = Screen::ArtistsList;
        }
        if ui.selectable_label(matches!(current, Screen::Folders), "Folders").clicked() {
            *current = Screen::Folders;
        }
        if ui.selectable_label(matches!(current, Screen::Queue), "Queue").clicked() {
            *current = Screen::Queue;
        }
        if ui.selectable_label(matches!(current, Screen::Equalizer), "EQ").clicked() {
            *current = Screen::Equalizer;
        }
        if ui.selectable_label(matches!(current, Screen::Settings), "Settings").clicked() {
            *current = Screen::Settings;
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(format!("{} songs", song_count));
            ui.separator();
            ui.add(egui::TextEdit::singleline(search).hint_text("Search\u{2026}").desired_width(200.0));
        });
    });
}
