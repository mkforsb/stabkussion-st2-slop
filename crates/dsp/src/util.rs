//! Small DSP building blocks shared by the engines and the channel strip.

use core::f32::consts::PI;

/// xorshift32 white noise.
#[derive(Clone, Debug)]
pub struct Noise {
    state: u32,
}

impl Noise {
    pub fn new(seed: u32) -> Self {
        // Scramble so nearby seeds (0, 1, 2 …) give unrelated sequences.
        let mut s = seed.wrapping_mul(0x9E37_79B9) ^ 0x6A09_E667;
        s ^= s >> 16;
        s = s.wrapping_mul(0x85EB_CA6B);
        s ^= s >> 13;
        Self { state: s.max(1) }
    }

    #[inline]
    pub fn next_u32(&mut self) -> u32 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.state = x;
        x
    }

    /// Uniform white noise in `[-1, 1)`.
    #[inline]
    pub fn sample(&mut self) -> f32 {
        (self.next_u32() >> 8) as f32 * (2.0 / 16_777_216.0) - 1.0
    }

    /// Uniform in `[0, 1)`.
    #[inline]
    pub fn uniform(&mut self) -> f32 {
        (self.next_u32() >> 8) as f32 * (1.0 / 16_777_216.0)
    }
}

/// Rational tanh approximation, accurate to ~0.2% and exact at the clamp.
#[inline]
pub fn fast_tanh(x: f32) -> f32 {
    let x = x.clamp(-3.0, 3.0);
    let x2 = x * x;
    x * (27.0 + x2) / (27.0 + 9.0 * x2)
}

/// `sin(2π·turns)` for any finite input, accurate to ~4e-6.
#[inline]
pub fn sin_turns(turns: f32) -> f32 {
    // Reduce to [-0.5, 0.5), then fold into [-0.25, 0.25].
    let mut t = turns - (turns + 0.5).floor();
    if t > 0.25 {
        t = 0.5 - t;
    } else if t < -0.25 {
        t = -0.5 - t;
    }
    let z = t * (2.0 * PI);
    let z2 = z * z;
    z * (1.0 - z2 / 6.0 * (1.0 - z2 / 20.0 * (1.0 - z2 / 42.0 * (1.0 - z2 / 72.0))))
}

/// `cos(2π·turns)`.
#[inline]
pub fn cos_turns(turns: f32) -> f32 {
    sin_turns(turns + 0.25)
}

/// Wrap a phase into `[0, 1)`.
#[inline]
pub fn wrap(p: f32) -> f32 {
    p - p.floor()
}

/// Coefficient of a one-pole smoother with time constant `tau_s` at rate `fs`.
#[inline]
pub fn one_pole_coeff(tau_s: f32, fs: f32) -> f32 {
    1.0 - (-1.0 / (tau_s * fs)).exp()
}

/// Per-sample gain that decays by 60 dB in `t60_s`.
#[inline]
pub fn t60_gain(t60_s: f32, fs: f32) -> f32 {
    (-6.907_755 / (t60_s.max(1e-4) * fs)).exp()
}

/// Keep recirculating state out of the (slow on x86) denormal range.
#[inline]
pub fn flush(x: f32) -> f32 {
    if x.abs() < 1e-20 { 0.0 } else { x }
}

/// Highest frequency an oscillator may put a partial at, at oversampled rate
/// `fs`, without an alias landing in the audio band. Partials between `fs/2`
/// and `fs − 24 kHz` fold back above 24 kHz, where the decimator removes them.
#[inline]
pub fn alias_free_limit(fs: f32) -> f32 {
    (fs - 24_000.0).max(0.45 * fs)
}

/// In-place iterative radix-2 FFT of `(re, im)`; `inverse` flips the sign of
/// the exponent (no 1/N scaling).
pub fn fft(re: &mut [f64], im: &mut [f64], inverse: bool) {
    let n = re.len();
    let mut j = 0;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j |= bit;
        if i < j {
            re.swap(i, j);
            im.swap(i, j);
        }
    }
    let sign = if inverse { 1.0 } else { -1.0 };
    let mut len = 2;
    while len <= n {
        let ang = sign * core::f64::consts::TAU / len as f64;
        for start in (0..n).step_by(len) {
            for k in 0..len / 2 {
                let (s, c) = (ang * k as f64).sin_cos();
                let (a, b) = (start + k, start + k + len / 2);
                let tr = re[b] * c - im[b] * s;
                let ti = re[b] * s + im[b] * c;
                re[b] = re[a] - tr;
                im[b] = im[a] - ti;
                re[a] += tr;
                im[a] += ti;
            }
        }
        len <<= 1;
    }
}

// --- polyBLEP -------------------------------------------------------------

/// Morph between sine, triangle, saw and square, all aligned so they rise
/// through zero at phase 0 and their discontinuities sit at phase 0 and ½.
/// Used with [`BlepOsc`], which removes the aliasing of the jumps.
#[derive(Clone, Copy, Debug, Default)]
pub struct Morph {
    pub sin: f32,
    pub tri: f32,
    pub saw: f32,
    pub sq: f32,
}

impl Morph {
    /// `wave` 0 … 1 crossfades sine → triangle → saw → square.
    pub fn sweep(wave: f32) -> Self {
        let s = wave.clamp(0.0, 1.0) * 3.0;
        let i = (s as usize).min(2);
        let f = s - i as f32;
        let mut w = [0.0f32; 4];
        w[i] = 1.0 - f;
        w[i + 1] = f;
        Self {
            sin: w[0],
            tri: w[1],
            saw: w[2],
            sq: w[3],
        }
    }

    /// Saw (0) … square (1).
    pub fn saw_square(x: f32) -> Self {
        let x = x.clamp(0.0, 1.0);
        Self {
            saw: 1.0 - x,
            sq: x,
            ..Self::default()
        }
    }

    /// The naive (aliasing) waveform at phase `p` in `[0, 1)`.
    #[inline]
    pub fn naive(&self, p: f32) -> f32 {
        let mut y = 0.0;
        if self.sin != 0.0 {
            y += self.sin * sin_turns(p);
        }
        if self.tri != 0.0 {
            let t = if p < 0.25 {
                4.0 * p
            } else if p < 0.75 {
                2.0 - 4.0 * p
            } else {
                4.0 * p - 4.0
            };
            y += self.tri * t;
        }
        if self.saw != 0.0 {
            y += self.saw * if p < 0.5 { 2.0 * p } else { 2.0 * p - 2.0 };
        }
        if self.sq != 0.0 {
            y += self.sq * if p < 0.5 { 1.0 } else { -1.0 };
        }
        y
    }

    /// Size of the jump as the phase crosses ½.
    #[inline]
    pub fn jump_half(&self) -> f32 {
        -2.0 * (self.saw + self.sq)
    }

    /// Size of the jump as the phase wraps from 1 to 0.
    #[inline]
    pub fn jump_wrap(&self) -> f32 {
        2.0 * self.sq
    }
}

/// Phase accumulator with polyBLEP correction of a [`Morph`]'s jumps. The
/// output is one sample late, so a discontinuity can correct the sample
/// before it as well as the one after.
#[derive(Clone, Copy, Debug, Default)]
pub struct BlepOsc {
    pub phase: f32,
    next: f32,
}

impl BlepOsc {
    pub fn reset(&mut self, phase: f32) {
        self.phase = phase;
        self.next = 0.0;
    }

    /// Restart at `phase` with the delayed sample already holding `m` there,
    /// so the first output is the wave at `phase` rather than zero.
    pub fn reset_to(&mut self, phase: f32, m: &Morph) {
        self.phase = phase;
        self.next = m.naive(phase);
    }

    /// Register a jump of `size` that happened `t` samples (0 … 1) before the
    /// current sample: corrects the delayed sample (`this`) and the current one.
    #[inline]
    pub fn blep(this: &mut f32, next: &mut f32, size: f32, t: f32) {
        *this += size * 0.5 * t * t;
        let u = 1.0 - t;
        *next -= size * 0.5 * u * u;
    }

    /// Advance by `inc` (< ½) and return the band-limited sample.
    #[inline]
    pub fn tick(&mut self, inc: f32, m: &Morph) -> f32 {
        let mut this = self.next;
        let mut next = 0.0;
        let before = self.phase;
        let mut p = before + inc;
        if before < 0.5 && p >= 0.5 {
            Self::blep(&mut this, &mut next, m.jump_half(), (p - 0.5) / inc);
        }
        if p >= 1.0 {
            p -= 1.0;
            Self::blep(&mut this, &mut next, m.jump_wrap(), p / inc);
        }
        self.phase = p;
        self.next = next + m.naive(p);
        this
    }

    /// Advance by `inc`, but restart at phase 0 `t` samples before the
    /// current one (hard sync from a master that just wrapped).
    #[inline]
    pub fn tick_synced(&mut self, inc: f32, m: &Morph, t: f32) -> f32 {
        let mut this = self.next;
        let mut next = 0.0;
        // Run up to the sync instant, catching any jump on the way.
        let before = self.phase;
        let at = before + inc * (1.0 - t);
        if before < 0.5 && at >= 0.5 {
            Self::blep(&mut this, &mut next, m.jump_half(), t + (at - 0.5) / inc);
        }
        let at = if at >= 1.0 {
            Self::blep(&mut this, &mut next, m.jump_wrap(), t + (at - 1.0) / inc);
            at - 1.0
        } else {
            at
        };
        // The reset itself: from wherever the slave was to phase 0.
        Self::blep(&mut this, &mut next, m.naive(0.0) - m.naive(at), t);
        let p = (inc * t).min(0.499);
        self.phase = p;
        self.next = next + m.naive(p);
        this
    }
}

// --- filters ---------------------------------------------------------------

/// Four-pole transistor-ladder lowpass (Moog style) in zero-delay-feedback
/// form, after V. Zavalishin, "The Art of VA Filter Design", ch. 5. The
/// feedback is solved linearly, then passed through a `tanh`, which bounds
/// self-oscillation. Like the Oberheim Xpander, the outputs of the input stage
/// and the four poles are mixed to give the other responses.
#[derive(Clone, Debug, Default)]
pub struct Ladder {
    s: [f32; 4],
}

/// Coefficients for [`Ladder`].
#[derive(Clone, Copy, Debug)]
pub struct LadderCoeffs {
    /// Prewarped integrator gain `tan(π·fc/fs)`.
    g: f32,
    /// One-pole gain `g / (1 + g)`.
    big_g: f32,
    /// Feedback, 0 … ~4.3 (self-oscillation from 4).
    k: f32,
    /// Weights of (input stage, pole 1, pole 2, pole 3, pole 4).
    mix: [f32; 5],
    /// Passband gain make-up.
    makeup: f32,
}

/// Mode mixes for LP12, LP24, BP and HP (in [`crate::params::FILTER_TYPE_NAMES`] order).
const LADDER_MODES: [[f32; 5]; 4] = [
    [0.0, 0.0, 1.0, 0.0, 0.0],
    [0.0, 0.0, 0.0, 0.0, 1.0],
    [0.0, 2.0, -2.0, 0.0, 0.0],
    [1.0, -2.0, 1.0, 0.0, 0.0],
];

/// Feedback is taken from `pole 4 − BASS_COMP · input`: resonance only costs
/// half as much low end as on a plain ladder.
const BASS_COMP: f32 = 0.5;

impl LadderCoeffs {
    pub fn new(fc_hz: f32, k: f32, mode: u32, fs: f32) -> Self {
        let g = (PI * fc_hz.clamp(5.0, 0.45 * fs) / fs).tan();
        let mode = (mode as usize).min(LADDER_MODES.len() - 1);
        // The low/bandpass gain at DC (resp. the peak) drops as the feedback
        // rises; give some of it back.
        let makeup = match mode {
            0 | 1 => (1.0 + k) / (1.0 + k * BASS_COMP),
            // The band and highpass mixes resonate on their own.
            _ => 1.0,
        };
        Self {
            g,
            big_g: g / (1.0 + g),
            k,
            mix: LADDER_MODES[mode],
            makeup: makeup.sqrt(),
        }
    }

    /// The same filter with its cutoff multiplied by about `scale`, cheaply
    /// enough to do every sample (audio-rate filter FM): `tan` is close to
    /// linear below a quarter of the rate, and the result is kept below the
    /// top of the range.
    #[inline]
    pub fn scaled(&self, scale: f32) -> Self {
        // tan(π · 0.45)
        const G_MAX: f32 = 6.313_752;
        let g = (self.g * scale).clamp(1e-4, G_MAX);
        Self {
            g,
            big_g: g / (1.0 + g),
            ..*self
        }
    }
}

impl Ladder {
    #[inline]
    pub fn process(&mut self, x: f32, c: &LadderCoeffs) -> f32 {
        let g = c.big_g;
        let h = 1.0 - g;
        // y4 = G⁴·u + S, with S the contribution of the stored states.
        let s = h * (g * (g * (g * self.s[0] + self.s[1]) + self.s[2]) + self.s[3]);
        let g4 = g * g * g * g;
        let xin = x * (1.0 + c.k * BASS_COMP);
        let y4_lin = (g4 * xin + s) / (1.0 + c.k * g4);
        let u = xin - c.k * fast_tanh(y4_lin);
        let mut stage_in = u;
        let mut y = [u, 0.0, 0.0, 0.0, 0.0];
        for (i, st) in self.s.iter_mut().enumerate() {
            let v = (stage_in - *st) * g;
            let out = v + *st;
            *st = flush(out + v);
            y[i + 1] = out;
            stage_in = out;
        }
        let m = &c.mix;
        (m[0] * y[0] + m[1] * y[1] + m[2] * y[2] + m[3] * y[3] + m[4] * y[4]) * c.makeup
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

/// Transposed direct form II biquad.
#[derive(Clone, Copy, Debug, Default)]
pub struct Biquad {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    z1: f32,
    z2: f32,
}

impl Biquad {
    /// Unity pass-through.
    pub fn identity() -> Self {
        Self {
            b0: 1.0,
            ..Self::default()
        }
    }

    fn set(&mut self, b: [f32; 3], a: [f32; 3]) {
        let n = 1.0 / a[0];
        self.b0 = b[0] * n;
        self.b1 = b[1] * n;
        self.b2 = b[2] * n;
        self.a1 = a[1] * n;
        self.a2 = a[2] * n;
    }

    fn w(f: f32, fs: f32) -> (f32, f32) {
        let w = 2.0 * PI * f.clamp(5.0, 0.45 * fs) / fs;
        (w.cos(), w.sin())
    }

    /// RBJ cookbook highpass.
    pub fn set_highpass(&mut self, f: f32, q: f32, fs: f32) {
        let (c, s) = Self::w(f, fs);
        let alpha = s / (2.0 * q);
        self.set(
            [(1.0 + c) / 2.0, -(1.0 + c), (1.0 + c) / 2.0],
            [1.0 + alpha, -2.0 * c, 1.0 - alpha],
        );
    }

    /// RBJ cookbook lowpass.
    pub fn set_lowpass(&mut self, f: f32, q: f32, fs: f32) {
        let (c, s) = Self::w(f, fs);
        let alpha = s / (2.0 * q);
        self.set(
            [(1.0 - c) / 2.0, 1.0 - c, (1.0 - c) / 2.0],
            [1.0 + alpha, -2.0 * c, 1.0 - alpha],
        );
    }

    /// RBJ cookbook peaking EQ.
    pub fn set_peak(&mut self, f: f32, q: f32, db: f32, fs: f32) {
        let (c, s) = Self::w(f, fs);
        let a = 10f32.powf(db / 40.0);
        let alpha = s / (2.0 * q);
        self.set(
            [1.0 + alpha * a, -2.0 * c, 1.0 - alpha * a],
            [1.0 + alpha / a, -2.0 * c, 1.0 - alpha / a],
        );
    }

    /// RBJ cookbook shelf with slope 1; `high` selects the high shelf.
    pub fn set_shelf(&mut self, f: f32, db: f32, high: bool, fs: f32) {
        let (c, s) = Self::w(f, fs);
        let a = 10f32.powf(db / 40.0);
        let alpha = s / 2.0 * core::f32::consts::SQRT_2;
        let k = 2.0 * a.sqrt() * alpha;
        let sg = if high { -1.0 } else { 1.0 };
        // Low shelf; the high shelf mirrors the signs of the cosine terms.
        let b0 = a * ((a + 1.0) - sg * (a - 1.0) * c + k);
        let b1 = 2.0 * sg * a * ((a - 1.0) - sg * (a + 1.0) * c);
        let b2 = a * ((a + 1.0) - sg * (a - 1.0) * c - k);
        let a0 = (a + 1.0) + sg * (a - 1.0) * c + k;
        let a1 = -2.0 * sg * ((a - 1.0) + sg * (a + 1.0) * c);
        let a2 = (a + 1.0) + sg * (a - 1.0) * c - k;
        self.set([b0, b1, b2], [a0, a1, a2]);
    }

    #[inline]
    pub fn process(&mut self, x: f32) -> f32 {
        let y = self.b0 * x + self.z1;
        self.z1 = flush(self.b1 * x - self.a1 * y + self.z2);
        self.z2 = flush(self.b2 * x - self.a2 * y);
        y
    }

    /// Magnitude response at `f` (for tests and displays).
    pub fn magnitude(&self, f: f32, fs: f32) -> f32 {
        let w = 2.0 * PI * f / fs;
        let (c1, s1) = (w.cos(), -w.sin());
        let (c2, s2) = ((2.0 * w).cos(), -(2.0 * w).sin());
        let num = (
            self.b0 + self.b1 * c1 + self.b2 * c2,
            self.b1 * s1 + self.b2 * s2,
        );
        let den = (
            1.0 + self.a1 * c1 + self.a2 * c2,
            self.a1 * s1 + self.a2 * s2,
        );
        ((num.0 * num.0 + num.1 * num.1) / (den.0 * den.0 + den.1 * den.1)).sqrt()
    }

    pub fn reset(&mut self) {
        self.z1 = 0.0;
        self.z2 = 0.0;
    }
}

/// Power-of-two ring buffer with fractional (cubic Hermite) reads.
#[derive(Clone, Debug)]
pub struct DelayLine {
    buf: Vec<f32>,
    mask: usize,
    pos: usize,
}

impl DelayLine {
    /// A line that can delay by at least `max_delay` samples.
    pub fn new(max_delay: usize) -> Self {
        let size = (max_delay + 4).next_power_of_two();
        Self {
            buf: vec![0.0; size],
            mask: size - 1,
            pos: 0,
        }
    }

    /// Longest delay [`DelayLine::read`] supports.
    pub fn max_delay(&self) -> f32 {
        (self.buf.len() - 4) as f32
    }

    #[inline]
    pub fn write(&mut self, x: f32) {
        self.pos = (self.pos + 1) & self.mask;
        self.buf[self.pos] = flush(x);
    }

    /// The sample written `k` writes ago (`0` = most recent).
    #[inline]
    pub fn tap(&self, k: usize) -> f32 {
        self.buf[self.pos.wrapping_sub(k) & self.mask]
    }

    /// Output delayed by `d` samples relative to the next [`write`](Self::write),
    /// i.e. call `read` before `write` in a sample loop. `d` is clamped to
    /// `2..=max_delay`.
    #[inline]
    pub fn read(&self, d: f32) -> f32 {
        let p = d.clamp(2.0, self.max_delay()) - 1.0;
        let i = p as usize;
        let f = p - i as f32;
        let xm1 = self.tap(i - 1);
        let x0 = self.tap(i);
        let x1 = self.tap(i + 1);
        let x2 = self.tap(i + 2);
        let c1 = 0.5 * (x1 - xm1);
        let c2 = xm1 - 2.5 * x0 + 2.0 * x1 - 0.5 * x2;
        let c3 = 0.5 * (x2 - xm1) + 1.5 * (x0 - x1);
        ((c3 * f + c2) * f + c1) * f + x0
    }

    pub fn clear(&mut self) {
        self.buf.iter_mut().for_each(|x| *x = 0.0);
    }
}

#[derive(Clone, Debug, Default)]
pub struct DcBlocker {
    x1: f32,
    y1: f32,
}

impl DcBlocker {
    /// `r` close to 1: `1 - 2π·fc/fs`.
    #[inline]
    pub fn process(&mut self, x: f32, r: f32) -> f32 {
        let y = x - self.x1 + r * self.y1;
        self.x1 = x;
        self.y1 = flush(y);
        y
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sin_turns_is_accurate() {
        for i in -2000..=2000 {
            let t = i as f32 * 0.001_37;
            let err = (sin_turns(t) - (t * 2.0 * PI).sin()).abs();
            assert!(err < 2e-5, "t={t} err={err}");
            let err = (cos_turns(t) - (t * 2.0 * PI).cos()).abs();
            assert!(err < 2e-5, "t={t} err={err}");
        }
    }

    #[test]
    fn fast_tanh_close() {
        for i in -40..=40 {
            let x = i as f32 * 0.1;
            assert!((fast_tanh(x) - x.tanh()).abs() < 0.025, "x={x}");
        }
    }

    #[test]
    fn noise_is_bounded_and_centered() {
        let mut n = Noise::new(1);
        let mut sum = 0.0f64;
        for _ in 0..100_000 {
            let x = n.sample();
            assert!((-1.0..1.0).contains(&x));
            sum += x as f64;
        }
        assert!((sum / 100_000.0).abs() < 0.01);
    }

    #[test]
    fn delay_line_delays() {
        let mut d = DelayLine::new(64);
        let mut out = Vec::new();
        for n in 0..40 {
            out.push(d.read(10.0));
            d.write(if n == 0 { 1.0 } else { 0.0 });
        }
        assert_eq!(out.iter().position(|&x| x == 1.0), Some(10));
    }

    #[test]
    fn morph_shapes_are_continuous_where_they_should_be() {
        for wave in [0.0, 0.2, 0.33, 0.5, 0.8, 1.0] {
            let m = Morph::sweep(wave);
            // Across ½ and across the wrap, the naive wave jumps by exactly
            // what jump_half / jump_wrap report.
            let e = 1e-4;
            let half = m.naive(0.5) - m.naive(0.5 - e);
            assert!((half - m.jump_half()).abs() < 0.01, "{wave}: {half}");
            let wrap = m.naive(0.0) - m.naive(1.0 - e);
            assert!((wrap - m.jump_wrap()).abs() < 0.01, "{wave}: {wrap}");
        }
    }

    /// Energy above the fundamental's 40th harmonic: the band-limited saw has
    /// far less of it than the naive one.
    #[test]
    fn blep_saw_aliases_less_than_naive() {
        let fs = 48_000.0;
        let inc = 1_234.5 / fs;
        let m = Morph::saw_square(0.0);
        let mut osc = BlepOsc::default();
        let mut naive_phase = 0.0f32;
        let n = 4_096;
        let blep: Vec<f32> = (0..n).map(|_| osc.tick(inc, &m)).collect();
        let naive: Vec<f32> = (0..n)
            .map(|_| {
                naive_phase = wrap(naive_phase + inc);
                m.naive(naive_phase)
            })
            .collect();
        // Compare the error against an ideal additive saw.
        let ideal = |i: usize| {
            let mut y = 0.0f32;
            let mut k = 1;
            while k as f32 * 1_234.5 < fs / 2.0 {
                let ph = (i as f32 + 1.0) * inc * k as f32;
                y -= 2.0 / PI * sin_turns(ph + 0.5 * k as f32) / k as f32;
                k += 1;
            }
            y
        };
        let err = |x: &[f32], delay: usize| {
            (200..1200)
                .map(|i| (x[i] - ideal(i - delay)).powi(2))
                .sum::<f32>()
        };
        let (e_blep, e_naive) = (err(&blep, 1), err(&naive, 0));
        assert!(e_blep < 0.3 * e_naive, "blep {e_blep} naive {e_naive}");
    }

    #[test]
    fn ladder_lowpass_passes_bass_and_cuts_highs() {
        let fs = 96_000.0;
        let gain = |hz: f32, k: f32, mode: u32| {
            let c = LadderCoeffs::new(500.0, k, mode, fs);
            let mut l = Ladder::default();
            let n = (fs * 0.2) as usize;
            let mut peak = 0.0f32;
            for i in 0..n {
                let y = l.process(0.1 * sin_turns(i as f32 * hz / fs), &c);
                if i > n / 2 {
                    peak = peak.max(y.abs());
                }
            }
            peak / 0.1
        };
        for k in [0.0, 2.0, 3.5] {
            assert!(gain(60.0, k, 1) > 0.6, "k {k}: bass {}", gain(60.0, k, 1));
            assert!(gain(8_000.0, k, 1) < 0.01);
            assert!(gain(8_000.0, k, 0) < 0.05);
            assert!(gain(60.0, k, 3) < 0.1, "hp bass {}", gain(60.0, k, 3));
            assert!(
                gain(8_000.0, k, 3) > 0.7,
                "hp highs {}",
                gain(8_000.0, k, 3)
            );
        }
        assert!(gain(500.0, 3.5, 1) > 2.0 * gain(500.0, 0.0, 1), "resonance");
    }

    #[test]
    fn ladder_self_oscillation_is_bounded() {
        let fs = 96_000.0;
        let c = LadderCoeffs::new(200.0, 4.3, 1, fs);
        let mut l = Ladder::default();
        let mut peak = 0.0f32;
        for i in 0..96_000 {
            let x = if i == 0 { 1.0 } else { 0.0 };
            peak = peak.max(l.process(x, &c).abs());
        }
        assert!(peak.is_finite() && peak < 6.0, "{peak}");
        assert!(peak > 0.1, "{peak}");
    }

    #[test]
    fn eq_curves_hit_their_gains() {
        let fs = 96_000.0;
        let mut b = Biquad::identity();
        b.set_shelf(100.0, 12.0, false, fs);
        let db = |m: f32| 20.0 * m.log10();
        assert!((db(b.magnitude(10.0, fs)) - 12.0).abs() < 0.5);
        assert!(db(b.magnitude(5_000.0, fs)).abs() < 0.5);
        b.set_shelf(5_000.0, -9.0, true, fs);
        assert!((db(b.magnitude(20_000.0, fs)) + 9.0).abs() < 1.0);
        assert!(db(b.magnitude(50.0, fs)).abs() < 0.3);
        b.set_peak(40.0, 2.0, 18.0, fs);
        assert!((db(b.magnitude(40.0, fs)) - 18.0).abs() < 0.3);
        b.set_highpass(30.0, core::f32::consts::FRAC_1_SQRT_2, fs);
        assert!((db(b.magnitude(30.0, fs)) + 3.0).abs() < 0.3);
        assert!(db(b.magnitude(8.0, fs)) < -20.0);
    }
}
