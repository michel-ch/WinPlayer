# Contributing

A short guide to extending the codebase. Read [architecture.md](./architecture.md)
first for the threading model and layer dependencies.

## Module map

| File | Responsibility |
|---|---|
| `src/main.rs` | eframe entry. Loads settings, spawns scan thread, starts engine, builds controller, runs UI. |
| `src/lib.rs` | Re-exports the seven top-level modules. |
| `src/settings.rs` | TOML persistence at `%APPDATA%\Recurate\Recurate\settings.toml`. |
| `src/renumberer.rs` | Two-pass folder rename. |
| `src/domain/song.rs` | `Song` struct + path-id hashing (case-folded on Windows). |
| `src/domain/playback_state.rs` | `PlaybackState` + `RepeatMode`. |
| `src/domain/sort.rs` | `SortOption` (13 variants) + `sort_songs`. |
| `src/domain/screen.rs` | `Screen` enum. |
| `src/data/library.rs` | `Library` (RwLock-protected `Vec<Song>` + version counter). |
| `src/data/scanner.rs` | `WalkDir` scan + extension allowlist. |
| `src/data/tags.rs` | Lofty wrapper with `catch_unwind` and tag fallbacks. |
| `src/engine/output.rs` | `AudioOutput` + `OutputControls` (cpal stream + ring buffer). |
| `src/engine/decoder.rs` | `prepare_decode` + `spawn_decode` (symphonia + rubato). |
| `src/engine/eq.rs` | 10-band biquad equalizer. |
| `src/engine/mod.rs` | `Engine` facade with command/event channels and engine thread. |
| `src/playback/queue.rs` | `Queue` with repeat-aware indexing. |
| `src/playback/deletion.rs` | `delete_song` pipeline. |
| `src/playback/controller.rs` | `PlaybackController` bridge with playback-events thread. |
| `src/ui/app.rs` | `eframe::App` impl + screen routing + keyboard shortcuts. |
| `src/ui/fonts.rs` | Unicode font fallback chain. |
| `src/ui/toasts.rs` | Top-right notifications. |
| `src/ui/components/seek_slider.rs` | Shared scrubber widget with memory-stash trick. |
| `src/ui/components/song_row.rs` | Reusable interactive row with rect-allocate-first. |
| `src/ui/components/top_bar.rs` | Top navigation. |
| `src/ui/components/mini_player.rs` | Bottom mini-player. |
| `src/ui/screens/all_songs.rs` | Paginated, sortable, searchable AllSongs. |
| `src/ui/screens/albums.rs` | AlbumsList + AlbumDetail. |
| `src/ui/screens/artists.rs` | ArtistsList + ArtistDetail. |
| `src/ui/screens/folders.rs` | Folders facet. |
| `src/ui/screens/queue.rs` | Live queue view with jump + remove. |
| `src/ui/screens/now_playing.rs` | Full-screen player view. |
| `src/ui/screens/equalizer.rs` | 10-band EQ editor. |
| `src/ui/screens/settings.rs` | Library paths + playback + renumber settings. |

## How to add a new screen

1. Add a variant to the `Screen` enum in `src/domain/screen.rs`. Decide whether
   `shows_chrome()` should return `false` (full-screen view).
2. Create `src/ui/screens/<name>.rs` with a `pub fn draw(...)` function that
   takes whatever state it needs (`&Arc<Library>`, `&Arc<PlaybackController>`,
   `&Arc<RwLock<Settings>>`, `&mut Toasts`, `&mut Screen`).
3. Re-export the module from `src/ui/screens/mod.rs`.
4. Add a `match` arm in `App::update`'s central panel `match self.screen.clone()`.
5. Add a `selectable_label` button in `src/ui/components/top_bar.rs`.
6. If your screen needs persistent UI state (e.g. selection, scroll position,
   filter), add a field to `App` and initialize it in `App::new`.

## How to add a new sort option

1. Add a variant to `SortOption` in `src/domain/sort.rs`.
2. Append it to `SortOption::ALL`.
3. Add a label in `SortOption::label`.
4. Add a sort arm in `sort_songs`. For deterministic sorts, prefer
   `sort_by_key` with a comparable key. For randomized sorts, hash a per-song
   id with a frame-stable nonce (see the `Shuffle` arm).
5. Add a unit test in the `tests` module verifying the sort order.

## How to add a new engine command

1. Add a variant to `EngineCmd` in `src/engine/mod.rs`.
2. Add a handler arm in `engine_thread`'s `match cmd`.
3. If the command produces an event, add a variant to `EngineEvent` and emit
   it. Add a corresponding handler in
   `PlaybackController::spawn_events_thread`.
4. Expose a wrapper method on `PlaybackController` that calls `self.engine.send(...)`.
5. Wire UI buttons to the new method.

## Test conventions

- **Unit tests live next to the code they test** in a `#[cfg(test)] mod tests`
  block at the bottom of the file.
- **Filesystem tests use `tempfile::tempdir()`** so they don't pollute the
  working directory or each other. See `renumberer.rs` and `playback/deletion.rs`
  for examples.
- **Audio tests are not unit-tested.** The cpal output and decoder threads
  require a real audio device. Smoke-test these manually via `cargo run`.
- **Scope:** prefer focused tests that pin one behavior. Don't bundle four
  assertions in one `#[test]` if they could each fail for different reasons.
- **Run all tests:** `cargo test`. Tests should complete in under 1 second.

## Style

- **Imports:** group `std`, then external crates, then `crate::`. No nested
  imports — keep them flat for grep-ability.
- **Comments:** explain *why*, not *what*. Don't add a comment that just
  restates the next line of code.
- **Errors:** at boundaries (file I/O, channel send, parse), use `Result<T, String>`
  with a context-rich message. Internal infallible code uses panics rarely
  and documents preconditions.
- **Naming:** follow the rest of the file. Modules are snake_case, types are
  PascalCase, functions are snake_case.
- **No emoji in source.** Unicode glyphs for UI icons are written as
  `\u{XXXX}` escapes, not literal characters, so the file stays editor-safe.

## Commit messages

Format: `<scope>(<area>): <short summary>`

```
feat(ui): add Now Playing screen with cover art
fix(engine): drain skip-buffer in one callback
perf(playback): cache library snapshot per version
docs: clarify rate-limit edge case
chore: bump symphonia to 0.5.4
```

The scope tells reviewers where to look; keep the summary under 70 chars.

## Threading rules

- **Never call into the engine from the audio callback.** The callback is
  bound by cpal's real-time guarantees; allocation, locking, or channel sends
  can cause buffer underruns.
- **Never block in the UI thread for more than a frame budget (~16 ms).** Long
  work goes on a background thread. Use `Arc<RwLock<T>>` snapshots for the UI
  to read.
- **Never lock during a `crossbeam_channel` send.** The receiver might also
  hold a lock that's needed by the sender's other code path.
- **Never spawn an unjoinable thread that owns shared state.** Use atomics or
  channels for ownership transfer.

## Adding tests for new threading code

For multi-threaded modules:

- Test the **logic** in single-threaded mode by extracting state machines into
  pure functions. `Queue::next_index` is a good example — pure, testable,
  no threads.
- Test the **wiring** with an integration test that uses real channels but
  fake threads. Drive the engine cmd channel and assert on the event channel.
  (Not yet present in this codebase; would live in `tests/`.)
- Don't test cpal callbacks in unit tests. They require a real device.

## Performance notes

- **Library snapshot** (`Library::songs_snapshot`) is `clone()` of `Vec<Song>`.
  For a 2,500-track library this is ~1 ms. UI screens cache the result keyed
  on `library.version()` to avoid cloning every frame.
- **AllSongs pagination + virtualization** keeps per-frame render under 1 ms
  even for tens of thousands of tracks. Don't lift it — it's load-bearing.
- **Engine recv_timeout** is 20 ms. Position events fire ~50× per second.
  This is enough resolution for the scrubber and time label without
  burning channel-write cycles.

## When in doubt

Run `cargo check --all-targets`. Run `cargo test`. Both should be clean before
you push.

```bash
cargo check --all-targets
cargo test
cargo clippy --all-targets       # optional but recommended
cargo build --release            # final verification
```
