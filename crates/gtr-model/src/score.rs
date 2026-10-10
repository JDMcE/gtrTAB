use serde::{Deserialize, Serialize};

use crate::{Duration, Rational, Tuning};

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
    pub artist: String,
    pub album: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MasterBar {
    pub time_signature: TimeSignature,
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
    pub staves: Vec<Staff>,
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
}

impl Beat {
    pub fn rest(duration: Duration) -> Self {
        Beat {
            duration,
            notes: Vec::new(),
        }
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
            && self
                .notes
                .iter()
                .all(|n| usize::from(n.string) < string_count)
            && self.notes.windows(2).all(|w| w[0].string < w[1].string)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Note {
    /// Index into the staff tuning; 0 is the highest string.
    pub string: u8,
    pub fret: u8,
    #[serde(default)]
    pub tied: bool,
}

impl Note {
    pub fn new(string: u8, fret: u8) -> Self {
        Note {
            string,
            fret,
            tied: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ModelError {
    #[error("tempo must be greater than zero")]
    InvalidTempo,
    #[error("bar {bar} has an invalid time signature")]
    InvalidTimeSignature { bar: usize },
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
            master_bars: vec![MasterBar {
                time_signature: TimeSignature::COMMON,
            }],
            tracks: vec![Track {
                name: "Guitar".into(),
                staves: vec![Staff {
                    tuning: Tuning::guitar_standard(),
                    capo: 0,
                    bars: vec![Bar::empty()],
                }],
            }],
        }
    }

    /// Checks the structural invariants the editor relies on. It does not check that bars are
    /// rhythmically complete; incomplete bars are legal while editing.
    pub fn validate(&self) -> Result<(), ModelError> {
        if self.tempo == 0 {
            return Err(ModelError::InvalidTempo);
        }
        if let Some(bar) = self
            .master_bars
            .iter()
            .position(|m| !m.time_signature.is_valid())
        {
            return Err(ModelError::InvalidTimeSignature { bar });
        }
        for (track, t) in self.tracks.iter().enumerate() {
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
