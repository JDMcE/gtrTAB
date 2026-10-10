use gtr_model::{
    Bar, Beat, Duration, MasterBar, Note, Score, ScoreInfo, Staff, TimeSignature, Tuning, Voice,
};
use serde::{Deserialize, Serialize};

/// Address of a beat. For insertions, `beat` may equal the voice length.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BeatRef {
    pub track: usize,
    pub staff: usize,
    pub bar: usize,
    pub voice: usize,
    pub beat: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Command {
    /// Adds a note to a beat, replacing any note already on that string.
    PutNote {
        at: BeatRef,
        note: Note,
    },
    RemoveNote {
        at: BeatRef,
        string: u8,
    },
    InsertBeat {
        at: BeatRef,
        beat: Beat,
    },
    RemoveBeat {
        at: BeatRef,
    },
    SetDuration {
        at: BeatRef,
        duration: Duration,
    },
    /// Inserts a bar in every staff. `bars` holds one bar per staff, grouped by track.
    InsertBar {
        index: usize,
        master_bar: MasterBar,
        bars: Vec<Vec<Bar>>,
    },
    RemoveBar {
        index: usize,
    },
    SetTimeSignature {
        bar: usize,
        time_signature: TimeSignature,
    },
    SetTempo {
        tempo: u32,
    },
    /// Changes a staff's tuning and capo. Fails if existing notes use a string that would
    /// no longer exist.
    SetTuning {
        track: usize,
        staff: usize,
        tuning: Tuning,
        capo: u8,
    },
    SetInfo {
        info: ScoreInfo,
    },
    /// Applies several commands as one undo step. If one fails, none are applied.
    Batch(Vec<Command>),
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EditError {
    #[error("location does not exist in the score")]
    InvalidLocation,
    #[error("string does not exist on this staff")]
    StringOutOfRange,
    #[error("there is no note on that string")]
    NoNoteOnString,
    #[error("beat is not valid for this staff")]
    InvalidBeat,
    #[error("duration is not valid")]
    InvalidDuration,
    #[error("inserted bars do not match the score's tracks and staves")]
    BarShapeMismatch,
    #[error("time signature is not valid")]
    InvalidTimeSignature,
    #[error("tempo must be greater than zero")]
    InvalidTempo,
    #[error("a tuning needs between 1 and {} strings", Tuning::MAX_STRINGS)]
    InvalidTuning,
    #[error("existing notes use strings the new tuning does not have")]
    NotesOnRemovedStrings,
    #[error("fret is out of range")]
    FretOutOfRange,
    #[error("a score needs at least one bar")]
    LastBar,
}

impl Command {
    /// An [`Command::InsertBar`] that adds an empty bar to every staff.
    pub fn insert_empty_bar(score: &Score, index: usize, time_signature: TimeSignature) -> Self {
        Command::InsertBar {
            index,
            master_bar: MasterBar { time_signature },
            bars: score
                .tracks
                .iter()
                .map(|t| vec![Bar::empty(); t.staves.len()])
                .collect(),
        }
    }

    /// Applies the command and returns the command that reverts it. On error the score is
    /// left unchanged.
    pub fn apply(self, score: &mut Score) -> Result<Command, EditError> {
        match self {
            Command::PutNote { at, note } => {
                if usize::from(note.string) >= staff(score, &at)?.tuning.strings.len() {
                    return Err(EditError::StringOutOfRange);
                }
                let notes = &mut beat_mut(score, &at)?.notes;
                Ok(
                    match notes.binary_search_by_key(&note.string, |n| n.string) {
                        Ok(i) => Command::PutNote {
                            at,
                            note: std::mem::replace(&mut notes[i], note),
                        },
                        Err(i) => {
                            notes.insert(i, note);
                            Command::RemoveNote {
                                at,
                                string: note.string,
                            }
                        }
                    },
                )
            }
            Command::RemoveNote { at, string } => {
                let notes = &mut beat_mut(score, &at)?.notes;
                let i = notes
                    .binary_search_by_key(&string, |n| n.string)
                    .map_err(|_| EditError::NoNoteOnString)?;
                Ok(Command::PutNote {
                    at,
                    note: notes.remove(i),
                })
            }
            Command::InsertBeat { at, beat } => {
                if !beat.is_valid_for(staff(score, &at)?.tuning.strings.len()) {
                    return Err(EditError::InvalidBeat);
                }
                let beats = &mut voice_mut(score, &at)?.beats;
                if at.beat > beats.len() {
                    return Err(EditError::InvalidLocation);
                }
                beats.insert(at.beat, beat);
                Ok(Command::RemoveBeat { at })
            }
            Command::RemoveBeat { at } => {
                let beats = &mut voice_mut(score, &at)?.beats;
                if at.beat >= beats.len() {
                    return Err(EditError::InvalidLocation);
                }
                Ok(Command::InsertBeat {
                    at,
                    beat: beats.remove(at.beat),
                })
            }
            Command::SetDuration { at, duration } => {
                if !duration.is_valid() {
                    return Err(EditError::InvalidDuration);
                }
                let beat = beat_mut(score, &at)?;
                Ok(Command::SetDuration {
                    at,
                    duration: std::mem::replace(&mut beat.duration, duration),
                })
            }
            Command::InsertBar {
                index,
                master_bar,
                bars,
            } => {
                if index > score.master_bars.len() {
                    return Err(EditError::InvalidLocation);
                }
                if !master_bar.time_signature.is_valid() {
                    return Err(EditError::InvalidTimeSignature);
                }
                check_bars(score, &bars)?;
                score.master_bars.insert(index, master_bar);
                for (track, track_bars) in score.tracks.iter_mut().zip(bars) {
                    for (staff, bar) in track.staves.iter_mut().zip(track_bars) {
                        staff.bars.insert(index, bar);
                    }
                }
                Ok(Command::RemoveBar { index })
            }
            Command::RemoveBar { index } => {
                if index >= score.master_bars.len() {
                    return Err(EditError::InvalidLocation);
                }
                let master_bar = score.master_bars.remove(index);
                let bars = score
                    .tracks
                    .iter_mut()
                    .map(|t| t.staves.iter_mut().map(|s| s.bars.remove(index)).collect())
                    .collect();
                Ok(Command::InsertBar {
                    index,
                    master_bar,
                    bars,
                })
            }
            Command::SetTimeSignature {
                bar,
                time_signature,
            } => {
                if !time_signature.is_valid() {
                    return Err(EditError::InvalidTimeSignature);
                }
                let master = score
                    .master_bars
                    .get_mut(bar)
                    .ok_or(EditError::InvalidLocation)?;
                Ok(Command::SetTimeSignature {
                    bar,
                    time_signature: std::mem::replace(&mut master.time_signature, time_signature),
                })
            }
            Command::SetTempo { tempo } => {
                if tempo == 0 {
                    return Err(EditError::InvalidTempo);
                }
                Ok(Command::SetTempo {
                    tempo: std::mem::replace(&mut score.tempo, tempo),
                })
            }
            Command::SetTuning {
                track,
                staff,
                tuning,
                capo,
            } => {
                let string_count = tuning.strings.len();
                if string_count == 0 || string_count > Tuning::MAX_STRINGS {
                    return Err(EditError::InvalidTuning);
                }
                let target = score
                    .tracks
                    .get_mut(track)
                    .and_then(|t| t.staves.get_mut(staff))
                    .ok_or(EditError::InvalidLocation)?;
                let fits = target
                    .bars
                    .iter()
                    .flat_map(|b| &b.voices)
                    .flat_map(|v| &v.beats)
                    .all(|b| b.is_valid_for(string_count));
                if !fits {
                    return Err(EditError::NotesOnRemovedStrings);
                }
                Ok(Command::SetTuning {
                    track,
                    staff,
                    tuning: std::mem::replace(&mut target.tuning, tuning),
                    capo: std::mem::replace(&mut target.capo, capo),
                })
            }
            Command::SetInfo { info } => Ok(Command::SetInfo {
                info: std::mem::replace(&mut score.info, info),
            }),
            Command::Batch(commands) => {
                let mut inverses = Vec::with_capacity(commands.len());
                for command in commands {
                    match command.apply(score) {
                        Ok(inverse) => inverses.push(inverse),
                        Err(e) => {
                            for inverse in inverses.into_iter().rev() {
                                inverse
                                    .apply(score)
                                    .expect("an inverse command always applies");
                            }
                            return Err(e);
                        }
                    }
                }
                inverses.reverse();
                Ok(Command::Batch(inverses))
            }
        }
    }
}

fn check_bars(score: &Score, bars: &[Vec<Bar>]) -> Result<(), EditError> {
    if bars.len() != score.tracks.len() {
        return Err(EditError::BarShapeMismatch);
    }
    for (track, track_bars) in score.tracks.iter().zip(bars) {
        if track_bars.len() != track.staves.len() {
            return Err(EditError::BarShapeMismatch);
        }
        for (staff, bar) in track.staves.iter().zip(track_bars) {
            let string_count = staff.tuning.strings.len();
            let valid = bar
                .voices
                .iter()
                .flat_map(|v| &v.beats)
                .all(|b| b.is_valid_for(string_count));
            if !valid {
                return Err(EditError::InvalidBeat);
            }
        }
    }
    Ok(())
}

fn staff<'a>(score: &'a Score, at: &BeatRef) -> Result<&'a Staff, EditError> {
    score
        .tracks
        .get(at.track)
        .and_then(|t| t.staves.get(at.staff))
        .ok_or(EditError::InvalidLocation)
}

fn voice_mut<'a>(score: &'a mut Score, at: &BeatRef) -> Result<&'a mut Voice, EditError> {
    score
        .tracks
        .get_mut(at.track)
        .and_then(|t| t.staves.get_mut(at.staff))
        .and_then(|s| s.bars.get_mut(at.bar))
        .and_then(|b| b.voices.get_mut(at.voice))
        .ok_or(EditError::InvalidLocation)
}

fn beat_mut<'a>(score: &'a mut Score, at: &BeatRef) -> Result<&'a mut Beat, EditError> {
    voice_mut(score, at)?
        .beats
        .get_mut(at.beat)
        .ok_or(EditError::InvalidLocation)
}
