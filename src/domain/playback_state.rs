use crate::domain::Song;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum RepeatMode {
    #[default]
    Off,
    All,
    One,
}

impl RepeatMode {
    pub fn next(self) -> Self {
        match self {
            RepeatMode::Off => RepeatMode::All,
            RepeatMode::All => RepeatMode::One,
            RepeatMode::One => RepeatMode::Off,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct PlaybackState {
    pub current_song: Option<Song>,
    pub is_playing: bool,
    pub current_position_ms: u64,
    pub duration_ms: u64,
    pub volume: f32,
    pub shuffle_enabled: bool,
    pub repeat_mode: RepeatMode,
}

impl PlaybackState {
    pub fn progress(&self) -> f32 {
        if self.duration_ms == 0 {
            0.0
        } else {
            (self.current_position_ms as f32 / self.duration_ms as f32).clamp(0.0, 1.0)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repeat_mode_cycles_off_all_one_off() {
        assert_eq!(RepeatMode::Off.next(), RepeatMode::All);
        assert_eq!(RepeatMode::All.next(), RepeatMode::One);
        assert_eq!(RepeatMode::One.next(), RepeatMode::Off);
    }

    #[test]
    fn progress_clamps_and_handles_zero_duration() {
        let mut s = PlaybackState::default();
        assert_eq!(s.progress(), 0.0);
        s.duration_ms = 1000;
        s.current_position_ms = 500;
        assert!((s.progress() - 0.5).abs() < 1e-6);
        s.current_position_ms = 5000;
        assert_eq!(s.progress(), 1.0);
    }
}
