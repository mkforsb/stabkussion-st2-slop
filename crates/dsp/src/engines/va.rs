//! VA: a virtual-analog saw/pulse oscillator with pulse-width modulation.
//!
//! The saw and the pulse share one phase, so crossfading between them keeps
//! the edges lined up. The pulse's width moves smoothly (once per sample) and
//! its edge is found wherever it crosses the phase, so every jump, including
//! a moving one, is polyBLEP corrected. The pulse is DC-free at any width.
//!
//! Each oscillator gets its own slow PWM LFO, at a rate and phase drawn per
//! hit, so a chord of pulses drifts against itself like an analog poly.
//!
//! * WAVE: saw → pulse.
//! * WIDTH: pulse width, 50% (square) … 5%.
//! * PWM: depth of the slow width modulation.
//! * SUB: a polyBLEP square one octave down, Juno-style, locked to the main
//!   phase.
//! * RATE: speeds the PWM LFOs up, ×1 … ×64 (up to the edge of audio rate,
//!   where PWM turns into a growl).
//! * DRIFT: analog drift. Each oscillator is detuned by a per-hit random
//!   amount and wanders slowly around it, up to ±25 cents.

use super::{Ctx, Osc, OscCtx, fm_inc, inc};
use crate::params::map;
use crate::util::{BlepOsc, Morph, Noise, sin_turns};
use crate::voice::CONTROL_BLOCK;

/// PWM LFO rates are drawn from this range (Hz).
const PWM_HZ: (f32, f32) = (0.3, 1.4);
/// Drift wander rates (Hz).
const DRIFT_HZ: (f32, f32) = (0.15, 0.6);
const SUB_WAVE: Morph = Morph {
    sin: 0.0,
    tri: 0.0,
    saw: 0.0,
    sq: 1.0,
};

#[derive(Clone, Debug, Default)]
pub struct Va {
    phase: f32,
    /// The polyBLEP output is one sample late (see [`BlepOsc`]).
    next: f32,
    inc: f32,
    saw: f32,
    pulse: f32,
    /// Current pulse width, and its per-sample step towards the target.
    width: f32,
    dw: f32,
    lfo: f32,
    lfo_hz: f32,
    rate_mul: f32,
    sub: BlepOsc,
    sub_gain: f32,
    main_gain: f32,
    /// Per-hit drift offset (−1 … 1), and the slow wander around it.
    drift: f32,
    drift_lfo: f32,
    drift_hz: f32,
}

impl Va {
    /// The naive saw/pulse mix at phase `p` and width `w`. The saw rises
    /// through zero at phase 0 and jumps at ½ (like [`crate::util::Morph`]).
    #[inline]
    fn naive(&self, p: f32, w: f32) -> f32 {
        let saw = if p < 0.5 { 2.0 * p } else { 2.0 * p - 2.0 };
        let pulse = if p < w { 1.0 } else { -1.0 } - (2.0 * w - 1.0);
        self.saw * saw + self.pulse * pulse
    }
}

impl Osc for Va {
    fn reset(&mut self) {
        self.phase = 0.0;
        self.next = 0.0;
        self.sub.reset(0.0);
    }

    fn trigger(&mut self, c: &Ctx, o: &OscCtx, rng: &mut Noise) {
        self.reset();
        self.lfo = rng.uniform();
        self.lfo_hz = PWM_HZ.0 + (PWM_HZ.1 - PWM_HZ.0) * rng.uniform();
        // The drift is seeded from the PWM draw rather than drawing more
        // numbers from the voice's generator, so the other oscillators (and
        // the ST-1 patches) roll exactly what they always did.
        let mut own = Noise::new((self.lfo * 16_777_216.0) as u32);
        self.drift = own.sample();
        self.drift_lfo = own.uniform();
        self.drift_hz = DRIFT_HZ.0 + (DRIFT_HZ.1 - DRIFT_HZ.0) * own.uniform();
        self.width = f32::NAN;
        self.control(c, o);
    }

    fn control(&mut self, c: &Ctx, o: &OscCtx) {
        let block_s = CONTROL_BLOCK as f32 / c.fs;
        self.drift_lfo += self.drift_hz * block_s;
        if self.drift_lfo >= 1.0 {
            self.drift_lfo -= 1.0;
        }
        let cents = map::drift_cents(c.w) * (0.7 * self.drift + 0.3 * sin_turns(self.drift_lfo));
        self.inc = inc(o.hz * (cents / 1200.0).exp2(), c.fs);
        self.pulse = c.x;
        self.saw = 1.0 - c.x;
        self.sub_gain = map::sub_gain(c.u);
        self.main_gain = 1.0 - 0.3 * c.u;
        self.rate_mul = map::pwm_rate_mul(c.v);
        self.lfo += self.lfo_hz * self.rate_mul * block_s;
        self.lfo -= self.lfo.floor();
        let w = map::pulse_width(c.y) + map::pwm_depth(c.z) * sin_turns(self.lfo);
        // Keep both halves at least two samples long.
        let edge = (2.0 * self.inc).clamp(0.03, 0.45);
        let target = w.clamp(edge, 1.0 - edge);
        if self.width.is_nan() {
            self.width = target;
            self.dw = 0.0;
        } else {
            self.dw = (target - self.width) / CONTROL_BLOCK as f32;
        }
    }

    #[inline]
    fn tick(&mut self, fm: f32) -> f32 {
        let inc = fm_inc(self.inc, fm);
        // The sub runs at half the rate from the same start, so it flips
        // on every other wrap of the main phase.
        let sub = if self.sub_gain != 0.0 {
            self.sub_gain * self.sub.tick(0.5 * inc, &SUB_WAVE)
        } else {
            self.sub.phase += 0.5 * inc;
            if self.sub.phase >= 1.0 {
                self.sub.phase -= 1.0;
            }
            0.0
        };
        let mut this = self.next;
        let mut next = 0.0;
        let w0 = self.width;
        let w1 = w0 + self.dw;
        self.width = w1;
        let before = self.phase;
        let mut p = before + inc;
        // The pulse's falling edge, wherever the (moving) width put it.
        let (d0, d1) = (before - w0, p - w1);
        if self.pulse != 0.0 && d0 < 0.0 && d1 >= 0.0 {
            BlepOsc::blep(&mut this, &mut next, -2.0 * self.pulse, d1 / (d1 - d0));
        }
        if before < 0.5 && p >= 0.5 && self.saw != 0.0 {
            BlepOsc::blep(&mut this, &mut next, -2.0 * self.saw, (p - 0.5) / inc);
        }
        if p >= 1.0 {
            p -= 1.0;
            if self.pulse != 0.0 {
                BlepOsc::blep(&mut this, &mut next, 2.0 * self.pulse, p / inc);
            }
        }
        self.phase = p;
        self.next = next + self.naive(p, w1);
        self.main_gain * this + sub
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pulse_and_sub_have_no_dc_at_any_width() {
        let fs = 48_000.0;
        for y in [0.0, 0.5, 1.0] {
            let mut c = Ctx::new(fs);
            (c.x, c.y, c.z, c.u) = (1.0, y, 0.0, 0.7);
            let o = OscCtx {
                hz: 100.0,
                env: 1.0,
                decay_s: 1.0,
            };
            let mut va = Va::default();
            va.trigger(&c, &o, &mut Noise::new(1));
            let n = 48_000;
            let mean = (0..n).map(|_| va.tick(1.0)).sum::<f32>() / n as f32;
            assert!(mean.abs() < 0.01, "width {y}: dc {mean}");
        }
    }
}
