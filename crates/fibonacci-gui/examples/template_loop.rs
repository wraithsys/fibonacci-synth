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
/// Beat to hold on the last frame before looping, so the resolve reads.
const HOLD_SECS: f32 = 0.9;

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
}

impl App {
    /// Forward through the set, then hold, then restart.
    fn frame_at(&self, t: f32) -> (&Frame, usize, bool) {
        let n = self.frames.len();
        let play = n as f32 * FRAME_SECS;
        let cycle = play + HOLD_SECS;
        let local = t % cycle;
        if local < play {
            let i = ((local / FRAME_SECS) as usize).min(n - 1);
            (&self.frames[i], i, false)
        } else {
            (&self.frames[n - 1], n - 1, true)
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        ctx.request_repaint();

        egui::CentralPanel::default().show(ctx, |ui| {
            let t = self.start.elapsed().as_secs_f32();
            let (frame, idx, holding) = self.frame_at(t);

            let rect = ui.max_rect();
            let painter = ui.painter();
            // Light marks on near-black, matching how the source render actually looks.
            painter.rect_filled(rect, 0.0, egui::Color32::from_gray(12));

            let avail = rect.shrink(28.0);
            let cell = (avail.width() / frame.w as f32)
                .min(avail.height() / frame.h as f32)
                .max(1.0);
            let origin = avail.min
                + egui::vec2(
                    (avail.width() - frame.w as f32 * cell) * 0.5,
                    (avail.height() - frame.h as f32 * cell) * 0.5,
                );
            for &(x, y) in &frame.pts {
                let p = origin + egui::vec2(x as f32 * cell, y as f32 * cell);
                painter.rect_filled(
                    egui::Rect::from_min_size(p, egui::vec2(cell * 0.9, cell * 0.9)),
                    0.0,
                    egui::Color32::from_gray(235),
                );
            }

            painter.text(
                rect.min + egui::vec2(10.0, 8.0),
                egui::Align2::LEFT_TOP,
                format!(
                    "{}  frame {}/{}{}",
                    self.name,
                    idx + 1,
                    self.frames.len(),
                    if holding { "  (hold)" } else { "" }
                ),
                egui::FontId::monospace(12.0),
                egui::Color32::from_gray(120),
            );
        });

        if let Some(path) = self.shot.clone() {
            if !self.shot_taken && self.start.elapsed().as_secs_f32() > FRAME_SECS * 5.5 {
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

    let dir = PathBuf::from("crates/fibonacci-gui/assets/portraits");
    let frames = load_set(&dir, &name);
    if frames.is_empty() {
        eprintln!("no frames for \"{name}\" in {}", dir.display());
        std::process::exit(2);
    }
    println!(
        "{name}: {} frames, {}x{}, {:.2}s play + {:.1}s hold",
        frames.len(),
        frames[0].w,
        frames[0].h,
        frames.len() as f32 * FRAME_SECS,
        HOLD_SECS
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
            }))
        }),
    )
}
