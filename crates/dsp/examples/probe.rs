//! Which parameter makes a patch quiet? Prints the hit's peak with each
//! parameter reset to its default in turn.
//!
//! ```sh
//! cargo run -p stabkussion-dsp --release --example probe -- 0.0,0.75,...
//! ```
use stabkussion_dsp::voice::Voice;
use stabkussion_dsp::{ALL_PARAMS, PARAM_COUNT};

fn peak(patch: &[f32; PARAM_COUNT]) -> f32 {
    let fs = 48_000.0;
    let mut v = Voice::new(fs, 0);
    for p in ALL_PARAMS {
        v.set_param(p, patch[p.index()]);
    }
    v.snap_params();
    v.trigger(0.9);
    (0..(fs * 0.3) as usize)
        .map(|_| v.tick())
        .fold(0.0f32, |a, b| a.max(b.abs()))
}

fn main() {
    let arg = std::env::args().nth(1).expect("comma separated patch");
    let vals: Vec<f32> = arg.split(',').map(|s| s.trim().parse().unwrap()).collect();
    let patch: [f32; PARAM_COUNT] = vals.try_into().expect("one value per parameter");
    println!("as given: {:.5}", peak(&patch));
    for p in ALL_PARAMS {
        let mut q = patch;
        q[p.index()] = p.default_value();
        println!(
            "{:<16} {:>8.4} -> default: {:.5}",
            format!("{p:?}"),
            patch[p.index()],
            peak(&q)
        );
    }
}
