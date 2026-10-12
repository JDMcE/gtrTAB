//! Guitar Pro import, from every version of its format: the binary files of versions 3 to
//! 5, the XML score of versions 6 to 8, in the container of version 6 or the zip archive
//! of the later ones. What a file holds that the model has no place for is left out and
//! listed in [`Imported::skipped`].

mod gp5;
mod gp7;
mod gpx;

use std::collections::BTreeSet;

use gtr_model::{
    Bar, Beat, Bend, CurvePoint, Duration, Dynamic, ModelError, Note, NoteValue, Score, Slide,
    Staff, Tuning, Tuplet, Voice,
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
        gpx::read(bytes, &mut skipped)?
    } else {
        gp5::read(bytes, &mut skipped)?
    };
    let mut score = score;
    // Guitar Pro writes a piano on strings; here it has none, and its notes are pitches.
    for track in score
        .tracks
        .iter_mut()
        .filter(|t| t.instrument.is_pitched())
    {
        for staff in &mut track.staves {
            *staff = staff.to_pitched();
        }
    }
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
    skipped.add_if(value.is_none(), "notes shorter than a 256th");
    skipped.add_if(dots > Duration::MAX_DOTS, "more than three dots");
    Duration {
        value: value.unwrap_or(NoteValue::TwoHundredFiftySixth),
        dots: dots.min(Duration::MAX_DOTS),
        tuplet: tuplet.and_then(|(actual, normal)| {
            Some(Tuplet {
                actual: actual.try_into().ok()?,
                normal: normal.try_into().ok()?,
            })
        }),
        nested: None,
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

/// Builds a pitch curve from points of `(position, height)`: positions from 0 to 60 across
/// the note, heights in hundredths of a whole tone. Returns `None` for a curve that never
/// leaves zero. The caller checks the result against the range its effect allows.
fn bend_from_points(points: impl Iterator<Item = (i32, i32)>) -> Option<Bend> {
    let mut curve: Vec<CurvePoint> = Vec::new();
    for (position, height) in points {
        let point = CurvePoint {
            position: position.clamp(0, i32::from(Bend::LENGTH)) as u8,
            value: (f64::from(height) / 25.0).round().clamp(-100.0, 100.0) as i8,
        };
        match curve.last_mut() {
            // Two points at the same moment: the later one wins.
            Some(last) if last.position >= point.position => last.value = point.value,
            _ => curve.push(point),
        }
    }
    curve.truncate(Bend::MAX_POINTS);
    curve
        .iter()
        .any(|p| p.value != 0)
        .then_some(Bend { points: curve })
}

/// Guitar Pro gives every beat a dynamic; the model only marks where the level changes,
/// starting from forte.
fn keep_dynamic_changes(staff: &mut Staff) {
    let mut current = Dynamic::F;
    for bar in &mut staff.bars {
        for (index, voice) in bar.voices.iter_mut().enumerate() {
            for beat in &mut voice.beats {
                let marked = beat.effects.dynamic.take();
                if index == 0 && !beat.is_rest() && marked.is_some_and(|d| d != current) {
                    beat.effects.dynamic = marked;
                    current = marked.unwrap_or(current);
                }
            }
        }
    }
}

/// Has the tempo change at a moment of a bar, given in whole notes from its start: the
/// bar's own tempo if that is its first beat, and otherwise a change inside it. A second
/// tempo for the same moment takes the place of the first. `gradual` has the tempo move
/// steadily from there to the next change.
fn set_tempo(
    master: &mut gtr_model::MasterBar,
    position: gtr_model::Rational,
    tempo: u32,
    gradual: bool,
) {
    let tempo = tempo.clamp(1, 1000);
    if position <= gtr_model::Rational::from_integer(0) {
        master.tempo = Some(tempo);
        master.tempo_gradual = gradual;
    } else if position < master.time_signature.bar_length() {
        let change = gtr_model::TempoChange {
            position,
            tempo,
            gradual,
        };
        match (master.tempo_changes).binary_search_by(|c| c.position.cmp(&position)) {
            Ok(at) => master.tempo_changes[at] = change,
            Err(at) => master.tempo_changes.insert(at, change),
        }
    }
}

/// Puts the words of one line of lyrics under a staff, a syllable to each beat that has
/// notes, from the bar numbered `start`. Words are split at spaces and after hyphens; a
/// plus sign joins two words under one beat, and what is in square brackets is a remark
/// that is not sung.
fn set_lyrics(staff: &mut Staff, line: usize, start: usize, words: &str) {
    if line >= gtr_model::BeatEffects::MAX_LYRIC_LINES {
        return;
    }
    let mut sung = String::new();
    let mut remark = 0;
    for c in words.chars() {
        match c {
            '[' => remark += 1,
            ']' => remark = (remark - 1).max(0),
            _ if remark > 0 => {}
            // A hyphen ends a syllable and stays with it.
            '-' => sung.push_str("- "),
            _ => sung.push(c),
        }
    }
    let mut syllables = sung
        .split_whitespace()
        .map(|syllable| syllable.replace('+', " "));
    let beats = (staff.bars.iter_mut().skip(start))
        .filter_map(|bar| bar.voices.first_mut())
        .flat_map(|voice| &mut voice.beats)
        .filter(|beat| !beat.is_rest());
    for beat in beats {
        let Some(syllable) = syllables.next() else {
            break;
        };
        if beat.effects.lyrics.len() <= line {
            beat.effects.lyrics.resize(line + 1, String::new());
        }
        beat.effects.lyrics[line] = syllable
            .chars()
            .take(gtr_model::BeatEffects::MAX_TEXT)
            .collect();
    }
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
