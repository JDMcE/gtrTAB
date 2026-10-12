//! The importers against another program's reading of the same files.

mod common;

use common::corpus;
use gtr_model::Score;
use serde_json::Value;

/// The pitches a track sounds in each bar, lowest first: every note of every staff and
/// voice, its grace notes with it.
fn pitches(score: &Score, track: usize) -> Vec<Vec<i64>> {
    let track = &score.tracks[track];
    let drums = track.instrument.is_percussion();
    (0..score.master_bars.len())
        .map(|bar| {
            let mut found = Vec::new();
            for staff in &track.staves {
                let notes = (staff.bars[bar].voices.iter())
                    .flat_map(|v| &v.beats)
                    .flat_map(|b| &b.notes);
                for note in notes {
                    let pitch = |string: u8, fret: u8| match drums {
                        true => Some(fret),
                        false => staff.pitch(string, fret),
                    };
                    found.extend(pitch(note.string, note.fret).map(i64::from));
                    for grace in note.effects.grace.iter().flat_map(|g| &g.notes) {
                        found.extend(pitch(grace.string, grace.fret).map(i64::from));
                    }
                }
            }
            found.sort_unstable();
            found
        })
        .collect()
}

/// The files alphaTab and the importers here read differently, with why. Everything else
/// in the corpus they read alike, note for note.
const DIFFERENT: [(&str, &str); 20] = [
    (
        "guitarpro5/percussion-all.gp5",
        "alphaTab gives the pieces of a kit numbers of its own; here they keep the file's",
    ),
    (
        "guitarpro7/fermata.gp",
        "ends on a grace note that leads into nothing, which is left out and said to be",
    ),
    (
        "musicxml-testsuite/01d-Pitches-Microtones.xml",
        "quarter tones go to the nearer semitone one way here and the other there",
    ),
    (
        "musicxml-testsuite/01f-Pitches-ParenthesizedMicrotoneAccidentals.xml",
        "quarter tones again",
    ),
    (
        "musicxml-testsuite/03a-Rhythm-Durations.xml",
        "has notes shorter than a 256th, which are left out and said to be",
    ),
    (
        "musicxml-testsuite/24a-GraceNotes.xml",
        "ends on grace notes that lead into nothing",
    ),
    (
        "musicxml-testsuite/24c-GraceNote-MeasureEnd.xml",
        "grace notes at the end of a bar are given to the first note of the next",
    ),
    (
        "musicxml-testsuite/24d-AfterGrace.xml",
        "grace notes after a note are given to the note that follows them",
    ),
    (
        "musicxml-testsuite/24e-GraceNote-StaffChange.xml",
        "grace notes on another staff than their note wait for a note on their own",
    ),
    (
        "musicxml-testsuite/24h-GraceNote-Simultaneous.xml",
        "has a third staff, which is left out and said to be",
    ),
    (
        "musicxml-testsuite/33f-Trill-EndingOnGraceNote.xml",
        "grace notes at the end of a bar again",
    ),
    (
        "musicxml-testsuite/34b-Colors.xml",
        "a note with no pitch is no note here, and pitch 0 to alphaTab",
    ),
    (
        "musicxml-testsuite/34c-Font-Size.xml",
        "a note with no pitch again",
    ),
    (
        "musicxml-testsuite/41c-StaffGroups.xml",
        "percussion without numbers, and a part of three staves",
    ),
    (
        "musicxml-testsuite/41h-TooManyParts.xml",
        "parts the part list does not name are read here and not by alphaTab",
    ),
    (
        "musicxml-testsuite/71e-TabStaves.xml",
        "has chords with two notes on one string, of which one is kept",
    ),
    (
        "musicxml-testsuite/73a-Percussion.xml",
        "percussion without numbers is the piece written there, and 0 to alphaTab",
    ),
    (
        "musicxml4/dead-note.xml",
        "alphaTab counts a note on its staff and again on its tablature",
    ),
    (
        "musicxml4/percussion-instrument-vs-pitched.xml",
        "notes on a percussion clef that are not percussion",
    ),
    (
        "musicxml4/string-annotation.xml",
        "alphaTab takes a string number without a fret as a place on the neck",
    ),
];

/// How a file reads here differs from how alphaTab read it, or `None` if it does not.
fn difference(name: &str, theirs: &Value) -> Option<String> {
    let bytes = std::fs::read(corpus().join("alphatab").join(name)).unwrap();
    let ours = gtr_io::import(name, &bytes)
        .unwrap_or_else(|e| panic!("{name}: {e}"))
        .score;
    let their_tracks = theirs["tracks"].as_array().unwrap();
    if their_tracks.len() != ours.tracks.len() {
        return Some(format!(
            "{} tracks, not {}",
            ours.tracks.len(),
            their_tracks.len()
        ));
    }
    if theirs["bars"].as_u64() != Some(ours.master_bars.len() as u64) {
        return Some(format!(
            "{} bars, not {}",
            ours.master_bars.len(),
            theirs["bars"]
        ));
    }
    for (track, their_track) in their_tracks.iter().enumerate() {
        let mine = pitches(&ours, track);
        for (bar, their_bar) in their_track.as_array().unwrap().iter().enumerate() {
            let their_bar: Vec<i64> = (their_bar.as_array().unwrap().iter())
                .filter_map(Value::as_i64)
                .collect();
            if mine[bar] != their_bar {
                return Some(format!(
                    "track {track} bar {bar}: {:?}, not {their_bar:?}",
                    mine[bar]
                ));
            }
        }
    }
    None
}

#[test]
fn the_corpus_reads_as_alphatab_reads_it() {
    // What alphaTab found in every Guitar Pro and MusicXML file of the corpus: for each
    // track and bar, the pitches of its notes. `alphatab.mjs` beside it wrote it.
    let path = corpus().join("reference/alphatab.json");
    let reference: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let files = reference.as_object().unwrap();
    assert!(files.len() > 390, "{}", files.len());
    let mut alike = 0;
    for (name, theirs) in files {
        assert!(
            theirs.get("error").is_none(),
            "{name}: alphaTab could not read it"
        );
        let known = DIFFERENT.iter().find(|(file, _)| file == name);
        match (difference(name, theirs), known) {
            (None, None) => alike += 1,
            (Some(_), Some(_)) => {}
            (Some(found), None) => panic!("{name} is read differently: {found}"),
            (None, Some((_, why))) => panic!("{name} is read alike now; it was not because {why}"),
        }
    }
    assert_eq!(alike + DIFFERENT.len(), files.len());
}
