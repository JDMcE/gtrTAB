use gtr_edit::{Editor, Move, Technique};
use gtr_model::{
    Bend, BendKind, Dynamic, Instrument, Mix, Note, NoteValue, Rational, Score, TimeSignature,
    Tuning, demo_score,
};
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
            tied: true,
            ..Note::new(5, 7)
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
    score
        .master_bars
        .push(gtr_model::MasterBar::new(TimeSignature {
            numerator: 6,
            denominator: 8,
        }));
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

#[test]
fn techniques_toggle_on_notes_chords_and_beats() {
    let mut e = Editor::new(demo_score());
    // The last beat of the demo is a three-note chord.
    e.set_cursor(1, 4, 5, false);
    e.toggle_technique(Technique::PalmMute).unwrap();
    assert!(beats(&e, 1)[4].notes.iter().all(|n| n.effects.palm_mute));
    e.toggle_technique(Technique::Vibrato).unwrap();
    let vibrato: Vec<bool> = (beats(&e, 1)[4].notes.iter())
        .map(|n| n.effects.vibrato.is_some())
        .collect();
    assert_eq!(vibrato, [false, false, true]);
    // One kind of slide replaces another; toggling the same one removes it.
    e.toggle_technique(Technique::SlideOutDown).unwrap();
    e.toggle_technique(Technique::SlideOutUp).unwrap();
    assert_eq!(
        e.state().note.unwrap().effects.slide,
        Some(gtr_model::Slide::OutUp)
    );
    e.toggle_technique(Technique::SlideOutUp).unwrap();
    assert_eq!(e.state().note.unwrap().effects.slide, None);

    e.toggle_technique(Technique::Tremolo).unwrap();
    assert!(e.state().beat_effects.unwrap().tremolo.is_some());
    e.toggle_technique(Technique::PalmMute).unwrap();
    assert!(beats(&e, 1)[4].notes.iter().all(|n| !n.effects.palm_mute));

    // With a selection, a technique covers every selected note and stays selected.
    e.set_cursor(0, 0, 5, false);
    e.set_cursor(0, 3, 5, true);
    e.toggle_technique(Technique::Hammer).unwrap();
    assert!(beats(&e, 0)[..4].iter().all(|b| b.notes[0].effects.hammer));
    assert!(!beats(&e, 0)[4].notes[0].effects.hammer);
    assert!(e.state().selection.is_some());

    e.score().validate().unwrap();
}

#[test]
fn effects_are_set_field_by_field() {
    use serde_json::json;
    let mut e = Editor::new(demo_score());
    e.set_cursor(0, 2, 4, false);

    let bend = Bend::preset(BendKind::BendRelease, 4);
    e.set_note_effect("bend", &serde_json::to_value(&bend).unwrap())
        .unwrap();
    e.set_note_effect("trill", &json!({ "fret": 4, "speed": "sixteenth" }))
        .unwrap();
    e.set_note_effect("left_finger", &json!("ring")).unwrap();
    let effects = e.state().note.unwrap().effects;
    assert_eq!(effects.bend, Some(bend));
    assert_eq!(effects.trill.unwrap().fret, 4);
    // Retyping the fret keeps the techniques.
    e.type_digit(5, false).unwrap();
    assert!(e.state().note.unwrap().effects.bend.is_some());
    e.set_note_effect("bend", &json!(null)).unwrap();
    assert_eq!(e.state().note.unwrap().effects.bend, None);

    // Values that make no sense are refused and leave the note alone.
    let too_high =
        json!({ "points": [{ "position": 0, "value": 0 }, { "position": 60, "value": 40 }] });
    assert!(e.set_note_effect("bend", &too_high).is_err());
    assert!(e.set_note_effect("no_such_effect", &json!(true)).is_err());
    assert!(e.set_note_effect("slide", &json!("sideways")).is_err());
    assert!(e.state().note.unwrap().effects.trill.is_some());

    e.set_beat_effect("dynamic", &json!("ff")).unwrap();
    e.set_beat_effect("whammy", &serde_json::to_value(Bend::dive(8)).unwrap())
        .unwrap();
    let beat = e.state().beat_effects.unwrap();
    assert_eq!(beat.dynamic, Some(Dynamic::Ff));
    assert_eq!(beat.whammy.unwrap().peak(), -8);
    // A string cannot be bent downwards, but the bar can go down.
    assert!(
        e.set_note_effect("bend", &serde_json::to_value(Bend::dive(8)).unwrap())
            .is_err()
    );
    e.score().validate().unwrap();
}

#[test]
fn tuplets_and_nested_tuplets() {
    use std::num::NonZeroU8;
    let tuplet = |actual, normal| gtr_model::Tuplet {
        actual: NonZeroU8::new(actual).unwrap(),
        normal: NonZeroU8::new(normal).unwrap(),
    };
    let mut e = Editor::new(Score::blank());
    assert!(e.set_nested_tuplet(Some(tuplet(3, 2))).is_err());
    e.set_note_value(NoteValue::Eighth).unwrap();
    e.set_tuplet(Some(tuplet(5, 4))).unwrap();
    e.type_digit(3, false).unwrap();
    assert_eq!(beats(&e, 0)[0].duration.length(), Rational::new(1, 10));
    e.set_nested_tuplet(Some(tuplet(3, 2))).unwrap();
    assert_eq!(beats(&e, 0)[0].duration.length(), Rational::new(1, 15));
    e.set_tuplet(None).unwrap();
    assert_eq!(beats(&e, 0)[0].duration.length(), Rational::new(1, 8));
}

#[derive(Debug, Clone)]
enum Op {
    Technique(usize),
    Bend(i8),
    Whammy(i8),
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
        4 => (0usize..Technique::ALL.len()).prop_map(Op::Technique),
        1 => (-2i8..16).prop_map(Op::Bend),
        1 => (-30i8..16).prop_map(Op::Whammy),
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
        Op::Technique(t) => e.toggle_technique(Technique::ALL[t]),
        Op::Bend(height) => {
            let bend = Bend::preset(BendKind::Bend, height);
            e.set_note_effect("bend", &serde_json::to_value(bend).unwrap())
        }
        Op::Whammy(depth) => {
            e.set_beat_effect("whammy", &serde_json::to_value(Bend::dive(depth)).unwrap())
        }
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

#[test]
fn tracks_are_added_moved_and_removed_around_the_cursor() {
    let names =
        |e: &Editor| -> Vec<String> { e.score().tracks.iter().map(|t| t.name.clone()).collect() };
    let mut e = Editor::new(demo_score());
    e.set_cursor(1, 2, 4, false);
    e.add_track("Bass", Instrument::Bass, None).unwrap();
    // The new track goes below the current one, takes the cursor and has every bar.
    assert_eq!(names(&e), ["Guitar", "Bass"]);
    assert_eq!((e.cursor().track, e.cursor().bar), (1, 1));
    assert_eq!(e.cursor().string, 3);
    assert_eq!(
        e.score().tracks[1].staves[0].tuning,
        Tuning::bass_standard()
    );
    assert_eq!(e.score().tracks[1].staves[0].bars.len(), 2);
    e.score().validate().unwrap();

    // Notes go to the cursor's track, and a new bar reaches every track.
    e.type_digit(3, false).unwrap();
    assert_eq!(
        e.score().tracks[1].staves[0].bars[1].voices[0].beats.len(),
        1
    );
    e.insert_bar().unwrap();
    assert!(e.score().tracks.iter().all(|t| t.staves[0].bars.len() == 3));

    e.set_track(0);
    let drop_d = Some((Tuning::guitar_drop_d(), 2));
    e.add_track("Lead", Instrument::DistortionGuitar, drop_d)
        .unwrap();
    assert_eq!(names(&e), ["Guitar", "Lead", "Bass"]);
    assert_eq!(e.cursor().track, 1);
    let lead = &e.score().tracks[1].staves[0];
    assert_eq!((&lead.tuning, lead.capo), (&Tuning::guitar_drop_d(), 2));
    // A tuning without strings is refused, and nothing is added.
    let no_strings = Some((Tuning::new(Vec::new()), 0));
    assert!(e.add_track("Air", Instrument::Bass, no_strings).is_err());
    assert_eq!(e.score().tracks.len(), 3);

    // Moving a track carries the cursor with it; moving another past it shifts the cursor.
    e.move_track(1, 2).unwrap();
    assert_eq!(names(&e), ["Guitar", "Bass", "Lead"]);
    assert_eq!(e.cursor().track, 2);
    e.move_track(0, 2).unwrap();
    assert_eq!(names(&e), ["Bass", "Lead", "Guitar"]);
    assert_eq!(e.cursor().track, 1);
    e.undo().unwrap();
    assert_eq!(names(&e), ["Guitar", "Bass", "Lead"]);
    assert_eq!(e.cursor().track, 2);

    // Removing a track above the cursor keeps the cursor on its track.
    e.remove_track(0).unwrap();
    assert_eq!(names(&e), ["Bass", "Lead"]);
    assert_eq!(e.cursor().track, 1);
    e.remove_track(1).unwrap();
    assert_eq!(e.cursor().track, 0);
    assert!(e.remove_track(0).is_err());
    e.undo().unwrap();
    e.undo().unwrap();
    assert_eq!(names(&e), ["Guitar", "Bass", "Lead"]);
    e.score().validate().unwrap();
}

#[test]
fn track_settings_are_undoable_and_checked() {
    let mut e = Editor::new(demo_score());
    e.set_track_info(0, "Rhythm", Instrument::OverdriveGuitar)
        .unwrap();
    let loud = Mix {
        volume: 100,
        pan: -40,
        mute: false,
        solo: true,
    };
    e.set_mix(0, loud).unwrap();
    let track = &e.score().tracks[0];
    assert_eq!(
        (track.name.as_str(), track.instrument),
        ("Rhythm", Instrument::OverdriveGuitar)
    );
    assert_eq!(track.mix, loud);

    // Setting the same mix again is not an edit.
    let revision = e.state().revision;
    e.set_mix(0, loud).unwrap();
    assert_eq!(e.state().revision, revision);
    let off_scale = Mix {
        volume: 101,
        ..loud
    };
    assert!(e.set_mix(0, off_scale).is_err());
    assert!(e.set_mix(3, loud).is_err());

    e.undo().unwrap();
    assert_eq!(e.score().tracks[0].mix, Mix::default());
    e.undo().unwrap();
    assert_eq!(e.score().tracks[0].name, "Guitar");
}

#[test]
fn a_second_voice_is_added_on_first_entry() {
    let mut e = Editor::new(demo_score());
    let voices = |e: &Editor, bar: usize| e.score().tracks[0].staves[0].bars[bar].voices.len();
    e.set_voice(1);
    assert_eq!(e.cursor().voice, 1);
    // The second voice of a full bar starts out empty, so the cursor is on its first slot.
    assert_eq!((e.cursor().bar, e.cursor().beat), (0, 0));
    assert!(e.state().note.is_none());
    // Looking at a voice does not create it.
    assert_eq!(voices(&e, 0), 1);

    e.set_note_value(NoteValue::Half).unwrap();
    e.type_digit(7, false).unwrap();
    e.move_cursor(Move::Right, false).unwrap();
    e.rest().unwrap();
    assert_eq!(voices(&e, 0), 2);
    let second = &e.score().tracks[0].staves[0].bars[0].voices[1].beats;
    assert_eq!(second.len(), 2);
    assert_eq!(second[0].notes[0].fret, 7);
    assert!(second[1].is_rest());
    // The first voice is untouched, and the other bar still has only one.
    assert_eq!(beats(&e, 0).len(), 8);
    assert_eq!(voices(&e, 1), 1);
    e.score().validate().unwrap();

    // Moving right from the end of a full voice goes on to the next bar, in the same voice.
    e.move_cursor(Move::Right, false).unwrap();
    assert_eq!(
        (e.cursor().bar, e.cursor().beat, e.cursor().voice),
        (1, 0, 1)
    );

    // Each entry is one undo step, voice and all.
    e.undo().unwrap();
    e.undo().unwrap();
    assert_eq!(voices(&e, 0), 1);
    assert_eq!(e.score(), &demo_score());

    // Voices beyond the fourth do not exist.
    e.set_voice(9);
    assert_eq!(e.cursor().voice, 3);
}

#[test]
fn key_signature_applies_until_the_next_change() {
    let mut score = demo_score();
    score
        .master_bars
        .push(gtr_model::MasterBar::new(TimeSignature::COMMON));
    score.tracks[0].staves[0].bars.push(gtr_model::Bar::empty());
    score.master_bars[2].key_signature = -1;
    let mut e = Editor::new(score);
    e.set_key_signature(2).unwrap();
    let keys: Vec<i8> = (e.score().master_bars.iter())
        .map(|m| m.key_signature)
        .collect();
    assert_eq!(keys, [2, 2, -1]);
    assert!(e.set_key_signature(8).is_err());
    e.undo().unwrap();
    assert_eq!(e.score().master_bars[0].key_signature, 0);
}

#[test]
fn a_piano_is_edited_one_staff_at_a_time() {
    let mut e = Editor::new(demo_score());
    e.add_track("Piano", Instrument::Piano, None).unwrap();
    assert_eq!(e.score().tracks[1].staves.len(), 2);
    assert_eq!((e.cursor().track, e.cursor().staff), (1, 0));

    e.set_staff(1);
    e.enter_note().unwrap();
    let staves = &e.score().tracks[1].staves;
    assert!(staves[0].bars[0].voices[0].beats.is_empty());
    assert_eq!(staves[1].bars[0].voices[0].beats[0].notes.len(), 1);
    // A new bar reaches both staves, and a staff that does not exist is not gone to.
    e.insert_bar().unwrap();
    assert!(e.score().tracks[1].staves.iter().all(|s| s.bars.len() == 3));
    e.set_staff(5);
    assert_eq!(e.cursor().staff, 1);
    // Going to another track starts on its first staff.
    e.set_track(0);
    assert_eq!(e.cursor().staff, 0);
    e.score().validate().unwrap();
}

#[test]
fn notes_without_strings_are_entered_by_name_and_moved_by_pitch() {
    let mut e = Editor::new(Score::blank());
    e.add_track("Piano", Instrument::Piano, None).unwrap();
    let pitches = |e: &Editor| -> Vec<u8> {
        let beat = &e.score().tracks[1].staves[e.cursor().staff].bars[0].voices[0].beats[0];
        beat.notes.iter().map(|n| n.string).collect()
    };
    // The cursor starts on the middle line of the staff: B above middle C.
    assert_eq!(e.cursor().string, 71);
    // A fret means nothing here.
    assert!(e.type_digit(3, false).is_err() && e.set_fret(3).is_err());

    // C is entered at the nearest C, which is the one above; then the E and G above it, as
    // many notes on the beat as are wanted.
    for letter in [0, 2, 4] {
        e.enter_letter(letter).unwrap();
    }
    assert_eq!(pitches(&e), [72, 76, 79]);
    assert_eq!(e.cursor().string, 79);
    // The nearest F to that G is the one below it.
    e.enter_letter(3).unwrap();
    assert_eq!(pitches(&e), [72, 76, 77, 79]);

    // Up and down go through the notes of the key; in D major that has F and C sharp.
    e.set_key_signature(2).unwrap();
    for (direction, expected) in [
        (Move::Up, 78),
        (Move::Up, 79),
        (Move::Down, 78),
        (Move::Down, 76),
    ] {
        e.move_cursor(direction, false).unwrap();
        assert_eq!(e.cursor().string, expected);
    }
    // And a letter means the note the key has for it: F sharp.
    e.enter_letter(3).unwrap();
    assert_eq!(e.cursor().string, 78);
    e.undo().unwrap();

    // A semitone at a time moves the note under the cursor, and the cursor with it, but
    // not onto another note; where there is no note only the cursor moves.
    e.set_cursor(0, 0, 76, false);
    e.shift_pitch(-1).unwrap();
    assert_eq!((pitches(&e), e.cursor().string), (vec![72, 75, 77, 79], 75));
    e.undo().unwrap();
    assert_eq!(pitches(&e), [72, 76, 77, 79]);
    e.set_cursor(0, 0, 76, false);
    e.shift_pitch(1).unwrap();
    assert_eq!((pitches(&e), e.cursor().string), (vec![72, 76, 77, 79], 77));
    e.shift_pitch(1).unwrap();
    assert_eq!((pitches(&e), e.cursor().string), (vec![72, 76, 78, 79], 78));

    // Delete takes the note under the cursor, and Enter puts one back where it is.
    e.delete().unwrap();
    assert_eq!(pitches(&e), [72, 76, 79]);
    e.enter_note().unwrap();
    e.enter_note().unwrap();
    assert_eq!(pitches(&e), [72, 76, 78, 79]);
    // The left hand starts lower, and going back finds the top note of the beat.
    e.set_staff(1);
    assert_eq!(e.cursor().string, 48);
    e.set_staff(0);
    assert_eq!(e.cursor().string, 79);
    e.score().validate().unwrap();
}

#[test]
fn changing_to_and_from_a_piano_rewrites_the_notes() {
    let mut e = Editor::new(Score::blank());
    // A G major chord across all six strings, low to high: G2 B2 D3 G3 B3 G4.
    for (string, fret) in [(5, 3), (4, 2), (3, 0), (2, 0), (1, 0), (0, 3)] {
        e.set_cursor(0, 0, string, false);
        e.type_digit(fret, false).unwrap();
    }
    let guitar = e.score().tracks[0].clone();

    e.set_track_info(0, "Keys", Instrument::Piano).unwrap();
    let track = &e.score().tracks[0];
    assert_eq!((track.name.as_str(), track.staves.len()), ("Keys", 2));
    assert!(
        track
            .staves
            .iter()
            .all(|s| s.is_pitched() && s.bars.len() == 1)
    );
    let pitches: Vec<u8> = (track.staves[0].bars[0].voices[0].beats[0].notes.iter())
        .map(|n| n.string)
        .collect();
    assert_eq!(pitches, [43, 47, 50, 55, 59, 67]);
    assert!(track.staves[1].bars[0].voices[0].beats.is_empty());
    assert!(usize::from(e.cursor().string) < 128);
    e.score().validate().unwrap();

    // Back on a guitar every note finds a string, as near the nut as it can be played.
    e.set_track_info(0, "Guitar", Instrument::SteelGuitar)
        .unwrap();
    let track = &e.score().tracks[0];
    assert_eq!(track.staves.len(), 1);
    assert_eq!(track.staves[0].tuning, Tuning::guitar_standard());
    let frets: Vec<(u8, u8)> = (track.staves[0].bars[0].voices[0].beats[0].notes.iter())
        .map(|n| (n.string, n.fret))
        .collect();
    assert_eq!(frets, [(0, 3), (1, 0), (2, 0), (3, 0), (4, 2), (5, 3)]);
    assert!(e.cursor().string <= 5);
    e.score().validate().unwrap();
    // One step each way to undo.
    e.undo().unwrap();
    assert!(e.score().tracks[0].staves[0].is_pitched());
    e.undo().unwrap();
    assert_eq!(e.score().tracks[0], guitar);

    // A bass has four strings for the six notes: the highest four get them. And a tuning
    // cannot give a piano strings or take a guitar's away.
    e.set_track_info(0, "Keys", Instrument::Piano).unwrap();
    e.set_track_info(0, "Bass", Instrument::Bass).unwrap();
    let frets: Vec<(u8, u8)> = (e.score().tracks[0].staves[0].bars[0].voices[0].beats[0]
        .notes
        .iter())
    .map(|n| (n.string, n.fret))
    .collect();
    assert_eq!(frets, [(0, 24), (1, 21), (2, 22), (3, 22)]);
    e.undo().unwrap();
    assert!(e.set_tuning(Tuning::guitar_standard(), 0).is_err());
    assert!(
        e.add_track(
            "Air",
            Instrument::Piano,
            Some((Tuning::guitar_standard(), 0))
        )
        .is_err()
    );
}

#[test]
fn drum_tracks_take_kit_numbers_as_frets() {
    let mut e = Editor::new(demo_score());
    // Fret 42 does not exist on a guitar.
    assert!(e.set_fret(42).is_err());
    e.type_digit(4, true).unwrap();
    e.type_digit(2, true).unwrap();
    assert_eq!(e.state().note.map(|n| n.fret), Some(2));

    e.add_track("Drums", Instrument::Drums, None).unwrap();
    assert_eq!(e.score().tracks[1].staves[0].tuning.strings.len(), 5);
    // Closed hi-hat by typing its number, then a snare in one go on the line below.
    e.type_digit(4, true).unwrap();
    e.type_digit(2, true).unwrap();
    assert_eq!(e.state().note.map(|n| n.fret), Some(42));
    e.move_cursor(Move::Up, false).unwrap();
    e.set_fret(38).unwrap();
    let frets: Vec<u8> = (e.score().tracks[1].staves[0].bars[0].voices[0].beats[0]
        .notes
        .iter())
    .map(|n| n.fret)
    .collect();
    assert_eq!(frets, [38, 42]);
    assert!(e.set_fret(88).is_err());
}

#[test]
fn a_tempo_change_goes_on_the_beat_under_the_cursor() {
    let mut e = Editor::new(Score::blank());
    for fret in [0, 3, 5, 7] {
        e.type_digit(fret, false).unwrap();
        e.move_cursor(Move::Right, false).unwrap();
    }
    // On the third quarter of the first bar: half-way through it.
    e.set_cursor(0, 2, 5, false);
    assert_eq!(e.state().tempo_change, None);
    e.set_tempo_change(Some(90)).unwrap();
    let changes = &e.score().master_bars[0].tempo_changes;
    assert_eq!(changes.len(), 1);
    assert_eq!(
        *changes[0].position.numer() * 2,
        *changes[0].position.denom()
    );
    assert_eq!(e.state().tempo_change, Some(90));
    // One before it keeps the two in order; a new value replaces the old.
    e.set_cursor(0, 1, 5, false);
    e.set_tempo_change(Some(60)).unwrap();
    e.set_tempo_change(Some(70)).unwrap();
    let tempos: Vec<u32> = (e.score().master_bars[0].tempo_changes.iter())
        .map(|c| c.tempo)
        .collect();
    assert_eq!(tempos, [70, 90]);
    e.score().validate().unwrap();

    // On the first beat it is the bar's own tempo, as before.
    e.set_cursor(0, 0, 5, false);
    e.set_tempo_change(Some(100)).unwrap();
    assert_eq!(e.score().master_bars[0].tempo, Some(100));
    assert_eq!(e.score().master_bars[0].tempo_changes.len(), 2);

    // Taking one away, and undoing that.
    e.set_cursor(0, 2, 5, false);
    e.set_tempo_change(None).unwrap();
    assert_eq!(e.score().master_bars[0].tempo_changes.len(), 1);
    assert_eq!(e.state().tempo_change, None);
    e.undo().unwrap();
    assert_eq!(e.state().tempo_change, Some(90));
}

#[test]
fn a_drum_track_can_have_a_kit_of_its_own() {
    let mut e = Editor::new(Score::blank());
    e.add_track("Drums", Instrument::Drums, None).unwrap();
    assert!(e.score().tracks[1].kit.is_empty());
    assert_eq!(e.score().tracks[1].drum_kit().len(), 23);

    // A kit of three pieces, with a cowbell where the snare is usually written.
    let mut kit = gtr_model::standard_kit();
    kit.retain(|p| matches!(p.number, 36 | 42 | 56));
    kit[2].position = 1;
    e.set_kit(1, kit.clone()).unwrap();
    assert_eq!(e.score().tracks[1].drum_kit(), kit);
    e.score().validate().unwrap();
    // The same number twice is refused, and nothing changes.
    let mut twice = kit.clone();
    twice[1].number = 36;
    assert!(e.set_kit(1, twice).is_err());
    assert_eq!(e.score().tracks[1].kit, kit);
    assert!(e.set_kit(9, kit.clone()).is_err());

    // Setting the kit it already has is not an edit; undo gives the standard kit back.
    e.set_kit(1, kit).unwrap();
    e.undo().unwrap();
    assert!(e.score().tracks[1].kit.is_empty());
    // The kit is kept when the track becomes another instrument, and saved with it.
    e.redo().unwrap();
    e.set_track_info(1, "Kit", Instrument::Drums).unwrap();
    assert_eq!(e.score().tracks[1].kit.len(), 3);
}
