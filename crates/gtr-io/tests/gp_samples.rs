//! Imports every Guitar Pro file under `tests/corpus/`, and in the directory named by
//! `GTR_GP_SAMPLES` if that is set, and checks the results hang together: every file
//! opens, a real piece has bars that add up, and a piece saved in several versions of
//! the format reads as the same music from each. What particular files of the corpus
//! should hold is checked in `corpus.rs`. Run with `--nocapture` for a summary of each file.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use gtr_io::guitar_pro::{Imported, import};
use gtr_model::{Bar, Beat, Bend, Score};

const EXTENSIONS: [&str; 5] = ["gp3", "gp4", "gp5", "gpx", "gp"];

/// Share of bars with music whose first voice exactly fills the time signature.
fn complete_bars(score: &Score) -> (usize, usize) {
    let mut complete = 0;
    let mut total = 0;
    for staff in score.tracks.iter().flat_map(|t| &t.staves) {
        for (bar, master) in staff.bars.iter().zip(&score.master_bars) {
            let voice = &bar.voices[0];
            if voice.beats.is_empty() {
                continue;
            }
            total += 1;
            if voice.length() == master.time_signature.bar_length() {
                complete += 1;
            }
        }
    }
    (complete, total)
}

/// The first voice of a bar without its trailing rests: the XML formats pad short and
/// unused bars with rests, where the binary ones leave them short.
fn music(bar: &Bar) -> &[Beat] {
    let beats = &bar.voices[0].beats;
    let end = beats
        .iter()
        .rposition(|b| !b.is_rest())
        .map_or(0, |i| i + 1);
    &beats[..end]
}

/// A beat as every version can say it: its length, and its notes with their strings,
/// frets, ties and the techniques that all the formats have had from the start. Pitch
/// curves are reduced to their outline, since the formats place the points of the same
/// bend at slightly different moments.
fn common(beat: &Beat) -> String {
    let notes: Vec<String> = (beat.notes.iter())
        .map(|n| {
            let e = &n.effects;
            let bend = e.bend.as_ref().map(|b| Bend::preset(b.kind(), b.peak()));
            format!(
                "{}:{}{}{}{}{}{}{}",
                n.string,
                n.fret,
                if n.tied { " tied" } else { "" },
                if e.dead { " dead" } else { "" },
                if e.hammer { " hammer" } else { "" },
                if e.let_ring { " ring" } else { "" },
                if e.ghost { " ghost" } else { "" },
                bend.map_or(String::new(), |b| format!(
                    " bend {:?} {}",
                    b.kind(),
                    b.peak()
                )),
            )
        })
        .collect();
    format!("{:?} [{}]", beat.duration, notes.join(", "))
}

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

/// Where two imports of the same piece differ, if they do.
fn difference(a: &Score, b: &Score) -> Option<String> {
    if a.master_bars.len() != b.master_bars.len() {
        return Some(format!(
            "{} bars against {}",
            a.master_bars.len(),
            b.master_bars.len()
        ));
    }
    if a.tracks.len() != b.tracks.len() {
        return Some(format!(
            "{} tracks against {}",
            a.tracks.len(),
            b.tracks.len()
        ));
    }
    for (i, (ma, mb)) in a.master_bars.iter().zip(&b.master_bars).enumerate() {
        let of = |m: &gtr_model::MasterBar| {
            (
                m.time_signature,
                m.key_signature,
                m.repeat_start,
                m.repeat_count,
                m.alternate_endings,
            )
        };
        if of(ma) != of(mb) {
            return Some(format!("bar {}: {:?} against {:?}", i + 1, of(ma), of(mb)));
        }
    }
    for (t, (ta, tb)) in a.tracks.iter().zip(&b.tracks).enumerate() {
        if ta.staves[0].tuning != tb.staves[0].tuning {
            return Some(format!("track {}: tunings differ", t + 1));
        }
        let bars = ta.staves[0].bars.iter().zip(&tb.staves[0].bars);
        for (i, (bar_a, bar_b)) in bars.enumerate() {
            let (beats_a, beats_b) = (music(bar_a), music(bar_b));
            if beats_a.len() != beats_b.len() {
                return Some(format!(
                    "track {} bar {}: {} beats against {}",
                    t + 1,
                    i + 1,
                    beats_a.len(),
                    beats_b.len()
                ));
            }
            for (j, (beat_a, beat_b)) in beats_a.iter().zip(beats_b).enumerate() {
                if common(beat_a) != common(beat_b) {
                    return Some(format!(
                        "track {} bar {} beat {}: {} against {}",
                        t + 1,
                        i + 1,
                        j + 1,
                        common(beat_a),
                        common(beat_b)
                    ));
                }
            }
        }
    }
    None
}

#[test]
fn sample_files_import_cleanly() {
    let corpus = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/corpus/alphatab");
    check_directory(&corpus);
    if let Some(directory) = std::env::var_os("GTR_GP_SAMPLES") {
        check_directory(Path::new(&directory));
    }
}

fn check_directory(directory: &Path) {
    let mut paths = Vec::new();
    files(directory, &mut paths);
    paths.sort();
    assert!(!paths.is_empty(), "no sample files found");

    // The pieces of each name, with the directory and so the version each came from.
    let mut by_name: BTreeMap<String, Vec<(String, Score)>> = BTreeMap::new();
    for path in paths {
        // Named the same way on every system, with the slashes this file writes them with.
        let name = (path.strip_prefix(directory).unwrap().to_string_lossy()).replace('\\', "/");
        let bytes = std::fs::read(&path).unwrap();
        let Imported { score, skipped } = import(&bytes).unwrap_or_else(|e| panic!("{name}: {e}"));
        let (complete, total) = complete_bars(&score);
        let notes: usize = (score.tracks.iter().flat_map(|t| &t.staves))
            .flat_map(|s| &s.bars)
            .flat_map(|b| &b.voices)
            .flat_map(|v| &v.beats)
            .map(|b| b.notes.len())
            .sum();
        println!(
            "{name}: {} tracks, {} bars, {} bpm, {notes} notes, {complete}/{total} bars complete, \
             title {:?}, skipped {skipped:?}",
            score.tracks.len(),
            score.master_bars.len(),
            score.tempo,
            score.info.title,
        );
        // A misread binary file shows up as bars that do not add up. Small demonstration
        // files are often short on purpose, so only real pieces are held to this.
        assert!(
            total < 50 || complete * 10 >= total * 9,
            "{name}: only {complete} of {total} bars are complete"
        );
        assert!(!score.tracks.is_empty(), "{name}: no tracks");
        // Files made to show the same thing in each version have the same name in each
        // version's directory; version 8's are of other things.
        let stem = path.file_stem().unwrap().to_string_lossy().into_owned();
        if !name.starts_with("guitarpro8") {
            by_name.entry(stem).or_default().push((name, score));
        }
    }

    let mut differing = Vec::new();
    let mut compared = 0;
    for versions in by_name.values() {
        for pair in versions.windows(2) {
            compared += 1;
            if let Some(found) = difference(&pair[0].1, &pair[1].1) {
                differing.push(format!("{} and {}: {found}", pair[0].0, pair[1].0));
            }
        }
    }
    println!("{compared} pairs compared, {} differ", differing.len());
    for line in &differing {
        println!("  {line}");
    }
    let known = known_differences();
    let unexpected: Vec<&String> = (differing.iter())
        .filter(|line| !known.iter().any(|k| line.starts_with(k)))
        .collect();
    assert!(unexpected.is_empty(), "{unexpected:#?}");
}

/// Pairs of files that are not the same music, or that the formats cannot say alike, each
/// with why.
fn known_differences() -> Vec<&'static str> {
    // These share a name without being the same piece: each version's file was made
    // afresh, with more bars or tracks, or other harmonics, than the last.
    vec![
        "guitarpro5/beaming-mode.gp5 and guitarpro6/beaming-mode.gpx",
        "guitarpro6/beaming-mode.gpx and guitarpro7/beaming-mode.gp",
        "guitarpro5/chords.gp5 and guitarpro6/chords.gpx",
        "guitarpro4/harmonic-types.gp4 and guitarpro5/harmonic-types.gp5",
        "guitarpro5/ottavia.gp5 and guitarpro7/ottavia.gp",
        "guitarpro4/score-info.gp4 and guitarpro5/score-info.gp5",
        "guitarpro6/slash.gpx and guitarpro7/slash.gp",
    ]
}

/// A damaged file is refused, or read as far as it makes sense: it never brings the
/// program down. Every file of the corpus is cut short at a few places, and has a few of
/// its bytes changed, and is imported so.
#[test]
fn damaged_files_do_not_panic() {
    let corpus = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/corpus/alphatab");
    let mut paths = Vec::new();
    files(&corpus, &mut paths);
    paths.sort();
    let mut tried = 0;
    for path in paths {
        let bytes = std::fs::read(&path).unwrap();
        // The zipped format is checked by its own sums; the others are read as they come.
        let cuts = [1, 3, 5, 7, 9].map(|tenth| bytes.len() * tenth / 10);
        for cut in cuts {
            let _ = import(&bytes[..cut]);
            tried += 1;
        }
        for step in [7usize, 31, 127, 509] {
            let mut damaged = bytes.clone();
            let mut at = step;
            while at < damaged.len() {
                damaged[at] = damaged[at].wrapping_mul(31).wrapping_add(at as u8) ^ 0x55;
                at += step * 37 + 11;
            }
            let _ = import(&damaged);
            tried += 1;
        }
    }
    assert!(tried > 1000);
}
