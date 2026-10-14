//! Reads tablature typed as plain text: the files this program writes, and tabs as they
//! are passed around, which follow no rule but habit. Lines that look like strings are
//! taken together as a line of music, cut into bars where every string has a bar line,
//! and into beats where a fret number starts. Note lengths come from a line of letters
//! above the strings if there is one; if there is none they are judged from how far
//! apart the notes are typed.

use gtr_model::{
    Bar, Beat, Bend, BendKind, Duration, Harmonic, Instrument, MasterBar, Note, NoteValue,
    Rational, Score, ScoreInfo, Slide, Staff, TimeSignature, Track, Tuning, Tuplet, Vibrato, Voice,
};

use super::write::{LENGTHS, PIECES, pow2};
use crate::util::{durations_for, pitch_of};
use crate::{ImportError, Skipped};

/// The most bars a file is read as.
const MAX_BARS: usize = 5000;

/// The names drum tabs give the pieces of a kit, beyond the ones written here, with the
/// General MIDI number of each.
const DRUM_NAMES: [(&str, u8); 24] = [
    ("KD", 36),
    ("K", 36),
    ("B", 36),
    ("BA", 36),
    ("SN", 38),
    ("S", 38),
    ("H", 42),
    ("HP", 44),
    ("CR", 49),
    ("C", 49),
    ("C1", 49),
    ("RC", 51),
    ("R", 51),
    ("RI", 51),
    ("HT", 50),
    ("T", 50),
    ("MT", 47),
    ("LT", 45),
    ("F", 43),
    ("F1", 43),
    ("CB", 56),
    ("SP", 55),
    ("CY", 49),
    ("BS", 36),
];

/// Names that only a drum tab has: a line of music with one of these is a kit's.
const DRUMS_ONLY: [&str; 16] = [
    "HH", "BD", "SD", "CC", "RD", "HO", "HF", "FT", "KD", "SN", "CR", "RC", "HT", "MT", "LT", "CY",
];

fn piece_of(label: &str) -> Option<u8> {
    let name = label.to_uppercase();
    (PIECES
        .iter()
        .find(|piece| piece.1 == name)
        .map(|piece| piece.0))
    .or_else(|| {
        DRUM_NAMES
            .iter()
            .find(|piece| piece.0 == name)
            .map(|piece| piece.1)
    })
}

/// One string's line of a line of music.
struct Row {
    label: String,
    /// The column the music starts in, and its characters from there.
    start: usize,
    content: Vec<char>,
}

/// What a line is, if it is a string's: a short name, or none, and then mostly dashes.
fn row(line: &str) -> Option<Row> {
    let chars: Vec<char> = line.trim_end().chars().collect();
    let lead = chars.iter().take_while(|c| c.is_whitespace()).count();
    let start = lead + chars[lead..].iter().position(|c| matches!(c, '|' | '-'))?;
    let label: String = chars[lead..start].iter().collect();
    let label = label.trim().trim_end_matches(':').to_owned();
    let named =
        label.chars().count() <= 3 && label.chars().all(|c| c.is_ascii_alphanumeric() || c == '#');
    let content = &chars[start..];
    let dashes = content.iter().filter(|c| **c == '-').count();
    let allowed = |c: &char| {
        c.is_ascii_digit() || "-|:hpbr/\\~xo()[]<>tsv*=^.,gf#@ ".contains(c.to_ascii_lowercase())
    };
    (named && content.len() >= 4 && dashes * 3 >= content.len() && content.iter().all(allowed))
        .then(|| Row {
            label,
            start,
            content: content.to_vec(),
        })
}

/// Words on a line with the column each starts in.
fn words(line: &str) -> Vec<(usize, String)> {
    let mut out: Vec<(usize, String)> = Vec::new();
    let mut open = false;
    for (column, c) in line.chars().enumerate() {
        match (c.is_whitespace(), open) {
            (true, _) => open = false,
            (false, true) => out.last_mut().expect("a word is open").1.push(c),
            (false, false) => {
                out.push((column, c.to_string()));
                open = true;
            }
        }
    }
    out
}

/// A note's length from its letter, its dots and the number of its tuplet.
fn length_of(mark: &str) -> Option<Duration> {
    let mut chars = mark.chars();
    let letter = chars.next()?.to_ascii_uppercase();
    let value = LENGTHS.iter().find(|(_, l)| *l == letter)?.0;
    let rest = chars.as_str();
    let dots = rest.chars().take_while(|c| *c == '.').count();
    let tuplet = &rest[dots..];
    let tuplet = match tuplet {
        "" => None,
        number => {
            let actual: u8 = number.parse().ok().filter(|n| (2..=15).contains(n))?;
            Some(Tuplet {
                actual: actual.try_into().ok()?,
                normal: pow2(actual).try_into().ok()?,
            })
        }
    };
    (dots <= usize::from(Duration::MAX_DOTS)).then_some(Duration {
        value,
        dots: dots as u8,
        tuplet,
        nested: None,
    })
}

/// A line of music: its strings, and the lines written over and under them.
struct System {
    rows: Vec<Row>,
    lengths: Vec<(usize, Duration)>,
    chords: Vec<(usize, String)>,
    lyrics: Vec<(usize, String)>,
    /// What was said before it, of the bars it starts with.
    section: Option<String>,
    time: Option<TimeSignature>,
    tempo: Option<u32>,
}

/// A track as the file has it: what was said of it, and its lines of music.
#[derive(Default)]
struct Part {
    name: Option<String>,
    instrument: Option<Instrument>,
    tuning: Option<Vec<u8>>,
    capo: u8,
    systems: Vec<System>,
}

/// A line that says something by name, as `Tempo: 120` does.
fn keyed(line: &str) -> Option<(String, &str)> {
    let (key, value) = line.split_once([':', '='])?;
    let key = key.trim().to_lowercase();
    const KEYS: [&str; 15] = [
        "title",
        "subtitle",
        "artist",
        "album",
        "words",
        "music",
        "tabbed by",
        "copyright",
        "tempo",
        "bpm",
        "capo",
        "tuning",
        "time",
        "track",
        "key",
    ];
    KEYS.contains(&key.as_str()).then_some((key, value.trim()))
}

/// Open strings from their names, lowest first: with octaves they are exact, and without
/// each is the note of that name nearest where the string usually is.
fn tuning_of(named: &str, usual: &[u8]) -> Option<Vec<u8>> {
    let names: Vec<&str> = named
        .split([' ', ',', '-'])
        .filter(|n| !n.is_empty())
        .collect();
    if names.is_empty() || names.len() > Tuning::MAX_STRINGS {
        return None;
    }
    let count = names.len();
    let mut strings = Vec::new();
    for (index, name) in names.iter().enumerate() {
        let mut chars = name.chars();
        let step = chars.next().filter(|c| c.is_ascii_alphabetic())?;
        let rest = chars.as_str();
        let (alter, rest) = match rest.chars().next() {
            Some('#') => (1, &rest[1..]),
            Some('b') => (-1, &rest[1..]),
            _ => (0, rest),
        };
        let pitch = match rest.parse::<i32>() {
            Ok(octave) => pitch_of(step, alter, octave)?,
            Err(_) if rest.is_empty() => {
                // The string this is, counted from the highest, and where it usually is.
                let usual = i32::from(*usual.get(count - 1 - index).or(usual.last())?);
                let class = i32::from(pitch_of(step, alter, 4)?) % 12;
                let below = usual - (usual - class).rem_euclid(12);
                let nearest = if usual - below > 6 { below + 12 } else { below };
                u8::try_from(nearest).ok().filter(|p| *p < 128)?
            }
            Err(_) => return None,
        };
        strings.push(pitch);
    }
    strings.reverse();
    Some(strings)
}

/// The strings an instrument with `count` of them usually has, highest first.
fn usual(count: usize) -> Vec<u8> {
    match count {
        4 => Tuning::bass_standard().strings,
        5 => Tuning::bass_5_standard().strings,
        7 => Tuning::guitar_7_standard().strings,
        8.. => {
            let mut strings = Tuning::guitar_8_standard().strings;
            // Further strings go on down in fourths.
            while strings.len() < count.min(Tuning::MAX_STRINGS) {
                strings.push(strings.last().map_or(25, |low| low.saturating_sub(5)));
            }
            strings
        }
        count => Tuning::guitar_standard().strings[..count.clamp(1, 6)].to_vec(),
    }
}

pub(super) fn read(text: &str, skipped: &mut Skipped) -> Result<Score, ImportError> {
    let lines: Vec<String> = text
        .lines()
        .map(|line| line.replace('\t', "    "))
        .collect();
    let rows: Vec<Option<Row>> = lines.iter().map(|line| row(line)).collect();
    let mut info = ScoreInfo::default();
    let mut tempo: Option<u32> = None;
    let mut parts: Vec<Part> = vec![Part::default()];
    // What has been said since the last line of music, for the next.
    let (mut section, mut time, mut new_tempo) = (None, None, None);
    let mut any_keyed = false;
    let mut index = 0;
    while index < lines.len() {
        let line = lines[index].trim();
        if rows[index].is_some() {
            // The strings of a line of music are the lines of this kind that follow on.
            let first = index;
            let mut taken = Vec::new();
            while index < lines.len() && rows[index].is_some() {
                taken.push(index);
                index += 1;
            }
            if taken.len() < 2 {
                continue;
            }
            // Over it, lengths and above them the names of chords; under it, words.
            let is_lengths = |line: &str| {
                let marks = words(line);
                !marks.is_empty() && marks.iter().all(|(_, mark)| length_of(mark).is_some())
            };
            let above = |back: usize| first.checked_sub(back).map(|i| lines[i].as_str());
            let (lengths, next) = match above(1).filter(|line| is_lengths(line)) {
                Some(line) => (words(line), 2),
                None => (Vec::new(), 1),
            };
            let chordlike = |line: &&str| {
                let found = words(line);
                !found.is_empty()
                    && keyed(line).is_none()
                    && found.iter().all(|(_, word)| {
                        word.starts_with(|c: char| ('A'..='G').contains(&c)) || word == "N.C."
                    })
            };
            let chords = above(next).filter(chordlike).map(words).unwrap_or_default();
            let below = lines.get(index).map(String::as_str).filter(|line| {
                !line.trim().is_empty()
                    && keyed(line).is_none()
                    && !line.trim().starts_with('[')
                    && !is_lengths(line)
            });
            let lyrics = below.map(words).unwrap_or_default();
            if below.is_some() {
                index += 1;
            }
            let rows: Vec<Row> = taken.into_iter().filter_map(|i| row(&lines[i])).collect();
            let part = parts.last_mut().expect("there is always a part");
            part.systems.push(System {
                rows,
                lengths: (lengths.into_iter())
                    .filter_map(|(column, mark)| Some((column, length_of(&mark)?)))
                    .collect(),
                chords,
                lyrics,
                section: section.take(),
                time: time.take(),
                tempo: new_tempo.take(),
            });
            continue;
        }
        index += 1;
        if let Some((key, value)) = keyed(line) {
            any_keyed = true;
            let said = value.to_owned();
            let part = parts.last_mut().expect("there is always a part");
            match key.as_str() {
                "title" => info.title = said,
                "subtitle" => info.subtitle = said,
                "artist" => info.artist = said,
                "album" => info.album = said,
                "words" => info.words = said,
                "music" => info.music = said,
                "tabbed by" => info.transcriber = said,
                "copyright" => info.copyright = said,
                "tempo" | "bpm" => {
                    let digits: String = (value.chars().skip_while(|c| !c.is_ascii_digit()))
                        .take_while(char::is_ascii_digit)
                        .collect();
                    if let Some(said) = digits
                        .parse::<u32>()
                        .ok()
                        .filter(|t| (1..=1000).contains(t))
                    {
                        match tempo {
                            None => tempo = Some(said),
                            Some(_) => new_tempo = Some(said),
                        }
                    }
                }
                "capo" => {
                    let digits: String = value.chars().filter(char::is_ascii_digit).collect();
                    part.capo = digits.parse::<u8>().unwrap_or(0).min(24);
                }
                "tuning" => {
                    part.tuning = tuning_of(value, &usual(value.split_whitespace().count()))
                }
                "time" => {
                    time = value.split_once('/').and_then(|(numerator, denominator)| {
                        let time = TimeSignature {
                            numerator: numerator.trim().parse().ok()?,
                            denominator: denominator.trim().parse().ok()?,
                        };
                        time.is_valid().then_some(time)
                    });
                }
                "track" => {
                    // A name, and after it in brackets what plays it.
                    let (name, instrument) = match value.rsplit_once('(') {
                        Some((name, kind)) => {
                            let kind = kind.trim_end_matches(')').trim();
                            let known = (Instrument::ALL.into_iter())
                                .find(|i| i.name().eq_ignore_ascii_case(kind));
                            match known {
                                Some(_) => (name.trim(), known),
                                None => (value, None),
                            }
                        }
                        None => (value, None),
                    };
                    if !part.systems.is_empty() || part.name.is_some() {
                        parts.push(Part::default());
                    }
                    let part = parts.last_mut().expect("there is always a part");
                    part.name = Some(name.chars().take(120).collect());
                    part.instrument = instrument;
                }
                _ => {}
            }
        } else if line.starts_with('[') && line.ends_with(']') && line.len() > 2 {
            let name: String = line[1..line.len() - 1].trim().chars().take(120).collect();
            section = Some(name).filter(|name| !name.is_empty());
        }
    }
    parts.retain(|part| !part.systems.is_empty());
    if parts.is_empty() {
        return Err(ImportError::UnknownFormat);
    }
    // A tab with nothing said by name is headed by its title, if by anything.
    if !any_keyed
        && let Some(first) = lines.iter().map(|l| l.trim()).find(|l| !l.is_empty())
        && row(first).is_none()
        && first.chars().count() <= 80
    {
        info.title = first.to_owned();
    }

    let mut masters: Vec<MasterBar> = Vec::new();
    let mut said: Vec<bool> = Vec::new();
    let mut tracks = Vec::new();
    let mut typed_at = Vec::new();
    for part in &parts {
        let (track, places) = read_part(part, &mut masters, &mut said, skipped)?;
        tracks.push(track);
        typed_at.push(places);
    }
    if masters.is_empty() {
        return Err(ImportError::UnknownFormat);
    }
    // Every track has every bar, and a time lasts until the next.
    let mut time = TimeSignature::COMMON;
    for (master, said) in masters.iter_mut().zip(&said) {
        match said {
            true => time = master.time_signature,
            false => master.time_signature = time,
        }
    }
    for track in &mut tracks {
        track.staves[0].bars.resize_with(masters.len(), Bar::empty);
    }
    finish(&mut tracks, &typed_at, &masters);
    Ok(Score {
        info,
        tempo: tempo.unwrap_or(120),
        master_bars: masters,
        tracks,
        effects: Vec::new(),
        audio: None,
        style: Default::default(),
    })
}

/// A note as it was typed, before its bar's rhythm is known.
struct Typed {
    column: usize,
    row: usize,
    note: Note,
    /// How many characters its fret took.
    digits: usize,
    /// It was typed in round brackets, which may mean tied and may mean quiet.
    bracketed: bool,
}

/// Reads the notes typed on one string between two bar lines.
fn typed(content: &[char], from: usize, to: usize, row: usize, start: usize, out: &mut Vec<Typed>) {
    let at = |index: usize| content.get(index).copied().filter(|_| index < to);
    let mut index = from;
    while index < to {
        let begins = index;
        let mut note = Note::new(row as u8, 0);
        let mut bracketed = false;
        // What comes before the fret: a tap, a slide in.
        loop {
            match at(index).map(|c| c.to_ascii_lowercase()) {
                Some('t') if at(index + 1).is_some_and(|c| c.is_ascii_digit()) => {
                    note.effects.tapped = true;
                }
                Some('/') if at(index + 1).is_some_and(|c| c.is_ascii_digit()) => {
                    note.effects.slide = Some(Slide::InFromBelow);
                }
                Some('\\') if at(index + 1).is_some_and(|c| c.is_ascii_digit()) => {
                    note.effects.slide = Some(Slide::InFromAbove);
                }
                _ => break,
            }
            index += 1;
        }
        let close = match at(index) {
            Some('(') => Some(')'),
            Some('[') => Some(']'),
            Some('<') => Some('>'),
            _ => None,
        };
        if close.is_some() {
            index += 1;
        }
        // The fret: one or two figures, or a cross for a note that is dead.
        let number = |index: usize| -> (Option<u8>, usize) {
            let digit = |i: usize| at(i).and_then(|c| c.to_digit(10)).map(|d| d as u8);
            match (digit(index), digit(index + 1)) {
                (Some(tens), Some(ones)) if tens * 10 + ones <= 36 => (Some(tens * 10 + ones), 2),
                (Some(one), _) => (Some(one), 1),
                _ => (None, 0),
            }
        };
        let (fret, digits) = number(index);
        match (fret, at(index).map(|c| c.to_ascii_lowercase())) {
            (Some(fret), _) => {
                note.fret = fret;
                index += digits;
            }
            (None, Some('x')) => {
                note.effects.dead = true;
                index += 1;
            }
            _ => {
                index = begins + 1;
                continue;
            }
        }
        match close {
            Some(')') => bracketed = true,
            Some(']') => note.effects.ghost = true,
            Some('>') => note.effects.harmonic = Some(Harmonic::Natural),
            _ => {}
        }
        if close.is_some() && at(index) == close {
            index += 1;
        }
        // What comes after it, as long as it goes on.
        loop {
            match at(index).map(|c| c.to_ascii_lowercase()) {
                Some('h' | 'p') => note.effects.hammer = true,
                Some('~' | 'v') => note.effects.vibrato = Some(Vibrato::Slight),
                Some('/' | 's') => note.effects.slide = note.effects.slide.or(Some(Slide::OutUp)),
                Some('\\') => note.effects.slide = note.effects.slide.or(Some(Slide::OutDown)),
                Some('b') => {
                    // Bent to the note of a fret, or a whole tone if it does not say; and
                    // perhaps let down again.
                    let (reach, digits) = number(index + 1);
                    let tones =
                        reach.map_or(4, |reach| (i32::from(reach) - i32::from(note.fret)) * 2);
                    index += digits;
                    let released = at(index + 1).map(|c| c.to_ascii_lowercase()) == Some('r');
                    if released {
                        index += 1 + number(index + 2).1;
                    }
                    let kind = match released {
                        true => BendKind::BendRelease,
                        false => BendKind::Bend,
                    };
                    note.effects.bend = Some(Bend::preset(kind, tones.clamp(1, 12) as i8));
                }
                _ => break,
            }
            index += 1;
        }
        out.push(Typed {
            column: start + begins,
            row,
            note,
            digits,
            bracketed,
        });
    }
}

fn read_part(
    part: &Part,
    masters: &mut Vec<MasterBar>,
    said: &mut Vec<bool>,
    skipped: &mut Skipped,
) -> Result<(Track, Vec<Option<Typing>>), ImportError> {
    // The part is of the kind most of its lines are: a kit's if they are named as a
    // kit's pieces, and otherwise strings, as many as most lines have.
    let is_kit = |system: &System| {
        (system.rows.iter()).any(|row| DRUMS_ONLY.contains(&row.label.to_uppercase().as_str()))
    };
    let drums = part.instrument.map_or_else(
        || part.systems.iter().filter(|s| is_kit(s)).count() * 2 > part.systems.len(),
        Instrument::is_percussion,
    );
    let mut counts: Vec<usize> = (part.systems.iter())
        .filter(|system| is_kit(system) == drums || part.instrument.is_some())
        .map(|system| system.rows.len())
        .collect();
    counts.sort_unstable();
    let strings = (counts.iter().copied())
        .max_by_key(|count| counts.iter().filter(|c| *c == count).count())
        .unwrap_or(6)
        .min(Tuning::MAX_STRINGS);

    // The tuning: what the part says, or what its strings are named, or the usual one.
    let named = (part.systems.iter())
        .find(|system| !drums && system.rows.len() == strings)
        .and_then(|system| {
            let labels: Vec<&str> = system
                .rows
                .iter()
                .rev()
                .map(|row| row.label.as_str())
                .collect();
            (labels.iter().all(|label| !label.is_empty())).then(|| labels.join(" "))
        })
        .and_then(|names| tuning_of(&names, &usual(strings)));
    let tuning = match drums {
        true => Tuning::new([0; 5]),
        false => Tuning::new(
            (part.tuning.clone().filter(|tuning| tuning.len() == strings))
                .or(named)
                .unwrap_or_else(|| usual(strings)),
        ),
    };
    let lowest = tuning.strings.last().copied().unwrap_or(40);
    let instrument = part.instrument.unwrap_or(match (drums, strings, lowest) {
        (true, ..) => Instrument::Drums,
        (_, 4 | 5, ..36) => Instrument::Bass,
        _ => Instrument::default(),
    });
    let mut staff = Staff {
        tuning,
        capo: if drums { 0 } else { part.capo },
        bars: Vec::new(),
    };
    let mut typed_at = Vec::new();

    for system in &part.systems {
        if !drums && system.rows.len() != strings
            || drums != is_kit(system) && part.instrument.is_none()
        {
            skipped.add("lines of music for another instrument than the rest");
            continue;
        }
        // Bar lines are the columns where every string has one.
        let begin = system.rows.iter().map(|row| row.start).min().unwrap_or(0);
        let end = system
            .rows
            .iter()
            .map(|row| row.start + row.content.len())
            .max()
            .unwrap_or(0);
        let cell = |row: &Row, column: usize| {
            column
                .checked_sub(row.start)
                .and_then(|i| row.content.get(i))
                .copied()
                .unwrap_or('-')
        };
        let every = |column: usize, wanted: fn(char) -> bool| {
            system.rows.iter().all(|row| wanted(cell(row, column)))
        };
        let mut edges: Vec<usize> = (begin..end)
            .filter(|column| every(*column, |c| c == '|'))
            .collect();
        if edges.first() != Some(&begin) {
            edges.insert(0, begin.saturating_sub(1));
        }
        edges.push(end);
        let pieces: Vec<Option<u8>> = system.rows.iter().map(|row| piece_of(&row.label)).collect();
        let mut first = true;
        for edge in edges.windows(2) {
            let (mut from, mut to) = (edge[0] + 1, edge[1]);
            if from >= to {
                continue;
            }
            // A count after a repeat's end: how many times in all.
            let label: String = (from..to)
                .map(|column| cell(&system.rows[0], column))
                .collect();
            if let Some(count) = label.strip_prefix('x').and_then(|n| n.parse::<u8>().ok())
                && let Some(last) = masters.get_mut(staff.bars.len().wrapping_sub(1))
                && last.repeat_count > 0
            {
                last.repeat_count = count.max(2);
                continue;
            }
            // A repeat is marked with colons or stars inside the bar line, on every
            // string or on a few of them.
            let marked = |column: usize| {
                let marks = (system.rows.iter())
                    .filter(|row| matches!(cell(row, column), ':' | '*'))
                    .count();
                marks >= 2.min(system.rows.len()) && every(column, |c| matches!(c, ':' | '*' | '-'))
            };
            let repeats = marked(from);
            let closes = marked(to - 1);
            from += usize::from(repeats);
            to -= usize::from(closes && to > from);
            let mut notes = Vec::new();
            for (index, row) in system.rows.iter().enumerate() {
                let within =
                    |column: usize| column.saturating_sub(row.start).min(row.content.len());
                if drums {
                    // A hit is any mark on the line of a piece; an `o` on the hi-hat's
                    // line is the hi-hat open.
                    for column in from..to {
                        let mark = cell(row, column);
                        let Some(piece) = pieces[index].filter(|_| mark.is_ascii_alphanumeric())
                        else {
                            continue;
                        };
                        let piece = match (piece, mark.to_ascii_lowercase()) {
                            (42, 'o') => 46,
                            (piece, _) => piece,
                        };
                        notes.push(Typed {
                            column,
                            row: index,
                            note: Note::new(0, piece),
                            digits: 1,
                            bracketed: false,
                        });
                    }
                } else {
                    typed(
                        &row.content,
                        within(from),
                        within(to),
                        index,
                        row.start,
                        &mut notes,
                    );
                }
            }
            let bar = staff.bars.len();
            if bar >= MAX_BARS {
                return Err(ImportError::Corrupt("it is longer than any piece".into()));
            }
            // An empty stretch between two bar lines, as at a double bar, is no bar.
            let blank = (from..to).all(|column| every(column, |c| matches!(c, '-' | ' ' | '|')));
            if notes.is_empty() && blank && to - from < 3 {
                continue;
            }
            if masters.len() <= bar {
                masters.push(MasterBar::new(TimeSignature::COMMON));
                said.push(false);
                let master = &mut masters[bar];
                master.repeat_start = repeats;
                master.repeat_count = if closes { 2 } else { 0 };
                if first {
                    master.section = system.section.clone();
                    master.tempo = system.tempo.filter(|_| bar > 0);
                    if let Some(time) = system.time {
                        master.time_signature = time;
                        said[bar] = true;
                    }
                }
            }
            first = false;
            let (read, places) = read_bar(system, notes, (from, to), drums, staff.places());
            staff.bars.push(read);
            typed_at.push(places);
        }
    }
    let name = part
        .name
        .clone()
        .unwrap_or_else(|| instrument.name().to_owned());
    Ok((Track::new(name, instrument, vec![staff]), typed_at))
}

/// A beat with the column it was typed in, and its length if a letter gave it one.
struct Placed {
    column: usize,
    beat: Beat,
    known: bool,
}

/// Where the beats of a bar were typed, for a bar that has no letters over it to give
/// their lengths: the lengths are judged from this once the bar's time is known.
struct Typing {
    /// The column each beat starts in, counted from the bar line.
    columns: Vec<usize>,
    /// How many columns the bar has.
    width: usize,
}

/// Reads a bar from the notes typed in it, with where they were typed if nothing gives
/// their lengths.
fn read_bar(
    system: &System,
    mut notes: Vec<Typed>,
    (from, to): (usize, usize),
    drums: bool,
    places: usize,
) -> (Bar, Option<Typing>) {
    notes.sort_by_key(|typed| (typed.column, typed.row));
    // Notes typed in the same column are one beat, and so is a note typed a column
    // late beside a fret of two figures.
    let mut beats: Vec<Placed> = Vec::new();
    let mut rows: Vec<Vec<usize>> = Vec::new();
    let mut wide: Vec<bool> = Vec::new();
    for typed in notes {
        let joins = match beats.last() {
            Some(last) if last.column == typed.column => true,
            Some(last) => {
                let (last_rows, last_wide) =
                    (rows.last().expect("as many"), wide.last().expect("as many"));
                last.column + 1 == typed.column
                    && (*last_wide || typed.digits == 2)
                    && !last_rows.contains(&typed.row)
                    && !drums
            }
            None => false,
        };
        if !joins {
            beats.push(Placed {
                column: typed.column,
                beat: Beat::rest(Duration::new(NoteValue::Eighth)),
                known: false,
            });
            rows.push(Vec::new());
            wide.push(false);
        }
        let beat = &mut beats.last_mut().expect("just pushed").beat;
        let mut note = typed.note;
        if drums {
            note.string = beat.notes.len() as u8;
        }
        // Round brackets are taken as a tie until the note before is known.
        note.tied |= typed.bracketed;
        if usize::from(note.string) < places && beat.notes.iter().all(|n| n.string != note.string) {
            beat.notes.push(note);
            beat.notes.sort_by_key(|n| n.string);
        }
        rows.last_mut().expect("as many").push(typed.row);
        *wide.last_mut().expect("as many") |= typed.digits == 2;
    }

    // The letters over the bar give the lengths, and a letter over nothing is a rest.
    let within = |column: usize| (from.saturating_sub(1)..to).contains(&column);
    let lettered = system.lengths.iter().any(|(column, _)| within(*column));
    for (column, duration) in system.lengths.iter().filter(|(column, _)| within(*column)) {
        let nearest = (beats.iter_mut())
            .filter(|placed| placed.column.abs_diff(*column) <= 1 && !placed.known)
            .min_by_key(|placed| placed.column.abs_diff(*column));
        match nearest {
            Some(placed) => {
                placed.beat.duration = *duration;
                placed.known = true;
            }
            None => {
                let at = beats.partition_point(|placed| placed.column < *column);
                beats.insert(
                    at,
                    Placed {
                        column: *column,
                        beat: Beat::rest(*duration),
                        known: true,
                    },
                );
            }
        }
    }
    fn nearest(beats: &mut [Placed], column: usize) -> Option<&mut Beat> {
        (beats.iter_mut())
            .filter(|placed| placed.column.abs_diff(column) <= 2)
            .min_by_key(|placed| placed.column.abs_diff(column))
            .map(|placed| &mut placed.beat)
    }
    for (column, name) in system.chords.iter().filter(|(column, _)| within(*column)) {
        if let Some(beat) = nearest(&mut beats, *column) {
            beat.effects.chord = Some(name.chars().take(120).collect());
        }
    }
    for (column, sung) in system.lyrics.iter().filter(|(column, _)| within(*column)) {
        if let Some(beat) = nearest(&mut beats, *column).filter(|beat| !beat.is_rest()) {
            beat.effects.lyrics = vec![sung.chars().take(120).collect()];
        }
    }
    // A beat no letter was over, in a bar that has letters, is as long as an eighth.
    for placed in beats.iter_mut().filter(|placed| !placed.known) {
        placed.beat.duration = Duration::new(NoteValue::Eighth);
    }
    let typed_at = (!lettered).then(|| Typing {
        columns: (beats.iter())
            .map(|placed| placed.column.saturating_sub(from))
            .collect(),
        width: (to - from).max(1),
    });
    let bar = Bar {
        voices: vec![Voice {
            beats: beats.into_iter().map(|placed| placed.beat).collect(),
        }],
    };
    (bar, typed_at)
}

/// Gives the bars typed without letters for their lengths the lengths their spacing
/// suggests, settles what round brackets meant, and tells a slide to the next note from
/// a slide away.
fn finish(tracks: &mut [Track], typed_at: &[Vec<Option<Typing>>], masters: &[MasterBar]) {
    for (track, typed_at) in tracks.iter_mut().zip(typed_at) {
        let drums = track.instrument.is_percussion();
        let staff = &mut track.staves[0];
        for ((bar, master), places) in staff.bars.iter_mut().zip(masters).zip(typed_at) {
            if let Some(places) = places {
                let beats = std::mem::take(&mut bar.voices[0].beats);
                let length = master.time_signature.bar_length();
                bar.voices[0].beats = spaced(beats, places, length, drums);
            }
        }
        if drums {
            continue;
        }
        // Along each string: a bracketed note at the fret of the note before is tied to
        // it, and a slide with a note right after it goes to that note.
        let mut previous: Vec<Option<u8>> = vec![None; staff.tuning.strings.len()];
        let mut sliding: Vec<Option<(usize, usize, usize)>> =
            vec![None; staff.tuning.strings.len()];
        for bar in 0..staff.bars.len() {
            for index in 0..staff.bars[bar].voices[0].beats.len() {
                let count = staff.bars[bar].voices[0].beats[index].notes.len();
                let mut on: Vec<u8> = Vec::new();
                for n in 0..count {
                    let note = &mut staff.bars[bar].voices[0].beats[index].notes[n];
                    let string = usize::from(note.string);
                    on.push(note.string);
                    if note.tied && previous[string] != Some(note.fret) {
                        // Round brackets with no note of the same fret before them to
                        // be tied to: a note played quietly.
                        note.tied = false;
                        note.effects.ghost = true;
                    }
                    previous[string] = Some(note.fret);
                    let slides = matches!(note.effects.slide, Some(Slide::OutUp | Slide::OutDown));
                    if let Some((b, i, m)) = sliding[string].take() {
                        staff.bars[b].voices[0].beats[i].notes[m].effects.slide =
                            Some(Slide::Shift);
                    }
                    if slides {
                        sliding[string] = Some((bar, index, n));
                    }
                }
                // A slide not followed at once on its string is a slide away.
                for (string, waiting) in sliding.iter_mut().enumerate() {
                    if waiting.is_some_and(|(b, i, _)| (b, i) != (bar, index))
                        && !on.contains(&(string as u8))
                    {
                        *waiting = None;
                    }
                }
            }
        }
    }
}

/// Lengths for the beats of a bar typed without them, from how far apart they were
/// typed. The narrowest gap between two beats is taken as one step, every other gap as
/// so many steps, and a step as the note value that makes the steps fill the bar, or
/// the longest plain one that lets them all in.
fn spaced(beats: Vec<Beat>, typing: &Typing, length: Rational, drums: bool) -> Vec<Beat> {
    let Some(last) = typing.columns.last() else {
        return beats;
    };
    let gaps: Vec<usize> = typing
        .columns
        .windows(2)
        .map(|pair| pair[1].saturating_sub(pair[0]))
        .collect();
    let trailing = typing.width.saturating_sub(*last).max(1);
    let step = gaps
        .iter()
        .copied()
        .filter(|gap| *gap > 0)
        .min()
        .unwrap_or(trailing);
    let steps = |gap: usize| ((gap + step / 2) / step).max(1) as i64;
    let mut each: Vec<i64> = gaps.iter().map(|gap| steps(*gap)).collect();
    each.push(steps(trailing));
    // A dash before the first note is how a bar is started; more than a step's worth of
    // them is a rest.
    let lead = typing.columns[0].saturating_sub(1);
    let rest = if lead >= 2 * step { steps(lead) } else { 0 };
    let total: i64 = rest + each.iter().sum::<i64>();
    let exact = length / total;
    let plain = |value: &Rational| *value.numer() == 1 && (*value.denom() as u64).is_power_of_two();
    let unit = match plain(&exact) && exact >= Rational::new(1, 64) {
        true => exact,
        false => (0..=5)
            .map(|power| Rational::new(1, 1 << power))
            .find(|value| value * total <= length)
            .unwrap_or(Rational::new(1, 32)),
    };
    let mut out: Vec<Beat> = (durations_for(unit * rest).into_iter())
        .map(Beat::rest)
        .collect();
    let alone = beats.len() == 1 && rest == 0;
    for (beat, steps) in beats.into_iter().zip(each) {
        // A note alone in its bar lasts the bar.
        let lasts = if alone { length } else { unit * steps };
        for (piece, duration) in durations_for(lasts).into_iter().enumerate() {
            let mut part = beat.clone();
            part.duration = duration;
            if piece > 0 {
                // What is left of a length no one note has: the same notes, tied on.
                part.effects = Default::default();
                part.notes.iter_mut().for_each(|note| {
                    *note = Note {
                        tied: true,
                        ..Note::new(note.string, note.fret)
                    };
                });
                if drums {
                    part.notes.clear();
                }
            }
            out.push(part);
        }
    }
    out
}
