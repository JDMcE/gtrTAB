//! MusicXML: what is written reads back, and files from other programs are read.

mod common;

use std::path::{Path, PathBuf};

use common::{band, corpus, difference, files};
use gtr_io::ImportError;
use gtr_io::musicxml::{export, import};
use gtr_model::{Bend, BendKind, Instrument, NoteValue, Score, Strum};

/// The score as MusicXML can say it: without what the format has no place for, which the
/// export lists.
fn as_written(mut score: Score) -> Score {
    for master in &mut score.master_bars {
        master.timer = false;
        master.tempo_gradual = false;
        master
            .tempo_changes
            .iter_mut()
            .for_each(|c| c.gradual = false);
    }
    for track in &mut score.tracks {
        track.mix.mute = false;
        track.mix.solo = false;
        let beats = (track.staves.iter_mut().flat_map(|s| &mut s.bars))
            .flat_map(|b| &mut b.voices)
            .flat_map(|v| &mut v.beats);
        for beat in beats {
            let effects = &mut beat.effects;
            (effects.whammy, effects.fade, effects.volume, effects.pan) = (None, None, None, None);
            effects
                .strum
                .take_if(|s| !matches!(s, Strum::ArpeggioDown | Strum::ArpeggioUp));
            for note in &mut beat.notes {
                note.effects.harmonic_touch = None;
                if let Some(trill) = &mut note.effects.trill {
                    trill.speed = NoteValue::Sixteenth;
                }
            }
        }
    }
    score
}

#[test]
fn a_score_with_everything_reads_back_as_it_was() {
    let score = band();
    let written = export(&score);
    common::keep("band.musicxml", &written.bytes);
    assert_eq!(
        written.skipped,
        [
            "changes of volume and pan",
            "fades",
            "how fast a trill is played",
            "mute and solo",
            "strums",
            "that a tempo changes by degrees",
            "timers",
            "whammy bar",
            "where a harmonic is touched"
        ]
    );
    let read = import(&written.bytes).unwrap();
    assert_eq!(read.skipped, [] as [&str; 0]);
    assert_eq!(difference(&as_written(score), &read.score), None);
}

/// Every MusicXML file of the corpus, and of the directory named by
/// `GTR_MUSICXML_SAMPLES` if that is set.
fn samples() -> Vec<PathBuf> {
    let extensions = ["xml", "musicxml", "mxl"];
    let mut found = files(&corpus(), &extensions);
    assert!(found.len() > 150, "{}", found.len());
    if let Some(more) = std::env::var_os("GTR_MUSICXML_SAMPLES") {
        found.extend(files(Path::new(&more), &extensions));
    }
    found
}

#[test]
fn every_file_is_read_and_reads_the_same_once_written_out() {
    for path in samples() {
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        let first =
            import(&std::fs::read(&path).unwrap()).unwrap_or_else(|e| panic!("{name}: {e}"));
        // What a MusicXML file says, MusicXML can say: nothing is lost on the way out.
        let written = export(&first.score);
        assert_eq!(written.skipped, [] as [&str; 0], "{name}");
        let second = import(&written.bytes).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(difference(&first.score, &second.score), None, "{name}");
    }
}

fn read(name: &str) -> Score {
    let path = corpus().join("alphatab").join(name);
    import(&std::fs::read(path).unwrap()).unwrap().score
}

/// The notes of a track's first staff and voice as strings and frets, bar by bar.
fn played(score: &Score, track: usize) -> Vec<Vec<Vec<(u8, u8)>>> {
    (score.tracks[track].staves[0].bars.iter())
        .map(|bar| {
            (bar.voices[0].beats.iter())
                .map(|beat| beat.notes.iter().map(|n| (n.string, n.fret)).collect())
                .collect()
        })
        .collect()
}

#[test]
fn tablature_is_read_with_its_strings_and_frets() {
    // Seven parts, each a staff of tablature with a tuning of its own.
    let score = read("musicxml-testsuite/71e-TabStaves.xml");
    let tunings: Vec<&[u8]> = (score.tracks.iter())
        .map(|t| t.staves[0].tuning.strings.as_slice())
        .collect();
    assert_eq!(tunings[0], [64, 59, 55, 50, 45, 40]);
    assert_eq!(tunings[2], [64, 57, 54, 50, 45, 38]);
    assert_eq!(tunings[4], [43, 38, 33, 28]);
    assert_eq!(tunings[5], [62, 60, 55, 50, 67]);
    let instruments: Vec<Instrument> = score.tracks.iter().map(|t| t.instrument).collect();
    assert_eq!(instruments[4], Instrument::Bass);
    assert_eq!(instruments[5], Instrument::Banjo);
    assert_eq!(instruments[7], Instrument::Ukulele);
    assert_eq!(
        played(&score, 0)[0],
        [vec![(3, 5)], vec![(1, 17)], vec![(1, 1)], vec![(3, 2)]]
    );
    assert_eq!(played(&score, 0)[1][0], [(2, 0), (3, 4), (5, 4)]);
}

#[test]
fn what_other_programs_write_for_a_guitar_is_read() {
    // Chord names with their diagrams, a hammer-on and a tie.
    let score = read("musicxml3/chord-diagram.musicxml");
    assert_eq!(score.tempo, 65);
    let beats = &score.tracks[0].staves[0].bars[0].voices[0].beats;
    assert_eq!(beats[0].effects.chord.as_deref(), Some("C"));
    assert_eq!(
        beats[0]
            .effects
            .diagram
            .as_ref()
            .map(|d| d.frets.as_slice()),
        Some([0, 1, 0, 2, 3, -1].as_slice())
    );
    assert_eq!(beats[4].effects.chord.as_deref(), Some("Fm"));
    let beats = &score.tracks[0].staves[0].bars[1].voices[0].beats;
    assert!(beats[2].notes[0].effects.hammer);
    assert!(beats[5].notes[0].tied);

    // Bends of each common shape, and one that goes up and down as it likes.
    let score = read("musicxml4/bends.xml");
    let bends: Vec<Option<Bend>> = (score.tracks[0].staves[0].bars[0].voices[0].beats.iter())
        .map(|beat| beat.notes[0].effects.bend.clone())
        .collect();
    assert_eq!(bends[0], Some(Bend::preset(BendKind::Bend, 2)));
    assert_eq!(bends[1], None);
    assert_eq!(bends[2], Some(Bend::preset(BendKind::BendRelease, 4)));
    assert_eq!(bends[3], Some(Bend::preset(BendKind::PrebendRelease, 2)));
    let free = &score.tracks[0].staves[0].bars[1].voices[0].beats[1].notes[0];
    let levels: Vec<i8> = (free.effects.bend.iter().flat_map(|b| &b.points))
        .map(|p| p.value)
        .collect();
    assert_eq!(levels, [0, 2, 1, 4, 0, 4]);

    // A dead note, as a cross for a notehead.
    let score = read("musicxml4/dead-note.xml");
    let dead: Vec<bool> = (score.tracks[0].staves[0].bars[0].voices[0].beats.iter())
        .map(|beat| beat.notes[0].effects.dead)
        .collect();
    assert_eq!(dead, [false, true]);
}

#[test]
fn drums_are_read_by_number_or_by_where_they_are_written() {
    // Notes given as pitches are the General MIDI numbers of their pieces.
    let score = read("musicxml4/percussion-articulation.xml");
    assert_eq!(score.tracks[0].instrument, Instrument::Drums);
    assert_eq!(played(&score, 0)[0], [vec![(0, 38)], vec![(0, 49)]]);
    // Without numbers, a note is the piece written nearest where it is.
    let score = read("musicxml-testsuite/73a-Percussion.xml");
    assert_eq!(score.tracks[0].instrument, Instrument::Piano);
    assert_eq!(score.tracks[1].instrument, Instrument::Drums);
    assert!(!played(&score, 1)[0].is_empty());
}

#[test]
fn a_compressed_file_and_a_timewise_one_are_read() {
    let score = read("musicxml3/compressed.mxl");
    assert_eq!(score.tracks[0].staves[0].bars[0].voices[0].beats.len(), 4);
    // The same bars listed part by part, and bar by bar.
    for name in ["basic", "anacrusis", "measure-numbering-none"] {
        let partwise = read(&format!("musicxml4/partwise-{name}.xml"));
        let timewise = read(&format!("musicxml4/timewise-{name}.xml"));
        assert_eq!(difference(&partwise, &timewise), None, "{name}");
        assert!(partwise.master_bars.len() >= 3);
    }
}

#[test]
fn what_is_not_musicxml_is_refused() {
    assert!(matches!(
        import(b"not a score"),
        Err(ImportError::UnknownFormat)
    ));
    assert!(matches!(
        import(b"<?xml version=\"1.0\"?><html></html>"),
        Err(ImportError::UnknownFormat)
    ));
    assert!(matches!(
        import(b"<score-partwise><part id=\"P1\"></part></score-partwise>"),
        Err(ImportError::Corrupt(_))
    ));
    // A file cut short is refused or read as far as it goes, and never more than that.
    let whole = export(&band()).bytes;
    for cut in (0..whole.len()).step_by(whole.len() / 300) {
        let _ = import(&whole[..cut]);
    }
}
