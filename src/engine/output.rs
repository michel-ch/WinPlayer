use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, Stream, StreamConfig};
use parking_lot::Mutex;
use ringbuf::{traits::*, HeapCons, HeapProd, HeapRb};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

const BUFFER_SECONDS: u32 = 2;

#[derive(Clone)]
pub struct OutputControls {
    pub sample_rate: u32,
    pub channels: u16,
    producer: Arc<Mutex<HeapProd<f32>>>,
    samples_played: Arc<AtomicU64>,
    position_offset_ms: Arc<AtomicU64>,
    skip_samples: Arc<AtomicU64>,
    volume: Arc<Mutex<f32>>,
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

    pub fn drain_buffer(&self) {
        let occupied = self.producer.lock().occupied_len();
        self.skip_samples.store(occupied as u64, Ordering::Release);
    }

    pub fn clear(&self) {
        let occupied = self.producer.lock().occupied_len();
        self.skip_samples.store(occupied as u64, Ordering::Release);
        self.samples_played.store(0, Ordering::Release);
        self.position_offset_ms.store(0, Ordering::Release);
    }

    pub fn play(&self) { self.paused.store(false, Ordering::Release); }
    pub fn pause(&self) { self.paused.store(true, Ordering::Release); }

    pub fn set_volume(&self, v: f32) {
        *self.volume.lock() = v.clamp(0.0, 1.0);
    }
}

pub struct AudioOutput {
    controls: OutputControls,
    _stream: Stream,
}

impl AudioOutput {
    pub fn new() -> Result<Self, String> {
        let host = cpal::default_host();
        let device = host.default_output_device()
            .ok_or_else(|| "no default output device".to_string())?;
        let supported = device.default_output_config()
            .map_err(|e| format!("default config: {e}"))?;
        let sample_format = supported.sample_format();
        let sr = supported.sample_rate().0;
        let ch = supported.channels();
        let cfg: StreamConfig = supported.into();

        let buffer_size = (sr as usize) * (ch as usize) * BUFFER_SECONDS as usize;
        let rb: HeapRb<f32> = HeapRb::new(buffer_size);
        let (producer, mut consumer) = rb.split();

        let samples_played = Arc::new(AtomicU64::new(0));
        let position_offset_ms = Arc::new(AtomicU64::new(0));
        let skip_samples = Arc::new(AtomicU64::new(0));
        let volume = Arc::new(Mutex::new(1.0_f32));
        let paused = Arc::new(AtomicBool::new(false));

        let played_clone = samples_played.clone();
        let skip_clone = skip_samples.clone();
        let vol_clone = volume.clone();
        let paused_clone = paused.clone();

        let err_fn = |e| log::error!("cpal stream error: {e}");

        let stream = match sample_format {
            SampleFormat::F32 => device.build_output_stream::<f32, _, _>(
                &cfg,
                move |out: &mut [f32], _info: &cpal::OutputCallbackInfo| {
                    fill_callback(out, &mut consumer, &played_clone, &skip_clone, &vol_clone, &paused_clone);
                },
                err_fn, None,
            ).map_err(|e| format!("build stream: {e}"))?,
            SampleFormat::I16 => device.build_output_stream::<i16, _, _>(
                &cfg,
                move |out: &mut [i16], _info: &cpal::OutputCallbackInfo| {
                    let mut tmp = vec![0.0f32; out.len()];
                    fill_callback(&mut tmp, &mut consumer, &played_clone, &skip_clone, &vol_clone, &paused_clone);
                    for (dst, src) in out.iter_mut().zip(tmp.iter()) {
                        *dst = (src.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
                    }
                },
                err_fn, None,
            ).map_err(|e| format!("build stream: {e}"))?,
            SampleFormat::U16 => device.build_output_stream::<u16, _, _>(
                &cfg,
                move |out: &mut [u16], _info: &cpal::OutputCallbackInfo| {
                    let mut tmp = vec![0.0f32; out.len()];
                    fill_callback(&mut tmp, &mut consumer, &played_clone, &skip_clone, &vol_clone, &paused_clone);
                    for (dst, src) in out.iter_mut().zip(tmp.iter()) {
                        let v = (src.clamp(-1.0, 1.0) + 1.0) * 0.5 * u16::MAX as f32;
                        *dst = v as u16;
                    }
                },
                err_fn, None,
            ).map_err(|e| format!("build stream: {e}"))?,
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
            volume,
            paused,
        };

        Ok(Self { controls, _stream: stream })
    }

    pub fn controls(&self) -> &OutputControls { &self.controls }
}

fn fill_callback(
    out: &mut [f32],
    consumer: &mut HeapCons<f32>,
    samples_played: &AtomicU64,
    skip_samples: &AtomicU64,
    volume: &Mutex<f32>,
    paused: &AtomicBool,
) {
    if paused.load(Ordering::Acquire) {
        out.fill(0.0);
        return;
    }
    let vol = *volume.lock();
    let mut i = 0;

    let skip = skip_samples.load(Ordering::Acquire);
    if skip > 0 {
        let take = (skip as usize).min(out.len());
        let mut discard = vec![0.0; take];
        let popped = consumer.pop_slice(&mut discard);
        skip_samples.fetch_sub(popped as u64, Ordering::AcqRel);
        for slot in &mut out[..take] { *slot = 0.0; }
        i = take;
    }

    if i < out.len() {
        let popped = consumer.pop_slice(&mut out[i..]);
        let total_filled = i + popped;
        for s in &mut out[i..total_filled] { *s *= vol; }
        if total_filled < out.len() {
            for s in &mut out[total_filled..] { *s = 0.0; }
        }
        samples_played.fetch_add(popped as u64, Ordering::AcqRel);
    }
}
