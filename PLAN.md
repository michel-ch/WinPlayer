# Music Player — Architecture & Functionality Plan

A single-binary native desktop music player built in Rust. This document
describes **only the music-player core**: library scanning, audio playback,
queue management, the playback UI, and persistent settings. The YouTube
re-curation pipeline, the duplicate-detection screen, and the bootstrap
copy-from-source screen are **out of scope** here — they are
library-management features that share the same binary but are not part of
the player itself.

---

## 1. Goal

Play a flat collection of local audio files (mp3 / flac / m4a / ogg / wav /
aac / opus) with the basics a daily-driver player needs:

- Scan one or more root folders, read tags, present a sortable / searchable
  list.
- Play / pause / next / previous, seek, volume, shuffle, three repeat modes.
- Per-track delete (with automatic track-number renumber of the affected
  folder).
- 10-band parametric EQ in the playback chain.
- Always-on footer mini-player; full-screen "Now Playing" view.
- Keyboard shortcuts that don't interfere with text input.
- Dataset target: ~thousands of files in flat folders; not tens of
  thousands; no nesting required.

---

## 2. Top-level layout

```
player/
├── Cargo.toml
└── src/
    ├── main.rs              # eframe entry; spawns scan threads, starts engine
    ├── lib.rs               # Re-exports the modules below
    ├── settings.rs          # TOML-persisted user settings
    ├── renumberer.rs        # Track-number normalizer (used by deletion)
    ├── domain/              # Song, PlaybackState, Screen, SortOption, RepeatMode
    ├── data/                # Library + scanner + tag reader
    ├── engine/              # Audio engine: decoder + output + EQ
    ├── playback/            # PlaybackController + Queue + deletion
    └── ui/                  # egui App, screens, components, toasts, fonts
```

Each layer depends downward only:

```
   UI  ─────►  Playback  ─────►  Engine  ─────►  Audio device
    │             │
    └────────────►  Library (data)
```

The UI never touches the audio device. The engine never touches the file
list. The playback controller is the bridge: it owns a handle to the engine
(commands + events) and a reference to the library (for queue lookups and
deletes).

---

## 3. Threading model

Five distinct threads cooperate via channels and atomics. No tokio, no
async runtime.

| Thread | Owner | Job |
|---|---|---|
| **UI / main** | `eframe` | egui frame loop. Reads playback state via `RwLock` snapshots. |
| **library-scan** | spawned in `main` | `WalkDir` over each scan root, reads tags, writes the resulting `Vec<Song>` into `Library`. One per root (one for the destination library, one for the read-only source catalog used elsewhere). |
| **engine** | `Engine::start` | Owns the `AudioOutput` and the current `DecodeJob`. Receives `EngineCmd` over a crossbeam channel; emits `EngineEvent` back. |
| **decoder** | `start_decode` | Pulls packets via `symphonia`, resamples via `rubato` if needed, pushes interleaved f32 samples into the ring buffer. Polls a stop flag and a seek-request mailbox. |
| **playback-events** | `PlaybackController::new` | Drains `EngineEvent`s and updates the shared `PlaybackState`. Triggers `next()` on `EndOfTrack`. |
| **cpal callback** | `cpal` | Pops samples out of the ring buffer in real time. Lock-free path: only loads atomics, no `Mutex`. |

Synchronization primitives:

- `parking_lot::RwLock<PlaybackState>` — UI reads, event thread writes.
- `parking_lot::RwLock<Vec<Song>>` — scan thread writes once, UI reads.
- `crossbeam_channel` — engine commands and events.
- `Arc<AtomicU64>` — `samples_played`, `position_offset_ms` (read by UI,
  written by audio callback / engine).
- `ringbuf::HeapRb<f32>` — single-producer / single-consumer between
  decoder and audio callback.

---

## 4. Modules

### 4.1 `domain/` — vocabulary types

- **`Song`** — `id: i64`, `title`, `artist`, `album`, `album_artist`,
  `duration: Duration`, optional `year` / `genre` / `composer` /
  `track_no`, `path: PathBuf`, `has_embedded_art: bool`.
- **`PlaybackState`** — `current_song: Option<Song>`, `is_playing`,
  `current_position_ms`, `duration_ms`, `shuffle_enabled`,
  `repeat_mode: RepeatMode`. Exposes `progress() -> f32` for the
  scrubber.
- **`RepeatMode`** — `Off | All | One`. Cycle order is
  `Off → All → One → Off`.
- **`SortOption`** — 13 variants covering title / artist / album /
  duration / track # / filename (asc + desc) plus `Shuffle`.
  `sort_songs(&mut [Song], opt)` is the central sorter; `Shuffle` uses a
  hash of `(song.id, system-time-nanos)` as a deterministic-per-frame
  scramble key.
- **`Screen`** — enum tag for the active screen. `shows_bottom_nav()`
  hides the top bar on full-screen views (Now Playing, Settings).

`Song.id` is a `DefaultHasher` of the **case-folded, slash-normalized**
path. On Windows that means `C:\Music\foo.mp3`,
`c:\music\foo.mp3`, and `C:/Music/foo.mp3` all hash to one id. On Unix the
path is used byte-exact. This is asserted by unit tests in
`data/scanner.rs` and `data/mod.rs`.

### 4.2 `data/` — Library

- **`Library`** — owns `RwLock<Vec<Song>>` + `RwLock<LibraryStatus>` +
  `AtomicU64` version counter. Status is `Idle → Scanning → Ready`.
  Every mutation (`scan`, `refresh_folder`, `remove_song`) bumps the
  version counter so UI caches can invalidate.
- **`Library::scan(roots)`** — walks each root, reads tags, dedupes by
  `normalize_path_key`. Logs missing roots and continues; failure of
  one root does not abort the rest.
- **`Library::refresh_folder(folder)`** — incremental rescan of a single
  folder. Drops every song whose folder matches (case-folded) and
  re-adds whatever the disk currently contains. Used after a delete
  causes a renumber.
- **`scanner.rs`** — `walkdir::WalkDir` filtered by extension allowlist
  (`mp3 flac m4a ogg wav aac opus`). One file → one `tags::read_song`
  call.
- **`tags.rs`** — wraps `lofty::Probe::open(path)?.read()` in
  `catch_unwind` because lofty's ID3v1 parser slices the fixed 30-byte
  title field as UTF-8 without checking codepoint boundaries — any
  CJK / Cyrillic title truncated mid-codepoint panics the decoder thread
  if not caught. Also derives sensible fallbacks: filename stem if title
  is empty; "Unknown Artist" if missing; parent folder name if album is
  missing; `parse_track_prefix(path)` if the tag has no track number.

### 4.3 `engine/` — audio engine

Three submodules, all owned by the dedicated `engine` thread.

#### `output.rs` — `AudioOutput`

Wraps a `cpal::Stream` with a `ringbuf::HeapRb<f32>`. Two-second buffer at
the device's native `(sample_rate, channels)`. Three sample-format paths
(F32, I16, U16) — the F32 path is the fast one; I16 / U16 fall back to a
per-callback intermediate `Vec<f32>` and convert at the end.

Position reporting is **anchored**:

```
played_duration() = position_offset_ms + samples_played / sample_rate
```

- `samples_played: AtomicU64` is bumped by the audio callback on every
  pop.
- `position_offset_ms: AtomicU64` is the seek anchor. Default 0.
- After a seek to T, the engine calls `set_position_anchor_ms(T)`, which
  stores T into the offset and zeros `samples_played`. The UI then
  immediately reports T even though the ring buffer is briefly empty
  while the decoder refills. Without the anchor, the UI scrubber would
  visually snap back to 0 the moment the user released the slider.
- `clear()` zeros both — used for stop / new-track load only.
- `reset_position()` zeros only the local sample counter — used by the
  decoder thread post-seek.

`drain_buffer()` is a soft drain: stores the current occupied length
into `skip_samples`, and the audio callback discards (zeros) that many
samples on its next iterations. This is how a seek actually invalidates
the in-flight buffered data without dropping the cpal stream.

#### `decoder.rs` — `DecodeJob`

Spawned per loaded track. Owns the symphonia `format` reader, the
codec decoder, and an optional `rubato::FftFixedInOut` resampler (built
only when source sample rate ≠ device sample rate; bypassed otherwise to
save CPU).

Decoder thread loop:

1. Check `stop_flag`; bail if set.
2. Check `seek_request`; if present, `format.seek(...)`, `decoder.reset()`,
   `output.lock().reset_position()`, clear the planar accumulator.
3. `format.next_packet()` → `decoder.decode(packet)`.
4. Copy interleaved samples into the planar buffer (per channel).
5. If resampling: drain enough frames from each channel,
   `rubato.process(...)`, re-interleave to the device's channel count,
   push to output.
6. If not resampling: channel-map (mono → stereo as duplicate, etc.),
   push to output.

Two utility helpers:

- `push_or_block` — `output.push_samples()` returns 0 if the ring buffer
  is full; sleep 5ms and retry. Re-checks `stop_flag` between retries.
- `push_with_channel_map` — handles src/dst channel mismatch (mono
  duplicated across stereo, surround down to stereo as truncate).

#### `eq.rs` — `Equalizer`

10 biquad bands at 31 / 62 / 125 / 250 / 500 / 1k / 2k / 4k / 8k / 16k Hz,
all `Type::PeakingEQ` with Butterworth Q. `process_inplace` runs each
sample through all 10 bands sequentially. Disabled by default; gated by
the `enabled` flag so the cost is one branch per sample when off.

#### `mod.rs` — `Engine` facade

Exposes `EngineCmd` (`Load { path, autoplay } | Play | Pause | Stop |
SeekFraction(f32) | SetVolume(f32) | Shutdown`) and `EngineEvent`
(`LoadStarted | LoadFailed | Started { duration } | Position { current_ms,
duration_ms } | Paused | Resumed | EndOfTrack`).

The engine loop dispatches commands and, while a track is playing,
periodically (`recv_timeout(20ms)`) emits `Position` events with
`output.lock().played_duration()`. When the decoder reports `is_finished`
AND the ring buffer drains to zero, the engine emits `EndOfTrack` and
clears the current job.

`SeekFraction` is the only command with non-trivial coordination: it
issues `job.seek(target)`, calls `out.drain_buffer()` to skip in-flight
audio, then `out.set_position_anchor_ms(target_ms)` so the UI reports
the new position immediately, then `out.play()` if the player wasn't
paused before the seek.

### 4.4 `playback/` — controller, queue, deletion

#### `controller.rs` — `PlaybackController`

The bridge layer. Owns `Engine` + `Arc<Library>` + `RwLock<Queue>` +
`RwLock<PlaybackState>`. The UI calls into this; this calls into the
engine.

Public API:

- `play_songs(songs, start_index, sort)` — replace the queue with this
  list (optionally pre-sorted) and start playing from `start_index`.
- `play_pause()` — toggles. Sends `Play` or `Pause` to the engine and
  updates `is_playing` synchronously.
- `next()` / `previous()` — advances the queue (respecting repeat) and
  loads the new song. `previous()` has the standard "rewind to start if
  >3s in" UX.
- `jump_to(index)` — direct queue index; used by the Queue screen.
- `remove_from_queue(id)` — drops the song from the queue and, if it
  was the current song, transparently jumps to whatever takes its slot
  (or stops if the queue is now empty).
- `seek_fraction(f32)` — forwards to engine.
- `set_volume`, `set_shuffle`, `set_repeat`, `cycle_repeat`.

The constructor spawns the **playback-events** thread that drains
`engine.events()` and updates `PlaybackState` so UI snapshots see fresh
data without polling. `EndOfTrack` triggers `next()` here, which is what
makes the queue auto-advance.

#### `queue.rs` — `Queue`

`Vec<Song>` + `current: Option<usize>` + `shuffle: bool` + `repeat:
RepeatMode`.

- `next_index()` — `RepeatOne → cur`, `RepeatAll → (cur+1) % len`,
  `RepeatOff → cur+1` or `None` at end.
- `prev_index()` — at index 0, `RepeatAll` wraps to last, otherwise
  stays at 0.
- `remove_song_id(id)` — drops every matching song while keeping
  `current` pointing at the right slot (decrements by the count of
  removed-before-current; clamps to `len-1` if the current itself was
  dropped).

Note: `shuffle` is stored on `Queue` for state but the actual shuffle
ordering is currently produced upstream by `SortOption::Shuffle` when
the queue is built. The flag exists for future per-queue reshuffle.

#### `deletion.rs` — `delete_song`

1. Look up the song in the library (errors out if it's already gone).
2. `std::fs::remove_file(path)`.
3. `library.remove_song(id)`.
4. If `renumber` is true, run `renumberer::renumber_folder(folder,
   threshold)`. On non-zero result, `library.refresh_folder(folder)` to
   pick up the renamed paths.
5. Returns a `DeletionResult { deleted_path, renumbered }` so the UI
   can toast a useful message.

### 4.5 `renumberer.rs` — track-number normalizer

Re-derives `01 - <title>.mp3`, `02 - <title>.mp3`, … filenames from the
existing files in a folder when at least `threshold` (default 0.5) of
the audio files already carry a `<digits> - ` prefix. Below threshold
the folder is considered "not numbered" and stays untouched.

Algorithm (pure rename, no audio touched):

1. Sort files by current full filename (so prefixed files lead and
   unprefixed trail alphabetically — matches the user's file-manager
   view).
2. Compute the new index for each file.
3. Rename every file to a temporary name first
   (`.tmp_renumber_<nanos>_<i>_<stem>.<ext>`) to avoid collisions with
   targets.
4. Rename each temp to its final name.

The two-pass rename is required because `01.mp3 → 02.mp3`,
`02.mp3 → 03.mp3` would otherwise overwrite `02.mp3` before reading it.

### 4.6 `ui/` — egui application

The UI is an `eframe::App` (`ui::App`) that holds `Arc`s to `Library`,
`PlaybackController`, and `Settings`, plus a tag indicating the current
`Screen`.

#### Per-frame structure

```
top_bar (nav)
─────────────
status_bar (only when an async op is active: scanning, fingerprinting)
─────────────
<active screen>
─────────────
mini_player (footer; always visible)
toasts (top-right overlay)
```

#### Screens (player-only subset)

| Screen | What it shows |
|---|---|
| **AllSongs** | Paginated (50 rows / page) virtualized list of every song. Sort picker (13 options). Search bar. Row click → `play_songs`. Per-row ▶ button (redundancy) and ✕ remove button (with renumber). |
| **AlbumsList / AlbumDetail** | Faceted view by `Song.album`. Detail = the songs in that album. |
| **ArtistsList / ArtistDetail** | Same shape, faceted by `Song.artist`. |
| **Folders** | The most useful facet for the target dataset (folders are the user's organization unit). One card per parent directory. |
| **Queue** | Live queue with reorder via remove + jump-to. Current row highlighted. |
| **NowPlaying** | Full-screen view: cover art (if `has_embedded_art`), title + artist + album, scrubber (same `draw_seek_slider` widget as the footer), prev / play-pause / next, shuffle and repeat toggles. |
| **Equalizer** | 10-band slider grid backed by `engine::eq::Equalizer`. Enabled/disabled toggle. |
| **Settings** | Library paths (destination + source), volume, renumber threshold; auto-rescan on path edit when leaving the screen. |

Out of scope here (handled by the same binary but not part of the
player): **Replacer**, **Missing**, **Duplicates**.

#### Components

- **`top_bar`** — top navigation row plus `"{count} songs"` on the
  right.
- **`mini_player`** — bottom footer: title/artist (truncated), prev /
  play-pause / next, **seek slider**, time, volume, "Now Playing"
  link.
- **`song_row::draw_with_options`** — the reusable interactive row.
  Allocates a fixed `[available_width, row_h]` rect via
  `allocate_exact_size` *before* rendering, then renders into a
  `child_ui` placed at that rect. This is what stops the per-frame
  width-growth that produced a "staircase" of right-aligned content
  drifting off-screen, and it makes the entire row click-to-play.
  Inside the rect: 28px row number, ▶ button, title, then a
  `right_to_left` cluster with optional ✕, duration, separator,
  truncated artist.
- **`toasts`** — top-right overlay with auto-dismiss TTL (4s info, 6s
  warn, 9s error) plus manual ✕. Used to surface user-facing failures
  that aren't visible in the diff (silent delete failures, etc.).

#### Caches

UI caches that depend on the library are keyed on
`Library::version()` so they invalidate exactly once per library
mutation:

- `cached_library_view: Option<LibraryView>` keyed on
  `(library_version, sort, query)` — the sorted+filtered+searched
  song list for AllSongs.
- `cached_folders: Option<(version, Arc<Vec<PathBuf>>)>` — cached
  folder list for the Folders screen.

Without these caches, every frame re-clones `Vec<Song>`, re-sorts it,
and re-filters it — which on a 2,500-row library was the dominant
source of UI lag.

#### Keyboard shortcuts

Three global shortcuts handled in `App::handle_shortcuts`, **gated on
`ctx.wants_keyboard_input()`** so typing into the search bar or any
text field never triggers them:

- `Space` — play / pause
- `Ctrl + →` — next track
- `Ctrl + ←` — previous track

The arrow keys are intentionally not bound bare — egui ScrollAreas use
them.

#### Seek slider — the awkward bit

`draw_seek_slider(ui, state) -> Option<f32>` is the shared scrubber
widget used by both the footer and the Now Playing screen. It
**stashes** the in-progress drag value into `egui::Memory` keyed by a
fixed `Id`, then restores it on the next frame's input. Without the
stash, every frame would reset `frac = state.progress()` *before* the
slider's `drag_stopped` event fired, so the committed value would be
the OLD position and seeking would no-op back to where you already were.

The widget commits — i.e. returns `Some(target)` — on
`drag_stopped() || lost_focus()` (the normal release case) OR on
`changed() && !dragged()` (a bare click on the track that egui doesn't
classify as a sustained drag). The `Memory` entry is cleared on commit.

This is the UI half of the seek fix. The engine half is the position
anchor in `output.rs`. They were both required: fixing only one left
the slider visibly snapping back to the start on release.

### 4.7 `settings.rs` — persistence

TOML at `%APPDATA%/Recurate/Recurate/settings.toml` (resolved via
`directories::ProjectDirs`). Sections:

```toml
[scan]
roots = ["C:/Users/you/Music"]
source_root = "C:/Users/you/Music_original"   # used by other features

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

Default behavior depends on the build:
- Debug builds default to `<cwd>/music` and `<cwd>/music_original` for
  the convenience of `cargo run` from `player/`.
- Release builds default to **empty paths** so a fresh install shows
  blank fields and the user picks their own.

Settings are loaded once at startup (`load_or_default`); a corrupt file
falls back to defaults with a warning rather than crashing the app.

---

## 5. End-to-end data flow

### 5.1 Startup

1. `main` loads `Settings`.
2. Spawns a thread that calls `library.scan(roots)`. The library status
   transitions `Idle → Scanning → Ready`; song count increments live.
3. Starts the `Engine` (which spawns the engine thread, which owns the
   `AudioOutput` and so opens the cpal stream).
4. Builds the `PlaybackController`, which spawns the
   playback-events thread.
5. Boots `eframe`, which calls `App::new` (which installs the Unicode
   font fallback chain) and then enters the egui frame loop.

### 5.2 Play a song from AllSongs

1. UI: user clicks a row.
2. `App` calls `playback.play_songs(visible_songs, clicked_index, None)`.
3. `PlaybackController` writes the queue, calls `start_current()` →
   `load_and_play(song)`.
4. Sets `state.current_song = Some(song)`, `state.is_playing = true`,
   sends `EngineCmd::Load { path, autoplay: true }`.
5. **Engine thread**: stops any prior decoder, calls `out.clear()` and
   `out.drain_buffer()`, calls `decoder::start_decode(...)` to spawn the
   decoder thread.
6. Engine emits `EngineEvent::Started { duration }`.
7. **Playback-events thread** receives `Started`, writes
   `state.duration_ms`, zeros `state.current_position_ms`, sets
   `is_playing = true`.
8. **Decoder thread** is now feeding the ring buffer.
9. **cpal callback** is now popping samples and incrementing
   `samples_played`.
10. **Engine thread** is also emitting `Position` events ~50× per second.
11. UI reads `state` each frame and renders the scrubber + time label.

### 5.3 Seek

1. UI: user drags the slider, releases. `draw_seek_slider` returns
   `Some(target_fraction)`.
2. `PlaybackController::seek_fraction` sends
   `EngineCmd::SeekFraction(target_fraction)`.
3. **Engine thread**: computes `target = duration * target_fraction`,
   calls `job.seek(target)` (which writes to the decoder's seek mailbox),
   `out.drain_buffer()`, `out.set_position_anchor_ms(target_ms)`,
   `out.play()` if not paused.
4. **Decoder thread** notices the seek mailbox, calls
   `format.seek(...)`, `decoder.reset()`, `out.reset_position()` (zeros
   only `samples_played`, preserves the new anchor).
5. UI position reads as `anchor_ms + 0` immediately, then climbs as the
   buffer refills.

### 5.4 Track ends

1. **Decoder thread** hits EOF, sets `finished = true`.
2. **Engine thread** notices `job.is_finished() && buffered_samples ==
   0`, emits `EndOfTrack`, drops the current job.
3. **Playback-events thread** receives `EndOfTrack`, calls
   `controller.next()`.
4. Queue advances per its repeat mode; `load_and_play` is called for
   the new song; everything from §5.2 step 4 onward repeats.

### 5.5 Delete a song

1. UI: user clicks the per-row ✕.
2. App calls `deletion::delete_song(&library, id, renumber=true,
   threshold)`.
3. `remove_file(path)` → `library.remove_song(id)`.
4. `renumberer::renumber_folder(folder, threshold)` runs the two-pass
   rename if the folder qualifies.
5. `library.refresh_folder(folder)` re-reads the folder so the new
   names are in the song list. Library version bumps; UI cache
   invalidates; AllSongs re-renders with the new names on the next
   frame.

---

## 6. Non-obvious design decisions

These are choices that look unusual until you know why.

1. **Position anchor on seek.** Standard "clear the buffer + zero the
   counter on seek" makes the UI scrubber snap back to 0 for the few
   frames before the buffer refills. Anchoring `played_duration` to
   the seek target eliminates the snap.

2. **Seek slider value in egui memory.** A naive slider re-reads
   `state.progress()` every frame, which clobbers the dragged value
   before the `drag_stopped` event fires. Stashing the in-flight value
   across frames is the only way to know what the user actually
   released on.

3. **`song_row` allocates the row rect first, renders second.**
   Otherwise `with_layout(right_to_left)` lays out into the
   already-consumed inner width, the right cluster ends up at width
   zero, and consecutive rows visibly drift further off-screen.

4. **Case-folded path → song id (Windows).** A scan-root edit from
   `C:\Music` to `c:\music` and back would otherwise insert every song
   twice with two different ids, breaking deduplication.

5. **`catch_unwind` around `lofty::Probe::open`.** lofty's ID3v1
   parser slices the fixed 30-byte title field as UTF-8 without
   `is_char_boundary` checks. CJK / Cyrillic titles truncated to 30
   bytes mid-codepoint panic the decoder thread; without the catch a
   single such file kills tag-reading for the whole batch.

6. **Engine command/event isolation.** All cross-thread communication
   with the audio engine flows through two `crossbeam_channel`s. The
   audio callback never takes a `Mutex` on the hot path — only loads
   atomics — which is what keeps it real-time-safe.

7. **Two-pass rename in `renumberer::apply`.** A single-pass
   `01 → 02, 02 → 03` would overwrite `02.mp3` before ever reading it.
   First-pass to temp names, second-pass to final names, breaks the
   chain.

8. **Cached library views keyed on `Library::version()`.** Without
   the cache, AllSongs re-clones, re-sorts, and re-filters the full
   `Vec<Song>` 60 times a second. With the cache, that work runs once
   per library mutation.

9. **Pagination at 50 rows + virtualization via `show_rows`.** Even
   with caching, rendering 2,500 row widgets per frame is hundreds of
   milliseconds. Pagination drops it to 50 widget allocations; row
   virtualization drops the on-screen subset to whatever fits in the
   viewport.

10. **Keyboard shortcuts gated on `ctx.wants_keyboard_input()`.** A
    bare `Space` binding pauses playback every time the user types a
    space into the search bar. Gating on focus state is the only
    correct fix; debouncing it would feel unresponsive.

---

## 7. Build & verify

```bash
cd player
cargo check --all-targets    # type-check everything
cargo test                   # unit + integration tests (no network, no audio)
cargo run --release          # launch the GUI
```

Default scan root in debug builds is `<cwd>/music`. Release builds
ship with empty paths; the user picks roots in
**Settings → Library paths**.

External runtime requirements for the player itself: **none**. cpal
opens the system default output device directly. (The Replacer screen
needs `yt-dlp` and `ffmpeg` on `PATH`, but that screen is out of scope
for this document.)
