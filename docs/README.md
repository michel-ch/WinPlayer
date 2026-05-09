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
| **See the original spec** — full architecture document | [../PLAN.md](../PLAN.md) |

## Layout of this repo

```
WinPlayer/
├── Cargo.toml             package + profile flags
├── PLAN.md                canonical architecture spec
├── README.md              short top-level readme
├── docs/                  this directory
│   ├── README.md
│   ├── architecture.md
│   ├── audio-pipeline.md
│   ├── build.md
│   ├── contributing.md
│   ├── troubleshooting.md
│   ├── usage.md
│   └── superpowers/plans/ implementation plan + execution log
└── src/
    ├── main.rs            eframe entry; spawns scan, engine, controller; runs UI
    ├── lib.rs             re-exports
    ├── settings.rs        TOML persistence
    ├── renumberer.rs      two-pass folder rename
    ├── domain/            Song, PlaybackState, RepeatMode, SortOption, Screen
    ├── data/              Library + scanner + tag reader
    ├── engine/            cpal output + symphonia decoder + biquad EQ + facade
    ├── playback/          PlaybackController + Queue + deletion pipeline
    └── ui/                eframe::App + screens + components + toasts
```

## Quick reference

- **Supported formats:** mp3, flac, m4a, ogg, wav, aac, opus
- **Rust version:** 1.94 or later
- **Build target:** `x86_64-pc-windows-msvc`
- **Binary size:** ~12 MB stripped release
- **Memory footprint:** ~50–100 MB with a few thousand tracks loaded
- **Audio backend:** cpal default host (WASAPI on Windows)
- **UI backend:** eframe + egui + wgpu

## Project status

The player core is complete: library scanning, playback (play/pause/next/prev/seek/volume),
queue with shuffle and three repeat modes, per-track delete with folder renumber,
EQ settings UI, all eight screens, keyboard shortcuts. The 10-band biquad EQ
filter chain is built but not yet inserted into the audio pipeline (the screen
edits its parameters; runtime patching is future work).
