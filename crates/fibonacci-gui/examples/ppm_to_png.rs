//! Convert a `BYPO_SHOT` capture into a PNG.
//!
//! `BYPO_SHOT=seconds:path` writes binary PPM (P6) because that costs zero
//! dependencies at runtime — see `write_ppm` in the GUI. The trade is that
//! almost nothing *consumes* PPM: itch.io, GitHub and every storefront want
//! PNG, so a promo shot or a cover image could be captured and then not
//! uploaded. This closes that gap.
//!
//! A dev-dependency, so it never reaches the shipped binary — the instrument
//! still decodes and encodes no images at runtime. `png` is already in the
//! lock for `png_to_grid`, so this costs no new compilation.
//!
//! ```text
//! cargo run --release -p fibonacci-gui --example ppm_to_png -- shot.ppm shot.png
//! cargo run --release -p fibonacci-gui --example ppm_to_png -- shot.ppm
//! ```
//!
//! With no output path the extension is swapped to `.png` beside the input.

use std::io::Write;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let input = match args.next() {
        Some(p) => std::path::PathBuf::from(p),
        None => {
            eprintln!("usage: ppm_to_png <in.ppm> [out.png]");
            std::process::exit(2);
        }
    };
    let output = args
        .next()
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| input.with_extension("png"));

    let raw = std::fs::read(&input)?;
    let (width, height, pixels) = parse_p6(&raw)?;

    let file = std::fs::File::create(&output)?;
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), width, height);
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header()?.write_image_data(pixels)?;

    // The captures are 1-bit art, so the interesting number is not the file
    // size but whether the thing that came out is the size you meant to grab:
    // the window logs its own figure on startup and resize.
    println!(
        "{} -> {} ({}x{})",
        input.display(),
        output.display(),
        width,
        height
    );
    std::io::stdout().flush()?;
    Ok(())
}

/// Binary PPM, tolerantly: the magic, three integers, then one whitespace
/// byte and the raw RGB.
///
/// Written against the spec rather than against `write_ppm`'s exact output,
/// because a parser that only accepts its own writer's formatting is a trap
/// for the first person who points it at a capture from anything else.
/// Comments (`#` to end of line) are legal anywhere in the header and are
/// skipped.
fn parse_p6(raw: &[u8]) -> Result<(u32, u32, &[u8]), String> {
    let mut at = 0usize;

    // One whitespace-delimited header token, skipping comments.
    let token = |raw: &[u8], at: &mut usize| -> Result<String, String> {
        loop {
            while *at < raw.len() && raw[*at].is_ascii_whitespace() {
                *at += 1;
            }
            if *at < raw.len() && raw[*at] == b'#' {
                while *at < raw.len() && raw[*at] != b'\n' {
                    *at += 1;
                }
                continue;
            }
            break;
        }
        let start = *at;
        while *at < raw.len() && !raw[*at].is_ascii_whitespace() {
            *at += 1;
        }
        if start == *at {
            return Err("header ended early — not a PPM?".into());
        }
        String::from_utf8(raw[start..*at].to_vec()).map_err(|e| e.to_string())
    };

    let magic = token(raw, &mut at)?;
    if magic != "P6" {
        return Err(format!(
            "expected a binary PPM (P6), found '{magic}'. \
             ASCII PPM (P3) is not supported — BYPO_SHOT writes P6."
        ));
    }
    let parse = |s: String| s.parse::<u32>().map_err(|e| format!("bad header value: {e}"));
    let width = parse(token(raw, &mut at)?)?;
    let height = parse(token(raw, &mut at)?)?;
    let max = parse(token(raw, &mut at)?)?;
    if max != 255 {
        return Err(format!(
            "only 8-bit PPM is supported (maxval 255), found {max}"
        ));
    }

    // Exactly one whitespace byte separates the header from the data. Any
    // more and it would be pixel data, which is why this is not a loop.
    if at >= raw.len() || !raw[at].is_ascii_whitespace() {
        return Err("no whitespace between the PPM header and its data".into());
    }
    at += 1;

    let want = width as usize * height as usize * 3;
    let have = raw.len() - at;
    if have < want {
        return Err(format!(
            "truncated: header claims {width}x{height} ({want} bytes of RGB), file has {have}"
        ));
    }
    Ok((width, height, &raw[at..at + want]))
}

#[cfg(test)]
mod tests {
    use super::parse_p6;

    /// The shape `write_ppm` actually emits.
    #[test]
    fn it_reads_what_the_shot_tool_writes() {
        let mut raw = b"P6\n2 1\n255\n".to_vec();
        raw.extend_from_slice(&[0, 0, 0, 255, 255, 255]);
        let (w, h, px) = parse_p6(&raw).unwrap();
        assert_eq!((w, h), (2, 1));
        assert_eq!(px, &[0, 0, 0, 255, 255, 255]);
    }

    /// Header comments and loose whitespace are legal PPM, so a capture from
    /// some other tool still converts.
    #[test]
    fn comments_and_stray_whitespace_are_legal() {
        let mut raw = b"P6\n# made by something else\n  2   1\n# and again\n255\n".to_vec();
        raw.extend_from_slice(&[1, 2, 3, 4, 5, 6]);
        let (w, h, px) = parse_p6(&raw).unwrap();
        assert_eq!((w, h), (2, 1));
        assert_eq!(px, &[1, 2, 3, 4, 5, 6]);
    }

    /// Trailing bytes are ignored, but missing ones are an error rather than
    /// a half-written image — a truncated capture should fail loudly, not
    /// produce a PNG that is quietly wrong at the bottom.
    #[test]
    fn a_truncated_capture_is_refused() {
        let mut raw = b"P6\n4 4\n255\n".to_vec();
        raw.extend_from_slice(&[0; 10]);
        let err = parse_p6(&raw).unwrap_err();
        assert!(err.contains("truncated"), "{err}");
    }

    #[test]
    fn ascii_ppm_and_deep_ppm_say_why_they_are_refused() {
        assert!(parse_p6(b"P3\n1 1\n255\n0 0 0").unwrap_err().contains("P3"));
        let mut deep = b"P6\n1 1\n65535\n".to_vec();
        deep.extend_from_slice(&[0; 6]);
        assert!(parse_p6(&deep).unwrap_err().contains("maxval 255"));
    }
}
