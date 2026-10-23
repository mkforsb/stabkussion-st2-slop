//! The channel effects: saturator, EQ, chorus and echo. (The plate reverb
//! lives in [`crate::reverb`].)

use crate::params::{SAT_FOLD, SAT_HARD, SAT_TUBE, map};
use crate::util::{Biquad, DelayLine, fast_tanh, flush, one_pole_coeff, sin_turns};

// --- saturator ---------------------------------------------------------------

/// Per-block saturator settings.
#[derive(Clone, Copy, Debug)]
pub struct SatCoeffs {
    kind: u32,
    gain: f32,
    /// Output trim so a -6 dBFS signal comes out at -6 dBFS whatever the drive.
    trim: f32,
    mix: f32,
}

impl Default for SatCoeffs {
    fn default() -> Self {
        Self::new(0.0, 0, 0.0)
    }
}

/// The transfer curves, before gain and trim.
#[inline]
fn curve(kind: u32, x: f32) -> f32 {
    match kind {
        // Asymmetric: the negative half saturates earlier, adding even
        // harmonics (and a level-dependent offset the DC blocker removes).
        SAT_TUBE => {
            if x >= 0.0 {
                fast_tanh(x)
            } else {
                fast_tanh(1.8 * x) / 1.8
            }
        }
        // Cubic soft clip: linear-ish until it hits the rail at ±1.5.
        SAT_HARD => {
            let x = x.clamp(-1.5, 1.5);
            x - x * x * x / 6.75
        }
        // Sine folder: past full scale it folds back instead of flattening.
        SAT_FOLD => sin_turns(0.25 * x),
        _ => fast_tanh(x),
    }
}

impl SatCoeffs {
    /// `drive` and `mix` are knob positions, `kind` one of the `SAT_*` curves.
    pub fn new(drive: f32, kind: u32, mix: f32) -> Self {
        let gain = (map::drive_db(drive) / 20.0 * core::f32::consts::LN_10).exp();
        let trim = if kind == SAT_FOLD {
            1.0
        } else {
            0.5 / curve(kind, 0.5 * gain)
        };
        // Fade the effect in over the first few percent of the DRIVE knob so
        // zero drive is a true bypass and leaving it there costs nothing.
        let mix = mix * (drive * 20.0).min(1.0);
        Self {
            kind,
            gain,
            trim,
            mix,
        }
    }

    #[inline]
    pub fn active(&self) -> bool {
        self.mix > 0.0
    }

    #[inline]
    pub fn process(&self, x: f32) -> f32 {
        let y = curve(self.kind, self.gain * x) * self.trim;
        x + (y - x) * self.mix
    }
}

// --- EQ -----------------------------------------------------------------------

/// Low cut, low shelf, mid bell and high shelf. Coefficients are only
/// recomputed when a knob has moved, and flat bands are skipped.
#[derive(Clone, Debug)]
pub struct Eq {
    bands: [Biquad; 4],
    on: [bool; 4],
    knobs: [f32; 8],
}

impl Default for Eq {
    fn default() -> Self {
        Self {
            bands: [Biquad::identity(); 4],
            on: [false; 4],
            knobs: [f32::NAN; 8],
        }
    }
}

/// Gains closer to 0 dB than this bypass their band.
const FLAT_DB: f32 = 0.02;

impl Eq {
    /// Knob positions: low cut, low freq, low gain, mid freq, mid gain, mid Q,
    /// high freq, high gain.
    pub fn update(&mut self, k: [f32; 8], fs: f32) {
        if k == self.knobs {
            return;
        }
        self.knobs = k;
        self.on[0] = k[0] > 0.0;
        if self.on[0] {
            self.bands[0].set_highpass(map::low_cut_hz(k[0]), core::f32::consts::FRAC_1_SQRT_2, fs);
        }
        let low = map::eq_gain_db(k[2]);
        self.on[1] = low.abs() > FLAT_DB;
        if self.on[1] {
            self.bands[1].set_shelf(map::low_shelf_hz(k[1]), low, false, fs);
        }
        let mid = map::eq_gain_db(k[4]);
        self.on[2] = mid.abs() > FLAT_DB;
        if self.on[2] {
            self.bands[2].set_peak(map::mid_hz(k[3]), map::mid_q(k[5]), mid, fs);
        }
        let high = map::eq_gain_db(k[7]);
        self.on[3] = high.abs() > FLAT_DB;
        if self.on[3] {
            self.bands[3].set_shelf(map::high_shelf_hz(k[6]), high, true, fs);
        }
    }

    #[inline]
    pub fn process(&mut self, mut x: f32) -> f32 {
        for (b, &on) in self.bands.iter_mut().zip(&self.on) {
            if on {
                x = b.process(x);
            } else {
                // Keep a bypassed band from replaying stale state when it
                // comes back.
                b.reset();
            }
        }
        x
    }

    /// Combined magnitude response at `f` (for tests and displays).
    pub fn magnitude(&self, f: f32, fs: f32) -> f32 {
        self.bands
            .iter()
            .zip(&self.on)
            .filter(|(_, on)| **on)
            .map(|(b, _)| b.magnitude(f, fs))
            .product()
    }
}

// --- chorus --------------------------------------------------------------------

/// Delay at the centre of the modulation.
const CHORUS_BASE_S: f32 = 0.007;
/// Longest delay the line is allocated for.
const CHORUS_MAX_S: f32 = 0.014;

/// Three-voice stereo chorus, mono in, stereo out.
///
/// The three taps are swept by one LFO at 120° offsets: left hears taps 1
/// and 2, right taps 3 and 2 inverted, so the image spreads wide and the dry
/// signal comb-filters against the taps for the hollow sound. Below the
/// crossover (4th-order Linkwitz–Riley, whose bands sum back flat) the signal
/// skips the chorus and stays mono and in phase, so the sub keeps its weight.
#[derive(Clone, Debug)]
pub struct Chorus {
    fs: f32,
    line: DelayLine,
    phase: f32,
    lows: [Biquad; 2],
    highs: [Biquad; 2],
    xover: f32,
    c: ChorusControls,
}

#[derive(Clone, Copy, Debug, Default)]
struct ChorusControls {
    mix: f32,
    base: f32,
    depth: f32,
    inc: f32,
    split: bool,
}

impl Chorus {
    pub fn new(fs: f32) -> Self {
        Self {
            fs,
            line: DelayLine::new((CHORUS_MAX_S * fs) as usize + 8),
            phase: 0.0,
            lows: [Biquad::identity(); 2],
            highs: [Biquad::identity(); 2],
            xover: f32::NAN,
            c: ChorusControls::default(),
        }
    }

    /// Set the knob positions (once per block is plenty).
    pub fn update(&mut self, mix: f32, rate: f32, depth: f32, xover: f32) {
        let fs = self.fs;
        let depth_s = (map::chorus_depth_ms(depth) * 0.001).min(CHORUS_BASE_S - 0.001);
        self.c = ChorusControls {
            mix,
            base: CHORUS_BASE_S * fs,
            depth: depth_s * fs,
            inc: map::chorus_hz(rate) / fs,
            split: xover > 0.0,
        };
        if xover != self.xover {
            self.xover = xover;
            if xover > 0.0 {
                let f = map::chorus_xover_hz(xover);
                let q = core::f32::consts::FRAC_1_SQRT_2;
                for b in &mut self.lows {
                    b.set_lowpass(f, q, fs);
                }
                for b in &mut self.highs {
                    b.set_highpass(f, q, fs);
                }
            }
        }
    }

    #[inline]
    pub fn process(&mut self, x: f32) -> (f32, f32) {
        let c = self.c;
        self.phase += c.inc;
        if self.phase >= 1.0 {
            self.phase -= 1.0;
        }
        if c.mix <= 0.0 {
            // Keep the line fed so turning the chorus up doesn't replay
            // stale audio.
            self.line.write(x);
            return (x, x);
        }
        let (low, high) = if c.split {
            let mut low = x;
            let mut high = x;
            for (lp, hp) in self.lows.iter_mut().zip(self.highs.iter_mut()) {
                low = lp.process(low);
                high = hp.process(high);
            }
            (low, high)
        } else {
            (0.0, x)
        };
        let tap = |k: f32| {
            self.line
                .read(c.base + c.depth * sin_turns(self.phase + k / 3.0))
        };
        let (w0, w1, w2) = (tap(0.0), tap(1.0), tap(2.0));
        self.line.write(high);
        let dry = low + high * (1.0 - 0.3 * c.mix);
        let wet = 0.7 * c.mix;
        (dry + wet * (w0 + 0.5 * w1), dry + wet * (w2 - 0.5 * w1))
    }
}

// --- echo ---------------------------------------------------------------------

/// Longest echo the lines are allocated for (the knob tops out at 1.2 s).
const ECHO_MAX_S: f32 = 1.25;
/// The repeats lose their lows below this, so they never pile up into mud.
const ECHO_HP_HZ: f32 = 150.0;
/// How fast the echo time follows its knob: slow enough to glide like tape.
const ECHO_GLIDE_S: f32 = 0.08;

/// A dub-style ping-pong echo, stereo in and out.
///
/// The input (summed to mono) enters the left line; each line feeds the
/// other, so the repeats bounce left, right, left… Inside the loop a lowpass
/// (TONE) darkens every repeat, a highpass keeps the lows from building up
/// and a soft clipper keeps high feedback bounded. Moving TIME glides the
/// read heads, pitching the tail like a tape echo. AMOUNT is a send: it
/// scales what goes in, so a tail keeps repeating after AMOUNT drops (on the
/// next hit of a mutating patch, say). Returns the wet signal only.
#[derive(Clone, Debug)]
pub struct Echo {
    fs: f32,
    lines: [DelayLine; 2],
    lp: [f32; 2],
    hp: [f32; 2],
    time: f32,
    c: EchoControls,
}

#[derive(Clone, Copy, Debug, Default)]
struct EchoControls {
    mix: f32,
    target: f32,
    feedback: f32,
    k_lp: f32,
    k_hp: f32,
    k_glide: f32,
}

impl Echo {
    pub fn new(fs: f32) -> Self {
        let n = (ECHO_MAX_S * fs) as usize + 8;
        Self {
            fs,
            lines: [DelayLine::new(n), DelayLine::new(n)],
            lp: [0.0; 2],
            hp: [0.0; 2],
            time: f32::NAN,
            c: EchoControls::default(),
        }
    }

    /// Set the knob positions (once per block is plenty).
    pub fn update(&mut self, mix: f32, time: f32, feedback: f32, tone: f32) {
        let fs = self.fs;
        let target = map::delay_seconds(time) * fs;
        if self.time.is_nan() {
            self.time = target;
        }
        let tone_hz = map::delay_tone_hz(tone).min(0.45 * fs);
        self.c = EchoControls {
            mix: map::send_gain(mix),
            target,
            feedback: map::delay_feedback(feedback),
            k_lp: one_pole_coeff(1.0 / (core::f32::consts::TAU * tone_hz), fs),
            k_hp: one_pole_coeff(1.0 / (core::f32::consts::TAU * ECHO_HP_HZ), fs),
            k_glide: one_pole_coeff(ECHO_GLIDE_S, fs),
        };
    }

    /// Jump the echo time to its target (no glide), e.g. when rendering a
    /// patch from scratch.
    pub fn snap(&mut self) {
        self.time = self.c.target;
    }

    #[inline]
    pub fn process(&mut self, l: f32, r: f32) -> (f32, f32) {
        let c = self.c;
        self.time += (c.target - self.time) * c.k_glide;
        let outs = [self.lines[0].read(self.time), self.lines[1].read(self.time)];
        // Each repeat is darkened, thinned and gently clipped on its way round.
        let mut back = [0.0f32; 2];
        for i in 0..2 {
            self.lp[i] = flush(self.lp[i] + (outs[i] - self.lp[i]) * c.k_lp);
            self.hp[i] = flush(self.hp[i] + (self.lp[i] - self.hp[i]) * c.k_hp);
            back[i] = fast_tanh(c.feedback * (self.lp[i] - self.hp[i]));
        }
        self.lines[0].write(c.mix * 0.5 * (l + r) + back[1]);
        self.lines[1].write(back[0]);
        (outs[0], outs[1])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::params::{SAT_TAPE, SAT_TYPE_NAMES};

    #[test]
    fn saturator_trim_holds_minus_6_db() {
        for kind in 0..SAT_TYPE_NAMES.len() as u32 {
            if kind == SAT_FOLD {
                continue;
            }
            for drive in [0.1, 0.5, 1.0] {
                let s = SatCoeffs::new(drive, kind, 1.0);
                assert!((s.process(0.5) - 0.5).abs() < 1e-3, "{kind} {drive}");
                assert!(s.process(4.0).abs() < 1.6, "{kind} {drive}");
            }
        }
        assert!(!SatCoeffs::new(0.0, SAT_TAPE, 1.0).active());
    }

    #[test]
    fn eq_flat_is_bypass_and_low_shelf_boosts_the_sub() {
        let fs = 96_000.0;
        let mut eq = Eq::default();
        eq.update([0.0, 0.4, 0.5, 0.45, 0.5, 0.35, 0.55, 0.5], fs);
        assert!(eq.on.iter().all(|on| !on));
        assert_eq!(eq.process(0.3), 0.3);
        eq.update([0.0, 0.4, 1.0, 0.45, 0.5, 0.35, 0.55, 0.5], fs);
        let db = 20.0 * eq.magnitude(30.0, fs).log10();
        assert!(db > 15.0, "{db}");
        eq.update([0.3, 0.4, 0.5, 0.45, 0.5, 0.35, 0.55, 0.5], fs);
        let cut = map::low_cut_hz(0.3);
        assert!(eq.magnitude(cut / 4.0, fs) < 0.1);
        assert!(eq.magnitude(cut * 8.0, fs) > 0.95);
    }

    #[test]
    fn chorus_keeps_the_sub_mono_and_widens_the_rest() {
        let fs = 48_000.0;
        let corr = |hz: f32| {
            let mut ch = Chorus::new(fs);
            ch.update(1.0, 0.5, 1.0, 0.6);
            let (mut lr, mut ll, mut rr) = (0.0f32, 0.0f32, 0.0f32);
            for i in 0..48_000 {
                let (l, r) = ch.process(sin_turns(i as f32 * hz / fs));
                if i > 4_800 {
                    lr += l * r;
                    ll += l * l;
                    rr += r * r;
                }
            }
            lr / (ll * rr).sqrt()
        };
        assert!(corr(40.0) > 0.95, "sub {}", corr(40.0));
        assert!(corr(2_000.0) < 0.8, "highs {}", corr(2_000.0));
    }

    #[test]
    fn chorus_at_zero_is_transparent() {
        let mut ch = Chorus::new(48_000.0);
        ch.update(0.0, 0.5, 1.0, 0.6);
        for i in 0..1_000 {
            let x = sin_turns(i as f32 * 0.01);
            let (l, r) = ch.process(x);
            assert_eq!((l, r), (x, x));
        }
    }

    #[test]
    fn echo_repeats_and_ping_pongs() {
        let fs = 48_000.0;
        let mut e = Echo::new(fs);
        // 0.02 · 60^0.5 ≈ 155 ms.
        e.update(1.0, 0.5, 0.6, 1.0);
        e.snap();
        let d = (map::delay_seconds(0.5) * fs) as usize;
        let mut l = vec![0.0; 4 * d];
        let mut r = vec![0.0; 4 * d];
        for i in 0..4 * d {
            // A short 1 kHz burst.
            let x = if i < 480 {
                sin_turns(i as f32 * 1_000.0 / fs)
            } else {
                0.0
            };
            (l[i], r[i]) = e.process(x, x);
        }
        let energy = |x: &[f32], at: usize| x[at..at + 600].iter().map(|v| v * v).sum::<f32>();
        // First repeat on the left, second on the right, third left again
        // and quieter.
        assert!(energy(&l, d) > 10.0 * energy(&r, d));
        assert!(energy(&r, 2 * d) > 10.0 * energy(&l, 2 * d));
        assert!(energy(&l, 3 * d) < energy(&l, d));
        assert!(energy(&l, 3 * d) > 0.0);
    }

    #[test]
    fn echo_at_zero_is_silent() {
        let mut e = Echo::new(48_000.0);
        e.update(0.0, 0.5, 0.9, 0.5);
        for i in 0..20_000 {
            let x = sin_turns(i as f32 * 0.01);
            assert_eq!(e.process(x, x), (0.0, 0.0));
        }
    }
}
