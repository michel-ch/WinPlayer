use crate::domain::Screen;
use crate::engine::eq::BAND_FREQS_HZ;
use crate::settings::Settings;
use parking_lot::RwLock;
use std::sync::Arc;

pub fn draw(ui: &mut egui::Ui, settings: &Arc<RwLock<Settings>>, screen: &mut Screen) {
    if ui.button("\u{25C0} Back").clicked() { *screen = Screen::AllSongs; }
    ui.heading("Equalizer");

    let mut s = settings.write();
    let mut enabled = s.equalizer.enabled;
    if ui.checkbox(&mut enabled, "Enabled").changed() {
        s.equalizer.enabled = enabled;
    }

    ui.add_space(8.0);
    ui.horizontal(|ui| {
        for (i, freq) in BAND_FREQS_HZ.iter().enumerate() {
            ui.vertical(|ui| {
                ui.label(format!("{:.0}{}",
                    if *freq < 1000.0 { *freq } else { *freq / 1000.0 },
                    if *freq < 1000.0 { "Hz" } else { "kHz" },
                ));
                let mut val = s.equalizer.bands[i];
                if ui.add(egui::Slider::new(&mut val, -12.0..=12.0).vertical().show_value(false)).changed() {
                    s.equalizer.bands[i] = val;
                }
                ui.label(format!("{:+.1} dB", val));
            });
        }
    });

    ui.add_space(12.0);
    if ui.button("Reset to flat").clicked() {
        s.equalizer.bands = [0.0; 10];
    }
    if ui.button("Save").clicked() {
        let snapshot = s.clone();
        drop(s);
        if let Err(e) = snapshot.save() {
            log::error!("save settings: {}", e);
        }
    }
}
