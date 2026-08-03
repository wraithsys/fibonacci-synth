//! Patch data: everything a sound *is*, independent of any running voice.

use crate::algorithm::{compile, AlgorithmId, NUM_OPS};

/// The golden ratio, `(1 + sqrt(5)) / 2` — the limit of consecutive
/// Fibonacci-number ratios.
pub const PHI: f32 = 1.618_034;

/// The plastic number ρ, the real root of `x³ = x + 1` — the limit of
/// consecutive Padovan-sequence ratios (`P(n) = P(n-2) + P(n-3)`), the
/// Fibonacci recursion's nearest cousin.
pub const PLASTIC: f32 = 1.324_718;

/// Convert a MIDI note number to Hz (A4 = 69 = 440 Hz, 12-TET).
pub fn midi_to_hz(note: u8) -> f32 {
    440.0 * ((note as f32 - 69.0) / 12.0).exp2()
}

/// The INDEX macro's per-operator response curve: `index^(φ^(depth−2))`.
///
/// One knob scales every modulation source, but each operator follows its own
/// nonlinearity, shaped by its position in the tree:
///
/// - depth 2 → exponent φ⁰ = 1 (linear)
/// - depth 3 → exponent φ ≈ 1.618
/// - depth 4 → exponent φ² ≈ 2.618
/// - depth 1 (a carrier's self-feedback) → exponent 1/φ ≈ 0.618 (concave —
///   feedback grit is the *first* thing to arrive)
///
/// All curves hit exactly 0 at index 0 and exactly 1 at index 1, so index 0
/// is pure sine carriers and index 1 is the stock sound. Deeper operators
/// get more convex curves, so raising the knob blooms the tree from the
/// shallow modulators downward — the INDEX knob unfolds the tree in time.
pub fn index_response(depth: u8, index: f32) -> f32 {
    index.clamp(0.0, 1.0).powf(PHI.powi(depth as i32 - 2))
}

/// Operator frequency-ratio model. Each model is documented in README.md;
/// ratios are relative to the voice's base frequency.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum RatioMode {
    /// Integer harmonic series: 1, 2, 3, 4, 5. The conventional, consonant
    /// reference point.
    Harmonic,
    /// The Fibonacci numbers themselves: 1, 1, 2, 3, 5. A subset of the
    /// harmonic series, so still consonant — but operators 0 and 1 sit at
    /// unison, which is where per-op detune turns into thickness.
    Fibonacci,
    /// Powers of the golden ratio: 1, φ, φ², φ³, φ⁴. φ is the "most
    /// irrational" number (its continued fraction is all 1s), so these
    /// partials are maximally far from any harmonic relationship —
    /// maximally inharmonic, bell-like/industrial.
    ///
    /// **This is the instrument's identity mode and the default.**
    Golden,
    /// φ-powers mirrored around the fundamental: 1/φ², 1/φ, 1, φ, φ².
    /// Same maximal inharmonicity as `Golden`, but two partials sit *below*
    /// the base frequency — and since operator 0 (the deepest modulator and
    /// feedback home) is now the lowest partial, deep algorithms get slow,
    /// heaving modulation. Subterranean variant of the identity sound.
    GoldenMirror,
    /// Powers of the plastic number: 1, ρ, ρ², ρ³, ρ⁴ (ρ ≈ 1.3247, the real
    /// root of x³ = x + 1, limit of Padovan-sequence ratios). Inharmonic
    /// like `Golden` but packed much tighter (top partial ~3.1× instead of
    /// ~6.9×): denser, darker clusters.
    Plastic,
}

/// The identity mode: [`RatioMode::Golden`].
impl Default for RatioMode {
    fn default() -> Self {
        RatioMode::Golden
    }
}

impl RatioMode {
    /// The frequency ratio this model assigns to operator `op` (0-based).
    pub fn ratio(self, op: usize) -> f32 {
        debug_assert!(op < NUM_OPS);
        match self {
            RatioMode::Harmonic => [1.0, 2.0, 3.0, 4.0, 5.0][op],
            RatioMode::Fibonacci => [1.0, 1.0, 2.0, 3.0, 5.0][op],
            RatioMode::Golden => PHI.powi(op as i32),
            RatioMode::GoldenMirror => PHI.powi(op as i32 - 2),
            RatioMode::Plastic => PLASTIC.powi(op as i32),
        }
    }

    /// Every mode, in enum order. The roster deliberately holds a Fibonacci
    /// number of modes (5), the same rule the algorithm roster follows.
    pub const ALL: [RatioMode; 5] = [
        RatioMode::Harmonic,
        RatioMode::Fibonacci,
        RatioMode::Golden,
        RatioMode::GoldenMirror,
        RatioMode::Plastic,
    ];
}

/// Per-operator parameters.
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct OpParams {
    /// The drone power switch. Turning an operator off is a true reset, not a
    /// bypass: it fades out over ~2 ms, then its phase accumulator and (for
    /// the feedback operator) feedback history are zeroed. Turning it on
    /// starts from phase 0, deterministically. See [`crate::voice::Voice`].
    pub enabled: bool,
    /// Frequency ratio relative to the voice's base frequency. Initialized
    /// from the patch's [`RatioMode`]; user-overridable per op.
    pub ratio: f32,
    /// Detune in cents, applied on top of `ratio`.
    pub detune_cents: f32,
    /// Output level, 0..=1. For a carrier this is mix amplitude; for a
    /// modulator it sets modulation index (see `MOD_SCALE` in the voice).
    pub level: f32,
}

/// A complete sound description.
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Patch {
    pub algorithm: AlgorithmId,
    pub ratio_mode: RatioMode,
    pub ops: [OpParams; NUM_OPS],
    /// Self-feedback amount, 0..=1, on the algorithm's feedback operator.
    pub feedback: f32,
    /// The INDEX macro, 0..=1: scales every modulation source through the
    /// per-depth golden-exponent curves of [`index_response`]. 0 = pure sine
    /// carriers; 1 = the stock sound.
    pub index: f32,
    /// The Rip, 0..=1 (0 = off, bit-identical to no Rip Line at all): the
    /// carriers' own output, delayed by the golden interval and phase-rotated
    /// π/5 per recirculation, is injected back into the *carriers' phase
    /// input*. When the rotation drifts toward inversion it interferes
    /// destructively with the live modulation — the spectrum hollows and
    /// blooms cyclically. See README §11 and [`crate::voice`].
    pub rip: f32,
    /// Master output level, 0..=1, applied after carrier-mean normalization.
    pub master_level: f32,
    /// Portamento time constant in seconds for pitch changes. 0 = instant.
    pub glide_seconds: f32,
}

impl Patch {
    /// A playable starting patch for the given algorithm and ratio mode.
    ///
    /// Carriers get level 1.0. Modulators get `1/φ^(depth-1)` — the golden
    /// decay keeps deep stacks bounded, and the exponent (one power hotter
    /// than the original `1/φ^depth`) plus feedback 0.5 are the audition-
    /// chosen "hard" defaults: the instrument starts at its intended energy,
    /// not at a polite version of it (documented in README.md).
    pub fn init(algorithm: AlgorithmId, ratio_mode: RatioMode) -> Self {
        let compiled = compile(algorithm);
        let mut ops = [OpParams {
            enabled: true,
            ratio: 1.0,
            detune_cents: 0.0,
            level: 1.0,
        }; NUM_OPS];
        for (i, op) in ops.iter_mut().enumerate() {
            op.ratio = ratio_mode.ratio(i);
            if compiled.depth[i] > 1 {
                op.level = PHI.powi(-(compiled.depth[i] as i32 - 1));
            }
        }
        Patch {
            algorithm,
            ratio_mode,
            ops,
            feedback: 0.5,
            index: 1.0,
            rip: 0.0,
            master_level: 0.8,
            glide_seconds: 0.05,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::algorithm::ALGORITHMS;

    #[test]
    fn golden_ratios_are_phi_powers() {
        for op in 0..NUM_OPS {
            let expected = PHI.powi(op as i32);
            assert!((RatioMode::Golden.ratio(op) - expected).abs() < 1e-4);
        }
        // phi^2 = phi + 1, the defining identity.
        assert!((PHI * PHI - (PHI + 1.0)).abs() < 1e-4);
    }

    #[test]
    fn init_levels_follow_golden_decay() {
        let patch = Patch::init(ALGORITHMS[0], RatioMode::Harmonic);
        let compiled = compile(ALGORITHMS[0]);
        for i in 0..NUM_OPS {
            if compiled.carriers >> i & 1 == 1 {
                assert_eq!(patch.ops[i].level, 1.0);
            } else {
                let expected = PHI.powi(-(compiled.depth[i] as i32 - 1));
                assert!((patch.ops[i].level - expected).abs() < 1e-6);
                assert!(patch.ops[i].level < 1.0);
            }
        }
    }

    #[test]
    fn plastic_number_satisfies_its_cubic() {
        // ρ³ = ρ + 1, the defining identity.
        assert!((PLASTIC.powi(3) - (PLASTIC + 1.0)).abs() < 1e-4);
        assert!((RatioMode::Plastic.ratio(4) - PLASTIC.powi(4)).abs() < 1e-4);
    }

    #[test]
    fn golden_mirror_is_symmetric_around_the_fundamental() {
        let m = RatioMode::GoldenMirror;
        assert!((m.ratio(2) - 1.0).abs() < 1e-6, "center op is the fundamental");
        assert!((m.ratio(0) * m.ratio(4) - 1.0).abs() < 1e-4);
        assert!((m.ratio(1) * m.ratio(3) - 1.0).abs() < 1e-4);
        assert!(m.ratio(0) < 1.0 && m.ratio(1) < 1.0, "two partials below base");
    }

    #[test]
    fn index_response_curves_are_ordered_by_depth() {
        for depth in 1..=4u8 {
            assert_eq!(index_response(depth, 0.0), 0.0, "depth {depth} at zero");
            assert!((index_response(depth, 1.0) - 1.0).abs() < 1e-6, "depth {depth} at one");
        }
        // Depth 2 is exactly linear.
        assert!((index_response(2, 0.3) - 0.3).abs() < 1e-6);
        // Mid-knob, deeper ops are further behind: the tree blooms top-down.
        let x = 0.5;
        assert!(index_response(1, x) > index_response(2, x));
        assert!(index_response(2, x) > index_response(3, x));
        assert!(index_response(3, x) > index_response(4, x));
        // Monotone spot check.
        assert!(index_response(4, 0.8) > index_response(4, 0.4));
    }

    #[test]
    fn mode_roster_count_is_fibonacci() {
        assert_eq!(RatioMode::ALL.len(), 5);
        assert_eq!(RatioMode::default(), RatioMode::Golden);
    }

    #[test]
    fn midi_reference_pitches() {
        assert!((midi_to_hz(69) - 440.0).abs() < 1e-3);
        assert!((midi_to_hz(45) - 110.0).abs() < 1e-3);
    }
}
