use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastKind { Info, Warn, Error }

pub struct Toast {
    pub kind: ToastKind,
    pub message: String,
    pub created: Instant,
    pub ttl: Duration,
}

pub struct Toasts {
    items: Vec<Toast>,
}

impl Toasts {
    pub fn new() -> Self { Self { items: Vec::new() } }

    pub fn info(&mut self, msg: impl Into<String>) {
        self.push(ToastKind::Info, msg.into(), Duration::from_secs(4));
    }
    pub fn warn(&mut self, msg: impl Into<String>) {
        self.push(ToastKind::Warn, msg.into(), Duration::from_secs(6));
    }
    pub fn error(&mut self, msg: impl Into<String>) {
        self.push(ToastKind::Error, msg.into(), Duration::from_secs(9));
    }
    fn push(&mut self, kind: ToastKind, message: String, ttl: Duration) {
        self.items.push(Toast { kind, message, created: Instant::now(), ttl });
    }

    pub fn show(&mut self, ctx: &egui::Context) {
        let now = Instant::now();
        self.items.retain(|t| now.duration_since(t.created) < t.ttl);
        let mut to_remove: Vec<usize> = Vec::new();
        egui::Area::new("toasts".into())
            .anchor(egui::Align2::RIGHT_TOP, egui::vec2(-12.0, 12.0))
            .show(ctx, |ui| {
                ui.vertical(|ui| {
                    for (idx, t) in self.items.iter().enumerate() {
                        let color = match t.kind {
                            ToastKind::Info => egui::Color32::from_rgb(0x33, 0x55, 0x88),
                            ToastKind::Warn => egui::Color32::from_rgb(0x88, 0x66, 0x22),
                            ToastKind::Error => egui::Color32::from_rgb(0x88, 0x33, 0x33),
                        };
                        let frame = egui::Frame::popup(ui.style())
                            .fill(color)
                            .stroke(egui::Stroke::NONE);
                        frame.show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new(&t.message).color(egui::Color32::WHITE));
                                if ui.small_button("\u{2715}").clicked() { to_remove.push(idx); }
                            });
                        });
                        ui.add_space(4.0);
                    }
                });
            });
        for idx in to_remove.into_iter().rev() {
            if idx < self.items.len() { self.items.remove(idx); }
        }
    }
}

impl Default for Toasts {
    fn default() -> Self { Self::new() }
}
