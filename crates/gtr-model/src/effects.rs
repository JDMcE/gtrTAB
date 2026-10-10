use serde::{Deserialize, Serialize};

use crate::NoteValue;

/// Playing techniques attached to a single note.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(default)]
pub struct NoteEffects {
    /// Hammer-on or pull-off into the next note on the same string.
    #[serde(skip_serializing_if = "is_false")]
    pub hammer: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slide: Option<Slide>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bend: Option<Bend>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vibrato: Option<Vibrato>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub harmonic: Option<Harmonic>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accent: Option<Accent>,
    #[serde(skip_serializing_if = "is_false")]
    pub palm_mute: bool,
    #[serde(skip_serializing_if = "is_false")]
    pub let_ring: bool,
    /// A muted, pitchless note, written as an x.
    #[serde(skip_serializing_if = "is_false")]
    pub dead: bool,
    /// A quiet note, written in parentheses.
    #[serde(skip_serializing_if = "is_false")]
    pub ghost: bool,
    #[serde(skip_serializing_if = "is_false")]
    pub staccato: bool,
    /// Tapped with the picking hand.
    #[serde(skip_serializing_if = "is_false")]
    pub tapped: bool,
}

fn is_false(value: &bool) -> bool {
    !*value
}

impl NoteEffects {
    pub fn is_none(&self) -> bool {
        *self == NoteEffects::default()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Slide {
    /// Slide to the next note and pick it.
    Shift,
    /// Slide to the next note without picking it.
    Legato,
    InFromBelow,
    InFromAbove,
    OutDown,
    OutUp,
}

impl Slide {
    /// Whether the slide leads into the next note on the string.
    pub fn joins_next(self) -> bool {
        matches!(self, Slide::Shift | Slide::Legato)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BendKind {
    Bend,
    BendRelease,
    Prebend,
    PrebendRelease,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Bend {
    pub kind: BendKind,
    /// Height of the bend in quarter tones: 2 is a half step, 4 a full step.
    pub quarter_tones: u8,
}

impl Bend {
    pub const MAX_QUARTER_TONES: u8 = 12;

    pub fn is_valid(&self) -> bool {
        (1..=Self::MAX_QUARTER_TONES).contains(&self.quarter_tones)
    }

    /// The height as guitarists write it: "full", "½", "1½" and so on.
    pub fn label(&self) -> String {
        let whole = self.quarter_tones / 4;
        let fraction = ["", "¼", "½", "¾"][usize::from(self.quarter_tones % 4)];
        match (whole, fraction) {
            (1, "") => "full".into(),
            (0, fraction) => fraction.into(),
            (whole, fraction) => format!("{whole}{fraction}"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Vibrato {
    Slight,
    Wide,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Harmonic {
    Natural,
    Artificial,
    Pinch,
    Tapped,
    Semi,
}

impl Harmonic {
    /// The abbreviation written above the staff.
    pub fn label(self) -> &'static str {
        match self {
            Harmonic::Natural => "N.H.",
            Harmonic::Artificial => "A.H.",
            Harmonic::Pinch => "P.H.",
            Harmonic::Tapped => "T.H.",
            Harmonic::Semi => "S.H.",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Accent {
    Normal,
    Heavy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PickStroke {
    Down,
    Up,
}

/// Playing techniques attached to a whole beat.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(default)]
pub struct BeatEffects {
    /// Tremolo picking: the beat is re-picked in notes of this value.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tremolo: Option<NoteValue>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stroke: Option<PickStroke>,
}

impl BeatEffects {
    pub fn is_none(&self) -> bool {
        *self == BeatEffects::default()
    }

    pub fn is_valid(&self) -> bool {
        self.tremolo.is_none_or(|value| {
            matches!(
                value,
                NoteValue::Eighth | NoteValue::Sixteenth | NoteValue::ThirtySecond
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bend_labels() {
        let label = |quarter_tones| {
            Bend {
                kind: BendKind::Bend,
                quarter_tones,
            }
            .label()
        };
        assert_eq!(label(1), "¼");
        assert_eq!(label(2), "½");
        assert_eq!(label(4), "full");
        assert_eq!(label(6), "1½");
        assert_eq!(label(8), "2");
    }

    #[test]
    fn plain_notes_serialise_without_effects() {
        let json = serde_json::to_string(&crate::Note::new(2, 5)).unwrap();
        assert_eq!(json, r#"{"string":2,"fret":5,"tied":false}"#);
        let note: crate::Note = serde_json::from_str(&json).unwrap();
        assert!(note.effects.is_none());
    }
}
