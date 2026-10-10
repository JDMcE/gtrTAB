use gtr_edit::{Editor, Move};
use gtr_model::{Note, NoteValue, Rational, Score, TimeSignature, Tuning, demo_score};
use proptest::prelude::*;

fn beats(editor: &Editor, bar: usize) -> &[gtr_model::Beat] {
    &editor.score().tracks[0].staves[0].bars[bar].voices[0].beats
}

#[test]
fn typing_a_bar_of_quarters() {
    let mut e = Editor::new(Score::blank());
    assert_eq!(e.cursor().string, 5);
    for fret in [0, 3, 5, 7] {
        e.type_digit(fret, false).unwrap();
        e.move_cursor(Move::Right, false).unwrap();
    }
    // The first bar is full, so the last move started a second bar.
    assert_eq!(e.score().master_bars.len(), 2);
    assert_eq!((e.cursor().bar, e.cursor().beat), (1, 0));
    let frets: Vec<u8> = beats(&e, 0).iter().map(|b| b.notes[0].fret).collect();
    assert_eq!(frets, [0, 3, 5, 7]);
    e.score().validate().unwrap();

    // An empty last bar does not grow the score further.
    e.move_cursor(Move::Right, false).unwrap();
    assert_eq!(e.score().master_bars.len(), 2);
}

#[test]
fn digits_combine_only_when_asked() {
    let mut e = Editor::new(Score::blank());
    e.type_digit(1, true).unwrap();
    e.type_digit(2, true).unwrap();
    assert_eq!(beats(&e, 0)[0].notes[0].fret, 12);
    e.type_digit(5, false).unwrap();
    assert_eq!(beats(&e, 0)[0].notes[0].fret, 5);
    // 5 then 9 would be fret 59, which does not exist.
    e.type_digit(9, true).unwrap();
    assert_eq!(beats(&e, 0)[0].notes[0].fret, 9);
    // Moving away ends the number.
    e.type_digit(1, true).unwrap();
    e.move_cursor(Move::Up, false).unwrap();
    e.move_cursor(Move::Down, false).unwrap();
    e.type_digit(2, true).unwrap();
    assert_eq!(beats(&e, 0)[0].notes[0].fret, 2);
}

#[test]
fn undo_restores_cursor_and_dirty_flag() {
    let mut e = Editor::new(Score::blank());
    assert!(!e.state().dirty);
    e.type_digit(3, false).unwrap();
    e.move_cursor(Move::Right, false).unwrap();
    e.type_digit(5, false).unwrap();
    assert!(e.state().dirty);
    e.mark_saved();
    assert!(!e.state().dirty);

    assert!(e.undo().unwrap());
    assert_eq!(e.cursor().beat, 1);
    assert!(e.state().dirty);
    assert!(e.redo().unwrap());
    assert!(!e.state().dirty);

    // Undoing past the save point and editing makes the saved state unreachable.
    e.undo().unwrap();
    e.type_digit(7, false).unwrap();
    e.undo().unwrap();
    assert!(e.state().dirty);
}

#[test]
fn durations_dots_and_rests() {
    let mut e = Editor::new(Score::blank());
    e.set_note_value(NoteValue::Eighth).unwrap();
    e.toggle_dot().unwrap();
    e.type_digit(0, false).unwrap();
    assert_eq!(beats(&e, 0)[0].duration.length(), Rational::new(3, 16));
    e.shorter().unwrap();
    assert_eq!(beats(&e, 0)[0].duration.value, NoteValue::Sixteenth);
    e.toggle_dot().unwrap();
    e.toggle_triplet().unwrap();
    assert_eq!(beats(&e, 0)[0].duration.length(), Rational::new(1, 24));
    e.rest().unwrap();
    assert!(beats(&e, 0)[0].is_rest());
    // Deleting a rest removes the beat.
    e.delete().unwrap();
    assert!(beats(&e, 0).is_empty());
}

#[test]
fn a_dot_does_not_carry_into_the_next_beat() {
    let mut e = Editor::new(Score::blank());
    e.toggle_dot().unwrap();
    e.type_digit(3, false).unwrap();
    e.move_cursor(Move::Right, false).unwrap();
    e.type_digit(5, false).unwrap();
    assert_eq!(beats(&e, 0)[0].duration.dots, 1);
    assert_eq!(beats(&e, 0)[1].duration.dots, 0);
    assert_eq!(beats(&e, 0)[1].duration.value, NoteValue::Quarter);

    // Changing string keeps a dot chosen for the beat about to be entered.
    e.move_cursor(Move::Right, false).unwrap();
    e.toggle_dot().unwrap();
    e.move_cursor(Move::Up, false).unwrap();
    e.type_digit(7, false).unwrap();
    assert_eq!(beats(&e, 0)[2].duration.dots, 1);
}

#[test]
fn tie_copies_the_previous_note() {
    let mut e = Editor::new(Score::blank());
    e.type_digit(7, false).unwrap();
    e.move_cursor(Move::Right, false).unwrap();
    e.toggle_tie().unwrap();
    assert_eq!(
        beats(&e, 0)[1].notes,
        [Note {
            string: 5,
            fret: 7,
            tied: true
        }]
    );
    e.toggle_tie().unwrap();
    assert!(!beats(&e, 0)[1].notes[0].tied);
}

#[test]
fn copy_and_paste_bars_and_beats() {
    let mut e = Editor::new(demo_score());
    // Select all of bar 0 and copy it as a bar.
    e.set_cursor(0, 0, 5, false);
    e.set_cursor(0, 7, 5, true);
    e.copy();
    e.move_cursor(Move::Last, false).unwrap();
    e.paste().unwrap();
    assert_eq!(e.score().master_bars.len(), 3);
    assert_eq!(beats(&e, 1), beats(&e, 0));
    assert_eq!(e.cursor().bar, 2);

    // Select two beats and paste them as loose beats.
    e.set_cursor(0, 2, 5, false);
    e.set_cursor(0, 3, 5, true);
    e.cut().unwrap();
    assert_eq!(beats(&e, 0).len(), 6);
    assert_eq!((e.cursor().bar, e.cursor().beat), (0, 2));
    e.paste().unwrap();
    assert_eq!(beats(&e, 0), beats(&e, 1));
    e.score().validate().unwrap();
}

#[test]
fn time_signature_applies_until_the_next_change() {
    let mut score = demo_score();
    score.master_bars.push(gtr_model::MasterBar {
        time_signature: TimeSignature {
            numerator: 6,
            denominator: 8,
        },
    });
    score.tracks[0].staves[0].bars.push(gtr_model::Bar::empty());
    let mut e = Editor::new(score);
    let three_four = TimeSignature {
        numerator: 3,
        denominator: 4,
    };
    e.set_time_signature(three_four).unwrap();
    let signatures: Vec<u8> = (e.score().master_bars.iter())
        .map(|m| m.time_signature.numerator)
        .collect();
    assert_eq!(signatures, [3, 3, 6]);
    e.undo().unwrap();
    assert_eq!(
        e.score().master_bars[0].time_signature,
        TimeSignature::COMMON
    );
}

#[test]
fn tuning_change_is_refused_when_notes_would_be_lost() {
    let mut e = Editor::new(demo_score());
    assert!(e.set_tuning(Tuning::bass_standard(), 0).is_err());
    e.set_tuning(Tuning::guitar_7_standard(), 2).unwrap();
    assert_eq!(e.score().tracks[0].staves[0].capo, 2);
}

#[derive(Debug, Clone)]
enum Op {
    Move(u8, bool),
    Digit(u8, bool),
    Click(usize, usize, u8, bool),
    Value(u8),
    Dot,
    Triplet,
    Rest,
    Tie,
    Delete,
    DeleteBeats,
    InsertBeat,
    InsertBar,
    RemoveBar,
    TimeSignature(u8, u8),
    Copy,
    Cut,
    Paste,
    Undo,
    Redo,
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        4 => (0u8..8, any::<bool>()).prop_map(|(m, e)| Op::Move(m, e)),
        4 => (0u8..10, any::<bool>()).prop_map(|(d, c)| Op::Digit(d, c)),
        1 => (0usize..6, 0usize..10, 0u8..8, any::<bool>())
            .prop_map(|(a, b, s, e)| Op::Click(a, b, s, e)),
        2 => (0u8..7).prop_map(Op::Value),
        1 => Just(Op::Dot),
        1 => Just(Op::Triplet),
        1 => Just(Op::Rest),
        1 => Just(Op::Tie),
        1 => Just(Op::Delete),
        1 => Just(Op::DeleteBeats),
        1 => Just(Op::InsertBeat),
        1 => Just(Op::InsertBar),
        1 => Just(Op::RemoveBar),
        1 => (1u8..13, 0u8..4).prop_map(|(n, d)| Op::TimeSignature(n, 1 << d)),
        1 => Just(Op::Copy),
        1 => Just(Op::Cut),
        2 => Just(Op::Paste),
        2 => Just(Op::Undo),
        1 => Just(Op::Redo),
    ]
}

fn apply(e: &mut Editor, op: Op) {
    const MOVES: [Move; 8] = [
        Move::Left,
        Move::Right,
        Move::Up,
        Move::Down,
        Move::PrevBar,
        Move::NextBar,
        Move::First,
        Move::Last,
    ];
    // Individual operations may be refused; that must leave the editor consistent.
    let _ = match op {
        Op::Move(m, extend) => e.move_cursor(MOVES[usize::from(m)], extend),
        Op::Digit(d, combine) => e.type_digit(d, combine),
        Op::Click(bar, beat, string, extend) => {
            e.set_cursor(bar, beat, string, extend);
            Ok(())
        }
        Op::Value(v) => e.set_note_value(NoteValue::ALL[usize::from(v)]),
        Op::Dot => e.toggle_dot(),
        Op::Triplet => e.toggle_triplet(),
        Op::Rest => e.rest(),
        Op::Tie => e.toggle_tie(),
        Op::Delete => e.delete(),
        Op::DeleteBeats => e.delete_beats(),
        Op::InsertBeat => e.insert_beat(),
        Op::InsertBar => e.insert_bar(),
        Op::RemoveBar => e.remove_bar(),
        Op::TimeSignature(numerator, denominator) => e.set_time_signature(TimeSignature {
            numerator,
            denominator,
        }),
        Op::Copy => {
            e.copy();
            Ok(())
        }
        Op::Cut => e.cut(),
        Op::Paste => e.paste(),
        Op::Undo => e.undo().map(drop),
        Op::Redo => e.redo().map(drop),
    };
}

proptest! {
    #[test]
    fn random_editing_stays_consistent(ops in prop::collection::vec(op(), 0..120)) {
        let original = demo_score();
        let mut e = Editor::new(original.clone());
        for op in ops {
            apply(&mut e, op);
            prop_assert_eq!(e.score().validate(), Ok(()));
            let c = e.cursor();
            let staff = &e.score().tracks[c.track].staves[0];
            prop_assert!(c.bar < staff.bars.len());
            prop_assert!(c.beat <= staff.bars[c.bar].voices[0].beats.len());
            prop_assert!(usize::from(c.string) < staff.tuning.strings.len());
        }
        while e.undo().unwrap() {}
        prop_assert_eq!(e.score(), &original);
        prop_assert!(!e.state().dirty);
    }
}
