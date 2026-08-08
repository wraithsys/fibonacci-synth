//! Play a converted Photomosh prototype on a loop, at animation speed.
//!
//! ```text
//! cargo run --release -p fibonacci-gui --example template_loop -- [name] [--screenshot out.png]
//! ```
//!
//! Defaults to the `template-moshed` frame set (Billy's logo entrance, converted from
//! `template-mark-moshed-*.mp4` by assetdraw).
//!
//! Different from `portrait_preview` in one way that matters: that one ping-pongs slowly,
//! which suits an idle portrait but reads as a slideshow for an entrance. This plays the
//! set forward at animation speed, holds on the final frame, then restarts -- so you see
//! the thing as an *entrance that repeats*, which is what a prototype loop is for.
//!
//! A dev-dependency probe. None of it reaches the shipped binary.

use eframe::egui;
use std::path::{Path, PathBuf};
use std::time::Instant;

/// Seconds per frame while the entrance plays.
const FRAME_SECS: f32 = 0.07;
/// How long the mark takes to deform and dissolve away before the loop restarts.
///
/// There is deliberately no static hold. A frozen frame reads as "the animation stopped"
/// rather than "the animation is between beats", and the exit is computed rather than
/// baked -- no extra frames, and the timing is a constant to tune, not a re-render.
const DISSOLVE_SECS: f32 = 1.1;

struct Frame {
    w: usize,
    h: usize,
    pts: Vec<(u8, u8)>,
}

/// Same rule as `parse_portrait_grid` in `src/main.rs`: `//` comments, BOM stripped,
/// and only a blank space is empty -- every other character is ink.
fn parse_grid(text: &str) -> Option<Frame> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut rows: Vec<&str> = text
        .lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .collect();
    while rows.last().is_some_and(|l| l.trim().is_empty()) {
        rows.pop();
    }
    if rows.is_empty() {
        return None;
    }
    let w = rows.iter().map(|r| r.chars().count()).max().unwrap_or(0);
    let h = rows.len();
    if w == 0 {
        return None;
    }
    let mut pts = Vec::new();
    for (y, row) in rows.iter().enumerate() {
        for (x, ch) in row.chars().enumerate() {
            if ch != ' ' {
                pts.push((x as u8, y as u8));
            }
        }
    }
    Some(Frame { w, h, pts })
}

fn load_set(dir: &Path, name: &str) -> Vec<Frame> {
    let mut frames = Vec::new();
    if let Ok(t) = std::fs::read_to_string(dir.join(format!("{name}.txt"))) {
        if let Some(f) = parse_grid(&t) {
            frames.push(f);
        }
    }
    for n in 1..=64 {
        if let Ok(t) = std::fs::read_to_string(dir.join(format!("{name}-{n}.txt"))) {
            if let Some(f) = parse_grid(&t) {
                frames.push(f);
            }
        }
    }
    frames
}

struct App {
    frames: Vec<Frame>,
    name: String,
    start: Instant,
    shot: Option<PathBuf>,
    shot_taken: bool,
    /// Where in the dissolve (0..1) a `--screenshot` lands.
    shot_p: f32,
}

/// Deterministic 0..1 per cell, so a point dissolves at the same moment every cycle
/// instead of flickering. Cheap integer hash -- no RNG, no state.
fn hash01(x: u8, y: u8) -> f32 {
    let h = (x as u32)
        .wrapping_mul(73_856_093)
        ^ (y as u32).wrapping_mul(19_349_663);
    let h = h ^ (h >> 13);
    ((h.wrapping_mul(1_274_126_177) >> 8) & 0xffff) as f32 / 65535.0
}

impl App {
    /// Forward through the set, then deform and dissolve, then restart.
    /// `Some(p)` is dissolve progress 0..1; `None` means still playing in.
    fn frame_at(&self, t: f32) -> (&Frame, usize, Option<f32>) {
        let n = self.frames.len();
        let play = n as f32 * FRAME_SECS;
        let cycle = play + DISSOLVE_SECS;
        let local = t % cycle;
        if local < play {
            let i = ((local / FRAME_SECS) as usize).min(n - 1);
            (&self.frames[i], i, None)
        } else {
            (&self.frames[n - 1], n - 1, Some((local - play) / DISSOLVE_SECS))
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        ctx.request_repaint();

        egui::CentralPanel::default().show(ctx, |ui| {
            let t = self.start.elapsed().as_secs_f32();
            let (frame, idx, dissolve) = self.frame_at(t);

            let rect = ui.max_rect();
            let painter = ui.painter();
            // Light marks on near-black, matching how the source render actually looks.
            painter.rect_filled(rect, 0.0, egui::Color32::from_gray(12));

            let avail = rect.shrink(28.0);
            let cell = (avail.width() / frame.w as f32)
                .min(avail.height() / frame.h as f32)
                .max(1.0);
            let mut origin = avail.min
                + egui::vec2(
                    (avail.width() - frame.w as f32 * cell) * 0.5,
                    (avail.height() - frame.h as f32 * cell) * 0.5,
                );

            // Whole-block jitter: the entire mark slams around as one unit, on a fast
            // snap clock and quantised to whole cells. Applied to the origin so every
            // point moves together -- that wholesale displacement is what sells it as a
            // signal breaking up, with the per-band tearing riding on top of it.
            if let Some(p) = dissolve {
                let bq = (t * 22.0).floor() as i64 as u8;
                let bx = (hash01(bq, 3) - 0.5) * 2.0;
                let by = (hash01(bq, 91) - 0.5) * 2.0;
                let amp = 11.0 * p;
                origin += egui::vec2(
                    (bx * amp).round() * cell,
                    (by * amp * 0.55).round() * cell,
                );
            }
            for &(x, y) in &frame.pts {
                let mut pos = origin + egui::vec2(x as f32 * cell, y as f32 * cell);
                let mut gray = 235.0_f32;

                if let Some(p) = dissolve {
                    let r = hash01(x, y);

                    // Erode: each cell has its own moment to go, so the mark eats away
                    // unevenly instead of fading as one flat block. Eased so it starts
                    // slow and accelerates.
                    if r < p.powf(1.6) {
                        continue;
                    }

                    // Glitch is COHERENT displacement -- whole rows and blocks jumping
                    // together. Per-point scatter reads as particles/snow instead, so the
                    // row-coherent terms dominate here and the per-cell jitter stays small.

                    // Chunky horizontal tearing: rows shift in whole-cell steps. Held on a
                    // coarse time quantum so it snaps between positions rather than
                    // sliding, which is what makes it read as a broken signal.
                    let tq = (t * 12.0).floor();
                    let band_id = (y as f32 / 5.0).floor();
                    let band = (hash01(band_id as u8, (tq as u8) & 0x3f) - 0.5) * 2.0;
                    let stepped = (band * 14.0 * p).round() * cell;

                    // A minority of bands blow out much further, and which ones changes
                    // on the same coarse clock.
                    let tear = if hash01((band_id as u8).wrapping_add(31), (tq as u8) & 0x3f) > 0.62 {
                        (hash01(band_id as u8, 7) - 0.5) * cell * 70.0 * p * p
                    } else {
                        0.0
                    };

                    // Blocks also drop in whole-cell steps, so the mark comes apart in
                    // slabs rather than raining evenly.
                    let slab = ((hash01(band_id as u8, 19) - 0.4) * 10.0 * p * p).round() * cell;

                    // Small per-cell jitter only, and only late, to rough up the edges.
                    let late = (p - 0.55).max(0.0) / 0.45;
                    let jitter_x = (r - 0.5) * cell * 2.2 * late;
                    let jitter_y = (hash01(y, x) - 0.5) * cell * 2.2 * late;

                    pos += egui::vec2(stepped + tear + jitter_x, slab + jitter_y);

                    // Survivors dim toward the background, so the last cells do not
                    // simply pop out of existence.
                    gray = 235.0 * (1.0 - p * 0.55) + 12.0 * (p * 0.55);
                }

                painter.rect_filled(
                    egui::Rect::from_min_size(pos, egui::vec2(cell * 0.9, cell * 0.9)),
                    0.0,
                    egui::Color32::from_gray(gray as u8),
                );
            }

            painter.text(
                rect.min + egui::vec2(10.0, 8.0),
                egui::Align2::LEFT_TOP,
                match dissolve {
                    None => format!("{}  frame {}/{}", self.name, idx + 1, self.frames.len()),
                    Some(p) => format!("{}  dissolving {:.0}%", self.name, p * 100.0),
                },
                egui::FontId::monospace(12.0),
                egui::Color32::from_gray(120),
            );
        });

        if let Some(path) = self.shot.clone() {
            // Land at a chosen point in the dissolve, which is the part worth checking.
            let shot_at = self.frames.len() as f32 * FRAME_SECS + DISSOLVE_SECS * self.shot_p;
            if !self.shot_taken && self.start.elapsed().as_secs_f32() > shot_at {
                self.shot_taken = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot);
            }
            for ev in ctx.input(|i| i.raw.events.clone()) {
                if let egui::Event::Screenshot { image, .. } = ev {
                    let file = std::fs::File::create(&path).expect("create screenshot");
                    let mut enc =
                        png::Encoder::new(std::io::BufWriter::new(file), image.width() as u32, image.height() as u32);
                    enc.set_color(png::ColorType::Rgba);
                    enc.set_depth(png::BitDepth::Eight);
                    let mut w = enc.write_header().expect("png header");
                    let mut bytes = Vec::with_capacity(image.pixels.len() * 4);
                    for px in &image.pixels {
                        bytes.extend_from_slice(&px.to_array());
                    }
                    w.write_image_data(&bytes).expect("png data");
                    println!("wrote {}", path.display());
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
        }
    }
}

fn main() -> eframe::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let name = args
        .iter()
        .find(|a| !a.starts_with("--"))
        .cloned()
        .unwrap_or_else(|| "template-moshed".to_string());
    let shot = args
        .iter()
        .position(|a| a == "--screenshot")
        .and_then(|i| args.get(i + 1))
        .map(PathBuf::from);
    let shot_p = args
        .iter()
        .position(|a| a == "--shot-p")
        .and_then(|i| args.get(i + 1))
        .and_then(|v| v.parse::<f32>().ok())
        .unwrap_or(0.45)
        .clamp(0.0, 0.99);

    let dir = PathBuf::from("crates/fibonacci-gui/assets/portraits");
    let frames = load_set(&dir, &name);
    if frames.is_empty() {
        eprintln!("no frames for \"{name}\" in {}", dir.display());
        std::process::exit(2);
    }
    println!(
        "{name}: {} frames, {}x{}, {:.2}s play + {:.1}s dissolve (never static)",
        frames.len(),
        frames[0].w,
        frames[0].h,
        frames.len() as f32 * FRAME_SECS,
        DISSOLVE_SECS
    );

    eframe::run_native(
        "template loop",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default().with_inner_size([560.0, 600.0]),
            ..Default::default()
        },
        Box::new(move |_cc| {
            Ok(Box::new(App {
                frames,
                name,
                start: Instant::now(),
                shot,
                shot_taken: false,
                shot_p,
            }))
        }),
    )
}
