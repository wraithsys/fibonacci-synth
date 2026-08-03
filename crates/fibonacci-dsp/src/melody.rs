//! The melodic engine: a sample-and-hold pitch source with a Fibonacci-count
//! roster of tunings — one quantized, four that treat 12-TET as optional.
//!
//! Every "random" choice here is deterministic. The default hold source is
//! the **golden Weyl sequence** — `x ← (x + 1/φ) mod 1` — an irrational
//! rotation of the unit interval: provably equidistributed (every degree in
//! range is visited in fair proportion), aperiodic (the sequence never
//! repeats), and reproducible (same settings, same melody, forever). The
//! alternative source is a fixed-seed xorshift32 — noisier in feel, still
//! bit-reproducible.
//!
//! ## Tunings (5 — a Fibonacci number, the same roster rule as everything)
//!
//! 1. **Scale** — quantized to 12-TET, using the 8-scale roster below.
//! 2. **Fibonacci Hz** — the Fibonacci numbers *as raw frequencies*:
//!    34, 55, 89, 144, 233, 377, 610, 987, 1597, 2584 Hz. Unrooted and
//!    absolute. 55 Hz is A1; consecutive rungs converge on φ (987/610 =
//!    1.61803…), so the ladder is a φ-spiral you can climb.
//! 3. **Golden powers** — `root · φᵏ`: the melody walks the same maximally
//!    inharmonic series the Golden ratio mode tunes its operators to.
//! 4. **Plastic powers** — `root · ρᵏ`: steps of log₂(ρ)·1200 ≈ 486 cents,
//!    an alien not-quite-half-octave that belongs to no temperament.
//! 5. **Golden walk** — each fire multiplies the current pitch by φ or 1/φ
//!    (the hold source picks the direction), geometrically reflected off
//!    the range bounds. No grid exists; every melodic interval is golden.
//!    Ranges narrower than log₂(φ)·1200 ≈ 833 cents pin the walk to its
//!    bounds.
//!
//! ## Scales (8 — Fibonacci again): six classical dark scales plus two
//! derived here:
//!
//! - **Fibonacci**: degrees are the Fibonacci numbers themselves mod 12 —
//!   0, 1, 2, 3, 5, 8. The scale terminates itself: the next Fibonacci
//!   number, 13, is octave-equivalent to 1, which is already present.
//! - **Phyllotaxis**: 7 notes placed around the pitch circle at the golden
//!   angle (steps of 12/φ ≈ 7.416 semitones, rounded), the way a sunflower
//!   places seeds. The result is [0,1,3,6,7,8,10] — *Phrygian with the 4th
//!   sharpened to the tritone*. Six of Phrygian's seven notes are golden;
//!   the hunch that Phrygian is φ-flavored is checkable, and checks out.
//!
//! Melodies drive the voice's existing glide, so `Patch::glide_seconds` is
//! the legato control: short glide = stepped line, long glide = the drone
//! prowling.

use crate::patch::{midi_to_hz, PHI, PLASTIC};

/// Scale roster size: a Fibonacci number.
pub const NUM_SCALES: usize = 8;
/// Tuning roster size: also a Fibonacci number.
pub const NUM_TUNINGS: usize = 5;
/// Upper pitch cap for the power-ladder tunings (C8, the top of a piano).
const PITCH_CAP_HZ: f32 = 4186.0;
/// The Fibonacci numbers that sit in comfortable audio range, as Hz.
const FIB_HZ: [f32; 10] = [
    34.0, 55.0, 89.0, 144.0, 233.0, 377.0, 610.0, 987.0, 1597.0, 2584.0,
];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Scale {
    Phrygian,
    PhrygianDominant,
    NaturalMinor,
    HarmonicMinor,
    NeapolitanMinor,
    Byzantine,
    Fibonacci,
    Phyllotaxis,
}

impl Scale {
    pub const ALL: [Scale; NUM_SCALES] = [
        Scale::Phrygian,
        Scale::PhrygianDominant,
        Scale::NaturalMinor,
        Scale::HarmonicMinor,
        Scale::NeapolitanMinor,
        Scale::Byzantine,
        Scale::Fibonacci,
        Scale::Phyllotaxis,
    ];

    /// Semitone offsets from the root, one octave.
    pub fn intervals(self) -> &'static [u8] {
        match self {
            Scale::Phrygian => &[0, 1, 3, 5, 7, 8, 10],
            Scale::PhrygianDominant => &[0, 1, 4, 5, 7, 8, 10],
            Scale::NaturalMinor => &[0, 2, 3, 5, 7, 8, 10],
            Scale::HarmonicMinor => &[0, 2, 3, 5, 7, 8, 11],
            Scale::NeapolitanMinor => &[0, 1, 3, 5, 7, 8, 11],
            // Double harmonic major.
            Scale::Byzantine => &[0, 1, 4, 5, 7, 8, 11],
            Scale::Fibonacci => &[0, 1, 2, 3, 5, 8],
            Scale::Phyllotaxis => &[0, 1, 3, 6, 7, 8, 10],
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Scale::Phrygian => "phrygian",
            Scale::PhrygianDominant => "phrygdom",
            Scale::NaturalMinor => "nat minor",
            Scale::HarmonicMinor => "harm minor",
            Scale::NeapolitanMinor => "neapolitan",
            Scale::Byzantine => "byzantine",
            Scale::Fibonacci => "fibonacci",
            Scale::Phyllotaxis => "phyllotaxis",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tuning {
    Scale,
    FibonacciHz,
    GoldenPowers,
    PlasticPowers,
    GoldenWalk,
}

impl Tuning {
    pub const ALL: [Tuning; NUM_TUNINGS] = [
        Tuning::Scale,
        Tuning::FibonacciHz,
        Tuning::GoldenPowers,
        Tuning::PlasticPowers,
        Tuning::GoldenWalk,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Tuning::Scale => "scale",
            Tuning::FibonacciHz => "fib hz",
            Tuning::GoldenPowers => "phi powers",
            Tuning::PlasticPowers => "rho powers",
            Tuning::GoldenWalk => "phi walk",
        }
    }
}

/// Where the sample-and-hold draws its values. Both are deterministic.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum HoldSource {
    /// `x ← (x + 1/φ) mod 1`: equidistributed, aperiodic, golden.
    GoldenWeyl,
    /// Fixed-seed xorshift32: noise-flavored, still reproducible.
    Xorshift,
}

#[derive(Clone, Copy, Debug)]
pub struct MelodyParams {
    pub enabled: bool,
    pub tuning: Tuning,
    /// Used when `tuning == Tuning::Scale`.
    pub scale: Scale,
    /// MIDI note of the root. Ignored by `FibonacciHz`, which is absolute.
    pub root_midi: u8,
    /// Reach: scale degrees (Scale), ladder rungs (Fibonacci/φ/ρ powers),
    /// or semitones above root (GoldenWalk bounds).
    pub range_degrees: u8,
    /// Sample-and-hold rate in Hz.
    pub rate_hz: f32,
    pub source: HoldSource,
}

impl Default for MelodyParams {
    fn default() -> Self {
        MelodyParams {
            enabled: false,
            tuning: Tuning::Scale,
            scale: Scale::Phrygian,
            root_midi: 45, // A2 = 110 Hz, the home drone
            range_degrees: 8,
            rate_hz: PHI, // 1.618 notes per second — a default, not a law
            source: HoldSource::GoldenWeyl,
        }
    }
}

pub struct Melody {
    params: MelodyParams,
    sample_rate: f32,
    /// Samples until the next hold fires.
    countdown: usize,
    weyl: f32,
    rng: u32,
    /// Current pitch of the golden walk.
    walk_hz: f32,
}

impl Melody {
    pub fn new(sample_rate: f32, params: MelodyParams) -> Self {
        let mut m = Melody {
            params,
            sample_rate,
            countdown: 0,
            weyl: 0.0,
            rng: 0x9E37_79B9, // floor(2^32 / φ), naturally
            walk_hz: midi_to_hz(params.root_midi),
        };
        m.countdown = m.period();
        m
    }

    fn period(&self) -> usize {
        ((self.sample_rate / self.params.rate_hz.max(0.01)) as usize).max(1)
    }

    pub fn params(&self) -> MelodyParams {
        self.params
    }

    pub fn set_params(&mut self, params: MelodyParams) {
        let was_disabled = !self.params.enabled;
        if params.tuning != self.params.tuning || params.root_midi != self.params.root_midi {
            self.walk_hz = midi_to_hz(params.root_midi);
        }
        self.params = params;
        self.countdown = self.countdown.min(self.period());
        if was_disabled && params.enabled {
            self.countdown = 0; // first note lands immediately
        }
    }

    /// Samples until the next hold fires, or `None` while disabled.
    pub fn samples_until_fire(&self) -> Option<usize> {
        self.params.enabled.then_some(self.countdown)
    }

    pub fn advance(&mut self, samples: usize) {
        if self.params.enabled {
            self.countdown = self.countdown.saturating_sub(samples);
        }
    }

    fn draw(&mut self) -> f32 {
        match self.params.source {
            HoldSource::GoldenWeyl => {
                self.weyl = (self.weyl + (PHI - 1.0)).fract();
                self.weyl
            }
            HoldSource::Xorshift => {
                self.rng ^= self.rng << 13;
                self.rng ^= self.rng >> 17;
                self.rng ^= self.rng << 5;
                (self.rng >> 8) as f32 / (1u32 << 24) as f32
            }
        }
    }

    /// Draw the next pitch and re-arm the countdown. Call when
    /// `samples_until_fire()` reports 0.
    pub fn fire(&mut self) -> f32 {
        self.countdown = self.period();
        let x = self.draw();
        let range = self.params.range_degrees.max(1) as usize;
        let degree = ((x * range as f32) as usize).min(range - 1);
        let root_hz = midi_to_hz(self.params.root_midi);
        match self.params.tuning {
            Tuning::Scale => {
                let intervals = self.params.scale.intervals();
                let midi = self.params.root_midi as usize
                    + 12 * (degree / intervals.len())
                    + intervals[degree % intervals.len()] as usize;
                midi_to_hz(midi.min(127) as u8)
            }
            Tuning::FibonacciHz => FIB_HZ[degree.min(FIB_HZ.len() - 1)],
            Tuning::GoldenPowers => (root_hz * PHI.powi(degree as i32)).min(PITCH_CAP_HZ),
            Tuning::PlasticPowers => (root_hz * PLASTIC.powi(degree as i32)).min(PITCH_CAP_HZ),
            Tuning::GoldenWalk => {
                let hi = root_hz * (self.params.range_degrees.max(1) as f32 / 12.0).exp2();
                let mut hz = self.walk_hz.clamp(root_hz, hi);
                hz = if x >= 0.5 { hz * PHI } else { hz / PHI };
                // Geometric reflection off the bounds (a mirror in log-
                // frequency), then a clamp for ranges too narrow to hold
                // one full phi step.
                if hz > hi {
                    hz = hi * hi / hz;
                }
                if hz < root_hz {
                    hz = root_hz * root_hz / hz;
                }
                hz = hz.clamp(root_hz, hi);
                self.walk_hz = hz;
                hz
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f32 = 48_000.0;

    fn hz_to_midi(hz: f32) -> i32 {
        (69.0 + 12.0 * (hz / 440.0).log2()).round() as i32
    }

    /// The Phyllotaxis scale really is golden-angle placement on the pitch
    /// circle — derived here exactly as documented.
    #[test]
    fn phyllotaxis_is_golden_angle_placement() {
        let mut notes: Vec<u8> = (0..7)
            .map(|k| ((k as f32 * 12.0 / PHI) % 12.0).round() as u8 % 12)
            .collect();
        notes.sort_unstable();
        notes.dedup();
        assert_eq!(notes.as_slice(), Scale::Phyllotaxis.intervals());
        // And the documented Phrygian kinship: identical except 5 -> 6.
        let phrygian = Scale::Phrygian.intervals();
        let shared = Scale::Phyllotaxis
            .intervals()
            .iter()
            .filter(|i| phrygian.contains(i))
            .count();
        assert_eq!(shared, 6);
    }

    /// The Fibonacci scale's degrees are the Fibonacci numbers mod 12, and
    /// the scale terminates itself: F = 13 is octave-equivalent to 1.
    #[test]
    fn fibonacci_scale_derives_and_self_terminates() {
        let mut degrees = vec![0u8];
        let (mut a, mut b) = (1u32, 1u32);
        loop {
            let n = (a % 12) as u8;
            if degrees.contains(&n) && a > 1 {
                assert_eq!(a, 13, "termination should occur at F = 13");
                break;
            }
            if !degrees.contains(&n) {
                degrees.push(n);
            }
            let next = a + b;
            b = a;
            a = next;
        }
        degrees.sort_unstable();
        assert_eq!(degrees.as_slice(), Scale::Fibonacci.intervals());
    }

    /// The Fibonacci-Hz ladder's consecutive ratios converge on φ.
    #[test]
    fn fibonacci_hz_ladder_converges_on_phi() {
        assert!((FIB_HZ[7] / FIB_HZ[6] - PHI).abs() < 1e-4, "987/610 vs phi");
        for w in FIB_HZ.windows(2) {
            let ratio = w[1] / w[0];
            assert!((ratio - PHI).abs() < 0.02, "ratio {ratio} strays from phi");
        }
    }

    /// Same params, same melody — for every tuning and both hold sources.
    #[test]
    fn melodies_are_deterministic() {
        for tuning in Tuning::ALL {
            for source in [HoldSource::GoldenWeyl, HoldSource::Xorshift] {
                let params = MelodyParams {
                    enabled: true,
                    tuning,
                    source,
                    ..Default::default()
                };
                let mut a = Melody::new(SR, params);
                let mut b = Melody::new(SR, params);
                for _ in 0..500 {
                    assert_eq!(a.fire(), b.fire(), "{tuning:?}/{source:?}");
                }
            }
        }
    }

    /// Every quantized note lands inside the selected scale and above root.
    #[test]
    fn fired_notes_stay_in_scale() {
        for scale in Scale::ALL {
            for source in [HoldSource::GoldenWeyl, HoldSource::Xorshift] {
                let params = MelodyParams {
                    enabled: true,
                    scale,
                    source,
                    range_degrees: 11,
                    ..Default::default()
                };
                let root = params.root_midi;
                let mut m = Melody::new(SR, params);
                for _ in 0..300 {
                    let offset = hz_to_midi(m.fire()) - root as i32;
                    assert!(offset >= 0, "{scale:?}: note below root");
                    assert!(
                        scale.intervals().contains(&((offset % 12) as u8)),
                        "{scale:?}: {offset} not in scale"
                    );
                }
            }
        }
    }

    /// The unquantized tunings stay inside their documented bounds, and the
    /// golden walk's unreflected steps are exactly phi.
    #[test]
    fn free_tunings_respect_their_bounds() {
        let root_hz = midi_to_hz(45);
        for tuning in [Tuning::GoldenPowers, Tuning::PlasticPowers, Tuning::GoldenWalk] {
            let params = MelodyParams {
                enabled: true,
                tuning,
                range_degrees: 24,
                ..Default::default()
            };
            let mut m = Melody::new(SR, params);
            let mut prev: Option<f32> = None;
            for _ in 0..500 {
                let hz = m.fire();
                assert!(hz >= root_hz * 0.999 && hz <= PITCH_CAP_HZ, "{tuning:?}: {hz}");
                if tuning == Tuning::GoldenWalk {
                    let hi = root_hz * (24.0 / 12.0_f32).exp2();
                    assert!(hz <= hi * 1.001, "walk escaped its bounds: {hz}");
                    if let Some(p) = prev {
                        let r = if hz > p { hz / p } else { p / hz };
                        // Steps are phi unless a reflection folded them.
                        assert!(
                            (r - PHI).abs() < 1e-3 || r < PHI,
                            "walk step {r} exceeds phi"
                        );
                    }
                    prev = Some(hz);
                }
            }
        }
    }

    /// The golden Weyl source is equidistributed: with range 7, every
    /// degree is visited in fair proportion.
    #[test]
    fn golden_weyl_visits_every_degree_fairly() {
        let params = MelodyParams {
            enabled: true,
            range_degrees: 7,
            ..Default::default()
        };
        let mut m = Melody::new(SR, params);
        let mut counts = [0usize; 7];
        let intervals = params.scale.intervals();
        for _ in 0..700 {
            let offset = (hz_to_midi(m.fire()) - params.root_midi as i32) as u8 % 12;
            let degree = intervals.iter().position(|&i| i == offset).unwrap();
            counts[degree] += 1;
        }
        for (d, &c) in counts.iter().enumerate() {
            assert!(c >= 50, "degree {d} visited only {c}/700 times");
        }
    }

    /// The countdown API fires at the configured rate.
    #[test]
    fn fires_at_the_configured_rate() {
        let params = MelodyParams {
            enabled: true,
            rate_hz: 2.0,
            ..Default::default()
        };
        let mut m = Melody::new(SR, params);
        let mut fires = 0;
        let mut remaining = (SR * 10.0) as usize; // 10 seconds
        while remaining > 0 {
            match m.samples_until_fire() {
                Some(0) => {
                    m.fire();
                    fires += 1;
                }
                Some(c) => {
                    let step = c.min(remaining);
                    m.advance(step);
                    remaining -= step;
                }
                None => break,
            }
        }
        assert!((19..=21).contains(&fires), "expected ~20 fires, got {fires}");
    }
}
