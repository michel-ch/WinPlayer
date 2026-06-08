use crate::ui::theme;
use egui::{FontFamily, FontId, RichText};

pub fn page_header(ui: &mut egui::Ui, crumb: &str, title: &str) {
    ui.add_space(20.0);
    ui.horizontal(|ui| {
        ui.add_space(8.0);
        ui.vertical(|ui| {
            if !crumb.is_empty() {
                ui.label(
                    RichText::new(crumb.to_uppercase())
                        .font(FontId::new(10.0, FontFamily::Proportional))
                        .color(theme::TEXT_3)
                        .strong(),
                );
                ui.add_space(2.0);
            }
            ui.label(
                RichText::new(title)
                    .font(FontId::new(36.0, theme::serif()))
                    .color(theme::TEXT),
            );
        });
    });
    ui.add_space(8.0);
    ui.separator();
    ui.add_space(4.0);
}

pub fn back_link(ui: &mut egui::Ui, label: &str) -> bool {
    let text = RichText::new(format!("\u{2190}  {label}"))
        .color(theme::TEXT_2)
        .size(12.0);
    ui.add(egui::Button::new(text).frame(false)).clicked()
}
