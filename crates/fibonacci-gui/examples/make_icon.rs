//! Build `icon/icon.ico` and `icon/icon_256.rgba` from a source PNG.
//!
//! Replaces `tools/logo_compose.py`, which needs a Python this machine does
//! not have. A dev-dependency, so none of it reaches the shipped binary.
//!
//! ```text
//! cargo run --release -p fibonacci-gui --example make_icon -- <in.png> [--invert]
//! ```
//!
//! **Alpha is respected throughout**, because Billy's logo is black marks on
//! transparency rather than grey on white — the greys are varying opacity, not
//! varying colour. Downsampling therefore premultiplies before averaging and
//! un-premultiplies after; averaging RGBA straight would drag the colour of
//! fully transparent pixels into the visible edge and fringe it.
//!
//! `--invert` flips the marks to white while leaving the transparency alone,
//! for the version that has to survive a dark taskbar.
//!
//! ## Format
//!
//! Sizes 16–128 are written as 32-bit BMP (bottom-up, with the vestigial AND
//! mask every ICO still requires), and 256 as PNG. That split is the
//! convention every icon toolchain follows, and it matters here because the
//! resource compiler has to parse this file at build time — PNG-only icons
//! are legal on Windows Vista and later but not universally handled by the
//! tools in between.

use std::io::Write;

/// The sizes an application icon is actually asked for: Explorer's list and
/// detail views, the taskbar, alt-tab, and the 256 that everything modern
/// scales from.
const SIZES: [u32; 6] = [16, 24, 32, 48, 64, 128];
const BIG: u32 = 256;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(input) = args.first().cloned() else {
        eprintln!("usage: make_icon <in.png> [--invert] [--bg 0..255]");
        std::process::exit(2);
    };
    let invert = args.iter().any(|a| a == "--invert");

    // `--bg <0..255>` composites onto an opaque field of that grey.
    //
    // An app icon is a different job from a logo. A logo sits on a page you
    // control; an icon sits on a taskbar you do not, which may be light or
    // dark and will not tell you which. Billy's mark runs pale at the edges to
    // near-black at the centre, so on a dark field it *inverts* — the solid
    // middle vanishes and the faint outer dither is all that survives, which
    // reads as a hole. An opaque backing makes the icon carry its own
    // background, which is why the Logalith one was a ringed disc.
    let bg: Option<u8> = args
        .iter()
        .position(|a| a == "--bg")
        .and_then(|i| args.get(i + 1))
        .and_then(|v| v.parse().ok());

    let (w, h, mut src) = read_png(&input)?;
    if invert {
        // Marks flipped, transparency untouched — for a mark that has to read
        // on a dark field instead of a light one.
        for px in src.chunks_mut(4) {
            px[0] = 255 - px[0];
            px[1] = 255 - px[1];
            px[2] = 255 - px[2];
        }
    }
    if let Some(level) = bg {
        for px in src.chunks_mut(4) {
            let a = px[3] as u32;
            for c in 0..3 {
                // Standard over-composite: mark over the field, by opacity.
                px[c] = ((px[c] as u32 * a + level as u32 * (255 - a)) / 255) as u8;
            }
            px[3] = 255;
        }
    }
    println!(
        "source {input}: {w}x{h}{}{}",
        if invert { " (inverted)" } else { "" },
        match bg {
            Some(l) => format!(" on grey {l}"),
            None => " (transparent)".into(),
        }
    );

    // Anchored to the crate, not the working directory. `cargo run` starts in
    // the workspace root, so a relative "icon/" quietly builds a second icon
    // directory beside Cargo.toml and leaves the real one untouched — which it
    // did, and the only reason it was noticed is that the preview still showed
    // the old Logalith.
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("icon");
    std::fs::create_dir_all(&dir)?;

    // The window icon: raw RGBA, exactly 256x256, read straight into eframe.
    let big = resize(&src, w, h, BIG, BIG);
    std::fs::write(dir.join("icon_256.rgba"), &big)?;
    println!("{}  {BIG}x{BIG} raw rgba", dir.join("icon_256.rgba").display());

    // The exe resource.
    let mut entries: Vec<(u32, Vec<u8>)> = SIZES
        .iter()
        .map(|&s| (s, bmp_entry(&resize(&src, w, h, s, s), s)))
        .collect();
    entries.push((BIG, encode_png(&big, BIG, BIG)?));
    write_ico(dir.join("icon.ico"), &entries)?;
    let total: usize = entries.iter().map(|(_, d)| d.len()).sum();
    println!(
        "{}       {} sizes ({}), {} bytes",
        dir.join("icon.ico").display(),
        entries.len(),
        entries
            .iter()
            .map(|(s, _)| s.to_string())
            .collect::<Vec<_>>()
            .join("/"),
        total + 6 + entries.len() * 16
    );
    Ok(())
}

fn read_png(path: &str) -> Result<(u32, u32, Vec<u8>), Box<dyn std::error::Error>> {
    let decoder = png::Decoder::new(std::io::BufReader::new(std::fs::File::open(path)?));
    let mut reader = decoder.read_info()?;
    let mut buf = vec![0; reader.output_buffer_size().ok_or("image too large")?];
    let info = reader.next_frame(&mut buf)?;
    let (w, h) = (info.width, info.height);
    let n = info.buffer_size();
    let px = &buf[..n];
    // Normalise whatever came in to 8-bit RGBA.
    let rgba: Vec<u8> = match info.color_type {
        png::ColorType::Rgba => px.to_vec(),
        png::ColorType::Rgb => px.chunks(3).flat_map(|p| [p[0], p[1], p[2], 255]).collect(),
        png::ColorType::GrayscaleAlpha => {
            px.chunks(2).flat_map(|p| [p[0], p[0], p[0], p[1]]).collect()
        }
        png::ColorType::Grayscale => px.iter().flat_map(|&g| [g, g, g, 255]).collect(),
        other => return Err(format!("unsupported colour type {other:?}").into()),
    };
    Ok((w, h, rgba))
}

/// Box-downsample in **premultiplied** space.
///
/// Averaging straight RGBA lets a fully transparent pixel's colour vote on the
/// result, which fringes every edge with whatever happened to be stored in the
/// invisible parts of the source. Premultiplying weights each pixel's colour
/// by its own opacity, which is the only arrangement that makes an edge come
/// out the colour it looks.
fn resize(src: &[u8], sw: u32, sh: u32, dw: u32, dh: u32) -> Vec<u8> {
    let mut out = vec![0u8; (dw * dh * 4) as usize];
    for y in 0..dh {
        let y0 = y * sh / dh;
        let y1 = (((y + 1) * sh + dh - 1) / dh).min(sh).max(y0 + 1);
        for x in 0..dw {
            let x0 = x * sw / dw;
            let x1 = (((x + 1) * sw + dw - 1) / dw).min(sw).max(x0 + 1);
            let (mut r, mut g, mut b, mut a, mut n) = (0.0f64, 0.0, 0.0, 0.0, 0.0);
            for sy in y0..y1 {
                for sx in x0..x1 {
                    let i = ((sy * sw + sx) * 4) as usize;
                    let pa = src[i + 3] as f64 / 255.0;
                    r += src[i] as f64 * pa;
                    g += src[i + 1] as f64 * pa;
                    b += src[i + 2] as f64 * pa;
                    a += pa;
                    n += 1.0;
                }
            }
            let o = ((y * dw + x) * 4) as usize;
            let mean_a = a / n;
            // Un-premultiply. Fully clear stays black rather than dividing by
            // nothing.
            let un = if a > 0.0 { 1.0 / a } else { 0.0 };
            out[o] = (r * un).round().clamp(0.0, 255.0) as u8;
            out[o + 1] = (g * un).round().clamp(0.0, 255.0) as u8;
            out[o + 2] = (b * un).round().clamp(0.0, 255.0) as u8;
            out[o + 3] = (mean_a * 255.0).round().clamp(0.0, 255.0) as u8;
        }
    }
    out
}

fn encode_png(rgba: &[u8], w: u32, h: u32) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut out, w, h);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        enc.write_header()?.write_image_data(rgba)?;
    }
    Ok(out)
}

/// A 32-bit BMP icon image: BITMAPINFOHEADER, the pixels bottom-up as BGRA,
/// then the AND mask.
///
/// The height in the header is **doubled** — the format's own quirk, counting
/// the colour bitmap and the mask together. The mask is all zeros because the
/// alpha channel already carries the transparency, but it cannot be omitted:
/// every ICO reader still measures the file against it.
fn bmp_entry(rgba: &[u8], size: u32) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&40u32.to_le_bytes()); // biSize
    out.extend_from_slice(&(size as i32).to_le_bytes());
    out.extend_from_slice(&((size * 2) as i32).to_le_bytes()); // colour + mask
    out.extend_from_slice(&1u16.to_le_bytes()); // biPlanes
    out.extend_from_slice(&32u16.to_le_bytes()); // biBitCount
    out.extend_from_slice(&0u32.to_le_bytes()); // BI_RGB
    out.extend_from_slice(&(size * size * 4).to_le_bytes()); // biSizeImage
    out.extend_from_slice(&[0u8; 16]); // resolution + palette counts

    for y in (0..size).rev() {
        for x in 0..size {
            let i = ((y * size + x) * 4) as usize;
            out.extend_from_slice(&[rgba[i + 2], rgba[i + 1], rgba[i], rgba[i + 3]]);
        }
    }
    // AND mask: one bit per pixel, rows padded to four bytes.
    let row = ((size + 31) / 32) * 4;
    out.extend(std::iter::repeat(0u8).take((row * size) as usize));
    out
}

fn write_ico(path: impl AsRef<std::path::Path>, entries: &[(u32, Vec<u8>)]) -> std::io::Result<()> {
    let mut f = std::io::BufWriter::new(std::fs::File::create(path)?);
    f.write_all(&0u16.to_le_bytes())?; // reserved
    f.write_all(&1u16.to_le_bytes())?; // type: icon
    f.write_all(&(entries.len() as u16).to_le_bytes())?;

    let mut offset = 6 + entries.len() * 16;
    for (size, data) in entries {
        // 256 is written as 0 — the field is one byte, so the largest size the
        // format can name is the one it cannot.
        let dim = if *size >= 256 { 0u8 } else { *size as u8 };
        f.write_all(&[dim, dim, 0, 0])?; // width, height, palette, reserved
        f.write_all(&1u16.to_le_bytes())?; // planes
        f.write_all(&32u16.to_le_bytes())?; // bits per pixel
        f.write_all(&(data.len() as u32).to_le_bytes())?;
        f.write_all(&(offset as u32).to_le_bytes())?;
        offset += data.len();
    }
    for (_, data) in entries {
        f.write_all(data)?;
    }
    f.flush()
}
