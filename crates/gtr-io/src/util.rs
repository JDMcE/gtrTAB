//! What the formats that deal in pitches and plain lengths share: where a pitch goes on
//! a fretboard, how a length is written as notes, how a pitch is named, and XML text.

use std::num::NonZeroU8;

use gtr_model::{Duration, NoteValue, Rational, Tuning, Tuplet};

/// Text as XML holds it, without the characters XML has no way to hold.
pub(crate) fn esc(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\t' | '\n' => out.push(c),
            c if c < ' ' => {}
            c => out.push(c),
        }
    }
    out
}

/// The text of a file that may start with a mark of its encoding, and may be UTF-16.
pub(crate) fn decode(bytes: &[u8]) -> String {
    let wide = |big: bool| {
        let units: Vec<u16> = (bytes[2..].chunks_exact(2))
            .map(|pair| match big {
                true => u16::from_be_bytes([pair[0], pair[1]]),
                false => u16::from_le_bytes([pair[0], pair[1]]),
            })
            .collect();
        String::from_utf16_lossy(&units)
    };
    match bytes {
        [0xFF, 0xFE, ..] => wide(false),
        [0xFE, 0xFF, ..] => wide(true),
        [0xEF, 0xBB, 0xBF, rest @ ..] => String::from_utf8_lossy(rest).into_owned(),
        _ => String::from_utf8_lossy(bytes).into_owned(),
    }
}

/// The highest fret a pitch is put on when it comes without one.
pub(crate) const MAX_FRET: u8 = 24;

/// Where a sounding pitch is played on strings tuned to `tuning` with a capo: on the
/// string that plays it nearest the nut, of those not in `taken`.
pub(crate) fn place(pitch: u8, tuning: &Tuning, capo: u8, taken: &[u8]) -> Option<(u8, u8)> {
    (tuning.strings.iter().enumerate())
        .filter_map(|(string, open)| {
            let fret = pitch.checked_sub(open.checked_add(capo)?)?;
            (fret <= MAX_FRET && !taken.contains(&(string as u8))).then_some((string as u8, fret))
        })
        .min_by_key(|(_, fret)| *fret)
}

fn tuplet(actual: u8, normal: u8) -> Option<Tuplet> {
    Some(Tuplet {
        actual: NonZeroU8::new(actual)?,
        normal: NonZeroU8::new(normal)?,
    })
}

/// The tuplets a length is tried as, commonest first.
const TUPLETS: [(u8, u8); 5] = [(3, 2), (5, 4), (6, 4), (7, 4), (9, 8)];

/// The one note that lasts exactly `length`, if there is one: plain, dotted or in one of
/// the common tuplets.
pub(crate) fn duration_of(length: Rational) -> Option<Duration> {
    let plain = |tuplet: Option<Tuplet>| {
        NoteValue::ALL.into_iter().find_map(|value| {
            (0..=2).find_map(|dots| {
                let duration = Duration {
                    value,
                    dots,
                    tuplet,
                    nested: None,
                };
                (duration.length() == length).then_some(duration)
            })
        })
    };
    plain(None).or_else(|| {
        (TUPLETS.into_iter()).find_map(|(actual, normal)| plain(tuplet(actual, normal)))
    })
}

/// Notes that together last `length`, longest first, for a stretch that no one note
/// fills: plain and dotted notes, and triplets where the length needs them. What is left
/// over once the shortest note no longer fits is dropped.
pub(crate) fn durations_for(length: Rational) -> Vec<Duration> {
    if let Some(one) = duration_of(length) {
        return vec![one];
    }
    let zero = Rational::from_integer(0);
    let mut left = length;
    let mut out = Vec::new();
    while left > zero && out.len() < 64 {
        // A length that is not so many halves of halves has a third in it somewhere.
        let dyadic = (*left.denom() as u64).is_power_of_two();
        let fitting = NoteValue::ALL.into_iter().find_map(|value| {
            let duration = Duration {
                value,
                dots: 0,
                tuplet: match dyadic {
                    true => None,
                    false => Some(Tuplet::TRIPLET),
                },
                nested: None,
            };
            (duration.length() <= left).then_some(duration)
        });
        let Some(duration) = fitting else {
            break;
        };
        left -= duration.length();
        out.push(duration);
    }
    out
}

/// A pitch by name: its letter, how far it is raised or lowered in semitones, and its
/// octave, with middle C in the fourth. `flats` chooses between the two names of a note
/// that is neither.
pub(crate) fn spell(pitch: u8, flats: bool) -> (char, i8, i32) {
    const SHARP: [(char, i8); 12] = [
        ('C', 0),
        ('C', 1),
        ('D', 0),
        ('D', 1),
        ('E', 0),
        ('F', 0),
        ('F', 1),
        ('G', 0),
        ('G', 1),
        ('A', 0),
        ('A', 1),
        ('B', 0),
    ];
    const FLAT: [(char, i8); 12] = [
        ('C', 0),
        ('D', -1),
        ('D', 0),
        ('E', -1),
        ('E', 0),
        ('F', 0),
        ('G', -1),
        ('G', 0),
        ('A', -1),
        ('A', 0),
        ('B', -1),
        ('B', 0),
    ];
    let (step, alter) = match flats {
        true => FLAT[usize::from(pitch % 12)],
        false => SHARP[usize::from(pitch % 12)],
    };
    (step, alter, i32::from(pitch / 12) - 1)
}

/// The MIDI pitch of a note by name, if it has one.
pub(crate) fn pitch_of(step: char, alter: i32, octave: i32) -> Option<u8> {
    let semitone = match step.to_ascii_uppercase() {
        'C' => 0,
        'D' => 2,
        'E' => 4,
        'F' => 5,
        'G' => 7,
        'A' => 9,
        'B' => 11,
        _ => return None,
    };
    u8::try_from((octave + 1) * 12 + semitone + alter)
        .ok()
        .filter(|pitch| *pitch < 128)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pitches_are_named_and_found_again() {
        assert_eq!(spell(60, false), ('C', 0, 4));
        assert_eq!(spell(61, false), ('C', 1, 4));
        assert_eq!(spell(61, true), ('D', -1, 4));
        assert_eq!(spell(40, false), ('E', 0, 2));
        for pitch in 0..128 {
            for flats in [false, true] {
                let (step, alter, octave) = spell(pitch, flats);
                assert_eq!(pitch_of(step, alter.into(), octave), Some(pitch));
            }
        }
        assert_eq!(pitch_of('H', 0, 4), None);
        assert_eq!(pitch_of('C', 0, 12), None);
    }

    #[test]
    fn a_pitch_goes_on_the_string_nearest_the_nut() {
        let tuning = Tuning::guitar_standard();
        assert_eq!(place(64, &tuning, 0, &[]), Some((0, 0)));
        assert_eq!(place(64, &tuning, 0, &[0]), Some((1, 5)));
        assert_eq!(place(64, &tuning, 2, &[]), Some((1, 3)));
        assert_eq!(place(39, &tuning, 0, &[]), None, "below the lowest string");
        assert_eq!(place(100, &tuning, 0, &[]), None, "above the last fret");
    }

    #[test]
    fn lengths_are_written_as_notes() {
        let length =
            |durations: &[Duration]| durations.iter().map(Duration::length).sum::<Rational>();
        assert_eq!(
            duration_of(Rational::new(3, 8)).map(|d| (d.value, d.dots)),
            Some((NoteValue::Quarter, 1))
        );
        assert_eq!(
            duration_of(Rational::new(1, 12)).and_then(|d| d.tuplet),
            Some(Tuplet::TRIPLET)
        );
        assert_eq!(duration_of(Rational::new(5, 8)), None);
        for (n, d) in [(5, 8), (5, 4), (7, 16), (1, 6), (5, 12), (11, 32), (2, 1)] {
            let wanted = Rational::new(n, d);
            assert_eq!(length(&durations_for(wanted)), wanted, "{n}/{d}");
        }
        assert!(durations_for(Rational::from_integer(0)).is_empty());
    }
}
