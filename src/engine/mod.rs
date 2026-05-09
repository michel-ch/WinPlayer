pub mod decoder;
pub mod eq;
pub mod output;

use crate::engine::decoder::{prepare_decode, spawn_decode, DecodeJob};
use crate::engine::output::AudioOutput;
use crossbeam_channel::{bounded, Receiver, Sender};
use std::path::PathBuf;
use std::time::Duration;

#[derive(Debug, Clone)]
pub enum EngineCmd {
    Load { path: PathBuf, autoplay: bool },
    Play,
    Pause,
    Stop,
    SeekFraction(f32),
    SetVolume(f32),
    Shutdown,
}

#[derive(Debug, Clone)]
pub enum EngineEvent {
    LoadStarted(PathBuf),
    LoadFailed { path: PathBuf, error: String },
    Started { duration_ms: u64 },
    Position { current_ms: u64, duration_ms: u64 },
    Paused,
    Resumed,
    EndOfTrack,
}

pub struct Engine {
    cmd_tx: Sender<EngineCmd>,
    evt_rx: Receiver<EngineEvent>,
}

impl Engine {
    pub fn start() -> Result<Self, String> {
        let (cmd_tx, cmd_rx) = bounded::<EngineCmd>(64);
        let (evt_tx, evt_rx) = bounded::<EngineEvent>(256);

        std::thread::Builder::new().name("engine".into()).spawn(move || {
            engine_thread(cmd_rx, evt_tx);
        }).map_err(|e| format!("spawn engine: {e}"))?;

        Ok(Self { cmd_tx, evt_rx })
    }

    pub fn send(&self, cmd: EngineCmd) {
        let _ = self.cmd_tx.send(cmd);
    }

    pub fn events(&self) -> Receiver<EngineEvent> { self.evt_rx.clone() }
}

fn engine_thread(cmd_rx: Receiver<EngineCmd>, evt_tx: Sender<EngineEvent>) {
    let output = match AudioOutput::new() {
        Ok(o) => o,
        Err(e) => {
            log::error!("audio output init failed: {e}");
            return;
        }
    };
    let controls = output.controls().clone();

    let mut current_job: Option<DecodeJob> = None;
    let mut current_duration_ms: u64 = 0;
    let mut paused = false;

    loop {
        match cmd_rx.recv_timeout(Duration::from_millis(20)) {
            Ok(cmd) => match cmd {
                EngineCmd::Shutdown => break,
                EngineCmd::Load { path, autoplay } => {
                    let _ = evt_tx.send(EngineEvent::LoadStarted(path.clone()));
                    // Do all the slow work (open + probe + build decoder + build
                    // resampler) BEFORE stopping the old track, so the previous
                    // song stays audible right up to the swap.
                    match prepare_decode(&path, controls.sample_rate, controls.channels) {
                        Ok(prepared) => {
                            if let Some(j) = current_job.take() { j.stop(); }
                            controls.clear();
                            let job = spawn_decode(prepared, controls.clone());
                            current_duration_ms = job.duration.as_millis() as u64;
                            current_job = Some(job);
                            paused = !autoplay;
                            if autoplay { controls.play(); } else { controls.pause(); }
                            let _ = evt_tx.send(EngineEvent::Started { duration_ms: current_duration_ms });
                        }
                        Err(e) => {
                            let _ = evt_tx.send(EngineEvent::LoadFailed { path, error: e });
                        }
                    }
                }
                EngineCmd::Play => {
                    controls.play();
                    paused = false;
                    let _ = evt_tx.send(EngineEvent::Resumed);
                }
                EngineCmd::Pause => {
                    controls.pause();
                    paused = true;
                    let _ = evt_tx.send(EngineEvent::Paused);
                }
                EngineCmd::Stop => {
                    if let Some(j) = current_job.take() { j.stop(); }
                    controls.clear();
                    current_duration_ms = 0;
                }
                EngineCmd::SeekFraction(frac) => {
                    if let Some(job) = &current_job {
                        let target_ms = (current_duration_ms as f32 * frac.clamp(0.0, 1.0)) as u64;
                        job.seek(target_ms);
                        controls.drain_buffer();
                        controls.set_position_anchor_ms(target_ms);
                        if !paused { controls.play(); }
                    }
                }
                EngineCmd::SetVolume(v) => {
                    controls.set_volume(v);
                }
            },
            Err(crossbeam_channel::RecvTimeoutError::Timeout) => {}
            Err(crossbeam_channel::RecvTimeoutError::Disconnected) => break,
        }

        if let Some(job) = &current_job {
            let current_ms = controls.played_duration_ms();
            let _ = evt_tx.send(EngineEvent::Position {
                current_ms,
                duration_ms: current_duration_ms,
            });
            if job.is_finished() && controls.buffered_samples() == 0 {
                current_job = None;
                current_duration_ms = 0;
                let _ = evt_tx.send(EngineEvent::EndOfTrack);
            }
        }
    }
    drop(output);
}
