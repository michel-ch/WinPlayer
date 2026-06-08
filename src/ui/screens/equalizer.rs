use crate::domain::Screen;
use crate::engine::eq::BAND_FREQS_HZ;
use crate::settings::Settings;
use crate::ui::toasts::Toasts;
use parking_lot::RwLock;
use std::sync::Arc;

pub fn draw(
    ui: &mut egui::Ui,
    settings: &Arc<RwLock<Settings>>,
    toasts: &mut Toasts,
    screen: &mut Screen,
) {
    if crate::ui::components::page_header::back_link(ui, "Library") {
        *screen = Screen::AllSongs;
    }
    crate::ui::components::page_header::page_header(ui, "Audio", "Equalizer");

    // Read a snapshot up front — `write()` is only taken when a field
    // actually changes. Holding the write lock across the whole frame
    // would block every other thread that wants to read settings.
    let snapshot = settings.read().equalizer.clone();
    let mut enabled = snapshot.enabled;
    let mut bands = snapshot.bands;

    let mut dirty = false;
    if ui.checkbox(&mut enabled, "Enabled").changed() {
        dirty = true;
    }

    ui.add_space(8.0);
    ui.horizontal(|ui| {
        for (i, freq) in BAND_FREQS_HZ.iter().enumerate() {
            ui.vertical(|ui| {
                ui.label(format!(
                    "{:.0}{}",
                    if *freq < 1000.0 {
                        *freq
                    } else {
                        *freq / 1000.0
                    },
                    if *freq < 1000.0 { "Hz" } else { "kHz" },
                ));
                let mut val = bands[i];
                if ui
                    .add(
                        egui::Slider::new(&mut val, -12.0..=12.0)
                            .vertical()
                            .show_value(false),
                    )
                    .changed()
                {
                    bands[i] = val;
                    dirty = true;
                }
                ui.label(format!("{:+.1} dB", val));
            });
        }
    });

    if dirty {
        let mut s = settings.write();
        s.equalizer.enabled = enabled;
        s.equalizer.bands = bands;
    }

    ui.add_space(12.0);
    if ui.button("Reset to flat").clicked() {
        settings.write().equalizer.bands = [0.0; 10];
    }
    if ui.button("Save").clicked() {
        let snap = settings.read().clone();
        match snap.save() {
            Ok(_) => toasts.info("Equalizer saved"),
            Err(e) => toasts.error(format!("Save failed: {e}")),
        }
    }
}
