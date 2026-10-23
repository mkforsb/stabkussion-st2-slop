//! The wavetable bank shared by TABLE and WT-FM: eight single-cycle frames
//! ordered from dark to bright, built once per process.
//!
//! Every frame is stored at ten bandwidths (512, 256 … 1 harmonics), one per
//! octave. An oscillator reads a [`Mip`]: a crossfade between two adjacent
//! levels whose weights move continuously with pitch, so the partials it
//! needs to drop fade out instead of vanishing mid-cycle (which clicks: the
//! top partials of the bright frames add up to a spike once per cycle). Both
//! levels always keep their top harmonic below
//! [`crate::util::alias_free_limit`]. Each
//! level is sampled at 8 points per cycle of its top harmonic (64 at least),
//! so linear interpolation stays clean: the interpolation images sit more
//! than 30 dB below even the top harmonic. All partials are sines in phase,
//! so every frame starts at zero and a phase reset on the hit doesn't click.

use std::sync::OnceLock;

use crate::util::alias_free_limit;

pub const FRAMES: usize = 8;
pub const MIPS: usize = 10;
/// Harmonics in the widest mip level.
pub const MAX_HARMONICS: usize = 512;
/// Table points per cycle of a level's top harmonic.
const OVERSAMPLING: usize = 8;
const MIN_LEN: usize = 64;
/// The mip level (64 harmonics) frames are normalized at.
const NORM_MIP: usize = 3;

/// A crossfade between mip `level` and `level + 1` (weight `mix` on the
/// narrower one).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Mip {
    pub level: usize,
    pub mix: f32,
}

pub struct Tables {
    data: Vec<f32>,
    /// Start of each (frame, mip) table in `data`, and the mip's length.
    offsets: [[usize; MIPS]; FRAMES],
}

static TABLES: OnceLock<Tables> = OnceLock::new();

pub fn tables() -> &'static Tables {
    TABLES.get_or_init(Tables::build)
}

/// Harmonics in mip level `mip`.
const fn harmonics_in(mip: usize) -> usize {
    MAX_HARMONICS >> mip
}

/// Points per cycle in mip level `mip` (a power of two).
const fn len_of(mip: usize) -> usize {
    let n = harmonics_in(mip) * OVERSAMPLING;
    if n < MIN_LEN { MIN_LEN } else { n }
}

/// One cycle of `Σ amps[k-1]·sin(2π·k·n/len)`.
fn synthesize(amps: &[f64], len: usize) -> Vec<f32> {
    let mut re = vec![0.0f64; len];
    let mut im = vec![0.0f64; len];
    for (i, &a) in amps.iter().enumerate() {
        let k = i + 1;
        // sin = (e^{iθ} − e^{−iθ}) / 2i
        im[k] = -0.5 * a;
        im[len - k] = 0.5 * a;
    }
    crate::util::fft(&mut re, &mut im, true);
    re.iter().map(|&x| x as f32).collect()
}

/// Amplitude of harmonic `k` (1-based) in `frame`.
fn harmonic(frame: usize, k: usize) -> f32 {
    let kf = k as f32;
    let odd = !k.is_multiple_of(2);
    match frame {
        // SINE
        0 => (k == 1) as u8 as f32,
        // OCTAVE: a sine with its octave and double octave.
        1 => match k {
            1 => 1.0,
            2 => 0.6,
            4 => 0.15,
            _ => 0.0,
        },
        // TRI
        2 => {
            if odd {
                let sign = if (k / 2).is_multiple_of(2) { 1.0 } else { -1.0 };
                sign / (kf * kf)
            } else {
                0.0
            }
        }
        // ORGAN: eight drawbars.
        3 => [1.0, 0.7, 0.5, 0.35, 0.25, 0.15, 0.1, 0.08]
            .get(k - 1)
            .copied()
            .unwrap_or(0.0),
        // NASAL: a saw-ish slope with a formant around the 5th harmonic.
        4 => (0.6 + 3.0 * (-((kf - 5.0) / 1.8).powi(2)).exp()) / kf,
        // SQUARE
        5 => {
            if odd {
                1.0 / kf
            } else {
                0.0
            }
        }
        // SAW
        6 => 1.0 / kf,
        // FAT: a saw with a doubled fundamental and a strong octave.
        _ => match k {
            1 => 2.0,
            2 => 0.9,
            _ => 1.0 / kf,
        },
    }
}

impl Tables {
    fn build() -> Self {
        let mut offsets = [[0usize; MIPS]; FRAMES];
        let mut total = 0;
        for frame in offsets.iter_mut() {
            for (mip, o) in frame.iter_mut().enumerate() {
                *o = total;
                // One guard point so interpolation never wraps.
                total += len_of(mip) + 1;
            }
        }
        let mut data = vec![0.0f32; total];
        for frame in 0..FRAMES {
            let amps: Vec<f64> = (1..=MAX_HARMONICS)
                .map(|k| harmonic(frame, k) as f64)
                .collect();
            for mip in 0..MIPS {
                let len = len_of(mip);
                let table = synthesize(&amps[..harmonics_in(mip)], len);
                let start = offsets[frame][mip];
                data[start..start + len].copy_from_slice(&table);
                data[start + len] = table[0];
            }
            // Normalize the frame (all its mips alike) to a peak of 1 at the
            // bandwidth of a typical bass note. Wider mips of the brightest
            // frames peak a little higher.
            let norm = offsets[frame][NORM_MIP];
            let peak = data[norm..norm + len_of(NORM_MIP)]
                .iter()
                .fold(0.0f32, |m, x| m.max(x.abs()));
            let (first, end) = (
                offsets[frame][0],
                offsets[frame][MIPS - 1] + len_of(MIPS - 1) + 1,
            );
            data[first..end]
                .iter_mut()
                .for_each(|x| *x /= peak.max(1e-6));
        }
        Self { data, offsets }
    }

    /// The widest mip level whose partials stay alias-free at `hz`.
    #[inline]
    pub fn mip_for(hz: f32, fs: f32) -> usize {
        Self::mip_for_band(hz, 0.0, fs)
    }

    /// The crossfaded mip for `hz` with sidebands reaching `sidebands_hz`
    /// above every harmonic.
    ///
    /// With `m = log2(needed / available bandwidth)`, level `⌈m⌉` is the
    /// widest safe one. Over `m ∈ (k−1, k]` the oscillator blends from level
    /// k towards k+1, arriving at k+1 exactly where k stops being safe, so the
    /// weights are continuous in pitch.
    #[inline]
    pub fn mip_blend(hz: f32, sidebands_hz: f32, fs: f32) -> Mip {
        let room = (alias_free_limit(fs) - sidebands_hz).max(hz.max(1.0));
        let m = (MAX_HARMONICS as f32 * hz.max(1.0) / room).log2().max(-1.0);
        let level = m.floor() + 1.0;
        if level >= (MIPS - 1) as f32 {
            return Mip {
                level: MIPS - 1,
                mix: 0.0,
            };
        }
        Mip {
            level: level as usize,
            mix: m - m.floor(),
        }
    }

    /// As [`Tables::mip_for`], when every harmonic also carries sidebands
    /// reaching `sidebands_hz` above it (phase modulation).
    #[inline]
    pub fn mip_for_band(hz: f32, sidebands_hz: f32, fs: f32) -> usize {
        let room = (alias_free_limit(fs) - sidebands_hz).max(hz.max(1.0));
        let ratio = MAX_HARMONICS as f32 * hz.max(1.0) / room;
        if ratio <= 1.0 {
            0
        } else {
            (ratio.log2().ceil() as usize).min(MIPS - 1)
        }
    }

    #[inline]
    fn read_frame(&self, frame: usize, mip: usize, phase: f32) -> f32 {
        let len = len_of(mip);
        let x = phase * len as f32;
        let idx = (x as usize).min(len - 1);
        let frac = x - idx as f32;
        let t = &self.data[self.offsets[frame][mip] + idx..];
        t[0] + (t[1] - t[0]) * frac
    }

    /// Read through a crossfaded [`Mip`].
    #[inline]
    pub fn read_mip(&self, pos: f32, mip: Mip, phase: f32) -> f32 {
        let a = self.read(pos, mip.level, phase);
        if mip.mix <= 0.0 {
            return a;
        }
        let b = self.read(pos, mip.level + 1, phase);
        a + (b - a) * mip.mix
    }

    /// Read at `phase` in `[0, 1)`, crossfading between frames at `pos`
    /// (0 … `FRAMES - 1`).
    #[inline]
    pub fn read(&self, pos: f32, mip: usize, phase: f32) -> f32 {
        let pos = pos.clamp(0.0, (FRAMES - 1) as f32);
        let f0 = (pos as usize).min(FRAMES - 2);
        let mix = pos - f0 as f32;
        let a = self.read_frame(f0, mip, phase);
        if mix <= 0.0 {
            return a;
        }
        let b = self.read_frame(f0 + 1, mip, phase);
        a + (b - a) * mix
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_are_normalized_and_start_at_zero() {
        let t = tables();
        for frame in 0..FRAMES {
            let peak = |mip: usize| {
                (0..4096)
                    .map(|n| t.read(frame as f32, mip, n as f32 / 4096.0).abs())
                    .fold(0.0f32, f32::max)
            };
            assert!((peak(NORM_MIP) - 1.0).abs() < 1e-3, "frame {frame}");
            assert!(peak(0) < 2.0, "frame {frame}: {}", peak(0));
            for mip in 0..MIPS {
                assert!(t.read(frame as f32, mip, 0.0).abs() < 1e-3);
            }
        }
    }

    #[test]
    fn mip_selection_keeps_partials_below_the_limit() {
        let fs = 96_000.0;
        assert_eq!(Tables::mip_for(30.0, fs), 0);
        for hz in [50.0f32, 200.0, 1_000.0, 5_000.0] {
            let mip = Tables::mip_for(hz, fs);
            let harmonics = harmonics_in(mip) as f32;
            assert!(
                harmonics * hz <= alias_free_limit(fs) || mip == MIPS - 1,
                "{hz}"
            );
            // …and it is the widest such level.
            if mip > 0 {
                assert!(harmonics * 2.0 * hz > alias_free_limit(fs));
            }
        }
    }

    #[test]
    fn mip_blend_is_continuous_and_safe() {
        let fs = 96_000.0;
        let mut last = Tables::mip_blend(10.0, 0.0, fs);
        let mut hz = 10.0f32;
        while hz < 20_000.0 {
            let m = Tables::mip_blend(hz, 0.0, fs);
            // Both levels in use are alias-free…
            let top = harmonics_in(m.level) as f32 * hz;
            assert!(
                top <= alias_free_limit(fs) * 1.001 || m.level == MIPS - 1,
                "{hz}"
            );
            // …and the effective level moves smoothly.
            let eff = |m: Mip| m.level as f32 + m.mix;
            assert!((eff(m) - eff(last)).abs() < 0.01, "{hz}: {last:?} -> {m:?}");
            last = m;
            hz *= 1.001;
        }
    }

    #[test]
    fn narrow_mips_of_the_saw_are_smoother() {
        let t = tables();
        let roughness = |mip: usize| {
            (1..4096)
                .map(|n| {
                    let p = |n: usize| t.read(6.0, mip, n as f32 / 4096.0);
                    (p(n) - p(n - 1)).abs()
                })
                .fold(0.0f32, f32::max)
        };
        assert!(roughness(0) > 4.0 * roughness(6));
    }

    #[test]
    fn synthesis_matches_the_additive_definition() {
        let amps = [1.0, 0.0, 0.5, 0.25];
        let len = 64;
        let x = synthesize(&amps, len);
        for (n, &v) in x.iter().enumerate() {
            let want: f64 = amps
                .iter()
                .enumerate()
                .map(|(i, a)| {
                    a * (core::f64::consts::TAU * (i + 1) as f64 * n as f64 / len as f64).sin()
                })
                .sum();
            assert!((v as f64 - want).abs() < 1e-5, "{n}: {v} vs {want}");
        }
    }
}
