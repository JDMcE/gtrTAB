//! MIDI: a score is written as it plays, and what a file plays is read as a score.

mod common;

use common::{band, beat, corpus, files};
use gtr_io::ImportError;
use gtr_io::midi::{export, import};
use gtr_model::{
    Dynamic, Instrument, NoteValue, Rational, Score, TimeSignature, Track, demo_score, play_order,
};

/// What a track sounds in each bar: for every beat of its first staff and voice that has
/// notes, where in the bar it starts, how long it lasts and the pitches it sounds.
fn sounded(score: &Score, track: usize) -> Vec<Vec<(Rational, Rational, Vec<u8>)>> {
    let staff = &score.tracks[track].staves[0];
    (staff.bars.iter())
        .map(|bar| {
            let mut start = Rational::from_integer(0);
            let mut out = Vec::new();
            for beat in &bar.voices[0].beats {
                let length = beat.duration.length();
                let mut pitches: Vec<u8> = (beat.notes.iter())
                    .filter_map(|n| staff.pitch(n.string, n.fret))
                    .collect();
                pitches.sort_unstable();
                if !pitches.is_empty() {
                    out.push((start, length, pitches));
                }
                start += length;
            }
            out
        })
        .collect()
}

#[test]
fn a_plain_score_reads_back_as_it_sounds() {
    // The demo riff with a bass under it, a little faster and a little to one side.
    let mut score = demo_score();
    score.tempo = 96;
    score.info.title = "Riff".into();
    score.master_bars[1].key_signature = 2;
    let mut bass = Track::empty("Low end", Instrument::Bass, 2);
    bass.staves[0].bars[0].voices[0].beats = vec![beat(NoteValue::Whole, &[(3, 0)])];
    bass.staves[0].bars[1].voices[0].beats = vec![
        beat(NoteValue::Half, &[(3, 3)]),
        beat(NoteValue::Quarter, &[(2, 2)]),
        beat(NoteValue::Quarter, &[(2, 0)]),
    ];
    bass.mix.volume = 60;
    bass.mix.pan = -100;
    score.tracks.push(bass);

    let written = export(&score);
    assert_eq!(written.skipped, [] as [&str; 0]);
    // A header, then a track for the tempo and one for each of the score's.
    assert_eq!(&written.bytes[..4], b"MThd");
    assert_eq!(&written.bytes[8..14], [0, 1, 0, 3, 0x03, 0xC0]);
    let read = import(&written.bytes).unwrap();
    assert_eq!(read.skipped, [] as [&str; 0]);
    let back = &read.score;
    assert_eq!((back.tempo, back.info.title.as_str()), (96, "Riff"));
    assert_eq!(back.master_bars.len(), 2);
    assert_eq!(back.master_bars[0].time_signature, TimeSignature::COMMON);
    assert_eq!(back.master_bars[1].key_signature, 2);
    let tracks: Vec<(&str, Instrument, u8, i8)> = (back.tracks.iter())
        .map(|t| (t.name.as_str(), t.instrument, t.mix.volume, t.mix.pan))
        .collect();
    assert_eq!(
        tracks,
        [
            ("Guitar", Instrument::SteelGuitar, 75, 0),
            ("Low end", Instrument::Bass, 60, -100)
        ]
    );
    for track in 0..2 {
        assert_eq!(
            sounded(back, track),
            sounded(&score, track),
            "track {track}"
        );
    }
}

#[test]
fn a_score_is_written_as_it_plays() {
    let score = band();
    let written = export(&score);
    common::keep("band.mid", &written.bytes);
    // The bass is muted, and is left out as it is left out of what is heard.
    assert_eq!(written.skipped, ["tracks that are muted"]);
    let read = import(&written.bytes).unwrap();
    assert_eq!(read.skipped, ["pitch bends"]);
    let back = &read.score;
    // The repeat is taken with each of its endings, and the piece gone back over as far
    // as its end.
    let order = play_order(&score.master_bars, false);
    assert_eq!(order, [0, 1, 0, 2, 3, 0, 2, 3]);
    let times: Vec<TimeSignature> = back.master_bars.iter().map(|m| m.time_signature).collect();
    let played: Vec<TimeSignature> = (order.iter())
        .map(|bar| score.master_bars[*bar].time_signature)
        .collect();
    assert_eq!(times, played);
    let keys: Vec<i8> = back.master_bars.iter().map(|m| m.key_signature).collect();
    assert_eq!(keys, [1, 1, 1, -2, -2, 1, -2, -2]);
    // The tempo changes where the score changes it, and where a beat is held.
    assert_eq!(back.tempo, 120);
    assert_eq!(back.master_bars[3].tempo, Some(90));
    assert_eq!(back.master_bars[4].tempo, Some(140));
    assert_eq!(back.master_bars[5].tempo, Some(120));
    let held: Vec<(Rational, u32)> = (back.master_bars[1].tempo_changes.iter())
        .map(|change| (change.position, change.tempo))
        .collect();
    assert_eq!(held, [(Rational::new(3, 4), 60)]);

    let tracks: Vec<(&str, Instrument)> = (back.tracks.iter())
        .map(|t| (t.name.as_str(), t.instrument))
        .collect();
    assert_eq!(
        tracks,
        [
            ("Lead", Instrument::DistortionGuitar),
            ("Keys", Instrument::Piano),
            ("Drums", Instrument::Drums)
        ]
    );
    assert_eq!(
        (back.tracks[0].mix.volume, back.tracks[0].mix.pan),
        (60, -21)
    );
    // The piano's chords in the right hand, its bass notes in the left.
    let piano = &back.tracks[1];
    assert_eq!(sounded(back, 1)[0][0].2, [60, 64, 67]);
    assert_eq!(
        piano.staves[1].bars[0].voices[0].beats[0].notes[0].string,
        36
    );
    // The drums play their pieces, and in the bar that swings the eighths are uneven.
    let hits: Vec<u8> = (back.tracks[2].staves[0].bars[0].voices[0].beats[0]
        .notes
        .iter())
    .map(|n| n.fret)
    .collect();
    assert_eq!(hits, [36, 42]);
    let swung = &back.tracks[2].staves[0].bars[3].voices[0].beats;
    assert!(swung[0].duration.length() > swung[1].duration.length());
    // The guitar's bent note and its hammered one are both the guitar's.
    let lead = sounded(back, 0);
    assert_eq!(lead[0][0].2, [71]);
    assert_eq!(lead[0][1].2, [68]);
}

/// A number as MIDI writes times.
fn quantity(value: u32) -> Vec<u8> {
    match value {
        0..=0x7F => vec![value as u8],
        _ => vec![(value >> 7) as u8 | 0x80, (value & 0x7F) as u8],
    }
}

#[test]
fn a_file_put_together_by_hand_is_read() {
    // One track of everything, as the simplest files are: 96 ticks to the quarter, a bar
    // of three quarters at 150 to the minute, and a guitar on the first channel playing
    // C, then E and G together, written the short way where an event repeats the kind of
    // the last and a note ends by being struck with no force.
    let mut track: Vec<u8> = Vec::new();
    let mut at = |delta: u32, bytes: &[u8]| {
        track.extend(quantity(delta));
        track.extend(bytes);
    };
    at(0, &[0xFF, 0x03, 5, b'P', b'i', b'e', b'c', b'e']);
    at(0, &[0xFF, 0x58, 4, 3, 2, 24, 8]);
    at(0, &[0xFF, 0x51, 3, 0x06, 0x1A, 0x80]);
    at(0, &[0xC0, 24]);
    at(0, &[0xB0, 7, 127]);
    at(0, &[0x90, 60, 100]);
    at(96, &[60, 0]);
    at(0, &[64, 30]);
    at(0, &[67, 30]);
    at(190, &[64, 0]);
    at(2, &[67, 0]);
    at(0, &[0xFF, 0x2F, 0]);
    let mut file = b"MThd".to_vec();
    file.extend([0, 0, 0, 6, 0, 0, 0, 1, 0, 96]);
    file.extend(b"MTrk");
    file.extend((track.len() as u32).to_be_bytes());
    file.extend(track);

    let read = import(&file).unwrap();
    assert_eq!(read.skipped, [] as [&str; 0]);
    let score = read.score;
    assert_eq!(score.tempo, 150);
    assert_eq!(score.master_bars.len(), 1);
    assert_eq!(score.master_bars[0].time_signature.numerator, 3);
    let track = &score.tracks[0];
    assert_eq!(
        (track.name.as_str(), track.instrument, track.mix.volume),
        ("Piece", Instrument::NylonGuitar, 100)
    );
    assert_eq!(
        sounded(&score, 0),
        [vec![
            (Rational::new(0, 1), Rational::new(1, 4), vec![60]),
            (Rational::new(1, 4), Rational::new(1, 2), vec![64, 67]),
        ]]
    );
    // The quiet chord after the loud note is marked as quieter.
    let beats = &track.staves[0].bars[0].voices[0].beats;
    assert_eq!(beats[0].effects.dynamic, None);
    assert_eq!(beats[1].effects.dynamic, Some(Dynamic::Pp));
    // Middle C is on the second string's first fret. Of the chord the higher note is
    // placed first, the G on the first string, and the E beside it on the second.
    let frets: Vec<(u8, u8)> = beats[1].notes.iter().map(|n| (n.string, n.fret)).collect();
    assert_eq!((beats[0].notes[0].string, beats[0].notes[0].fret), (1, 1));
    assert_eq!(frets, [(0, 3), (1, 5)]);
}

#[test]
fn pieces_typeset_by_other_people_are_read() {
    // Five pieces for the guitar from the Mutopia Project, as LilyPond wrote them.
    let found = files(&corpus().join("mutopia"), &["mid"]);
    assert_eq!(found.len(), 5);
    for path in &found {
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        let read = import(&std::fs::read(path).unwrap()).unwrap_or_else(|e| panic!("{name}: {e}"));
        let score = &read.score;
        assert!(score.master_bars.len() > 30, "{name}");
        // Nearly every bar of each part that has notes is full: the notes fell on beats.
        for (index, track) in score.tracks.iter().enumerate() {
            let bars = &track.staves[0].bars;
            let full = (bars.iter().zip(&score.master_bars))
                .filter(|(bar, master)| {
                    bar.voices[0].length() == master.time_signature.bar_length()
                })
                .count();
            let sounding = bars
                .iter()
                .filter(|b| !b.voices[0].beats.is_empty())
                .count();
            assert!(
                sounding > 20 && full * 10 >= sounding * 9,
                "{name} track {index}"
            );
        }
        // And what was read is written and read again without a panic.
        import(&export(score).bytes).unwrap();
    }
    // The prelude in D minor: three in a bar, one flat, sixteenths over a bass on D.
    let path = corpus().join("mutopia/Bach_Prelude_BWV999.mid");
    let score = import(&std::fs::read(path).unwrap()).unwrap().score;
    assert_eq!((score.tempo, score.master_bars.len()), (80, 43));
    assert_eq!(score.master_bars[0].time_signature.numerator, 3);
    assert_eq!(score.master_bars[0].key_signature, -1);
    assert_eq!(score.tracks[0].instrument, Instrument::NylonGuitar);
    let first = &sounded(&score, 0)[0];
    assert_eq!(first.len(), 12);
    assert!(first.iter().all(|beat| beat.1 == Rational::new(1, 16)));
    assert_eq!(first[0].2, [50]);
}

#[test]
fn what_is_not_midi_is_refused() {
    assert!(matches!(
        import(b"RIFF...."),
        Err(ImportError::UnknownFormat)
    ));
    assert!(matches!(import(b""), Err(ImportError::UnknownFormat)));
    // Time counted in frames of film, not in beats.
    let mut frames = b"MThd".to_vec();
    frames.extend([0, 0, 0, 6, 0, 1, 0, 1, 0xE7, 0x28]);
    assert!(matches!(
        import(&frames),
        Err(ImportError::UnsupportedVersion(_))
    ));
    // A file with no notes in it.
    let mut silent = b"MThd".to_vec();
    silent.extend([0, 0, 0, 6, 0, 1, 0, 1, 1, 0xE0]);
    silent.extend(b"MTrk");
    silent.extend([0, 0, 0, 4, 0, 0xFF, 0x2F, 0]);
    assert!(matches!(import(&silent), Err(ImportError::Corrupt(_))));
    // A file cut short, or with a byte changed, is refused or read as far as it goes.
    let whole = export(&band()).bytes;
    for cut in 0..whole.len().min(400) {
        let _ = import(&whole[..cut]);
    }
    for at in (0..whole.len()).step_by(7) {
        let mut changed = whole.clone();
        changed[at] ^= 0xFF;
        let _ = import(&changed);
    }
}
