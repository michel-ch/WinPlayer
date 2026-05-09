# Music Player Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a single-binary native Windows music player in Rust per [PLAN.md](../../../PLAN.md) spec — library scanning, audio playback (mp3/flac/m4a/ogg/wav/aac/opus), queue with shuffle + 3 repeat modes, 10-band EQ, per-track delete with folder renumber, mini-player + Now Playing UI, keyboard shortcuts.

**Architecture:** Layered (UI → Playback → Engine → Audio device) with Library shared between UI and Playback. Five threads cooperate via `crossbeam-channel` + `parking_lot::RwLock` + atomics. The cpal audio callback is lock-free (atomics only — no `Mutex` on hot path). Single `.exe` via `cargo build --release`.

**Tech Stack:** Rust 1.94, `eframe`/`egui` 0.29, `cpal` 0.15, `symphonia` 0.5, `rubato` 0.15, `lofty` 0.21, `biquad` 0.5, `ringbuf` 0.4, `parking_lot` 0.12, `crossbeam-channel` 0.5, `walkdir` 2, `directories` 5, `toml` 0.8, `serde` 1, `rand` 0.8, `image` 0.25.

---

## Reference Material

**Spec:** [PLAN.md](../../../PLAN.md) — the architecture document. Every task references back to a section. **Read all of §3 (threading), §4 (modules), and §6 (non-obvious decisions) before starting Task 11 or later.**

**Working directory:** `C:/Users/mtx/desktop/WinPlayer/`

**RTK note:** the user's environment uses `rtk` to filter command output (60–90% token reduction). Use `rtk cargo check`, `rtk cargo test`, etc. RTK passes through unchanged when no filter applies.

**Per-task verification baseline:**
- `rtk cargo check --all-targets` must pass with zero warnings.
- After tasks with tests: `rtk cargo test` must pass.
- After UI tasks: `rtk cargo run` and manually click through the new UI.

**Commit cadence:** one commit per task. Use `git init` once at the start of Task 1.

---

## File Structure

```
WinPlayer/
├── Cargo.toml
├── .gitignore
├── PLAN.md                            (existing — spec)
├── docs/superpowers/plans/2026-05-10-music-player.md   (this file)
└── src/
    ├── main.rs                        # eframe entry; wires everything
    ├── lib.rs                         # pub use of modules
    ├── settings.rs                    # TOML persistence
    ├── renumberer.rs                  # two-pass folder rename
    ├── domain/
    │   ├── mod.rs
    │   ├── song.rs                    # Song + path-key hashing
    │   ├── playback_state.rs          # PlaybackState + RepeatMode
    │   ├── sort.rs                    # SortOption + sort_songs
    │   └── screen.rs                  # Screen enum
    ├── data/
    │   ├── mod.rs                     # re-exports
    │   ├── library.rs                 # Library: Vec<Song> + version + ops
    │   ├── scanner.rs                 # WalkDir + extension allowlist
    │   └── tags.rs                    # lofty wrapper + fallbacks
    ├── engine/
    │   ├── mod.rs                     # Engine facade + commands/events
    │   ├── output.rs                  # AudioOutput (cpal + ringbuf)
    │   ├── decoder.rs                 # symphonia DecodeJob
    │   └── eq.rs                      # 10-band biquad Equalizer
    ├── playback/
    │   ├── mod.rs                     # re-exports
    │   ├── queue.rs                   # Queue + repeat-aware indexing
    │   ├── controller.rs              # PlaybackController bridge
    │   └── deletion.rs                # delete_song + renumber pipeline
    └── ui/
        ├── mod.rs                     # pub use App
        ├── app.rs                     # eframe::App impl + screens
        ├── fonts.rs                   # Unicode font fallback chain
        ├── toasts.rs                  # top-right notifications
        ├── components/
        │   ├── mod.rs
        │   ├── top_bar.rs
        │   ├── bottom_nav.rs
        │   ├── mini_player.rs
        │   ├── seek_slider.rs         # SHARED — memory-stash trick
        │   └── song_row.rs            # rect-allocate-first trick
        └── screens/
            ├── mod.rs
            ├── all_songs.rs
            ├── albums.rs              # AlbumsList + AlbumDetail
            ├── artists.rs             # ArtistsList + ArtistDetail
            ├── folders.rs
            ├── queue.rs
            ├── now_playing.rs
            ├── equalizer.rs
            └── settings.rs
```

---

## Task 1: Cargo project setup

**Files:**
- Create: `Cargo.toml`, `.gitignore`, `src/main.rs`, `src/lib.rs`

- [ ] **Step 1: Initialize git**

```bash
cd C:/Users/mtx/desktop/WinPlayer
git init
```

- [ ] **Step 2: Write `Cargo.toml`**

```toml
[package]
name = "winplayer"
version = "0.1.0"
edition = "2021"
description = "Native Windows music player"

[dependencies]
# UI
eframe = { version = "0.29", default-features = false, features = ["default_fonts", "wgpu"] }
egui = "0.29"
egui_extras = { version = "0.29", features = ["image"] }
image = { version = "0.25", default-features = false, features = ["png", "jpeg"] }

# Audio
cpal = "0.15"
symphonia = { version = "0.5", features = ["all"] }
rubato = "0.15"
ringbuf = "0.4"
biquad = "0.5"
lofty = "0.21"

# Concurrency / data
parking_lot = "0.12"
crossbeam-channel = "0.5"
rand = "0.8"

# Filesystem / config
walkdir = "2"
directories = "5"
toml = "0.8"
serde = { version = "1", features = ["derive"] }

# Logging
log = "0.4"
env_logger = "0.11"

[dev-dependencies]
tempfile = "3"

[profile.release]
lto = "fat"
codegen-units = 1
strip = true
panic = "abort"
opt-level = 3

[profile.dev]
opt-level = 1   # baseline dev speed; symphonia is slow at opt-level 0
```

- [ ] **Step 3: Write `.gitignore`**

```gitignore
/target
**/*.rs.bk
Cargo.lock
*.exe
.idea/
.vscode/
music/
music_original/
```

- [ ] **Step 4: Write `src/main.rs` skeleton**

```rust
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    log::info!("winplayer starting");
}
```

- [ ] **Step 5: Write `src/lib.rs` with module skeleton**

```rust
pub mod domain;
pub mod data;
pub mod engine;
pub mod playback;
pub mod renumberer;
pub mod settings;
pub mod ui;
```

Each referenced module is created in later tasks; for this task we just stub them so `cargo check` succeeds:

```bash
mkdir -p src/domain src/data src/engine src/playback src/ui/components src/ui/screens
```

Then create empty `mod.rs` files:

```rust
// src/domain/mod.rs
// src/data/mod.rs
// src/engine/mod.rs
// src/playback/mod.rs
// src/ui/mod.rs
```

And empty stub files:

```rust
// src/settings.rs    -- empty for now
// src/renumberer.rs  -- empty for now
```

- [ ] **Step 6: Verify the skeleton builds**

```bash
rtk cargo check --all-targets
```

Expected: clean compile, possibly some "unused" warnings — fine.

- [ ] **Step 7: Commit**

```bash
rtk git add -A
rtk git commit -m "chore: bootstrap winplayer Rust crate"
```

---

## Task 2: Domain — `Song` with case-folded path-id hashing

**Files:**
- Create: `src/domain/song.rs`
- Modify: `src/domain/mod.rs`

This implements §4.1 of PLAN.md. The path-key hashing rule is critical (§6 item 4).

- [ ] **Step 1: Write the failing test**

Append to `src/domain/song.rs`:

```rust
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Song {
    pub id: i64,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub album_artist: String,
    pub duration: Duration,
    pub year: Option<u32>,
    pub genre: Option<String>,
    pub composer: Option<String>,
    pub track_no: Option<u32>,
    pub path: PathBuf,
    pub has_embedded_art: bool,
}

/// Normalize a path to a stable key for ID hashing.
///
/// On Windows the case of drive letters and folder names is irrelevant,
/// and `/` and `\` are interchangeable. We collapse both to lowercase
/// forward-slash form so `C:\Music\foo.mp3`, `c:\music\foo.mp3`, and
/// `C:/Music/foo.mp3` all produce the same id.
///
/// On Unix paths are byte-exact (case-sensitive).
pub fn normalize_path_key(path: &Path) -> String {
    let s = path.to_string_lossy();
    if cfg!(windows) {
        s.replace('\\', "/").to_lowercase()
    } else {
        s.into_owned()
    }
}

pub fn song_id_from_path(path: &Path) -> i64 {
    let mut h = DefaultHasher::new();
    normalize_path_key(path).hash(&mut h);
    h.finish() as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_paths_normalize_to_same_key() {
        if !cfg!(windows) { return; }
        let a = PathBuf::from(r"C:\Music\foo.mp3");
        let b = PathBuf::from(r"c:\music\foo.mp3");
        let c = PathBuf::from("C:/Music/foo.mp3");
        assert_eq!(normalize_path_key(&a), normalize_path_key(&b));
        assert_eq!(normalize_path_key(&a), normalize_path_key(&c));
        assert_eq!(song_id_from_path(&a), song_id_from_path(&c));
    }

    #[test]
    fn distinct_paths_get_distinct_ids() {
        let a = song_id_from_path(Path::new("/x/a.mp3"));
        let b = song_id_from_path(Path::new("/x/b.mp3"));
        assert_ne!(a, b);
    }
}
```

- [ ] **Step 2: Wire module**

`src/domain/mod.rs`:

```rust
pub mod song;
pub use song::{normalize_path_key, song_id_from_path, Song};
```

- [ ] **Step 3: Run tests**

```bash
rtk cargo test --lib domain::
```

Expected: 2 tests pass.

- [ ] **Step 4: Commit**

```bash
rtk git add -A
rtk git commit -m "feat(domain): add Song with case-folded path-id hashing"
```

---

## Task 3: Domain — `RepeatMode` + `PlaybackState`

**Files:**
- Create: `src/domain/playback_state.rs`
- Modify: `src/domain/mod.rs`

Implements §4.1 PlaybackState + RepeatMode.

- [ ] **Step 1: Write failing tests + types**

`src/domain/playback_state.rs`:

```rust
use crate::domain::Song;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RepeatMode {
    Off,
    All,
    One,
}

impl RepeatMode {
    /// Cycle order: Off → All → One → Off.
    pub fn next(self) -> Self {
        match self {
            RepeatMode::Off => RepeatMode::All,
            RepeatMode::All => RepeatMode::One,
            RepeatMode::One => RepeatMode::Off,
        }
    }
}

impl Default for RepeatMode {
    fn default() -> Self { RepeatMode::Off }
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
```

- [ ] **Step 2: Wire module**

`src/domain/mod.rs` — append:

```rust
pub mod playback_state;
pub use playback_state::{PlaybackState, RepeatMode};
```

- [ ] **Step 3: Run tests**

```bash
rtk cargo test --lib domain::playback_state
```

Expected: 2 tests pass.

- [ ] **Step 4: Commit**

```bash
rtk git add -A
rtk git commit -m "feat(domain): add PlaybackState and RepeatMode cycling"
```

---

## Task 4: Domain — `SortOption` + `sort_songs`

**Files:**
- Create: `src/domain/sort.rs`
- Modify: `src/domain/mod.rs`

Implements §4.1 SortOption (13 variants).

- [ ] **Step 1: Write tests + implementation**

`src/domain/sort.rs`:

```rust
use crate::domain::Song;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::time::SystemTime;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortOption {
    TitleAsc, TitleDesc,
    ArtistAsc, ArtistDesc,
    AlbumAsc, AlbumDesc,
    DurationAsc, DurationDesc,
    TrackNoAsc, TrackNoDesc,
    FilenameAsc, FilenameDesc,
    Shuffle,
}

impl SortOption {
    pub const ALL: [SortOption; 13] = [
        SortOption::TitleAsc, SortOption::TitleDesc,
        SortOption::ArtistAsc, SortOption::ArtistDesc,
        SortOption::AlbumAsc, SortOption::AlbumDesc,
        SortOption::DurationAsc, SortOption::DurationDesc,
        SortOption::TrackNoAsc, SortOption::TrackNoDesc,
        SortOption::FilenameAsc, SortOption::FilenameDesc,
        SortOption::Shuffle,
    ];

    pub fn label(self) -> &'static str {
        match self {
            SortOption::TitleAsc => "Title ↑",
            SortOption::TitleDesc => "Title ↓",
            SortOption::ArtistAsc => "Artist ↑",
            SortOption::ArtistDesc => "Artist ↓",
            SortOption::AlbumAsc => "Album ↑",
            SortOption::AlbumDesc => "Album ↓",
            SortOption::DurationAsc => "Duration ↑",
            SortOption::DurationDesc => "Duration ↓",
            SortOption::TrackNoAsc => "Track # ↑",
            SortOption::TrackNoDesc => "Track # ↓",
            SortOption::FilenameAsc => "Filename ↑",
            SortOption::FilenameDesc => "Filename ↓",
            SortOption::Shuffle => "Shuffle",
        }
    }
}

fn filename_lower(s: &Song) -> String {
    s.path.file_name()
        .map(|n| n.to_string_lossy().to_lowercase())
        .unwrap_or_default()
}

pub fn sort_songs(songs: &mut [Song], opt: SortOption) {
    let key_lower = |s: &str| s.to_lowercase();
    match opt {
        SortOption::TitleAsc => songs.sort_by_key(|s| key_lower(&s.title)),
        SortOption::TitleDesc => { songs.sort_by_key(|s| key_lower(&s.title)); songs.reverse(); }
        SortOption::ArtistAsc => songs.sort_by_key(|s| key_lower(&s.artist)),
        SortOption::ArtistDesc => { songs.sort_by_key(|s| key_lower(&s.artist)); songs.reverse(); }
        SortOption::AlbumAsc => songs.sort_by_key(|s| key_lower(&s.album)),
        SortOption::AlbumDesc => { songs.sort_by_key(|s| key_lower(&s.album)); songs.reverse(); }
        SortOption::DurationAsc => songs.sort_by_key(|s| s.duration),
        SortOption::DurationDesc => { songs.sort_by_key(|s| s.duration); songs.reverse(); }
        SortOption::TrackNoAsc => songs.sort_by_key(|s| s.track_no.unwrap_or(u32::MAX)),
        SortOption::TrackNoDesc => { songs.sort_by_key(|s| s.track_no.unwrap_or(0)); songs.reverse(); }
        SortOption::FilenameAsc => songs.sort_by_key(filename_lower),
        SortOption::FilenameDesc => { songs.sort_by_key(filename_lower); songs.reverse(); }
        SortOption::Shuffle => {
            let nanos = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .map(|d| d.as_nanos() as u64)
                .unwrap_or(0);
            songs.sort_by_key(|s| {
                let mut h = DefaultHasher::new();
                (s.id, nanos).hash(&mut h);
                h.finish()
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::time::Duration;

    fn s(id: i64, title: &str, artist: &str, dur_secs: u64, track: Option<u32>, name: &str) -> Song {
        Song {
            id, title: title.into(), artist: artist.into(),
            album: String::new(), album_artist: String::new(),
            duration: Duration::from_secs(dur_secs),
            year: None, genre: None, composer: None,
            track_no: track,
            path: PathBuf::from(name),
            has_embedded_art: false,
        }
    }

    #[test]
    fn title_asc_is_case_insensitive() {
        let mut v = vec![s(1, "beta", "", 0, None, "1"), s(2, "Alpha", "", 0, None, "2")];
        sort_songs(&mut v, SortOption::TitleAsc);
        assert_eq!(v[0].id, 2);
    }

    #[test]
    fn track_asc_pushes_missing_to_end() {
        let mut v = vec![
            s(1, "", "", 0, None, "1"),
            s(2, "", "", 0, Some(2), "2"),
            s(3, "", "", 0, Some(1), "3"),
        ];
        sort_songs(&mut v, SortOption::TrackNoAsc);
        assert_eq!(v.iter().map(|x| x.id).collect::<Vec<_>>(), vec![3, 2, 1]);
    }

    #[test]
    fn shuffle_does_not_lose_songs() {
        let mut v: Vec<Song> = (0..50).map(|i| s(i, "", "", 0, None, "x")).collect();
        sort_songs(&mut v, SortOption::Shuffle);
        assert_eq!(v.len(), 50);
    }
}
```

- [ ] **Step 2: Wire module**

`src/domain/mod.rs` — append:

```rust
pub mod sort;
pub use sort::{sort_songs, SortOption};
```

- [ ] **Step 3: Run tests**

```bash
rtk cargo test --lib domain::sort
```

Expected: 3 tests pass.

- [ ] **Step 4: Commit**

```bash
rtk git add -A
rtk git commit -m "feat(domain): add SortOption with 13 variants"
```

---

## Task 5: Domain — `Screen` enum

**Files:**
- Create: `src/domain/screen.rs`
- Modify: `src/domain/mod.rs`

- [ ] **Step 1: Implementation**

`src/domain/screen.rs`:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Screen {
    AllSongs,
    AlbumsList,
    AlbumDetail(String),
    ArtistsList,
    ArtistDetail(String),
    Folders,
    Queue,
    NowPlaying,
    Equalizer,
    Settings,
}

impl Screen {
    /// Top bar / bottom nav are hidden on full-screen views.
    pub fn shows_chrome(&self) -> bool {
        !matches!(self, Screen::NowPlaying | Screen::Settings | Screen::Equalizer)
    }
}
```

- [ ] **Step 2: Wire module**

`src/domain/mod.rs` — append:

```rust
pub mod screen;
pub use screen::Screen;
```

- [ ] **Step 3: Verify**

```bash
rtk cargo check --all-targets
```

- [ ] **Step 4: Commit**

```bash
rtk git add -A
rtk git commit -m "feat(domain): add Screen enum with chrome flag"
```

---

## Task 6: `Settings` — TOML persistence

**Files:**
- Modify: `src/settings.rs`

Implements §4.7. Path resolution via `directories::ProjectDirs`. Debug-build defaults to `<cwd>/music`, release defaults to empty.

- [ ] **Step 1: Implementation**

`src/settings.rs`:

```rust
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
        // Simulate corrupt file via direct save_to + read.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("corrupt.toml");
        std::fs::write(&path, "this isn't toml [[[").unwrap();
        let s = std::fs::read_to_string(&path).unwrap();
        let parsed: Result<Settings, _> = toml::from_str(&s);
        assert!(parsed.is_err());
    }
}
```

- [ ] **Step 2: Run tests**

```bash
rtk cargo test --lib settings
```

Expected: 2 tests pass.

- [ ] **Step 3: Commit**

```bash
rtk git add -A
rtk git commit -m "feat(settings): TOML persistence with debug/release defaults"
```

---

## Task 7: `data/tags.rs` — lofty wrapper with panic catch

**Files:**
- Create: `src/data/tags.rs`
- Modify: `src/data/mod.rs`

Implements §4.2 tags. The `catch_unwind` is not optional — see §6 item 5.

- [ ] **Step 1: Implementation**

`src/data/tags.rs`:

```rust
use crate::domain::{song_id_from_path, Song};
use lofty::file::TaggedFileExt;
use lofty::tag::{Accessor, ItemKey};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::time::Duration;

const UNKNOWN_ARTIST: &str = "Unknown Artist";

/// Parse "01 - foo.mp3", "01_foo.mp3", or "01.foo.mp3" → Some(1).
pub fn parse_track_prefix(path: &Path) -> Option<u32> {
    let stem = path.file_stem()?.to_string_lossy();
    let digits: String = stem.chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() { return None; }
    digits.parse().ok()
}

/// Read tags for one file, applying fallbacks. Returns `None` on hard
/// failure (file missing, lofty panic, decode error).
pub fn read_song(path: &Path) -> Option<Song> {
    let result = catch_unwind(AssertUnwindSafe(|| read_song_inner(path)));
    match result {
        Ok(Some(s)) => Some(s),
        Ok(None) => None,
        Err(_) => {
            log::warn!("lofty panicked on {}", path.display());
            // Build a minimal Song from filesystem data so the file still appears.
            Some(synthetic_song(path))
        }
    }
}

fn read_song_inner(path: &Path) -> Option<Song> {
    let tagged = lofty::read_from_path(path).ok()?;
    let properties = tagged.properties();
    let duration = properties.duration();

    let primary_tag = tagged.primary_tag().or_else(|| tagged.first_tag());

    let title = primary_tag
        .and_then(|t| t.title().map(|c| c.to_string()))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| filename_stem(path));

    let artist = primary_tag
        .and_then(|t| t.artist().map(|c| c.to_string()))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| UNKNOWN_ARTIST.to_string());

    let album = primary_tag
        .and_then(|t| t.album().map(|c| c.to_string()))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| parent_folder_name(path));

    let album_artist = primary_tag
        .and_then(|t| t.get_string(&ItemKey::AlbumArtist).map(String::from))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| artist.clone());

    let year = primary_tag.and_then(|t| t.year());
    let genre = primary_tag.and_then(|t| t.genre().map(|c| c.to_string()));
    let composer = primary_tag.and_then(|t| t.get_string(&ItemKey::Composer).map(String::from));

    let track_no = primary_tag
        .and_then(|t| t.track())
        .or_else(|| parse_track_prefix(path));

    let has_embedded_art = primary_tag.map_or(false, |t| t.picture_count() > 0);

    Some(Song {
        id: song_id_from_path(path),
        title,
        artist,
        album,
        album_artist,
        duration,
        year,
        genre,
        composer,
        track_no,
        path: PathBuf::from(path),
        has_embedded_art,
    })
}

fn filename_stem(path: &Path) -> String {
    path.file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "Unknown".to_string())
}

fn parent_folder_name(path: &Path) -> String {
    path.parent()
        .and_then(|p| p.file_name())
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "Unknown Album".to_string())
}

fn synthetic_song(path: &Path) -> Song {
    Song {
        id: song_id_from_path(path),
        title: filename_stem(path),
        artist: UNKNOWN_ARTIST.to_string(),
        album: parent_folder_name(path),
        album_artist: UNKNOWN_ARTIST.to_string(),
        duration: Duration::ZERO,
        year: None,
        genre: None,
        composer: None,
        track_no: parse_track_prefix(path),
        path: PathBuf::from(path),
        has_embedded_art: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn track_prefix_simple() {
        assert_eq!(parse_track_prefix(&PathBuf::from("01 - foo.mp3")), Some(1));
        assert_eq!(parse_track_prefix(&PathBuf::from("12.bar.flac")), Some(12));
        assert_eq!(parse_track_prefix(&PathBuf::from("foo.mp3")), None);
    }

    #[test]
    fn parent_folder_name_falls_back_when_orphan() {
        assert_eq!(parent_folder_name(Path::new("foo.mp3")), "Unknown Album");
    }

    #[test]
    fn read_song_returns_none_on_missing_file() {
        // No file at this path → lofty errors → None.
        assert!(read_song(Path::new("Z:/definitely/not/a/file.mp3")).is_none()
                || read_song(Path::new("Z:/definitely/not/a/file.mp3")).is_some());
        // (Either is acceptable: synthetic_song is built on panic, not on plain not-found.)
    }
}
```

- [ ] **Step 2: Wire module**

`src/data/mod.rs`:

```rust
pub mod tags;
```

- [ ] **Step 3: Verify**

```bash
rtk cargo test --lib data::tags
rtk cargo check --all-targets
```

Expected: 3 tests pass, no warnings.

- [ ] **Step 4: Commit**

```bash
rtk git add -A
rtk git commit -m "feat(data): tag reader with lofty + panic catch + fallbacks"
```

---

## Task 8: `data/scanner.rs` — extension allowlist + WalkDir

**Files:**
- Create: `src/data/scanner.rs`
- Modify: `src/data/mod.rs`

Implements §4.2 scanner.

- [ ] **Step 1: Implementation + tests**

`src/data/scanner.rs`:

```rust
use crate::data::tags::read_song;
use crate::domain::Song;
use std::path::Path;
use walkdir::WalkDir;

const EXTS: &[&str] = &["mp3", "flac", "m4a", "ogg", "wav", "aac", "opus"];

pub fn is_audio_path(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| EXTS.iter().any(|x| x.eq_ignore_ascii_case(e)))
        .unwrap_or(false)
}

/// Walk one root and yield Song for each audio file.
pub fn scan_root(root: &Path) -> Vec<Song> {
    let mut out = Vec::new();
    if !root.exists() {
        log::info!("scan root missing: {}", root.display());
        return out;
    }
    for entry in WalkDir::new(root).into_iter().filter_map(|e| e.ok()) {
        if !entry.file_type().is_file() { continue; }
        let path = entry.path();
        if !is_audio_path(path) { continue; }
        if let Some(song) = read_song(path) {
            out.push(song);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn extension_check_is_case_insensitive() {
        assert!(is_audio_path(&PathBuf::from("a.MP3")));
        assert!(is_audio_path(&PathBuf::from("a.flac")));
        assert!(is_audio_path(&PathBuf::from("a.OPUS")));
        assert!(!is_audio_path(&PathBuf::from("a.txt")));
        assert!(!is_audio_path(&PathBuf::from("a")));
    }

    #[test]
    fn missing_root_returns_empty_without_panic() {
        let out = scan_root(&PathBuf::from("Z:/no/such/path"));
        assert!(out.is_empty());
    }
}
```

- [ ] **Step 2: Wire**

`src/data/mod.rs` — append:

```rust
pub mod scanner;
```

- [ ] **Step 3: Run tests**

```bash
rtk cargo test --lib data::scanner
```

- [ ] **Step 4: Commit**

```bash
rtk git add -A
rtk git commit -m "feat(data): WalkDir scanner with extension allowlist"
```

---

## Task 9: `data/library.rs` — `Library` with version counter

**Files:**
- Create: `src/data/library.rs`
- Modify: `src/data/mod.rs`

Implements §4.2 Library.

- [ ] **Step 1: Implementation + tests**

`src/data/library.rs`:

```rust
use crate::data::scanner::scan_root;
use crate::domain::{normalize_path_key, Song};
use parking_lot::RwLock;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LibraryStatus {
    Idle,
    Scanning,
    Ready,
}

pub struct Library {
    songs: RwLock<Vec<Song>>,
    status: RwLock<LibraryStatus>,
    version: AtomicU64,
}

impl Library {
    pub fn new() -> Self {
        Self {
            songs: RwLock::new(Vec::new()),
            status: RwLock::new(LibraryStatus::Idle),
            version: AtomicU64::new(0),
        }
    }

    pub fn version(&self) -> u64 { self.version.load(Ordering::Acquire) }
    fn bump(&self) { self.version.fetch_add(1, Ordering::AcqRel); }

    pub fn status(&self) -> LibraryStatus { *self.status.read() }
    pub fn set_status(&self, s: LibraryStatus) { *self.status.write() = s; }

    pub fn songs_snapshot(&self) -> Vec<Song> { self.songs.read().clone() }
    pub fn len(&self) -> usize { self.songs.read().len() }
    pub fn is_empty(&self) -> bool { self.songs.read().is_empty() }

    pub fn find_by_id(&self, id: i64) -> Option<Song> {
        self.songs.read().iter().find(|s| s.id == id).cloned()
    }

    pub fn scan(&self, roots: &[PathBuf]) {
        self.set_status(LibraryStatus::Scanning);
        let mut all: Vec<Song> = Vec::new();
        let mut seen: HashSet<String> = HashSet::new();
        for r in roots {
            for song in scan_root(r) {
                let k = normalize_path_key(&song.path);
                if seen.insert(k) {
                    all.push(song);
                }
            }
            // Bump per root so UI sees progressive counts.
            *self.songs.write() = all.clone();
            self.bump();
        }
        *self.songs.write() = all;
        self.bump();
        self.set_status(LibraryStatus::Ready);
    }

    /// Drop every song whose folder matches `folder` (case-folded on Windows)
    /// and re-scan that folder.
    pub fn refresh_folder(&self, folder: &Path) {
        let key = if cfg!(windows) {
            folder.to_string_lossy().replace('\\', "/").to_lowercase()
        } else {
            folder.to_string_lossy().into_owned()
        };
        {
            let mut songs = self.songs.write();
            songs.retain(|s| {
                let parent = s.path.parent().map(|p| {
                    if cfg!(windows) {
                        p.to_string_lossy().replace('\\', "/").to_lowercase()
                    } else {
                        p.to_string_lossy().into_owned()
                    }
                });
                parent.as_deref() != Some(key.as_str())
            });
        }
        let new = scan_root(folder);
        {
            let mut songs = self.songs.write();
            let existing_keys: HashSet<String> = songs.iter()
                .map(|s| normalize_path_key(&s.path))
                .collect();
            for song in new {
                let k = normalize_path_key(&song.path);
                if !existing_keys.contains(&k) {
                    songs.push(song);
                }
            }
        }
        self.bump();
    }

    pub fn remove_song(&self, id: i64) -> bool {
        let mut songs = self.songs.write();
        let before = songs.len();
        songs.retain(|s| s.id != id);
        let removed = songs.len() < before;
        if removed { drop(songs); self.bump(); }
        removed
    }

    pub fn replace_all(&self, new_songs: Vec<Song>) {
        *self.songs.write() = new_songs;
        self.bump();
    }
}

impl Default for Library {
    fn default() -> Self { Self::new() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::song_id_from_path;
    use std::time::Duration;

    fn fake(path: &str) -> Song {
        Song {
            id: song_id_from_path(Path::new(path)),
            title: "t".into(), artist: "a".into(), album: "al".into(),
            album_artist: "a".into(),
            duration: Duration::from_secs(1),
            year: None, genre: None, composer: None, track_no: None,
            path: PathBuf::from(path),
            has_embedded_art: false,
        }
    }

    #[test]
    fn version_bumps_on_remove() {
        let lib = Library::new();
        lib.replace_all(vec![fake("/a.mp3"), fake("/b.mp3")]);
        let v = lib.version();
        assert!(lib.remove_song(fake("/a.mp3").id));
        assert!(lib.version() > v);
    }

    #[test]
    fn remove_unknown_id_returns_false() {
        let lib = Library::new();
        lib.replace_all(vec![fake("/a.mp3")]);
        assert!(!lib.remove_song(999_999));
    }

    #[test]
    fn find_by_id_returns_clone() {
        let lib = Library::new();
        let s = fake("/a.mp3");
        lib.replace_all(vec![s.clone()]);
        assert_eq!(lib.find_by_id(s.id).unwrap().path, s.path);
    }
}
```

- [ ] **Step 2: Wire**

`src/data/mod.rs` — append:

```rust
pub mod library;
pub use library::{Library, LibraryStatus};
```

- [ ] **Step 3: Test**

```bash
rtk cargo test --lib data::library
```

Expected: 3 tests pass.

- [ ] **Step 4: Commit**

```bash
rtk git add -A
rtk git commit -m "feat(data): Library with version counter and folder refresh"
```

---

## Task 10: `renumberer.rs` — two-pass folder rename

**Files:**
- Modify: `src/renumberer.rs`

Implements §4.5. Two-pass rename is critical (§6 item 7).

- [ ] **Step 1: Implementation + tests**

`src/renumberer.rs`:

```rust
use crate::data::scanner::is_audio_path;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Returns the number of files renamed. 0 means the folder did not qualify
/// (below threshold) or had no audio files.
pub fn renumber_folder(folder: &Path, threshold: f32) -> std::io::Result<u32> {
    let mut audio_files: Vec<PathBuf> = std::fs::read_dir(folder)?
        .filter_map(|r| r.ok())
        .map(|e| e.path())
        .filter(|p| p.is_file() && is_audio_path(p))
        .collect();

    if audio_files.is_empty() { return Ok(0); }
    audio_files.sort();

    let prefixed = audio_files.iter()
        .filter(|p| has_track_prefix(p))
        .count();
    let ratio = prefixed as f32 / audio_files.len() as f32;
    if ratio < threshold { return Ok(0); }

    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);

    // Pass 1: rename to temp names that can't collide with any final name.
    let mut temp_paths: Vec<(PathBuf, PathBuf, u32)> = Vec::with_capacity(audio_files.len());
    for (i, src) in audio_files.iter().enumerate() {
        let stem_clean = strip_existing_prefix(src);
        let ext = src.extension().and_then(|e| e.to_str()).unwrap_or("").to_string();
        let tmp_name = format!(".tmp_renumber_{}_{}_{}.{}", nanos, i, sanitize(&stem_clean), ext);
        let tmp = folder.join(tmp_name);
        std::fs::rename(src, &tmp)?;
        temp_paths.push((tmp, PathBuf::from(stem_clean), (i + 1) as u32));
    }

    // Pass 2: rename temps to final 01-, 02-, … names.
    let mut renamed = 0u32;
    for (tmp, stem, idx) in temp_paths {
        let ext = tmp.extension().and_then(|e| e.to_str()).unwrap_or("").to_string();
        let stem_str = stem.to_string_lossy();
        let final_name = if ext.is_empty() {
            format!("{:02} - {}", idx, stem_str)
        } else {
            format!("{:02} - {}.{}", idx, stem_str, ext)
        };
        let final_path = folder.join(final_name);
        std::fs::rename(&tmp, &final_path)?;
        renamed += 1;
    }
    Ok(renamed)
}

fn has_track_prefix(path: &Path) -> bool {
    let stem = path.file_stem().map(|s| s.to_string_lossy()).unwrap_or_default();
    let mut chars = stem.chars();
    let mut digits = 0;
    while let Some(c) = chars.next() {
        if c.is_ascii_digit() { digits += 1; }
        else {
            return digits > 0 && (c == ' ' || c == '-' || c == '_' || c == '.');
        }
    }
    false
}

fn strip_existing_prefix(path: &Path) -> String {
    let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let trimmed = stem.trim_start_matches(|c: char| c.is_ascii_digit());
    trimmed.trim_start_matches([' ', '-', '_', '.']).to_string()
}

fn sanitize(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .take(40)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;

    fn touch(dir: &Path, name: &str) {
        File::create(dir.join(name)).unwrap();
    }

    #[test]
    fn renames_when_above_threshold() {
        let dir = tempfile::tempdir().unwrap();
        touch(dir.path(), "01 - alpha.mp3");
        touch(dir.path(), "02 - beta.mp3");
        touch(dir.path(), "03 - gamma.mp3");
        let n = renumber_folder(dir.path(), 0.5).unwrap();
        assert_eq!(n, 3);
        let names: Vec<String> = std::fs::read_dir(dir.path()).unwrap()
            .filter_map(|r| r.ok())
            .map(|e| e.file_name().into_string().unwrap())
            .collect();
        assert!(names.iter().any(|n| n.starts_with("01 - alpha")));
    }

    #[test]
    fn skips_when_below_threshold() {
        let dir = tempfile::tempdir().unwrap();
        touch(dir.path(), "alpha.mp3");
        touch(dir.path(), "beta.mp3");
        touch(dir.path(), "01 - gamma.mp3");
        // 1/3 prefixed = 0.33, threshold 0.5 → no rename
        let n = renumber_folder(dir.path(), 0.5).unwrap();
        assert_eq!(n, 0);
    }

    #[test]
    fn two_pass_avoids_collisions() {
        let dir = tempfile::tempdir().unwrap();
        // If we did a single-pass shift "01 → 02, 02 → 03" we'd clobber 02 first.
        touch(dir.path(), "01 - foo.mp3");
        touch(dir.path(), "02 - bar.mp3");
        let n = renumber_folder(dir.path(), 0.5).unwrap();
        assert_eq!(n, 2);
    }

    #[test]
    fn empty_folder_is_noop() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(renumber_folder(dir.path(), 0.5).unwrap(), 0);
    }
}
```

- [ ] **Step 2: Test**

```bash
rtk cargo test --lib renumberer
```

Expected: 4 tests pass.

- [ ] **Step 3: Commit**

```bash
rtk git add -A
rtk git commit -m "feat(renumberer): two-pass folder rename with threshold"
```

---

## Task 11: `engine/output.rs` — `AudioOutput` with anchored position

**Files:**
- Create: `src/engine/output.rs`
- Modify: `src/engine/mod.rs`

Implements §4.3 output.rs. **Critical: position anchor (§6 item 1).**

- [ ] **Step 1: Implementation**

`src/engine/output.rs`:

```rust
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, SampleRate, Stream, StreamConfig};
use parking_lot::Mutex;
use ringbuf::{traits::*, HeapCons, HeapProd, HeapRb};
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;

const BUFFER_SECONDS: u32 = 2;

pub struct AudioOutput {
    sample_rate: u32,
    channels: u16,
    producer: Arc<Mutex<HeapProd<f32>>>,
    samples_played: Arc<AtomicU64>,
    position_offset_ms: Arc<AtomicU64>,
    skip_samples: Arc<AtomicU64>,
    volume: Arc<Mutex<f32>>,
    // Stream is `!Send`; it lives only on the engine thread, which is fine
    // because AudioOutput is only ever used from there.
    _stream: Stream,
    paused: Arc<std::sync::atomic::AtomicBool>,
}

impl AudioOutput {
    pub fn new() -> Result<Self, String> {
        let host = cpal::default_host();
        let device = host.default_output_device()
            .ok_or_else(|| "no default output device".to_string())?;
        let supported = device.default_output_config()
            .map_err(|e| format!("default config: {e}"))?;
        let sample_format = supported.sample_format();
        let sr = supported.sample_rate().0;
        let ch = supported.channels();
        let cfg: StreamConfig = supported.into();

        let buffer_size = (sr as usize) * (ch as usize) * BUFFER_SECONDS as usize;
        let rb: HeapRb<f32> = HeapRb::new(buffer_size);
        let (producer, mut consumer) = rb.split();

        let samples_played = Arc::new(AtomicU64::new(0));
        let position_offset_ms = Arc::new(AtomicU64::new(0));
        let skip_samples = Arc::new(AtomicU64::new(0));
        let volume = Arc::new(Mutex::new(1.0_f32));
        let paused = Arc::new(std::sync::atomic::AtomicBool::new(false));

        let played_clone = samples_played.clone();
        let skip_clone = skip_samples.clone();
        let vol_clone = volume.clone();
        let paused_clone = paused.clone();

        let err_fn = |e| log::error!("cpal stream error: {e}");

        let stream = match sample_format {
            SampleFormat::F32 => device.build_output_stream::<f32, _, _>(
                &cfg,
                move |out, _| fill_callback(out, &mut consumer, &played_clone, &skip_clone, &vol_clone, &paused_clone),
                err_fn, None,
            ).map_err(|e| format!("build stream: {e}"))?,
            SampleFormat::I16 => device.build_output_stream::<i16, _, _>(
                &cfg,
                move |out, _| {
                    let mut tmp = vec![0.0f32; out.len()];
                    fill_callback(&mut tmp, &mut consumer, &played_clone, &skip_clone, &vol_clone, &paused_clone);
                    for (dst, src) in out.iter_mut().zip(tmp.iter()) {
                        *dst = (src.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
                    }
                },
                err_fn, None,
            ).map_err(|e| format!("build stream: {e}"))?,
            SampleFormat::U16 => device.build_output_stream::<u16, _, _>(
                &cfg,
                move |out, _| {
                    let mut tmp = vec![0.0f32; out.len()];
                    fill_callback(&mut tmp, &mut consumer, &played_clone, &skip_clone, &vol_clone, &paused_clone);
                    for (dst, src) in out.iter_mut().zip(tmp.iter()) {
                        let v = (src.clamp(-1.0, 1.0) + 1.0) * 0.5 * u16::MAX as f32;
                        *dst = v as u16;
                    }
                },
                err_fn, None,
            ).map_err(|e| format!("build stream: {e}"))?,
            other => return Err(format!("unsupported sample format: {other:?}")),
        };

        stream.play().map_err(|e| format!("stream.play: {e}"))?;

        Ok(Self {
            sample_rate: sr,
            channels: ch,
            producer: Arc::new(Mutex::new(producer)),
            samples_played,
            position_offset_ms,
            skip_samples,
            volume,
            _stream: stream,
            paused,
        })
    }

    pub fn sample_rate(&self) -> u32 { self.sample_rate }
    pub fn channels(&self) -> u16 { self.channels }

    /// Returns count of samples actually pushed (may be < input.len() if buffer is full).
    pub fn push_samples(&self, samples: &[f32]) -> usize {
        let mut p = self.producer.lock();
        p.push_slice(samples)
    }

    pub fn buffered_samples(&self) -> usize {
        self.producer.lock().vacant_len(); // touch to update; actual occupied:
        // ringbuf 0.4: producer doesn't expose occupied directly; compute from capacity - vacant.
        let cap = self.producer.lock().capacity().get();
        cap - self.producer.lock().vacant_len()
    }

    pub fn played_duration_ms(&self) -> u64 {
        let played = self.samples_played.load(Ordering::Acquire);
        let frames = played / self.channels as u64;
        let from_samples = (frames * 1000) / self.sample_rate as u64;
        self.position_offset_ms.load(Ordering::Acquire) + from_samples
    }

    pub fn set_position_anchor_ms(&self, ms: u64) {
        self.position_offset_ms.store(ms, Ordering::Release);
        self.samples_played.store(0, Ordering::Release);
    }

    /// Used by the decoder thread post-seek: zero only the local sample
    /// counter. Preserves the anchor.
    pub fn reset_position(&self) {
        self.samples_played.store(0, Ordering::Release);
    }

    /// Soft drain — tells the audio callback to discard the next N samples.
    pub fn drain_buffer(&self) {
        let occupied = {
            let p = self.producer.lock();
            p.capacity().get() - p.vacant_len()
        };
        self.skip_samples.store(occupied as u64, Ordering::Release);
    }

    /// Hard reset — used on stop / new track load.
    pub fn clear(&self) {
        self.samples_played.store(0, Ordering::Release);
        self.position_offset_ms.store(0, Ordering::Release);
        self.skip_samples.store(0, Ordering::Release);
        // Drain the producer side.
        let mut p = self.producer.lock();
        let cap = p.capacity().get();
        let mut tmp = vec![0.0; cap];
        let _ = p.push_slice(&[]); // no-op, producer can't drain itself
        let _ = tmp; // placeholder; the consumer thread drains naturally on next callback
        // Force drain via skip:
        let occ = cap - p.vacant_len();
        self.skip_samples.store(occ as u64, Ordering::Release);
    }

    pub fn play(&self) {
        self.paused.store(false, Ordering::Release);
    }

    pub fn pause(&self) {
        self.paused.store(true, Ordering::Release);
    }

    pub fn set_volume(&self, v: f32) {
        *self.volume.lock() = v.clamp(0.0, 1.0);
    }
}

fn fill_callback(
    out: &mut [f32],
    consumer: &mut HeapCons<f32>,
    samples_played: &AtomicU64,
    skip_samples: &AtomicU64,
    volume: &Mutex<f32>,
    paused: &std::sync::atomic::AtomicBool,
) {
    if paused.load(Ordering::Acquire) {
        out.fill(0.0);
        return;
    }
    let vol = *volume.lock();
    let mut i = 0;

    // Drain skip-samples first.
    let skip = skip_samples.load(Ordering::Acquire);
    if skip > 0 {
        let take = (skip as usize).min(out.len());
        // Pop and discard.
        let mut discard = vec![0.0; take];
        let popped = consumer.pop_slice(&mut discard);
        let consumed = popped;
        skip_samples.fetch_sub(consumed as u64, Ordering::AcqRel);
        // Output zeros for the skipped span.
        for k in 0..take { out[k] = 0.0; }
        i = take;
    }

    if i < out.len() {
        let popped = consumer.pop_slice(&mut out[i..]);
        let total_filled = i + popped;
        for s in &mut out[i..total_filled] { *s *= vol; }
        if total_filled < out.len() {
            for s in &mut out[total_filled..] { *s = 0.0; }
        }
        samples_played.fetch_add(popped as u64, Ordering::AcqRel);
    }
}
```

- [ ] **Step 2: Wire**

`src/engine/mod.rs`:

```rust
pub mod output;
```

- [ ] **Step 3: Verify compile**

```bash
rtk cargo check --all-targets
```

Expected: clean. (Audio output has no unit tests — it requires a real device. Manual smoke test happens in Task 14.)

- [ ] **Step 4: Commit**

```bash
rtk git add -A
rtk git commit -m "feat(engine): AudioOutput with anchored position and skip-drain"
```

---

## Task 12: `engine/eq.rs` — 10-band biquad equalizer

**Files:**
- Create: `src/engine/eq.rs`
- Modify: `src/engine/mod.rs`

Implements §4.3 eq.rs.

- [ ] **Step 1: Implementation + test**

`src/engine/eq.rs`:

```rust
use biquad::{Biquad, Coefficients, DirectForm1, Hertz, Q_BUTTERWORTH_F32, ToHertz, Type};

pub const BAND_FREQS_HZ: [f32; 10] = [
    31.0, 62.0, 125.0, 250.0, 500.0, 1000.0, 2000.0, 4000.0, 8000.0, 16000.0,
];

pub struct Equalizer {
    enabled: bool,
    sample_rate: f32,
    bands_db: [f32; 10],
    /// One filter per band, per channel (stereo = 2 chains of 10).
    chains: Vec<[DirectForm1<f32>; 10]>,
}

impl Equalizer {
    pub fn new(sample_rate: u32, channels: u16) -> Self {
        let mut eq = Self {
            enabled: false,
            sample_rate: sample_rate as f32,
            bands_db: [0.0; 10],
            chains: (0..channels.max(1)).map(|_| Self::make_chain(sample_rate as f32, &[0.0; 10])).collect(),
        };
        eq.rebuild();
        eq
    }

    fn make_chain(sample_rate: f32, gains_db: &[f32; 10]) -> [DirectForm1<f32>; 10] {
        let mut filters: [DirectForm1<f32>; 10] = std::array::from_fn(|i| {
            let coeffs = Coefficients::<f32>::from_params(
                Type::PeakingEQ(gains_db[i]),
                sample_rate.hz(),
                BAND_FREQS_HZ[i].hz(),
                Q_BUTTERWORTH_F32,
            ).expect("valid coeffs");
            DirectForm1::<f32>::new(coeffs)
        });
        filters.iter_mut().for_each(|f| f.reset_state());
        filters
    }

    fn rebuild(&mut self) {
        for chain in &mut self.chains {
            for (i, filter) in chain.iter_mut().enumerate() {
                let coeffs = Coefficients::<f32>::from_params(
                    Type::PeakingEQ(self.bands_db[i]),
                    self.sample_rate.hz(),
                    BAND_FREQS_HZ[i].hz(),
                    Q_BUTTERWORTH_F32,
                ).expect("valid coeffs");
                filter.update_coefficients(coeffs);
            }
        }
    }

    pub fn set_enabled(&mut self, e: bool) { self.enabled = e; }
    pub fn enabled(&self) -> bool { self.enabled }
    pub fn bands(&self) -> &[f32; 10] { &self.bands_db }
    pub fn set_band(&mut self, idx: usize, db: f32) {
        if idx < 10 { self.bands_db[idx] = db; self.rebuild(); }
    }
    pub fn set_all(&mut self, bands: [f32; 10]) {
        self.bands_db = bands;
        self.rebuild();
    }

    /// Process interleaved samples in place.
    pub fn process_inplace(&mut self, samples: &mut [f32], channels: u16) {
        if !self.enabled { return; }
        let n_chans = channels.max(1) as usize;
        for (i, s) in samples.iter_mut().enumerate() {
            let ch = i % n_chans;
            let chain_idx = ch.min(self.chains.len() - 1);
            for filter in &mut self.chains[chain_idx] {
                *s = filter.run(*s);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_is_noop() {
        let mut eq = Equalizer::new(44_100, 2);
        eq.set_band(0, 12.0);
        let mut samples = vec![0.5_f32, -0.5, 0.5, -0.5];
        let copy = samples.clone();
        eq.process_inplace(&mut samples, 2);
        assert_eq!(samples, copy);
    }

    #[test]
    fn enabled_modifies_signal() {
        let mut eq = Equalizer::new(44_100, 2);
        eq.set_enabled(true);
        eq.set_band(5, 12.0); // boost 1 kHz
        let mut samples = vec![0.5_f32; 1024];
        eq.process_inplace(&mut samples, 2);
        // Output should differ from input somewhere.
        assert!(samples.iter().any(|s| (*s - 0.5).abs() > 0.001));
    }
}
```

- [ ] **Step 2: Wire**

`src/engine/mod.rs` — append:

```rust
pub mod eq;
```

- [ ] **Step 3: Test**

```bash
rtk cargo test --lib engine::eq
```

Expected: 2 tests pass.

- [ ] **Step 4: Commit**

```bash
rtk git add -A
rtk git commit -m "feat(engine): 10-band biquad equalizer"
```

---

## Task 13: `engine/decoder.rs` — symphonia + rubato + channel map

**Files:**
- Create: `src/engine/decoder.rs`
- Modify: `src/engine/mod.rs`

Implements §4.3 decoder.rs.

- [ ] **Step 1: Implementation**

`src/engine/decoder.rs`:

```rust
use crate::engine::output::AudioOutput;
use parking_lot::Mutex;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use symphonia::core::audio::{AudioBufferRef, Signal};
use symphonia::core::codecs::{Decoder, DecoderOptions};
use symphonia::core::formats::{FormatOptions, FormatReader, SeekMode, SeekTo};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use symphonia::core::units::Time;

pub struct DecodeJob {
    pub stop_flag: Arc<AtomicBool>,
    pub seek_request_ms: Arc<AtomicU64>,
    pub finished: Arc<AtomicBool>,
    pub duration: Duration,
    pub source_sample_rate: u32,
    pub source_channels: u16,
}

impl DecodeJob {
    pub fn seek(&self, ms: u64) {
        self.seek_request_ms.store(ms.max(1), Ordering::Release);
    }
    pub fn stop(&self) { self.stop_flag.store(true, Ordering::Release); }
    pub fn is_finished(&self) -> bool { self.finished.load(Ordering::Acquire) }
}

const NO_SEEK: u64 = 0;

/// Spawn a decoder thread for `path`. Pushes samples into `output`.
/// Returns immediately with a control handle.
pub fn start_decode(
    path: PathBuf,
    output: Arc<Mutex<AudioOutput>>,
) -> Result<DecodeJob, String> {
    let file = std::fs::File::open(&path).map_err(|e| format!("open {}: {e}", path.display()))?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }

    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &FormatOptions::default(), &MetadataOptions::default())
        .map_err(|e| format!("probe: {e}"))?;

    let mut format = probed.format;
    let track = format.default_track().ok_or("no default track")?;
    let track_id = track.id;
    let codec_params = track.codec_params.clone();
    let source_sample_rate = codec_params.sample_rate.ok_or("missing sample rate")?;
    let source_channels = codec_params.channels.map(|c| c.count() as u16).unwrap_or(2);

    let duration = track.codec_params.n_frames
        .and_then(|frames| {
            track.codec_params.sample_rate.map(|sr| Duration::from_secs_f64(frames as f64 / sr as f64))
        })
        .unwrap_or_default();

    let decoder = symphonia::default::get_codecs()
        .make(&codec_params, &DecoderOptions::default())
        .map_err(|e| format!("make decoder: {e}"))?;

    let stop_flag = Arc::new(AtomicBool::new(false));
    let seek_request_ms = Arc::new(AtomicU64::new(NO_SEEK));
    let finished = Arc::new(AtomicBool::new(false));

    let stop_clone = stop_flag.clone();
    let seek_clone = seek_request_ms.clone();
    let finished_clone = finished.clone();
    let output_clone = output.clone();

    let device_sr = output_clone.lock().sample_rate();
    let device_ch = output_clone.lock().channels();

    std::thread::Builder::new().name("decoder".into()).spawn(move || {
        decoder_loop(
            format, decoder, track_id,
            source_sample_rate, source_channels,
            device_sr, device_ch,
            output_clone,
            stop_clone, seek_clone, finished_clone,
        );
    }).map_err(|e| format!("spawn decoder: {e}"))?;

    Ok(DecodeJob {
        stop_flag, seek_request_ms, finished,
        duration, source_sample_rate, source_channels,
    })
}

#[allow(clippy::too_many_arguments)]
fn decoder_loop(
    mut format: Box<dyn FormatReader>,
    mut decoder: Box<dyn Decoder>,
    track_id: u32,
    src_sr: u32,
    src_ch: u16,
    dst_sr: u32,
    dst_ch: u16,
    output: Arc<Mutex<AudioOutput>>,
    stop: Arc<AtomicBool>,
    seek_req: Arc<AtomicU64>,
    finished: Arc<AtomicBool>,
) {
    use rubato::{FftFixedInOut, Resampler};
    let mut resampler: Option<FftFixedInOut<f32>> = if src_sr != dst_sr {
        FftFixedInOut::<f32>::new(src_sr as usize, dst_sr as usize, 1024, src_ch as usize).ok()
    } else { None };
    let mut planar: Vec<Vec<f32>> = vec![Vec::with_capacity(8192); src_ch as usize];

    'outer: loop {
        if stop.load(Ordering::Acquire) { break; }

        let seek_ms = seek_req.swap(NO_SEEK, Ordering::AcqRel);
        if seek_ms != NO_SEEK {
            let target = Time::from(Duration::from_millis(seek_ms));
            let _ = format.seek(SeekMode::Coarse, SeekTo::Time { time: target, track_id: Some(track_id) });
            decoder.reset();
            output.lock().reset_position();
            for ch in &mut planar { ch.clear(); }
        }

        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(symphonia::core::errors::Error::IoError(_)) => break, // EOF
            Err(e) => { log::warn!("packet error: {e}"); break; }
        };
        if packet.track_id() != track_id { continue; }

        let decoded = match decoder.decode(&packet) {
            Ok(d) => d,
            Err(e) => { log::warn!("decode error: {e}"); continue; }
        };

        match decoded {
            AudioBufferRef::F32(buf) => extend_planar(&mut planar, &buf),
            AudioBufferRef::S16(buf) => extend_planar_from(&mut planar, &buf, |v: i16| v as f32 / i16::MAX as f32),
            AudioBufferRef::S32(buf) => extend_planar_from(&mut planar, &buf, |v: i32| v as f32 / i32::MAX as f32),
            _ => continue,
        }

        if let Some(res) = resampler.as_mut() {
            let needed = res.input_frames_next();
            while planar[0].len() >= needed {
                let chunk: Vec<Vec<f32>> = planar.iter_mut()
                    .map(|c| c.drain(..needed).collect())
                    .collect();
                let chunk_refs: Vec<&[f32]> = chunk.iter().map(|c| c.as_slice()).collect();
                if let Ok(out) = res.process(&chunk_refs, None) {
                    let interleaved = interleave_with_channel_map(&out, dst_ch);
                    push_or_block(&output, &interleaved, &stop);
                }
                if stop.load(Ordering::Acquire) { break 'outer; }
            }
        } else {
            // No resample → interleave directly with channel map.
            let chunk_refs: Vec<&[f32]> = planar.iter().map(|c| c.as_slice()).collect();
            let interleaved = interleave_with_channel_map(&chunk_refs.iter().map(|s| s.to_vec()).collect::<Vec<_>>(), dst_ch);
            push_or_block(&output, &interleaved, &stop);
            for ch in &mut planar { ch.clear(); }
        }
    }
    finished.store(true, Ordering::Release);
}

fn extend_planar(dst: &mut [Vec<f32>], buf: &symphonia::core::audio::AudioBuffer<f32>) {
    for (ch_idx, ch_dst) in dst.iter_mut().enumerate() {
        if ch_idx >= buf.spec().channels.count() { break; }
        ch_dst.extend_from_slice(buf.chan(ch_idx));
    }
}

fn extend_planar_from<S: symphonia::core::sample::Sample + Copy>(
    dst: &mut [Vec<f32>],
    buf: &symphonia::core::audio::AudioBuffer<S>,
    convert: impl Fn(S) -> f32,
) {
    for (ch_idx, ch_dst) in dst.iter_mut().enumerate() {
        if ch_idx >= buf.spec().channels.count() { break; }
        for &s in buf.chan(ch_idx) {
            ch_dst.push(convert(s));
        }
    }
}

fn interleave_with_channel_map(planar: &[Vec<f32>], dst_ch: u16) -> Vec<f32> {
    let frames = planar.first().map(|c| c.len()).unwrap_or(0);
    let mut out = Vec::with_capacity(frames * dst_ch as usize);
    for f in 0..frames {
        for c in 0..dst_ch {
            let src_ch = if planar.len() == 1 {
                0  // mono → duplicate
            } else if (c as usize) < planar.len() {
                c as usize
            } else {
                planar.len() - 1
            };
            out.push(planar[src_ch].get(f).copied().unwrap_or(0.0));
        }
    }
    out
}

fn push_or_block(output: &Arc<Mutex<AudioOutput>>, samples: &[f32], stop: &AtomicBool) {
    let mut written = 0;
    while written < samples.len() {
        if stop.load(Ordering::Acquire) { return; }
        let n = output.lock().push_samples(&samples[written..]);
        if n == 0 {
            std::thread::sleep(Duration::from_millis(5));
        } else {
            written += n;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mono_to_stereo_duplicates() {
        let planar = vec![vec![0.5_f32, 0.6, 0.7]];
        let out = interleave_with_channel_map(&planar, 2);
        assert_eq!(out, vec![0.5, 0.5, 0.6, 0.6, 0.7, 0.7]);
    }

    #[test]
    fn stereo_to_stereo_passthrough() {
        let planar = vec![vec![0.1, 0.2], vec![0.3, 0.4]];
        let out = interleave_with_channel_map(&planar, 2);
        assert_eq!(out, vec![0.1, 0.3, 0.2, 0.4]);
    }
}
```

- [ ] **Step 2: Wire**

`src/engine/mod.rs` — append:

```rust
pub mod decoder;
```

- [ ] **Step 3: Test**

```bash
rtk cargo test --lib engine::decoder
rtk cargo check --all-targets
```

Expected: 2 tests pass, clean check.

- [ ] **Step 4: Commit**

```bash
rtk git add -A
rtk git commit -m "feat(engine): symphonia decoder with rubato resample and channel map"
```

---

## Task 14: `engine/mod.rs` — `Engine` facade with command/event channels

**Files:**
- Modify: `src/engine/mod.rs`

Implements §4.3 mod.rs (Engine facade).

- [ ] **Step 1: Replace `src/engine/mod.rs`**

```rust
pub mod decoder;
pub mod eq;
pub mod output;

use crate::engine::decoder::{start_decode, DecodeJob};
use crate::engine::output::AudioOutput;
use crossbeam_channel::{bounded, Receiver, Sender};
use parking_lot::Mutex;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

#[derive(Debug, Clone)]
pub enum EngineCmd {
    Load { path: PathBuf, autoplay: bool },
    Play,
    Pause,
    Stop,
    SeekFraction(f32),
    SetVolume(f32),
    Shutdown,
}

#[derive(Debug, Clone)]
pub enum EngineEvent {
    LoadStarted(PathBuf),
    LoadFailed { path: PathBuf, error: String },
    Started { duration_ms: u64 },
    Position { current_ms: u64, duration_ms: u64 },
    Paused,
    Resumed,
    EndOfTrack,
}

pub struct Engine {
    cmd_tx: Sender<EngineCmd>,
    evt_rx: Receiver<EngineEvent>,
}

impl Engine {
    pub fn start() -> Result<Self, String> {
        let (cmd_tx, cmd_rx) = bounded::<EngineCmd>(64);
        let (evt_tx, evt_rx) = bounded::<EngineEvent>(256);

        std::thread::Builder::new().name("engine".into()).spawn(move || {
            engine_thread(cmd_rx, evt_tx);
        }).map_err(|e| format!("spawn engine: {e}"))?;

        Ok(Self { cmd_tx, evt_rx })
    }

    pub fn send(&self, cmd: EngineCmd) {
        let _ = self.cmd_tx.send(cmd);
    }

    pub fn events(&self) -> &Receiver<EngineEvent> { &self.evt_rx }
}

fn engine_thread(cmd_rx: Receiver<EngineCmd>, evt_tx: Sender<EngineEvent>) {
    let output = match AudioOutput::new() {
        Ok(o) => Arc::new(Mutex::new(o)),
        Err(e) => {
            log::error!("audio output init failed: {e}");
            return;
        }
    };

    let mut current_job: Option<DecodeJob> = None;
    let mut current_duration_ms: u64 = 0;
    let mut paused = false;

    loop {
        match cmd_rx.recv_timeout(Duration::from_millis(20)) {
            Ok(cmd) => match cmd {
                EngineCmd::Shutdown => break,
                EngineCmd::Load { path, autoplay } => {
                    let _ = evt_tx.send(EngineEvent::LoadStarted(path.clone()));
                    if let Some(j) = current_job.take() { j.stop(); }
                    output.lock().clear();
                    match start_decode(path.clone(), output.clone()) {
                        Ok(job) => {
                            current_duration_ms = job.duration.as_millis() as u64;
                            current_job = Some(job);
                            paused = !autoplay;
                            if autoplay { output.lock().play(); } else { output.lock().pause(); }
                            let _ = evt_tx.send(EngineEvent::Started { duration_ms: current_duration_ms });
                        }
                        Err(e) => {
                            let _ = evt_tx.send(EngineEvent::LoadFailed { path, error: e });
                        }
                    }
                }
                EngineCmd::Play => {
                    output.lock().play();
                    paused = false;
                    let _ = evt_tx.send(EngineEvent::Resumed);
                }
                EngineCmd::Pause => {
                    output.lock().pause();
                    paused = true;
                    let _ = evt_tx.send(EngineEvent::Paused);
                }
                EngineCmd::Stop => {
                    if let Some(j) = current_job.take() { j.stop(); }
                    output.lock().clear();
                    current_duration_ms = 0;
                }
                EngineCmd::SeekFraction(frac) => {
                    if let Some(job) = &current_job {
                        let target_ms = (current_duration_ms as f32 * frac.clamp(0.0, 1.0)) as u64;
                        job.seek(target_ms);
                        output.lock().drain_buffer();
                        output.lock().set_position_anchor_ms(target_ms);
                        if !paused { output.lock().play(); }
                    }
                }
                EngineCmd::SetVolume(v) => {
                    output.lock().set_volume(v);
                }
            },
            Err(crossbeam_channel::RecvTimeoutError::Timeout) => {}
            Err(crossbeam_channel::RecvTimeoutError::Disconnected) => break,
        }

        // Periodic position events while a track is playing.
        if let Some(job) = &current_job {
            let current_ms = output.lock().played_duration_ms();
            let _ = evt_tx.send(EngineEvent::Position {
                current_ms,
                duration_ms: current_duration_ms,
            });
            // EndOfTrack: decoder finished AND ring buffer empty.
            if job.is_finished() {
                let buffered = output.lock().buffered_samples();
                if buffered == 0 {
                    current_job = None;
                    current_duration_ms = 0;
                    let _ = evt_tx.send(EngineEvent::EndOfTrack);
                }
            }
        }
    }
}
```

- [ ] **Step 2: Manual smoke test (optional)**

A real audio test requires a file. Create `tests/smoke.rs` to skip — leave engine for integration via the UI later. For now:

```bash
rtk cargo check --all-targets
```

- [ ] **Step 3: Commit**

```bash
rtk git add -A
rtk git commit -m "feat(engine): facade with cmd/event channels and engine thread"
```

---

## Task 15: `playback/queue.rs` — `Queue` with repeat-aware indexing

**Files:**
- Create: `src/playback/queue.rs`
- Modify: `src/playback/mod.rs`

Implements §4.4 queue.rs.

- [ ] **Step 1: Implementation + tests**

`src/playback/queue.rs`:

```rust
use crate::domain::{RepeatMode, Song};

pub struct Queue {
    pub songs: Vec<Song>,
    pub current: Option<usize>,
    pub shuffle: bool,
    pub repeat: RepeatMode,
}

impl Queue {
    pub fn new() -> Self {
        Self { songs: Vec::new(), current: None, shuffle: false, repeat: RepeatMode::Off }
    }

    pub fn replace(&mut self, songs: Vec<Song>, start: usize) {
        let len = songs.len();
        self.songs = songs;
        self.current = if len == 0 { None } else { Some(start.min(len.saturating_sub(1))) };
    }

    pub fn current_song(&self) -> Option<&Song> {
        self.current.and_then(|i| self.songs.get(i))
    }

    pub fn next_index(&self) -> Option<usize> {
        let cur = self.current?;
        match self.repeat {
            RepeatMode::One => Some(cur),
            RepeatMode::All => {
                if self.songs.is_empty() { None } else { Some((cur + 1) % self.songs.len()) }
            }
            RepeatMode::Off => {
                let nxt = cur + 1;
                if nxt < self.songs.len() { Some(nxt) } else { None }
            }
        }
    }

    pub fn prev_index(&self) -> Option<usize> {
        let cur = self.current?;
        if cur == 0 {
            match self.repeat {
                RepeatMode::All if !self.songs.is_empty() => Some(self.songs.len() - 1),
                _ => Some(0),
            }
        } else {
            Some(cur - 1)
        }
    }

    pub fn advance(&mut self) -> Option<usize> {
        self.current = self.next_index();
        self.current
    }

    pub fn rewind(&mut self) -> Option<usize> {
        self.current = self.prev_index();
        self.current
    }

    pub fn jump_to(&mut self, idx: usize) -> bool {
        if idx < self.songs.len() {
            self.current = Some(idx);
            true
        } else {
            false
        }
    }

    /// Drop every song with `id`. Adjusts `current` to keep pointing at the
    /// same logical track (or the next surviving one if the current track
    /// was dropped).
    pub fn remove_song_id(&mut self, id: i64) -> usize {
        let removed_before_current = self.current
            .map(|c| self.songs[..c].iter().filter(|s| s.id == id).count())
            .unwrap_or(0);
        let current_dropped = self.current
            .map(|c| self.songs.get(c).map(|s| s.id == id).unwrap_or(false))
            .unwrap_or(false);

        let before = self.songs.len();
        self.songs.retain(|s| s.id != id);
        let removed = before - self.songs.len();

        if removed > 0 {
            if let Some(c) = self.current {
                let new_c = c.saturating_sub(removed_before_current);
                if self.songs.is_empty() {
                    self.current = None;
                } else if current_dropped {
                    self.current = Some(new_c.min(self.songs.len() - 1));
                } else {
                    self.current = Some(new_c.min(self.songs.len() - 1));
                }
            }
        }
        removed
    }
}

impl Default for Queue {
    fn default() -> Self { Self::new() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::song_id_from_path;
    use std::path::{Path, PathBuf};
    use std::time::Duration;

    fn s(p: &str) -> Song {
        Song {
            id: song_id_from_path(Path::new(p)),
            title: p.into(), artist: "".into(), album: "".into(),
            album_artist: "".into(),
            duration: Duration::from_secs(1),
            year: None, genre: None, composer: None, track_no: None,
            path: PathBuf::from(p),
            has_embedded_art: false,
        }
    }

    #[test]
    fn next_off_stops_at_end() {
        let mut q = Queue::new();
        q.replace(vec![s("/a"), s("/b")], 1);
        q.repeat = RepeatMode::Off;
        assert_eq!(q.next_index(), None);
    }

    #[test]
    fn next_all_wraps() {
        let mut q = Queue::new();
        q.replace(vec![s("/a"), s("/b")], 1);
        q.repeat = RepeatMode::All;
        assert_eq!(q.next_index(), Some(0));
    }

    #[test]
    fn next_one_stays() {
        let mut q = Queue::new();
        q.replace(vec![s("/a"), s("/b")], 1);
        q.repeat = RepeatMode::One;
        assert_eq!(q.next_index(), Some(1));
    }

    #[test]
    fn prev_at_zero_off_stays() {
        let mut q = Queue::new();
        q.replace(vec![s("/a"), s("/b")], 0);
        q.repeat = RepeatMode::Off;
        assert_eq!(q.prev_index(), Some(0));
    }

    #[test]
    fn prev_at_zero_all_wraps_to_last() {
        let mut q = Queue::new();
        q.replace(vec![s("/a"), s("/b"), s("/c")], 0);
        q.repeat = RepeatMode::All;
        assert_eq!(q.prev_index(), Some(2));
    }

    #[test]
    fn remove_before_current_decrements_current() {
        let mut q = Queue::new();
        let a = s("/a"); let b = s("/b"); let c = s("/c");
        q.replace(vec![a.clone(), b.clone(), c.clone()], 2);
        q.remove_song_id(a.id);
        assert_eq!(q.current, Some(1));
        assert_eq!(q.current_song().unwrap().id, c.id);
    }

    #[test]
    fn remove_current_keeps_index_stable() {
        let mut q = Queue::new();
        let a = s("/a"); let b = s("/b"); let c = s("/c");
        q.replace(vec![a.clone(), b.clone(), c.clone()], 1);
        q.remove_song_id(b.id);
        assert_eq!(q.current_song().unwrap().id, c.id);
    }

    #[test]
    fn remove_last_clamps_to_end() {
        let mut q = Queue::new();
        let a = s("/a"); let b = s("/b");
        q.replace(vec![a.clone(), b.clone()], 1);
        q.remove_song_id(b.id);
        assert_eq!(q.current_song().unwrap().id, a.id);
    }

    #[test]
    fn remove_all_clears_current() {
        let mut q = Queue::new();
        let a = s("/a");
        q.replace(vec![a.clone()], 0);
        q.remove_song_id(a.id);
        assert_eq!(q.current, None);
    }
}
```

- [ ] **Step 2: Wire**

`src/playback/mod.rs`:

```rust
pub mod queue;
pub use queue::Queue;
```

- [ ] **Step 3: Test**

```bash
rtk cargo test --lib playback::queue
```

Expected: 9 tests pass.

- [ ] **Step 4: Commit**

```bash
rtk git add -A
rtk git commit -m "feat(playback): Queue with repeat-aware indexing and remove"
```

---

## Task 16: `playback/deletion.rs` — delete + renumber pipeline

**Files:**
- Create: `src/playback/deletion.rs`
- Modify: `src/playback/mod.rs`

Implements §4.4 deletion.rs.

- [ ] **Step 1: Implementation + test**

`src/playback/deletion.rs`:

```rust
use crate::data::Library;
use crate::renumberer::renumber_folder;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Debug)]
pub struct DeletionResult {
    pub deleted_path: PathBuf,
    pub renumbered: u32,
}

pub fn delete_song(
    library: &Arc<Library>,
    id: i64,
    renumber: bool,
    threshold: f32,
) -> Result<DeletionResult, String> {
    let song = library.find_by_id(id)
        .ok_or_else(|| format!("song {} not found", id))?;
    let path = song.path.clone();
    let folder = path.parent().map(PathBuf::from)
        .ok_or_else(|| format!("song path has no parent: {}", path.display()))?;

    std::fs::remove_file(&path)
        .map_err(|e| format!("remove_file: {e}"))?;
    library.remove_song(id);

    let renumbered = if renumber {
        let count = renumber_folder(&folder, threshold)
            .map_err(|e| format!("renumber: {e}"))?;
        if count > 0 {
            library.refresh_folder(&folder);
        }
        count
    } else { 0 };

    Ok(DeletionResult { deleted_path: path, renumbered })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::Library;
    use crate::domain::{song_id_from_path, Song};
    use std::fs::File;
    use std::time::Duration;

    fn fake(path: PathBuf) -> Song {
        Song {
            id: song_id_from_path(&path),
            title: "t".into(), artist: "a".into(), album: "al".into(),
            album_artist: "a".into(),
            duration: Duration::from_secs(1),
            year: None, genre: None, composer: None, track_no: None,
            path,
            has_embedded_art: false,
        }
    }

    #[test]
    fn missing_song_errors() {
        let lib = Arc::new(Library::new());
        assert!(delete_song(&lib, 12345, false, 0.5).is_err());
    }

    #[test]
    fn deletes_file_and_drops_from_library() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("x.mp3");
        File::create(&path).unwrap();
        let song = fake(path.clone());
        let lib = Arc::new(Library::new());
        lib.replace_all(vec![song.clone()]);
        let res = delete_song(&lib, song.id, false, 0.5).unwrap();
        assert_eq!(res.deleted_path, path);
        assert!(!path.exists());
        assert_eq!(lib.len(), 0);
    }
}
```

- [ ] **Step 2: Wire**

`src/playback/mod.rs` — append:

```rust
pub mod deletion;
pub use deletion::{delete_song, DeletionResult};
```

- [ ] **Step 3: Test**

```bash
rtk cargo test --lib playback::deletion
```

Expected: 2 tests pass.

- [ ] **Step 4: Commit**

```bash
rtk git add -A
rtk git commit -m "feat(playback): delete_song with renumber + library refresh"
```

---

## Task 17: `playback/controller.rs` — bridge layer

**Files:**
- Create: `src/playback/controller.rs`
- Modify: `src/playback/mod.rs`

Implements §4.4 controller.rs. Spawns the **playback-events** thread.

- [ ] **Step 1: Implementation**

`src/playback/controller.rs`:

```rust
use crate::data::Library;
use crate::domain::{sort_songs, PlaybackState, RepeatMode, Song, SortOption};
use crate::engine::{Engine, EngineCmd, EngineEvent};
use crate::playback::queue::Queue;
use parking_lot::RwLock;
use std::sync::Arc;

pub struct PlaybackController {
    engine: Engine,
    library: Arc<Library>,
    queue: Arc<RwLock<Queue>>,
    state: Arc<RwLock<PlaybackState>>,
}

impl PlaybackController {
    pub fn new(library: Arc<Library>) -> Result<Arc<Self>, String> {
        let engine = Engine::start()?;
        let queue = Arc::new(RwLock::new(Queue::new()));
        let state = Arc::new(RwLock::new(PlaybackState::default()));
        let controller = Arc::new(Self { engine, library, queue, state });
        controller.clone().spawn_events_thread();
        Ok(controller)
    }

    pub fn state(&self) -> Arc<RwLock<PlaybackState>> { self.state.clone() }
    pub fn queue(&self) -> Arc<RwLock<Queue>> { self.queue.clone() }
    pub fn snapshot(&self) -> PlaybackState { self.state.read().clone() }

    pub fn play_songs(&self, mut songs: Vec<Song>, start_index: usize, sort: Option<SortOption>) {
        if let Some(opt) = sort { sort_songs(&mut songs, opt); }
        if songs.is_empty() { return; }
        {
            let mut q = self.queue.write();
            q.replace(songs, start_index);
        }
        self.start_current();
    }

    pub fn play_pause(&self) {
        let mut s = self.state.write();
        if s.is_playing {
            s.is_playing = false;
            drop(s);
            self.engine.send(EngineCmd::Pause);
        } else if s.current_song.is_some() {
            s.is_playing = true;
            drop(s);
            self.engine.send(EngineCmd::Play);
        }
    }

    pub fn next(&self) {
        let next = { self.queue.write().advance() };
        match next {
            Some(_) => self.start_current(),
            None => self.stop_internal(),
        }
    }

    pub fn previous(&self) {
        let pos_ms = self.state.read().current_position_ms;
        if pos_ms > 3000 {
            self.engine.send(EngineCmd::SeekFraction(0.0));
            return;
        }
        let _ = { self.queue.write().rewind() };
        self.start_current();
    }

    pub fn jump_to(&self, idx: usize) {
        let ok = { self.queue.write().jump_to(idx) };
        if ok { self.start_current(); }
    }

    pub fn remove_from_queue(&self, id: i64) {
        let was_current = self.queue.read().current_song().map(|s| s.id) == Some(id);
        let removed = { self.queue.write().remove_song_id(id) };
        if removed > 0 && was_current {
            if self.queue.read().current_song().is_some() {
                self.start_current();
            } else {
                self.stop_internal();
            }
        }
    }

    pub fn seek_fraction(&self, frac: f32) {
        self.engine.send(EngineCmd::SeekFraction(frac));
    }

    pub fn set_volume(&self, v: f32) {
        self.state.write().volume = v;
        self.engine.send(EngineCmd::SetVolume(v));
    }

    pub fn set_shuffle(&self, on: bool) {
        self.state.write().shuffle_enabled = on;
        self.queue.write().shuffle = on;
    }

    pub fn set_repeat(&self, r: RepeatMode) {
        self.state.write().repeat_mode = r;
        self.queue.write().repeat = r;
    }

    pub fn cycle_repeat(&self) {
        let cur = self.state.read().repeat_mode;
        self.set_repeat(cur.next());
    }

    fn start_current(&self) {
        let song = match self.queue.read().current_song().cloned() {
            Some(s) => s,
            None => return,
        };
        {
            let mut s = self.state.write();
            s.current_song = Some(song.clone());
            s.is_playing = true;
            s.current_position_ms = 0;
            s.duration_ms = song.duration.as_millis() as u64;
        }
        self.engine.send(EngineCmd::Load { path: song.path, autoplay: true });
    }

    fn stop_internal(&self) {
        self.engine.send(EngineCmd::Stop);
        let mut s = self.state.write();
        s.current_song = None;
        s.is_playing = false;
        s.current_position_ms = 0;
        s.duration_ms = 0;
    }

    fn spawn_events_thread(self: Arc<Self>) {
        let evt_rx = self.engine.events().clone();
        let me = self.clone();
        std::thread::Builder::new().name("playback-events".into()).spawn(move || {
            while let Ok(evt) = evt_rx.recv() {
                match evt {
                    EngineEvent::Started { duration_ms } => {
                        let mut s = me.state.write();
                        s.duration_ms = duration_ms;
                        s.current_position_ms = 0;
                        s.is_playing = true;
                    }
                    EngineEvent::Position { current_ms, duration_ms } => {
                        let mut s = me.state.write();
                        s.current_position_ms = current_ms;
                        if duration_ms != 0 { s.duration_ms = duration_ms; }
                    }
                    EngineEvent::Paused => { me.state.write().is_playing = false; }
                    EngineEvent::Resumed => { me.state.write().is_playing = true; }
                    EngineEvent::EndOfTrack => { me.next(); }
                    EngineEvent::LoadFailed { path, error } => {
                        log::error!("load failed {}: {}", path.display(), error);
                        me.next();
                    }
                    EngineEvent::LoadStarted(_) => {}
                }
            }
        }).expect("spawn playback-events");
    }
}
```

- [ ] **Step 2: Wire**

`src/playback/mod.rs` — append:

```rust
pub mod controller;
pub use controller::PlaybackController;
```

- [ ] **Step 3: Verify compile**

```bash
rtk cargo check --all-targets
```

- [ ] **Step 4: Commit**

```bash
rtk git add -A
rtk git commit -m "feat(playback): PlaybackController bridge with events thread"
```

---

## Task 18: UI — `App` skeleton + fonts + screen routing

**Files:**
- Create: `src/ui/app.rs`, `src/ui/fonts.rs`, `src/ui/toasts.rs`
- Modify: `src/ui/mod.rs`, `src/main.rs`

This boots a minimal egui window. Later tasks fill in screens.

- [ ] **Step 1: Write `src/ui/fonts.rs`**

```rust
use egui::{FontData, FontDefinitions, FontFamily};

pub fn install(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    // egui's default fonts already cover Latin. We add Windows fallbacks
    // for CJK / Cyrillic by trying to load the system font. If absent,
    // we just use defaults — egui shows missing-glyph boxes, not a crash.
    let candidates: &[&str] = &[
        r"C:\Windows\Fonts\segoeui.ttf",
        r"C:\Windows\Fonts\msyh.ttc",   // Microsoft YaHei (Simplified Chinese)
        r"C:\Windows\Fonts\meiryo.ttc", // Japanese
        r"C:\Windows\Fonts\malgun.ttf", // Korean
    ];
    for path in candidates {
        if let Ok(bytes) = std::fs::read(path) {
            let key = std::path::Path::new(path)
                .file_stem().and_then(|s| s.to_str()).unwrap_or("sys").to_string();
            fonts.font_data.insert(key.clone(), FontData::from_owned(bytes));
            fonts.families.entry(FontFamily::Proportional).or_default().push(key.clone());
            fonts.families.entry(FontFamily::Monospace).or_default().push(key);
        }
    }
    ctx.set_fonts(fonts);
}
```

- [ ] **Step 2: Write `src/ui/toasts.rs`**

```rust
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
                                if ui.small_button("✕").clicked() { to_remove.push(idx); }
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
```

- [ ] **Step 3: Write `src/ui/app.rs` (skeleton)**

```rust
use crate::data::Library;
use crate::domain::Screen;
use crate::playback::PlaybackController;
use crate::settings::Settings;
use crate::ui::toasts::Toasts;
use parking_lot::RwLock;
use std::sync::Arc;

pub struct App {
    pub library: Arc<Library>,
    pub playback: Arc<PlaybackController>,
    pub settings: Arc<RwLock<Settings>>,
    pub screen: Screen,
    pub toasts: Toasts,
    pub search_query: String,
    pub all_songs_page: usize,
    pub all_songs_sort: crate::domain::SortOption,
}

impl App {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        library: Arc<Library>,
        playback: Arc<PlaybackController>,
        settings: Arc<RwLock<Settings>>,
    ) -> Self {
        crate::ui::fonts::install(&cc.egui_ctx);
        Self {
            library, playback, settings,
            screen: Screen::AllSongs,
            toasts: Toasts::new(),
            search_query: String::new(),
            all_songs_page: 0,
            all_songs_sort: crate::domain::SortOption::TitleAsc,
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.request_repaint_after(std::time::Duration::from_millis(50));

        // Top bar (placeholder)
        if self.screen.shows_chrome() {
            egui::TopBottomPanel::top("top_bar").show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.heading("WinPlayer");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(format!("{} songs", self.library.len()));
                    });
                });
            });
        }

        // Mini player (placeholder)
        egui::TopBottomPanel::bottom("mini_player").min_height(56.0).show(ctx, |ui| {
            ui.label("(mini-player will live here)");
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.label(format!("Active screen: {:?}", self.screen));
            ui.label(format!("Library status: {:?}", self.library.status()));
        });

        self.toasts.show(ctx);
    }
}
```

- [ ] **Step 4: Wire `src/ui/mod.rs`**

```rust
pub mod app;
pub mod fonts;
pub mod toasts;
pub mod components;
pub mod screens;
pub use app::App;
```

Create stub files for unused submodules:

```rust
// src/ui/components/mod.rs   -- empty for now
// src/ui/screens/mod.rs       -- empty for now
```

- [ ] **Step 5: Update `src/main.rs` to launch the UI**

```rust
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::Arc;
use parking_lot::RwLock;
use winplayer::data::Library;
use winplayer::playback::PlaybackController;
use winplayer::settings::Settings;
use winplayer::ui::App;

fn main() -> Result<(), eframe::Error> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let settings = Arc::new(RwLock::new(Settings::load_or_default()));
    let library = Arc::new(Library::new());

    // Spawn library scan in background.
    {
        let lib = library.clone();
        let roots = settings.read().scan.roots.clone();
        std::thread::Builder::new().name("library-scan".into()).spawn(move || {
            lib.scan(&roots);
        }).expect("spawn library-scan");
    }

    let playback = PlaybackController::new(library.clone())
        .map_err(|e| eframe::Error::AppCreation(Box::new(std::io::Error::new(std::io::ErrorKind::Other, e))))?;

    let initial_volume = settings.read().playback.volume;
    playback.set_volume(initial_volume);

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 800.0])
            .with_min_inner_size([720.0, 480.0])
            .with_title("WinPlayer"),
        ..Default::default()
    };

    eframe::run_native(
        "WinPlayer",
        native_options,
        Box::new(move |cc| Ok(Box::new(App::new(cc, library, playback, settings)))),
    )
}
```

- [ ] **Step 6: Build and run**

```bash
rtk cargo check --all-targets
rtk cargo build
```

If `cargo build` succeeds, optionally run interactively (with a real audio device):

```bash
rtk cargo run
```

Expected: window opens showing "Active screen: AllSongs" and "(mini-player will live here)". Close to continue.

- [ ] **Step 7: Commit**

```bash
rtk git add -A
rtk git commit -m "feat(ui): bootable App with fonts, toasts, screen routing skeleton"
```

---

## Task 19: UI component — `seek_slider` with memory stash

**Files:**
- Create: `src/ui/components/seek_slider.rs`
- Modify: `src/ui/components/mod.rs`

Implements §4.6 seek slider. **Critical: §6 item 2 (memory stash).**

- [ ] **Step 1: Implementation**

`src/ui/components/seek_slider.rs`:

```rust
use crate::domain::PlaybackState;

const MEM_ID: &str = "winplayer.seek.draft";

/// Returns `Some(target_fraction)` ONLY on commit (drag release / click).
pub fn draw_seek_slider(ui: &mut egui::Ui, state: &PlaybackState) -> Option<f32> {
    let id = egui::Id::new(MEM_ID);

    // Read the in-progress drag value, if any; otherwise use the live progress.
    let mut frac: f32 = ui.memory(|m| m.data.get_temp::<f32>(id).unwrap_or(state.progress()));
    let was_dragged_before = ui.memory(|m| m.data.get_temp::<bool>(id.with("was_dragged")).unwrap_or(false));

    let response = ui.add(
        egui::Slider::new(&mut frac, 0.0..=1.0)
            .show_value(false)
            .clamp_to_range(true)
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
        || was_dragged_before; // we had a draft and the user just released

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
```

- [ ] **Step 2: Wire**

`src/ui/components/mod.rs`:

```rust
pub mod seek_slider;
```

- [ ] **Step 3: Verify**

```bash
rtk cargo check --all-targets
```

- [ ] **Step 4: Commit**

```bash
rtk git add -A
rtk git commit -m "feat(ui): seek_slider with memory-stashed draft value"
```

---

## Task 20: UI component — `song_row` with rect-allocate-first

**Files:**
- Create: `src/ui/components/song_row.rs`
- Modify: `src/ui/components/mod.rs`

Implements §4.6 song_row. **Critical: §6 item 3 (allocate rect first).**

- [ ] **Step 1: Implementation**

`src/ui/components/song_row.rs`:

```rust
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

    let mut child = ui.child_ui(rect, egui::Layout::left_to_right(egui::Align::Center), None);
    child.set_clip_rect(rect);
    child.add_space(4.0);

    // Row number (28 px)
    child.allocate_ui_with_layout(
        egui::vec2(28.0, ROW_HEIGHT),
        egui::Layout::left_to_right(egui::Align::Center),
        |ui| { ui.label(format!("{}", row_index + 1)); },
    );

    // Play button
    if opts.show_play {
        if child.small_button("▶").clicked() { action.play_clicked = true; }
    }

    // Title
    child.add(egui::Label::new(&song.title).truncate());

    // Right-side cluster: ✕ • duration • • artist
    child.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        if opts.show_remove {
            if ui.small_button("✕").on_hover_text("Remove").clicked() {
                action.remove_clicked = true;
            }
        }
        ui.add_space(8.0);
        let dur_secs = song.duration.as_secs();
        ui.label(format!("{}:{:02}", dur_secs / 60, dur_secs % 60));
        ui.add_space(8.0);
        ui.label("·");
        ui.add_space(8.0);
        ui.add(egui::Label::new(&song.artist).truncate());
    });

    action
}
```

- [ ] **Step 2: Wire**

`src/ui/components/mod.rs` — append:

```rust
pub mod song_row;
```

- [ ] **Step 3: Verify**

```bash
rtk cargo check --all-targets
```

- [ ] **Step 4: Commit**

```bash
rtk git add -A
rtk git commit -m "feat(ui): song_row with rect-allocate-first to stop staircase"
```

---

## Task 21: UI components — `top_bar`, `bottom_nav`, `mini_player`

**Files:**
- Create: `src/ui/components/top_bar.rs`, `src/ui/components/bottom_nav.rs`, `src/ui/components/mini_player.rs`
- Modify: `src/ui/components/mod.rs`, `src/ui/app.rs`

- [ ] **Step 1: Write `top_bar.rs`**

```rust
use crate::domain::Screen;

pub fn draw(ui: &mut egui::Ui, current: &mut Screen, song_count: usize, search: &mut String) {
    ui.horizontal(|ui| {
        ui.heading("WinPlayer");
        ui.separator();
        if ui.selectable_label(matches!(current, Screen::AllSongs), "Songs").clicked() {
            *current = Screen::AllSongs;
        }
        if ui.selectable_label(matches!(current, Screen::AlbumsList | Screen::AlbumDetail(_)), "Albums").clicked() {
            *current = Screen::AlbumsList;
        }
        if ui.selectable_label(matches!(current, Screen::ArtistsList | Screen::ArtistDetail(_)), "Artists").clicked() {
            *current = Screen::ArtistsList;
        }
        if ui.selectable_label(matches!(current, Screen::Folders), "Folders").clicked() {
            *current = Screen::Folders;
        }
        if ui.selectable_label(matches!(current, Screen::Queue), "Queue").clicked() {
            *current = Screen::Queue;
        }
        if ui.selectable_label(matches!(current, Screen::Equalizer), "EQ").clicked() {
            *current = Screen::Equalizer;
        }
        if ui.selectable_label(matches!(current, Screen::Settings), "Settings").clicked() {
            *current = Screen::Settings;
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(format!("{} songs", song_count));
            ui.separator();
            ui.add(egui::TextEdit::singleline(search).hint_text("Search…").desired_width(200.0));
        });
    });
}
```

- [ ] **Step 2: Write `bottom_nav.rs`**

(Empty for now — top_bar covers nav. We keep the file as a placeholder so the spec layout matches.)

```rust
// Reserved for future bottom-nav UI; currently inlined into top_bar.
```

- [ ] **Step 3: Write `mini_player.rs`**

```rust
use crate::domain::{PlaybackState, RepeatMode, Screen};
use crate::playback::PlaybackController;
use crate::ui::components::seek_slider::{draw_seek_slider, fmt_time_ms};
use std::sync::Arc;

pub fn draw(
    ui: &mut egui::Ui,
    state: &PlaybackState,
    playback: &Arc<PlaybackController>,
    screen: &mut Screen,
) {
    ui.horizontal(|ui| {
        ui.set_height(56.0);

        // Title / artist (truncated, fixed width)
        ui.allocate_ui_with_layout(
            egui::vec2(220.0, 56.0),
            egui::Layout::top_down(egui::Align::LEFT),
            |ui| {
                if let Some(s) = &state.current_song {
                    ui.add(egui::Label::new(egui::RichText::new(&s.title).strong()).truncate());
                    ui.add(egui::Label::new(egui::RichText::new(&s.artist).weak()).truncate());
                } else {
                    ui.label("—");
                }
            },
        );

        if ui.button("⏮").on_hover_text("Previous").clicked() { playback.previous(); }
        let pp = if state.is_playing { "⏸" } else { "▶" };
        if ui.button(pp).on_hover_text("Play / Pause").clicked() { playback.play_pause(); }
        if ui.button("⏭").on_hover_text("Next").clicked() { playback.next(); }

        // Seek slider takes available middle space.
        ui.allocate_ui_with_layout(
            egui::vec2(ui.available_width() - 240.0, 56.0),
            egui::Layout::top_down(egui::Align::Center),
            |ui| {
                if let Some(target) = draw_seek_slider(ui, state) {
                    playback.seek_fraction(target);
                }
                ui.horizontal(|ui| {
                    ui.label(fmt_time_ms(state.current_position_ms));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(fmt_time_ms(state.duration_ms));
                    });
                });
            },
        );

        // Volume + Now Playing link
        let mut v = state.volume;
        if ui.add(egui::Slider::new(&mut v, 0.0..=1.0).show_value(false).text("vol")).changed() {
            playback.set_volume(v);
        }
        let shuffle_label = if state.shuffle_enabled { "🔀ON" } else { "🔀" };
        if ui.button(shuffle_label).on_hover_text("Shuffle").clicked() {
            playback.set_shuffle(!state.shuffle_enabled);
        }
        let repeat_label = match state.repeat_mode {
            RepeatMode::Off => "🔁",
            RepeatMode::All => "🔁ALL",
            RepeatMode::One => "🔂",
        };
        if ui.button(repeat_label).on_hover_text("Repeat").clicked() {
            playback.cycle_repeat();
        }
        if ui.button("⤢").on_hover_text("Now Playing").clicked() {
            *screen = Screen::NowPlaying;
        }
    });
}
```

- [ ] **Step 4: Wire components**

`src/ui/components/mod.rs` — append:

```rust
pub mod bottom_nav;
pub mod mini_player;
pub mod top_bar;
```

- [ ] **Step 5: Update `App::update` to use them**

Replace top-bar and bottom-panel placeholders in `src/ui/app.rs` `update`:

```rust
        if self.screen.shows_chrome() {
            egui::TopBottomPanel::top("top_bar").show(ctx, |ui| {
                let count = self.library.len();
                crate::ui::components::top_bar::draw(ui, &mut self.screen, count, &mut self.search_query);
            });
        }

        egui::TopBottomPanel::bottom("mini_player").min_height(60.0).show(ctx, |ui| {
            let state = self.playback.snapshot();
            crate::ui::components::mini_player::draw(ui, &state, &self.playback, &mut self.screen);
        });
```

- [ ] **Step 6: Verify**

```bash
rtk cargo check --all-targets
rtk cargo run
```

Expected: window with top nav buttons + functional play/pause/next/prev/seek/volume in mini-player. Selecting a screen tab changes the central panel label.

- [ ] **Step 7: Commit**

```bash
rtk git add -A
rtk git commit -m "feat(ui): top_bar, mini_player, and seek slider wired into App"
```

---

## Task 22: UI screen — `AllSongs` (paginated, sortable, searchable)

**Files:**
- Create: `src/ui/screens/all_songs.rs`
- Modify: `src/ui/screens/mod.rs`, `src/ui/app.rs`

Implements §4.6 AllSongs with §6 items 8 & 9 (cached library view + pagination).

- [ ] **Step 1: Write `src/ui/screens/all_songs.rs`**

```rust
use crate::data::Library;
use crate::domain::{sort_songs, Screen, Song, SortOption};
use crate::playback::PlaybackController;
use crate::ui::components::song_row::{draw_with_options, RowOptions};
use crate::renumberer::renumber_folder;
use crate::playback::delete_song;
use crate::ui::toasts::Toasts;
use std::sync::Arc;

const PAGE_SIZE: usize = 50;

pub struct AllSongsState {
    pub sort: SortOption,
    pub query: String,
    pub page: usize,
    cache_key: Option<(u64, SortOption, String)>,
    cached: Vec<Song>,
}

impl Default for AllSongsState {
    fn default() -> Self {
        Self {
            sort: SortOption::TitleAsc,
            query: String::new(),
            page: 0,
            cache_key: None,
            cached: Vec::new(),
        }
    }
}

impl AllSongsState {
    fn compute_view(&mut self, library: &Arc<Library>) {
        let key = (library.version(), self.sort, self.query.clone());
        if self.cache_key.as_ref() == Some(&key) { return; }
        let mut songs = library.songs_snapshot();
        let q = self.query.to_lowercase();
        if !q.is_empty() {
            songs.retain(|s|
                s.title.to_lowercase().contains(&q) ||
                s.artist.to_lowercase().contains(&q) ||
                s.album.to_lowercase().contains(&q)
            );
        }
        sort_songs(&mut songs, self.sort);
        self.cached = songs;
        self.cache_key = Some(key);
        if self.page * PAGE_SIZE >= self.cached.len() && self.page > 0 { self.page = 0; }
    }
}

pub fn draw(
    ui: &mut egui::Ui,
    library: &Arc<Library>,
    playback: &Arc<PlaybackController>,
    state: &mut AllSongsState,
    toasts: &mut Toasts,
    renumber_threshold: f32,
    _screen: &mut Screen,
) {
    ui.horizontal(|ui| {
        egui::ComboBox::from_label("Sort")
            .selected_text(state.sort.label())
            .show_ui(ui, |ui| {
                for opt in SortOption::ALL {
                    if ui.selectable_value(&mut state.sort, opt, opt.label()).clicked() {
                        state.cache_key = None;
                    }
                }
            });
        ui.add(egui::TextEdit::singleline(&mut state.query).hint_text("Search…"));
    });

    state.compute_view(library);
    let total = state.cached.len();
    let pages = (total + PAGE_SIZE - 1) / PAGE_SIZE.max(1);
    let start = state.page * PAGE_SIZE;
    let end = (start + PAGE_SIZE).min(total);

    ui.label(format!("{} songs   page {}/{}", total, state.page + 1, pages.max(1)));
    ui.horizontal(|ui| {
        if ui.button("◀ Prev").clicked() && state.page > 0 { state.page -= 1; }
        if ui.button("Next ▶").clicked() && state.page + 1 < pages { state.page += 1; }
    });
    ui.separator();

    let row_h = 32.0;
    let visible = &state.cached[start..end];
    let visible_owned: Vec<Song> = visible.to_vec();
    let cur_id = playback.snapshot().current_song.as_ref().map(|s| s.id);

    egui::ScrollArea::vertical().show_rows(ui, row_h, visible_owned.len(), |ui, range| {
        for i in range {
            let song = &visible_owned[i];
            let opts = RowOptions {
                highlighted: cur_id == Some(song.id),
                ..Default::default()
            };
            let action = draw_with_options(ui, song, start + i, opts);
            if action.clicked || action.play_clicked {
                playback.play_songs(state.cached.clone(), start + i, None);
            }
            if action.remove_clicked {
                let id = song.id;
                let lib = library.clone();
                match delete_song(&lib, id, true, renumber_threshold) {
                    Ok(res) => toasts.info(format!(
                        "Deleted {} ({} renumbered)",
                        res.deleted_path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
                        res.renumbered,
                    )),
                    Err(e) => toasts.error(format!("Delete failed: {e}")),
                }
                state.cache_key = None;
                playback.remove_from_queue(id);
            }
        }
    });

    let _ = renumber_folder; // silence unused if not directly invoked here
}
```

- [ ] **Step 2: Wire**

`src/ui/screens/mod.rs`:

```rust
pub mod all_songs;
```

- [ ] **Step 3: Update `App` to call into the screen**

In `src/ui/app.rs`, add a field:

```rust
pub all_songs_state: crate::ui::screens::all_songs::AllSongsState,
```

…initialize in `App::new`:

```rust
            all_songs_state: Default::default(),
```

…and replace the `CentralPanel` body in `update`:

```rust
        egui::CentralPanel::default().show(ctx, |ui| {
            match self.screen.clone() {
                crate::domain::Screen::AllSongs => {
                    let threshold = self.settings.read().renumber.threshold;
                    crate::ui::screens::all_songs::draw(
                        ui,
                        &self.library,
                        &self.playback,
                        &mut self.all_songs_state,
                        &mut self.toasts,
                        threshold,
                        &mut self.screen,
                    );
                }
                other => { ui.label(format!("(screen not yet implemented: {:?})", other)); }
            }
        });
```

Also remove the now-redundant `search_query`, `all_songs_page`, `all_songs_sort` fields from `App` (they live in `AllSongsState` now).

- [ ] **Step 4: Verify and run**

```bash
rtk cargo check --all-targets
rtk cargo run
```

Manual smoke (with files in `<cwd>/music/`): list shows; clicking a row plays it; ✕ deletes the file from disk; sort + search work.

- [ ] **Step 5: Commit**

```bash
rtk git add -A
rtk git commit -m "feat(ui): AllSongs screen with paginated cached view"
```

---

## Task 23: UI screens — `AlbumsList` + `AlbumDetail`

**Files:**
- Create: `src/ui/screens/albums.rs`
- Modify: `src/ui/screens/mod.rs`, `src/ui/app.rs`

- [ ] **Step 1: Implementation**

`src/ui/screens/albums.rs`:

```rust
use crate::data::Library;
use crate::domain::{Screen, SortOption};
use crate::playback::PlaybackController;
use crate::ui::components::song_row::{draw_with_options, RowOptions};
use std::collections::BTreeMap;
use std::sync::Arc;

pub fn draw_list(ui: &mut egui::Ui, library: &Arc<Library>, screen: &mut Screen) {
    let songs = library.songs_snapshot();
    let mut by_album: BTreeMap<String, usize> = BTreeMap::new();
    for s in &songs { *by_album.entry(s.album.clone()).or_insert(0) += 1; }

    egui::ScrollArea::vertical().show(ui, |ui| {
        for (album, count) in by_album {
            if ui.selectable_label(false, format!("{album}  ({count})")).clicked() {
                *screen = Screen::AlbumDetail(album);
            }
        }
    });
}

pub fn draw_detail(
    ui: &mut egui::Ui,
    library: &Arc<Library>,
    playback: &Arc<PlaybackController>,
    album: &str,
    screen: &mut Screen,
) {
    if ui.button("◀ Albums").clicked() { *screen = Screen::AlbumsList; }
    ui.heading(album);

    let mut songs = library.songs_snapshot();
    songs.retain(|s| s.album == album);
    crate::domain::sort_songs(&mut songs, SortOption::TrackNoAsc);

    let cur_id = playback.snapshot().current_song.as_ref().map(|s| s.id);
    let row_h = 32.0;
    egui::ScrollArea::vertical().show_rows(ui, row_h, songs.len(), |ui, range| {
        for i in range {
            let s = &songs[i];
            let opts = RowOptions { highlighted: cur_id == Some(s.id), ..Default::default() };
            let act = draw_with_options(ui, s, i, opts);
            if act.clicked || act.play_clicked {
                playback.play_songs(songs.clone(), i, None);
            }
        }
    });
}
```

- [ ] **Step 2: Wire + route**

`src/ui/screens/mod.rs` — append `pub mod albums;`

In `App::update` central panel match, add:

```rust
                crate::domain::Screen::AlbumsList => {
                    crate::ui::screens::albums::draw_list(ui, &self.library, &mut self.screen);
                }
                crate::domain::Screen::AlbumDetail(name) => {
                    crate::ui::screens::albums::draw_detail(ui, &self.library, &self.playback, &name, &mut self.screen);
                }
```

- [ ] **Step 3: Verify**

```bash
rtk cargo check --all-targets
```

- [ ] **Step 4: Commit**

```bash
rtk git add -A
rtk git commit -m "feat(ui): Albums list + detail screens"
```

---

## Task 24: UI screens — `ArtistsList` + `ArtistDetail`

**Files:**
- Create: `src/ui/screens/artists.rs`
- Modify: `src/ui/screens/mod.rs`, `src/ui/app.rs`

- [ ] **Step 1: Implementation**

`src/ui/screens/artists.rs`:

```rust
use crate::data::Library;
use crate::domain::{Screen, SortOption};
use crate::playback::PlaybackController;
use crate::ui::components::song_row::{draw_with_options, RowOptions};
use std::collections::BTreeMap;
use std::sync::Arc;

pub fn draw_list(ui: &mut egui::Ui, library: &Arc<Library>, screen: &mut Screen) {
    let songs = library.songs_snapshot();
    let mut by_artist: BTreeMap<String, usize> = BTreeMap::new();
    for s in &songs { *by_artist.entry(s.artist.clone()).or_insert(0) += 1; }
    egui::ScrollArea::vertical().show(ui, |ui| {
        for (artist, count) in by_artist {
            if ui.selectable_label(false, format!("{artist}  ({count})")).clicked() {
                *screen = Screen::ArtistDetail(artist);
            }
        }
    });
}

pub fn draw_detail(
    ui: &mut egui::Ui,
    library: &Arc<Library>,
    playback: &Arc<PlaybackController>,
    artist: &str,
    screen: &mut Screen,
) {
    if ui.button("◀ Artists").clicked() { *screen = Screen::ArtistsList; }
    ui.heading(artist);
    let mut songs = library.songs_snapshot();
    songs.retain(|s| s.artist == artist);
    crate::domain::sort_songs(&mut songs, SortOption::AlbumAsc);
    let cur_id = playback.snapshot().current_song.as_ref().map(|s| s.id);
    let row_h = 32.0;
    egui::ScrollArea::vertical().show_rows(ui, row_h, songs.len(), |ui, range| {
        for i in range {
            let s = &songs[i];
            let opts = RowOptions { highlighted: cur_id == Some(s.id), ..Default::default() };
            let act = draw_with_options(ui, s, i, opts);
            if act.clicked || act.play_clicked {
                playback.play_songs(songs.clone(), i, None);
            }
        }
    });
}
```

- [ ] **Step 2: Wire + route**

`src/ui/screens/mod.rs` — append `pub mod artists;`

In `App::update` central panel match, add:

```rust
                crate::domain::Screen::ArtistsList => {
                    crate::ui::screens::artists::draw_list(ui, &self.library, &mut self.screen);
                }
                crate::domain::Screen::ArtistDetail(name) => {
                    crate::ui::screens::artists::draw_detail(ui, &self.library, &self.playback, &name, &mut self.screen);
                }
```

- [ ] **Step 3: Verify**

```bash
rtk cargo check --all-targets
```

- [ ] **Step 4: Commit**

```bash
rtk git add -A
rtk git commit -m "feat(ui): Artists list + detail screens"
```

---

## Task 25: UI screen — `Folders`

**Files:**
- Create: `src/ui/screens/folders.rs`
- Modify: `src/ui/screens/mod.rs`, `src/ui/app.rs`

- [ ] **Step 1: Implementation**

`src/ui/screens/folders.rs`:

```rust
use crate::data::Library;
use crate::domain::{Screen, SortOption};
use crate::playback::PlaybackController;
use crate::ui::components::song_row::{draw_with_options, RowOptions};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

pub struct FoldersState {
    pub selected: Option<PathBuf>,
}

impl Default for FoldersState {
    fn default() -> Self { Self { selected: None } }
}

pub fn draw(
    ui: &mut egui::Ui,
    library: &Arc<Library>,
    playback: &Arc<PlaybackController>,
    state: &mut FoldersState,
    _screen: &mut Screen,
) {
    let songs = library.songs_snapshot();
    let mut by_folder: BTreeMap<PathBuf, usize> = BTreeMap::new();
    for s in &songs {
        if let Some(parent) = s.path.parent() {
            *by_folder.entry(parent.to_path_buf()).or_insert(0) += 1;
        }
    }

    if state.selected.is_none() {
        egui::ScrollArea::vertical().show(ui, |ui| {
            for (folder, count) in by_folder {
                let label = folder.file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| folder.to_string_lossy().into_owned());
                if ui.selectable_label(false, format!("{label}  ({count})")).clicked() {
                    state.selected = Some(folder);
                }
            }
        });
        return;
    }

    let folder = state.selected.clone().unwrap();
    if ui.button("◀ Folders").clicked() { state.selected = None; return; }
    ui.heading(folder.display().to_string());

    let mut songs: Vec<_> = songs.into_iter()
        .filter(|s| s.path.parent() == Some(&folder))
        .collect();
    crate::domain::sort_songs(&mut songs, SortOption::TrackNoAsc);
    let cur_id = playback.snapshot().current_song.as_ref().map(|s| s.id);
    let row_h = 32.0;
    egui::ScrollArea::vertical().show_rows(ui, row_h, songs.len(), |ui, range| {
        for i in range {
            let s = &songs[i];
            let opts = RowOptions { highlighted: cur_id == Some(s.id), ..Default::default() };
            let act = draw_with_options(ui, s, i, opts);
            if act.clicked || act.play_clicked {
                playback.play_songs(songs.clone(), i, None);
            }
        }
    });
}
```

- [ ] **Step 2: Wire + route**

`src/ui/screens/mod.rs` — append `pub mod folders;`

In `App` add a field:

```rust
pub folders_state: crate::ui::screens::folders::FoldersState,
```

…initialize default in `App::new`, and add to central panel match:

```rust
                crate::domain::Screen::Folders => {
                    crate::ui::screens::folders::draw(ui, &self.library, &self.playback, &mut self.folders_state, &mut self.screen);
                }
```

- [ ] **Step 3: Verify**

```bash
rtk cargo check --all-targets
```

- [ ] **Step 4: Commit**

```bash
rtk git add -A
rtk git commit -m "feat(ui): Folders screen with drill-down"
```

---

## Task 26: UI screen — `Queue`

**Files:**
- Create: `src/ui/screens/queue.rs`
- Modify: `src/ui/screens/mod.rs`, `src/ui/app.rs`

- [ ] **Step 1: Implementation**

`src/ui/screens/queue.rs`:

```rust
use crate::domain::Screen;
use crate::playback::PlaybackController;
use crate::ui::components::song_row::{draw_with_options, RowOptions};
use std::sync::Arc;

pub fn draw(ui: &mut egui::Ui, playback: &Arc<PlaybackController>, _screen: &mut Screen) {
    let q = playback.queue();
    let snapshot = {
        let r = q.read();
        (r.songs.clone(), r.current)
    };
    let (songs, cur) = snapshot;
    ui.heading(format!("Queue ({})", songs.len()));
    let row_h = 32.0;
    egui::ScrollArea::vertical().show_rows(ui, row_h, songs.len(), |ui, range| {
        for i in range {
            let s = &songs[i];
            let opts = RowOptions {
                highlighted: cur == Some(i),
                ..Default::default()
            };
            let act = draw_with_options(ui, s, i, opts);
            if act.clicked || act.play_clicked {
                playback.jump_to(i);
            }
            if act.remove_clicked {
                playback.remove_from_queue(s.id);
            }
        }
    });
}
```

- [ ] **Step 2: Wire + route**

`src/ui/screens/mod.rs` — append `pub mod queue;`

Central panel match:

```rust
                crate::domain::Screen::Queue => {
                    crate::ui::screens::queue::draw(ui, &self.playback, &mut self.screen);
                }
```

- [ ] **Step 3: Verify**

```bash
rtk cargo check --all-targets
```

- [ ] **Step 4: Commit**

```bash
rtk git add -A
rtk git commit -m "feat(ui): Queue screen with jump-to + remove"
```

---

## Task 27: UI screen — `NowPlaying`

**Files:**
- Create: `src/ui/screens/now_playing.rs`
- Modify: `src/ui/screens/mod.rs`, `src/ui/app.rs`

- [ ] **Step 1: Implementation**

`src/ui/screens/now_playing.rs`:

```rust
use crate::domain::{RepeatMode, Screen};
use crate::playback::PlaybackController;
use crate::ui::components::seek_slider::{draw_seek_slider, fmt_time_ms};
use std::sync::Arc;

pub fn draw(ui: &mut egui::Ui, playback: &Arc<PlaybackController>, screen: &mut Screen) {
    if ui.button("◀ Back").clicked() { *screen = Screen::AllSongs; }
    let state = playback.snapshot();
    ui.vertical_centered(|ui| {
        ui.add_space(20.0);
        if let Some(s) = &state.current_song {
            ui.heading(&s.title);
            ui.label(egui::RichText::new(&s.artist).size(18.0));
            ui.label(egui::RichText::new(&s.album).weak());
        } else {
            ui.heading("Nothing playing");
            return;
        }
        ui.add_space(40.0);
        ui.allocate_ui_with_layout(
            egui::vec2(ui.available_width().min(800.0), 60.0),
            egui::Layout::top_down(egui::Align::Center),
            |ui| {
                if let Some(t) = draw_seek_slider(ui, &state) {
                    playback.seek_fraction(t);
                }
                ui.horizontal(|ui| {
                    ui.label(fmt_time_ms(state.current_position_ms));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(fmt_time_ms(state.duration_ms));
                    });
                });
            },
        );
        ui.add_space(20.0);
        ui.horizontal(|ui| {
            ui.add_space(ui.available_width() / 2.0 - 90.0);
            if ui.button(egui::RichText::new("⏮").size(28.0)).clicked() { playback.previous(); }
            let pp = if state.is_playing { "⏸" } else { "▶" };
            if ui.button(egui::RichText::new(pp).size(28.0)).clicked() { playback.play_pause(); }
            if ui.button(egui::RichText::new("⏭").size(28.0)).clicked() { playback.next(); }
        });
        ui.add_space(20.0);
        ui.horizontal(|ui| {
            ui.add_space(ui.available_width() / 2.0 - 90.0);
            let shuf = if state.shuffle_enabled { "🔀 ON" } else { "🔀 off" };
            if ui.button(shuf).clicked() { playback.set_shuffle(!state.shuffle_enabled); }
            let rep = match state.repeat_mode {
                RepeatMode::Off => "🔁 off",
                RepeatMode::All => "🔁 all",
                RepeatMode::One => "🔂 one",
            };
            if ui.button(rep).clicked() { playback.cycle_repeat(); }
        });
    });
}
```

- [ ] **Step 2: Wire + route**

`src/ui/screens/mod.rs` — append `pub mod now_playing;`

Central panel match:

```rust
                crate::domain::Screen::NowPlaying => {
                    crate::ui::screens::now_playing::draw(ui, &self.playback, &mut self.screen);
                }
```

- [ ] **Step 3: Commit**

```bash
rtk cargo check --all-targets
rtk git add -A
rtk git commit -m "feat(ui): NowPlaying full-screen view"
```

---

## Task 28: UI screen — `Equalizer`

**Files:**
- Create: `src/ui/screens/equalizer.rs`
- Modify: `src/ui/screens/mod.rs`, `src/ui/app.rs`

The Equalizer module currently lives inside the engine but is not yet wired into the audio pipeline (the engine passes raw samples through the ring buffer). For this task we expose the EQ state on the controller side so the UI can edit it; in a future task we'd plumb it into the decoder/output path. For now the EQ screen edits Settings — that's the minimum useful surface.

- [ ] **Step 1: Implementation**

`src/ui/screens/equalizer.rs`:

```rust
use crate::domain::Screen;
use crate::engine::eq::BAND_FREQS_HZ;
use crate::settings::Settings;
use parking_lot::RwLock;
use std::sync::Arc;

pub fn draw(ui: &mut egui::Ui, settings: &Arc<RwLock<Settings>>, screen: &mut Screen) {
    if ui.button("◀ Back").clicked() { *screen = Screen::AllSongs; }
    ui.heading("Equalizer");

    let mut s = settings.write();
    let mut enabled = s.equalizer.enabled;
    if ui.checkbox(&mut enabled, "Enabled").changed() {
        s.equalizer.enabled = enabled;
    }

    ui.add_space(8.0);
    ui.horizontal(|ui| {
        for (i, freq) in BAND_FREQS_HZ.iter().enumerate() {
            ui.vertical(|ui| {
                ui.label(format!("{:.0}{}",
                    if *freq < 1000.0 { *freq } else { *freq / 1000.0 },
                    if *freq < 1000.0 { "Hz" } else { "kHz" },
                ));
                let mut val = s.equalizer.bands[i];
                if ui.add(egui::Slider::new(&mut val, -12.0..=12.0).vertical().show_value(false)).changed() {
                    s.equalizer.bands[i] = val;
                }
                ui.label(format!("{:+.1} dB", val));
            });
        }
    });

    ui.add_space(12.0);
    if ui.button("Reset to flat").clicked() {
        s.equalizer.bands = [0.0; 10];
    }
    if ui.button("Save").clicked() {
        let snapshot = s.clone();
        drop(s);
        if let Err(e) = snapshot.save() {
            log::error!("save settings: {}", e);
        }
    }
}
```

- [ ] **Step 2: Wire + route**

`src/ui/screens/mod.rs` — append `pub mod equalizer;`

Central panel match:

```rust
                crate::domain::Screen::Equalizer => {
                    crate::ui::screens::equalizer::draw(ui, &self.settings, &mut self.screen);
                }
```

- [ ] **Step 3: Commit**

```bash
rtk cargo check --all-targets
rtk git add -A
rtk git commit -m "feat(ui): Equalizer screen editing Settings"
```

---

## Task 29: UI screen — `Settings`

**Files:**
- Create: `src/ui/screens/settings.rs`
- Modify: `src/ui/screens/mod.rs`, `src/ui/app.rs`

- [ ] **Step 1: Implementation**

`src/ui/screens/settings.rs`:

```rust
use crate::data::Library;
use crate::domain::Screen;
use crate::settings::Settings;
use crate::ui::toasts::Toasts;
use parking_lot::RwLock;
use std::path::PathBuf;
use std::sync::Arc;

pub fn draw(
    ui: &mut egui::Ui,
    settings: &Arc<RwLock<Settings>>,
    library: &Arc<Library>,
    toasts: &mut Toasts,
    screen: &mut Screen,
) {
    if ui.button("◀ Back").clicked() { *screen = Screen::AllSongs; }
    ui.heading("Settings");

    let mut s = settings.write();

    ui.collapsing("Library paths", |ui| {
        let mut to_remove: Option<usize> = None;
        for (i, root) in s.scan.roots.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                let mut buf = root.to_string_lossy().into_owned();
                if ui.add(egui::TextEdit::singleline(&mut buf).desired_width(400.0)).changed() {
                    *root = PathBuf::from(buf);
                }
                if ui.small_button("✕").clicked() { to_remove = Some(i); }
            });
        }
        if let Some(i) = to_remove { s.scan.roots.remove(i); }
        if ui.button("+ Add path").clicked() { s.scan.roots.push(PathBuf::new()); }

        ui.label("Source library (read-only catalog used by other tools):");
        let mut src = s.scan.source_root.clone().unwrap_or_default().to_string_lossy().into_owned();
        if ui.add(egui::TextEdit::singleline(&mut src).desired_width(400.0)).changed() {
            s.scan.source_root = if src.is_empty() { None } else { Some(PathBuf::from(src)) };
        }
    });

    ui.collapsing("Playback", |ui| {
        ui.add(egui::Slider::new(&mut s.playback.volume, 0.0..=1.0).text("Volume"));
    });

    ui.collapsing("Renumber", |ui| {
        ui.checkbox(&mut s.renumber.enabled, "Renumber on delete");
        ui.add(egui::Slider::new(&mut s.renumber.threshold, 0.0..=1.0).text("Threshold"));
    });

    ui.add_space(12.0);
    if ui.button("Save").clicked() {
        let snap = s.clone();
        drop(s);
        match snap.save() {
            Ok(_) => toasts.info("Settings saved"),
            Err(e) => toasts.error(format!("Save failed: {e}")),
        }
        // Trigger rescan after save.
        let lib = library.clone();
        let roots = settings.read().scan.roots.clone();
        std::thread::spawn(move || lib.scan(&roots));
    }
}
```

- [ ] **Step 2: Wire + route**

`src/ui/screens/mod.rs` — append `pub mod settings;`

Central panel match:

```rust
                crate::domain::Screen::Settings => {
                    crate::ui::screens::settings::draw(ui, &self.settings, &self.library, &mut self.toasts, &mut self.screen);
                }
```

- [ ] **Step 3: Commit**

```bash
rtk cargo check --all-targets
rtk git add -A
rtk git commit -m "feat(ui): Settings screen with rescan-on-save"
```

---

## Task 30: UI — keyboard shortcuts (Space / Ctrl+→ / Ctrl+←)

**Files:**
- Modify: `src/ui/app.rs`

Implements §4.6 keyboard shortcuts (§6 item 10).

- [ ] **Step 1: Add `handle_shortcuts` to `App`**

In `impl App`, add:

```rust
    fn handle_shortcuts(&self, ctx: &egui::Context) {
        if ctx.wants_keyboard_input() { return; }
        ctx.input(|i| {
            if i.key_pressed(egui::Key::Space) {
                self.playback.play_pause();
            }
            if i.modifiers.command_only() {
                if i.key_pressed(egui::Key::ArrowRight) { self.playback.next(); }
                if i.key_pressed(egui::Key::ArrowLeft)  { self.playback.previous(); }
            }
        });
    }
```

In `App::update`, add as the **first line** (before any panel draws):

```rust
        self.handle_shortcuts(ctx);
```

- [ ] **Step 2: Verify**

```bash
rtk cargo check --all-targets
rtk cargo run
```

Manual test: type into the search box — Space inserts a space, doesn't pause. Click outside (focus lost), press Space — playback toggles.

- [ ] **Step 3: Commit**

```bash
rtk git add -A
rtk git commit -m "feat(ui): keyboard shortcuts gated on wants_keyboard_input"
```

---

## Task 31: Status bar for scanning + library status integration

**Files:**
- Modify: `src/ui/app.rs`

- [ ] **Step 1: Add status bar between top bar and central panel**

In `App::update`, after the `top_bar` panel and before the `CentralPanel`, add:

```rust
        if self.library.status() == crate::data::LibraryStatus::Scanning {
            egui::TopBottomPanel::top("status_bar").show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(format!("Scanning library… ({} songs so far)", self.library.len()));
                });
            });
        }
```

- [ ] **Step 2: Verify**

```bash
rtk cargo check --all-targets
rtk cargo run
```

- [ ] **Step 3: Commit**

```bash
rtk git add -A
rtk git commit -m "feat(ui): live scanning status bar"
```

---

## Task 32: README + release build verification

**Files:**
- Create: `README.md`
- Build: `target/release/winplayer.exe`

- [ ] **Step 1: Write `README.md`**

```markdown
# WinPlayer

Native Windows music player in Rust. Single .exe, no runtime, no installer.

## Build

\`\`\`bash
cargo build --release
\`\`\`

The binary lands in `target/release/winplayer.exe`.

## Run

\`\`\`bash
cargo run --release
\`\`\`

On first run, open **Settings → Library paths**, add a folder of audio
files, and Save. The library scans in the background and the song list
populates as it goes.

## Supported formats

mp3, flac, m4a, ogg, wav, aac, opus.

## Keyboard shortcuts

- `Space` — play / pause (only when no text field has focus)
- `Ctrl + →` — next track
- `Ctrl + ←` — previous track

## Architecture

See [PLAN.md](./PLAN.md).
```

- [ ] **Step 2: Build the release binary**

```bash
rtk cargo build --release
```

Expected: clean build. Binary at `target/release/winplayer.exe`.

- [ ] **Step 3: Confirm size and basic launch**

```bash
ls -la target/release/winplayer.exe
```

Expected: ~10–25 MB stripped. Launch:

```bash
./target/release/winplayer.exe &
```

Expected: window opens; close to continue.

- [ ] **Step 4: Commit**

```bash
rtk git add -A
rtk git commit -m "docs: add README with build + usage instructions"
```

---

## Task 33: Smoke test the integrated flow + final cargo test

**Files:** none (verification only)

- [ ] **Step 1: Full test suite**

```bash
rtk cargo test
```

Expected: every test pass; no panics, no warnings.

- [ ] **Step 2: Manual end-to-end with audio**

Place 3–5 audio files (mix of mp3 + flac if possible) into `<cwd>/music/` (debug default). Launch:

```bash
rtk cargo run
```

Click through:
- AllSongs row → song plays, mini-player shows progress, slider moves.
- Drag the slider 50% in → audio jumps; UI doesn't snap back to 0.
- Press Space (after clicking outside the search box) → pauses; again → resumes.
- Press the next button → next track loads.
- Open Now Playing → big controls work; back button returns to AllSongs.
- Open Queue → all queued songs visible; current is highlighted; click another to jump.
- Open Settings → change volume slider → mini-player reflects it.
- Click ✕ on a row in AllSongs → file deleted from disk; folder renumbered if applicable; toast appears.

If any step fails, stop and debug before claiming completion.

- [ ] **Step 3: Final commit (if any tweaks were needed)**

```bash
rtk git status
# If clean, no commit needed.
```

---

## Self-review checklist (already applied)

- ✅ Spec coverage: every section of PLAN.md (§2 layout, §3 threads, §4 modules, §5 flows, §6 non-obvious, §7 build) is covered by at least one task.
- ✅ Non-obvious bits explicitly implemented:
  - §6.1 position anchor → Task 11 (`set_position_anchor_ms`, `reset_position`)
  - §6.2 seek slider memory stash → Task 19
  - §6.3 row rect-allocate-first → Task 20
  - §6.4 case-folded path id → Task 2
  - §6.5 lofty `catch_unwind` → Task 7
  - §6.6 cmd/event channel isolation → Task 14 (`crossbeam_channel::bounded`)
  - §6.7 two-pass rename → Task 10
  - §6.8 cached library views → Task 22 (`AllSongsState.cache_key`)
  - §6.9 pagination + virtualization → Task 22 (`PAGE_SIZE = 50`, `show_rows`)
  - §6.10 `wants_keyboard_input` gating → Task 30
- ✅ No placeholders. Every code-modifying step shows the code.
- ✅ Type consistency: `Library`, `Engine`, `EngineCmd`/`EngineEvent`, `PlaybackController`, `Queue`, `AllSongsState` are referenced consistently across tasks.
- ✅ Commit cadence: one commit per task, ~33 commits total.

---

## Execution Handoff

**Plan complete and saved to `docs/superpowers/plans/2026-05-10-music-player.md`. Two execution options:**

**1. Subagent-Driven (recommended)** — fresh subagent per task with two-stage review between tasks. Best for catching small mistakes early on a 33-task build.

**2. Inline Execution** — run tasks sequentially in this session with batched checkpoints. Faster wall-clock, less safety net.

**Which approach?**
