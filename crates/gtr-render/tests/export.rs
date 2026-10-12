//! The backends that draw with fonts of their own: PDF, PNG, and SVG in outlines.

use std::path::{Path, PathBuf};

use gtr_layout::{Layout, Notation, Page, View, layout};
use gtr_model::{Bar, MasterBar, Score, TimeSignature, demo_score};
use gtr_render::{Fonts, SvgOptions, pdf, png, svg};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The fonts the app exports with.
fn font_files() -> [Vec<u8>; 3] {
    [
        "Bravura.otf",
        "NotoSans-Regular.ttf",
        "NotoSans-SemiBold.ttf",
    ]
    .map(|name| std::fs::read(root().join("app/ui/public/fonts").join(name)).unwrap())
}

/// Keeps a file a test has made in the directory named by `GTR_KEEP`, to be looked at.
fn keep(name: &str, bytes: &[u8]) {
    if let Some(directory) = std::env::var_os("GTR_KEEP") {
        std::fs::write(Path::new(&directory).join(name), bytes).unwrap();
    }
}

/// The demo riff over and over, `bars` bars long.
fn long(bars: usize) -> Score {
    let mut score = demo_score();
    let riff: Vec<Bar> = score.tracks[0].staves[0].bars.clone();
    score.master_bars = vec![MasterBar::new(TimeSignature::COMMON); bars];
    score.tracks[0].staves[0].bars = (0..bars).map(|bar| riff[bar % 2].clone()).collect();
    score
}

fn find(haystack: &[u8], needle: &[u8]) -> Vec<usize> {
    (0..haystack.len().saturating_sub(needle.len() - 1))
        .filter(|at| haystack[*at..].starts_with(needle))
        .collect()
}

#[test]
fn a_pdf_has_a_page_for_each_page_and_says_where_everything_is() {
    let files = font_files();
    let fonts = Fonts::parse(&files[0], &files[1], &files[2]).unwrap();
    let score = long(60);
    let pages = layout(&score, &View::default(), Page::A4.width).pages(Page::A4);
    assert!(pages.len() >= 3);
    let file = pdf(&pages, &fonts, "Demo riff");
    keep("long.pdf", &file);
    assert!(file.starts_with(b"%PDF-1.4"));
    assert!(file.ends_with(b"%%EOF\n"));
    assert_eq!(find(&file, b"/Type /Page ").len(), pages.len());
    // A4, in points.
    assert!(!find(&file, b"/MediaBox [0 0 595.5 842.25]").is_empty());
    // The table at the end says where each object starts, and each starts there.
    let marker = *find(&file, b"startxref").last().unwrap();
    let tail = String::from_utf8_lossy(&file[marker + 9..]);
    let table: usize = tail
        .trim()
        .trim_end_matches("%%EOF")
        .trim()
        .parse()
        .unwrap();
    assert!(file[table..].starts_with(b"xref\n"));
    let listed = String::from_utf8_lossy(&file[table..marker]).into_owned();
    let entries: Vec<&str> = (listed.lines().skip(3))
        .take_while(|line| line.ends_with("n "))
        .collect();
    assert!(entries.len() > 20, "{}", entries.len());
    for (index, entry) in entries.iter().enumerate() {
        let offset: usize = entry[..10].parse().unwrap();
        let header = format!("{} 0 obj\n", index + 1);
        assert!(
            file[offset..].starts_with(header.as_bytes()),
            "object {}",
            index + 1
        );
    }
    // A glyph is in the file once, however often it is drawn.
    let forms = find(&file, b"/Subtype /Form").len();
    assert!((10..200).contains(&forms), "{forms}");
}

#[test]
fn a_png_is_the_score_in_pixels() {
    let files = font_files();
    let fonts = Fonts::parse(&files[0], &files[1], &files[2]).unwrap();
    let drawn = layout(&demo_score(), &View::default(), 800.0).printed();
    let file = png(&drawn, &fonts, 2.0);
    keep("demo.png", &file);
    assert!(file.starts_with(b"\x89PNG\r\n\x1a\n"));
    // The header: twice the layout's size, eight bits a colour, red green and blue.
    let (width, height) = (drawn.width as u32 * 2, (drawn.height * 2.0).ceil() as u32);
    assert_eq!(&file[12..16], b"IHDR");
    assert_eq!(file[16..20], width.to_be_bytes());
    assert_eq!(file[20..24], height.to_be_bytes());
    assert_eq!(file[24..26], [8, 2]);
    assert!(file.ends_with(b"IEND\xAE\x42\x60\x82"));
    // The pixels: mostly paper, with ink on it.
    let data = find(&file, b"IDAT")[0];
    let length = u32::from_be_bytes(file[data - 4..data].try_into().unwrap()) as usize;
    let rows =
        miniz_oxide::inflate::decompress_to_vec_zlib(&file[data + 4..data + 4 + length]).unwrap();
    assert_eq!(rows.len(), (width as usize * 3 + 1) * height as usize);
    let pixels: Vec<&[u8]> = (rows.chunks_exact(width as usize * 3 + 1))
        .flat_map(|row| row[1..].chunks_exact(3))
        .collect();
    let paper = pixels.iter().filter(|p| **p == [255, 255, 255]).count();
    let ink = pixels.iter().filter(|p| p[0] < 0x40).count();
    assert!(paper * 10 > pixels.len() * 8, "{paper} of {}", pixels.len());
    assert!(ink * 1000 > pixels.len() * 5, "{ink} of {}", pixels.len());
}

#[test]
fn an_svg_drawn_in_outlines_needs_no_fonts() {
    let files = font_files();
    let fonts = Fonts::parse(&files[0], &files[1], &files[2]).unwrap();
    let view = View {
        notation: Notation::Both,
        ..View::default()
    };
    let drawn: Layout = layout(&demo_score(), &view, 800.0);
    let options = SvgOptions {
        fonts: Some(&fonts),
        ..SvgOptions::default()
    };
    let file = svg(&drawn, options);
    keep("demo.svg", file.as_bytes());
    assert!(!file.contains("<text") && !file.contains("font-family"));
    // A path for each piece of text: the title, the tempo, clef and time, noteheads,
    // fret numbers.
    assert!(file.matches("<path d=\"M").count() > 30);
    // Fonts that are not fonts are refused.
    assert!(Fonts::parse(b"not a font", &files[1], &files[2]).is_err());
}
