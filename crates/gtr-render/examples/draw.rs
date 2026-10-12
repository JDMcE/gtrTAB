//! Draws a score file as a picture, to look at what an importer and the layout make of
//! it without opening the app:
//!
//! ```text
//! cargo run -p gtr-render --example draw -- song.gp5 song.png [tab|standard|both|slash] [all]
//! ```
//!
//! The picture is a PNG, a PDF of pages or an SVG, by the name it is given. `all` shows
//! every track. The fonts are the app's, from `app/ui/public/fonts`.

use std::path::{Path, PathBuf};

use gtr_layout::{Notation, Page, View, layout};
use gtr_render::{Fonts, SvgOptions, pdf, png, svg};

fn main() {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let [file, out, rest @ ..] = arguments.as_slice() else {
        eprintln!(
            "usage: draw <score file> <out.png|out.pdf|out.svg> [tab|standard|both|slash] [all]"
        );
        std::process::exit(2);
    };
    let bytes = std::fs::read(file).unwrap_or_else(|e| panic!("{file}: {e}"));
    let score = match file.ends_with(".gtrtab") {
        true => gtr_io::native::load(&bytes).unwrap_or_else(|e| panic!("{file}: {e}")),
        false => {
            let read = gtr_io::import(file, &bytes).unwrap_or_else(|e| panic!("{file}: {e}"));
            if !read.skipped.is_empty() {
                eprintln!("not imported: {}", read.skipped.join(", "));
            }
            read.score
        }
    };
    let view = View {
        notation: match rest.iter().find(|word| *word != "all").map(String::as_str) {
            Some("tab") => Notation::Tab,
            Some("standard") => Notation::Standard,
            Some("slash") => Notation::Slash,
            _ => Notation::Both,
        },
        multitrack: rest.iter().any(|word| word == "all"),
        ..View::default()
    };
    let fonts = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../app/ui/public/fonts");
    let font =
        |name: &str| std::fs::read(fonts.join(name)).unwrap_or_else(|e| panic!("{name}: {e}"));
    let files = [
        font("Bravura.otf"),
        font("NotoSans-Regular.ttf"),
        font("NotoSans-SemiBold.ttf"),
    ];
    let fonts = Fonts::parse(&files[0], &files[1], &files[2]).expect("the app's fonts");
    let drawn = match Path::new(out).extension().and_then(|e| e.to_str()) {
        Some("pdf") => {
            let pages = layout(&score, &view, Page::A4.width).pages(Page::A4);
            pdf(&pages, &fonts, &score.info.title)
        }
        Some("svg") => {
            let options = SvgOptions {
                fonts: Some(&fonts),
                ..SvgOptions::default()
            };
            svg(&layout(&score, &view, 1000.0).printed(), options).into_bytes()
        }
        _ => png(&layout(&score, &view, 1000.0).printed(), &fonts, 1.5),
    };
    std::fs::write(out, drawn).unwrap_or_else(|e| panic!("{out}: {e}"));
}
