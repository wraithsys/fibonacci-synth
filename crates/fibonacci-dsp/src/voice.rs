//! The monophonic drone voice.
//!
//! No envelopes: once a frequency is set the voice sounds until told
//! otherwise. The expressive controls are the algorithm's structure toggles,
//! the operator power switches, and pitch glide.
//!
//! Determinism contract: the same sequence of calls on a fresh [`Voice`]
//! produces bit-identical output, regardless of how `render` is chunked.
//! Enabling an operator always resets its phase (and feedback history, if it
//! is the feedback operator) *before* it fades in, so a re-enabled operator
//! sounds exactly like it did the first time. Disabling fades out over
//! [`RAMP_SECONDS`], then zeroes that state — an off operator holds no ghost
//! state whatsoever. Re-enabling *during* the fade-out still resets phase
//! immediately (determinism wins over the small discontinuity that can cause).

use crate::algorithm::{compile, AlgorithmId, Compiled, NUM_OPS};
use crate::patch::{index_response, Patch, PHI};
use crate::phase::{allpass_coeff_for, PhaseRotator, DESIGN_HZ, PHASE_PER_PASS};

/// One sample of the voice, with the per-operator outputs exposed — this is
/// what the reverb listens to (including the modulators a mono mix never
/// reveals).
#[derive(Clone, Copy, Debug)]
pub struct Frame {
    /// Current-sample output of every operator (post level, gain, and INDEX).
    pub ops: [f32; NUM_OPS],
    /// The dry mono mix: mean of carriers × master level.
    pub mix: f32,
}

const TAU: f32 = core::f32::consts::TAU;
/// Phase-modulation index, in radians, contributed by a modulator running at
/// level 1.0. With the default `1/φ^(depth-1)` levels this lands in hot,
/// DX-and-then-some index territory — deliberately.
const MOD_SCALE: f32 = TAU;
/// Feedback index, in radians, at feedback amount 1.0. The feedback signal is
/// the average of the operator's previous two output samples — the standard
/// trick that tames feedback's tendency to flip into noise.
const FB_SCALE: f32 = core::f32::consts::PI;
/// De-click fade time for the operator power switches.
const RAMP_SECONDS: f32 = 0.002;
/// The Rip Line's delay: the golden interval above the Room's base comb.
const RIP_DELAY_SECONDS: f32 = 0.029 * PHI;
/// Phase-modulation injected into each carrier by the Rip at rip = 1.0, in
/// radians per unit of ripped signal.
const RIP_SCALE: f32 = core::f32::consts::PI;
/// Loop-gain ceiling for the Rip's recirculation at rip = 1.0.
const RIP_MAX_FB: f32 = 0.9;
/// One-pole lowpass coefficient in the Rip's recirculation: 1/φ² — the past
/// darkens as it decays. Without this, recirculated highs stack up and the
/// phase injection tips into papery feedback-PM noise at the center of the
/// worble cycle (found by ear, 2026-08-03).
const RIP_DAMP: f32 = 1.0 / (PHI * PHI);

/// The Rip Line (README §11): the Ghost Line's mechanism transplanted into
/// the voice itself. Fed by the carriers' mean output; its phase-rotated,
/// damped recirculation is injected into the carriers' *phase input*, so the
/// drone modulates itself with its own inverted past.
struct RipLine {
    buf: Vec<f32>,
    write: usize,
    delay: usize,
    rot: PhaseRotator,
    lp: f32,
    fb: f32,
}

impl RipLine {
    fn new(sample_rate: f32) -> Self {
        let delay = ((RIP_DELAY_SECONDS * sample_rate) as usize).max(1);
        RipLine {
            buf: vec![0.0; delay + 1],
            write: 0,
            delay,
            rot: PhaseRotator::new(allpass_coeff_for(DESIGN_HZ, sample_rate, PHASE_PER_PASS)),
            lp: 0.0,
            fb: 0.0,
        }
    }

    fn process(&mut self, x: f32) -> f32 {
        let read = (self.write + self.buf.len() - self.delay) % self.buf.len();
        let y = self.rot.process(self.buf[read]);
        self.lp = y + RIP_DAMP * (self.lp - y);
        self.buf[self.write] = x + self.lp * self.fb;
        self.write = (self.write + 1) % self.buf.len();
        self.lp
    }

    fn clear(&mut self) {
        self.buf.fill(0.0);
        self.write = 0;
        self.rot.clear();
        self.lp = 0.0;
    }
}

pub struct Voice {
    sample_rate: f32,
    patch: Patch,
    compiled: Compiled,
    /// Per-operator phase accumulators in cycles, wrapped to `[0, 1)`.
    phase: [f32; NUM_OPS],
    /// Current-sample operator outputs, read by shallower operators.
    out: [f32; NUM_OPS],
    /// Previous two output samples of the feedback operator.
    fb_hist: [f32; 2],
    /// Power-switch fade gain per operator.
    gain: [f32; NUM_OPS],
    /// Set when an operator is switched off; performs the state reset once
    /// the fade-out completes.
    pending_reset: [bool; NUM_OPS],
    /// Glide-smoothed base frequency in Hz.
    freq: f32,
    target_freq: f32,
    /// The Rip Line and its most recent output (used one sample later as
    /// carrier phase modulation).
    rip_line: RipLine,
    rip_sig: f32,
}

impl Voice {
    pub fn new(sample_rate: f32, patch: Patch) -> Self {
        let compiled = compile(patch.algorithm);
        let mut gain = [0.0; NUM_OPS];
        for (g, op) in gain.iter_mut().zip(patch.ops.iter()) {
            *g = if op.enabled { 1.0 } else { 0.0 };
        }
        Voice {
            sample_rate,
            patch,
            compiled,
            phase: [0.0; NUM_OPS],
            out: [0.0; NUM_OPS],
            fb_hist: [0.0; 2],
            gain,
            pending_reset: [false; NUM_OPS],
            freq: 110.0,
            target_freq: 110.0,
            rip_line: RipLine::new(sample_rate),
            rip_sig: 0.0,
        }
    }

    /// Jump the base frequency immediately (no glide).
    pub fn set_freq_hz(&mut self, hz: f32) {
        self.freq = hz;
        self.target_freq = hz;
    }

    /// Glide the base frequency toward `hz` at the patch's glide time.
    pub fn glide_to_hz(&mut self, hz: f32) {
        self.target_freq = hz;
    }

    /// The operator power switch (see module docs for the reset contract).
    pub fn set_op_enabled(&mut self, op: usize, on: bool) {
        if on {
            self.reset_op(op);
            self.pending_reset[op] = false;
            self.patch.ops[op].enabled = true;
        } else if self.patch.ops[op].enabled {
            self.patch.ops[op].enabled = false;
            self.pending_reset[op] = true;
        }
    }

    /// Replace the whole patch. If the algorithm changed, the routing is
    /// recompiled and every operator re-strikes from phase 0 — an algorithm
    /// change is a deterministic restructure, not a crossfade. Power-switch
    /// flips inside the new patch go through the same reset contract as
    /// [`Voice::set_op_enabled`].
    pub fn set_patch(&mut self, patch: Patch) {
        let restructure = patch.algorithm != self.patch.algorithm;
        let was_enabled: [bool; NUM_OPS] = core::array::from_fn(|i| self.patch.ops[i].enabled);
        self.patch = patch;
        if restructure {
            self.compiled = compile(self.patch.algorithm);
            self.reset_all();
        }
        for op in 0..NUM_OPS {
            let now_enabled = self.patch.ops[op].enabled;
            if now_enabled {
                if !was_enabled[op] {
                    self.reset_op(op);
                }
                self.pending_reset[op] = false;
            } else if was_enabled[op] {
                self.pending_reset[op] = true;
            }
        }
    }

    /// Switch algorithm, keeping all other patch settings.
    pub fn set_algorithm(&mut self, algorithm: AlgorithmId) {
        let mut patch = self.patch;
        patch.algorithm = algorithm;
        self.set_patch(patch);
    }

    /// Live parameter access. Fine for levels, detune, feedback, glide;
    /// do **not** flip `algorithm` here (use [`Voice::set_algorithm`]) or
    /// `ops[i].enabled` (use [`Voice::set_op_enabled`]) — those two need the
    /// state transitions this accessor bypasses.
    pub fn patch_mut(&mut self) -> &mut Patch {
        &mut self.patch
    }

    pub fn patch(&self) -> &Patch {
        &self.patch
    }

    pub fn compiled(&self) -> &Compiled {
        &self.compiled
    }

    /// Current phase (in cycles, `[0, 1)`) of an operator — for tests and,
    /// later, the UI's structure display.
    pub fn op_phase(&self, op: usize) -> f32 {
        self.phase[op]
    }

    fn reset_op(&mut self, op: usize) {
        self.phase[op] = 0.0;
        self.out[op] = 0.0;
        if op == self.compiled.feedback_op {
            self.fb_hist = [0.0; 2];
        }
    }

    fn reset_all(&mut self) {
        self.phase = [0.0; NUM_OPS];
        self.out = [0.0; NUM_OPS];
        self.fb_hist = [0.0; 2];
        self.rip_line.clear();
        self.rip_sig = 0.0;
    }

    /// Render `count` samples, handing each [`Frame`] to `emit` — the
    /// full-fat path that exposes per-operator outputs for the reverb.
    /// Allocation-free.
    pub fn render_frames(&mut self, count: usize, mut emit: impl FnMut(usize, &Frame)) {
        // Per-block constants (parameters are fixed within a block, so this
        // cannot introduce chunk-size dependence).
        let glide = if self.patch.glide_seconds > 0.0 {
            (-1.0 / (self.patch.glide_seconds * self.sample_rate)).exp()
        } else {
            0.0
        };
        let ramp_step = 1.0 / (RAMP_SECONDS * self.sample_rate);
        let mut freq_mult = [0.0f32; NUM_OPS];
        for (mult, op) in freq_mult.iter_mut().zip(self.patch.ops.iter()) {
            *mult = op.ratio * (op.detune_cents / 1200.0).exp2();
        }
        // The INDEX macro: modulator levels and feedback scale through their
        // per-depth golden-exponent curves; carrier levels are untouched.
        let mut eff_level = [0.0f32; NUM_OPS];
        for i in 0..NUM_OPS {
            let depth = self.compiled.depth[i];
            eff_level[i] = self.patch.ops[i].level
                * if depth > 1 {
                    index_response(depth, self.patch.index)
                } else {
                    1.0
                };
        }
        let eff_feedback = self.patch.feedback
            * index_response(
                self.compiled.depth[self.compiled.feedback_op],
                self.patch.index,
            );
        let mix_norm = self.patch.master_level / self.compiled.carrier_count as f32;
        let inv_carriers = 1.0 / self.compiled.carrier_count as f32;
        let rip = self.patch.rip.clamp(0.0, 1.0);
        self.rip_line.fb = RIP_MAX_FB * rip;
        let rip_amount = RIP_SCALE * rip;
        let eval_order = self.compiled.eval_order;

        for n in 0..count {
            self.freq = self.target_freq + (self.freq - self.target_freq) * glide;

            // The Rip's phase injection, soft-limited (x/(1+|x|)) so
            // recirculation buildup bends the carriers' phase — capped at
            // RIP_SCALE radians — instead of shattering it into noise.
            let rip_pm = rip_amount * (self.rip_sig / (1.0 + self.rip_sig.abs()));

            for &op in eval_order.iter() {
                let enabled = self.patch.ops[op].enabled;

                // Power-switch fade.
                let target = if enabled { 1.0 } else { 0.0 };
                let g = self.gain[op];
                self.gain[op] = if g < target {
                    (g + ramp_step).min(target)
                } else {
                    (g - ramp_step).max(target)
                };

                if !enabled && self.gain[op] == 0.0 {
                    if self.pending_reset[op] {
                        self.reset_op(op);
                        self.pending_reset[op] = false;
                    }
                    self.out[op] = 0.0;
                    continue;
                }

                let mut phase_mod = 0.0;
                for m in 0..NUM_OPS {
                    if self.compiled.modulators[op] >> m & 1 == 1 {
                        phase_mod += self.out[m];
                    }
                }
                let mut angle = TAU * self.phase[op] + MOD_SCALE * phase_mod;
                if op == self.compiled.feedback_op {
                    angle += FB_SCALE * eff_feedback * 0.5 * (self.fb_hist[0] + self.fb_hist[1]);
                }
                // The Rip: the carriers' own delayed, phase-rotated past
                // drips back into their phase input.
                if self.compiled.carriers >> op & 1 == 1 {
                    angle += rip_pm;
                }

                let y = angle.sin() * eff_level[op] * self.gain[op];
                self.out[op] = y;
                if op == self.compiled.feedback_op {
                    self.fb_hist[1] = self.fb_hist[0];
                    self.fb_hist[0] = y;
                }

                let phase = self.phase[op] + self.freq * freq_mult[op] / self.sample_rate;
                self.phase[op] = phase - phase.floor();
            }

            let mut mix = 0.0;
            for op in 0..NUM_OPS {
                if self.compiled.carriers >> op & 1 == 1 {
                    mix += self.out[op];
                }
            }
            // Feed the Rip Line with the carriers' (master-independent) mean;
            // its output modulates the carriers one sample later.
            self.rip_sig = self.rip_line.process(mix * inv_carriers);
            emit(
                n,
                &Frame {
                    ops: self.out,
                    mix: mix * mix_norm,
                },
            );
        }
    }

    /// Render mono samples into `buf`, overwriting it. Allocation-free.
    pub fn render(&mut self, buf: &mut [f32]) {
        self.render_frames(buf.len(), |i, frame| buf[i] = frame.mix);
    }
}
