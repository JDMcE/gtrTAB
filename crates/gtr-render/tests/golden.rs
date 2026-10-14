//! Golden images: a score for each area of the notation, laid out and written as SVG,
//! compared with the picture of it kept in `tests/golden`.
//!
//! A difference fails the test and leaves the new picture in `target/golden` to be looked
//! at beside the old one. If the change is meant, run the tests again with `GTR_BLESS=1`
//! to keep the new pictures, and commit them. The pictures load the music font from
//! `app/ui/public/fonts`, so they open in a browser straight from the repository.

use std::path::PathBuf;

use gtr_layout::{Notation, View, layout};
use gtr_model::{
    Bar, Beat, Bend, BendKind, ChordDiagram, Duration, Dynamic, Grace, Hairpin, Harmonic,
    Instrument, Jump, MasterBar, Note, NoteValue, Score, Slide, Strum, Target, TempoChange,
    TimeSignature, Track, Tuplet, Vibrato, Voice, demo_score,
};
use gtr_render::{SvgOptions, svg};

const WIDTH: f64 = 1000.0;
use NoteValue::{Eighth, Half, Quarter, Sixteenth, Whole};

fn beat(value: NoteValue, notes: &[(u8, u8)]) -> Beat {
    let notes = notes.iter().map(|&(string, fret)| Note::new(string, fret));
    Beat::new(Duration::new(value), notes.collect())
}

/// A score of one guitar track with the bars given, each a single voice.
fn guitar(bars: Vec<Vec<Beat>>) -> Score {
    let mut score = demo_score();
    score.info.title = String::new();
    score.master_bars = vec![MasterBar::new(TimeSignature::COMMON); bars.len()];
    score.tracks[0].staves[0].bars = (bars.into_iter())
        .map(|beats| Bar {
            voices: vec![Voice { beats }],
        })
        .collect();
    score
}

fn check(name: &str, score: &Score, view: View) {
    score.validate().unwrap_or_else(|e| panic!("{name}: {e}"));
    let options = SvgOptions {
        music_font_url: Some("../../app/ui/public/fonts/Bravura.woff2"),
        ..SvgOptions::default()
    };
    let drawn = svg(&layout(score, &view, WIDTH), options);
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let golden = root.join("tests/golden").join(format!("{name}.svg"));
    if std::env::var_os("GTR_BLESS").is_some() {
        std::fs::write(&golden, &drawn).unwrap();
        return;
    }
    let kept = std::fs::read_to_string(&golden).unwrap_or_default();
    // Compare line by line, so that how a checkout ends its lines does not matter.
    if !kept.lines().eq(drawn.lines()) {
        let out = root.join("target/golden");
        std::fs::create_dir_all(&out).unwrap();
        let new = out.join(format!("{name}.svg"));
        // `target/golden` is as deep as `tests/golden`, so the font is found from there too.
        std::fs::write(&new, &drawn).unwrap();
        let changed = (kept.lines().zip(drawn.lines()))
            .filter(|(a, b)| a != b)
            .count()
            + kept.lines().count().abs_diff(drawn.lines().count());
        panic!(
            "{name} is not drawn as in {}: {changed} elements differ. The new picture is in \
             {}. If the change is meant, run again with GTR_BLESS=1 and commit the picture.",
            golden.display(),
            new.display()
        );
    }
}

fn view(notation: Notation) -> View {
    View {
        notation,
        ..View::default()
    }
}

/// Beams of every lean, flags, a chromatic line and chords that need their accidentals
/// chosen and stacked.
fn pitches() -> Score {
    let mut score = guitar(vec![
        vec![
            beat(Eighth, &[(0, 0)]),
            beat(Eighth, &[(0, 7)]),
            beat(Eighth, &[(0, 8)]),
            beat(Eighth, &[(0, 1)]),
            beat(Sixteenth, &[(4, 0)]),
            beat(Sixteenth, &[(4, 2)]),
            beat(Sixteenth, &[(3, 0)]),
            beat(Sixteenth, &[(3, 2)]),
            beat(Sixteenth, &[(2, 2)]),
            beat(Eighth, &[(4, 3)]),
            beat(Sixteenth, &[(5, 0)]),
        ],
        (1..=5)
            .chain((2..=4).rev())
            .map(|fret| beat(Eighth, &[(1, fret)]))
            .collect(),
        vec![
            beat(Quarter, &[(0, 0), (1, 0), (2, 1)]),
            beat(Quarter, &[(0, 4), (1, 1), (2, 8)]),
            beat(Quarter, &[(1, 1), (2, 6)]),
            beat(Quarter, &[(3, 4), (4, 4), (5, 2)]),
        ],
        vec![beat(
            Whole,
            &[(0, 3), (1, 3), (2, 0), (3, 0), (4, 2), (5, 3)],
        )],
        vec![
            beat(Half, &[(2, 2)]),
            beat(Eighth, &[(2, 4)]),
            Beat::rest(Duration::new(Eighth)),
            beat(Eighth, &[(5, 3)]),
            beat(Sixteenth, &[(5, 5)]),
            Beat::rest(Duration::new(Sixteenth)),
        ],
    ]);
    // The last two bars in a key of their own.
    score.master_bars[3].key_signature = -3;
    score.master_bars[4].key_signature = 4;
    let dotted = &mut score.tracks[0].staves[0].bars[4].voices[0].beats;
    dotted[0].duration = Duration {
        dots: 1,
        ..Duration::new(Quarter)
    };
    dotted.insert(1, beat(Eighth, &[(2, 2)]));
    dotted[1].notes[0].tied = true;
    score
}

/// Everything that is written around the notes of a lead part.
fn techniques() -> Score {
    let mut score = guitar(vec![
        vec![
            beat(Quarter, &[(1, 5)]),
            beat(Quarter, &[(1, 7)]),
            beat(Quarter, &[(2, 7)]),
            beat(Quarter, &[(2, 9)]),
        ],
        vec![
            beat(Eighth, &[(2, 5)]),
            beat(Eighth, &[(2, 7)]),
            beat(Eighth, &[(2, 5)]),
            beat(Eighth, &[(3, 7)]),
            beat(Quarter, &[(0, 12)]),
            beat(Quarter, &[(0, 0), (1, 0), (2, 1), (3, 2)]),
        ],
        vec![beat(Half, &[(1, 8)]), beat(Half, &[(1, 10)])],
    ]);
    let bars = &mut score.tracks[0].staves[0].bars;
    let first = &mut bars[0].voices[0].beats;
    first[0].notes[0].effects.slide = Some(Slide::Shift);
    first[0].effects.chord = Some("Am7".into());
    first[0].effects.diagram = Some(ChordDiagram {
        frets: vec![0, 1, 0, 2, 0, -1],
    });
    first[0].effects.text = Some("with feeling".into());
    first[0].effects.dynamic = Some(Dynamic::Mf);
    first[0].effects.lyrics = vec!["Oh".into()];
    first[1].notes[0].effects.bend = Some(Bend::preset(BendKind::BendRelease, 4));
    first[1].effects.hairpin = Some(Hairpin::Crescendo);
    first[2].notes[0].effects.palm_mute = true;
    first[2].notes[0].effects.slide = Some(Slide::InFromBelow);
    first[2].effects.hairpin = Some(Hairpin::Crescendo);
    first[3].notes[0].effects.palm_mute = true;
    first[3].notes[0].effects.vibrato = Some(Vibrato::Wide);
    first[3].notes[0].effects.slide = Some(Slide::OutDown);
    first[3].effects.dynamic = Some(Dynamic::Ff);
    let second = &mut bars[1].voices[0].beats;
    second[0].notes[0].effects.hammer = true;
    second[1].notes[0].effects.hammer = true;
    second[2].notes[0].effects.let_ring = true;
    second[3].notes[0].effects.let_ring = true;
    second[3].notes[0].effects.grace = Some(Grace::single(3, 5, false, true));
    second[4].notes[0].effects.harmonic = Some(Harmonic::Natural);
    second[4].effects.fermata = true;
    second[5].effects.strum = Some(Strum::ArpeggioDown);
    second[5].notes[1].effects.ghost = true;
    second[5].notes[3].effects.dead = true;
    let third = &mut bars[2].voices[0].beats;
    third[0].notes[0].effects.bend = Some(Bend::new([
        (0, 0),
        (10, 2),
        (20, 2),
        (30, 6),
        (40, 4),
        (50, 6),
        (60, 6),
    ]));
    third[1].notes[0].effects.bend = Some(Bend::new([(0, 2), (20, 2), (40, 4), (50, 1), (60, 1)]));
    score
}

/// Two voices sharing a staff, tuplets and ties.
fn voices() -> Score {
    let triplet = |notes: &[(u8, u8)]| {
        let mut beat = beat(Eighth, notes);
        beat.duration.tuplet = Some(Tuplet::TRIPLET);
        beat
    };
    let mut score = guitar(vec![
        vec![
            triplet(&[(0, 5)]),
            triplet(&[(0, 7)]),
            triplet(&[(0, 8)]),
            beat(Quarter, &[(0, 8)]),
            triplet(&[(1, 5)]),
            triplet(&[(1, 8)]),
            triplet(&[(0, 5)]),
            beat(Quarter, &[(0, 7)]),
        ],
        vec![beat(Half, &[(0, 7)]), Beat::rest(Duration::new(Half))],
    ]);
    let bars = &mut score.tracks[0].staves[0].bars;
    bars[0].voices[0].beats[3].notes[0].tied = true;
    bars[1].voices[0].beats[0].notes[0].tied = true;
    bars[0].voices.push(Voice {
        beats: vec![beat(Half, &[(4, 0)]), beat(Half, &[(5, 3)])],
    });
    bars[1].voices.push(Voice {
        beats: vec![beat(Whole, &[(5, 0)])],
    });
    score
}

/// What is written over whole bars: repeats, endings, sections, signs, jumps and tempo.
fn bar_marks() -> Score {
    let riff = || vec![beat(Half, &[(3, 2)]), beat(Half, &[(3, 4)])];
    let mut score = guitar((0..6).map(|_| riff()).collect());
    let bars = &mut score.master_bars;
    bars[0].section = Some("Verse".into());
    bars[0].repeat_start = true;
    bars[1].alternate_endings = 0b01;
    bars[1].repeat_count = 2;
    bars[2].alternate_endings = 0b10;
    bars[2].targets = vec![Target::Segno];
    bars[3].tempo = Some(96);
    bars[3].key_signature = 2;
    bars[3].time_signature = TimeSignature {
        numerator: 6,
        denominator: 8,
    };
    bars[3].tempo_changes = vec![TempoChange {
        position: gtr_model::Rational::new(3, 8),
        tempo: 132,
        gradual: false,
    }];
    bars[4].repeat_start = true;
    bars[4].repeat_count = 3;
    bars[4].targets = vec![Target::Fine];
    bars[5].jumps = vec![Jump::DalSegnoAlFine];
    bars[5].timer = true;
    for bar in &mut bars[4..6] {
        bar.key_signature = 2;
    }
    let short = &mut score.tracks[0].staves[0].bars[3].voices[0].beats;
    *short = vec![
        beat(Eighth, &[(3, 2)]),
        beat(Eighth, &[(3, 4)]),
        beat(Eighth, &[(2, 2)]),
        beat(Eighth, &[(3, 4)]),
        beat(Eighth, &[(3, 2)]),
        beat(Eighth, &[(4, 4)]),
    ];
    score
}

/// A band: guitar, bass, piano and drums, each with the staves it has.
fn band() -> Score {
    let mut score = guitar(vec![
        vec![
            beat(Eighth, &[(3, 2), (4, 2), (5, 0)]),
            beat(Eighth, &[(3, 2), (4, 2), (5, 0)]),
            beat(Quarter, &[(3, 4), (4, 4), (5, 2)]),
            beat(Half, &[(3, 5), (4, 5), (5, 3)]),
        ],
        vec![beat(Whole, &[(3, 2), (4, 2), (5, 0)])],
    ]);
    score.info.title = "Band".into();
    let mut bass = Track::empty("Bass", Instrument::Bass, 2);
    bass.staves[0].bars[0].voices[0].beats = vec![
        beat(Quarter, &[(3, 0)]),
        beat(Quarter, &[(3, 2)]),
        beat(Half, &[(3, 3)]),
    ];
    bass.staves[0].bars[1].voices[0].beats = vec![beat(Whole, &[(3, 0)])];
    let mut piano = Track::empty("Piano", Instrument::Piano, 2);
    piano.staves[0].bars[0].voices[0].beats = vec![
        beat(Half, &[(64, 0), (67, 0), (71, 0)]),
        beat(Half, &[(67, 0), (70, 0), (74, 0)]),
    ];
    piano.staves[1].bars[0].voices[0].beats = vec![beat(Half, &[(40, 0)]), beat(Half, &[(43, 0)])];
    let mut drums = Track::empty("Drums", Instrument::Drums, 2);
    drums.staves[0].bars[0].voices[0].beats = [
        &[(0, 42), (4, 36)][..],
        &[(0, 42)],
        &[(0, 42), (2, 38)],
        &[(0, 42), (4, 36)],
        &[(0, 42), (4, 36)],
        &[(0, 42)],
        &[(0, 46), (2, 38)],
        &[(0, 42)],
    ]
    .map(|notes| beat(Eighth, notes))
    .to_vec();
    drums.staves[0].bars[1].voices[0].beats = vec![beat(Whole, &[(0, 49), (4, 36)])];
    score.tracks.extend([bass, piano, drums]);
    score
}

/// What other formats have that came late to the model: second segnos and codas, several
/// jumps on a bar, the dotted and Scottish feels, a tempo that changes by degrees, fades,
/// notes down to a 256th and a third dot.
fn late_arrivals() -> Score {
    use NoteValue::{HundredTwentyEighth, SixtyFourth, ThirtySecond, TwoHundredFiftySixth};
    let run = |value: NoteValue, count: u8| (0..count).map(move |i| beat(value, &[(1, 5 + i % 4)]));
    let mut score = guitar(vec![
        vec![
            beat(Quarter, &[(2, 7)]),
            beat(Quarter, &[(2, 9)]),
            beat(Half, &[(1, 8)]),
        ],
        vec![
            beat(Half, &[(1, 5)]),
            beat(Quarter, &[(1, 7)]),
            beat(Quarter, &[(1, 8)]),
        ],
        run(ThirtySecond, 8)
            .chain(run(SixtyFourth, 16))
            .chain(run(HundredTwentyEighth, 16))
            .chain(run(TwoHundredFiftySixth, 32))
            .chain([beat(Quarter, &[(1, 5)])])
            .collect(),
        vec![
            beat(Half, &[(3, 2)]),
            Beat::rest(Duration::new(HundredTwentyEighth)),
        ],
    ]);
    let bars = &mut score.master_bars;
    bars[0].targets = vec![Target::Segno, Target::SegnoSegno];
    bars[0].tempo_gradual = true;
    bars[0].feel = Some(gtr_model::Feel::DottedEighths);
    bars[1].targets = vec![Target::DoubleCoda];
    bars[1].tempo = Some(160);
    bars[1].feel = Some(gtr_model::Feel::ScottishSixteenths);
    bars[1].tempo_changes = vec![TempoChange {
        position: gtr_model::Rational::new(1, 2),
        tempo: 140,
        gradual: true,
    }];
    bars[2].tempo = Some(80);
    bars[3].jumps = vec![Jump::DalSegnoSegno, Jump::DalSegnoAlDoubleCoda];
    let first = &mut score.tracks[0].staves[0].bars[0].voices[0].beats;
    first[0].effects.fade = Some(gtr_model::Fade::In);
    first[1].effects.fade = Some(gtr_model::Fade::Out);
    first[2].effects.fade = Some(gtr_model::Fade::Swell);
    // A half note with three dots and a 128th rest fill the last bar but for a sliver.
    score.tracks[0].staves[0].bars[3].voices[0].beats[0]
        .duration
        .dots = 3;
    score
}

#[test]
fn what_came_late_from_guitar_pro() {
    check("late-arrivals-both", &late_arrivals(), view(Notation::Both));
    check("late-arrivals-tab", &late_arrivals(), view(Notation::Tab));
}

#[test]
fn beams_stems_and_accidentals() {
    check("pitches-notation", &pitches(), view(Notation::Standard));
    check("pitches-slashes", &pitches(), view(Notation::Slash));
}

#[test]
fn techniques_in_each_view() {
    check("techniques-both", &techniques(), view(Notation::Both));
    check(
        "techniques-notation",
        &techniques(),
        view(Notation::Standard),
    );
    check("techniques-tab", &techniques(), view(Notation::Tab));
}

#[test]
fn voices_tuplets_and_ties() {
    check("voices-both", &voices(), view(Notation::Both));
    let second = View {
        voice: 1,
        ..view(Notation::Tab)
    };
    check("voices-tab-second-voice", &voices(), second);
}

#[test]
fn repeats_signs_and_changes_over_the_bars() {
    check("bar-marks-both", &bar_marks(), view(Notation::Both));
}

#[test]
fn a_band_on_one_page() {
    let all = View {
        multitrack: true,
        ..view(Notation::Both)
    };
    check("band-both", &band(), all);
    let drums = View {
        track: 3,
        ..view(Notation::Both)
    };
    check("band-drums", &band(), drums);
}
