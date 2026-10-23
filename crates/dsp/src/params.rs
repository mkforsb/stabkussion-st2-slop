//! Parameter definitions shared by the engine, the wasm worklet and the UI.
//!
//! Every parameter is stored as an `f32`. Continuous parameters are normalized
//! to `0.0..=1.0` (think "knob position"); choice parameters hold the index of
//! the selected option. The [`map`] module converts knob positions into
//! physical units and is used by both the DSP and the UI read-outs, so what the
//! panel displays is exactly what the engine does.
//!
//! The three macro knobs (X, Y, Z) mean something different for every engine;
//! [`macro_label`] and [`macro_display`] give their per-engine names.

/// Per-voice parameters. The discriminant is the stable wire id.
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Param {
    /// Synthesis engine (see [`ENGINE_NAMES`]).
    Engine = 0,
    /// Pitch of oscillator 1 (continuous, not quantized to notes).
    Pitch,
    /// Fine tune.
    Fine,
    /// Amplitude decay time of oscillator 1 (T60 towards SUSTAIN).
    Decay,
    /// Engine macro 1.
    MacroX,
    /// Engine macro 2.
    MacroY,
    /// Engine macro 3.
    MacroZ,
    /// Oscillator 1 level.
    Level1,
    /// Oscillator 2 tuning relative to oscillator 1, free (±24 semitones).
    Tune2,
    /// Oscillator 2 level.
    Level2,
    /// Oscillator 2 decay as a multiple of DECAY.
    Decay2,
    /// Oscillator 3 tuning relative to oscillator 1.
    Tune3,
    /// Oscillator 3 level.
    Level3,
    /// Oscillator 3 decay as a multiple of DECAY.
    Decay3,
    /// Oscillator 4 tuning relative to oscillator 1.
    Tune4,
    /// Oscillator 4 level.
    Level4,
    /// Oscillator 4 decay as a multiple of DECAY.
    Decay4,
    /// Amplitude envelope attack.
    Attack,
    /// Level the decay settles at while the gate is open.
    Sustain,
    /// Gate length: how long the note is held before the release (top = held
    /// for as long as it rings).
    Gate,
    /// Release time after the gate closes.
    Release,
    /// Click transient level.
    Click,
    /// Noise burst amount and length.
    Noise,
    /// Pitch sweep time.
    SweepSpeed,
    /// Pitch sweep depth, bipolar (centre = off, up = starts high).
    SweepDepth,
    /// Chaos modulator (Lorenz attractor) rate.
    ChaosRate,
    /// Chaos modulator depth.
    ChaosDepth,
    /// Chaos modulator destination.
    ChaosDest,
    /// Per-hit random offsets of pitch and macros.
    Jitter,
    /// Filter cutoff.
    Cutoff,
    /// Filter resonance (ladder feedback).
    Reso,
    /// Filter envelope → cutoff amount, bipolar.
    FilterEnv,
    /// Filter envelope decay time.
    FilterDecay,
    /// Cutoff key tracking (follows PITCH).
    KeyTrack,
    /// Filter type: LP12 / LP24 / BP / HP.
    FilterType,
    /// Saturator drive.
    Drive,
    /// Saturator curve.
    SatType,
    /// Saturator dry/wet.
    SatMix,
    /// Saturator position: before the filter or after the EQ.
    SatPos,
    /// EQ low cut (12 dB/oct highpass), off at zero.
    LowCut,
    /// EQ low shelf frequency.
    LowFreq,
    /// EQ low shelf gain, bipolar.
    LowGain,
    /// EQ mid bell frequency.
    MidFreq,
    /// EQ mid bell gain, bipolar.
    MidGain,
    /// EQ mid bell Q.
    MidQ,
    /// EQ high shelf frequency.
    HighFreq,
    /// EQ high shelf gain, bipolar.
    HighGain,
    /// Chorus wet amount.
    ChorusMix,
    /// Chorus LFO rate.
    ChorusRate,
    /// Chorus modulation depth.
    ChorusDepth,
    /// Chorus crossover: everything below it stays dry and mono, off at zero.
    ChorusXover,
    /// Echo level.
    DelayMix,
    /// Echo time.
    DelayTime,
    /// Echo feedback.
    DelayFeedback,
    /// Echo tone (lowpass in the feedback loop).
    DelayTone,
    /// Channel output level.
    Level,
    /// Trigger sensitivity.
    Sense,
    /// Stereo position.
    Pan,
    /// Reverb send level (post channel output level).
    ReverbMix,
    /// Reverb decay time (≈ RT60).
    ReverbDecay,
    /// Reverb damping / brightness.
    ReverbTone,
    /// Reverb pre-delay.
    ReverbPredelay,
    // --- ST-2: three more engine macros ----------------------------------
    /// Engine macro 4.
    MacroU,
    /// Engine macro 5.
    MacroV,
    /// Engine macro 6.
    MacroW,
    // --- ST-2: cross-modulation, what this channel takes from the other ----
    /// Exponential audio-rate FM of this channel's oscillators by the other
    /// channel's signal.
    XFm,
    /// Ring modulation by the other channel's signal.
    XRing,
    /// Audio-rate filter FM: the other channel's signal moves this cutoff.
    XFilterFm,
    /// Sample-and-hold of this channel's oscillators, clocked by the other
    /// channel's zero crossings.
    XCrush,
    /// The other channel's envelope → this cutoff, bipolar.
    XCutoff,
    /// The other channel's envelope → this pitch, bipolar.
    XPitch,
    /// The other channel's envelope → one of this channel's macros, bipolar.
    XMacro,
    /// Which macro [`Param::XMacro`] moves.
    XMacroDest,
    /// Sidechain ducking: the other channel's envelope turns this one down.
    XDuck,
    /// Chain trigger: a hit on the other channel strikes this one, this much
    /// softer (off at zero).
    XChain,
    /// Chain trigger delay.
    XChainTime,
    /// Which of the other channel's signals the audio-rate cross-modulation
    /// (FM, RING, F.FM, S&H) hears: its oscillators or its output.
    XSource,
}

pub const PARAM_COUNT: usize = 77;

pub const ALL_PARAMS: [Param; PARAM_COUNT] = [
    Param::Engine,
    Param::Pitch,
    Param::Fine,
    Param::Decay,
    Param::MacroX,
    Param::MacroY,
    Param::MacroZ,
    Param::Level1,
    Param::Tune2,
    Param::Level2,
    Param::Decay2,
    Param::Tune3,
    Param::Level3,
    Param::Decay3,
    Param::Tune4,
    Param::Level4,
    Param::Decay4,
    Param::Attack,
    Param::Sustain,
    Param::Gate,
    Param::Release,
    Param::Click,
    Param::Noise,
    Param::SweepSpeed,
    Param::SweepDepth,
    Param::ChaosRate,
    Param::ChaosDepth,
    Param::ChaosDest,
    Param::Jitter,
    Param::Cutoff,
    Param::Reso,
    Param::FilterEnv,
    Param::FilterDecay,
    Param::KeyTrack,
    Param::FilterType,
    Param::Drive,
    Param::SatType,
    Param::SatMix,
    Param::SatPos,
    Param::LowCut,
    Param::LowFreq,
    Param::LowGain,
    Param::MidFreq,
    Param::MidGain,
    Param::MidQ,
    Param::HighFreq,
    Param::HighGain,
    Param::ChorusMix,
    Param::ChorusRate,
    Param::ChorusDepth,
    Param::ChorusXover,
    Param::DelayMix,
    Param::DelayTime,
    Param::DelayFeedback,
    Param::DelayTone,
    Param::Level,
    Param::Sense,
    Param::Pan,
    Param::ReverbMix,
    Param::ReverbDecay,
    Param::ReverbTone,
    Param::ReverbPredelay,
    Param::MacroU,
    Param::MacroV,
    Param::MacroW,
    Param::XFm,
    Param::XRing,
    Param::XFilterFm,
    Param::XCrush,
    Param::XCutoff,
    Param::XPitch,
    Param::XMacro,
    Param::XMacroDest,
    Param::XDuck,
    Param::XChain,
    Param::XChainTime,
    Param::XSource,
];

/// The six engine macros, in slot order.
pub const MACROS: [Param; MACRO_COUNT] = [
    Param::MacroX,
    Param::MacroY,
    Param::MacroZ,
    Param::MacroU,
    Param::MacroV,
    Param::MacroW,
];
pub const MACRO_COUNT: usize = 6;

/// The cross-modulation controls, in panel order.
pub const XMOD_PARAMS: [Param; 12] = [
    Param::XFm,
    Param::XRing,
    Param::XFilterFm,
    Param::XCrush,
    Param::XSource,
    Param::XCutoff,
    Param::XPitch,
    Param::XMacro,
    Param::XMacroDest,
    Param::XDuck,
    Param::XChain,
    Param::XChainTime,
];

pub const ENGINE_VA: u32 = 0;
pub const ENGINE_SYNC: u32 = 1;
pub const ENGINE_SUPER: u32 = 2;
pub const ENGINE_FM: u32 = 3;
pub const ENGINE_FBFM: u32 = 4;
pub const ENGINE_RING: u32 = 5;
pub const ENGINE_PD: u32 = 6;
pub const ENGINE_WTFM: u32 = 7;
pub const ENGINE_COUNT: usize = 8;

pub const ENGINE_NAMES: &[&str] = &["VA", "SYNC", "SUPER", "FM", "FB-FM", "RING", "PD", "WT-FM"];

/// One-line description of each engine, for the panel.
pub const ENGINE_BLURBS: [&str; ENGINE_COUNT] = [
    "Virtual analog: saw and pulse with free width and slow pulse-width modulation",
    "Virtual analog hard-synced to a hidden master, with an envelope-driven sync zap",
    "Unison stack of five detuned saw/pulse voices per oscillator",
    "Two-operator FM with a free, often inharmonic, ratio and a decaying index",
    "FM from a self-feedback modulator: a sine that turns saw-like and gritty",
    "Ring modulation: a virtual-analog wave times a sine at a free ratio",
    "Casio-style phase distortion: a sine bent into saw, square and resonant waves",
    "Wavetable carrier phase-modulated by a sine, the bark decaying with the hit",
];

/// ST-1's five destinations keep their positions; U, V and W follow.
pub const CHAOS_DEST_NAMES: &[&str] = &["PITCH", "X", "Y", "Z", "CUTOFF", "U", "V", "W"];
pub const CHAOS_PITCH: u32 = 0;
pub const CHAOS_X: u32 = 1;
pub const CHAOS_Y: u32 = 2;
pub const CHAOS_Z: u32 = 3;
pub const CHAOS_CUTOFF: u32 = 4;
pub const CHAOS_U: u32 = 5;
pub const CHAOS_V: u32 = 6;
pub const CHAOS_W: u32 = 7;

/// The macro slot a chaos destination moves, if it is a macro.
pub fn chaos_macro_slot(dest: u32) -> Option<usize> {
    match dest {
        CHAOS_X..=CHAOS_Z => Some((dest - CHAOS_X) as usize),
        CHAOS_U..=CHAOS_W => Some(3 + (dest - CHAOS_U) as usize),
        _ => None,
    }
}

pub const XMOD_MACRO_NAMES: &[&str] = &["X", "Y", "Z", "U", "V", "W"];

pub const XSOURCE_NAMES: &[&str] = &["OSC", "OUT"];
pub const XSOURCE_OSC: u32 = 0;
pub const XSOURCE_OUT: u32 = 1;

pub const FILTER_TYPE_NAMES: &[&str] = &["LP12", "LP24", "BP", "HP"];
pub const FILTER_LP12: u32 = 0;
pub const FILTER_LP24: u32 = 1;
pub const FILTER_BP: u32 = 2;
pub const FILTER_HP: u32 = 3;

pub const SAT_TYPE_NAMES: &[&str] = &["TAPE", "TUBE", "HARD", "FOLD"];
pub const SAT_TAPE: u32 = 0;
pub const SAT_TUBE: u32 = 1;
pub const SAT_HARD: u32 = 2;
pub const SAT_FOLD: u32 = 3;

pub const SAT_POS_NAMES: &[&str] = &["PRE", "POST"];
pub const SAT_PRE: u32 = 0;
pub const SAT_POST: u32 = 1;

/// WT-FM: names of the wavetable frames, in POS order.
pub const FRAME_NAMES: [&str; 8] = [
    "SINE", "OCTAVE", "TRI", "ORGAN", "NASAL", "SQUARE", "SAW", "FAT",
];

/// PD: names of the waveforms WAVE morphs between.
pub const PD_WAVE_NAMES: [&str; 4] = ["SAW", "SQUARE", "RESO", "PULSE"];

/// SYNC and RING: names of the waveforms WAVE morphs between.
pub const VA_WAVE_NAMES: [&str; 4] = ["SINE", "TRI", "SAW", "SQUARE"];

/// RING R.WAVE: the ring modulator's waveforms.
pub const RING_WAVE_NAMES: [&str; 3] = ["SINE", "TRI", "SQUARE"];

/// FM C.WAVE: the carrier's waveforms (OPL-style half sine, soft square).
pub const FM_WAVE_NAMES: [&str; 3] = ["SINE", "HALF", "SQUARE"];

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ParamKind {
    Continuous,
    Choice(&'static [&'static str]),
}

#[derive(Clone, Copy, Debug)]
pub struct ParamInfo {
    pub param: Param,
    /// Descriptive name.
    pub name: &'static str,
    /// Panel label.
    pub label: &'static str,
    pub default: f32,
    pub kind: ParamKind,
}

impl Param {
    pub fn id(self) -> u32 {
        self as u32
    }

    pub fn index(self) -> usize {
        self as usize
    }

    pub fn from_id(id: u32) -> Option<Param> {
        ALL_PARAMS.get(id as usize).copied()
    }

    pub fn info(self) -> ParamInfo {
        use ParamKind::*;
        let (name, label, default, kind) = match self {
            Param::Engine => ("Engine", "ENGINE", ENGINE_VA as f32, Choice(ENGINE_NAMES)),
            Param::Pitch => ("Pitch", "PITCH", 0.4, Continuous),
            Param::Fine => ("Fine tune", "FINE", 0.5, Continuous),
            Param::Decay => ("Decay", "DECAY", 0.5, Continuous),
            Param::MacroX => ("Macro X", "X", 0.0, Continuous),
            Param::MacroY => ("Macro Y", "Y", 0.0, Continuous),
            Param::MacroZ => ("Macro Z", "Z", 0.0, Continuous),
            Param::Level1 => ("Oscillator 1 level", "LEVEL", 1.0, Continuous),
            Param::Tune2 => ("Oscillator 2 tune", "TUNE", map::tune_knob(3.0), Continuous),
            Param::Level2 => ("Oscillator 2 level", "LEVEL", 0.0, Continuous),
            Param::Decay2 => ("Oscillator 2 decay", "DECAY", 0.8, Continuous),
            Param::Tune3 => ("Oscillator 3 tune", "TUNE", map::tune_knob(7.0), Continuous),
            Param::Level3 => ("Oscillator 3 level", "LEVEL", 0.0, Continuous),
            Param::Decay3 => ("Oscillator 3 decay", "DECAY", 0.8, Continuous),
            Param::Tune4 => (
                "Oscillator 4 tune",
                "TUNE",
                map::tune_knob(10.0),
                Continuous,
            ),
            Param::Level4 => ("Oscillator 4 level", "LEVEL", 0.0, Continuous),
            Param::Decay4 => ("Oscillator 4 decay", "DECAY", 0.8, Continuous),
            Param::Attack => ("Attack", "ATTACK", 0.1, Continuous),
            Param::Sustain => ("Sustain", "SUSTAIN", 0.0, Continuous),
            Param::Gate => ("Gate length", "GATE", 1.0, Continuous),
            Param::Release => ("Release", "RELEASE", 0.3, Continuous),
            Param::Click => ("Click transient", "CLICK", 0.0, Continuous),
            Param::Noise => ("Noise burst", "NOISE", 0.0, Continuous),
            Param::SweepSpeed => ("Sweep time", "TIME", 0.6, Continuous),
            Param::SweepDepth => ("Sweep depth", "DEPTH", 0.5, Continuous),
            Param::ChaosRate => ("Chaos rate", "RATE", 0.4, Continuous),
            Param::ChaosDepth => ("Chaos depth", "DEPTH", 0.0, Continuous),
            Param::ChaosDest => (
                "Chaos destination",
                "DEST",
                CHAOS_CUTOFF as f32,
                Choice(CHAOS_DEST_NAMES),
            ),
            Param::Jitter => ("Per-hit jitter", "JITTER", 0.0, Continuous),
            Param::Cutoff => ("Filter cutoff", "CUTOFF", 0.3, Continuous),
            Param::Reso => ("Filter resonance", "RESO", 0.2, Continuous),
            Param::FilterEnv => ("Filter envelope", "ENV", 0.75, Continuous),
            Param::FilterDecay => ("Filter envelope decay", "F.DEC", 0.4, Continuous),
            Param::KeyTrack => ("Cutoff key tracking", "KEY", 0.5, Continuous),
            Param::FilterType => (
                "Filter type",
                "TYPE",
                FILTER_LP24 as f32,
                Choice(FILTER_TYPE_NAMES),
            ),
            Param::Drive => ("Saturator drive", "DRIVE", 0.0, Continuous),
            Param::SatType => (
                "Saturator curve",
                "CURVE",
                SAT_TAPE as f32,
                Choice(SAT_TYPE_NAMES),
            ),
            Param::SatMix => ("Saturator mix", "MIX", 1.0, Continuous),
            Param::SatPos => (
                "Saturator position",
                "POS",
                SAT_PRE as f32,
                Choice(SAT_POS_NAMES),
            ),
            Param::LowCut => ("EQ low cut", "LO CUT", 0.0, Continuous),
            Param::LowFreq => ("EQ low shelf frequency", "LOW F", 0.4, Continuous),
            Param::LowGain => ("EQ low shelf gain", "LOW", 0.5, Continuous),
            Param::MidFreq => ("EQ mid frequency", "MID F", 0.45, Continuous),
            Param::MidGain => ("EQ mid gain", "MID", 0.5, Continuous),
            Param::MidQ => ("EQ mid Q", "Q", 0.35, Continuous),
            Param::HighFreq => ("EQ high shelf frequency", "HIGH F", 0.3, Continuous),
            Param::HighGain => ("EQ high shelf gain", "HIGH", 0.5, Continuous),
            Param::ChorusMix => ("Chorus amount", "AMOUNT", 0.0, Continuous),
            Param::ChorusRate => ("Chorus rate", "RATE", 0.45, Continuous),
            Param::ChorusDepth => ("Chorus depth", "DEPTH", 0.5, Continuous),
            Param::ChorusXover => ("Chorus crossover", "X-OVER", 0.3, Continuous),
            Param::DelayMix => ("Echo amount", "AMOUNT", 0.0, Continuous),
            Param::DelayTime => ("Echo time", "TIME", 0.6, Continuous),
            Param::DelayFeedback => ("Echo feedback", "FDBK", 0.4, Continuous),
            Param::DelayTone => ("Echo tone", "TONE", 0.4, Continuous),
            Param::Level => ("Output", "OUTPUT", 0.75, Continuous),
            Param::Sense => ("Sense", "SENSE", 0.5, Continuous),
            Param::Pan => ("Pan", "PAN", 0.5, Continuous),
            Param::ReverbMix => ("Reverb amount", "AMOUNT", 0.15, Continuous),
            Param::ReverbDecay => ("Reverb decay", "DECAY", 0.35, Continuous),
            Param::ReverbTone => ("Reverb tone", "TONE", 0.4, Continuous),
            Param::ReverbPredelay => ("Reverb pre-delay", "PRE", 0.1, Continuous),
            Param::MacroU => ("Macro U", "U", 0.0, Continuous),
            Param::MacroV => ("Macro V", "V", 0.0, Continuous),
            Param::MacroW => ("Macro W", "W", 0.0, Continuous),
            Param::XFm => ("Cross FM", "FM", 0.0, Continuous),
            Param::XRing => ("Cross ring modulation", "RING", 0.0, Continuous),
            Param::XFilterFm => ("Cross filter FM", "F.FM", 0.0, Continuous),
            Param::XCrush => ("Cross sample & hold", "S&H", 0.0, Continuous),
            Param::XCutoff => ("Envelope → cutoff", "→CUT", 0.5, Continuous),
            Param::XPitch => ("Envelope → pitch", "→PITCH", 0.5, Continuous),
            Param::XMacro => ("Envelope → macro", "→MACRO", 0.5, Continuous),
            Param::XMacroDest => (
                "Envelope → macro destination",
                "DEST",
                0.0,
                Choice(XMOD_MACRO_NAMES),
            ),
            Param::XDuck => ("Sidechain duck", "DUCK", 0.0, Continuous),
            Param::XChain => ("Chain trigger", "CHAIN", 0.0, Continuous),
            Param::XChainTime => ("Chain delay", "C.TIME", 0.5, Continuous),
            Param::XSource => (
                "Cross-modulation source",
                "SRC",
                XSOURCE_OSC as f32,
                Choice(XSOURCE_NAMES),
            ),
        };
        ParamInfo {
            param: self,
            name,
            label,
            default,
            kind,
        }
    }

    pub fn default_value(self) -> f32 {
        self.info().default
    }

    /// Clamp/quantize a raw value to what this parameter accepts.
    pub fn sanitize(self, v: f32) -> f32 {
        let v = if v.is_finite() {
            v
        } else {
            self.default_value()
        };
        match self.info().kind {
            ParamKind::Continuous => v.clamp(0.0, 1.0),
            ParamKind::Choice(names) => v.round().clamp(0.0, (names.len() - 1) as f32),
        }
    }

    /// Bipolar controls are centred at 0.5 (the panel lights them from the
    /// middle). Macro knobs depend on the engine: see [`Param::is_bipolar_for`].
    pub fn is_bipolar(self) -> bool {
        matches!(
            self,
            Param::Fine
                | Param::Tune2
                | Param::Tune3
                | Param::Tune4
                | Param::SweepDepth
                | Param::FilterEnv
                | Param::LowGain
                | Param::MidGain
                | Param::HighGain
                | Param::Pan
                | Param::XCutoff
                | Param::XPitch
                | Param::XMacro
        )
    }

    pub fn is_bipolar_for(self, engine: u32) -> bool {
        match self.macro_slot() {
            Some(slot) => macro_bipolar(engine, slot),
            None => self.is_bipolar(),
        }
    }

    /// The macro slot (0 = X, 1 = Y, 2 = Z) of a macro parameter.
    pub fn macro_slot(self) -> Option<usize> {
        match self {
            Param::MacroX => Some(0),
            Param::MacroY => Some(1),
            Param::MacroZ => Some(2),
            Param::MacroU => Some(3),
            Param::MacroV => Some(4),
            Param::MacroW => Some(5),
            _ => None,
        }
    }

    /// Controls that RND and MUTATE never change (and that therefore can't be
    /// locked): the channel's level, trigger sensitivity and stereo placement.
    pub fn is_performance(self) -> bool {
        matches!(self, Param::Level | Param::Sense | Param::Pan)
    }

    /// One of the cross-modulation controls (the channel's CROSS-MOD lane).
    pub fn is_xmod(self) -> bool {
        XMOD_PARAMS.contains(&self)
    }

    /// Panel label, resolving macro knobs to the given engine's names.
    pub fn label_for(self, engine: u32) -> &'static str {
        match self.macro_slot() {
            Some(slot) => macro_label(engine, slot),
            None => self.info().label,
        }
    }

    /// Descriptive name, resolving macro knobs to the given engine's names.
    pub fn name_for(self, engine: u32) -> &'static str {
        match self.macro_slot() {
            Some(slot) => macro_name(engine, slot),
            None => self.info().name,
        }
    }

    /// Human readable value for the panel read-out. Macro knobs need the
    /// engine; use [`Param::display_for`] for those.
    pub fn display(self, v: f32) -> String {
        self.display_for(ENGINE_VA, v)
    }

    /// Human readable value, resolving macro knobs for the given engine.
    pub fn display_for(self, engine: u32, v: f32) -> String {
        if let Some(slot) = self.macro_slot() {
            return macro_display(engine, slot, v);
        }
        let off_or = |f: &dyn Fn() -> String| if v <= 0.0 { "OFF".into() } else { f() };
        let pct = || format!("{:.0}%", v * 100.0);
        match self {
            Param::Engine
            | Param::ChaosDest
            | Param::FilterType
            | Param::SatType
            | Param::SatPos
            | Param::XMacroDest
            | Param::XSource => match self.info().kind {
                ParamKind::Choice(names) => names[self.sanitize(v) as usize].to_string(),
                ParamKind::Continuous => unreachable!(),
            },
            Param::Pitch => fmt_hz(map::pitch_hz(v, 0.5)),
            Param::Fine => format!("{:+.0} ct", map::fine_semitones(v) * 100.0),
            Param::Decay => fmt_secs(map::decay_seconds(v)),
            Param::Tune2 | Param::Tune3 | Param::Tune4 => fmt_semitones(map::tune_semitones(v)),
            Param::Level1 | Param::Level2 | Param::Level3 | Param::Level4 => {
                let g = map::osc_gain(v);
                if g <= 0.0 { "OFF".into() } else { fmt_db(g) }
            }
            Param::Decay2 | Param::Decay3 | Param::Decay4 => {
                format!("x{:.2}", map::osc_decay_ratio(v))
            }
            Param::Attack => fmt_secs(map::attack_seconds(v)),
            Param::Sustain => pct(),
            Param::Gate => match map::gate_seconds(v) {
                Some(s) => fmt_secs(s),
                None => "HOLD".into(),
            },
            Param::Release => fmt_secs(map::release_seconds(v)),
            Param::Click
            | Param::ChaosDepth
            | Param::Jitter
            | Param::ChorusMix
            | Param::KeyTrack => off_or(&pct),
            Param::SatMix => pct(),
            Param::Noise => off_or(&|| fmt_secs(map::noise_seconds(v))),
            Param::SweepSpeed => fmt_secs(map::sweep_seconds(v)),
            Param::SweepDepth => {
                let o = map::sweep_octaves(v);
                if o.abs() < 0.005 {
                    "OFF".into()
                } else {
                    format!("{:+.2} oct", o)
                }
            }
            Param::ChaosRate => fmt_hz(map::chaos_hz(v)),
            Param::Cutoff => fmt_hz(map::cutoff_hz(v)),
            Param::Reso => format!("{:.0}%", 100.0 * map::reso_k(v) / 4.0),
            Param::FilterEnv => format!("{:+.1} oct", map::filter_env_octaves(v)),
            Param::FilterDecay => fmt_secs(map::filter_decay_seconds(v)),
            Param::Drive => off_or(&|| format!("+{:.1} dB", map::drive_db(v))),
            Param::LowCut => off_or(&|| fmt_hz(map::low_cut_hz(v))),
            Param::LowFreq => fmt_hz(map::low_shelf_hz(v)),
            Param::MidFreq => fmt_hz(map::mid_hz(v)),
            Param::HighFreq => fmt_hz(map::high_shelf_hz(v)),
            Param::LowGain | Param::MidGain | Param::HighGain => {
                let db = map::eq_gain_db(v);
                if db.abs() < 0.05 {
                    "0 dB".into()
                } else {
                    format!("{db:+.1} dB")
                }
            }
            Param::MidQ => format!("Q {:.1}", map::mid_q(v)),
            Param::ChorusRate => fmt_hz(map::chorus_hz(v)),
            Param::ChorusDepth => format!("{:.1} ms", map::chorus_depth_ms(v)),
            Param::ChorusXover => off_or(&|| fmt_hz(map::chorus_xover_hz(v))),
            Param::DelayMix | Param::ReverbMix => {
                if v <= 0.0 {
                    "OFF".into()
                } else {
                    fmt_db(map::send_gain(v))
                }
            }
            Param::DelayTime => fmt_secs(map::delay_seconds(v)),
            Param::DelayFeedback => format!("{:.0}%", 100.0 * map::delay_feedback(v)),
            Param::DelayTone => fmt_hz(map::delay_tone_hz(v)),
            Param::Level => fmt_db(map::level_gain(v)),
            Param::Sense => format!("x{:.2}", map::sense_gain(v)),
            Param::Pan => {
                let p = (v - 0.5) * 200.0;
                if p.abs() < 1.0 {
                    "C".to_string()
                } else if p < 0.0 {
                    format!("L{:.0}", -p)
                } else {
                    format!("R{:.0}", p)
                }
            }
            Param::ReverbDecay => fmt_secs(map::reverb_seconds(v)),
            Param::ReverbTone => fmt_hz(map::reverb_tone_hz(v)),
            Param::ReverbPredelay => fmt_secs(map::reverb_predelay_seconds(v)),
            Param::XFm => off_or(&|| format!("{:.2} oct", map::xfm_octaves(v))),
            Param::XRing | Param::XCrush | Param::XDuck => off_or(&pct),
            Param::XFilterFm => off_or(&|| format!("{:.1} oct", map::xfilter_fm_octaves(v))),
            Param::XCutoff => fmt_bipolar(map::xcutoff_octaves(v), "oct"),
            Param::XPitch => fmt_bipolar(12.0 * map::xpitch_octaves(v), "st"),
            Param::XMacro => {
                let m = map::xmacro_amount(v);
                if m.abs() < 0.005 {
                    "OFF".into()
                } else {
                    format!("{:+.0}%", 100.0 * m)
                }
            }
            Param::XChain => off_or(&|| format!("x{:.2}", map::chain_gain(v))),
            Param::XChainTime => fmt_secs(map::chain_seconds(v)),
            Param::MacroX
            | Param::MacroY
            | Param::MacroZ
            | Param::MacroU
            | Param::MacroV
            | Param::MacroW => unreachable!(),
        }
    }
}

/// Short panel label of an engine macro knob. Slots 0–2 are ST-1's X, Y
/// and Z; 3–5 are ST-2's U, V and W, all of which are neutral at zero (a
/// patch with U, V and W at zero sounds exactly as it did on the ST-1).
pub fn macro_label(engine: u32, slot: usize) -> &'static str {
    const LABELS: [[&str; MACRO_COUNT]; ENGINE_COUNT] = [
        ["WAVE", "WIDTH", "PWM", "SUB", "RATE", "DRIFT"],
        ["WAVE", "SYNC", "SWEEP", "Z.DEC", "FM", "WINDOW"],
        ["DETUNE", "BLEND", "WAVE", "OCTAVE", "SUB", "BLOOM"],
        ["RATIO", "INDEX", "MOD DEC", "M.SUS", "BEAT", "C.WAVE"],
        ["RATIO", "INDEX", "FDBK", "I.DEC", "LOOP", "M.SWP"],
        ["WAVE", "RATIO", "RING", "R.WAVE", "R.ENV", "FIXED"],
        ["DIST", "WAVE", "D.ENV", "D.DEC", "LINE 2", "NOISE"],
        ["POS", "INDEX", "RATIO", "SCAN", "SPREAD", "BEND"],
    ];
    LABELS[(engine as usize).min(ENGINE_COUNT - 1)][slot.min(MACRO_COUNT - 1)]
}

/// Descriptive name of an engine macro knob.
pub fn macro_name(engine: u32, slot: usize) -> &'static str {
    const NAMES: [[&str; MACRO_COUNT]; ENGINE_COUNT] = [
        [
            "Saw ↔ pulse",
            "Pulse width",
            "Pulse-width modulation",
            "Sub-oscillator (square, −1 oct)",
            "PWM rate",
            "Analog drift",
        ],
        [
            "Waveform",
            "Sync ratio",
            "Sync sweep (envelope)",
            "Sync sweep decay",
            "Master → slave FM",
            "Windowed (formant) sync",
        ],
        [
            "Unison detune",
            "Side voice blend",
            "Saw ↔ pulse",
            "Octave-up unison pair",
            "Sub-oscillator (sine, −1 oct)",
            "Detune bloom (envelope)",
        ],
        [
            "Modulator ratio",
            "Modulation index",
            "Index decay",
            "Index sustain",
            "Modulator beat (Hz offset)",
            "Carrier waveform",
        ],
        [
            "Modulator ratio",
            "Modulation index",
            "Modulator feedback",
            "Index decay",
            "Carrier → modulator loop",
            "Modulator ratio sweep",
        ],
        [
            "Waveform",
            "Ring modulator ratio",
            "Ring modulation amount",
            "Ring modulator waveform",
            "Ring ratio sweep (envelope)",
            "Ring modulator: tracking → fixed",
        ],
        [
            "Distortion",
            "Waveform",
            "Distortion envelope",
            "Distortion envelope decay",
            "Second, detuned line",
            "Noise phase modulation",
        ],
        [
            "Table position",
            "Modulation index",
            "Modulator ratio",
            "Table scan (index envelope)",
            "Detuned second carrier",
            "Phase bend",
        ],
    ];
    NAMES[(engine as usize).min(ENGINE_COUNT - 1)][slot.min(MACRO_COUNT - 1)]
}

/// Macro knobs that are centred. (None at present.)
pub fn macro_bipolar(_engine: u32, _slot: usize) -> bool {
    false
}

/// Read-out of an engine macro knob.
pub fn macro_display(engine: u32, slot: usize, v: f32) -> String {
    let pct = || format!("{:.0}%", v * 100.0);
    let off_or = |f: &dyn Fn() -> String| if v <= 0.0 { "OFF".into() } else { f() };
    match (engine, slot) {
        (ENGINE_VA, 0) => morph_name(&["SAW", "PULSE"], v),
        (ENGINE_VA, 1) => format!("{:.0}%", 100.0 * map::pulse_width(v)),
        (ENGINE_SYNC | ENGINE_RING, 0) => morph_name(&VA_WAVE_NAMES, v),
        (ENGINE_SYNC, 1) => format!("x{:.2}", map::sync_ratio(v)),
        (ENGINE_SYNC, 2) => format!("+{:.1} oct", map::sync_sweep_octaves(v)),
        (ENGINE_SUPER, 0) => format!("{:.0} ct", map::unison_detune_cents(v)),
        (ENGINE_SUPER, 2) => morph_name(&["SAW", "PULSE"], v),
        (ENGINE_FM | ENGINE_FBFM, 0) | (ENGINE_RING, 1) | (ENGINE_WTFM, 2) => {
            format!("x{:.3}", map::fm_ratio(v))
        }
        (ENGINE_FM, 1) => format!("{:.2}", map::fm_index(v)),
        (ENGINE_FM, 2) => format!("x{:.2}", map::fm_index_decay(v)),
        (ENGINE_FBFM, 1) => format!("{:.2}", map::fbfm_index(v)),
        (ENGINE_FBFM, 2) => format!("{:.2}", map::fbfm_feedback(v)),
        (ENGINE_PD, 1) => morph_name(&PD_WAVE_NAMES, v),
        (ENGINE_WTFM, 0) => morph_name(&FRAME_NAMES, v),
        (ENGINE_WTFM, 1) => format!("{:.2}", map::wtfm_index(v)),
        // --- U, V, W ---
        (ENGINE_VA, 4) => format!("x{:.1}", map::pwm_rate_mul(v)),
        (ENGINE_VA, 5) | (ENGINE_PD, 4) => off_or(&|| format!("{:.1} ct", map::drift_cents(v))),
        (ENGINE_SYNC, 3) | (ENGINE_PD, 3) => {
            if v <= 0.0 {
                "ENV".into()
            } else {
                format!("x{:.2}", map::zap_decay_ratio(v))
            }
        }
        (ENGINE_SYNC, 4) => off_or(&|| format!("{:.0}%", 100.0 * map::sync_fm_depth(v))),
        (ENGINE_SUPER, 5) => off_or(&|| format!("+{:.0} ct", map::bloom_cents(v))),
        (ENGINE_FM, 4) => off_or(&|| format!("+{:.2} Hz", map::beat_hz(v))),
        (ENGINE_FM, 5) => morph_name(&FM_WAVE_NAMES, v),
        (ENGINE_FBFM, 4) => off_or(&|| format!("{:.2}", map::fbfm_loop(v))),
        (ENGINE_FBFM, 5) | (ENGINE_RING, 4) => {
            off_or(&|| format!("+{:.1} oct", map::ratio_sweep_octaves(v)))
        }
        (ENGINE_RING, 3) => morph_name(&RING_WAVE_NAMES, v),
        (ENGINE_WTFM, 4) => off_or(&|| format!("{:.1} ct", map::spread_cents(v))),
        (ENGINE_WTFM, 5) => off_or(&|| format!("x{:.2}", map::bend_power(v))),
        (_, 3..) => off_or(&pct),
        _ => pct(),
    }
}

/// "SAW", "SAW>SQU 40%" … for knobs that morph through named shapes.
fn morph_name(names: &[&str], v: f32) -> String {
    let (i, f) = map::segment(v, names.len());
    if f < 0.03 {
        names[i].to_string()
    } else if f > 0.97 {
        names[i + 1].to_string()
    } else {
        let short = |s: &str| s.chars().take(3).collect::<String>();
        format!(
            "{}>{} {:.0}%",
            short(names[i]),
            short(names[i + 1]),
            f * 100.0
        )
    }
}

fn fmt_hz(hz: f32) -> String {
    if hz >= 1000.0 {
        format!("{:.2} kHz", hz / 1000.0)
    } else if hz >= 100.0 {
        format!("{:.0} Hz", hz)
    } else {
        format!("{:.1} Hz", hz)
    }
}

fn fmt_db(gain: f32) -> String {
    if gain <= 1e-4 {
        "-inf dB".to_string()
    } else {
        format!("{:.1} dB", 20.0 * gain.log10())
    }
}

fn fmt_secs(s: f32) -> String {
    if s >= 1.0 {
        format!("{:.2} s", s)
    } else if s >= 0.01 {
        format!("{:.0} ms", s * 1000.0)
    } else {
        format!("{:.1} ms", s * 1000.0)
    }
}

/// "OFF" near zero, else a signed value with its unit.
fn fmt_bipolar(x: f32, unit: &str) -> String {
    if x.abs() < 0.005 {
        "OFF".into()
    } else {
        format!("{x:+.2} {unit}")
    }
}

fn fmt_semitones(st: f32) -> String {
    if st.abs() < 0.005 {
        "0.00 st".into()
    } else {
        format!("{st:+.2} st")
    }
}

/// Knob position → physical unit mappings.
pub mod map {
    /// Lowest pitch at the bottom of the PITCH fader.
    pub const PITCH_MIN_HZ: f32 = 16.0;
    /// Span of the PITCH fader in octaves (16 Hz … 1 kHz).
    pub const PITCH_OCTAVES: f32 = 6.0;
    /// Range of the oscillator 2–4 TUNE knobs, ± semitones.
    pub const TUNE_RANGE_ST: f32 = 24.0;
    /// Pitch at which key tracking leaves the cutoff where CUTOFF says.
    pub const KEY_REF_HZ: f32 = 100.0;

    #[inline]
    fn bipolar(v: f32) -> f32 {
        (v - 0.5) * 2.0
    }

    /// `(segment index, fraction)` of `v` over `n` evenly spaced named points.
    pub fn segment(v: f32, n: usize) -> (usize, f32) {
        let s = v.clamp(0.0, 1.0) * (n - 1) as f32;
        let i = (s as usize).min(n - 2);
        (i, s - i as f32)
    }

    pub fn fine_semitones(fine: f32) -> f32 {
        bipolar(fine) * 2.0
    }

    pub fn pitch_octaves(pitch: f32, fine: f32) -> f32 {
        PITCH_MIN_HZ.log2() + pitch * PITCH_OCTAVES + fine_semitones(fine) / 12.0
    }

    pub fn pitch_hz(pitch: f32, fine: f32) -> f32 {
        pitch_octaves(pitch, fine).exp2()
    }

    /// PITCH fader position of a frequency (with FINE centred).
    pub fn pitch_knob(hz: f32) -> f32 {
        (hz.log2() - PITCH_MIN_HZ.log2()) / PITCH_OCTAVES
    }

    /// Oscillator 2–4 offset from oscillator 1, in (fractional) semitones.
    pub fn tune_semitones(tune: f32) -> f32 {
        bipolar(tune) * TUNE_RANGE_ST
    }

    /// Knob position of a TUNE offset in semitones (for presets and tests).
    pub const fn tune_knob(semitones: f32) -> f32 {
        0.5 + semitones / (2.0 * TUNE_RANGE_ST)
    }

    /// Oscillator level knob → gain.
    pub fn osc_gain(level: f32) -> f32 {
        level * level
    }

    /// -60 dB decay time of oscillator 1: 30 ms … 6 s.
    pub fn decay_seconds(decay: f32) -> f32 {
        0.03 * 200f32.powf(decay)
    }

    /// Oscillator 2–4 decay as a multiple of oscillator 1's (x1 at 0.8).
    pub fn osc_decay_ratio(v: f32) -> f32 {
        0.0625 * 32f32.powf(v)
    }

    /// 0.5 ms … 200 ms.
    pub fn attack_seconds(attack: f32) -> f32 {
        0.000_5 * 400f32.powf(attack)
    }

    /// Gate length, 30 ms … 3 s; `None` (held) at the top of the knob.
    pub fn gate_seconds(gate: f32) -> Option<f32> {
        if gate >= 0.999 {
            None
        } else {
            Some(0.03 * 100f32.powf(gate))
        }
    }

    /// Release (T60) after the gate closes: 3 ms … 3 s.
    pub fn release_seconds(release: f32) -> f32 {
        0.003 * 1000f32.powf(release)
    }

    /// Peak level of the click transient at full velocity.
    pub fn click_gain(click: f32) -> f32 {
        1.2 * click * click
    }

    /// Noise burst decay time constant.
    pub fn noise_seconds(noise: f32) -> f32 {
        0.001 + 0.08 * noise * noise
    }

    /// Pitch sweep time constant. Higher knob = shorter.
    pub fn sweep_seconds(speed: f32) -> f32 {
        1.0 * (0.002f32 / 1.0).powf(speed)
    }

    /// Bipolar sweep depth in octaves: above centre the hit starts high and
    /// falls back to the pitch, below it starts low and rises.
    pub fn sweep_octaves(depth: f32) -> f32 {
        let x = bipolar(depth);
        4.0 * x * x.abs().sqrt()
    }

    /// Approximate orbit rate of the chaos modulator.
    pub fn chaos_hz(rate: f32) -> f32 {
        0.05 * 600f32.powf(rate)
    }

    /// 20 Hz … 20 kHz.
    pub fn cutoff_hz(cutoff: f32) -> f32 {
        20.0 * 1000f32.powf(cutoff)
    }

    /// Ladder feedback; the filter self-oscillates just below the top.
    pub fn reso_k(reso: f32) -> f32 {
        4.3 * reso
    }

    pub fn filter_env_octaves(env: f32) -> f32 {
        let x = bipolar(env);
        8.0 * x * x.abs().sqrt()
    }

    /// Filter envelope decay (T60): 10 ms … 4 s.
    pub fn filter_decay_seconds(v: f32) -> f32 {
        0.01 * 400f32.powf(v)
    }

    pub fn drive_db(drive: f32) -> f32 {
        36.0 * drive
    }

    /// EQ low cut frequency (the knob at zero bypasses it).
    pub fn low_cut_hz(v: f32) -> f32 {
        10.0 * 40f32.powf(v)
    }

    pub fn low_shelf_hz(v: f32) -> f32 {
        20.0 * 40f32.powf(v)
    }

    pub fn mid_hz(v: f32) -> f32 {
        25.0 * 320f32.powf(v)
    }

    pub fn mid_q(v: f32) -> f32 {
        0.3 * 30f32.powf(v)
    }

    pub fn high_shelf_hz(v: f32) -> f32 {
        800.0 * 20f32.powf(v)
    }

    pub fn eq_gain_db(v: f32) -> f32 {
        bipolar(v) * 18.0
    }

    pub fn chorus_hz(rate: f32) -> f32 {
        0.05 * 160f32.powf(rate)
    }

    pub fn chorus_depth_ms(depth: f32) -> f32 {
        5.0 * depth
    }

    pub fn chorus_xover_hz(v: f32) -> f32 {
        20.0 * 30f32.powf(v)
    }

    /// Echo and reverb send gain.
    pub fn send_gain(amount: f32) -> f32 {
        amount * amount
    }

    /// Echo time: 20 ms … 1.2 s.
    pub fn delay_seconds(v: f32) -> f32 {
        0.02 * 60f32.powf(v)
    }

    /// Echo feedback, 0 … 92%.
    pub fn delay_feedback(v: f32) -> f32 {
        0.92 * v
    }

    /// Echo loop lowpass: 400 Hz … 12 kHz.
    pub fn delay_tone_hz(v: f32) -> f32 {
        400.0 * 30f32.powf(v)
    }

    pub fn level_gain(level: f32) -> f32 {
        level * level
    }

    pub fn sense_gain(sense: f32) -> f32 {
        0.25 * 16f32.powf(sense)
    }

    /// Approximate RT60 of the plate.
    pub fn reverb_seconds(decay: f32) -> f32 {
        0.3 * 40f32.powf(decay)
    }

    /// Damping lowpass cutoff (input bandwidth and tank damping).
    pub fn reverb_tone_hz(tone: f32) -> f32 {
        1500.0 * 12f32.powf(tone)
    }

    pub fn reverb_predelay_seconds(predelay: f32) -> f32 {
        0.15 * predelay
    }

    // --- engine macros ---------------------------------------------------

    /// VA WIDTH: pulse width, 50% … 5%.
    pub fn pulse_width(v: f32) -> f32 {
        0.5 - 0.45 * v
    }

    /// VA PWM: peak width swing of the slow modulation.
    pub fn pwm_depth(v: f32) -> f32 {
        0.4 * v
    }

    /// SYNC: static slave/master frequency ratio.
    pub fn sync_ratio(v: f32) -> f32 {
        (3.0 * v).exp2()
    }

    /// SYNC SWEEP: extra octaves of sync ratio at the envelope peak.
    pub fn sync_sweep_octaves(v: f32) -> f32 {
        4.0 * v
    }

    /// SUPER DETUNE: spread of the outer unison voices, ± cents.
    pub fn unison_detune_cents(v: f32) -> f32 {
        80.0 * v * v
    }

    /// FM / FB-FM / RING / WT-FM modulator ratio: 0.25 … 8, continuous.
    pub fn fm_ratio(v: f32) -> f32 {
        0.25 * 32f32.powf(v)
    }

    /// Knob position of an FM ratio.
    pub fn fm_ratio_knob(ratio: f32) -> f32 {
        (ratio / 0.25).log2() / 5.0
    }

    /// FM peak modulation index (radians).
    pub fn fm_index(v: f32) -> f32 {
        12.0 * v * v
    }

    /// FM index decay as a multiple of the oscillator's amplitude decay.
    pub fn fm_index_decay(v: f32) -> f32 {
        0.03 * 33.3f32.powf(v)
    }

    /// FB-FM peak modulation index (radians).
    pub fn fbfm_index(v: f32) -> f32 {
        6.0 * v * v
    }

    /// FB-FM modulator self-feedback (radians).
    pub fn fbfm_feedback(v: f32) -> f32 {
        1.4 * v
    }

    /// WT-FM peak modulation index (radians).
    pub fn wtfm_index(v: f32) -> f32 {
        6.0 * v * v
    }

    // --- ST-2 macros (U, V, W): zero is always "as on the ST-1" --------------

    /// VA SUB: level of the square one octave down.
    pub fn sub_gain(v: f32) -> f32 {
        0.8 * v
    }

    /// VA RATE: multiplier of the PWM LFOs' (randomly drawn) rates, ×1 … ×64.
    pub fn pwm_rate_mul(v: f32) -> f32 {
        64f32.powf(v)
    }

    /// VA DRIFT and PD LINE 2: detune in cents.
    pub fn drift_cents(v: f32) -> f32 {
        25.0 * v * v
    }

    /// SYNC Z.DEC and PD D.DEC: the zap's own decay as a multiple of the
    /// oscillator's (×1 … ×0.02); at zero the zap follows the amp envelope.
    pub fn zap_decay_ratio(v: f32) -> f32 {
        0.02f32.powf(v)
    }

    /// SYNC FM: how far the master's sine swings the slave's rate.
    pub fn sync_fm_depth(v: f32) -> f32 {
        0.9 * v
    }

    /// SUPER BLOOM: extra detune of the outer voices at the envelope peak.
    pub fn bloom_cents(v: f32) -> f32 {
        60.0 * v
    }

    /// FM BEAT: modulator offset in Hz (a fixed offset beats slower on low
    /// notes than on high ones, like a DX's detune).
    pub fn beat_hz(v: f32) -> f32 {
        12.0 * v * v
    }

    /// FB-FM LOOP: carrier → modulator cross-feedback (radians).
    pub fn fbfm_loop(v: f32) -> f32 {
        1.6 * v
    }

    /// FB-FM M.SWP and RING R.ENV: octaves of ratio sweep at the peak.
    pub fn ratio_sweep_octaves(v: f32) -> f32 {
        3.0 * v
    }

    /// WT-FM SPREAD: detune of the second carrier.
    pub fn spread_cents(v: f32) -> f32 {
        30.0 * v * v
    }

    /// WT-FM BEND: exponent of the phase bend `p^a`.
    pub fn bend_power(v: f32) -> f32 {
        1.0 + 3.0 * v
    }

    /// RING FIXED and the shared reference the ring modulator is pinned to.
    pub const RING_FIXED_HZ: f32 = 110.0;

    // --- cross-modulation ---------------------------------------------------

    /// Cross FM depth in octaves per unit of the other channel's signal.
    pub fn xfm_octaves(v: f32) -> f32 {
        3.0 * v * v
    }

    /// Cross filter FM depth in octaves per unit of the other's signal.
    pub fn xfilter_fm_octaves(v: f32) -> f32 {
        5.0 * v * v
    }

    /// Envelope → cutoff, ±5 octaves.
    pub fn xcutoff_octaves(v: f32) -> f32 {
        let x = bipolar(v);
        5.0 * x * x.abs()
    }

    /// Envelope → pitch, ±2 octaves.
    pub fn xpitch_octaves(v: f32) -> f32 {
        let x = bipolar(v);
        2.0 * x * x.abs()
    }

    /// Envelope → macro, ±1 (a full knob turn).
    pub fn xmacro_amount(v: f32) -> f32 {
        let x = bipolar(v);
        x * x.abs()
    }

    /// Chain trigger: velocity of the struck hit relative to the striking
    /// one. Below 1, so two channels chained both ways bounce and die out.
    pub fn chain_gain(v: f32) -> f32 {
        0.95 * v
    }

    /// Chain delay: 5 ms … 1.5 s.
    pub fn chain_seconds(v: f32) -> f32 {
        0.005 * 300f32.powf(v)
    }
}

/// A complete voice patch: one value per [`Param`], indexed by `Param::index`.
pub type VoicePatch = [f32; PARAM_COUNT];

pub fn default_patch() -> VoicePatch {
    let mut p = [0.0; PARAM_COUNT];
    for param in ALL_PARAMS {
        p[param.index()] = param.default_value();
    }
    p
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_roundtrip() {
        for (i, p) in ALL_PARAMS.iter().enumerate() {
            assert_eq!(p.index(), i);
            assert_eq!(Param::from_id(p.id()), Some(*p));
        }
        assert_eq!(Param::from_id(PARAM_COUNT as u32), None);
    }

    #[test]
    fn sanitize_quantizes_choices() {
        assert_eq!(Param::Engine.sanitize(2.4), 2.0);
        assert_eq!(Param::Engine.sanitize(99.0), 7.0);
        assert_eq!(Param::FilterType.sanitize(9.0), 3.0);
        assert_eq!(Param::Pitch.sanitize(1.5), 1.0);
        assert_eq!(
            Param::Pitch.sanitize(f32::NAN),
            Param::Pitch.default_value()
        );
    }

    #[test]
    fn mapping_ranges() {
        assert!((map::pitch_hz(0.0, 0.5) - 16.0).abs() < 0.01);
        assert!((map::pitch_hz(1.0, 0.5) - 1024.0).abs() < 0.5);
        assert!((map::pitch_hz(map::pitch_knob(110.0), 0.5) - 110.0).abs() < 0.01);
        assert!((map::sweep_seconds(1.0) - 0.002).abs() < 1e-5);
        assert!((map::decay_seconds(1.0) - 6.0).abs() < 1e-3);
        assert!((map::osc_decay_ratio(0.8) - 1.0).abs() < 1e-4);
        assert!((map::cutoff_hz(1.0) - 20_000.0).abs() < 1.0);
        assert_eq!(map::gate_seconds(1.0), None);
        assert!((map::gate_seconds(0.0).unwrap() - 0.03).abs() < 1e-6);
        assert_eq!(map::sweep_octaves(0.5), 0.0);
        assert_eq!(map::tune_semitones(0.5), 0.0);
        assert!((map::tune_semitones(map::tune_knob(-12.0)) + 12.0).abs() < 1e-4);
        assert!((map::tune_semitones(map::tune_knob(7.02)) - 7.02).abs() < 1e-4);
        assert!((map::fm_ratio(0.0) - 0.25).abs() < 1e-5);
        assert!((map::fm_ratio(1.0) - 8.0).abs() < 1e-3);
        assert!((map::fm_ratio(map::fm_ratio_knob(0.864)) - 0.864).abs() < 1e-4);
        assert_eq!(map::eq_gain_db(0.5), 0.0);
        assert_eq!(map::segment(1.0, 8), (6, 1.0));
    }

    #[test]
    fn display_is_sane() {
        assert_eq!(Param::Engine.display(5.0), "RING");
        assert_eq!(Param::Pan.display(0.5), "C");
        assert_eq!(Param::Pitch.display(1.0), "1.02 kHz");
        assert_eq!(Param::SweepDepth.display(0.5), "OFF");
        assert_eq!(Param::Tune2.display(0.25), "-12.00 st");
        assert_eq!(Param::Gate.display(1.0), "HOLD");
        assert_eq!(Param::LowGain.display(0.5), "0 dB");
        assert_eq!(Param::LowCut.display(0.0), "OFF");
        assert_eq!(Param::FilterType.display(1.0), "LP24");
        assert_eq!(Param::MacroX.display_for(ENGINE_WTFM, 0.0), "SINE");
        assert_eq!(Param::MacroX.display_for(ENGINE_WTFM, 1.0), "FAT");
        assert_eq!(Param::MacroX.display_for(ENGINE_FM, 0.0), "x0.250");
        assert_eq!(Param::MacroY.display_for(ENGINE_RING, 1.0), "x8.000");
        assert_eq!(Param::MacroZ.display_for(ENGINE_WTFM, 1.0), "x8.000");
        assert_eq!(Param::MacroZ.display_for(ENGINE_RING, 1.0), "100%");
        assert_eq!(Param::MacroX.label_for(ENGINE_SUPER), "DETUNE");
        assert_eq!(Param::Decay.label_for(ENGINE_PD), "DECAY");
        assert_eq!(Param::MacroU.label_for(ENGINE_VA), "SUB");
        assert_eq!(Param::MacroW.label_for(ENGINE_WTFM), "BEND");
        assert_eq!(Param::MacroU.display_for(ENGINE_SYNC, 0.0), "ENV");
        assert_eq!(Param::MacroV.display_for(ENGINE_FM, 0.0), "OFF");
        assert_eq!(Param::XCutoff.display(0.5), "OFF");
        assert_eq!(Param::XChain.display(0.0), "OFF");
        assert_eq!(Param::XSource.display(1.0), "OUT");
    }

    #[test]
    fn st1_ids_are_unchanged() {
        // Saved panels and the worklet's wire ids from the ST-1 still work.
        assert_eq!(Param::ReverbPredelay.id(), 61);
        assert_eq!(Param::MacroU.id(), 62);
        assert_eq!(CHAOS_CUTOFF, 4);
        for (slot, m) in MACROS.iter().enumerate() {
            assert_eq!(m.macro_slot(), Some(slot));
        }
        for p in XMOD_PARAMS {
            assert!(p.is_xmod() && !p.is_performance());
        }
    }

    #[test]
    fn every_macro_has_a_label_and_display() {
        for e in 0..ENGINE_COUNT as u32 {
            for slot in 0..MACRO_COUNT {
                assert!(!macro_label(e, slot).is_empty());
                assert!(!macro_name(e, slot).is_empty());
                for v in [0.0, 0.33, 0.5, 1.0] {
                    assert!(!macro_display(e, slot, v).is_empty());
                }
            }
        }
    }

    #[test]
    fn every_param_displays() {
        for p in ALL_PARAMS {
            for v in [0.0, 0.5, 1.0] {
                assert!(!p.display(p.sanitize(v)).is_empty(), "{p:?}");
            }
        }
    }
}
