# WinPlayer

Native Windows music player in Rust. Single .exe, no runtime, no installer.

## Build

```bash
cargo build --release
```

The binary lands in `target/release/winplayer.exe`.

## Run

```bash
cargo run --release
```

On first run, open **Settings**, add a folder of audio files to the Library
paths, click Save. The library scans in the background and the song list
populates as it goes.

## Supported formats

mp3, flac, m4a, ogg, wav, aac, opus.

## Keyboard shortcuts

- `Space` — play / pause (only when no text field has focus)
- `Ctrl + \u{2192}` — next track
- `Ctrl + \u{2190}` — previous track

## Architecture

See [PLAN.md](./PLAN.md). Implementation plan and execution log live in
`docs/superpowers/plans/`.
