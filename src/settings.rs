use crate::domain::RepeatMode;
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default)]
    pub scan: ScanSettings,
    #[serde(default)]
    pub playback: PlaybackSettings,
    #[serde(default)]
    pub equalizer: EqualizerSettings,
    #[serde(default)]
    pub renumber: RenumberSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ScanSettings {
    #[serde(default)]
    pub roots: Vec<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PlaybackSettings {
    pub volume: f32,
    pub shuffle: bool,
    pub repeat: RepeatMode,
    pub crossfade_ms: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct EqualizerSettings {
    pub enabled: bool,
    pub bands: [f32; 10],
    pub bass_boost: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct RenumberSettings {
    pub enabled: bool,
    pub threshold: f32,
}

impl Default for PlaybackSettings {
    fn default() -> Self {
        Self {
            volume: 0.7,
            shuffle: false,
            repeat: RepeatMode::Off,
            crossfade_ms: 0,
        }
    }
}
impl Default for EqualizerSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            bands: [0.0; 10],
            bass_boost: 0.0,
        }
    }
}
impl Default for RenumberSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            threshold: 0.5,
        }
    }
}

impl Default for Settings {
    fn default() -> Self {
        let scan = if cfg!(debug_assertions) {
            let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
            ScanSettings {
                roots: vec![cwd.join("music")],
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
    ProjectDirs::from("", "Recurate", "Recurate").map(|p| p.config_dir().join("settings.toml"))
}

/// Return `v` if it is finite, otherwise `fallback`. Used to scrub NaN/inf
/// out of values that originate from a (possibly hand-edited) config file.
fn finite_or(v: f32, fallback: f32) -> f32 {
    if v.is_finite() {
        v
    } else {
        fallback
    }
}

impl Settings {
    pub fn load_or_default() -> Self {
        let Some(path) = settings_path() else {
            return Self::default();
        };
        Self::load_from_path_or_default(&path)
    }

    fn load_from_path_or_default(path: &Path) -> Self {
        match std::fs::read_to_string(path) {
            Ok(s) => match toml::from_str::<Self>(&s) {
                Ok(mut v) => {
                    v.clamp_invariants();
                    v
                }
                Err(e) => {
                    log::warn!("settings.toml parse failed ({e}); backing up and using defaults");
                    let backup = path.with_extension("toml.bak");
                    if let Err(be) = std::fs::rename(path, &backup) {
                        log::warn!("could not back up corrupt settings: {be}");
                    }
                    Self::default()
                }
            },
            Err(_) => Self::default(),
        }
    }

    fn clamp_invariants(&mut self) {
        // `f32::clamp` propagates NaN rather than clamping it, and TOML can
        // encode `nan` — so a hand-edited config could otherwise smuggle a
        // NaN volume straight into the audio callback. Coerce non-finite
        // values to a safe default before clamping.
        self.playback.volume = finite_or(self.playback.volume, 0.7).clamp(0.0, 1.0);
        self.renumber.threshold = finite_or(self.renumber.threshold, 0.5).clamp(0.0, 1.0);
        for b in &mut self.equalizer.bands {
            *b = finite_or(*b, 0.0).clamp(-24.0, 24.0);
        }
    }

    pub fn save(&self) -> std::io::Result<()> {
        let Some(path) = settings_path() else {
            return Err(std::io::Error::other("no project dir"));
        };
        self.save_to(&path)
    }

    pub fn save_to(&self, path: &Path) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let s = toml::to_string_pretty(self).map_err(std::io::Error::other)?;
        // Include pid + nanos in the temp name so two concurrent saves (and
        // any stale `.tmp` from a crashed prior run) don't collide.
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let pid = std::process::id();
        let tmp = path.with_extension(format!("toml.{pid}.{nanos}.tmp"));
        std::fs::write(&tmp, s)?;
        match std::fs::rename(&tmp, path) {
            Ok(()) => Ok(()),
            Err(e) => {
                let _ = std::fs::remove_file(&tmp);
                Err(e)
            }
        }
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

    #[test]
    fn clamp_invariants_scrubs_non_finite_and_out_of_range() {
        let mut s = Settings::default();
        // TOML can encode `nan` / `inf`; f32::clamp would propagate NaN
        // straight into the audio gain. clamp_invariants must scrub it.
        s.playback.volume = f32::NAN;
        s.renumber.threshold = f32::INFINITY;
        s.equalizer.bands[0] = f32::NAN;
        s.equalizer.bands[1] = 99.0;
        s.clamp_invariants();
        assert!(s.playback.volume.is_finite());
        assert!((0.0..=1.0).contains(&s.playback.volume));
        assert!((0.0..=1.0).contains(&s.renumber.threshold));
        assert_eq!(s.equalizer.bands[0], 0.0);
        assert_eq!(s.equalizer.bands[1], 24.0);
    }

    #[test]
    fn nan_volume_in_toml_is_scrubbed_on_load() {
        // Simulate the load path: parse a hand-edited config with a NaN
        // volume, then apply invariants as load_or_default does.
        let txt = "[scan]\nroots = []\n\
                   [playback]\nvolume = nan\nshuffle = false\nrepeat = \"off\"\ncrossfade_ms = 0\n\
                   [equalizer]\nenabled = false\nbands = [0,0,0,0,0,0,0,0,0,0]\nbass_boost = 0.0\n\
                   [renumber]\nenabled = true\nthreshold = 0.5\n";
        let mut parsed: Settings = toml::from_str(txt).unwrap();
        assert!(parsed.playback.volume.is_nan());
        parsed.clamp_invariants();
        assert!(parsed.playback.volume.is_finite());
    }

    #[test]
    fn older_settings_missing_new_fields_preserve_existing_preferences() {
        let txt = "[scan]\nroots = [\"C:/Music\"]\n\
                   [playback]\nvolume = 0.25\nshuffle = true\nrepeat = \"all\"\n";

        let parsed: Settings = toml::from_str(txt).unwrap();

        assert_eq!(parsed.scan.roots, vec![PathBuf::from("C:/Music")]);
        assert_eq!(parsed.playback.volume, 0.25);
        assert!(parsed.playback.shuffle);
        assert_eq!(parsed.playback.crossfade_ms, 0);
        assert!(!parsed.equalizer.enabled);
        assert!(parsed.renumber.enabled);
    }

    #[test]
    fn load_from_path_backs_up_corrupt_settings_and_returns_default() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.toml");
        std::fs::write(&path, "this isn't toml [[[").unwrap();

        let loaded = Settings::load_from_path_or_default(&path);

        assert_eq!(loaded.playback.volume, Settings::default().playback.volume);
        assert!(!path.exists());
        assert!(path.with_extension("toml.bak").exists());
    }
}
