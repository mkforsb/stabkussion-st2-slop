//! RING: a virtual-analog wave ring-modulated by a second wave.
//!
//! Multiplying by a sine at `r` times the pitch moves every harmonic `k` of
//! the wave to `k ± r`: unless `r` is a whole number (or a half) the result
//! is inharmonic, the bell-like, detuned clang of a ring-modulated stab. The
//! wave is the polyBLEP morph SYNC uses, so its edges stay clean; the product
//! only reaches `r` pitches higher, well within the oversampled band.
//!
//! * WAVE: sine → triangle → saw → square.
//! * RATIO: ring modulator ratio, 0.25 … 8, continuous.
//! * RING: amount, from the plain wave (0) to full ring modulation (1).
//! * R.WAVE: the ring modulator's own wave, sine → triangle → square
//!   (polyBLEP, like the carrier wave). A square ring modulator is the harsh,
//!   hollow diode-ring sound.
//! * R.ENV: the ratio starts up to three octaves higher at the envelope peak
//!   and falls back as the hit decays (harder hits further).
//! * FIXED: moves the ring modulator from tracking the pitch (RATIO × pitch)
//!   to a fixed frequency (RATIO × 110 Hz), as on a hardware ring modulator
//!   with its own oscillator. All four oscillators then share it, and a
//!   chord is shifted into one inharmonic cluster.

use super::{Ctx, Osc, OscCtx, fm_inc, inc};
use crate::params::map;
use crate::util::{BlepOsc, Morph, Noise};

/// The ring modulator starts at the top of its wave (a cosine), so the hit
/// starts at full level rather than in a null of the modulation.
const RING_START: f32 = 0.25;

#[derive(Clone, Debug, Default)]
pub struct Ring {
    wave: BlepOsc,
    w_inc: f32,
    shape: Morph,
    ring: BlepOsc,
    /// The ring modulator one more sample back: the ST-1 ran it a sample
    /// behind the wave, and its (fitted) presets rely on that.
    r_prev: f32,
    r_inc: f32,
    r_shape: Morph,
    amount: f32,
}

/// R.WAVE: sine → triangle → square.
fn ring_shape(v: f32) -> Morph {
    let (i, f) = map::segment(v, 3);
    let mut w = [0.0f32; 3];
    w[i] = 1.0 - f;
    w[i + 1] = f;
    Morph {
        sin: w[0],
        tri: w[1],
        saw: 0.0,
        sq: w[2],
    }
}

impl Osc for Ring {
    fn reset(&mut self) {
        self.wave.reset(0.0);
        self.ring.reset_to(RING_START, &self.r_shape);
        self.r_prev = self.r_shape.naive(RING_START);
    }

    fn trigger(&mut self, c: &Ctx, o: &OscCtx, _rng: &mut Noise) {
        self.r_shape = ring_shape(c.u);
        self.reset();
        self.control(c, o);
    }

    fn control(&mut self, c: &Ctx, o: &OscCtx) {
        self.w_inc = inc(o.hz, c.fs);
        self.shape = Morph::sweep(c.x);
        let ratio = map::fm_ratio(c.y)
            * (map::ratio_sweep_octaves(c.v) * o.env * (0.5 + 0.5 * c.velocity)).exp2();
        let hz = if c.w > 0.0 {
            // Log-interpolate between tracking and fixed.
            o.hz.powf(1.0 - c.w) * map::RING_FIXED_HZ.powf(c.w)
        } else {
            o.hz
        };
        self.r_inc = inc(hz * ratio, c.fs);
        self.r_shape = ring_shape(c.u);
        self.amount = c.z;
    }

    #[inline]
    fn tick(&mut self, fm: f32) -> f32 {
        let w = self.wave.tick(fm_inc(self.w_inc, fm), &self.shape);
        let ring = self.r_prev;
        self.r_prev = self.ring.tick(fm_inc(self.r_inc, fm), &self.r_shape);
        w * (1.0 - self.amount + self.amount * ring)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ring_modulator_starts_at_full_level() {
        for u in [0.0, 0.5, 1.0] {
            let mut r = Ring::default();
            let mut c = Ctx::new(48_000.0);
            (c.x, c.y, c.z, c.u) = (0.0, 0.4, 1.0, u);
            r.trigger(&c, &OscCtx::default(), &mut Noise::new(1));
            let y = r.ring.tick(r.r_inc, &r.r_shape);
            assert!((y - 1.0).abs() < 1e-5, "{u}: {y}");
        }
    }
}
