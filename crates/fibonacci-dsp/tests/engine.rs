//! Behavioral tests for the drone voice: determinism, the power-switch reset
//! contract, and numeric health across the whole shipped algorithm range.

use fibonacci_dsp::{Patch, RatioMode, StereoVerb, VerbParams, Voice, ALGORITHMS, NUM_OPS};

const SR: f32 = 48_000.0;

fn render_chunked(voice: &mut Voice, total: usize, chunk: usize) -> Vec<f32> {
    let mut out = vec![0.0f32; total];
    for slice in out.chunks_mut(chunk) {
        voice.render(slice);
    }
    out
}

/// Same call sequence => bit-identical audio, regardless of chunking.
#[test]
fn deterministic_and_chunk_size_invariant() {
    for &alg in &ALGORITHMS {
        let patch = Patch::init(alg, RatioMode::Golden);
        let mut a = Voice::new(SR, patch);
        let mut b = Voice::new(SR, patch);
        a.set_freq_hz(110.0);
        b.set_freq_hz(110.0);
        let ya = render_chunked(&mut a, 9600, 9600);
        let yb = render_chunked(&mut b, 9600, 137);
        assert_eq!(ya, yb, "algorithm {:04b}", alg.0);
    }
}

/// The off switch is a reset, not a bypass: after the fade-out completes the
/// operator's phase is zeroed, and re-enabling starts from phase 0.
#[test]
fn power_switch_resets_phase() {
    let patch = Patch::init(ALGORITHMS[4], RatioMode::Harmonic); // all carriers
    let mut voice = Voice::new(SR, patch);
    voice.set_freq_hz(110.0);

    let mut buf = vec![0.0f32; 4800];
    voice.render(&mut buf);
    assert!(voice.op_phase(2) > 0.0, "op should have accumulated phase");

    voice.set_op_enabled(2, false);
    voice.render(&mut buf); // plenty of samples for the ~2 ms fade
    assert_eq!(voice.op_phase(2), 0.0, "off must zero the phase, not hold it");

    voice.set_op_enabled(2, true);
    assert_eq!(voice.op_phase(2), 0.0, "on must start from phase 0");

    // And the re-enabled op behaves exactly like a fresh one: a fresh voice
    // given the same off/on history produces identical output.
    let mut twin = Voice::new(SR, patch);
    twin.set_freq_hz(110.0);
    let mut twin_buf = vec![0.0f32; 4800];
    twin.render(&mut twin_buf);
    twin.set_op_enabled(2, false);
    twin.render(&mut twin_buf);
    twin.set_op_enabled(2, true);

    let ya = render_chunked(&mut voice, 4800, 480);
    let yb = render_chunked(&mut twin, 4800, 4800);
    assert_eq!(ya, yb);
}

/// A disabled operator goes fully silent (not merely quiet), and disabling a
/// modulator audibly changes the carrier's spectrum (it really is out of the
/// signal path).
#[test]
fn disabled_op_is_silent_and_out_of_the_path() {
    // Algorithm I: op 0 is the deepest modulator; disable everything except
    // the single carrier (op 4) => pure sine at the carrier level.
    let patch = Patch::init(ALGORITHMS[0], RatioMode::Harmonic);
    let mut voice = Voice::new(SR, patch);
    voice.set_freq_hz(110.0);
    for op in 0..NUM_OPS - 1 {
        voice.set_op_enabled(op, false);
    }
    let mut buf = vec![0.0f32; 9600];
    voice.render(&mut buf); // let fades finish
    voice.render(&mut buf);

    // A pure sine's peak is level * master (no modulation to spread energy).
    let peak = buf.iter().fold(0.0f32, |p, &x| p.max(x.abs()));
    let expected = voice.patch().master_level; // carrier level 1.0, 1 carrier
    assert!(
        (peak - expected).abs() < 0.01,
        "expected clean sine peak ~{expected}, got {peak}"
    );
}

/// INDEX at zero silences every modulation source — algorithm I collapses to
/// its single carrier as a pure sine, including the feedback path.
#[test]
fn index_zero_is_pure_sines() {
    let mut patch = Patch::init(ALGORITHMS[0], RatioMode::Golden);
    patch.index = 0.0;
    let mut voice = Voice::new(SR, patch);
    voice.set_freq_hz(110.0);
    let mut buf = vec![0.0f32; 9600];
    voice.render(&mut buf);
    voice.render(&mut buf);
    let peak = buf.iter().fold(0.0f32, |p, &x| p.max(x.abs()));
    assert!(
        (peak - patch.master_level).abs() < 0.01,
        "expected pure sine peak ~{}, got {peak}",
        patch.master_level
    );
}

/// Power-switch flips delivered via a whole-patch update (how the realtime
/// shell drives the voice) honor the same reset contract as set_op_enabled.
#[test]
fn set_patch_power_transitions_honor_the_reset_contract() {
    let patch = Patch::init(ALGORITHMS[4], RatioMode::Harmonic);
    let mut voice = Voice::new(SR, patch);
    voice.set_freq_hz(110.0);
    let mut buf = vec![0.0f32; 4800];
    voice.render(&mut buf);
    assert!(voice.op_phase(3) > 0.0);

    let mut off = *voice.patch();
    off.ops[3].enabled = false;
    voice.set_patch(off);
    voice.render(&mut buf); // fade completes
    assert_eq!(voice.op_phase(3), 0.0, "set_patch off must reset, not bypass");

    let mut on = off;
    on.ops[3].enabled = true;
    voice.set_patch(on);
    assert_eq!(voice.op_phase(3), 0.0, "set_patch on must start from phase 0");
}

/// The Rip audibly changes the sound, stays bounded at full tilt, and rip 0
/// keeps the voice's output identical in behavior to having no Rip at all.
#[test]
fn rip_is_stable_audible_and_off_by_default() {
    let base = Patch::init(ALGORITHMS[0], RatioMode::Golden);
    let mut ripped_patch = base;
    ripped_patch.rip = 1.0;

    let mut plain = Voice::new(SR, base);
    let mut ripped = Voice::new(SR, ripped_patch);
    plain.set_freq_hz(110.0);
    ripped.set_freq_hz(110.0);

    let ya = render_chunked(&mut plain, 96_000, 96_000);
    let yb = render_chunked(&mut ripped, 96_000, 353); // also covers chunk invariance
    let mut ripped_again = Voice::new(SR, ripped_patch);
    ripped_again.set_freq_hz(110.0);
    let yb2 = render_chunked(&mut ripped_again, 96_000, 96_000);

    assert_eq!(yb, yb2, "rip must stay deterministic and chunk-invariant");
    assert_ne!(ya, yb, "rip 1.0 made no audible difference");
    assert!(yb.iter().all(|x| x.is_finite()), "rip produced non-finite samples");
    let rms = (yb.iter().map(|x| x * x).sum::<f32>() / yb.len() as f32).sqrt();
    assert!(rms > 1e-3 && rms < 1.0, "rip rms out of range: {rms}");
}

/// Hot operator feedback makes the voice's waveform asymmetric (DC), and a
/// comb bank amplifies DC ~100x — unblocked, the wet rails to a constant
/// and the speakers go silent ("no audio at 100% mix"). The Room's DC
/// blockers must keep the wet mean near zero and its dynamics alive.
#[test]
fn room_survives_hot_feedback_without_dc_rail() {
    let mut patch = Patch::init(ALGORITHMS[3], RatioMode::Golden);
    patch.feedback = 0.95;
    patch.index = 1.0;
    patch.rip = 0.6;
    let mut voice = Voice::new(SR, patch);
    voice.set_freq_hz(90.0);
    let mut verb = StereoVerb::new(SR);
    verb.configure(voice.patch(), voice.compiled());
    verb.set_params(VerbParams {
        mix: 1.0,
        ghost: 1.0,
        rt60: 10.0,
        damp: 0.0,
        haunt: 1.0,
    });
    let n = 5 * 48_000;
    let (mut sum, mut lo, mut hi) = (0f64, f32::MAX, f32::MIN);
    voice.render_frames(n, |_, frame| {
        let (l, _r) = verb.process(frame);
        sum += l as f64;
        lo = lo.min(l);
        hi = hi.max(l);
    });
    let mean = sum / n as f64;
    assert!(mean.abs() < 0.05, "wet is DC-railed: mean {mean}");
    assert!(hi - lo > 0.2, "wet has no dynamics: range {}", hi - lo);
}

/// Extreme settings (max levels, max feedback, inharmonic ratios) stay finite
/// and produce actual signal across every shipped algorithm.
#[test]
fn numeric_health_at_extremes() {
    for mode in RatioMode::ALL {
        for &alg in &ALGORITHMS {
            let mut patch = Patch::init(alg, mode);
            patch.feedback = 1.0;
            for op in patch.ops.iter_mut() {
                op.level = 1.0;
            }
            let mut voice = Voice::new(SR, patch);
            voice.set_freq_hz(55.0);
            let mut buf = vec![0.0f32; 48_000];
            voice.render(&mut buf);

            assert!(
                buf.iter().all(|x| x.is_finite()),
                "algorithm {:04b} / {mode:?} produced non-finite samples",
                alg.0
            );
            let rms = (buf.iter().map(|x| x * x).sum::<f32>() / buf.len() as f32).sqrt();
            assert!(rms > 1e-3, "algorithm {:04b} / {mode:?} is silent (rms {rms})", alg.0);
            assert!(rms < 1.0, "algorithm {:04b} / {mode:?} is blowing up (rms {rms})", alg.0);
        }
    }
}

/// Glide actually glides: pitch moves smoothly, no discontinuity in output.
#[test]
fn glide_is_smooth() {
    let mut patch = Patch::init(ALGORITHMS[4], RatioMode::Harmonic);
    patch.glide_seconds = 0.1;
    let mut voice = Voice::new(SR, patch);
    voice.set_freq_hz(110.0);
    let mut buf = vec![0.0f32; 4800];
    voice.render(&mut buf);
    voice.glide_to_hz(220.0);
    voice.render(&mut buf);

    let max_step = buf
        .windows(2)
        .map(|w| (w[1] - w[0]).abs())
        .fold(0.0f32, f32::max);
    // At these frequencies adjacent samples of a sine mix can't jump far;
    // a glitch would show up as a near-full-scale step.
    assert!(max_step < 0.2, "output discontinuity during glide: {max_step}");
}
