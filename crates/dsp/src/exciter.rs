//! The transient layer on top of the oscillators: a click and a noise burst.
//!
//! * The click is one cycle of a raised-cosine-windowed sine, the beater
//!   "tick" of a kick drum. Harder hits make it shorter and brighter: 0.6 ms
//!   (≈ 1.7 kHz) for a soft hit, 0.2 ms (≈ 5 kHz) at full velocity.
//! * The noise burst is lowpassed white noise with an exponential decay;
//!   velocity opens the lowpass.

use crate::params::map;
use crate::util::{Noise, cos_turns, one_pole_coeff, sin_turns};

#[derive(Clone, Debug)]
pub struct Transient {
    fs: f32,
    t: u32,
    click_len: u32,
    click_step: f32,
    click_gain: f32,
    noise_env: f32,
    noise_mul: f32,
    noise_lp: f32,
    noise_k: f32,
    noise_gain: f32,
}

impl Transient {
    pub fn new(fs: f32) -> Self {
        Self {
            fs,
            t: 0,
            click_len: 0,
            click_step: 0.0,
            click_gain: 0.0,
            noise_env: 0.0,
            noise_mul: 0.0,
            noise_lp: 0.0,
            noise_k: 1.0,
            noise_gain: 0.0,
        }
    }

    /// Start a hit. `click` and `noise` are the knob positions.
    pub fn trigger(&mut self, velocity: f32, click: f32, noise: f32) {
        let fs = self.fs;
        let seconds = 0.000_6 - 0.000_4 * velocity;
        self.t = 0;
        self.click_len = ((seconds * fs) as u32).max(4);
        self.click_step = 1.0 / self.click_len as f32;
        self.click_gain = map::click_gain(click) * velocity;
        if noise > 0.0 {
            let tau = map::noise_seconds(noise);
            self.noise_env = 1.0;
            self.noise_mul = (-1.0 / (tau * fs)).exp();
            let cutoff = (2_000.0 * 8f32.powf(velocity)).min(0.45 * fs);
            self.noise_k = one_pole_coeff(1.0 / (core::f32::consts::TAU * cutoff), fs);
            self.noise_gain = 0.8 * noise.sqrt() * velocity;
        } else {
            self.noise_env = 0.0;
        }
    }

    #[inline]
    pub fn tick(&mut self, rng: &mut Noise) -> f32 {
        let mut y = 0.0;
        if self.t < self.click_len {
            let ph = self.t as f32 * self.click_step;
            let window = 0.5 - 0.5 * cos_turns(ph);
            y += self.click_gain * window * sin_turns(ph);
            self.t += 1;
        }
        if self.noise_env > 1e-4 {
            self.noise_lp += (rng.sample() - self.noise_lp) * self.noise_k;
            y += self.noise_lp * self.noise_env * self.noise_gain;
            self.noise_env *= self.noise_mul;
        }
        y
    }

    pub fn reset(&mut self) {
        self.t = self.click_len;
        self.noise_env = 0.0;
        self.noise_lp = 0.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn harder_clicks_are_shorter_and_off_is_silent() {
        let len = |velocity: f32| {
            let mut t = Transient::new(96_000.0);
            let mut rng = Noise::new(1);
            t.trigger(velocity, 1.0, 0.0);
            (0..2_000).filter(|_| t.tick(&mut rng) != 0.0).count()
        };
        assert!(len(0.1) > 2 * len(1.0));
        let mut t = Transient::new(96_000.0);
        let mut rng = Noise::new(1);
        t.trigger(1.0, 0.0, 0.0);
        assert!((0..2_000).all(|_| t.tick(&mut rng) == 0.0));
    }
}
