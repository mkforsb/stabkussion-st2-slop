//! PulseAudio backend (works with PipeWire's pulse server too).
//!
//! A dedicated thread owns the engine and a `pa_simple` playback stream. The UI
//! sends [`Command`]s over a channel; the thread drains it before every block,
//! so parameter changes and triggers land within one block plus the stream's
//! target latency.

use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use dioxus::prelude::*;
use libpulse_binding::def::BufferAttr;
use libpulse_binding::sample::{Format, Spec};
use libpulse_binding::stream::Direction;
use libpulse_simple_binding::Simple;
use stabkussion_dsp::Engine;

use super::{AudioStatus, Command, ParamCache};

const SAMPLE_RATE: u32 = 48_000;
const CHANNELS: u8 = 2;
const BLOCK: usize = 128;
const BYTES_PER_FRAME: usize = 4 * CHANNELS as usize;
/// Default target latency; override with `STABKUSSION_LATENCY_MS`.
const DEFAULT_LATENCY_MS: u32 = 20;

pub struct PulseBackend {
    tx: mpsc::Sender<Command>,
}

impl PulseBackend {
    pub fn new(mut status: Signal<AudioStatus>, initial: ParamCache) -> Self {
        let (tx, rx) = mpsc::channel::<Command>();
        let (ready_tx, ready_rx) = mpsc::channel::<Result<String, String>>();
        let latency_ms = std::env::var("STABKUSSION_LATENCY_MS")
            .ok()
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(DEFAULT_LATENCY_MS)
            .clamp(3, 500);

        let spawned = thread::Builder::new()
            .name("stabkussion-audio".into())
            .spawn(move || match open_stream(latency_ms) {
                Ok(stream) => {
                    let _ =
                        ready_tx.send(Ok(format!("PulseAudio · {latency_ms} ms target latency")));
                    run(stream, rx, initial);
                }
                Err(e) => {
                    let _ = ready_tx.send(Err(e));
                }
            });

        let result = match spawned {
            Err(e) => Err(format!("could not spawn audio thread: {e}")),
            Ok(_) => ready_rx
                .recv_timeout(Duration::from_secs(5))
                .unwrap_or_else(|_| Err("timed out connecting to PulseAudio".into())),
        };
        status.set(match result {
            Ok(detail) => AudioStatus::Running {
                sample_rate: SAMPLE_RATE,
                detail,
            },
            Err(e) => {
                eprintln!("stabkussion: audio unavailable: {e}");
                AudioStatus::Failed(e)
            }
        });
        Self { tx }
    }

    pub fn send(&self, cmd: Command) {
        // If the audio thread died the error is already shown in the status.
        let _ = self.tx.send(cmd);
    }

    pub fn user_gesture(&self) {}
}

fn open_stream(latency_ms: u32) -> Result<Simple, String> {
    let spec = Spec {
        format: Format::F32le,
        channels: CHANNELS,
        rate: SAMPLE_RATE,
    };
    if !spec.is_valid() {
        return Err("invalid sample spec".into());
    }
    let tlength = (SAMPLE_RATE * latency_ms / 1000) * BYTES_PER_FRAME as u32;
    let attr = BufferAttr {
        maxlength: u32::MAX,
        tlength,
        prebuf: u32::MAX,
        minreq: (BLOCK * BYTES_PER_FRAME) as u32,
        fragsize: u32::MAX,
    };
    Simple::new(
        None,
        "Stabkussion ST-1",
        Direction::Playback,
        None,
        "Techno stab synthesizer",
        &spec,
        None,
        Some(&attr),
    )
    .map_err(|e| format!("PulseAudio: {e}"))
}

fn run(stream: Simple, rx: mpsc::Receiver<Command>, initial: ParamCache) {
    let mut engine = Engine::new(SAMPLE_RATE as f32);
    for cmd in initial.commands() {
        apply(&mut engine, cmd);
    }
    // Start on the restored patch rather than gliding to it.
    engine.snap_params();
    let mut left = [0.0f32; BLOCK];
    let mut right = [0.0f32; BLOCK];
    let mut bytes = vec![0u8; BLOCK * BYTES_PER_FRAME];
    loop {
        loop {
            match rx.try_recv() {
                Ok(cmd) => apply(&mut engine, cmd),
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => return,
            }
        }
        engine.render(&mut left, &mut right);
        for (i, (l, r)) in left.iter().zip(right.iter()).enumerate() {
            bytes[i * 8..i * 8 + 4].copy_from_slice(&l.to_le_bytes());
            bytes[i * 8 + 4..i * 8 + 8].copy_from_slice(&r.to_le_bytes());
        }
        if let Err(e) = stream.write(&bytes) {
            eprintln!("stabkussion: PulseAudio write failed: {e}");
            return;
        }
    }
}

fn apply(engine: &mut Engine, cmd: Command) {
    match cmd {
        Command::Param {
            voice,
            param,
            value,
        } => engine.set_param(voice, param, value),
        Command::Master(v) => engine.set_master(v),
        Command::Trigger {
            voice,
            velocity,
            snap,
        } => {
            if snap {
                engine.snap_voice(voice);
            }
            engine.trigger(voice, velocity);
        }
    }
}
