//! Draw a Space Z portrait frame set as an animated egui point cloud.
//!
//! ```text
//! cargo run -p fibonacci-gui --example portrait_preview -- <name> [--screenshot out.png]
//! ```
//!
//! `<name>` is a portrait base name in `assets/portraits/` (same lookup as the app:
//! `<name>.txt` plus `<name>-1.txt`, `<name>-2.txt` … up to 64, gap-tolerant). This is
//! a probe, not the app's plinth — the plinth is currently parked (see
//! `Portrait::frame_at` in `src/main.rs`), so there is no live in-app rendering to
//! match yet. This just proves a frame set is valid and animates correctly, using the
//! same parsing rule (blank = empty, anything else = ink) and the same ping-pong
//! timing the plinth will use once it is unparked.
//!
//! `--screenshot out.png` captures one frame partway through the cycle and exits —
//! for driving this from a script rather than watching the window.
//!
//! A dev-dependency, so none of it reaches the shipped binary.

use eframe::egui;
use std::path::{Path, PathBuf};
use std::time::Instant;

struct Frame {
    w: usize,
    h: usize,
    pts: Vec<(u8, u8)>,
}

/// Mirrors `parse_portrait_grid` in `src/main.rs`: one line per row, `//` comments,
/// a UTF-8 BOM stripped, and *only a blank space is empty* — everything else is ink.
fn parse_grid(text: &str) -> Option<Frame> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let rows: Vec<&str> = text
        .lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .collect();
    let rows: Vec<&str> = {
        let mut r = rows;
        while r.last().is_some_and(|l| l.trim().is_empty()) {
            r.pop();
        }
        r
    };
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

fn load_frame_set(dir: &Path, name: &str) -> Vec<Frame> {
    let mut frames = Vec::new();
    let base = dir.join(format!("{name}.txt"));
    if let Ok(text) = std::fs::read_to_string(&base) {
        if let Some(f) = parse_grid(&text) {
            frames.push(f);
        }
    }
    for n in 1..=64 {
        let p = dir.join(format!("{name}-{n}.txt"));
        if let Ok(text) = std::fs::read_to_string(&p) {
            if let Some(f) = parse_grid(&text) {
                frames.push(f);
            }
        }
    }
    frames
}

/// Same ping-pong as `Portrait::frame_at`: up through the set and back down without
/// repeating either end.
fn frame_at(frames: &[Frame], t: f32, secs: f32) -> &Frame {
    let n = frames.len();
    if n <= 1 {
        return &frames[0];
    }
    let span = 2 * n - 2;
    let step = ((t / secs).floor() as i64).rem_euclid(span as i64) as usize;
    let i = if step < n { step } else { span - step };
    &frames[i]
}

struct App {
    frames: Vec<Frame>,
    name: String,
    start: Instant,
    screenshot_path: Option<PathBuf>,
    screenshot_taken: bool,
}

const SECS_PER_FRAME: f32 = 0.45;

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.request_repaint(); // keep animating

        egui::CentralPanel::default().show(ctx, |ui| {
            let t = self.start.elapsed().as_secs_f32();
            let f = frame_at(&self.frames, t, SECS_PER_FRAME);

            let rect = ui.max_rect();
            let pad = 24.0;
            let avail = rect.shrink(pad);
            let scale = (avail.width() / f.w as f32).min(avail.height() / f.h as f32);
            let cell = scale.max(1.0);
            let origin = avail.min
                + egui::vec2(
                    (avail.width() - f.w as f32 * cell) * 0.5,
                    (avail.height() - f.h as f32 * cell) * 0.5,
                );

            let painter = ui.painter();
            painter.rect_filled(rect, 0.0, egui::Color32::from_gray(245));
            for &(px, py) in &f.pts {
                let p0 = origin + egui::vec2(px as f32 * cell, py as f32 * cell);
                painter.rect_filled(
                    egui::Rect::from_min_size(p0, egui::vec2(cell * 0.92, cell * 0.92)),
                    0.0,
                    egui::Color32::from_gray(20),
                );
            }

            let idx = self
                .frames
                .iter()
                .position(|fr| std::ptr::eq(fr, f))
                .unwrap_or(0);
            ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(egui::Rect::from_min_size(rect.min + egui::vec2(8.0, 6.0), egui::vec2(400.0, 20.0))),
            )
            .label(format!(
                "{}  --  frame {}/{}  ({}x{}, {} pts)",
                self.name,
                idx + 1,
                self.frames.len(),
                f.w,
                f.h,
                f.pts.len()
            ));
        });

        if let Some(path) = self.screenshot_path.clone() {
            if !self.screenshot_taken && self.start.elapsed().as_secs_f32() > SECS_PER_FRAME * 2.2 {
                self.screenshot_taken = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot);
            }
            for ev in ctx.input(|i| i.raw.events.clone()) {
                if let egui::Event::Screenshot { image, .. } = ev {
                    save_png(&path, &image);
                    println!("wrote {}", path.display());
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
        }
    }
}

fn save_png(path: &Path, image: &egui::ColorImage) {
    let file = std::fs::File::create(path).expect("create screenshot file");
    let w = std::io::BufWriter::new(file);
    let mut enc = png::Encoder::new(w, image.width() as u32, image.height() as u32);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    let mut writer = enc.write_header().expect("write png header");
    let mut bytes = Vec::with_capacity(image.pixels.len() * 4);
    for px in &image.pixels {
        bytes.extend_from_slice(&px.to_array());
    }
    writer.write_image_data(&bytes).expect("write png data");
}

fn main() -> eframe::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let name = args
        .first()
        .cloned()
        .unwrap_or_else(|| "zz-moshpro-anim-test".to_string());
    let screenshot_path = args
        .iter()
        .position(|a| a == "--screenshot")
        .and_then(|i| args.get(i + 1))
        .map(PathBuf::from);

    let dir = PathBuf::from("crates/fibonacci-gui/assets/portraits");
    let frames = load_frame_set(&dir, &name);
    if frames.is_empty() {
        eprintln!(
            "no frames found for \"{name}\" in {} (looked for {name}.txt, {name}-1.txt, ...)",
            dir.display()
        );
        std::process::exit(2);
    }
    println!(
        "{name}: {} frame(s), {}x{} .. {}x{}",
        frames.len(),
        frames[0].w,
        frames[0].h,
        frames.last().unwrap().w,
        frames.last().unwrap().h
    );

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([420.0, 460.0]),
        ..Default::default()
    };
    eframe::run_native(
        "portrait_preview",
        options,
        Box::new(move |_cc| {
            Ok(Box::new(App {
                frames,
                name,
                start: Instant::now(),
                screenshot_path,
                screenshot_taken: false,
            }))
        }),
    )
}
