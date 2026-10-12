//! What the Guitar Pro files in `tests/corpus/alphatab` hold: repeats, alternate endings,
//! signs and jumps, and drum tracks, in both formats read.

use std::path::PathBuf;

use gtr_io::guitar_pro::{Imported, import};
use gtr_model::{Instrument, Jump, MasterBar, Score, Target, play_order};

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
    for name in ["repeat-close.gp5", "repeat-close.gp"] {
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
    for name in ["repeat-close-multi.gp5", "repeat-close-multi.gp"] {
        let score = score(name);
        assert_eq!(score.master_bars[1].repeat_count, 4, "{name}");
        assert_eq!(play_order(&score.master_bars, false).len(), 4 * 2 + 1);
    }
    // A repeat that nothing opens goes back to the start.
    let score = score("repeat-close-without-start-at-beginning.gp5");
    assert_eq!(repeats(&score), [(false, 0, 0), (false, 2, 0)]);
    assert_eq!(play_order(&score.master_bars, false), [0, 1, 0, 1]);

    let score = self::score("repeats.gp");
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
        "repeat-close-alternate-endings.gp5",
        "repeat-close-alternate-endings.gp",
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
    let score = score("alternate-endings.gp");
    let endings: Vec<u8> = repeats(&score).iter().map(|r| r.2).collect();
    assert_eq!(endings, [0, 0, 0, 0b0101, 0, 0b1010]);
}

#[test]
fn signs_and_jumps_are_read() {
    // Guitar Pro 5 lists a bar for each of its nineteen directions, and this file has one
    // on every bar: the signs, the jumps from the top, each jump from the segno beside the
    // same from the second segno, then the jumps to the codas. The second segno and coda
    // are read as the first.
    let score = score("directions.gp5");
    let targets: Vec<Option<Target>> = score.master_bars.iter().map(|m| m.target).collect();
    let jumps: Vec<Option<Jump>> = score.master_bars.iter().map(|m| m.jump).collect();
    use Jump::*;
    assert_eq!(
        targets[..5],
        [
            Target::Coda,
            Target::Coda,
            Target::Segno,
            Target::Segno,
            Target::Fine
        ]
        .map(Some)
    );
    assert!(targets[5..].iter().all(Option::is_none));
    assert!(jumps[..5].iter().all(Option::is_none));
    assert_eq!(
        jumps[5..],
        [
            DaCapo,
            DaCapoAlCoda,
            DaCapoAlCoda,
            DaCapoAlFine,
            DalSegno,
            DalSegno,
            DalSegnoAlCoda,
            DalSegnoAlCoda,
            DalSegnoAlCoda,
            DalSegnoAlCoda,
            DalSegnoAlFine,
            DalSegnoAlFine,
            ToCoda,
            ToCoda
        ]
        .map(Some)
    );

    // The newer format can put several on a bar, where a bar here holds one sign and one
    // jump. The first of each is kept, and the import says that the rest were not.
    let Imported { score, skipped } = load("directions-gp8.gp");
    assert_eq!(skipped, ["all but the first sign and jump on a bar"]);
    let read: Vec<_> = (score.master_bars.iter())
        .map(|m| (m.target, m.jump))
        .collect();
    assert_eq!(
        read,
        [
            (Some(Target::Coda), None),
            (None, Some(DaCapo)),
            (None, Some(DaCapoAlCoda)),
            (None, Some(DaCapoAlCoda)),
            (None, Some(DaCapoAlFine)),
            (None, None)
        ]
    );
}

#[test]
fn drum_tracks_are_read_from_both_formats() {
    // Every General MIDI percussion number in turn, four to a bar. The file does not flag
    // the track as percussion: it is on MIDI channel 10.
    let score = score("percussion-all.gp5");
    assert_eq!(score.tracks[0].instrument, Instrument::Drums);
    let played: Vec<u8> = frets(&score, 0).concat();
    assert_eq!(played, (27..=87).collect::<Vec<u8>>());

    // The newer format has a kit of its own for each track, and a note says which
    // articulation of it is played. What is read is the General MIDI number that sounds.
    let score = self::score("drum-custom-lines.gp");
    assert_eq!(score.tracks[0].instrument, Instrument::Drums);
    assert_eq!(frets(&score, 0), [vec![36; 4], vec![]]);

    let score = self::score("drum-tabs.gp");
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

    let score = self::score("percussion.gp");
    let names: Vec<&str> = score.tracks.iter().map(|t| t.name.as_str()).collect();
    assert_eq!(names, ["Single Line", "Two Lines", "Three Lines"]);
    for track in &score.tracks {
        assert_eq!(track.instrument, Instrument::Drums);
        assert_eq!(track.staves[0].tuning.strings.len(), 5);
    }
    assert_eq!(frets(&score, 2), [vec![56; 4], vec![56; 2]]);
}
