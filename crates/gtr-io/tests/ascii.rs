//! Tablature as plain text: what is written reads back, and tabs typed the way people
//! type them are read.

mod common;

use common::{band, corpus};
use gtr_io::ImportError;
use gtr_io::ascii::{export, import};
use gtr_model::{
    Beat, Bend, BendKind, Duration, Harmonic, Instrument, NoteValue, Rational, Score, Slide,
    Vibrato,
};

/// A beat as tablature says it: its length, and each of its notes with the techniques
/// that have a sign.
fn said(beat: &Beat) -> String {
    let notes: Vec<String> = (beat.notes.iter())
        .map(|note| {
            let e = &note.effects;
            let bend = e.bend.as_ref().map(|b| (b.kind(), b.peak()));
            format!(
                "{}:{}{}{}{}{}{}{}{}{:?}{:?}",
                note.string,
                note.fret,
                if note.tied { " tied" } else { "" },
                if e.dead { " dead" } else { "" },
                if e.ghost { " ghost" } else { "" },
                if e.hammer { " hammer" } else { "" },
                if e.tapped { " tapped" } else { "" },
                if e.harmonic.is_some() {
                    " harmonic"
                } else {
                    ""
                },
                if e.vibrato.is_some() { " vibrato" } else { "" },
                e.slide,
                bend,
            )
        })
        .collect();
    let lyric = beat.effects.lyrics.first().cloned().unwrap_or_default();
    format!(
        "{:?} {notes:?} {:?} {lyric}",
        beat.duration, beat.effects.chord
    )
}

#[test]
fn a_score_reads_back_as_far_as_tablature_says_it() {
    let score = band();
    let written = export(&score);
    common::keep("band.txt", &written.bytes);
    let text = String::from_utf8(written.bytes.clone()).unwrap();
    // The first line of the guitar's music, as it stands.
    let lines: Vec<&str> = text.lines().collect();
    let first = lines
        .iter()
        .position(|line| line.starts_with("E|"))
        .unwrap();
    assert_eq!(lines[first - 2], "    Am7                   C");
    assert_eq!(lines[first - 1], "    Q    Q   Q   E  E     H       Q   Q");
    assert_eq!(lines[first], "E|:-5b7~----------------|-0---------------:|");
    assert_eq!(
        lines[first + 1],
        "B|:------7h--8/--10-----|-1---------------:|"
    );
    assert_eq!(
        lines[first + 2],
        "G|:-----------------[9]-|-0---------------:|"
    );
    assert_eq!(lines[first + 6], "    Oh   my");
    assert!(text.contains("HH|:-x-x-x-x-x-x-x-x-|"));
    // What plain text has no sign for is listed, and the piano has no tablature.
    for lost in [
        "dynamics",
        "tracks for instruments without strings",
        "whammy bar",
    ] {
        assert!(written.skipped.iter().any(|s| s == lost), "{lost}");
    }

    let read = import(&written.bytes).unwrap();
    assert_eq!(read.skipped, [] as [&str; 0]);
    let back = read.score;
    assert_eq!(back.info, score.info);
    assert_eq!(back.tempo, 120);
    let times: Vec<(u8, u8)> = (back.master_bars.iter())
        .map(|m| (m.time_signature.numerator, m.time_signature.denominator))
        .collect();
    assert_eq!(times, [(4, 4), (4, 4), (3, 4), (4, 4)]);
    assert_eq!(back.master_bars[0].section.as_deref(), Some("Intro"));
    assert!(back.master_bars[0].repeat_start);
    assert_eq!(back.master_bars[1].repeat_count, 2);
    assert_eq!(back.master_bars[2].tempo, Some(90));
    assert_eq!(back.master_bars[3].tempo, Some(140));

    // The guitar, the bass and the drums, each with its strings and its capo.
    let kept = [0, 1, 3];
    let names: Vec<(&str, Instrument)> = (back.tracks.iter())
        .map(|t| (t.name.as_str(), t.instrument))
        .collect();
    assert_eq!(
        names,
        [
            ("Lead", Instrument::DistortionGuitar),
            ("Bass", Instrument::Bass),
            ("Drums", Instrument::Drums)
        ]
    );
    for (read, original) in back.tracks.iter().zip(kept) {
        let original = &score.tracks[original];
        assert_eq!(read.staves[0].tuning, original.staves[0].tuning);
        assert_eq!(read.staves[0].capo, original.staves[0].capo);
        for (bar, (read, original)) in (read.staves[0].bars.iter())
            .zip(&original.staves[0].bars)
            .enumerate()
        {
            // The one bar with a second voice is written as one voice, and is not the same.
            if original.voices.len() > 1 {
                continue;
            }
            let read: Vec<String> = read.voices[0].beats.iter().map(said).collect();
            let original: Vec<String> = original.voices[0].beats.iter().map(said).collect();
            assert_eq!(read, original, "{} bar {bar}", read.len());
        }
    }
}

fn read(name: &str) -> Score {
    let path = corpus().join("ascii").join(name);
    import(&std::fs::read(path).unwrap()).unwrap().score
}

/// A beat as its length and the fret on each string it plays.
type Played = (Rational, Vec<(u8, u8)>);

/// The beats of a track's first voice, bar by bar, as lengths and the frets on each string.
fn played(score: &Score, track: usize) -> Vec<Vec<Played>> {
    (score.tracks[track].staves[0].bars.iter())
        .map(|bar| {
            (bar.voices[0].beats.iter())
                .map(|beat| {
                    let notes = beat.notes.iter().map(|n| (n.string, n.fret)).collect();
                    (beat.duration.length(), notes)
                })
                .collect()
        })
        .collect()
}

#[test]
fn a_tune_typed_without_note_lengths_gets_them_from_its_spacing() {
    let score = read("ode-to-joy.txt");
    assert_eq!(
        score.info.title,
        "Ode to Joy (L. v. Beethoven), first line of the tune"
    );
    assert_eq!(score.tracks[0].instrument, Instrument::SteelGuitar);
    assert_eq!(
        score.tracks[0].staves[0].tuning.strings,
        [64, 59, 55, 50, 45, 40]
    );
    let quarter = Rational::new(1, 4);
    let tune = played(&score, 0);
    assert_eq!(tune.len(), 4);
    assert_eq!(
        tune[0],
        [
            (quarter, vec![(0, 0)]),
            (quarter, vec![(0, 0)]),
            (quarter, vec![(0, 1)]),
            (quarter, vec![(0, 3)])
        ]
    );
    assert_eq!(tune[2][3], (quarter, vec![(2, 0)]));
    // The last bar: a dotted quarter, an eighth and a half.
    assert_eq!(
        tune[3],
        [
            (Rational::new(3, 8), vec![(0, 0)]),
            (Rational::new(1, 8), vec![(1, 3)]),
            (Rational::new(1, 2), vec![(1, 3)])
        ]
    );
}

#[test]
fn the_signs_people_type_for_techniques_are_read() {
    let score = read("blues-lick.txt");
    assert_eq!(score.info.title, "");
    let staff = &score.tracks[0].staves[0];
    assert_eq!(staff.capo, 2);
    assert_eq!(staff.tuning.strings, [64, 59, 55, 50, 45, 40]);
    assert!(!score.master_bars[0].repeat_start && score.master_bars[1].repeat_start);
    assert_eq!(score.master_bars[1].repeat_count, 2);

    let first = &staff.bars[0].voices[0].beats;
    let frets: Vec<(u8, u8)> = (first.iter().flat_map(|b| &b.notes))
        .map(|n| (n.string, n.fret))
        .collect();
    assert_eq!(frets, [(2, 5), (2, 7), (1, 5), (1, 8), (1, 5), (2, 7)]);
    assert_eq!(first[0].effects.chord.as_deref(), Some("A7"));
    assert_eq!(first[0].effects.lyrics, ["Oh"]);
    // Hammered on from the fifth fret to the seventh, typed without a gap.
    assert!(first[0].notes[0].effects.hammer && !first[1].notes[0].effects.hammer);
    // Bent from the eighth fret to the note of the tenth, a whole tone, and let down.
    assert_eq!(
        first[3].notes[0].effects.bend,
        Some(Bend::preset(BendKind::BendRelease, 4))
    );

    let second = &staff.bars[1].voices[0].beats;
    // The chord's name goes to the beat typed under it, not to the dead note before.
    assert!(second[0].notes[0].effects.dead);
    assert_eq!(second[0].effects.chord, None);
    assert_eq!(second[1].effects.chord.as_deref(), Some("D7"));
    // A slide from the fifth fret to the seventh, a fret of two figures with vibrato, a
    // pull-off, and a note in brackets at the fret of the one before: tied to it.
    assert_eq!(second[1].notes[0].effects.slide, Some(Slide::Shift));
    assert_eq!(
        (second[4].notes[0].fret, second[4].notes[0].effects.vibrato),
        (10, Some(Vibrato::Slight))
    );
    assert!(second[5].notes[0].effects.hammer);
    let last = second.last().unwrap();
    assert_eq!((last.notes[0].fret, last.notes[0].tied), (5, true));
    assert!(
        staff
            .bars
            .iter()
            .all(|bar| bar.voices[0].beats.iter().all(|b| b.effects.is_valid()))
    );
    assert_eq!(first[0].notes[0].effects.harmonic, None::<Harmonic>);
}

#[test]
fn drum_tab_and_bass_tab_are_read() {
    let score = read("rock-beat.txt");
    assert_eq!(score.tracks[0].instrument, Instrument::Drums);
    let eighth = Duration::new(NoteValue::Eighth).length();
    let beat = played(&score, 0);
    let pieces = |bar: usize| -> Vec<Vec<u8>> {
        beat[bar]
            .iter()
            .map(|b| b.1.iter().map(|n| n.1).collect())
            .collect()
    };
    // A crash with the bass drum, then the hi-hat in eighths with the snare on two and
    // four.
    assert_eq!(
        pieces(0),
        [
            vec![49, 36],
            vec![42],
            vec![42, 38],
            vec![42],
            vec![42, 36],
            vec![42, 36],
            vec![42, 38],
            vec![42]
        ]
    );
    assert!(beat[0].iter().all(|b| b.0 == eighth));
    // The second bar ends with the hi-hat open and two sixteenths on the snare.
    let lengths: Vec<Rational> = beat[1].iter().map(|b| b.0).collect();
    assert_eq!(lengths[..7], [eighth; 7]);
    assert_eq!(lengths[7..], [eighth / 2, eighth / 2]);
    assert_eq!(pieces(1)[7..], [vec![46, 38], vec![38]]);

    let score = read("walking-bass.txt");
    assert_eq!(score.tempo, 132);
    assert_eq!(score.tracks[0].instrument, Instrument::Bass);
    assert_eq!(score.tracks[0].staves[0].tuning.strings, [43, 38, 33, 28]);
    let line = played(&score, 0);
    // Two lines of two bars, every note a quarter; the double bar at the end is no bar.
    assert_eq!(line.len(), 4);
    let walked: Vec<(u8, u8)> = (line.iter().flatten()).flat_map(|b| b.1.clone()).collect();
    assert_eq!(
        walked,
        [
            (3, 3),
            (2, 2),
            (2, 5),
            (1, 2),
            (2, 3),
            (1, 0),
            (1, 2),
            (0, 0),
            (1, 5),
            (1, 4),
            (1, 2),
            (1, 0),
            (3, 3),
            (2, 2),
            (2, 0),
            (3, 3)
        ]
    );
    assert!(line.iter().flatten().all(|b| b.0 == Rational::new(1, 4)));
}

#[test]
fn what_is_not_tablature_is_refused() {
    assert!(matches!(import(b""), Err(ImportError::UnknownFormat)));
    assert!(matches!(
        import(b"Dear all,\n\nthe minutes of the meeting - such as they are - follow.\n"),
        Err(ImportError::UnknownFormat)
    ));
    assert!(matches!(
        import(b"MThd\0\0\0\x06"),
        Err(ImportError::UnknownFormat)
    ));
    // Text cut short, or with lines swapped for noise, is read as far as it makes sense.
    let whole = export(&band()).bytes;
    for cut in (0..whole.len()).step_by(11) {
        let _ = import(&whole[..cut]);
        let mut noisy = whole.clone();
        noisy[cut] = b"|-x9(~\n"[cut % 7];
        let _ = import(&noisy);
    }
}
