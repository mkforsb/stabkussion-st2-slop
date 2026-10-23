//! SYNC: a virtual-analog oscillator hard-synced to a hidden master.
//!
//! The master runs at the oscillator's pitch and only resets the slave, so the
//! pitch you hear is always the master's; the slave's frequency, a multiple of
//! it, moves a formant-like peak through the spectrum. All jumps (the slave's
//! own and the sync resets) are polyBLEP corrected.
//!
//! * WAVE: the slave's shape, sine → triangle → saw → square.
//! * SYNC: static slave ratio, ×1 (no sync) … ×8.
//! * SWEEP: how many octaves the ratio rises at the envelope peak: the classic
//!   sync "zap" that settles as the hit decays. Velocity scales it.
//!
//! Hard sync moves energy away from the fundamental as the ratio rises, which
//! thins a bass out. So the master is audible too: a sine at the pitch is
//! blended in as the ratio climbs (up to half the mix), keeping the note
//! anchored under the zap.
//!
//! * Z.DEC: gives the zap its own, faster decay (×1 … ×0.02 of the
//!   oscillator's), instead of following the amplitude envelope: the
//!   classic short sync "bwow" at the front of a long stab.
//! * FM: the master's sine frequency-modulates the slave (up to ±90%), the
//!   sync-plus-FM tone of a complex oscillator. It is periodic in the
//!   master's cycle, so it stays harmonic.
//! * WINDOW: fades each master cycle of the slave in and out with a Hann
//!   window, from hard sync to windowed sync: a vocal, formant-like peak at
//!   the slave frequency with no edge at the reset at all.

use super::{Ctx, MAX_INC, Osc, OscCtx, block_decay, fm_inc, inc};
use crate::params::map;
use crate::util::{BlepOsc, Morph, Noise, cos_turns, sin_turns};

#[derive(Clone, Debug, Default)]
pub struct Sync {
    master: f32,
    master_inc: f32,
    slave: BlepOsc,
    slave_inc: f32,
    shape: Morph,
    /// Share of the master's sine in the output.
    anchor: f32,
    /// Master phase one sample back, matching the slave's delayed output.
    last_master: f32,
    /// The zap's own envelope (Z.DEC), 1 → 0.
    zap: f32,
    fm_depth: f32,
    window: f32,
    window_gain: f32,
}

impl Osc for Sync {
    fn reset(&mut self) {
        self.master = 0.0;
        self.last_master = 0.0;
        self.slave.reset(0.0);
    }

    fn trigger(&mut self, c: &Ctx, o: &OscCtx, _rng: &mut Noise) {
        self.reset();
        self.zap = 1.0;
        self.control(c, o);
    }

    fn control(&mut self, c: &Ctx, o: &OscCtx) {
        self.master_inc = inc(o.hz, c.fs);
        let env = if c.u > 0.0 {
            self.zap *= block_decay(o.decay_s * map::zap_decay_ratio(c.u), c.fs);
            self.zap
        } else {
            o.env
        };
        let sweep = map::sync_sweep_octaves(c.z) * env * (0.5 + 0.5 * c.velocity);
        let ratio = map::sync_ratio(c.y) * sweep.exp2();
        self.slave_inc = inc(o.hz * ratio, c.fs);
        self.shape = Morph::sweep(c.x);
        self.anchor = 0.5 * (1.0 - 1.0 / ratio);
        self.fm_depth = map::sync_fm_depth(c.v);
        self.window = c.w;
        // The Hann window's RMS is √(3/8): make up most of the level.
        self.window_gain = 1.0 / (1.0 - 0.35 * c.w);
    }

    #[inline]
    fn tick(&mut self, fm: f32) -> f32 {
        let fundamental = sin_turns(self.last_master);
        let window = 0.5 - 0.5 * cos_turns(self.last_master);
        self.last_master = self.master;
        let master_inc = fm_inc(self.master_inc, fm);
        let mut slave_inc = fm_inc(self.slave_inc, fm);
        if self.fm_depth != 0.0 {
            slave_inc = (slave_inc * (1.0 + self.fm_depth * fundamental)).min(MAX_INC);
        }
        self.master += master_inc;
        let mut slave = if self.master >= 1.0 {
            self.master -= 1.0;
            let t = self.master / master_inc.max(1e-9);
            self.slave.tick_synced(slave_inc, &self.shape, t.min(1.0))
        } else {
            self.slave.tick(slave_inc, &self.shape)
        };
        if self.window != 0.0 {
            slave *= (1.0 - self.window + self.window * window) * self.window_gain;
        }
        slave + (fundamental - slave) * self.anchor
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn render(x: f32, y: f32, z: f32, hz: f32, n: usize) -> Vec<f32> {
        render_uvw(x, y, z, 0.0, 0.0, 0.0, hz, n)
    }

    #[allow(clippy::too_many_arguments)]
    fn render_uvw(x: f32, y: f32, z: f32, u: f32, v: f32, w: f32, hz: f32, n: usize) -> Vec<f32> {
        let mut c = Ctx::new(96_000.0);
        (c.x, c.y, c.z, c.u, c.v, c.w) = (x, y, z, u, v, w);
        let o = OscCtx {
            hz,
            env: 1.0,
            decay_s: 1.0,
        };
        let mut s = Sync::default();
        s.trigger(&c, &o, &mut Noise::new(1));
        (0..n).map(|_| s.tick(1.0)).collect()
    }

    #[test]
    fn unsynced_sine_is_a_clean_sine_at_pitch() {
        let out = render(0.0, 0.0, 0.0, 100.0, 96_000);
        let crossings = out.windows(2).filter(|w| w[0] <= 0.0 && w[1] > 0.0).count();
        assert!((crossings as i32 - 100).abs() <= 1, "{crossings}");
        let peak = out.iter().fold(0.0f32, |a, &b| a.max(b.abs()));
        assert!((peak - 1.0).abs() < 0.01);
    }

    #[test]
    fn synced_output_repeats_at_the_master_pitch() {
        // Autocorrelation at the master period beats the slave period.
        let out = render(0.66, 0.6, 0.0, 110.0, 19_200);
        let period = (96_000.0f32 / 110.0).round() as usize;
        let corr = |lag: usize| {
            (2_000..10_000).map(|i| out[i] * out[i + lag]).sum::<f32>()
                / (2_000..10_000).map(|i| out[i] * out[i]).sum::<f32>()
        };
        assert!(corr(period) > 0.95, "{}", corr(period));
        assert!(out.iter().all(|s| s.is_finite() && s.abs() < 1.6));
    }

    #[test]
    fn windowed_sync_has_no_edges_and_keeps_the_pitch() {
        // A sine slave at ×3.5: the hard sync resets jump, windowed ones
        // don't (the slave's own edges, on a saw, would remain).
        let jump = |w: f32| {
            let out = render_uvw(0.0, 0.6, 0.0, 0.0, 0.0, w, 110.0, 9_600);
            // Largest second difference: kinks and (smoothed) steps.
            out.windows(3)
                .map(|p| (p[2] - 2.0 * p[1] + p[0]).abs())
                .fold(0.0f32, f32::max)
        };
        assert!(jump(1.0) < 0.3 * jump(0.0), "{} {}", jump(1.0), jump(0.0));
        let out = render_uvw(0.66, 0.6, 0.0, 0.0, 0.7, 1.0, 110.0, 19_200);
        let period = (96_000.0f32 / 110.0).round() as usize;
        let corr = (2_000..10_000)
            .map(|i| out[i] * out[i + period])
            .sum::<f32>()
            / (2_000..10_000).map(|i| out[i] * out[i]).sum::<f32>();
        assert!(corr > 0.95, "{corr}");
    }
}
