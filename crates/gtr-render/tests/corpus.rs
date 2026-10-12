//! The corpus drawn: every file that can be imported is laid out in every view, with
//! nothing off the page and every bar in its place, and a few of them are compared
//! with the pictures of them kept in `tests/golden/corpus`.
//!
//! As with the other golden pictures, a difference leaves the new picture in
//! `target/golden/corpus`, and `GTR_BLESS=1` keeps it.

use std::path::{Path, PathBuf};

use gtr_layout::{BarStatus, Item, Layout, Notation, Page, View, layout};
use gtr_model::Score;
use gtr_render::{SvgOptions, svg};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

const EXTENSIONS: [&str; 11] = [
    "gp3", "gp4", "gp5", "gpx", "gp", "xml", "musicxml", "mxl", "mid", "txt", "tab",
];

fn files(directory: &Path, found: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(directory).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            files(&path, found);
        } else if (path.extension().and_then(|e| e.to_str()))
            .is_some_and(|e| EXTENSIONS.contains(&e))
        {
            found.push(path);
        }
    }
}

fn import(path: &Path) -> Score {
    let name = path.file_name().unwrap().to_string_lossy();
    let bytes = std::fs::read(path).unwrap();
    gtr_io::import(&name, &bytes)
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()))
        .score
}

/// The places an item is drawn from and to.
fn points(item: &Item) -> Vec<(f64, f64)> {
    match item {
        Item::Line { x1, y1, x2, y2, .. } | Item::Beam { x1, y1, x2, y2, .. } => {
            vec![(*x1, *y1), (*x2, *y2)]
        }
        Item::Rect { x, y, w, h, .. } => vec![(*x, *y), (x + w, y + h)],
        Item::Dot { x, y, .. } | Item::Text { x, y, .. } => vec![(*x, *y)],
        Item::Arc { x1, x2, y, .. } => vec![(*x1, *y), (*x2, *y)],
        Item::Curve {
            x1,
            y1,
            cx,
            cy,
            x2,
            y2,
        } => vec![(*x1, *y1), (*cx, *cy), (*x2, *y2)],
    }
}

/// Checks that a layout is one a page can show: everything on it, every bar placed.
fn check(name: &str, score: &Score, drawn: &Layout) {
    assert!(drawn.width > 0.0 && drawn.height > 0.0, "{name}");
    for item in &drawn.items {
        for (x, y) in points(item) {
            assert!(x.is_finite() && y.is_finite(), "{name}: {item:?}");
            // A mark may hang a little over the edge of the paper, and no more.
            assert!(
                (-40.0..=drawn.width + 40.0).contains(&x),
                "{name}: {item:?}"
            );
            assert!(
                (-40.0..=drawn.height + 40.0).contains(&y),
                "{name}: {item:?}"
            );
        }
    }
    assert_eq!(drawn.bars.len(), score.master_bars.len(), "{name}");
    assert!(!drawn.systems.is_empty(), "{name}");
    for (index, bar) in drawn.bars.iter().enumerate() {
        assert!(
            bar.w > 0.0 && bar.system < drawn.systems.len(),
            "{name} bar {index}"
        );
        assert!(
            bar.beats.windows(2).all(|pair| pair[0] < pair[1]),
            "{name} bar {index}: {:?}",
            bar.beats
        );
        assert!(
            bar.beats
                .iter()
                .all(|x| (bar.x..=bar.x + bar.w).contains(x)),
            "{name} bar {index}"
        );
    }
}

#[test]
fn every_file_of_the_corpus_is_drawn_in_every_view() {
    let mut found = Vec::new();
    files(&root().join("tests/corpus"), &mut found);
    found.sort();
    assert!(found.len() > 400, "{}", found.len());
    let mut bars = (0, 0);
    for path in &found {
        let name = path.strip_prefix(root()).unwrap().display().to_string();
        let score = import(path);
        for (notation, multitrack) in [
            (Notation::Tab, false),
            (Notation::Standard, false),
            (Notation::Both, true),
            (Notation::Slash, false),
        ] {
            let view = View {
                notation,
                multitrack,
                ..View::default()
            };
            let drawn = layout(&score, &view, 1000.0);
            check(&format!("{name} as {notation:?}"), &score, &drawn);
            if notation == Notation::Both {
                // On paper too, and as a picture.
                let pages = layout(&score, &view, Page::A4.width).pages(Page::A4);
                assert!(!pages.is_empty(), "{name}");
                assert!(svg(&drawn, SvgOptions::default()).len() > 200, "{name}");
                bars.0 += drawn.bars.len();
                bars.1 += (drawn.bars.iter())
                    .filter(|bar| matches!(bar.status, BarStatus::Complete | BarStatus::Empty))
                    .count();
            }
        }
    }
    // Recognisably music: nine bars in ten of the corpus add up to their time or are
    // empty. The rest are in files made with bars that do not: pickups, bars left
    // short or overfull to show one thing, and the importers' own hard cases.
    assert!(bars.1 * 100 >= bars.0 * 90, "{} of {} bars", bars.1, bars.0);
}

/// Compares a file of the corpus, drawn, with the picture of it that is kept.
fn compare(file: &str, notation: Notation, multitrack: bool) {
    let score = import(&root().join("tests/corpus").join(file));
    let view = View {
        notation,
        multitrack,
        ..View::default()
    };
    let options = SvgOptions {
        music_font_url: Some("../../../app/ui/public/fonts/Bravura.woff2"),
        ..SvgOptions::default()
    };
    let drawn = svg(&layout(&score, &view, 1000.0), options);
    let name = Path::new(file)
        .file_name()
        .unwrap()
        .to_string_lossy()
        .replace('.', "-");
    let golden = root()
        .join("tests/golden/corpus")
        .join(format!("{name}.svg"));
    if std::env::var_os("GTR_BLESS").is_some() {
        std::fs::create_dir_all(golden.parent().unwrap()).unwrap();
        std::fs::write(&golden, &drawn).unwrap();
        return;
    }
    let kept = std::fs::read_to_string(&golden).unwrap_or_default();
    if !kept.lines().eq(drawn.lines()) {
        let out = root().join("target/golden/corpus");
        std::fs::create_dir_all(&out).unwrap();
        let new = out.join(format!("{name}.svg"));
        std::fs::write(&new, &drawn).unwrap();
        let changed = (kept.lines().zip(drawn.lines()))
            .filter(|(a, b)| a != b)
            .count()
            + kept.lines().count().abs_diff(drawn.lines().count());
        panic!(
            "{file} is not drawn as in {}: {changed} elements differ. The new picture is in \
             {}. If the change is meant, run again with GTR_BLESS=1 and commit the picture.",
            golden.display(),
            new.display()
        );
    }
}

#[test]
fn files_of_each_format_are_drawn_as_they_were() {
    for (file, notation, multitrack) in [
        // Guitar Pro, a version each: techniques, bends, chords, lyrics, drums, a band.
        ("alphatab/guitarpro3/effects.gp3", Notation::Both, false),
        ("alphatab/guitarpro4/bends.gp4", Notation::Tab, false),
        ("alphatab/guitarpro5/chords.gp5", Notation::Both, false),
        ("alphatab/guitarpro6/tuplets.gpx", Notation::Both, false),
        ("alphatab/guitarpro7/beat-lyrics.gp", Notation::Both, false),
        ("alphatab/guitarpro8/directions.gp", Notation::Tab, false),
        (
            "alphatab/visual-tests/percussion.gp",
            Notation::Standard,
            true,
        ),
        // MusicXML: tablature, a diagram over a chord, and a piano's two staves.
        (
            "alphatab/musicxml-testsuite/71e-TabStaves.xml",
            Notation::Tab,
            true,
        ),
        (
            "alphatab/musicxml3/chord-diagram.musicxml",
            Notation::Both,
            false,
        ),
        (
            "alphatab/musicxml-testsuite/43a-PianoStaff.xml",
            Notation::Standard,
            false,
        ),
        // MIDI, and tabs typed as text.
        ("mutopia/carcassi-op60-01.mid", Notation::Both, false),
        ("ascii/blues-lick.txt", Notation::Both, false),
        ("ascii/rock-beat.txt", Notation::Standard, false),
    ] {
        compare(file, notation, multitrack);
    }
}
