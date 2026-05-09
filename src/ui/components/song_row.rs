use crate::domain::Song;

#[derive(Debug, Clone, Copy)]
pub struct RowOptions {
    pub show_remove: bool,
    pub show_play: bool,
    pub highlighted: bool,
}

impl Default for RowOptions {
    fn default() -> Self {
        Self { show_remove: true, show_play: true, highlighted: false }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct RowAction {
    pub clicked: bool,
    pub play_clicked: bool,
    pub remove_clicked: bool,
}

const ROW_HEIGHT: f32 = 32.0;

pub fn draw_with_options(
    ui: &mut egui::Ui,
    song: &Song,
    row_index: usize,
    opts: RowOptions,
) -> RowAction {
    let avail_w = ui.available_width();
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(avail_w, ROW_HEIGHT),
        egui::Sense::click(),
    );

    let mut action = RowAction::default();
    if response.clicked() { action.clicked = true; }

    if opts.highlighted {
        ui.painter().rect_filled(rect, 4.0, egui::Color32::from_rgba_unmultiplied(80, 130, 200, 50));
    } else if response.hovered() {
        ui.painter().rect_filled(rect, 4.0, egui::Color32::from_rgba_unmultiplied(255, 255, 255, 12));
    }

    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(rect)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    child.set_clip_rect(rect);
    child.add_space(4.0);

    child.allocate_ui_with_layout(
        egui::vec2(28.0, ROW_HEIGHT),
        egui::Layout::left_to_right(egui::Align::Center),
        |ui| { ui.label(format!("{}", row_index + 1)); },
    );

    if opts.show_play {
        if child.small_button("\u{25B6}").clicked() { action.play_clicked = true; }
    }

    child.add(egui::Label::new(&song.title).truncate());

    child.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        if opts.show_remove {
            if ui.small_button("\u{2715}").on_hover_text("Remove").clicked() {
                action.remove_clicked = true;
            }
        }
        ui.add_space(8.0);
        let dur_secs = song.duration.as_secs();
        ui.label(format!("{}:{:02}", dur_secs / 60, dur_secs % 60));
        ui.add_space(8.0);
        ui.label("\u{00B7}");
        ui.add_space(8.0);
        ui.add(egui::Label::new(&song.artist).truncate());
    });

    action
}
