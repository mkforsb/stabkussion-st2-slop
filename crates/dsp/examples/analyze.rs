//! Time-series analysis of one hit of a preset: per window RMS (dB), an
//! autocorrelation pitch estimate and a brightness proxy (the "centroid" of
//! the first difference, ≈ fs·rms(Δx)/(2π·rms(x))).
//!
//! ```sh
//! cargo run -p stabkussion-dsp --release --example analyze -- "Init"
//! ```

use stabkussion_dsp::voice::Voice;
use stabkussion_dsp::{ALL_PARAMS, presets::PRESETS};

fn pitch(x: &[f32], fs: f32) -> Option<f32> {
    let n = x.len() / 2;
    let r = |lag: usize| (0..n).map(|i| x[i] * x[i + lag]).sum::<f32>();
    let r0 = r(0);
    if r0 <= 1e-9 {
        return None;
    }
    let lo = (fs / 2000.0) as usize;
    let hi = ((fs / 15.0) as usize).min(n - 2);
    // First lag after the first dip whose correlation is within 90% of the best.
    let vals: Vec<f32> = (lo..=hi).map(r).collect();
    let best = vals.iter().cloned().fold(f32::MIN, f32::max);
    if best < 0.3 * r0 {
        return None;
    }
    let first_dip = vals.iter().position(|&v| v < 0.5 * r0)?;
    let i = (first_dip..vals.len()).find(|&i| vals[i] > 0.9 * best)?;
    Some(fs / (lo + i) as f32)
}

fn main() {
    let name = std::env::args().nth(1).expect("preset name");
    let preset = PRESETS
        .iter()
        .find(|p| p.name == name)
        .expect("no such preset");
    let fs = 48_000.0;
    let mut v = Voice::new(fs, 1);
    let patch = preset.patch();
    for p in ALL_PARAMS {
        v.set_param(p, patch[p.index()]);
    }
    v.snap_params();
    v.trigger(1.0);
    let out: Vec<f32> = (0..(fs * 1.2) as usize).map(|_| v.tick()).collect();
    let win = (fs * 0.08) as usize;
    let dc = out.iter().sum::<f32>() / out.len() as f32;
    println!("{name}: dc {dc:+.5}");
    println!(
        "{:>6} {:>7} {:>8} {:>9}",
        "t(ms)", "rms dB", "pitch", "bright Hz"
    );
    for (i, w) in out.chunks_exact(win).enumerate() {
        let rms = (w.iter().map(|x| x * x).sum::<f32>() / w.len() as f32).sqrt();
        if rms < 1e-4 {
            println!("{:>6} {:>7}", i * 80, "-");
            continue;
        }
        let d = (w.windows(2).map(|p| (p[1] - p[0]).powi(2)).sum::<f32>() / w.len() as f32).sqrt();
        let bright = fs * d / (core::f32::consts::TAU * rms);
        let p = pitch(w, fs)
            .map(|p| format!("{p:.0}"))
            .unwrap_or("-".into());
        println!(
            "{:>6} {:>7.1} {:>8} {:>9.0}",
            i * 80,
            20.0 * rms.log10(),
            p,
            bright
        );
    }
}
