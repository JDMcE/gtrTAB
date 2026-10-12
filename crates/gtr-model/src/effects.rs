use std::ops::RangeInclusive;

use serde::{Deserialize, Serialize};

use crate::NoteValue;

/// Playing techniques attached to a single note. Fields that are off are left out of the
/// saved file.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
#[serde(default, deny_unknown_fields)]
pub struct NoteEffects {
    /// Hammer-on or pull-off into the next note on the same string.
    #[serde(skip_serializing_if = "is_false")]
    #[ts(optional, as = "Option<bool>")]
    pub hammer: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub slide: Option<Slide>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub bend: Option<Bend>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub vibrato: Option<Vibrato>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub harmonic: Option<Harmonic>,
    /// For a harmonic made on a stopped string: how many frets above the note the string
    /// is touched, which decides the overtone that sounds. `None` is the kind's usual place.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub harmonic_touch: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub accent: Option<Accent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub trill: Option<Trill>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub grace: Option<Grace>,
    /// Fretting-hand finger.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub left_finger: Option<Finger>,
    /// Picking-hand finger.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub right_finger: Option<Finger>,
    #[serde(skip_serializing_if = "is_false")]
    #[ts(optional, as = "Option<bool>")]
    pub palm_mute: bool,
    #[serde(skip_serializing_if = "is_false")]
    #[ts(optional, as = "Option<bool>")]
    pub let_ring: bool,
    /// A muted, pitchless note, written as an x.
    #[serde(skip_serializing_if = "is_false")]
    #[ts(optional, as = "Option<bool>")]
    pub dead: bool,
    /// A quiet note, written in parentheses.
    #[serde(skip_serializing_if = "is_false")]
    #[ts(optional, as = "Option<bool>")]
    pub ghost: bool,
    #[serde(skip_serializing_if = "is_false")]
    #[ts(optional, as = "Option<bool>")]
    pub staccato: bool,
    /// Tapped with the picking hand.
    #[serde(skip_serializing_if = "is_false")]
    #[ts(optional, as = "Option<bool>")]
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
            && (self.harmonic_touch).is_none_or(|t| (1..=Harmonic::MAX_TOUCH).contains(&t))
            && self.grace.as_ref().is_none_or(Grace::is_valid)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
#[serde(rename_all = "snake_case")]
pub enum BendKind {
    Bend,
    BendRelease,
    Prebend,
    PrebendRelease,
}

/// One point of a [`Bend`] curve.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct CurvePoint {
    /// When the point is reached, from 0 at the start of the note to
    /// [`Bend::LENGTH`] at its end.
    pub position: u8,
    /// Pitch offset in quarter tones: 2 is a half step, 4 a full step.
    pub value: i8,
}

/// A pitch curve over the length of a note, used for string bends and for the whammy bar.
/// The pitch moves in straight lines from point to point.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
#[serde(rename_all = "snake_case")]
pub enum Vibrato {
    Slight,
    Wide,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
#[serde(rename_all = "snake_case")]
pub enum Harmonic {
    Natural,
    Artificial,
    Pinch,
    Tapped,
    Semi,
}

impl Harmonic {
    /// The furthest above a stopped note that its string can be touched for a harmonic.
    pub const MAX_TOUCH: u8 = 24;

    /// Semitones above a string's pitch of the harmonic that sounds when the string is
    /// touched `frets` above where it is stopped, or above the nut if it is open.
    pub fn overtone(frets: u8) -> u8 {
        let above = match frets % 12 {
            0 => 12,
            7 => 19,
            5 => 24,
            4 | 9 => 28,
            3 => 31,
            other => 12 + other,
        };
        above + if frets >= 24 { 12 } else { 0 }
    }

    /// Where the string is usually touched for this kind of harmonic, in frets above the
    /// stopped note: an octave up, or for a pinch harmonic, which is made near the
    /// pickups, at the place that sounds two octaves up.
    pub fn usual_touch(self) -> u8 {
        match self {
            Harmonic::Pinch => 5,
            _ => 12,
        }
    }

    /// Semitones above the stopped note that a harmonic of this kind sounds, touched at
    /// `touch` frets above it or at the usual place. Not for natural harmonics, whose
    /// pitch comes from the open string.
    pub fn interval(self, touch: Option<u8>) -> u8 {
        Harmonic::overtone(touch.unwrap_or(self.usual_touch()))
    }

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
#[serde(rename_all = "snake_case")]
pub enum Accent {
    Normal,
    Heavy,
}

/// A rapid alternation between the note and another fret on the same string.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
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

/// One note of a [`Grace`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct GraceNote {
    /// Index into the staff tuning, like a note's.
    pub string: u8,
    pub fret: u8,
}

/// Quick ornamental notes played one after another just before the main note, on its
/// string or on others.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct Grace {
    /// In the order they are played.
    pub notes: Vec<GraceNote>,
    /// Takes its time from the main note instead of from the beat before.
    #[serde(default)]
    pub on_beat: bool,
    /// Each note is hammered or pulled into the next rather than picked separately.
    #[serde(default)]
    pub hammer: bool,
}

impl Grace {
    pub const MAX_NOTES: usize = 4;

    /// A single grace note.
    pub fn single(string: u8, fret: u8, on_beat: bool, hammer: bool) -> Self {
        Grace {
            notes: vec![GraceNote { string, fret }],
            on_beat,
            hammer,
        }
    }

    pub fn is_valid(&self) -> bool {
        (1..=Grace::MAX_NOTES).contains(&self.notes.len())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
#[serde(rename_all = "snake_case")]
pub enum PickStroke {
    Down,
    Up,
}

/// How the notes of a chord are spread out in time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
#[serde(rename_all = "snake_case")]
pub enum Slap {
    Slap,
    Pop,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, ts_rs::TS,
)]
#[ts(export)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
#[serde(rename_all = "snake_case")]
pub enum Hairpin {
    Crescendo,
    Decrescendo,
}

/// The volume of a beat's notes moved by hand, as with a guitar's volume knob.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
#[serde(rename_all = "snake_case")]
pub enum Fade {
    /// From nothing up to full: the attack is not heard.
    In,
    /// From full down to nothing.
    Out,
    /// Up from nothing and down again.
    Swell,
}

/// The fingering of a chord: for each string from the highest to the lowest, the fret it is
/// stopped at, zero for an open string or -1 for one that is not played.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct ChordDiagram {
    pub frets: Vec<i8>,
}

impl ChordDiagram {
    pub const MAX_FRET: i8 = 36;

    pub fn is_valid(&self) -> bool {
        (1..=crate::Tuning::MAX_STRINGS).contains(&self.frets.len())
            && (self.frets.iter()).all(|f| (-1..=ChordDiagram::MAX_FRET).contains(f))
    }

    /// The fret the diagram's top line stands for: 1 for a chord near the nut, otherwise
    /// the lowest fret that is stopped.
    pub fn base_fret(&self) -> i8 {
        let stopped = || self.frets.iter().copied().filter(|f| *f > 0);
        match stopped().max() {
            Some(highest) if highest > 5 => stopped().min().unwrap_or(1),
            _ => 1,
        }
    }
}

/// Playing techniques attached to a whole beat.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
#[serde(default, deny_unknown_fields)]
pub struct BeatEffects {
    /// Tremolo picking: the beat is re-picked in notes of this value.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub tremolo: Option<NoteValue>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub stroke: Option<PickStroke>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub strum: Option<Strum>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub slap: Option<Slap>,
    /// Whammy bar movement over the beat.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub whammy: Option<Bend>,
    /// A change of loudness that lasts until the next one.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub dynamic: Option<Dynamic>,
    /// The beat is part of a gradual change of loudness.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub hairpin: Option<Hairpin>,
    /// A pause mark over the beat.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    #[ts(optional, as = "Option<bool>")]
    pub fermata: bool,
    /// The notes fade in, fade out or swell.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub fade: Option<Fade>,
    /// The name of the chord from this beat on, such as `Am7`.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub chord: Option<String>,
    /// How the chord is fingered, drawn as a diagram above the beat.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub diagram: Option<ChordDiagram>,
    /// The track's volume from this beat on, on the scale of the mixer.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub volume: Option<u8>,
    /// The track's pan from this beat on, on the scale of the mixer.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub pan: Option<i8>,
    /// Free text written above the beat.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub text: Option<String>,
    /// The syllable sung on this beat in each line of the lyrics; empty for none.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[ts(optional, as = "Option<Vec<String>>")]
    pub lyrics: Vec<String>,
}

impl BeatEffects {
    /// Longest chord name, text or syllable, in characters.
    pub const MAX_TEXT: usize = 120;
    pub const MAX_LYRIC_LINES: usize = 5;

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
        let short = |text: &String| text.chars().count() <= BeatEffects::MAX_TEXT;
        tremolo
            && (self.whammy.as_ref()).is_none_or(|w| w.is_valid(Bend::WHAMMY_RANGE))
            && self.chord.as_ref().is_none_or(short)
            && self.diagram.as_ref().is_none_or(ChordDiagram::is_valid)
            && self.volume.is_none_or(|v| v <= crate::Mix::MAX_VOLUME)
            && self
                .pan
                .is_none_or(|p| p.unsigned_abs() <= crate::Mix::MAX_PAN.unsigned_abs())
            && self.text.as_ref().is_none_or(short)
            && self.lyrics.len() <= BeatEffects::MAX_LYRIC_LINES
            && self.lyrics.iter().all(short)
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
