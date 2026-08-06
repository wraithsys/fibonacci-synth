//! Diagnostic probe for the Room: sustained tonal input, full wet, sweep of
//! rt60 values. Prints per-second RMS/peak and counts non-finite samples.
//! Reproduces the "silence past rt60 1.0" report offline.

use fibonacci_dsp::{compile, Frame, Patch, RatioMode, StereoVerb, VerbParams, ALGORITHMS};

fn main() {
    let sr = 44_100.0f32;
    for rt60 in [0.5f32, 1.0, 2.0, 5.0, 7.5, 15.0] {
        let mut verb = StereoVerb::new(sr);
        let patch = Patch::init(ALGORITHMS[0], RatioMode::Golden);
        verb.configure(&patch, &compile(patch.algorithm));
        verb.set_params(VerbParams {
            mix: 1.0,
            ghost: 0.0,
            rt60,
            damp: 0.5,
            haunt: 0.0,
        });
        let mut nonfinite = 0usize;
        print!("rt60 {rt60:>4}: rms/s =");
        for sec in 0..10 {
            let mut acc = 0f64;
            let mut peak = 0f32;
            for n in 0..44_100u32 {
                let t = (sec * 44_100 + n) as f32 / sr;
                let s = (core::f32::consts::TAU * 65.0 * t).sin() * 0.5;
                let frame = Frame {
                    ops: [s * 0.15, s * 0.2, s * 0.3, s * 0.4, s * 0.8],
                    mix: s * 0.6,
                    master: 1.0,
                    // Probes measure the room itself: no field movement.
                    field: 0.0,
                };
                let (l, r) = verb.process(&frame);
                if !l.is_finite() || !r.is_finite() {
                    nonfinite += 1;
                }
                acc += 0.5 * (l * l + r * r) as f64;
                peak = peak.max(l.abs()).max(r.abs());
            }
            print!(" {:.3}", (acc / 44_100.0).sqrt());
        }
        println!("  nonfinite = {nonfinite}");
    }
}
