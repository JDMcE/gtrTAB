use gtr_model::{
    Bar, Beat, BeatEffects, Clef, Duration, Instrument, KitPiece, MasterBar, Mix, Note, NoteValue,
    Score, ScoreInfo, Staff, TimeSignature, Track, Tuning, Tuplet, Voice,
};
use gtr_model::{Rational, TempoChange};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::technique::Target;
use crate::{BeatRef, Command, EditError, History, Technique};

/// The highest fret on a stringed instrument; see `Instrument::max_fret` for the others.
pub const MAX_FRET: u8 = 36;

/// Where typing goes. `beat` may equal the number of beats in the bar: that is the empty slot
/// after the last beat, available while the bar is not yet full.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ts_rs::TS)]
#[ts(export)]
pub struct Cursor {
    pub track: usize,
    pub bar: usize,
    pub beat: usize,
    /// The string typing goes to, or on a staff without strings the pitch.
    pub string: u8,
    /// The voice being edited. A bar may not have it yet; it is added on the first entry.
    pub voice: usize,
    /// The staff of the track being edited: a piano has one for each hand.
    pub staff: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Move {
    Left,
    Right,
    Up,
    Down,
    PrevBar,
    NextBar,
    First,
    Last,
}

/// A `(bar, beat)` position within the cursor's track.
type Position = (usize, usize);

#[derive(Debug, Clone)]
enum Clipboard {
    Beats(Vec<Beat>),
    Bars(Vec<(MasterBar, Bar)>),
}

/// Snapshot of the editor state the UI needs besides the score itself.
#[derive(Debug, Clone, Serialize, ts_rs::TS)]
#[ts(export)]
pub struct EditorState {
    pub cursor: Cursor,
    /// First and last selected `[bar, beat]`, inclusive.
    pub selection: Option<[Position; 2]>,
    /// Duration of the beat under the cursor, or of the next beat to be entered.
    pub duration: Duration,
    /// The note under the cursor, if there is one.
    pub note: Option<Note>,
    /// Effects of the beat under the cursor, if there is one.
    pub beat_effects: Option<BeatEffects>,
    /// The tempo that is written to change on the cursor's beat, if one is.
    pub tempo_change: Option<u32>,
    pub can_undo: bool,
    pub can_redo: bool,
    pub can_paste: bool,
    pub dirty: bool,
    /// Changes whenever the score does.
    #[ts(type = "number")]
    pub revision: u64,
}

/// A score plus everything needed to edit it from the keyboard: cursor, selection, input
/// duration, clipboard and undo history. Edits go to the first staff of the cursor's track,
/// in the cursor's voice.
pub struct Editor {
    score: Score,
    history: History<Cursor>,
    cursor: Cursor,
    anchor: Option<Position>,
    duration: Duration,
    clipboard: Option<Clipboard>,
    /// History depth at the last save, or `None` if that state can no longer be reached.
    saved_depth: Option<usize>,
    digit_pending: bool,
    revision: u64,
}

impl Editor {
    /// Wraps a score for editing. Scores without a track, staff or bar get a default one so
    /// that the cursor always has somewhere to be.
    pub fn new(mut score: Score) -> Self {
        if score.tracks.is_empty() {
            let track = Track::new("Guitar", Instrument::default(), Vec::new());
            score.tracks.push(track);
        }
        if score.master_bars.is_empty() {
            score
                .master_bars
                .push(MasterBar::new(TimeSignature::COMMON));
        }
        let bar_count = score.master_bars.len();
        for track in &mut score.tracks {
            if track.staves.is_empty() {
                track.staves.push(Staff {
                    tuning: track.instrument.default_tuning(),
                    capo: 0,
                    bars: Vec::new(),
                });
            }
            for staff in &mut track.staves {
                staff.bars.resize_with(bar_count, Bar::empty);
            }
        }
        let mut editor = Editor {
            score,
            history: History::default(),
            cursor: Cursor {
                track: 0,
                bar: 0,
                beat: 0,
                string: 0,
                voice: 0,
                staff: 0,
            },
            anchor: None,
            duration: Duration::new(NoteValue::Quarter),
            clipboard: None,
            saved_depth: Some(0),
            digit_pending: false,
            revision: 0,
        };
        editor.cursor.string = u8::MAX;
        editor.settle();
        editor.home();
        editor
    }

    pub fn score(&self) -> &Score {
        &self.score
    }

    pub fn cursor(&self) -> Cursor {
        self.cursor
    }

    pub fn state(&self) -> EditorState {
        EditorState {
            cursor: self.cursor,
            selection: self.selection().map(|(a, b)| [a, b]),
            duration: self.duration,
            note: (self.current_beat())
                .and_then(|b| b.note_on(self.cursor.string))
                .cloned(),
            beat_effects: self.current_beat().map(|b| b.effects.clone()),
            tempo_change: self.tempo_change(),
            can_undo: self.history.can_undo(),
            can_redo: self.history.can_redo(),
            can_paste: self.clipboard.is_some(),
            dirty: self.saved_depth != Some(self.history.depth()),
            revision: self.revision,
        }
    }

    /// Records that the current state is what is on disk.
    pub fn mark_saved(&mut self) {
        self.saved_depth = Some(self.history.depth());
    }

    /// Records that the score has never been saved in its current form, as after an import.
    pub fn mark_unsaved(&mut self) {
        self.saved_depth = None;
    }

    // --- Cursor and selection ---

    pub fn move_cursor(&mut self, direction: Move, extend: bool) -> Result<(), EditError> {
        self.anchor = match (extend, self.anchor) {
            (true, None) => Some((self.cursor.bar, self.cursor.beat)),
            (true, anchor) => anchor,
            (false, _) => None,
        };
        let last_bar = self.score.master_bars.len() - 1;
        let Cursor { bar, beat, .. } = self.cursor;
        let len = self.beats(bar).len();
        match direction {
            // On a staff without strings the cursor moves through the notes of the key.
            Move::Up | Move::Down if self.staff().is_pitched() => {
                let key = self.score.master_bars[bar].key_signature;
                let up = direction == Move::Up;
                self.cursor.string = step_in_key(self.cursor.string, key, up);
            }
            Move::Up => self.cursor.string = self.cursor.string.saturating_sub(1),
            Move::Down => self.cursor.string = self.cursor.string.saturating_add(1),
            Move::Left if beat > 0 => self.cursor.beat -= 1,
            Move::Left | Move::PrevBar if bar > 0 && beat == 0 => {
                self.cursor.bar -= 1;
                self.cursor.beat = match direction {
                    Move::Left => self.beats(bar - 1).len().saturating_sub(1),
                    _ => 0,
                };
            }
            Move::Left | Move::PrevBar => self.cursor.beat = 0,
            Move::Right if beat + 1 < len => self.cursor.beat += 1,
            Move::Right if beat < len && !self.bar_full(bar) => self.cursor.beat = len,
            Move::Right | Move::NextBar => {
                if bar < last_bar {
                    self.cursor.bar += 1;
                    self.cursor.beat = 0;
                } else if len > 0 && !extend {
                    // Moving past the end of a score that ends with music starts a new bar.
                    let time_signature = self.score.master_bars[bar].time_signature;
                    self.exec(Command::insert_empty_bar(
                        &self.score,
                        bar + 1,
                        time_signature,
                    ))?;
                    self.cursor.bar += 1;
                    self.cursor.beat = 0;
                }
            }
            Move::First => {
                self.cursor.bar = 0;
                self.cursor.beat = 0;
            }
            Move::Last => {
                self.cursor.bar = last_bar;
                self.cursor.beat = 0;
            }
        }
        self.settle();
        // A new beat takes the note value of the one before it, but not its dot.
        let moved = (self.cursor.bar, self.cursor.beat) != (bar, beat);
        if moved && self.current_beat().is_none() {
            self.duration.dots = 0;
        }
        Ok(())
    }

    /// Moves the cursor to a clicked position.
    pub fn set_cursor(&mut self, bar: usize, beat: usize, string: u8, extend: bool) {
        self.anchor = match (extend, self.anchor) {
            (true, None) => Some((self.cursor.bar, self.cursor.beat)),
            (true, anchor) => anchor,
            (false, _) => None,
        };
        self.cursor.bar = bar;
        self.cursor.beat = beat;
        self.cursor.string = string;
        self.settle();
    }

    /// Moves the cursor to the start of a bar, on the string or pitch it is on.
    pub fn go_to_bar(&mut self, bar: usize) {
        self.set_cursor(bar, 0, self.cursor.string, false);
    }

    pub fn set_track(&mut self, track: usize) {
        self.anchor = None;
        self.cursor.track = track;
        self.cursor.staff = 0;
        self.settle();
        self.home();
    }

    /// Switches to another staff of the cursor's track, counted from the top.
    pub fn set_staff(&mut self, staff: usize) {
        self.anchor = None;
        self.cursor.staff = staff;
        self.settle();
        self.home();
    }

    /// On arriving at a staff without strings, puts the cursor on the top note of its
    /// beat, or failing that in the middle of the staff. A string needs no such help: the
    /// one the cursor was on, or the nearest there is, will do.
    fn home(&mut self) {
        if !self.staff().is_pitched() {
            return;
        }
        let top = self.current_beat().and_then(|b| b.notes.last());
        let Cursor { track, staff, .. } = self.cursor;
        let middle = match self.score.tracks[track].instrument.clef(staff) {
            Clef::Bass => 48,
            _ => 71,
        };
        self.cursor.string = top.map_or(middle, |n| n.string);
    }

    /// Switches to another voice of the staff, counted from zero.
    pub fn set_voice(&mut self, voice: usize) {
        self.anchor = None;
        self.cursor.voice = voice.min(Bar::MAX_VOICES - 1);
        self.settle();
    }

    fn selection(&self) -> Option<(Position, Position)> {
        let anchor = self.anchor?;
        let cursor = (self.cursor.bar, self.cursor.beat);
        Some((anchor.min(cursor), anchor.max(cursor)))
    }

    /// Existing beats covered by the selection, in score order.
    fn selected_beats(&self) -> Vec<Position> {
        let Some((start, end)) = self.selection() else {
            return Vec::new();
        };
        (start.0..=end.0)
            .flat_map(|bar| (0..self.beats(bar).len()).map(move |beat| (bar, beat)))
            .filter(|&p| p >= start && p <= end)
            .collect()
    }

    // --- Note entry ---

    /// Types one digit of a fret number. With `combine`, a digit typed straight after
    /// another extends it, so 1 then 2 gives fret 12.
    pub fn type_digit(&mut self, digit: u8, combine: bool) -> Result<(), EditError> {
        if self.staff().is_pitched() {
            return Err(EditError::NoFrets);
        }
        let note = (self.current_beat())
            .and_then(|b| b.note_on(self.cursor.string))
            .cloned();
        let existing = note.as_ref().map(|n| n.fret);
        let fret = match existing {
            Some(tens)
                if combine
                    && self.digit_pending
                    && tens > 0
                    && u16::from(tens) * 10 + u16::from(digit) <= u16::from(self.max_fret()) =>
            {
                tens * 10 + digit
            }
            _ => digit,
        };
        // Retyping the fret of an existing note keeps its techniques.
        self.put_note(Note {
            fret,
            tied: false,
            ..note.unwrap_or(Note::new(self.cursor.string, 0))
        })?;
        self.digit_pending = true;
        Ok(())
    }

    /// Puts a note on a given fret under the cursor, keeping the techniques of a note
    /// already there. On a drum track the fret is the number of a piece of the kit.
    pub fn set_fret(&mut self, fret: u8) -> Result<(), EditError> {
        if self.staff().is_pitched() {
            return Err(EditError::NoFrets);
        }
        let note = (self.current_beat())
            .and_then(|b| b.note_on(self.cursor.string))
            .cloned();
        self.put_note(Note {
            fret,
            tied: false,
            ..note.unwrap_or(Note::new(self.cursor.string, 0))
        })
    }

    /// On a staff without strings, enters the note with a letter name (0 for C up to 6 for
    /// B) in the octave nearest the cursor, sharpened or flattened as the key has it, and
    /// moves the cursor to it. Does nothing on a staff with strings.
    pub fn enter_letter(&mut self, letter: u8) -> Result<(), EditError> {
        if !self.staff().is_pitched() {
            return Ok(());
        }
        let key = self.score.master_bars[self.cursor.bar].key_signature;
        let class = i16::from(key_scale(key)[usize::from(letter.min(6))]);
        let cursor = i16::from(self.cursor.string);
        // The nearest note of that name: up to a tritone above, or else below.
        let above = cursor + (class - cursor).rem_euclid(12);
        let pitch = match above - cursor > 6 || above > 127 {
            true if above >= 12 => above - 12,
            _ => above,
        };
        self.cursor.string = pitch.clamp(0, 127) as u8;
        self.enter_note()
    }

    /// On a staff without strings, puts a note at the cursor's pitch if there is none.
    /// Does nothing on a staff with strings, where a note needs a fret.
    pub fn enter_note(&mut self) -> Result<(), EditError> {
        let pitch = self.cursor.string;
        let there = self
            .current_beat()
            .is_some_and(|b| b.note_on(pitch).is_some());
        if !self.staff().is_pitched() || there {
            return Ok(());
        }
        self.put_note(Note::new(pitch, 0))
    }

    /// On a staff without strings, moves the note under the cursor up or down by
    /// `semitones` and the cursor with it, or without a note there just the cursor. A note
    /// is not moved onto another.
    pub fn shift_pitch(&mut self, semitones: i8) -> Result<(), EditError> {
        if !self.staff().is_pitched() {
            return Ok(());
        }
        let from = self.cursor.string;
        let to = (i16::from(from) + i16::from(semitones)).clamp(0, 127) as u8;
        let note = (self.current_beat()).and_then(|b| b.note_on(from)).cloned();
        let taken = self.current_beat().is_some_and(|b| b.note_on(to).is_some());
        if let Some(note) = note.filter(|_| !taken && to != from) {
            let at = self.at();
            let moved = Note { string: to, ..note };
            self.keep_selection(|e| {
                e.run(Command::Batch(vec![
                    Command::RemoveNote { at, string: from },
                    Command::PutNote { at, note: moved },
                ]))
            })?;
        }
        self.cursor.string = to;
        Ok(())
    }

    /// The highest fret the cursor's track has.
    fn max_fret(&self) -> u8 {
        self.score.tracks[self.cursor.track].instrument.max_fret()
    }

    fn put_note(&mut self, note: Note) -> Result<(), EditError> {
        if note.fret > self.max_fret() {
            return Err(EditError::FretOutOfRange);
        }
        let at = self.at();
        let command = if self.current_beat().is_some() {
            Command::PutNote { at, note }
        } else {
            self.in_voice(vec![
                Command::InsertBeat {
                    at,
                    beat: Beat::rest(self.duration),
                },
                Command::PutNote { at, note },
            ])
        };
        self.run(command)
    }

    /// Turns the current beat into a rest, or enters a rest in the empty slot.
    pub fn rest(&mut self) -> Result<(), EditError> {
        let at = self.at();
        let command = match self.current_beat() {
            Some(beat) => Command::Batch(
                beat.notes
                    .iter()
                    .map(|n| Command::RemoveNote {
                        at,
                        string: n.string,
                    })
                    .collect(),
            ),
            None => self.in_voice(vec![Command::InsertBeat {
                at,
                beat: Beat::rest(self.duration),
            }]),
        };
        self.run(command)
    }

    /// Toggles the tie on the note under the cursor. On an empty string it adds a note tied
    /// to the previous note on that string.
    pub fn toggle_tie(&mut self) -> Result<(), EditError> {
        let string = self.cursor.string;
        if let Some(note) = (self.current_beat())
            .and_then(|b| b.note_on(string))
            .cloned()
        {
            return self.run(Command::PutNote {
                at: self.at(),
                note: Note {
                    tied: !note.tied,
                    ..note
                },
            });
        }
        let Cursor { bar, beat, .. } = self.cursor;
        let previous = match beat.checked_sub(1) {
            Some(i) => self.beats(bar).get(i),
            None => bar.checked_sub(1).and_then(|b| self.beats(b).last()),
        };
        match previous.and_then(|b| b.note_on(string)).map(|n| n.fret) {
            Some(fret) => self.put_note(Note {
                tied: true,
                ..Note::new(string, fret)
            }),
            None => Ok(()),
        }
    }

    // --- Techniques ---

    /// The notes a technique or bend applies to: every note of the selected beats, or
    /// without a selection the note under the cursor, or its whole chord if `chord`.
    fn target_notes(&self, chord: bool) -> Vec<(BeatRef, Note)> {
        let mut beats = self.selected_beats();
        let everything = chord || !beats.is_empty();
        if beats.is_empty() && self.current_beat().is_some() {
            beats.push((self.cursor.bar, self.cursor.beat));
        }
        beats
            .into_iter()
            .flat_map(|(bar, beat)| {
                let at = self.beat_ref(bar, beat);
                (self.beats(bar)[beat].notes.iter())
                    .filter(move |n| everything || n.string == self.cursor.string)
                    .map(move |n| (at, n.clone()))
            })
            .collect()
    }

    /// Switches a technique on, or off if everything it applies to already has it.
    pub fn toggle_technique(&mut self, technique: Technique) -> Result<(), EditError> {
        let commands: Vec<Command> = match technique.target() {
            Target::Beat => {
                let mut beats = self.selected_beats();
                if beats.is_empty() && self.current_beat().is_some() {
                    beats.push((self.cursor.bar, self.cursor.beat));
                }
                beats.retain(|&(bar, beat)| !self.beats(bar)[beat].is_rest());
                let on = !(beats.iter())
                    .all(|&(bar, beat)| technique.on_beat(&self.beats(bar)[beat].effects));
                beats
                    .into_iter()
                    .map(|(bar, beat)| {
                        let mut effects = self.beats(bar)[beat].effects.clone();
                        technique.set_on_beat(&mut effects, on);
                        Command::SetBeatEffects {
                            at: self.beat_ref(bar, beat),
                            effects,
                        }
                    })
                    .collect()
            }
            target => {
                let notes = self.target_notes(matches!(target, Target::Chord));
                let on = !notes.iter().all(|(_, n)| technique.on_note(&n.effects));
                notes
                    .into_iter()
                    .map(|(at, mut note)| {
                        technique.set_on_note(&mut note.effects, on);
                        Command::PutNote { at, note }
                    })
                    .collect()
            }
        };
        self.keep_selection(|e| e.run(Command::Batch(commands)))
    }

    /// Sets one field of the effects of the note under the cursor, or of every selected
    /// note. `field` is the field's name in the saved file, such as `"bend"` or `"trill"`,
    /// and `value` its JSON; `null` clears it.
    pub fn set_note_effect(&mut self, field: &str, value: &Value) -> Result<(), EditError> {
        let commands = self
            .target_notes(false)
            .into_iter()
            .map(|(at, mut note)| {
                note.effects = patched(&note.effects, field, value)?;
                Ok(Command::PutNote { at, note })
            })
            .collect::<Result<_, EditError>>()?;
        self.keep_selection(|e| e.run(Command::Batch(commands)))
    }

    /// Sets one field of the effects of the beat under the cursor, or of every selected
    /// beat; see [`Self::set_note_effect`].
    pub fn set_beat_effect(&mut self, field: &str, value: &Value) -> Result<(), EditError> {
        let mut beats = self.selected_beats();
        if beats.is_empty() && self.current_beat().is_some() {
            beats.push((self.cursor.bar, self.cursor.beat));
        }
        let commands = beats
            .into_iter()
            .map(|(bar, beat)| {
                Ok(Command::SetBeatEffects {
                    at: self.beat_ref(bar, beat),
                    effects: patched(&self.beats(bar)[beat].effects, field, value)?,
                })
            })
            .collect::<Result<_, EditError>>()?;
        self.keep_selection(|e| e.run(Command::Batch(commands)))
    }

    // --- Durations ---

    pub fn set_note_value(&mut self, value: NoteValue) -> Result<(), EditError> {
        self.apply_duration(Duration {
            value,
            ..self.duration
        })
    }

    pub fn shorter(&mut self) -> Result<(), EditError> {
        match self.duration.value.shorter() {
            Some(value) => self.set_note_value(value),
            None => Ok(()),
        }
    }

    pub fn longer(&mut self) -> Result<(), EditError> {
        match self.duration.value.longer() {
            Some(value) => self.set_note_value(value),
            None => Ok(()),
        }
    }

    pub fn toggle_dot(&mut self) -> Result<(), EditError> {
        self.apply_duration(Duration {
            dots: if self.duration.dots == 0 { 1 } else { 0 },
            ..self.duration
        })
    }

    pub fn toggle_triplet(&mut self) -> Result<(), EditError> {
        self.apply_duration(Duration {
            tuplet: match self.duration.tuplet {
                Some(_) => None,
                None => Some(Tuplet::TRIPLET),
            },
            nested: None,
            ..self.duration
        })
    }

    /// Sets or clears the tuplet, e.g. 5 notes in the time of 4. Clearing it also clears
    /// any tuplet nested inside.
    pub fn set_tuplet(&mut self, tuplet: Option<Tuplet>) -> Result<(), EditError> {
        self.apply_duration(Duration {
            tuplet,
            nested: self.duration.nested.filter(|_| tuplet.is_some()),
            ..self.duration
        })
    }

    /// Sets or clears a second tuplet inside the first.
    pub fn set_nested_tuplet(&mut self, nested: Option<Tuplet>) -> Result<(), EditError> {
        if nested.is_some() && self.duration.tuplet.is_none() {
            return Err(EditError::InvalidDuration);
        }
        self.apply_duration(Duration {
            nested,
            ..self.duration
        })
    }

    /// Sets the input duration and applies it to the selected beats, or to the beat under
    /// the cursor.
    fn apply_duration(&mut self, duration: Duration) -> Result<(), EditError> {
        self.duration = duration;
        let mut targets = self.selected_beats();
        if targets.is_empty() && self.current_beat().is_some() {
            targets.push((self.cursor.bar, self.cursor.beat));
        }
        if targets.is_empty() {
            return Ok(());
        }
        let commands = targets
            .into_iter()
            .map(|(bar, beat)| Command::SetDuration {
                at: self.beat_ref(bar, beat),
                duration,
            })
            .collect();
        self.keep_selection(|e| e.run(Command::Batch(commands)))
    }

    // --- Beats and bars ---

    /// Deletes the selection; otherwise the note under the cursor, or the beat if it is a rest.
    pub fn delete(&mut self) -> Result<(), EditError> {
        if self.anchor.is_some() {
            return self.delete_beats();
        }
        let at = self.at();
        let string = self.cursor.string;
        match self.current_beat() {
            Some(beat) if beat.note_on(string).is_some() => {
                self.run(Command::RemoveNote { at, string })
            }
            Some(beat) if beat.is_rest() => self.run(Command::RemoveBeat { at }),
            _ => Ok(()),
        }
    }

    /// Removes the selected beats, or the beat under the cursor.
    pub fn delete_beats(&mut self) -> Result<(), EditError> {
        let mut targets = self.selected_beats();
        let start = self.selection().map(|(start, _)| start);
        if self.anchor.is_none() && self.current_beat().is_some() {
            targets.push((self.cursor.bar, self.cursor.beat));
        }
        let commands = targets
            .into_iter()
            .rev()
            .map(|(bar, beat)| Command::RemoveBeat {
                at: self.beat_ref(bar, beat),
            })
            .collect();
        self.run(Command::Batch(commands))?;
        if let Some((bar, beat)) = start {
            self.cursor.bar = bar;
            self.cursor.beat = beat;
            self.settle();
        }
        Ok(())
    }

    /// Inserts a rest before the beat under the cursor.
    pub fn insert_beat(&mut self) -> Result<(), EditError> {
        let command = self.in_voice(vec![Command::InsertBeat {
            at: self.at(),
            beat: Beat::rest(self.duration),
        }]);
        self.run(command)
    }

    /// Inserts an empty bar before the cursor's bar.
    pub fn insert_bar(&mut self) -> Result<(), EditError> {
        let bar = self.cursor.bar;
        let time_signature = self.score.master_bars[bar].time_signature;
        self.run(Command::insert_empty_bar(&self.score, bar, time_signature))?;
        self.cursor.beat = 0;
        self.settle();
        Ok(())
    }

    /// Removes the cursor's bar from every track. The last remaining bar cannot be removed.
    pub fn remove_bar(&mut self) -> Result<(), EditError> {
        if self.score.master_bars.len() == 1 {
            return Err(EditError::LastBar);
        }
        self.run(Command::RemoveBar {
            index: self.cursor.bar,
        })
    }

    /// Changes the time signature from the cursor's bar up to the next existing change.
    pub fn set_time_signature(&mut self, time_signature: TimeSignature) -> Result<(), EditError> {
        let start = self.cursor.bar;
        let old = self.score.master_bars[start].time_signature;
        let commands = self.score.master_bars[start..]
            .iter()
            .take_while(|m| m.time_signature == old)
            .enumerate()
            .map(|(i, _)| Command::SetTimeSignature {
                bar: start + i,
                time_signature,
            })
            .collect();
        self.run(Command::Batch(commands))
    }

    /// Sets one property of the cursor's bar that every track shares. `field` is its name
    /// in the saved file, such as `"repeat_count"` or `"section"`, and `value` its JSON;
    /// `null` clears it.
    pub fn set_bar_property(&mut self, field: &str, value: &Value) -> Result<(), EditError> {
        let bar = self.cursor.bar;
        let master_bar = patched(&self.score.master_bars[bar], field, value)
            .map_err(|_| EditError::InvalidBarProperty)?;
        if master_bar == self.score.master_bars[bar] {
            return Ok(());
        }
        self.keep_selection(|e| e.run(Command::SetMasterBar { bar, master_bar }))
    }

    /// Changes the key signature from the cursor's bar up to the next existing change.
    /// `key` counts sharps, or flats if negative.
    pub fn set_key_signature(&mut self, key: i8) -> Result<(), EditError> {
        let start = self.cursor.bar;
        let old = self.score.master_bars[start].key_signature;
        let commands = self.score.master_bars[start..]
            .iter()
            .take_while(|m| m.key_signature == old)
            .enumerate()
            .map(|(i, _)| Command::SetKeySignature {
                bar: start + i,
                key,
            })
            .collect();
        self.keep_selection(|e| e.run(Command::Batch(commands)))
    }

    /// Where the cursor's beat starts in its bar, in whole notes.
    fn cursor_position(&self) -> Rational {
        let Cursor { bar, beat, .. } = self.cursor;
        (self.beats(bar).iter().take(beat))
            .map(|b| b.duration.length())
            .sum()
    }

    /// The tempo that is written to change on the cursor's beat, if one is.
    fn tempo_change(&self) -> Option<u32> {
        let master = &self.score.master_bars[self.cursor.bar];
        let position = self.cursor_position();
        match *position.numer() {
            0 => master.tempo,
            _ => (master.tempo_changes.iter())
                .find(|c| c.position == position)
                .map(|c| c.tempo),
        }
    }

    /// Changes the tempo from the cursor's beat on, or with `None` takes away the change
    /// written there.
    pub fn set_tempo_change(&mut self, tempo: Option<u32>) -> Result<(), EditError> {
        let bar = self.cursor.bar;
        let position = self.cursor_position();
        let mut master_bar = self.score.master_bars[bar].clone();
        if *position.numer() == 0 {
            master_bar.tempo = tempo;
        } else {
            let changes = &mut master_bar.tempo_changes;
            changes.retain(|c| c.position != position);
            if let Some(tempo) = tempo {
                let at = changes.partition_point(|c| c.position < position);
                changes.insert(at, TempoChange { position, tempo });
            }
        }
        if master_bar == self.score.master_bars[bar] {
            return Ok(());
        }
        self.keep_selection(|e| e.run(Command::SetMasterBar { bar, master_bar }))
    }

    pub fn set_tempo(&mut self, tempo: u32) -> Result<(), EditError> {
        self.run(Command::SetTempo { tempo })
    }

    pub fn set_tuning(&mut self, tuning: Tuning, capo: u8) -> Result<(), EditError> {
        self.run(Command::SetTuning {
            track: self.cursor.track,
            staff: self.cursor.staff,
            tuning,
            capo,
        })
    }

    pub fn set_info(&mut self, info: ScoreInfo) -> Result<(), EditError> {
        self.run(Command::SetInfo { info })
    }

    // --- Tracks ---

    /// Adds an empty track below the cursor's track and moves the cursor to it. Without a
    /// tuning the track starts in its instrument's usual one, with no capo.
    pub fn add_track(
        &mut self,
        name: &str,
        instrument: Instrument,
        tuning: Option<(Tuning, u8)>,
    ) -> Result<(), EditError> {
        let index = self.cursor.track + 1;
        let mut track = Track::empty(name, instrument, self.score.master_bars.len());
        // An instrument has strings to tune or it has not; a tuning cannot change that.
        if (tuning.as_ref()).is_some_and(|(t, _)| t.strings.is_empty() != instrument.is_pitched()) {
            return Err(EditError::InvalidTuning);
        }
        if let Some((tuning, capo)) = tuning {
            track.staves[0].tuning = tuning;
            track.staves[0].capo = capo;
        }
        self.run(Command::InsertTrack { index, track })?;
        self.set_track(index);
        Ok(())
    }

    /// Removes a track. The cursor stays on the track it was on, or takes the next one if
    /// that is the track removed.
    pub fn remove_track(&mut self, index: usize) -> Result<(), EditError> {
        self.exec(Command::RemoveTrack { index })?;
        self.anchor = None;
        if index < self.cursor.track {
            self.cursor.track -= 1;
        }
        self.settle();
        Ok(())
    }

    /// Moves a track to position `to`. The cursor stays on the track it was on.
    pub fn move_track(&mut self, from: usize, to: usize) -> Result<(), EditError> {
        if from == to {
            return Ok(());
        }
        self.exec(Command::MoveTrack { from, to })?;
        let current = self.cursor.track;
        self.cursor.track = if current == from {
            to
        } else if from < current && current <= to {
            current - 1
        } else if to <= current && current < from {
            current + 1
        } else {
            current
        };
        self.settle();
        Ok(())
    }

    /// Renames a track or changes what it is played on. A change to or from an instrument
    /// without strings rewrites the music for the new one: the notes become the pitches
    /// they sound at, or are put on the strings of the instrument's usual tuning, and the
    /// track gets the staves the instrument has. What the strings cannot play, and a
    /// piano's second staff, is lost.
    pub fn set_track_info(
        &mut self,
        track: usize,
        name: &str,
        instrument: Instrument,
    ) -> Result<(), EditError> {
        let name = name.to_owned();
        let old = (self.score.tracks.get(track)).ok_or(EditError::InvalidLocation)?;
        let tunings = instrument.default_tunings();
        let same_kind = old.staves.len() == tunings.len()
            && (old.staves.iter().zip(&tunings))
                .all(|(s, t)| s.is_pitched() == t.strings.is_empty());
        if same_kind {
            return self.keep_selection(|e| {
                e.run(Command::SetTrackInfo {
                    track,
                    name,
                    instrument,
                })
            });
        }
        let bar_count = self.score.master_bars.len();
        // Drums have nothing to say to a pitch, nor a pitch to drums.
        let carries = !old.instrument.is_percussion() && !instrument.is_percussion();
        let staves = (tunings.into_iter().enumerate())
            .map(
                |(index, tuning)| match old.staves.get(index).filter(|_| carries) {
                    Some(staff) if tuning.strings.is_empty() => staff.to_pitched(),
                    Some(staff) => staff.to_pitched().to_strings(tuning, instrument.max_fret()),
                    None => Staff {
                        tuning,
                        capo: 0,
                        bars: vec![Bar::empty(); bar_count],
                    },
                },
            )
            .collect();
        let replacement = Track {
            name,
            instrument,
            mix: old.mix,
            kit: old.kit.clone(),
            staves,
        };
        self.run(Command::ReplaceTrack {
            index: track,
            track: replacement,
        })?;
        self.home();
        Ok(())
    }

    /// Gives a drum track a kit of its own, or with an empty one the standard kit back.
    pub fn set_kit(&mut self, track: usize, kit: Vec<KitPiece>) -> Result<(), EditError> {
        if self.score.tracks.get(track).is_some_and(|t| t.kit == kit) {
            return Ok(());
        }
        self.keep_selection(|e| e.run(Command::SetKit { track, kit }))
    }

    pub fn set_mix(&mut self, track: usize, mix: Mix) -> Result<(), EditError> {
        if self.score.tracks.get(track).is_some_and(|t| t.mix == mix) {
            return Ok(());
        }
        self.keep_selection(|e| e.run(Command::SetMix { track, mix }))
    }

    // --- Clipboard ---

    /// Copies the selection, or the beat under the cursor. A selection of whole bars is
    /// copied as bars, anything else as loose beats.
    pub fn copy(&mut self) {
        let Some((start, end)) = self.selection() else {
            if let Some(beat) = self.current_beat() {
                self.clipboard = Some(Clipboard::Beats(vec![beat.clone()]));
            }
            return;
        };
        let whole_bars = start.1 == 0
            && end.1 + 1 >= self.beats(end.0).len()
            && (start.0 != end.0 || !self.beats(start.0).is_empty());
        self.clipboard = Some(if whole_bars {
            Clipboard::Bars(
                (start.0..=end.0)
                    .map(|bar| {
                        (
                            self.score.master_bars[bar].clone(),
                            self.staff().bars[bar].clone(),
                        )
                    })
                    .collect(),
            )
        } else {
            Clipboard::Beats(
                self.selected_beats()
                    .into_iter()
                    .map(|(bar, beat)| self.beats(bar)[beat].clone())
                    .collect(),
            )
        });
    }

    pub fn cut(&mut self) -> Result<(), EditError> {
        self.copy();
        self.delete_beats()
    }

    /// Pastes beats at the cursor, or bars before the cursor's bar. Notes on strings the
    /// current tuning does not have are dropped.
    pub fn paste(&mut self) -> Result<(), EditError> {
        let (places, pitched) = (self.string_count(), self.staff().is_pitched());
        let fit = |beat: &Beat| Beat {
            duration: beat.duration,
            effects: beat.effects.clone(),
            notes: beat
                .notes
                .iter()
                .filter(|n| usize::from(n.string) < places && (!pitched || n.fret == 0))
                .cloned()
                .map(|mut note| {
                    // An ornament on a string the staff lacks goes with it.
                    (note.effects.grace).take_if(|g| {
                        g.notes
                            .iter()
                            .any(|g| usize::from(g.string) >= places || (pitched && g.fret != 0))
                    });
                    note
                })
                .collect(),
        };
        let Cursor {
            track,
            staff,
            bar,
            beat,
            ..
        } = self.cursor;
        match self.clipboard.clone() {
            None => Ok(()),
            Some(Clipboard::Beats(beats)) => {
                let count = beats.len();
                let commands = beats
                    .iter()
                    .enumerate()
                    .map(|(i, b)| Command::InsertBeat {
                        at: self.beat_ref(bar, beat + i),
                        beat: fit(b),
                    })
                    .collect();
                let command = self.in_voice(commands);
                self.run(command)?;
                self.cursor.beat = beat + count;
                self.settle();
                Ok(())
            }
            Some(Clipboard::Bars(bars)) => {
                let count = bars.len();
                let commands = bars
                    .into_iter()
                    .enumerate()
                    .map(|(i, (master_bar, source))| {
                        let mut pasted = Bar::empty();
                        if !source.voices.is_empty() {
                            pasted.voices = (source.voices.iter())
                                .map(|v| Voice {
                                    beats: v.beats.iter().map(fit).collect(),
                                })
                                .collect();
                        }
                        let mut pasted = Some(pasted);
                        Command::InsertBar {
                            index: bar + i,
                            master_bar,
                            bars: (self.score.tracks.iter().enumerate())
                                .map(|(t, tr)| {
                                    (0..tr.staves.len())
                                        .map(|s| {
                                            match pasted.take_if(|_| t == track && s == staff) {
                                                Some(bar) => bar,
                                                None => Bar::empty(),
                                            }
                                        })
                                        .collect()
                                })
                                .collect(),
                        }
                    })
                    .collect();
                self.run(Command::Batch(commands))?;
                self.cursor.bar = bar + count;
                self.cursor.beat = 0;
                self.settle();
                Ok(())
            }
        }
    }

    // --- History ---

    /// Returns `false` if there was nothing to undo.
    pub fn undo(&mut self) -> Result<bool, EditError> {
        let restored = self.history.undo(&mut self.score, self.cursor)?;
        self.restore(restored)
    }

    /// Returns `false` if there was nothing to redo.
    pub fn redo(&mut self) -> Result<bool, EditError> {
        let restored = self.history.redo(&mut self.score, self.cursor)?;
        self.restore(restored)
    }

    fn restore(&mut self, cursor: Option<Cursor>) -> Result<bool, EditError> {
        self.anchor = None;
        self.revision += 1;
        if let Some(cursor) = cursor {
            self.cursor = cursor;
        }
        self.settle();
        Ok(cursor.is_some())
    }

    // --- Internals ---

    /// Executes an edit, clears the selection and tidies up the cursor.
    fn run(&mut self, command: Command) -> Result<(), EditError> {
        let result = self.exec(command);
        self.anchor = None;
        self.settle();
        result
    }

    /// Runs an edit that should leave the selection in place.
    fn keep_selection(
        &mut self,
        edit: impl FnOnce(&mut Self) -> Result<(), EditError>,
    ) -> Result<(), EditError> {
        let anchor = self.anchor;
        let result = edit(self);
        self.anchor = anchor;
        result
    }

    fn exec(&mut self, command: Command) -> Result<(), EditError> {
        if matches!(&command, Command::Batch(commands) if commands.is_empty()) {
            return Ok(());
        }
        let depth = self.history.depth();
        self.history
            .execute(&mut self.score, command, self.cursor)?;
        self.revision += 1;
        // The saved state was on the redo stack that this edit just discarded.
        if self.saved_depth.is_some_and(|saved| saved > depth) {
            self.saved_depth = None;
        }
        Ok(())
    }

    /// Clamps the cursor to a valid position and picks up the duration of the beat under it.
    fn settle(&mut self) {
        self.digit_pending = false;
        self.cursor.track = self.cursor.track.min(self.score.tracks.len() - 1);
        let staves = self.score.tracks[self.cursor.track].staves.len();
        self.cursor.staff = self.cursor.staff.min(staves - 1);
        self.cursor.bar = self.cursor.bar.min(self.score.master_bars.len() - 1);
        let len = self.beats(self.cursor.bar).len();
        self.cursor.beat = self.cursor.beat.min(len);
        if self.cursor.beat == len && len > 0 && self.bar_full(self.cursor.bar) {
            self.cursor.beat = len - 1;
        }
        self.cursor.string = (self.cursor.string).min(self.string_count() as u8 - 1);
        if let Some(anchor) = &mut self.anchor {
            anchor.0 = anchor.0.min(self.score.master_bars.len() - 1);
        }
        if let Some(beat) = self.current_beat() {
            self.duration = beat.duration;
        }
    }

    fn staff(&self) -> &Staff {
        &self.score.tracks[self.cursor.track].staves[self.cursor.staff]
    }

    /// How many strings the cursor's staff has, or if it has none how many pitches.
    fn string_count(&self) -> usize {
        self.staff().places()
    }

    /// The cursor's voice in a bar, which is empty if the bar does not have that voice yet.
    fn beats(&self, bar: usize) -> &[Beat] {
        (self.staff().bars[bar].voices.get(self.cursor.voice)).map_or(&[], |v| &v.beats)
    }

    fn bar_full(&self, bar: usize) -> bool {
        let length: gtr_model::Rational =
            (self.beats(bar).iter()).map(|b| b.duration.length()).sum();
        length >= self.score.master_bars[bar].time_signature.bar_length()
    }

    /// Wraps edits to the cursor's bar so that they first give the bar the cursor's voice,
    /// if it does not have it yet.
    fn in_voice(&self, mut commands: Vec<Command>) -> Command {
        let Cursor {
            track,
            staff,
            bar,
            voice,
            ..
        } = self.cursor;
        if self.staff().bars[bar].voices.len() <= voice {
            commands.insert(
                0,
                Command::SetVoiceCount {
                    track,
                    staff,
                    bar,
                    count: voice + 1,
                },
            );
        }
        Command::Batch(commands)
    }

    fn current_beat(&self) -> Option<&Beat> {
        self.beats(self.cursor.bar).get(self.cursor.beat)
    }

    fn at(&self) -> BeatRef {
        self.beat_ref(self.cursor.bar, self.cursor.beat)
    }

    fn beat_ref(&self, bar: usize, beat: usize) -> BeatRef {
        BeatRef {
            track: self.cursor.track,
            staff: self.cursor.staff,
            bar,
            voice: self.cursor.voice,
            beat,
        }
    }
}

/// The pitch class of each letter from C to B in a key of `key` sharps, or flats if negative.
fn key_scale(key: i8) -> [u8; 7] {
    let mut scale = [0, 2, 4, 5, 7, 9, 11];
    // Sharps are added in the order F C G D A E B, and flats in the reverse of it.
    const SHARPS: [usize; 7] = [3, 0, 4, 1, 5, 2, 6];
    for &letter in SHARPS.iter().take(key.clamp(0, 7) as usize) {
        scale[letter] = (scale[letter] + 1) % 12;
    }
    for &letter in SHARPS.iter().rev().take((-key).clamp(0, 7) as usize) {
        scale[letter] = (scale[letter] + 11) % 12;
    }
    scale
}

/// The next pitch above or below `pitch` that belongs to the key.
fn step_in_key(pitch: u8, key: i8, up: bool) -> u8 {
    let scale = key_scale(key);
    let in_key = |p: &u8| scale.contains(&(p % 12));
    let next = match up {
        true => (pitch.saturating_add(1)..=127).find(in_key),
        false => (0..pitch).rev().find(in_key),
    };
    next.unwrap_or(pitch)
}

/// Returns a copy of an effects struct with one of its fields replaced from JSON.
fn patched<T: Serialize + DeserializeOwned>(
    effects: &T,
    field: &str,
    value: &Value,
) -> Result<T, EditError> {
    let mut json = serde_json::to_value(effects).map_err(|_| EditError::InvalidEffect)?;
    let fields = json.as_object_mut().ok_or(EditError::InvalidEffect)?;
    if value.is_null() {
        fields.remove(field);
    } else {
        fields.insert(field.to_owned(), value.clone());
    }
    serde_json::from_value(json).map_err(|_| EditError::InvalidEffect)
}
