use crate::engine::eq::Equalizer;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, Stream, StreamConfig};
use parking_lot::Mutex;
use ringbuf::{traits::*, HeapCons, HeapProd, HeapRb};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;

/// Ring buffer length in milliseconds. Smaller = lower latency for volume
/// / EQ changes; larger = more decode headroom under load. 500 ms is plenty
/// for a music player that doesn't do realtime processing tricks.
const BUFFER_MILLIS: u32 = 500;

#[derive(Clone)]
pub struct OutputControls {
    pub sample_rate: u32,
    pub channels: u16,
    producer: Arc<Mutex<HeapProd<f32>>>,
    samples_played: Arc<AtomicU64>,
    position_offset_ms: Arc<AtomicU64>,
    skip_samples: Arc<AtomicU64>,
    volume_bits: Arc<AtomicU32>,
    eq_settings: Arc<EqSettings>,
    paused: Arc<AtomicBool>,
}

impl OutputControls {
    pub fn push_samples(&self, samples: &[f32]) -> usize {
        let mut p = self.producer.lock();
        p.push_slice(samples)
    }

    pub fn buffered_samples(&self) -> usize {
        self.producer.lock().occupied_len()
    }

    pub fn played_duration_ms(&self) -> u64 {
        let played = self.samples_played.load(Ordering::Acquire);
        let frames = played / self.channels.max(1) as u64;
        let from_samples = (frames * 1000) / self.sample_rate.max(1) as u64;
        self.position_offset_ms.load(Ordering::Acquire) + from_samples
    }

    pub fn set_position_anchor_ms(&self, ms: u64) {
        self.position_offset_ms.store(ms, Ordering::Release);
        self.samples_played.store(0, Ordering::Release);
    }

    pub fn reset_position(&self) {
        self.samples_played.store(0, Ordering::Release);
    }

    /// Mark every sample currently in the ring buffer as stale; the audio
    /// callback will drain them as fast as possible (silence) and resume
    /// normal playback once the buffer refills with the new track.
    pub fn drain_buffer(&self) {
        let buffered = self.producer.lock().occupied_len();
        self.skip_samples.store(buffered as u64, Ordering::Release);
    }

    pub fn clear(&self) {
        let buffered = self.producer.lock().occupied_len();
        self.skip_samples.store(buffered as u64, Ordering::Release);
        self.samples_played.store(0, Ordering::Release);
        self.position_offset_ms.store(0, Ordering::Release);
    }

    pub fn play(&self) {
        self.paused.store(false, Ordering::Release);
    }
    pub fn pause(&self) {
        self.paused.store(true, Ordering::Release);
    }

    pub fn set_volume(&self, v: f32) {
        // Guard against NaN before clamping — `f32::clamp` propagates NaN,
        // and a NaN gain would write NaN samples straight to the device.
        let safe = if v.is_finite() { v } else { 0.0 };
        let clamped = safe.clamp(0.0, 1.0);
        self.volume_bits.store(clamped.to_bits(), Ordering::Relaxed);
    }

    pub fn set_equalizer(&self, enabled: bool, bands_db: [f32; 10]) {
        self.eq_settings.set(enabled, bands_db);
    }
}

struct EqSettings {
    enabled: AtomicBool,
    bands_bits: [AtomicU32; 10],
    version: AtomicU64,
}

impl EqSettings {
    fn new() -> Self {
        Self {
            enabled: AtomicBool::new(false),
            bands_bits: std::array::from_fn(|_| AtomicU32::new(0.0_f32.to_bits())),
            version: AtomicU64::new(0),
        }
    }

    fn set(&self, enabled: bool, bands_db: [f32; 10]) {
        for (slot, db) in self.bands_bits.iter().zip(bands_db) {
            let safe = if db.is_finite() {
                db.clamp(-24.0, 24.0)
            } else {
                0.0
            };
            slot.store(safe.to_bits(), Ordering::Relaxed);
        }
        self.enabled.store(enabled, Ordering::Release);
        self.version.fetch_add(1, Ordering::AcqRel);
    }

    fn snapshot(&self) -> (bool, [f32; 10], u64) {
        let version = self.version.load(Ordering::Acquire);
        let enabled = self.enabled.load(Ordering::Acquire);
        let bands =
            std::array::from_fn(|i| f32::from_bits(self.bands_bits[i].load(Ordering::Relaxed)));
        (enabled, bands, version)
    }
}

struct CallbackEq {
    settings: Arc<EqSettings>,
    eq: Equalizer,
    channels: u16,
    seen_version: u64,
}

impl CallbackEq {
    fn new(sample_rate: u32, channels: u16, settings: Arc<EqSettings>) -> Self {
        Self {
            settings,
            eq: Equalizer::new(sample_rate, channels),
            channels,
            seen_version: u64::MAX,
        }
    }

    fn process(&mut self, samples: &mut [f32], channels: u16) {
        let version = self.settings.version.load(Ordering::Acquire);
        if version != self.seen_version {
            let (enabled, bands, snapshot_version) = self.settings.snapshot();
            self.eq.set_all(bands);
            self.eq.set_enabled(enabled);
            self.seen_version = snapshot_version;
        }
        self.eq.process_inplace(samples, channels);
    }
}

pub struct AudioOutput {
    controls: OutputControls,
    _stream: Stream,
}

impl AudioOutput {
    pub fn new() -> Result<Self, String> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or_else(|| "no default output device".to_string())?;
        let supported = device
            .default_output_config()
            .map_err(|e| format!("default config: {e}"))?;
        let sample_format = supported.sample_format();
        let sr = supported.sample_rate().0;
        let ch = supported.channels();
        let cfg: StreamConfig = supported.into();

        let buffer_size = (sr as usize) * (ch as usize) * BUFFER_MILLIS as usize / 1000;
        let rb: HeapRb<f32> = HeapRb::new(buffer_size);
        let (producer, mut consumer) = rb.split();

        let samples_played = Arc::new(AtomicU64::new(0));
        let position_offset_ms = Arc::new(AtomicU64::new(0));
        let skip_samples = Arc::new(AtomicU64::new(0));
        let volume_bits = Arc::new(AtomicU32::new(1.0_f32.to_bits()));
        let eq_settings = Arc::new(EqSettings::new());
        let paused = Arc::new(AtomicBool::new(false));

        let played_clone = samples_played.clone();
        let skip_clone = skip_samples.clone();
        let vol_clone = volume_bits.clone();
        let eq_clone = eq_settings.clone();
        let paused_clone = paused.clone();

        let err_fn = |e| log::error!("cpal stream error: {e}");

        let stream = match sample_format {
            SampleFormat::F32 => device
                .build_output_stream::<f32, _, _>(
                    &cfg,
                    {
                        let mut eq = CallbackEq::new(sr, ch, eq_clone);
                        move |out: &mut [f32], _info: &cpal::OutputCallbackInfo| {
                            fill_callback(
                                out,
                                &mut consumer,
                                &played_clone,
                                &skip_clone,
                                &vol_clone,
                                &paused_clone,
                                &mut eq,
                            );
                        }
                    },
                    err_fn,
                    None,
                )
                .map_err(|e| format!("build stream: {e}"))?,
            SampleFormat::I16 => {
                let mut tmp: Vec<f32> = vec![0.0; buffer_size.max(ch as usize)];
                let mut eq = CallbackEq::new(sr, ch, eq_settings.clone());
                device
                    .build_output_stream::<i16, _, _>(
                        &cfg,
                        move |out: &mut [i16], _info: &cpal::OutputCallbackInfo| {
                            let mut ctx = CallbackContext {
                                samples_played: &played_clone,
                                skip_samples: &skip_clone,
                                volume_bits: &vol_clone,
                                paused: &paused_clone,
                                eq: &mut eq,
                            };
                            fill_i16_callback(out, &mut tmp, &mut consumer, &mut ctx);
                        },
                        err_fn,
                        None,
                    )
                    .map_err(|e| format!("build stream: {e}"))?
            }
            SampleFormat::U16 => {
                let mut tmp: Vec<f32> = vec![0.0; buffer_size.max(ch as usize)];
                let mut eq = CallbackEq::new(sr, ch, eq_settings.clone());
                device
                    .build_output_stream::<u16, _, _>(
                        &cfg,
                        move |out: &mut [u16], _info: &cpal::OutputCallbackInfo| {
                            let mut ctx = CallbackContext {
                                samples_played: &played_clone,
                                skip_samples: &skip_clone,
                                volume_bits: &vol_clone,
                                paused: &paused_clone,
                                eq: &mut eq,
                            };
                            fill_u16_callback(out, &mut tmp, &mut consumer, &mut ctx);
                        },
                        err_fn,
                        None,
                    )
                    .map_err(|e| format!("build stream: {e}"))?
            }
            other => return Err(format!("unsupported sample format: {other:?}")),
        };

        stream.play().map_err(|e| format!("stream.play: {e}"))?;

        let controls = OutputControls {
            sample_rate: sr,
            channels: ch,
            producer: Arc::new(Mutex::new(producer)),
            samples_played,
            position_offset_ms,
            skip_samples,
            volume_bits,
            eq_settings,
            paused,
        };

        Ok(Self {
            controls,
            _stream: stream,
        })
    }

    pub fn controls(&self) -> &OutputControls {
        &self.controls
    }
}

fn fill_callback(
    out: &mut [f32],
    consumer: &mut HeapCons<f32>,
    samples_played: &AtomicU64,
    skip_samples: &AtomicU64,
    volume_bits: &AtomicU32,
    paused: &AtomicBool,
    eq: &mut CallbackEq,
) {
    if paused.load(Ordering::Acquire) {
        out.fill(0.0);
        return;
    }

    // Drain pending stale samples (set by clear() at track switch). The
    // engine guarantees the previous decoder is fully joined before clear()
    // takes the occupied snapshot, so skip_samples is exactly the count of
    // old-track samples; new decoder pushes can't sneak in before this.
    let skip = skip_samples.load(Ordering::Acquire);
    if skip > 0 {
        let mut tmp = [0.0f32; 1024];
        let mut total: u64 = 0;
        while total < skip {
            let want = ((skip - total) as usize).min(tmp.len());
            let popped = consumer.pop_slice(&mut tmp[..want]) as u64;
            if popped == 0 {
                break;
            }
            total += popped;
        }
        if total > 0 {
            skip_samples.fetch_sub(total, Ordering::AcqRel);
        }
    }

    let vol = f32::from_bits(volume_bits.load(Ordering::Relaxed));
    let popped = consumer.pop_slice(out);
    eq.process(&mut out[..popped], eq.channels);
    for s in &mut out[..popped] {
        *s *= vol;
    }
    if popped < out.len() {
        for s in &mut out[popped..] {
            *s = 0.0;
        }
    }
    samples_played.fetch_add(popped as u64, Ordering::AcqRel);
}

fn fill_i16_callback(
    out: &mut [i16],
    scratch: &mut [f32],
    consumer: &mut HeapCons<f32>,
    ctx: &mut CallbackContext<'_>,
) {
    if scratch.is_empty() {
        out.fill(0);
        return;
    }
    for chunk in out.chunks_mut(scratch.len()) {
        let tmp = &mut scratch[..chunk.len()];
        fill_callback(
            tmp,
            consumer,
            ctx.samples_played,
            ctx.skip_samples,
            ctx.volume_bits,
            ctx.paused,
            ctx.eq,
        );
        for (dst, src) in chunk.iter_mut().zip(tmp.iter()) {
            *dst = (src.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
        }
    }
}

fn fill_u16_callback(
    out: &mut [u16],
    scratch: &mut [f32],
    consumer: &mut HeapCons<f32>,
    ctx: &mut CallbackContext<'_>,
) {
    if scratch.is_empty() {
        out.fill(u16::MAX / 2);
        return;
    }
    for chunk in out.chunks_mut(scratch.len()) {
        let tmp = &mut scratch[..chunk.len()];
        fill_callback(
            tmp,
            consumer,
            ctx.samples_played,
            ctx.skip_samples,
            ctx.volume_bits,
            ctx.paused,
            ctx.eq,
        );
        for (dst, src) in chunk.iter_mut().zip(tmp.iter()) {
            let v = (src.clamp(-1.0, 1.0) + 1.0) * 0.5 * u16::MAX as f32;
            *dst = v as u16;
        }
    }
}

struct CallbackContext<'a> {
    samples_played: &'a AtomicU64,
    skip_samples: &'a AtomicU64,
    volume_bits: &'a AtomicU32,
    paused: &'a AtomicBool,
    eq: &'a mut CallbackEq,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn callback_parts(
        capacity: usize,
    ) -> (
        HeapProd<f32>,
        HeapCons<f32>,
        AtomicU64,
        AtomicU64,
        AtomicU32,
        AtomicBool,
        CallbackEq,
    ) {
        let rb: HeapRb<f32> = HeapRb::new(capacity);
        let (producer, consumer) = rb.split();
        let eq_settings = Arc::new(EqSettings::new());
        let eq = CallbackEq::new(44_100, 2, eq_settings);
        (
            producer,
            consumer,
            AtomicU64::new(0),
            AtomicU64::new(0),
            AtomicU32::new(1.0_f32.to_bits()),
            AtomicBool::new(false),
            eq,
        )
    }

    #[test]
    fn fill_callback_applies_equalizer_settings() {
        let (mut producer, mut consumer, played, skip, volume, paused, mut eq) =
            callback_parts(4096);
        eq.settings
            .set(true, [0.0, 0.0, 0.0, 0.0, 0.0, 12.0, 0.0, 0.0, 0.0, 0.0]);
        let input = vec![0.5_f32; 2048];
        producer.push_slice(&input);

        let mut out = vec![0.0_f32; 2048];
        fill_callback(
            &mut out,
            &mut consumer,
            &played,
            &skip,
            &volume,
            &paused,
            &mut eq,
        );

        assert!(out.iter().any(|s| (*s - 0.5).abs() > 0.001));
    }

    #[test]
    fn i16_callback_uses_fixed_scratch_without_growth() {
        let (mut producer, mut consumer, played, skip, volume, paused, mut eq) = callback_parts(32);
        let input = vec![0.25_f32; 16];
        producer.push_slice(&input);
        let mut out = [0_i16; 16];
        let mut scratch = vec![0.0_f32; 4];
        let capacity_before = scratch.capacity();
        let mut ctx = CallbackContext {
            samples_played: &played,
            skip_samples: &skip,
            volume_bits: &volume,
            paused: &paused,
            eq: &mut eq,
        };

        fill_i16_callback(&mut out, &mut scratch, &mut consumer, &mut ctx);

        assert_eq!(scratch.capacity(), capacity_before);
        assert!(out.iter().all(|s| *s > 0));
        assert_eq!(consumer.occupied_len(), 0);
    }

    #[test]
    fn buffered_samples_is_derived_from_ring_state() {
        let rb: HeapRb<f32> = HeapRb::new(8);
        let (producer, mut consumer) = rb.split();
        let controls = OutputControls {
            sample_rate: 44_100,
            channels: 2,
            producer: Arc::new(Mutex::new(producer)),
            samples_played: Arc::new(AtomicU64::new(0)),
            position_offset_ms: Arc::new(AtomicU64::new(0)),
            skip_samples: Arc::new(AtomicU64::new(0)),
            volume_bits: Arc::new(AtomicU32::new(1.0_f32.to_bits())),
            eq_settings: Arc::new(EqSettings::new()),
            paused: Arc::new(AtomicBool::new(false)),
        };

        assert_eq!(controls.push_samples(&[0.1, 0.2, 0.3, 0.4]), 4);
        assert_eq!(controls.buffered_samples(), 4);

        let mut drained = [0.0_f32; 4];
        assert_eq!(consumer.pop_slice(&mut drained), 4);
        assert_eq!(controls.buffered_samples(), 0);
    }
}
