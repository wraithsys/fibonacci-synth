//! Vet candidate fonts before any of them enters `assets/fonts/`.
//!
//! ```text
//! cargo run -p fibonacci-gui --example font_probe -- <dir-or-file>...
//! ```
//!
//! Two questions, both of which can only be answered by the file itself:
//!
//! 1. **What does it say its licence is?** A font's `name` table can carry a
//!    licence description (id 13) and a licence URL (id 14). Seven of the twelve
//!    candidate ZIPs shipped no licence file at all — exactly the situation that
//!    makes Xilla a release blocker — so the embedded metadata is the only other
//!    evidence there is.
//! 2. **Does it carry the glyphs this instrument uses?** The UI's marginalia needs
//!    φ, π, ×, →, subscripts; Billy's relic log needs Δ, ρ, •, ◦, ¤ and curly
//!    quotes. A display face that lacks them falls back mid-sentence to a different
//!    typeface, which looks like a bug. This was a guess until the files arrived.
//!
//! Dev-dependency tooling; the instrument itself never parses a font table.

use std::path::{Path, PathBuf};

/// Everything the program or Billy's text actually puts on screen beyond ASCII.
/// Grouped so a report can say *what* a font would break rather than just how many.
const REQUIRED: &[(&str, &[char])] = &[
    ("maths", &['φ', 'ρ', 'π', 'Δ', '×', '·', '≈', '→']),
    ("subscripts", &['₁', '₂', '₋']),
    ("punctuation", &['—', '’', '‘', '…']),
    ("sigils", &['•', '◦', '¤']),
];

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!("usage: font_probe <dir-or-file>...");
        std::process::exit(2);
    }

    let mut files: Vec<PathBuf> = Vec::new();
    for a in &args {
        let p = PathBuf::from(a);
        if p.is_dir() {
            collect(&p, &mut files);
        } else {
            files.push(p);
        }
    }
    files.sort();
    if files.is_empty() {
        return Err("no .ttf or .otf found".into());
    }

    for f in &files {
        let data = match std::fs::read(f) {
            Ok(d) => d,
            Err(e) => {
                println!("{}\n  unreadable: {e}\n", label(f));
                continue;
            }
        };
        let face = match ttf_parser::Face::parse(&data, 0) {
            Ok(x) => x,
            Err(e) => {
                println!("{}\n  unparseable: {e}\n", label(f));
                continue;
            }
        };

        println!("{}", label(f));

        // Licence, copyright and vendor, straight from the name table.
        let mut licence = None;
        let mut licence_url = None;
        let mut copyright = None;
        for n in face.names() {
            let v = n.to_string().unwrap_or_default();
            if v.trim().is_empty() {
                continue;
            }
            match n.name_id {
                0 => copyright = copyright.or(Some(v)),
                13 => licence = licence.or(Some(v)),
                14 => licence_url = licence_url.or(Some(v)),
                _ => {}
            }
        }
        match (&licence, &licence_url) {
            (None, None) => println!("  licence : *** none embedded ***"),
            _ => {
                if let Some(l) = &licence {
                    println!("  licence : {}", trim(l, 200));
                }
                if let Some(u) = &licence_url {
                    println!("  lic url : {}", trim(u, 120));
                }
            }
        }
        if let Some(c) = &copyright {
            println!("  (c)     : {}", trim(c, 140));
        }

        // Glyph coverage, by group.
        let mut missing_total = 0;
        let mut report = Vec::new();
        for (group, chars) in REQUIRED {
            let missing: String = chars
                .iter()
                .filter(|c| face.glyph_index(**c).is_none())
                .collect();
            missing_total += missing.chars().count();
            if missing.is_empty() {
                report.push(format!("{group} ok"));
            } else {
                report.push(format!("{group} MISSING {missing}"));
            }
        }
        println!("  glyphs  : {}", report.join("  |  "));
        println!(
            "  verdict : {}",
            match (missing_total, licence.is_some() || licence_url.is_some()) {
                (0, true) => "usable — licence embedded, full coverage",
                (0, false) => "coverage fine; licence must come from elsewhere",
                (_, true) => "licence embedded; will fall back for the missing glyphs",
                (_, false) => "no embedded licence AND incomplete coverage",
            }
        );
        println!();
    }
    Ok(())
}

fn collect(dir: &Path, out: &mut Vec<PathBuf>) {
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                collect(&p, out);
            } else if p
                .extension()
                .and_then(|s| s.to_str())
                .is_some_and(|x| x.eq_ignore_ascii_case("ttf") || x.eq_ignore_ascii_case("otf"))
            {
                out.push(p);
            }
        }
    }
}

fn label(p: &Path) -> String {
    p.file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("?")
        .to_string()
}

fn trim(s: &str, n: usize) -> String {
    let one_line: String = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if one_line.chars().count() <= n {
        one_line
    } else {
        format!("{}…", one_line.chars().take(n - 1).collect::<String>())
    }
}
