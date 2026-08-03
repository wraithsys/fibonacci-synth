//! fibonacci-app: the realtime shell around `fibonacci-dsp`.
//!
//! WASAPI output via `cpal` (shared mode, default device), MIDI note input
//! via `midir`, and a line-based control REPL. **It starts droning
//! immediately** — no envelopes, no play button; that's the instrument.
//!
//! Threading model: the audio callback owns the [`Voice`] outright. Control
//! reaches it through two SPSC lock-free ring buffers (one from the REPL,
//! one from the MIDI callback thread) carrying `Copy` events — the audio
//! thread never locks and never allocates.

use anyhow::{bail, Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use fibonacci_dsp::{
    compile, midi_to_hz, Patch, RatioMode, StereoVerb, VerbParams, Voice, ALGORITHMS, NUM_OPS,
};
use std::io::{self, BufRead, Write};

const START_HZ: f32 = 110.0; // A2

/// Control messages into the audio thread. `Copy`, so ring-buffer traffic
/// never allocates or drops on the audio side.
#[derive(Clone, Copy)]
enum Event {
    SetPatch(Patch),
    SetVerb(VerbParams),
    GlideTo(f32),
}

fn apply(voice: &mut Voice, verb: &mut StereoVerb, ev: Event) {
    match ev {
        Event::SetPatch(p) => {
            voice.set_patch(p);
            verb.configure(voice.patch(), voice.compiled());
        }
        Event::SetVerb(p) => verb.set_params(p),
        Event::GlideTo(hz) => voice.glide_to_hz(hz),
    }
}

fn mode_name(mode: RatioMode) -> &'static str {
    match mode {
        RatioMode::Harmonic => "harmonic",
        RatioMode::Fibonacci => "fibonacci",
        RatioMode::Golden => "golden",
        RatioMode::GoldenMirror => "goldenmirror",
        RatioMode::Plastic => "plastic",
    }
}

fn parse_mode(name: &str) -> Option<RatioMode> {
    RatioMode::ALL
        .into_iter()
        .find(|&m| mode_name(m).eq_ignore_ascii_case(name))
}

/// Rebuild a patch for a new algorithm/mode, carrying over everything the
/// player has set by hand (power switches, detunes, feedback, master, glide)
/// while letting `Patch::init` re-derive the depth-dependent levels.
fn rebuild(old: &Patch, algorithm_index: usize, mode: RatioMode) -> Patch {
    let mut p = Patch::init(ALGORITHMS[algorithm_index], mode);
    for (new_op, old_op) in p.ops.iter_mut().zip(old.ops.iter()) {
        new_op.enabled = old_op.enabled;
        new_op.detune_cents = old_op.detune_cents;
    }
    p.feedback = old.feedback;
    p.master_level = old.master_level;
    p.glide_seconds = old.glide_seconds;
    p
}

fn algorithm_index(patch: &Patch) -> usize {
    ALGORITHMS
        .iter()
        .position(|&a| a == patch.algorithm)
        .expect("shell only deals in shipped algorithms")
}

fn print_status(patch: &Patch, verb: &VerbParams) {
    let compiled = compile(patch.algorithm);
    println!(
        "algorithm {} (id {:04b})  mode {}  carriers {}  feedback {:.2}  index {:.2}  rip {:.2}  master {:.2}  glide {:.0} ms",
        algorithm_index(patch) + 1,
        patch.algorithm.0,
        mode_name(patch.ratio_mode),
        compiled.carrier_count,
        patch.feedback,
        patch.index,
        patch.rip,
        patch.master_level,
        patch.glide_seconds * 1000.0,
    );
    println!(
        "room: mix {:.2}  ghost {:.2}  rt60 {:.1} s  damp {:.2}  haunt {:.2}",
        verb.mix, verb.ghost, verb.rt60, verb.damp, verb.haunt
    );
    for i in 0..NUM_OPS {
        println!(
            "  op{} [{}] ratio {:.4}  level {:.3}  depth {}{}{}",
            i + 1,
            if patch.ops[i].enabled { "on " } else { "OFF" },
            patch.ops[i].ratio,
            patch.ops[i].level,
            compiled.depth[i],
            if compiled.carriers >> i & 1 == 1 { "  carrier" } else { "" },
            if compiled.feedback_op == i { "  feedback" } else { "" },
        );
    }
}

fn banner() {
    println!("BLOW YOUR PHASE OFF — droning at {START_HZ} Hz (A2), golden mode, algorithm I");
    println!("commands:");
    println!("  alg <1-5>        switch algorithm (hard restrike)");
    println!("  mode <name>      harmonic | fibonacci | golden | goldenmirror | plastic");
    println!("  op <1-5> on|off  operator power switch (off = true reset)");
    println!("  note <0-127>     glide to MIDI note   |  hz <freq>  glide to frequency");
    println!("  index <0-1>      the INDEX macro: 0 = pure sines, 1 = full madness");
    println!("  rip <0-1>        the Rip: carriers phase-modulated by their own inverted past");
    println!("  fb <0-1>         feedback   |  level <0-1>  master   |  glide <secs>");
    println!("  mix <0-1>        room wet/dry   |  ghost <0-1>  modulator bleed");
    println!("  rt60 <secs>      room decay     |  damp <0-1>   room damping");
    println!("  haunt <0-1>      the Ghost Line: phase-inverting cross-feed echoes");
    println!("  status           show the current patch and room");
    println!("  quit             stop the drone and exit");
}

/// Connect to the first MIDI input port, if any. Note-ons glide the drone to
/// the played pitch; note-offs and velocity are ignored (no envelopes).
fn connect_midi(mut tx: rtrb::Producer<Event>) -> Option<midir::MidiInputConnection<()>> {
    let input = midir::MidiInput::new("fibonacci-synth").ok()?;
    let ports = input.ports();
    let port = ports.first()?;
    let name = input.port_name(port).unwrap_or_else(|_| "unknown".into());
    match input.connect(
        port,
        "fibonacci-in",
        move |_, msg, _| {
            if msg.len() >= 3 && msg[0] & 0xF0 == 0x90 && msg[2] > 0 {
                let _ = tx.push(Event::GlideTo(midi_to_hz(msg[1])));
            }
        },
        (),
    ) {
        Ok(conn) => {
            println!("MIDI in: {name}");
            Some(conn)
        }
        Err(_) => None,
    }
}

fn main() -> Result<()> {
    let patch = Patch::init(ALGORITHMS[0], RatioMode::default());

    let (mut repl_tx, mut repl_rx) = rtrb::RingBuffer::<Event>::new(64);
    let (midi_tx, mut midi_rx) = rtrb::RingBuffer::<Event>::new(256);

    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .context("no default audio output device")?;
    let config = device
        .default_output_config()
        .context("no default output config")?;
    if config.sample_format() != cpal::SampleFormat::F32 {
        bail!(
            "default output format is {:?}, expected f32 (WASAPI shared mode)",
            config.sample_format()
        );
    }
    let sample_rate = config.sample_rate().0 as f32;
    let channels = config.channels() as usize;
    println!(
        "audio out: {} @ {} Hz, {} ch",
        device.name().unwrap_or_else(|_| "unknown".into()),
        sample_rate,
        channels
    );

    let mut voice = Voice::new(sample_rate, patch);
    voice.set_freq_hz(START_HZ);
    let mut verb = StereoVerb::new(sample_rate);
    verb.configure(voice.patch(), voice.compiled());

    let stream = device.build_output_stream(
        &config.into(),
        move |data: &mut [f32], _| {
            while let Ok(ev) = repl_rx.pop() {
                apply(&mut voice, &mut verb, ev);
            }
            while let Ok(ev) = midi_rx.pop() {
                apply(&mut voice, &mut verb, ev);
            }
            let frames = data.len() / channels;
            let verb = &mut verb;
            voice.render_frames(frames, |n, frame| {
                let (l, r) = verb.process(frame);
                let base = n * channels;
                if channels == 1 {
                    data[base] = 0.5 * (l + r);
                } else {
                    data[base] = l;
                    data[base + 1] = r;
                    for c in 2..channels {
                        data[base + c] = 0.0;
                    }
                }
            });
        },
        |err| eprintln!("audio stream error: {err}"),
        None,
    )?;
    stream.play()?;

    let _midi = connect_midi(midi_tx);
    if _midi.is_none() {
        println!("no MIDI input found — REPL control only");
    }
    banner();

    // The REPL owns shadow copies of the patch and room; every edit sends the
    // whole (Copy) state through the ring, so audio and shell can never drift.
    let mut shadow = patch;
    let mut shadow_verb = VerbParams::default();
    let stdin = io::stdin();
    print!("> ");
    io::stdout().flush().ok();
    for line in stdin.lock().lines() {
        let line = line?;
        let mut parts = line.split_whitespace();
        let mut send_patch = false;
        let mut send_verb = false;
        match (parts.next(), parts.next(), parts.next()) {
            (Some("alg"), Some(n), _) => match n.parse::<usize>() {
                Ok(k) if (1..=ALGORITHMS.len()).contains(&k) => {
                    shadow = rebuild(&shadow, k - 1, shadow.ratio_mode);
                    send_patch = true;
                }
                _ => println!("alg wants 1-{}", ALGORITHMS.len()),
            },
            (Some("mode"), Some(name), _) => match parse_mode(name) {
                Some(mode) => {
                    shadow = rebuild(&shadow, algorithm_index(&shadow), mode);
                    send_patch = true;
                }
                None => println!("modes: harmonic fibonacci golden goldenmirror plastic"),
            },
            (Some("op"), Some(n), Some(state)) => {
                match (n.parse::<usize>(), state) {
                    (Ok(k), "on") | (Ok(k), "off") if (1..=NUM_OPS).contains(&k) => {
                        shadow.ops[k - 1].enabled = state == "on";
                        send_patch = true;
                    }
                    _ => println!("usage: op <1-{NUM_OPS}> on|off"),
                }
            }
            (Some("note"), Some(n), _) => match n.parse::<u8>() {
                Ok(note) if note < 128 => {
                    let _ = repl_tx.push(Event::GlideTo(midi_to_hz(note)));
                }
                _ => println!("note wants 0-127"),
            },
            (Some("hz"), Some(f), _) => match f.parse::<f32>() {
                Ok(hz) if hz > 0.0 && hz.is_finite() => {
                    let _ = repl_tx.push(Event::GlideTo(hz));
                }
                _ => println!("hz wants a positive frequency"),
            },
            (Some("fb"), Some(v), _) => match v.parse::<f32>() {
                Ok(x) if (0.0..=1.0).contains(&x) => {
                    shadow.feedback = x;
                    send_patch = true;
                }
                _ => println!("fb wants 0-1"),
            },
            (Some("index"), Some(v), _) => match v.parse::<f32>() {
                Ok(x) if (0.0..=1.0).contains(&x) => {
                    shadow.index = x;
                    send_patch = true;
                }
                _ => println!("index wants 0-1"),
            },
            (Some("rip"), Some(v), _) => match v.parse::<f32>() {
                Ok(x) if (0.0..=1.0).contains(&x) => {
                    shadow.rip = x;
                    send_patch = true;
                }
                _ => println!("rip wants 0-1"),
            },
            (Some("mix"), Some(v), _) => match v.parse::<f32>() {
                Ok(x) if (0.0..=1.0).contains(&x) => {
                    shadow_verb.mix = x;
                    send_verb = true;
                }
                _ => println!("mix wants 0-1"),
            },
            (Some("ghost"), Some(v), _) => match v.parse::<f32>() {
                Ok(x) if (0.0..=1.0).contains(&x) => {
                    shadow_verb.ghost = x;
                    send_verb = true;
                }
                _ => println!("ghost wants 0-1"),
            },
            (Some("rt60"), Some(v), _) => match v.parse::<f32>() {
                Ok(x) if (0.05..=60.0).contains(&x) => {
                    shadow_verb.rt60 = x;
                    send_verb = true;
                }
                _ => println!("rt60 wants seconds (0.05-60)"),
            },
            (Some("damp"), Some(v), _) => match v.parse::<f32>() {
                Ok(x) if (0.0..=0.99).contains(&x) => {
                    shadow_verb.damp = x;
                    send_verb = true;
                }
                _ => println!("damp wants 0-0.99"),
            },
            (Some("haunt"), Some(v), _) => match v.parse::<f32>() {
                Ok(x) if (0.0..=1.0).contains(&x) => {
                    shadow_verb.haunt = x;
                    send_verb = true;
                }
                _ => println!("haunt wants 0-1"),
            },
            (Some("level"), Some(v), _) => match v.parse::<f32>() {
                Ok(x) if (0.0..=1.0).contains(&x) => {
                    shadow.master_level = x;
                    send_patch = true;
                }
                _ => println!("level wants 0-1"),
            },
            (Some("glide"), Some(v), _) => match v.parse::<f32>() {
                Ok(x) if (0.0..=30.0).contains(&x) => {
                    shadow.glide_seconds = x;
                    send_patch = true;
                }
                _ => println!("glide wants seconds (0-30)"),
            },
            (Some("status"), _, _) => print_status(&shadow, &shadow_verb),
            (Some("quit"), _, _) | (Some("exit"), _, _) => break,
            (None, _, _) => {}
            _ => println!("unknown command — see the banner above"),
        }
        if send_patch && repl_tx.push(Event::SetPatch(shadow)).is_err() {
            eprintln!("control ring full — command dropped");
        }
        if send_verb && repl_tx.push(Event::SetVerb(shadow_verb)).is_err() {
            eprintln!("control ring full — command dropped");
        }
        print!("> ");
        io::stdout().flush().ok();
    }

    drop(stream);
    println!("drone stopped.");
    Ok(())
}
