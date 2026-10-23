//! FB-FM: a sine carrier phase-modulated by a sine with self-feedback.
//!
//! Feeding a modulator's output back into its own phase bends it from a sine
//! towards a saw (the DX7's operator feedback). Modulating with that instead
//! of a sine fills the gaps between the sidebands: at a whole-number ratio
//! the result is a dense, buzzy harmonic spectrum; at any other ratio it
//! is a thick inharmonic clang. The feedback path averages the last two
//! outputs, which keeps high feedback from breaking into noise.
//!
//! * RATIO: modulator/carrier ratio, 0.25 … 8, continuous.
//! * INDEX: modulation index, 0 … 6 rad; velocity scales it. It holds for
//!   the length of the note (the filter envelope does the closing).
//! * FDBK: modulator feedback, 0 … 1.4 rad.
//! * I.DEC: how much of the index decays, quickly (in about a seventh of the
//!   oscillator's decay): 0 holds it, 1 lets the bark die into a sine.
//! * LOOP: the carrier's output fed back into the modulator's phase, up to
//!   1.6 rad. The two operators modulate each other, the cross-coupled FM
//!   of a Buchla-style complex oscillator: grit and growl at low settings,
//!   a tumbling, chaotic snarl at high ones.
//! * M.SWP: the modulator's ratio starts up to three octaves higher and
//!   falls to RATIO within about 120 ms: an inharmonic laser zap on the hit.

use super::{Ctx, Osc, OscCtx, block_decay, fm_inc, inc};
use crate::params::map;
use crate::util::{Noise, sin_turns};

/// I.DEC: the index envelope's T60 as a fraction of the oscillator's decay.
const INDEX_DECAY_RATIO: f32 = 0.15;
/// M.SWP: T60 of the ratio sweep.
const SWEEP_T60_S: f32 = 0.12;

#[derive(Clone, Debug, Default)]
pub struct FbFm {
    carrier: f32,
    modulator: f32,
    c_inc: f32,
    m_inc: f32,
    /// Index and feedback in turns.
    index: f32,
    feedback: f32,
    /// The modulator's last two outputs.
    y1: f32,
    y2: f32,
    /// I.DEC and M.SWP envelopes, 1 → 0.
    index_env: f32,
    sweep_env: f32,
    /// LOOP gain in turns, and the carrier's last two outputs.
    loop_gain: f32,
    c1: f32,
    c2: f32,
}

impl Osc for FbFm {
    fn reset(&mut self) {
        self.carrier = 0.0;
        self.modulator = 0.0;
        self.y1 = 0.0;
        self.y2 = 0.0;
        self.c1 = 0.0;
        self.c2 = 0.0;
    }

    fn trigger(&mut self, c: &Ctx, o: &OscCtx, _rng: &mut Noise) {
        self.reset();
        self.index_env = 1.0;
        self.sweep_env = 1.0;
        self.control(c, o);
    }

    fn control(&mut self, c: &Ctx, o: &OscCtx) {
        self.index_env *= block_decay(o.decay_s * INDEX_DECAY_RATIO, c.fs);
        self.sweep_env *= block_decay(SWEEP_T60_S, c.fs);
        let sweep = (map::ratio_sweep_octaves(c.w) * self.sweep_env).exp2();
        self.c_inc = inc(o.hz, c.fs);
        self.m_inc = inc(o.hz * map::fm_ratio(c.x) * sweep, c.fs);
        let held = 1.0 - c.u + c.u * self.index_env;
        self.index =
            map::fbfm_index(c.y) * (0.4 + 0.6 * c.velocity) * held / core::f32::consts::TAU;
        self.feedback = map::fbfm_feedback(c.z) / core::f32::consts::TAU;
        self.loop_gain = map::fbfm_loop(c.v) / core::f32::consts::TAU;
    }

    #[inline]
    fn tick(&mut self, fm: f32) -> f32 {
        let mut phase = self.modulator + self.feedback * 0.5 * (self.y1 + self.y2);
        if self.loop_gain != 0.0 {
            phase += self.loop_gain * 0.5 * (self.c1 + self.c2);
        }
        let m = sin_turns(phase);
        self.y2 = self.y1;
        self.y1 = m;
        let y = sin_turns(self.carrier + self.index * m);
        self.c2 = self.c1;
        self.c1 = y;
        self.carrier += fm_inc(self.c_inc, fm);
        if self.carrier >= 1.0 {
            self.carrier -= 1.0;
        }
        self.modulator += fm_inc(self.m_inc, fm);
        if self.modulator >= 1.0 {
            self.modulator -= 1.0;
        }
        y
    }
}
