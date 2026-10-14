//! Guitar Pro export: a score written as a `.gp` file reads back as the score it was.

mod common;

use common::{band, corpus, difference, files};
use gtr_io::guitar_pro::{export, import};
use gtr_model::{Bend, Instrument, Score, Tuning};

/// A pitch curve as its outline: where it starts, how far it goes and where it ends.
fn outline(bend: &mut Bend) {
    let first = bend.points.first().map_or(0, |p| p.value);
    let last = bend.points.last().map_or(0, |p| p.value);
    *bend = Bend::new([(0, first), (30, bend.peak()), (60, last)]);
}

/// The score as a `.gp` file can say it, where that is not as it stands. A drum staff has
/// five lines there and its notes take them from the top; a volume or pan written on the
/// first beat that is the mixer's own is the mixer's; and with `outlines`, pitch curves
/// are compared by their outline alone.
fn as_written(mut score: Score, outlines: bool) -> Score {
    for master in &mut score.master_bars {
        (master.line_break, master.page_break) = (false, false);
    }
    for track in &mut score.tracks {
        let mix = track.mix;
        let first =
            (track.staves[0].bars.first_mut()).and_then(|bar| bar.voices[0].beats.first_mut());
        if let Some(beat) = first {
            beat.effects.volume.take_if(|volume| *volume == mix.volume);
            beat.effects.pan.take_if(|pan| *pan == mix.pan);
        }
        let drums = track.instrument == Instrument::Drums;
        for staff in &mut track.staves {
            if drums {
                staff.tuning = Tuning::new([0; 5]);
            }
            let beats = (staff.bars.iter_mut().flat_map(|bar| &mut bar.voices))
                .flat_map(|voice| &mut voice.beats);
            for beat in beats {
                if drums {
                    beat.notes.truncate(5);
                    for (line, note) in beat.notes.iter_mut().enumerate() {
                        note.string = line as u8;
                    }
                }
                if outlines {
                    beat.effects.whammy.as_mut().map(outline);
                    for note in &mut beat.notes {
                        note.effects.bend.as_mut().map(outline);
                    }
                }
            }
        }
    }
    score
}

#[test]
fn a_score_with_everything_reads_back_as_it_was() {
    let score = band();
    let written = export(&score).unwrap();
    common::keep("band.gp", &written.bytes);
    assert_eq!(
        written.skipped,
        ["where lines and pages were made to break"]
    );
    let read = import(&written.bytes).unwrap();
    assert_eq!(read.skipped, [] as [&str; 0]);
    let score = as_written(score, false);
    assert_eq!(difference(&score, &read.score), None);
    assert_eq!(score, read.score);
}

#[test]
fn every_file_of_the_corpus_reads_back_as_it_was() {
    let found = files(&corpus(), &["gp3", "gp4", "gp5", "gpx", "gp"]);
    assert!(found.len() > 200);
    let mut rough = 0;
    for path in &found {
        let name = path.strip_prefix(corpus()).unwrap().display().to_string();
        let first = import(&std::fs::read(path).unwrap()).unwrap().score;
        let written = export(&first).unwrap_or_else(|e| panic!("{name}: {e}"));
        let second = import(&written.bytes).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(second.skipped, [] as [&str; 0], "{name}");
        // The one thing these files have that the format cannot say exactly.
        let outlines = match written.skipped.as_slice() {
            [] => false,
            [only] if only.contains("curves") => true,
            other => panic!("{name}: {other:?}"),
        };
        rough += usize::from(outlines);
        let (first, second) = (
            as_written(first, outlines),
            as_written(second.score, outlines),
        );
        assert_eq!(difference(&first, &second), None, "{name}");
    }
    // The curves of more than four points are in the files of the older versions.
    assert!(rough < 12, "{rough}");
}

#[test]
fn what_the_format_cannot_say_is_reported() {
    let mut score = band();
    let beat = &mut score.tracks[0].staves[0].bars[0].voices[0].beats[0];
    beat.notes[0].effects.bend = Some(Bend::new([(0, 0), (10, 4), (20, 2), (30, 6), (40, 0)]));
    score.tracks[1].mix.volume = 100;
    let written = export(&score).unwrap();
    assert_eq!(
        written.skipped,
        [
            "the exact shape of curves with more than four points",
            "volumes louder than Guitar Pro goes",
            "where lines and pages were made to break"
        ]
    );
    // The curve keeps its start, its furthest reach and its end.
    let read = import(&written.bytes).unwrap().score;
    let bend = (read.tracks[0].staves[0].bars[0].voices[0].beats[0].notes[0]
        .effects
        .bend)
        .clone()
        .unwrap();
    assert_eq!(bend, Bend::new([(0, 0), (30, 6), (40, 0), (60, 0)]));
    assert_eq!(read.tracks[1].mix.volume, 94);
}

#[test]
fn pedals_sounds_and_the_switches_between_them_read_back_as_they_were() {
    use gtr_model::{Pedal, PedalKind, Sound};
    let pedal = |kind, on, knobs: &[u8]| {
        let mut pedal = Pedal::new(kind);
        pedal.on = on;
        pedal.knobs[..knobs.len()].copy_from_slice(knobs);
        pedal
    };
    let mut score = band();
    let guitar = &mut score.tracks[0];
    guitar.effects = vec![
        pedal(PedalKind::Overdrive, true, &[70, 40, 60]),
        pedal(PedalKind::Amp, true, &[80, 45, 55, 65]),
        pedal(PedalKind::Reverb, false, &[30, 40, 20]),
    ];
    guitar.sounds = vec![
        Sound {
            name: "Lead".into(),
            instrument: Instrument::DistortionGuitar,
            effects: vec![pedal(PedalKind::Eq, true, &[60, 70, 40])],
        },
        Sound {
            name: "Clean".into(),
            instrument: Instrument::CleanGuitar,
            effects: Vec::new(),
        },
    ];
    // To the lead in the second bar, and back to its own in the third.
    let bars = &mut score.tracks[0].staves[0].bars;
    bars[1].voices[0].beats[0].effects.sound = Some(1);
    if let Some(last) = bars[2].voices[0].beats.last_mut() {
        last.effects.sound = Some(0);
    }
    score.effects = vec![
        pedal(PedalKind::Compressor, true, &[40, 60]),
        pedal(PedalKind::Reverb, true, &[50, 50, 25]),
    ];

    let written = export(&score).unwrap();
    common::keep("pedals.gp", &written.bytes);
    assert_eq!(
        written.skipped,
        ["where lines and pages were made to break"]
    );
    let read = import(&written.bytes).unwrap();
    assert_eq!(read.skipped, [] as [&str; 0]);
    let score = as_written(score, false);
    assert_eq!(difference(&score, &read.score), None);
    assert_eq!(score, read.score);

    // A pedal Guitar Pro has nothing like is left out, and said to be.
    let mut wah = score.clone();
    wah.tracks[0].effects.push(Pedal::new(PedalKind::Wah));
    let written = export(&wah).unwrap();
    assert!(
        written
            .skipped
            .contains(&"pedals Guitar Pro has no match for".to_string())
    );
    assert_eq!(
        import(&written.bytes).unwrap().score.tracks[0].effects,
        score.tracks[0].effects
    );
}
