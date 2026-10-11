//! Horizontal spacing. Every staff and voice shown shares one set of positions per bar, so
//! that notes sounding together line up down the page.

use std::collections::BTreeMap;

use gtr_model::{Bar, Beat, Rational, Strum, TimeSignature};

use crate::BarStatus;

pub(crate) const BAR_PAD_LEFT: f64 = 12.0;
const BAR_PAD_RIGHT: f64 = 4.0;
pub(crate) const TIME_SIG_DIGIT_WIDTH: f64 = 24.0;
/// Width of one sharp or flat in a key signature.
pub(crate) const KEY_ACCIDENTAL_WIDTH: f64 = 9.0;
/// Distance from the start of a beat's space to the centre of its notes.
const BEAT_INSET: f64 = 9.0;
const SLOT_WIDTH: f64 = 26.0;
pub(crate) const MIN_BAR_WIDTH: f64 = 76.0;
/// Extra room a repeat sign takes at the start or end of a bar.
pub(crate) const REPEAT_WIDTH: f64 = 8.0;

/// What one staff contributes to a bar's spacing.
pub(crate) struct Row<'a> {
    pub bar: &'a Bar,
    /// Room each beat needs to its left for accidentals, as `[voice][beat]`.
    pub leads: Option<&'a [Vec<f64>]>,
}

/// A bar measured at its natural width, with x positions relative to the bar's left edge.
pub(crate) struct Measured {
    /// Width of the part that does not stretch when the system is justified.
    pub fixed: f64,
    pub width: f64,
    /// Position of every beat, as `[row][voice][beat]`.
    pub beats: Vec<Vec<Vec<f64>>>,
    /// Position of the empty slot after the last beat of the voice being edited.
    pub slot: Option<f64>,
    /// How the voice being edited compares with the time signature.
    pub status: BarStatus,
    /// The time signature, if it is written at the start of this bar.
    pub time_signature: Option<TimeSignature>,
    /// The key signature, if it changes at the start of this bar.
    pub key: Option<i8>,
}

/// Width a key signature takes, including the space after it.
pub(crate) fn key_width(key: i8) -> f64 {
    match key {
        0 => 0.0,
        _ => f64::from(key.unsigned_abs()) * KEY_ACCIDENTAL_WIDTH + 6.0,
    }
}

/// Space a beat's markings need to the left of its notes: grace notes and strum arrows.
fn lead(beat: &Beat) -> f64 {
    let grace = beat.notes.iter().any(|n| n.effects.grace.is_some());
    let strum = matches!(beat.effects.strum, Some(s) if s != Strum::Rasgueado);
    let room = |needed: bool, width: f64| if needed { width } else { 0.0 };
    room(grace, 12.0) + room(strum, 6.0)
}

/// Markings that sit beside a note need the beat to be at least a little wider.
fn wide(beat: &Beat) -> bool {
    beat.notes.iter().any(|n| {
        let e = &n.effects;
        e.bend.is_some()
            || e.harmonic.is_some()
            || e.slide.is_some()
            || e.hammer
            || e.trill.is_some()
    })
}

#[derive(Default)]
struct Column {
    lead: f64,
    wide: bool,
    x: f64,
}

/// Measures one bar across the staves shown. `edit` is the row and voice being edited,
/// which decide the bar's status and where its empty slot goes.
pub(crate) fn measure(
    rows: &[Row],
    edit: (usize, usize),
    time_signature: TimeSignature,
    shown: Option<TimeSignature>,
    key: Option<i8>,
    repeat: (bool, bool),
) -> Measured {
    let mut x = BAR_PAD_LEFT + if repeat.0 { REPEAT_WIDTH } else { 0.0 };
    if let Some(key) = key {
        // A change to no sharps or flats is written as nothing, but still marks the spot.
        x += key_width(key);
    }
    if let Some(ts) = shown {
        let digits = ts.numerator.max(ts.denominator).to_string().len();
        x += digits as f64 * TIME_SIG_DIGIT_WIDTH + 6.0;
    }
    let fixed = x;

    // One column for every moment at which some beat starts.
    let zero = Rational::from_integer(0);
    let mut columns: BTreeMap<Rational, Column> = BTreeMap::new();
    let mut end = zero;
    let mut edited: Option<(Rational, bool)> = None;
    for (r, row) in rows.iter().enumerate() {
        for (v, voice) in row.bar.voices.iter().enumerate() {
            let mut time = zero;
            for (i, beat) in voice.beats.iter().enumerate() {
                let column = columns.entry(time).or_default();
                let accidentals = (row.leads)
                    .and_then(|l| l.get(v)?.get(i).copied())
                    .unwrap_or(0.0);
                column.lead = column.lead.max(lead(beat) + accidentals);
                column.wide |= wide(beat);
                time += beat.duration.length();
            }
            end = end.max(time);
            if (r, v) == edit {
                edited = Some((time, voice.beats.is_empty()));
            }
        }
    }
    let (length, empty) = edited.unwrap_or((zero, true));
    let status = match length.cmp(&time_signature.bar_length()) {
        _ if empty => BarStatus::Empty,
        std::cmp::Ordering::Less => BarStatus::Incomplete,
        std::cmp::Ordering::Equal => BarStatus::Complete,
        std::cmp::Ordering::Greater => BarStatus::Overfull,
    };
    let has_slot = matches!(status, BarStatus::Empty | BarStatus::Incomplete);
    if has_slot {
        columns.entry(length).or_default();
    }

    // Longer notes get more room, but less than proportionally.
    let times: Vec<Rational> = columns.keys().copied().collect();
    for (i, column) in columns.values_mut().enumerate() {
        let gap = times.get(i + 1).copied().unwrap_or(end) - times[i];
        let quarters = gap * 4;
        let quarters = *quarters.numer() as f64 / *quarters.denom() as f64;
        let width = if quarters > 0.0 {
            (28.0 * quarters.powf(0.55)).max(if column.wide { 30.0 } else { 20.0 })
        } else {
            // Nothing sounds here: it is the slot at the end of the bar.
            SLOT_WIDTH
        };
        column.x = x + BEAT_INSET + column.lead;
        x += width + column.lead;
    }

    let beats = rows
        .iter()
        .map(|row| {
            (row.bar.voices.iter())
                .map(|voice| {
                    let mut time = zero;
                    (voice.beats.iter())
                        .map(|beat| {
                            let at = columns[&time].x;
                            time += beat.duration.length();
                            at
                        })
                        .collect()
                })
                .collect()
        })
        .collect();
    Measured {
        fixed,
        width: (x + BAR_PAD_RIGHT + if repeat.1 { REPEAT_WIDTH } else { 0.0 }).max(MIN_BAR_WIDTH),
        beats,
        slot: has_slot.then(|| columns[&length].x),
        status,
        time_signature: shown,
        key,
    }
}
