//! Two-channel engine: runs both voices at 2× the output rate, decimates each
//! one, widens it in its own chorus, sends it through its own ping-pong echo
//! and then its own plate reverb (which hears the echoes too), pans, mixes
//! and applies the output stage.
//!
//! ST-2: the two voices hear each other. Every oversampled sample channel 1
//! runs on what channel 2 did one sample earlier and channel 2 on what
//! channel 1 just did (see [`crate::voice::XIn`]). A hit can also strike the
//! other channel after its CHAIN delay; chained both ways, the two bounce a
//! hit back and forth, each strike softer, until it dies out.

use crate::fx::{Chorus, Echo};
use crate::params::{Param, VoicePatch, map};
use crate::reverb::Plate;
use crate::util::{DcBlocker, fast_tanh};
use crate::voice::{Voice, XIn};

/// A chained hit softer than this doesn't strike.
const CHAIN_MIN_VELOCITY: f32 = 0.03;

pub const VOICES: usize = 2;
pub const OVERSAMPLE: usize = 2;
pub const DEFAULT_MASTER: f32 = 0.8;
/// The output DC blocker sits well below the lowest pitch.
const DC_HZ: f32 = 4.0;

/// Windowed-sinc lowpass + 2:1 decimator.
#[derive(Clone, Debug)]
struct Decimator {
    taps: [f32; Self::TAPS],
    /// Doubled history so the dot product is always over a contiguous slice.
    hist: [f32; 2 * Self::TAPS],
    pos: usize,
}

impl Decimator {
    const TAPS: usize = 47;

    fn new() -> Self {
        // Cutoff at 0.23 × the oversampled rate (≈ 22 kHz at 96 kHz).
        let fc = 0.23f32;
        let m = (Self::TAPS - 1) as f32 / 2.0;
        let mut taps = [0.0f32; Self::TAPS];
        for (n, t) in taps.iter_mut().enumerate() {
            let x = n as f32 - m;
            let sinc = if x == 0.0 {
                2.0 * fc
            } else {
                (2.0 * core::f32::consts::PI * fc * x).sin() / (core::f32::consts::PI * x)
            };
            let w = n as f32 / (Self::TAPS - 1) as f32 * core::f32::consts::TAU;
            let blackman = 0.42 - 0.5 * w.cos() + 0.08 * (2.0 * w).cos();
            *t = sinc * blackman;
        }
        let sum: f32 = taps.iter().sum();
        taps.iter_mut().for_each(|t| *t /= sum);
        Self {
            taps,
            hist: [0.0; 2 * Self::TAPS],
            pos: 0,
        }
    }

    #[inline]
    fn push(&mut self, x: f32) {
        self.pos = if self.pos == 0 {
            Self::TAPS - 1
        } else {
            self.pos - 1
        };
        self.hist[self.pos] = x;
        self.hist[self.pos + Self::TAPS] = x;
    }

    #[inline]
    fn process(&mut self, block: [f32; OVERSAMPLE]) -> f32 {
        for x in block {
            self.push(x);
        }
        let h = &self.hist[self.pos..self.pos + Self::TAPS];
        h.iter().zip(self.taps.iter()).map(|(a, b)| a * b).sum()
    }
}

#[derive(Clone, Debug)]
pub struct Engine {
    sample_rate: f32,
    voices: [Voice; VOICES],
    master_target: f32,
    master: f32,
    k_master: f32,
    dc_r: f32,
    /// One decimator per voice (voices are mono until the chorus).
    dec: [Decimator; VOICES],
    choruses: [Chorus; VOICES],
    echoes: [Echo; VOICES],
    reverbs: [Plate; VOICES],
    dc: [DcBlocker; 2],
    /// Chained hits waiting to strike: (output samples to go, raw velocity).
    pending: [Option<(u32, f32)>; VOICES],
    /// Hits per voice, chained ones included (for the panel's LEDs).
    hits: [u32; VOICES],
}

impl Engine {
    pub fn new(sample_rate: f32) -> Self {
        let fs = sample_rate * OVERSAMPLE as f32;
        Self {
            sample_rate,
            voices: [Voice::new(fs, 1), Voice::new(fs, 2)],
            master_target: DEFAULT_MASTER,
            master: DEFAULT_MASTER,
            k_master: 1.0 - (-1.0 / (0.01 * sample_rate)).exp(),
            dc_r: 1.0 - core::f32::consts::TAU * DC_HZ / sample_rate,
            dec: [Decimator::new(), Decimator::new()],
            choruses: [Chorus::new(sample_rate), Chorus::new(sample_rate)],
            echoes: [Echo::new(sample_rate), Echo::new(sample_rate)],
            reverbs: [Plate::new(sample_rate), Plate::new(sample_rate)],
            dc: [DcBlocker::default(), DcBlocker::default()],
            pending: [None; VOICES],
            hits: [0; VOICES],
        }
    }

    pub fn sample_rate(&self) -> f32 {
        self.sample_rate
    }

    pub fn set_param(&mut self, voice: usize, param: Param, value: f32) {
        if let Some(v) = self.voices.get_mut(voice) {
            v.set_param(param, value);
        }
    }

    pub fn param(&self, voice: usize, param: Param) -> f32 {
        self.voices[voice].param(param)
    }

    pub fn load_patch(&mut self, voice: usize, patch: &VoicePatch) {
        if let Some(v) = self.voices.get_mut(voice) {
            for p in crate::params::ALL_PARAMS {
                v.set_param(p, patch[p.index()]);
            }
        }
    }

    /// Jump smoothed parameters straight to their targets.
    pub fn snap_params(&mut self) {
        self.voices.iter_mut().for_each(Voice::snap_params);
        self.update_effects();
        self.echoes.iter_mut().for_each(Echo::snap);
        self.master = self.master_target;
    }

    /// Hand the chorus and echo their (block-rate) settings.
    fn update_effects(&mut self) {
        for ((chorus, echo), v) in self
            .choruses
            .iter_mut()
            .zip(self.echoes.iter_mut())
            .zip(&self.voices)
        {
            chorus.update(
                v.smoothed(Param::ChorusMix),
                v.smoothed(Param::ChorusRate),
                v.smoothed(Param::ChorusDepth),
                v.smoothed(Param::ChorusXover),
            );
            echo.update(
                v.smoothed(Param::DelayMix),
                v.smoothed(Param::DelayTime),
                v.smoothed(Param::DelayFeedback),
                v.smoothed(Param::DelayTone),
            );
        }
    }

    /// Master volume, `0..=1` knob position.
    pub fn set_master(&mut self, value: f32) {
        self.master_target = if value.is_finite() {
            value.clamp(0.0, 1.0)
        } else {
            DEFAULT_MASTER
        };
    }

    /// Jump one voice's smoothed parameters to their targets, e.g. so a patch
    /// change sent together with a trigger takes effect exactly on the hit.
    pub fn snap_voice(&mut self, voice: usize) {
        if let Some(v) = self.voices.get_mut(voice) {
            v.snap_params();
        }
    }

    pub fn trigger(&mut self, voice: usize, velocity: f32) {
        let Some(v) = self.voices.get_mut(voice) else {
            return;
        };
        v.trigger(velocity);
        self.hits[voice] = self.hits[voice].wrapping_add(1);
        // Strike the other channel later, if it is chained to this one.
        let other = (voice + 1) % VOICES;
        let o = &self.voices[other];
        let chained = velocity.clamp(0.0, 1.0) * map::chain_gain(o.param(Param::XChain));
        if chained >= CHAIN_MIN_VELOCITY {
            let delay = map::chain_seconds(o.param(Param::XChainTime)) * self.sample_rate;
            self.pending[other] = Some(((delay as u32).max(1), chained));
        }
    }

    /// Hits so far on `voice`, chained ones included.
    pub fn hit_count(&self, voice: usize) -> u32 {
        self.hits[voice]
    }

    /// Run the chain-trigger countdowns by one output sample.
    #[inline]
    fn advance_chains(&mut self) {
        for voice in 0..VOICES {
            if let Some((left, velocity)) = self.pending[voice] {
                if left <= 1 {
                    self.pending[voice] = None;
                    self.trigger(voice, velocity);
                } else {
                    self.pending[voice] = Some((left - 1, velocity));
                }
            }
        }
    }

    pub fn lfo_led(&self, voice: usize) -> bool {
        self.voices[voice].lfo_led()
    }

    pub fn envelope(&self, voice: usize) -> f32 {
        self.voices[voice].envelope()
    }

    /// Render stereo audio. `left` and `right` must be the same length.
    pub fn render(&mut self, left: &mut [f32], right: &mut [f32]) {
        debug_assert_eq!(left.len(), right.len());
        let pans: [(f32, f32); VOICES] = core::array::from_fn(|i| {
            // Equal-power pan, normalized so the centre is unity per side.
            let a = self.voices[i].param(Param::Pan) * core::f32::consts::FRAC_PI_2;
            (
                a.cos() * core::f32::consts::SQRT_2,
                a.sin() * core::f32::consts::SQRT_2,
            )
        });
        // The chorus and echo settings move slowly enough to update once per
        // block.
        self.update_effects();
        for (l, r) in left.iter_mut().zip(right.iter_mut()) {
            if self.pending.iter().any(Option::is_some) {
                self.advance_chains();
            }
            // Both voices at the oversampled rate, each hearing the other.
            let mut blocks = [[0.0f32; OVERSAMPLE]; VOICES];
            for s in 0..OVERSAMPLE {
                for i in 0..VOICES {
                    let other = &self.voices[(i + 1) % VOICES];
                    let (osc, out) = other.taps();
                    let xin = XIn {
                        osc,
                        out,
                        env: other.envelope_peak(),
                    };
                    blocks[i][s] = self.voices[i].tick_x(&xin);
                }
            }
            let (mut yl, mut yr) = (0.0, 0.0);
            for (i, (gl, gr)) in pans.iter().enumerate() {
                let v = &self.voices[i];
                let dry = self.dec[i].process(blocks[i]);
                let (cl, cr) = self.choruses[i].process(dry);
                let (el, er) = self.echoes[i].process(cl, cr);
                let (cl, cr) = (cl + el, cr + er);
                // Post-fader send into the channel's own plate. The plate always
                // runs so the tail keeps ringing when AMOUNT is turned down.
                let plate = &mut self.reverbs[i];
                let controls = plate.controls(
                    v.smoothed(Param::ReverbDecay),
                    v.smoothed(Param::ReverbTone),
                    v.smoothed(Param::ReverbPredelay),
                );
                let (wl, wr) = plate.process(0.5 * (cl + cr), &controls);
                let send = map::send_gain(v.smoothed(Param::ReverbMix));
                // Pan acts as a balance control on the stereo signal.
                yl += (cl + send * wl) * gl;
                yr += (cr + send * wr) * gr;
            }
            self.master += (self.master_target - self.master) * self.k_master;
            let gain = map::level_gain(self.master) * 1.4;
            *l = soft_clip(self.dc[0].process(yl, self.dc_r) * gain);
            *r = soft_clip(self.dc[1].process(yr, self.dc_r) * gain);
        }
    }

    /// Convenience for offline rendering.
    pub fn render_vec(&mut self, frames: usize) -> (Vec<f32>, Vec<f32>) {
        let mut l = vec![0.0; frames];
        let mut r = vec![0.0; frames];
        self.render(&mut l, &mut r);
        (l, r)
    }
}

/// Output stage: transparent below ~-6 dBFS, gently saturating above.
#[inline]
fn soft_clip(x: f32) -> f32 {
    const KNEE: f32 = 0.5;
    let a = x.abs();
    if a <= KNEE {
        x
    } else {
        let over = (a - KNEE) / (1.0 - KNEE);
        (KNEE + (1.0 - KNEE) * fast_tanh(over)).copysign(x)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::params::{ALL_PARAMS, ENGINE_COUNT, ParamKind};

    fn rms(x: &[f32]) -> f32 {
        (x.iter().map(|v| v * v).sum::<f32>() / x.len() as f32).sqrt()
    }

    #[test]
    fn silent_until_triggered() {
        let mut e = Engine::new(48_000.0);
        let (l, r) = e.render_vec(4_800);
        assert!(rms(&l) < 1e-4 && rms(&r) < 1e-4);
    }

    #[test]
    fn every_engine_makes_sound_and_decays() {
        for engine in 0..ENGINE_COUNT {
            let mut e = Engine::new(48_000.0);
            e.set_param(0, Param::Engine, engine as f32);
            e.set_param(0, Param::Decay, 0.3);
            e.set_param(0, Param::ReverbMix, 0.0);
            e.snap_params();
            e.trigger(0, 1.0);
            let (l, _) = e.render_vec(4_800);
            let early = rms(&l);
            assert!(early > 0.01, "engine {engine} too quiet: {early}");
            let (l, _) = e.render_vec(48_000 * 3);
            let late = rms(&l[l.len() - 4_800..]);
            assert!(late < early * 0.01, "engine {engine} did not decay: {late}");
        }
    }

    #[test]
    fn output_is_finite_and_bounded_under_random_patches() {
        let mut rng = crate::util::Noise::new(99);
        let mut e = Engine::new(44_100.0);
        for round in 0..80 {
            for v in 0..VOICES {
                for p in ALL_PARAMS {
                    let x = rng.uniform();
                    let val = match p.info().kind {
                        ParamKind::Continuous => x,
                        ParamKind::Choice(n) => (x * n.len() as f32).floor(),
                    };
                    e.set_param(v, p, val);
                }
                e.trigger(v, rng.uniform());
            }
            e.set_master(1.0);
            let (l, r) = e.render_vec(2_205);
            for s in l.iter().chain(r.iter()) {
                assert!(s.is_finite() && s.abs() <= 1.0, "round {round}: {s}");
            }
        }
    }

    #[test]
    fn higher_velocity_is_louder() {
        for engine in 0..ENGINE_COUNT {
            let level = |vel: f32| {
                let mut e = Engine::new(48_000.0);
                e.set_param(0, Param::Engine, engine as f32);
                e.set_param(0, Param::Sense, 0.5);
                e.snap_params();
                e.trigger(0, vel);
                rms(&e.render_vec(4_800).0)
            };
            assert!(level(1.0) > level(0.3) * 1.5, "engine {engine}");
        }
    }

    fn hit(configure: impl Fn(&mut Engine), frames: usize) -> (Vec<f32>, Vec<f32>) {
        let mut e = Engine::new(48_000.0);
        e.set_param(0, Param::Decay, 0.25);
        e.set_param(0, Param::ReverbMix, 0.0);
        configure(&mut e);
        e.snap_params();
        e.trigger(0, 1.0);
        e.render_vec(frames)
    }

    #[test]
    fn reverb_at_zero_amount_leaves_the_dry_signal_untouched() {
        let dry = hit(|_| {}, 24_000);
        let other = hit(
            |e| {
                e.set_param(0, Param::ReverbDecay, 1.0);
                e.set_param(0, Param::ReverbTone, 0.0);
                e.set_param(0, Param::ReverbPredelay, 1.0);
            },
            24_000,
        );
        assert_eq!(dry, other);
    }

    #[test]
    fn reverb_adds_a_tail() {
        let frames = 48_000 * 2;
        let with_reverb = |amount: f32| {
            let (l, r) = hit(
                |e| {
                    e.set_param(0, Param::ReverbMix, amount);
                    e.set_param(0, Param::ReverbDecay, 0.6);
                },
                frames,
            );
            rms(&l[48_000..72_000]) + rms(&r[48_000..72_000])
        };
        let dry = with_reverb(0.0);
        let wet = with_reverb(0.7);
        assert!(dry < 1e-4, "dry tail {dry}");
        assert!(wet > 1e-3, "reverb tail {wet}");
    }

    #[test]
    fn chorus_widens_a_mono_channel() {
        let side = |amount: f32| {
            let (l, r) = hit(
                |e| {
                    e.set_param(0, Param::Engine, crate::params::ENGINE_SUPER as f32);
                    e.set_param(0, Param::Pitch, 0.4);
                    e.set_param(0, Param::ChorusMix, amount);
                    e.set_param(0, Param::ChorusXover, 0.0);
                },
                9_600,
            );
            let s: Vec<f32> = l.iter().zip(&r).map(|(a, b)| a - b).collect();
            rms(&s) / rms(&l)
        };
        assert!(side(0.0) < 1e-6);
        assert!(side(1.0) > 0.2, "{}", side(1.0));
    }

    #[test]
    fn echo_repeats_the_hit() {
        let frames = 48_000;
        let tail = |amount: f32| {
            let (l, r) = hit(
                |e| {
                    e.set_param(0, Param::Decay, 0.1);
                    e.set_param(0, Param::DelayMix, amount);
                    // 0.02 · 60^0.6 ≈ 233 ms.
                    e.set_param(0, Param::DelayTime, 0.6);
                    e.set_param(0, Param::DelayFeedback, 0.5);
                },
                frames,
            );
            // Around the first repeat.
            rms(&l[11_000..13_000]) + rms(&r[11_000..13_000])
        };
        assert!(tail(0.0) < 1e-4, "{}", tail(0.0));
        assert!(tail(0.8) > 1e-3, "{}", tail(0.8));
    }

    #[test]
    fn pan_hard_left_with_reverb() {
        let (l, r) = hit(
            |e| {
                e.set_param(0, Param::Pan, 0.0);
                e.set_param(0, Param::ReverbMix, 0.8);
                e.set_param(0, Param::ChorusMix, 0.8);
                e.set_param(0, Param::DelayMix, 0.8);
            },
            24_000,
        );
        assert!(rms(&r) < rms(&l) * 0.01);
    }

    #[test]
    fn snap_voice_applies_pending_changes_to_that_voice_only() {
        let mut e = Engine::new(48_000.0);
        e.snap_params();
        for v in 0..VOICES {
            e.set_param(v, Param::Pitch, 0.9);
        }
        e.snap_voice(0);
        assert_eq!(e.voices[0].smoothed(Param::Pitch), 0.9);
        assert_ne!(
            e.voices[1].smoothed(Param::Pitch),
            0.9,
            "voice 2 should still glide"
        );
    }

    // --- cross-modulation ---------------------------------------------------

    /// Channel 1 on a held sine-ish patch, channel 2 as the modulator, its
    /// output recorded. `xmod` sets channel 1's cross-mod; `hit2` strikes
    /// channel 2 too.
    fn xhit(xmod: &[(Param, f32)], hit2: bool) -> Vec<f32> {
        let mut e = Engine::new(48_000.0);
        for v in 0..VOICES {
            e.set_param(v, Param::ReverbMix, 0.0);
            e.set_param(v, Param::Decay, 0.7);
        }
        e.set_param(0, Param::Engine, crate::params::ENGINE_FM as f32);
        e.set_param(0, Param::MacroY, 0.0);
        e.set_param(0, Param::Pan, 0.0);
        e.set_param(1, Param::Pan, 1.0);
        e.set_param(1, Param::Pitch, 0.43);
        for &(p, v) in xmod {
            e.set_param(0, p, v);
        }
        e.snap_params();
        e.trigger(0, 1.0);
        if hit2 {
            e.trigger(1, 1.0);
        }
        // Channel 1 alone, on the left.
        e.render_vec(24_000).0
    }

    const EVERYTHING: [(Param, f32); 10] = [
        (Param::XFm, 1.0),
        (Param::XRing, 1.0),
        (Param::XFilterFm, 1.0),
        (Param::XCrush, 1.0),
        (Param::XCutoff, 0.0),
        (Param::XPitch, 1.0),
        (Param::XMacro, 1.0),
        (Param::XMacroDest, 1.0),
        (Param::XDuck, 1.0),
        (Param::XSource, 0.0),
    ];

    #[test]
    fn a_silent_other_channel_changes_nothing() {
        let dry = xhit(&[], false);
        assert!(rms(&dry) > 0.01);
        assert_eq!(dry, xhit(&EVERYTHING, false));
        let mut out = EVERYTHING;
        out[9] = (Param::XSource, 1.0);
        assert_eq!(dry, xhit(&out, false));
    }

    #[test]
    fn every_cross_modulation_does_something_when_the_other_plays() {
        let dry = xhit(&[], true);
        for &(p, v) in EVERYTHING[..9]
            .iter()
            .filter(|(p, _)| *p != Param::XMacroDest)
        {
            // →MACRO onto Y, the FM index (X, the ratio, of a sine is moot).
            let wet = xhit(&[(p, v), (Param::XMacroDest, 1.0)], true);
            let diff: Vec<f32> = wet.iter().zip(&dry).map(|(a, b)| a - b).collect();
            let rel = rms(&diff) / rms(&dry);
            assert!(rel > 0.05, "{p:?}: {rel}");
            assert!(wet.iter().all(|s| s.is_finite() && s.abs() <= 1.0), "{p:?}");
        }
    }

    #[test]
    fn ducking_turns_this_channel_down_while_the_other_plays() {
        let dry = xhit(&[], true);
        let ducked = xhit(&[(Param::XDuck, 1.0)], true);
        assert!(rms(&ducked[2_400..9_600]) < 0.5 * rms(&dry[2_400..9_600]));
    }

    #[test]
    fn chained_channels_strike_each_other_and_die_out() {
        let mut e = Engine::new(24_000.0);
        for v in 0..VOICES {
            e.set_param(v, Param::Decay, 0.2);
            e.set_param(v, Param::XChain, 0.9);
            // 0.005 · 300^0.3 ≈ 28 ms.
            e.set_param(v, Param::XChainTime, 0.3);
        }
        e.snap_params();
        e.trigger(0, 1.0);
        let _ = e.render_vec(240);
        assert_eq!((e.hit_count(0), e.hit_count(1)), (1, 0));
        let _ = e.render_vec(720);
        assert_eq!(e.hit_count(1), 1, "the chain strikes channel 2");
        let _ = e.render_vec(24_000 * 10);
        let total = e.hit_count(0) + e.hit_count(1);
        // 0.855^n falls below 0.03 after 22 strikes.
        assert!((15..40).contains(&total), "{total}");
        let before = total;
        let _ = e.render_vec(24_000);
        assert_eq!(e.hit_count(0) + e.hit_count(1), before, "the bounce stops");
    }

    #[test]
    fn chain_off_never_strikes() {
        let mut e = Engine::new(24_000.0);
        e.trigger(0, 1.0);
        let _ = e.render_vec(24_000 * 2);
        assert_eq!(e.hit_count(1), 0);
    }
}
