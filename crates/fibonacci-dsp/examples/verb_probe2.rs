//! Forensic probe: Billy's exact failing configs from 2026-08-03, driven by
//! the real Voice at 44.1 kHz. Per-side RMS, non-finite counts, and a
//! stickiness test (extreme settings, then mild — does the Room recover?).

use fibonacci_dsp::{Patch, RatioMode, StereoVerb, VerbParams, Voice, ALGORITHMS};

#[allow(clippy::too_many_arguments)]
fn run(
    label: &str,
    mode: RatioMode,
    alg_idx: usize,
    hz: f32,
    fb: f32,
    rip: f32,
    verb1: VerbParams,
    verb2: Option<(usize, VerbParams)>,
    secs: usize,
) {
    let sr = 44_100.0f32;
    let mut patch = Patch::init(ALGORITHMS[alg_idx], mode);
    patch.feedback = fb;
    patch.rip = rip;
    patch.index = 1.0;
    let mut voice = Voice::new(sr, patch);
    voice.set_freq_hz(hz);
    let mut verb = StereoVerb::new(sr);
    verb.configure(voice.patch(), voice.compiled());
    verb.set_params(verb1);
    println!("== {label}");
    for sec in 0..secs {
        if let Some((at, p2)) = verb2 {
            if sec == at {
                verb.set_params(p2);
                println!("   [params -> mild]");
            }
        }
        let n = 44_100usize;
        let (mut al, mut ar) = (0f64, 0f64);
        let mut bad = 0usize;
        voice.render_frames(n, |_, frame| {
            let (l, r) = verb.process(frame);
            if l.is_finite() && r.is_finite() {
                al += (l * l) as f64;
                ar += (r * r) as f64;
            } else {
                bad += 1;
            }
        });
        println!(
            "  s{sec:02}  L {:.4}  R {:.4}  nonfinite {bad}",
            (al / n as f64).sqrt(),
            (ar / n as f64).sqrt()
        );
    }
}

fn main() {
    run(
        "repro #3: alg IV fibonacci, 370 Hz, op-fb .95 | mix 1 ghost .65 rt60 13.7 damp .55 haunt .8",
        RatioMode::Fibonacci,
        3,
        370.0,
        0.95,
        0.0,
        VerbParams { mix: 1.0, ghost: 0.65, rt60: 13.7, damp: 0.55, haunt: 0.8 },
        None,
        12,
    );
    run(
        "repro #4: alg IV golden, 90 Hz, op-fb .67 | mix 1 ghost 1 rt60 5.5 damp 0 haunt 1",
        RatioMode::Golden,
        3,
        90.0,
        0.67,
        0.0,
        VerbParams { mix: 1.0, ghost: 1.0, rt60: 5.5, damp: 0.0, haunt: 1.0 },
        None,
        12,
    );
    run(
        "repro #5: as #4 but op-fb 0, damp .53 (Billy: alive again)",
        RatioMode::Golden,
        3,
        90.0,
        0.0,
        0.0,
        VerbParams { mix: 1.0, ghost: 1.0, rt60: 5.5, damp: 0.53, haunt: 1.0 },
        None,
        12,
    );
    run(
        "stickiness: extreme (rip .62, everything max) 8 s, then mild — recovery?",
        RatioMode::Golden,
        3,
        90.0,
        0.9,
        0.62,
        VerbParams { mix: 1.0, ghost: 1.0, rt60: 20.0, damp: 0.0, haunt: 1.0 },
        Some((8, VerbParams { mix: 0.5, ghost: 0.4, rt60: 3.0, damp: 0.3, haunt: 0.3 })),
        16,
    );
}
