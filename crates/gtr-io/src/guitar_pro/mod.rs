//! Read-only Guitar Pro import. It brings in tracks, tunings, bars, time signatures, the
//! starting tempo, notes, rests and ties; everything else is skipped and listed in
//! [`Imported::skipped`].

mod gp5;
mod gp7;

use std::collections::BTreeSet;

use gtr_model::{
    Bar, Beat, Bend, BendKind, Duration, ModelError, Note, NoteValue, Score, Slide, Tuning, Tuplet,
    Voice,
};

/// A score read from another program's file.
#[derive(Debug)]
pub struct Imported {
    pub score: Score,
    /// Kinds of content the file contained that were not imported, in alphabetical order.
    pub skipped: Vec<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum ImportError {
    #[error("not a Guitar Pro file")]
    UnknownFormat,
    #[error("{0} files are not supported yet")]
    UnsupportedVersion(String),
    #[error("the file is damaged: {0}")]
    Corrupt(String),
    #[error("the imported score is inconsistent: {0}")]
    Invalid(#[from] ModelError),
}

/// Imports a Guitar Pro file, working out the version from its contents.
pub fn import(bytes: &[u8]) -> Result<Imported, ImportError> {
    let mut skipped = Skipped::default();
    let score = if bytes.starts_with(b"PK") {
        gp7::read(bytes, &mut skipped)?
    } else if bytes.starts_with(b"BCFZ") || bytes.starts_with(b"BCFS") {
        return Err(ImportError::UnsupportedVersion(
            "Guitar Pro 6 (.gpx)".into(),
        ));
    } else {
        gp5::read(bytes, &mut skipped)?
    };
    score.validate()?;
    Ok(Imported {
        score,
        skipped: skipped.0.into_iter().map(String::from).collect(),
    })
}

/// Names of the kinds of content that were left out.
#[derive(Default)]
struct Skipped(BTreeSet<&'static str>);

impl Skipped {
    fn add(&mut self, what: &'static str) {
        self.0.insert(what);
    }

    fn add_if(&mut self, condition: bool, what: &'static str) {
        if condition {
            self.add(what);
        }
    }
}

/// Builds a duration from Guitar Pro's parts, clamping what the model cannot hold.
fn duration(
    value: Option<NoteValue>,
    dots: u8,
    tuplet: Option<(u8, u8)>,
    skipped: &mut Skipped,
) -> Duration {
    skipped.add_if(value.is_none(), "notes shorter than a 64th");
    skipped.add_if(dots > Duration::MAX_DOTS, "triple dots");
    Duration {
        value: value.unwrap_or(NoteValue::SixtyFourth),
        dots: dots.min(Duration::MAX_DOTS),
        tuplet: tuplet.and_then(|(actual, normal)| {
            Some(Tuplet {
                actual: actual.try_into().ok()?,
                normal: normal.try_into().ok()?,
            })
        }),
    }
}

/// Builds a beat, dropping notes the staff cannot show and keeping one note per string.
fn beat(duration: Duration, mut notes: Vec<Note>, string_count: usize) -> Beat {
    notes.retain(|n| usize::from(n.string) < string_count);
    notes.sort_by_key(|n| n.string);
    notes.dedup_by_key(|n| n.string);
    Beat::new(duration, notes)
}

/// Builds a bar from its voices, leaving out trailing voices that have no notes.
fn bar(mut voices: Vec<Voice>) -> Bar {
    while voices.len() > 1
        && voices
            .last()
            .is_some_and(|v| v.beats.iter().all(Beat::is_rest))
    {
        voices.pop();
    }
    if voices.is_empty() {
        voices.push(Voice::default());
    }
    Bar { voices }
}

/// A tuning from pitches listed highest string first, or standard tuning if there are none.
fn tuning(strings: Vec<u8>) -> Tuning {
    if strings.is_empty() || strings.len() > Tuning::MAX_STRINGS {
        Tuning::guitar_standard()
    } else {
        Tuning::new(strings)
    }
}

/// Reduces a bend curve to the nearest preset shape. `heights` are the curve's points in
/// hundredths of a whole tone, in time order.
fn bend_from_heights(heights: &[i32]) -> Option<Bend> {
    let highest = heights.iter().copied().max()?;
    let quarter_tones = ((highest + 12) / 25).clamp(0, i32::from(Bend::MAX_QUARTER_TONES)) as u8;
    if quarter_tones == 0 {
        return None;
    }
    let prebend = heights[0] > 12;
    let release = heights[heights.len() - 1] < highest - 12;
    Some(Bend {
        kind: match (prebend, release) {
            (false, false) => BendKind::Bend,
            (false, true) => BendKind::BendRelease,
            (true, false) => BendKind::Prebend,
            (true, true) => BendKind::PrebendRelease,
        },
        quarter_tones,
    })
}

/// Guitar Pro's slide bit flags, shared by its binary and XML formats.
fn slide_from_flags(flags: u8) -> Option<Slide> {
    [
        (0x01, Slide::Shift),
        (0x02, Slide::Legato),
        (0x04, Slide::OutDown),
        (0x08, Slide::OutUp),
        (0x10, Slide::InFromBelow),
        (0x20, Slide::InFromAbove),
    ]
    .into_iter()
    .find(|(bit, _)| flags & bit != 0)
    .map(|(_, slide)| slide)
}
