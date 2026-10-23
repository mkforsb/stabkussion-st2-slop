//! The chaos modulator: a slow Lorenz attractor used as an LFO that never
//! repeats. It drifts around one lobe, hesitates, then jumps to the other.

use crate::params::map;

const SIGMA: f32 = 10.0;
const RHO: f32 = 28.0;
const BETA: f32 = 8.0 / 3.0;
/// Lorenz time units per "cycle" (one loop around a lobe at ρ = 28).
const UNITS_PER_CYCLE: f32 = 0.62;
const MAX_STEP: f32 = 0.004;

#[derive(Clone, Debug)]
pub struct ChaosLfo {
    x: f32,
    y: f32,
    z: f32,
}

impl ChaosLfo {
    /// Different seeds start at different points on the attractor.
    pub fn new(seed: u32) -> Self {
        let mut c = Self {
            x: 1.0 + seed as f32 * 0.37,
            y: 1.0,
            z: 20.0,
        };
        // Settle onto the attractor.
        for _ in 0..4_000 {
            c.step(0.004);
        }
        c
    }

    fn step(&mut self, h: f32) {
        let d = |x: f32, y: f32, z: f32| (SIGMA * (y - x), x * (RHO - z) - y, x * y - BETA * z);
        let (dx, dy, dz) = d(self.x, self.y, self.z);
        let (dx, dy, dz) = d(
            self.x + 0.5 * h * dx,
            self.y + 0.5 * h * dy,
            self.z + 0.5 * h * dz,
        );
        self.x += h * dx;
        self.y += h * dy;
        self.z += h * dz;
    }

    /// Advance by `seconds` at the RATE knob position.
    pub fn advance(&mut self, rate: f32, seconds: f32) {
        let mut units = map::chaos_hz(rate) * UNITS_PER_CYCLE * seconds;
        while units > 0.0 {
            let h = units.min(MAX_STEP);
            self.step(h);
            units -= h;
        }
        if !(self.x.is_finite() && self.y.is_finite() && self.z.is_finite()) {
            *self = Self::new(0);
        }
    }

    /// Output in about `-1..=1`.
    pub fn value(&self) -> f32 {
        (self.x / 18.0).clamp(-1.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wanders_both_lobes_within_range() {
        let mut c = ChaosLfo::new(1);
        let (mut lo, mut hi) = (0.0f32, 0.0f32);
        for _ in 0..20_000 {
            c.advance(0.5, 0.001);
            lo = lo.min(c.value());
            hi = hi.max(c.value());
        }
        assert!(lo < -0.5 && hi > 0.5, "{lo} {hi}");
    }
}
