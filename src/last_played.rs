use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LastPlayed {
    pub path: PathBuf,
    #[serde(default)]
    pub position_ms: u64,
}

fn record_path() -> Option<PathBuf> {
    ProjectDirs::from("", "Recurate", "Recurate")
        .map(|p| p.data_local_dir().join("last_played.toml"))
}

/// Atomic write: writes to `.tmp` then renames. A crash mid-write leaves the
/// previous valid file in place rather than a half-written one.
pub fn save(record: &LastPlayed) -> std::io::Result<()> {
    let path = record_path().ok_or_else(|| std::io::Error::other("no project dir"))?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let body = toml::to_string_pretty(record).map_err(std::io::Error::other)?;
    // Make tmp name unique so concurrent saves (rare) and stale tmp files
    // from prior crashes don't collide.
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let tmp = path.with_extension(format!("toml.{}.{nanos}.tmp", std::process::id()));
    std::fs::write(&tmp, body)?;
    if let Err(e) = std::fs::rename(&tmp, &path) {
        let _ = std::fs::remove_file(&tmp);
        return Err(e);
    }
    Ok(())
}

pub fn load() -> Option<LastPlayed> {
    let path = record_path()?;
    let body = std::fs::read_to_string(&path).ok()?;
    match toml::from_str(&body) {
        Ok(v) => Some(v),
        Err(e) => {
            log::warn!("last_played.toml parse failed: {e}");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_through_toml() {
        let r = LastPlayed {
            path: PathBuf::from("C:/Music/x.mp3"),
            position_ms: 12345,
        };
        let txt = toml::to_string(&r).unwrap();
        let back: LastPlayed = toml::from_str(&txt).unwrap();
        assert_eq!(r.path, back.path);
        assert_eq!(r.position_ms, back.position_ms);
    }

    #[test]
    fn parses_with_missing_position() {
        let txt = "path = \"C:/x.mp3\"\n";
        let back: LastPlayed = toml::from_str(txt).unwrap();
        assert_eq!(back.path, PathBuf::from("C:/x.mp3"));
        assert_eq!(back.position_ms, 0);
    }
}
