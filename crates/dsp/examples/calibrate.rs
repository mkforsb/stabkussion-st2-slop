//! Loudness survey: peak and RMS (first 200 ms) of one full-velocity hit per
//! engine across pitch, click and macro settings, dry, at unity level.
//!
//! ```sh
//! cargo run -p stabkussion-dsp --release --example calibrate
//! ```

use stabkussion_dsp::Param;
use stabkussion_dsp::params::ENGINE_NAMES;
use stabkussion_dsp::voice::Voice;

fn main() {
    let fs = 96_000.0;
    println!(
        "{:<8} {:>7} {:>7} {:>7} {:>7} {:>7}",
        "engine", "pk min", "pk med", "pk max", "rms med", "rms max"
    );
    for (engine, name) in ENGINE_NAMES.iter().enumerate() {
        let mut peaks = Vec::new();
        let mut rmss = Vec::new();
        for pitch in [0.1f32, 0.3, 0.5, 0.7, 0.9] {
            for click in [0.0f32, 0.5, 1.0] {
                for m in [0.0f32, 0.5, 1.0] {
                    for noise in [0.0f32, 0.5] {
                        let mut v = Voice::new(fs, 1);
                        v.set_param(Param::Engine, engine as f32);
                        v.set_param(Param::Pitch, pitch);
                        v.set_param(Param::Click, click);
                        v.set_param(Param::Noise, noise);
                        v.set_param(Param::MacroX, m);
                        v.set_param(Param::MacroY, 1.0 - m);
                        v.set_param(Param::MacroZ, m);
                        v.set_param(Param::Level, 1.0);
                        v.set_param(Param::Sense, 0.5);
                        v.snap_params();
                        v.trigger(1.0);
                        let out: Vec<f32> = (0..(fs * 0.2) as usize).map(|_| v.tick()).collect();
                        peaks.push(out.iter().fold(0.0f32, |a, &b| a.max(b.abs())));
                        rmss.push(
                            (out.iter().map(|x| x * x).sum::<f32>() / out.len() as f32).sqrt(),
                        );
                    }
                }
            }
        }
        peaks.sort_by(f32::total_cmp);
        rmss.sort_by(f32::total_cmp);
        let med = |v: &[f32]| v[v.len() / 2];
        println!(
            "{:<8} {:>7.3} {:>7.3} {:>7.3} {:>7.3} {:>7.3}",
            name,
            peaks[0],
            med(&peaks),
            peaks[peaks.len() - 1],
            med(&rmss),
            rmss[rmss.len() - 1]
        );
    }
}
