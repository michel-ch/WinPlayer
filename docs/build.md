# Build

## Prerequisites

- **Rust 1.94 or later** — `rustup default stable` if you have older.
- **MSVC build tools** on Windows (Visual Studio Build Tools 2019+ or full VS).
  Cargo on Windows uses the MSVC toolchain by default.
- **No external runtime requirements at runtime.** cpal opens the system
  default output device directly.

Verify your toolchain:

```bash
rustc --version
cargo --version
```

## Quick build

```bash
cargo build --release
```

The binary lands in `target/release/winplayer.exe` (~12 MB, statically linked).

## Run from source

```bash
cargo run --release
```

In **debug** mode (`cargo run`), the default scan root is `<cwd>/music` —
useful for `cargo run` from the project root with a `music/` folder of test
files. In **release** mode the default scan paths are empty; the user picks
their library in **Settings**.

## Profile flags

`Cargo.toml` ships with these release profile flags for size + speed:

```toml
[profile.release]
lto = "fat"           # whole-program link-time optimization (~10% perf, slower link)
codegen-units = 1     # single codegen unit for max optimization (slower compile)
strip = true          # strip debug symbols (much smaller binary)
panic = "abort"       # no unwinding tables (smaller, faster panic)
opt-level = 3         # max optimization

[profile.dev]
opt-level = 1         # baseline dev speed; symphonia is slow at opt-level 0
```

Trade-offs:
- `lto = "fat"` adds ~30 s to release link time but produces meaningfully faster code.
- `panic = "abort"` means panics terminate immediately. `data/tags.rs` wraps
  lofty reads in `catch_unwind`, but aborting release builds do not unwind, so
  that wrapper only catches panics in profiles that unwind. In release, a panic
  from tag parsing is process-fatal.
- `opt-level = 1` for dev: `cargo run` of debug mode at `opt-level = 0` makes
  symphonia decode painfully slow; level 1 is a fine middle ground.

## Cross-platform notes

- **Windows-first.** The `windows_subsystem = "windows"` attribute in `main.rs`
  hides the console on release builds. Debug builds keep the console for log output.
- **Linux** — should mostly work (cpal supports ALSA/PulseAudio, eframe supports
  X11/Wayland, lofty/symphonia are pure Rust). Untested. The font fallback in
  `ui/fonts.rs` references `C:\Windows\Fonts\…` which is silently skipped on
  non-Windows; egui's bundled fonts cover Latin scripts.
- **macOS** — should work (CoreAudio via cpal, AppKit via eframe). Untested.

## Binary size

The release binary is ~11.8 MB stripped. The largest contributors:

| Crate | ~Size |
|---|---|
| `wgpu` + `naga` (egui backend) | ~3 MB |
| `symphonia` (scoped codec set) | ~2 MB |
| `lofty` | ~600 KB |
| `rubato` (FFT resampling) | ~400 KB |
| Everything else | ~5 MB |

`symphonia` is already scoped in `Cargo.toml` to the codecs the scanner needs
(`mp3, flac, aac, alac, isomp4, ogg, vorbis, wav, pcm`) via
`default-features = false` rather than the catch-all `features = ["all"]`,
which trims ~1 MB of unused decoders (and that much less untrusted-input parser
surface). The unused `rand` dependency was also dropped.

To shrink further:
- Replace the `wgpu` backend with `glow` in eframe features. Smaller, but
  may miss compositor effects.
- Drop more `symphonia` codecs if your library is a single format.

## Build matrix

| Build | Command | Time | Size |
|---|---|---|---|
| Dev (debug) | `cargo build` | ~5 s incremental, ~60 s clean | ~150 MB unstripped |
| Release | `cargo build --release` | ~20 s incremental, ~4 min clean (LTO) | ~12 MB |
| Tests | `cargo test` | ~5 s incremental | n/a |

## Common build issues

**"linker `link.exe` not found"**: install Visual Studio Build Tools (Desktop
C++ workload).

**"failed to run custom build command for `wgpu-hal`"**: Windows SDK missing —
install via VS Installer.

**"error: linker `lld-link` not found"**: only an issue if you've configured a
non-MSVC linker. Default MSVC works out of the box.

**Release build fails with "Access denied" on `winplayer.exe`**: the binary is
running. Close the app and rebuild.

## Verifying a build

After a clean release build:

```bash
cargo test                       # unit + integration tests pass
ls target/release/winplayer.exe  # binary exists, ~12 MB
./target/release/winplayer.exe   # window opens, mini-player visible
```

The window should open in <1 second.
