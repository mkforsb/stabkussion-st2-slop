//! Platform audio backends behind one small handle.
//!
//! * `web`: an AudioWorklet running the DSP compiled to a standalone wasm module.
//! * `desktop`: a PulseAudio playback stream fed from a dedicated thread.
//!
//! The UI only ever talks to [`AudioHandle`]; it never blocks and never touches
//! the engine directly.

// Without a platform feature only the null backend exists.
#![cfg_attr(not(any(feature = "web", feature = "desktop")), allow(dead_code))]

use dioxus::prelude::*;
use stabkussion_dsp::{PARAM_COUNT, Param, VOICES, params::default_patch};

#[cfg(feature = "desktop")]
mod pulse;
#[cfg(all(feature = "web", not(feature = "desktop")))]
mod web;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Command {
    Param {
        voice: usize,
        param: Param,
        value: f32,
    },
    Master(f32),
    /// `snap`: apply this voice's pending parameter changes instantly
    /// instead of gliding, so they take effect exactly on the hit.
    Trigger {
        voice: usize,
        velocity: f32,
        snap: bool,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub enum AudioStatus {
    /// Web only: waiting for a user gesture before the AudioContext may start.
    #[cfg_attr(feature = "desktop", allow(dead_code))]
    NeedsGesture,
    Starting,
    Running {
        sample_rate: u32,
        detail: String,
    },
    Failed(String),
}

/// Last value sent for every parameter, so a backend that comes up late (the
/// web one, after a user gesture) can be brought in sync in one go.
#[derive(Clone, Debug)]
pub struct ParamCache {
    pub voices: [[f32; PARAM_COUNT]; VOICES],
    pub master: f32,
}

impl Default for ParamCache {
    fn default() -> Self {
        Self {
            voices: [default_patch(); VOICES],
            master: stabkussion_dsp::engine::DEFAULT_MASTER,
        }
    }
}

impl ParamCache {
    #[cfg_attr(feature = "desktop", allow(dead_code))]
    pub fn apply(&mut self, cmd: Command) {
        match cmd {
            Command::Param {
                voice,
                param,
                value,
            } => {
                if let Some(v) = self.voices.get_mut(voice) {
                    v[param.index()] = value;
                }
            }
            Command::Master(m) => self.master = m,
            Command::Trigger { .. } => {}
        }
    }

    pub fn commands(&self) -> impl Iterator<Item = Command> + '_ {
        self.voices
            .iter()
            .enumerate()
            .flat_map(|(voice, patch)| {
                stabkussion_dsp::ALL_PARAMS
                    .iter()
                    .map(move |&param| Command::Param {
                        voice,
                        param,
                        value: patch[param.index()],
                    })
            })
            .chain(std::iter::once(Command::Master(self.master)))
    }
}

#[cfg(feature = "desktop")]
type Backend = pulse::PulseBackend;
#[cfg(all(feature = "web", not(feature = "desktop")))]
type Backend = web::WebBackend;
#[cfg(not(any(feature = "web", feature = "desktop")))]
type Backend = NullBackend;

/// Cheap to clone; shared through the Dioxus context.
#[derive(Clone)]
pub struct AudioHandle {
    backend: std::rc::Rc<Backend>,
    pub status: Signal<AudioStatus>,
}

impl AudioHandle {
    /// Must be called inside the Dioxus runtime (e.g. from `use_hook`).
    /// `initial` is the patch the engine should start with.
    pub fn new(initial: ParamCache) -> Self {
        let status = Signal::new(AudioStatus::Starting);
        let backend = std::rc::Rc::new(Backend::new(status, initial));
        Self { backend, status }
    }

    pub fn send(&self, cmd: Command) {
        self.backend.send(cmd);
    }

    pub fn set_param(&self, voice: usize, param: Param, value: f32) {
        self.send(Command::Param {
            voice,
            param,
            value,
        });
    }

    pub fn trigger(&self, voice: usize, velocity: f32, snap: bool) {
        self.send(Command::Trigger {
            voice,
            velocity,
            snap,
        });
    }

    /// Call from user-gesture handlers; lets the web backend start or resume.
    pub fn user_gesture(&self) {
        self.backend.user_gesture();
    }
}

/// Used when building without a platform feature (e.g. `cargo check`).
#[cfg(not(any(feature = "web", feature = "desktop")))]
pub struct NullBackend;

#[cfg(not(any(feature = "web", feature = "desktop")))]
impl NullBackend {
    fn new(mut status: Signal<AudioStatus>, _initial: ParamCache) -> Self {
        status.set(AudioStatus::Failed("built without an audio backend".into()));
        Self
    }
    fn send(&self, _cmd: Command) {}
    fn user_gesture(&self) {}
}
