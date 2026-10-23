//! Fit a patch to each reference stab: search the panel for the settings
//! whose hit's spectrogram is closest to the reference's.
//!
//! ```sh
//! cargo run -p stabkussion-dsp --release --example fit -- examples fits [names…]
//! ```
//!
//! For every `*.wav` in the folder (or just the named ones) it:
//!
//! 1. picks the reference's strongest partials, and seeds a few thousand
//!    random patches (all engines) whose OSC 1 sits on one of them (or an
//!    octave or a twelfth below) and whose OSC 2–4 often sit on others: a
//!    chord stab's partials are oscillators;
//! 2. refines the best few seeds (at most two per engine) with a sparse
//!    (1+1) evolution strategy, then polishes the winner;
//!    The score is the spectrogram distance of [`Gram::distance`] (dB) plus
//!    [`FEATURE_WEIGHT`] × the feature distance of [`Features::distance`],
//!    so the fit gets the width, length and darkening right too. With
//!    `--from DIR`, a previous run's `NAME.fit.txt` joins the finalists;
//! 3. writes `NAME.fit.wav` (the reference, then the fit), `NAME.fit.txt`
//!    (the patch, as `Param value` lines) and prints the distance: the
//!    spectrogram distance of [`Gram::distance`] (dB), next to the distance
//!    from the reference to the nearest *other* reference as a yardstick,
//!    and the feature distance.

mod common;

use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use stabkussion_dsp::analysis::{Features, Gram};
use stabkussion_dsp::params::{ENGINE_COUNT, ENGINE_NAMES, ParamKind, default_patch, map};
use stabkussion_dsp::random::{NO_LOCKS, random_patch};
use stabkussion_dsp::util::Noise;
use stabkussion_dsp::{ALL_PARAMS, Param, VoicePatch};

const FS: f32 = 24_000.0;
const SEEDS: usize = 2_000;
const FINALISTS: usize = 8;
const REFINE_STEPS: usize = 500;
const POLISH_STEPS: usize = 2_500;
const MAX_SECONDS: f32 = 1.6;
/// dB of spectrogram distance per unit of feature distance.
const FEATURE_WEIGHT: f32 = 2.0;

struct Reference {
    name: String,
    left: Vec<f32>,
    right: Vec<f32>,
    gram: Gram,
    features: Features,
    peaks: Vec<f32>,
}

fn load(path: &std::path::Path) -> Reference {
    let w = common::read_wav(path).expect("wav");
    let mut left = common::resample(&w.left, w.fs, FS);
    let mut right = common::resample(&w.right, w.fs, FS);
    left.truncate((MAX_SECONDS * FS) as usize);
    right.truncate((MAX_SECONDS * FS) as usize);
    let gram = Gram::new(&left, &right, FS);
    let features = Features::measure(&left, &right, FS);
    let peaks = partials(&left, &right, features.attack);
    Reference {
        name: path.file_stem().unwrap().to_string_lossy().into_owned(),
        left,
        right,
        gram,
        features,
        peaks,
    }
}

/// The strongest spectral peaks (Hz) of the body, 25 Hz – 2 kHz, loudest
/// first: local maxima within 30 dB of the loudest.
fn partials(left: &[f32], right: &[f32], peak_s: f32) -> Vec<f32> {
    let a = ((peak_s + 0.03) * FS) as usize;
    let b = (a + (0.35 * FS) as usize).min(left.len());
    if b < a + 1024 {
        return vec![100.0];
    }
    let n = 1 << 15;
    let mut re = vec![0.0f64; n];
    let mut im = vec![0.0f64; n];
    for i in a..b {
        let t = (i - a) as f64 / (b - a) as f64;
        let w = 0.5 - 0.5 * (core::f64::consts::TAU * t).cos();
        re[i - a] = 0.5 * (left[i] + right[i]) as f64 * w;
    }
    stabkussion_dsp::util::fft(&mut re, &mut im, false);
    let df = FS as f64 / n as f64;
    let mag: Vec<f64> = (0..n / 2)
        .map(|k| (re[k] * re[k] + im[k] * im[k]).sqrt())
        .collect();
    let lo = (25.0 / df) as usize;
    let hi = (2_000.0 / df) as usize;
    let max = mag[lo..hi].iter().cloned().fold(0.0, f64::max);
    let reach = (6.0 / df) as usize;
    let mut peaks: Vec<(f64, f32)> = (lo..hi)
        .filter(|&k| {
            mag[k] > max * 0.03
                && (k.saturating_sub(reach)..=k + reach).all(|j| j == k || mag[j] <= mag[k])
        })
        .map(|k| (mag[k], (k as f64 * df) as f32))
        .collect();
    peaks.sort_by(|x, y| y.0.total_cmp(&x.0));
    peaks.truncate(8);
    peaks.into_iter().map(|p| p.1).collect()
}

fn render(patch: &VoicePatch, seconds: f32) -> (Vec<f32>, Vec<f32>) {
    common::render_hit(patch, 1.0, FS, seconds)
}

fn score(patch: &VoicePatch, r: &Reference) -> f32 {
    let secs = r.left.len() as f32 / FS;
    let (l, rr) = render(patch, secs);
    let peak = l.iter().chain(&rr).fold(0.0f32, |a, &b| a.max(b.abs()));
    if !peak.is_finite() || peak < 1e-4 {
        return 1e3;
    }
    Gram::new(&l, &rr, FS).distance(&r.gram)
        + FEATURE_WEIGHT * Features::measure(&l, &rr, FS).distance(&r.features)
}

/// A patch from a `Param value` text file.
fn read_patch(path: &std::path::Path) -> Option<VoicePatch> {
    let text = std::fs::read_to_string(path).ok()?;
    let mut p = default_patch();
    for line in text.lines() {
        let mut it = line.split_whitespace();
        let (Some(name), Some(v)) = (it.next(), it.next()) else {
            continue;
        };
        if let (Some(param), Ok(v)) = (
            ALL_PARAMS.iter().find(|q| format!("{q:?}") == name),
            v.parse::<f32>(),
        ) {
            p[param.index()] = param.sanitize(v);
        }
    }
    Some(p)
}

/// Parameters the search may move.
fn searchable(p: Param) -> bool {
    !p.is_performance() && p != Param::Engine && p != Param::Jitter
}

fn seed_patch(r: &Reference, engine: u32, rng: &mut Noise) -> VoicePatch {
    let mut p = random_patch(&default_patch(), &NO_LOCKS, rng);
    p[Param::Engine.index()] = engine as f32;
    // Re-roll the macros for this engine.
    let mut q = default_patch();
    q[Param::Engine.index()] = engine as f32;
    let mut locks = NO_LOCKS;
    locks[Param::Engine.index()] = true;
    let m = random_patch(&q, &locks, rng);
    for mp in [Param::MacroX, Param::MacroY, Param::MacroZ] {
        p[mp.index()] = m[mp.index()];
    }
    p[Param::Jitter.index()] = 0.0;
    let pick =
        |rng: &mut Noise| r.peaks[(rng.uniform() * r.peaks.len() as f32) as usize % r.peaks.len()];
    let div = [1.0, 1.0, 1.0, 2.0, 3.0][(rng.uniform() * 5.0) as usize % 5];
    let root = (pick(rng) / div).max(18.0);
    p[Param::Pitch.index()] = map::pitch_knob(root).clamp(0.0, 1.0);
    p[Param::Fine.index()] = 0.5;
    for (tune, level) in [
        (Param::Tune2, Param::Level2),
        (Param::Tune3, Param::Level3),
        (Param::Tune4, Param::Level4),
    ] {
        if rng.uniform() < 0.6 {
            let st = 12.0 * (pick(rng) / root).log2();
            if st.abs() <= map::TUNE_RANGE_ST {
                p[tune.index()] = map::tune_knob(st);
                p[level.index()] = 0.5 + 0.5 * rng.uniform();
            }
        }
    }
    p
}

/// Sparse (1+1)-ES: perturb a few parameters at a time, keep improvements,
/// adapt the step size by the 1/5 rule.
fn refine(
    start: VoicePatch,
    start_score: f32,
    r: &Reference,
    steps: usize,
    rng: &mut Noise,
) -> (VoicePatch, f32) {
    let params: Vec<Param> = ALL_PARAMS
        .iter()
        .copied()
        .filter(|&p| searchable(p))
        .collect();
    let (mut best, mut best_score) = (start, start_score);
    let mut sigma = 0.08f32;
    let mut successes = 0;
    for step in 1..=steps {
        let mut cand = best;
        let k = 1 + (rng.uniform() * 4.0) as usize;
        for _ in 0..k {
            let p = params[(rng.uniform() * params.len() as f32) as usize % params.len()];
            let i = p.index();
            cand[i] = match p.info().kind {
                ParamKind::Choice(names) => {
                    if rng.uniform() < 0.3 {
                        (rng.uniform() * names.len() as f32).floor()
                    } else {
                        cand[i]
                    }
                }
                ParamKind::Continuous => {
                    // Pitch and tunings move in finer steps: a semitone is
                    // 1/72 of PITCH and 1/48 of TUNE.
                    let s = match p {
                        Param::Pitch => sigma * 0.15,
                        Param::Tune2 | Param::Tune3 | Param::Tune4 | Param::Fine => sigma * 0.25,
                        _ => sigma,
                    };
                    let g = rng.sample() + rng.sample() + rng.sample();
                    cand[i] + s * g
                }
            };
            cand[i] = p.sanitize(cand[i]);
        }
        let s = score(&cand, r);
        if s < best_score {
            best = cand;
            best_score = s;
            successes += 1;
        }
        if step % 20 == 0 {
            sigma *= if successes > 4 { 1.5 } else { 0.8 };
            sigma = sigma.clamp(0.005, 0.3);
            successes = 0;
        }
    }
    (best, best_score)
}

fn fit(r: &Reference, seed: u32, previous: Option<VoicePatch>) -> (VoicePatch, f32) {
    let mut rng = Noise::new(seed);
    let mut seeds: Vec<(f32, VoicePatch)> = (0..SEEDS)
        .map(|i| {
            let p = seed_patch(r, (i % ENGINE_COUNT) as u32, &mut rng);
            (score(&p, r), p)
        })
        .collect();
    seeds.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut finalists = Vec::new();
    let mut per_engine = [0usize; ENGINE_COUNT];
    for (s, p) in seeds {
        let e = p[Param::Engine.index()] as usize;
        if per_engine[e] < 2 {
            per_engine[e] += 1;
            finalists.push((s, p));
        }
        if finalists.len() == FINALISTS {
            break;
        }
    }
    if let Some(p) = previous {
        finalists.push((score(&p, r), p));
    }
    let mut best = (default_patch(), f32::MAX);
    for (s, p) in finalists {
        let (q, t) = refine(p, s, r, REFINE_STEPS, &mut rng);
        if t < best.1 {
            best = (q, t);
        }
    }
    refine(best.0, best.1, r, POLISH_STEPS, &mut rng)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let dir = args.get(1).expect("reference folder");
    let out = std::path::PathBuf::from(args.get(2).expect("output folder"));
    std::fs::create_dir_all(&out).unwrap();
    let mut from = None;
    let mut only: Vec<&String> = Vec::new();
    let mut rest = args.iter().skip(3);
    while let Some(a) = rest.next() {
        if a == "--from" {
            from = rest.next().map(std::path::PathBuf::from);
        } else {
            only.push(a);
        }
    }
    let mut paths: Vec<_> = std::fs::read_dir(dir)
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "wav"))
        .collect();
    paths.sort();
    let refs: Vec<Reference> = paths.iter().map(|p| load(p)).collect();
    let todo: Vec<usize> = (0..refs.len())
        .filter(|&i| only.is_empty() || only.iter().any(|n| **n == refs[i].name))
        .collect();
    let next = AtomicUsize::new(0);
    let results = Mutex::new(Vec::new());
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    std::thread::scope(|s| {
        for _ in 0..threads {
            s.spawn(|| {
                loop {
                    let k = next.fetch_add(1, Ordering::Relaxed);
                    let Some(&i) = todo.get(k) else { break };
                    let r = &refs[i];
                    let previous = from
                        .as_ref()
                        .and_then(|d| read_patch(&d.join(format!("{}.fit.txt", r.name))));
                    let (patch, d) = fit(r, 1 + i as u32, previous);
                    let secs = r.left.len() as f32 / FS;
                    let (l, rr) = render(&patch, secs);
                    let d = d - FEATURE_WEIGHT * Features::measure(&l, &rr, FS).distance(&r.features);
                    // Yardstick: the nearest other reference, same measure.
                    let nearest = refs
                        .iter()
                        .enumerate()
                        .filter(|(j, _)| *j != i)
                        .map(|(_, o)| {
                            let mut l = o.left.clone();
                            let mut rr = o.right.clone();
                            l.resize(r.left.len(), 0.0);
                            rr.resize(r.left.len(), 0.0);
                            Gram::new(&l, &rr, FS).distance(&r.gram)
                        })
                        .fold(f32::MAX, f32::min);
                    let fd = Features::measure(&l, &rr, FS).distance(&r.features);
                    let mut both_l = r.left.clone();
                    let mut both_r = r.right.clone();
                    both_l.extend(vec![0.0; (0.3 * FS) as usize]);
                    both_r.extend(vec![0.0; (0.3 * FS) as usize]);
                    both_l.extend(&l);
                    both_r.extend(&rr);
                    common::write_wav(&out.join(format!("{}.fit.wav", r.name)), FS as u32, &both_l, &both_r)
                        .unwrap();
                    let txt: String = ALL_PARAMS
                        .iter()
                        .map(|p| format!("{p:?} {}\n", patch[p.index()]))
                        .collect();
                    std::fs::write(out.join(format!("{}.fit.txt", r.name)), txt).unwrap();
                    let line = format!(
                        "{:<12} {:<6} fit {:>5.2} dB  nearest other {:>5.2} dB  features {:>4.2}  peaks {:?}",
                        r.name,
                        ENGINE_NAMES[patch[Param::Engine.index()] as usize],
                        d,
                        nearest,
                        fd,
                        r.peaks.iter().map(|p| p.round() as i32).collect::<Vec<_>>()
                    );
                    println!("{line}");
                    results.lock().unwrap().push((r.name.clone(), line));
                }
            });
        }
    });
    let mut results = results.into_inner().unwrap();
    results.sort();
    let summary: String = results.iter().map(|(_, l)| format!("{l}\n")).collect();
    std::fs::write(out.join("summary.txt"), summary).unwrap();
}
