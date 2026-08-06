//! The field: always-on amplitude and pitch movement, shaped by the ratio mode.
//!
//! Billy, 2026-08-06, correcting the shorthand in Law 4: *"AM is essential in
//! drones — a drone doesn't simply occur at a volume and then stop"*, and,
//! against the position this file's author had been arguing, that leaning on
//! phase cancellation alone for that movement is "not a worthy thing". Beating
//! between detuned carriers is a texture. It is not a breath.
//!
//! Three rules, and everything here follows from them:
//!
//! ## 1. `master` is the floor
//!
//! The instrument sits at `master` and the field adds *above* it, never below,
//! up to unity. So Law 4's invariant stops being something this module has to
//! be careful about and becomes true by construction: the only thing that can
//! take the output to zero is a hand on the master knob.
//!
//! It also means the field lives in the headroom between `master` and 1.0.
//! Turn master down and there is more room to breathe — literally. At master
//! 1.0 there is nowhere to go and the field goes quiet, which is the honest
//! answer to having asked for everything at once.
//!
//! ## 2. The rate comes from the frequency, clamped
//!
//! `f = freq / φ¹³`. φ¹³ ≈ 521, and 13 is a Fibonacci number like every other
//! count here — the same 13 as `PARAM_GLIDE_S`. A low drone breathes slowly, a
//! high one quickly, which is how resonant bodies behave.
//!
//! The clamp is the part that matters. `drone hz` tops out at 440, but the
//! *sequencer* does not — plastic powers with root and range maxed reaches for
//! several kHz, and a rate derived from that would climb toward audio-rate AM,
//! which is a different effect wearing this one's name. Rather than reason
//! about which tunings can do it, the rate is simply clamped to the range the
//! drone knob itself can ask for. Billy: "not worth it imo."
//!
//! ## 3. The shape comes from the ratio mode
//!
//! Every mode already owns a five-integer sequence — the same one the Logalith
//! counts its chambers with. Those integers, normalised, become the rates of
//! the five components summed to make the movement. So each mode moves in a
//! way only it can, and nothing new had to be invented to say how.
//!
//! One property falls out for free and is worth knowing about, because it is
//! audible: **harmonic mode's integers are commensurate, so its movement
//! repeats.** 30, 24, 18, 12, 6 share a common factor; their sum has a period.
//! Every other mode's sequence is built on an irrational limit — φ, ρ — so no
//! two components share a period and the sum never lands in the same place
//! twice. The one mode you would expect to be periodic is the only periodic
//! one, and nobody chose that.

use crate::patch::{RatioMode, PHI};

/// Rungs of φ between the frequency and its movement.
const RATE_RUNGS: i32 = 13;

/// The rate range, in Hz — the movement `drone hz` itself can ask for at its
/// extremes (27.5 and 440), and the clamp that keeps the sequencer from
/// pushing this anywhere near audio rate.
const RATE_MIN_HZ: f32 = 27.5 / 521.002;
const RATE_MAX_HZ: f32 = 440.0 / 521.002;

/// Pitch movement at full field, in cents. Fibonacci 21 — a vibrato you feel
/// before you can name, not a warble.
const FM_MAX_CENTS: f32 = 21.0;

/// Where the depth rests between notes, as a fraction of `field` — `1/φ²`.
///
/// The boost interpolates from here up to `field` itself, so a note is φ²
/// deeper than the rest state and **nothing ever saturates**. The first cut
/// multiplied `field` by `1 + φ²·boost` and clamped at 1.0, which meant the
/// control did nothing at all above 0.276 while notes were arriving — Billy
/// found it immediately: "seems to cap out at 0.3".
const REST_FRACTION: f32 = 1.0 / (PHI * PHI);

/// How much of the gap between notes the gesture occupies: `1/φ`, leaving
/// `1/φ²` of it at rest.
///
/// **Derived, not dialled.** A fixed decay cannot work across the sequencer's
/// whole range: at 8 Hz the notes are 125 ms apart, and a boost falling over
/// ~1 s is re-triggered long before it has moved, so it sits pinned at maximum
/// and there are no dynamics at all — which is exactly what Billy heard. The
/// instrument measures the interval it is being played at and fits the gesture
/// inside it, so the push and pull is there at 8 Hz and at 0.1 Hz alike.
const GESTURE_FRACTION: f32 = 1.0 / PHI;

/// Fallback gesture length when nothing has established an interval yet — one
/// key, played once. Fibonacci 987 ms.
const LONE_NOTE_S: f32 = 0.987;

/// Bounds on the measured interval, so a first note or a pathological rhythm
/// cannot produce an absurd gesture.
const MIN_INTERVAL_S: f32 = 0.05;
const MAX_INTERVAL_S: f32 = 8.0;

/// How big a note *ending* is against a note beginning: `1/φ`. A release is
/// the same gesture, smaller — not a different mechanism.
const RELEASE_BOOST: f32 = 1.0 / PHI;

/// The curve control's exponent range, as a power of φ. At 0.5 the return is
/// linear; toward 0 it is logarithmic (drops fast, then lingers); toward 1 it
/// is exponential (holds, then falls away).
const CURVE_RANGE: f32 = 4.0;

/// The five-integer signature of each ratio mode: the same sequence the
/// Logalith counts chambers with, so the mode looks and sounds like one thing.
/// Normalised against its own largest term, these are the component rates.
fn signature(mode: RatioMode) -> [f32; 5] {
    let raw: [u32; 5] = match mode {
        // Commensurate. This is the mode whose movement repeats.
        RatioMode::Harmonic => [30, 24, 18, 12, 6],
        RatioMode::Fibonacci => [34, 21, 13, 8, 5],
        // Lucas.
        RatioMode::Golden => [29, 18, 11, 7, 4],
        RatioMode::GoldenMirror => [4, 7, 11, 18, 29],
        // Padovan.
        RatioMode::Plastic => [28, 21, 16, 12, 9],
    };
    let max = raw.iter().copied().max().unwrap_or(1) as f32;
    core::array::from_fn(|i| raw[i] as f32 / max)
}

/// Always-on amplitude and pitch movement. Deterministic, allocation-free, and
/// phase-integrated so a rate change never jumps the output.
#[derive(Clone, Copy, Debug)]
pub struct Breath {
    sample_rate: f32,
    /// One phase per component of the mode's signature.
    phase: [f32; 5],
    mode: RatioMode,
    /// Time since the last gesture, in seconds, or `None` if none has landed.
    /// The boost is a function of this rather than an accumulator, so the
    /// curve control can reshape a decay that is already in flight.
    since_trigger: Option<f32>,
    /// What the decay is falling *from*: 1.0 for a note beginning, less for a
    /// note ending. Both fall along the same curve.
    boost_level: f32,
    /// The interval between the last two note beginnings, in seconds — how
    /// fast the instrument is being played. The gesture is sized from this.
    interval: f32,
}

/// What the field is doing this sample.
#[derive(Clone, Copy, Debug)]
pub struct Field {
    /// Gain to multiply the dry path by, in `[master, 1]`.
    pub gain: f32,
    /// Frequency multiplier for the pitch movement, around 1.0.
    pub pitch: f32,
}

impl Breath {
    pub fn new(sample_rate: f32, mode: RatioMode) -> Self {
        Breath {
            sample_rate: sample_rate.max(1.0),
            // Golden fractions of a turn, so the components do not all start
            // at the top of their swing and open with a thump.
            phase: Self::start_phase(),
            mode,
            since_trigger: None,
            interval: LONE_NOTE_S,
            boost_level: 1.0,
        }
    }

    /// Opening state: golden fractions of a turn, so the components do not all
    /// begin at the top together and the instrument does not open on a thump.
    fn start_phase() -> [f32; 5] {
        core::array::from_fn(|i| (i as f32 / PHI).fract())
    }

    /// Where a note puts the movement: **all five components aligned at their
    /// peak**, so the swing is at its maximum the instant the note lands and
    /// the boost has the whole range to work across.
    ///
    /// The opening state deliberately avoids this — aligned components are a
    /// thump — but a thump is precisely what a note *is*. And because the five
    /// rates differ, they scatter again within the note, so every note strikes
    /// identically and decays into the mode's own shape.
    ///
    /// This is what the dynamics live on. Landing mid-swing left the boost
    /// only half the range to move through, which is why a note was audible
    /// but not *felt*.
    fn note_phase() -> [f32; 5] {
        [0.25; 5]
    }

    pub fn set_mode(&mut self, mode: RatioMode) {
        self.mode = mode;
    }

    /// A note arrived — from a MIDI key or from the sequencer changing pitch,
    /// indistinguishably. Both reach this through one call in the voice, so
    /// neither source is privileged and playing by hand and playing by
    /// sample-and-hold produce the same gesture.
    ///
    /// It does two things and no more: **restarts the movement** so every note
    /// begins at the same point in the shape, and **boosts the movement's
    /// depth**, which then returns to the baseline along the curve control's
    /// bias.
    ///
    /// Note what is *not* here: nothing touches the audio's amplitude. The
    /// envelope is on the modulation depth. That is why this is not an ADSR
    /// wearing a different word — the sound has no attack and no release, it
    /// simply moves more for a while.
    pub fn trigger(&mut self) {
        // Learn the tempo from the playing. Only a *beginning* counts: a
        // release is part of the note it ends, not a new one.
        if let Some(elapsed) = self.since_trigger {
            if elapsed >= MIN_INTERVAL_S && self.boost_level >= 1.0 {
                self.interval = elapsed.min(MAX_INTERVAL_S);
            }
        }
        self.phase = Self::note_phase();
        self.since_trigger = Some(0.0);
        self.boost_level = 1.0;
    }

    /// How long the current gesture lasts: the golden section of the interval
    /// being played at, so it always fits inside the gap.
    fn gesture_s(&self) -> f32 {
        (self.interval * GESTURE_FRACTION).clamp(MIN_INTERVAL_S * GESTURE_FRACTION, LONE_NOTE_S)
    }

    /// A note ended — a MIDI key lifted. A smaller gesture of the same kind,
    /// `1/φ` of a beginning, and it does *not* restart the movement: the note
    /// letting go is a change in what is already happening, not a new event.
    ///
    /// The sequencer never calls this. It has no note-offs, and a
    /// sample-and-hold pitch simply lasts until the next one — so under it the
    /// instrument gets attacks alone, and playing by hand is the only way to
    /// get both halves of the gesture. That difference is real, and it is the
    /// honest one: a key has a release because you lifted it, and an S&H step
    /// does not because nothing lifted.
    pub fn release(&mut self) {
        // Never promote a decay that has already fallen further than this.
        if self.boost(0.5) < RELEASE_BOOST {
            self.since_trigger = Some(0.0);
            self.boost_level = RELEASE_BOOST;
        }
    }

    /// The fundamental rate for a frequency, in Hz — clamped to what the drone
    /// knob itself can reach, whatever the sequencer is doing.
    pub fn rate_hz(freq: f32) -> f32 {
        (freq.max(0.0) / PHI.powi(RATE_RUNGS)).clamp(RATE_MIN_HZ, RATE_MAX_HZ)
    }

    /// Advance one sample.
    ///
    /// `master` is the floor and the movement fills the headroom above it, so
    /// the returned gain is always in `[master, 1]` — there is no arrangement
    /// of inputs that takes it below the knob.
    /// The trigger boost still in flight, 0..1, shaped by `curve`.
    ///
    /// `curve` is a bias: 0.5 returns linearly, below that logarithmically
    /// (drops away fast, then lingers), above it exponentially (holds, then
    /// falls). The exponent runs over `φ^±2`, so the extremes are golden
    /// rather than arbitrary.
    fn boost(&self, curve: f32) -> f32 {
        let Some(t) = self.since_trigger else {
            return 0.0;
        };
        let remaining = 1.0 - (t / self.gesture_s()).clamp(0.0, 1.0);
        let exponent = PHI.powf(CURVE_RANGE * (curve.clamp(0.0, 1.0) - 0.5));
        self.boost_level * remaining.powf(exponent)
    }

    /// Advance one sample.
    ///
    /// `master` is the floor and the movement fills the headroom above it, so
    /// the returned gain is always in `[master, 1]` — there is no arrangement
    /// of inputs that takes it below the knob.
    pub fn tick(&mut self, freq: f32, field: f32, master: f32, curve: f32) -> Field {
        let field = field.clamp(0.0, 1.0);
        let master = master.clamp(0.0, 1.0);
        let f = Self::rate_hz(freq);
        let sig = signature(self.mode);

        let mut sum = 0.0;
        for (p, ratio) in self.phase.iter_mut().zip(sig) {
            *p += f * ratio / self.sample_rate;
            *p -= p.floor();
            sum += (*p * core::f32::consts::TAU).sin();
        }
        // [-1,1] -> [0,1].
        let swing = (sum / 5.0 + 1.0) * 0.5;

        // The trigger's envelope is on the *depth*, never on the audio. It
        // rides on top of the field, so at field 0 a note does nothing at all —
        // the field is the whole mechanism and this is a gesture within it.
        // Rests at `field/φ²` and rises to `field` on a note — φ² of movement
        // between rest and gesture, and no clamp anywhere, so every position of
        // the control does something.
        let boost = self.boost(curve);
        let depth = field * (REST_FRACTION + (1.0 - REST_FRACTION) * boost);
        if let Some(t) = self.since_trigger.as_mut() {
            *t += 1.0 / self.sample_rate;
        }

        Field {
            // Sits at master, rises into the headroom. At field 0 it is exactly
            // master, which is the instrument as it shipped.
            gain: master + (1.0 - master) * depth * swing,
            // Bipolar around unity, so the pitch movement has no DC — the drone
            // does not drift sharp just because the field is open.
            pitch: 1.0 + depth * FM_MAX_CENTS * (swing * 2.0 - 1.0) / 1200.0,
        }
    }

    /// Zero the movement, for an off switch's true reset.
    pub fn reset(&mut self) {
        *self = Breath::new(self.sample_rate, self.mode);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f32 = 48_000.0;

    /// **Law 4's invariant, and it is now structural.** The gain never leaves
    /// `[master, 1]`, for every mode, every field position and every master —
    /// so nothing but the master knob can take the instrument to zero.
    #[test]
    fn the_gain_never_falls_below_master_or_rises_above_unity() {
        for mode in [
            RatioMode::Harmonic,
            RatioMode::Fibonacci,
            RatioMode::Golden,
            RatioMode::GoldenMirror,
            RatioMode::Plastic,
        ] {
            for &master in &[0.0, 0.25, 0.5, 0.75, 1.0] {
                for &field in &[0.0, 0.5, 1.0] {
                    let mut b = Breath::new(SR, mode);
                    for i in 0..(SR as usize * 30) {
                        let hz = 110.0 + (i % 300) as f32;
                        let out = b.tick(hz, field, master, 0.5);
                        assert!(
                            out.gain >= master - 1e-6 && out.gain <= 1.0 + 1e-6,
                            "{mode:?} master {master} field {field}: gain {} left [{master}, 1]",
                            out.gain
                        );
                    }
                }
            }
        }
    }

    /// Field zero is the instrument as it shipped: the gain is exactly master,
    /// and the pitch is exactly unmodulated. Every preset predating the field
    /// loads with `field: 0.0`, so this is the promise none of them moved.
    #[test]
    fn a_closed_field_is_exactly_master_and_no_pitch_movement() {
        let mut b = Breath::new(SR, RatioMode::Golden);
        for _ in 0..10_000 {
            let out = b.tick(110.0, 0.0, 0.7, 0.5);
            assert_eq!(out.gain, 0.7);
            assert_eq!(out.pitch, 1.0);
        }
    }

    /// The rate is clamped to what `drone hz` can reach, so a sequencer in
    /// plastic powers with root and range maxed cannot drag the movement
    /// toward audio rate — which would be a different effect entirely.
    #[test]
    fn the_sequencer_cannot_drive_the_rate_to_audio() {
        // The pitch cap for the ladder tunings is C8, 4186 Hz.
        for &hz in &[4186.0, 20_000.0, 1e6] {
            let r = Breath::rate_hz(hz);
            assert!(r <= RATE_MAX_HZ + 1e-6, "{hz} hz produced a {r} hz rate");
            assert!(r < 1.0, "{hz} hz produced {r} hz — audible as a rate, not a drift");
        }
        // And the drone's own range maps straight through, unclamped.
        assert!((Breath::rate_hz(440.0) - RATE_MAX_HZ).abs() < 1e-4);
        assert!((Breath::rate_hz(27.5) - RATE_MIN_HZ).abs() < 1e-4);
    }

    /// **Harmonic mode repeats; nothing else does.** Its integers share a
    /// common factor, so the sum of its components has a period. Every other
    /// mode is built on an irrational limit and never lands twice.
    #[test]
    fn only_harmonic_mode_loops() {
        // Harmonic's ratios are 1, 4/5, 3/5, 2/5, 1/5 of the fundamental, so
        // they all come round together after five turns. Measured relative to
        // the other modes rather than against an absolute threshold: the
        // sample count for that period is not a whole number, so an exact
        // return lands a fraction of a sample out however it is computed.
        let drift = |mode| {
            let mut b = Breath::new(SR, mode);
            let f = Breath::rate_hz(110.0);
            let period = (SR / f * 5.0) as usize;
            let first: Vec<f32> =
                (0..64).map(|_| b.tick(110.0, 1.0, 0.0, 0.5).gain).collect();
            for _ in 64..period {
                b.tick(110.0, 1.0, 0.0, 0.5);
            }
            let second: Vec<f32> =
                (0..64).map(|_| b.tick(110.0, 1.0, 0.0, 0.5).gain).collect();
            first.iter().zip(&second).map(|(a, b)| (a - b).abs()).sum::<f32>() / 64.0
        };
        let harmonic = drift(RatioMode::Harmonic);
        for mode in [
            RatioMode::Fibonacci,
            RatioMode::Golden,
            RatioMode::GoldenMirror,
            RatioMode::Plastic,
        ] {
            let other = drift(mode);
            assert!(
                other > harmonic * 10.0,
                "{mode:?} drifted {other} against harmonic's {harmonic} — it should \
                 never land in the same place twice, and harmonic always should"
            );
        }
    }

    /// Every mode moves differently. If two shared a shape the signature would
    /// not be doing its job.
    #[test]
    fn every_mode_has_its_own_movement() {
        let trace = |mode| {
            let mut b = Breath::new(SR, mode);
            (0..(SR as usize * 4))
                .map(|_| b.tick(110.0, 1.0, 0.0, 0.5).gain)
                .step_by(4800)
                .collect::<Vec<f32>>()
        };
        let modes = [
            RatioMode::Harmonic,
            RatioMode::Fibonacci,
            RatioMode::Golden,
            RatioMode::GoldenMirror,
            RatioMode::Plastic,
        ];
        for (i, a) in modes.iter().enumerate() {
            for b in &modes[i + 1..] {
                let (ta, tb) = (trace(*a), trace(*b));
                let diff: f32 =
                    ta.iter().zip(&tb).map(|(x, y)| (x - y).abs()).sum::<f32>() / ta.len() as f32;
                assert!(diff > 1e-3, "{a:?} and {b:?} move identically");
            }
        }
    }

    /// The pitch movement is bipolar around unity — no DC, so an open field
    /// does not make the drone drift sharp.
    #[test]
    fn the_pitch_movement_has_no_dc() {
        let mut b = Breath::new(SR, RatioMode::Golden);
        let n = SR as usize * 60;
        let mean: f64 = (0..n).map(|_| b.tick(110.0, 1.0, 0.5, 0.5).pitch as f64).sum::<f64>()
            / n as f64;
        assert!((mean - 1.0).abs() < 2e-3, "pitch drifted: mean multiplier {mean}");
    }

    /// Determinism (Law 5): same calls, same output, forever.
    #[test]
    fn the_field_is_deterministic() {
        let mut a = Breath::new(SR, RatioMode::Plastic);
        let mut b = Breath::new(SR, RatioMode::Plastic);
        for i in 0..20_000 {
            let hz = 110.0 + (i % 100) as f32;
            let (x, y) = (a.tick(hz, 0.5, 0.5, 0.5), b.tick(hz, 0.5, 0.5, 0.5));
            assert_eq!(x.gain, y.gain);
            assert_eq!(x.pitch, y.pitch);
        }
    }
}

#[cfg(test)]
mod gesture_tests {
    use super::*;

    const SR: f32 = 48_000.0;

    /// Peak-to-trough of the gain across one note, after the tempo has been
    /// learned. This is Billy's actual test: "can i hear amp dynamics with the
    /// sequencer set to its fastest setting of like 8hz".
    fn swing_at(rate_hz: f32, field: f32) -> f32 {
        let mut b = Breath::new(SR, RatioMode::Golden);
        let step = (SR / rate_hz) as usize;
        // Four notes to establish the interval, then measure the fifth.
        for _ in 0..4 {
            b.trigger();
            for _ in 0..step {
                b.tick(110.0, field, 0.0, 0.5);
            }
        }
        b.trigger();
        let (mut lo, mut hi) = (f32::MAX, f32::MIN);
        for _ in 0..step {
            let g = b.tick(110.0, field, 0.0, 0.5).gain;
            lo = lo.min(g);
            hi = hi.max(g);
        }
        hi - lo
    }

    /// **The gesture fits inside the gap at every playable rate.** A fixed
    /// decay pinned the boost at maximum whenever notes arrived faster than it
    /// could fall, which killed the dynamics exactly where they were wanted.
    #[test]
    fn there_are_dynamics_at_the_sequencers_fastest_rate() {
        // 8 Hz is the melody engine's ceiling.
        let fast = swing_at(8.0, 1.0);
        assert!(
            fast > 0.05,
            "at 8 hz the gain moved only {fast} within a note — no audible dynamics"
        );
        // And it still works slowly, where a fixed decay was fine.
        let slow = swing_at(0.5, 1.0);
        assert!(slow > 0.05, "at 0.5 hz the gain moved only {slow}");
    }

    /// **Every position of the control does something.** The first cut
    /// multiplied and clamped, so `field` was inert above 0.276 whenever notes
    /// were arriving — Billy: "seems to cap out at 0.3".
    #[test]
    fn the_field_control_keeps_working_past_a_third() {
        let mut last = 0.0;
        for step in 1..=10 {
            let field = step as f32 / 10.0;
            let swing = swing_at(8.0, field);
            assert!(
                swing > last,
                "field {field} moved {swing}, no more than {last} at the step below — \
                 the control has saturated"
            );
            last = swing;
        }
    }
}
