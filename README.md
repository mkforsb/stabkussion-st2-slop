# Stabkussion ST-2

The successor to [Stabkussion ST-1](https://github.com/mkforsb/stabkussion-st1-slop),
built on one premise: **more is better**.

ST-1 is a two-channel **techno stab** synthesizer: dub chords, FM clangs,
gated minor sevenths, ring-modulated hits and filtered plucks. Each channel
runs four freely tuned oscillators of one of eight engines through a ladder
filter, saturator, EQ, chorus, ping-pong echo and plate reverb, with MUTATE,
RND and per-control locks. ST-2 keeps all of that and adds:

- **Twice the engine knobs.** Every engine has six macros instead of three:
  ST-1's X, Y and Z, plus **U, V and W**, each one a real synthesis feature
  of that engine (a Juno sub and PWM rate, windowed sync, a hypersaw octave
  pair, an OPL half-sine carrier, a carrier→modulator feedback loop, a diode
  ring with its own fixed oscillator, a second CZ line, wavetable phase bend,
  and 16 more). All 24 of them are **neutral at zero**, so every ST-1 patch
  and preset sounds as it did on the ST-1: rendered by both, the 33 factory
  presets match sample for sample to within 7 LSB at 16 bits (float
  rounding; 27 of them bit for bit).
- **A CROSS-MOD panel** where the two channels modulate and circuit-bend
  each other: audio-rate exponential FM, ring modulation, audio-rate filter
  FM, a sample & hold clocked by the other channel's zero crossings, the
  other's envelope on cutoff, pitch or any macro, sidechain ducking, and
  **chain triggers** that bounce a hit back and forth between the channels.
- **Scenes**: both channels and their cross-modulation in one go.
- Eight new presets, one per engine, built on the new macros.

RND and MUTATE reach all of it (the new macros about half the time, the
cross-modulation rarely and gently; the panel has its own, bolder RND), and
every new control can be locked. RND still lands **98%** of its hits among
the reference stabs, the same as ST-1 (see [RND](#rnd)).

It's written in Rust with a [Dioxus](https://dioxuslabs.com) UI and builds for
two targets from one codebase:

| Target | Audio | How |
| --- | --- | --- |
| Web (wasm) | WebAudio `AudioWorklet` | `make serve-web` |
| Native Linux | PulseAudio (also PipeWire's pulse server) | `make run-desktop` |

## Building

Requirements:

- Rust (stable) with `rustup target add wasm32-unknown-unknown`
- Dioxus CLI 0.7: `cargo binstall dioxus-cli` or grab `dx` from the Dioxus releases
- Desktop only (Debian/Ubuntu package names): `libwebkit2gtk-4.1-dev libgtk-3-dev libxdo-dev libssl-dev libpulse-dev pkg-config`

```sh
make serve-web      # http://127.0.0.1:8080, click once to enable audio
make run-desktop    # native window
make web desktop    # release bundles in target/dx/stabkussion/release/
make test lint
make renders        # offline-render every preset to ./renders/*.wav
make scenes         # offline-render every cross-mod scene to ./renders/scene_*.wav
make rnd-rate       # how stab-like RND is, per engine (no reference WAVs needed)
make bench          # engine speed as a real-time factor
make calibrate      # loudness survey of every engine
make stabs          # how stab-like RND is, against the WAVs in ./examples
make fit            # fit a patch to every WAV in ./examples (slow)
```

For the desktop build, `STABKUSSION_LATENCY_MS` (default 20) sets the
PulseAudio target latency.

## Playing it

- **Pads**: click a channel's STRIKE pad. Where you click sets the velocity:
  near the top is a hard hit, near the bottom a soft one. Velocity sets the
  level, how far the filter envelope opens, and on most engines the
  brightness too (FM index, sync sweep, distortion).
- **Keyboard**: `Z X C V` trigger channel 1 at increasing velocity,
  `N M , .` do the same for channel 2, and `Space` triggers both. A
  channel can also be struck by the other one through its CHAIN control.
- **Knobs/faders**: drag vertically. Hold `Shift` for fine control, use the
  scroll wheel to nudge, and double-click to reset.
- **Locks**: **Ctrl-click** (or ⌘-click) any knob, fader, switch or engine
  button, including the CROSS-MOD ones, to lock it. A locked control shows a padlock and turns rose, and
  RND and MUTATE leave it exactly where it is. Ctrl-click again to unlock.
  The padlock counter in the channel header shows how many controls are
  locked; click it to unlock them all. Locks belong to the channel, not the
  patch: loading a preset or copying a channel changes locked values but
  keeps them locked. They are saved with the rest of the panel.
  - Everything that RND can change is lockable. OUTPUT, SENSE and PAN are
    never randomized, so they have no lock.
  - Locking never changes what the free controls roll. Every control still
    draws its own random number, in a fixed order, whether or not it's
    locked.
  - The rules RND applies afterwards (below) only move free controls.
- **MUTATE** (the knob under each pad) changes the channel's sound on every
  hit. Each trigger draws a fresh random patch, as RND would, and moves each
  free control that fraction of the way towards it:
  `clamp(current + (random − current) × MUTATE)`. At **RND** every hit is a
  new stab. Lock PITCH and the four TUNEs, set MUTATE to 30% and the chord
  stays put while its timbre wanders.
- **Presets**, **RND** and **→ CHn** (copy a channel to the other) are in
  each channel's header. The whole panel is saved automatically: to
  `localStorage` on the web, and to `~/.config/stabkussion-st2/state.txt` on
  the desktop. The ST-2 reads ST-1 state files too (copy one across to bring
  an old panel over); the new controls start at their neutral defaults.
- **CROSS-MOD** is the panel above the channels; see
  [Cross-modulation](#cross-modulation). Its **Scene…** menu loads both
  channels at once, its **RND** rolls new cross-modulation both ways
  (locked controls stay put), and **OFF** switches it all off.
- The **HIT** display is an offline render of one hit of the current patch,
  and the **EQ** display is the EQ's actual response, computed by the same
  filter code the voice runs.

## The oscillators

Every engine is an oscillator type. A channel runs four of them, **OSC 1–4**:

- **PITCH** (16 Hz – 1 kHz) and **FINE** tune OSC 1. PITCH is continuous, not
  quantized to notes.
- **TUNE** sets OSC 2, 3 and 4 relative to OSC 1, ±24 semitones,
  continuous. Shift-drag for hundredths of a semitone. A minor seventh
  stab is +3, +7, +10; just intervals (+7.02 for 3:2), slightly sour
  chords, octave doublings a few cents off and inharmonic clusters are
  all a turn away.
- **LEVEL** sets each oscillator's level. Stacked oscillators are scaled back
  towards the loudness of one.
- **DECAY** for OSC 2–4 is a multiple of the main DECAY (×0.06 … ×2): a
  chord tone that dies before the root, or one that outlasts it.
- All of them share the envelope and follow the **SWEEP**, the chaos
  modulator and the per-hit **JITTER**.

## The engines

The six big knobs under the faders are the engine's macros, shared by all
four oscillators: X, Y and Z (as on the ST-1) and, below them, U, V and W.
They are relabelled for each engine, and so are the chaos modulator's and
the cross-mod →MACRO's DEST switches.

| Engine | Model | X | Y | Z |
| --- | --- | --- | --- | --- |
| **VA** | Virtual analog: polyBLEP saw and pulse on one phase. Each oscillator gets its own slow PWM LFO, at a rate and phase drawn per hit | WAVE: saw → pulse | WIDTH: 50% … 5% | PWM: depth |
| **SYNC** | polyBLEP sine/tri/saw/square hard-synced to a hidden master, whose sine is blended back in as the ratio rises | WAVE | SYNC: ×1 … ×8 | SWEEP: extra octaves of ratio at the envelope peak |
| **SUPER** | Five detuned polyBLEP voices per oscillator | DETUNE: ±0 … 80 cents | BLEND: side voices vs centre | WAVE: saw → square |
| **FM** | Two-operator FM with its own index envelope | RATIO: 0.25 … 8, continuous | INDEX: 0 … 12 rad | MOD DEC: index decay ×0.03 … ×1 |
| **FB-FM** | A sine carrier modulated by a sine with self-feedback (DX-style), which bends the modulator towards a saw | RATIO: 0.25 … 8 | INDEX: 0 … 6 rad | FDBK: 0 … 1.4 rad |
| **RING** | A polyBLEP wave times a second wave at a free ratio: every harmonic *k* moves to *k ± r* | WAVE | RATIO: 0.25 … 8 | RING: dry wave … full ring modulation |
| **PD** | Casio CZ-style phase distortion | DIST | WAVE: saw → square → reso → pulse | D.ENV: how much of DIST follows the envelope |
| **WT-FM** | A band-limited wavetable (SINE … SAW, FAT) phase-modulated by a sine whose index decays | POS | INDEX: 0 … 6 rad | RATIO: 0.25 … 8 |

### U, V and W (new in ST-2)

At zero each of these does nothing at all, so an ST-1 patch is unchanged
(to within float rounding).

| Engine | U | V | W |
| --- | --- | --- | --- |
| **VA** | **SUB**: a polyBLEP square one octave down, locked to the main phase (Juno-style) | **RATE**: PWM LFO speed ×1 … ×64, up to the edge of audio rate where PWM turns into a growl | **DRIFT**: per-hit random detune plus a slow wander, up to ±25 cents per oscillator |
| **SYNC** | **Z.DEC**: gives the sync zap its own decay (×1 … ×0.02 of the oscillator's) instead of following the amp envelope: the short "bwow" at the front of a long stab | **FM**: the master's sine frequency-modulates the slave (±90%), the sync+FM tone of a complex oscillator; periodic in the master's cycle, so still harmonic | **WINDOW**: fades each master cycle of the slave in and out with a Hann window, from hard sync to windowed (formant) sync, a vocal peak at the slave frequency with no reset edge at all |
| **SUPER** | **OCTAVE**: a detuned pair one octave up ("hypersaw" sheen) | **SUB**: a sine one octave down | **BLOOM**: up to +60 cents of extra detune at the envelope peak, settling as the hit decays: the stab opens wide and closes in |
| **FM** | **M.SUS**: the level the index decays to instead of nothing | **BEAT**: offsets the modulator by up to 12 Hz: DX-style detuned-operator shimmer, and every chord tone beats at the same rate | **C.WAVE**: the carrier's waveform, sine → OPL-style half sine (DC-free) → soft square |
| **FB-FM** | **I.DEC**: how much of the (otherwise held) index decays, quickly | **LOOP**: the carrier fed back into the modulator's phase (up to 1.6 rad), Buchla-style cross-coupled FM: grit at low settings, a tumbling chaotic snarl at high ones | **M.SWP**: the modulator ratio starts up to 3 octaves higher and falls to RATIO in ~120 ms: an inharmonic laser zap |
| **RING** | **R.WAVE**: the ring modulator's wave, sine → triangle → square (polyBLEP); a square is the harsh diode-ring sound | **R.ENV**: the ratio starts up to 3 octaves higher at the envelope peak and falls back | **FIXED**: moves the ring modulator from tracking the pitch to a fixed RATIO × 110 Hz, like a hardware ring mod with its own oscillator: all four oscillators share it, so a chord becomes one inharmonic cluster |
| **PD** | **D.DEC**: the distortion envelope (the CZ's DCW) gets its own, faster decay | **LINE 2**: a second line, detuned up to 25 cents and faded in (the CZ's 1+2′) | **NOISE**: lowpassed noise on the phase, the CZ-1000's noise modulation: breath, then rasp |
| **WT-FM** | **SCAN**: the table position follows the index envelope, starting further along the table and sweeping back to POS with the bark | **SPREAD**: a second carrier reading the same table, detuned up to 30 cents | **BEND**: phase bend `p^(1…4)`, a wavetable phase warp; the mip level follows the bend's steepest slope, so it stays band-limited |

The chaos modulator's DEST now reaches U, V and W as well, and JITTER
offsets all six macros per hit. Jitter never switches U, V or W on from
zero, though.

FM, FB-FM and RING at whole-number ratios are harmonic; anywhere else they
are the inharmonic, metallic stabs. Velocity scales the FM indices.

### Anti-aliasing

The voices run at 2× the output rate and a 47-tap windowed-sinc filter
decimates each one, so a partial between fs/2 and fs − 24 kHz folds back
above 24 kHz, where the decimator removes it.

- **polyBLEP** corrects every jump of VA, SYNC, SUPER and RING, including
  VA's moving pulse edge (the width changes every sample and the edge is
  found where it crosses the phase) and SYNC's resets.
- PD's waves only have slope breaks; the FM engines are sums of sines (the
  feedback path is averaged over two samples so it can't break into
  noise). WT-FM's mipmapped tables pick their bandwidth for the modulated
  phase.
- A test runs every engine at bright settings through an FFT and requires
  the inharmonic energy (aliases) in 20 Hz – 20 kHz to stay below −45 dB.
  A second one does the same with the new macros that keep an oscillator
  periodic switched on (VA's sub, SYNC's FM and window, SUPER's octave pair
  and sub, FM's held index and soft-square carrier, FB-FM's index decay,
  RING's square ring modulator, PD's distortion decay, WT-FM's scan and
  full bend).

## The rest of the channel

```
 PITCH ─┬─ SWEEP ─┬─ CHAOS ─┬─ JITTER ─► pitch ─┬── +TUNE2 ─┬── +TUNE3 ─┬── +TUNE4
        ▼                                     OSC 1      OSC 2      OSC 3      OSC 4  (the engine, ×4, each × LEVEL × its envelope)
 TRIGGER ─► amp envelopes ─────────────────────┴──────────┴──────────┴──────────┴─► Σ + CLICK + NOISE
        └─► filter envelope ─┐                                                       │
                             ▼                                                       │
 [SAT pre] ◄─────────────────┼───────────────────────────────────────────────────────┘
     ▼                       ▼
 LADDER (cutoff + KEY + F.ENV) ─► EQ ─► [SAT post] ─► OUTPUT ─► CHORUS ─► ECHO ─┬─────────► PAN ─► mix
                                                                                └─► PLATE ─┘
```

- **ENVELOPE**: ATTACK (0.5–200 ms) rises to the hit's velocity; then each
  oscillator decays towards **SUSTAIN** at its own rate (DECAY, 30 ms – 6
  s, times its multiple). After **GATE** (30 ms – 3 s, or HOLD at the top)
  everything fades out with **RELEASE** (3 ms – 3 s). With the gate held and
  no sustain it's a plain percussive decay; a short gate, some sustain and
  a fast release give the flat, chopped stab.
- **SWEEP · TRANSIENT**: a pitch sweep (TIME, bipolar DEPTH, ±4 octaves),
  and the CLICK and NOISE burst. Both run through the filter.
- **CHAOS**: a slow Lorenz attractor used as an LFO that never repeats. RATE,
  DEPTH and a DEST (pitch, X, Y, Z or cutoff), plus JITTER: per-hit random
  offsets of pitch, macros and cutoff.
- **LADDER FILTER**: a 4-pole transistor ladder in zero-delay-feedback form
  (LP12, LP24, and Xpander-style BP and HP mixes). The cutoff follows:
  - **ENV**: the filter envelope, ±8 octaves, further on harder hits. It
    shares the attack and decays to nothing in **F.DEC** (10 ms – 4 s), so
    the stab opens bright and closes before it stops sounding.
  - **KEY**: key tracking. At 100% the cutoff moves with PITCH, so a chord
    keeps its brightness wherever it's played (relative to 100 Hz).
- **SATURATOR**: DRIVE, MIX and four curves (TAPE, TUBE, HARD, FOLD), before
  the filter or after the EQ.
- **EQ**: LO CUT (12 dB/oct, 10–400 Hz), a low shelf, a mid bell and a
  high shelf, ±18 dB. RBJ biquads; a flat band is bypassed.
- **CHORUS**: three taps swept 120° apart by one LFO, wide and hollow. Below
  its X-OVER the signal stays dry and mono.
- **ECHO**: a dub-style ping-pong delay. TIME (20 ms – 1.2 s, unsynced: it
  glides like tape when moved), FDBK (up to 92%) and TONE (a lowpass in the
  loop, 400 Hz – 12 kHz; a fixed highpass keeps the repeats from piling up
  lows, a soft clipper keeps high feedback bounded). AMOUNT is a send, so
  a tail keeps repeating when the next hit of a mutating patch turns it
  down.
- **REVERB**: Xenokussion's per-channel Dattorro plate, fed by the chorus
  and the echo. AMOUNT (post fader), DECAY, TONE and PRE. PAN acts as a
  balance on the stereo signal.

## Cross-modulation

The CROSS-MOD panel has one lane per direction: **CH2 → CH1** above
channel 1 and **CH1 → CH2** above channel 2. A lane's controls belong to
the *receiving* channel: they are part of its patch, so its RND, MUTATE,
locks, presets, → CHn copies and the saved panel all include them. A lane
glows while any of it is active.

```
 other channel ─ SRC (OSC: its oscillators before the filter / OUT: its output)
       ├─ FM ─── × every phase increment of this channel's oscillators, 2^(depth·signal)
       ├─ RING ─ × this channel's oscillator sum, while the other one sounds
       ├─ S&H ── hold this channel's oscillator sum between the other's zero crossings
       └─ F.FM ─ × this channel's ladder cutoff, 2^(depth·signal), every sample
 other channel's amplitude envelope
       ├─ →CUT ±5 oct · →PITCH ±2 oct · →MACRO ±a full knob turn, on DEST (X … W)
       └─ DUCK ─ × this channel's output (sidechain)
 a hit on the other channel ─ CHAIN (velocity ×0 … ×0.95) after C.TIME (5 ms … 1.5 s) ─► strikes this one
```

| Control | What it does |
| --- | --- |
| **FM** | Exponential, audio-rate FM of all four oscillators by the other channel's signal, up to 3 octaves per unit. The analog cross-mod sound: pitch-dependent, inharmonic sidebands that move as both channels decay. Both ways at once is circular FM |
| **RING** | Ring modulation by the other channel. Its depth follows the other's envelope and its carrier is the other's signal at a steady level, so it rings while the other plays and leaves this channel dry once it has stopped |
| **F.FM** | The other's signal sweeps this channel's ladder cutoff at audio rate (up to 5 octaves per unit): growl, sidebands, and with resonance, screaming |
| **S&H** | A sample & hold on this channel's oscillators, clocked by the other channel's zero crossings: a circuit-bent, stepped version of this chord that sings at the other's pitch. With nothing to clock it, it passes the signal through |
| **SRC** | Which of the other's signals the four above hear: **OSC** (its oscillators and transient, before its filter) or **OUT** (its output, after filter, EQ and saturator) |
| **→CUT**, **→PITCH**, **→MACRO** + **DEST** | The other channel's envelope on this one's cutoff, pitch, or any of its six macros (bipolar, centre off) |
| **DUCK** | Sidechain: turns this channel down by up to 100% of the other's envelope, the techno pump |
| **CHAIN**, **C.TIME** | A hit on the other channel strikes this one after C.TIME, at CHAIN × its velocity. Chained both ways, a single hit bounces back and forth, softer each time, until it falls below 3% (at most about 70 strikes) |

With the other channel silent, none of them changes anything: a test
renders a channel with everything up full against a silent partner and
requires the output to be identical, bit for bit.

Chained hits happen inside the engine, so they don't MUTATE and don't
flash the pads. The HIT preview shows a channel on its own, without
cross-modulation.

### Scenes

| Scene | Channel 1 | Channel 2 |
| --- | --- | --- |
| **Ping-Pong Bounce** | Dub Minor 7, chained to channel 2 | Drumcode FM, chained back: hit either and they bounce it, panned apart, every 170 ms |
| **Sidechain Pump** | Sync Zap Stab, the "kick" | PWM Dub Chord, held, ducked by channel 1 and struck with it |
| **Cross-FM Clang** | FM Bell Stab, FM'd by channel 2 | Ring Cluster, FM'd by channel 1 and struck with it: circular FM |
| **Zero-Cross Crush** | Super Low Chord, sampled at channel 2's zero crossings | Sync Minor Pluck, quietly, struck with channel 1 |
| **Envelope Bender** | Feedback Bark | Table Ninths: struck 65 ms later, bent down, opened up and its index pushed by channel 1's envelope |
| **Filter FM Growl** | Saw Chord Pluck with its ladder swept by channel 2's output | Feedback Drone, ring-modulated by channel 1 and struck with it |

`make scenes` renders each to `renders/scene_*.wav` (channel 1, then channel
2, then both).

## How close it gets

The reference set is 27 stabs (not part of this repository; put WAVs in
`./examples` to use the tools). The survey found:

- **Register**: the lowest partials sit at 35–165 Hz, but the weight is in
  the low mids: the spectral centroid is 150–530 Hz (median 260 Hz), and
  almost nothing is above 2 kHz.
- **Length**: −20 dB within 140–600 ms of the peak; several hold flat and
  then cut off (a gate).
- **The filter envelope**: the spectrum darkens by up to 4.7× within
  about 100 ms.
- **Chords**: many bodies are stacked intervals a little off equal
  temperament, e.g. 64/77/96 Hz (1 : 1.20 : 1.50, a minor triad).
- **Inharmonic FM**: partials at *c ± n·m* with ratios like 0.86.
- **Space**: mostly wide (side/mid 0.3–1.1), some with echoes.

### Fitting the references

`examples/fit.rs` searches the panel for each reference. It seeds a few
thousand patches across all engines with OSC 1 on one of the reference's
strongest partials (or an octave or a twelfth below) and OSC 2–4 often on
others, refines the best with a sparse (1+1) evolution strategy, and scores
each candidate on a two-resolution log-frequency spectrogram (mean absolute
dB difference, over the reference's length) plus the feature distance
below.

The results, for the presets as shipped (level-matched, rounded). The ST-2
leaves these presets exactly as they were; its U, V and W are at zero in
all of them:

| Reference | Preset | Engine | Error (dB) | Nearest other reference (dB) | Feature distance |
| --- | --- | --- | --- | --- | --- |
| sample-001 | Feedback Drone | FB-FM | 4.6 | 8.8 | 0.20 |
| sample-002 | Table Swell | WT-FM | 4.4 | 6.9 | 0.42 |
| sample-003 | Ring Cluster | RING | 4.6 | 8.1 | 0.20 |
| sample-004 | Sync Minor Pluck | SYNC | 4.6 | 9.0 | 0.26 |
| sample-005 | Ring Sustain | RING | 4.8 | 9.4 | 0.72 |
| sample-006 | Super Low Chord | SUPER | 4.3 | 9.1 | 0.15 |
| sample-007 | PD Organ Stab | PD | 5.3 | 9.2 | 0.47 |
| sample-008 | Saw Chord Pluck | VA | 3.9 | 6.7 | 0.15 |
| sample-009 | Super Fifths | SUPER | 4.5 | 8.8 | 0.23 |
| sample-010 | PD Pluck | PD | 4.0 | 10.5 | 0.62 |
| sample-011 | Detuned Table | WT-FM | 3.8 | 12.1 | 0.54 |
| sample-012 | Table Ninths | WT-FM | 4.9 | 6.4 | 0.10 |
| sample-013 | Dub Ring Echo | RING | 4.1 | 8.3 | 0.38 |
| sample-014 | Wide Table Gate | WT-FM | 4.7 | 9.9 | 0.52 |
| sample-015 | Short Ring Stab | RING | 5.9 | 11.3 | 0.36 |
| sample-016 | Chorus Saw Stab | VA | 5.3 | 10.0 | 0.44 |
| sample-017 | Dub Echo Chord | WT-FM | 5.2 | 9.3 | 0.70 |
| sample-018 | Feedback Bark | FB-FM | 4.5 | 11.8 | 0.42 |
| sample-019 | Closing Table | WT-FM | 4.7 | 8.2 | 0.26 |
| sample-020 | Mono Super Gate | SUPER | 4.2 | 11.5 | 0.37 |
| sample-021 | Pulse Gate | VA | 4.3 | 9.1 | 0.60 |
| sample-022 | Super Sub Stab | SUPER | 5.9 | 10.8 | 0.35 |
| sample-023 | Metal FM Hold | FB-FM | 4.5 | 5.9 | 0.76 |
| sample-024 | Metal FM Gate | FM | 5.1 | 4.6 | 0.51 |
| sample-025 | Low Table Clang | WT-FM | 4.8 | 11.2 | 0.34 |
| sample-026 | Ring Pluck | RING | 5.5 | 10.8 | 0.57 |
| sample-027 | FM Bell Stab | FM | 5.3 | 11.3 | 0.37 |

- **Mean error 4.7 dB**, against 9.2 dB from each reference to the nearest
  *other* reference. As a yardstick, the same measure rates a reference
  against itself played 25 cents sharp at 2.0 dB and a semitone sharp at
  4.9 dB: a fitted preset is about as close to its reference as the
  reference transposed by a semitone (`stabs yardstick`).
- Every preset is closer to its reference than any other reference is,
  except *Metal FM Gate*: its reference (024) has a near-twin (023) that is
  closer still.
- Every preset's features are within 0.76 of its reference's: closer than
  the median reference is to its nearest neighbour, bar four (0.62–0.76).
- What the fits miss: the echo repeats of 016 come out weaker, 022's
  bright first 50 ms is a little dull, and the references that are cut off
  at the end of the file ring on in the fits (only the reference's length
  is scored).

Spectrogram sheets of reference vs fit: `python3 tools/fit_sheet.py fits
out.png` after `make fit`.

### RND

`random_patch` draws every control from a range taken from the survey, the
fitted patches and a sensitivity analysis of the misses:

- a root of 85–165 Hz;
- OSC 2–4 on 70% of draws a chord tone (fifths, minor thirds and
  sevenths, octaves, fourths, ninths, a few major thirds) up to 25 cents
  off true, on 15% the octave a few cents off, on 15% anywhere from an
  octave down to an octave and a half up; their decays ×0.28–×1 of the
  root's;
- decays of 0.55–1.8 s, gated half the time (0.22–0.7 s, with up to 50%
  sustain);
- always a lowpass at 160–450 Hz for a 100 Hz root (with 20–70% key
  tracking), opened 2–4.8 octaves by a filter envelope that closes in
  0.27–1.2 s; resonance up to 45%;
- a mid EQ of −1 to +4 dB (cutting the low mids takes the stab out of the
  stab), chorus always, echo half the time, always some plate;
- each engine's macros from the part of their range that stays tonal:
  FM, FB-FM and RING ratios of 1–4.6 (below 1 the sidebands land under the
  root), PD that doesn't relax all the way to a sine, and so on.

Then a few rules, which only move free controls:

- OSC 2–4 fold by octaves into 55–330 Hz, and an oscillator within 0.6
  semitones of another moves to the octave furthest from the rest (two
  near-unison sine-like waves beat the stab away a few hundred
  milliseconds in);
- the cutoff moves by a per-engine amount (a synced or unison saw is
  brighter than a sine-based FM at the same cutoff) and stays at 1.3× the
  root or more (below that, the stab is just the filter envelope's burst);
- a low cut at 0.5–0.85 of the root removes FM and ring-modulation
  difference tones;
- the gate stays open 220 ms past the attack, and sustain is only used on
  gated notes.

RND's cross-modulation is mostly off: a ducked channel a quarter of the
time, and each of the others on 4–15% of rolls, gently. The panel's own RND
(`random_xmod`) rolls only the cross-modulation and goes much further.

**Is a random hit a stab like the references?** `analysis::Features`
measures nine things about a hit: the spectral centroid, the frequencies
below which 10/50/90% of the energy lies, the time to the peak, the time
from the peak to −20 dB, how much darker the 100–200 ms after the peak
are than the first 40 ms, the stereo width and the spectral flatness.
Each is scaled by its spread across the references. A hit counts as a match
when it is at least as close to some reference as the loneliest reference
is to its nearest neighbour. Over 6,000 RND hits (MUTATE at RND does
exactly the same per hit) rendered through the whole engine:

- **97.9%** match on the ST-1 (seeds of 600–1,000 hits: 97.1–98.7%).
- **98.1%** on the ST-2 with U, V and W in play, 97.9% with them held at
  zero (`make rnd-rate`, 3,000 hits against the compiled-in reference
  features; every engine 96.7% or better). The new macros are drawn per
  engine from the part of their range that keeps a stab a stab, and are
  off about half the time each.
- The median hit is 0.66 from its nearest reference; the references
  themselves are a median 0.60 from their nearest neighbours.
- The misses are mostly long, bright, organ-like holds and a few very
  bright, very short plucks.
- `make stabs` prints the comparison, per engine and per feature, and
  `stabs sensitivity DIR N` shows which draws the misses come from (that
  is how the rules above were found). A unit test holds RND to at least
  95% against the reference features, which are compiled into the DSP crate
  (`analysis::REFERENCES`), so it runs without the WAVs.

## Layout

```
crates/dsp        engine, the 8 engines, wavetables, transient, chaos modulator, ladder,
                  saturator, EQ, chorus, echo, plate reverb, parameters, presets, the hit
                  preview, RND/MUTATE/locks, and the analysis (features, spectrogram
                  distance, the reference table); no dependencies
  src/engines/      va, sync, unison (SUPER), fm, fbfm, ring, pd, wtfm; tables (the wavetable bank)
  src/voice.rs      one channel, including what it takes from the other (`XIn`)
  src/engine.rs     both channels, wired into each other every sample, and the chain triggers
  examples/         render (presets → WAV), scenes (cross-mod scenes → WAV), rnd_rate,
                    probe_preset (a preset's features vs its nearest reference),
                    bench, calibrate (loudness survey),
                    analyze (RMS/pitch/brightness over time), probe (which knob made it quiet?),
                    stabs (survey the references; measure RND against them; render RND hits),
                    fit (fit a patch to each reference)
crates/worklet    C ABI over the engine, compiled to a standalone wasm module with no imports
crates/app        Dioxus UI with `web` and `desktop` features; panel state and persistence;
                  the CROSS-MOD panel and scene picker
tools/            Python (numpy/scipy/matplotlib) helpers: spectrogram sheets of a folder,
                  of fits, and a table of fitted parameters
```

The UI never touches the audio engine directly. It sends small commands (set
a parameter, trigger, master volume): over the worklet's `MessagePort` on the
web, and over an `mpsc` channel on the desktop. Both backends render in
blocks of 128 frames. Locks live only in the UI; the engine never sees them.

## Inspiration and sources

New in ST-2:

- Xfer **Serum** and Matt Tytel's **Vital**: oscillator warp modes (phase
  bend, formant/windowed sync, FM and ring modulation between oscillators)
- The **Buchla 259** and Make Noise **DPO**: cross-coupled FM between two
  oscillators, sync plus FM
- Yamaha **TX81Z** / OPL waveforms (the half-sine carrier), the DX's
  detuned and fixed-frequency operators
- Casio **CZ** 1+2′ line mode and the CZ-1000's noise modulation
- Roland **Juno** sub-oscillator; the JP-8000 / Serum "hypersaw"
- Hardware ring modulators with their own carrier oscillator
- Erica Synths' and other Eurorack approaches to circuit-bending: audio-rate
  sample & hold, envelope followers, trigger chains between voices
- Sidechain ducking, the pump of every techno record

From ST-1:

- Basic Channel / Maurizio and the dub techno chord: minor triads and
  sevenths through a lowpass, a tape echo and a spring or plate
- Drumcode-style FM stabs: a sine modulator two octaves up, velocity on the
  FM amount and the cutoff
- Juno-style pulse-width modulation; Roland and Oberheim-style unison
- Yamaha DX operator feedback; Casio **CZ** phase distortion
- V. Välimäki et al., polyBLEP oscillators; R. Bristow-Johnson, *Audio EQ
  Cookbook*; V. Zavalishin, *The Art of VA Filter Design* (ZDF ladder)
- J. Dattorro, "Effect Design, Part 1: Reverberator and Other Filters", JAES 45(9), 1997 (plate reverb)
- E. N. Lorenz, "Deterministic Nonperiodic Flow", J. Atmos. Sci. 20, 1963 (the chaos modulator)
