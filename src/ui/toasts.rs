use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastKind {
    Info,
    Warn,
    Error,
}

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
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }

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
        self.items.push(Toast {
            kind,
            message,
            created: Instant::now(),
            ttl,
        });
    }

    pub fn has_active(&self) -> bool {
        !self.items.is_empty()
    }

    pub fn show(&mut self, ctx: &egui::Context) {
        let now = Instant::now();
        self.items.retain(|t| now.duration_since(t.created) < t.ttl);
        if self.items.is_empty() {
            return;
        }
        if let Some(next_expiry) = self
            .items
            .iter()
            .map(|t| t.ttl.saturating_sub(now.duration_since(t.created)))
            .min()
        {
            ctx.request_repaint_after(next_expiry);
        }
        let mut to_remove: Vec<usize> = Vec::new();
        let id = egui::Id::new("winplayer.toasts");
        egui::Area::new(id)
            .anchor(egui::Align2::RIGHT_TOP, egui::vec2(-12.0, 12.0))
            .show(ctx, |ui| {
                ui.vertical(|ui| {
                    for (idx, t) in self.items.iter().enumerate() {
                        let (fill, fg) = match t.kind {
                            ToastKind::Info => (crate::ui::theme::SURFACE, crate::ui::theme::TEXT),
                            ToastKind::Warn => {
                                (crate::ui::theme::ACCENT_SOFT, crate::ui::theme::TEXT)
                            }
                            ToastKind::Error => {
                                (crate::ui::theme::ACCENT, crate::ui::theme::ACCENT_INK)
                            }
                        };
                        let frame = egui::Frame::popup(ui.style())
                            .fill(fill)
                            .stroke(egui::Stroke::new(1.0, crate::ui::theme::BORDER_SOFT));
                        frame.show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new(&t.message).color(fg));
                                if ui.small_button("\u{2715}").clicked() {
                                    to_remove.push(idx);
                                }
                            });
                        });
                        ui.add_space(4.0);
                    }
                });
            });
        for idx in to_remove.into_iter().rev() {
            if idx < self.items.len() {
                self.items.remove(idx);
            }
        }
    }
}

impl Default for Toasts {
    fn default() -> Self {
        Self::new()
    }
}
