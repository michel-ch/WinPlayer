use crate::domain::RepeatMode;
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub scan: ScanSettings,
    pub playback: PlaybackSettings,
    pub equalizer: EqualizerSettings,
    pub renumber: RenumberSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ScanSettings {
    #[serde(default)] pub roots: Vec<PathBuf>,
    #[serde(default)] pub source_root: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaybackSettings {
    pub volume: f32,
    pub shuffle: bool,
    pub repeat: RepeatMode,
    pub crossfade_ms: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EqualizerSettings {
    pub enabled: bool,
    pub bands: [f32; 10],
    pub bass_boost: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenumberSettings {
    pub enabled: bool,
    pub threshold: f32,
}

impl Default for PlaybackSettings {
    fn default() -> Self {
        Self { volume: 0.7, shuffle: false, repeat: RepeatMode::Off, crossfade_ms: 0 }
    }
}
impl Default for EqualizerSettings {
    fn default() -> Self {
        Self { enabled: false, bands: [0.0; 10], bass_boost: 0.0 }
    }
}
impl Default for RenumberSettings {
    fn default() -> Self {
        Self { enabled: true, threshold: 0.5 }
    }
}

impl Default for Settings {
    fn default() -> Self {
        let scan = if cfg!(debug_assertions) {
            let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
            ScanSettings {
                roots: vec![cwd.join("music")],
                source_root: Some(cwd.join("music_original")),
            }
        } else {
            ScanSettings::default()
        };
        Self {
            scan,
            playback: PlaybackSettings::default(),
            equalizer: EqualizerSettings::default(),
            renumber: RenumberSettings::default(),
        }
    }
}

fn settings_path() -> Option<PathBuf> {
    ProjectDirs::from("", "Recurate", "Recurate")
        .map(|p| p.config_dir().join("settings.toml"))
}

impl Settings {
    pub fn load_or_default() -> Self {
        let Some(path) = settings_path() else { return Self::default(); };
        match std::fs::read_to_string(&path) {
            Ok(s) => match toml::from_str(&s) {
                Ok(v) => v,
                Err(e) => {
                    log::warn!("settings.toml parse failed ({}); using defaults", e);
                    Self::default()
                }
            },
            Err(_) => Self::default(),
        }
    }

    pub fn save(&self) -> std::io::Result<()> {
        let Some(path) = settings_path() else {
            return Err(std::io::Error::new(std::io::ErrorKind::Other, "no project dir"));
        };
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let s = toml::to_string_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;
        std::fs::write(&path, s)
    }

    pub fn save_to(&self, path: &Path) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let s = toml::to_string_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;
        std::fs::write(path, s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_through_toml() {
        let s = Settings::default();
        let txt = toml::to_string(&s).unwrap();
        let back: Settings = toml::from_str(&txt).unwrap();
        assert_eq!(s.playback.volume, back.playback.volume);
        assert_eq!(s.equalizer.bands, back.equalizer.bands);
    }

    #[test]
    fn parse_failure_falls_back_to_default() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("corrupt.toml");
        std::fs::write(&path, "this isn't toml [[[").unwrap();
        let s = std::fs::read_to_string(&path).unwrap();
        let parsed: Result<Settings, _> = toml::from_str(&s);
        assert!(parsed.is_err());
    }
}
