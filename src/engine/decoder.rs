use crate::engine::output::OutputControls;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use symphonia::core::audio::{AudioBufferRef, Signal};
use symphonia::core::codecs::{Decoder, DecoderOptions};
use symphonia::core::formats::{FormatOptions, FormatReader, SeekMode, SeekTo};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use symphonia::core::units::Time;

pub struct DecodeJob {
    pub stop_flag: Arc<AtomicBool>,
    pub seek_request_ms: Arc<AtomicU64>,
    pub finished: Arc<AtomicBool>,
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
    pub fn stop(&self) { self.stop_flag.store(true, Ordering::Release); }
    pub fn is_finished(&self) -> bool { self.finished.load(Ordering::Acquire) }
}

pub fn start_decode(
    path: PathBuf,
    output: OutputControls,
) -> Result<DecodeJob, String> {
    let file = std::fs::File::open(&path).map_err(|e| format!("open {}: {e}", path.display()))?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }

    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &FormatOptions::default(), &MetadataOptions::default())
        .map_err(|e| format!("probe: {e}"))?;

    let format = probed.format;
    let track = format.default_track().ok_or("no default track")?;
    let track_id = track.id;
    let codec_params = track.codec_params.clone();
    let source_sample_rate = codec_params.sample_rate.ok_or("missing sample rate")?;
    let source_channels = codec_params.channels.map(|c| c.count() as u16).unwrap_or(2);

    let duration = track.codec_params.n_frames
        .and_then(|frames| {
            track.codec_params.sample_rate.map(|sr| Duration::from_secs_f64(frames as f64 / sr as f64))
        })
        .unwrap_or_default();

    let decoder = symphonia::default::get_codecs()
        .make(&codec_params, &DecoderOptions::default())
        .map_err(|e| format!("make decoder: {e}"))?;

    let stop_flag = Arc::new(AtomicBool::new(false));
    let seek_request_ms = Arc::new(AtomicU64::new(NO_SEEK));
    let finished = Arc::new(AtomicBool::new(false));

    let stop_clone = stop_flag.clone();
    let seek_clone = seek_request_ms.clone();
    let finished_clone = finished.clone();

    let device_sr = output.sample_rate;
    let device_ch = output.channels;

    std::thread::Builder::new().name("decoder".into()).spawn(move || {
        decoder_loop(
            format, decoder, track_id,
            source_sample_rate, source_channels,
            device_sr, device_ch,
            output,
            stop_clone, seek_clone, finished_clone,
        );
    }).map_err(|e| format!("spawn decoder: {e}"))?;

    Ok(DecodeJob {
        stop_flag, seek_request_ms, finished,
        duration, source_sample_rate, source_channels,
    })
}

#[allow(clippy::too_many_arguments)]
fn decoder_loop(
    mut format: Box<dyn FormatReader>,
    mut decoder: Box<dyn Decoder>,
    track_id: u32,
    src_sr: u32,
    src_ch: u16,
    dst_sr: u32,
    dst_ch: u16,
    output: OutputControls,
    stop: Arc<AtomicBool>,
    seek_req: Arc<AtomicU64>,
    finished: Arc<AtomicBool>,
) {
    use rubato::{FftFixedInOut, Resampler};
    let chunk_in: usize = 1024;
    let mut resampler: Option<FftFixedInOut<f32>> = if src_sr != dst_sr {
        FftFixedInOut::<f32>::new(src_sr as usize, dst_sr as usize, chunk_in, src_ch as usize).ok()
    } else { None };
    let mut planar: Vec<Vec<f32>> = vec![Vec::with_capacity(8192); src_ch as usize];

    'outer: loop {
        if stop.load(Ordering::Acquire) { break; }

        let seek_ms = seek_req.swap(NO_SEEK, Ordering::AcqRel);
        if seek_ms != NO_SEEK {
            let target = Time::from(Duration::from_millis(seek_ms));
            let _ = format.seek(SeekMode::Coarse, SeekTo::Time { time: target, track_id: Some(track_id) });
            decoder.reset();
            output.reset_position();
            for ch in &mut planar { ch.clear(); }
        }

        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(symphonia::core::errors::Error::IoError(_)) => break,
            Err(e) => { log::warn!("packet error: {e}"); break; }
        };
        if packet.track_id() != track_id { continue; }

        let decoded = match decoder.decode(&packet) {
            Ok(d) => d,
            Err(e) => { log::warn!("decode error: {e}"); continue; }
        };

        match decoded {
            AudioBufferRef::F32(buf) => {
                for (ch_idx, ch_dst) in planar.iter_mut().enumerate() {
                    if ch_idx >= buf.spec().channels.count() { break; }
                    ch_dst.extend_from_slice(buf.chan(ch_idx));
                }
            }
            AudioBufferRef::S16(buf) => {
                for (ch_idx, ch_dst) in planar.iter_mut().enumerate() {
                    if ch_idx >= buf.spec().channels.count() { break; }
                    for &s in buf.chan(ch_idx) {
                        ch_dst.push(s as f32 / i16::MAX as f32);
                    }
                }
            }
            AudioBufferRef::S32(buf) => {
                for (ch_idx, ch_dst) in planar.iter_mut().enumerate() {
                    if ch_idx >= buf.spec().channels.count() { break; }
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
                let chunk: Vec<Vec<f32>> = planar.iter_mut()
                    .map(|c| c.drain(..needed).collect())
                    .collect();
                let chunk_refs: Vec<&[f32]> = chunk.iter().map(|c| c.as_slice()).collect();
                if let Ok(out) = res.process(&chunk_refs, None) {
                    let interleaved = interleave_with_channel_map(&out, dst_ch);
                    push_or_block(&output, &interleaved, &stop);
                }
                if stop.load(Ordering::Acquire) { break 'outer; }
            }
        } else {
            let interleaved = interleave_with_channel_map(&planar, dst_ch);
            push_or_block(&output, &interleaved, &stop);
            for ch in &mut planar { ch.clear(); }
        }
    }
    finished.store(true, Ordering::Release);
}

pub fn interleave_with_channel_map(planar: &[Vec<f32>], dst_ch: u16) -> Vec<f32> {
    let frames = planar.first().map(|c| c.len()).unwrap_or(0);
    let mut out = Vec::with_capacity(frames * dst_ch as usize);
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
    out
}

fn push_or_block(output: &OutputControls, samples: &[f32], stop: &AtomicBool) {
    let mut written = 0;
    while written < samples.len() {
        if stop.load(Ordering::Acquire) { return; }
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
}
