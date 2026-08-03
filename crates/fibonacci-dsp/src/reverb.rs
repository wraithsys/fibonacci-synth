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
//!   `g = 0.001^(delay / rt60)`, with a one-pole lowpass in each loop for
//!   high-frequency damping.
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
    fb: f32,
    lp: f32,
}

impl Comb {
    fn new(len: usize) -> Self {
        Comb {
            buf: vec![0.0; len],
            write: 0,
            delay: 1,
            delay_seconds: 0.0,
            fb: 0.0,
            lp: 0.0,
        }
    }

    fn set_delay(&mut self, seconds: f32, sample_rate: f32) {
        self.delay_seconds = seconds;
        self.delay = ((seconds * sample_rate) as usize).clamp(1, self.buf.len() - 1);
    }

    fn set_rt60(&mut self, rt60: f32) {
        self.fb = 0.001f32.powf(self.delay_seconds / rt60.max(0.05));
    }

    fn process(&mut self, x: f32, damp: f32) -> f32 {
        let read = (self.write + self.buf.len() - self.delay) % self.buf.len();
        let y = self.buf[read];
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
    params: VerbParams,
    /// Structural send per op: carriers 1.0, modulators `1/φ^(depth−1)`
    /// (further scaled by `params.ghost` at runtime).
    send: [f32; NUM_OPS],
    is_carrier: [bool; NUM_OPS],
    combs_l: Vec<Comb>,
    combs_r: Vec<Comb>,
    ap_l: Vec<Allpass>,
    ap_r: Vec<Allpass>,
    ghosts: Vec<GhostLine>,
}

impl StereoVerb {
    pub fn new(sample_rate: f32) -> Self {
        let len = (MAX_DELAY_SECONDS * sample_rate) as usize + 2;
        let ghost_len = (MAX_DELAY_SECONDS * GHOST_DELAY_FACTOR * sample_rate) as usize + 2;
        let ghost_a = allpass_coeff_for(DESIGN_HZ, sample_rate, PHASE_PER_PASS);
        StereoVerb {
            sample_rate,
            params: VerbParams::default(),
            send: [1.0; NUM_OPS],
            is_carrier: [true; NUM_OPS],
            combs_l: (0..NUM_OPS).map(|_| Comb::new(len)).collect(),
            combs_r: (0..NUM_OPS).map(|_| Comb::new(len)).collect(),
            ap_l: ALLPASS_SECONDS.iter().map(|&s| Allpass::new(s, sample_rate)).collect(),
            ap_r: ALLPASS_SECONDS.iter().map(|&s| Allpass::new(s, sample_rate)).collect(),
            ghosts: (0..NUM_OPS).map(|_| GhostLine::new(ghost_len, ghost_a)).collect(),
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
            self.ghosts[i].fb = GHOST_MAX_FB * self.params.haunt;
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
        for ghost in self.ghosts.iter_mut() {
            ghost.fb = GHOST_MAX_FB * params.haunt;
        }
    }

    pub fn params(&self) -> VerbParams {
        self.params
    }

    /// One sample in, one stereo sample out. Allocation-free.
    pub fn process(&mut self, frame: &Frame) -> (f32, f32) {
        // Op sends: carriers full, modulators through the ghost bleed.
        let mut sends = [0.0f32; NUM_OPS];
        for i in 0..NUM_OPS {
            let bleed = if self.is_carrier[i] { 1.0 } else { self.params.ghost };
            sends[i] = frame.ops[i] * self.send[i] * bleed;
        }
        // Ghost Lines first: op i's recirculating, phase-rotating echo…
        let mut haunted = [0.0f32; NUM_OPS];
        if self.params.haunt > 0.0 {
            for i in 0..NUM_OPS {
                haunted[i] = self.ghosts[i].process(sends[i]);
            }
        }
        let mut wet_l = 0.0;
        let mut wet_r = 0.0;
        for i in 0..NUM_OPS {
            // …arrives in op (i + GHOST_STEP) mod 5's combs, not its own.
            let from = (i + NUM_OPS - GHOST_STEP) % NUM_OPS;
            let x = sends[i] + self.params.haunt * haunted[from];
            wet_l += self.combs_l[i].process(x, self.params.damp);
            wet_r += self.combs_r[i].process(x, self.params.damp);
        }
        let scale = 1.0 / NUM_OPS as f32;
        wet_l *= scale;
        wet_r *= scale;
        for ap in self.ap_l.iter_mut() {
            wet_l = ap.process(wet_l);
        }
        for ap in self.ap_r.iter_mut() {
            wet_r = ap.process(wet_r);
        }
        let dry = frame.mix * (1.0 - self.params.mix);
        (dry + wet_l * self.params.mix, dry + wet_r * self.params.mix)
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
