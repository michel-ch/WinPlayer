# WinPlayer Documentation

Native Windows music player in Rust. Single .exe, no runtime, no installer.

## Where to start

| If you want to... | Read |
|---|---|
| **Install and run** the app | [build.md](./build.md) |
| **Use** the app — screens, shortcuts, settings | [usage.md](./usage.md) |
| **Understand the design** — modules, threads, data flow | [architecture.md](./architecture.md) |
| **Modify the audio engine** — cpal, ring buffer, seek, EQ | [audio-pipeline.md](./audio-pipeline.md) |
| **Contribute code** — add a screen, a sort option, a test | [contributing.md](./contributing.md) |
| **Fix a problem** — diagnostics for common issues | [troubleshooting.md](./troubleshooting.md) |

## Layout of this repo

```
WinPlayer/
├── Cargo.toml             package + profile flags
├── LICENSE                proprietary "All Rights Reserved" license
├── README.md              this documentation index
├── docs/                  architecture / usage / build / contributing / troubleshooting
└── src/
    ├── main.rs            eframe entry; spawns scan, engine, controller; runs UI
    ├── lib.rs             re-exports
    ├── settings.rs        TOML persistence (atomic write, clamp-on-load)
    ├── last_played.rs     crash-safe last-played record for startup resume
    ├── renumberer.rs      two-pass folder rename with rollback
    ├── domain/            Song, PlaybackState, RepeatMode, SortOption, Screen
    ├── data/              Library + scanner + tag reader
    ├── engine/            cpal output + symphonia decoder + biquad EQ + facade
    ├── playback/          PlaybackController + Queue + deletion pipeline
    └── ui/                eframe::App + theme + screens + components + toasts
```

## Quick reference

- **Supported formats:** mp3, flac, m4a, ogg, wav, aac
- **Rust version:** 1.94 or later
- **Build target:** `x86_64-pc-windows-msvc`
- **Binary size:** ~11.8 MB stripped release
- **Memory footprint:** ~50–100 MB with a few thousand tracks loaded
- **Audio backend:** cpal default host (WASAPI on Windows)
- **UI backend:** eframe + egui + wgpu
- **Look & feel:** editorial light theme — cream paper, sepia ink, a single
  terracotta accent, serif headings (see `src/ui/theme.rs`)

## Project status

The player core is complete: library scanning, playback (play/pause/next/prev/seek/volume),
queue with shuffle and three repeat modes, per-track delete with folder renumber,
EQ settings UI, all screens, keyboard shortcuts. Sessions resume where you left
off — the last-played track reopens (paused) at its saved position, and the saved
volume is applied on startup. Shuffle and repeat are persisted in settings, but
are not currently reapplied during startup. The audio engine has a 10-band
biquad EQ command path; the Equalizer screen currently edits and persists the
settings, but does not yet push changes into the running playback controller.

## License

WinPlayer (c) 2026 michel-ch. **All Rights Reserved.**

This software is proprietary. You may view and build it locally for personal,
non-commercial study. Copying, modifying, redistributing, or commercial use
requires prior written permission from the author. See [LICENSE](./LICENSE)
for the full terms.

Third-party Rust crates used at runtime (eframe, egui, cpal, symphonia,
rubato, lofty, etc.) are governed by their own licenses (predominantly
MIT / Apache-2.0) and are unaffected by this license.
