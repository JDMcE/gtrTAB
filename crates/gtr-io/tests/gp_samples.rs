//! Imports every Guitar Pro file in the directory named by `GTR_GP_SAMPLES` and checks the
//! results hang together. Does nothing when the variable is not set, because no sample files
//! are kept in the repository yet. Run with `--nocapture` for a summary of each file.

use std::collections::BTreeMap;
use std::path::PathBuf;

use gtr_io::guitar_pro::{Imported, import};
use gtr_model::Score;

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

/// The first voice of a bar without its trailing rests: Guitar Pro 7 pads short and unused
/// bars with rests, where Guitar Pro 5 leaves them short.
fn music(bar: &gtr_model::Bar) -> &[gtr_model::Beat] {
    let beats = &bar.voices[0].beats;
    let end = beats
        .iter()
        .rposition(|b| !b.is_rest())
        .map_or(0, |i| i + 1);
    &beats[..end]
}

#[test]
fn sample_files_import_cleanly() {
    let Some(directory) = std::env::var_os("GTR_GP_SAMPLES") else {
        return;
    };
    let mut paths: Vec<PathBuf> = std::fs::read_dir(directory)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|p| matches!(p.extension().and_then(|e| e.to_str()), Some("gp" | "gp5")))
        .collect();
    paths.sort();
    assert!(!paths.is_empty(), "no sample files found");

    let mut by_name: BTreeMap<String, Vec<(String, Score)>> = BTreeMap::new();
    for path in paths {
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
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
        // A misread binary file shows up as bars that do not add up. Small demonstration files
        // are often short on purpose, so only real pieces are held to this.
        assert!(
            total < 50 || complete * 10 >= total * 9,
            "{name}: only {complete} of {total} bars are complete"
        );
        assert!(!score.tracks.is_empty(), "{name}: no tracks");
        let stem = path.file_stem().unwrap().to_string_lossy().into_owned();
        by_name.entry(stem).or_default().push((name, score));
    }

    // The same piece saved as .gp and .gp5 must import to the same music.
    for versions in by_name.values() {
        if let [(a_name, a), (b_name, b)] = versions.as_slice() {
            assert_eq!(a.master_bars, b.master_bars, "{a_name} vs {b_name}: bars");
            assert_eq!(
                a.tracks.len(),
                b.tracks.len(),
                "{a_name} vs {b_name}: tracks"
            );
            for (ta, tb) in a.tracks.iter().zip(&b.tracks) {
                assert_eq!(
                    ta.staves[0].tuning, tb.staves[0].tuning,
                    "{a_name} vs {b_name}"
                );
                let bars = ta.staves[0].bars.iter().zip(&tb.staves[0].bars);
                for (i, (bar_a, bar_b)) in bars.enumerate() {
                    assert_eq!(music(bar_a), music(bar_b), "{a_name} vs {b_name}: bar {i}");
                }
            }
        }
    }
}
