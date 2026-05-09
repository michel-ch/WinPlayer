# Architecture

A layered, thread-cooperative design. The UI never touches the audio device.
The engine never touches the file list. The playback controller is the bridge.

For the full canonical spec see [../PLAN.md](../PLAN.md). This document is a
shorter operational view.

## Layer dependencies

```
   UI  ───►  Playback  ───►  Engine  ───►  Audio device
    │           │
    └──────────►  Library (data)
```

- **UI** reads `PlaybackState` snapshots and `Library` snapshots; calls into
  `PlaybackController` for every action.
- **Playback** owns a `Queue` and an `Engine` handle; bridges UI actions to
  engine commands and engine events back to UI-visible state.
- **Engine** owns the cpal output stream and the current decoder thread.
- **Library** is shared between UI (for browsing) and Playback (for queue
  lookups and deletes).

## Threads

Five threads cooperate. No tokio, no async runtime.

| Thread | Owner | Job |
|---|---|---|
| **UI / main** | `eframe` | egui frame loop. Reads `PlaybackState` via `RwLock` snapshots. |
| **library-scan** | spawned in `main` | `WalkDir` + tag read. One per scan root. |
| **engine** | `Engine::start` | Owns the `AudioOutput` (with cpal `Stream`). Receives `EngineCmd`, emits `EngineEvent`. |
| **decoder** | `start_decode` (per track) | symphonia decode → optional rubato resample → push to ring buffer. |
| **playback-events** | `PlaybackController::new` | Drains `EngineEvent`, updates `PlaybackState`, triggers `next()` on `EndOfTrack`. |
| **cpal callback** | cpal | Pops from ring buffer in real time. Lock-free path: only atomics. |

## Synchronization primitives

| Primitive | Used for |
|---|---|
| `parking_lot::RwLock<PlaybackState>` | UI reads, events thread writes |
| `parking_lot::RwLock<Vec<Song>>` | scan thread writes once, UI reads |
| `parking_lot::RwLock<Queue>` | controller mutates, UI peeks |
| `crossbeam_channel::bounded` | engine commands and events |
| `Arc<AtomicU64>` | `samples_played`, `position_offset_ms`, `skip_samples` |
| `Arc<AtomicBool>` | decoder `stop_flag`, `finished`; engine `paused` |
| `ringbuf::HeapRb<f32>` | SPSC between decoder and audio callback |

## Crossing layer boundaries

- **UI → Playback:** direct method calls on `Arc<PlaybackController>`.
- **Playback → Engine:** `crossbeam_channel::Sender<EngineCmd>` (`Load`, `Play`,
  `Pause`, `Stop`, `SeekFraction`, `SetVolume`, `Shutdown`).
- **Engine → Playback:** `crossbeam_channel::Receiver<EngineEvent>` (`Started`,
  `Position`, `Paused`, `Resumed`, `EndOfTrack`, `LoadFailed`).
- **Engine → cpal callback:** atomic loads only — no locks on the hot path.
  This is what keeps the audio callback real-time-safe.

## End-to-end flows

### Startup

1. `main` loads `Settings` (TOML at `%APPDATA%\Recurate\Recurate\settings.toml`).
2. Spawns the **library-scan** thread; status transitions `Idle → Scanning → Ready`.
3. Starts the `Engine`, which spawns the **engine** thread, which opens the
   default cpal output stream.
4. Builds the `PlaybackController`, which spawns the **playback-events** thread.
5. Boots `eframe` and runs the egui frame loop.

### Play a song

1. UI: row clicked → `playback.play_songs(visible_songs, idx, None)`.
2. Controller writes the queue, sets `state.current_song`, sends
   `EngineCmd::Load { path, autoplay: true }`.
3. **Engine thread** runs `prepare_decode(path)` (file open + symphonia probe +
   build decoder + build resampler) — slow work that happens *while the previous
   track is still audible*.
4. Engine stops the prior decoder, calls `controls.clear()`, then
   `spawn_decode(prepared, controls)` — fast handoff.
5. Engine sends `EngineEvent::Started { duration_ms }`.
6. **Playback-events thread** updates `state.duration_ms` and zeros position.
7. **Decoder thread** is now feeding the ring buffer.
8. **cpal callback** is now popping samples.
9. Engine emits `Position` events ~50× per second; UI scrubber and time label
   update on next frame.

### Seek

1. UI: drag released → `draw_seek_slider` returns `Some(target_fraction)`.
2. `playback.seek_fraction(f)` sends `EngineCmd::SeekFraction(f)`.
3. Engine: `target_ms = duration_ms * f`. Calls `job.seek(target_ms)` (writes the
   decoder's seek mailbox), `controls.drain_buffer()` (sets `skip_samples`),
   `controls.set_position_anchor_ms(target_ms)`, `controls.play()` if not paused.
4. Decoder thread sees the seek mailbox, calls `format.seek(...)`,
   `decoder.reset()`, `controls.reset_position()`. Old samples discarded by
   the audio callback in the same callback iteration. New samples flow.

### Track ends

1. Decoder hits EOF → sets `finished = true`.
2. Engine notices `job.is_finished() && controls.buffered_samples() == 0` →
   emits `EndOfTrack`, drops the current job.
3. Playback-events thread receives `EndOfTrack` → calls `controller.next()`.
4. Queue advances per repeat mode; the play flow above repeats from step 2.

### Delete a song

1. UI: per-row ✕ → `deletion::delete_song(&library, id, true, threshold)`.
2. `remove_file(path)` → `library.remove_song(id)`.
3. `renumberer::renumber_folder(folder, threshold)` runs the two-pass rename
   if the folder qualifies (>= threshold of files already track-prefixed).
4. `library.refresh_folder(folder)` re-reads the folder.
5. `library.version()` bumps; UI cache invalidates; AllSongs re-renders next
   frame with new names.

## Library version counter

Every mutation (`scan`, `refresh_folder`, `remove_song`, `replace_all`) bumps
an `AtomicU64`. UI caches that depend on `Vec<Song>` (e.g. the sorted+filtered
AllSongs view) key off `(version, sort, query)`. They invalidate exactly once
per mutation rather than once per frame. This is what keeps a 2,500-song UI
running at 60 fps without re-sorting on every frame.

## Why no async

The engine is fundamentally a soft-real-time system: cpal callbacks need
predictable latency, and the decoder needs to keep the ring buffer fed faster
than it drains. Async runtimes add scheduling indirection that doesn't help
here. Five OS threads with channels are simpler, more predictable, and easier
to reason about for the audio path.
