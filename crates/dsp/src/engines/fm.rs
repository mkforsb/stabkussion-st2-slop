//! FM: two-operator phase modulation, a sine modulating a sine.
//!
//! The modulation index has its own envelope, a faster decay than the
//! amplitude's, so a hit starts bright and metallic and relaxes into a pure
//! sine: the classic FM bass pluck and the "DX kick". Velocity scales the
//! index.
//!
//! * RATIO: modulator/carrier frequency ratio, 0.25 … 8, continuous. Whole
//!   numbers are harmonic, everything else clangs.
//! * INDEX: peak modulation index, 0 … 12 radians.
//! * MOD DEC: index decay as a multiple of the oscillator's decay (×0.03 …
//!   ×1, where the brightness lasts as long as the sound).
//! * M.SUS: the level the index decays to instead of nothing, so the hit
//!   keeps some of its bark.
//! * BEAT: offsets the modulator by up to 12 Hz. A fixed offset makes the
//!   sidebands beat against the harmonics, slowly on low notes and faster on
//!   high ones: the DX "detuned operator" shimmer, and on a chord every tone
//!   beats at the same rate.
//! * C.WAVE: the carrier's waveform, sine → half sine (OPL-style, DC-free)
//!   → soft square (a saturated sine).

use super::{Ctx, Osc, OscCtx, fm_inc, inc};
use crate::params::map;
use crate::util::{Noise, fast_tanh, sin_turns, t60_gain};

/// Drive of the soft-square carrier.
const SQUARE_DRIVE: f32 = 2.5;

#[derive(Clone, Debug, Default)]
pub struct Fm {
    carrier: f32,
    modulator: f32,
    c_inc: f32,
    m_inc: f32,
    /// Peak index in turns.
    index: f32,
    index_env: f32,
    index_mul: f32,
    index_sus: f32,
    /// Carrier waveform segment and crossfade.
    wave: usize,
    wave_mix: f32,
}

/// Carrier waveform `wave` (0 sine, 1 half sine, 2 soft square) at `p` turns.
#[inline]
fn carrier(wave: usize, p: f32) -> f32 {
    let s = sin_turns(p);
    match wave {
        0 => s,
        // max(sin, 0) has a mean of 1/π: remove it, and scale to ±1-ish.
        1 => 2.0 * (s.max(0.0) - core::f32::consts::FRAC_1_PI),
        _ => fast_tanh(SQUARE_DRIVE * s) / fast_tanh(SQUARE_DRIVE),
    }
}

impl Osc for Fm {
    fn reset(&mut self) {
        self.carrier = 0.0;
        self.modulator = 0.0;
        self.index_env = 0.0;
    }

    fn trigger(&mut self, c: &Ctx, o: &OscCtx, _rng: &mut Noise) {
        self.reset();
        self.index_env = 1.0;
        self.control(c, o);
    }

    fn control(&mut self, c: &Ctx, o: &OscCtx) {
        self.c_inc = inc(o.hz, c.fs);
        self.m_inc = inc(o.hz * map::fm_ratio(c.x) + map::beat_hz(c.v), c.fs);
        self.index = map::fm_index(c.y) * (0.4 + 0.6 * c.velocity) / core::f32::consts::TAU;
        self.index_mul = t60_gain(o.decay_s * map::fm_index_decay(c.z), c.fs);
        self.index_sus = c.u;
        let (i, f) = map::segment(c.w, 3);
        (self.wave, self.wave_mix) = if c.w <= 0.0 { (0, 0.0) } else { (i, f) };
    }

    #[inline]
    fn tick(&mut self, fm: f32) -> f32 {
        let m = self.index * self.index_env * sin_turns(self.modulator);
        let s = self.index_sus;
        self.index_env = s + (self.index_env - s) * self.index_mul;
        let p = self.carrier + m;
        let y = if self.wave == 0 && self.wave_mix == 0.0 {
            sin_turns(p)
        } else {
            let a = carrier(self.wave, p);
            a + (carrier(self.wave + 1, p) - a) * self.wave_mix
        };
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn carrier_waves_are_bounded_and_dc_free() {
        for wave in 0..3 {
            let n = 10_000;
            let mean = (0..n)
                .map(|i| carrier(wave, i as f32 / n as f32))
                .sum::<f32>()
                / n as f32;
            assert!(mean.abs() < 1e-3, "wave {wave}: dc {mean}");
            assert!((0..n).all(|i| carrier(wave, i as f32 / n as f32).abs() <= 1.4));
        }
    }

    #[test]
    fn index_sustain_keeps_the_bark() {
        let mut c = Ctx::new(48_000.0);
        (c.x, c.y, c.z) = (0.6, 0.7, 0.0);
        let o = OscCtx {
            hz: 100.0,
            env: 1.0,
            decay_s: 0.5,
        };
        let env_after = |u: f32| {
            let mut c = c;
            c.u = u;
            let mut f = Fm::default();
            f.trigger(&c, &o, &mut Noise::new(1));
            for _ in 0..48_000 {
                f.tick(1.0);
            }
            f.index_env
        };
        assert!(env_after(0.0) < 1e-3);
        assert!((env_after(0.4) - 0.4).abs() < 1e-3);
    }
}
