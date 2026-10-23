//! SUPER: five detuned unison voices per oscillator.
//!
//! Detuned saws are the classic chord-stab thickener: the beating makes the
//! stab shimmer. Here every oscillator is a stack of five band-limited
//! saw/pulse voices, the outer pair detuned by DETUNE and the inner pair by an
//! irrational fraction of it, so the beating never lines up. The voices start
//! at fixed, spread-out phases: every hit has the same attack without the
//! stack slamming in phase.
//!
//! * DETUNE: outer voices ±0 … 80 cents.
//! * BLEND: level of the four side voices against the centre one.
//! * WAVE: saw → square.
//! * OCTAVE: fades in a sixth and seventh voice an octave up, detuned like
//!   the inner pair: the "hypersaw" sheen.
//! * SUB: a sine one octave down, under the stack.
//! * BLOOM: extra detune of the whole stack at the envelope peak (up to 60
//!   cents on the outer pair), settling as the hit decays: the stab opens
//!   wide and closes in.

use super::{Ctx, Osc, OscCtx, fm_inc, inc};
use crate::params::map;
use crate::util::{BlepOsc, Morph, Noise, sin_turns};

const VOICES: usize = 5;
/// Detune of each voice as a fraction of DETUNE.
const SPREAD: [f32; VOICES] = [-1.0, -0.43, 0.0, 0.47, 1.0];
/// Start phases (the centre voice starts at zero).
const PHASES: [f32; VOICES] = [0.37, 0.71, 0.0, 0.13, 0.58];
/// The octave pair: detune fractions and start phases.
const OCT_SPREAD: [f32; 2] = [-0.61, 0.53];
const OCT_PHASES: [f32; 2] = [0.21, 0.83];

#[derive(Clone, Debug, Default)]
pub struct Unison {
    voices: [BlepOsc; VOICES],
    incs: [f32; VOICES],
    gains: [f32; VOICES],
    shape: Morph,
    octave: [BlepOsc; 2],
    oct_incs: [f32; 2],
    oct_gain: f32,
    sub: f32,
    sub_inc: f32,
    sub_gain: f32,
}

impl Osc for Unison {
    fn reset(&mut self) {
        for (v, &p) in self.voices.iter_mut().zip(&PHASES) {
            v.reset(p);
        }
        for (v, &p) in self.octave.iter_mut().zip(&OCT_PHASES) {
            v.reset(p);
        }
        self.sub = 0.0;
    }

    fn trigger(&mut self, c: &Ctx, o: &OscCtx, _rng: &mut Noise) {
        self.reset();
        self.control(c, o);
    }

    fn control(&mut self, c: &Ctx, o: &OscCtx) {
        let cents = map::unison_detune_cents(c.x) + map::bloom_cents(c.w) * o.env * o.env;
        for (i, &s) in SPREAD.iter().enumerate() {
            self.incs[i] = inc(o.hz * (s * cents / 1200.0).exp2(), c.fs);
        }
        for (i, &s) in OCT_SPREAD.iter().enumerate() {
            self.oct_incs[i] = inc(2.0 * o.hz * (s * cents / 1200.0).exp2(), c.fs);
        }
        // Detuned, uncorrelated voices add up in power: normalize for that.
        let side = c.y;
        let oct = c.u;
        let norm = 1.0 / (1.0 + 4.0 * side * side + 2.0 * oct * oct).sqrt();
        for (i, g) in self.gains.iter_mut().enumerate() {
            *g = norm * if i == VOICES / 2 { 1.0 } else { side };
        }
        self.oct_gain = norm * oct;
        self.shape = Morph::saw_square(c.z);
        self.sub_inc = inc(0.5 * o.hz, c.fs);
        self.sub_gain = 0.9 * c.v;
    }

    #[inline]
    fn tick(&mut self, fm: f32) -> f32 {
        let mut y = 0.0;
        for i in 0..VOICES {
            if self.gains[i] != 0.0 {
                y += self.gains[i] * self.voices[i].tick(fm_inc(self.incs[i], fm), &self.shape);
            }
        }
        if self.oct_gain != 0.0 {
            for i in 0..2 {
                y += self.oct_gain * self.octave[i].tick(fm_inc(self.oct_incs[i], fm), &self.shape);
            }
        }
        if self.sub_gain != 0.0 {
            // One sample behind, like the polyBLEP voices.
            y += self.sub_gain * sin_turns(self.sub);
        }
        self.sub += fm_inc(self.sub_inc, fm);
        if self.sub >= 1.0 {
            self.sub -= 1.0;
        }
        y
    }
}
