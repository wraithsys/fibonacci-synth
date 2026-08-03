//! BLOW YOUR PHASE OFF — the 1-bit front end.
//!
//! Visual language: strict two colors (white on black), hard rectangles,
//! monospace type, and *dithering instead of brightness* — an operator
//! working hard is a node seething with stipple. Reference aesthetic:
//! 1-bit horror-RPG panels (World of Horror school).
//!
//! Diegesis rule: every line of text the program authors must be TRUE —
//! verifiable against the running engine or against real mathematics. The
//! event log reports measurements only. All *voiced* text comes from
//! `assets/voice.txt` (written by Billy, hot-reloaded, one box per blank-
//! line-separated paragraph); if the file is absent the instrument stays
//! silent.
//!
//! Centerpiece: the Monolith — a logarithmic (golden) spiral whose vertices
//! are displaced by the live stereo *side* signal (L−R). A clean drone
//! leaves the shell serene; rip and haunt tear visible cracks through it.
//! Structural phase cancellation, rendered.
//!
//! Threading: identical to the REPL shell (audio callback owns Voice +
//! StereoVerb, SPSC rings carry Copy events in), plus one ring pointed the
//! other way: the callback publishes decimated `VizFrame`s (per-op outputs
//! + post-Room stereo) that drive the tree stipple, the spiral, and the
//! scopes.

use anyhow::{bail, Context as _, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, Rounding, Sense, Stroke, Vec2};
use fibonacci_dsp::{
    compile, midi_to_hz, HoldSource, Melody, MelodyParams, Patch, RatioMode, Scale, StereoVerb,
    Tuning, VerbParams, Voice, ALGORITHMS, NUM_OPS,
};
use std::collections::VecDeque;
use std::time::Instant;

const START_HZ: f32 = 110.0;
const WHITE: Color32 = Color32::WHITE;
const BLACK: Color32 = Color32::BLACK;
/// Publish one VizFrame every N audio samples (48 kHz / 4 = 12 k/s).
const VIZ_DECIMATE: u32 = 4;

#[derive(Clone, Copy)]
enum Event {
    SetPatch(Patch),
    SetVerb(VerbParams),
    SetMelody(MelodyParams),
    GlideTo(f32),
}

#[derive(Clone, Copy, Default)]
struct VizFrame {
    ops: [f32; NUM_OPS],
    l: f32,
    r: f32,
}

fn apply(voice: &mut Voice, verb: &mut StereoVerb, melody: &mut Melody, ev: Event) {
    match ev {
        Event::SetPatch(p) => {
            voice.set_patch(p);
            verb.configure(voice.patch(), voice.compiled());
        }
        Event::SetVerb(p) => verb.set_params(p),
        Event::SetMelody(p) => melody.set_params(p),
        Event::GlideTo(hz) => voice.glide_to_hz(hz),
    }
}

fn mode_name(mode: RatioMode) -> &'static str {
    match mode {
        RatioMode::Harmonic => "harmonic",
        RatioMode::Fibonacci => "fibonacci",
        RatioMode::Golden => "golden",
        RatioMode::GoldenMirror => "mirror",
        RatioMode::Plastic => "plastic",
    }
}

const ROMAN: [&str; 5] = ["I", "II", "III", "IV", "V"];

fn rebuild(old: &Patch, algorithm_index: usize, mode: RatioMode) -> Patch {
    let mut p = Patch::init(ALGORITHMS[algorithm_index], mode);
    for (new_op, old_op) in p.ops.iter_mut().zip(old.ops.iter()) {
        new_op.enabled = old_op.enabled;
        new_op.detune_cents = old_op.detune_cents;
    }
    p.feedback = old.feedback;
    p.index = old.index;
    p.rip = old.rip;
    p.master_level = old.master_level;
    p.glide_seconds = old.glide_seconds;
    p
}

fn algorithm_index(patch: &Patch) -> usize {
    ALGORITHMS
        .iter()
        .position(|&a| a == patch.algorithm)
        .unwrap_or(0)
}

fn connect_midi(mut tx: rtrb::Producer<Event>) -> Option<midir::MidiInputConnection<()>> {
    let input = midir::MidiInput::new("blow-your-phase-off").ok()?;
    let ports = input.ports();
    let port = ports.first()?;
    input
        .connect(
            port,
            "bypo-in",
            move |_, msg, _| {
                if msg.len() >= 3 && msg[0] & 0xF0 == 0x90 && msg[2] > 0 {
                    let _ = tx.push(Event::GlideTo(midi_to_hz(msg[1])));
                }
            },
            (),
        )
        .ok()
}

struct AudioRig {
    _stream: cpal::Stream,
    _midi: Option<midir::MidiInputConnection<()>>,
    ctrl_tx: rtrb::Producer<Event>,
    viz_rx: rtrb::Consumer<VizFrame>,
    device_name: String,
    sample_rate: f32,
    midi_connected: bool,
}

fn start_audio(patch: Patch, verb_params: VerbParams) -> Result<AudioRig> {
    let (ctrl_tx, mut ctrl_rx) = rtrb::RingBuffer::<Event>::new(256);
    let (midi_tx, mut midi_rx) = rtrb::RingBuffer::<Event>::new(256);
    let (mut viz_tx, viz_rx) = rtrb::RingBuffer::<VizFrame>::new(16384);

    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .context("no default audio output device")?;
    let config = device
        .default_output_config()
        .context("no default output config")?;
    if config.sample_format() != cpal::SampleFormat::F32 {
        bail!("default output format is {:?}, expected f32", config.sample_format());
    }
    let sample_rate = config.sample_rate().0 as f32;
    let channels = config.channels() as usize;
    let device_name = device.name().unwrap_or_else(|_| "unknown".into());

    let mut voice = Voice::new(sample_rate, patch);
    voice.set_freq_hz(START_HZ);
    let mut verb = StereoVerb::new(sample_rate);
    verb.configure(voice.patch(), voice.compiled());
    verb.set_params(verb_params);
    let mut melody = Melody::new(sample_rate, MelodyParams::default());
    let mut decim: u32 = 0;

    let stream = device.build_output_stream(
        &config.into(),
        move |data: &mut [f32], _| {
            while let Ok(ev) = ctrl_rx.pop() {
                apply(&mut voice, &mut verb, &mut melody, ev);
            }
            while let Ok(ev) = midi_rx.pop() {
                apply(&mut voice, &mut verb, &mut melody, ev);
            }
            let frames = data.len() / channels;
            // Render in runs bounded by the melody's next fire, so held
            // notes land sample-accurately.
            let mut done = 0;
            while done < frames {
                if let Some(0) = melody.samples_until_fire() {
                    let hz = melody.fire();
                    voice.glide_to_hz(hz);
                }
                let run = melody
                    .samples_until_fire()
                    .map_or(frames - done, |c| c.min(frames - done))
                    .max(1);
                let base = done;
                let verb = &mut verb;
                let viz_tx = &mut viz_tx;
                let decim = &mut decim;
                voice.render_frames(run, |n, frame| {
                    let (l, r) = verb.process(frame);
                    *decim += 1;
                    if *decim >= VIZ_DECIMATE {
                        *decim = 0;
                        let _ = viz_tx.push(VizFrame { ops: frame.ops, l, r });
                    }
                    let at = (base + n) * channels;
                    if channels == 1 {
                        data[at] = 0.5 * (l + r);
                    } else {
                        data[at] = l;
                        data[at + 1] = r;
                        for c in 2..channels {
                            data[at + c] = 0.0;
                        }
                    }
                });
                melody.advance(run);
                done += run;
            }
        },
        |err| eprintln!("audio stream error: {err}"),
        None,
    )?;
    stream.play()?;
    let midi = connect_midi(midi_tx);
    let midi_connected = midi.is_some();

    Ok(AudioRig {
        _stream: stream,
        _midi: midi,
        ctrl_tx,
        viz_rx,
        device_name,
        sample_rate,
        midi_connected,
    })
}

/// 4x4 Bayer matrix, thresholds in 0..1 — the 1-bit shading primitive.
const BAYER: [[f32; 4]; 4] = [
    [0.0 / 16.0, 8.0 / 16.0, 2.0 / 16.0, 10.0 / 16.0],
    [12.0 / 16.0, 4.0 / 16.0, 14.0 / 16.0, 6.0 / 16.0],
    [3.0 / 16.0, 11.0 / 16.0, 1.0 / 16.0, 9.0 / 16.0],
    [15.0 / 16.0, 7.0 / 16.0, 13.0 / 16.0, 5.0 / 16.0],
];

/// Fill `rect` with white dither cells at the given density (0..1).
fn dither_rect(painter: &egui::Painter, rect: Rect, density: f32, cell: f32) {
    if density <= 0.0 {
        return;
    }
    let cols = (rect.width() / cell).ceil() as i32;
    let rows = (rect.height() / cell).ceil() as i32;
    for iy in 0..rows {
        for ix in 0..cols {
            if density > BAYER[(iy & 3) as usize][(ix & 3) as usize] {
                let p = rect.min + Vec2::new(ix as f32 * cell, iy as f32 * cell);
                painter.rect_filled(
                    Rect::from_min_size(p, Vec2::splat(cell - 1.0)),
                    Rounding::ZERO,
                    WHITE,
                );
            }
        }
    }
}

fn dither_circle(painter: &egui::Painter, center: Pos2, radius: f32, density: f32, cell: f32) {
    if density <= 0.0 {
        return;
    }
    let r2 = radius * radius;
    let steps = (radius * 2.0 / cell).ceil() as i32;
    for iy in 0..steps {
        for ix in 0..steps {
            let off = Vec2::new(ix as f32 * cell - radius, iy as f32 * cell - radius);
            if off.length_sq() <= r2
                && density > BAYER[(iy & 3) as usize][(ix & 3) as usize]
            {
                painter.rect_filled(
                    Rect::from_min_size(center + off, Vec2::splat(cell - 1.0)),
                    Rounding::ZERO,
                    WHITE,
                );
            }
        }
    }
}

/// Install the design-pass font if present (Billy's pick — currently Xilla).
/// Falls back to the built-in monospace per missing file or missing glyphs
/// (egui keeps the default fonts as fallbacks in the family list).
fn install_font(ctx: &egui::Context) {
    for path in [
        "crates/fibonacci-gui/assets/font.otf",
        "crates/fibonacci-gui/assets/font.ttf",
        "assets/font.otf",
        "assets/font.ttf",
    ] {
        if let Ok(bytes) = std::fs::read(path) {
            let mut fonts = egui::FontDefinitions::default();
            fonts
                .font_data
                .insert("billy".into(), egui::FontData::from_owned(bytes));
            for family in [egui::FontFamily::Monospace, egui::FontFamily::Proportional] {
                if let Some(list) = fonts.families.get_mut(&family) {
                    list.insert(0, "billy".into());
                }
            }
            ctx.set_fonts(fonts);
            return;
        }
    }
}

fn one_bit_style(ctx: &egui::Context) {
    // No anti-aliasing anywhere: the reference look is crisp deliberate
    // pixels, not smoothed edges (Billy: "they dont look aliased").
    ctx.tessellation_options_mut(|t| t.feathering = false);
    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = BLACK;
    visuals.window_fill = BLACK;
    visuals.extreme_bg_color = BLACK;
    visuals.faint_bg_color = BLACK;
    visuals.selection.bg_fill = WHITE;
    visuals.selection.stroke = Stroke::new(1.0_f32, BLACK);
    visuals.slider_trailing_fill = true;
    let white = Stroke::new(1.0_f32, WHITE);
    for w in [
        &mut visuals.widgets.noninteractive,
        &mut visuals.widgets.inactive,
        &mut visuals.widgets.hovered,
        &mut visuals.widgets.active,
        &mut visuals.widgets.open,
    ] {
        w.bg_fill = BLACK;
        w.weak_bg_fill = BLACK;
        w.bg_stroke = white;
        w.fg_stroke = white;
        w.rounding = Rounding::ZERO;
        w.expansion = 0.0;
    }
    visuals.widgets.hovered.bg_stroke = Stroke::new(2.0_f32, WHITE);
    visuals.widgets.active.bg_stroke = Stroke::new(2.0_f32, WHITE);
    let mut style = (*ctx.style()).clone();
    style.visuals = visuals;
    for (_, font) in style.text_styles.iter_mut() {
        *font = FontId::monospace(12.0);
    }
    style
        .text_styles
        .insert(egui::TextStyle::Heading, FontId::monospace(15.0));
    // The OS theme must never touch this instrument: pin the theme and
    // install the 1-bit style on BOTH theme slots, so a system light mode
    // can't swap in stock visuals (which rendered everything hardcoded
    // white as white-on-white).
    ctx.set_theme(egui::ThemePreference::Dark);
    ctx.style_mut_of(egui::Theme::Dark, |s| *s = style.clone());
    ctx.style_mut_of(egui::Theme::Light, |s| *s = style);
}

/// Everything a session is: the sound, the room, the melody, the pitch.
/// Saved as `state.json` on exit, restored on launch; the same shape is a
/// named preset when saved into the presets directory.
#[derive(Clone, Copy, serde::Serialize, serde::Deserialize)]
struct SavedState {
    patch: Patch,
    verb: VerbParams,
    melody: MelodyParams,
    drone_hz: f32,
}

/// Presets and state live beside the assets when run from the workspace,
/// or in ./presets beside a shipped binary.
fn preset_dir() -> std::path::PathBuf {
    if std::path::Path::new("crates/fibonacci-gui").is_dir() {
        "crates/fibonacci-gui/presets".into()
    } else {
        "presets".into()
    }
}

/// Clamp everything a loaded file could get wrong. A hand-edited or
/// corrupted preset must never put the engine outside its documented
/// ranges.
fn sanitize(mut s: SavedState) -> SavedState {
    if !ALGORITHMS.contains(&s.patch.algorithm) {
        s.patch = Patch::init(ALGORITHMS[0], s.patch.ratio_mode);
    }
    s.patch.feedback = s.patch.feedback.clamp(0.0, 1.0);
    s.patch.index = s.patch.index.clamp(0.0, 1.0);
    s.patch.rip = s.patch.rip.clamp(0.0, 1.0);
    s.patch.master_level = s.patch.master_level.clamp(0.0, 1.0);
    s.patch.glide_seconds = s.patch.glide_seconds.clamp(0.0, 30.0);
    for op in s.patch.ops.iter_mut() {
        op.level = op.level.clamp(0.0, 1.0);
        op.ratio = op.ratio.clamp(0.01, 64.0);
        op.detune_cents = op.detune_cents.clamp(-1200.0, 1200.0);
    }
    s.verb.mix = s.verb.mix.clamp(0.0, 1.0);
    s.verb.ghost = s.verb.ghost.clamp(0.0, 1.0);
    s.verb.rt60 = s.verb.rt60.clamp(0.05, 60.0);
    s.verb.damp = s.verb.damp.clamp(0.0, 0.99);
    s.verb.haunt = s.verb.haunt.clamp(0.0, 1.0);
    s.melody.rate_hz = s.melody.rate_hz.clamp(0.1, 8.0);
    s.melody.range_degrees = s.melody.range_degrees.clamp(1, 13);
    s.melody.root_midi = s.melody.root_midi.clamp(24, 57);
    s.drone_hz = s.drone_hz.clamp(27.5, 440.0);
    s
}

fn load_saved(path: &std::path::Path) -> Option<SavedState> {
    let text = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&text).ok().map(sanitize)
}

fn scan_presets() -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(preset_dir())
        .map(|entries| {
            entries
                .filter_map(|e| e.ok())
                .filter_map(|e| {
                    let name = e.file_name().to_string_lossy().into_owned();
                    name.strip_suffix(".json")
                        .filter(|n| *n != "state")
                        .map(String::from)
                })
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    names
}

fn load_voice_lines() -> Vec<String> {
    for path in [
        "crates/fibonacci-gui/assets/voice.txt",
        "assets/voice.txt",
    ] {
        if let Ok(text) = std::fs::read_to_string(path) {
            return text
                .split("\n\n")
                .map(str::trim)
                .filter(|s| !s.is_empty() && !s.starts_with('#'))
                .map(String::from)
                .collect();
        }
    }
    Vec::new()
}

struct App {
    rig: AudioRig,
    shadow: Patch,
    shadow_verb: VerbParams,
    shadow_melody: MelodyParams,
    start: Instant,
    // Live visual state, fed by the viz ring.
    env: [f32; NUM_OPS],
    scope: VecDeque<f32>,
    lissajous: VecDeque<(f32, f32)>,
    side: VecDeque<f32>,
    log: VecDeque<String>,
    voice_lines: Vec<String>,
    voice_index: usize,
    voice_last_advance: f64,
    voice_last_reload: f64,
    frame_count: u64,
    drone_hz: f32,
    preset_name: String,
    preset_names: Vec<String>,
    restored: bool,
}

impl App {
    fn new() -> Result<Self> {
        // Restore the last session if it exists; otherwise stock defaults.
        let restored_state = load_saved(&preset_dir().join("state.json"));
        let restored = restored_state.is_some();
        let state = restored_state.unwrap_or(SavedState {
            patch: Patch::init(ALGORITHMS[0], RatioMode::default()),
            verb: VerbParams::default(),
            melody: MelodyParams::default(),
            drone_hz: START_HZ,
        });
        let shadow = state.patch;
        let shadow_verb = state.verb;
        let rig = start_audio(shadow, shadow_verb)?;
        let mut app = App {
            rig,
            shadow,
            shadow_verb,
            shadow_melody: state.melody,
            start: Instant::now(),
            env: [0.0; NUM_OPS],
            scope: VecDeque::with_capacity(1024),
            lissajous: VecDeque::with_capacity(512),
            side: VecDeque::with_capacity(256),
            log: VecDeque::new(),
            voice_lines: load_voice_lines(),
            voice_index: 0,
            voice_last_advance: 0.0,
            voice_last_reload: 0.0,
            frame_count: 0,
            drone_hz: state.drone_hz,
            preset_name: String::new(),
            preset_names: scan_presets(),
            restored,
        };
        if app.restored {
            app.send_melody();
            let _ = app.rig.ctrl_tx.push(Event::GlideTo(app.drone_hz));
            app.push_log("previous session restored from state.json.".into());
        }
        let compiled = compile(app.shadow.algorithm);
        app.push_log(format!(
            "audio out: {} @ {} hz. drone begins at {} hz.",
            app.rig.device_name, app.rig.sample_rate, START_HZ
        ));
        app.push_log(format!(
            "algorithm I compiled. carriers: {}. max depth: {}.",
            compiled.carrier_count,
            compiled.depth.iter().max().unwrap()
        ));
        if !app.rig.midi_connected {
            app.push_log("no midi input found. pitch control by ui only.".into());
        }
        Ok(app)
    }

    fn push_log(&mut self, line: String) {
        let t = self.start.elapsed().as_secs();
        self.log
            .push_front(format!("[{:02}:{:02}:{:02}] {line}", t / 3600, t / 60 % 60, t % 60));
        self.log.truncate(64);
    }

    fn send_patch(&mut self) {
        let _ = self.rig.ctrl_tx.push(Event::SetPatch(self.shadow));
    }

    fn send_verb(&mut self) {
        let _ = self.rig.ctrl_tx.push(Event::SetVerb(self.shadow_verb));
    }

    fn send_melody(&mut self) {
        let _ = self.rig.ctrl_tx.push(Event::SetMelody(self.shadow_melody));
    }

    fn current_state(&self) -> SavedState {
        SavedState {
            patch: self.shadow,
            verb: self.shadow_verb,
            melody: self.shadow_melody,
            drone_hz: self.drone_hz,
        }
    }

    fn save_preset(&mut self, name: &str) {
        let file: String = name
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || c == '-' || c == '_' || c == ' ' {
                    c
                } else {
                    '-'
                }
            })
            .collect();
        let dir = preset_dir();
        if std::fs::create_dir_all(&dir).is_err() {
            self.push_log("preset directory could not be created.".into());
            return;
        }
        let path = dir.join(format!("{file}.json"));
        match serde_json::to_string_pretty(&self.current_state()) {
            Ok(json) => match std::fs::write(&path, json) {
                Ok(()) => {
                    self.preset_names = scan_presets();
                    self.push_log(format!("preset '{file}' written."));
                }
                Err(e) => self.push_log(format!("preset write failed: {e}.")),
            },
            Err(e) => self.push_log(format!("preset serialize failed: {e}.")),
        }
    }

    fn load_preset(&mut self, name: &str) {
        let path = preset_dir().join(format!("{name}.json"));
        match load_saved(&path) {
            Some(s) => {
                self.shadow = s.patch;
                self.shadow_verb = s.verb;
                self.shadow_melody = s.melody;
                self.drone_hz = s.drone_hz;
                self.send_patch();
                self.send_verb();
                self.send_melody();
                let _ = self.rig.ctrl_tx.push(Event::GlideTo(self.drone_hz));
                self.push_log(format!(
                    "preset '{name}' loaded. algorithm {} / {}, {:.1} hz.",
                    ROMAN[algorithm_index(&self.shadow)],
                    mode_name(self.shadow.ratio_mode),
                    self.drone_hz
                ));
            }
            None => self.push_log(format!("preset '{name}' unreadable — ignored.")),
        }
    }

    fn set_algorithm(&mut self, idx: usize) {
        if idx == algorithm_index(&self.shadow) {
            return;
        }
        self.shadow = rebuild(&self.shadow, idx, self.shadow.ratio_mode);
        self.send_patch();
        let compiled = compile(self.shadow.algorithm);
        self.push_log(format!(
            "algorithm {} compiled ({:04b}). carriers: {}. max depth: {}. all phases re-struck.",
            ROMAN[idx],
            self.shadow.algorithm.0,
            compiled.carrier_count,
            compiled.depth.iter().max().unwrap()
        ));
    }

    fn drain_viz(&mut self) {
        while let Ok(frame) = self.rig.viz_rx.pop() {
            for i in 0..NUM_OPS {
                let a = frame.ops[i].abs();
                self.env[i] = if a > self.env[i] {
                    a
                } else {
                    self.env[i] * 0.9985
                };
            }
            let mono = 0.5 * (frame.l + frame.r);
            let side = 0.5 * (frame.l - frame.r);
            if self.scope.len() >= 1024 {
                self.scope.pop_front();
            }
            self.scope.push_back(mono);
            if self.lissajous.len() >= 512 {
                self.lissajous.pop_front();
            }
            self.lissajous.push_back((frame.l, frame.r));
            if self.side.len() >= 256 {
                self.side.pop_front();
            }
            self.side.push_back(side);
        }
    }

    /// The Logalith: a logarithmic spiral, vertices displaced by the live
    /// side signal, segments broken as rip/haunt rise. Serene when the
    /// phase is coherent; cracked when it is not.
    ///
    /// Growth rate is the chambered nautilus's ≈3× per whorl — NOT φ per
    /// quarter-turn (×6.85 per whorl), which is the famous myth and also
    /// collapses every inner whorl into a dot on screen. The φ in this
    /// creature lives in its septa placement (golden-angle stations) and
    /// everywhere else in the instrument.
    fn draw_monolith(&self, painter: &egui::Painter, rect: Rect) {
        let center = rect.center();
        let max_r = rect.width().min(rect.height()) * 0.44;
        const GROWTH_PER_WHORL: f32 = 3.0;
        let tau = std::f32::consts::TAU;
        let turns = 3.25_f32;
        let theta_max = turns * tau;
        let r_at = |theta: f32| max_r * GROWTH_PER_WHORL.powf((theta - theta_max) / tau);
        let violence = (self.shadow.rip + self.shadow_verb.haunt).min(1.5);
        let n = 420;
        let mut prev: Option<Pos2> = None;
        for k in 0..=n {
            let theta = theta_max * k as f32 / n as f32;
            let side = if self.side.is_empty() {
                0.0
            } else {
                self.side[(k * 7 + self.frame_count as usize) % self.side.len()]
            };
            let jitter = side * max_r * 0.35 * (0.15 + violence);
            let rr = (r_at(theta) + jitter).max(0.0);
            let p = center + Vec2::new(theta.cos(), theta.sin()) * rr;
            if let Some(q) = prev {
                // Cracks: segments drop out as phase violence rises.
                let h = (k as u64)
                    .wrapping_mul(2654435761)
                    .wrapping_add(self.frame_count / 6) % 100;
                if (h as f32) >= violence * 38.0 {
                    painter.line_segment([q, p], Stroke::new(1.0_f32, WHITE));
                }
            }
            prev = Some(p);
        }
        // Septa: one chamber wall per operator, spanning adjacent whorls at
        // golden-angle stations, drawn as a dotted chain that thickens with
        // the operator's live level. Bounded by the shell — nothing sticks
        // out of the animal.
        let golden_angle = tau * (1.0 - 1.0 / fibonacci_dsp::PHI);
        for s in 0..NUM_OPS {
            let a = (s as f32 * golden_angle) % tau;
            let theta = a + tau; // the mid-shell whorl crossing
            let r1 = r_at(theta);
            let r2 = r_at(theta + tau).min(max_r * 0.9);
            let dir = Vec2::new(a.cos(), a.sin());
            let amp = self.env[s].min(1.0);
            let dots = 3 + (amp * 9.0) as usize;
            for d in 0..=dots {
                let t = d as f32 / dots as f32;
                let p = center + dir * (r1 + (r2 - r1) * t);
                painter.rect_filled(
                    Rect::from_center_size(p, Vec2::splat(2.0)),
                    Rounding::ZERO,
                    WHITE,
                );
            }
        }
    }

    fn draw_tree(&mut self, ui: &mut egui::Ui) {
        let (response, painter) =
            ui.allocate_painter(Vec2::new(ui.available_width(), 150.0), Sense::click());
        let rect = response.rect;
        let at = |x: f32, y: f32| Pos2::new(rect.min.x + x * rect.width(), rect.min.y + y * rect.height());
        // Node layout: combination nodes then leaves (ops 0..4).
        let nodes = [at(0.50, 0.10), at(0.30, 0.34), at(0.16, 0.58), at(0.74, 0.34)];
        let leaves = [at(0.08, 0.86), at(0.26, 0.86), at(0.44, 0.62), at(0.64, 0.64), at(0.86, 0.64)];
        let stroke = Stroke::new(1.0_f32, WHITE);
        painter.line_segment([nodes[0], nodes[1]], stroke);
        painter.line_segment([nodes[0], nodes[3]], stroke);
        painter.line_segment([nodes[1], nodes[2]], stroke);
        painter.line_segment([nodes[1], leaves[2]], stroke);
        painter.line_segment([nodes[2], leaves[0]], stroke);
        painter.line_segment([nodes[2], leaves[1]], stroke);
        painter.line_segment([nodes[3], leaves[3]], stroke);
        painter.line_segment([nodes[3], leaves[4]], stroke);

        let idx = algorithm_index(&self.shadow);
        // Which node flips to reach the neighbouring roster algorithms?
        let flip_bit = |a: usize, b: usize| -> Option<usize> {
            let x = ALGORITHMS[a].0 ^ ALGORITHMS[b].0;
            (x.count_ones() == 1).then(|| x.trailing_zeros() as usize)
        };
        let next_flip = (idx + 1 < ALGORITHMS.len()).then(|| flip_bit(idx, idx + 1)).flatten();
        let prev_flip = (idx > 0).then(|| flip_bit(idx, idx - 1)).flatten();

        let blink = (self.frame_count / 24) % 2 == 0;
        for (bit, &pos) in nodes.iter().enumerate() {
            let parallel = self.shadow.algorithm.0 >> bit & 1 == 1;
            let live = Some(bit) == next_flip || Some(bit) == prev_flip;
            let r = if live { 10.0 } else { 8.0 };
            painter.circle_filled(pos, r, BLACK);
            painter.circle_stroke(pos, r, Stroke::new(if live && blink { 2.0_f32 } else { 1.0_f32 }, WHITE));
            painter.text(
                pos,
                Align2::CENTER_CENTER,
                if parallel { "P" } else { "S" },
                FontId::monospace(11.0),
                WHITE,
            );
        }
        for (i, &pos) in leaves.iter().enumerate() {
            let r = 7.0;
            painter.circle_stroke(pos, r, stroke);
            dither_circle(&painter, pos, r - 1.5, (self.env[i] * 1.6).min(1.0), 2.0);
            painter.text(
                pos + Vec2::new(0.0, 13.0),
                Align2::CENTER_CENTER,
                format!("{}", i + 1),
                FontId::monospace(10.0),
                WHITE,
            );
        }
        if response.clicked() {
            if let Some(click) = response.interact_pointer_pos() {
                for (bit, &pos) in nodes.iter().enumerate() {
                    if (click - pos).length() < 12.0 {
                        if Some(bit) == next_flip {
                            self.set_algorithm(idx + 1);
                        } else if Some(bit) == prev_flip {
                            self.set_algorithm(idx - 1);
                        } else {
                            self.push_log(
                                "node dormant. off-roster structures wake at roster size 8.".into(),
                            );
                        }
                        break;
                    }
                }
            }
        }
    }

    /// The Room's cross-feed graph: 5 ops on a circle, rotation-by-3 arcs.
    /// It is a pentagram because the mathematics says so.
    fn draw_pentagram(&self, painter: &egui::Painter, rect: Rect) {
        let center = rect.center();
        let radius = rect.width().min(rect.height()) * 0.40;
        let point = |i: usize| {
            let a = -std::f32::consts::FRAC_PI_2 + std::f32::consts::TAU * i as f32 / 5.0;
            center + Vec2::new(a.cos(), a.sin()) * radius
        };
        let haunt = self.shadow_verb.haunt;
        for i in 0..5 {
            let from = point(i);
            let to = point((i + 3) % 5);
            if haunt > 0.0 {
                painter.line_segment([from, to], Stroke::new(1.0_f32, WHITE));
            } else {
                // Dormant: dotted.
                let steps = 14;
                for s in 0..steps {
                    if s % 2 == 0 {
                        let a = from + (to - from) * (s as f32 / steps as f32);
                        let b = from + (to - from) * ((s as f32 + 0.6) / steps as f32);
                        painter.line_segment([a, b], Stroke::new(1.0_f32, WHITE));
                    }
                }
            }
        }
        for i in 0..5 {
            let p = point(i);
            painter.circle_filled(p, 7.0, BLACK);
            painter.circle_stroke(p, 7.0, Stroke::new(1.0_f32, WHITE));
            dither_circle(painter, p, 5.5, (self.env[i] * haunt * 2.0).min(1.0), 2.0);
            painter.text(
                p,
                Align2::CENTER_CENTER,
                format!("{}", i + 1),
                FontId::monospace(10.0),
                WHITE,
            );
        }
    }

    fn draw_scopes(&self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let (resp, painter) = ui.allocate_painter(
                Vec2::new(ui.available_width() - 130.0, 72.0),
                Sense::hover(),
            );
            let rect = resp.rect;
            painter.rect_stroke(rect, Rounding::ZERO, Stroke::new(2.0_f32, WHITE));
            let inner = rect.shrink(4.0);
            let painter = painter.with_clip_rect(inner);
            if self.scope.len() > 2 {
                let n = self.scope.len();
                let mut prev: Option<Pos2> = None;
                for (i, &s) in self.scope.iter().enumerate() {
                    let p = Pos2::new(
                        inner.min.x + inner.width() * i as f32 / n as f32,
                        inner.center().y - s.clamp(-1.0, 1.0) * inner.height() * 0.5,
                    );
                    if let Some(q) = prev {
                        painter.line_segment([q, p], Stroke::new(1.0_f32, WHITE));
                    }
                    prev = Some(p);
                }
            }
            // The phase image: a connected beam, clipped to its box.
            let (resp2, painter2) =
                ui.allocate_painter(Vec2::new(122.0, 72.0), Sense::hover());
            let rect2 = resp2.rect;
            painter2.rect_stroke(rect2, Rounding::ZERO, Stroke::new(2.0_f32, WHITE));
            let inner2 = rect2.shrink(4.0);
            let painter2 = painter2.with_clip_rect(inner2);
            let c = inner2.center();
            let scale = inner2.height() * 0.5;
            let mut prev: Option<Pos2> = None;
            for &(l, r) in self.lissajous.iter() {
                let x = (l - r) * std::f32::consts::FRAC_1_SQRT_2;
                let y = (l + r) * std::f32::consts::FRAC_1_SQRT_2;
                let p = c + Vec2::new(x, -y) * scale;
                if let Some(q) = prev {
                    painter2.line_segment([q, p], Stroke::new(1.5_f32, WHITE));
                }
                prev = Some(p);
            }
        });
    }

    /// A small bordered chip for the header strip (WoH top-bar style).
    fn header_chip(ui: &mut egui::Ui, text: &str) {
        egui::Frame::none()
            .stroke(Stroke::new(1.0_f32, WHITE))
            .inner_margin(egui::Margin::symmetric(6.0, 2.0))
            .show(ui, |ui| {
                ui.label(egui::RichText::new(text).color(WHITE));
            });
    }

    fn inverted_strip(ui: &mut egui::Ui, text: &str) {
        egui::Frame::none().fill(WHITE).inner_margin(4.0).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(egui::RichText::new(text).color(BLACK).monospace());
        });
    }
}

impl eframe::App for App {
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        let dir = preset_dir();
        let _ = std::fs::create_dir_all(&dir);
        if let Ok(json) = serde_json::to_string_pretty(&self.current_state()) {
            let _ = std::fs::write(dir.join("state.json"), json);
        }
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.frame_count += 1;
        self.drain_viz();
        ctx.request_repaint();
        let now = ctx.input(|i| i.time);

        // Hot-reload Billy's voice file every ~2 s.
        if now - self.voice_last_reload > 2.0 {
            self.voice_last_reload = now;
            let lines = load_voice_lines();
            if lines.len() != self.voice_lines.len() {
                self.voice_lines = lines;
                self.voice_index = 0;
            } else {
                self.voice_lines = lines;
            }
        }

        egui::TopBottomPanel::top("title").show(ctx, |ui| {
            ui.horizontal(|ui| {
                let t = self.start.elapsed().as_secs();
                Self::header_chip(ui, &format!("{:02}:{:02}:{:02}", t / 3600, t / 60 % 60, t % 60));
                ui.label(
                    egui::RichText::new("BLOW YOUR PHASE OFF")
                        .font(FontId::monospace(16.0))
                        .color(WHITE),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    Self::header_chip(ui, concat!("v", env!("CARGO_PKG_VERSION")));
                    Self::header_chip(ui, &format!("{:.0}k", self.rig.sample_rate / 1000.0));
                    if self.rig.midi_connected {
                        Self::header_chip(ui, "midi");
                    }
                    ui.label(
                        egui::RichText::new(&self.rig.device_name)
                            .color(WHITE)
                            .font(FontId::monospace(11.0)),
                    );
                });
            });
        });

        egui::TopBottomPanel::bottom("log").min_height(150.0).show(ctx, |ui| {
            self.draw_scopes(ui);
            ui.add_space(4.0);
            // Billy's voice, if the file exists.
            if !self.voice_lines.is_empty() {
                if now - self.voice_last_advance > 45.0 {
                    self.voice_last_advance = now;
                    self.voice_index = (self.voice_index + 1) % self.voice_lines.len();
                }
                let text = self.voice_lines[self.voice_index].clone();
                let resp = egui::Frame::none()
                    .stroke(Stroke::new(1.0_f32, WHITE))
                    .inner_margin(6.0)
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.label(egui::RichText::new(text).color(WHITE));
                    })
                    .response
                    .interact(Sense::click());
                if resp.clicked() {
                    self.voice_index = (self.voice_index + 1) % self.voice_lines.len();
                    self.voice_last_advance = now;
                }
                ui.add_space(2.0);
            }
            for line in self.log.iter().take(4) {
                ui.label(egui::RichText::new(line).color(WHITE));
            }
        });

        egui::SidePanel::left("stats").exact_width(195.0).show(ctx, |ui| {
            let compiled = compile(self.shadow.algorithm);
            Self::inverted_strip(ui, "STRUCTURE");
            ui.add_space(2.0);
            ui.label(format!("carriers   {}", compiled.carrier_count));
            ui.label(format!("max depth  {}", compiled.depth.iter().max().unwrap()));
            ui.label("feedback   op 1");
            ui.label(format!("id         {:04b}", self.shadow.algorithm.0));
            ui.add_space(6.0);
            Self::inverted_strip(ui, "THE TREE");
            self.draw_tree(ui);
            ui.add_space(6.0);
            // Display composite of the two phase-violence parameters:
            // integrity = 100 * (1 - max(rip, haunt)). Documented in README.
            let integrity =
                100.0 * (1.0 - self.shadow.rip.max(self.shadow_verb.haunt));
            Self::inverted_strip(ui, "LOCAL φ INTEGRITY");
            let (resp, painter) =
                ui.allocate_painter(Vec2::new(ui.available_width(), 14.0), Sense::hover());
            painter.rect_stroke(resp.rect, Rounding::ZERO, Stroke::new(2.0_f32, WHITE));
            let fill = resp.rect.shrink(2.0);
            dither_rect(
                &painter,
                Rect::from_min_size(
                    fill.min,
                    Vec2::new(fill.width() * integrity / 100.0, fill.height()),
                ),
                0.85,
                2.0,
            );
            ui.label(format!("{integrity:>3.0}%  = 100·(1−max(rip,haunt))"));
            ui.add_space(6.0);
            Self::inverted_strip(ui, "MELODY");
            ui.style_mut().spacing.slider_width = 88.0;
            let m = &mut self.shadow_melody;
            let mut changed = false;
            let mut log_line: Option<String> = None;
            let on_label = if m.enabled { "S&H ON" } else { "S&H OFF" };
            let b = egui::Button::new(
                egui::RichText::new(on_label).color(if m.enabled { BLACK } else { WHITE }),
            )
            .fill(if m.enabled { WHITE } else { BLACK })
            .min_size(Vec2::new(70.0, 20.0));
            if ui.add(b).clicked() {
                m.enabled = !m.enabled;
                changed = true;
                log_line = Some(if m.enabled {
                    format!(
                        "sample & hold engaged: {} at {:.3} hz, range {}.",
                        m.tuning.name(),
                        m.rate_hz,
                        m.range_degrees
                    )
                } else {
                    "sample & hold released. drone holds its last pitch.".into()
                });
            }
            ui.horizontal_wrapped(|ui| {
                for tuning in Tuning::ALL {
                    let active = m.tuning == tuning;
                    let b = egui::Button::new(
                        egui::RichText::new(tuning.name())
                            .color(if active { BLACK } else { WHITE }),
                    )
                    .fill(if active { WHITE } else { BLACK });
                    if ui.add(b).clicked() && !active {
                        m.tuning = tuning;
                        changed = true;
                        log_line = Some(match tuning {
                            Tuning::Scale => "tuning: 12-tet scale bank.".into(),
                            Tuning::FibonacciHz => {
                                "tuning: fibonacci integers as hz. 987/610 = 1.61803.".into()
                            }
                            Tuning::GoldenPowers => "tuning: root × φ^k, unquantized.".into(),
                            Tuning::PlasticPowers => {
                                "tuning: root × ρ^k. step ≈ 486 cents.".into()
                            }
                            Tuning::GoldenWalk => {
                                "tuning: hz ← hz·φ^±1, reflected in range. no grid.".into()
                            }
                        });
                    }
                }
            });
            if m.tuning == Tuning::Scale {
                ui.horizontal_wrapped(|ui| {
                    for scale in Scale::ALL {
                        let active = m.scale == scale;
                        let b = egui::Button::new(
                            egui::RichText::new(scale.name())
                                .color(if active { BLACK } else { WHITE }),
                        )
                        .fill(if active { WHITE } else { BLACK });
                        if ui.add(b).clicked() && !active {
                            m.scale = scale;
                            changed = true;
                            log_line = Some(format!(
                                "scale {}: {}.",
                                scale.name(),
                                scale
                                    .intervals()
                                    .iter()
                                    .map(|i| i.to_string())
                                    .collect::<Vec<_>>()
                                    .join(" ")
                            ));
                        }
                    }
                });
            }
            changed |= ui
                .add(
                    egui::Slider::new(&mut m.rate_hz, 0.1..=8.0)
                        .logarithmic(true)
                        .text("rate hz"),
                )
                .changed();
            let mut range = m.range_degrees as i32;
            if ui.add(egui::Slider::new(&mut range, 1..=13).text("range")).changed() {
                m.range_degrees = range as u8;
                changed = true;
            }
            let mut root = m.root_midi as i32;
            if ui.add(egui::Slider::new(&mut root, 24..=57).text("root")).changed() {
                m.root_midi = root as u8;
                changed = true;
            }
            ui.horizontal(|ui| {
                for source in [HoldSource::GoldenWeyl, HoldSource::Xorshift] {
                    let active = m.source == source;
                    let name = match source {
                        HoldSource::GoldenWeyl => "golden",
                        HoldSource::Xorshift => "random",
                    };
                    let b = egui::Button::new(
                        egui::RichText::new(name).color(if active { BLACK } else { WHITE }),
                    )
                    .fill(if active { WHITE } else { BLACK });
                    if ui.add(b).clicked() && !active {
                        m.source = source;
                        changed = true;
                    }
                }
            });
            if changed {
                self.send_melody();
            }
            if let Some(line) = log_line {
                self.push_log(line);
            }
        });

        egui::SidePanel::right("ops").exact_width(250.0).show(ctx, |ui| {
            Self::inverted_strip(ui, "OPERATORS");
            ui.add_space(2.0);
            let compiled = compile(self.shadow.algorithm);
            let mut toggled: Option<usize> = None;
            for i in 0..NUM_OPS {
                ui.horizontal(|ui| {
                    let on = self.shadow.ops[i].enabled;
                    let label = if on { format!("{}", i + 1) } else { "×".into() };
                    let button = egui::Button::new(
                        egui::RichText::new(label).color(if on { BLACK } else { WHITE }),
                    )
                    .fill(if on { WHITE } else { BLACK })
                    .min_size(Vec2::new(24.0, 22.0));
                    if ui.add(button).clicked() {
                        toggled = Some(i);
                    }
                    ui.label(format!("×{:<6.3}", self.shadow.ops[i].ratio));
                    let (resp, painter) =
                        ui.allocate_painter(Vec2::new(70.0, 12.0), Sense::hover());
                    painter.rect_stroke(resp.rect, Rounding::ZERO, Stroke::new(2.0_f32, WHITE));
                    let inner = resp.rect.shrink(2.0);
                    dither_rect(
                        &painter,
                        Rect::from_min_size(
                            inner.min,
                            Vec2::new(inner.width() * (self.env[i] * 1.4).min(1.0), inner.height()),
                        ),
                        0.9,
                        2.0,
                    );
                    let carrier = compiled.carriers >> i & 1 == 1;
                    ui.label(if carrier {
                        "car"
                    } else if compiled.feedback_op == i {
                        "fb"
                    } else {
                        "mod"
                    });
                });
            }
            if let Some(i) = toggled {
                let on = !self.shadow.ops[i].enabled;
                self.shadow.ops[i].enabled = on;
                self.send_patch();
                self.push_log(if on {
                    format!("op {} enabled. phase starts from zero.", i + 1)
                } else {
                    format!("op {} disabled. 2 ms fade, then phase zeroed. no state retained.", i + 1)
                });
            }
            ui.add_space(6.0);
            Self::inverted_strip(ui, "RATIO MODE");
            ui.horizontal_wrapped(|ui| {
                let mut new_mode: Option<RatioMode> = None;
                for mode in RatioMode::ALL {
                    let active = self.shadow.ratio_mode == mode;
                    let b = egui::Button::new(
                        egui::RichText::new(mode_name(mode)).color(if active { BLACK } else { WHITE }),
                    )
                    .fill(if active { WHITE } else { BLACK });
                    if ui.add(b).clicked() && !active {
                        new_mode = Some(mode);
                    }
                }
                if let Some(mode) = new_mode {
                    self.shadow = rebuild(&self.shadow, algorithm_index(&self.shadow), mode);
                    self.send_patch();
                    self.push_log(format!(
                        "ratio mode {}. op ratios: {}.",
                        mode_name(mode),
                        (0..NUM_OPS)
                            .map(|i| format!("{:.3}", mode.ratio(i)))
                            .collect::<Vec<_>>()
                            .join(" ")
                    ));
                }
            });
            ui.add_space(6.0);
            Self::inverted_strip(ui, "THE ROOM");
            let (resp, painter) =
                ui.allocate_painter(Vec2::new(ui.available_width(), 110.0), Sense::hover());
            self.draw_pentagram(&painter, resp.rect);
            let v = &mut self.shadow_verb;
            let mut verb_changed = false;
            verb_changed |= ui.add(egui::Slider::new(&mut v.mix, 0.0..=1.0).text("mix")).changed();
            verb_changed |= ui.add(egui::Slider::new(&mut v.ghost, 0.0..=1.0).text("ghost")).changed();
            verb_changed |= ui.add(egui::Slider::new(&mut v.rt60, 0.05..=20.0).text("rt60 s")).changed();
            verb_changed |= ui.add(egui::Slider::new(&mut v.damp, 0.0..=0.99).text("damp")).changed();
            verb_changed |= ui.add(egui::Slider::new(&mut v.haunt, 0.0..=1.0).text("haunt")).changed();
            if verb_changed {
                self.send_verb();
            }
            ui.add_space(6.0);
            Self::inverted_strip(ui, "PRESETS");
            ui.horizontal(|ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut self.preset_name)
                        .hint_text("name")
                        .desired_width(150.0),
                );
                if ui.button("SAVE").clicked() {
                    let name = self.preset_name.trim().to_string();
                    if name.is_empty() {
                        self.push_log("a preset needs a name.".into());
                    } else {
                        self.save_preset(&name);
                    }
                }
            });
            ui.horizontal_wrapped(|ui| {
                let names = self.preset_names.clone();
                for name in names {
                    if ui.button(&name).clicked() {
                        self.load_preset(&name);
                    }
                }
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            let idx = algorithm_index(&self.shadow);
            Self::inverted_strip(
                ui,
                &format!(
                    "THE LOGALITH — ALGORITHM {} ({:04b}) — {} (DRONE)",
                    ROMAN[idx],
                    self.shadow.algorithm.0,
                    mode_name(self.shadow.ratio_mode).to_uppercase()
                ),
            );
            let spiral_h = (ui.available_height() - 150.0).max(140.0);
            let (resp, painter) =
                ui.allocate_painter(Vec2::new(ui.available_width(), spiral_h), Sense::hover());
            painter.rect_stroke(resp.rect, Rounding::ZERO, Stroke::new(2.0_f32, WHITE));
            self.draw_monolith(&painter, resp.rect);

            ui.horizontal(|ui| {
                let mut clicked: Option<usize> = None;
                for (i, roman) in ROMAN.iter().enumerate() {
                    let active = i == idx;
                    let b = egui::Button::new(
                        egui::RichText::new(*roman).color(if active { BLACK } else { WHITE }),
                    )
                    .fill(if active { WHITE } else { BLACK })
                    .min_size(Vec2::new(30.0, 20.0));
                    if ui.add(b).clicked() && !active {
                        clicked = Some(i);
                    }
                }
                if let Some(i) = clicked {
                    self.set_algorithm(i);
                }
                ui.label("the unfolding");
            });

            let mut patch_changed = false;
            let mut log_line: Option<String> = None;
            let r = ui.add(egui::Slider::new(&mut self.shadow.index, 0.0..=1.0).text("INDEX"));
            patch_changed |= r.changed();
            if r.drag_stopped() {
                log_line = Some(format!(
                    "index {:.2}. modulator response x^(φ^(depth−2)).",
                    self.shadow.index
                ));
            }
            let r = ui.add(egui::Slider::new(&mut self.shadow.rip, 0.0..=1.0).text("RIP"));
            patch_changed |= r.changed();
            if r.drag_stopped() {
                log_line = Some(format!(
                    "rip {:.2}. carriers modulated by their own past: 46.9 ms, 36°/pass.",
                    self.shadow.rip
                ));
            }
            ui.horizontal(|ui| {
                patch_changed |= ui
                    .add(egui::Slider::new(&mut self.shadow.feedback, 0.0..=1.0).text("fb"))
                    .changed();
                patch_changed |= ui
                    .add(
                        egui::Slider::new(&mut self.shadow.master_level, 0.0..=1.0).text("master"),
                    )
                    .changed();
            });
            ui.horizontal(|ui| {
                patch_changed |= ui
                    .add(
                        egui::Slider::new(&mut self.shadow.glide_seconds, 0.0..=2.0)
                            .text("glide s"),
                    )
                    .changed();
                let resp = ui.add(
                    egui::Slider::new(&mut self.drone_hz, 27.5..=440.0)
                        .logarithmic(true)
                        .text("drone hz"),
                );
                if resp.changed() {
                    let _ = self.rig.ctrl_tx.push(Event::GlideTo(self.drone_hz));
                }
            });
            if patch_changed {
                self.send_patch();
            }
            if let Some(line) = log_line {
                self.push_log(line);
            }
        });
    }
}

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size(egui::vec2(1180.0, 780.0))
            .with_title("BLOW YOUR PHASE OFF"),
        ..Default::default()
    };
    eframe::run_native(
        "BLOW YOUR PHASE OFF",
        options,
        Box::new(|cc| {
            install_font(&cc.egui_ctx);
            one_bit_style(&cc.egui_ctx);
            match App::new() {
                Ok(app) => Ok(Box::new(app) as Box<dyn eframe::App>),
                Err(e) => Err(format!("{e:#}").into()),
            }
        }),
    )
}
