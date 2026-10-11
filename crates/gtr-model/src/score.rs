use serde::{Deserialize, Serialize};

use crate::{BeatEffects, Duration, Instrument, Mix, NoteEffects, Rational, Tuning};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Score {
    #[serde(default)]
    pub info: ScoreInfo,
    /// Quarter notes per minute.
    pub tempo: u32,
    /// Bar properties shared by every track. Each staff has exactly one bar per master bar.
    pub master_bars: Vec<MasterBar>,
    pub tracks: Vec<Track>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ScoreInfo {
    pub title: String,
    pub subtitle: String,
    pub artist: String,
    pub album: String,
    /// Who wrote the words.
    pub words: String,
    /// Who wrote the music.
    pub music: String,
    /// Who wrote the score down.
    pub transcriber: String,
    pub copyright: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MasterBar {
    pub time_signature: TimeSignature,
    /// Sharps in the key signature if positive, flats if negative.
    #[serde(default)]
    pub key_signature: i8,
    /// A repeated passage starts with this bar.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub repeat_start: bool,
    /// A repeated passage ends with this bar and is played this many times in all; zero
    /// if none ends here.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub repeat_count: u8,
    /// The passes of a repeat on which this bar is played, as a bit per pass starting with
    /// the first; zero for a bar played every time.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub alternate_endings: u8,
    /// A section name or rehearsal mark, such as `Chorus` or `A`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub section: Option<String>,
    /// A place that a jump leads to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<Target>,
    /// An instruction, at the end of this bar, to carry on from somewhere else.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub jump: Option<Jump>,
    /// A new tempo from this bar on, in quarter notes per minute.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tempo: Option<u32>,
    /// Changes of tempo part-way through the bar, in the order they come.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tempo_changes: Vec<TempoChange>,
    /// Whether pairs of short notes are played unevenly from this bar on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub feel: Option<Feel>,
    /// The time elapsed when the piece reaches this bar is written above it.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub timer: bool,
}

/// A new tempo from a beat inside a bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TempoChange {
    /// Where in the bar it happens, in whole notes from the bar's start.
    pub position: Rational,
    /// Quarter notes per minute.
    pub tempo: u32,
}

fn is_zero(value: &u8) -> bool {
    *value == 0
}

/// A marked place in the score.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Target {
    Segno,
    Coda,
    /// The end of the piece, once a jump "al fine" has been taken.
    Fine,
}

/// Where to carry on from at the end of a bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Jump {
    /// Back to the beginning.
    DaCapo,
    DaCapoAlFine,
    DaCapoAlCoda,
    /// Back to the segno.
    DalSegno,
    DalSegnoAlFine,
    DalSegnoAlCoda,
    /// On to the coda, once a jump "al coda" has been taken.
    ToCoda,
}

impl Jump {
    pub fn label(self) -> &'static str {
        match self {
            Jump::DaCapo => "D.C.",
            Jump::DaCapoAlFine => "D.C. al Fine",
            Jump::DaCapoAlCoda => "D.C. al Coda",
            Jump::DalSegno => "D.S.",
            Jump::DalSegnoAlFine => "D.S. al Fine",
            Jump::DalSegnoAlCoda => "D.S. al Coda",
            Jump::ToCoda => "To Coda",
        }
    }
}

/// How evenly pairs of short notes are played.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Feel {
    Straight,
    /// Pairs of eighth notes are played long-short, as two thirds and one third.
    SwingEighths,
    SwingSixteenths,
}

impl MasterBar {
    pub const MAX_KEY: i8 = 7;

    /// A bar in the given time, in a key without sharps or flats.
    pub fn new(time_signature: TimeSignature) -> Self {
        MasterBar {
            time_signature,
            key_signature: 0,
            repeat_start: false,
            repeat_count: 0,
            alternate_endings: 0,
            section: None,
            target: None,
            jump: None,
            tempo: None,
            tempo_changes: Vec::new(),
            feel: None,
            timer: false,
        }
    }

    pub fn is_valid(&self) -> bool {
        self.time_signature.is_valid()
            && self.key_signature.abs() <= MasterBar::MAX_KEY
            && self.repeat_count != 1
            && self.tempo != Some(0)
            && (self.tempo_changes.iter()).all(|c| {
                c.tempo != 0
                    && c.position > Rational::from_integer(0)
                    && c.position < self.time_signature.bar_length()
            })
            && (self.tempo_changes.windows(2)).all(|w| w[0].position < w[1].position)
            && (self.section.as_ref()).is_none_or(|s| s.chars().count() <= 120)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TimeSignature {
    pub numerator: u8,
    pub denominator: u8,
}

impl TimeSignature {
    pub const COMMON: TimeSignature = TimeSignature {
        numerator: 4,
        denominator: 4,
    };

    pub fn is_valid(&self) -> bool {
        self.numerator > 0 && self.denominator.is_power_of_two() && self.denominator <= 64
    }

    /// Length of a full bar in whole notes. Panics if the denominator is zero.
    pub fn bar_length(&self) -> Rational {
        Rational::new(self.numerator.into(), self.denominator.into())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Track {
    pub name: String,
    #[serde(default)]
    pub instrument: Instrument,
    #[serde(default)]
    pub mix: Mix,
    pub staves: Vec<Staff>,
}

impl Track {
    /// A track with default mixer settings.
    pub fn new(name: impl Into<String>, instrument: Instrument, staves: Vec<Staff>) -> Self {
        Track {
            name: name.into(),
            instrument,
            mix: Mix::default(),
            staves,
        }
    }

    /// A track for `instrument` with the staves it usually has, in their usual tunings,
    /// and `bar_count` empty bars.
    pub fn empty(name: impl Into<String>, instrument: Instrument, bar_count: usize) -> Self {
        let staves = (instrument.default_tunings().into_iter())
            .map(|tuning| Staff {
                tuning,
                capo: 0,
                bars: vec![Bar::empty(); bar_count],
            })
            .collect();
        Track::new(name, instrument, staves)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Staff {
    pub tuning: Tuning,
    #[serde(default)]
    pub capo: u8,
    pub bars: Vec<Bar>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Bar {
    pub voices: Vec<Voice>,
}

impl Bar {
    /// The most voices a bar can hold.
    pub const MAX_VOICES: usize = 4;

    /// A bar with a single voice and no beats.
    pub fn empty() -> Self {
        Bar {
            voices: vec![Voice::default()],
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Voice {
    pub beats: Vec<Beat>,
}

impl Voice {
    /// Total length of the beats in whole notes.
    pub fn length(&self) -> Rational {
        self.beats.iter().map(|b| b.duration.length()).sum()
    }
}

/// A rhythmic position. A beat without notes is a rest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Beat {
    pub duration: Duration,
    /// Sorted by string, at most one note per string.
    #[serde(default)]
    pub notes: Vec<Note>,
    #[serde(default, skip_serializing_if = "BeatEffects::is_none")]
    pub effects: BeatEffects,
}

impl Beat {
    pub fn new(duration: Duration, notes: Vec<Note>) -> Self {
        Beat {
            duration,
            notes,
            effects: BeatEffects::default(),
        }
    }

    pub fn rest(duration: Duration) -> Self {
        Beat::new(duration, Vec::new())
    }

    pub fn is_rest(&self) -> bool {
        self.notes.is_empty()
    }

    /// The note on `string`, if any.
    pub fn note_on(&self, string: u8) -> Option<&Note> {
        self.notes.iter().find(|n| n.string == string)
    }

    /// Checks the beat against a staff with `string_count` strings.
    pub fn is_valid_for(&self, string_count: usize) -> bool {
        self.duration.is_valid()
            && self.effects.is_valid()
            && self.notes.iter().all(|n| n.effects.is_valid())
            && (self.notes.iter())
                .flat_map(|n| n.effects.grace.iter().flat_map(|g| &g.notes))
                .all(|g| usize::from(g.string) < string_count)
            && self
                .notes
                .iter()
                .all(|n| usize::from(n.string) < string_count)
            && self.notes.windows(2).all(|w| w[0].string < w[1].string)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Note {
    /// Index into the staff tuning; 0 is the highest string.
    pub string: u8,
    pub fret: u8,
    #[serde(default)]
    pub tied: bool,
    #[serde(default, skip_serializing_if = "NoteEffects::is_none")]
    pub effects: NoteEffects,
}

impl Note {
    pub fn new(string: u8, fret: u8) -> Self {
        Note {
            string,
            fret,
            tied: false,
            effects: NoteEffects::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ModelError {
    #[error("tempo must be greater than zero")]
    InvalidTempo,
    #[error("bar {bar} has an invalid time or key signature")]
    InvalidTimeSignature { bar: usize },
    #[error("track {track} has no staff, or mixer settings that are out of range")]
    InvalidTrack { track: usize },
    #[error("track {track} staff {staff} has no strings")]
    EmptyTuning { track: usize, staff: usize },
    #[error("track {track} staff {staff} has {found} bars, expected {expected}")]
    BarCountMismatch {
        track: usize,
        staff: usize,
        expected: usize,
        found: usize,
    },
    #[error("track {track} staff {staff} bar {bar} has no voice or contains an invalid beat")]
    InvalidBeat {
        track: usize,
        staff: usize,
        bar: usize,
    },
}

impl Score {
    /// A blank score: one six-string guitar track with a single empty 4/4 bar.
    pub fn blank() -> Self {
        Score {
            info: ScoreInfo::default(),
            tempo: 120,
            master_bars: vec![MasterBar::new(TimeSignature::COMMON)],
            tracks: vec![Track::empty("Guitar", Instrument::default(), 1)],
        }
    }

    /// Checks the structural invariants the editor relies on. It does not check that bars are
    /// rhythmically complete; incomplete bars are legal while editing.
    pub fn validate(&self) -> Result<(), ModelError> {
        if self.tempo == 0 {
            return Err(ModelError::InvalidTempo);
        }
        if let Some(bar) = self.master_bars.iter().position(|m| !m.is_valid()) {
            return Err(ModelError::InvalidTimeSignature { bar });
        }
        for (track, t) in self.tracks.iter().enumerate() {
            if t.staves.is_empty() || !t.mix.is_valid() {
                return Err(ModelError::InvalidTrack { track });
            }
            for (staff, s) in t.staves.iter().enumerate() {
                let string_count = s.tuning.strings.len();
                if string_count == 0 {
                    return Err(ModelError::EmptyTuning { track, staff });
                }
                if s.bars.len() != self.master_bars.len() {
                    return Err(ModelError::BarCountMismatch {
                        track,
                        staff,
                        expected: self.master_bars.len(),
                        found: s.bars.len(),
                    });
                }
                for (bar, b) in s.bars.iter().enumerate() {
                    let valid = !b.voices.is_empty()
                        && b.voices
                            .iter()
                            .flat_map(|v| &v.beats)
                            .all(|beat| beat.is_valid_for(string_count));
                    if !valid {
                        return Err(ModelError::InvalidBeat { track, staff, bar });
                    }
                }
            }
        }
        Ok(())
    }
}
