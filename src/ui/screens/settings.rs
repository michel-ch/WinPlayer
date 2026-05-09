use crate::data::Library;
use crate::domain::Screen;
use crate::settings::Settings;
use crate::ui::toasts::Toasts;
use parking_lot::RwLock;
use std::path::PathBuf;
use std::sync::Arc;

pub fn draw(
    ui: &mut egui::Ui,
    settings: &Arc<RwLock<Settings>>,
    library: &Arc<Library>,
    toasts: &mut Toasts,
    screen: &mut Screen,
) {
    if ui.button("\u{25C0} Back").clicked() { *screen = Screen::AllSongs; }
    ui.heading("Settings");

    let mut s = settings.write();

    ui.collapsing("Library paths", |ui| {
        let mut to_remove: Option<usize> = None;
        for (i, root) in s.scan.roots.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                let mut buf = root.to_string_lossy().into_owned();
                if ui.add(egui::TextEdit::singleline(&mut buf).desired_width(400.0)).changed() {
                    *root = PathBuf::from(buf);
                }
                if ui.small_button("\u{2715}").clicked() { to_remove = Some(i); }
            });
        }
        if let Some(i) = to_remove { s.scan.roots.remove(i); }
        if ui.button("+ Add path").clicked() { s.scan.roots.push(PathBuf::new()); }

        ui.label("Source library (read-only catalog used by other tools):");
        let mut src = s.scan.source_root.clone().unwrap_or_default().to_string_lossy().into_owned();
        if ui.add(egui::TextEdit::singleline(&mut src).desired_width(400.0)).changed() {
            s.scan.source_root = if src.is_empty() { None } else { Some(PathBuf::from(src)) };
        }
    });

    ui.collapsing("Playback", |ui| {
        ui.add(egui::Slider::new(&mut s.playback.volume, 0.0..=1.0).text("Volume"));
    });

    ui.collapsing("Renumber", |ui| {
        ui.checkbox(&mut s.renumber.enabled, "Renumber on delete");
        ui.add(egui::Slider::new(&mut s.renumber.threshold, 0.0..=1.0).text("Threshold"));
    });

    ui.add_space(12.0);
    if ui.button("Save").clicked() {
        let snap = s.clone();
        drop(s);
        match snap.save() {
            Ok(_) => toasts.info("Settings saved"),
            Err(e) => toasts.error(format!("Save failed: {e}")),
        }
        let lib = library.clone();
        let roots = settings.read().scan.roots.clone();
        std::thread::spawn(move || lib.scan(&roots));
    }
}
