use crate::data::{Library, LibraryStatus};
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
    if crate::ui::components::page_header::back_link(ui, "Library") {
        *screen = Screen::AllSongs;
    }
    crate::ui::components::page_header::page_header(ui, "Preferences", "Settings");

    let snapshot = settings.read().clone();
    let mut edited = snapshot.clone();
    let mut dirty = false;
    let mut save_clicked = false;

    ui.collapsing("Library paths", |ui| {
        let mut to_remove: Option<usize> = None;
        for (i, root) in edited.scan.roots.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                let mut buf = root.to_string_lossy().into_owned();
                if ui
                    .add(egui::TextEdit::singleline(&mut buf).desired_width(400.0))
                    .changed()
                {
                    *root = PathBuf::from(buf);
                    dirty = true;
                }
                if ui.small_button("\u{2715}").clicked() {
                    to_remove = Some(i);
                }
            });
        }
        if let Some(i) = to_remove {
            edited.scan.roots.remove(i);
            dirty = true;
        }
        if ui.button("+ Add path").clicked() {
            edited.scan.roots.push(PathBuf::new());
            dirty = true;
        }
    });

    ui.collapsing("Playback", |ui| {
        if ui
            .add(egui::Slider::new(&mut edited.playback.volume, 0.0..=1.0).text("Volume"))
            .changed()
        {
            dirty = true;
        }
    });

    ui.collapsing("Renumber", |ui| {
        if ui
            .checkbox(&mut edited.renumber.enabled, "Renumber on delete")
            .changed()
        {
            dirty = true;
        }
        if ui
            .add(egui::Slider::new(&mut edited.renumber.threshold, 0.0..=1.0).text("Threshold"))
            .changed()
        {
            dirty = true;
        }
    });

    ui.add_space(12.0);
    if ui.button("Save").clicked() {
        save_clicked = true;
    }

    if dirty {
        *settings.write() = edited.clone();
    }

    if save_clicked {
        let snap = if dirty {
            edited
        } else {
            settings.read().clone()
        };
        match snap.save() {
            Ok(_) => toasts.info("Settings saved"),
            Err(e) => toasts.error(format!("Save failed: {e}")),
        }
        // Skip rescan if one is already running — otherwise a double-click
        // launches two parallel scans on the same library.
        if library.status() != LibraryStatus::Scanning {
            let lib = library.clone();
            let roots = snap.scan.roots.clone();
            std::thread::Builder::new()
                .name("library-rescan".into())
                .spawn(move || {
                    lib.scan(&roots);
                })
                .ok();
        } else {
            toasts.warn("Scan already in progress; new paths will be picked up after it finishes");
        }
    }
}
