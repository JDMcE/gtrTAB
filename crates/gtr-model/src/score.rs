use serde::{Deserialize, Serialize};

use crate::{
    BeatEffects, Duration, Instrument, KitPiece, Mix, NoteEffects, Pedal, Rational, Tuning,
    kit_is_valid, standard_kit,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct Score {
    #[serde(default)]
    pub info: ScoreInfo,
    /// Quarter notes per minute.
    pub tempo: u32,
    /// Bar properties shared by every track. Each staff has exactly one bar per master bar.
    pub master_bars: Vec<MasterBar>,
    pub tracks: Vec<Track>,
    /// The pedals the whole mix goes through, in order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[ts(optional, as = "Option<Vec<Pedal>>")]
    pub effects: Vec<Pedal>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct MasterBar {
    pub time_signature: TimeSignature,
    /// Sharps in the key signature if positive, flats if negative.
    #[serde(default)]
    pub key_signature: i8,
    /// A repeated passage starts with this bar.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    #[ts(optional, as = "Option<bool>")]
    pub repeat_start: bool,
    /// A repeated passage ends with this bar and is played this many times in all; zero
    /// if none ends here.
    #[serde(default, skip_serializing_if = "is_zero")]
    #[ts(optional, as = "Option<u8>")]
    pub repeat_count: u8,
    /// The passes of a repeat on which this bar is played, as a bit per pass starting with
    /// the first; zero for a bar played every time.
    #[serde(default, skip_serializing_if = "is_zero")]
    #[ts(optional, as = "Option<u8>")]
    pub alternate_endings: u8,
    /// A section name or rehearsal mark, such as `Chorus` or `A`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub section: Option<String>,
    /// The places this bar is that a jump leads to. Usually one, if any.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[ts(optional, as = "Option<Vec<Target>>")]
    pub targets: Vec<Target>,
    /// Instructions, at the end of this bar, to carry on from somewhere else, in the
    /// order they are taken: each is followed once.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[ts(optional, as = "Option<Vec<Jump>>")]
    pub jumps: Vec<Jump>,
    /// A new tempo from this bar on, in quarter notes per minute.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub tempo: Option<u32>,
    /// From the start of this bar the tempo moves steadily towards the next change of
    /// tempo, wherever that is, instead of holding until it.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    #[ts(optional, as = "Option<bool>")]
    pub tempo_gradual: bool,
    /// Changes of tempo part-way through the bar, in the order they come.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[ts(optional, as = "Option<Vec<TempoChange>>")]
    pub tempo_changes: Vec<TempoChange>,
    /// Whether pairs of short notes are played unevenly from this bar on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub feel: Option<Feel>,
    /// The time elapsed when the piece reaches this bar is written above it.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    #[ts(optional, as = "Option<bool>")]
    pub timer: bool,
}

/// A new tempo from a beat inside a bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct TempoChange {
    /// Where in the bar it happens, in whole notes from the bar's start.
    #[ts(type = "[number, number]")]
    pub position: Rational,
    /// Quarter notes per minute.
    pub tempo: u32,
    /// From here the tempo moves steadily towards the next change, instead of holding.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    #[ts(optional, as = "Option<bool>")]
    pub gradual: bool,
}

fn is_zero(value: &u8) -> bool {
    *value == 0
}

/// A marked place in the score.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
#[serde(rename_all = "snake_case")]
pub enum Target {
    Segno,
    Coda,
    /// The end of the piece, once a jump "al fine" has been taken.
    Fine,
    /// A second segno, for a piece that goes back to two places.
    SegnoSegno,
    /// A second coda.
    DoubleCoda,
}

impl Target {
    pub const ALL: [Target; 5] = [
        Target::Segno,
        Target::SegnoSegno,
        Target::Coda,
        Target::DoubleCoda,
        Target::Fine,
    ];
}

/// Where to carry on from at the end of a bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
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
    DaCapoAlDoubleCoda,
    DalSegnoAlDoubleCoda,
    /// Back to the second segno.
    DalSegnoSegno,
    DalSegnoSegnoAlFine,
    DalSegnoSegnoAlCoda,
    DalSegnoSegnoAlDoubleCoda,
    /// On to the second coda, once a jump "al double coda" has been taken.
    ToDoubleCoda,
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
            Jump::DaCapoAlDoubleCoda => "D.C. al Double Coda",
            Jump::DalSegnoAlDoubleCoda => "D.S. al Double Coda",
            Jump::DalSegnoSegno => "D.S.S.",
            Jump::DalSegnoSegnoAlFine => "D.S.S. al Fine",
            Jump::DalSegnoSegnoAlCoda => "D.S.S. al Coda",
            Jump::DalSegnoSegnoAlDoubleCoda => "D.S.S. al Double Coda",
            Jump::ToDoubleCoda => "To Double Coda",
        }
    }

    /// The sign it goes back to, or `None` for the start of the piece or for a jump on
    /// to a coda.
    pub fn back_to(self) -> Option<Target> {
        use Jump::*;
        match self {
            DalSegno | DalSegnoAlFine | DalSegnoAlCoda | DalSegnoAlDoubleCoda => {
                Some(Target::Segno)
            }
            DalSegnoSegno
            | DalSegnoSegnoAlFine
            | DalSegnoSegnoAlCoda
            | DalSegnoSegnoAlDoubleCoda => Some(Target::SegnoSegno),
            _ => None,
        }
    }

    /// Where the piece is to go once it has gone back: on to the end, to stop at the
    /// fine, or to leave for one of the codas.
    pub fn until(self) -> Option<Target> {
        use Jump::*;
        match self {
            DaCapoAlFine | DalSegnoAlFine | DalSegnoSegnoAlFine => Some(Target::Fine),
            DaCapoAlCoda | DalSegnoAlCoda | DalSegnoSegnoAlCoda => Some(Target::Coda),
            DaCapoAlDoubleCoda | DalSegnoAlDoubleCoda | DalSegnoSegnoAlDoubleCoda => {
                Some(Target::DoubleCoda)
            }
            _ => None,
        }
    }

    /// The coda a jump onwards leads to, for the two jumps that are that.
    pub fn onward(self) -> Option<Target> {
        match self {
            Jump::ToCoda => Some(Target::Coda),
            Jump::ToDoubleCoda => Some(Target::DoubleCoda),
            _ => None,
        }
    }
}

/// How evenly pairs of short notes are played.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
#[serde(rename_all = "snake_case")]
pub enum Feel {
    Straight,
    /// Pairs of eighth notes are played long-short, as two thirds and one third.
    SwingEighths,
    SwingSixteenths,
    /// Long-short as three quarters and one quarter: a dotted note and a short one.
    DottedEighths,
    DottedSixteenths,
    /// Short-long, the Scotch snap: one quarter and three quarters.
    ScottishEighths,
    ScottishSixteenths,
}

impl Feel {
    /// The length of a pair of the notes it moves, and the share of it the first takes;
    /// nothing for a straight feel.
    pub fn pair(self) -> Option<(Rational, Rational)> {
        let (eighths, share) = match self {
            Feel::Straight => return None,
            Feel::SwingEighths => (true, Rational::new(2, 3)),
            Feel::SwingSixteenths => (false, Rational::new(2, 3)),
            Feel::DottedEighths => (true, Rational::new(3, 4)),
            Feel::DottedSixteenths => (false, Rational::new(3, 4)),
            Feel::ScottishEighths => (true, Rational::new(1, 4)),
            Feel::ScottishSixteenths => (false, Rational::new(1, 4)),
        };
        Some((Rational::new(1, if eighths { 4 } else { 8 }), share))
    }

    pub fn label(self) -> &'static str {
        match self {
            Feel::Straight => "Straight",
            Feel::SwingEighths => "Swing 8ths",
            Feel::SwingSixteenths => "Swing 16ths",
            Feel::DottedEighths => "Dotted 8ths",
            Feel::DottedSixteenths => "Dotted 16ths",
            Feel::ScottishEighths => "Scottish 8ths",
            Feel::ScottishSixteenths => "Scottish 16ths",
        }
    }
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
            targets: Vec::new(),
            jumps: Vec::new(),
            tempo: None,
            tempo_gradual: false,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct Track {
    pub name: String,
    #[serde(default)]
    pub instrument: Instrument,
    #[serde(default)]
    pub mix: Mix,
    /// The drum kit of a drum track that has one of its own: which pieces it has and how
    /// each is written. Empty for the standard kit.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[ts(optional, as = "Option<Vec<KitPiece>>")]
    pub kit: Vec<KitPiece>,
    /// The pedals the track's sound goes through on its way to the mix, in order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[ts(optional, as = "Option<Vec<Pedal>>")]
    pub effects: Vec<Pedal>,
    pub staves: Vec<Staff>,
}

impl Track {
    /// The track's drum kit: its own, or the standard one.
    pub fn drum_kit(&self) -> Vec<KitPiece> {
        match self.kit.is_empty() {
            true => standard_kit(),
            false => self.kit.clone(),
        }
    }

    /// A track with default mixer settings.
    pub fn new(name: impl Into<String>, instrument: Instrument, staves: Vec<Staff>) -> Self {
        Track {
            name: name.into(),
            instrument,
            mix: Mix::default(),
            kit: Vec::new(),
            effects: Vec::new(),
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct Staff {
    /// The strings the staff's notes are played on. A staff without any is for an
    /// instrument that has none: its notes are pitches (see [`Note::string`]).
    pub tuning: Tuning,
    #[serde(default)]
    pub capo: u8,
    pub bars: Vec<Bar>,
}

impl Staff {
    /// How many pitches a staff without strings can hold: every MIDI pitch.
    pub const PITCHES: usize = 128;

    /// Whether the staff's notes are pitches, not frets on strings.
    pub fn is_pitched(&self) -> bool {
        self.tuning.strings.is_empty()
    }

    /// How many places a beat has for a note: one for each string, or on a staff without
    /// strings one for each pitch.
    pub fn places(&self) -> usize {
        match self.is_pitched() {
            true => Staff::PITCHES,
            false => self.tuning.strings.len(),
        }
    }

    /// Sounding MIDI pitch of a note at `string` and `fret`, or `None` if the staff has no
    /// such string.
    pub fn pitch(&self, string: u8, fret: u8) -> Option<u8> {
        match self.is_pitched() {
            true => (usize::from(string) < Staff::PITCHES).then_some(string),
            false => self.tuning.pitch(string, fret, self.capo),
        }
    }

    /// Checks a beat against the staff: its notes are on places the staff has, and on a
    /// staff without strings they have no fret.
    pub fn holds(&self, beat: &Beat) -> bool {
        beat.is_valid_for(self.places())
            && (!self.is_pitched() || beat.notes.iter().all(|n| n.fret == 0))
    }

    /// The same music for an instrument without strings: every note becomes the pitch it
    /// sounds at. Two strings stopped at the same pitch become one note.
    pub fn to_pitched(&self) -> Staff {
        let mut staff = Staff {
            tuning: Tuning::none(),
            capo: 0,
            bars: self.bars.clone(),
        };
        if self.is_pitched() {
            return staff;
        }
        for beat in staff
            .bars
            .iter_mut()
            .flat_map(|b| &mut b.voices)
            .flat_map(|v| &mut v.beats)
        {
            let mut notes: Vec<Note> = (beat.notes.drain(..))
                .filter_map(|mut note| {
                    if let Some(grace) = &mut note.effects.grace {
                        grace.notes.retain_mut(|g| {
                            let pitch = self.pitch(g.string, g.fret);
                            (g.string, g.fret) = (pitch.unwrap_or(0), 0);
                            pitch.is_some_and(|p| usize::from(p) < Staff::PITCHES)
                        });
                    }
                    note.effects.grace.take_if(|g| g.notes.is_empty());
                    let pitch = self.pitch(note.string, note.fret)?;
                    (usize::from(pitch) < Staff::PITCHES).then_some(Note {
                        string: pitch,
                        fret: 0,
                        ..note
                    })
                })
                .collect();
            notes.sort_by_key(|n| n.string);
            notes.dedup_by_key(|n| n.string);
            beat.notes = notes;
        }
        staff
    }

    /// The same music on strings tuned to `tuning`, with frets up to `max_fret`: each note
    /// goes on the string that plays it nearest the nut, highest notes first. Notes the
    /// strings cannot reach, or that have no string left in their chord, are left out.
    pub fn to_strings(&self, tuning: Tuning, max_fret: u8) -> Staff {
        let place = |pitch: u8, taken: &[u8]| {
            (tuning.strings.iter().enumerate())
                .filter(|(string, open)| {
                    !taken.contains(&(*string as u8))
                        && **open <= pitch
                        && pitch - **open <= max_fret
                })
                .min_by_key(|(_, open)| pitch - **open)
                .map(|(string, open)| (string as u8, pitch - *open))
        };
        let mut bars = self.bars.clone();
        if !self.is_pitched() {
            return Staff {
                tuning,
                capo: 0,
                bars,
            };
        }
        for beat in bars
            .iter_mut()
            .flat_map(|b| &mut b.voices)
            .flat_map(|v| &mut v.beats)
        {
            let mut taken = Vec::new();
            let mut notes: Vec<Note> = (beat.notes.drain(..).rev())
                .filter_map(|mut note| {
                    let (string, fret) = place(note.string, &taken)?;
                    taken.push(string);
                    if let Some(grace) = &mut note.effects.grace {
                        // An ornament stays on the string of the note it leads to.
                        grace.notes.retain_mut(|g| {
                            let fret = g.string.checked_sub(tuning.strings[usize::from(string)]);
                            (g.string, g.fret) = (string, fret.unwrap_or(0));
                            fret.is_some_and(|fret| fret <= max_fret)
                        });
                    }
                    note.effects.grace.take_if(|g| g.notes.is_empty());
                    Some(Note {
                        string,
                        fret,
                        ..note
                    })
                })
                .collect();
            notes.sort_by_key(|n| n.string);
            beat.notes = notes;
        }
        Staff {
            tuning,
            capo: 0,
            bars,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
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

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct Beat {
    pub duration: Duration,
    /// Sorted by string, at most one note per string; on a staff without strings, sorted
    /// by pitch, at most one note per pitch.
    #[serde(default)]
    pub notes: Vec<Note>,
    #[serde(default, skip_serializing_if = "BeatEffects::is_none")]
    #[ts(optional, as = "Option<BeatEffects>")]
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

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct Note {
    /// Index into the staff tuning; 0 is the highest string. On a staff without strings
    /// this is the note itself: its MIDI pitch.
    pub string: u8,
    /// The fret the string is stopped at; zero on a staff without strings. On a drum
    /// track it is the General MIDI number of a piece of the kit.
    pub fret: u8,
    #[serde(default)]
    pub tied: bool,
    #[serde(default, skip_serializing_if = "NoteEffects::is_none")]
    #[ts(optional, as = "Option<NoteEffects>")]
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
    #[error("the master effects are not valid")]
    InvalidEffects,
    #[error("track {track} has no staff, or settings that are out of range")]
    InvalidTrack { track: usize },
    #[error("track {track} staff {staff} has too many strings")]
    TooManyStrings { track: usize, staff: usize },
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
            effects: Vec::new(),
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
        if !Pedal::chain_is_valid(&self.effects) {
            return Err(ModelError::InvalidEffects);
        }
        for (track, t) in self.tracks.iter().enumerate() {
            if t.staves.is_empty()
                || !t.mix.is_valid()
                || !kit_is_valid(&t.kit)
                || !Pedal::chain_is_valid(&t.effects)
            {
                return Err(ModelError::InvalidTrack { track });
            }
            for (staff, s) in t.staves.iter().enumerate() {
                if s.tuning.strings.len() > Tuning::MAX_STRINGS {
                    return Err(ModelError::TooManyStrings { track, staff });
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
                            .all(|beat| s.holds(beat));
                    if !valid {
                        return Err(ModelError::InvalidBeat { track, staff, bar });
                    }
                }
            }
        }
        Ok(())
    }
}
