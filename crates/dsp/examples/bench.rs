//! Real-time factor of the engine with both voices hitting every engine
//! with all four oscillators, the whole channel strip, the echo and the
//! reverb, all six macros and the audio-rate cross-modulation both ways.
//!
//! ```sh
//! cargo run -p stabkussion-dsp --release --example bench
//! ```

use stabkussion_dsp::params::ENGINE_NAMES;
use stabkussion_dsp::{Engine, Param};
use std::time::Instant;

fn main() {
    let sr = 48_000.0;
    let seconds = 20.0;
    let mut worst = f64::MAX;
    for (engine, name) in ENGINE_NAMES.iter().enumerate() {
        let mut e = Engine::new(sr);
        for v in 0..2 {
            e.set_param(v, Param::Engine, engine as f32);
            e.set_param(v, Param::ReverbMix, 0.5);
            e.set_param(v, Param::Decay, 0.7);
            e.set_param(v, Param::MacroX, 0.6);
            e.set_param(v, Param::MacroY, 0.6);
            e.set_param(v, Param::MacroZ, 0.6);
            e.set_param(v, Param::MacroU, 0.6);
            e.set_param(v, Param::MacroV, 0.6);
            e.set_param(v, Param::MacroW, 0.6);
            e.set_param(v, Param::XFm, 0.3);
            e.set_param(v, Param::XRing, 0.3);
            e.set_param(v, Param::XFilterFm, 0.3);
            e.set_param(v, Param::XCrush, 0.3);
            // Worst case: four oscillators, the whole strip busy.
            e.set_param(v, Param::Level2, 1.0);
            e.set_param(v, Param::Level3, 1.0);
            e.set_param(v, Param::Level4, 1.0);
            e.set_param(v, Param::DelayMix, 0.5);
            e.set_param(v, Param::Drive, 0.5);
            e.set_param(v, Param::Reso, 0.5);
            e.set_param(v, Param::LowCut, 0.2);
            e.set_param(v, Param::LowGain, 0.7);
            e.set_param(v, Param::MidGain, 0.3);
            e.set_param(v, Param::HighGain, 0.6);
            e.set_param(v, Param::ChorusMix, 0.5);
        }
        e.snap_params();
        let mut l = [0.0f32; 128];
        let mut r = [0.0f32; 128];
        let blocks = (sr * seconds / 128.0) as usize;
        let start = Instant::now();
        for b in 0..blocks {
            if b % 150 == 0 {
                e.trigger(0, 1.0);
                e.trigger(1, 0.7);
            }
            e.render(&mut l, &mut r);
        }
        let rtf = seconds as f64 / start.elapsed().as_secs_f64();
        worst = worst.min(rtf);
        println!("{:<8} {rtf:>6.0}x real time (both channels)", name);
    }
    println!("worst case: {worst:.0}x");
}
