//! Measuring hits: what a stab sounds like, in numbers.
//!
//! Two tools, both used offline (tests, the `fit` and `stabness` examples):
//!
//! * [`Features`]: a handful of scalars that describe a hit the way the
//!   reference stabs were surveyed: where its energy sits (spectral
//!   centroid and the 10/50/90% energy frequencies), how long it lasts (time
//!   from the peak to −20 dB), how fast it peaks, how much darker it gets
//!   after the first 40 ms (the filter envelope), how wide it is, and how
//!   tonal (spectral flatness). [`Features::distance`] compares two hits on
//!   those, each feature scaled by how much it varies across the reference
//!   set ([`FEATURE_SCALES`]).
//! * [`Gram`]: two log-frequency spectrograms (one fine in frequency, one
//!   fine in time) of the mid signal, in dB relative to the hit's peak.
//!   [`Gram::distance`] is the mean absolute dB difference, the measure the
//!   example fitter minimizes.

use crate::util::fft;

/// Lowest and highest analysed frequency.
const F_LO: f32 = 30.0;
const F_HI: f32 = 6_000.0;
/// Cells further below the peak than this are treated as silence.
const FLOOR_DB: f32 = -50.0;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Features {
    /// Power-weighted mean frequency of the whole hit (Hz).
    pub centroid: f32,
    /// Frequencies below which 10%, 50% and 90% of the energy lies (Hz).
    pub f10: f32,
    pub f50: f32,
    pub f90: f32,
    /// Time from the start to the RMS peak (s): the first time the 5 ms RMS
    /// comes within 1 dB of its maximum.
    pub attack: f32,
    /// Time from the RMS peak to 20 dB below it (s); the length after the
    /// peak if it never gets there.
    pub t20: f32,
    /// Centroid in the 40 ms after the peak over the centroid 100–200 ms
    /// after it: how much the filter envelope closes.
    pub darkening: f32,
    /// RMS of side over RMS of mid.
    pub width: f32,
    /// Spectral flatness (geometric / arithmetic mean power) of the body,
    /// 50 Hz – 2 kHz: near 0 for a few partials, near 1 for noise.
    pub flatness: f32,
}

/// Names of the features, in [`Features::to_vec`] order.
pub const FEATURE_NAMES: [&str; 9] = [
    "centroid",
    "f10",
    "f50",
    "f90",
    "attack",
    "t20",
    "darkening",
    "width",
    "flatness",
];

/// Scale of each feature (in the units [`Features::to_vec`] compares in):
/// the standard deviation of the reference stabs (rounded), so every feature
/// counts about equally. Frequencies and times compare as log2 (octaves,
/// doublings). The references are all tonal (flatness ≤ 0.03), so flatness
/// gets a fixed scale instead: 0.05 more flatness is one unit.
pub const FEATURE_SCALES: [f32; 9] = [0.4, 0.55, 0.55, 0.5, 1.4, 0.55, 0.55, 0.3, 0.05];

impl Features {
    /// The features as compared: log2 of the frequencies and times (the
    /// attack from a 3 ms floor), the rest as they are.
    pub fn to_vec(&self) -> [f32; 9] {
        [
            self.centroid.max(1.0).log2(),
            self.f10.max(1.0).log2(),
            self.f50.max(1.0).log2(),
            self.f90.max(1.0).log2(),
            (self.attack + 0.003).log2(),
            self.t20.max(0.005).log2(),
            self.darkening.max(0.05).log2(),
            self.width,
            self.flatness,
        ]
    }

    /// Scaled RMS distance between two hits' features.
    pub fn distance(&self, other: &Features) -> f32 {
        let (a, b) = (self.to_vec(), other.to_vec());
        let sum: f32 = (0..9)
            .map(|i| ((a[i] - b[i]) / FEATURE_SCALES[i]).powi(2))
            .sum();
        (sum / 9.0).sqrt()
    }

    /// Measure a stereo hit at sample rate `fs`.
    pub fn measure(left: &[f32], right: &[f32], fs: f32) -> Features {
        let n = left.len().min(right.len());
        let mid: Vec<f32> = (0..n).map(|i| 0.5 * (left[i] + right[i])).collect();
        let side: Vec<f32> = (0..n).map(|i| 0.5 * (left[i] - right[i])).collect();
        let energy = |x: &[f32]| x.iter().map(|v| v * v).sum::<f32>();
        let width = (energy(&side) / energy(&mid).max(1e-20)).sqrt();

        // 5 ms RMS envelope.
        let hop = ((0.005 * fs) as usize).max(1);
        let env: Vec<f32> = mid
            .chunks(hop)
            .map(|c| (energy(c) / c.len() as f32).sqrt())
            .collect();
        // The peak is the first time the envelope comes within 1 dB of its
        // maximum: on a flat stab an echo or the reverb can top the hit by a
        // hair much later, and that isn't where the stab peaks.
        let max = env.iter().cloned().fold(0.0f32, f32::max);
        let ipk = env.iter().position(|&e| e >= 0.89 * max).unwrap_or(0);
        let pk = env[ipk];
        let attack = ipk as f32 * hop as f32 / fs;
        let t20 = env[ipk..]
            .iter()
            .position(|&e| e < 0.1 * pk)
            .unwrap_or(env.len() - ipk) as f32
            * hop as f32
            / fs;

        // Long-term spectrum.
        let spec = power_spectrum(&mid);
        let df = fs / (2 * (spec.len() - 1)) as f32;
        let band = |f: f32| f >= 20.0 && f < 0.5 * fs;
        let total: f32 = spec
            .iter()
            .enumerate()
            .filter(|(k, _)| band(*k as f32 * df))
            .map(|(_, p)| p)
            .sum();
        let centroid = spec
            .iter()
            .enumerate()
            .filter(|(k, _)| band(*k as f32 * df))
            .map(|(k, p)| k as f32 * df * p)
            .sum::<f32>()
            / total.max(1e-20);
        let quantile = |q: f32| {
            let mut acc = 0.0;
            for (k, p) in spec.iter().enumerate() {
                if !band(k as f32 * df) {
                    continue;
                }
                acc += p;
                if acc >= q * total {
                    return k as f32 * df;
                }
            }
            0.5 * fs
        };
        let pk_s = attack;
        let seg_centroid = |t0: f32, t1: f32| {
            let (a, b) = (((t0 * fs) as usize).min(n), ((t1 * fs) as usize).min(n));
            if b < a + 64 {
                return f32::NAN;
            }
            let s = power_spectrum_windowed(&mid[a..b]);
            let df = fs / (2 * (s.len() - 1)) as f32;
            let (num, den) = s
                .iter()
                .enumerate()
                .fold((0.0, 0.0), |(num, den), (k, &p)| {
                    (num + k as f32 * df * p, den + p)
                });
            num / den.max(1e-20)
        };
        let early = seg_centroid(pk_s, pk_s + 0.04);
        let late = seg_centroid(pk_s + 0.1, pk_s + 0.2);
        let darkening = if early.is_finite() && late.is_finite() && late > 0.0 {
            early / late
        } else {
            1.0
        };

        // Flatness of the body (the first 300 ms after the peak), 50 Hz – 2 kHz.
        let (a, b) = (
            ((pk_s * fs) as usize).min(n),
            (((pk_s + 0.3) * fs) as usize).min(n),
        );
        let flatness = if b > a + 256 {
            let s = power_spectrum_windowed(&mid[a..b]);
            let df = fs / (2 * (s.len() - 1)) as f32;
            let vals: Vec<f32> = s
                .iter()
                .enumerate()
                .filter(|(k, _)| (50.0..2_000.0).contains(&(*k as f32 * df)))
                .map(|(_, &p)| p.max(1e-20))
                .collect();
            let am = vals.iter().sum::<f32>() / vals.len() as f32;
            let gm = (vals.iter().map(|p| p.ln()).sum::<f32>() / vals.len() as f32).exp();
            gm / am.max(1e-20)
        } else {
            0.0
        };

        Features {
            centroid,
            f10: quantile(0.1),
            f50: quantile(0.5),
            f90: quantile(0.9),
            attack,
            t20,
            darkening,
            width,
            flatness,
        }
    }
}

/// The features of the 27 reference stabs the synth was built against
/// (measured at 24 kHz by `cargo run --example stabs -- rust DIR`). The
/// RND test holds random hits to them.
pub const REFERENCES: [Features; 27] = [
    // sample-001
    Features {
        centroid: 296.7,
        f10: 175.0,
        f50: 260.4,
        f90: 523.7,
        attack: 0.050,
        t20: 0.375,
        darkening: 0.923,
        width: 0.501,
        flatness: 0.0171,
    },
    // sample-002
    Features {
        centroid: 236.5,
        f10: 91.6,
        f50: 98.1,
        f90: 575.7,
        attack: 0.040,
        t20: 0.295,
        darkening: 1.359,
        width: 0.495,
        flatness: 0.0352,
    },
    // sample-003
    Features {
        centroid: 527.6,
        f10: 234.4,
        f50: 539.1,
        f90: 740.5,
        attack: 0.010,
        t20: 0.315,
        darkening: 1.175,
        width: 0.721,
        flatness: 0.0146,
    },
    // sample-004
    Features {
        centroid: 371.1,
        f10: 128.2,
        f50: 208.0,
        f90: 868.7,
        attack: 0.015,
        t20: 0.220,
        darkening: 2.648,
        width: 0.377,
        flatness: 0.0095,
    },
    // sample-005
    Features {
        centroid: 253.3,
        f10: 68.1,
        f50: 198.1,
        f90: 467.3,
        attack: 0.010,
        t20: 0.355,
        darkening: 1.304,
        width: 0.525,
        flatness: 0.0000,
    },
    // sample-006
    Features {
        centroid: 260.3,
        f10: 90.8,
        f50: 153.1,
        f90: 635.7,
        attack: 0.015,
        t20: 0.175,
        darkening: 2.177,
        width: 0.400,
        flatness: 0.0108,
    },
    // sample-007
    Features {
        centroid: 209.3,
        f10: 66.7,
        f50: 158.9,
        f90: 392.6,
        attack: 0.040,
        t20: 0.430,
        darkening: 0.910,
        width: 0.968,
        flatness: 0.0082,
    },
    // sample-008
    Features {
        centroid: 244.3,
        f10: 96.7,
        f50: 231.4,
        f90: 347.2,
        attack: 0.030,
        t20: 0.145,
        darkening: 1.829,
        width: 0.282,
        flatness: 0.0004,
    },
    // sample-009
    Features {
        centroid: 254.4,
        f10: 131.1,
        f50: 155.3,
        f90: 403.6,
        attack: 0.090,
        t20: 0.260,
        darkening: 1.655,
        width: 0.993,
        flatness: 0.0050,
    },
    // sample-010
    Features {
        centroid: 187.4,
        f10: 65.9,
        f50: 194.8,
        f90: 235.8,
        attack: 0.010,
        t20: 0.245,
        darkening: 1.553,
        width: 0.316,
        flatness: 0.0001,
    },
    // sample-011
    Features {
        centroid: 242.2,
        f10: 120.1,
        f50: 127.4,
        f90: 369.1,
        attack: 0.020,
        t20: 0.185,
        darkening: 1.055,
        width: 0.811,
        flatness: 0.0015,
    },
    // sample-012
    Features {
        centroid: 242.4,
        f10: 96.7,
        f50: 231.4,
        f90: 347.2,
        attack: 0.030,
        t20: 0.220,
        darkening: 1.648,
        width: 0.335,
        flatness: 0.0023,
    },
    // sample-013
    Features {
        centroid: 290.2,
        f10: 64.5,
        f50: 202.1,
        f90: 587.4,
        attack: 0.040,
        t20: 0.250,
        darkening: 0.953,
        width: 0.795,
        flatness: 0.0186,
    },
    // sample-014
    Features {
        centroid: 306.1,
        f10: 111.3,
        f50: 279.8,
        f90: 440.9,
        attack: 0.105,
        t20: 0.300,
        darkening: 1.198,
        width: 0.432,
        flatness: 0.0002,
    },
    // sample-015
    Features {
        centroid: 231.6,
        f10: 120.1,
        f50: 167.0,
        f90: 392.6,
        attack: 0.005,
        t20: 0.195,
        darkening: 1.556,
        width: 0.823,
        flatness: 0.0001,
    },
    // sample-016
    Features {
        centroid: 327.3,
        f10: 115.7,
        f50: 232.9,
        f90: 697.3,
        attack: 0.040,
        t20: 0.515,
        darkening: 1.775,
        width: 1.063,
        flatness: 0.0124,
    },
    // sample-017
    Features {
        centroid: 363.4,
        f10: 101.1,
        f50: 260.0,
        f90: 698.7,
        attack: 0.015,
        t20: 0.225,
        darkening: 3.349,
        width: 1.045,
        flatness: 0.0029,
    },
    // sample-018
    Features {
        centroid: 428.4,
        f10: 95.2,
        f50: 326.7,
        f90: 780.8,
        attack: 0.005,
        t20: 0.225,
        darkening: 4.695,
        width: 1.112,
        flatness: 0.0059,
    },
    // sample-019
    Features {
        centroid: 226.9,
        f10: 71.8,
        f50: 111.3,
        f90: 544.9,
        attack: 0.010,
        t20: 0.260,
        darkening: 1.356,
        width: 0.433,
        flatness: 0.0147,
    },
    // sample-020
    Features {
        centroid: 295.2,
        f10: 42.5,
        f50: 279.8,
        f90: 543.5,
        attack: 0.005,
        t20: 0.335,
        darkening: 2.153,
        width: 0.000,
        flatness: 0.0000,
    },
    // sample-021
    Features {
        centroid: 210.1,
        f10: 121.6,
        f50: 203.6,
        f90: 357.4,
        attack: 0.010,
        t20: 0.620,
        darkening: 1.444,
        width: 0.294,
        flatness: 0.0000,
    },
    // sample-022
    Features {
        centroid: 241.0,
        f10: 45.4,
        f50: 254.9,
        f90: 410.2,
        attack: 0.045,
        t20: 0.360,
        darkening: 0.962,
        width: 0.555,
        flatness: 0.0002,
    },
    // sample-023
    Features {
        centroid: 311.0,
        f10: 96.7,
        f50: 359.6,
        f90: 498.8,
        attack: 0.000,
        t20: 0.505,
        darkening: 1.498,
        width: 0.369,
        flatness: 0.0002,
    },
    // sample-024
    Features {
        centroid: 292.5,
        f10: 95.2,
        f50: 304.0,
        f90: 500.2,
        attack: 0.000,
        t20: 0.340,
        darkening: 1.833,
        width: 0.340,
        flatness: 0.0001,
    },
    // sample-025
    Features {
        centroid: 153.3,
        f10: 126.0,
        f50: 133.3,
        f90: 199.2,
        attack: 0.030,
        t20: 0.205,
        darkening: 1.055,
        width: 0.646,
        flatness: 0.0000,
    },
    // sample-026
    Features {
        centroid: 280.1,
        f10: 87.9,
        f50: 180.2,
        f90: 602.1,
        attack: 0.005,
        t20: 0.145,
        darkening: 1.542,
        width: 0.473,
        flatness: 0.0077,
    },
    // sample-027
    Features {
        centroid: 318.3,
        f10: 164.1,
        f50: 228.5,
        f90: 561.0,
        attack: 0.005,
        t20: 0.505,
        darkening: 2.071,
        width: 0.254,
        flatness: 0.0000,
    },
];

/// How far each reference is from its nearest other reference.
pub fn nearest_neighbour_distances(refs: &[Features]) -> Vec<f32> {
    refs.iter()
        .enumerate()
        .map(|(i, a)| {
            refs.iter()
                .enumerate()
                .filter(|(j, _)| *j != i)
                .map(|(_, b)| a.distance(b))
                .fold(f32::MAX, f32::min)
        })
        .collect()
}

/// A hit "is a stab like the references" when it is at least as close to
/// one of them as the loneliest reference is to its nearest neighbour.
pub fn stab_threshold(refs: &[Features]) -> f32 {
    nearest_neighbour_distances(refs)
        .into_iter()
        .fold(0.0, f32::max)
}

/// Distance from `f` to the nearest of `refs`.
pub fn nearest_reference(f: &Features, refs: &[Features]) -> f32 {
    refs.iter().map(|r| f.distance(r)).fold(f32::MAX, f32::min)
}

/// `|FFT|²` of `x` zero-padded to a power of two, bins 0 … N/2.
fn power_spectrum(x: &[f32]) -> Vec<f32> {
    let n = x.len().next_power_of_two().max(2);
    let mut re: Vec<f64> = x.iter().map(|&v| v as f64).collect();
    re.resize(n, 0.0);
    let mut im = vec![0.0; n];
    fft(&mut re, &mut im, false);
    (0..=n / 2)
        .map(|k| (re[k] * re[k] + im[k] * im[k]) as f32)
        .collect()
}

/// [`power_spectrum`] of a Hann-windowed segment.
fn power_spectrum_windowed(x: &[f32]) -> Vec<f32> {
    let n = x.len();
    let w: Vec<f32> = x
        .iter()
        .enumerate()
        .map(|(i, &v)| {
            let t = core::f32::consts::TAU * i as f32 / n as f32;
            v * (0.5 - 0.5 * t.cos())
        })
        .collect();
    power_spectrum(&w)
}

/// One log-frequency spectrogram: `frames × bands` energies in dB.
#[derive(Clone, Debug)]
struct BandGram {
    bands: usize,
    db: Vec<f32>,
}

impl BandGram {
    /// Bands of `per_octave` per octave from [`F_LO`] to [`F_HI`], Hann
    /// windows of `win` samples every `hop`.
    fn new(x: &[f32], fs: f32, win: usize, hop: usize, per_octave: f32) -> Self {
        let bands = ((F_HI / F_LO).log2() * per_octave).ceil() as usize;
        let df = fs / win as f32;
        // Bin → band (or none).
        let band_of: Vec<Option<usize>> = (0..=win / 2)
            .map(|k| {
                let f = k as f32 * df;
                if f < F_LO || f >= F_HI {
                    None
                } else {
                    Some((((f / F_LO).log2() * per_octave) as usize).min(bands - 1))
                }
            })
            .collect();
        let window: Vec<f64> = (0..win)
            .map(|i| 0.5 - 0.5 * (core::f64::consts::TAU * i as f64 / win as f64).cos())
            .collect();
        let frames = if x.len() > win {
            (x.len() - win) / hop + 1
        } else {
            1
        };
        let mut db = Vec::with_capacity(frames * bands);
        let mut re = vec![0.0f64; win];
        let mut im = vec![0.0f64; win];
        for fr in 0..frames {
            let start = fr * hop;
            for i in 0..win {
                re[i] = x.get(start + i).copied().unwrap_or(0.0) as f64 * window[i];
                im[i] = 0.0;
            }
            fft(&mut re, &mut im, false);
            let mut e = vec![1e-12f64; bands];
            for (k, b) in band_of.iter().enumerate() {
                if let Some(b) = *b {
                    e[b] += re[k] * re[k] + im[k] * im[k];
                }
            }
            db.extend(e.iter().map(|&v| (10.0 * v.log10()) as f32));
        }
        // Relative to the loudest cell, floored.
        let max = db.iter().cloned().fold(f32::MIN, f32::max);
        for v in db.iter_mut() {
            *v = (*v - max).max(FLOOR_DB);
        }
        Self { bands, db }
    }

    fn frames(&self) -> usize {
        self.db.len() / self.bands
    }

    /// Mean |dB difference| over the cells where either is above the floor,
    /// over the first `frames` frames (a missing frame counts as silence).
    fn distance(&self, other: &BandGram, frames: usize) -> f32 {
        let (mut sum, mut count) = (0.0f32, 0usize);
        for fr in 0..frames {
            for b in 0..self.bands {
                let get = |g: &BandGram| {
                    if fr < g.frames() {
                        g.db[fr * g.bands + b]
                    } else {
                        FLOOR_DB
                    }
                };
                let (x, y) = (get(self), get(other));
                if x > FLOOR_DB || y > FLOOR_DB {
                    sum += (x - y).abs();
                    count += 1;
                }
            }
        }
        if count == 0 { 0.0 } else { sum / count as f32 }
    }
}

/// Two-resolution log-frequency spectrogram of a hit's mid signal, plus its
/// stereo width.
#[derive(Clone, Debug)]
pub struct Gram {
    /// 1/6 octave, ~170 ms windows (partials and chords).
    fine_f: BandGram,
    /// 1/2 octave, ~21 ms windows (attack and filter sweep).
    fine_t: BandGram,
    width: f32,
}

impl Gram {
    /// `fs` should be 24 kHz or more; the windows scale with it.
    pub fn new(left: &[f32], right: &[f32], fs: f32) -> Self {
        let n = left.len().min(right.len());
        let mid: Vec<f32> = (0..n).map(|i| 0.5 * (left[i] + right[i])).collect();
        let scale = (fs / 24_000.0).max(0.5);
        let pow2 = |x: f32| (x as usize).next_power_of_two();
        let hop_f = ((0.010 * fs) as usize).max(1);
        let hop_t = ((0.005 * fs) as usize).max(1);
        let e = |x: &[f32]| x.iter().map(|v| v * v).sum::<f32>();
        let side: Vec<f32> = (0..n).map(|i| 0.5 * (left[i] - right[i])).collect();
        Self {
            fine_f: BandGram::new(&mid, fs, pow2(4096.0 * scale), hop_f, 6.0),
            fine_t: BandGram::new(&mid, fs, pow2(512.0 * scale), hop_t, 2.0),
            width: (e(&side) / e(&mid).max(1e-20)).sqrt(),
        }
    }

    /// Mean absolute dB difference of the two spectrograms over the length
    /// of `reference` (whatever `self` does after that is ignored: the
    /// reference stabs are cut off), plus 10 dB per unit of width
    /// difference.
    pub fn distance(&self, reference: &Gram) -> f32 {
        let a = self
            .fine_f
            .distance(&reference.fine_f, reference.fine_f.frames());
        let b = self
            .fine_t
            .distance(&reference.fine_t, reference.fine_t.frames());
        0.5 * (a + b) + 10.0 * (self.width - reference.width).abs()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::util::sin_turns;

    fn tone(hz: &[f32], fs: f32, secs: f32, decay_s: f32) -> Vec<f32> {
        (0..(fs * secs) as usize)
            .map(|i| {
                let t = i as f32 / fs;
                let env = (-6.9 * t / decay_s).exp();
                hz.iter().map(|&f| sin_turns(f * t)).sum::<f32>() * env * 0.2
            })
            .collect()
    }

    #[test]
    fn features_of_a_decaying_tone() {
        let fs = 24_000.0;
        let x = tone(&[200.0], fs, 1.0, 0.6);
        let f = Features::measure(&x, &x, fs);
        assert!((f.centroid - 200.0).abs() < 20.0, "{f:?}");
        assert!((f.f50 - 200.0).abs() < 10.0, "{f:?}");
        // -20 dB is a third of the -60 dB time.
        assert!((f.t20 - 0.2).abs() < 0.02, "{f:?}");
        assert!(f.attack < 0.01);
        assert!(f.width < 1e-6);
        assert!(f.flatness < 0.05, "{f:?}");
    }

    #[test]
    fn gram_distance_tells_tones_apart() {
        let fs = 24_000.0;
        let a = tone(&[100.0, 150.0], fs, 0.8, 0.5);
        let b = tone(&[100.0, 150.0], fs, 0.8, 0.45);
        let c = tone(&[100.0, 119.0], fs, 0.8, 0.5);
        let (ga, gb, gc) = (
            Gram::new(&a, &a, fs),
            Gram::new(&b, &b, fs),
            Gram::new(&c, &c, fs),
        );
        assert!(ga.distance(&ga) < 1e-6);
        assert!(gb.distance(&ga) < 0.5 * gc.distance(&ga));
    }
}
