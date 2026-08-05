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
//! Centerpiece: quartered at the golden section into four hard-framed cells
//! (Billy's 2026-08-04 annotation). Top row — the performance controls, and
//! the Logalith: a logarithmic spiral whose vertices are displaced by the
//! live stereo *side* signal (L−R), stamped into a coarse pixel raster so
//! its edges staircase like the reference pixel art. A clean drone leaves
//! the shell serene; rip and haunt tear visible cracks through it.
//! Structural phase cancellation, rendered. Bottom row — the portrait
//! plinth, and the voice, which types itself out.
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
    compile, master_gain, midi_to_hz, HoldSource, Melody, MelodyParams, Patch, RatioMode, Scale,
    StereoVerb, Tuning, VerbParams, Voice, ALGORITHMS, NUM_OPS, PHI,
};
use std::collections::VecDeque;
use std::time::{Instant, SystemTime};

const START_HZ: f32 = 110.0;
const WHITE: Color32 = Color32::WHITE;
const BLACK: Color32 = Color32::BLACK;
/// Publish one VizFrame every N audio samples (48 kHz / 4 = 12 k/s).
const VIZ_DECIMATE: u32 = 4;

// ── The quartered centerpiece ──────────────────────────────────────────────
// The middle is cut at the golden section in both axes: the top row (controls
// | Logalith) takes 1/φ of the height and the bottom row (portrait | voice)
// the remaining 1/φ², and the left column is the *minor* section of the width
// — floored at the width the control column actually needs, because a golden
// proportion that clips a slider is just a broken slider.
/// 1/φ ≈ 0.618 — the major section.
const GOLDEN_MAJOR: f32 = 1.0 / PHI;
/// Minimum width for the control column, in pixels.
const CONTROL_COL_MIN: f32 = 320.0;
/// Gutter between cells, in pixels. Panels stay packed; this is the hairline
/// of black that keeps two 2 px frames from reading as one 4 px frame.
const CELL_GUTTER: f32 = 5.0;
// Resize-down priority (Billy's sweep, 2026-08-05: "create priority list of
// sections to hide and set minimum"). Ornament yields before controls, in
// this order: 1. the bottom row — record card and voice — because the card
// never changes shape and a row too short would clip it into a glitch;
// 2. the Logalith's cell, once it would be a sliver; 3. nothing else — the
// controls, operators and stats survive to the window minimum (800×600).
/// The bottom row hides below this height, its own words' worth of room.
const BOTTOM_ROW_MIN_H: f32 = 220.0;
/// The Logalith's cell hides below this width.
const SHELL_CELL_MIN_W: f32 = 200.0;

// ── The Logalith: a Fibonacci shell, engraved ──────────────────────────────
// Billy, 2026-08-04, against a watercolour of a flat many-whorled snail shell:
// "more emphasis on it being a fibonacci shell, i think we need more
// granularity... running it as pixels is not the right idea".
//
// He is right, and the reason is arithmetic: a raster cell was 11 px at his
// panel, so a whorl 14 px across could not be drawn at all — the inner shell
// had to be faked as a solid core. **Strokes have no such floor.** Dropping the
// raster is what buys the granularity, and it is also cheaper: ~1,500 stroked
// segments against 8,360 per-cell tests.
//
// The structure is the real anatomy of the reference: one continuous **suture**
// (the seam the shell coils against itself along), whose successive turns *are*
// the whorl boundaries; **septa** crossing each whorl; and finer **growth ribs**
// between them. Nothing is filled — the shell is drawn, the way the reference
// is drawn.
/// Whorls, and growth per whorl. Five whorls and a doubling: 5 and 2 are both
/// Fibonacci, doubling is the simplest self-similar growth there is, and five
/// whorls is what leaves room for five Fibonacci chamber counts below.
///
/// This is no longer claiming to be a nautilus (≈3×/whorl, which only ever
/// resolved three whorls). It is still emphatically not φ per quarter-turn —
/// that is the golden-spiral myth, it grows ×6.85 a whorl, and this instrument
/// does not ship myths.
const SHELL_WHORLS: usize = 5;
const SHELL_GROWTH: f32 = 2.0;
const SHELL_TURNS: f32 = SHELL_WHORLS as f32;
/// A chamber's angular width is the scale for every sideways displacement — but
/// it cannot be, past this cap. GoldenMirror puts only 4 chambers on the outer
/// whorl, so a chamber there spans 1.57 rad (777 px) and an uncapped ripple
/// would swing a rib clean across the shell. Capped, displacement stays a
/// sensible fraction of the rib's own length whatever the chamber count.
const SHELL_SPAN_CAP: f32 = 0.35;
/// Extra growth ribs a chamber grows at full operator level, above the one it
/// always has. Engraving adds *lines* to darken, so level is line count.
const SHELL_RIB_MAX: f32 = 3.0;
/// How far across its whorl a growth rib reaches, measured from the outer
/// suture inward: `min` at silence, plus `range` at full level. Ribs stopping
/// short of the inner suture is both what the reference does and the depth
/// cue — the whorl is lit at the seam it hangs from and falls away behind.
const ENGRAVE_REACH_MIN: f32 = 0.30;
const ENGRAVE_REACH_RANGE: f32 = 0.45;
/// How far a rib leans back from its outer end to its inner one, **as a
/// fraction of its own chamber's angular width**. Measuring it against the
/// chamber rather than in absolute radians is what keeps the shape consistent
/// across whorls: a chamber on the outer whorl spans 0.185 rad and one on the
/// innermost spans 1.26, so a fixed angle leaned the outside hard and the inside
/// not at all. A growth line runs perpendicular to the shell's direction of
/// growth, which at this pitch is close to radial but not radial — straight
/// spokes read as a bicycle wheel.
const SHELL_RIB_SWEEP: f32 = 0.55;
/// Undulation along a rib, same units. The rib S-bends with both ends anchored
/// rather than merely leaning, which is what reads as grown instead of ruled.
const SHELL_RIB_WAVE: f32 = 0.22;
/// Multiplier on the *wave* — not the lean — for the growth ribs inside a
/// chamber. They are free edges where the septa are structural, so they wander
/// more (Billy: "the waviness of the lines can be quite a bit more — doubley so
/// the ones inside the whorl").
///
/// The lean stays uniform on purpose. All ribs leaning the same way stay
/// parallel and can never cross; it is only a *difference* in shape between two
/// neighbouring ribs that tangles them. Doubling the lean too put the inner ribs
/// 130 % of a chamber width off their septum — straight through the next
/// chamber. Doubling the wave alone leaves 20 px of divergence against 23 px of
/// rib spacing at full density, so they crowd their septa without crossing.
const SHELL_RIB_INNER: f32 = 2.0;
/// Radial undulation of the suture itself, as a fraction of the local radius,
/// and how many undulations per whorl (13, Fibonacci). The reference shell's
/// whorl edges are not smooth curves, and a few percent of wobble is the
/// difference between a spiral and a grown thing. Adjacent whorls sit a whole
/// radius apart at this growth rate, so this cannot make them collide.
const SUTURE_WAVE: f32 = 0.035;
/// How fast that undulation travels round the shell, rad/s.
const SUTURE_SPEED: f64 = 0.35;

// ── What the sound does to the shell ───────────────────────────────────────
// Billy's mappings, 2026-08-04. Each is the engine's own mechanic drawn, not a
// mood assigned to a number.
/// `damp` is the Room's high-frequency rolloff, so it smooths the shell's own
/// waves: undulations per whorl on the suture, and wavelengths along a rib.
/// Damping *is* the removal of high frequencies — this is the same operation in
/// another domain. The suture's two limits are 21 and 5, both Fibonacci.
const SUTURE_CYCLES_OPEN: f32 = 21.0;
const SUTURE_CYCLES_DAMPED: f32 = 5.0;
const RIPPLE_ALONG_OPEN: f32 = 3.0;
const RIPPLE_ALONG_DAMPED: f32 = 0.8;
/// `fb` is an operator re-injecting its own past, so every rib is drawn
/// alongside its own repeats: up to `ECHO_MAX` dashed copies, each stepped this
/// far round. Feedback as literal repetition rather than as an atmosphere.
const ECHO_MAX: f32 = 2.0;
const ECHO_STEP: f32 = 0.17;
/// `haunt` is the Ghost Line, whose echo rotates π/5 per pass and comes back
/// fully inverted after five. So haunt draws up to five ghost sutures, each
/// turned a further π/5, thinning with every pass the way the ghost's own loop
/// gain does (0.92 per pass). The mechanic made visible, and nothing is
/// destroyed to say it.
const GHOST_MAX: f32 = 5.0;
const GHOST_GAIN: f32 = 0.92;

// ── The ripple ─────────────────────────────────────────────────────────────
// Billy, 2026-08-04: "i meant how much they move... how much they are animated
// can be increased in amplitude quite a bit".
/// Ripple amplitude, as a fraction of the chamber's angular width. This can be
/// large where the static wave could not: it is a *travelling* wave with a long
/// wavelength around the shell, so neighbouring ribs stay nearly in phase and
/// move together instead of diverging into each other. It is also purely
/// angular — the ribs swing along their whorl, never outward — so no amount of
/// it can push the shell past the frame.
const RIPPLE_AMP: f32 = 0.60;
/// Wavelengths around the *outer* whorl. The phase gradient scales with radius,
/// so the wavelength is constant in pixels rather than in angle — which is both
/// what a wave in a medium does and what keeps neighbours in step on the tight
/// inner whorls. Held in angle instead, the innermost whorl (5 chambers, so ribs
/// 0.31 rad apart) diverged 14 px against 10 px of spacing and its ribs crossed.
/// Wavelengths *along* a rib come from `damp`.
const RIPPLE_WRAP: f32 = 2.0;
/// Phase speed, rad/s.
const RIPPLE_SPEED: f64 = 0.9;
/// What `rip` does to the ripple: **speed only**, up to 3× (Billy: "whatever rip
/// does should be happening constantly but rip changes the speed of it"). The
/// amplitude is a constant, so the shell is always writhing by the same amount
/// and rip decides how frantically. Tying amplitude to rip meant a slack Rip
/// left the creature nearly still, which is the opposite of alive.
///
/// This replaces the cracks entirely: violence changes how the shell moves, never
/// whether its lines exist.
const AGITATE_SPEED: f32 = 2.0;
/// Pixels the whole shell stutters by at critical dread, on the same clock and
/// in the same direction as the sliders that caused it — so the panel and its
/// controls shake together (Billy: "once local integrity hits 0 i want the
/// logalith jittering like the parametters do too"). 2 px rather than the
/// sliders' 1 px: the same 1 px on a 495 px shell would not register.
const SHELL_TREMBLE_PX: f32 = 2.0;
/// **The seizure.** At critical dread the ripple's phase is re-thrown every
/// frame across this range, so the wave stops travelling and the whole pattern
/// flickers between unrelated configurations — the shell convulses rather than
/// writhes.
///
/// Billy found this state by accident: while the sliders were trembling he
/// grabbed the Rip slider, whose rect was shifting a pixel a frame under his
/// cursor, so the value juddered — and because the phase was then computed as
/// `elapsed × rate`, every tiny rate change threw the phase a long way. He asked
/// for it deliberately. It is now deliberate, which also means it no longer
/// depends on how long the app has been running (see `update`), and it belongs to
/// the dread band rather than to an interaction accident.
const SHELL_SEIZE: f32 = std::f32::consts::TAU;
/// `master` sets the shell's **scale**: the creature draws itself up as the
/// instrument gets loud and settles back when it doesn't. Silence leaves it at
/// `SCALE_MIN` rather than nothing — it shrinks back, it does not vanish.
///
/// `SCALE_MAX` goes **past** the fitted radius on purpose, so a loud instrument
/// pushes the shell off the edges of its panel and gets cropped by them. That is
/// safe because containment was never the fit's job — the panel's clip is what
/// guarantees nothing draws outside the frame, and Billy's rule is that being cut
/// off is fine while overlapping is not. Note the consequence: **above 1.0 the
/// rock's containment arithmetic no longer holds** (its headroom is budgeted
/// against the fitted radius) and the clip alone holds the line. That is the
/// intended trade, not an oversight.
///
/// At 1.45, the default `master` of 0.8 lands at scale 1.27 — the shell filling
/// the panel with 7 % of its height cropped — and full master at 1.45 crops 19 %.
/// Pushing the ceiling to 2.0 would crop 31 % at the *default*, which is a
/// different picture rather than a bigger one.
///
/// Heavily smoothed on purpose (Billy: "master control the scale with a whole lot
/// of smoothing"): a 1.8 s time constant, so a slider move takes ≈5 s to settle
/// and the size reads as a slow swell rather than as a readout of the control.
/// Smoothed exponentially against real elapsed time, so the swell is the same
/// speed whatever the frame rate.
const SCALE_MIN: f32 = 0.55;
const SCALE_MAX: f32 = 1.45;
const SCALE_SMOOTH_S: f32 = 1.8;
/// **The umbilicus**, opened by INDEX — inversely, and for a real reason. INDEX 0
/// is pure sine carriers: the tree's depths are silent, so the shell has no
/// interior and its centre stands open at `UMBILICUS_MAX` of the radius. Raising
/// INDEX "blooms the tree from the shallow modulators downward" (README §8), so
/// the *deepest* operators engage last — and the deepest operators are the
/// innermost whorls. The centre closes as the tree fills in.
const UMBILICUS_MAX: f32 = 0.12;
const INDEX_SMOOTH_S: f32 = 0.6;
/// Stop drawing where a whorl is thinner than this many pixels: the limit is
/// now the *stroke*, not a grid cell, which is the whole point of the change.
const SHELL_MIN_PX: f32 = 2.5;
/// Screen-space step along the suture, in pixels. The step is taken in angle
/// scaled by radius, so the outer whorls get the segments and the inner ones
/// are not oversampled into a heap of sub-pixel slivers.
const SUTURE_STEP_PX: f32 = 3.0;
/// Fraction of the panel the silhouette fills once fitted.
const SHELL_FILL: f32 = 0.97;

// ── The Logalith's rock ────────────────────────────────────────────────────
// Billy, 2026-08-04: "logalith should rock side to side diagonally whatever and
// rotate a little side to side - very subtle."
/// Sway period, seconds. The second oscillator runs at `period / φ`, so the two
/// are incommensurable and the motion is quasi-periodic — it never repeats, the
/// same argument that makes the drone's own waveform never repeat. Nothing
/// audible has an oscillator like this; the rock is presentational only.
const ROCK_PERIOD_S: f64 = 13.0;
/// Translation amplitude, as a fraction of **the shell's own radius** — not of
/// the panel. Tied to the panel, a small shell swayed a large fraction of itself
/// (twitchy) and a large one a small fraction (inert).
///
/// Applied against **scale squared**, not scale. Billy's reasoning is the right
/// one — "if something is moving around by an amount from a distance, as it gets
/// closer that movement would become perceptually much larger" — and a strict
/// perspective projection makes that growth *linear* in apparent size. Linear was
/// the first attempt and he could not see it: ±22 px on a 717 px shell is under
/// 3 % of its own radius. Squaring is a deliberate exaggeration of a real effect,
/// which is the honest description of it.
///
/// The tilt's amplitude stays a fixed *angle*: its effect in pixels is `r × tilt`
/// and already scales with the shell, so scaling the angle too would double-count.
const ROCK_SWAY: f32 = 0.060;
/// Seconds for one full turn of the spin, at scale 1 — 377, a Fibonacci number,
/// and slowed further by scale.
const ROCK_SPIN_S: f64 = 377.0;
// Scale also stretches the rock's periods, so a bigger shell moves more slowly —
// 7.2 s at its smallest, 18.9 s at its largest (Billy: "the scale has to
// influence the speed and amount of that slow movement... to make it feel like
// it's floating in space"). Mass, essentially: a large thing adrift does not
// hurry. Applied where the phases are integrated, in `update`.
/// Rotation amplitude in radians (≈4.3°). Rotation is the load-bearing half of
/// the rock: in a coarse raster a uniform translation snaps the whole shell a
/// whole cell at a time, whereas a rotation moves the outer whorls further than
/// the inner ones, so cells flip a few at a time and the motion reads as a live
/// shimmer instead of a jump. At ≈1.5° that shimmer was one cell of edge travel
/// and read as flicker rather than movement (Billy: "a little too subtle — it
/// should be enough so you can believe it's alive"); this is ≈3 cells.
const ROCK_TILT: f32 = 0.075;
/// The tilt is two golden-related sines summed, not one: a single sine rocks
/// like a metronome, and a creature does not. The second runs at `period / φ²`
/// at a third of the weight, and the sum is renormalised so the amplitude
/// still cannot exceed `ROCK_TILT` — which is what keeps the containment
/// arithmetic below a proof rather than an estimate.
const ROCK_TILT_2: f32 = 0.34;

// ── The portrait plinth (Space Z) ──────────────────────────────────────────
// Parked (2026-08-05). The drawing side — scanline field, animated point cloud,
// the turning shell mark, and the `sig_*` snapped-stroke helpers it used — lives
// at `52c5a99` and was lifted back out of there once already, so restoring it is
// a known move. The loading side stays: grids, parser, frame sets and hot-reload
// are Billy's assets, and they are correct whether or not anything draws them.


// ── The Logalith's sky ─────────────────────────────────────────────────────
// Billy's mythology (2026-08-04): the shell is a space entity that emits
// resonant sound at targets, punishing humans for experiments that locally
// subvert the physical law its own form is bound to. So its panel is space.
// (This is also, at last, what LOCAL φ INTEGRITY has been measuring all
// along.) The stars are the whole scene — the planet and the beam were built,
// seen, and cut; see DESIGN.md.
/// Stars in the Logalith's panel. Positions come from a hash of the star's
/// index — deterministic, so it is the same sky every launch, with no RNG and
/// no allocation. Sizes are 1 px, some 2 px, and the occasional 4-point
/// sparkle: the register of the 1-bit space panels Billy referenced.
const STAR_COUNT: usize = 170;

// ── The background's motion field ──────────────────────────────────────────
// Billy, 2026-08-04: the background moves with a separate motion field — "not
// literal movement — field behaviour": drift, parallax wobble, extremely slow
// curvature, noise warping, lensing around the Logalith, edge shimmer.
//
// Every term is a pure function of (star index, time): no state, no RNG, no
// allocation, and the same sky every launch. Periods are Fibonacci seconds and
// pairwise coprime — 21·34·55·89 = 3,495,030 s ≈ 40 days before the field
// returns to its starting configuration, so it never repeats inside a session.
// This field is entirely independent of the shell's own rock, which is the
// point: two motions that do not share a clock read as a creature in a medium,
// where one shared clock reads as a single moving picture.
/// Drift of the nearest parallax layer, in panel-widths per second.
const SKY_DRIFT: f32 = 0.0045;
/// Drift bearing. The golden angle, so the field slides along no axis.
const SKY_DRIFT_ANGLE: f32 = 2.399_963_2;
/// Parallax wobble amplitude for the nearest layer, as a fraction of the panel.
const SKY_WOBBLE: f32 = 0.010;
/// Peak curvature coefficient, and its period. Positive bulges the field away
/// from centre, negative draws it in; at ±0.09 over ‖u‖² ≤ 0.5 the field
/// breathes by ≈4 % of its own width, which is invisible per frame and obvious
/// over a minute — "extremely slow curvature distortion".
const SKY_CURVE: f32 = 0.09;
const SKY_CURVE_PERIOD_S: f64 = 89.0;
/// Noise warp amplitude, as a fraction of the panel.
const SKY_WARP: f32 = 0.014;
/// Lensing: stars are pushed off the shell by `SKY_LENS · R² / d`, capped at
/// `SKY_LENS_MAX · R`. Deflection ∝ 1/d is the real lensing law, and the cap is
/// what stops the singularity at d → 0 from flinging stars across the panel.
const SKY_LENS: f32 = 0.055;
const SKY_LENS_MAX: f32 = 0.40;
/// Shimmer ramps in over this fraction of the panel at each edge, and takes a
/// star off for at most this duty. The field is calm in the middle and restless
/// at its boundary; it also hides the seam where a drifting star wraps.
const SKY_EDGE: f32 = 0.24;
const SKY_SHIMMER_DUTY: f32 = 0.34;

// ── The voice, generating ──────────────────────────────────────────────────
/// Teletype rate, characters per second, before humanising. At 30 cps a box
/// of Billy's takes 9–11 s — brisk, but close enough to reading pace that you
/// can follow it as it lands. (Was 38, which outran reading by 2×.)
const TYPE_CPS: f32 = 30.0;
/// Per-character dwell jitter, ±fraction. A metronome reads as a machine;
/// this is what makes it read as someone typing. Derived from a hash of the
/// character's index, never from a clock or an RNG — so a given box types
/// with the same rhythm every time it comes round.
const TYPE_JITTER: f32 = 0.4;
/// **Console speed**, for the entity: it does not type, it prints. Three times
/// faster, and perfectly even — no jitter, no holds at punctuation (Billy: "two
/// different type speeds — transcript speed... and console speed").
///
/// The *cadence* is a second channel saying "this is not a person", working
/// alongside the inverted type. A human hesitates at a full stop; a device does
/// not, and the absence of hesitation is what you notice.
const CONSOLE_CPS: f32 = 90.0;

// ── The header marquee ─────────────────────────────────────────────────────
/// Marquee scroll rate, pixels per second, advanced in whole pixels only.
const MARQUEE_PX_PER_SEC: f64 = 40.0;
/// How many of the most recent log lines ride the ticker.
const MARQUEE_LINES: usize = 6;
/// The ticker's face — deliberately NOT the interface's own, so the log reads
/// as a different stream from the device text beside it (Billy's sweep,
/// 2026-08-05). Pixeloid Sans shares Pixeloid Mono's 9 px grid, so it lands at
/// the same sizes, and font_probe confirms it carries the log's maths set
/// (φ ρ π Δ × · ≈ →); the glyphs it lacks (subscripts, ◦) never ride the ticker.
const MARQUEE_FACE: &str = "pixeloid_sans";

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

/// Which presets-pane control is one activation away from firing. The pane's
/// double-usage principle (Billy's sweep, 2026-08-05): a first activation
/// *selects* — the outline goes to a dotted bevel — and only repeating the
/// same activation confirms it. Nothing in the pane fires on a single press.
/// The highlighted preset is tracked separately (`preset_selected`), because
/// DELETE acts on it and must not steal its highlight while arming.
#[derive(Clone, Copy, PartialEq, Eq)]
enum PaneArm {
    None,
    Save,
    Delete,
}

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
                        data[at] = (0.5 * (l + r)).clamp(-1.0, 1.0);
                    } else {
                        data[at] = l.clamp(-1.0, 1.0);
                        data[at + 1] = r.clamp(-1.0, 1.0);
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

/// Schlappi-style connector: a chain of small circles with smoothly
/// undulating radii instead of a plain line. `active` chains are full
/// bubble trails; dormant ones decay to sparse dots.
fn bubble_chain(painter: &egui::Painter, a: Pos2, b: Pos2, active: bool) {
    let span = b - a;
    let len = span.length();
    if len < 4.0 {
        return;
    }
    let spacing = if active { 7.0 } else { 10.0 };
    let n = ((len / spacing).ceil() as usize).max(2);
    for k in 0..=n {
        let t = k as f32 / n as f32;
        let p = a + span * t;
        if active {
            let r = 1.3 + 1.4 * (0.5 + 0.5 * (k as f32 * 0.9 + len * 0.13).sin());
            painter.circle_stroke(p, r, Stroke::new(1.0_f32, WHITE));
        } else {
            painter.rect_filled(
                Rect::from_center_size(p, Vec2::splat(1.5)),
                Rounding::ZERO,
                WHITE,
            );
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

// ── The type roster ────────────────────────────────────────────────────────────
// Twelve licensed faces, each sitting beside its own `LICENSE.txt` in
// `assets/fonts/<slug>/`, with roles and credits recorded in `fonts/NOTICE.md`.
// This replaces a single unlicensed file, so the rule that replaced it is the one
// worth restating in code: **a face with no licence file never enters the folder,
// and only what is in the folder can load.**
//
// `native` is the pixel grid the face was drawn on — *measured*, by
// `examples/font_probe.rs`, not read off its name. Every coordinate in a bitmap
// face is a multiple of one step, so the GCD of its outlines is that step and
// `units_per_em / step` is the height in pixels at which the face lands whole.
// Ten of the twelve are 16 px, the two Pixeloids are 9, and PixAntiqua shares no
// common step at all, so it is free at any size. Modern DOS is 16 despite being
// named 8x16: the 8 is its width, which is the kind of thing that is only settled
// by measuring.
struct Face {
    /// Both the folder under `assets/fonts/` and the egui family name.
    slug: &'static str,
    file: &'static str,
    /// The grid it was drawn on, in pixels. `None` — no grid; size it freely.
    native: Option<f32>,
}

const FACES: &[Face] = &[
    Face { slug: "pixeloid_mono", file: "PixeloidMono.ttf", native: Some(9.0) },
    Face { slug: "pixeloid_sans", file: "PixeloidSans.ttf", native: Some(9.0) },
    Face { slug: "unifontexmono", file: "UnifontExMono.ttf", native: Some(16.0) },
    Face { slug: "modern_dos", file: "ModernDOS8x16.ttf", native: Some(16.0) },
    Face { slug: "european_teletext", file: "EuropeanTeletext.ttf", native: Some(16.0) },
    Face { slug: "pixel_operator", file: "PixelOperator.ttf", native: Some(16.0) },
    Face { slug: "better_vcr", file: "BetterVCR 25.09.ttf", native: Some(16.0) },
    Face { slug: "enter_command", file: "EnterCommand.ttf", native: Some(16.0) },
    Face { slug: "scriptorium", file: "Scriptorium.ttf", native: Some(16.0) },
    Face { slug: "cyborgsister", file: "CyborgSister.ttf", native: Some(16.0) },
    Face { slug: "alkhemikal", file: "Alkhemikal.ttf", native: Some(16.0) },
    Face { slug: "pixantiqua", file: "PixAntiqua.ttf", native: None },
];

/// The face the interface itself speaks in: chrome, labels, marginalia.
const UI_FACE: &str = "pixeloid_mono";

/// The entity's own voice, and the fallback beneath every other face. Unifont is
/// the only face here with complete coverage of φ ρ π Δ • ◦ ¤ and the subscripts,
/// and it carries most of the BMP besides — so a glyph its neighbour lacks resolves
/// to a real glyph instead of a tofu box. That it is also what the Logalith speaks
/// in is not a coincidence: the thing underneath everything is the thing underneath
/// everything.
const FALLBACK_FACE: &str = "unifontexmono";

/// The voice box's size ladder, largest first: the box is drawn at the biggest of
/// these its finished entry actually fits in.
///
/// Two rungs, because these are pixel faces and a ladder is the only way to be both
/// larger and crisp. Ten of the twelve are drawn on a 16 px grid, so **between 16
/// and 32 there is no size at all** — asking for 24 just rounds to one of them. The
/// long entries are why the smaller rung has to stay: MycelliAI's six lines at 32 px
/// wrap past twice the height of the cell, while most boxes have room to spare at
/// 32. The two Pixeloids, on their 9 px grid, get 36 and 18.
const VOICE_PX: [f32; 2] = [32.0, 16.0];

/// Who speaks in what. Keyed on the archetype in `relic_log.json` and matched as a
/// normalised prefix, so `Logalith Intrusion F` finds the entity and `MycelliAI
/// Global Forest Agent` finds Alkhemikal without either full name living here.
/// The roles are NOTICE.md's; this is that table made executable.
const VOICES: &[(&str, &str)] = &[
    ("overworked acoustic engineer", "modern_dos"),
    ("archivist historian", "pixantiqua"),
    ("government auditor", "european_teletext"),
    ("field psychologist", "pixeloid_sans"),
    ("cosmic acoustician", "pixel_operator"),
    ("survivor", "better_vcr"),
    ("junior technician", "enter_command"),
    ("shell-listening monk", "scriptorium"),
    ("future witness", "cyborgsister"),
    ("mycelliai", "alkhemikal"),
    ("logalith intrusion", FALLBACK_FACE),
];

/// Fold an archetype to the form the table is written in.
///
/// Necessary because archetypes are Billy's prose and arrive with whatever the
/// editor produced: the Monk is written with a **non-breaking hyphen** (U+2011),
/// which is indistinguishable on screen from `-` and never equal to it. A table
/// keyed on raw strings would have silently given him the fallback face and looked
/// like a font bug rather than a punctuation one.
fn normalize_archetype(s: &str) -> String {
    s.trim()
        .chars()
        .map(|c| match c {
            // The dash family, plus the minus sign, plus underscore.
            '\u{2010}'..='\u{2015}' | '\u{2212}' | '_' => '-',
            c => c.to_ascii_lowercase(),
        })
        .collect()
}

/// The face an archetype speaks in.
fn face_for_archetype(archetype: &str) -> &'static str {
    let key = normalize_archetype(archetype);
    VOICES
        .iter()
        .find(|(name, _)| key.starts_with(name))
        .map(|(_, slug)| *slug)
        // Someone Billy has written but not yet cast speaks in the interface's own
        // face — which reads as unattributed, rather than as somebody else.
        .unwrap_or(UI_FACE)
}

fn family(slug: &str) -> egui::FontFamily {
    egui::FontFamily::Name(slug.into())
}

/// A size at which a face lands whole, in **device** pixels.
///
/// egui rasterises at `size × pixels_per_point`, so the display's scaling is part
/// of this arithmetic rather than a detail to wave at: on a 125 % display, asking
/// for 16 gets 20 device pixels, and a face drawn on a 16 px grid stretched to 20
/// doubles every fifth column. Asking for 12.8 instead gets exactly 16. The nearest
/// whole multiple is what is picked, so the apparent size stays as close to the
/// request as the grid allows — a coarse grid simply cannot honour every request,
/// and pretending otherwise is what makes pixel type look muddy.
fn grid_size(want: f32, native: Option<f32>, ppp: f32) -> f32 {
    let Some(native) = native else { return want };
    let k = (want * ppp / native).round().max(1.0);
    k * native / ppp
}

fn native_of(slug: &str) -> Option<f32> {
    FACES.iter().find(|f| f.slug == slug).and_then(|f| f.native)
}

/// A size in the interface's own face, landed on its grid.
fn ui_font(ctx: &egui::Context, want: f32) -> FontId {
    FontId::new(
        grid_size(want, native_of(UI_FACE), ctx.pixels_per_point()),
        family(UI_FACE),
    )
}

fn install_fonts(ctx: &egui::Context) {
    ctx.set_fonts(font_definitions());
    // What the interface will actually be drawn at, said out loud. The display's
    // scaling decides this and a screenshot cannot show it — a 12 px request lands
    // on 9 at 100 % and on 18 at 125 %, which is the difference between the type
    // looking small and looking large for reasons that are nobody's taste.
    let ppp = ctx.pixels_per_point();
    eprintln!(
        "type: {UI_FACE} on a {} px grid, {ppp:.2} px/pt — body {:.1}, heading {:.1}, voice up to {:.1}",
        native_of(UI_FACE).unwrap_or(0.0),
        grid_size(12.0, native_of(UI_FACE), ppp),
        grid_size(15.0, native_of(UI_FACE), ppp),
        VOICE_PX[0],
    );
}

/// The roster as egui sees it: every face that is present gets its own family, with
/// the fallback beneath it and egui's own fonts beneath that.
///
/// A missing file costs its own role and nothing else. The family is registered
/// either way — an unregistered `FontFamily::Name` is a **panic at layout time**,
/// and a face failing to copy must not be able to take the instrument down. What it
/// gets instead is the fallback, which is the correct answer anyway.
///
/// Separate from `install_fonts` so the families can be checked without a window:
/// that panic is the one new way this code can kill the instrument, and it is not
/// the kind of thing to find out about on Billy's machine.
fn font_definitions() -> egui::FontDefinitions {
    let mut fonts = egui::FontDefinitions::default();
    // Whatever egui ships with, kept at the bottom of every chain so emoji and the
    // stock faces still resolve.
    let builtin: Vec<String> = fonts
        .families
        .get(&egui::FontFamily::Monospace)
        .cloned()
        .unwrap_or_default();

    let mut missing: Vec<&str> = Vec::new();
    for face in FACES {
        match asset_path(&format!("fonts/{}/{}", face.slug, face.file))
            .and_then(|p| std::fs::read(p).ok())
        {
            Some(bytes) => {
                fonts
                    .font_data
                    .insert(face.slug.to_string(), egui::FontData::from_owned(bytes));
            }
            None => missing.push(face.slug),
        }
    }
    if !missing.is_empty() {
        // A windowed binary has nowhere to put this, but a `cargo run` does, and
        // silent type substitution is exactly the bug that is hardest to see.
        eprintln!("fonts not found, falling back: {}", missing.join(", "));
    }

    let have = |slug: &str| fonts.font_data.contains_key(slug);
    let chain = |slug: &str| -> Vec<String> {
        let mut list = Vec::new();
        if have(slug) {
            list.push(slug.to_string());
        }
        if slug != FALLBACK_FACE && have(FALLBACK_FACE) {
            list.push(FALLBACK_FACE.to_string());
        }
        list.extend(builtin.iter().cloned());
        list
    };

    for face in FACES {
        fonts.families.insert(family(face.slug), chain(face.slug));
    }
    // The interface's face answers to the stock families too, so every `monospace`
    // and every proportional label already in the source lands on the roster
    // without being rewritten one call at a time.
    for stock in [egui::FontFamily::Monospace, egui::FontFamily::Proportional] {
        fonts.families.insert(stock, chain(UI_FACE));
    }
    fonts
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
    // Sizes go through the grid, because the roster is pixel type: 12 px of a face
    // drawn on a 9 px grid is nine pixels of glyph smeared across twelve, which is
    // the same law as `feathering = false` applied to letterforms instead of edges.
    // The two requests below are the sizes this instrument has always asked for;
    // what changed is that they now land somewhere the face can actually be drawn.
    for (_, font) in style.text_styles.iter_mut() {
        *font = ui_font(ctx, 12.0);
    }
    style
        .text_styles
        .insert(egui::TextStyle::Heading, ui_font(ctx, 15.0));

    // Breathing room, app-wide (Billy: "give the parameter controls some padding
    // (may be worth looking at this app wide)"). Set here rather than per-panel so
    // every control gets it, and every value is a whole number of pixels — with
    // feathering off, a half-pixel of spacing is a half-pixel of blur on whatever
    // lands next to it.
    //
    // Panels stay packed edge to edge; this is only the space *inside* them, which
    // is what was missing. Hard rectangles with cramped contents read as a bug
    // rather than as density.
    style.spacing.item_spacing = Vec2::new(7.0, 6.0);
    style.spacing.button_padding = Vec2::new(6.0, 3.0);
    style.spacing.interact_size.y = 22.0;
    style.spacing.slider_rail_height = 6.0;
    style.spacing.window_margin = egui::Margin::same(6.0);

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
    // The honest ceiling: rt60_probe measured decay saturating near 7 s, so
    // anything a preset asks for beyond 8 was never real.
    s.verb.rt60 = s.verb.rt60.clamp(0.05, 8.0);
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

/// The discouragement bands (DESIGN.md): purely presentational, with
/// hysteresis so states don't flicker at the threshold. Low enters below
/// 47.5% and exits above 52.5%; Critical enters below 12.5% and exits
/// above 17.5%.
#[derive(Clone, Copy, PartialEq)]
enum Dread {
    Normal,
    Low,
    Critical,
}

/// Time constant for the critical band's ramp, in seconds. The band itself
/// switches discretely (with hysteresis), but everything it *drives* — the
/// trembling controls, the shell's stutter, the shell's seizure — fades in and
/// out through this, so crossing the threshold is an onset rather than a switch
/// being flipped (Billy: "add smoothing to the ramp up and down to the integrity
/// triggers").
const DREAD_SMOOTH_S: f32 = 0.9;

/// Where the shell sits and how far it is turned this frame. Shared between the
/// shell and the background, so the lensing bends around the shell that is
/// actually drawn.
struct ShellPose {
    /// The silhouette's centre — what the shell turns about, and what the
    /// background lenses around.
    pivot: Pos2,
    /// The spiral's pole after sway, *before* the turn. The renderer maps
    /// screen cells back through the turn rather than forward, so it wants the
    /// unturned pole.
    pole: Pos2,
    tilt: f32,
    max_r: f32,
}

// ── Space Z: drawn portraits ────────────────────────────────────────────────
// Billy, 2026-08-04: "instead of storing all those pngs you have a bit of code
// compiled to draw it and the animated motion", and "my plan is just to find
// licensable images and dither them or something to get them into a drawable
// format for you".
//
// So the portrait is not an image the program displays — it is a **point cloud
// the program draws**. Billy dithers a licensed image to 1-bit and exports it as
// a text grid; the loader turns every inked cell into a point, and the points are
// what get animated. That is the whole reason for the format: a raster can only
// be moved in whole pixels as a block, whereas every point in a cloud can move
// independently. The same lesson as the shell — strokes and points animate,
// rasters don't.
/// A source grid wider or taller than this is rejected: the cloud is redrawn every
/// frame, so grid size is a per-frame cost.
///
/// The width was 96 until Billy's second batch arrived 98–120 wide and **nine of
/// its ten portraits were silently refused** — a rejected grid falls back to the
/// shell mark, so the art simply never appeared and nothing said why. 128 is the
/// new width, chosen off the measurement rather than the other way round: the
/// widest piece is the Logalith at 120, and the batch's point counts run 750–4,300
/// against the first batch's 2,718, so the per-frame cost the cap exists to bound
/// did not actually change.
const PORTRAIT_MAX_W: usize = 128;
const PORTRAIT_MAX_H: usize = 120;
/// How many numbered frames to look for beside the base name.
const PORTRAIT_MAX_FRAMES: usize = 64;

/// One dither of a portrait: the inked cells of a 1-bit grid, kept as points
/// because points are what can be animated individually.
struct PortraitFrame {
    w: usize,
    h: usize,
    pts: Vec<(u8, u8)>,
}

impl PortraitFrame {
    fn ink(&self) -> f32 {
        self.pts.len() as f32 / (self.w * self.h).max(1) as f32
    }
}

/// A portrait is a **set of dithers of the same image**, cycled (Billy: "try
/// putting them in some kind of order switching between them to draw at 0.5").
///
/// Frames are `<name>.txt` and `<name>-1.txt`, `<name>-2.txt` … in the portraits
/// folder — exactly what the batch converter emits. They are **ordered here by ink
/// density, not by filename**, so the sequence is a tonal ramp whatever order the
/// files happened to be written in, and played up and back down so it never jumps
/// from the densest dither to the sparsest.
struct Portrait {
    /// The base name, for detecting a change of speaker.
    key: String,
    /// Newest mtime across the frames, for hot-reload.
    stamp: Option<SystemTime>,
    frames: Vec<PortraitFrame>,
}

impl Portrait {
    /// Which frame a ping-ponged cycle is showing at time `t`, given `secs` per
    /// frame — up through the set and back down without repeating either end, so
    /// there is no seam and no double-length hold at the turning points.
    ///
    /// Unused while the plinth is parked. Kept because the frame *sets* and their
    /// ordering are the part of the portrait work most likely to survive a rethink,
    /// and this is the only piece of it with arithmetic worth not re-deriving.
    #[allow(dead_code)]
    fn frame_at(&self, t: f32, secs: f32) -> &PortraitFrame {
        let n = self.frames.len();
        if n <= 1 {
            return &self.frames[0];
        }
        let span = 2 * n - 2;
        let step = ((t / secs).floor() as i64).rem_euclid(span as i64) as usize;
        let i = if step < n { step } else { span - step };
        &self.frames[i]
    }
}

/// Parse a dithered 1-bit grid.
///
/// One line per pixel row. **Only a space is empty; every other character is ink**
/// — deliberately liberal, because image-to-ASCII converters emit all sorts (`#`,
/// `@`, `*`, `X`, `1`, `%`) and Billy should not have to match a character set.
/// Comments start with `//`, *not* `#`, since `#` is the commonest ink character
/// there is. A UTF-8 BOM is stripped, as everywhere else.
///
/// `.` and `0` used to count as empty too, and that was a real bug rather than a
/// harmless extra: Billy's second batch of art (2026-08-05) used `0` as its darkest
/// tone, so one portrait arrived 99×51 with 721 zeros and 28 other marks and would
/// have drawn as two dozen scattered dots — a broken-looking asset caused by a
/// format disagreement, in the same family as the CRLF and BOM bugs. The rule is
/// now the narrowest one that cannot be ambiguous: **whatever is not blank, is
/// there.** The fourteen grids from the first batch used `.` as their background
/// and contained no spaces at all, so they were migrated to spaces in the same
/// pass; their point counts came through identical.
///
/// Ragged lines are fine: width is the longest row, and short rows are simply
/// empty to their right.
fn parse_portrait_grid(text: &str) -> Option<(usize, usize, Vec<(u8, u8)>)> {
    let text = text.trim_start_matches('\u{feff}').replace('\r', "");
    let rows: Vec<&str> = text
        .lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .collect();
    // Trim wholly-empty rows off the top and bottom, so a converter's padding
    // does not shrink the drawn portrait.
    let inked = |l: &str| l.chars().any(|c| c != ' ');
    let first = rows.iter().position(|l| inked(l))?;
    let last = rows.iter().rposition(|l| inked(l))?;
    let rows = &rows[first..=last];
    let w = rows.iter().map(|l| l.chars().count()).max()?;
    let h = rows.len();
    if w == 0 || h == 0 || w > PORTRAIT_MAX_W || h > PORTRAIT_MAX_H {
        return None;
    }
    let mut pts = Vec::new();
    for (y, line) in rows.iter().enumerate() {
        for (x, c) in line.chars().enumerate() {
            if c != ' ' {
                pts.push((x as u8, y as u8));
            }
        }
    }
    (!pts.is_empty()).then_some((w, h, pts))
}

// ── The relic log ──────────────────────────────────────────────────────────
// `assets/relic_log.json` (Billy's, 2026-08-04): 30 entries, every id a
// Fibonacci number, 18 archetypes sharing 11 avatars. 21 are human logs, 8 are
// pure Logalith intrusions, and one — MycelliAI — is a human log the entity
// breaks into. It replaces voice.txt and both integrity pools outright.
/// Selection weights by rarity, Fibonacci themselves: a common entry comes up
/// eight times as often as a very rare one.
/// Odds that the next box is the entity rather than a witness, given how agitated
/// it is. Clamped below 1 so a human log is never impossible.
fn intrude_chance(agitation: f32) -> f32 {
    (DECAY_INTRUDE_CHANCE + agitation.clamp(0.0, 1.0) * AGITATE_INTRUDE_CHANCE).min(0.95)
}

/// The asset name a portrait is looked up under: `portraits/<stem>.txt`.
///
/// Any extension in the data is discarded, so `engineer_1bit`, `engineer_1bit.png`
/// and `engineer_1bit.txt` all resolve to the same file. Any directory part is
/// stripped and `..` is refused: the name arrives from a data file, so it must not
/// be able to point outside the portraits folder.
fn portrait_asset_name(avatar: &str) -> Option<String> {
    let stem = avatar.rsplit_once('.').map(|(s, _)| s).unwrap_or(avatar);
    let stem = stem.rsplit(['/', '\\']).next().unwrap_or(stem);
    // Stripping the directory is what makes escape impossible; this rejects the
    // leftovers that are not names at all — "", ".", "..", "..." — which would
    // otherwise resolve to a nonsense file inside the folder.
    stem.chars()
        .any(|c| c != '.')
        .then(|| format!("portraits/{stem}.txt"))
}

fn rarity_weight(rarity: &str) -> u32 {
    match rarity {
        "common" => 8,
        "uncommon" => 5,
        "rare" => 3,
        _ => 1, // very_rare, and anything unrecognised
    }
}

/// How agitated the entity is, 0..1 — an accumulation, not a threshold.
///
/// Dread is instantaneous: it reads `max(rip, haunt)` and drives the visuals.
/// Agitation is the *history* of that, and it drives the voice. It fills while
/// the player subverts and drains when they relent, which is DESIGN.md's
/// forgiveness rule made mechanical — the instrument never holds a grudge, so
/// pushing it again is always a fresh choice. Rising over ~20 s and draining
/// over ~45 s means sustained provocation is what earns an interruption, not a
/// slider flick.
const AGITATE_RISE_S: f32 = 20.0;
const AGITATE_FALL_S: f32 = 45.0;
/// The chance a box is an intrusion: a standing floor from the interface's own
/// decay — independent of anything the player has done, which is what suggests how
/// long this thing has been sitting here (Billy) — plus the entity's accumulated
/// agitation on top. 8 % at rest, 78 % when fully provoked.
///
/// A *probability*, not a threshold. A threshold with a discharge was the first
/// design and it could not work at this cadence: agitation refills from a spend in
/// about 4 seconds while a box lasts nearer 60, so the charge was always repaid
/// before the next box and the discharge gated nothing at all. Scaling the odds
/// instead behaves correctly at every level, and it leaves a human log always
/// possible — the entity dominating the channel is never quite the same as it
/// owning it.
const DECAY_INTRUDE_CHANCE: f32 = 0.08;
const AGITATE_INTRUDE_CHANCE: f32 = 0.70;
/// Above this, a relic carrying both a log and an intrusion has the log cut short
/// by its own intrusion.
const AGITATE_INTERRUPT: f32 = 0.45;

/// The found-document furniture: what a record says about itself besides what its
/// witness said. Shown on the record card in cell Z.
#[derive(Clone, serde::Deserialize)]
struct RelicMeta {
    #[serde(default)]
    tstamp: String,
    #[serde(default)]
    alias: String,
    #[serde(default)]
    extra: String,
}

/// One thing said, and everything the record knows about who said it. `id`,
/// `archetype`, `era`, `rarity` and `metadata` are the record card in cell Z;
/// `log` and `intrusion` are the voice in cell W.
#[derive(Clone, serde::Deserialize)]
struct Relic {
    #[serde(default)]
    id: String,
    #[serde(default)]
    archetype: String,
    #[serde(default)]
    era: String,
    #[serde(default)]
    rarity: String,
    #[serde(default)]
    avatar: String,
    #[serde(default)]
    metadata: RelicMeta,
    #[serde(default)]
    log: Vec<String>,
    #[serde(default)]
    intrusion: Vec<String>,
}

impl Default for RelicMeta {
    fn default() -> Self {
        RelicMeta {
            tstamp: String::new(),
            alias: String::new(),
            extra: String::new(),
        }
    }
}

impl Relic {
    /// An entry with no log is the entity speaking unaccompanied.
    fn is_pure_intrusion(&self) -> bool {
        self.log.is_empty() && !self.intrusion.is_empty()
    }
}

// ── Space Z: the record card ───────────────────────────────────────────────────
// Billy's pick for the zone once the portraits came out of it: the found-document
// furniture — archetype, era, tstamp, alias, extra, id — standing beside the log
// text in W rather than crowding it.

/// What a blank field shows. Most entries carry no `tstamp`, `alias` or `extra` at
/// all — they are optional by design, so that adding a line to the log stays four
/// lines of JSON.
const NOT_RECORDED: &str = "——";

/// The speaker's name on the card, in the speaker's own face.
const RECORD_NAME_PX: f32 = 16.0;

/// Rarity is drawn as pips rather than named alone, because the weights *are* the
/// answer to "how often will I see this": 8, 5, 3, 1 — Fibonacci, and out of eight,
/// so a full row is the commonest thing in the log and one pip is the rarest. Drawn
/// as squares rather than typed as bullets for the same reason the window buttons
/// are drawn: a glyph can fall back to another face's idea of `•`.
const RARITY_SLOTS: u32 = 8;
const PIP_PX: f32 = 6.0;
const PIP_GAP: f32 = 3.0;

/// The label column, in characters. The interface face is monospaced, so padding a
/// label to a fixed width is all the alignment this needs.
const RECORD_LABEL_W: usize = 8;

/// The record card's rows, in fixed order, blanks included.
///
/// Fixed is the whole point. A card that showed only the fields it had would change
/// height every time the voice advanced, and a panel that resizes itself every 45
/// seconds reads as a glitch; a form with an unfilled line reads as a record, which
/// is the idiom this furniture is borrowed from.
fn record_rows(r: &Relic) -> [(&'static str, String); 4] {
    let filled = |s: &str| {
        if s.trim().is_empty() {
            NOT_RECORDED.to_string()
        } else {
            s.trim().to_string()
        }
    };
    [
        ("ERA", filled(&r.era)),
        ("TSTAMP", filled(&r.metadata.tstamp)),
        ("ALIAS", filled(&r.metadata.alias)),
        ("EXTRA", filled(&r.metadata.extra)),
    ]
}

// ── The relic log's on-disk shape ──────────────────────────────────────────
// Characters own their entries, so **adding a line to an existing witness is four
// lines of JSON, and adding a witness is one block** (Billy, 2026-08-04). The
// original flat array repeated `archetype`, `era`, `rarity` and `avatar` on every
// single entry, which meant thirty copies of eleven characters' details and an id
// to invent each time.
//
// Everything except the log lines is optional. An entry inherits its character's
// archetype, era, rarity and portrait unless it says otherwise — which is what
// lets the eight Logalith intrusions each keep their own A–H label while sharing
// one portrait and one rarity.

/// Ids are **assigned by position** from the distinct Fibonacci numbers — 1, 2, 3,
/// 5, 8, 13 … — so Billy never types one and the property holds by construction.
///
/// This is not a retrofit: the hand-written ids in the original file were exactly
/// this sequence in exactly this order, which is how the grouped structure was
/// confirmed to be the shape the data already had. Saturating, because F(87)
/// leaves u64 and a log that long is not the problem to solve.
fn fib_id(n: usize) -> u64 {
    let (mut a, mut b) = (1u64, 2u64);
    for _ in 0..n {
        let next = a.saturating_add(b);
        a = b;
        b = next;
    }
    a
}

#[derive(serde::Deserialize)]
struct RelicFile {
    #[serde(default)]
    characters: Vec<RelicCharacter>,
}

#[derive(serde::Deserialize)]
struct RelicCharacter {
    #[serde(default)]
    archetype: String,
    #[serde(default)]
    era: String,
    #[serde(default)]
    rarity: String,
    /// Portrait basename, extension optional — `portraits/<name>.txt` is what gets
    /// loaded. `avatar` is accepted as an alias so the original field name still
    /// works.
    #[serde(default, alias = "avatar")]
    portrait: String,
    #[serde(default)]
    entries: Vec<RelicEntry>,
}

/// One thing said. Every field but the lines is an optional override of the
/// character it belongs to.
#[derive(serde::Deserialize)]
struct RelicEntry {
    #[serde(default)]
    archetype: String,
    #[serde(default)]
    era: String,
    #[serde(default)]
    rarity: String,
    #[serde(default, alias = "avatar")]
    portrait: String,
    #[serde(default)]
    tstamp: String,
    #[serde(default)]
    alias: String,
    #[serde(default)]
    extra: String,
    #[serde(default)]
    log: Vec<String>,
    #[serde(default)]
    intrusion: Vec<String>,
}

/// Either shape loads. The grouped object is the format to author in; the original
/// flat array still parses, so a half-converted file — or an old copy pasted back
/// in — is never a silent mute.
#[derive(serde::Deserialize)]
#[serde(untagged)]
enum RelicSource {
    Grouped(RelicFile),
    Flat(Vec<Relic>),
}

impl RelicSource {
    fn flatten(self) -> Vec<Relic> {
        let mut out = Vec::new();
        match self {
            RelicSource::Flat(v) => out = v,
            RelicSource::Grouped(f) => {
                for c in f.characters {
                    for e in c.entries {
                        let pick = |entry: String, character: &String| {
                            if entry.is_empty() {
                                character.clone()
                            } else {
                                entry
                            }
                        };
                        out.push(Relic {
                            id: String::new(), // assigned below, by position
                            archetype: pick(e.archetype, &c.archetype),
                            era: pick(e.era, &c.era),
                            rarity: pick(e.rarity, &c.rarity),
                            avatar: pick(e.portrait, &c.portrait),
                            metadata: RelicMeta {
                                tstamp: e.tstamp,
                                alias: e.alias,
                                extra: e.extra,
                            },
                            log: e.log,
                            intrusion: e.intrusion,
                        });
                    }
                }
            }
        }
        // Entries that say nothing would be selectable but blank.
        out.retain(|r| !r.log.is_empty() || !r.intrusion.is_empty());
        for (i, r) in out.iter_mut().enumerate() {
            if r.id.is_empty() {
                r.id = fib_id(i).to_string();
            }
        }
        out
    }
}

/// Load the relic log from the assets.
fn load_relics() -> Vec<Relic> {
    for path in asset_dirs().into_iter().map(|d| d.join("relic_log.json")) {
        if let Ok(text) = std::fs::read_to_string(&path) {
            // Strip a UTF-8 byte-order mark. PowerShell writes one by default, and
            // `serde_json` treats it as a syntax error at line 1 column 1 — so the
            // whole log parses as nothing and the instrument goes mute with only a
            // console line to say why, which a windowed binary has nowhere to put.
            // Same family as the CRLF bug that used to empty the voice pools: text
            // authored on Windows needs meeting halfway.
            let text = text.trim_start_matches('\u{feff}');
            match serde_json::from_str::<RelicSource>(text) {
                Ok(src) => {
                    let relics = src.flatten();
                    if !relics.is_empty() {
                        return relics;
                    }
                    eprintln!("relic_log.json parsed but held no entries");
                }
                // A malformed file must not silence the instrument; it also must
                // not be silent about itself.
                Err(e) => eprintln!("relic_log.json unreadable: {e}"),
            }
        }
    }
    Vec::new()
}


/// Where assets are looked for, in order: run from the workspace root, run from
/// beside a shipped binary, then the crate's own source tree.
///
/// That last one is `CARGO_MANIFEST_DIR`, baked at compile time. It exists because
/// the first two are relative to the *working* directory, so the instrument only
/// found its assets when launched from one particular folder — and `cargo test`
/// launches from another, which is how this surfaced. In a shipped binary the
/// path simply won't exist and the entry costs nothing.
fn asset_dirs() -> [std::path::PathBuf; 3] {
    [
        std::path::PathBuf::from("crates/fibonacci-gui/assets"),
        std::path::PathBuf::from("assets"),
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets"),
    ]
}

/// Resolve an asset name to a readable path.
fn asset_path(base: &str) -> Option<std::path::PathBuf> {
    asset_dirs().into_iter().map(|d| d.join(base)).find(|p| p.is_file())
}

fn file_stamp(path: &std::path::Path) -> Option<SystemTime> {
    std::fs::metadata(path).ok()?.modified().ok()
}

/// The shell's polar radius at `theta`, in units of its outermost radius, with
/// the suture's travelling undulation at phase `ph` — so the ribs' endpoints ride
/// the same rippling seam the suture is drawn along.
fn shell_unit_r(theta: f32, ph: f32, cycles: f32) -> f32 {
    let theta_max = SHELL_TURNS * std::f32::consts::TAU;
    SHELL_GROWTH.powf((theta - theta_max) / std::f32::consts::TAU)
        * (1.0 + SUTURE_WAVE * (theta * cycles + ph).sin())
}

/// The shell's target scale for a given master level.
fn scale_for(master: f32) -> f32 {
    SCALE_MIN + (SCALE_MAX - SCALE_MIN) * master.clamp(0.0, 1.0)
}

/// A rib's angular offset from its own station, at fractional position `u` along
/// it (0 at the inner end, 1 at the outer). Three terms: the lean back toward the
/// centre, the static S-bend, and the travelling ripple — the last two windowed so
/// both ends stay pinned to their sutures.
///
/// `span` is the chamber's angular width, capped; `waviness` doubles the S-bend
/// for the free ribs inside a chamber; `along` is wavelengths along the rib (from
/// `damp`); `phase` is where this rib stands in the travelling wave.
fn rib_offset(u: f32, span: f32, waviness: f32, along: f32, phase: f32) -> f32 {
    let pi = std::f32::consts::PI;
    let s = span.min(SHELL_SPAN_CAP);
    let lean = SHELL_RIB_SWEEP * s * (1.0 - u);
    let wave = SHELL_RIB_WAVE * s * waviness * (u * std::f32::consts::TAU).sin();
    let ripple = RIPPLE_AMP * s * (u * pi * along + phase).sin() * (u * pi).sin();
    -lean + wave + ripple
}

/// Chambers per whorl, outermost first — and this is where the shell says which
/// tuning it is in. Each mode's counts come from the integer sequence whose
/// ratio limit *is* that mode's own constant:
///
/// - **harmonic**: the harmonic series itself, 6 × (5, 4, 3, 2, 1).
/// - **fibonacci**: F(9)…F(5).
/// - **golden**: the Lucas numbers. `L(n)/L(n−1) → φ` just as the Fibonacci
///   ratios do — the other golden sequence, so golden mode is visibly a
///   *different* creature from fibonacci mode while meaning the same limit.
/// - **mirror**: those same Lucas numbers reversed, so the dense divisions sit at
///   the centre — a mode that mirrors its exponents around the fundamental,
///   mirrored.
/// - **plastic**: Padovan numbers, `P(n) = P(n−2) + P(n−3)`, whose ratio limit is
///   ρ. That is the very sequence the mode's own operator ratios come from.
///
/// No two whorls in a mode share a count, so no two share an angular division and
/// the per-operator assignment cannot line up into radial stripes. (Chambers are
/// numbered by one running counter across the whole shell, so a stripe would need
/// two whorls sharing both a count and a starting operator — individual counts
/// being divisible by 5 is harmless, and two of them are.)
fn chambers_for(mode: RatioMode) -> [u32; SHELL_WHORLS] {
    match mode {
        RatioMode::Harmonic => [30, 24, 18, 12, 6],
        RatioMode::Fibonacci => [34, 21, 13, 8, 5],
        RatioMode::Golden => [29, 18, 11, 7, 4],
        RatioMode::GoldenMirror => [4, 7, 11, 18, 29],
        RatioMode::Plastic => [28, 21, 16, 12, 9],
    }
}

/// Where the shell sits in a panel: the origin its polar equation is drawn
/// about, and the pixel length of one unit radius.
///
/// A logarithmic spiral is not centred on its own origin, and its origin is
/// not its middle: the silhouette is fat at the terminal angle and thin a
/// whorl back, so its outline's centre sits ≈(+0.181, +0.229) radii from the
/// pole. Drawn about the panel centre it lands in a corner with dead space
/// opposite (Billy, 2026-08-04: "other than position"). Measure the unit
/// outline once — arm thickness included — then scale it to fill the panel and
/// translate so the *outline* is centred. Shared by the shell and by anything
/// that has to aim at it.
fn shell_fit(rect: Rect) -> (Pos2, f32) {
    let (lo, hi) = shell_unit_bounds();
    let span = hi - lo;
    let max_r = ((rect.width() / span.x).min(rect.height() / span.y) * SHELL_FILL).max(1.0);
    (rect.center() - (lo + hi) * 0.5 * max_r, max_r)
}

/// The shell's unit silhouette as `(lo, hi)` in units of the outermost radius.
/// The rim *is* the suture, so the curve's own bounds are the silhouette's.
///
/// Measured on the **smooth** spiral and then inflated by the undulation's worst
/// case: the suture ripples with time, and a fit recomputed against a moving
/// boundary would make the whole shell breathe in *scale*, which is a different
/// and much worse effect than the one intended.
fn shell_unit_bounds() -> (Vec2, Vec2) {
    let theta_max = SHELL_TURNS * std::f32::consts::TAU;
    let tau = std::f32::consts::TAU;
    let (mut lo, mut hi) = (Vec2::new(f32::MAX, f32::MAX), Vec2::new(f32::MIN, f32::MIN));
    let probe = 720;
    for k in 0..=probe {
        let theta = theta_max * k as f32 / probe as f32;
        let r = SHELL_GROWTH.powf((theta - theta_max) / tau) * (1.0 + SUTURE_WAVE);
        let p = Vec2::new(theta.cos(), theta.sin()) * r;
        lo = Vec2::new(lo.x.min(p.x), lo.y.min(p.y));
        hi = Vec2::new(hi.x.max(p.x), hi.y.max(p.y));
    }
    (lo, hi)
}

struct App {
    rig: AudioRig,
    shadow: Patch,
    shadow_verb: VerbParams,
    shadow_melody: MelodyParams,
    /// Whether the scale bank is on show. A *view* toggle only (Billy's
    /// sweep, 2026-08-05): the SCALE button reveals the bank so a scale can
    /// be chosen, then hides it again so the phase image gets the panel back.
    /// Never touches the tuning and is deliberately not persisted.
    scale_picker_open: bool,
    start: Instant,
    // Live visual state, fed by the viz ring.
    env: [f32; NUM_OPS],
    scope: VecDeque<f32>,
    lissajous: VecDeque<(f32, f32)>,
    log: VecDeque<String>,
    /// Billy's relic log — the only voice source. Replaced voice.txt and both
    /// integrity pools.
    relics: Vec<Relic>,
    /// Index of the relic currently speaking, and whether its human log is being
    /// cut short by its own intrusion.
    relic: usize,
    relic_interrupted: bool,
    /// The voice's own PRNG. Seeded from the clock at startup — the one piece of
    /// nondeterminism in the program, so the log does not recite itself in the
    /// same order every launch. It cannot reach the audio path.
    voice_rng: u32,
    /// Accumulated provocation, 0..1. Dread reads the instant; this reads the
    /// history, and it is what earns an interruption.
    agitation: f32,
    dread: Dread,
    last_status_log: f64,
    voice_last_advance: f64,
    voice_last_reload: f64,
    /// The box currently being typed. Compared against the selected box each
    /// frame: any difference (advance, pool switch, or Billy editing the file
    /// under us) restarts the reveal.
    voice_typing: String,
    /// Characters of `voice_typing` revealed so far, and the unspent typing
    /// time carried between frames.
    voice_revealed: usize,
    voice_credit: f32,
    /// Wall clock of the previous frame, for the reveal's own timebase.
    last_frame_time: f64,
    /// Integrated phases for the shell's ripple and its suture's undulation.
    /// See `update` for why these are accumulated rather than computed from
    /// elapsed time.
    ripple_phase: f32,
    suture_phase: f32,
    /// The rock's three oscillators, integrated separately. They cannot share one
    /// phase scaled by φ powers: wrapping a shared phase at τ would leave the
    /// φ-multiplied ones discontinuous, since φ is irrational and no common period
    /// exists. Kept apart, each wraps cleanly.
    rock_phase: [f32; 3],
    /// The shell's continuous slow turn.
    spin: f32,
    /// The shell's smoothed scale, chasing `master`.
    shell_scale: f32,
    /// Last reported size of cell Z's interior, so a resize reports once.
    portrait_area: (i32, i32),
    /// How far into the critical band we are, 0..1, smoothed. Everything the band
    /// drives is scaled by this, so it ramps instead of snapping.
    dread_level: f32,
    /// Smoothed INDEX, driving the umbilicus. A slider move should open or close
    /// the centre, not teleport it.
    index_smooth: f32,
    /// Space Z's art, if a slot is filled, and the band it was resolved for —
    /// so a band change re-resolves the slot immediately instead of waiting
    /// for the next asset poll.
    portrait: Option<Portrait>,
    /// The avatar filename the plinth is currently holding, so a change of
    /// speaker reloads it immediately instead of waiting for the asset poll.
    portrait_avatar: String,
    frame_count: u64,
    drone_hz: f32,
    /// One text, two duties: it filters the bank live and it names a save.
    preset_name: String,
    preset_names: Vec<String>,
    /// The header pane's visibility. Not persisted.
    presets_open: bool,
    /// The single highlighted preset — at most one, ever.
    preset_selected: Option<String>,
    preset_armed: PaneArm,
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
            scale_picker_open: false,
            start: Instant::now(),
            env: [0.0; NUM_OPS],
            scope: VecDeque::with_capacity(1024),
            lissajous: VecDeque::with_capacity(512),
            log: VecDeque::new(),
            relics: load_relics(),
            relic: 0,
            relic_interrupted: false,
            voice_rng: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .map(|d| d.subsec_nanos() ^ d.as_secs() as u32)
                .unwrap_or(0x9E37_79B9)
                | 1,
            agitation: 0.0,
            dread: Dread::Normal,
            last_status_log: 0.0,
            voice_last_advance: 0.0,
            voice_last_reload: 0.0,
            voice_typing: String::new(),
            voice_revealed: 0,
            voice_credit: 0.0,
            last_frame_time: 0.0,
            ripple_phase: 0.0,
            suture_phase: 0.0,
            rock_phase: [0.0; 3],
            spin: 0.0,
            // Start settled, so a restored session opens at its own size rather
            // than swelling in from small.
            shell_scale: scale_for(shadow.master_level),
            portrait_area: (0, 0),
            dread_level: 0.0,
            index_smooth: shadow.index,
            portrait: None,
            portrait_avatar: String::new(),
            frame_count: 0,
            drone_hz: state.drone_hz,
            preset_name: String::new(),
            preset_names: scan_presets(),
            presets_open: false,
            preset_selected: None,
            preset_armed: PaneArm::None,
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
        if asset_path("portrait.png").is_none() {
            app.push_log("portrait slot empty: assets/portrait.png absent.".into());
        }
        Ok(app)
    }

    fn integrity(&self) -> f32 {
        100.0 * (1.0 - self.shadow.rip.max(self.shadow_verb.haunt))
    }

    /// Advance the dread state machine (hysteresis per the band constants)
    /// and let the maintenance routine report more often as things degrade
    /// — measurements only, always.
    fn update_dread(&mut self, now: f64) {
        let integrity = self.integrity();
        let next = match self.dread {
            Dread::Normal if integrity < 47.5 => Dread::Low,
            Dread::Low if integrity > 52.5 => Dread::Normal,
            Dread::Low if integrity < 12.5 => Dread::Critical,
            Dread::Critical if integrity > 17.5 => Dread::Low,
            current => current,
        };
        if next != self.dread {
            self.dread = next;
            let band = match next {
                Dread::Normal => "nominal",
                Dread::Low => "low",
                Dread::Critical => "critical",
            };
            self.push_log(format!("integrity {integrity:.0}%. band: {band}."));
        }
        let interval = match self.dread {
            Dread::Normal => f64::INFINITY,
            Dread::Low => 30.0,
            Dread::Critical => 10.0,
        };
        if now - self.last_status_log > interval {
            self.last_status_log = now;
            self.push_log(format!(
                "integrity {integrity:.0}%. rip {:.2}. haunt {:.2}.",
                self.shadow.rip, self.shadow_verb.haunt
            ));
        }
    }

    /// Horizontal tremble for the controls that caused the damage — and for the
    /// shell. ±1 px scaled by how far into the critical band we are, so it fades
    /// in and out rather than snapping on. Purely visual, never functional.
    /// Callers that lay out widgets round it, so the controls stay pixel-crisp
    /// while the ramp resolves in whole steps.
    fn tremble(&self) -> f32 {
        (((self.frame_count / 2 % 3) as f32) - 1.0) * self.dread_level
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

    /// Writes the current state under `name` and returns what was actually
    /// written. Never overwrites: a taken name gets `~X` appended (Billy's
    /// sweep, 2026-08-05) until one is free, so the only way to lose a preset
    /// is DELETE's own double confirmation.
    fn save_preset(&mut self, name: &str) -> Option<String> {
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
            return None;
        }
        let mut written = file.clone();
        let mut x = 1;
        while dir.join(format!("{written}.json")).exists() {
            written = format!("{file}~{x}");
            x += 1;
        }
        let path = dir.join(format!("{written}.json"));
        match serde_json::to_string_pretty(&self.current_state()) {
            Ok(json) => match std::fs::write(&path, json) {
                Ok(()) => {
                    self.preset_names = scan_presets();
                    self.push_log(format!("preset '{written}' written."));
                    Some(written)
                }
                Err(e) => {
                    self.push_log(format!("preset write failed: {e}."));
                    None
                }
            },
            Err(e) => {
                self.push_log(format!("preset serialize failed: {e}."));
                None
            }
        }
    }

    /// Removes a preset file. Only reachable through the pane's double
    /// confirmation, and only ever the highlighted preset.
    fn delete_preset(&mut self, name: &str) {
        let path = preset_dir().join(format!("{name}.json"));
        match std::fs::remove_file(&path) {
            Ok(()) => {
                self.preset_names = scan_presets();
                if self.preset_selected.as_deref() == Some(name) {
                    self.preset_selected = None;
                }
                self.push_log(format!("preset '{name}' deleted."));
            }
            Err(e) => self.push_log(format!("preset delete failed: {e}.")),
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
            if self.scope.len() >= 1024 {
                self.scope.pop_front();
            }
            self.scope.push_back(mono);
            if self.lissajous.len() >= 512 {
                self.lissajous.pop_front();
            }
            self.lissajous.push_back((frame.l, frame.r));
        }
    }

    /// Where the shell is this frame — evaluated once and shared, because the
    /// background's lensing has to bend around exactly the shell that gets
    /// drawn, not a stale copy of it.
    ///
    /// The rock: golden-related oscillators, one pair for the diagonal sway and
    /// a summed pair — offset by π/5, the instrument's own rotation constant —
    /// for the tilt, so the turn leads the drift rather than tracking it, and
    /// neither is a metronome.
    ///
    /// The fit happens inside a panel shrunk by everything the rock can add, so
    /// sway and tilt provably never reach the frame. The shell turns about its
    /// own silhouette centre, not the spiral's pole: the pole is ≈0.29 radii
    /// off-centre, so turning about it would swing the whole shape through an
    /// arc and cost far more room than turning it in place. Rotating a box in
    /// place grows it by the *other* axis's extent times sin(tilt), and since
    /// the fit is height-limited the horizontal share of that is free — budget
    /// the axes separately and the amplitude comes much cheaper. Signal jitter
    /// is deliberately NOT budgeted for: it is caged by `SHELL_JITTER_MAX` and
    /// clipped by the panel, because a Room violent enough to push the shell
    /// into its own frame should look like it.
    fn shell_pose(&self, rect: Rect) -> ShellPose {
        // Phases are integrated in `update` — and divided there by the scale, so a
        // bigger shell drifts more slowly.
        let [p0, p1, p2] = self.rock_phase;
        let w1 = p0.sin();
        let w2 = p1.sin();
        // The oscillating tilt, plus the continuous spin.
        let tilt = ((p1 + std::f32::consts::PI / 5.0).sin() + ROCK_TILT_2 * p2.sin())
            / (1.0 + ROCK_TILT_2)
            * ROCK_TILT
            + self.spin;

        let (_, probe_r) = shell_fit(rect);
        let (lo, hi) = shell_unit_bounds();
        // Sway grows with the *square* of the scale, so a close shell ranges
        // across the panel instead of nudging.
        let s = self.shell_scale;
        let sway = Vec2::new(w1, w2) * (probe_r * s * s * ROCK_SWAY);
        let half = (hi - lo) * 0.5 * probe_r;
        let swing = ROCK_TILT.sin();
        // The sway is deliberately **not** budgeted here. Budgeting it would shrink
        // the shell to guarantee the drift stays inside the frame, which is the
        // opposite of the point: it is supposed to range across the space and be
        // cropped when it reaches the edge. Only the tilt and the stutter are
        // reserved for; the clip holds everything else.
        let headroom = Vec2::new(
            half.y * swing + SHELL_TREMBLE_PX,
            half.x * swing,
        );
        // At critical dread the whole shell stutters, on the same clock and in the
        // same direction as the sliders that caused it, so the panel and its
        // controls shake together. Budgeted into the headroom above rather than
        // left to the clip, so the containment stays a proof.
        // Ramped by `dread_level` inside `tremble`, so the stutter fades in.
        let stutter = Vec2::new(self.tremble() * SHELL_TREMBLE_PX, 0.0);
        let (_, fitted_r) = shell_fit(rect.shrink2(headroom));
        // `master`, smoothed, scales the shell. The pole has to be re-derived
        // from the scaled radius rather than simply moved: its offset from the
        // silhouette's centre is itself proportional to the radius, so scaling
        // `max_r` alone would shrink the shell toward its own pole and slide it
        // off-centre.
        let max_r = fitted_r * self.shell_scale;
        let pole = rect.center() - (lo + hi) * 0.5 * max_r;
        ShellPose {
            pivot: rect.center() + sway + stutter,
            pole: pole + sway + stutter,
            tilt,
            max_r,
        }
    }

    /// The Logalith: a Fibonacci shell, drawn rather than filled.
    ///
    /// One continuous **suture** whose successive turns are the whorl
    /// boundaries; **septa** crossing each whorl, `chambers_for(mode)` of them —
    /// an integer sequence whose ratio limit is the tuning's own constant, which
    /// is where the shell says what it is; and finer **growth ribs** between the
    /// septa, their count and their reach across the whorl carrying one
    /// operator's live level.
    ///
    /// The whole of it ripples on a travelling wave, and the sound reaches it
    /// through six channels: operator levels engrave the chambers, `rip`
    /// agitates the ripple, `damp` smooths its wavelengths, `fb` draws every rib
    /// alongside its own echoes, `haunt` raises ghost sutures at π/5, and the
    /// ratio mode divides the whorls. Nothing is destroyed to say any of it.
    ///
    /// Strokes, not cells. A raster cell was 11 px at Billy's panel, so a whorl
    /// 14 px across could not be drawn at all; a 1 px stroke has no such floor,
    /// which is what lets the inner whorls exist (Billy: "more granularity...
    /// running it as pixels is not the right idea"). Points are snapped to whole
    /// pixels, because feathering is off and a fractional vertex is a blurry
    /// vertex.
    fn draw_monolith(&self, painter: &egui::Painter, rect: Rect, pose: &ShellPose) {
        if rect.width().min(rect.height()) < 16.0 {
            return;
        }
        let tau = std::f32::consts::TAU;
        let theta_max = SHELL_TURNS * tau;
        let ShellPose { pole, tilt, max_r, .. } = *pose;
        // rip drives the ripple's *speed*, integrated into these phases in
        // `update`. The amplitude is constant: the shell always writhes, rip
        // decides how frantically. haunt gets its own conveyance below (the
        // ghosts), so it is not doubled up here.
        //
        // At critical dread the phase is re-thrown every frame, so the wave stops
        // travelling and the shell convulses. One offset for the whole shell, not
        // one per rib: the ribs keep their phase relationships, so the pattern
        // stays a pattern and flickers between configurations rather than
        // dissolving into noise.
        // Ramped by the same smoothed band level as the stutter, so the convulsion
        // comes on rather than being switched on.
        let seize = if self.dread_level > 0.001 {
            let h = self.frame_count.wrapping_mul(2654435761) % 10007;
            (h as f32 / 10007.0 - 0.5) * SHELL_SEIZE * self.dread_level
        } else {
            0.0
        };
        let suture_ph = self.suture_phase + seize;
        let ripple_clock = self.ripple_phase + seize;

        // damp smooths the shell's own waves: fewer undulations per whorl, and
        // longer wavelengths along each rib.
        let damp = self.shadow_verb.damp.clamp(0.0, 1.0);
        let cycles = SUTURE_CYCLES_OPEN + (SUTURE_CYCLES_DAMPED - SUTURE_CYCLES_OPEN) * damp;
        let along = RIPPLE_ALONG_OPEN + (RIPPLE_ALONG_DAMPED - RIPPLE_ALONG_OPEN) * damp;
        // fb is an operator re-injecting its past: every rib is drawn with its
        // own repeats alongside it.
        let echoes = (self.shadow.feedback.clamp(0.0, 1.0) * ECHO_MAX).round() as u32;

        let r_at = |t: f32| max_r * shell_unit_r(t, suture_ph, cycles);
        // The umbilicus: the open centre, dilated as INDEX falls.
        let eye = (SHELL_MIN_PX + max_r * UMBILICUS_MAX * (1.0 - self.index_smooth))
            .min(max_r * 0.9);
        // Whole-pixel vertices: feathering is off, so a fractional point is a
        // blurry point.
        let at = |t: f32, r: f32| {
            let a = t + tilt;
            Pos2::new(
                (pole.x + a.cos() * r).round(),
                (pole.y + a.sin() * r).round(),
            )
        };
        // A rib is a wavy curve, not a spoke: it leans back toward the centre and
        // S-bends on the way, both ends anchored. `span` is the chamber's own
        // angular width, so the shape is the same on every whorl. `waviness`
        // scales the S-bend only — see `SHELL_RIB_INNER` for why the lean is
        // deliberately left uniform.
        let rib = |t: f32, r_inner: f32, r_outer: f32, w: f32, span: f32, waviness: f32| {
            // The travelling wave's phase where this rib stands, its gradient
            // scaled by radius so the wavelength is constant in pixels. That
            // keeps neighbours nearly in step, which is what lets the amplitude
            // be large without ribs meeting each other.
            let ph = ripple_clock + t * RIPPLE_WRAP * (r_outer / max_r);
            let n = 14;
            let shape = |off: f32| -> Vec<Pos2> {
                (0..=n)
                    .map(|k| {
                        let u = k as f32 / n as f32;
                        at(
                            t + rib_offset(u, span, waviness, along, ph) + off,
                            r_inner + (r_outer - r_inner) * u,
                        )
                    })
                    .collect()
            };
            painter.add(egui::Shape::line(shape(0.0), Stroke::new(w, WHITE)));
            // fb's echoes: the same rib again, stepped round and dashed, so it
            // reads as a repeat rather than as a second rib.
            for e in 1..=echoes {
                let pts = shape(ECHO_STEP * span.min(SHELL_SPAN_CAP) * e as f32);
                for k in (0..pts.len() - 1).step_by(2) {
                    painter.line_segment([pts[k], pts[k + 1]], Stroke::new(1.0_f32, WHITE));
                }
            }
        };

        // The suture, as broken polyline runs. The outermost turn is drawn
        // heavier: that edge is the silhouette, and it has to read as one.
        let mut run: Vec<Pos2> = Vec::new();
        let mut run_heavy = false;
        let flush = |run: &mut Vec<Pos2>, heavy: bool| {
            if run.len() > 1 {
                let w = if heavy { 2.0_f32 } else { 1.0 };
                painter.add(egui::Shape::line(std::mem::take(run), Stroke::new(w, WHITE)));
            } else {
                run.clear();
            }
        };
        let mut t = 0.0_f32;
        while t <= theta_max {
            let r = r_at(t);
            let heavy = t > theta_max - tau;
            if r < eye || heavy != run_heavy {
                flush(&mut run, run_heavy);
                run_heavy = heavy;
            }
            if r >= eye {
                run.push(at(t, r));
            }
            // Step by arc length, so the outer whorls get the segments and the
            // inner ones are not oversampled into sub-pixel slivers.
            t += (SUTURE_STEP_PX / r.max(1.0)).clamp(0.004, 0.30);
        }
        flush(&mut run, run_heavy);

        // haunt's ghosts: the Ghost Line drawn. Each pass is the suture again,
        // turned a further π/5 and stippled more sparsely — after five passes the
        // echo is fully inverted against the drone that spawned it, which is
        // exactly what five ghosts at π/5 look like. Drawn as dots because in
        // strict 1-bit, sparseness is the only way to be faint.
        let ghosts = (self.shadow_verb.haunt.clamp(0.0, 1.0) * GHOST_MAX).round() as u32;
        for p in 1..=ghosts {
            let turn = p as f32 * std::f32::consts::PI / 5.0;
            // Thin with the ghost's own loop gain, one pass at a time.
            let step = SUTURE_STEP_PX / GHOST_GAIN.powi(p as i32 * 4);
            let mut gt = 0.0_f32;
            while gt <= theta_max {
                let r = r_at(gt);
                if r >= eye {
                    let a = gt + tilt + turn;
                    painter.rect_filled(
                        Rect::from_center_size(
                            Pos2::new(
                                (pole.x + a.cos() * r).round(),
                                (pole.y + a.sin() * r).round(),
                            ),
                            Vec2::splat(1.0),
                        ),
                        Rounding::ZERO,
                        WHITE,
                    );
                }
                gt += (step / r.max(1.0)).clamp(0.004, 0.30);
            }
        }

        // Septa and growth ribs, whorl by whorl from the outside in.
        let chamber_counts = chambers_for(self.shadow.ratio_mode);
        let mut chamber_index: u32 = 0;
        for whorl in 0..SHELL_WHORLS {
            let chambers = chamber_counts[whorl];
            let outer_t = theta_max - whorl as f32 * tau;
            let span = tau / chambers as f32;
            for j in 0..chambers {
                let t = outer_t - j as f32 * span;
                let op = (chamber_index % NUM_OPS as u32) as usize;
                chamber_index += 1;
                if t < 0.0 {
                    continue;
                }
                // Clipped to the umbilicus: a rib may end on it but never cross it.
                let (r_in, r_out) = (r_at(t - tau).max(eye), r_at(t));
                if r_out < eye + SHELL_MIN_PX {
                    continue;
                }
                // The septum spans the whole whorl. Structural, so it keeps the
                // base waviness.
                rib(t, r_in.max(1.0), r_out, 1.0, span, 1.0);
                // Growth ribs: more of them, reaching further in, as the
                // operator gets louder. They stop short of the inner suture —
                // what the reference does, and the depth cue.
                let level = self.env[op].min(1.0);
                let mut ribs = 1 + (level * SHELL_RIB_MAX).round() as u32;
                // Thin the ribs out where a whorl is too finely divided to hold
                // them: GoldenMirror puts 29 chambers on the innermost whorl,
                // where ribs would sit 1.7 px apart and merge into a smudge.
                // Below 3 px of spacing the chamber shows its septum only.
                while ribs > 1 && span * r_out / (ribs as f32) < 3.0 {
                    ribs -= 1;
                }
                let reach = ENGRAVE_REACH_MIN + ENGRAVE_REACH_RANGE * level;
                for m in 1..ribs {
                    let rt = t - span * m as f32 / ribs as f32;
                    if rt < 0.0 {
                        continue;
                    }
                    let (ri, ro) = (r_at(rt - tau).max(eye), r_at(rt));
                    if ro < eye + SHELL_MIN_PX {
                        continue;
                    }
                    rib(rt, ro - (ro - ri) * reach, ro, 1.0, span, SHELL_RIB_INNER);
                }
            }
        }

        // The aperture: the shell's mouth, at the terminal end of the suture.
        rib(
            theta_max,
            r_at(theta_max - tau),
            r_at(theta_max),
            2.0,
            tau / chamber_counts[0] as f32,
            1.0,
        );
    }


    /// Re-resolve the portrait: the current relic's avatar, with its extension
    /// swapped for `.txt`, looked up under `assets/portraits/`. Reloads when the
    /// file changes on disk, so Billy can convert an image and watch it land.
    fn refresh_art(&mut self, _ctx: &egui::Context) {
        let avatar = self
            .relics
            .get(self.relic)
            .map(|r| r.avatar.clone())
            .unwrap_or_default();
        if avatar.is_empty() {
            self.portrait = None;
            return;
        }
        let Some(base) = portrait_asset_name(&avatar) else {
            self.portrait = None;
            return;
        };
        let stem = base.trim_end_matches(".txt").to_string();

        // Collect the frame set: the plain name, then every `-N` beside it. A gap in
        // the numbering just means one fewer frame.
        let mut paths: Vec<std::path::PathBuf> = Vec::new();
        if let Some(p) = asset_path(&base) {
            paths.push(p);
        }
        for n in 1..=PORTRAIT_MAX_FRAMES {
            if let Some(p) = asset_path(&format!("{stem}-{n}.txt")) {
                paths.push(p);
            }
        }
        if paths.is_empty() {
            self.portrait = None;
            return;
        }
        let stamp = paths.iter().filter_map(|p| file_stamp(p)).max();
        if let Some(cur) = &self.portrait {
            if cur.key == stem && cur.stamp == stamp && cur.frames.len() == paths.len() {
                return;
            }
        }

        let mut frames: Vec<PortraitFrame> = paths
            .iter()
            .filter_map(|p| std::fs::read_to_string(p).ok())
            .filter_map(|t| parse_portrait_grid(&t))
            .map(|(w, h, pts)| PortraitFrame { w, h, pts })
            .collect();
        // Order by density, so the cycle is a tonal ramp however the files were
        // named. `total_cmp` because these are floats and a NaN would otherwise be
        // an invalid ordering.
        frames.sort_by(|a, b| a.ink().total_cmp(&b.ink()));
        self.portrait = (!frames.is_empty()).then_some(Portrait {
            key: stem,
            stamp,
            frames,
        });
    }

    /// The sky the Logalith hangs in, and its motion field. Drawn first, so the
    /// shell occludes it — a star behind a solid white arm is simply not there.
    ///
    /// Six terms, composed in this order, because each later one wants to act on
    /// the result of the earlier ones:
    ///
    /// 1. **Drift**, wrapped in normalised space so the field is seamless and
    ///    endless. Bearing is the golden angle, so it slides along no axis.
    /// 2. **Parallax wobble** — a per-layer Lissajous. Depth comes from the
    ///    star's own hash, and near stars are both bigger and faster, which is
    ///    the whole of parallax.
    /// 3. **Curvature**, a barrel/pincushion coefficient breathing on an 89 s
    ///    period: the field itself bulges and draws in.
    /// 4. **Noise warp** — a smooth sum-of-products field in x, y and t, so the
    ///    sky shears locally instead of only moving as a block.
    /// 5. **Lensing** around the shell, deflection ∝ 1/d and capped.
    /// 6. **Edge shimmer**, which also hides the wrap seam from (1).
    fn draw_starfield(&self, painter: &egui::Painter, rect: Rect, pose: &ShellPose) {
        let dot = |p: Pos2, size: f32| {
            painter.rect_filled(
                Rect::from_center_size(p, Vec2::splat(size)),
                Rounding::ZERO,
                WHITE,
            );
        };
        let t = self.last_frame_time;
        let ftau = std::f64::consts::TAU;
        let tf = t as f32;
        let size = rect.size();
        let curve = SKY_CURVE * (t / SKY_CURVE_PERIOD_S * ftau).sin() as f32;
        let (drift_x, drift_y) = (SKY_DRIFT_ANGLE.cos(), SKY_DRIFT_ANGLE.sin());

        for i in 0..STAR_COUNT {
            let h = (i as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
            let kind = (h >> 5) & 31;
            // Depth: three layers. Near stars are the big ones, and they move
            // most — so brightness and speed agree, as they must.
            let depth = ((h >> 21) & 3) as f32 / 3.0;
            let near = 0.35 + 0.65 * depth;
            let phase = ((h >> 41) & 0xFFFF) as f64 / 65535.0 * ftau;

            // (1) drift, wrapped
            let mut fx = (((h >> 11) & 0xFFFF) as f32 / 65535.0
                + tf * SKY_DRIFT * near * drift_x)
                .rem_euclid(1.0);
            let mut fy = (((h >> 27) & 0xFFFF) as f32 / 65535.0
                + tf * SKY_DRIFT * near * drift_y)
                .rem_euclid(1.0);
            // Edge factor is taken from the wrapped position, before any
            // displacement, so a star shimmers because of where it sits in the
            // field rather than because the warp happened to shove it out.
            let edge = (1.0 - (fx - 0.5).abs() * 2.0)
                .min(1.0 - (fy - 0.5).abs() * 2.0)
                .max(0.0);
            let edge = (1.0 - edge / SKY_EDGE).clamp(0.0, 1.0);

            // (2) parallax wobble
            fx += (t / 34.0 * ftau + phase).sin() as f32 * SKY_WOBBLE * near;
            fy += (t / 21.0 * ftau + phase).sin() as f32 * SKY_WOBBLE * near;

            // (3) curvature of the whole field
            let mut u = Vec2::new(fx - 0.5, fy - 0.5);
            u *= 1.0 + curve * u.length_sq();

            // (4) noise warp: smooth in space and time, and cheap
            let (nx, ny) = (u.x * 6.283, u.y * 6.283);
            u += Vec2::new(
                (nx * 1.0 + (t * 0.11) as f32).sin() * (ny * 1.3 - (t * 0.07) as f32).cos(),
                (ny * 0.8 - (t * 0.09) as f32).sin() * (nx * 1.7 + (t * 0.13) as f32).cos(),
            ) * SKY_WARP;

            let mut p = rect.center() + Vec2::new(u.x * size.x, u.y * size.y);

            // (5) lensing around the shell
            let off = p - pose.pivot;
            let d = off.length();
            if d > 0.5 {
                let push =
                    (SKY_LENS * pose.max_r * pose.max_r / d).min(SKY_LENS_MAX * pose.max_r);
                p += off / d * push;
            }

            // (6) edge shimmer
            if edge > 0.0 {
                let rate = 1.3 + 1.7 * (((h >> 17) & 15) as f64 / 15.0);
                let blink = 0.5 + 0.5 * (t * rate + phase).sin() as f32;
                if blink < edge * SKY_SHIMMER_DUTY {
                    continue;
                }
            }

            let p = Pos2::new(p.x.round(), p.y.round());
            match kind {
                0 => {
                    // A sparkle: the 4-point cross of the reference panels.
                    dot(p, 2.0);
                    for d in [2.0_f32, 3.0] {
                        dot(p + Vec2::new(d, 0.0), 1.0);
                        dot(p - Vec2::new(d, 0.0), 1.0);
                        dot(p + Vec2::new(0.0, d), 1.0);
                        dot(p - Vec2::new(0.0, d), 1.0);
                    }
                }
                1..=5 => dot(p, 2.0),
                _ => dot(p, 1.0),
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
        let inset = |from: Pos2, to: Pos2, margin_a: f32, margin_b: f32| {
            let dir = (to - from).normalized();
            (from + dir * margin_a, to - dir * margin_b)
        };
        for (from, to, m_to) in [
            (nodes[0], nodes[1], 11.0),
            (nodes[0], nodes[3], 11.0),
            (nodes[1], nodes[2], 11.0),
            (nodes[1], leaves[2], 9.0),
            (nodes[2], leaves[0], 9.0),
            (nodes[2], leaves[1], 9.0),
            (nodes[3], leaves[3], 9.0),
            (nodes[3], leaves[4], 9.0),
        ] {
            let (a, b) = inset(from, to, 11.0, m_to);
            bubble_chain(&painter, a, b, true);
        }

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
                ui_font(painter.ctx(), 11.0),
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
                ui_font(painter.ctx(), 10.0),
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
        // String-art web (the sigil): chords k -> 2k around a 21-point
        // circle (21 = F(8)) — the cardioid construction. Absent at haunt 0,
        // one chord revealed at a time until the full web stands.
        const WEB_POINTS: usize = 21;
        let circle_pt = |t: f32| {
            let a = -std::f32::consts::FRAC_PI_2 + std::f32::consts::TAU * t;
            center + Vec2::new(a.cos(), a.sin()) * radius
        };
        let chords = (haunt * WEB_POINTS as f32) as usize;
        for k in 1..=chords {
            let a = circle_pt(k as f32 / WEB_POINTS as f32);
            let b = circle_pt((k * 2 % WEB_POINTS) as f32 / WEB_POINTS as f32);
            painter.line_segment([a, b], Stroke::new(1.0_f32, WHITE));
        }
        for i in 0..5 {
            let from = point(i);
            let to = point((i + 3) % 5);
            let dir = (to - from).normalized();
            bubble_chain(&painter, from + dir * 9.0, to - dir * 9.0, haunt > 0.0);
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
                ui_font(painter.ctx(), 10.0),
                WHITE,
            );
        }
    }

    /// An analog-scope graticule: dotted division grid, denser center axes
    /// with tick marks. Subtle by construction — the beam owns the display.
    fn draw_graticule(painter: &egui::Painter, rect: Rect, cols: usize, rows: usize) {
        let dot = |p: Pos2| {
            painter.rect_filled(Rect::from_center_size(p, Vec2::splat(1.0)), Rounding::ZERO, WHITE)
        };
        for c in 1..cols {
            let x = rect.min.x + rect.width() * c as f32 / cols as f32;
            let step = if c * 2 == cols { 5.0 } else { 9.0 };
            let mut y = rect.min.y;
            while y < rect.max.y {
                dot(Pos2::new(x, y));
                y += step;
            }
        }
        for r in 1..rows {
            let y = rect.min.y + rect.height() * r as f32 / rows as f32;
            let step = if r * 2 == rows { 5.0 } else { 9.0 };
            let mut x = rect.min.x;
            while x < rect.max.x {
                dot(Pos2::new(x, y));
                x += step;
            }
        }
        // Tick marks along the center axes, one per division.
        let cy = rect.center().y;
        for c in 0..=cols {
            let x = rect.min.x + rect.width() * c as f32 / cols as f32;
            painter.line_segment(
                [Pos2::new(x, cy - 2.0), Pos2::new(x, cy + 2.0)],
                Stroke::new(1.0_f32, WHITE),
            );
        }
        let cx = rect.center().x;
        for r in 0..=rows {
            let y = rect.min.y + rect.height() * r as f32 / rows as f32;
            painter.line_segment(
                [Pos2::new(cx - 2.0, y), Pos2::new(cx + 2.0, y)],
                Stroke::new(1.0_f32, WHITE),
            );
        }
    }

    /// A phosphor-style beam segment: 2px core plus a sparse dithered halo.
    fn beam_segment(painter: &egui::Painter, a: Pos2, b: Pos2, k: usize, decayed: bool) {
        if decayed {
            // Old trace: broken segments, like phosphor letting go.
            if k % 2 == 0 {
                painter.line_segment([a, b], Stroke::new(1.0_f32, WHITE));
            }
            return;
        }
        painter.line_segment([a, b], Stroke::new(2.0_f32, WHITE));
        let off = Vec2::new(0.0, 2.0);
        if k % 2 == 0 {
            painter.line_segment([a + off, b + off], Stroke::new(1.0_f32, WHITE));
        }
        if k % 3 == 0 {
            painter.line_segment([a - off, b - off], Stroke::new(1.0_f32, WHITE));
        }
    }

    /// The mono waveform, full width of the footer. Hidden, not gone: the
    /// sweep note reads "hide footer scope (may be remove if confirmed it
    /// looks better without)", so the panel call is out but this and the
    /// scope buffer stay until that verdict lands.
    #[allow(dead_code)]
    fn draw_wave(&self, ui: &mut egui::Ui, height: f32) {
        let (resp, painter) =
            ui.allocate_painter(Vec2::new(ui.available_width(), height), Sense::hover());
        let rect = resp.rect;
        painter.rect_stroke(rect, Rounding::ZERO, Stroke::new(2.0_f32, WHITE));
        let inner = rect.shrink(4.0);
        let painter = painter.with_clip_rect(inner);
        Self::draw_graticule(&painter, inner, 10, 4);
        if self.scope.len() > 2 {
            let n = self.scope.len();
            let mut prev: Option<Pos2> = None;
            for (i, &s) in self.scope.iter().enumerate() {
                let p = Pos2::new(
                    inner.min.x + inner.width() * i as f32 / n as f32,
                    inner.center().y - s.clamp(-1.0, 1.0) * inner.height() * 0.5,
                );
                if let Some(q) = prev {
                    Self::beam_segment(&painter, q, p, i, false);
                }
                prev = Some(p);
            }
        }
    }

    /// The phase image (L/R Lissajous, mid/side rotated): a connected beam,
    /// clipped to its box, circular whatever the box's aspect.
    fn draw_phase(&self, ui: &mut egui::Ui, height: f32) {
        let (resp, painter) =
            ui.allocate_painter(Vec2::new(ui.available_width(), height), Sense::hover());
        let rect = resp.rect;
        painter.rect_stroke(rect, Rounding::ZERO, Stroke::new(2.0_f32, WHITE));
        let inner = rect.shrink(4.0);
        let painter = painter.with_clip_rect(inner);
        Self::draw_graticule(&painter, inner, 6, 6);
        let c = inner.center();
        let scale = inner.width().min(inner.height()) * 0.5;
        let n = self.lissajous.len().max(1);
        let mut prev: Option<Pos2> = None;
        for (i, &(l, r)) in self.lissajous.iter().enumerate() {
            let x = (l - r) * std::f32::consts::FRAC_1_SQRT_2;
            let y = (l + r) * std::f32::consts::FRAC_1_SQRT_2;
            let p = c + Vec2::new(x, -y) * scale;
            if let Some(q) = prev {
                // Oldest third of the trace decays — phosphor persistence.
                Self::beam_segment(&painter, q, p, i, i < n / 3);
            }
            prev = Some(p);
        }
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

    /// A 2×2 px dot every 4 px around the rect — the pane's "dotted bevel",
    /// the *selected* state of the double-usage principle. Dots, not dashes:
    /// with feathering off a dash needs sub-pixel care, a dot is a square.
    fn dotted_rect(p: &egui::Painter, r: Rect, color: Color32) {
        const STEP: f32 = 4.0;
        const DOT: f32 = 2.0;
        let dot = |p: &egui::Painter, x: f32, y: f32| {
            p.rect_filled(
                Rect::from_min_size(Pos2::new(x.round(), y.round()), Vec2::splat(DOT)),
                Rounding::ZERO,
                color,
            );
        };
        let mut x = r.min.x;
        while x <= r.max.x - DOT {
            dot(p, x, r.min.y);
            dot(p, x, r.max.y - DOT);
            x += STEP;
        }
        let mut y = r.min.y;
        while y <= r.max.y - DOT {
            dot(p, r.min.x, y);
            dot(p, r.max.x - DOT, y);
            y += STEP;
        }
    }

    /// One control of the presets pane, in the double-usage language: plain =
    /// 1 px outline, hover = 2 px (the app-wide thicken), selected/armed =
    /// dotted bevel, and the *confirming* press inverts the box until release
    /// — the only inverted state, so a lit box always means "this fires when
    /// you let go".
    fn pane_button(ui: &mut egui::Ui, text: &str, armed: bool) -> egui::Response {
        let font = ui_font(ui.ctx(), 12.0);
        let galley = ui
            .painter()
            .layout_no_wrap(text.to_owned(), font.clone(), WHITE);
        let (rect, resp) =
            ui.allocate_exact_size(galley.size() + Vec2::new(14.0, 8.0), Sense::click());
        if ui.is_rect_visible(rect) {
            let pressed = armed && resp.is_pointer_button_down_on();
            let (bg, fg) = if pressed { (WHITE, BLACK) } else { (BLACK, WHITE) };
            let p = ui.painter();
            p.rect_filled(rect, Rounding::ZERO, bg);
            if armed {
                Self::dotted_rect(p, rect, fg);
            } else {
                let w = if resp.hovered() { 2.0_f32 } else { 1.0_f32 };
                p.rect_stroke(rect, Rounding::ZERO, Stroke::new(w, WHITE));
            }
            p.text(rect.center(), egui::Align2::CENTER_CENTER, text, font, fg);
        }
        resp
    }

    fn inverted_strip(ui: &mut egui::Ui, text: &str) {
        egui::Frame::none().fill(WHITE).inner_margin(4.0).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(egui::RichText::new(text).color(BLACK).monospace());
        });
    }

    /// A title bar with the three window buttons at its right end, so the parameter
    /// cell reads as a utility window inside the instrument — the lab-software idiom
    /// the whole panel language comes from.
    ///
    /// **Deliberately non-functional** (Billy: "minimise - fullscreen - close (not
    /// functional)"). They are set dressing, and they are drawn rather than typed:
    /// a bar, an outline and a cross are three shapes, and drawing them means they
    /// cannot fall back to a different font's idea of `—`, `□` and `✕` — which on a
    /// display face is usually a tofu box.
    fn window_chrome(ui: &mut egui::Ui, title: &str) {
        egui::Frame::none()
            .fill(WHITE)
            .inner_margin(egui::Margin::symmetric(4.0, 3.0))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(title).color(BLACK).monospace());
                    ui.with_layout(
                        egui::Layout::right_to_left(egui::Align::Center),
                        |ui| {
                            // Close, maximise, minimise — right to left, as they sit.
                            for kind in 0..3 {
                                let (resp, p) = ui.allocate_painter(
                                    Vec2::new(13.0, 11.0),
                                    Sense::hover(),
                                );
                                let r = resp.rect;
                                let s = Stroke::new(1.0_f32, BLACK);
                                p.rect_stroke(r, Rounding::ZERO, s);
                                let g = r.shrink(3.0);
                                let snap = |x: f32| x.round();
                                match kind {
                                    // Close: a cross.
                                    0 => {
                                        p.line_segment(
                                            [
                                                Pos2::new(snap(g.min.x), snap(g.min.y)),
                                                Pos2::new(snap(g.max.x), snap(g.max.y)),
                                            ],
                                            s,
                                        );
                                        p.line_segment(
                                            [
                                                Pos2::new(snap(g.max.x), snap(g.min.y)),
                                                Pos2::new(snap(g.min.x), snap(g.max.y)),
                                            ],
                                            s,
                                        );
                                    }
                                    // Maximise: an outline.
                                    1 => {
                                        p.rect_stroke(
                                            Rect::from_min_max(
                                                Pos2::new(snap(g.min.x), snap(g.min.y)),
                                                Pos2::new(snap(g.max.x), snap(g.max.y)),
                                            ),
                                            Rounding::ZERO,
                                            s,
                                        );
                                    }
                                    // Minimise: a bar on the baseline.
                                    _ => {
                                        p.line_segment(
                                            [
                                                Pos2::new(snap(g.min.x), snap(g.max.y)),
                                                Pos2::new(snap(g.max.x), snap(g.max.y)),
                                            ],
                                            s,
                                        );
                                    }
                                }
                            }
                        },
                    );
                });
            });
    }

    /// The event log, as a ticker across the header strip (Billy, 2026-08-04:
    /// "a kind of marquee box at the top"). The most recent `MARQUEE_LINES`
    /// measurements loop leftward at a constant rate, advanced in **whole
    /// pixels** — a sub-pixel scroll is a blurry scroll, which the no-AA rule
    /// forbids. The routine murmuring along the top is the same sphexish log
    /// it always was; only its furniture changed.
    fn draw_marquee(&self, painter: &egui::Painter, rect: Rect, now: f64) {
        painter.rect_stroke(rect, Rounding::ZERO, Stroke::new(1.0_f32, WHITE));
        let inner = rect.shrink(3.0);
        if inner.width() < 8.0 || self.log.is_empty() {
            return;
        }
        let text = self
            .log
            .iter()
            .take(MARQUEE_LINES)
            .cloned()
            .collect::<Vec<_>>()
            .join("   ·   ");
        let painter = painter.with_clip_rect(inner);
        let font = FontId::new(
            grid_size(11.0, native_of(MARQUEE_FACE), painter.ctx().pixels_per_point()),
            family(MARQUEE_FACE),
        );
        let galley = painter.layout_no_wrap(text, font, WHITE);
        // A short gap, not a screenful: a screenful left the band blank for a
        // third of every cycle (measured off Billy's 00:00:57 screenshot —
        // ~23 s of nothing out of 58 s). The ticker stays populated now.
        let period = (galley.size().x + 80.0).max(1.0);
        let shift = ((now * MARQUEE_PX_PER_SEC).floor() as f32).rem_euclid(period);
        let y = (inner.center().y - galley.size().y * 0.5).round();
        let x = (inner.max.x - shift).round();
        painter.galley(Pos2::new(x, y), galley.clone(), WHITE);
        painter.galley(Pos2::new(x + period, y), galley, WHITE);
    }

    /// The presets pane (Billy's sweep, 2026-08-05): a toggleable window
    /// hanging under the header chip. One text field both filters the bank
    /// live and names a save; SAVE and DELETE sit beside it; the bank lists
    /// beneath. Everything speaks the double-usage language of
    /// [`Self::pane_button`] — click a preset to highlight it, click it again
    /// to load it; ENTER selects SAVE, ENTER again writes; DELETE (its own
    /// double confirmation) removes the highlighted preset. Nothing fires on
    /// a single press.
    fn draw_presets_pane(&mut self, ctx: &egui::Context) {
        egui::Window::new("PRESETS")
            .title_bar(false)
            .resizable(false)
            .movable(false)
            .anchor(Align2::RIGHT_TOP, Vec2::new(-6.0, 34.0))
            .fixed_size(Vec2::new(340.0, 300.0))
            .frame(
                egui::Frame::none()
                    .fill(BLACK)
                    .stroke(Stroke::new(2.0_f32, WHITE))
                    .inner_margin(6.0),
            )
            .show(ctx, |ui| {
                Self::window_chrome(ui, "PRESETS");
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    let te = ui.add(
                        egui::TextEdit::singleline(&mut self.preset_name)
                            .hint_text("search / name")
                            .desired_width(190.0),
                    );
                    if te.changed() {
                        // An edited name is a new intent: disarm.
                        self.preset_armed = PaneArm::None;
                    }
                    let enter =
                        te.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                    let save =
                        Self::pane_button(ui, "SAVE", self.preset_armed == PaneArm::Save);
                    let delete =
                        Self::pane_button(ui, "DELETE", self.preset_armed == PaneArm::Delete);
                    if enter || save.clicked() {
                        let name = self.preset_name.trim().to_string();
                        if name.is_empty() {
                            self.push_log("a preset needs a name.".into());
                            self.preset_armed = PaneArm::None;
                        } else if self.preset_armed == PaneArm::Save {
                            self.preset_armed = PaneArm::None;
                            if let Some(written) = self.save_preset(&name) {
                                self.preset_selected = Some(written);
                            }
                        } else {
                            self.preset_armed = PaneArm::Save;
                        }
                        if enter {
                            // ENTER surrenders focus; take it back so the
                            // second ENTER lands in the same box.
                            te.request_focus();
                        }
                    }
                    if delete.clicked() {
                        match (self.preset_selected.clone(), self.preset_armed) {
                            (None, _) => {
                                self.push_log("no preset highlighted to delete.".into());
                                self.preset_armed = PaneArm::None;
                            }
                            (Some(name), PaneArm::Delete) => {
                                self.preset_armed = PaneArm::None;
                                self.delete_preset(&name);
                            }
                            (Some(_), _) => self.preset_armed = PaneArm::Delete,
                        }
                    }
                });
                ui.add_space(4.0);
                let query = self.preset_name.trim().to_lowercase();
                let names: Vec<String> = self
                    .preset_names
                    .iter()
                    .filter(|n| query.is_empty() || n.to_lowercase().contains(&query))
                    .cloned()
                    .collect();
                if names.is_empty() {
                    ui.label(if self.preset_names.is_empty() {
                        "no presets saved."
                    } else {
                        "nothing matches."
                    });
                }
                egui::ScrollArea::vertical().max_height(230.0).show(ui, |ui| {
                    for name in names {
                        let selected = self.preset_selected.as_deref() == Some(name.as_str());
                        if Self::pane_button(ui, &name, selected).clicked() {
                            if selected {
                                self.load_preset(&name);
                            } else {
                                self.preset_selected = Some(name.clone());
                                self.preset_armed = PaneArm::None;
                            }
                        }
                    }
                });
            });
    }

    /// How long the reveal dwells on character `i`, in seconds.
    ///
    /// Billy asked for the teletype "but humanised", and the two things that
    /// separate a person from a metronome are unevenness and punctuation: the
    /// per-character dwell is jittered ±`TYPE_JITTER` by a hash of the index,
    /// and the reveal holds after a stop the way someone drawing breath does
    /// — longest at a full stop, less at a comma or a dash. The hash means
    /// there is no clock and no RNG in it, so a given box types with exactly
    /// the same rhythm every time it comes round.
    fn char_dwell(i: usize, prev: Option<char>, console: bool) -> f32 {
        if console {
            // The entity prints. Perfectly even, and it does not pause for
            // punctuation — that absence is the tell.
            return 1.0 / CONSOLE_CPS;
        }
        let base = 1.0 / TYPE_CPS;
        let h = (i as u64).wrapping_mul(2654435761) ^ 0x9E37_79B9;
        let unit = ((h >> 13) & 0xFFFF) as f32 / 65535.0;
        let jitter = 1.0 + TYPE_JITTER * (unit * 2.0 - 1.0);
        let hold = match prev {
            Some('.') | Some('!') | Some('?') => 9.0,
            Some('\n') => 5.0,
            Some(',') | Some(';') | Some(':') | Some('—') => 4.0,
            _ => 0.0,
        };
        base * (jitter + hold)
    }

    /// One step of the voice's own PRNG. Deterministic within a session but
    /// seeded from the clock at startup, so the log does not recite itself in the
    /// same order every launch. This is the *only* nondeterminism in the program
    /// and it cannot reach the audio path — the engine's bit-identical contract
    /// is untouched.
    fn roll(&mut self) -> u32 {
        let mut x = self.voice_rng;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.voice_rng = x;
        x
    }

    /// Choose the next thing to speak.
    ///
    /// Two sources of interruption, per Billy. **Provoked**: once agitation
    /// crosses `AGITATE_INTRUDE` the entity speaks for itself, and doing so
    /// spends part of the charge. **Decay**: a small standing chance of an
    /// intrusion regardless, because the interface itself is failing — which is
    /// what implies how long this thing has been sitting here. Otherwise a human
    /// log, weighted by rarity.
    fn pick_relic(&mut self) {
        if self.relics.is_empty() {
            return;
        }
        let chance = intrude_chance(self.agitation);
        let want_intrusion = (self.roll() % 1000) as f32 / 1000.0 < chance;

        let pool: Vec<usize> = self
            .relics
            .iter()
            .enumerate()
            .filter(|(_, r)| {
                if want_intrusion {
                    r.is_pure_intrusion()
                } else {
                    !r.log.is_empty()
                }
            })
            .map(|(i, _)| i)
            .collect();
        let pool = if pool.is_empty() {
            (0..self.relics.len()).collect::<Vec<_>>()
        } else {
            pool
        };

        // Rarity-weighted draw. Intrusions are all very_rare, so among
        // themselves the weighting is flat and this reduces to a fair pick.
        let total: u32 = pool
            .iter()
            .map(|&i| rarity_weight(&self.relics[i].rarity))
            .sum();
        let mut ticket = self.roll() % total.max(1);
        let mut chosen = pool[0];
        for &i in &pool {
            let w = rarity_weight(&self.relics[i].rarity);
            if ticket < w {
                chosen = i;
                break;
            }
            ticket -= w;
        }
        // Don't repeat the entry that just spoke if there is any alternative.
        if chosen == self.relic && pool.len() > 1 {
            let alt = pool[(self.roll() as usize) % pool.len()];
            if alt != chosen {
                chosen = alt;
            }
        }
        self.relic = chosen;
        // A relic carrying both — MycelliAI — has its log cut short by its own
        // intrusion, but only when the entity is already agitated.
        self.relic_interrupted = self.agitation >= AGITATE_INTERRUPT
            && !self.relics[chosen].intrusion.is_empty()
            && !self.relics[chosen].log.is_empty();
    }

    /// The text the current relic is saying, and whether the entity is the one
    /// saying it. An interrupted log is truncated mid-thought and the intrusion
    /// lands on top of it.
    fn voice_text(&self) -> (String, bool) {
        let Some(r) = self.relics.get(self.relic) else {
            return (String::new(), false);
        };
        if r.is_pure_intrusion() {
            return (r.intrusion.join("\n"), true);
        }
        if self.relic_interrupted {
            let keep = (r.log.len() / 2).max(1);
            let mut lines: Vec<String> = r.log[..keep].to_vec();
            // Cut the last surviving line off mid-sentence: the entity does not
            // wait for a full stop.
            if let Some(last) = lines.last_mut() {
                let cut = (last.chars().count() * 2 / 3).max(8);
                *last = last.chars().take(cut).collect::<String>() + "—";
            }
            lines.extend(r.intrusion.iter().cloned());
            return (lines.join("\n"), true);
        }
        (r.log.join("\n"), false)
    }

    /// Cell Z: the record's header for whoever is speaking in W.
    ///
    /// Reads the same relic the voice box reads, so the card and the words can never
    /// disagree about who this is — including the face, which comes from the same
    /// archetype mapping. The card is the only place the log's own bookkeeping is
    /// visible: `id`, `rarity` and the metadata were parsed from the first version
    /// of the file and had nowhere to go until now.
    fn draw_record(&self, ui: &mut egui::Ui) {
        let Some(r) = self.relics.get(self.relic) else {
            return;
        };
        Self::inverted_strip(ui, &format!("RELIC {}", r.id));
        ui.add_space(6.0);

        // The speaker's name in the speaker's own face, and left exactly as Billy
        // wrote it: the archetype is his text, so the program does not get to
        // uppercase it into a label.
        let face = face_for_archetype(&r.archetype);
        ui.label(
            egui::RichText::new(&r.archetype).color(WHITE).font(FontId::new(
                grid_size(RECORD_NAME_PX, native_of(face), ui.ctx().pixels_per_point()),
                family(face),
            )),
        );
        ui.add_space(5.0);

        for (label, value) in record_rows(r) {
            ui.label(format!("{label:<RECORD_LABEL_W$}{value}"));
        }

        ui.add_space(5.0);
        ui.horizontal(|ui| {
            ui.label(format!("{:<RECORD_LABEL_W$}", "RARITY"));
            let filled = rarity_weight(&r.rarity);
            let (resp, painter) = ui.allocate_painter(
                Vec2::new(
                    RARITY_SLOTS as f32 * (PIP_PX + PIP_GAP),
                    PIP_PX + 2.0,
                ),
                Sense::hover(),
            );
            let y = (resp.rect.center().y - PIP_PX * 0.5).round();
            for i in 0..RARITY_SLOTS {
                let x = (resp.rect.min.x + i as f32 * (PIP_PX + PIP_GAP)).round();
                let pip = Rect::from_min_size(Pos2::new(x, y), Vec2::splat(PIP_PX));
                if i < filled {
                    painter.rect_filled(pip, Rounding::ZERO, WHITE);
                } else {
                    painter.rect_stroke(pip, Rounding::ZERO, Stroke::new(1.0_f32, WHITE));
                }
            }
            ui.label(&r.rarity);
        });

        // Same rule as the parameter cell: what is left over becomes ornament rather
        // than void, at a fixed low density that carries no measurement.
        let rest = ui.available_size();
        if rest.y > 10.0 {
            let (resp, painter) = ui.allocate_painter(rest, Sense::hover());
            dither_rect(&painter, resp.rect, 0.12, 3.0);
        }
    }

    /// The voice cell: the box types itself out, then holds. Clicking skips a
    /// reveal in progress, or advances to the next box once it has finished.
    fn draw_voice(&mut self, ui: &mut egui::Ui, now: f64, dt: f32) {
        if self.relics.is_empty() {
            return;
        }
        let (selected, is_intrusion) = self.voice_text();
        if selected.is_empty() {
            return;
        }
        // Any change of box — advance, or Billy editing the file under us —
        // restarts the reveal from the first character.
        if selected != self.voice_typing {
            self.voice_typing = selected;
            self.voice_revealed = 0;
            self.voice_credit = 0.0;
        }
        // Each witness speaks in their own face. It is the one piece of characterisation
        // a strict 1-bit interface can carry without a single new pixel of chrome, and
        // it costs nothing at runtime: egui builds an atlas per family the first time
        // one is asked for, and there are eleven.
        let face = self
            .relics
            .get(self.relic)
            .map(|r| face_for_archetype(&r.archetype))
            .unwrap_or(UI_FACE);
        let font = self.voice_font(ui, face);
        let chars: Vec<char> = self.voice_typing.chars().collect();
        self.voice_credit += dt;
        while self.voice_revealed < chars.len() {
            let prev = self.voice_revealed.checked_sub(1).map(|k| chars[k]);
            let cost = Self::char_dwell(self.voice_revealed, prev, is_intrusion);
            if self.voice_credit < cost {
                break;
            }
            self.voice_credit -= cost;
            self.voice_revealed += 1;
        }
        let complete = self.voice_revealed >= chars.len();
        if complete && now - self.voice_last_advance > 45.0 {
            self.voice_last_advance = now;
            self.pick_relic();
        } else if !complete {
            // The hold timer starts when the last character lands, not when
            // the box was selected — a long paragraph gets its full 45 s.
            self.voice_last_advance = now;
        }
        let visible: String = chars[..self.voice_revealed].iter().collect();
        // **The box does not answer the mouse** (Billy, 2026-08-05). Clicking used
        // to generate the next entry, which made the cell a page-turner; the log now
        // advances only on its own 45 s hold, so it reads as a record playing rather
        // than something being flicked through. Nothing here takes a `Sense`, so a
        // click passes straight through to whatever is behind it — which is nothing.
        //
        // The entity speaks in inverted type. In strict 1-bit that is the whole
        // available vocabulary for "this is not a person talking", and it is the
        // same inversion the panel headers use, so it reads as the interface
        // itself seizing the channel.
        if is_intrusion {
            let rect = ui.max_rect();
            ui.painter().rect_filled(rect, Rounding::ZERO, WHITE);
            ui.label(egui::RichText::new(visible).color(BLACK).font(font));
        } else {
            ui.label(egui::RichText::new(visible).color(WHITE).font(font));
        }
    }

    /// The largest size on the ladder that the current entry fits the cell at.
    ///
    /// Measured against the **finished** text, not the part revealed so far — sizing
    /// on the visible prefix would start every box at 32 px and shrink it mid-
    /// sentence as the reveal outgrew the cell. The size is a property of the entry,
    /// so it is settled before the first character lands and does not move again.
    fn voice_font(&self, ui: &egui::Ui, face: &str) -> FontId {
        let native = native_of(face);
        let ppp = ui.ctx().pixels_per_point();
        let avail = ui.max_rect().size();
        let mut smallest = None;
        for want in VOICE_PX {
            let id = FontId::new(grid_size(want, native, ppp), family(face));
            let height = ui.ctx().fonts(|f| {
                f.layout(self.voice_typing.clone(), id.clone(), WHITE, avail.x)
                    .size()
                    .y
            });
            if height <= avail.y {
                return id;
            }
            smallest = Some(id);
        }
        // Nothing on the ladder fits: the entry is simply long. The bottom rung is
        // still the right answer, and it is what the box used before there was a
        // ladder at all.
        smallest.unwrap_or_else(|| FontId::new(grid_size(16.0, native, ppp), family(face)))
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
        // The reveal's own timebase. Clamped so a stalled frame (a resize, a
        // device change) doesn't dump a paragraph out at once.
        let dt = if self.last_frame_time > 0.0 {
            ((now - self.last_frame_time) as f32).clamp(0.0, 0.1)
        } else {
            0.0
        };
        self.last_frame_time = now;
        // The shell's ripple phases are **integrated**, not derived from
        // `elapsed × rate`. Multiplying accumulated time by a rate makes the
        // phase jump whenever the rate changes, by an amount proportional to how
        // long the app has been running: moving the Rip slider snapped the whole
        // shell, and the snap grew worse all session. Accumulating `dt × rate`
        // instead means a rate change alters the speed from here on and nothing
        // else. Wrapped, so f32 keeps its precision over a long session.
        let rate = 1.0 + AGITATE_SPEED * self.shadow.rip.clamp(0.0, 1.0);
        let tau = std::f32::consts::TAU;
        self.ripple_phase =
            (self.ripple_phase + dt * RIPPLE_SPEED as f32 * rate).rem_euclid(tau);
        self.suture_phase =
            (self.suture_phase + dt * SUTURE_SPEED as f32 * rate).rem_euclid(tau);
        // The shell's scale chases `master` through a long one-pole. Exponential
        // against real elapsed time, so the swell runs at the same speed whatever
        // the frame rate is doing.
        let k = 1.0 - (-dt / SCALE_SMOOTH_S).exp();
        self.shell_scale += (scale_for(self.shadow.master_level) - self.shell_scale) * k;
        // The rock, integrated for the same reason the ripple is — and divided by
        // the scale, so a bigger shell drifts more slowly. Periods are
        // `ROCK_PERIOD_S / φ^i`, which is why the phases advance at φ^i × the base
        // rate.
        let scale = self.shell_scale.max(0.05);
        for (i, p) in self.rock_phase.iter_mut().enumerate() {
            let rate = (std::f64::consts::TAU / ROCK_PERIOD_S) as f32
                * PHI.powi(i as i32)
                / scale;
            *p = (*p + dt * rate).rem_euclid(tau);
        }
        // The spin: a slow continuous turn in one direction, also slowed by
        // scale. An oscillating tilt of a few degrees is nearly invisible on a
        // form this close to rotationally symmetric — a spiral leaned 4° looks
        // like the same spiral. A turn that keeps going does not (Billy: "shell
        // also appears to not be rotating").
        self.spin = (self.spin + dt * (tau / ROCK_SPIN_S as f32) / scale).rem_euclid(tau);
        // INDEX opens and closes the umbilicus; smoothed so it dilates.
        let ki = 1.0 - (-dt / INDEX_SMOOTH_S).exp();
        self.index_smooth += (self.shadow.index.clamp(0.0, 1.0) - self.index_smooth) * ki;
        self.update_dread(now);
        // Everything the critical band drives ramps through this.
        let kd = 1.0 - (-dt / DREAD_SMOOTH_S).exp();
        let target = if self.dread == Dread::Critical { 1.0 } else { 0.0 };
        self.dread_level += (target - self.dread_level) * kd;
        // Agitation: a leaky integral of how far the player is subverting the
        // law, filling over ~20 s and draining over ~45. Dread is the instant,
        // this is the history — and it drains when the player relents, which is
        // the forgiveness rule the design asks for.
        let subversion = self.shadow.rip.max(self.shadow_verb.haunt).clamp(0.0, 1.0);
        self.agitation = (self.agitation
            + dt * (subversion / AGITATE_RISE_S - self.agitation / AGITATE_FALL_S))
            .clamp(0.0, 1.0);
        // A change of speaker reloads the plinth at once, rather than waiting for
        // the asset poll.
        let avatar = self
            .relics
            .get(self.relic)
            .map(|r| r.avatar.clone())
            .unwrap_or_default();
        if avatar != self.portrait_avatar {
            self.portrait_avatar = avatar;
            self.refresh_art(ctx);
        }

        // Hot-reload Billy's relic log every ~2 s, and Space Z's art with it.
        if now - self.voice_last_reload > 2.0 {
            self.voice_last_reload = now;
            let relics = load_relics();
            if !relics.is_empty() {
                if relics.len() != self.relics.len() {
                    self.relic = self.relic.min(relics.len() - 1);
                }
                self.relics = relics;
            }
            self.refresh_art(ctx);
        }

        egui::TopBottomPanel::top("title").show(ctx, |ui| {
            ui.horizontal(|ui| {
                let t = self.start.elapsed().as_secs();
                Self::header_chip(ui, &format!("{:02}:{:02}:{:02}", t / 3600, t / 60 % 60, t % 60));
                ui.label(
                    egui::RichText::new("BLOW YOUR PHASE OFF")
                        .font(ui_font(ui.ctx(), 16.0))
                        .color(WHITE),
                );
                // Everything to the right of the title is placed first, so the
                // marquee can have exactly the gap that survives it.
                let gap_start = ui.cursor().min.x;
                let right = ui
                    .with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        Self::header_chip(ui, concat!("v", env!("CARGO_PKG_VERSION")));
                        Self::header_chip(ui, &format!("{:.0}k", self.rig.sample_rate / 1000.0));
                        if self.rig.midi_connected {
                            Self::header_chip(ui, "midi");
                        }
                        ui.label(
                            egui::RichText::new(&self.rig.device_name)
                                .color(WHITE)
                                .font(ui_font(ui.ctx(), 11.0)),
                        );
                        // The presets chip: the one functional control in the
                        // header, inverted while its pane hangs open.
                        let open = self.presets_open;
                        let chip = ui.add(
                            egui::Button::new(
                                egui::RichText::new("presets")
                                    .color(if open { BLACK } else { WHITE }),
                            )
                            .fill(if open { WHITE } else { BLACK })
                            .stroke(Stroke::new(1.0_f32, WHITE)),
                        );
                        if chip.clicked() {
                            self.presets_open = !self.presets_open;
                        }
                    })
                    .response
                    .rect;
                let row = ui.min_rect();
                let band = Rect::from_min_max(
                    Pos2::new(gap_start + CELL_GUTTER, row.min.y),
                    Pos2::new(right.min.x - CELL_GUTTER, row.max.y),
                );
                if band.width() > 40.0 {
                    self.draw_marquee(ui.painter(), band, now);
                }
            });
        });

        if self.presets_open {
            self.draw_presets_pane(ctx);
        }

        egui::SidePanel::left("stats").exact_width(195.0).show(ctx, |ui| {
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
            let mut toggle_picker = false;
            let mut open_picker = false;
            // The switch and its hold source share a row: the source belongs
            // to the S&H, and on its own line it left a hole (Billy: "move
            // golden and random together theres empty space").
            ui.horizontal(|ui| {
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
            ui.horizontal_wrapped(|ui| {
                for tuning in Tuning::ALL {
                    let active = m.tuning == tuning;
                    let b = egui::Button::new(
                        egui::RichText::new(tuning.name())
                            .color(if active { BLACK } else { WHITE }),
                    )
                    .fill(if active { WHITE } else { BLACK });
                    if ui.add(b).clicked() {
                        // A second press on an already-active SCALE flips the
                        // bank's visibility and nothing else (Billy's sweep:
                        // "by itself it will do nothing else") — pick a
                        // scale, toggle the bank away, get the phase back.
                        if tuning == Tuning::Scale && active {
                            toggle_picker = true;
                        } else if !active {
                            m.tuning = tuning;
                            changed = true;
                            if tuning == Tuning::Scale {
                                open_picker = true;
                            }
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
                }
            });
            if toggle_picker {
                self.scale_picker_open = !self.scale_picker_open;
            }
            if open_picker {
                self.scale_picker_open = true;
            }
            if m.tuning == Tuning::Scale && self.scale_picker_open {
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
            if changed {
                self.send_melody();
            }
            if let Some(line) = log_line {
                self.push_log(line);
            }
            // The phase image lives under the melody parameters (Billy, 2026-08-04).
            // It was under the presets in the right panel, where the app-wide padding
            // pushed it past the bottom edge and it silently stopped drawing — a
            // "draw it in whatever height is left" panel is one layout change away
            // from having none left. It is the panel's *last* element, so all
            // remaining height is its by construction — clamping it square left a
            // dead band under the box at full screen (Billy's sweep, 2026-08-05:
            // "center it, let it grow on the Y axis"). The beam stays circular and
            // centered whatever the box's aspect; the graticule fills the box.
            ui.add_space(CELL_GUTTER);
            let h = ui.available_height() - 22.0;
            if h > 40.0 {
                Self::inverted_strip(ui, "PHASE");
                self.draw_phase(ui, h);
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
            let t = self.tremble();
            let v = &mut self.shadow_verb;
            let mut verb_changed = false;
            verb_changed |= ui.add(egui::Slider::new(&mut v.mix, 0.0..=1.0).text("mix")).changed();
            verb_changed |= ui.add(egui::Slider::new(&mut v.ghost, 0.0..=1.0).text("ghost")).changed();
            // Honest range with log travel: rt60_probe (2026-08-05) measured
            // real decay saturating near 7 s — the old 20 s top was two-thirds
            // lie. Log spacing gives the low seconds the granularity Billy
            // asked for; equal knob distance = equal decay ratio.
            verb_changed |= ui
                .add(
                    egui::Slider::new(&mut v.rt60, 0.05..=8.0)
                        .logarithmic(true)
                        .text("rt60 s"),
                )
                .changed();
            verb_changed |= ui.add(egui::Slider::new(&mut v.damp, 0.0..=0.99).text("damp")).changed();
            verb_changed |= ui
                .horizontal(|ui| {
                    // Rounded: a fractional offset would put the widget on a
                    // half-pixel, and feathering is off.
                    ui.add_space(2.0 + t.round());
                    ui.add(egui::Slider::new(&mut v.haunt, 0.0..=1.0).text("haunt"))
                })
                .inner
                .changed();
            if verb_changed {
                self.send_verb();
            }
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
            ui.add_space(CELL_GUTTER);
            // Four cells, each in its own hard frame (Billy's annotation:
            // "seperate each element in the middle"). Both cuts are golden:
            // the top row takes the major section of the height and the left
            // column the minor section of the width — floored at the width the
            // sliders actually need, because a golden proportion that clips a
            // control is just a clipped control.
            let body = ui.available_rect_before_wrap();
            let col_w = (body.width() * (1.0 - GOLDEN_MAJOR))
                .max(CONTROL_COL_MIN)
                .min(body.width() * 0.6)
                .floor();
            let row_h = (body.height() * GOLDEN_MAJOR).floor();
            // The resize-down priorities (see the constants): the bottom row
            // yields first, the shell's cell second, the controls never.
            let show_bottom = body.height() - row_h >= BOTTOM_ROW_MIN_H;
            let show_shell = body.width() - col_w >= SHELL_CELL_MIN_W;
            let split_x = body.min.x + col_w;
            let split_y = body.min.y + row_h;
            let g = CELL_GUTTER;
            // X: controls. Y: the Logalith. Z: the portrait. W: the voice.
            // A hidden neighbour donates its space: no shell widens X, no
            // bottom row deepens X and Y.
            let x_max = Pos2::new(
                if show_shell { split_x - g } else { body.max.x },
                if show_bottom { split_y - g } else { body.max.y },
            );
            let cell_x = Rect::from_min_max(body.min, x_max);
            let cell_y = Rect::from_min_max(
                Pos2::new(split_x, body.min.y),
                Pos2::new(body.max.x, x_max.y),
            );
            let cell_z = Rect::from_min_max(
                Pos2::new(body.min.x, split_y),
                Pos2::new(split_x - g, body.max.y),
            );
            let cell_w = Rect::from_min_max(Pos2::new(split_x, split_y), body.max);
            let mut cells = vec![cell_x];
            if show_shell {
                cells.push(cell_y);
            }
            if show_bottom {
                cells.push(cell_z);
                cells.push(cell_w);
            }
            for cell in cells {
                ui.painter()
                    .rect_stroke(cell, Rounding::ZERO, Stroke::new(2.0_f32, WHITE));
            }
            if show_shell {
                // Sky, then the entity over it — both from one pose, so the
                // field's lensing bends around the shell that actually gets drawn.
                let stage = cell_y.shrink(4.0);
                let sky = ui.painter().with_clip_rect(cell_y.shrink(3.0));
                let pose = self.shell_pose(stage);
                self.draw_starfield(&sky, stage, &pose);
                self.draw_monolith(&sky, stage, &pose);
            }
            // The panels are content-sized, so the only honest answer to "how big
            // can the art be" is the running app's own measurement. Reported to the
            // log on startup and on any resize (only while the cell exists —
            // a hidden cell has no figure to report).
            if show_bottom {
                let z = cell_z.shrink(4.0).size();
                let area = (z.x.round() as i32, z.y.round() as i32);
                if self.portrait_area != area {
                    self.portrait_area = area;
                    self.push_log(format!(
                        "cell z {}x{} px. aspect {:.2}.",
                        area.0,
                        area.1,
                        area.0 as f32 / area.1.max(1) as f32
                    ));
                }
            }
            // Cell Z is the record card, across the whole zone (Billy, 2026-08-05:
            // "its coverage can be the whole of that zblock"). The plinth shared it
            // for about an hour; the portrait work is parked, and the drawing side
            // of it — scanline field, point cloud, the turning mark — is in history
            // at `52c5a99` and was restored from there once already, so getting it
            // back is a known move rather than a rewrite. What stays here is the
            // loading side: the grids, the parser and its format are Billy's assets
            // and are correct whether or not anything draws them today.
            if show_bottom {
                ui.allocate_new_ui(
                    egui::UiBuilder::new().max_rect(cell_z.shrink(8.0)),
                    |ui| self.draw_record(ui),
                );
                ui.allocate_new_ui(
                    egui::UiBuilder::new().max_rect(cell_w.shrink(8.0)),
                    |ui| self.draw_voice(ui, now, dt),
                );
            }
            ui.allocate_new_ui(
                egui::UiBuilder::new().max_rect(cell_x.shrink(8.0)),
                |ui| {
                    ui.style_mut().spacing.slider_width =
                        (cell_x.width() - 175.0).clamp(90.0, 220.0);
                    // The cell keeps its size; the chrome takes its space out of the
                    // dither filler at the bottom, which is ornament and has none to
                    // lose.
                    Self::window_chrome(ui, "PARAMETERS");
                    ui.add_space(2.0);
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
            // The INDEX knob's taper (README §8): travel is position^φ, which
            // exactly linearizes the first-arriving response — feedback's
            // concave x^(1/φ) — in knob travel. The box shows the true index
            // (what response() receives) to 0.001; typing inverts the taper.
            // Position is recomputed from the index each frame rather than
            // stored, and written back only on interaction, so the powf
            // round-trip can never walk the patch on its own.
            let mut pos = self.shadow.index.clamp(0.0, 1.0).powf(1.0 / PHI);
            let r = ui.add(
                egui::Slider::new(&mut pos, 0.0..=1.0)
                    .text("INDEX")
                    .custom_formatter(|p, _| format!("{:.3}", p.powf(PHI as f64)))
                    .custom_parser(|s| {
                        s.parse::<f64>()
                            .ok()
                            .map(|v| v.clamp(0.0, 1.0).powf(1.0 / PHI as f64))
                    })
                    .drag_value_speed(0.001),
            );
            if r.changed() {
                self.shadow.index = pos.powf(PHI);
            }
            patch_changed |= r.changed();
            if r.drag_stopped() {
                log_line = Some(format!(
                    "index {:.3}. modulator response x^(φ^(depth−2)), knob x^φ.",
                    self.shadow.index
                ));
            }
            let t = self.tremble();
            let r = ui
                .horizontal(|ui| {
                    // Rounded: a fractional offset would put the widget on a
                    // half-pixel, and feathering is off.
                    ui.add_space(2.0 + t.round());
                    ui.add(egui::Slider::new(&mut self.shadow.rip, 0.0..=1.0).text("RIP"))
                })
                .inner;
            patch_changed |= r.changed();
            if r.drag_stopped() {
                log_line = Some(format!(
                    "rip {:.2}. carriers modulated by their own past: 46.9 ms, 36°/pass.",
                    self.shadow.rip
                ));
            }
            patch_changed |= ui
                .add(egui::Slider::new(&mut self.shadow.feedback, 0.0..=1.0).text("fb"))
                .changed();
            patch_changed |= ui
                .add(egui::Slider::new(&mut self.shadow.master_level, 0.0..=1.0).text("master"))
                .changed();
            // The GLIDE knob's taper (README §6): seconds = 2·position^φ⁴.
            // Past 0.1 s a glide is already outside normal portamento scope,
            // so ~65% of the travel serves 0–0.1 s (the bottom half covers
            // 0–17 ms) while the full 2 s stays at the top. Same stateless
            // position round-trip as INDEX: derived each frame, written back
            // only on interaction.
            let glide_exp = PHI * PHI * PHI * PHI;
            let mut glide_pos =
                (self.shadow.glide_seconds.clamp(0.0, 2.0) / 2.0).powf(1.0 / glide_exp);
            let r = ui.add(
                egui::Slider::new(&mut glide_pos, 0.0..=1.0)
                    .text("glide s")
                    .custom_formatter(move |p, _| {
                        format!("{:.3}", 2.0 * p.powf(glide_exp as f64))
                    })
                    .custom_parser(move |s| {
                        s.parse::<f64>()
                            .ok()
                            .map(|v| (v.clamp(0.0, 2.0) / 2.0).powf(1.0 / glide_exp as f64))
                    })
                    .drag_value_speed(0.001),
            );
            if r.changed() {
                self.shadow.glide_seconds = 2.0 * glide_pos.powf(glide_exp);
            }
            patch_changed |= r.changed();
            let resp = ui.add(
                egui::Slider::new(&mut self.drone_hz, 27.5..=440.0)
                    .logarithmic(true)
                    .text("drone hz"),
            );
            if resp.changed() {
                let _ = self.rig.ctrl_tx.push(Event::GlideTo(self.drone_hz));
            }
            if patch_changed {
                self.send_patch();
            }
            if let Some(line) = log_line {
                self.push_log(line);
            }
                    // Marginalia (design pass item 7): the real models behind
                    // the sliders above, in the annotated-notebook register.
                    // Every line is checkable against README's mathematical
                    // models — this is measurement, not wallpaper, and it is
                    // what fills the space Billy flagged as empty.
                    ui.add_space(8.0);
                    let compiled = compile(self.shadow.algorithm);
                    for line in [
                        format!("index   x^(φ^(d−2))   d=2,3,4 → 1  {:.3}  {:.3}", PHI, PHI * PHI),
                        format!(
                            "levels  1/φ^(d−1)     → {:.3}  {:.3}  {:.3}",
                            1.0 / PHI,
                            1.0 / (PHI * PHI),
                            1.0 / (PHI * PHI * PHI)
                        ),
                        "rip     φ×29 ms = 46.9 ms, π/5 per pass".to_string(),
                        "fb      avg(y₋₁, y₋₂) → π rad at fb 1".to_string(),
                        format!("mix     mean of {} carrier(s)", compiled.carrier_count),
                        format!(
                            "master  x^φ → gain {:.3}, one-pole 13 ms",
                            master_gain(self.shadow.master_level)
                        ),
                        format!(
                            "glide   one-pole, {:.3} s → {:.1} hz",
                            self.shadow.glide_seconds, self.drone_hz
                        ),
                    ] {
                        ui.label(
                            egui::RichText::new(line)
                                .font(ui_font(ui.ctx(), 10.0))
                                .color(WHITE),
                        );
                    }
                    // Whatever is still left becomes ornament rather than void
                    // (Billy: "anything but large areas of nothing"). Fixed
                    // low density — it carries no measurement and must never
                    // be mistaken for one.
                    let rest = ui.available_size();
                    if rest.y > 10.0 {
                        let (resp, painter) = ui.allocate_painter(rest, Sense::hover());
                        dither_rect(&painter, resp.rect, 0.12, 3.0);
                    }
                },
            );
        });
    }
}

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size(egui::vec2(1180.0, 780.0))
            // The floor under the resize-down priorities: below this the
            // control column itself would clip, and a clipped control is a
            // broken control.
            .with_min_inner_size(egui::vec2(800.0, 600.0))
            .with_title("BLOW YOUR PHASE OFF"),
        ..Default::default()
    };
    eframe::run_native(
        "BLOW YOUR PHASE OFF",
        options,
        Box::new(|cc| {
            install_fonts(&cc.egui_ctx);
            one_bit_style(&cc.egui_ctx);
            match App::new() {
                Ok(app) => Ok(Box::new(app) as Box<dyn eframe::App>),
                Err(e) => Err(format!("{e:#}").into()),
            }
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use fibonacci_dsp::PLASTIC;

    const TAU: f32 = std::f32::consts::TAU;

    /// The chamber counts are the documented integer sequences, and each converges
    /// on its own mode's constant. This is the shell's claim about what it is; if
    /// it drifts, the shell is lying.
    #[test]
    fn chamber_sequences_converge_on_their_modes_constant() {
        assert_eq!(chambers_for(RatioMode::Harmonic), [30, 24, 18, 12, 6]);
        assert_eq!(chambers_for(RatioMode::Fibonacci), [34, 21, 13, 8, 5]);
        assert_eq!(chambers_for(RatioMode::Golden), [29, 18, 11, 7, 4]);
        assert_eq!(chambers_for(RatioMode::GoldenMirror), [4, 7, 11, 18, 29]);
        assert_eq!(chambers_for(RatioMode::Plastic), [28, 21, 16, 12, 9]);

        // Fibonacci and Lucas both converge on phi; Padovan on the plastic number.
        let ratio = |c: [u32; SHELL_WHORLS]| c[0] as f32 / c[1] as f32;
        assert!((ratio(chambers_for(RatioMode::Fibonacci)) - PHI).abs() < 0.03);
        assert!((ratio(chambers_for(RatioMode::Golden)) - PHI).abs() < 0.03);
        assert!((ratio(chambers_for(RatioMode::Plastic)) - PLASTIC).abs() < 0.02);
        // Harmonic is the harmonic series, so consecutive terms sit at 5:4.
        assert!((ratio(chambers_for(RatioMode::Harmonic)) - 1.25).abs() < 1e-6);
        // Mirror is Golden reversed, exactly.
        let mut g = chambers_for(RatioMode::Golden);
        g.reverse();
        assert_eq!(g, chambers_for(RatioMode::GoldenMirror));
    }

    /// Every whorl in a mode carries a *different* number of chambers, so no two
    /// whorls share an angular division and the per-operator assignment cannot
    /// line up into radial stripes. Counts are also strictly monotonic, so the
    /// shell reads as opening out (or closing in, for mirror) rather than as an
    /// arbitrary set.
    ///
    /// This test previously asserted that no count was a multiple of `NUM_OPS`,
    /// which was simply false — harmonic's 30 and fibonacci's 5 both are — and was
    /// not the property that mattered anyway. Chambers are numbered by a running
    /// counter across the whole shell, so what would produce a stripe is two whorls
    /// sharing both a count and a starting operator, not a count being divisible.
    #[test]
    fn chamber_counts_never_repeat_within_a_mode() {
        for mode in RatioMode::ALL {
            let c = chambers_for(mode);
            for i in 0..c.len() {
                for j in i + 1..c.len() {
                    assert!(
                        c[i] != c[j],
                        "{}: whorls {} and {} both have {} chambers",
                        mode_name(mode),
                        i,
                        j,
                        c[i]
                    );
                }
            }
            let ascending = c.windows(2).all(|w| w[0] < w[1]);
            let descending = c.windows(2).all(|w| w[0] > w[1]);
            assert!(
                ascending || descending,
                "{}: {:?} is neither opening out nor closing in",
                mode_name(mode),
                c
            );
            // Any whorl with at least NUM_OPS chambers must give every operator a
            // chamber, or an operator would be invisible on that whorl.
            let mut index = 0u32;
            for &n in c.iter() {
                let mut seen = [false; NUM_OPS];
                for _ in 0..n {
                    seen[(index % NUM_OPS as u32) as usize] = true;
                    index += 1;
                }
                if n >= NUM_OPS as u32 {
                    assert!(
                        seen.iter().all(|&s| s),
                        "{}: a whorl of {} chambers missed an operator",
                        mode_name(mode),
                        n
                    );
                }
            }
        }
    }

    /// The shell doubles every whorl, exactly, and reaches 1.0 at its mouth.
    #[test]
    fn growth_is_a_doubling_per_whorl() {
        let theta_max = SHELL_TURNS * TAU;
        assert!((shell_unit_r(theta_max, 0.0, 0.0) - 1.0).abs() < 1e-5);
        for k in 1..=SHELL_WHORLS {
            let expected = 1.0 / SHELL_GROWTH.powi(k as i32);
            let got = shell_unit_r(theta_max - k as f32 * TAU, 0.0, 0.0);
            assert!(
                (got - expected).abs() < 1e-5,
                "whorl {k}: expected {expected}, got {got}"
            );
        }
    }

    /// The suture's undulation stays inside its stated bound, so the silhouette
    /// measurement — which inflates the smooth spiral by SUTURE_WAVE rather than
    /// tracking the moving curve — really does contain it.
    #[test]
    fn suture_undulation_is_bounded() {
        let theta_max = SHELL_TURNS * TAU;
        let smooth = |t: f32| SHELL_GROWTH.powf((t - theta_max) / TAU);
        for i in 0..2000 {
            let t = theta_max * i as f32 / 2000.0;
            for ph in [0.0, 1.0, 2.5, 4.0] {
                let ratio = shell_unit_r(t, ph, SUTURE_CYCLES_OPEN) / smooth(t);
                assert!(
                    ratio <= 1.0 + SUTURE_WAVE + 1e-4 && ratio >= 1.0 - SUTURE_WAVE - 1e-4,
                    "undulation {ratio} outside +-{SUTURE_WAVE}"
                );
            }
        }
    }

    /// A spiral's silhouette is not centred on its pole. That is the defect that
    /// once threw the shell into a corner, so the offset is asserted to be real and
    /// the bounds to actually contain the curve.
    #[test]
    fn silhouette_is_offset_from_the_pole_and_contains_the_curve() {
        let (lo, hi) = shell_unit_bounds();
        let mid = (lo + hi) * 0.5;
        assert!(
            mid.length() > 0.05,
            "silhouette centre sits {mid:?} from the pole — if this is ~0 the \
             centring correction is no longer needed and should be removed"
        );
        let theta_max = SHELL_TURNS * TAU;
        for i in 0..4000 {
            let t = theta_max * i as f32 / 4000.0;
            let r = shell_unit_r(t, 1.7, SUTURE_CYCLES_OPEN);
            let p = Vec2::new(t.cos(), t.sin()) * r;
            assert!(
                p.x >= lo.x - 1e-3
                    && p.x <= hi.x + 1e-3
                    && p.y >= lo.y - 1e-3
                    && p.y <= hi.y + 1e-3,
                "point {p:?} escapes bounds {lo:?}..{hi:?}"
            );
        }
    }

    /// Master maps to scale monotonically across the documented range, and the
    /// ceiling is above 1.0 — the shell is *meant* to overrun its panel.
    #[test]
    fn scale_spans_its_range_monotonically() {
        assert!((scale_for(0.0) - SCALE_MIN).abs() < 1e-6);
        assert!((scale_for(1.0) - SCALE_MAX).abs() < 1e-6);
        assert!(SCALE_MAX > 1.0, "the ceiling must overrun the fit");
        assert!(SCALE_MIN > 0.0, "silence must shrink the shell, not erase it");
        let mut prev = f32::MIN;
        for i in 0..=20 {
            let s = scale_for(i as f32 / 20.0);
            assert!(s > prev);
            prev = s;
        }
        // Out-of-range input is clamped, never extrapolated.
        assert_eq!(scale_for(-1.0), scale_for(0.0));
        assert_eq!(scale_for(9.0), scale_for(1.0));
    }

    /// **The invariant that keeps the shell legible.** Neighbouring ribs sit a
    /// fraction of a chamber apart and carry the same wave at slightly different
    /// phases; if that phase difference displaces them by more than their spacing
    /// they cross, and the engraving turns to spaghetti. Checked for every ratio
    /// mode, every whorl, at full rib density — the case with the least room.
    #[test]
    fn ribs_never_cross_their_neighbours() {
        let ribs_per_chamber = 1.0 + SHELL_RIB_MAX;
        for mode in RatioMode::ALL {
            for (whorl, &n) in chambers_for(mode).iter().enumerate() {
                let r_out = 1.0 / SHELL_GROWTH.powi(whorl as i32);
                let span = TAU / n as f32;
                let rib_gap = span / ribs_per_chamber;
                // The phase gradient is scaled by radius (constant wavelength in
                // pixels), which is precisely what protects the tight inner whorls.
                let dphase = RIPPLE_WRAP * r_out * rib_gap;
                let mut worst: f32 = 0.0;
                for i in 0..=64 {
                    let u = i as f32 / 64.0;
                    for along in [RIPPLE_ALONG_DAMPED, RIPPLE_ALONG_OPEN] {
                        let a = rib_offset(u, span, 1.0, along, 0.0);
                        let b = rib_offset(u, span, 1.0, along, dphase);
                        worst = worst.max((a - b).abs());
                    }
                }
                assert!(
                    worst < rib_gap,
                    "{}: whorl {} ({} chambers) diverges {:.4} rad against {:.4} of \
                     spacing — ribs would cross",
                    mode_name(mode),
                    whorl,
                    n,
                    worst,
                    rib_gap
                );
            }
        }
    }

    /// The span cap stops a coarsely divided whorl throwing a rib across the shell.
    /// GoldenMirror's outer whorl — 4 chambers, 1.57 rad — is the case that forces
    /// the cap to exist.
    #[test]
    fn span_cap_bounds_displacement_on_coarse_whorls() {
        let coarse = TAU / 4.0;
        assert!(coarse > SHELL_SPAN_CAP, "this test's premise has gone stale");
        let mut worst: f32 = 0.0;
        for i in 0..=200 {
            let u = i as f32 / 200.0;
            for ph in [0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0] {
                let o = rib_offset(u, coarse, SHELL_RIB_INNER, RIPPLE_ALONG_OPEN, ph);
                worst = worst.max(o.abs());
            }
        }
        // A rib spans one whorl, so its length is (1 - 1/growth) of its outer
        // radius. Sideways travel must stay under that, or it sweeps through the
        // neighbouring chambers instead of bending.
        let rib_len_frac = 1.0 - 1.0 / SHELL_GROWTH;
        assert!(
            worst < rib_len_frac,
            "displacement {worst:.3} rad exceeds the rib's own length fraction \
             {rib_len_frac:.3} — the span cap is not doing its job"
        );
    }

    /// Both rib ends stay pinned to their sutures at any phase: the ripple and the
    /// S-bend are windowed, so only the lean survives at the ends.
    #[test]
    fn rib_ends_stay_pinned() {
        for span in [TAU / 34.0, TAU / 4.0, TAU / 29.0] {
            for ph in [0.0, 0.7, 1.9, 3.3, 5.5] {
                for waviness in [1.0, SHELL_RIB_INNER] {
                    let outer = rib_offset(1.0, span, waviness, RIPPLE_ALONG_OPEN, ph);
                    assert!(
                        outer.abs() < 1e-5,
                        "outer end wandered {outer} — it must sit on its suture"
                    );
                    let inner = rib_offset(0.0, span, waviness, RIPPLE_ALONG_OPEN, ph);
                    let lean = -SHELL_RIB_SWEEP * span.min(SHELL_SPAN_CAP);
                    assert!((inner - lean).abs() < 1e-5, "inner end: {inner} vs {lean}");
                }
            }
        }
    }

    /// The growth ribs wave harder than the septa, but the *lean* is shared — which
    /// is what keeps the two families from crossing each other.
    #[test]
    fn inner_ribs_wave_more_but_lean_the_same() {
        let span = TAU / 34.0;
        let mut septum: f32 = 0.0;
        let mut inner: f32 = 0.0;
        for i in 0..=100 {
            let u = i as f32 / 100.0;
            let base = -SHELL_RIB_SWEEP * span * (1.0 - u);
            septum = septum.max((rib_offset(u, span, 1.0, 0.0, 0.0) - base).abs());
            inner = inner.max((rib_offset(u, span, SHELL_RIB_INNER, 0.0, 0.0) - base).abs());
        }
        assert!(
            inner > septum * 1.2,
            "inner ribs ({inner:.4}) should wave clearly more than septa ({septum:.4})"
        );
    }

    /// INDEX dilates the umbilicus, inversely and monotonically: silent tree depths
    /// mean an open centre.
    #[test]
    fn umbilicus_closes_as_index_rises() {
        let max_r = 500.0_f32;
        let eye = |index: f32| SHELL_MIN_PX + max_r * UMBILICUS_MAX * (1.0 - index);
        assert!(eye(0.0) > eye(1.0), "index 0 must open the centre widest");
        assert!(
            (eye(1.0) - SHELL_MIN_PX).abs() < 1e-4,
            "at full index the centre should close to the stroke limit"
        );
        assert!(
            eye(0.0) < max_r * 0.5,
            "an open umbilicus must not swallow the shell"
        );
        let mut prev = f32::MAX;
        for i in 0..=20 {
            let e = eye(i as f32 / 20.0);
            assert!(e < prev);
            prev = e;
        }
    }

    /// The portrait grid parser: liberal about which character means ink, because
    /// image-to-ASCII converters emit all sorts and Billy should not have to match
    /// a character set.
    #[test]
    fn portrait_grid_parses_any_converters_output() {
        for grid in [
            "  ##  \n #### \n  ##  ",
            "@@**@@\n@****@\n@@**@@",
            "  11    1111    ",
        ] {
            assert!(parse_portrait_grid(grid).is_some(), "failed to parse {grid:?}");
        }
        let (w, h, pts) = parse_portrait_grid("  ##  \n #### \n  ##  ").unwrap();
        assert_eq!((w, h), (6, 3));
        assert_eq!(pts.len(), 2 + 4 + 2);
        // `#` is ink, so comments are `//` — the commonest ink character cannot
        // also be the comment marker.
        let (_, h2, _) = parse_portrait_grid("// a face\n  ##  \n #### ").unwrap();
        assert_eq!(h2, 2, "the comment line was counted as a pixel row");
        // Blank padding is trimmed, so a converter's margin doesn't shrink the art.
        let (_, h3, _) = parse_portrait_grid("\n\n  ##  \n\n\n").unwrap();
        assert_eq!(h3, 1);
        // **Only a blank is empty.** `.` and `0` are ink like anything else, which
        // is the whole of the 2026-08-05 fix: a portrait drawn in `0` used to read
        // as 28 stray dots out of 749 marks.
        let (_, _, dots) = parse_portrait_grid("..0..").unwrap();
        assert_eq!(dots.len(), 5, "`.` and `0` must count as ink");
        // Ragged rows are fine: width is the longest.
        assert_eq!(parse_portrait_grid("#\n####\n##").unwrap().0, 4);
    }

    /// Nothing empty, blank or oversized gets through. The cloud is redrawn every
    /// frame, so grid size is a per-frame cost.
    #[test]
    fn portrait_grid_rejects_what_it_cannot_draw() {
        assert!(parse_portrait_grid("").is_none(), "empty");
        assert!(parse_portrait_grid("    \n    ").is_none(), "no ink");
        assert!(parse_portrait_grid("// only a comment").is_none());
        assert!(
            parse_portrait_grid(&"#".repeat(PORTRAIT_MAX_W + 1)).is_none(),
            "over max width"
        );
        let tall = vec!["#"; PORTRAIT_MAX_H + 1].join("\n");
        assert!(parse_portrait_grid(&tall).is_none(), "over max height");
        assert!(
            parse_portrait_grid(&"#".repeat(PORTRAIT_MAX_W)).is_some(),
            "rejected the max width"
        );
    }

    /// The two cadences must be distinguishable *as cadences*, not merely as speeds:
    /// a person hesitates and varies, a device does neither. If console mode ever
    /// picked up jitter or punctuation holds it would still be fast, and the tell
    /// would be gone without anything looking broken.
    #[test]
    fn transcript_and_console_read_differently() {
        let sentence = "It hummed back at me. Twice.";
        let chars: Vec<char> = sentence.chars().collect();
        let total = |console: bool| -> f32 {
            (0..chars.len())
                .map(|i| {
                    App::char_dwell(i, i.checked_sub(1).map(|k| chars[k]), console)
                })
                .sum()
        };
        let (transcript, console) = (total(false), total(true));
        assert!(
            transcript > console * 2.0,
            "the entity should print far faster than a person types: {transcript:.2}s \
             vs {console:.2}s"
        );

        // Console is perfectly even — every character costs the same, whatever
        // precedes it.
        let a = App::char_dwell(0, None, true);
        for (i, c) in chars.iter().enumerate() {
            assert_eq!(
                App::char_dwell(i, Some(*c), true),
                a,
                "console dwell varied at {i} after {c:?}"
            );
        }

        // Transcript is not: it varies per character, and it holds at punctuation.
        let plain = App::char_dwell(4, Some('m'), false);
        let stop = App::char_dwell(4, Some('.'), false);
        assert!(stop > plain * 5.0, "a full stop should hold: {stop} vs {plain}");
        let varied: Vec<f32> = (0..12).map(|i| App::char_dwell(i, Some('m'), false)).collect();
        assert!(
            varied.windows(2).any(|w| w[0] != w[1]),
            "transcript dwell is a metronome — the jitter is missing"
        );

        // And it stays deterministic: the same box types the same way every time.
        assert_eq!(varied, (0..12).map(|i| App::char_dwell(i, Some('m'), false)).collect::<Vec<_>>());
    }

    /// The dither cycle ping-pongs without repeating either end, so it never jumps
    /// from the densest frame to the sparsest and never stalls for two frames at a
    /// turning point. An off-by-one here is a visible stutter twice per cycle.
    #[test]
    fn the_dither_cycle_pingpongs_seamlessly() {
        let mk = |n: usize| Portrait {
            key: "t".into(),
            stamp: None,
            frames: (0..n)
                .map(|i| PortraitFrame {
                    w: 10,
                    h: 10,
                    // i+1 points, so ink() — and therefore the frame's identity —
                    // is distinct and ascending.
                    pts: (0..=i as u8).map(|k| (k, 0)).collect(),
                })
                .collect(),
        };

        // One frame: always itself, never a panic.
        let one = mk(1);
        for k in 0..10 {
            assert_eq!(one.frame_at(k as f32 * 0.5, 0.5).pts.len(), 1);
        }

        for n in [2usize, 3, 5, 13] {
            let p = mk(n);
            let seq: Vec<usize> = (0..(4 * n))
                .map(|k| p.frame_at(k as f32 * 0.5 + 0.01, 0.5).pts.len() - 1)
                .collect();
            // Walks up to the top and back down.
            assert_eq!(seq[0], 0, "n={n} should start at the sparsest");
            assert_eq!(seq[n - 1], n - 1, "n={n} should reach the densest");
            // Never jumps more than one frame, including across the wrap.
            for w in seq.windows(2) {
                assert_eq!(
                    (w[1] as i64 - w[0] as i64).abs(),
                    1,
                    "n={n} jumped {:?} -> {:?} in {seq:?}",
                    w[0],
                    w[1]
                );
            }
            // Both ends are visited exactly once per cycle: no double-length hold.
            let cycle = &seq[..(2 * n - 2)];
            assert_eq!(
                cycle.iter().filter(|&&i| i == 0).count(),
                1,
                "n={n} held the sparsest frame twice: {cycle:?}"
            );
            assert_eq!(
                cycle.iter().filter(|&&i| i == n - 1).count(),
                1,
                "n={n} held the densest frame twice: {cycle:?}"
            );
            // And it is periodic.
            assert_eq!(seq[0], seq[2 * n - 2], "n={n} did not close its cycle");
        }
    }

    /// A BOM and CRLF are both stripped — the two ways Windows has already broken a
    /// content file in this project.
    #[test]
    fn portrait_grid_survives_windows() {
        let (w, h, _) = parse_portrait_grid("\u{feff}  ##  \r\n #### \r\n").unwrap();
        assert_eq!((w, h), (6, 2), "BOM or CRLF leaked into the grid");
    }

    /// The shipped relic log parses, and its shape is what the voice code assumes:
    /// every id a Fibonacci number, every entry carrying something to say, every
    /// entry naming an avatar, and both intrusion kinds present.
    #[test]
    fn the_relic_log_parses_and_holds_its_shape() {
        let relics = load_relics();
        assert!(
            !relics.is_empty(),
            "relic_log.json did not load — the instrument would be mute"
        );

        let mut fib = vec![1u64, 1];
        while *fib.last().unwrap() < 10_000_000_000 {
            let n = fib[fib.len() - 1] + fib[fib.len() - 2];
            fib.push(n);
        }
        for r in &relics {
            let id: u64 = r
                .id
                .parse()
                .unwrap_or_else(|_| panic!("id {:?} is not a number", r.id));
            assert!(fib.contains(&id), "id {id} is not a Fibonacci number");
            assert!(
                !r.log.is_empty() || !r.intrusion.is_empty(),
                "relic {id} has nothing to say"
            );
            assert!(!r.avatar.is_empty(), "relic {id} names no portrait");
            assert!(
                portrait_asset_name(&r.avatar).is_some(),
                "relic {}'s portrait {:?} does not resolve to an asset",
                id,
                r.avatar
            );
            assert!(
                rarity_weight(&r.rarity) > 0,
                "relic {id} has an unusable rarity"
            );
        }
        // Both interruption sources need something to draw from.
        assert!(
            relics.iter().any(|r| r.is_pure_intrusion()),
            "no pure intrusions — the entity could never speak for itself"
        );
        assert!(
            relics.iter().any(|r| !r.log.is_empty()),
            "no human logs — there would be nothing to interrupt"
        );
    }

    /// An entry inherits everything from its character and overrides only what it
    /// states. This is the whole point of the grouped shape: adding a line to an
    /// existing witness must not mean retyping who they are.
    #[test]
    fn entries_inherit_their_character() {
        let json = r#"{ "characters": [ {
            "archetype": "Overworked Acoustic Engineer",
            "era": "1987", "rarity": "common", "portrait": "engineer_1bit",
            "entries": [
              { "log": ["one"] },
              { "log": ["two"], "era": "1988", "archetype": "Retired Engineer" }
            ] } ] }"#;
        let relics = serde_json::from_str::<RelicSource>(json).unwrap().flatten();
        assert_eq!(relics.len(), 2);
        // Inherited wholesale.
        assert_eq!(relics[0].archetype, "Overworked Acoustic Engineer");
        assert_eq!(relics[0].era, "1987");
        assert_eq!(relics[0].rarity, "common");
        assert_eq!(relics[0].avatar, "engineer_1bit");
        // Overridden where stated, inherited where not.
        assert_eq!(relics[1].archetype, "Retired Engineer");
        assert_eq!(relics[1].era, "1988");
        assert_eq!(relics[1].rarity, "common", "rarity should still be inherited");
        assert_eq!(relics[1].avatar, "engineer_1bit");
    }

    /// Ids are assigned by position from the distinct Fibonacci numbers, so Billy
    /// never types one. The sequence must match the ids that were hand-written in
    /// the original file — that correspondence is what proved the grouped shape was
    /// the shape the data already had.
    #[test]
    fn ids_are_positional_fibonacci() {
        let expected: [u64; 12] = [1, 2, 3, 5, 8, 13, 21, 34, 55, 89, 144, 233];
        for (i, &want) in expected.iter().enumerate() {
            assert_eq!(fib_id(i), want, "position {i}");
        }
        // Saturating rather than overflowing: a log this long is not our problem,
        // but a panic would be.
        assert!(fib_id(200) > 0, "fib_id must not overflow to zero or panic");
    }

    /// The original flat array still parses, so a half-converted file — or an old
    /// copy pasted back in — is never a silent mute.
    #[test]
    fn the_original_flat_shape_still_loads() {
        let json = r#"[ { "id": "99", "archetype": "Survivor", "era": "unknown",
            "rarity": "rare", "avatar": "survivor_1bit.png",
            "metadata": { "tstamp": "t", "alias": "a", "extra": "e" },
            "log": ["found it"], "intrusion": [] } ]"#;
        let relics = serde_json::from_str::<RelicSource>(json).unwrap().flatten();
        assert_eq!(relics.len(), 1);
        assert_eq!(relics[0].archetype, "Survivor");
        assert_eq!(relics[0].id, "99", "an explicit id must be respected");
        // A .png in old data still resolves — the extension is discarded.
        assert_eq!(
            portrait_asset_name(&relics[0].avatar).unwrap(),
            "portraits/survivor_1bit.txt"
        );
    }

    /// Entries with nothing to say are dropped rather than selected and rendered
    /// blank.
    #[test]
    fn silent_entries_are_dropped() {
        let json = r#"{ "characters": [ { "archetype": "X", "portrait": "x",
            "entries": [ { "log": [] }, { "log": ["real"] }, { "intrusion": [] } ] } ] }"#;
        let relics = serde_json::from_str::<RelicSource>(json).unwrap().flatten();
        assert_eq!(relics.len(), 1, "empty entries should not survive");
        assert_eq!(relics[0].log, vec!["real"]);
        assert_eq!(relics[0].id, "1", "ids are assigned after the cull, not before");
    }

    /// A portrait name comes out of a data file, so whatever it says, the result
    /// must address something inside the portraits folder and nothing else.
    ///
    /// Directory parts are **stripped rather than rejected**, which is deliberate:
    /// `portraits/engineer_1bit.png` is a very likely thing to write given what the
    /// folder is called, and stripping makes it work instead of failing silently.
    /// Stripping is also what makes escape impossible, so it is the safety property
    /// and the convenience at once.
    #[test]
    fn portrait_names_always_stay_inside_their_folder() {
        assert_eq!(
            portrait_asset_name("engineer_1bit").unwrap(),
            "portraits/engineer_1bit.txt"
        );
        // Whatever extension the data carries, it resolves to the same file.
        for n in ["monk_1bit", "monk_1bit.png", "monk_1bit.txt"] {
            assert_eq!(portrait_asset_name(n).unwrap(), "portraits/monk_1bit.txt");
        }
        // The invariant, over everything hostile or careless.
        for n in [
            "../../secrets/key.png",
            "..\\..\\windows\\system32\\x.png",
            "portraits/face.png",
            "nested/dir/face.png",
            "/etc/passwd",
            "C:\\keys\\a.png",
        ] {
            if let Some(got) = portrait_asset_name(n) {
                assert!(
                    got.starts_with("portraits/"),
                    "{n:?} escaped to {got:?}"
                );
                assert!(!got.contains(".."), "{n:?} left traversal in {got:?}");
                assert_eq!(
                    got.matches('/').count(),
                    1,
                    "{n:?} produced a nested path {got:?}"
                );
            }
        }
        // Nothing usable is refused outright rather than guessed at.
        assert_eq!(portrait_asset_name(""), None, "empty");
        assert_eq!(portrait_asset_name(".png"), None, "no stem at all");
        assert_eq!(portrait_asset_name(".."), None, "bare traversal");
    }

    /// Agitation fills under sustained subversion, drains when the player relents,
    /// and reaches the interrupt threshold in a plausible time. This is the
    /// forgiveness rule as arithmetic: it must not latch.
    #[test]
    fn agitation_fills_then_forgives() {
        let step = |a: f32, subversion: f32, dt: f32| {
            (a + dt * (subversion / AGITATE_RISE_S - a / AGITATE_FALL_S)).clamp(0.0, 1.0)
        };
        // Full subversion, held: reaches the interrupt level, and in a window a
        // player would connect to their own action.
        let mut a = 0.0f32;
        let mut secs = 0.0f32;
        while a < AGITATE_INTERRUPT && secs < 600.0 {
            a = step(a, 1.0, 0.05);
            secs += 0.05;
        }
        assert!(
            a >= AGITATE_INTERRUPT,
            "sustained subversion never provokes the entity"
        );
        assert!(
            (5.0..180.0).contains(&secs),
            "provocation took {secs:.0}s — outside anything a player would connect \
             to their own action"
        );
        // Relenting drains it back down, and keeps going to rest.
        let mut b = a;
        let mut relent = 0.0f32;
        while b > AGITATE_INTERRUPT * 0.5 && relent < 600.0 {
            b = step(b, 0.0, 0.05);
            relent += 0.05;
        }
        assert!(
            b <= AGITATE_INTERRUPT * 0.5,
            "agitation latches — it must forgive"
        );
        for _ in 0..20_000 {
            b = step(b, 0.0, 0.05);
        }
        assert!(b < 0.01, "agitation never returns to rest, settling at {b}");
        // No subversion, no provocation, ever.
        let mut c = 0.0f32;
        for _ in 0..20_000 {
            c = step(c, 0.0, 0.05);
        }
        assert!(c < 1e-6, "the entity gets provoked by nothing at all");
    }

    /// The intrusion odds rise with agitation, start at the decay floor, and never
    /// reach certainty — a witness must always be able to get a word in.
    ///
    /// This replaced a threshold-and-discharge design that a test killed: agitation
    /// refills from a discharge in ~4 s while a box lasts ~60, so the discharge was
    /// always repaid before the next box and gated nothing whatsoever.
    #[test]
    fn intrusion_odds_rise_with_agitation_but_never_certain() {
        assert!(
            (intrude_chance(0.0) - DECAY_INTRUDE_CHANCE).abs() < 1e-6,
            "at rest the only intrusions should be the interface's own decay"
        );
        assert!(
            intrude_chance(0.0) > 0.0,
            "decay intrusions must happen unprompted, or the entity only ever \
             appears as punishment"
        );
        assert!(
            intrude_chance(1.0) < 1.0,
            "fully provoked must still leave room for a human log"
        );
        assert!(intrude_chance(1.0) > 0.5, "full provocation should dominate the channel");
        let mut prev = -1.0;
        for i in 0..=20 {
            let c = intrude_chance(i as f32 / 20.0);
            assert!(c > prev, "odds must rise monotonically");
            prev = c;
        }
        // Out of range is clamped, not extrapolated past certainty.
        assert!(intrude_chance(9.0) < 1.0);
        assert!((intrude_chance(-1.0) - DECAY_INTRUDE_CHANCE).abs() < 1e-6);
    }

    /// Rarity weights are Fibonacci and strictly ordered, so a common entry really
    /// is commoner than a rare one.
    #[test]
    fn rarity_weights_are_ordered_fibonacci() {
        let (c, u, r, v) = (
            rarity_weight("common"),
            rarity_weight("uncommon"),
            rarity_weight("rare"),
            rarity_weight("very_rare"),
        );
        assert!(c > u && u > r && r > v && v > 0);
        assert_eq!([c, u, r, v], [8, 5, 3, 1]);
        // An unknown rarity must fall back to something usable, not zero.
        assert!(rarity_weight("legendary") > 0);
        assert!(rarity_weight("") > 0);
    }

    /// **The rule that Xilla broke, as a test.** Every face on the roster is on disk
    /// and has its licence beside it. A font that arrives without its licence now
    /// fails the suite instead of reaching a release, which is the only version of
    /// this rule that survives being forgotten.
    #[test]
    fn every_face_ships_with_its_licence() {
        for face in FACES {
            let font = format!("fonts/{}/{}", face.slug, face.file);
            assert!(
                asset_path(&font).is_some(),
                "{font} is on the roster but not on disk"
            );
            let licence = format!("fonts/{}/LICENSE.txt", face.slug);
            assert!(
                asset_path(&licence).is_some(),
                "{} ships with no licence file — it must not be in the tree",
                face.slug
            );
        }
    }

    /// No unlicensed face is reachable. `font.otf` was Xilla, loaded by name for the
    /// whole design pass; the roster replaces it, and nothing should be able to
    /// quietly restore a face that has no licence next to it.
    #[test]
    fn the_unlicensed_face_is_gone() {
        for stray in ["font.otf", "font.ttf"] {
            assert!(
                asset_path(stray).is_none(),
                "{stray} is back in assets — every face lives in fonts/<slug>/ with its licence"
            );
        }
    }

    /// Everyone Billy has written is cast. An archetype that matches nothing falls
    /// back to the interface's own face, which is correct behaviour and invisible —
    /// so the log itself is checked, or a new witness silently arrives unvoiced.
    #[test]
    fn every_witness_in_the_log_has_a_face() {
        let relics = load_relics();
        assert!(!relics.is_empty(), "relic_log.json did not load");
        for r in &relics {
            let face = face_for_archetype(&r.archetype);
            assert_ne!(
                face, UI_FACE,
                "'{}' is not in the VOICES table — it would speak in the interface's face",
                r.archetype
            );
            assert!(
                FACES.iter().any(|f| f.slug == face),
                "'{}' maps to '{face}', which is not on the roster",
                r.archetype
            );
        }
    }

    /// The Monk is written with a **non-breaking hyphen** — U+2011, identical on
    /// screen to `-` and never equal to it. This is the case that would have looked
    /// like a font bug for as long as it took someone to paste the string into a hex
    /// dump, so it is pinned by character code rather than by copied text.
    #[test]
    fn punctuation_never_decides_who_speaks() {
        let monk = "Shell\u{2011}Listening Monk";
        assert_eq!(face_for_archetype(monk), "scriptorium");
        assert_eq!(face_for_archetype("Shell-Listening Monk"), "scriptorium");
        // The whole dash family folds, and case never matters.
        assert_eq!(face_for_archetype("SHELL\u{2013}LISTENING MONK"), "scriptorium");
        // A prefix match, so every A–H intrusion finds the entity.
        for tag in ["", " A", " H"] {
            assert_eq!(face_for_archetype(&format!("Logalith Intrusion{tag}")), FALLBACK_FACE);
        }
        // And an unwritten archetype is unattributed rather than mis-attributed.
        assert_eq!(face_for_archetype("Night Porter"), UI_FACE);
    }

    /// **Every grid in the portraits folder actually loads, and carries a figure.**
    ///
    /// Both halves of this caught a real failure on 2026-08-05. Nine of the ten
    /// portraits in Billy's second batch were 98–120 wide against a cap of 96 and
    /// were refused outright; four more used `0` or `.` as a tone, which the parser
    /// then read as background — one of them ending up 28 marks out of 749. Neither
    /// failure says anything at runtime: a rejected or hollowed-out portrait just
    /// falls back to the turning shell mark, which looks exactly like art that has
    /// not been drawn yet.
    #[test]
    fn every_portrait_on_disk_parses_and_carries_a_figure() {
        let dir = asset_dirs()
            .into_iter()
            .map(|d| d.join("portraits"))
            .find(|d| d.is_dir())
            .expect("the portraits folder");
        let mut checked = 0;
        for entry in std::fs::read_dir(&dir).unwrap().flatten() {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("txt") {
                continue;
            }
            let name = path.file_name().unwrap().to_string_lossy().to_string();
            // Billy's source bundle — ten portraits in one file behind `/name`
            // markers. Working material, split into the grids beside it; the
            // instrument never loads it.
            if name.starts_with("zboxtxt") {
                continue;
            }
            let text = std::fs::read_to_string(&path).unwrap();
            let (w, h, pts) = parse_portrait_grid(&text).unwrap_or_else(|| {
                panic!("{name} does not parse — it would silently fall back to the mark")
            });
            let ink = pts.len() as f32 / (w * h).max(1) as f32;
            assert!(
                ink > 0.03,
                "{name} is {:.1}% ink at {w}×{h} — a figure that sparse is a format \
                 mismatch, not a drawing",
                ink * 100.0
            );
            checked += 1;
        }
        // Zero is a valid answer, and it is what a fresh clone sees: the grids are
        // derivatives of source images whose licence is not established, so they are
        // deliberately never committed — the same discipline that makes Xilla a
        // release blocker, applied to pictures. On Billy's machine this checks the
        // real folder; anywhere else it correctly checks nothing.
        eprintln!("portrait grids checked: {checked}");
    }

    /// The record card is the same shape for every entry. Most of the log carries no
    /// `tstamp`, `alias` or `extra`, so a card that rendered only what it had would
    /// resize itself every time the voice advanced — a panel twitching every 45
    /// seconds reads as a bug, and a form with a blank line reads as a record.
    #[test]
    fn the_record_card_never_changes_shape() {
        let bare = Relic {
            id: "1".into(),
            archetype: "Survivor".into(),
            era: String::new(),
            rarity: "rare".into(),
            avatar: String::new(),
            metadata: RelicMeta::default(),
            log: vec!["x".into()],
            intrusion: Vec::new(),
        };
        let full = Relic {
            era: "  1987  ".into(),
            metadata: RelicMeta {
                tstamp: "1987-04-12::14:03".into(),
                alias: "op1_reboot".into(),
                extra: "φ-index strobe 0.665".into(),
            },
            ..bare.clone()
        };
        let (a, b) = (record_rows(&bare), record_rows(&full));
        assert_eq!(a.len(), b.len());
        for (i, ((la, va), (lb, vb))) in a.iter().zip(b.iter()).enumerate() {
            assert_eq!(la, lb, "row {i} changed its label");
            assert!(!va.is_empty() && !vb.is_empty(), "row {i} rendered as nothing");
        }
        // Blank fields are marked, not omitted; present ones are trimmed.
        assert!(a.iter().all(|(_, v)| v == NOT_RECORDED));
        assert_eq!(b[0].1, "1987");
        assert_eq!(b[3].1, "φ-index strobe 0.665");
        // And every entry Billy has actually written renders.
        for r in load_relics() {
            assert!(record_rows(&r).iter().all(|(_, v)| !v.is_empty()));
        }
    }

    /// Rarity pips can never outrun the row they are drawn in. The pip strip
    /// allocates exactly `RARITY_SLOTS` cells, so a rarity weighted heavier than
    /// eight would paint outside its own rect and over whatever sits beside it —
    /// which is the kind of thing that looks like a rendering bug rather than a
    /// data one.
    #[test]
    fn rarity_pips_fit_their_row() {
        let mut seen = vec!["common", "uncommon", "rare", "very_rare", "", "legendary"];
        let relics = load_relics();
        seen.extend(relics.iter().map(|r| r.rarity.as_str()));
        for rarity in seen {
            let w = rarity_weight(rarity);
            assert!(
                w <= RARITY_SLOTS,
                "rarity '{rarity}' weighs {w}, which overflows a {RARITY_SLOTS}-pip row"
            );
            assert!(w > 0, "rarity '{rarity}' would draw no pips at all");
        }
    }

    /// Where the voice ladder actually lands on Billy's log.
    ///
    /// "Most boxes have room at 32 and the long ones don't" is a claim about *his
    /// text*, not about the code, so it is measured against the real entries at the
    /// real cell size rather than asserted in a comment. The bound is deliberately
    /// loose — this fails when the log changes shape enough to matter, not when a
    /// sentence gains a word.
    #[test]
    fn the_voice_ladder_lands_where_it_says() {
        let ctx = egui::Context::default();
        ctx.set_fonts(font_definitions());
        // egui builds its atlases on the first frame and refuses to lay anything out
        // before one has run, so this drives an empty one.
        let _ = ctx.run(Default::default(), |_| {});
        // Cell W at the shipped 1180×780 window, less its padding.
        let avail = Vec2::new(780.0, 250.0);
        let (mut large, mut small) = (0, 0);
        for r in load_relics() {
            let text = if r.is_pure_intrusion() {
                r.intrusion.join("\n")
            } else {
                r.log.join("\n")
            };
            let face = face_for_archetype(&r.archetype);
            let fits = |want: f32| {
                let id = FontId::new(grid_size(want, native_of(face), 1.0), family(face));
                ctx.fonts(|f| f.layout(text.clone(), id, WHITE, avail.x).size().y) <= avail.y
            };
            if fits(VOICE_PX[0]) {
                large += 1;
            } else {
                small += 1;
            }
        }
        assert!(large + small >= 25, "the log did not load");
        assert!(
            large > 0,
            "no entry fits the top rung — the ladder is decorative and the box never \
             gets bigger than it was"
        );
        assert!(
            small > 0,
            "every entry fits at {}px, so the bottom rung is dead weight — either the \
             ladder wants another rung above it or the cell grew",
            VOICE_PX[0]
        );
        eprintln!("voice ladder: {large} entries at the top rung, {small} at the bottom");
    }

    /// Every family the program can ask for is registered and resolves to real font
    /// data. An unregistered `FontFamily::Name` panics inside egui's layout, so the
    /// failure would be a crash on whichever launch first rolled an uncast witness —
    /// which is to say, not on the launch that shipped it.
    #[test]
    fn every_family_the_program_can_ask_for_exists() {
        let fonts = font_definitions();
        let asked: Vec<egui::FontFamily> = FACES
            .iter()
            .map(|f| family(f.slug))
            .chain([
                family(UI_FACE),
                family(FALLBACK_FACE),
                egui::FontFamily::Monospace,
                egui::FontFamily::Proportional,
            ])
            // Every face any archetype in the log can select, by the same route the
            // voice box takes.
            .chain(load_relics().iter().map(|r| family(face_for_archetype(&r.archetype))))
            .collect();
        for fam in asked {
            let chain = fonts
                .families
                .get(&fam)
                .unwrap_or_else(|| panic!("{fam:?} is never registered — layout would panic"));
            assert!(!chain.is_empty(), "{fam:?} resolves to nothing");
            for name in chain {
                assert!(
                    fonts.font_data.contains_key(name),
                    "{fam:?} names '{name}', which has no font data"
                );
            }
        }
        // And the fallback really is underneath the others, not merely present.
        for face in FACES.iter().filter(|f| f.slug != FALLBACK_FACE) {
            let chain = &fonts.families[&family(face.slug)];
            assert_eq!(chain.first().map(String::as_str), Some(face.slug));
            assert!(
                chain.iter().any(|n| n == FALLBACK_FACE),
                "{} has no fallback — a glyph it lacks would be a tofu box",
                face.slug
            );
        }
    }

    /// A pixel face is only ever asked for at a whole multiple of its own grid, in
    /// **device** pixels — including on a scaled display, which is where the naive
    /// version silently fails. A face with no grid is left exactly as asked.
    #[test]
    fn type_lands_on_the_pixel_grid() {
        for ppp in [1.0_f32, 1.25, 1.5, 2.0] {
            for want in [9.0_f32, 10.0, 11.0, 12.0, 15.0, 16.0, 18.0] {
                for native in [9.0_f32, 16.0] {
                    let got = grid_size(want, Some(native), ppp);
                    let device = got * ppp;
                    let k = device / native;
                    assert!(
                        (k - k.round()).abs() < 1e-4,
                        "{want} at ppp {ppp} on a {native} px grid gave {device} device px"
                    );
                    assert!(k >= 1.0, "a size must never round away to nothing");
                    // Nearest multiple: never more than half a grid step off.
                    assert!((device - want * ppp).abs() <= native * 0.5 + 1e-4);
                }
            }
        }
        assert_eq!(grid_size(12.0, None, 1.0), 12.0);
        assert_eq!(grid_size(12.0, None, 1.75), 12.0);
    }
}
