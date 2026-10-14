use gtr_model::{
    Bar, Beat, BeatEffects, Duration, Instrument, KitPiece, MasterBar, Mix, Note, Pedal, Score,
    ScoreInfo, Sound, Staff, Style, TimeSignature, Track, Tuning, Voice,
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
    SetBeatEffects {
        at: BeatRef,
        effects: BeatEffects,
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
    /// Replaces a bar's shared properties: repeats, endings, section and so on.
    SetMasterBar {
        bar: usize,
        master_bar: MasterBar,
    },
    SetKeySignature {
        bar: usize,
        /// Sharps if positive, flats if negative.
        key: i8,
    },
    SetTempo {
        tempo: u32,
    },
    /// Sets how many voices a bar has. Only empty voices can be taken away.
    SetVoiceCount {
        track: usize,
        staff: usize,
        bar: usize,
        count: usize,
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
    SetStyle {
        style: Style,
    },
    /// Adds a track, which must have a bar for every bar of the score.
    InsertTrack {
        index: usize,
        track: Track,
    },
    /// Puts another track in a track's place, as when its instrument changes to one whose
    /// staves are of another kind.
    ReplaceTrack {
        index: usize,
        track: Track,
    },
    /// Removes a track. The last remaining track cannot be removed.
    RemoveTrack {
        index: usize,
    },
    /// Moves a track to position `to` among the others.
    MoveTrack {
        from: usize,
        to: usize,
    },
    SetTrackInfo {
        track: usize,
        name: String,
        instrument: Instrument,
    },
    SetMix {
        track: usize,
        mix: Mix,
    },
    /// Gives a drum track a kit of its own, or with an empty one the standard kit back.
    SetKit {
        track: usize,
        kit: Vec<KitPiece>,
    },
    /// Sets the pedals a track's sound goes through, or with no track those of the whole mix.
    SetEffects {
        track: Option<usize>,
        effects: Vec<Pedal>,
    },
    /// Gives the score an audio track, changes it, or with `None` takes it away.
    SetAudio {
        audio: Option<gtr_model::AudioTrack>,
    },
    /// Sets the other sounds a track can switch to.
    SetSounds {
        track: usize,
        sounds: Vec<Sound>,
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
    #[error("that is not a valid setting for a bar")]
    InvalidBarProperty,
    #[error("the stylesheet has a setting out of range")]
    InvalidStyle,
    #[error("a key signature has at most seven sharps or flats")]
    InvalidKeySignature,
    #[error(
        "a bar has between 1 and {} voices, and only empty ones can be removed",
        Bar::MAX_VOICES
    )]
    InvalidVoiceCount,
    #[error("tempo must be greater than zero")]
    InvalidTempo,
    #[error("a tuning needs between 1 and {} strings", Tuning::MAX_STRINGS)]
    InvalidTuning,
    #[error("existing notes use strings the new tuning does not have")]
    NotesOnRemovedStrings,
    #[error("that is not a valid setting for this effect")]
    InvalidEffect,
    #[error("fret is out of range")]
    FretOutOfRange,
    #[error("this staff has no frets: enter notes by name, with the letters A to G")]
    NoFrets,
    #[error("a score needs at least one bar")]
    LastBar,
    #[error("a score needs at least one track")]
    LastTrack,
    #[error("the track does not fit the score")]
    InvalidTrack,
    #[error("a drum kit cannot have a number twice, or a piece that far from the staff")]
    InvalidKit,
    #[error("volume or pan is out of range")]
    InvalidMix,
    #[error("too many pedals, or a knob turned past its end")]
    InvalidEffects,
    #[error("a sound needs a name, and a drum track's sounds must be drums and only its")]
    InvalidSound,
    #[error("the audio track's bars must start in the order they are played")]
    InvalidAudio,
}

impl Command {
    /// Where each of the score's `bars` bars will be once the command has been applied,
    /// `None` for one it takes away; or `None` altogether if it changes no bar's place,
    /// repeats or jumps, and so leaves the order the bars are played in as it was.
    pub fn moves_bars(&self, bars: usize) -> Option<Vec<Option<usize>>> {
        fn walk(command: &Command, map: &mut Vec<Option<usize>>) -> bool {
            match command {
                Command::InsertBar { index, .. } => {
                    for bar in map.iter_mut().flatten() {
                        *bar += usize::from(*bar >= *index);
                    }
                    true
                }
                Command::RemoveBar { index } => {
                    for slot in map.iter_mut() {
                        *slot = slot.and_then(|bar| match bar.cmp(index) {
                            std::cmp::Ordering::Less => Some(bar),
                            std::cmp::Ordering::Equal => None,
                            std::cmp::Ordering::Greater => Some(bar - 1),
                        });
                    }
                    true
                }
                Command::SetMasterBar { .. } => true,
                Command::Batch(commands) => {
                    (commands.iter()).fold(false, |moved, command| walk(command, map) | moved)
                }
                _ => false,
            }
        }
        let mut map = (0..bars).map(Some).collect();
        walk(self, &mut map).then_some(map)
    }

    /// An [`Command::InsertBar`] that adds an empty bar to every staff.
    pub fn insert_empty_bar(score: &Score, index: usize, time_signature: TimeSignature) -> Self {
        Command::InsertBar {
            index,
            master_bar: MasterBar::new(time_signature),
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
                let staff = staff(score, &at)?;
                if usize::from(note.string) >= staff.places() {
                    return Err(EditError::StringOutOfRange);
                }
                if staff.is_pitched() && note.fret != 0 {
                    return Err(EditError::NoFrets);
                }
                if !note.effects.is_valid() {
                    return Err(EditError::InvalidEffect);
                }
                let notes = &mut beat_mut(score, &at)?.notes;
                Ok(
                    match notes.binary_search_by_key(&note.string, |n| n.string) {
                        Ok(i) => Command::PutNote {
                            at,
                            note: std::mem::replace(&mut notes[i], note),
                        },
                        Err(i) => {
                            let string = note.string;
                            notes.insert(i, note);
                            Command::RemoveNote { at, string }
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
                if !staff(score, &at)?.holds(&beat) {
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
            Command::SetBeatEffects { at, effects } => {
                if !effects.is_valid() {
                    return Err(EditError::InvalidEffect);
                }
                let beat = beat_mut(score, &at)?;
                Ok(Command::SetBeatEffects {
                    at,
                    effects: std::mem::replace(&mut beat.effects, effects),
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
                if !master_bar.is_valid() {
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
            Command::SetMasterBar { bar, master_bar } => {
                if !master_bar.is_valid() {
                    return Err(EditError::InvalidBarProperty);
                }
                let master = (score.master_bars.get_mut(bar)).ok_or(EditError::InvalidLocation)?;
                Ok(Command::SetMasterBar {
                    bar,
                    master_bar: std::mem::replace(master, master_bar),
                })
            }
            Command::SetKeySignature { bar, key } => {
                if key.abs() > MasterBar::MAX_KEY {
                    return Err(EditError::InvalidKeySignature);
                }
                let master = (score.master_bars.get_mut(bar)).ok_or(EditError::InvalidLocation)?;
                Ok(Command::SetKeySignature {
                    bar,
                    key: std::mem::replace(&mut master.key_signature, key),
                })
            }
            Command::SetVoiceCount {
                track,
                staff,
                bar,
                count,
            } => {
                let voices = score
                    .tracks
                    .get_mut(track)
                    .and_then(|t| t.staves.get_mut(staff))
                    .and_then(|s| s.bars.get_mut(bar))
                    .map(|b| &mut b.voices)
                    .ok_or(EditError::InvalidLocation)?;
                let old = voices.len();
                let removable = voices.iter().skip(count).all(|v| v.beats.is_empty());
                if count == 0 || count > Bar::MAX_VOICES || !removable {
                    return Err(EditError::InvalidVoiceCount);
                }
                voices.resize_with(count, Voice::default);
                Ok(Command::SetVoiceCount {
                    track,
                    staff,
                    bar,
                    count: old,
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
                // A staff without strings is not given any this way: its notes are
                // pitches, which have to be put on the strings (see `ReplaceTrack`).
                if target.is_pitched() {
                    return Err(EditError::InvalidTuning);
                }
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
            Command::SetStyle { style } => {
                if !style.is_valid() {
                    return Err(EditError::InvalidStyle);
                }
                Ok(Command::SetStyle {
                    style: std::mem::replace(&mut score.style, style),
                })
            }
            Command::InsertTrack { index, track } => {
                if index > score.tracks.len() {
                    return Err(EditError::InvalidLocation);
                }
                if !track_fits(score, &track) {
                    return Err(EditError::InvalidTrack);
                }
                score.tracks.insert(index, track);
                Ok(Command::RemoveTrack { index })
            }
            Command::ReplaceTrack { index, track } => {
                if !track_fits(score, &track) {
                    return Err(EditError::InvalidTrack);
                }
                let target = (score.tracks.get_mut(index)).ok_or(EditError::InvalidLocation)?;
                Ok(Command::ReplaceTrack {
                    index,
                    track: std::mem::replace(target, track),
                })
            }
            Command::RemoveTrack { index } => {
                if index >= score.tracks.len() {
                    return Err(EditError::InvalidLocation);
                }
                if score.tracks.len() == 1 {
                    return Err(EditError::LastTrack);
                }
                Ok(Command::InsertTrack {
                    index,
                    track: score.tracks.remove(index),
                })
            }
            Command::MoveTrack { from, to } => {
                if from >= score.tracks.len() || to >= score.tracks.len() {
                    return Err(EditError::InvalidLocation);
                }
                let track = score.tracks.remove(from);
                score.tracks.insert(to, track);
                Ok(Command::MoveTrack { from: to, to: from })
            }
            Command::SetTrackInfo {
                track,
                name,
                instrument,
            } => {
                let target = (score.tracks.get_mut(track)).ok_or(EditError::InvalidLocation)?;
                if !Sound::list_is_valid(&target.sounds, instrument) {
                    return Err(EditError::InvalidSound);
                }
                Ok(Command::SetTrackInfo {
                    track,
                    name: std::mem::replace(&mut target.name, name),
                    instrument: std::mem::replace(&mut target.instrument, instrument),
                })
            }
            Command::SetMix { track, mix } => {
                if !mix.is_valid() {
                    return Err(EditError::InvalidMix);
                }
                let target = (score.tracks.get_mut(track)).ok_or(EditError::InvalidLocation)?;
                Ok(Command::SetMix {
                    track,
                    mix: std::mem::replace(&mut target.mix, mix),
                })
            }
            Command::SetKit { track, kit } => {
                if !gtr_model::kit_is_valid(&kit) {
                    return Err(EditError::InvalidKit);
                }
                let target = (score.tracks.get_mut(track)).ok_or(EditError::InvalidLocation)?;
                Ok(Command::SetKit {
                    track,
                    kit: std::mem::replace(&mut target.kit, kit),
                })
            }
            Command::SetEffects { track, effects } => {
                if !Pedal::chain_is_valid(&effects) {
                    return Err(EditError::InvalidEffects);
                }
                let target = match track {
                    Some(track) => {
                        &mut (score.tracks.get_mut(track))
                            .ok_or(EditError::InvalidLocation)?
                            .effects
                    }
                    None => &mut score.effects,
                };
                Ok(Command::SetEffects {
                    track,
                    effects: std::mem::replace(target, effects),
                })
            }
            Command::SetAudio { audio } => {
                if audio.as_ref().is_some_and(|a| !a.is_valid()) {
                    return Err(EditError::InvalidAudio);
                }
                Ok(Command::SetAudio {
                    audio: std::mem::replace(&mut score.audio, audio),
                })
            }
            Command::SetSounds { track, sounds } => {
                let target = (score.tracks.get_mut(track)).ok_or(EditError::InvalidLocation)?;
                if !Sound::list_is_valid(&sounds, target.instrument) {
                    return Err(EditError::InvalidSound);
                }
                Ok(Command::SetSounds {
                    track,
                    sounds: std::mem::replace(&mut target.sounds, sounds),
                })
            }
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

/// Whether a track can be part of the score: it has staves, each with a bar for every bar
/// of the score and nothing in them that its strings, or its lack of them, cannot hold.
fn track_fits(score: &Score, track: &Track) -> bool {
    let bar_count = score.master_bars.len();
    !track.staves.is_empty()
        && track.mix.is_valid()
        && gtr_model::kit_is_valid(&track.kit)
        && track.staves.iter().all(|s| {
            s.tuning.strings.len() <= Tuning::MAX_STRINGS
                && s.bars.len() == bar_count
                && s.bars.iter().all(|b| {
                    !b.voices.is_empty()
                        && b.voices.len() <= Bar::MAX_VOICES
                        && (b.voices.iter().flat_map(|v| &v.beats)).all(|beat| s.holds(beat))
                })
        })
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
            let valid = !bar.voices.is_empty()
                && bar.voices.len() <= Bar::MAX_VOICES
                && (bar.voices.iter().flat_map(|v| &v.beats)).all(|b| staff.holds(b));
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
