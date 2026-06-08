use crate::engine::output::OutputControls;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;
use symphonia::core::audio::{AudioBufferRef, Signal};
use symphonia::core::codecs::{Decoder, DecoderOptions};
use symphonia::core::formats::{FormatOptions, FormatReader, SeekMode, SeekTo};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use symphonia::core::units::Time;

pub struct DecodeJob {
    stop_flag: Arc<AtomicBool>,
    seek_request_ms: Arc<AtomicU64>,
    finished: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
    pub duration: Duration,
    pub source_sample_rate: u32,
    pub source_channels: u16,
}

const NO_SEEK: u64 = u64::MAX;

impl DecodeJob {
    pub fn seek(&self, ms: u64) {
        let v = if ms == NO_SEEK { NO_SEEK - 1 } else { ms };
        self.seek_request_ms.store(v, Ordering::Release);
    }

    /// Signal stop and block until the decoder thread has fully exited.
    /// This is the critical synchronization point that lets the engine
    /// safely snapshot the ring buffer's stale-sample count before the
    /// next track's decoder starts pushing.
    pub fn stop(mut self) {
        self.stop_flag.store(true, Ordering::Release);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }

    pub fn is_finished(&self) -> bool {
        self.finished.load(Ordering::Acquire)
    }
}

impl Drop for DecodeJob {
    fn drop(&mut self) {
        // Belt-and-suspenders: if someone drops a DecodeJob without calling
        // stop(), make sure the thread doesn't leak indefinitely.
        if let Some(h) = self.handle.take() {
            self.stop_flag.store(true, Ordering::Release);
            let _ = h.join();
        }
    }
}

/// Prepared track state. All slow work (file open, symphonia probe, decoder
/// construction, FFT-plan-based resampler init) is finished by the time this
/// returns — so the caller can keep playing the previous track right up
/// until the moment of swap.
pub struct PreparedDecode {
    format: Box<dyn FormatReader>,
    decoder: Box<dyn Decoder>,
    track_id: u32,
    src_sr: u32,
    src_ch: u16,
    duration: Duration,
    resampler: Option<rubato::FftFixedInOut<f32>>,
}

impl PreparedDecode {
    pub fn duration(&self) -> Duration {
        self.duration
    }
}

pub fn prepare_decode(
    path: &std::path::Path,
    dst_sr: u32,
    _dst_ch: u16,
) -> Result<PreparedDecode, String> {
    let file = std::fs::File::open(path).map_err(|e| format!("open {}: {e}", path.display()))?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }

    let probed = symphonia::default::get_probe()
        .format(
            &hint,
            mss,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .map_err(|e| format!("probe: {e}"))?;

    let format = probed.format;
    let track = format.default_track().ok_or("no default track")?;
    let track_id = track.id;
    let codec_params = track.codec_params.clone();
    let src_sr = codec_params.sample_rate.ok_or("missing sample rate")?;
    let src_ch = codec_params.channels.map(|c| c.count() as u16).unwrap_or(2);
    // A file claiming zero channels would make the resampler's `planar[0]`
    // index panic later — and with `panic = "abort"` that takes the whole
    // process down. Reject it up front as a malformed stream.
    if src_ch == 0 {
        return Err("stream reports zero channels".to_string());
    }

    // `Duration::from_secs_f64` panics on overflow/NaN; a crafted header with
    // an absurd `n_frames` could otherwise abort the process. Use the
    // fallible variant and fall back to "unknown duration" (zero).
    let duration = track
        .codec_params
        .n_frames
        .and_then(|frames| track.codec_params.sample_rate.map(|sr| (frames, sr)))
        .and_then(|(frames, sr)| Duration::try_from_secs_f64(frames as f64 / sr as f64).ok())
        .unwrap_or_default();

    let decoder = symphonia::default::get_codecs()
        .make(&codec_params, &DecoderOptions::default())
        .map_err(|e| format!("make decoder: {e}"))?;

    let resampler = build_resampler(src_sr, dst_sr, src_ch)?;

    Ok(PreparedDecode {
        format,
        decoder,
        track_id,
        src_sr,
        src_ch,
        duration,
        resampler,
    })
}

fn build_resampler(
    src_sr: u32,
    dst_sr: u32,
    src_ch: u16,
) -> Result<Option<rubato::FftFixedInOut<f32>>, String> {
    if src_sr == dst_sr {
        return Ok(None);
    }
    rubato::FftFixedInOut::<f32>::new(src_sr as usize, dst_sr as usize, 1024, src_ch as usize)
        .map(Some)
        .map_err(|e| format!("resampler {src_sr} Hz -> {dst_sr} Hz ({src_ch} channels): {e}"))
}

pub fn spawn_decode(prepared: PreparedDecode, output: OutputControls) -> Result<DecodeJob, String> {
    let stop_flag = Arc::new(AtomicBool::new(false));
    let seek_request_ms = Arc::new(AtomicU64::new(NO_SEEK));
    let finished = Arc::new(AtomicBool::new(false));

    let stop_clone = stop_flag.clone();
    let seek_clone = seek_request_ms.clone();
    let finished_clone = finished.clone();

    let device_ch = output.channels;
    let duration = prepared.duration;
    let src_sr = prepared.src_sr;
    let src_ch = prepared.src_ch;

    let handle = std::thread::Builder::new()
        .name("decoder".into())
        .spawn(move || {
            decoder_loop(
                prepared.format,
                prepared.decoder,
                prepared.track_id,
                prepared.src_ch,
                device_ch,
                output,
                stop_clone,
                seek_clone,
                finished_clone,
                prepared.resampler,
            );
        })
        .map_err(|e| format!("spawn decoder: {e}"))?;

    Ok(DecodeJob {
        stop_flag,
        seek_request_ms,
        finished,
        handle: Some(handle),
        duration,
        source_sample_rate: src_sr,
        source_channels: src_ch,
    })
}

#[allow(clippy::too_many_arguments)]
fn decoder_loop(
    mut format: Box<dyn FormatReader>,
    mut decoder: Box<dyn Decoder>,
    track_id: u32,
    src_ch: u16,
    dst_ch: u16,
    output: OutputControls,
    stop: Arc<AtomicBool>,
    seek_req: Arc<AtomicU64>,
    finished: Arc<AtomicBool>,
    pre_built_resampler: Option<rubato::FftFixedInOut<f32>>,
) {
    use rubato::{FftFixedInOut, Resampler};
    let mut resampler: Option<FftFixedInOut<f32>> = pre_built_resampler;
    let mut planar: Vec<Vec<f32>> = vec![Vec::with_capacity(8192); src_ch as usize];
    let mut interleaved: Vec<f32> = Vec::with_capacity(8192);

    'outer: loop {
        if stop.load(Ordering::Acquire) {
            break;
        }

        let seek_ms = seek_req.swap(NO_SEEK, Ordering::AcqRel);
        if seek_ms != NO_SEEK {
            let target = Time::from(Duration::from_millis(seek_ms));
            let _ = format.seek(
                SeekMode::Coarse,
                SeekTo::Time {
                    time: target,
                    track_id: Some(track_id),
                },
            );
            decoder.reset();
            if let Some(res) = resampler.as_mut() {
                res.reset();
            }
            output.reset_position();
            output.drain_buffer();
            for ch in &mut planar {
                ch.clear();
            }
        }

        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(symphonia::core::errors::Error::IoError(_)) => break,
            Err(e) => {
                log::warn!("packet error: {e}");
                break;
            }
        };
        if packet.track_id() != track_id {
            continue;
        }

        let decoded = match decoder.decode(&packet) {
            Ok(d) => d,
            Err(e) => {
                log::warn!("decode error: {e}");
                continue;
            }
        };

        match decoded {
            AudioBufferRef::F32(buf) => {
                for (ch_idx, ch_dst) in planar.iter_mut().enumerate() {
                    if ch_idx >= buf.spec().channels.count() {
                        break;
                    }
                    ch_dst.extend_from_slice(buf.chan(ch_idx));
                }
            }
            AudioBufferRef::S16(buf) => {
                for (ch_idx, ch_dst) in planar.iter_mut().enumerate() {
                    if ch_idx >= buf.spec().channels.count() {
                        break;
                    }
                    for &s in buf.chan(ch_idx) {
                        ch_dst.push(s as f32 / i16::MAX as f32);
                    }
                }
            }
            AudioBufferRef::S32(buf) => {
                for (ch_idx, ch_dst) in planar.iter_mut().enumerate() {
                    if ch_idx >= buf.spec().channels.count() {
                        break;
                    }
                    for &s in buf.chan(ch_idx) {
                        ch_dst.push(s as f32 / i32::MAX as f32);
                    }
                }
            }
            _ => continue,
        }

        if let Some(res) = resampler.as_mut() {
            let needed = res.input_frames_next();
            while planar[0].len() >= needed && !stop.load(Ordering::Acquire) {
                // Per-iteration `Vec<&[f32]>` — the borrow checker won't let
                // us hoist this across iterations because `planar` is
                // mutated below. Cost is one small Vec alloc per resample
                // chunk (~46/sec at 44.1k → 48k); inconsequential.
                let chunk_refs: Vec<&[f32]> = planar.iter().map(|c| &c[..needed]).collect();
                if let Ok(out) = res.process(&chunk_refs, None) {
                    interleave_into(&out, dst_ch, &mut interleaved);
                    push_or_block(&output, &interleaved, &stop);
                }
                for ch in &mut planar {
                    ch.drain(..needed);
                }
                if stop.load(Ordering::Acquire) {
                    break 'outer;
                }
            }
        } else {
            interleave_into(&planar, dst_ch, &mut interleaved);
            push_or_block(&output, &interleaved, &stop);
            for ch in &mut planar {
                ch.clear();
            }
        }
    }
    finished.store(true, Ordering::Release);
}

/// Interleave `planar` channels into `out`, reusing its allocation. Handles
/// channel-count mismatch with a basic mono-up / clamp-down map (proper
/// surround downmix is future work).
pub fn interleave_into(planar: &[Vec<f32>], dst_ch: u16, out: &mut Vec<f32>) {
    let frames = planar.first().map(|c| c.len()).unwrap_or(0);
    let total = frames * dst_ch as usize;
    out.clear();
    out.reserve(total);
    for f in 0..frames {
        for c in 0..dst_ch {
            let src_ch = if planar.len() == 1 {
                0
            } else if (c as usize) < planar.len() {
                c as usize
            } else {
                planar.len() - 1
            };
            out.push(planar[src_ch].get(f).copied().unwrap_or(0.0));
        }
    }
}

#[cfg(test)]
pub fn interleave_with_channel_map(planar: &[Vec<f32>], dst_ch: u16) -> Vec<f32> {
    let mut out = Vec::new();
    interleave_into(planar, dst_ch, &mut out);
    out
}

fn push_or_block(output: &OutputControls, samples: &[f32], stop: &AtomicBool) {
    let mut written = 0;
    while written < samples.len() {
        if stop.load(Ordering::Acquire) {
            return;
        }
        let n = output.push_samples(&samples[written..]);
        if n == 0 {
            std::thread::sleep(Duration::from_millis(5));
        } else {
            written += n;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mono_to_stereo_duplicates() {
        let planar = vec![vec![0.5_f32, 0.6, 0.7]];
        let out = interleave_with_channel_map(&planar, 2);
        assert_eq!(out, vec![0.5, 0.5, 0.6, 0.6, 0.7, 0.7]);
    }

    #[test]
    fn stereo_to_stereo_passthrough() {
        let planar = vec![vec![0.1, 0.2], vec![0.3, 0.4]];
        let out = interleave_with_channel_map(&planar, 2);
        assert_eq!(out, vec![0.1, 0.3, 0.2, 0.4]);
    }

    #[test]
    fn resampler_mismatch_reports_construction_error() {
        let err = match build_resampler(44_100, 0, 2) {
            Ok(_) => panic!("invalid output rate must error"),
            Err(err) => err,
        };
        assert!(err.contains("resampler"));
    }
}
