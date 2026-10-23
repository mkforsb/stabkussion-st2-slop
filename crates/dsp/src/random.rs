//! RND and MUTATE: random patches shaped for techno stabs, and the
//! parameter locks that keep controls out of them.
//!
//! This lives in the DSP crate (it has no UI dependencies) so the analysis
//! examples can roll and render random hits exactly as the panel does.

use crate::params::{
    ENGINE_COUNT, FILTER_BP, FILTER_HP, MACRO_COUNT, MACROS, ParamKind, XMOD_PARAMS, map,
};
use crate::util::Noise;
use crate::{ALL_PARAMS, PARAM_COUNT, Param, VoicePatch};

/// Which parameters of one channel are locked against RND and MUTATE.
pub type Locks = [bool; PARAM_COUNT];

pub const NO_LOCKS: Locks = [false; PARAM_COUNT];

/// Can this control be locked? Everything RND and MUTATE would change.
pub fn lockable(p: Param) -> bool {
    !p.is_performance()
}

/// A random patch, as produced by the RND button (and MUTATE at RND on every
/// hit). Performance controls (level, sense, pan) and `locks`ed controls keep
/// their values from `current`.
///
/// Every control draws exactly one number, in parameter order, whether or
/// not it is locked, so locking one control never changes what the others
/// roll. The draws are shaped for *techno stabs*: a root between 35 and
/// 160 Hz, the other oscillators mostly on chord intervals (slightly off
/// equal temperament) or a few cents off for thickness, now and then
/// anywhere, short-to-medium envelopes that are often gated, a lowpass that
/// a filter envelope opens on the hit and closes within a few hundred
/// milliseconds, and a generous helping of chorus, echo and plate.
///
/// ST-2's U, V and W macros are each off about half the time; the
/// cross-modulation is mostly off (a ducked channel now and then, rarely
/// anything stranger). The panel's own XMOD RND, [`random_xmod`], goes
/// further.
pub fn random_patch(current: &VoicePatch, locks: &Locks, rng: &mut Noise) -> VoicePatch {
    let mut patch = *current;
    let mut draws = [0.0f32; PARAM_COUNT];
    for p in ALL_PARAMS {
        draws[p.index()] = rng.sample() * 0.5 + 0.5;
    }
    let bipolar = |x: f32, depth: f32| 0.5 + depth * (2.0 * x - 1.0).powi(3);
    // Often off, otherwise anywhere up to `max`.
    let sometimes = |x: f32, off: f32, max: f32| ((x - off) / (1.0 - off)).max(0.0) * max;
    let lerp = |lo: f32, hi: f32, x: f32| lo + (hi - lo) * x;
    let sometimes_bipolar = |x: f32, off: f32, depth: f32| {
        if x < off {
            0.5
        } else {
            0.5 + depth * (2.0 * (x - off) / (1.0 - off) - 1.0)
        }
    };
    let free = |p: Param| !p.is_performance() && !locks[p.index()];
    for p in ALL_PARAMS {
        if !free(p) {
            continue;
        }
        let x = draws[p.index()];
        patch[p.index()] = p.sanitize(match (p, p.info().kind) {
            // Always a lowpass (the bandpass and highpass are for hand work).
            (Param::FilterType, _) => {
                if x < 0.35 {
                    0.0
                } else {
                    1.0
                }
            }
            (Param::SatType, _) => (x * 3.0).floor(),
            (_, ParamKind::Choice(names)) => (x * names.len() as f32).floor(),
            // 85 Hz … 165 Hz.
            (Param::Pitch, _) => map::pitch_knob(85.0 * (165.0f32 / 85.0).powf(x)),
            (Param::Fine, _) => lerp(0.4, 0.6, x),
            // 0.55 s … 1.8 s.
            (Param::Decay, _) => lerp(0.55, 0.77, x),
            (Param::Level1, _) => lerp(0.75, 1.0, x),
            (Param::Level2 | Param::Level3 | Param::Level4, _) => sometimes(x, 0.3, 1.0),
            (Param::Tune2 | Param::Tune3 | Param::Tune4, _) => map::tune_knob(chord_tone(x)),
            // ×0.28 … ×1: the chord tones never outlast the root.
            (Param::Decay2 | Param::Decay3 | Param::Decay4, _) => lerp(0.55, 0.8, x),
            // Up to 33 ms, mostly short.
            (Param::Attack, _) => 0.7 * x.powf(1.5),
            (Param::Sustain, _) => sometimes(x, 0.5, 0.5),
            // Gated about half the time, for 0.22 … 0.7 s.
            (Param::Gate, _) => {
                if x < 0.5 {
                    lerp(0.43, 0.68, x / 0.5)
                } else {
                    1.0
                }
            }
            // 10 … 200 ms.
            (Param::Release, _) => lerp(0.17, 0.6, x),
            (Param::Click, _) => 0.2 * x.powi(3),
            (Param::Noise, _) => 0.15 * x.powi(3),
            (Param::SweepSpeed, _) => lerp(0.5, 0.9, x),
            // Mostly none, sometimes a short drop of up to a few semitones.
            (Param::SweepDepth, _) => 0.5 + sometimes(x, 0.6, 0.15),
            (Param::ChaosDepth, _) => 0.4 * x * x,
            (Param::Jitter, _) => 0.2 * x * x,
            // 160 Hz … 450 Hz at a 100 Hz root, opened +2 … +4.8 octaves
            // by a filter envelope that closes in 0.27 … 1.2 s.
            (Param::Cutoff, _) => lerp(0.3, 0.45, x),
            (Param::Reso, _) => 0.45 * x.powf(1.5),
            (Param::FilterEnv, _) => lerp(0.68, 0.85, x),
            (Param::FilterDecay, _) => lerp(0.55, 0.8, x),
            (Param::KeyTrack, _) => lerp(0.2, 0.7, x),
            (Param::Drive, _) => sometimes(x, 0.4, 0.5),
            (Param::SatMix, _) => lerp(0.4, 1.0, x),
            // Set from the root below.
            (Param::LowCut, _) => x,
            (Param::LowFreq, _) => lerp(0.2, 0.5, x),
            (Param::LowGain, _) => bipolar(x, 0.15),
            (Param::MidFreq, _) => lerp(0.3, 0.6, x),
            // −1 … +4 dB: cutting the low mids takes the stab out of the stab.
            (Param::MidGain, _) => lerp(0.47, 0.61, x),
            (Param::MidQ, _) => lerp(0.2, 0.6, x),
            (Param::HighFreq, _) => lerp(0.0, 0.4, x),
            (Param::HighGain, _) => lerp(0.5, 0.3, x),
            (Param::ChorusMix, _) => lerp(0.15, 1.0, x),
            (Param::ChorusRate, _) => lerp(0.2, 0.7, x),
            (Param::ChorusDepth, _) => lerp(0.5, 1.0, x),
            (Param::ChorusXover, _) => lerp(0.0, 0.45, x),
            (Param::DelayMix, _) => sometimes(x, 0.5, 0.7),
            (Param::DelayTime, _) => lerp(0.55, 0.85, x),
            (Param::DelayFeedback, _) => lerp(0.2, 0.7, x),
            (Param::DelayTone, _) => lerp(0.0, 0.5, x),
            (Param::ReverbMix, _) => lerp(0.3, 0.75, x),
            (Param::ReverbDecay, _) => lerp(0.1, 0.65, x),
            (Param::ReverbTone, _) => 0.6 * x,
            (Param::ReverbPredelay, _) => 0.4 * x,
            // Cross-modulation: rarely, and gently.
            (Param::XFm, _) => sometimes(x, 0.92, 0.35),
            (Param::XRing, _) => sometimes(x, 0.95, 0.5),
            (Param::XFilterFm, _) => sometimes(x, 0.92, 0.4),
            (Param::XCrush, _) => sometimes(x, 0.96, 0.4),
            (Param::XCutoff, _) => sometimes_bipolar(x, 0.85, 0.35),
            (Param::XPitch, _) => sometimes_bipolar(x, 0.95, 0.2),
            (Param::XMacro, _) => sometimes_bipolar(x, 0.85, 0.4),
            (Param::XDuck, _) => sometimes(x, 0.75, 0.8),
            (Param::XChain, _) => sometimes(x, 0.94, 0.8),
            (Param::XChainTime, _) => lerp(0.3, 0.8, x),
            _ => x,
        });
    }
    // Each engine's macros, within their useful range.
    let engine = patch[Param::Engine.index()] as u32;
    for (slot, p) in MACROS.into_iter().enumerate() {
        if free(p) {
            patch[p.index()] = macro_draw(engine, slot, draws[p.index()]);
        }
    }
    // The gate stays open at least 220 ms past the attack, or a slow attack
    // is cut off before it has sounded.
    if free(Param::Gate) {
        let attack = map::attack_seconds(patch[Param::Attack.index()]);
        if let Some(g) = map::gate_seconds(patch[Param::Gate.index()])
            && g < attack + 0.22
        {
            patch[Param::Gate.index()] = ((attack + 0.22) / 0.03).log(100.0);
        }
    }
    // Sustain only on gated notes: a held note at a sustain level never
    // ends.
    if patch[Param::Gate.index()] >= 0.999 && free(Param::Sustain) {
        patch[Param::Sustain.index()] = 0.0;
    }
    let base = map::pitch_hz(patch[Param::Pitch.index()], patch[Param::Fine.index()]);
    // Some engines are brighter than others at the same cutoff (a synced or
    // unison saw against a sine-based FM): move the cutoff so that every
    // engine's stabs are about equally bright.
    if free(Param::Cutoff) {
        const OCTAVES: [f32; ENGINE_COUNT] = [0.5, -0.5, -0.65, 0.6, 0.2, 0.25, 0.9, 0.6];
        let oct = OCTAVES[(engine as usize).min(ENGINE_COUNT - 1)];
        let v = patch[Param::Cutoff.index()] + oct / 1000f32.log2();
        patch[Param::Cutoff.index()] = Param::Cutoff.sanitize(v);
    }
    // A lowpass below the root leaves nothing but the filter envelope's
    // burst: keep the cutoff (after key tracking) at 1.3 × the root or more.
    if free(Param::Cutoff) {
        let key = patch[Param::KeyTrack.index()] * (base / map::KEY_REF_HZ).log2();
        let hz = map::cutoff_hz(patch[Param::Cutoff.index()]) * key.exp2();
        if hz < 1.3 * base {
            let v = patch[Param::Cutoff.index()] + (1.3 * base / hz).log2() / 1000f32.log2();
            patch[Param::Cutoff.index()] = Param::Cutoff.sanitize(v);
        }
    }
    // A bandpass needs its cutoff in the body of the sound.
    if patch[Param::FilterType.index()] as u32 == FILTER_BP && free(Param::Cutoff) {
        patch[Param::Cutoff.index()] = lerp(0.3, 0.5, draws[Param::Cutoff.index()]);
    }
    if patch[Param::FilterType.index()] as u32 == FILTER_HP && free(Param::Cutoff) {
        patch[Param::Cutoff.index()] = 0.25 * draws[Param::Cutoff.index()];
    }
    // Keep oscillators 2–4 between 55 and 330 Hz: fold them by octaves if
    // not. A stab's weight is in its low mids. And keep them off each
    // other: two oscillators within a few cents of each other beat, and on
    // sine-like waves the beating cancels the stab out a few hundred
    // milliseconds in. Such an oscillator moves to the octave (within the
    // register, if there is one) that is furthest from the others.
    const REGISTER_HZ: (f32, f32) = (55.0, 330.0);
    let in_register = |st: f32| {
        let hz = base * (st / 12.0).exp2();
        (REGISTER_HZ.0..=REGISTER_HZ.1).contains(&hz) && st.abs() <= map::TUNE_RANGE_ST
    };
    let tunes = [Param::Tune2, Param::Tune3, Param::Tune4];
    let mut taken = vec![0.0f32];
    for tune in tunes {
        let mut st = map::tune_semitones(patch[tune.index()]);
        if free(tune) {
            let gap = |st: f32| {
                taken
                    .iter()
                    .map(|t: &f32| (t - st).abs())
                    .fold(f32::MAX, f32::min)
            };
            let octaves = [
                st,
                st + 12.0,
                st - 12.0,
                st + 24.0,
                st - 24.0,
                st + 36.0,
                st - 36.0,
            ];
            // In the register and clear of the others, nearest the draw
            // first; else in the register, clearest; else as drawn.
            st = octaves
                .iter()
                .copied()
                .find(|&c| in_register(c) && gap(c) >= 0.6)
                .or_else(|| {
                    octaves
                        .iter()
                        .copied()
                        .filter(|&c| in_register(c))
                        .max_by(|a, b| gap(*a).total_cmp(&gap(*b)))
                })
                .unwrap_or(st);
            patch[tune.index()] = map::tune_knob(st);
        }
        taken.push(st);
    }
    // A low cut at 0.5 … 0.85 of the root: it thins the root a little and
    // removes the FM and ring-modulation difference tones that can land
    // far below it.
    if free(Param::LowCut) {
        let hz = base * lerp(0.5, 0.85, draws[Param::LowCut.index()]);
        patch[Param::LowCut.index()] = Param::LowCut.sanitize((hz / 10.0).log(40.0));
    }
    patch
}

/// Intervals (semitones) a chord tone is drawn from, with their weights:
/// fifths, minor thirds and sevenths, octaves, fourths, ninths and the odd
/// major third: the vocabulary of dub and minimal chord stabs.
const CHORD_TONES: [(f32, f32); 12] = [
    (7.0, 3.0),
    (3.0, 2.5),
    (10.0, 2.0),
    (12.0, 2.0),
    (5.0, 1.2),
    (15.0, 1.0),
    (-12.0, 1.0),
    (-5.0, 0.8),
    (14.0, 0.7),
    (4.0, 0.7),
    (19.0, 0.6),
    (2.0, 0.4),
];

/// One draw → an oscillator offset in semitones: 70% a chord tone a few
/// cents off true, 15% the octave above detuned by up to a third of a
/// semitone (a thickener that doesn't beat the root away the way a unison
/// detune of a sine-like wave does; that is SUPER's job), 15% anywhere from
/// an octave down to an octave and a half up (the inharmonic clusters).
pub fn chord_tone(x: f32) -> f32 {
    if x < 0.7 {
        let u = x / 0.7;
        let total: f32 = CHORD_TONES.iter().map(|t| t.1).sum();
        let mut acc = 0.0;
        for &(st, w) in &CHORD_TONES {
            let share = w / total;
            if u < acc + share || st == CHORD_TONES[CHORD_TONES.len() - 1].0 {
                // Where in this tone's slice: ±25 cents around it.
                let f = ((u - acc) / share).clamp(0.0, 1.0);
                return st + 0.5 * (f - 0.5);
            }
            acc += share;
        }
        unreachable!()
    } else if x < 0.85 {
        12.0 + 0.35 * (2.0 * (x - 0.7) / 0.15 - 1.0)
    } else {
        -12.0 + 31.0 * (x - 0.85) / 0.15
    }
}

/// The part of each engine's macro ranges (X, Y, Z) that RND draws from.
pub fn macro_range(engine: u32, slot: usize) -> (f32, f32) {
    use crate::params::*;
    const RANGES: [[(f32, f32); 3]; ENGINE_COUNT] = [
        // VA: any mix of saw and pulse, mostly wide pulses, some PWM.
        [(0.0, 1.0), (0.0, 0.7), (0.0, 0.8)],
        // SYNC: any wave, moderate sync, a modest zap.
        [(0.2, 1.0), (0.0, 0.6), (0.0, 0.6)],
        // SUPER: a few to ~30 cents of detune.
        [(0.15, 0.6), (0.3, 0.9), (0.0, 0.7)],
        // FM: ratios 1 … 4.6 (below 1 the sidebands land under the root),
        // an index that decays, but not to nothing.
        [(0.4, 0.84), (0.4, 0.7), (0.3, 0.9)],
        // FB-FM: ratios 1 … 4.6, a moderate index, any feedback.
        [(0.4, 0.84), (0.35, 0.8), (0.0, 1.0)],
        // RING: brighter waves, ratios 1 … 4.6, mostly ringing.
        [(0.3, 1.0), (0.4, 0.84), (0.3, 1.0)],
        // PD: distortion that relaxes, but not all the way; saw and square,
        // a little resonance (the resonant waves alone lose the fundamental).
        [(0.5, 0.95), (0.0, 0.55), (0.0, 0.6)],
        // WT-FM: any frame, a moderate index.
        [(0.0, 1.0), (0.1, 0.6), (0.2, 0.75)],
    ];
    RANGES[(engine as usize).min(ENGINE_COUNT - 1)][slot.min(2)]
}

/// How RND draws ST-2's U, V and W: off for a share of the draws, otherwise
/// anywhere in a range: `(share off, lo, hi)`.
pub fn macro_range_uvw(engine: u32, slot: usize) -> (f32, f32, f32) {
    const RANGES: [[(f32, f32, f32); 3]; ENGINE_COUNT] = [
        // VA: SUB, RATE, DRIFT.
        [(0.5, 0.2, 0.8), (0.6, 0.0, 0.5), (0.4, 0.1, 0.6)],
        // SYNC: Z.DEC, FM, WINDOW.
        [(0.5, 0.2, 0.9), (0.6, 0.1, 0.5), (0.5, 0.2, 1.0)],
        // SUPER: OCTAVE, SUB, BLOOM.
        [(0.6, 0.1, 0.6), (0.5, 0.2, 0.8), (0.5, 0.1, 0.6)],
        // FM: M.SUS, BEAT, C.WAVE.
        [(0.5, 0.05, 0.3), (0.6, 0.1, 0.6), (0.6, 0.1, 1.0)],
        // FB-FM: I.DEC, LOOP, M.SWP.
        [(0.4, 0.2, 1.0), (0.6, 0.05, 0.5), (0.6, 0.1, 0.6)],
        // RING: R.WAVE, R.ENV, FIXED.
        [(0.5, 0.1, 0.7), (0.6, 0.05, 0.4), (0.75, 0.1, 0.6)],
        // PD: D.DEC, LINE 2, NOISE.
        [(0.5, 0.2, 0.9), (0.5, 0.1, 0.6), (0.8, 0.05, 0.4)],
        // WT-FM: SCAN, SPREAD, BEND.
        [(0.5, 0.1, 0.6), (0.5, 0.1, 0.6), (0.6, 0.05, 0.5)],
    ];
    RANGES[(engine as usize).min(ENGINE_COUNT - 1)][slot.clamp(3, MACRO_COUNT - 1) - 3]
}

/// One draw `x` (0..1) → a macro knob position for `engine`'s `slot`.
pub fn macro_draw(engine: u32, slot: usize, x: f32) -> f32 {
    if slot < 3 {
        let (lo, hi) = macro_range(engine, slot);
        lo + (hi - lo) * x
    } else {
        let (off, lo, hi) = macro_range_uvw(engine, slot);
        if x < off {
            0.0
        } else {
            lo + (hi - lo) * (x - off) / (1.0 - off)
        }
    }
}

/// The CROSS-MOD panel's RND: new cross-modulation for one channel (what it
/// takes from the other), bolder than the channel RND's, leaving every
/// other control and anything locked alone. Like RND, every control draws
/// one number in order whether or not it is locked.
pub fn random_xmod(current: &VoicePatch, locks: &Locks, rng: &mut Noise) -> VoicePatch {
    let mut patch = *current;
    let sometimes = |x: f32, off: f32, max: f32| ((x - off) / (1.0 - off)).max(0.0) * max;
    let bipolar = |x: f32, off: f32, depth: f32| {
        if x < off {
            0.5
        } else {
            0.5 + depth * (2.0 * (x - off) / (1.0 - off) - 1.0)
        }
    };
    for p in XMOD_PARAMS {
        let x = rng.uniform();
        if locks[p.index()] {
            continue;
        }
        patch[p.index()] = p.sanitize(match (p, p.info().kind) {
            (_, ParamKind::Choice(names)) => (x * names.len() as f32).floor(),
            (Param::XFm, _) => sometimes(x, 0.5, 0.7),
            (Param::XRing, _) => sometimes(x, 0.6, 1.0),
            (Param::XFilterFm, _) => sometimes(x, 0.5, 0.75),
            (Param::XCrush, _) => sometimes(x, 0.65, 0.9),
            (Param::XCutoff, _) => bipolar(x, 0.3, 0.8),
            (Param::XPitch, _) => bipolar(x, 0.6, 0.5),
            (Param::XMacro, _) => bipolar(x, 0.3, 0.9),
            (Param::XDuck, _) => sometimes(x, 0.5, 1.0),
            (Param::XChain, _) => sometimes(x, 0.6, 1.0),
            (Param::XChainTime, _) => 0.2 + 0.7 * x,
            _ => x,
        });
    }
    patch
}

/// Move every free control `amount` (0..=1) of the way towards a fresh random
/// patch: `clamp(current + (random - current) * amount)`. At 1 this is exactly
/// [`random_patch`]; choice controls round to the nearest position. Locked
/// controls never move.
pub fn mutate_patch(
    current: &VoicePatch,
    amount: f32,
    locks: &Locks,
    rng: &mut Noise,
) -> VoicePatch {
    let target = random_patch(current, locks, rng);
    if amount >= 1.0 {
        return target;
    }
    let amount = amount.max(0.0);
    let mut patch = *current;
    for p in ALL_PARAMS {
        let (c, t) = (current[p.index()], target[p.index()]);
        patch[p.index()] = p.sanitize(c + (t - c) * amount);
    }
    patch
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::params::{ENGINE_COUNT, default_patch};

    fn is_sanitized(patch: &VoicePatch) -> bool {
        ALL_PARAMS
            .iter()
            .all(|p| p.sanitize(patch[p.index()]) == patch[p.index()])
    }

    #[test]
    fn mutate_off_changes_nothing() {
        let current = default_patch();
        assert_eq!(
            mutate_patch(&current, 0.0, &NO_LOCKS, &mut Noise::new(7)),
            current
        );
    }

    #[test]
    fn mutate_at_rnd_equals_the_rnd_button() {
        let current = default_patch();
        let a = mutate_patch(&current, 1.0, &NO_LOCKS, &mut Noise::new(7));
        let b = random_patch(&current, &NO_LOCKS, &mut Noise::new(7));
        assert_eq!(a, b);
        assert_ne!(a, current);
    }

    #[test]
    fn partial_mutation_moves_part_way_towards_the_random_patch() {
        let current = default_patch();
        for seed in 1..50 {
            let target = random_patch(&current, &NO_LOCKS, &mut Noise::new(seed));
            let half = mutate_patch(&current, 0.5, &NO_LOCKS, &mut Noise::new(seed));
            assert!(is_sanitized(&half));
            for p in ALL_PARAMS {
                let (c, t, h) = (current[p.index()], target[p.index()], half[p.index()]);
                match p.info().kind {
                    ParamKind::Continuous => {
                        assert!((h - (c + (t - c) * 0.5)).abs() < 1e-6, "{p:?}")
                    }
                    ParamKind::Choice(_) => assert_eq!(h, p.sanitize(c + (t - c) * 0.5), "{p:?}"),
                }
            }
        }
    }

    #[test]
    fn mutation_keeps_level_sense_and_pan() {
        let mut current = default_patch();
        current[Param::Level.index()] = 0.33;
        current[Param::Sense.index()] = 0.66;
        current[Param::Pan.index()] = 0.1;
        let mut rng = Noise::new(3);
        for amount in [0.25, 0.5, 1.0] {
            let m = mutate_patch(&current, amount, &NO_LOCKS, &mut rng);
            for p in [Param::Level, Param::Sense, Param::Pan] {
                assert_eq!(m[p.index()], current[p.index()]);
            }
        }
    }

    #[test]
    fn locked_controls_never_move() {
        let mut current = default_patch();
        current[Param::Engine.index()] = 3.0;
        current[Param::Pitch.index()] = 0.123;
        current[Param::FilterType.index()] = FILTER_HP as f32;
        current[Param::Cutoff.index()] = 0.9;
        let mut locks = NO_LOCKS;
        for p in [
            Param::Engine,
            Param::Pitch,
            Param::FilterType,
            Param::Cutoff,
        ] {
            locks[p.index()] = true;
        }
        let mut rng = Noise::new(17);
        let mut patch = current;
        for i in 0..500 {
            patch = mutate_patch(&patch, [0.3, 1.0][i % 2], &locks, &mut rng);
            for p in [
                Param::Engine,
                Param::Pitch,
                Param::FilterType,
                Param::Cutoff,
            ] {
                assert_eq!(patch[p.index()], current[p.index()], "{p:?}");
            }
        }
        // …while the free controls do move.
        assert_ne!(patch[Param::Decay.index()], current[Param::Decay.index()]);
    }

    #[test]
    fn locking_one_control_leaves_the_other_rolls_unchanged() {
        let current = default_patch();
        let mut locks = NO_LOCKS;
        locks[Param::Decay.index()] = true;
        locks[Param::MacroY.index()] = true;
        let a = random_patch(&current, &NO_LOCKS, &mut Noise::new(42));
        let b = random_patch(&current, &locks, &mut Noise::new(42));
        for p in ALL_PARAMS {
            if !locks[p.index()] {
                assert_eq!(a[p.index()], b[p.index()], "{p:?}");
            }
        }
    }

    #[test]
    fn new_macros_are_often_off_and_xmod_rnd_only_touches_xmod() {
        let current = default_patch();
        let mut rng = Noise::new(77);
        let mut off = 0;
        for _ in 0..400 {
            let p = random_patch(&current, &NO_LOCKS, &mut rng);
            off += [Param::MacroU, Param::MacroV, Param::MacroW]
                .iter()
                .filter(|m| p[m.index()] == 0.0)
                .count();
        }
        assert!((400..900).contains(&off), "{off} of 1200 off");
        let mut locks = NO_LOCKS;
        locks[Param::XDuck.index()] = true;
        let mut current = default_patch();
        current[Param::XDuck.index()] = 0.42;
        for _ in 0..50 {
            let p = random_xmod(&current, &locks, &mut rng);
            for q in ALL_PARAMS {
                if !q.is_xmod() || q == Param::XDuck {
                    assert_eq!(p[q.index()], current[q.index()], "{q:?}");
                }
            }
        }
    }

    #[test]
    fn a_locked_highpass_still_gets_a_low_cutoff() {
        let mut current = default_patch();
        current[Param::FilterType.index()] = FILTER_HP as f32;
        let mut locks = NO_LOCKS;
        locks[Param::FilterType.index()] = true;
        let mut rng = Noise::new(9);
        for _ in 0..100 {
            let p = random_patch(&current, &locks, &mut rng);
            assert!(p[Param::Cutoff.index()] <= 0.25);
        }
    }

    #[test]
    fn repeated_mutation_stays_in_range() {
        let mut patch = default_patch();
        let mut rng = Noise::new(11);
        for i in 0..1000 {
            patch = mutate_patch(&patch, [0.1, 0.5, 0.9][i % 3], &NO_LOCKS, &mut rng);
            assert!(is_sanitized(&patch));
        }
    }

    /// RND (and MUTATE at RND) must always give something audible and sane,
    /// whichever engine it lands on.
    #[test]
    fn random_patches_are_playable() {
        use crate::voice::Voice;
        let mut rng = Noise::new(2024);
        let mut engines = [0usize; ENGINE_COUNT];
        let fs = 48_000.0;
        for i in 0..1_000 {
            let patch = random_patch(&default_patch(), &NO_LOCKS, &mut rng);
            engines[patch[Param::Engine.index()] as usize] += 1;
            let mut v = Voice::new(fs, i);
            for p in ALL_PARAMS {
                v.set_param(p, patch[p.index()]);
            }
            v.snap_params();
            v.trigger(0.9);
            let out: Vec<f32> = (0..(fs * 0.4) as usize).map(|_| v.tick()).collect();
            let peak = out.iter().fold(0.0f32, |a, &b| a.max(b.abs()));
            assert!(out.iter().all(|x| x.is_finite()), "patch {i}");
            assert!(
                peak > 0.02 && peak < 4.0,
                "patch {i}: peak {peak} {patch:?}"
            );
        }
        assert!(engines.iter().all(|&n| n > 60), "engine spread {engines:?}");
    }

    /// The point of the synth: RND hits sound like the reference stabs. At
    /// least 95% of them must be as close to a reference (on the
    /// [`crate::analysis::Features`]) as the loneliest reference is to its
    /// nearest neighbour, rendered through the whole engine as the panel
    /// plays them. (`cargo run --example stabs -- rnd DIR` measures the same
    /// against the WAVs; it runs at about 98%.)
    #[test]
    fn random_hits_are_stabs() {
        use crate::analysis::{Features, REFERENCES, nearest_reference, stab_threshold};
        let fs = 24_000.0;
        let threshold = stab_threshold(&REFERENCES);
        let mut rng = Noise::new(31);
        let hits = 200;
        let mut stabs = 0;
        for _ in 0..hits {
            let patch = random_patch(&default_patch(), &NO_LOCKS, &mut rng);
            let mut e = crate::Engine::new(fs);
            e.load_patch(0, &patch);
            e.snap_params();
            e.trigger(0, 1.0);
            let (l, r) = e.render_vec((fs * 2.5) as usize);
            if nearest_reference(&Features::measure(&l, &r, fs), &REFERENCES) <= threshold {
                stabs += 1;
            }
        }
        assert!(stabs * 100 >= hits * 95, "{stabs}/{hits} hits are stabs");
    }
}
