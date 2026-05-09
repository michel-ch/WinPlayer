# Audio pipeline

The hot path from disk to speaker, with the design decisions that make track
switching feel instant and seeking feel snappy.

```
file → symphonia → planar f32 → [optional: rubato resample] → interleaved f32
     → ring buffer → cpal callback → device
```

## Components

### `AudioOutput` (`src/engine/output.rs`)

Wraps a `cpal::Stream` plus a `ringbuf::HeapRb<f32>` (single-producer /
single-consumer) sized at 2 seconds of buffer at the device's native sample
rate and channel count. Three sample-format paths (F32 / I16 / U16); the F32
path is the fast one, I16 / U16 fall back to a per-callback intermediate
`Vec<f32>` and convert at the end.

`OutputControls` is the `Send + Clone` handle the engine and decoder threads
share. It exposes `push_samples`, `played_duration_ms`, `set_position_anchor_ms`,
`reset_position`, `drain_buffer`, `clear`, `play`, `pause`, `set_volume`,
`buffered_samples`. The `cpal::Stream` itself stays on the engine thread —
on Windows WASAPI requires same-thread stream ownership.

### `DecodeJob` (`src/engine/decoder.rs`)

Spawned per loaded track. Owns the symphonia `FormatReader`, `Decoder`, and
optional pre-built `rubato::FftFixedInOut<f32>` resampler. The decoder thread:

1. Check `stop_flag`. Bail if set.
2. Check `seek_request_ms`. If pending, `format.seek(...)`, `decoder.reset()`,
   `controls.reset_position()`, clear the planar accumulator.
3. `format.next_packet()` → `decoder.decode(packet)`.
4. Copy samples into per-channel planar buffers (with i16 / i32 → f32 conversion
   if needed).
5. If resampling: drain `chunk_in` frames per channel, run rubato, interleave
   to device channel count, push to ring buffer.
6. If not resampling: interleave with channel map (mono → stereo dup, surround
   → stereo truncate), push to ring buffer.

`push_or_block`: when the ring buffer is full, sleep 5 ms and retry. Re-checks
`stop_flag` between retries so an in-flight push doesn't pin the thread on
track change.

### `Equalizer` (`src/engine/eq.rs`)

10 biquad bands at 31 / 62 / 125 / 250 / 500 / 1k / 2k / 4k / 8k / 16k Hz, all
`Type::PeakingEQ` with Butterworth Q. One filter chain per channel. Currently
**not wired into the audio pipeline** — the screen edits the parameters and
persists them, but the filter chain is not yet applied to samples on the
decoder thread. This is the only part of the original spec not yet hot.

### `Engine` facade (`src/engine/mod.rs`)

The engine thread receives `EngineCmd` and emits `EngineEvent`:

```
EngineCmd: Load { path, autoplay } | Play | Pause | Stop
         | SeekFraction(f32) | SetVolume(f32) | Shutdown

EngineEvent: LoadStarted | LoadFailed | Started { duration_ms }
           | Position { current_ms, duration_ms } | Paused | Resumed
           | EndOfTrack
```

`recv_timeout(20 ms)` gives the loop a periodic tick to emit `Position` events
without burning CPU when idle.

## Critical design decisions

### Position anchor

`played_duration_ms` is anchored:

```
played_duration_ms = position_offset_ms + (samples_played / channels / sample_rate * 1000)
```

- `samples_played` is bumped by the audio callback on every pop.
- `position_offset_ms` is the seek anchor. Default 0.
- After a seek to T, the engine calls `set_position_anchor_ms(T)`, which
  stores T into the offset and zeros `samples_played`. The UI then immediately
  reports T even though the ring buffer is briefly empty while the decoder
  refills.

Without the anchor, the UI scrubber visibly snaps back to 0 the moment the
user releases the slider, before snapping forward again as the new audio
catches up. This was item §6.1 of the spec.

### Skip-drain (instant track switch)

When a Load command fires, the engine calls `controls.clear()`, which sets
`skip_samples = ring_buffer.occupied_len()`. The audio callback's job is to
discard those samples before popping new ones.

The naive implementation drains only one callback's worth per invocation
(~480 samples at 10 ms callback period). With a 2-second / 88,200-sample
buffer, that takes ~1.84 seconds of audible silence between tracks.

The fix (in `fill_callback`):

```rust
let skip = skip_samples.load(Ordering::Acquire);
if skip > 0 {
    let mut tmp = [0.0f32; 1024];
    let mut total: u64 = 0;
    while total < skip {
        let want = ((skip - total) as usize).min(tmp.len());
        let popped = consumer.pop_slice(&mut tmp[..want]) as u64;
        if popped == 0 { break; }
        total += popped;
    }
    skip_samples.fetch_sub(total, Ordering::AcqRel);
}
// then proceed with normal fill from new samples
```

The drain runs in a tight pop loop *within a single callback*, so by the time
the callback returns the ring buffer is empty and ready for the new decoder.
Track switch silence drops from ~1.84 s to ~10 ms (one callback period).

The 1024-sample stack array keeps the work allocation-free. Total time on
modern CPUs: a few microseconds per drain — well within the audio callback's
real-time budget.

### Prep-before-swap (low perceptual load latency)

The engine processes a `Load` command in two phases:

1. **`prepare_decode(path)`** — file open, symphonia probe, decoder construction,
   FFT-plan-based resampler init. This is the slow part (5–80 ms typical).
   It runs **while the previous track is still audible** — no silence yet.
2. **Stop old + clear + `spawn_decode(prepared, controls)`** — atomic flag flip,
   buffer drain, thread spawn. Total: < 5 ms.

The user-visible silence between tracks is just the cpal callback period
(~10 ms) plus the decoder's first-packet decode (~5–15 ms). End-to-end:
~15–25 ms.

Compared to the naive sequence of `stop → clear → prepare (silent) → spawn`,
this saves the entire prep window (5–80 ms) of audible silence.

### Seek slider memory stash

Implemented in `src/ui/components/seek_slider.rs`. The slider widget is shared
between the mini-player and Now Playing screen.

A naive slider re-reads `state.progress()` every frame to position itself —
which clobbers the dragged value before the `drag_stopped` event fires. The
result: dragging the scrubber appears to do nothing.

The fix: stash the in-progress drag value into `egui::Memory` keyed by a
fixed `Id`, then restore it on the next frame's read. The widget commits —
i.e. returns `Some(target)` — on `drag_stopped() || lost_focus()` (the normal
release case) OR on `changed() && !dragged()` (a bare click on the track that
egui doesn't classify as a sustained drag). The Memory entry is cleared on
commit.

This is the UI half of the seek fix. The engine half is the position anchor.
Both were required: fixing only one left the slider visibly snapping back to
the start on release.

### Lock-free callback

The cpal audio callback uses only:

- `consumer.pop_slice(...)` — single-producer/single-consumer ring buffer pop,
  lock-free by design.
- `AtomicU64::load`, `fetch_add`, `fetch_sub` — atomic loads/stores.
- `AtomicBool::load` — for the paused flag.
- `Mutex<f32>::lock()` — for the volume. *This is the one lock on the hot
  path.* It's contended only when the user drags the volume slider, and the
  critical section is one f32 read. In practice this is fine for soft real-time;
  if it ever causes audible glitching, swap to `AtomicU32::load` with bit-cast
  semantics.

No other locks. No `Arc::clone` in the callback. No allocation. The 1024-sample
skip-drain temporary is on the stack.

### Two-pass renumber

`renumber_folder` (in `src/renumberer.rs`) renames audio files in a folder to
`01 - <name>.<ext>`, `02 - <name>.<ext>`, etc.

A single-pass shift like `01.mp3 → 02.mp3, 02.mp3 → 03.mp3` overwrites
`02.mp3` before reading it. Two-pass:

1. Rename every file to a temp name (`.tmp_renumber_<nanos>_<i>_<stem>.<ext>`)
   that can't collide with any final name.
2. Rename each temp to its final `01 - …`, `02 - …` name.

The unique temp namespace (`<nanos>_<i>`) means concurrent renumberings of
different folders can never collide.

## Sample rate handling

The decoder thread builds an `FftFixedInOut<f32>` resampler **only when**
`source_sample_rate != device_sample_rate`. For the common case where the
device is configured to the source's native rate (typical for music — most
listeners run their device at 44.1 or 48 kHz, matching most albums), the
resampler is bypassed entirely. This saves CPU and avoids any resampling
artifacts.

When resampling is needed, `FftFixedInOut` precomputes an FFT plan at
construction time. This plan generation can take 10–50 ms, which is why we
build the resampler in `prepare_decode` (during the prep-before-swap window)
rather than on the decoder thread after the swap.

## Channel mapping

`interleave_with_channel_map(planar, dst_ch)` handles common cases:

- Mono source, stereo device: duplicate each sample to both channels.
- Stereo source, stereo device: passthrough.
- Surround source, stereo device: take the first 2 channels (truncate).
- Source channel count exceeds device: extra channels dropped.
- Device channel count exceeds source: last source channel duplicated to
  fill remaining device channels.

This is a reasonable default. A more sophisticated downmix (proper L/R/C/Ls/Rs
to L/R) would require channel layout awareness and is left as future work.
