use std::num::NonZeroU8;

use serde::{Deserialize, Serialize};

/// Exact musical time, measured in whole notes.
pub type Rational = num_rational::Ratio<i64>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
#[serde(rename_all = "snake_case")]
pub enum NoteValue {
    Whole,
    Half,
    Quarter,
    Eighth,
    Sixteenth,
    ThirtySecond,
    SixtyFourth,
    HundredTwentyEighth,
    TwoHundredFiftySixth,
}

impl NoteValue {
    pub const ALL: [NoteValue; 9] = [
        NoteValue::Whole,
        NoteValue::Half,
        NoteValue::Quarter,
        NoteValue::Eighth,
        NoteValue::Sixteenth,
        NoteValue::ThirtySecond,
        NoteValue::SixtyFourth,
        NoteValue::HundredTwentyEighth,
        NoteValue::TwoHundredFiftySixth,
    ];

    pub fn length(self) -> Rational {
        Rational::new(1, 1 << (self as u32))
    }

    /// The next shorter value, e.g. quarter → eighth.
    pub fn shorter(self) -> Option<NoteValue> {
        Self::ALL.get(self as usize + 1).copied()
    }

    /// The next longer value, e.g. quarter → half.
    pub fn longer(self) -> Option<NoteValue> {
        (self as usize).checked_sub(1).map(|i| Self::ALL[i])
    }

    /// Number of flags or beams: 0 up to a quarter, 1 for an eighth, and so on.
    pub fn flags(self) -> u32 {
        (self as u32).saturating_sub(NoteValue::Quarter as u32)
    }
}

/// `actual` notes played in the time of `normal`, e.g. 3:2 for a triplet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct Tuplet {
    pub actual: NonZeroU8,
    pub normal: NonZeroU8,
}

impl Tuplet {
    pub const TRIPLET: Tuplet = Tuplet {
        actual: NonZeroU8::new(3).unwrap(),
        normal: NonZeroU8::new(2).unwrap(),
    };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct Duration {
    pub value: NoteValue,
    #[serde(default)]
    pub dots: u8,
    #[serde(default)]
    pub tuplet: Option<Tuplet>,
    /// A second tuplet inside the first, such as a triplet within a quintuplet.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub nested: Option<Tuplet>,
}

impl Duration {
    pub const MAX_DOTS: u8 = 3;

    pub fn new(value: NoteValue) -> Self {
        Duration {
            value,
            dots: 0,
            tuplet: None,
            nested: None,
        }
    }

    pub fn is_valid(&self) -> bool {
        self.dots <= Self::MAX_DOTS
    }

    /// Length in whole notes. Dots beyond [`Self::MAX_DOTS`] are ignored.
    pub fn length(&self) -> Rational {
        let dots = u32::from(self.dots.min(Self::MAX_DOTS));
        // n dots multiply the length by 2 - 1/2^n.
        let dotted = self.value.length() * Rational::new((2 << dots) - 1, 1 << dots);
        [self.tuplet, self.nested]
            .into_iter()
            .flatten()
            .fold(dotted, |length, t| {
                length * Rational::new(t.normal.get().into(), t.actual.get().into())
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lengths() {
        assert_eq!(NoteValue::Quarter.length(), Rational::new(1, 4));
        assert_eq!(NoteValue::SixtyFourth.length(), Rational::new(1, 64));
        assert_eq!(
            NoteValue::TwoHundredFiftySixth.length(),
            Rational::new(1, 256)
        );
        assert_eq!(NoteValue::HundredTwentyEighth.flags(), 5);
        assert_eq!(
            NoteValue::SixtyFourth.shorter(),
            Some(NoteValue::HundredTwentyEighth)
        );
        assert_eq!(NoteValue::TwoHundredFiftySixth.shorter(), None);
        // A third dot adds an eighth of the note again.
        let mut dotted = Duration::new(NoteValue::Half);
        dotted.dots = 3;
        assert_eq!(dotted.length(), Rational::new(15, 16));
        assert!(dotted.is_valid());

        let mut d = Duration::new(NoteValue::Quarter);
        d.dots = 1;
        assert_eq!(d.length(), Rational::new(3, 8));
        d.dots = 2;
        assert_eq!(d.length(), Rational::new(7, 16));

        let mut triplet = Duration::new(NoteValue::Eighth);
        triplet.tuplet = Some(Tuplet::TRIPLET);
        assert_eq!(triplet.length() * 3, Rational::new(1, 4));
        // Three of these fill one note of a quintuplet of eighths.
        let mut nested = Duration::new(NoteValue::Sixteenth);
        nested.tuplet = Some(Tuplet {
            actual: NonZeroU8::new(5).unwrap(),
            normal: NonZeroU8::new(4).unwrap(),
        });
        nested.nested = Some(Tuplet::TRIPLET);
        assert_eq!(nested.length() * 3, Rational::new(1, 10));
    }
}
