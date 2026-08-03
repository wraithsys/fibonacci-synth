//! Shared phase-rotation DSP: the first-order allpass used by both the Ghost
//! Line (in the Room, `reverb.rs`) and the Rip Line (in the voice,
//! `voice.rs`). One mechanism, two hauntings.

/// The design pitch for phase rotation: A2, the instrument's home drone.
pub(crate) const DESIGN_HZ: f32 = 110.0;
/// Phase rotation per recirculation at the design pitch: π/5 — full
/// inversion after F(5) = 5 passes, back in phase after 10.
pub(crate) const PHASE_PER_PASS: f32 = core::f32::consts::PI / 5.0;

/// Phase response of the first-order allpass `H(z) = (a + z⁻¹)/(1 + a·z⁻¹)`
/// at angular frequency `w`.
pub(crate) fn allpass_phase(a: f32, w: f32) -> f32 {
    let (s, c) = w.sin_cos();
    let num = (-s).atan2(a + c);
    let den = (-a * s).atan2(1.0 + a * c);
    num - den
}

/// Solve (by bisection) for the allpass coefficient that rotates phase by
/// exactly `-target` radians at frequency `hz`. Runs once at construction.
pub(crate) fn allpass_coeff_for(hz: f32, sample_rate: f32, target: f32) -> f32 {
    let w = core::f32::consts::TAU * hz / sample_rate;
    let (mut lo, mut hi) = (-0.99f32, 0.999f32); // phase(lo) ≪ -target < phase(hi)
    for _ in 0..48 {
        let mid = 0.5 * (lo + hi);
        if allpass_phase(mid, w) < -target {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    0.5 * (lo + hi)
}

/// First-order allpass state: `y[n] = a·x[n] + x[n−1] − a·y[n−1]`.
pub(crate) struct PhaseRotator {
    a: f32,
    x1: f32,
    y1: f32,
}

impl PhaseRotator {
    pub(crate) fn new(a: f32) -> Self {
        PhaseRotator { a, x1: 0.0, y1: 0.0 }
    }

    pub(crate) fn process(&mut self, x: f32) -> f32 {
        let y = self.a * x + self.x1 - self.a * self.y1;
        self.x1 = x;
        self.y1 = y;
        y
    }

    pub(crate) fn clear(&mut self) {
        self.x1 = 0.0;
        self.y1 = 0.0;
    }
}
