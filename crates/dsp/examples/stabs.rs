//! How stab-like is RND? Compares random hits with a folder of reference
//! stabs on the [`Features`] of `stabkussion_dsp::analysis`.
//!
//! ```sh
//! # The references' features, their spread, and how far each one is from
//! # its nearest neighbour:
//! cargo run -p stabkussion-dsp --release --example stabs -- survey examples
//! # N RND hits (seeded), each matched to its nearest reference:
//! cargo run -p stabkussion-dsp --release --example stabs -- rnd examples 400 [seed]
//! ```
//!
//! A hit "matches" when it is at least as close to some reference as the
//! loneliest reference is to its nearest neighbour.

// Feature tables are indexed in parallel.
#![allow(clippy::needless_range_loop)]

mod common;

use stabkussion_dsp::analysis::{FEATURE_NAMES, FEATURE_SCALES, Features};
use stabkussion_dsp::params::{ENGINE_COUNT, ENGINE_NAMES, default_patch};
use stabkussion_dsp::random::{NO_LOCKS, random_patch};
use stabkussion_dsp::util::Noise;
use stabkussion_dsp::{ALL_PARAMS, Param, VoicePatch};

const FS: f32 = 24_000.0;

pub fn references(dir: &str) -> Vec<(String, Features)> {
    let mut paths: Vec<_> = std::fs::read_dir(dir)
        .expect("reference folder")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "wav"))
        .collect();
    paths.sort();
    paths
        .iter()
        .map(|p| {
            let w = common::read_wav(p).expect("wav");
            let l = common::resample(&w.left, w.fs, FS);
            let r = common::resample(&w.right, w.fs, FS);
            (
                p.file_stem().unwrap().to_string_lossy().into_owned(),
                Features::measure(&l, &r, FS),
            )
        })
        .collect()
}

fn print_row(name: &str, f: &Features) {
    let v = [
        f.centroid,
        f.f10,
        f.f50,
        f.f90,
        f.attack,
        f.t20,
        f.darkening,
        f.width,
        f.flatness,
    ];
    print!("{name:<14}");
    for x in v {
        print!(" {x:>9.3}");
    }
    println!();
}

fn header() {
    print!("{:<14}", "");
    for n in FEATURE_NAMES {
        print!(" {n:>9}");
    }
    println!();
}

/// Each reference's distance to its nearest other reference.
fn nn_distances(refs: &[(String, Features)]) -> Vec<f32> {
    refs.iter()
        .enumerate()
        .map(|(i, (_, a))| {
            refs.iter()
                .enumerate()
                .filter(|(j, _)| *j != i)
                .map(|(_, (_, b))| a.distance(b))
                .fold(f32::MAX, f32::min)
        })
        .collect()
}

fn survey(dir: &str) {
    let refs = references(dir);
    header();
    for (name, f) in &refs {
        print_row(name, f);
    }
    // Spread of each feature in comparison units.
    let vecs: Vec<[f32; 9]> = refs.iter().map(|(_, f)| f.to_vec()).collect();
    print!("{:<14}", "std (cmp)");
    for k in 0..9 {
        let m = vecs.iter().map(|v| v[k]).sum::<f32>() / vecs.len() as f32;
        let s = (vecs.iter().map(|v| (v[k] - m).powi(2)).sum::<f32>() / vecs.len() as f32).sqrt();
        print!(" {s:>9.3}");
    }
    println!();
    print!("{:<14}", "scale used");
    for s in FEATURE_SCALES {
        print!(" {s:>9.3}");
    }
    println!();
    let nn = nn_distances(&refs);
    for ((name, _), d) in refs.iter().zip(&nn) {
        println!("{name:<14} nearest other reference at {d:.2}");
    }
    let max = nn.iter().cloned().fold(0.0, f32::max);
    let mut sorted = nn.clone();
    sorted.sort_by(f32::total_cmp);
    println!(
        "thresholds: match (loneliest reference) {max:.2}, close (median reference) {:.2}",
        sorted[sorted.len() / 2]
    );
}

fn rnd(dir: &str, n: usize, seed: u32) {
    let refs = references(dir);
    let nn = nn_distances(&refs);
    let threshold = nn.iter().cloned().fold(0.0, f32::max);
    let mut sorted = nn.clone();
    sorted.sort_by(f32::total_cmp);
    let close = sorted[sorted.len() / 2];
    let mut rng = Noise::new(seed);
    let mut matched = 0;
    let mut close_count = 0;
    let mut per_engine = [(0usize, 0usize); ENGINE_COUNT];
    let mut dists = Vec::new();
    let mut nearest_count = vec![0usize; refs.len()];
    let mut worst: Vec<(f32, usize, Features, String)> = Vec::new();
    let mut all = Vec::new();
    // For hits outside the threshold: which feature (relative to the
    // nearest reference) contributes most, and in which direction.
    let mut blame = [[0usize; 2]; 9];
    let mut by_engine: Vec<Vec<Features>> = vec![Vec::new(); ENGINE_COUNT];
    for i in 0..n {
        let patch = random_patch(&default_patch(), &NO_LOCKS, &mut rng);
        let (l, r) = common::render_hit(&patch, 1.0, FS, 2.5);
        let f = Features::measure(&l, &r, FS);
        let (j, d) = refs
            .iter()
            .enumerate()
            .map(|(j, (_, g))| (j, f.distance(g)))
            .fold((0, f32::MAX), |a, b| if b.1 < a.1 { b } else { a });
        nearest_count[j] += 1;
        let e = patch[Param::Engine.index()] as usize;
        per_engine[e].1 += 1;
        if d <= close {
            close_count += 1;
        }
        if d <= threshold {
            matched += 1;
            per_engine[e].0 += 1;
        } else {
            let (a, b) = (f.to_vec(), refs[j].1.to_vec());
            let k = (0..9)
                .max_by(|&x, &y| {
                    ((a[x] - b[x]) / FEATURE_SCALES[x])
                        .abs()
                        .total_cmp(&((a[y] - b[y]) / FEATURE_SCALES[y]).abs())
                })
                .unwrap();
            blame[k][(a[k] > b[k]) as usize] += 1;
            if std::env::var("STABS_DUMP").is_ok() {
                let q = |p: Param| patch[p.index()];
                println!(
                    "  #{i} d {d:.2} {} {}: {} {:.0} Hz osc {:.2}/{:+.1} {:.2}/{:+.1} {:.2}/{:+.1} dec {} gate {} cut {} env {} fdec {} key {:.2} xyz {:.2} {:.2} {:.2}",
                    FEATURE_NAMES[k],
                    if a[k] > b[k] { "high" } else { "low" },
                    ENGINE_NAMES[e],
                    stabkussion_dsp::params::map::pitch_hz(q(Param::Pitch), q(Param::Fine)),
                    q(Param::Level2),
                    stabkussion_dsp::params::map::tune_semitones(q(Param::Tune2)),
                    q(Param::Level3),
                    stabkussion_dsp::params::map::tune_semitones(q(Param::Tune3)),
                    q(Param::Level4),
                    stabkussion_dsp::params::map::tune_semitones(q(Param::Tune4)),
                    Param::Decay.display(q(Param::Decay)),
                    Param::Gate.display(q(Param::Gate)),
                    Param::Cutoff.display(q(Param::Cutoff)),
                    Param::FilterEnv.display(q(Param::FilterEnv)),
                    Param::FilterDecay.display(q(Param::FilterDecay)),
                    q(Param::KeyTrack),
                    q(Param::MacroX),
                    q(Param::MacroY),
                    q(Param::MacroZ),
                );
                print_row("", &f);
            }
        }
        dists.push(d);
        let desc = ALL_PARAMS
            .iter()
            .map(|p| format!("{:.3}", patch[p.index()]))
            .collect::<Vec<_>>()
            .join(",");
        worst.push((d, i, f, desc));
        all.push(f);
        by_engine[e].push(f);
    }
    dists.sort_by(f32::total_cmp);
    let pct = |q: f32| dists[((dists.len() - 1) as f32 * q) as usize];
    println!(
        "{matched}/{n} hits ({:.1}%) within {threshold:.2} of a reference; distance median {:.2}, p90 {:.2}, max {:.2}",
        100.0 * matched as f32 / n as f32,
        pct(0.5),
        pct(0.9),
        pct(1.0)
    );
    println!(
        "{close_count}/{n} hits ({:.1}%) within {close:.2} (as close as the median reference is to its neighbour)",
        100.0 * close_count as f32 / n as f32
    );
    for (e, (m, t)) in per_engine.iter().enumerate() {
        let med = |k: usize| {
            let mut v: Vec<f32> = by_engine[e].iter().map(|f| f.to_vec()[k]).collect();
            v.sort_by(f32::total_cmp);
            v.get(v.len() / 2).copied().unwrap_or(f32::NAN)
        };
        println!(
            "  {:<6} {m:>4}/{t:<4}  median centroid {:>4.0} Hz, f90 {:>4.0} Hz, t20 {:>3.0} ms, width {:.2}",
            ENGINE_NAMES[e],
            med(0).exp2(),
            med(3).exp2(),
            1000.0 * med(5).exp2(),
            med(7)
        );
    }
    print!("unmatched, worst feature (low/high):");
    for k in 0..9 {
        if blame[k] != [0, 0] {
            print!(" {} {}/{}", FEATURE_NAMES[k], blame[k][0], blame[k][1]);
        }
    }
    println!();
    println!("nearest reference counts:");
    for ((name, _), c) in refs.iter().zip(&nearest_count) {
        print!(" {name}:{c}");
    }
    println!();
    // Feature ranges: references vs RND.
    let range = |fs: &[Features], k: usize| {
        let mut v: Vec<f32> = fs.iter().map(|f| f.to_vec()[k]).collect();
        v.sort_by(f32::total_cmp);
        (
            v[0],
            v[v.len() / 10],
            v[v.len() / 2],
            v[v.len() * 9 / 10],
            v[v.len() - 1],
        )
    };
    let rf: Vec<Features> = refs.iter().map(|r| r.1).collect();
    println!("feature (cmp units)   refs min/p10/med/p90/max  |  rnd min/p10/med/p90/max");
    for k in 0..9 {
        let a = range(&rf, k);
        let b = range(&all, k);
        println!(
            "{:<10} {:>6.2} {:>6.2} {:>6.2} {:>6.2} {:>6.2} | {:>6.2} {:>6.2} {:>6.2} {:>6.2} {:>6.2}",
            FEATURE_NAMES[k], a.0, a.1, a.2, a.3, a.4, b.0, b.1, b.2, b.3, b.4
        );
    }
    worst.sort_by(|a, b| b.0.total_cmp(&a.0));
    println!("farthest hits:");
    for (d, i, f, desc) in worst.iter().take(8) {
        print!("#{i:<4} d {d:.2} ");
        print_row("", f);
        println!("      {desc}");
    }
}

/// Render hit `index` of RND seed `seed` to a WAV and print its patch and a
/// 10 ms RMS envelope.
fn hit(seed: u32, index: usize, out: &str) {
    let mut rng = Noise::new(seed);
    let mut patch = default_patch();
    for _ in 0..=index {
        patch = random_patch(&default_patch(), &NO_LOCKS, &mut rng);
    }
    let engine = patch[Param::Engine.index()] as u32;
    for p in ALL_PARAMS {
        println!(
            "{:<16} {:>8.4}  {}",
            format!("{p:?}"),
            patch[p.index()],
            p.display_for(engine, patch[p.index()])
        );
    }
    let (l, r) = common::render_hit(&patch, 1.0, FS, 2.0);
    common::write_wav(std::path::Path::new(out), FS as u32, &l, &r).unwrap();
    let hop = (0.01 * FS) as usize;
    let env: Vec<String> = l
        .chunks(hop)
        .take(80)
        .map(|c| {
            let rms = (c.iter().map(|x| x * x).sum::<f32>() / c.len() as f32).sqrt();
            format!("{:.0}", 20.0 * rms.max(1e-6).log10())
        })
        .collect();
    println!("10 ms RMS (dB): {}", env.join(" "));
    print_row("features", &Features::measure(&l, &r, FS));
}

/// Short-term loudness proxy: RMS (dB) of the mid over the loudest 300 ms
/// of a full-velocity hit.
fn loudness_db(patch: &VoicePatch) -> f32 {
    let (l, r) = common::render_hit(patch, 1.0, FS, 1.5);
    let w = (0.3 * FS) as usize;
    let mid: Vec<f32> = l.iter().zip(&r).map(|(a, b)| 0.5 * (a + b)).collect();
    let best = mid
        .windows(w)
        .step_by(w / 6)
        .map(|c| c.iter().map(|x| x * x).sum::<f32>() / w as f32)
        .fold(0.0, f32::max);
    10.0 * best.max(1e-12).log10()
}

/// Turn a folder of fitted patches (`NAME.fit.txt`) into preset entries for
/// `presets.rs`, level-matched to −22 dB short-term RMS. `names` maps each
/// file to a preset name (`NAME=Preset name` lines).
fn presets(dir: &str, names: Option<&str>) {
    let names: Vec<(String, String)> = names
        .map(|p| {
            std::fs::read_to_string(p)
                .unwrap()
                .lines()
                .filter_map(|l| l.split_once('='))
                .map(|(a, b)| (a.trim().to_string(), b.trim().to_string()))
                .collect()
        })
        .unwrap_or_default();
    let mut paths: Vec<_> = std::fs::read_dir(dir)
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.to_string_lossy().ends_with(".fit.txt"))
        .collect();
    paths.sort();
    let defaults = default_patch();
    for path in paths {
        let stem = path
            .file_name()
            .unwrap()
            .to_string_lossy()
            .replace(".fit.txt", "");
        let name = names
            .iter()
            .find(|(k, _)| *k == stem)
            .map_or(stem.clone(), |(_, v)| v.clone());
        let mut patch = defaults;
        for line in std::fs::read_to_string(&path).unwrap().lines() {
            let mut it = line.split_whitespace();
            let (Some(k), Some(v)) = (it.next(), it.next()) else {
                continue;
            };
            if let Some(p) = ALL_PARAMS.iter().find(|p| format!("{p:?}") == k) {
                patch[p.index()] = p.sanitize(v.parse().unwrap());
            }
        }
        // Level: −22 dB, by the level knob's square law.
        let lvl = patch[Param::Level.index()];
        let db = loudness_db(&patch);
        let gain = 10f32.powf((-22.0 - db) / 20.0);
        patch[Param::Level.index()] = (lvl * gain.sqrt()).clamp(0.05, 1.0);
        println!("    // Fitted to {stem}.");
        println!("    Preset {{");
        println!("        name: {name:?},");
        println!("        values: &[");
        for p in ALL_PARAMS {
            let v = (patch[p.index()] * 1000.0).round() / 1000.0;
            if (v - defaults[p.index()]).abs() > 0.0005 || p == Param::Engine {
                match p.info().kind {
                    stabkussion_dsp::params::ParamKind::Choice(_) => {
                        println!("            ({p:?}, {:.1}),", v)
                    }
                    _ => println!("            ({p:?}, {v:?}),"),
                }
            }
        }
        println!("        ],");
        println!("    }},");
    }
}

/// What spectrogram distances mean: each reference against itself played
/// 25 cents and a semitone sharp (resampled, so also a little shorter).
fn yardstick(dir: &str) {
    use stabkussion_dsp::analysis::Gram;
    let mut paths: Vec<_> = std::fs::read_dir(dir)
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "wav"))
        .collect();
    paths.sort();
    let mut sums = [0.0f32; 2];
    for p in &paths {
        let w = common::read_wav(p).unwrap();
        let cap = (1.6 * FS) as usize;
        let mut l = common::resample(&w.left, w.fs, FS);
        let mut r = common::resample(&w.right, w.fs, FS);
        l.truncate(cap);
        r.truncate(cap);
        let g = Gram::new(&l, &r, FS);
        let mut row = Vec::new();
        for (k, cents) in [25.0f32, 100.0].iter().enumerate() {
            let ratio = (cents / 1200.0).exp2();
            let ls = common::resample(&l, FS * ratio, FS);
            let rs = common::resample(&r, FS * ratio, FS);
            let d = Gram::new(&ls, &rs, FS).distance(&g);
            sums[k] += d;
            row.push(format!("{d:.2}"));
        }
        println!(
            "{:<12} +25 ct {}  +1 st {}",
            p.file_stem().unwrap().to_string_lossy(),
            row[0],
            row[1]
        );
    }
    let n = paths.len() as f32;
    println!(
        "mean: +25 ct {:.2} dB, +1 st {:.2} dB",
        sums[0] / n,
        sums[1] / n
    );
}

/// Which draws drive the misses? For every parameter, the miss rate of RND
/// hits by quintile of its knob value (choices by value), sorted by spread.
fn sensitivity(dir: &str, n: usize) {
    let refs: Vec<Features> = references(dir).into_iter().map(|r| r.1).collect();
    let threshold = nn_distances_f(&refs).into_iter().fold(0.0, f32::max);
    let results = std::sync::Mutex::new(Vec::new());
    let threads = std::thread::available_parallelism().map_or(4, |t| t.get());
    std::thread::scope(|sc| {
        for t in 0..threads {
            let (refs, results) = (&refs, &results);
            sc.spawn(move || {
                let mut rng = Noise::new(9_000 + t as u32);
                for _ in 0..n / threads {
                    let patch = random_patch(&default_patch(), &NO_LOCKS, &mut rng);
                    let (l, r) = common::render_hit(&patch, 1.0, FS, 2.5);
                    let f = Features::measure(&l, &r, FS);
                    let d = refs.iter().map(|g| f.distance(g)).fold(f32::MAX, f32::min);
                    results.lock().unwrap().push((patch, d > threshold));
                }
            });
        }
    });
    let results = results.into_inner().unwrap();
    let total_miss = results.iter().filter(|r| r.1).count();
    println!(
        "{} hits, {} misses ({:.1}%)",
        results.len(),
        total_miss,
        100.0 * total_miss as f32 / results.len() as f32
    );
    let mut rows = Vec::new();
    for p in ALL_PARAMS {
        if p.is_performance() {
            continue;
        }
        let mut vals: Vec<(f32, bool)> = results.iter().map(|(q, m)| (q[p.index()], *m)).collect();
        vals.sort_by(|a, b| a.0.total_cmp(&b.0));
        let k = 5;
        let rates: Vec<(f32, f32)> = (0..k)
            .map(|i| {
                let c = &vals[i * vals.len() / k..(i + 1) * vals.len() / k];
                let miss = c.iter().filter(|v| v.1).count() as f32 / c.len().max(1) as f32;
                (c[c.len() / 2].0, 100.0 * miss)
            })
            .collect();
        let spread = rates.iter().map(|r| r.1).fold(0.0, f32::max)
            - rates.iter().map(|r| r.1).fold(f32::MAX, f32::min);
        rows.push((spread, p, rates));
    }
    rows.sort_by(|a, b| b.0.total_cmp(&a.0));
    for (spread, p, rates) in rows.iter().take(20) {
        print!("{:<14} spread {spread:>4.1}%:", format!("{p:?}"));
        for (v, r) in rates {
            print!("  {v:.2}→{r:.1}%");
        }
        println!();
    }
}

fn nn_distances_f(refs: &[Features]) -> Vec<f32> {
    stabkussion_dsp::analysis::nearest_neighbour_distances(refs)
}

/// How well do the shipped presets reproduce the references they were
/// fitted to? `names` maps reference names to preset names (`NAME=Preset`).
fn fits(dir: &str, names: &str) {
    use stabkussion_dsp::analysis::Gram;
    use stabkussion_dsp::presets::PRESETS;
    let mut rows = Vec::new();
    let loaded: Vec<(String, Vec<f32>, Vec<f32>)> = {
        let mut paths: Vec<_> = std::fs::read_dir(dir)
            .unwrap()
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().is_some_and(|x| x == "wav"))
            .collect();
        paths.sort();
        paths
            .iter()
            .map(|p| {
                let w = common::read_wav(p).unwrap();
                let cap = (1.6 * FS) as usize;
                let mut l = common::resample(&w.left, w.fs, FS);
                let mut r = common::resample(&w.right, w.fs, FS);
                l.truncate(cap);
                r.truncate(cap);
                (p.file_stem().unwrap().to_string_lossy().into_owned(), l, r)
            })
            .collect()
    };
    for line in std::fs::read_to_string(names).unwrap().lines() {
        let Some((reference, preset)) = line.split_once('=') else {
            continue;
        };
        let Some(i) = loaded.iter().position(|x| x.0 == reference.trim()) else {
            continue;
        };
        let Some(p) = PRESETS.iter().find(|p| p.name == preset.trim()) else {
            continue;
        };
        let (_, l, r) = &loaded[i];
        let g = Gram::new(l, r, FS);
        let f = Features::measure(l, r, FS);
        let (pl, pr) = common::render_hit(&p.patch(), 1.0, FS, l.len() as f32 / FS);
        let d = Gram::new(&pl, &pr, FS).distance(&g);
        let fd = Features::measure(&pl, &pr, FS).distance(&f);
        let nearest = loaded
            .iter()
            .enumerate()
            .filter(|(j, _)| *j != i)
            .map(|(_, (_, ol, or))| {
                let mut ol = ol.clone();
                let mut or = or.clone();
                ol.resize(l.len(), 0.0);
                or.resize(l.len(), 0.0);
                Gram::new(&ol, &or, FS).distance(&g)
            })
            .fold(f32::MAX, f32::min);
        println!(
            "| {} | {} | {} | {d:.1} | {nearest:.1} | {fd:.2} |",
            reference.trim(),
            p.name,
            ENGINE_NAMES[p.patch()[Param::Engine.index()] as usize]
        );
        rows.push((d, nearest, fd));
    }
    let n = rows.len() as f32;
    println!(
        "mean: preset {:.2} dB, nearest other reference {:.2} dB, features {:.2}; closer than the nearest other reference: {}/{}",
        rows.iter().map(|r| r.0).sum::<f32>() / n,
        rows.iter().map(|r| r.1).sum::<f32>() / n,
        rows.iter().map(|r| r.2).sum::<f32>() / n,
        rows.iter().filter(|r| r.0 < r.1).count(),
        rows.len()
    );
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("survey") => survey(&args[2]),
        Some("rnd") => rnd(
            &args[2],
            args.get(3).and_then(|s| s.parse().ok()).unwrap_or(400),
            args.get(4).and_then(|s| s.parse().ok()).unwrap_or(1),
        ),
        Some("rust") => {
            // The references' features as a Rust table for
            // `analysis::REFERENCES`.
            for (name, f) in references(&args[2]) {
                println!(
                    "    // {name}\n    Features {{ centroid: {:.1}, f10: {:.1}, f50: {:.1}, f90: {:.1}, attack: {:.3}, t20: {:.3}, darkening: {:.3}, width: {:.3}, flatness: {:.4} }},",
                    f.centroid,
                    f.f10,
                    f.f50,
                    f.f90,
                    f.attack,
                    f.t20,
                    f.darkening,
                    f.width,
                    f.flatness
                );
            }
        }
        Some("batch") => {
            // N RND hits of a seed as WAVs, for listening.
            let seed: u32 = args[2].parse().unwrap();
            let n: usize = args[3].parse().unwrap();
            let dir = std::path::Path::new(&args[4]);
            std::fs::create_dir_all(dir).unwrap();
            let mut rng = Noise::new(seed);
            for i in 0..n {
                let patch = random_patch(&default_patch(), &NO_LOCKS, &mut rng);
                let (l, r) = common::render_hit(&patch, 1.0, 48_000.0, 1.5);
                let name = format!(
                    "rnd-{seed}-{i:03}-{}.wav",
                    ENGINE_NAMES[patch[Param::Engine.index()] as usize]
                );
                common::write_wav(&dir.join(name), 48_000, &l, &r).unwrap();
            }
        }
        Some("presets") => presets(&args[2], args.get(3).map(String::as_str)),
        Some("yardstick") => yardstick(&args[2]),
        Some("fits") => fits(&args[2], &args[3]),
        Some("sensitivity") => sensitivity(
            &args[2],
            args.get(3).and_then(|s| s.parse().ok()).unwrap_or(3000),
        ),
        Some("hit") => hit(args[2].parse().unwrap(), args[3].parse().unwrap(), &args[4]),
        _ => eprintln!(
            "usage: stabs survey DIR | stabs rnd DIR [N] [SEED] | stabs hit SEED INDEX OUT.wav"
        ),
    }
}
