//! Measure the Room's *actual* decay time against the commanded rt60.
//!
//! ```text
//! cargo run -p fibonacci-dsp --example rt60_probe
//! ```
//!
//! Method: charge the room with the standard probe drone (65 Hz, the same
//! per-op levels as `verb_probe.rs`) for 4 s, cut the input dead, and time
//! how long the fully-wet output takes to fall 60 dB below its level at the
//! cut — which is what rt60 *claims* to be. Reported per commanded rt60 and
//! per damp setting, because the comb-loop lowpass removes energy per pass
//! on top of the feedback factor and therefore shortens the real decay.
//!
//! Reading the table: `>` means the tail was still above −60 dB when the
//! watch window (45 s) ran out; `cap` marks commands the feedback ceiling
//! (MAX_FB = 0.985) cannot honour — beyond it the knob commands nothing.

use fibonacci_dsp::{compile, Frame, Patch, RatioMode, StereoVerb, VerbParams, ALGORITHMS};

const SR: f32 = 48_000.0;
const CHARGE_S: usize = 4;
const WATCH_S: usize = 45;
const WIN: usize = 4800; // 100 ms RMS windows

fn main() {
    let patch = Patch::init(ALGORITHMS[0], RatioMode::Golden);
    let compiled = compile(patch.algorithm);
    println!("commanded rt60 vs measured T60 (s), fully wet, ghost 0.4, haunt 0:");
    println!("{:>10} | {:>8} {:>8} {:>8} {:>8}", "rt60", "damp 0", "damp .4", "damp .65", "damp .9");
    for rt60 in [0.5f32, 1.0, 2.0, 4.0, 8.0, 13.0, 20.0] {
        print!("{rt60:>10}");
        for damp in [0.0f32, 0.4, 0.65, 0.9] {
            let mut verb = StereoVerb::new(SR);
            verb.configure(&patch, &compiled);
            verb.set_params(VerbParams {
                mix: 1.0,
                ghost: 0.4,
                rt60,
                damp,
                haunt: 0.0,
            });
            // Charge.
            let mut level0 = 0.0f64;
            for n in 0..(CHARGE_S * SR as usize) {
                let t = n as f32 / SR;
                let s = (core::f32::consts::TAU * 65.0 * t).sin() * 0.5;
                let frame = Frame {
                    ops: [s * 0.15, s * 0.2, s * 0.3, s * 0.4, s * 0.8],
                    mix: s * 0.6,
                    master: 1.0,
                };
                let (l, r) = verb.process(&frame);
                level0 = level0 * (1.0 - 1e-4) + 1e-4 * (0.5 * (l * l + r * r)) as f64;
            }
            // Cut the input; time the fall to −60 dB in RMS windows.
            let silent = Frame {
                ops: [0.0; 5],
                mix: 0.0,
                master: 1.0,
            };
            let target = level0 * 1e-6; // −60 dB in power
            let mut t60: Option<f32> = None;
            'watch: for w in 0..(WATCH_S * SR as usize / WIN) {
                let mut acc = 0.0f64;
                for _ in 0..WIN {
                    let (l, r) = verb.process(&silent);
                    acc += (0.5 * (l * l + r * r)) as f64;
                }
                if acc / WIN as f64 <= target {
                    t60 = Some((w + 1) as f32 * WIN as f32 / SR);
                    break 'watch;
                }
            }
            let capped = 0.001f32.powf(0.029 / rt60) > 0.985;
            match t60 {
                Some(t) => print!(" | {t:>5.1}{}", if capped { " cap" } else { "    " }),
                None => print!(" | {:>5}{}", ">", if capped { " cap" } else { "    " }),
            }
        }
        println!();
    }
}
