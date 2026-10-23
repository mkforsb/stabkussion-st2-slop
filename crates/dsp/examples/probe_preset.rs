//! Features of one preset against its nearest reference (a tuning aid).
//!
//! ```sh
//! cargo run -p stabkussion-dsp --release --example probe_preset -- "Preset Name"
//! ```
use stabkussion_dsp::analysis::{FEATURE_NAMES, Features, REFERENCES, stab_threshold};
use stabkussion_dsp::{Engine, presets::PRESETS};

fn main() {
    let name = std::env::args().nth(1).unwrap_or_else(|| "Init".into());
    let p = PRESETS
        .iter()
        .find(|p| p.name == name)
        .expect("no such preset");
    let fs = 24_000.0;
    let mut e = Engine::new(fs);
    e.load_patch(0, &p.patch());
    e.snap_params();
    e.trigger(0, 1.0);
    let (l, r) = e.render_vec((fs * 2.5) as usize);
    let f = Features::measure(&l, &r, fs);
    let (i, d) = REFERENCES
        .iter()
        .enumerate()
        .map(|(i, q)| (i, f.distance(q)))
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .unwrap();
    println!(
        "{name}: {d:.2} from sample-{:03} (threshold {:.2})",
        i + 1,
        stab_threshold(&REFERENCES)
    );
    for (k, n) in FEATURE_NAMES.iter().enumerate() {
        println!(
            "  {n:>12} {:>9.3} ref {:>9.3}",
            f.to_vec()[k],
            REFERENCES[i].to_vec()[k]
        );
    }
}
