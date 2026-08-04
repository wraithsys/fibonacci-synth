//! Convert dithered PNGs into Space Z portrait grids.
//!
//! ```text
//! cargo run -p fibonacci-gui --example png_to_grid -- [options] <in.png>...
//!
//!   --out <name>    portrait name; only valid with a single input
//!   --dir <folder>  take every .png in a folder as input
//!   --dry-run       report densities, write nothing
//!   --alpha         ink = the opaque pixels; brightness ignored entirely
//!   --invert        flip whichever test is in use
//!   --threshold N   luminance cut, 1..254, default 128
//!   --stats         luminance/alpha histogram per file
//!   --preview       print the grid (single input only)
//! ```
//!
//! With more than one input, each portrait name is derived from its filename —
//! extension stripped, then a trailing `.txt`, then any ` (3)` duplicate markers
//! Windows adds. So `engineer_1bit.txt (1) (6).png` becomes `engineer_1bit`. Names
//! that then collide get `-2`, `-3` … so a batch of variants never silently
//! overwrites itself, and the summary tells you which is which.
//!
//! **Ink is the *dark* pixels**, because the plinth draws ink black over a white
//! scanline field: ink erases the stripes it covers, so shadows go solid and
//! highlights stay striped, which is the natural tonal reading. Inking the light
//! pixels instead renders a photograph as its own negative.
//!
//! Transparent pixels are never ink, whatever their colour, so a cut-out
//! background stays striped.
//!
//! **Two kinds of source.** Most dithered exports carry their shape in
//! *brightness*, and the default handles those. But a cut-out carries it in
//! *transparency*, where every opaque pixel is the figure whatever colour it is,
//! and thresholding brightness finds nothing at all. `--alpha` is for those. Run
//! `--stats` if unsure: ~50 % transparent and otherwise near-black is a cut-out.
//!
//! This is a dev-dependency tool. The instrument itself decodes no images.

use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};

const MAX_W: usize = 96;
const MAX_H: usize = 120;
/// Densities outside this band get flagged in the summary: below reads as sparse
/// noise, above as a slab with the stripes barely showing.
const GOOD_INK: (f32, f32) = (0.25, 0.60);

struct Opts {
    by_alpha: bool,
    invert: bool,
    threshold: u32,
    stats: bool,
    preview: bool,
    dry_run: bool,
    only_good: bool,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let flag = |name: &str| args.iter().any(|a| a == name);
    let value = |name: &str| {
        args.iter()
            .position(|a| a == name)
            .and_then(|i| args.get(i + 1))
            .cloned()
    };

    let opts = Opts {
        by_alpha: flag("--alpha"),
        invert: flag("--invert"),
        threshold: value("--threshold").and_then(|v| v.parse().ok()).unwrap_or(128),
        stats: flag("--stats"),
        preview: flag("--preview"),
        dry_run: flag("--dry-run"),
        only_good: flag("--only-good"),
    };
    let out_name = value("--out");
    let dir = value("--dir");

    // Positionals are inputs. Skip the values belonging to --out/--dir/--threshold.
    #[allow(unused_mut)]
    let mut inputs: Vec<PathBuf>;
    inputs = Vec::new();
    let mut skip_next = false;
    for a in &args {
        if skip_next {
            skip_next = false;
            continue;
        }
        if a.starts_with("--") {
            skip_next = matches!(a.as_str(), "--out" | "--dir" | "--threshold");
            continue;
        }
        inputs.push(PathBuf::from(a));
    }
    if let Some(d) = &dir {
        let mut found: Vec<PathBuf> = std::fs::read_dir(d)?
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| {
                p.extension()
                    .and_then(|s| s.to_str())
                    .is_some_and(|e| e.eq_ignore_ascii_case("png"))
            })
            .collect();
        found.sort();
        inputs.extend(found);
    }

    if inputs.is_empty() {
        eprintln!("usage: png_to_grid [options] <in.png>...   (see the file header)");
        std::process::exit(2);
    }
    if out_name.is_some() && inputs.len() > 1 {
        return Err(format!(
            "--out takes a single input, but {} were given. Drop --out and the names \
             come from the filenames.",
            inputs.len()
        )
        .into());
    }

    let dir_out = PathBuf::from("crates/fibonacci-gui/assets/portraits");
    if !opts.dry_run {
        std::fs::create_dir_all(&dir_out)?;
    }

    // `--only-good` drops anything outside the useful density band before naming,
    // so the surviving frames number contiguously — the app treats `<name>-N.txt`
    // as a frame set, and a gap in the numbering is one fewer frame, not an error.
    if opts.only_good {
        let mut keep = Vec::new();
        for i in inputs {
            match convert(&i, &opts) {
                Ok(g) => {
                    let f = g.inked as f32 / (g.w * g.h) as f32;
                    if (GOOD_INK.0..=GOOD_INK.1).contains(&f) {
                        keep.push(i);
                    } else {
                        println!("skip {:<32} {:.0}% ink", file_label(&i), 100.0 * f);
                    }
                }
                Err(e) => println!("skip {:<32} {e}", file_label(&i)),
            }
        }
        inputs = keep;
        if inputs.is_empty() {
            return Err("nothing left in the 25-60% band".into());
        }
        println!();
    }

    // Count derived names first. Where several inputs want the same one — a set of
    // variants for one character — *all* of them get numbered rather than the first
    // claiming the plain name: which file sorts first is arbitrary, and silently
    // handing it the real name means the chosen portrait is chosen by alphabet.
    let bases: Vec<String> = inputs
        .iter()
        .map(|i| match &out_name {
            Some(n) => n.trim_end_matches(".txt").to_string(),
            None => derive_name(i),
        })
        .collect();
    let mut counts: HashMap<&str, usize> = HashMap::new();
    for b in &bases {
        *counts.entry(b.as_str()).or_insert(0) += 1;
    }
    let mut seen: HashMap<&str, usize> = HashMap::new();
    let mut rows: Vec<(String, String, usize, usize, f32, Option<String>)> = Vec::new();

    for (input, base) in inputs.iter().zip(&bases) {
        let n = seen.entry(base.as_str()).or_insert(0);
        *n += 1;
        let name = if counts[base.as_str()] > 1 {
            format!("{base}-{n}")
        } else {
            base.clone()
        };

        match convert(input, &opts) {
            Ok(g) => {
                let frac = g.inked as f32 / (g.w * g.h) as f32;
                let note = if frac < GOOD_INK.0 {
                    Some("sparse — may read as noise".into())
                } else if frac > GOOD_INK.1 {
                    Some("dense — stripes barely show".into())
                } else {
                    None
                };
                if opts.preview && inputs.len() == 1 {
                    for r in &g.rows {
                        println!("{r}");
                    }
                }
                if !opts.dry_run {
                    let path = dir_out.join(format!("{name}.txt"));
                    let mut text = format!(
                        "// {}x{}, {} points ({:.0}% ink) -- from {}\n",
                        g.w,
                        g.h,
                        g.inked,
                        100.0 * frac,
                        input.file_name().and_then(|s| s.to_str()).unwrap_or("?")
                    );
                    for r in &g.rows {
                        text.push_str(r);
                        text.push('\n');
                    }
                    std::fs::write(&path, text.as_bytes())?;
                }
                rows.push((
                    file_label(input),
                    name,
                    g.w,
                    g.h,
                    100.0 * frac,
                    note,
                ));
            }
            Err(e) => rows.push((file_label(input), name, 0, 0, 0.0, Some(e.to_string()))),
        }
    }

    let width = rows.iter().map(|r| r.0.len()).max().unwrap_or(10).min(40);
    println!(
        "\n{:<width$}  {:<22} {:>5}  {:>5}  {}",
        "source", "portrait", "size", "ink", "note"
    );
    for (src, name, w, h, ink, note) in &rows {
        let size = if *w == 0 {
            "-".to_string()
        } else {
            format!("{w}x{h}")
        };
        let inkc = if *w == 0 {
            "-".to_string()
        } else {
            format!("{ink:.0}%")
        };
        println!(
            "{:<width$}  {:<22} {:>5}  {:>5}  {}",
            truncate(src, width),
            name,
            size,
            inkc,
            note.clone().unwrap_or_default()
        );
    }
    let ok = rows.iter().filter(|r| r.2 > 0).count();
    println!(
        "\n{} of {} converted{}",
        ok,
        rows.len(),
        if opts.dry_run { " (dry run, nothing written)" } else { "" }
    );
    if !opts.dry_run && ok > 0 {
        println!("-> crates/fibonacci-gui/assets/portraits/");
    }
    Ok(())
}

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        let keep: String = s.chars().take(n.saturating_sub(1)).collect();
        format!("{keep}…")
    }
}

fn file_label(p: &Path) -> String {
    p.file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("?")
        .to_string()
}

/// `engineer_1bit.txt (1) (6).png` → `engineer_1bit`.
///
/// Strips the extension, then a trailing `.txt` (left over from naming the export
/// after its destination), then any number of Windows' ` (3)` duplicate markers.
fn derive_name(p: &Path) -> String {
    let mut s = p
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("portrait")
        .to_string();
    loop {
        let t = s.trim_end();
        // A trailing " (12)" duplicate marker.
        let stripped = t.strip_suffix(')').and_then(|x| {
            let i = x.rfind('(')?;
            x[i + 1..].chars().all(|c| c.is_ascii_digit()).then(|| x[..i].to_string())
        });
        match stripped {
            Some(next) => s = next,
            None => break,
        }
    }
    let s = s.trim().trim_end_matches(".txt").trim().to_string();
    if s.is_empty() {
        "portrait".into()
    } else {
        s
    }
}

struct Grid {
    w: usize,
    h: usize,
    inked: usize,
    rows: Vec<String>,
}

fn convert(input: &Path, o: &Opts) -> Result<Grid, Box<dyn std::error::Error>> {
    let mut decoder = png::Decoder::new(BufReader::new(File::open(input)?));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info()?;
    let mut buf = vec![0u8; reader.output_buffer_size().unwrap_or(0)];
    let info = reader.next_frame(&mut buf)?;
    let (w, h) = (info.width as usize, info.height as usize);
    let channels = match info.color_type {
        png::ColorType::Grayscale => 1,
        png::ColorType::GrayscaleAlpha => 2,
        png::ColorType::Rgb => 3,
        png::ColorType::Rgba => 4,
        png::ColorType::Indexed => return Err("indexed survived EXPAND".into()),
    };
    if w > MAX_W || h > MAX_H {
        return Err(format!("{w}x{h} over the {MAX_W}x{MAX_H} limit — resize to 64x80").into());
    }

    let sample = |x: usize, y: usize| -> (u32, u8) {
        let i = (y * w + x) * channels;
        match channels {
            1 => (buf[i] as u32, 255),
            2 => (buf[i] as u32, buf[i + 1]),
            3 => (luma(buf[i], buf[i + 1], buf[i + 2]), 255),
            _ => (luma(buf[i], buf[i + 1], buf[i + 2]), buf[i + 3]),
        }
    };

    if o.stats {
        let mut buckets = [0usize; 8];
        let mut transparent = 0usize;
        for y in 0..h {
            for x in 0..w {
                let (lum, alpha) = sample(x, y);
                if alpha < 128 {
                    transparent += 1;
                } else {
                    buckets[(lum / 32).min(7) as usize] += 1;
                }
            }
        }
        let total = (w * h) as f32;
        println!("{} — {w}x{h}, {:?}", file_label(input), info.color_type);
        println!("  transparent {transparent} ({:.0}%)", 100.0 * transparent as f32 / total);
        for (b, n) in buckets.iter().enumerate() {
            if *n > 0 {
                println!(
                    "  luma {:>3}-{:>3} {:>6} ({:.0}%)",
                    b * 32,
                    b * 32 + 31,
                    n,
                    100.0 * *n as f32 / total
                );
            }
        }
    }

    let mut rows = Vec::with_capacity(h);
    let mut inked = 0usize;
    for y in 0..h {
        let mut row = String::with_capacity(w);
        for x in 0..w {
            let (lum, alpha) = sample(x, y);
            // A cut-out defines its figure by opacity; everything else by
            // brightness. Note `<` — ink is the *dark* pixels, because the plinth
            // draws ink black.
            let mut ink = if o.by_alpha {
                alpha >= 128
            } else {
                alpha >= 128 && lum < o.threshold
            };
            if o.invert {
                ink = !ink;
            }
            row.push(if ink { '#' } else { '.' });
            if ink {
                inked += 1;
            }
        }
        rows.push(row);
    }
    if inked == 0 {
        return Err("nothing inked — try --invert or a higher --threshold".into());
    }
    if inked == w * h {
        return Err("everything inked — try --invert or a lower --threshold".into());
    }
    Ok(Grid { w, h, inked, rows })
}

fn luma(r: u8, g: u8, b: u8) -> u32 {
    (0.299 * r as f32 + 0.587 * g as f32 + 0.114 * b as f32).round() as u32
}
