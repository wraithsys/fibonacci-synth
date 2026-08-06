//! "The Room": a stereo comb-filter constellation built from the tone's own
//! mathematics.
//!
//! Instead of reverberating the mono mix, the Room listens to the operators
//! *individually*. Every operator — carrier or not — feeds its own pair of
//! comb filters (one per ear):
//!
//! - **Left delays come from the operator's frequency ratio**:
//!   `BASE / √ratio`. The room inherits the patch's consonance model — in
//!   Golden mode the ten combs are φ-spaced (maximally non-coincident modes,
//!   the smoothest possible tail, for the same continued-fraction reason the
//!   ratios are maximally inharmonic), while in Harmonic mode the delays are
//!   rationally related and the room turns resonant and tuned. Same knob-less
//!   principle as everything else: the structure *is* the sound design.
//! - **Right delays are the golden midpoints of the left**: `left × √φ`, so
//!   each right-ear comb sits exactly halfway (geometrically) up the left
//!   ear's ladder. The two tails share no delay and decorrelate — that is
//!   the stereo width, no panning involved.
//! - **Ghost sends**: modulators are inaudible in the dry mix (they only
//!   bend phase), but the Room hears them, attenuated by the golden decay
//!   `1/φ^(depth−1)` times the `ghost` amount. The hidden skeleton of the
//!   tree is audible only in the reverb.
//! - **Diffusion**: two series allpasses per ear with golden-related delays;
//!   the allpass coefficient is 1/φ = φ − 1 ≈ 0.618.
//! - **Decay**: per-comb feedback follows the standard RT60 model
//!   `g = 0.001^(delay / rt60)`. The in-loop lowpass keeps only 1/φ² of the
//!   damp knob — the knob's voice is the MS-20 bus filter (see the constants
//!   below) — so the measured T60 holds still while damp turns. Honest range
//!   0.05–8 s: `examples/rt60_probe.rs` measured real decay saturating near
//!   7 s (feedback cap + tap-wobble interpolation loss), so the old 20 s top
//!   commanded nothing.
//! - **The Ghost Line** (`haunt` > 0): each operator also feeds a delay line
//!   at the golden interval (φ × its left comb delay) whose recirculation
//!   path runs through a first-order allpass solved so the design pitch
//!   (110 Hz) rotates exactly π/5 per pass — fully phase-*inverted* against
//!   the still-sounding drone after 5 passes, back in phase after 10. The
//!   line's output is injected into a *different* operator's combs (rotation
//!   by 3, coprime with 5). Because the drone never stops, the drifting
//!   phase audibly breathes: slow cancellation and reinforcement that no
//!   envelope is producing.
//!
//! `process` is allocation-free; buffers are allocated in `new` and delay
//! *taps* (not buffers) are retuned by `configure` at patch-change time. A
//! retune deliberately keeps the old buffer contents: the previous
//! structure's tail haunts the new one for a moment.

use crate::algorithm::{Compiled, NUM_OPS};
use crate::patch::{Patch, PHI};
use crate::voice::Frame;

/// Base comb delay (seconds) for an operator at ratio 1.0.
const BASE_DELAY_SECONDS: f32 = 0.029;
/// Comb buffer headroom; delays are clamped so `left × √φ` always fits.
const MAX_DELAY_SECONDS: f32 = 0.08;
/// Allpass diffusion delays (seconds): a golden pair.
const ALLPASS_SECONDS: [f32; 2] = [0.0051, 0.0051 / PHI];
/// Allpass coefficient: 1/φ = φ − 1.
const ALLPASS_G: f32 = 1.0 / PHI;
/// Gain staging for a drone-fed reverb (Freeverb-style): fixed attenuation
/// into the comb bank, fixed makeup after diffusion. rt60 changes decay
/// TIME only — it must never scale loudness downward (an earlier fix
/// normalized comb output by √(1−g), which made long rt60 *quieter*:
/// wrong feel, found by ear 2026-08-03).
const COMB_INPUT_GAIN: f32 = 0.06;
/// Tuned empirically with examples/verb_probe.rs: full-wet RMS should sit
/// near dry RMS for sustained tonal input (drones live off-resonance, so
/// the comb bank's average gain is far below its resonant peak — the
/// arithmetic guesses of 2.5 and 5.0 left the Room 14 dB underwater).
const WET_MAKEUP_GAIN: f32 = 25.0;
/// Feedback ceiling: bounds worst-case standing-wave buildup so sustained
/// resonance growls into the tanh ceiling instead of pinning it.
const MAX_FB: f32 = 0.985;
/// Per-sample one-pole coefficient for parameter smoothing (~15 ms at
/// 48 kHz). Every user parameter glides: instant jumps on charged delay
/// lines are audible clicks — the "mix is clicky" bug was zipper noise.
const PARAM_SMOOTH: f32 = 0.0014;
/// How far the field opens the damping as the instrument pushes.
const FIELD_TO_DAMP: f32 = 0.5;
/// How far the field pulls the wet back as the instrument pushes — the room
/// steps aside on a surge and returns as it settles.
const FIELD_TO_MIX: f32 = 0.1;
/// Tap modulation: a static source cannot freeze a moving room. Without
/// internal motion, a drone through a comb bank converges to fixed
/// coloration — at long rt60 the modes narrow, lock onto the stationary
/// partials, and the *reverberation percept* dies while RMS stays flat
/// (found by ear: "rt60 kills the reverb"; the probe measured constant
/// level and missed the motionlessness). Cure, as in every studio reverb:
/// each comb tap breathes a few samples at an infra-rate LFO —
/// golden-staggered rates, fractional reads, ±ppm pitch wobble.
const MOD_DEPTH_SAMPLES: f32 = 9.0;
const MOD_BASE_HZ: f32 = 0.31;
/// DC blocker pole. A comb amplifies DC by 1/(1−g) — up to 67× at the
/// feedback cap, ~100× through the full wet chain — and hot operator
/// feedback (or the Rip) makes the voice's waveform asymmetric, i.e. DC.
/// Unblocked, that DC rails the tanh to a constant: RMS 1.0, speakers
/// silent, one side dying before the other (found by probe, 2026-08-03 —
/// "no audio at 100% mix"). The blocker is a zero at 0 Hz with a ~3.5 Hz
/// corner: even a 27.5 Hz drone passes untouched.
const DC_BLOCK_R: f32 = 0.9995;

struct DcBlock {
    x1: f32,
    y1: f32,
}

impl DcBlock {
    fn new() -> Self {
        DcBlock { x1: 0.0, y1: 0.0 }
    }

    fn process(&mut self, x: f32) -> f32 {
        let y = x - self.x1 + DC_BLOCK_R * self.y1;
        self.x1 = x;
        self.y1 = y;
        y
    }
}

// ── The MS-20 move (Billy's sweep, 2026-08-05) ─────────────────────────────
// The damp knob's voice moved from the comb loops to the wet bus: a resonant
// 4-pole lowpass (a resonant 2-pole SVF into a plain 2-pole, 24 dB/oct with
// one peak) feeding straight into the tanh — filter into saturator, which is
// the MS-20's own topology and why it sings rather than merely darkens. The
// filter lives OUTSIDE the comb loops on purpose: a resonant peak inside
// recirculation multiplies loop gain above the feedback cap and turns the
// room into an oscillator; on the bus the scream stays bounded by the tanh.
/// Cutoff at damp 0 — effectively open.
const FC_MAX_HZ: f32 = 18_000.0;
/// The lowest the cutoff may reach, however far the field pushes the knob.
///
/// **The control is deliberately dishonest at its edges** (Billy, 2026-08-06:
/// "make the control dishonest — the cutoff can't go below idk 20 hz whilst
/// being effected but damp at 0 doesn't have to be anywhere near 20hz").
///
/// The field subtracts from `damp`, and the knob's own number is not what the
/// filter obeys — the cutoff obeys this range. Unbounded it is a crash, not a
/// curiosity: the mapping is exponential, `18 kHz · φ^(−10·damp)`, so a `damp`
/// of −0.5 asks for **200 kHz**, and the field reaches that easily. The SVF's
/// coefficient is `g = tan(π·fc/fs)`, and as the cutoff sweeps up through
/// Nyquist that argument sweeps through π/2, where the tangent **diverges to
/// infinity**. One infinite coefficient and the whole room is NaN: no audio
/// and a blank scope, because NaN is not quiet, it is absent. Billy found it
/// by reducing damp with the field open, which walks the cutoff straight
/// across the singularity.
const FC_MIN_HZ: f32 = 20.0;
/// Ceiling on the cutoff as a fraction of the sample rate, so the filter stays
/// stable on a device running below 40 kHz where 18 kHz alone would already be
/// past Nyquist.
const FC_MAX_NYQUIST_FRACTION: f32 = 0.45;
/// The cutoff descends this many golden steps across the knob:
/// `fc = 18 kHz / φ^(10·damp)` ≈ 146 Hz at damp 1.
const FC_GOLDEN_STEPS: f32 = 10.0;
/// Below the knee the filter is plain (Butterworth, no peak).
const Q_BASE: f32 = 0.707_106_8;
/// Resonance wakes at damp = 1/φ and rises on a convex φ curve to φ⁴ ≈ 6.85
/// at damp 1 — earned at the extreme, silent below the knee.
const Q_KNEE: f32 = 1.0 / PHI;
/// The comb loops keep 1/φ² of the knob for per-pass rolloff — enough that
/// a damped room still decays dark, small enough that the measured T60
/// stays close to the commanded rt60 across the knob (rt60_probe found
/// full in-loop damp shaving ~20% off the real decay).
const LOOP_DAMP_SCALE: f32 = 1.0 / (PHI * PHI);

/// A trapezoidal (Zavalishin/TPT) state-variable lowpass: stable under
/// per-sample cutoff sweeps, which the glided damp performs constantly.
#[derive(Default)]
struct Svf {
    ic1: f32,
    ic2: f32,
}

impl Svf {
    /// One lowpass sample. `g = tan(π·fc/fs)`, `k = 1/Q`.
    fn process(&mut self, x: f32, g: f32, k: f32) -> f32 {
        let a1 = 1.0 / (1.0 + g * (g + k));
        let a2 = g * a1;
        let a3 = g * a2;
        let v3 = x - self.ic2;
        let v1 = a1 * self.ic1 + a2 * v3;
        let v2 = self.ic2 + a2 * self.ic1 + a3 * v3;
        self.ic1 = 2.0 * v1 - self.ic1;
        self.ic2 = 2.0 * v2 - self.ic2;
        v2
    }
}

/// User-facing reverb parameters.
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct VerbParams {
    /// Wet/dry, 0..=1. 0 = fully dry.
    pub mix: f32,
    /// How loudly the Room hears the modulators, 0..=1 (see module docs).
    pub ghost: f32,
    /// Decay time to −60 dB, seconds.
    pub rt60: f32,
    /// High-frequency damping in the comb loops, 0..=1.
    pub damp: f32,
    /// The Ghost Line, 0..=1 (0 = off, leaves the Room untouched): each
    /// operator's signal recirculates through a delay line whose phase
    /// rotates π/5 further on every pass (full inversion after 5 passes),
    /// and is injected into a *different* operator's comb — the cross-feed
    /// permutation is rotation by 3, coprime with 5, so the ghost visits
    /// every room before returning home. See module docs.
    pub haunt: f32,
}

impl Default for VerbParams {
    fn default() -> Self {
        VerbParams {
            mix: 0.35,
            ghost: 0.4,
            rt60: 4.0,
            damp: 0.4,
            haunt: 0.0,
        }
    }
}

struct Comb {
    buf: Vec<f32>,
    write: usize,
    delay: usize,
    delay_seconds: f32,
    /// Smoothed feedback (glides toward `fb_target` per sample).
    fb: f32,
    fb_target: f32,
    lp: f32,
    lfo_phase: f32,
    lfo_inc: f32,
}

impl Comb {
    fn new(len: usize, lfo_inc: f32, lfo_phase: f32) -> Self {
        Comb {
            buf: vec![0.0; len],
            write: 0,
            delay: 1,
            delay_seconds: 0.0,
            fb: 0.0,
            fb_target: 0.0,
            lp: 0.0,
            lfo_phase,
            lfo_inc,
        }
    }

    fn set_delay(&mut self, seconds: f32, sample_rate: f32) {
        self.delay_seconds = seconds;
        self.delay = ((seconds * sample_rate) as usize).clamp(1, self.buf.len() - 1);
    }

    fn set_rt60(&mut self, rt60: f32) {
        self.fb_target = 0.001f32.powf(self.delay_seconds / rt60.max(0.05)).min(MAX_FB);
    }

    fn process(&mut self, x: f32, damp: f32) -> f32 {
        self.fb += PARAM_SMOOTH * (self.fb_target - self.fb);
        self.lfo_phase += self.lfo_inc;
        if self.lfo_phase >= 1.0 {
            self.lfo_phase -= 1.0;
        }
        let wobble = (core::f32::consts::TAU * self.lfo_phase).sin() * MOD_DEPTH_SAMPLES;
        let len = self.buf.len();
        let d = (self.delay as f32 + wobble).clamp(1.0, (len - 2) as f32);
        let di = d as usize;
        let frac = d - di as f32;
        let i0 = (self.write + len - di) % len;
        let i1 = (self.write + len - di - 1) % len;
        let y = self.buf[i0] * (1.0 - frac) + self.buf[i1] * frac;
        self.lp = y + damp * (self.lp - y);
        self.buf[self.write] = x + self.lp * self.fb;
        self.write = (self.write + 1) % self.buf.len();
        y
    }
}

use crate::phase::{allpass_coeff_for, PhaseRotator, DESIGN_HZ, PHASE_PER_PASS};

/// Cross-feed permutation step: op i haunts op (i + 3) mod 5. 3 is a
/// Fibonacci number coprime with 5, so the cycle visits every operator.
const GHOST_STEP: usize = 3;
/// Ghost delay relative to the source op's left comb: the golden interval.
const GHOST_DELAY_FACTOR: f32 = PHI;
/// Loop-gain ceiling for the recirculation at haunt = 1.
const GHOST_MAX_FB: f32 = 0.92;

/// One operator's Ghost Line: a delay whose recirculation path runs through
/// a phase-rotating allpass, so every pass drifts π/5 further out of phase
/// with the (static, still-sounding) drone that fed it.
struct GhostLine {
    buf: Vec<f32>,
    write: usize,
    delay: usize,
    rot: PhaseRotator,
    fb: f32,
}

impl GhostLine {
    fn new(len: usize, ap_a: f32) -> Self {
        GhostLine {
            buf: vec![0.0; len],
            write: 0,
            delay: 1,
            rot: PhaseRotator::new(ap_a),
            fb: 0.0,
        }
    }

    fn set_delay(&mut self, seconds: f32, sample_rate: f32) {
        self.delay = ((seconds * sample_rate) as usize).clamp(1, self.buf.len() - 1);
    }

    fn process(&mut self, x: f32) -> f32 {
        let read = (self.write + self.buf.len() - self.delay) % self.buf.len();
        let y = self.rot.process(self.buf[read]);
        self.buf[self.write] = x + y * self.fb;
        self.write = (self.write + 1) % self.buf.len();
        y
    }
}

struct Allpass {
    buf: Vec<f32>,
    write: usize,
    delay: usize,
}

impl Allpass {
    fn new(seconds: f32, sample_rate: f32) -> Self {
        let delay = ((seconds * sample_rate) as usize).max(1);
        Allpass {
            buf: vec![0.0; delay + 1],
            write: 0,
            delay,
        }
    }

    fn process(&mut self, x: f32) -> f32 {
        let read = (self.write + self.buf.len() - self.delay) % self.buf.len();
        let d = self.buf[read];
        let y = d - ALLPASS_G * x;
        self.buf[self.write] = x + ALLPASS_G * d;
        self.write = (self.write + 1) % self.buf.len();
        y
    }
}

pub struct StereoVerb {
    sample_rate: f32,
    /// Target parameters; the smoothed actuals below glide toward them.
    params: VerbParams,
    mix_s: f32,
    ghost_s: f32,
    haunt_s: f32,
    damp_s: f32,
    /// Structural send per op: carriers 1.0, modulators `1/φ^(depth−1)`
    /// (further scaled by `params.ghost` at runtime).
    send: [f32; NUM_OPS],
    is_carrier: [bool; NUM_OPS],
    combs_l: Vec<Comb>,
    combs_r: Vec<Comb>,
    ap_l: Vec<Allpass>,
    ap_r: Vec<Allpass>,
    ghosts: Vec<GhostLine>,
    dc: Vec<DcBlock>,
    /// Post-saturation blockers: tanh *rectifies* — squashing an
    /// asymmetric (hot-feedback) waveform manufactures fresh DC downstream
    /// of the input blockers.
    wet_dc_l: DcBlock,
    wet_dc_r: DcBlock,
    /// The MS-20 bus filter, two 2-pole stages per ear (see module consts).
    svf_l: [Svf; 2],
    svf_r: [Svf; 2],
}

impl StereoVerb {
    pub fn new(sample_rate: f32) -> Self {
        let len = (MAX_DELAY_SECONDS * sample_rate) as usize + 2;
        let ghost_len = (MAX_DELAY_SECONDS * GHOST_DELAY_FACTOR * sample_rate) as usize + 2;
        let ghost_a = allpass_coeff_for(DESIGN_HZ, sample_rate, PHASE_PER_PASS);
        let defaults = VerbParams::default();
        StereoVerb {
            sample_rate,
            params: defaults,
            mix_s: defaults.mix,
            ghost_s: defaults.ghost,
            haunt_s: defaults.haunt,
            damp_s: defaults.damp,
            send: [1.0; NUM_OPS],
            is_carrier: [true; NUM_OPS],
            combs_l: (0..NUM_OPS)
                .map(|i| {
                    let rate = MOD_BASE_HZ * PHI.powf(-0.5 * i as f32);
                    Comb::new(len, rate / sample_rate, 0.19 * i as f32)
                })
                .collect(),
            combs_r: (0..NUM_OPS)
                .map(|i| {
                    let rate = MOD_BASE_HZ * PHI.powf(-0.5 * i as f32);
                    Comb::new(len, rate / sample_rate, 0.19 * i as f32 + 0.37)
                })
                .collect(),
            ap_l: ALLPASS_SECONDS.iter().map(|&s| Allpass::new(s, sample_rate)).collect(),
            ap_r: ALLPASS_SECONDS.iter().map(|&s| Allpass::new(s, sample_rate)).collect(),
            ghosts: (0..NUM_OPS).map(|_| GhostLine::new(ghost_len, ghost_a)).collect(),
            dc: (0..NUM_OPS).map(|_| DcBlock::new()).collect(),
            wet_dc_l: DcBlock::new(),
            wet_dc_r: DcBlock::new(),
            svf_l: [Svf::default(), Svf::default()],
            svf_r: [Svf::default(), Svf::default()],
        }
    }

    /// Rebuild the room from the patch's mathematics. Call after every
    /// patch/algorithm/mode change. Buffers are kept (the old tail haunts
    /// the new structure); only taps, sends, and feedback gains move.
    pub fn configure(&mut self, patch: &Patch, compiled: &Compiled) {
        for i in 0..NUM_OPS {
            let ratio = patch.ops[i].ratio.max(1e-3);
            let left = (BASE_DELAY_SECONDS / ratio.sqrt()).min(MAX_DELAY_SECONDS / PHI.sqrt());
            self.combs_l[i].set_delay(left, self.sample_rate);
            self.combs_r[i].set_delay(left * PHI.sqrt(), self.sample_rate);
            self.combs_l[i].set_rt60(self.params.rt60);
            self.combs_r[i].set_rt60(self.params.rt60);
            self.ghosts[i].set_delay(left * GHOST_DELAY_FACTOR, self.sample_rate);
            let carrier = compiled.carriers >> i & 1 == 1;
            self.is_carrier[i] = carrier;
            self.send[i] = if carrier {
                1.0
            } else {
                PHI.powi(-(compiled.depth[i] as i32 - 1))
            };
        }
    }

    pub fn set_params(&mut self, params: VerbParams) {
        self.params = params;
        for comb in self.combs_l.iter_mut().chain(self.combs_r.iter_mut()) {
            comb.set_rt60(params.rt60);
        }
    }

    pub fn params(&self) -> VerbParams {
        self.params
    }

    /// One sample in, one stereo sample out. Allocation-free.
    pub fn process(&mut self, frame: &Frame) -> (f32, f32) {
        // Glide every user parameter toward its target: instant jumps on a
        // loud sustained signal are clicks.
        self.mix_s += PARAM_SMOOTH * (self.params.mix - self.mix_s);
        self.ghost_s += PARAM_SMOOTH * (self.params.ghost - self.ghost_s);
        self.haunt_s += PARAM_SMOOTH * (self.params.haunt - self.haunt_s);
        self.damp_s += PARAM_SMOOTH * (self.params.damp - self.damp_s);

        // **The room answers the field** (Billy, 2026-08-06). As the
        // instrument pushes, the damping opens and the wet pulls back — so the
        // room brightens and steps aside on a surge, then closes and returns
        // as it settles. It is the same movement that lifts INDEX and the
        // amplitude, arriving here as one number rather than three modulators
        // that happen to agree.
        //
        // Not smoothed: the field is already a continuous, per-sample signal
        // glided at its own rate, so there is no step to chase. Not clamped at
        // the bottom either, on Billy's instruction — a negative damp is a
        // filter further open than the knob goes, and a negative mix is dryer
        // than dry, and both are just the movement continuing past the control
        // rather than a state anything has to special-case.
        let damp = self.damp_s - frame.field * FIELD_TO_DAMP;
        // The field ducks the wet *level*, and deliberately not the dry/wet
        // balance. Subtracting from `mix` was the obvious reading of "decrease
        // mix" and it has a consequence nobody wanted: at mix 1.0 it pulls the
        // balance below full wet, so **dry reappears** — Billy heard it
        // immediately, "with mix turned all the way up playing with damp i can
        // still here some of the unaffected signal", and it measured at 5% of
        // the dry signal at field 0.5.
        //
        // Ducking the wet gets the gesture — the room steps aside on a surge
        // and returns as it settles — while a control at its endpoint keeps
        // meaning what it says. Same lesson as the floor: a coupling may push
        // a control, but it may not push it past its own edge.
        let wet_duck = (1.0 - frame.field * FIELD_TO_MIX).max(0.0);

        // Op sends: carriers full, modulators through the ghost bleed, and
        // everything through the master gain — the master governs how loudly
        // the instrument speaks *into* the room (already glided in the voice,
        // so no zipper arrives here). At master 0 the room falls silent
        // rather than reverberating a voice nobody dry-hears.
        let mut sends = [0.0f32; NUM_OPS];
        for i in 0..NUM_OPS {
            let bleed = if self.is_carrier[i] { 1.0 } else { self.ghost_s };
            sends[i] = frame.ops[i] * frame.master * self.send[i] * bleed;
        }
        // Ghost Lines first: op i's recirculating, phase-rotating echo…
        let mut haunted = [0.0f32; NUM_OPS];
        if self.haunt_s > 1e-4 {
            for i in 0..NUM_OPS {
                self.ghosts[i].fb = GHOST_MAX_FB * self.haunt_s;
                haunted[i] = self.ghosts[i].process(sends[i]);
            }
        }
        let mut wet_l = 0.0;
        let mut wet_r = 0.0;
        // In-loop damping keeps only 1/φ² of the knob: the knob's voice is
        // the bus filter below, and a full-strength loop lowpass was quietly
        // shortening the real decay (rt60_probe, 2026-08-05).
        // Never negative — a precaution rather than a diagnosed fault. In-loop
        // damping is a one-pole whose HF gain is (1−d)/(1+d); below zero that
        // is a *boost*, ~1.5× at the depths the field reaches, sitting inside a
        // comb whose feedback runs to 0.985. That product exceeds unity, which
        // is the shape of a runaway even though the crash Billy hit was the
        // cutoff singularity alone (verified by reverting each fix separately:
        // only the unclamped cutoff reproduced it). The field opening the
        // damping should brighten the room, never arm it.
        let loop_damp = damp.max(0.0) * LOOP_DAMP_SCALE;
        for i in 0..NUM_OPS {
            // …arrives in op (i + GHOST_STEP) mod 5's combs, not its own.
            let from = (i + NUM_OPS - GHOST_STEP) % NUM_OPS;
            let x =
                self.dc[i].process(sends[i] + self.haunt_s * haunted[from]) * COMB_INPUT_GAIN;
            wet_l += self.combs_l[i].process(x, loop_damp);
            wet_r += self.combs_r[i].process(x, loop_damp);
        }
        let scale = WET_MAKEUP_GAIN / NUM_OPS as f32;
        wet_l *= scale;
        wet_r *= scale;
        for ap in self.ap_l.iter_mut() {
            wet_l = ap.process(wet_l);
        }
        for ap in self.ap_r.iter_mut() {
            wet_r = ap.process(wet_r);
        }
        // The MS-20 move: the damp knob is a resonant 4-pole lowpass on the
        // wet bus, feeding straight into the tanh — filter into saturator,
        // the MS-20's own topology. Cutoff descends ten golden steps across
        // the knob; resonance wakes at the 1/φ knee and rises on a convex φ
        // curve to φ⁴. Stage two is plain (k = 2), so the slope is 24 dB/oct
        // with a single peak.
        // Bounded, not because the coupling should stop but because the filter
        // cannot be more open than open. See FC_MIN_HZ.
        let fc = (FC_MAX_HZ * PHI.powf(-FC_GOLDEN_STEPS * damp))
            .clamp(FC_MIN_HZ, FC_MAX_HZ.min(FC_MAX_NYQUIST_FRACTION * self.sample_rate));
        let g = (core::f32::consts::PI * fc / self.sample_rate).tan();
        let knee = ((damp - Q_KNEE) / (1.0 - Q_KNEE)).clamp(0.0, 1.0);
        let q = Q_BASE + (PHI.powi(4) - Q_BASE) * knee.powf(PHI);
        let k1 = 1.0 / q;
        wet_l = self.svf_l[0].process(wet_l, g, k1);
        wet_l = self.svf_l[1].process(wet_l, g, 2.0);
        wet_r = self.svf_r[0].process(wet_r, g, k1);
        wet_r = self.svf_r[1].process(wet_r, g, 2.0);
        // The wet bus saturates smoothly instead of clipping hard: a
        // haunted room may growl, but it may not click. The dry path stays
        // untouched — no compression or saturation ever touches the
        // instrument's direct voice.
        wet_l = self.wet_dc_l.process(wet_l.tanh());
        wet_r = self.wet_dc_r.process(wet_r.tanh());
        let dry = frame.mix * (1.0 - self.mix_s);
        (
            dry + wet_l * self.mix_s * wet_duck,
            dry + wet_r * self.mix_s * wet_duck,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::algorithm::{compile, ALGORITHMS};
    use crate::patch::RatioMode;

    const SR: f32 = 48_000.0;

    fn impulse_frame(v: f32) -> Frame {
        Frame {
            ops: [v; NUM_OPS],
            mix: v,
            master: 1.0,
            // Probes measure the room itself: no field movement.
            field: 0.0,
        }
    }

    fn configured(mode: RatioMode) -> StereoVerb {
        let patch = Patch::init(ALGORITHMS[0], mode);
        let mut verb = StereoVerb::new(SR);
        verb.configure(&patch, &compile(patch.algorithm));
        verb
    }

    #[test]
    fn silence_in_silence_out() {
        let mut verb = configured(RatioMode::Golden);
        for _ in 0..48_000 {
            let (l, r) = verb.process(&impulse_frame(0.0));
            assert_eq!((l, r), (0.0, 0.0));
        }
    }

    #[test]
    fn impulse_tail_decays_and_stays_finite() {
        for mode in RatioMode::ALL {
            let mut verb = configured(mode);
            verb.set_params(VerbParams {
                rt60: 0.5,
                ..VerbParams::default()
            });
            let (mut early, mut late) = (0.0f64, 0.0f64);
            for n in 0..96_000 {
                let x = if n == 0 { 1.0 } else { 0.0 };
                let (l, r) = verb.process(&impulse_frame(x));
                assert!(l.is_finite() && r.is_finite(), "{mode:?} at sample {n}");
                let e = (l * l + r * r) as f64;
                if n < 24_000 {
                    early += e;
                } else if n >= 72_000 {
                    late += e;
                }
            }
            assert!(early > 0.0, "{mode:?}: no tail at all");
            assert!(
                late < early / 100.0,
                "{mode:?}: tail not decaying (early {early}, late {late})"
            );
        }
    }

    #[test]
    fn the_two_ears_decorrelate() {
        let mut verb = configured(RatioMode::Golden);
        let mut differ = 0usize;
        for n in 0..48_000 {
            let x = if n == 0 { 1.0 } else { 0.0 };
            let (l, r) = verb.process(&impulse_frame(x));
            if (l - r).abs() > 1e-9 {
                differ += 1;
            }
        }
        assert!(differ > 10_000, "ears too similar: {differ} differing samples");
    }

    #[test]
    fn ghost_allpass_rotates_the_design_pitch_by_pi_over_five() {
        use crate::phase::allpass_phase;
        let a = allpass_coeff_for(DESIGN_HZ, SR, PHASE_PER_PASS);
        let w = core::f32::consts::TAU * DESIGN_HZ / SR;
        let phase = allpass_phase(a, w);
        assert!(
            (phase + PHASE_PER_PASS).abs() < 1e-3,
            "solved a = {a}, phase = {phase} (want {})",
            -PHASE_PER_PASS
        );
    }

    #[test]
    fn haunt_zero_leaves_the_room_untouched_and_full_haunt_is_stable() {
        // haunt = 0 must be bit-identical to a Room without Ghost Lines: the
        // default params already have haunt 0, so compare against explicit 0.
        let mut plain = configured(RatioMode::Golden);
        let mut zeroed = configured(RatioMode::Golden);
        let mut p = zeroed.params();
        p.haunt = 0.0;
        zeroed.set_params(p);
        let mut differs = false;
        for n in 0..9600 {
            let x = if n % 480 == 0 { 0.5 } else { 0.0 };
            if plain.process(&impulse_frame(x)) != zeroed.process(&impulse_frame(x)) {
                differs = true;
            }
        }
        assert!(!differs, "haunt 0 changed the approved Room sound");

        // Full haunt with a periodic input stays finite and audibly differs.
        let mut haunted = configured(RatioMode::Golden);
        let mut hp = haunted.params();
        hp.haunt = 1.0;
        haunted.set_params(hp);
        let mut reference = configured(RatioMode::Golden);
        let mut diverged = false;
        for n in 0..192_000 {
            let x = (core::f32::consts::TAU * 110.0 * n as f32 / SR).sin() * 0.5;
            let (l, r) = haunted.process(&impulse_frame(x));
            assert!(l.is_finite() && r.is_finite(), "haunt blew up at {n}");
            assert!(l.abs() < 10.0 && r.abs() < 10.0, "haunt runaway at {n}");
            if (l, r) != reference.process(&impulse_frame(x)) {
                diverged = true;
            }
        }
        assert!(diverged, "haunt 1.0 made no difference at all");
    }

    /// rt60 changes decay time, not loudness: sustained input at long rt60
    /// must never be quieter than at short rt60 (the regression that
    /// √(1−g) output normalization introduced).
    #[test]
    fn longer_rt60_is_never_quieter() {
        let rms_at = |rt60: f32| {
            let mut verb = configured(RatioMode::Golden);
            verb.set_params(VerbParams {
                mix: 1.0,
                rt60,
                ..VerbParams::default()
            });
            let mut acc = 0.0f64;
            for n in 0..192_000 {
                let x = (core::f32::consts::TAU * 110.0 * n as f32 / SR).sin() * 0.3;
                let (l, r) = verb.process(&impulse_frame(x));
                if n >= 96_000 {
                    acc += (l * l + r * r) as f64;
                }
            }
            (acc / 96_000.0).sqrt()
        };
        let short = rms_at(1.0);
        let long = rms_at(8.0);
        assert!(
            long >= short * 0.8,
            "long rt60 must not be quieter (short {short}, long {long})"
        );
    }

    /// Measure the room's real T60 the way `examples/rt60_probe.rs` does:
    /// charge, cut, time the fall to −60 dB in 100 ms RMS windows.
    fn measured_t60(rt60: f32, damp: f32) -> f32 {
        let mut verb = configured(RatioMode::Golden);
        verb.set_params(VerbParams {
            mix: 1.0,
            rt60,
            damp,
            haunt: 0.0,
            ..VerbParams::default()
        });
        let mut level0 = 0.0f64;
        for n in 0..96_000 {
            let x = (core::f32::consts::TAU * 65.0 * n as f32 / SR).sin() * 0.5;
            let (l, r) = verb.process(&impulse_frame(x));
            level0 = level0 * (1.0 - 1e-4) + 1e-4 * (0.5 * (l * l + r * r)) as f64;
        }
        let target = level0 * 1e-6;
        for w in 0..100 {
            let mut acc = 0.0f64;
            for _ in 0..4800 {
                let (l, r) = verb.process(&impulse_frame(0.0));
                acc += (0.5 * (l * l + r * r)) as f64;
            }
            if acc / 4800.0 <= target {
                return (w + 1) as f32 * 0.1;
            }
        }
        f32::INFINITY
    }

    /// The decay the knob commands is the decay the room performs, and damp
    /// no longer shortens it: the in-loop lowpass keeps only 1/φ² of the
    /// knob, the rest of damp's voice living on the bus filter *outside* the
    /// loops (rt60_probe found full in-loop damp shaving ~20% off the tail).
    #[test]
    fn commanded_decay_is_real_and_damp_invariant() {
        let plain = measured_t60(2.0, 0.0);
        let damped = measured_t60(2.0, 0.9);
        assert!(
            (1.5..=2.8).contains(&plain),
            "commanded 2 s measured {plain} s"
        );
        assert!(
            (damped - plain).abs() <= 0.3,
            "damp moved the decay: {plain} s plain vs {damped} s damped"
        );
    }

    /// The MS-20 bus filter at full damp — cutoff ten golden steps down,
    /// resonance at φ⁴ — must growl, never blow up: the filter sits outside
    /// the comb loops precisely so its peak cannot multiply loop gain.
    #[test]
    fn resonant_damp_screams_but_never_runs_away() {
        let mut verb = configured(RatioMode::Golden);
        verb.set_params(VerbParams {
            mix: 1.0,
            rt60: 8.0,
            damp: 0.99,
            haunt: 1.0,
            ..VerbParams::default()
        });
        for n in 0..480_000 {
            let x = (core::f32::consts::TAU * 65.0 * n as f32 / SR).sin() * 0.8;
            let (l, r) = verb.process(&impulse_frame(x));
            assert!(l.is_finite() && r.is_finite(), "blew up at {n}");
            assert!(l.abs() < 10.0 && r.abs() < 10.0, "runaway at {n}");
        }
    }

    #[test]
    fn every_mode_and_algorithm_configures_within_buffers() {
        // configure() clamps taps into the allocated buffers; this asserts
        // the clamp math holds across the whole shipped space (GoldenMirror's
        // sub-unity ratios are the stress case).
        for mode in RatioMode::ALL {
            for &alg in &ALGORITHMS {
                let patch = Patch::init(alg, mode);
                let mut verb = StereoVerb::new(SR);
                verb.configure(&patch, &compile(alg));
                for c in verb.combs_l.iter().chain(verb.combs_r.iter()) {
                    assert!(c.delay >= 1 && c.delay < c.buf.len());
                }
                for g in verb.ghosts.iter() {
                    assert!(g.delay >= 1 && g.delay < g.buf.len());
                }
            }
        }
    }
}
