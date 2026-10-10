use std::ops::RangeInclusive;

use serde::{Deserialize, Serialize};

use crate::NoteValue;

/// Playing techniques attached to a single note. Fields that are off are left out of the
/// saved file.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trill: Option<Trill>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grace: Option<Grace>,
    /// Fretting-hand finger.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub left_finger: Option<Finger>,
    /// Picking-hand finger.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub right_finger: Option<Finger>,
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

    pub fn is_valid(&self) -> bool {
        (self.bend.as_ref()).is_none_or(|b| b.is_valid(Bend::BEND_RANGE))
            && self.trill.is_none_or(|t| t.is_valid())
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

/// The common shapes of bend.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BendKind {
    Bend,
    BendRelease,
    Prebend,
    PrebendRelease,
}

/// One point of a [`Bend`] curve.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CurvePoint {
    /// When the point is reached, from 0 at the start of the note to
    /// [`Bend::LENGTH`] at its end.
    pub position: u8,
    /// Pitch offset in quarter tones: 2 is a half step, 4 a full step.
    pub value: i8,
}

/// A pitch curve over the length of a note, used for string bends and for the whammy bar.
/// The pitch moves in straight lines from point to point.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Bend {
    pub points: Vec<CurvePoint>,
}

impl Bend {
    /// Position of the end of the note.
    pub const LENGTH: u8 = 60;
    pub const MAX_POINTS: usize = 16;
    /// A string can only be bent upwards, by up to three tones.
    pub const BEND_RANGE: RangeInclusive<i8> = 0..=12;
    /// The whammy bar goes both ways, and a long way down.
    pub const WHAMMY_RANGE: RangeInclusive<i8> = -24..=24;

    pub fn new(points: impl IntoIterator<Item = (u8, i8)>) -> Self {
        Bend {
            points: (points.into_iter())
                .map(|(position, value)| CurvePoint { position, value })
                .collect(),
        }
    }

    /// One of the common bend shapes, `quarter_tones` high.
    pub fn preset(kind: BendKind, quarter_tones: i8) -> Self {
        let v = quarter_tones;
        Bend::new(match kind {
            BendKind::Bend => vec![(0, 0), (15, v), (60, v)],
            BendKind::BendRelease => vec![(0, 0), (15, v), (30, v), (45, 0), (60, 0)],
            BendKind::Prebend => vec![(0, v), (60, v)],
            BendKind::PrebendRelease => vec![(0, v), (20, v), (40, 0), (60, 0)],
        })
    }

    /// A whammy bar dip: down by `quarter_tones` and back.
    pub fn dip(quarter_tones: i8) -> Self {
        Bend::new([(0, 0), (10, -quarter_tones), (20, 0), (60, 0)])
    }

    /// A whammy bar dive: down by `quarter_tones` over the length of the note.
    pub fn dive(quarter_tones: i8) -> Self {
        Bend::new([(0, 0), (60, -quarter_tones)])
    }

    /// Checks the curve has between one and [`Self::MAX_POINTS`] points in time order, with
    /// values inside `range`, and that it moves the pitch at all.
    pub fn is_valid(&self, range: RangeInclusive<i8>) -> bool {
        (1..=Self::MAX_POINTS).contains(&self.points.len())
            && self
                .points
                .windows(2)
                .all(|w| w[0].position < w[1].position)
            && (self.points.iter()).all(|p| p.position <= Self::LENGTH && range.contains(&p.value))
            && self.points.iter().any(|p| p.value != 0)
    }

    /// The value furthest from zero.
    pub fn peak(&self) -> i8 {
        (self.points.iter().map(|p| p.value))
            .max_by_key(|v| v.unsigned_abs())
            .unwrap_or(0)
    }

    /// The nearest common shape: whether the curve starts bent, and whether it comes back
    /// down.
    pub fn kind(&self) -> BendKind {
        let first = self.points.first().map_or(0, |p| p.value);
        let last = self.points.last().map_or(0, |p| p.value);
        match (first > 0, last < self.peak()) {
            (false, false) => BendKind::Bend,
            (false, true) => BendKind::BendRelease,
            (true, false) => BendKind::Prebend,
            (true, true) => BendKind::PrebendRelease,
        }
    }
}

/// A pitch interval as guitarists write it: "full", "½", "1½", "-2" and so on.
pub fn interval_label(quarter_tones: i8) -> String {
    let size = quarter_tones.unsigned_abs();
    let fraction = ["", "¼", "½", "¾"][usize::from(size % 4)];
    let sign = if quarter_tones < 0 { "-" } else { "" };
    match (size / 4, fraction) {
        (1, "") if quarter_tones > 0 => "full".into(),
        (0, fraction) => format!("{sign}{fraction}"),
        (whole, fraction) => format!("{sign}{whole}{fraction}"),
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

/// A rapid alternation between the note and another fret on the same string.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Trill {
    pub fret: u8,
    /// Length of each note of the trill.
    pub speed: NoteValue,
}

impl Trill {
    pub fn is_valid(&self) -> bool {
        matches!(
            self.speed,
            NoteValue::Sixteenth | NoteValue::ThirtySecond | NoteValue::SixtyFourth
        )
    }
}

/// A quick ornamental note played on the same string just before the main note.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Grace {
    pub fret: u8,
    /// Takes its time from the main note instead of from the beat before.
    #[serde(default)]
    pub on_beat: bool,
    /// Hammered or pulled into the main note rather than picked separately.
    #[serde(default)]
    pub hammer: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Finger {
    Thumb,
    Index,
    Middle,
    Ring,
    Pinky,
}

impl Finger {
    /// Fretting-hand name: T and 1 to 4.
    pub fn left_label(self) -> &'static str {
        ["T", "1", "2", "3", "4"][self as usize]
    }

    /// Picking-hand name, from the Spanish: pulgar, índice, medio, anular, chico.
    pub fn right_label(self) -> &'static str {
        ["p", "i", "m", "a", "c"][self as usize]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PickStroke {
    Down,
    Up,
}

/// How the notes of a chord are spread out in time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Strum {
    /// A quick strum from the lowest string to the highest.
    BrushDown,
    BrushUp,
    /// A slow spread from the lowest string to the highest.
    ArpeggioDown,
    ArpeggioUp,
    Rasgueado,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Slap {
    Slap,
    Pop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Dynamic {
    Ppp,
    Pp,
    P,
    Mp,
    Mf,
    F,
    Ff,
    Fff,
}

impl Dynamic {
    pub const ALL: [Dynamic; 8] = [
        Dynamic::Ppp,
        Dynamic::Pp,
        Dynamic::P,
        Dynamic::Mp,
        Dynamic::Mf,
        Dynamic::F,
        Dynamic::Ff,
        Dynamic::Fff,
    ];

    /// Loudness relative to forte, the level scores start at.
    pub fn gain(self) -> f32 {
        [0.3, 0.4, 0.52, 0.66, 0.82, 1.0, 1.2, 1.4][self as usize]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Hairpin {
    Crescendo,
    Decrescendo,
}

/// Playing techniques attached to a whole beat.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct BeatEffects {
    /// Tremolo picking: the beat is re-picked in notes of this value.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tremolo: Option<NoteValue>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stroke: Option<PickStroke>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strum: Option<Strum>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slap: Option<Slap>,
    /// Whammy bar movement over the beat.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub whammy: Option<Bend>,
    /// A change of loudness that lasts until the next one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dynamic: Option<Dynamic>,
    /// The beat is part of a gradual change of loudness.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hairpin: Option<Hairpin>,
}

impl BeatEffects {
    pub fn is_none(&self) -> bool {
        *self == BeatEffects::default()
    }

    pub fn is_valid(&self) -> bool {
        let tremolo = self.tremolo.is_none_or(|value| {
            matches!(
                value,
                NoteValue::Eighth | NoteValue::Sixteenth | NoteValue::ThirtySecond
            )
        });
        tremolo && (self.whammy.as_ref()).is_none_or(|w| w.is_valid(Bend::WHAMMY_RANGE))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interval_labels() {
        assert_eq!(interval_label(1), "¼");
        assert_eq!(interval_label(2), "½");
        assert_eq!(interval_label(4), "full");
        assert_eq!(interval_label(6), "1½");
        assert_eq!(interval_label(8), "2");
        assert_eq!(interval_label(-4), "-1");
        assert_eq!(interval_label(-10), "-2½");
    }

    #[test]
    fn presets_report_their_own_kind() {
        for kind in [
            BendKind::Bend,
            BendKind::BendRelease,
            BendKind::Prebend,
            BendKind::PrebendRelease,
        ] {
            let bend = Bend::preset(kind, 4);
            assert_eq!(bend.kind(), kind);
            assert_eq!(bend.peak(), 4);
            assert!(bend.is_valid(Bend::BEND_RANGE));
        }
        assert_eq!(Bend::dive(8).peak(), -8);
        assert!(Bend::dip(4).is_valid(Bend::WHAMMY_RANGE));
        assert!(!Bend::dip(4).is_valid(Bend::BEND_RANGE));
    }

    #[test]
    fn invalid_curves_are_rejected() {
        let valid =
            |points: &[(u8, i8)]| Bend::new(points.iter().copied()).is_valid(Bend::BEND_RANGE);
        assert!(valid(&[(0, 0), (30, 4)]));
        assert!(!valid(&[]));
        assert!(!valid(&[(0, 0), (60, 0)]), "flat");
        assert!(!valid(&[(30, 4), (10, 0)]), "out of order");
        assert!(!valid(&[(0, 0), (61, 4)]), "past the end");
        assert!(!valid(&[(0, 0), (60, 13)]), "too high");
    }

    #[test]
    fn plain_notes_serialise_without_effects() {
        let json = serde_json::to_string(&crate::Note::new(2, 5)).unwrap();
        assert_eq!(json, r#"{"string":2,"fret":5,"tied":false}"#);
        let note: crate::Note = serde_json::from_str(&json).unwrap();
        assert!(note.effects.is_none());
    }
}
