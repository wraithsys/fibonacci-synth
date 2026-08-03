//! Offline audition renders, centered on the instrument's identity mode
//! (Golden): every shipped algorithm as a standalone Golden drone, an
//! "unfolding" demo per ratio mode (algorithms I..V back to back), INDEX
//! macro sweeps, and stereo renders through The Room.
//!
//! Run from the workspace root:
//!
//! ```text
//! cargo run --release --example render
//! ```
//!
//! Output lands in `renders/`.

use fibonacci_dsp::{
    Melody, MelodyParams, Patch, RatioMode, Scale, StereoVerb, Tuning, VerbParams, Voice,
    ALGORITHMS,
};

const SR: u32 = 48_000;
const DRONE_HZ: f32 = 110.0; // A2

fn mode_name(mode: RatioMode) -> &'static str {
    match mode {
        RatioMode::Harmonic => "harmonic",
        RatioMode::Fibonacci => "fibonacci",
        RatioMode::Golden => "golden",
        RatioMode::GoldenMirror => "goldenmirror",
        RatioMode::Plastic => "plastic",
    }
}

fn wav_spec(channels: u16) -> hound::WavSpec {
    hound::WavSpec {
        channels,
        sample_rate: SR,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    }
}

/// 10 ms end fade so audition playlists don't click.
fn end_fade(i: usize, n: usize) -> f32 {
    let fade = (SR / 100) as usize;
    if i + fade >= n {
        (n - i) as f32 / fade as f32
    } else {
        1.0
    }
}

fn write_wav(path: &str, samples: &[f32]) {
    let mut writer = hound::WavWriter::create(path, wav_spec(1)).expect("create wav");
    for (i, &x) in samples.iter().enumerate() {
        let v = (x * end_fade(i, samples.len())).clamp(-1.0, 1.0);
        writer.write_sample((v * i16::MAX as f32) as i16).expect("write");
    }
    writer.finalize().expect("finalize wav");
    println!("wrote {path}");
}

fn write_wav_stereo(path: &str, left: &[f32], right: &[f32]) {
    let mut writer = hound::WavWriter::create(path, wav_spec(2)).expect("create wav");
    for (i, (&l, &r)) in left.iter().zip(right.iter()).enumerate() {
        let g = end_fade(i, left.len());
        for v in [l * g, r * g] {
            writer
                .write_sample((v.clamp(-1.0, 1.0) * i16::MAX as f32) as i16)
                .expect("write");
        }
    }
    writer.finalize().expect("finalize wav");
    println!("wrote {path}");
}

/// Render algorithms I..V back to back on one voice, 2 s each.
fn render_unfold(mode: RatioMode) -> Vec<f32> {
    let mut voice = Voice::new(SR as f32, Patch::init(ALGORITHMS[0], mode));
    voice.set_freq_hz(DRONE_HZ);
    let seg = (SR * 2) as usize;
    let mut buf = vec![0.0f32; seg * ALGORITHMS.len()];
    for (i, &alg) in ALGORITHMS.iter().enumerate() {
        voice.set_patch(Patch::init(alg, mode));
        voice.render(&mut buf[i * seg..(i + 1) * seg]);
    }
    buf
}

/// Same unfold, but heard through The Room.
fn render_unfold_verb(mode: RatioMode) -> (Vec<f32>, Vec<f32>) {
    let mut voice = Voice::new(SR as f32, Patch::init(ALGORITHMS[0], mode));
    voice.set_freq_hz(DRONE_HZ);
    let mut verb = StereoVerb::new(SR as f32);
    let seg = (SR * 2) as usize;
    let total = seg * ALGORITHMS.len();
    let mut left = vec![0.0f32; total];
    let mut right = vec![0.0f32; total];
    for (i, &alg) in ALGORITHMS.iter().enumerate() {
        voice.set_patch(Patch::init(alg, mode));
        verb.configure(voice.patch(), voice.compiled());
        let (l, r) = (&mut left[i * seg..(i + 1) * seg], &mut right[i * seg..(i + 1) * seg]);
        voice.render_frames(seg, |n, frame| {
            let (wl, wr) = verb.process(frame);
            l[n] = wl;
            r[n] = wr;
        });
    }
    (left, right)
}

/// Algorithm I, INDEX swept 0 → 1 over 8 s: the tree blooming top-down.
fn render_index_sweep(through_verb: bool) -> (Vec<f32>, Vec<f32>) {
    let mut voice = Voice::new(SR as f32, Patch::init(ALGORITHMS[0], RatioMode::Golden));
    voice.set_freq_hz(DRONE_HZ);
    let mut verb = StereoVerb::new(SR as f32);
    verb.configure(voice.patch(), voice.compiled());
    let total = (SR * 8) as usize;
    let block = 480; // update the macro every 10 ms
    let mut left = vec![0.0f32; total];
    let mut right = vec![0.0f32; total];
    let mut done = 0;
    while done < total {
        let n = block.min(total - done);
        voice.patch_mut().index = done as f32 / total as f32;
        let (l, r) = (&mut left[done..done + n], &mut right[done..done + n]);
        voice.render_frames(n, |k, frame| {
            if through_verb {
                let (wl, wr) = verb.process(frame);
                l[k] = wl;
                r[k] = wr;
            } else {
                l[k] = frame.mix;
                r[k] = frame.mix;
            }
        });
        done += n;
    }
    (left, right)
}

fn main() {
    std::fs::create_dir_all("renders").expect("create renders dir");

    // The identity mode, one 4-second drone per algorithm.
    for (i, &alg) in ALGORITHMS.iter().enumerate() {
        let mut voice = Voice::new(SR as f32, Patch::init(alg, RatioMode::Golden));
        voice.set_freq_hz(DRONE_HZ);
        let mut buf = vec![0.0f32; (SR * 4) as usize];
        voice.render(&mut buf);
        write_wav(&format!("renders/alg{}_golden.wav", i + 1), &buf);
    }

    // The unfolding, once per ratio mode, dry.
    for mode in RatioMode::ALL {
        let buf = render_unfold(mode);
        write_wav(&format!("renders/unfold_{}.wav", mode_name(mode)), &buf);
    }

    // The unfolding through The Room (stereo), for the identity family.
    for mode in [RatioMode::Golden, RatioMode::GoldenMirror, RatioMode::Plastic] {
        let (l, r) = render_unfold_verb(mode);
        write_wav_stereo(&format!("renders/unfold_{}_verb.wav", mode_name(mode)), &l, &r);
    }

    // INDEX sweeps: dry mono and through The Room.
    let (l, _) = render_index_sweep(false);
    write_wav("renders/index_sweep_golden.wav", &l);
    let (l, r) = render_index_sweep(true);
    write_wav_stereo("renders/index_sweep_golden_verb.wav", &l, &r);

    // The Rip: dry static drone, carriers chewing on their own inverted
    // past — the worble needs no reverb to exist.
    {
        let mut patch = Patch::init(ALGORITHMS[0], RatioMode::Golden);
        patch.rip = 0.8;
        let mut voice = Voice::new(SR as f32, patch);
        voice.set_freq_hz(DRONE_HZ);
        let mut buf = vec![0.0f32; (SR * 12) as usize];
        voice.render(&mut buf);
        write_wav("renders/drone_golden_rip.wav", &buf);
    }

    // The Ghost Line: a long static drone so the π/5-per-pass phase drift
    // has time to breathe against the living tone, then the unfold.
    let haunted = VerbParams {
        haunt: 0.7,
        ..VerbParams::default()
    };
    {
        let mut voice = Voice::new(SR as f32, Patch::init(ALGORITHMS[0], RatioMode::Golden));
        voice.set_freq_hz(DRONE_HZ);
        let mut verb = StereoVerb::new(SR as f32);
        verb.configure(voice.patch(), voice.compiled());
        verb.set_params(haunted);
        let total = (SR * 12) as usize;
        let mut left = vec![0.0f32; total];
        let mut right = vec![0.0f32; total];
        voice.render_frames(total, |n, frame| {
            let (l, r) = verb.process(frame);
            left[n] = l;
            right[n] = r;
        });
        write_wav_stereo("renders/drone_golden_haunt.wav", &left, &right);
    }
    {
        let mut voice = Voice::new(SR as f32, Patch::init(ALGORITHMS[0], RatioMode::Golden));
        voice.set_freq_hz(DRONE_HZ);
        let mut verb = StereoVerb::new(SR as f32);
        verb.set_params(haunted);
        let seg = (SR * 2) as usize;
        let total = seg * ALGORITHMS.len();
        let mut left = vec![0.0f32; total];
        let mut right = vec![0.0f32; total];
        for (i, &alg) in ALGORITHMS.iter().enumerate() {
            voice.set_patch(Patch::init(alg, RatioMode::Golden));
            verb.configure(voice.patch(), voice.compiled());
            let (l, r) = (&mut left[i * seg..(i + 1) * seg], &mut right[i * seg..(i + 1) * seg]);
            voice.render_frames(seg, |n, frame| {
                let (wl, wr) = verb.process(frame);
                l[n] = wl;
                r[n] = wr;
            });
        }
        write_wav_stereo("renders/unfold_golden_haunt.wav", &left, &right);
    }

    // The melodic engine: algorithm II golden through the Room, the golden
    // sample-and-hold walking two of the new tunings.
    for (tuning, scale, name) in [
        (Tuning::Scale, Scale::Phyllotaxis, "melody_phyllotaxis"),
        (Tuning::GoldenWalk, Scale::Phrygian, "melody_phiwalk"),
    ] {
        let mut voice = Voice::new(SR as f32, Patch::init(ALGORITHMS[1], RatioMode::Golden));
        voice.set_freq_hz(DRONE_HZ);
        let mut verb = StereoVerb::new(SR as f32);
        verb.configure(voice.patch(), voice.compiled());
        let mut melody = Melody::new(
            SR as f32,
            MelodyParams {
                enabled: true,
                tuning,
                scale,
                ..Default::default()
            },
        );
        let total = (SR * 15) as usize;
        let mut left = vec![0.0f32; total];
        let mut right = vec![0.0f32; total];
        let mut done = 0;
        while done < total {
            if let Some(0) = melody.samples_until_fire() {
                let hz = melody.fire();
                voice.glide_to_hz(hz);
            }
            let run = melody
                .samples_until_fire()
                .map_or(total - done, |c| c.min(total - done))
                .max(1);
            let (l, r) = (&mut left[done..done + run], &mut right[done..done + run]);
            voice.render_frames(run, |n, frame| {
                let (wl, wr) = verb.process(frame);
                l[n] = wl;
                r[n] = wr;
            });
            melody.advance(run);
            done += run;
        }
        let path = format!("renders/{name}.wav");
        write_wav_stereo(&path, &left, &right);
    }

    // The full séance: rip + haunt + the Room, one static drone.
    {
        let mut patch = Patch::init(ALGORITHMS[0], RatioMode::Golden);
        patch.rip = 0.7;
        let mut voice = Voice::new(SR as f32, patch);
        voice.set_freq_hz(DRONE_HZ);
        let mut verb = StereoVerb::new(SR as f32);
        verb.configure(voice.patch(), voice.compiled());
        verb.set_params(VerbParams {
            haunt: 0.7,
            ..VerbParams::default()
        });
        let total = (SR * 12) as usize;
        let mut left = vec![0.0f32; total];
        let mut right = vec![0.0f32; total];
        voice.render_frames(total, |n, frame| {
            let (l, r) = verb.process(frame);
            left[n] = l;
            right[n] = r;
        });
        write_wav_stereo("renders/drone_golden_rip_haunt.wav", &left, &right);
    }
}
