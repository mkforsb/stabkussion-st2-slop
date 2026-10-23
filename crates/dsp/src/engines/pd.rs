//! PD: phase distortion in the manner of the Casio CZ.
//!
//! A sine is read through a bent phase. Bending the phase into a fast and a
//! slow half gives a saw, two holds a square. The resonant waves instead
//! window a sine at a higher, unsynced frequency, the CZ's trick for a swept
//! resonant peak without a filter. With DIST at zero every wave is a pure
//! sine. All the waves are continuous (only their slopes break), so they need
//! no anti-aliasing beyond the oversampling.
//!
//! * DIST: distortion depth; velocity adds to it.
//! * WAVE: saw → square → reso (saw window) → pulse (triangle window).
//! * D.ENV: how much of DIST follows the envelope (0: static, 1: the hit opens
//!   up and closes back to a sine).
//! * D.DEC: gives that distortion envelope (the CZ's DCW) its own, faster
//!   decay, ×1 … ×0.02 of the oscillator's, instead of following the
//!   amplitude: a bright "dwow" at the front of a long, mellow note.
//! * LINE 2: a second line, as in the CZ's 1+2' mode, detuned up to 25
//!   cents and faded in: a built-in chorus of two phase-distorted waves.
//! * NOISE: lowpassed noise added to the phase, the CZ-1000's noise
//!   modulation: breath at low settings, a raspy, bit-crushed growl at the
//!   top.

use super::{Ctx, Osc, OscCtx, block_decay, fm_inc, inc};
use crate::params::map;
use crate::util::{Noise, sin_turns, wrap};

/// NOISE: peak phase deviation (turns) at the top of the knob.
const NOISE_TURNS: f32 = 0.12;
/// NOISE: one-pole smoothing of the noise (per sample).
const NOISE_SMOOTH: f32 = 0.25;

/// Highest resonance ratio of the resonant waves.
const MAX_RESO: f32 = 16.0;

#[derive(Clone, Debug, Default)]
pub struct Pd {
    phase: f32,
    inc: f32,
    /// Knee of the saw/square warps: ¼ (or ½) = no distortion … tiny.
    knee: f32,
    reso: f32,
    /// Crossfade from the plain sine into the resonant waves at low DIST.
    reso_amt: f32,
    /// Shape pair and crossfade.
    wave: usize,
    wave_mix: f32,
    /// D.DEC's envelope, 1 → 0.
    denv: f32,
    /// LINE 2: phase, increment and level.
    phase2: f32,
    inc2: f32,
    line2: f32,
    /// NOISE: depth (turns), the generator and its lowpass.
    noise: f32,
    rng: Option<Noise>,
    noise_lp: f32,
}

#[inline]
fn saw(p: f32, d: f32) -> f32 {
    // [0, d] → [0, ½], [d, 1] → [½, 1].
    let q = if p < d {
        0.5 * p / d
    } else {
        0.5 + 0.5 * (p - d) / (1.0 - d)
    };
    sin_turns(q)
}

#[inline]
fn square(p: f32, d: f32) -> f32 {
    // Rise to the top in d, hold, fall to the bottom over 2d around ½, hold,
    // rise back to zero in d. d = ¼ is a plain sine.
    let q = if p < d {
        0.25 * p / d
    } else if p < 0.5 - d {
        0.25
    } else if p < 0.5 + d {
        0.25 + 0.5 * (p - (0.5 - d)) / (2.0 * d)
    } else if p < 1.0 - d {
        0.75
    } else {
        0.75 + 0.25 * (p - (1.0 - d)) / d
    };
    sin_turns(q)
}

impl Pd {
    #[inline]
    fn wave_at(&self, p: f32) -> f32 {
        let a = self.shape(self.wave, p);
        if self.wave_mix > 0.0 {
            let b = self.shape(self.wave + 1, p);
            a + (b - a) * self.wave_mix
        } else {
            a
        }
    }

    #[inline]
    fn shape(&self, wave: usize, p: f32) -> f32 {
        match wave {
            0 => saw(p, 2.0 * self.knee),
            1 => square(p, self.knee),
            _ => {
                // Windowed resonance, faded in from the plain sine.
                let window = if wave == 2 {
                    1.0 - p
                } else {
                    1.0 - (2.0 * p - 1.0).abs()
                };
                let r = window * sin_turns(self.reso * p);
                let s = sin_turns(p);
                s + (r - s) * self.reso_amt
            }
        }
    }
}

impl Osc for Pd {
    fn reset(&mut self) {
        self.phase = 0.0;
        self.phase2 = 0.0;
        self.noise_lp = 0.0;
    }

    fn trigger(&mut self, c: &Ctx, o: &OscCtx, _rng: &mut Noise) {
        self.reset();
        self.denv = 1.0;
        self.control(c, o);
    }

    fn control(&mut self, c: &Ctx, o: &OscCtx) {
        self.inc = inc(o.hz, c.fs);
        self.inc2 = inc(o.hz * (map::drift_cents(c.v) / 1200.0).exp2(), c.fs);
        self.line2 = (3.0 * c.v).min(1.0);
        self.noise = NOISE_TURNS * c.w * c.w;
        if self.noise > 0.0 && self.rng.is_none() {
            // Its own generator (seeded from the pitch): drawing from the
            // voice's would change what everything after it rolls.
            self.rng = Some(Noise::new(o.hz.to_bits()));
        }
        let shape_env = if c.u > 0.0 {
            self.denv *= block_decay(o.decay_s * map::zap_decay_ratio(c.u), c.fs);
            self.denv
        } else {
            o.env
        };
        let env = 1.0 - c.z + c.z * shape_env;
        let dist = (c.x * env * (0.7 + 0.3 * c.velocity)).clamp(0.0, 1.0);
        // The sharper the knee, the more harmonics: keep it at least a couple
        // of samples wide.
        let min_knee = (2.0 * self.inc).min(0.2);
        self.knee = (0.25 * (1.0 - 0.97 * dist)).max(min_knee);
        self.reso = 1.0 + (MAX_RESO - 1.0) * dist * dist;
        // Don't let the resonance climb past where it would alias.
        let limit = 0.3 / self.inc.max(1e-6);
        self.reso = self.reso.min(limit.max(1.0));
        self.reso_amt = (dist * 4.0).min(1.0);
        let (i, f) = map::segment(c.y, 4);
        self.wave = i;
        self.wave_mix = f;
    }

    #[inline]
    fn tick(&mut self, fm: f32) -> f32 {
        let mut offset = 0.0;
        if self.noise > 0.0
            && let Some(rng) = self.rng.as_mut()
        {
            self.noise_lp += (rng.sample() - self.noise_lp) * NOISE_SMOOTH;
            offset = self.noise * self.noise_lp;
        }
        let read = |p: f32| if offset != 0.0 { wrap(p + offset) } else { p };
        let mut y = self.wave_at(read(self.phase));
        if self.line2 > 0.0 {
            let b = self.wave_at(read(self.phase2));
            // Two detuned lines add in power once they drift apart.
            y = (y + self.line2 * b) / (1.0 + 0.6 * self.line2);
        }
        self.phase += fm_inc(self.inc, fm);
        if self.phase >= 1.0 {
            self.phase -= 1.0;
        }
        self.phase2 += fm_inc(self.inc2, fm);
        if self.phase2 >= 1.0 {
            self.phase2 -= 1.0;
        }
        y
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn waves_are_continuous_and_bounded() {
        let mut pd = Pd {
            inc: 0.001,
            ..Default::default()
        };
        for knee in [0.25f32, 0.1, 0.01] {
            pd.knee = knee;
            pd.reso = 9.3;
            pd.reso_amt = 1.0;
            for wave in 0..4 {
                let mut last = pd.shape(wave, 0.0);
                assert!(last.abs() < 1e-4, "wave {wave} starts at {last}");
                for n in 1..=2_000 {
                    let y = pd.shape(wave, (n as f32 / 2_000.0) % 1.0);
                    assert!(y.abs() <= 1.0 + 1e-4);
                    assert!(
                        (y - last).abs() < 0.2,
                        "wave {wave} knee {knee} jumps at {n}"
                    );
                    last = y;
                }
            }
        }
    }

    #[test]
    fn no_distortion_is_a_sine() {
        let mut pd = Pd::default();
        let mut c = Ctx::new(48_000.0);
        c.x = 0.0;
        for y in [0.0, 0.33, 0.67, 1.0] {
            c.y = y;
            pd.control(&c, &OscCtx::default());
            for n in 0..100 {
                let p = n as f32 / 100.0;
                assert!(
                    (pd.shape(pd.wave, p) - sin_turns(p)).abs() < 1e-3,
                    "wave {y}"
                );
            }
        }
    }
}
