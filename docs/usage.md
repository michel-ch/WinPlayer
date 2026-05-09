# Usage

## First run

1. Launch `winplayer.exe`.
2. Open **Settings** (top-right tab).
3. Click **+ Add path** under "Library paths".
4. Type or paste the absolute path to a folder of audio files
   (e.g. `C:\Users\you\Music`).
5. Click **Save**.

The library scans in the background. You'll see a "Scanning library… (N songs
so far)" status bar at the top while it runs. Once scanning completes the
status bar disappears and the song count is shown in the top bar.

## Screens

The top bar provides navigation between the seven main screens. Some screens
hide the chrome for a focused view (Now Playing, Settings, Equalizer).

### Songs (AllSongs)

The default screen — every track in the library, paginated 50 per page.

- **Sort** dropdown: 13 options across title, artist, album, duration, track #,
  filename, plus shuffle.
- **Search** field: filters by title, artist, or album substring (case-insensitive).
- **Click any row** to start playing that track. The visible page becomes the
  queue; you can advance through it with next/prev.
- **▶ button** on a row: same as clicking the row.
- **✕ button** on a row: deletes the file from disk. The folder is then
  re-numbered if it qualifies (see [Renumber](#renumber-on-delete) below).
- **◀ Prev / Next ▶** buttons under the search bar: page navigation.

### Albums

`AlbumsList` shows every distinct album with its track count. Click an album
to open `AlbumDetail`, which shows just the tracks in that album, sorted by
track number.

### Artists

Same shape as Albums but faceted by artist. `ArtistDetail` sorts by album.

### Folders

The most useful facet for the typical dataset — one entry per parent folder
of the audio files. Click a folder to drill in.

### Queue

The current playback queue. The currently-playing row is highlighted. Click
any row to jump there. Click ✕ on a row to remove it from the queue (and
auto-jump if it was the current one).

### Now Playing

Full-screen view of the current track: title, artist, album, large scrubber,
oversize transport buttons, shuffle and repeat toggles. Click **◀ Back**
(top-left) to return to the previous screen.

### Equalizer

Ten band sliders at 31 / 62 / 125 / 250 / 500 / 1k / 2k / 4k / 8k / 16k Hz.
Each band has a range of ±12 dB. Toggle **Enabled** to switch the EQ on/off.
**Reset to flat** zeros all bands. **Save** writes to `settings.toml`.

Note: at the time of writing the EQ chain is built but not yet inserted into
the audio pipeline — the screen edits parameters and persists them, but the
filters do not yet affect playback.

### Settings

- **Library paths** — list of folders the scanner walks. `+ Add path` adds an
  empty row; type a path; ✕ removes a row.
- **Playback** — master volume slider.
- **Renumber** — toggle and threshold for the auto-renumber-on-delete feature.

**Save** persists to `%APPDATA%\Recurate\Recurate\settings.toml` and triggers
a re-scan in the background.

## Mini-player (always visible)

The bottom panel is the mini-player. It stays on every screen except
Now Playing.

| Element | Action |
|---|---|
| Title / artist (left) | Display only — truncated if narrow |
| ⏮ | Previous track (or rewind to start if >3 s in) |
| ⏸ / ▶ | Play / pause toggle |
| ⏭ | Next track |
| Scrubber | Drag to seek |
| Time labels | Current / total |
| Volume slider | Master volume |
| 🔀 / 🔀ON | Shuffle toggle |
| 🔁 / 🔁ALL / 🔂 | Repeat: Off → All → One → Off |
| ⤢ | Open Now Playing screen |

## Keyboard shortcuts

Three global shortcuts. They are gated on `ctx.wants_keyboard_input()`, so
typing into the search bar or any text field never triggers them.

| Key | Action |
|---|---|
| `Space` | Play / pause |
| `Ctrl + →` | Next track |
| `Ctrl + ←` | Previous track |

Bare arrow keys are intentionally not bound — egui ScrollAreas use them.

## Repeat modes

| Mode | Behavior at end of track | Behavior at end of queue |
|---|---|---|
| **Off** | Advance to next | Stop |
| **All** | Advance to next | Wrap to first |
| **One** | Replay the same track | Replay the same track |

## Shuffle

Toggling shuffle ON sets a flag on the queue. Note: the **shuffle order** for
a freshly-built queue is produced upstream by `SortOption::Shuffle` when you
select "Shuffle" from the Sort dropdown in AllSongs. The queue's shuffle flag
exists for future per-queue reshuffling.

## Renumber on delete

When you delete a track via the ✕ button, the deletion pipeline:

1. Removes the file from disk.
2. Drops the song from the in-memory library.
3. If the **renumber threshold** is met (default: 0.5 — at least 50 % of
   audio files in that folder already have a `<digits>-` prefix), the folder
   is re-numbered: `01 - …`, `02 - …`, in alphabetical order.

This keeps numeric prefixes consistent after deletions. To disable, uncheck
**Renumber on delete** in Settings.

The rename is two-pass (temp names → final names) so files can't collide
during the shift.

## Settings storage

Path: `%APPDATA%\Recurate\Recurate\settings.toml`

Sections:

```toml
[scan]
roots = ["C:/Users/you/Music"]

[playback]
volume = 0.7
shuffle = false
repeat = "off"
crossfade_ms = 0

[equalizer]
enabled = false
bands = [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]
bass_boost = 0.0

[renumber]
enabled = true
threshold = 0.5
```

A corrupt file is replaced with defaults at startup with a warning logged —
the app does not crash.

## Library paths

A path can point to any folder; the scanner walks recursively through every
subfolder. Multiple paths are supported (each gets its own scan thread).
Missing paths are logged and skipped — they do not abort the scan.

Supported extensions: `mp3`, `flac`, `m4a`, `ogg`, `wav`, `aac`, `opus`.
Other files are silently skipped.
