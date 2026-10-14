use serde::{Deserialize, Serialize};

use crate::Tuning;

/// What a track is played on. It decides the tuning a new track starts with, how the track
/// sounds and which icon it gets.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
#[serde(rename_all = "snake_case")]
pub enum Instrument {
    #[default]
    SteelGuitar,
    NylonGuitar,
    CleanGuitar,
    OverdriveGuitar,
    DistortionGuitar,
    Bass,
    Ukulele,
    Banjo,
    Mandolin,
    /// Two staves, one for each hand. It has no strings: its notes are pitches.
    Piano,
    /// Every note is a piece of the kit: its fret is the piece's General MIDI number.
    Drums,
}

/// The clef a staff's standard notation is written in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
#[serde(rename_all = "snake_case")]
pub enum Clef {
    Treble,
    Bass,
    Percussion,
}

impl Instrument {
    pub const ALL: [Instrument; 11] = [
        Instrument::SteelGuitar,
        Instrument::NylonGuitar,
        Instrument::CleanGuitar,
        Instrument::OverdriveGuitar,
        Instrument::DistortionGuitar,
        Instrument::Bass,
        Instrument::Ukulele,
        Instrument::Banjo,
        Instrument::Mandolin,
        Instrument::Piano,
        Instrument::Drums,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Instrument::SteelGuitar => "Steel guitar",
            Instrument::NylonGuitar => "Nylon guitar",
            Instrument::CleanGuitar => "Clean guitar",
            Instrument::OverdriveGuitar => "Overdrive guitar",
            Instrument::DistortionGuitar => "Distortion guitar",
            Instrument::Bass => "Bass",
            Instrument::Ukulele => "Ukulele",
            Instrument::Banjo => "Banjo",
            Instrument::Mandolin => "Mandolin",
            Instrument::Piano => "Piano",
            Instrument::Drums => "Drums",
        }
    }

    /// The tuning a new track for this instrument starts with: that of its first staff.
    pub fn default_tuning(self) -> Tuning {
        self.default_tunings().swap_remove(0)
    }

    /// The tuning of each staff a new track for this instrument starts with.
    pub fn default_tunings(self) -> Vec<Tuning> {
        match self {
            Instrument::Bass => vec![Tuning::bass_standard()],
            Instrument::Ukulele => vec![Tuning::new([69, 64, 60, 67])],
            Instrument::Banjo => vec![Tuning::new([62, 59, 55, 50, 67])],
            Instrument::Mandolin => vec![Tuning::new([76, 69, 62, 55])],
            // A staff for each hand, neither with strings.
            Instrument::Piano => vec![Tuning::none(), Tuning::none()],
            // Lines to write the kit on. They have no pitch, so a note's fret alone says
            // which piece it is.
            Instrument::Drums => vec![Tuning::new([0; 5])],
            _ => vec![Tuning::guitar_standard()],
        }
    }

    /// Whether the instrument has no strings, so that its notes are written as pitches.
    pub fn is_pitched(self) -> bool {
        self == Instrument::Piano
    }

    pub fn is_percussion(self) -> bool {
        self == Instrument::Drums
    }

    /// The highest fret a note can have: on drums, the last General MIDI percussion number.
    pub fn max_fret(self) -> u8 {
        match self {
            Instrument::Drums => 87,
            _ => 36,
        }
    }

    /// The clef of one of the instrument's staves, counted from the top.
    pub fn clef(self, staff: usize) -> Clef {
        match (self, staff) {
            (Instrument::Drums, _) => Clef::Percussion,
            (Instrument::Bass, _) | (Instrument::Piano, 1..) => Clef::Bass,
            _ => Clef::Treble,
        }
    }

    /// Semitones added to the sounding pitch when writing standard notation. Guitars,
    /// basses and banjos are written an octave above where they sound.
    pub fn written_transposition(self) -> i8 {
        match self {
            Instrument::Ukulele | Instrument::Mandolin | Instrument::Piano | Instrument::Drums => 0,
            _ => 12,
        }
    }

    /// General MIDI program number, counted from zero.
    pub fn midi_program(self) -> u8 {
        match self {
            Instrument::NylonGuitar => 24,
            Instrument::SteelGuitar => 25,
            Instrument::CleanGuitar => 27,
            Instrument::OverdriveGuitar => 29,
            Instrument::DistortionGuitar => 30,
            Instrument::Bass => 33,
            Instrument::Banjo => 105,
            // General MIDI has neither; these are the programs `.gp` files use for them.
            Instrument::Ukulele => 24,
            Instrument::Mandolin => 25,
            Instrument::Piano => 0,
            // Drums are a MIDI channel, not a program.
            Instrument::Drums => 0,
        }
    }

    /// The nearest instrument to a General MIDI program, counted from zero.
    pub fn from_midi_program(program: u8) -> Instrument {
        match program {
            0..=7 => Instrument::Piano,
            24 => Instrument::NylonGuitar,
            25 => Instrument::SteelGuitar,
            26..=28 => Instrument::CleanGuitar,
            29 => Instrument::OverdriveGuitar,
            30 | 31 => Instrument::DistortionGuitar,
            32..=39 => Instrument::Bass,
            105 => Instrument::Banjo,
            _ => Instrument::SteelGuitar,
        }
    }
}

/// A track's mixer settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
#[serde(default)]
pub struct Mix {
    /// From 0 to [`Mix::MAX_VOLUME`]; [`Mix::UNITY`] plays the track at its natural level.
    pub volume: u8,
    /// From -[`Mix::MAX_PAN`] (left) to [`Mix::MAX_PAN`] (right).
    pub pan: i8,
    pub mute: bool,
    pub solo: bool,
}

impl Mix {
    pub const MAX_VOLUME: u8 = 100;
    pub const UNITY: u8 = 75;
    pub const MAX_PAN: i8 = 100;

    pub fn is_valid(&self) -> bool {
        self.volume <= Mix::MAX_VOLUME && self.pan.unsigned_abs() <= Mix::MAX_PAN.unsigned_abs()
    }

    /// Linear gain for the volume setting: silent at zero, 1.0 at [`Mix::UNITY`].
    pub fn gain(&self) -> f32 {
        Mix::gain_of(self.volume)
    }

    /// Linear gain for a volume on the mixer's scale.
    pub fn gain_of(volume: u8) -> f32 {
        let level = f32::from(volume) / f32::from(Mix::UNITY);
        level * level
    }
}

impl Default for Mix {
    fn default() -> Self {
        Mix {
            volume: Mix::UNITY,
            pan: 0,
            mute: false,
            solo: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn programs_round_trip_where_general_midi_has_the_instrument() {
        for instrument in Instrument::ALL {
            let back = Instrument::from_midi_program(instrument.midi_program());
            let shares_a_program = matches!(
                instrument,
                Instrument::Ukulele | Instrument::Mandolin | Instrument::Drums
            );
            assert!(back == instrument || shares_a_program, "{instrument:?}");
            assert_eq!(
                instrument.default_tuning().strings.is_empty(),
                instrument.is_pitched()
            );
        }
    }

    #[test]
    fn mix_gain_and_range() {
        assert_eq!(Mix::default().gain(), 1.0);
        let silent = Mix {
            volume: 0,
            ..Mix::default()
        };
        assert_eq!(silent.gain(), 0.0);
        let off_scale = Mix {
            volume: 101,
            ..Mix::default()
        };
        assert!(!off_scale.is_valid());
        let hard_left = Mix {
            pan: -100,
            ..Mix::default()
        };
        assert!(hard_left.is_valid());
    }
}
