use gtr_model::{
    Bar, Beat, BeatEffects, Duration, MasterBar, Note, NoteValue, Score, ScoreInfo, Staff,
    TimeSignature, Track, Tuning, Tuplet,
};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::technique::Target;
use crate::{BeatRef, Command, EditError, History, Technique};

pub const MAX_FRET: u8 = 36;

/// Where typing goes. `beat` may equal the number of beats in the bar: that is the empty slot
/// after the last beat, available while the bar is not yet full.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Cursor {
    pub track: usize,
    pub bar: usize,
    pub beat: usize,
    pub string: u8,
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
#[derive(Debug, Clone, Serialize)]
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
    pub can_undo: bool,
    pub can_redo: bool,
    pub can_paste: bool,
    pub dirty: bool,
    /// Changes whenever the score does.
    pub revision: u64,
}

/// A score plus everything needed to edit it from the keyboard: cursor, selection, input
/// duration, clipboard and undo history. Edits go to the first staff and voice of the
/// cursor's track.
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
            score.tracks.push(Track {
                name: "Guitar".into(),
                staves: Vec::new(),
            });
        }
        if score.master_bars.is_empty() {
            score.master_bars.push(MasterBar {
                time_signature: TimeSignature::COMMON,
            });
        }
        let bar_count = score.master_bars.len();
        for track in &mut score.tracks {
            if track.staves.is_empty() {
                track.staves.push(Staff {
                    tuning: Tuning::guitar_standard(),
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
            Move::Up => self.cursor.string = self.cursor.string.saturating_sub(1),
            Move::Down => self.cursor.string += 1,
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

    pub fn set_track(&mut self, track: usize) {
        self.anchor = None;
        self.cursor.track = track;
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
        let note = (self.current_beat())
            .and_then(|b| b.note_on(self.cursor.string))
            .cloned();
        let existing = note.as_ref().map(|n| n.fret);
        let fret = match existing {
            Some(tens)
                if combine
                    && self.digit_pending
                    && tens > 0
                    && u16::from(tens) * 10 + u16::from(digit) <= u16::from(MAX_FRET) =>
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

    fn put_note(&mut self, note: Note) -> Result<(), EditError> {
        if note.fret > MAX_FRET {
            return Err(EditError::FretOutOfRange);
        }
        let at = self.at();
        let command = if self.current_beat().is_some() {
            Command::PutNote { at, note }
        } else {
            Command::Batch(vec![
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
            None => Command::InsertBeat {
                at,
                beat: Beat::rest(self.duration),
            },
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
        self.run(Command::InsertBeat {
            at: self.at(),
            beat: Beat::rest(self.duration),
        })
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

    pub fn set_tempo(&mut self, tempo: u32) -> Result<(), EditError> {
        self.run(Command::SetTempo { tempo })
    }

    pub fn set_tuning(&mut self, tuning: Tuning, capo: u8) -> Result<(), EditError> {
        self.run(Command::SetTuning {
            track: self.cursor.track,
            staff: 0,
            tuning,
            capo,
        })
    }

    pub fn set_info(&mut self, info: ScoreInfo) -> Result<(), EditError> {
        self.run(Command::SetInfo { info })
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
        let string_count = self.string_count();
        let fit = |beat: &Beat| Beat {
            duration: beat.duration,
            effects: beat.effects.clone(),
            notes: beat
                .notes
                .iter()
                .filter(|n| usize::from(n.string) < string_count)
                .cloned()
                .collect(),
        };
        let Cursor {
            track, bar, beat, ..
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
                self.run(Command::Batch(commands))?;
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
                        pasted.voices[0].beats = source
                            .voices
                            .first()
                            .map(|v| v.beats.iter().map(fit).collect())
                            .unwrap_or_default();
                        let mut pasted = Some(pasted);
                        Command::InsertBar {
                            index: bar + i,
                            master_bar,
                            bars: (self.score.tracks.iter().enumerate())
                                .map(|(t, tr)| {
                                    (0..tr.staves.len())
                                        .map(|s| match pasted.take_if(|_| t == track && s == 0) {
                                            Some(bar) => bar,
                                            None => Bar::empty(),
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
        &self.score.tracks[self.cursor.track].staves[0]
    }

    fn string_count(&self) -> usize {
        self.staff().tuning.strings.len()
    }

    fn beats(&self, bar: usize) -> &[Beat] {
        &self.staff().bars[bar].voices[0].beats
    }

    fn bar_full(&self, bar: usize) -> bool {
        self.staff().bars[bar].voices[0].length()
            >= self.score.master_bars[bar].time_signature.bar_length()
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
            staff: 0,
            bar,
            voice: 0,
            beat,
        }
    }
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
