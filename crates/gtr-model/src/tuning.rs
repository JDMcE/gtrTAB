use serde::{Deserialize, Serialize};

/// Open-string MIDI pitches, from string 1 (highest) to the lowest string. A staff for an
/// instrument without strings, such as a piano, has none.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct Tuning {
    pub strings: Vec<u8>,
}

impl Tuning {
    pub const MAX_STRINGS: usize = 10;

    pub fn new(strings: impl Into<Vec<u8>>) -> Self {
        Tuning {
            strings: strings.into(),
        }
    }

    /// The tuning of a staff without strings, whose notes are pitches.
    pub fn none() -> Self {
        Tuning::new([])
    }

    pub fn guitar_standard() -> Self {
        Tuning::new([64, 59, 55, 50, 45, 40])
    }

    pub fn guitar_drop_d() -> Self {
        Tuning::new([64, 59, 55, 50, 45, 38])
    }

    pub fn guitar_7_standard() -> Self {
        Tuning::new([64, 59, 55, 50, 45, 40, 35])
    }

    pub fn guitar_8_standard() -> Self {
        Tuning::new([64, 59, 55, 50, 45, 40, 35, 30])
    }

    pub fn bass_standard() -> Self {
        Tuning::new([43, 38, 33, 28])
    }

    pub fn bass_5_standard() -> Self {
        Tuning::new([43, 38, 33, 28, 23])
    }

    /// Sounding MIDI pitch of a fretted note, or `None` if the string does not exist.
    pub fn pitch(&self, string: u8, fret: u8, capo: u8) -> Option<u8> {
        let open = *self.strings.get(usize::from(string))?;
        open.checked_add(fret)?.checked_add(capo)
    }
}
