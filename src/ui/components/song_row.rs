use crate::domain::Song;

#[derive(Debug, Clone, Copy)]
pub struct RowOptions {
    pub show_remove: bool,
    pub show_play: bool,
    pub highlighted: bool,
}

impl Default for RowOptions {
    fn default() -> Self {
        Self {
            show_remove: true,
            show_play: true,
            highlighted: false,
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct RowAction {
    pub clicked: bool,
    pub play_clicked: bool,
    pub remove_clicked: bool,
}

impl RowAction {
    fn from_hits(row_clicked: bool, play_clicked: bool, remove_clicked: bool) -> Self {
        if remove_clicked {
            Self {
                remove_clicked: true,
                ..Default::default()
            }
        } else if play_clicked {
            Self {
                play_clicked: true,
                ..Default::default()
            }
        } else {
            Self {
                clicked: row_clicked,
                ..Default::default()
            }
        }
    }
}

const ROW_HEIGHT: f32 = 32.0;

pub fn draw_with_options(
    ui: &mut egui::Ui,
    song: &Song,
    row_index: usize,
    opts: RowOptions,
) -> RowAction {
    let avail_w = ui.available_width();
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(avail_w, ROW_HEIGHT), egui::Sense::click());

    // Clip the row background to the row's own rect intersected with the
    // parent ui's clip rect. Using a row-scoped painter guarantees we can't
    // bleed into the mini-player panel below when the scroll viewport ends
    // mid-row.
    let painter = ui.painter_at(rect);
    if opts.highlighted {
        painter.rect_filled(rect, 4.0, crate::ui::theme::ACCENT_SOFT);
    } else if response.hovered() {
        painter.rect_filled(rect, 4.0, crate::ui::theme::HOVER);
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
        |ui| {
            ui.label(format!("{}", row_index + 1));
        },
    );

    let play_clicked = opts.show_play && child.small_button("\u{25B6}").clicked();

    child.add(egui::Label::new(&song.title).truncate());

    let remove_clicked = child
        .with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let remove_clicked = opts.show_remove
                && ui
                    .small_button("\u{2715}")
                    .on_hover_text("Remove")
                    .clicked();
            ui.add_space(8.0);
            let dur_secs = song.duration.as_secs();
            ui.label(format!("{}:{:02}", dur_secs / 60, dur_secs % 60));
            ui.add_space(8.0);
            ui.label("\u{00B7}");
            ui.add_space(8.0);
            ui.add(egui::Label::new(&song.artist).truncate());
            remove_clicked
        })
        .inner;

    RowAction::from_hits(response.clicked(), play_clicked, remove_clicked)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn child_clicks_take_precedence_over_row_activation() {
        let play = RowAction::from_hits(true, true, false);
        assert!(!play.clicked);
        assert!(play.play_clicked);
        assert!(!play.remove_clicked);

        let remove = RowAction::from_hits(true, false, true);
        assert!(!remove.clicked);
        assert!(!remove.play_clicked);
        assert!(remove.remove_clicked);
    }
}
