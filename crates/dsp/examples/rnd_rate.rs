//! How often is an RND hit a stab? Like `stabs rnd`, but against the
//! reference features compiled into the crate (`analysis::REFERENCES`), so
//! it needs no WAVs. Prints the match rate per engine, for RND as shipped
//! and with ST-2's U, V and W held at zero (the ST-1's sound space).
//!
//! ```sh
//! cargo run -p stabkussion-dsp --release --example rnd_rate -- [hits] [seed]
//! ```

use stabkussion_dsp::analysis::{Features, REFERENCES, nearest_reference, stab_threshold};
use stabkussion_dsp::params::{ENGINE_COUNT, ENGINE_NAMES, default_patch};
use stabkussion_dsp::random::{NO_LOCKS, random_patch};
use stabkussion_dsp::util::Noise;
use stabkussion_dsp::{Engine, Param};

fn main() {
    let mut args = std::env::args().skip(1);
    let hits: usize = args.next().and_then(|s| s.parse().ok()).unwrap_or(2_000);
    let seed: u32 = args.next().and_then(|s| s.parse().ok()).unwrap_or(1);
    let fs = 24_000.0;
    let threshold = stab_threshold(&REFERENCES);
    let mut rng = Noise::new(seed);
    let patches: Vec<_> = (0..hits)
        .map(|_| random_patch(&default_patch(), &NO_LOCKS, &mut rng))
        .collect();
    for (label, uvw_off) in [("RND", false), ("RND, U/V/W off", true)] {
        let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
        let chunks: Vec<Vec<_>> = patches
            .chunks(hits.div_ceil(threads))
            .map(|c| c.to_vec())
            .collect();
        let results: Vec<(usize, bool)> = std::thread::scope(|s| {
            let handles: Vec<_> = chunks
                .into_iter()
                .map(|chunk| {
                    s.spawn(move || {
                        chunk
                            .into_iter()
                            .map(|mut patch| {
                                if uvw_off {
                                    for m in [Param::MacroU, Param::MacroV, Param::MacroW] {
                                        patch[m.index()] = 0.0;
                                    }
                                }
                                let mut e = Engine::new(fs);
                                e.load_patch(0, &patch);
                                e.snap_params();
                                e.trigger(0, 1.0);
                                let (l, r) = e.render_vec((fs * 2.5) as usize);
                                let d =
                                    nearest_reference(&Features::measure(&l, &r, fs), &REFERENCES);
                                (patch[Param::Engine.index()] as usize, d <= threshold)
                            })
                            .collect::<Vec<_>>()
                    })
                })
                .collect();
            handles
                .into_iter()
                .flat_map(|h| h.join().unwrap())
                .collect()
        });
        let total = results.iter().filter(|r| r.1).count();
        println!(
            "{label}: {total}/{hits} = {:.1}% stabs",
            100.0 * total as f32 / hits as f32
        );
        for (e, name) in ENGINE_NAMES.iter().enumerate().take(ENGINE_COUNT) {
            let of: Vec<_> = results.iter().filter(|r| r.0 == e).collect();
            let ok = of.iter().filter(|r| r.1).count();
            println!(
                "  {:<6} {:>5.1}% of {}",
                name,
                100.0 * ok as f32 / of.len().max(1) as f32,
                of.len()
            );
        }
    }
}
