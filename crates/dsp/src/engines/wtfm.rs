//! WT-FM: a wavetable carrier phase-modulated by a sine.
//!
//! The wavetable gives the body its colour, the FM gives it a bark. As in FM,
//! the index decays faster than the amplitude (a third of the oscillator's
//! decay), so the hit snarls and then settles into the plain table.
//!
//! * POS: carrier table position (as on TABLE).
//! * INDEX: peak modulation index, 0 … 6 radians; velocity scales it.
//! * RATIO: modulator ratio, 0.25 … 8, continuous.
//! * SCAN: the table position follows the index envelope: the hit starts up
//!   to the whole table further along and sweeps back to POS with the bark.
//! * SPREAD: a second carrier reading the same table, detuned up to 30
//!   cents and faded in (both share the modulator).
//! * BEND: bends the carrier's phase, `p^(1…4)`, like a wavetable synth's
//!   phase warp: the cycle is squeezed towards its end, which brightens and
//!   narrows it. The mip level follows the steepest part of the bent phase,
//!   so the bent wave stays band-limited.

use super::tables::{FRAMES, Mip, Tables, tables};
use super::{Ctx, Osc, OscCtx, fm_inc, inc};
use crate::params::map;
use crate::util::{Noise, sin_turns, t60_gain, wrap};

const INDEX_DECAY_RATIO: f32 = 0.33;

#[derive(Clone, Debug, Default)]
pub struct WtFm {
    carrier: f32,
    modulator: f32,
    c_inc: f32,
    m_inc: f32,
    pos: f32,
    mip: Mip,
    /// Peak index in turns.
    index: f32,
    index_env: f32,
    index_mul: f32,
    /// SPREAD: the second carrier's phase, increment and level.
    carrier2: f32,
    c2_inc: f32,
    spread: f32,
    /// BEND exponent (1 = off).
    bend: f32,
}

impl WtFm {
    #[inline]
    fn bent(&self, p: f32) -> f32 {
        if self.bend > 1.0 {
            p.powf(self.bend)
        } else {
            p
        }
    }
}

impl Osc for WtFm {
    fn reset(&mut self) {
        self.carrier = 0.0;
        self.carrier2 = 0.0;
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
        let ratio = map::fm_ratio(c.z);
        self.m_inc = inc(o.hz * ratio, c.fs);
        let pos = if c.u > 0.0 {
            (c.x + c.u * self.index_env).min(1.0)
        } else {
            c.x
        };
        self.pos = pos * (FRAMES - 1) as f32;
        let index = map::wtfm_index(c.y) * (0.4 + 0.6 * c.velocity);
        self.index = index / core::f32::consts::TAU;
        self.index_mul = t60_gain(o.decay_s * INDEX_DECAY_RATIO, c.fs);
        let cents = map::spread_cents(c.v);
        self.c2_inc = inc(o.hz * (cents / 1200.0).exp2(), c.fs);
        self.spread = (3.0 * c.v).min(1.0);
        self.bend = map::bend_power(c.w);
        // Shifting the phase of a whole table shifts harmonic k's phase k
        // times as far: its peak frequency is k·(carrier + index·modulator),
        // plus one modulator frequency of Carson sideband. Pick the mip for
        // that. As the index decays the table opens up again, crossfading
        // between levels (see `Tables::mip_blend`) so nothing clicks. The
        // bend speeds the phase up by up to its exponent, the spread carrier
        // runs a little sharp.
        let beta = index * self.index_env;
        let fm = o.hz * ratio;
        let (swing, sidebands) = if beta > 0.01 {
            (beta * fm, fm)
        } else {
            (0.0, 0.0)
        };
        let top = if self.spread > 0.0 {
            (cents / 1200.0).exp2()
        } else {
            1.0
        };
        self.mip = Tables::mip_blend(o.hz * self.bend * top + swing, sidebands, c.fs);
    }

    #[inline]
    fn tick(&mut self, fm: f32) -> f32 {
        let m = self.index * self.index_env * sin_turns(self.modulator);
        self.index_env *= self.index_mul;
        let t = tables();
        let mut y = t.read_mip(self.pos, self.mip, wrap(self.bent(self.carrier) + m));
        if self.spread > 0.0 {
            let b = t.read_mip(self.pos, self.mip, wrap(self.bent(self.carrier2) + m));
            y = (y + self.spread * b) / (1.0 + 0.6 * self.spread);
        }
        self.carrier += fm_inc(self.c_inc, fm);
        if self.carrier >= 1.0 {
            self.carrier -= 1.0;
        }
        self.carrier2 += fm_inc(self.c2_inc, fm);
        if self.carrier2 >= 1.0 {
            self.carrier2 -= 1.0;
        }
        self.modulator += fm_inc(self.m_inc, fm);
        if self.modulator >= 1.0 {
            self.modulator -= 1.0;
        }
        y
    }
}
