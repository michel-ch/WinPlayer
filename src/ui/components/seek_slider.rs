use crate::domain::PlaybackState;

const MEM_ID: &str = "winplayer.seek.draft";

pub fn draw_seek_slider(ui: &mut egui::Ui, state: &PlaybackState) -> Option<f32> {
    let id = egui::Id::new(MEM_ID);

    let mut frac: f32 = ui.memory(|m| m.data.get_temp::<f32>(id).unwrap_or(state.progress()));
    let was_dragged_before = ui.memory(|m| m.data.get_temp::<bool>(id.with("was_dragged")).unwrap_or(false));

    let response = ui.add(
        egui::Slider::new(&mut frac, 0.0..=1.0)
            .show_value(false)
            .clamping(egui::SliderClamping::Always)
    );

    if response.dragged() {
        ui.memory_mut(|m| {
            m.data.insert_temp(id, frac);
            m.data.insert_temp(id.with("was_dragged"), true);
        });
        return None;
    }

    let committed = response.drag_stopped() || response.lost_focus()
        || (response.changed() && !response.dragged())
        || was_dragged_before;

    if committed {
        ui.memory_mut(|m| {
            m.data.remove_temp::<f32>(id);
            m.data.remove_temp::<bool>(id.with("was_dragged"));
        });
        return Some(frac.clamp(0.0, 1.0));
    }

    None
}

pub fn fmt_time_ms(ms: u64) -> String {
    let total_secs = ms / 1000;
    let m = total_secs / 60;
    let s = total_secs % 60;
    format!("{:01}:{:02}", m, s)
}
