//! What particular Guitar Pro files in `tests/corpus/alphatab` hold: repeats, alternate
//! endings, signs and jumps, drum tracks, chords, lyrics, text and changes of tempo, from
//! each version of the format that has them.

use std::path::PathBuf;

use gtr_io::guitar_pro::{Imported, import};
use gtr_model::{
    Beat, Fade, Harmonic, Instrument, Jump, MasterBar, Rational, Score, Target, play_order,
};

fn load(name: &str) -> Imported {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/corpus/alphatab")
        .join(name);
    let bytes = std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    import(&bytes).unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn score(name: &str) -> Score {
    let imported = load(name);
    assert_eq!(imported.skipped, [] as [&str; 0], "{name}");
    imported.score
}

/// Where each bar's repeat starts, how often it is played and which endings it belongs to.
fn repeats(score: &Score) -> Vec<(bool, u8, u8)> {
    let of = |m: &MasterBar| (m.repeat_start, m.repeat_count, m.alternate_endings);
    score.master_bars.iter().map(of).collect()
}

/// The frets of every note of a track's first staff, bar by bar.
fn frets(score: &Score, track: usize) -> Vec<Vec<u8>> {
    (score.tracks[track].staves[0].bars.iter())
        .map(|bar| {
            (bar.voices[0].beats.iter())
                .flat_map(|beat| beat.notes.iter().map(|n| n.fret))
                .collect()
        })
        .collect()
}

#[test]
fn repeats_are_read_alike_from_both_formats_and_followed() {
    for name in ["guitarpro5/repeat-close.gp5", "guitarpro7/repeat-close.gp"] {
        let score = score(name);
        let plain = (false, 0, 0);
        assert_eq!(
            repeats(&score),
            [(true, 0, 0), (false, 2, 0), plain],
            "{name}"
        );
        assert_eq!(play_order(&score.master_bars, false), [0, 1, 0, 1, 2]);
        assert_eq!(frets(&score, 0), vec![vec![1, 2, 3, 4]; 3]);
    }
    for name in [
        "guitarpro5/repeat-close-multi.gp5",
        "guitarpro7/repeat-close-multi.gp",
    ] {
        let score = score(name);
        assert_eq!(score.master_bars[1].repeat_count, 4, "{name}");
        assert_eq!(play_order(&score.master_bars, false).len(), 4 * 2 + 1);
    }
    // A repeat that nothing opens goes back to the start.
    let score = score("guitarpro5/repeat-close-without-start-at-beginning.gp5");
    assert_eq!(repeats(&score), [(false, 0, 0), (false, 2, 0)]);
    assert_eq!(play_order(&score.master_bars, false), [0, 1, 0, 1]);

    let score = self::score("visual-tests/repeats.gp");
    let found = repeats(&score);
    let starts: Vec<usize> = (0..found.len()).filter(|&i| found[i].0).collect();
    let ends: Vec<(usize, u8)> = (0..found.len())
        .filter(|&i| found[i].1 > 0)
        .map(|i| (i, found[i].1))
        .collect();
    assert_eq!((starts, ends), (vec![0, 4], vec![(2, 2), (7, 5)]));
}

#[test]
fn alternate_endings_are_read_alike_from_both_formats_and_followed() {
    for name in [
        "guitarpro5/repeat-close-alternate-endings.gp5",
        "guitarpro7/repeat-close-alternate-endings.gp",
    ] {
        let score = score(name);
        // The first and third times through take one ending, the second time the other.
        assert_eq!(
            repeats(&score),
            [
                (true, 0, 0),
                (false, 0, 0b101),
                (false, 0, 0b010),
                (false, 4, 0),
                (false, 0, 0)
            ],
            "{name}"
        );
        assert_eq!(
            play_order(&score.master_bars, false),
            [0, 1, 3, 0, 2, 3, 0, 1, 3, 0, 3, 4],
            "{name}"
        );
    }
    let score = score("visual-tests/alternate-endings.gp");
    let endings: Vec<u8> = repeats(&score).iter().map(|r| r.2).collect();
    assert_eq!(endings, [0, 0, 0, 0b0101, 0, 0b1010]);
}

#[test]
fn signs_and_jumps_are_read() {
    // A `.gp5` file lists a bar for each of its nineteen directions, and this file has one
    // on every bar: the five signs, the jumps from the top, the jumps from the segno and
    // from the second segno, and the jumps on to the two codas.
    let score = score("guitarpro5/directions.gp5");
    let signs: Vec<&[Target]> = (score.master_bars.iter())
        .map(|m| m.targets.as_slice())
        .collect();
    use Target::*;
    for (bar, sign) in [Coda, DoubleCoda, Segno, SegnoSegno, Fine]
        .iter()
        .enumerate()
    {
        assert_eq!(signs[bar], [*sign]);
    }
    assert!(signs[5..].iter().all(|signs| signs.is_empty()));
    let jumps: Vec<Jump> = (score.master_bars.iter())
        .flat_map(|m| m.jumps.clone())
        .collect();
    use Jump::*;
    assert_eq!(
        jumps,
        [
            DaCapo,
            DaCapoAlCoda,
            DaCapoAlDoubleCoda,
            DaCapoAlFine,
            DalSegno,
            DalSegnoSegno,
            DalSegnoAlCoda,
            DalSegnoAlDoubleCoda,
            DalSegnoSegnoAlCoda,
            DalSegnoSegnoAlDoubleCoda,
            DalSegnoAlFine,
            DalSegnoSegnoAlFine,
            ToCoda,
            ToDoubleCoda
        ]
    );
    assert!(score.master_bars[5..].iter().all(|m| m.jumps.len() == 1));

    // The newer format puts several on a bar: all five signs on the first bar of this
    // file, and then every jump there is, a few to a bar.
    let score = self::score("guitarpro8/directions.gp");
    let bars = &score.master_bars;
    assert_eq!(bars[0].targets, [Coda, DoubleCoda, Segno, SegnoSegno, Fine]);
    assert_eq!(
        bars[1].jumps,
        [DaCapo, DalSegno, DalSegnoSegno, ToCoda, ToDoubleCoda]
    );
    assert_eq!(
        bars[2].jumps,
        [DaCapoAlCoda, DalSegnoAlCoda, DalSegnoSegnoAlCoda]
    );
    assert_eq!(
        bars[3].jumps,
        [
            DaCapoAlDoubleCoda,
            DalSegnoAlDoubleCoda,
            DalSegnoSegnoAlDoubleCoda
        ]
    );
    assert_eq!(
        bars[4].jumps,
        [DaCapoAlFine, DalSegnoAlFine, DalSegnoSegnoAlFine]
    );
    assert!(bars[5].jumps.is_empty() && bars[5].targets.is_empty());
}

#[test]
fn drum_tracks_are_read_from_both_formats() {
    // Every General MIDI percussion number in turn, four to a bar. The file does not flag
    // the track as percussion: it is on MIDI channel 10.
    let score = score("guitarpro5/percussion-all.gp5");
    assert_eq!(score.tracks[0].instrument, Instrument::Drums);
    let played: Vec<u8> = frets(&score, 0).concat();
    assert_eq!(played, (27..=87).collect::<Vec<u8>>());

    // The newer format has a kit of its own for each track, and a note says which
    // articulation of it is played. What is read is the General MIDI number that sounds.
    let score = self::score("guitarpro7/drum-custom-lines.gp");
    assert_eq!(score.tracks[0].instrument, Instrument::Drums);
    assert_eq!(frets(&score, 0), [vec![36; 4], vec![]]);

    let score = self::score("guitarpro7/drum-tabs.gp");
    let played = frets(&score, 0);
    assert_eq!(played.len(), 4);
    assert!(played.iter().all(|bar| bar.len() >= 15));
    assert!(
        played
            .concat()
            .iter()
            .all(|piece| (27..=87).contains(piece))
    );
    // A hi-hat, a bass drum and a snare are among them, by their usual numbers.
    for piece in [42, 36, 38] {
        assert!(played.concat().contains(&piece), "no {piece}");
    }

    let score = self::score("visual-tests/percussion.gp");
    let names: Vec<&str> = score.tracks.iter().map(|t| t.name.as_str()).collect();
    assert_eq!(names, ["Single Line", "Two Lines", "Three Lines"]);
    for track in &score.tracks {
        assert_eq!(track.instrument, Instrument::Drums);
        assert_eq!(track.staves[0].tuning.strings.len(), 5);
    }
    assert_eq!(frets(&score, 2), [vec![56; 4], vec![56; 2]]);
}

/// The beats of a track's first staff and voice, bar by bar.
fn beats(score: &Score, track: usize) -> Vec<&[Beat]> {
    (score.tracks[track].staves[0].bars.iter())
        .map(|bar| bar.voices[0].beats.as_slice())
        .collect()
}

/// Every beat of a track's first staff and voice.
fn all_beats(score: &Score, track: usize) -> Vec<&Beat> {
    beats(score, track).into_iter().flatten().collect()
}

#[test]
fn every_version_is_read() {
    // The same four notes in versions 3, 4, 5, 6 and 7: what `gp_samples.rs` checks
    // of every file that the versions share, said out loud for one.
    for name in [
        "guitarpro3/dead.gp3",
        "guitarpro4/dead.gp4",
        "guitarpro5/dead.gp5",
        "guitarpro6/dead.gpx",
        "guitarpro7/dead.gp",
    ] {
        let score = score(name);
        let first = beats(&score, 0)[0];
        assert_eq!(first.len(), 4, "{name}");
        let notes: Vec<(u8, bool)> = (first.iter())
            .map(|beat| (beat.notes[0].string, beat.notes[0].effects.dead))
            .collect();
        assert_eq!(
            notes,
            [(5, true), (4, true), (3, true), (2, true)],
            "{name}"
        );
    }
    // Before version 5 a harmonic's kind is a number, and an artificial one's says how
    // far up the string it is touched; version 3 marks them on the beat.
    let score = self::score("guitarpro4/harmonics.gp4");
    let kinds: Vec<Option<Harmonic>> = (beats(&score, 0)[0].iter())
        .map(|beat| beat.notes[0].effects.harmonic)
        .collect();
    use Harmonic::*;
    assert_eq!(kinds, [Natural, Artificial, Tapped, Semi, Pinch].map(Some));
    let score = self::score("guitarpro3/harmonics.gp3");
    let kinds: Vec<Option<Harmonic>> = (beats(&score, 0)[0].iter())
        .map(|beat| beat.notes[0].effects.harmonic)
        .collect();
    assert_eq!(kinds[..2], [Natural, Artificial].map(Some));
}

#[test]
fn chords_come_with_their_names_and_fingerings() {
    // The binary format keeps a chord on its beat; the XML ones keep a list for the
    // track, which the beats point into. Both give a C as it is played by the nut.
    for name in ["guitarpro5/chords.gp5", "guitarpro6/chords.gpx"] {
        let score = score(name);
        let chords: Vec<(&str, Vec<i8>)> = (beats(&score, 0)[0].iter())
            .map(|beat| {
                let diagram = beat.effects.diagram.as_ref().expect("a fingering");
                (
                    beat.effects.chord.as_deref().unwrap(),
                    diagram.frets.clone(),
                )
            })
            .collect();
        assert_eq!(chords[0], ("C", vec![0, 1, 0, 2, 3, -1]), "{name}");
        assert_eq!(chords[1], ("Cm", vec![-1, -1, 0, 1, 3, -1]), "{name}");
        // The same chord as a barre at the third fret: the frets are the frets played.
        assert_eq!(chords[2], ("C", vec![3, 5, 5, 5, 3, -1]), "{name}");
    }
    // A beat can point at a chord its track does not have, and then has none.
    let score = score("guitarpro7/chord-no-diagram.gp");
    assert!(
        all_beats(&score, 0)
            .iter()
            .all(|beat| beat.effects.chord.is_none())
    );
}

#[test]
fn lyrics_and_text_are_put_on_their_beats() {
    let score = score("guitarpro7/beat-lyrics.gp");
    let sung: Vec<&str> = (all_beats(&score, 0).into_iter())
        .filter_map(|beat| beat.effects.lyrics.first().map(String::as_str))
        .collect();
    assert_eq!(sung, ["This", "is", "a", "test file", "for", "lyrics"]);

    for name in [
        "guitarpro3/effects.gp3",
        "guitarpro4/effects.gp4",
        "guitarpro5/effects.gp5",
        "guitarpro6/effects.gpx",
        "guitarpro7/effects.gp",
    ] {
        let Imported { score, skipped } = load(name);
        let written: Vec<&str> = (all_beats(&score, 0).into_iter())
            .filter_map(|beat| beat.effects.text.as_deref())
            .collect();
        assert_eq!(written, ["Text"], "{name}");
        assert_eq!(skipped, [] as [&str; 0], "{name}");
        // The notes that fade in do.
        let fading = (all_beats(&score, 0).iter())
            .filter(|beat| beat.effects.fade == Some(Fade::In))
            .count();
        assert!(fading >= 1, "{name}");
    }
}

#[test]
fn the_tempo_changes_where_the_file_says() {
    // A new tempo at the start of a bar is the bar's own; one inside it stands at its
    // place in the bar, in whole notes from the start.
    let score = score("guitarpro8/beat-tempo-change.gp");
    assert_eq!(score.tempo, 120);
    let bars = &score.master_bars;
    let inside = |bar: usize| -> Vec<(Rational, u32)> {
        (bars[bar].tempo_changes.iter())
            .map(|change| (change.position, change.tempo))
            .collect()
    };
    assert_eq!(
        (bars[0].tempo, inside(0)),
        (None, vec![(Rational::new(1, 2), 60)])
    );
    assert_eq!(bars[1].tempo, Some(100));
    assert_eq!(
        (bars[2].tempo, inside(2)),
        (Some(121), vec![(Rational::new(3, 8), 120)])
    );
    assert!(score.validate().is_ok());

    // The binary format has them in its mixing-desk changes, on a beat of some track.
    for name in ["guitarpro4/effects.gp4", "guitarpro5/effects.gp5"] {
        let score = self::score(name);
        let changed: Vec<(usize, u32)> = (score.master_bars.iter().enumerate())
            .filter_map(|(bar, master)| Some((bar, master.tempo?)))
            .collect();
        assert_eq!(changed, [(21, 120)], "{name}");
    }
}

#[test]
fn fermatas_timers_and_the_mixing_desk_are_read() {
    // Fermatas are kept with the bar, at a moment in it, and land on the beats there.
    // The file ends on a grace note that leads into nothing, which is said and left out.
    let read = load("guitarpro7/fermata.gp");
    assert_eq!(read.skipped, ["grace notes with no note after them"]);
    let score = read.score;
    let held: Vec<Vec<bool>> = (beats(&score, 0).iter())
        .map(|bar| bar.iter().map(|beat| beat.effects.fermata).collect())
        .collect();
    assert!(
        held[0].iter().filter(|held| **held).count() >= 3,
        "{held:?}"
    );
    assert!(held[0][0]);

    let score = self::score("guitarpro8/timer.gp");
    assert!(score.master_bars.iter().any(|bar| bar.timer));

    // Seven tracks from full volume down to almost none, and five from left to right.
    let score = self::score("guitarpro7/track-volume.gp");
    let volumes: Vec<u8> = score.tracks.iter().map(|t| t.mix.volume).collect();
    assert!(volumes.windows(2).all(|w| w[0] > w[1]), "{volumes:?}");
    assert_eq!((volumes[0], volumes[volumes.len() - 1]), (94, 1));
    let score = self::score("guitarpro7/track-balance.gp");
    let pans: Vec<i8> = score.tracks.iter().map(|t| t.mix.pan).collect();
    assert_eq!(pans, [-100, -50, 0, 50, 100]);
    assert!(score.tracks.iter().all(|t| t.mix.volume == 75));
}
