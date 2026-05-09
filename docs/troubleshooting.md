# Troubleshooting

Common issues and how to diagnose them. Logs go to stderr by default;
in debug builds the console is visible. In release builds the console is
hidden, so to see logs run from a command prompt:

```bash
target\release\winplayer.exe
```

Or capture to a file:

```bash
target\release\winplayer.exe 2> winplayer.log
```

## "No audio plays"

**1. Check the cpal default device exists.**

Open Windows Sound Settings; ensure a default playback device is selected
and not muted. cpal binds to whatever the OS reports as the default.

**2. Check the volume slider isn't at 0.**

The mini-player has a small "vol" slider. The Settings → Playback section
also has a master volume.

**3. Check the engine started.**

In the log:

```
[INFO  winplayer] winplayer starting
```

If you see:

```
[ERROR winplayer::engine] audio output init failed: <reason>
```

the cpal stream couldn't open. Common reasons:

- **"no default output device"** — no playback device configured in Windows.
- **"unsupported sample format"** — your device reports U24 or some exotic
  format we don't have a fast path for. File a bug with the format reported.
- **"build stream: ..."** — driver issue. Try restarting Windows Audio
  service (`services.msc`).

**4. Check a track actually loaded.**

In the log:

```
[INFO  winplayer::engine] LoadStarted("path/to/song.mp3")
[INFO  winplayer::engine] Started { duration_ms: 234567 }
```

If you see `LoadFailed`, the file couldn't be decoded. Check the path exists
and the format is supported.

## "Songs don't appear in the list"

**1. Check the scan finished.**

Top of the screen should show "(N songs)" rather than "Scanning…". If
scanning is stuck, see below.

**2. Check the library path.**

Open **Settings**. The path you entered must be an absolute path to a
folder that exists on disk. Typos are silent — the scanner just reports
"scan root missing" and continues.

**3. Check the file format.**

Only `mp3`, `flac`, `m4a`, `ogg`, `wav`, `aac`, `opus` are scanned. Other
formats are silently skipped.

**4. Check file readability.**

Files in folders the user doesn't have read access to (e.g. a system folder
or a read-protected network share) are skipped. The scanner uses
`WalkDir + filter_map(|e| e.ok())` which silently drops `Err` entries.

## "Scanning hangs or never completes"

**1. Slow disk or network share.**

A scan over a 50,000-track library on a slow disk can take 5–10 minutes.
Watch the song count tick up — if it's still climbing, scanning is working.

**2. A pathological file panicked the tag reader.**

The scanner wraps `lofty::Probe::open` in `catch_unwind` to survive
malformed ID3 tags. If a panic does slip through (e.g. an unrelated codec
panic), the offending file logs:

```
[WARN  winplayer::data::tags] lofty panicked on <path>
```

A synthetic Song is built from filename and parent folder name, so the
file still appears in the library — it just has minimal metadata.

**3. The scan thread crashed.**

Check the log for a stack trace. If you can reproduce, file a bug with the
file path that triggered it.

## "Crash on a track with foreign characters in the title"

This was the motivation for the `catch_unwind` in `data/tags.rs`. lofty's
ID3v1 parser slices the fixed 30-byte title field as UTF-8 *without* checking
codepoint boundaries. Truncated CJK / Cyrillic titles can panic the decoder.
We catch this and fall back to filename + parent folder name.

If you see a crash log without the `catch_unwind` warning, the panic came
from somewhere else. File a bug with the log and the file path.

## "Track switching is slow / has audible silence"

This was a real bug fixed in commit `cab… perf(engine): drain skip-buffer in
one callback + prep next track before swap`. If you're on an older build,
update.

If you're on a recent build and still see slowness:

- **Log the load timeline.** Add `log::debug!` traces in `engine_thread`'s
  Load handler and re-run with `RUST_LOG=debug`.
- **Check for slow disk on prepare_decode.** Symphonia probe time scales
  with header size; flac files with embedded art can take 50–200 ms to probe
  on a slow disk.
- **Check for resampler init cost.** If your device sample rate doesn't
  match your music's, every track build a fresh `FftFixedInOut`. Plan
  generation is 10–50 ms. To avoid: in Windows Sound, set the default device
  rate to match your music (typically 44.1 or 48 kHz).

## "Seek snaps back to the start"

Should not happen. If it does, the bug is in either the position anchor
(`engine/output.rs::set_position_anchor_ms`) or the seek slider memory
stash (`ui/components/seek_slider.rs`). Both must be working for seek to
behave correctly. See [audio-pipeline.md](./audio-pipeline.md) for details.

## "Settings won't save"

**1. Check `%APPDATA%\Recurate\Recurate\` is writable.**

The settings file is at `%APPDATA%\Recurate\Recurate\settings.toml`. If the
directory is read-only or the disk is full, save will fail and toast an error.

**2. Corrupt existing file.**

If a previous version wrote a malformed file, the app loads defaults at
startup and logs:

```
[WARN  winplayer::settings] settings.toml parse failed (...); using defaults
```

To reset, delete `%APPDATA%\Recurate\Recurate\settings.toml`.

## "Delete didn't renumber the folder"

**1. Threshold not met.**

The renumber threshold (default 0.5) is the *fraction of audio files in the
folder that already have a `<digits>-` prefix*. If only 30 % of files are
prefixed, the folder is not renumbered.

Adjust the threshold in **Settings → Renumber**.

**2. Renumber disabled.**

Check the **Renumber on delete** checkbox in Settings.

**3. Permission denied during rename.**

If the folder is read-only or another process has the file open, the rename
fails. The deletion still succeeds (the file is removed), but the renumber
step errors and toasts.

## "EQ doesn't seem to do anything"

**Currently expected.** The EQ filter chain is built but not yet inserted
into the audio pipeline. The Equalizer screen edits parameters and saves them
to settings.toml, but the filters do not yet affect playback.

Wiring the EQ into the decoder thread is a small future task: insert
`equalizer.process_inplace(samples, channels)` before the `push_or_block`
call in `decoder_loop`. The EQ is already designed for this — `process_inplace`
is a no-op when `enabled` is false, so the cost is one branch per sample.

## "Window won't open / crashes immediately"

**1. wgpu / GPU driver issue.**

eframe defaults to wgpu (which uses Vulkan / DX12 / Metal). On Windows you
may need updated graphics drivers, especially on integrated GPUs.

**2. No display.**

If running over RDP or in a headless context, eframe needs a window system.
WinPlayer is desktop-only; there's no headless mode.

**3. Catastrophic startup error.**

Check the log:

```bash
target\release\winplayer.exe 2>&1
```

Most startup errors fall into "audio output init failed" or "spawn engine
failed", both surfaced as `log::error!`. Less commonly, an eframe / wgpu
panic from the rendering backend — those produce a Rust backtrace.

## Getting help

Open an issue with:

- OS version (Windows 10 / 11, build number).
- Rust version (`rustc --version`) if you're building from source.
- The exact command you ran.
- The log output (with `RUST_LOG=debug` if you can reproduce).
- The audio file format / track that triggered the issue, if applicable.
