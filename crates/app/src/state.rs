//! Panel state, the controller shared through the Dioxus context, and
//! persistence (localStorage on the web, `~/.config/stabkussion` on desktop).

use std::cell::RefCell;
use std::rc::Rc;

use dioxus::prelude::*;
use stabkussion_dsp::params::default_patch;
pub use stabkussion_dsp::random::{
    Locks, NO_LOCKS, lockable, mutate_patch, random_patch, random_xmod,
};
use stabkussion_dsp::util::Noise;
use stabkussion_dsp::{ALL_PARAMS, Param, VOICES, VoicePatch};

use crate::audio::{AudioHandle, Command, ParamCache};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PanelState {
    pub voices: [VoicePatch; VOICES],
    pub master: f32,
    /// Per-channel MUTATE knob: 0 = off … 1 = full randomization on every
    /// trigger. A performance control rather than part of the sound, so it
    /// lives outside the patch and survives presets, RND and channel copies.
    pub mutate: [f32; VOICES],
    /// Per-channel parameter locks (Ctrl-click a control). Like MUTATE they
    /// belong to the channel, not the patch: presets and copies leave them be.
    pub locks: [Locks; VOICES],
}

impl Default for PanelState {
    fn default() -> Self {
        // Give the two channels different starting sounds.
        let preset = |name: &str| {
            stabkussion_dsp::presets::PRESETS
                .iter()
                .find(|p| p.name == name)
                .map(|p| p.patch())
                .unwrap_or_else(default_patch)
        };
        let mut voices = [preset("Dub Minor 7"), preset("Drumcode FM")];
        voices[0][Param::Pan.index()] = 0.5;
        Self {
            voices,
            master: stabkussion_dsp::engine::DEFAULT_MASTER,
            mutate: [0.0; VOICES],
            locks: [NO_LOCKS; VOICES],
        }
    }
}

pub fn mutate_display(amount: f32) -> String {
    if amount <= 0.0 {
        "OFF".into()
    } else if amount >= 1.0 {
        "RND".into()
    } else {
        format!("{:.0}%", amount * 100.0)
    }
}

const FORMAT_HEADER: &str = "stabkussion-st2-state 1";
/// ST-1 panels load too: every ST-1 control is still there under its name,
/// and the new ones keep their (neutral) defaults.
const ST1_HEADER: &str = "stabkussion-st1-state 1";

impl PanelState {
    pub fn to_cache(self) -> ParamCache {
        ParamCache {
            voices: self.voices,
            master: self.master,
        }
    }

    pub fn serialize(&self) -> String {
        let mut out = format!("{FORMAT_HEADER}\nmaster {}\n", self.master);
        for (v, m) in self.mutate.iter().enumerate() {
            out += &format!("mutate {v} {m}\n");
        }
        for (v, locks) in self.locks.iter().enumerate() {
            for p in ALL_PARAMS.iter().filter(|p| locks[p.index()]) {
                out += &format!("lock {v} {p:?}\n");
            }
        }
        for (v, patch) in self.voices.iter().enumerate() {
            for p in ALL_PARAMS {
                out += &format!("{v} {p:?} {}\n", patch[p.index()]);
            }
        }
        out
    }

    /// Lenient: unknown lines are skipped and missing values keep defaults.
    pub fn deserialize(text: &str) -> Option<Self> {
        let mut lines = text.lines();
        let header = lines.next()?.trim();
        if header != FORMAT_HEADER && header != ST1_HEADER {
            return None;
        }
        let param_named = |name: &str| {
            ALL_PARAMS
                .iter()
                .copied()
                .find(|p| format!("{p:?}") == name)
        };
        let mut state = PanelState::default();
        for line in lines {
            let parts: Vec<&str> = line.split_whitespace().collect();
            match parts.as_slice() {
                ["master", v] => {
                    if let Ok(v) = v.parse::<f32>()
                        && v.is_finite()
                    {
                        state.master = v.clamp(0.0, 1.0);
                    }
                }
                ["mutate", voice, v] => {
                    if let (Ok(voice), Ok(v)) = (voice.parse::<usize>(), v.parse::<f32>())
                        && let Some(m) = state.mutate.get_mut(voice)
                        && v.is_finite()
                    {
                        *m = v.clamp(0.0, 1.0);
                    }
                }
                ["lock", voice, name] => {
                    if let (Ok(voice), Some(p)) = (voice.parse::<usize>(), param_named(name))
                        && let Some(locks) = state.locks.get_mut(voice)
                        && lockable(p)
                    {
                        locks[p.index()] = true;
                    }
                }
                [voice, name, v] => {
                    let (Ok(voice), Ok(v)) = (voice.parse::<usize>(), v.parse::<f32>()) else {
                        continue;
                    };
                    if let (Some(patch), Some(p)) = (state.voices.get_mut(voice), param_named(name))
                    {
                        patch[p.index()] = p.sanitize(v);
                    }
                }
                _ => {}
            }
        }
        Some(state)
    }
}

/// What a drag gesture is adjusting.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Target {
    Param(usize, Param),
    Master,
    /// A channel's MUTATE knob.
    Mutate(usize),
}

impl Target {
    /// The lockable parameter behind this control, if any.
    pub fn lockable(self) -> Option<(usize, Param)> {
        match self {
            Target::Param(v, p) if lockable(p) => Some((v, p)),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Drag {
    pub target: Target,
    pub start_y: f64,
    pub start_value: f32,
    /// Pixels of vertical travel for the full range.
    pub span_px: f64,
    pub fine: bool,
}

/// Everything the controls need, shared via `use_context::<Synth>()`.
#[derive(Clone)]
pub struct Synth {
    pub state: Signal<PanelState>,
    pub audio: AudioHandle,
    /// Last touched parameter per channel, for the channel display.
    pub touched: Signal<[Option<Param>; VOICES]>,
    /// Trigger counters per channel; the trigger LED keys its flash on these.
    pub hits: Signal<[u32; VOICES]>,
    pub drag: Signal<Option<Drag>>,
    /// One generator for RND and MUTATE, so rapid hits never share a seed.
    rng: Rc<RefCell<Noise>>,
}

impl Synth {
    pub fn new() -> Self {
        let initial = load().unwrap_or_default();
        Self {
            audio: AudioHandle::new(initial.to_cache()),
            state: Signal::new(initial),
            touched: Signal::new([None; VOICES]),
            hits: Signal::new([0; VOICES]),
            drag: Signal::new(None),
            rng: Rc::new(RefCell::new(Noise::new(random_seed()))),
        }
    }

    pub fn value(&self, target: Target) -> f32 {
        let s = self.state.read();
        match target {
            Target::Param(v, p) => s.voices[v][p.index()],
            Target::Master => s.master,
            Target::Mutate(v) => s.mutate[v],
        }
    }

    pub fn set(&self, target: Target, value: f32) {
        match target {
            Target::Param(voice, param) => self.set_param(voice, param, value),
            Target::Master => {
                let value = value.clamp(0.0, 1.0);
                let mut state = self.state;
                if state.peek().master != value {
                    state.write().master = value;
                    self.audio.send(Command::Master(value));
                }
            }
            Target::Mutate(voice) => {
                // UI-only: nothing to send, it acts when the channel is triggered.
                let value = value.clamp(0.0, 1.0);
                let mut state = self.state;
                if state.peek().mutate[voice] != value {
                    state.write().mutate[voice] = value;
                }
            }
        }
    }

    pub fn set_param(&self, voice: usize, param: Param, value: f32) {
        let value = param.sanitize(value);
        let mut state = self.state;
        if state.peek().voices[voice][param.index()] != value {
            state.write().voices[voice][param.index()] = value;
            self.audio.set_param(voice, param, value);
        }
        self.touch(voice, param);
    }

    fn touch(&self, voice: usize, param: Param) {
        let mut touched = self.touched;
        if touched.peek()[voice] != Some(param) {
            touched.write()[voice] = Some(param);
        }
    }

    pub fn reset(&self, target: Target) {
        match target {
            Target::Param(_, p) => self.set(target, p.default_value()),
            Target::Master => self.set(target, stabkussion_dsp::engine::DEFAULT_MASTER),
            Target::Mutate(_) => self.set(target, 0.0),
        }
        self.save();
    }

    // --- locks ---------------------------------------------------------------

    /// Reactive: re-renders the caller when the lock changes.
    pub fn is_locked(&self, target: Target) -> bool {
        target
            .lockable()
            .is_some_and(|(v, p)| self.state.read().locks[v][p.index()])
    }

    /// Ctrl-click: lock or unlock a control. Returns whether it was lockable.
    pub fn toggle_lock(&self, target: Target) -> bool {
        let Some((voice, param)) = target.lockable() else {
            return false;
        };
        let mut state = self.state;
        let locked = !state.peek().locks[voice][param.index()];
        state.write().locks[voice][param.index()] = locked;
        self.touch(voice, param);
        self.save();
        true
    }

    pub fn lock_count(&self, voice: usize) -> usize {
        self.state.read().locks[voice]
            .iter()
            .filter(|&&l| l)
            .count()
    }

    pub fn clear_locks(&self, voice: usize) {
        let mut state = self.state;
        state.write().locks[voice] = NO_LOCKS;
        self.save();
    }

    // --- patches -------------------------------------------------------------

    pub fn load_patch(&self, voice: usize, patch: &VoicePatch) {
        self.apply_patch(voice, patch);
        self.save();
    }

    /// Set a whole patch, sending only the values that changed. The channel
    /// display falls back to its summary (engine, tune, decay).
    fn apply_patch(&self, voice: usize, patch: &VoicePatch) {
        let mut state = self.state;
        for p in ALL_PARAMS {
            let value = p.sanitize(patch[p.index()]);
            if state.peek().voices[voice][p.index()] != value {
                state.write().voices[voice][p.index()] = value;
                self.audio.set_param(voice, p, value);
            }
        }
        let mut touched = self.touched;
        touched.write()[voice] = None;
    }

    /// Randomize the free sound-shaping controls, leaving level, sense, pan
    /// and anything locked alone.
    pub fn randomize(&self, voice: usize) {
        let (current, locks) = {
            let s = self.state.peek();
            (s.voices[voice], s.locks[voice])
        };
        let patch = random_patch(&current, &locks, &mut self.rng.borrow_mut());
        self.load_patch(voice, &patch);
    }

    /// Load a scene: both channels, cross-modulation included. Like a
    /// preset it sets locked controls too (and leaves them locked).
    pub fn load_scene(&self, scene: &stabkussion_dsp::presets::Scene) {
        for (voice, patch) in scene.patches().iter().enumerate() {
            self.apply_patch(voice, patch);
        }
        self.save();
    }

    /// The CROSS-MOD panel's RND: new cross-modulation both ways (locked
    /// controls stay put), the rest of both channels untouched.
    pub fn randomize_xmod(&self) {
        for voice in 0..VOICES {
            let (current, locks) = {
                let s = self.state.peek();
                (s.voices[voice], s.locks[voice])
            };
            let patch = random_xmod(&current, &locks, &mut self.rng.borrow_mut());
            self.apply_patch(voice, &patch);
        }
        self.save();
    }

    /// Switch all cross-modulation off, both ways (locked controls stay).
    pub fn clear_xmod(&self) {
        for voice in 0..VOICES {
            let (mut patch, locks) = {
                let s = self.state.peek();
                (s.voices[voice], s.locks[voice])
            };
            for p in stabkussion_dsp::params::XMOD_PARAMS {
                if !locks[p.index()] {
                    patch[p.index()] = p.default_value();
                }
            }
            self.apply_patch(voice, &patch);
        }
        self.save();
    }

    pub fn copy_voice(&self, from: usize, to: usize) {
        let mut patch = self.state.peek().voices[from];
        // Keep the destination's placement in the stereo field.
        patch[Param::Pan.index()] = self.state.peek().voices[to][Param::Pan.index()];
        self.load_patch(to, &patch);
    }

    /// Hit a channel. With MUTATE above zero the channel's patch is mutated
    /// first, and the trigger snaps the voice so the new sound starts exactly
    /// on this hit rather than gliding in.
    pub fn trigger(&self, voice: usize, velocity: f32) {
        self.audio.user_gesture();
        let amount = self.state.peek().mutate[voice];
        let mutated = amount > 0.0;
        if mutated {
            let (current, locks) = {
                let s = self.state.peek();
                (s.voices[voice], s.locks[voice])
            };
            let patch = mutate_patch(&current, amount, &locks, &mut self.rng.borrow_mut());
            self.apply_patch(voice, &patch);
            self.save();
        }
        self.audio.trigger(voice, velocity, mutated);
        let mut hits = self.hits;
        hits.write()[voice] += 1;
    }

    pub fn save(&self) {
        save(&self.state.peek());
    }
}

#[cfg(all(feature = "web", not(feature = "desktop")))]
fn random_seed() -> u32 {
    (js_sys::Math::random() * u32::MAX as f64) as u32
}

#[cfg(not(all(feature = "web", not(feature = "desktop"))))]
fn random_seed() -> u32 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos() ^ d.as_secs() as u32)
        .unwrap_or(12345)
}

// --- persistence -----------------------------------------------------------

#[cfg(all(feature = "web", not(feature = "desktop")))]
const STORAGE_KEY: &str = "stabkussion-st2.state";

#[cfg(all(feature = "web", not(feature = "desktop")))]
fn load() -> Option<PanelState> {
    let storage = web_sys::window()?.local_storage().ok()??;
    PanelState::deserialize(&storage.get_item(STORAGE_KEY).ok()??)
}

#[cfg(all(feature = "web", not(feature = "desktop")))]
fn save(state: &PanelState) {
    if let Some(Ok(Some(storage))) = web_sys::window().map(|w| w.local_storage()) {
        let _ = storage.set_item(STORAGE_KEY, &state.serialize());
    }
}

#[cfg(not(all(feature = "web", not(feature = "desktop"))))]
fn state_path() -> Option<std::path::PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .filter(|s| !s.is_empty())
        .map(std::path::PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".config"))
        })?;
    Some(base.join("stabkussion-st2").join("state.txt"))
}

#[cfg(not(all(feature = "web", not(feature = "desktop"))))]
fn load() -> Option<PanelState> {
    PanelState::deserialize(&std::fs::read_to_string(state_path()?).ok()?)
}

#[cfg(not(all(feature = "web", not(feature = "desktop"))))]
fn save(state: &PanelState) {
    let Some(path) = state_path() else { return };
    let result = path
        .parent()
        .map(std::fs::create_dir_all)
        .unwrap_or(Ok(()))
        .and_then(|_| std::fs::write(&path, state.serialize()));
    if let Err(e) = result {
        eprintln!(
            "stabkussion: could not save state to {}: {e}",
            path.display()
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mutate_display_names_the_ends() {
        assert_eq!(mutate_display(0.0), "OFF");
        assert_eq!(mutate_display(0.5), "50%");
        assert_eq!(mutate_display(1.0), "RND");
    }

    #[test]
    fn state_roundtrip() {
        let mut s = PanelState {
            master: 0.33,
            mutate: [0.5, 1.0],
            ..Default::default()
        };
        s.voices[0][Param::Engine.index()] = 4.0;
        s.voices[1][Param::Pitch.index()] = 0.123;
        s.locks[0][Param::Pitch.index()] = true;
        s.locks[1][Param::Engine.index()] = true;
        s.locks[1][Param::ChorusMix.index()] = true;
        s.voices[0][Param::MacroW.index()] = 0.7;
        s.voices[1][Param::XDuck.index()] = 0.6;
        s.locks[0][Param::XChain.index()] = true;
        let back = PanelState::deserialize(&s.serialize()).unwrap();
        assert_eq!(back, s);
    }

    #[test]
    fn loads_an_st1_panel_with_neutral_new_controls() {
        let text = format!("{ST1_HEADER}\nmaster 0.5\n0 Engine 3\n1 MacroZ 0.25\n");
        let s = PanelState::deserialize(&text).unwrap();
        assert_eq!(s.voices[0][Param::Engine.index()], 3.0);
        assert_eq!(s.voices[1][Param::MacroZ.index()], 0.25);
        for p in [Param::MacroU, Param::XFm, Param::XChain] {
            assert_eq!(s.voices[1][p.index()], 0.0);
        }
    }

    #[test]
    fn rejects_foreign_text() {
        assert!(PanelState::deserialize("hello").is_none());
    }

    #[test]
    fn tolerates_garbage_lines() {
        let text = format!(
            "{FORMAT_HEADER}\nmaster 0.5\n0 Nope 1\n7 Pitch 0.2\n0 Pitch abc\n1 Engine 3\n\
             lock 0 Nope\nlock 9 Pitch\nlock 0 Level\nlock 1 Decay\n"
        );
        let s = PanelState::deserialize(&text).unwrap();
        assert_eq!(s.master, 0.5);
        assert_eq!(s.voices[1][Param::Engine.index()], 3.0);
        // Performance controls can't be locked.
        assert!(!s.locks[0][Param::Level.index()]);
        assert!(s.locks[1][Param::Decay.index()]);
        assert_eq!(s.locks.iter().flatten().filter(|&&l| l).count(), 1);
    }

    /// Both channels at RND: one press of Space gives both a brand new sound.
    #[test]
    fn space_at_rnd_rerolls_both_channels() {
        let mut state = PanelState {
            mutate: [1.0; VOICES],
            ..Default::default()
        };
        let mut rng = Noise::new(5);
        for _ in 0..20 {
            let before = state.voices;
            for (patch, (&amount, locks)) in state
                .voices
                .iter_mut()
                .zip(state.mutate.iter().zip(&state.locks))
            {
                *patch = mutate_patch(patch, amount, locks, &mut rng);
            }
            for (now, then) in state.voices.iter().zip(&before) {
                assert_ne!(now, then);
            }
            assert_ne!(state.voices[0], state.voices[1]);
        }
    }
}
