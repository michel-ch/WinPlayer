# Architecture

A layered, thread-cooperative design. The UI never touches the audio device.
The engine never touches the file list. The playback controller is the bridge.

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
| **decoder** | `spawn_decode` (per track) | symphonia decode → optional rubato resample → push to ring buffer. Joined on stop. |
| **playback-events** | `PlaybackController::new` | Drains `EngineEvent`, updates `PlaybackState`, triggers `next()` on `EndOfTrack`. |
| **cpal callback** | cpal | Pops from ring buffer in real time. Lock-free path: only atomics. |

## Synchronization primitives

| Primitive | Used for |
|---|---|
| `parking_lot::RwLock<PlaybackState>` | UI reads, events thread writes |
| `parking_lot::RwLock<Vec<Song>>` | scan thread writes once, UI reads |
| `parking_lot::RwLock<Queue>` | controller mutates, UI peeks |
| `crossbeam_channel::bounded` | engine commands and events |
| `Arc<AtomicU64>` | `samples_played`, `position_offset_ms`, `skip_samples`, EQ settings version |
| `Arc<AtomicU32>` | `volume_bits` — volume as bit-cast f32, so the audio callback reads it lock-free |
| `Arc<AtomicBool>` | decoder `stop_flag`, `finished`; engine `paused` |
| `ringbuf::HeapRb<f32>` | SPSC between decoder and audio callback |

`buffered_samples()` derives the fill level from the ring buffer state rather
than maintaining a separate counter, so producer/callback ordering cannot make
end-of-track detection observe a stale fill count.

## Crossing layer boundaries

- **UI → Playback:** direct method calls on `Arc<PlaybackController>`.
- **Playback → Engine:** `crossbeam_channel::Sender<EngineCmd>` (`Load`, `Play`,
  `Pause`, `Stop`, `SeekFraction`, `SetVolume`, `SetEqualizer`, `Shutdown`).
- **Engine → Playback:** `crossbeam_channel::Receiver<EngineEvent>` (`LoadStarted`,
  `Started`, `Position`, `Paused`, `Resumed`, `EndOfTrack`, `LoadFailed`).
  `LoadStarted`, `Paused`, `Resumed`, and `Position` are sent with `try_send`
  because they are recoverable or superseded by later state; critical lifecycle
  events still use blocking `send`.
- **Engine → cpal callback:** atomic loads only — no locks on the hot path.
  This is what keeps the audio callback real-time-safe.

## End-to-end flows

### Startup

1. `main` loads `Settings` (TOML at `%APPDATA%\Recurate\Recurate\settings.toml`).
2. Spawns the **library-scan** thread; status transitions `Idle → Scanning → Ready`.
3. Starts the `Engine`, which spawns the **engine** thread, which opens the
   default cpal output stream.
4. Builds the `PlaybackController`, which spawns the **playback-events** thread.
5. Applies the saved playback volume from `Settings`.
6. Restores the previous session: `last_played::load()` reads the saved record
   from `%LOCALAPPDATA%`; if the path still exists, the track is loaded **paused**
   via `load_paused(song, position_ms)`. A non-zero position is stashed in the
   `pending_seek_ms` atomic and applied as a seek when the engine reports
   `Started` — so you reopen exactly where you left off. Shuffle and repeat are
   stored in settings but are not currently reapplied during startup.
7. Boots `eframe` and runs the egui frame loop.

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

1. UI: per-row ✕ calls `delete_song_with_playback(...)`. If the song is the
   currently playing track, playback is stopped best-effort before the file is
   removed so Windows can release the handle.
2. The deletion helper removes the file from disk, then drops it from the
   in-memory library with `library.remove_song(id)`.
3. `renumberer::renumber_folder(folder, threshold)` runs the two-pass rename
   if the folder qualifies (>= threshold of files already track-prefixed).
   On any rename error it rolls back every move it made.
4. If renumbering changed any files, `library.refresh_folder(folder)` re-reads
   the folder.
5. The queue is only mutated **after** a successful delete (for a non-playing
   track), so a failed delete never leaves the UI out of sync with the disk.
6. `library.version()` bumps; UI caches invalidate; AllSongs re-renders next
   frame with new names.

### Shutdown

1. The user closes the window; `eframe`'s loop returns and `App` is dropped.
2. `Drop for App` persists state best-effort: writes the live volume / shuffle /
   repeat back into `Settings` and saves, and writes `last_played` with the
   current playback position.
3. It then calls `playback.shutdown()`, which sends `EngineCmd::Shutdown`. The
   engine thread breaks its loop and drops the cpal `Stream`. When the engine
   drops its event sender, the **playback-events** thread sees the event channel
   disconnect and exits instead of leaking.

## Library version counter

Every mutation (`scan`, `refresh_folder`, `remove_song`, `replace_all`) bumps
an `AtomicU64` **while still holding the songs write lock**, so a reader can
never observe the new list under the old version number. UI caches that depend
on `Vec<Song>` — the sorted+filtered AllSongs view keyed on `(version, sort,
query)`, and the Albums / Artists / Folders aggregations keyed on `version` —
invalidate exactly once per mutation rather than once per frame. This is what
keeps a multi-thousand-song UI running at 60 fps without re-sorting or
rebuilding facet maps on every frame.

## Why no async

The engine is fundamentally a soft-real-time system: cpal callbacks need
predictable latency, and the decoder needs to keep the ring buffer fed faster
than it drains. Async runtimes add scheduling indirection that doesn't help
here. Five OS threads with channels are simpler, more predictable, and easier
to reason about for the audio path.
