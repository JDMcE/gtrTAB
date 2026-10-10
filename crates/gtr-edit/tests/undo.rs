use gtr_edit::{BeatRef, Command, History};
use gtr_model::{
    Beat, Duration, Note, NoteValue, Score, TimeSignature, Tuning, Tuplet, demo_score,
};
use proptest::prelude::*;

type Seed = (u8, [usize; 3], u8, u8);

/// Turns random numbers into a command that is usually, but not always, valid for `score`.
fn build(score: &Score, (kind, idx, a, b): Seed) -> Command {
    let track = idx[0] % score.tracks.len();
    let bar = idx[1] % (score.master_bars.len() + 1);
    let beats = score.tracks[track].staves[0]
        .bars
        .get(bar)
        .map_or(0, |b| b.voices[0].beats.len());
    let at = BeatRef {
        track,
        staff: 0,
        bar,
        voice: 0,
        beat: idx[2] % (beats + 1),
    };
    let duration = Duration {
        dots: b % 4,
        tuplet: (a % 2 == 1).then_some(Tuplet::TRIPLET),
        ..Duration::new(NoteValue::ALL[usize::from(a) % NoteValue::ALL.len()])
    };
    let note = Note {
        tied: b % 2 == 0,
        ..Note::new(a % 7, b % 25)
    };
    let time_signature = TimeSignature {
        numerator: a % 13,
        denominator: 1 << (b % 5),
    };
    if kind >= 200 {
        // A batch whose last command may fail, to exercise rollback.
        return Command::Batch(vec![
            Command::InsertBeat {
                at,
                beat: Beat::rest(Duration::new(NoteValue::Quarter)),
            },
            Command::PutNote { at, note },
            Command::SetTempo {
                tempo: u32::from(b % 3),
            },
        ]);
    }
    if kind >= 190 {
        return Command::SetTuning {
            track,
            staff: 0,
            tuning: Tuning::new(vec![40; usize::from(a % 9)]),
            capo: b % 5,
        };
    }
    match kind % 9 {
        0 => Command::PutNote { at, note },
        1 => Command::RemoveNote {
            at,
            string: note.string,
        },
        2 => Command::InsertBeat {
            at,
            beat: Beat::new(duration, vec![note]),
        },
        3 => Command::InsertBeat {
            at,
            beat: Beat::rest(duration),
        },
        4 => Command::RemoveBeat { at },
        5 => Command::SetDuration { at, duration },
        6 => Command::insert_empty_bar(score, bar, time_signature),
        7 => Command::RemoveBar { index: bar },
        _ => match a % 2 {
            0 => Command::SetTimeSignature {
                bar,
                time_signature,
            },
            _ => Command::SetTempo {
                tempo: u32::from(b),
            },
        },
    }
}

proptest! {
    #[test]
    fn undo_restores_and_redo_replays(seeds in prop::collection::vec(any::<Seed>(), 0..80)) {
        let original = demo_score();
        let mut score = original.clone();
        let mut history = History::<()>::default();
        let mut applied = 0;

        for seed in seeds {
            let before = score.clone();
            let command = build(&score, seed);
            match history.execute(&mut score, command, ()) {
                Ok(()) => {
                    applied += 1;
                    prop_assert_eq!(score.validate(), Ok(()));
                }
                Err(_) => prop_assert_eq!(&score, &before),
            }
        }
        let edited = score.clone();

        let mut undone = 0;
        while history.undo(&mut score, ()).unwrap().is_some() {
            undone += 1;
        }
        prop_assert_eq!(undone, applied);
        prop_assert_eq!(&score, &original);

        while history.redo(&mut score, ()).unwrap().is_some() {}
        prop_assert_eq!(&score, &edited);
    }
}

#[test]
fn execute_clears_redo() {
    let mut score = demo_score();
    let mut history = History::<()>::default();
    history
        .execute(&mut score, Command::SetTempo { tempo: 90 }, ())
        .unwrap();
    history.undo(&mut score, ()).unwrap();
    assert!(history.can_redo());
    history
        .execute(&mut score, Command::SetTempo { tempo: 100 }, ())
        .unwrap();
    assert!(!history.can_redo());
    assert_eq!(score.tempo, 100);
}
