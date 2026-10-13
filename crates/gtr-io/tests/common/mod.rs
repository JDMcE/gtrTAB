//! What the tests of the formats share: a score with something of everything the model
//! holds, the files of the corpus, and a way to say where two scores differ.
#![allow(dead_code)]

use std::num::NonZeroU8;
use std::path::{Path, PathBuf};

use gtr_model::{
    Accent, Bar, Beat, Bend, BendKind, ChordDiagram, Duration, Dynamic, Fade, Feel, Finger, Grace,
    Hairpin, Harmonic, Instrument, Jump, MasterBar, Note, NoteValue, PickStroke, Rational, Score,
    ScoreInfo, Slap, Slide, Staff, Strum, Target, TempoChange, TimeSignature, Track, Trill, Tuning,
    Tuplet, Vibrato, Voice,
};
use serde_json::Value;

/// Keeps a file a test has written in the directory named by `GTR_KEEP`, if that is set,
/// so that it can be opened in the program whose format it is.
pub fn keep(name: &str, bytes: &[u8]) {
    if let Some(directory) = std::env::var_os("GTR_KEEP") {
        std::fs::write(Path::new(&directory).join(name), bytes).unwrap();
    }
}

pub fn corpus() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/corpus")
}

/// Every file under `directory` with one of `extensions`, in order.
pub fn files(directory: &Path, extensions: &[&str]) -> Vec<PathBuf> {
    fn walk(directory: &Path, extensions: &[&str], found: &mut Vec<PathBuf>) {
        for entry in std::fs::read_dir(directory).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                walk(&path, extensions, found);
            } else if (path.extension().and_then(|e| e.to_str()))
                .is_some_and(|e| extensions.contains(&e))
            {
                found.push(path);
            }
        }
    }
    let mut found = Vec::new();
    walk(directory, extensions, &mut found);
    found.sort();
    found
}

/// Where two scores first differ, as a path into them, or `None` if they are the same.
pub fn difference(a: &Score, b: &Score) -> Option<String> {
    fn walk(a: &Value, b: &Value, path: &str) -> Option<String> {
        match (a, b) {
            (Value::Object(x), Value::Object(y)) => {
                let keys: std::collections::BTreeSet<&String> = x.keys().chain(y.keys()).collect();
                keys.into_iter().find_map(|key| {
                    let null = Value::Null;
                    walk(
                        x.get(key).unwrap_or(&null),
                        y.get(key).unwrap_or(&null),
                        &format!("{path}/{key}"),
                    )
                })
            }
            (Value::Array(x), Value::Array(y)) if x.len() == y.len() => (x.iter().zip(y))
                .enumerate()
                .find_map(|(index, (x, y))| walk(x, y, &format!("{path}/{index}"))),
            _ if a == b => None,
            _ => {
                let short = |v: &Value| v.to_string().chars().take(200).collect::<String>();
                Some(format!("{path}: {} became {}", short(a), short(b)))
            }
        }
    }
    walk(
        &serde_json::to_value(a).unwrap(),
        &serde_json::to_value(b).unwrap(),
        "",
    )
}

pub fn value(value: NoteValue) -> Duration {
    Duration::new(value)
}

pub fn beat(length: NoteValue, notes: &[(u8, u8)]) -> Beat {
    let notes = notes.iter().map(|&(string, fret)| Note::new(string, fret));
    Beat::new(Duration::new(length), notes.collect())
}

fn bar(beats: Vec<Beat>) -> Bar {
    Bar {
        voices: vec![Voice { beats }],
    }
}

/// Four bars for a guitar, a bass, a piano and drums, with something of everything a
/// score can hold: a repeat with two endings, a bar in three, changes of tempo and feel,
/// signs and a jump, and on the guitar every technique.
pub fn band() -> Score {
    use NoteValue::{Eighth, Half, Quarter, Sixteenth, Whole};
    let dotted = |length| Duration {
        dots: 1,
        ..Duration::new(length)
    };
    let tuplet = |length, actual, normal| Duration {
        tuplet: Some(Tuplet {
            actual: NonZeroU8::new(actual).unwrap(),
            normal: NonZeroU8::new(normal).unwrap(),
        }),
        ..Duration::new(length)
    };
    let three = TimeSignature {
        numerator: 3,
        denominator: 4,
    };

    let mut masters = vec![MasterBar::new(TimeSignature::COMMON); 4];
    masters[0].section = Some("Intro".into());
    masters[0].repeat_start = true;
    masters[0].key_signature = 1;
    masters[1].key_signature = 1;
    masters[1].alternate_endings = 0b01;
    masters[1].repeat_count = 2;
    masters[2].time_signature = three;
    masters[2].key_signature = -2;
    masters[2].alternate_endings = 0b10;
    masters[2].tempo = Some(90);
    masters[2].tempo_changes = vec![TempoChange {
        position: Rational::new(1, 4),
        tempo: 100,
        gradual: true,
    }];
    masters[2].feel = Some(Feel::SwingEighths);
    masters[2].targets = vec![Target::Coda];
    masters[3].key_signature = -2;
    masters[3].feel = Some(Feel::Straight);
    masters[3].tempo = Some(140);
    masters[3].targets = vec![Target::Fine];
    masters[3].jumps = vec![Jump::DaCapoAlFine];
    masters[3].timer = true;

    // The guitar.
    let mut first = vec![
        beat(Quarter, &[(0, 5)]),
        beat(Quarter, &[(1, 7)]),
        beat(Quarter, &[(1, 8)]),
        beat(Eighth, &[(1, 10)]),
        beat(Eighth, &[(2, 9)]),
    ];
    first[0].notes[0].effects.bend = Some(Bend::preset(BendKind::Bend, 4));
    first[0].notes[0].effects.vibrato = Some(Vibrato::Slight);
    first[0].effects.dynamic = Some(Dynamic::Mf);
    first[0].effects.chord = Some("Am7".into());
    first[0].effects.diagram = Some(ChordDiagram {
        frets: vec![0, 1, 0, 2, 0, -1],
    });
    first[0].effects.text = Some("let it sing".into());
    first[0].effects.lyrics = vec!["Oh".into(), "la".into()];
    first[0].effects.stroke = Some(PickStroke::Down);
    first[1].notes[0].effects.hammer = true;
    first[1].effects.lyrics = vec!["my".into()];
    first[2].notes[0].effects.slide = Some(Slide::Shift);
    first[2].notes[0].effects.accent = Some(Accent::Heavy);
    first[3].notes[0].effects.palm_mute = true;
    first[3].notes[0].effects.staccato = true;
    first[4].notes[0].effects.let_ring = true;
    first[4].notes[0].effects.ghost = true;
    first[4].notes[0].effects.grace = Some(Grace::single(2, 7, false, true));

    let mut second = vec![
        beat(Half, &[(0, 0), (1, 1), (2, 0)]),
        beat(Quarter, &[(3, 2)]),
        Beat::rest(value(Quarter)),
    ];
    second[0].effects.strum = Some(Strum::BrushDown);
    second[0].effects.dynamic = Some(Dynamic::Ff);
    second[0].effects.chord = Some("C".into());
    second[1].notes[0].effects.harmonic = Some(Harmonic::Artificial);
    second[1].notes[0].effects.harmonic_touch = Some(7);
    second[1].notes[0].effects.trill = Some(Trill {
        fret: 4,
        speed: NoteValue::ThirtySecond,
    });
    let mut under = beat(Whole, &[(5, 0)]);
    under.notes[0].effects.dead = true;

    let mut third = vec![
        Beat::new(dotted(Quarter), vec![Note::new(0, 12)]),
        beat(Eighth, &[(0, 12)]),
        beat(Quarter, &[(4, 3)]),
    ];
    third[0].notes[0].effects.harmonic = Some(Harmonic::Natural);
    third[0].effects.whammy = Some(Bend::dip(4));
    third[0].effects.hairpin = Some(Hairpin::Crescendo);
    third[1].notes[0].tied = true;
    third[1].effects.volume = Some(50);
    third[1].effects.pan = Some(30);
    third[1].effects.fade = Some(Fade::Swell);
    third[2].notes[0].effects.tapped = true;
    third[2].notes[0].effects.left_finger = Some(Finger::Index);
    third[2].notes[0].effects.right_finger = Some(Finger::Middle);
    third[2].effects.tremolo = Some(Sixteenth);
    third[2].effects.slap = Some(Slap::Pop);

    let mut fourth: Vec<Beat> = [5, 6, 7]
        .into_iter()
        .map(|fret| Beat::new(tuplet(Eighth, 3, 2), vec![Note::new(1, fret)]))
        .collect();
    fourth.extend((0..5).map(|fret| Beat::new(tuplet(Sixteenth, 5, 4), vec![Note::new(2, fret)])));
    fourth.push(Beat::rest(value(Half)));

    let mut guitar = Track::new(
        "Lead",
        Instrument::DistortionGuitar,
        vec![Staff {
            tuning: Tuning::guitar_drop_d(),
            capo: 2,
            bars: vec![
                bar(first),
                Bar {
                    voices: vec![Voice { beats: second }, Voice { beats: vec![under] }],
                },
                bar(third),
                bar(fourth),
            ],
        }],
    );
    guitar.mix.volume = 60;
    guitar.mix.pan = -20;

    let mut bass = Track::empty("Bass", Instrument::Bass, 4);
    bass.staves[0].bars = vec![
        bar(vec![beat(Whole, &[(3, 0)])]),
        bar(vec![beat(Half, &[(3, 3)]), beat(Half, &[(2, 2)])]),
        bar(vec![Beat::new(dotted(Half), vec![Note::new(2, 0)])]),
        bar(vec![beat(Whole, &[(3, 5)])]),
    ];
    bass.mix.mute = true;

    let chord = |pitches: &[u8]| pitches.iter().map(|p| (*p, 0)).collect::<Vec<_>>();
    let mut piano = Track::empty("Keys", Instrument::Piano, 4);
    piano.staves[0].bars = vec![
        bar(vec![beat(Quarter, &chord(&[60, 64, 67])); 4]),
        bar(vec![beat(Quarter, &chord(&[62, 66, 69])); 4]),
        bar(vec![Beat::new(dotted(Half), vec![Note::new(70, 0)])]),
        bar(vec![beat(Whole, &chord(&[58, 62, 65, 70]))]),
    ];
    piano.staves[1].bars = vec![
        bar(vec![beat(Whole, &[(36, 0)])]),
        bar(vec![beat(Whole, &[(38, 0)])]),
        bar(vec![Beat::new(dotted(Half), vec![Note::new(34, 0)])]),
        bar(vec![beat(Whole, &[(34, 0)])]),
    ];

    let groove = |count: usize| -> Vec<Beat> {
        (0..count)
            .map(|index| match index % 4 {
                0 => beat(Eighth, &[(0, 42), (1, 36)]),
                2 => beat(Eighth, &[(0, 42), (1, 38)]),
                _ => beat(Eighth, &[(0, 42)]),
            })
            .collect()
    };
    let mut drums = Track::empty("Drums", Instrument::Drums, 4);
    drums.staves[0].bars = vec![
        bar(groove(8)),
        bar(groove(8)),
        bar(groove(6)),
        bar(vec![beat(Whole, &[(0, 49), (1, 36)])]),
    ];

    let mut score = Score {
        info: ScoreInfo {
            title: "Everything".into(),
            subtitle: "A test & a half".into(),
            artist: "gtrTab".into(),
            album: "Fixtures".into(),
            words: "Nobody".into(),
            music: "Somebody".into(),
            transcriber: "A <machine>".into(),
            copyright: "None".into(),
        },
        tempo: 120,
        master_bars: masters,
        tracks: vec![guitar, bass, piano, drums],
        effects: Vec::new(),
        audio: None,
    };
    // A pause is everybody's: every beat that starts at the same moment has it.
    let voices = (score.tracks.iter_mut().flat_map(|t| &mut t.staves))
        .flat_map(|staff| &mut staff.bars[1].voices);
    for voice in voices {
        let mut time = Rational::from_integer(0);
        for beat in &mut voice.beats {
            beat.effects.fermata = time == Rational::new(3, 4);
            time += beat.duration.length();
        }
    }
    score.validate().unwrap();
    score
}
