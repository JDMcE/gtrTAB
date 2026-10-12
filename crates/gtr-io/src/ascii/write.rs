//! Tablature as plain text: a line for each string, a line of note lengths above them
//! and the signs guitarists type for the techniques. A drum track is written as drum
//! tab, a line for each piece of the kit. Standard notation has no place in it, so a
//! track for an instrument without strings is left out.

use std::fmt::Write as _;

use gtr_model::{
    Beat, BendKind, Duration, Harmonic, MasterBar, Note, NoteValue, Rational, Score, Slide, Staff,
    Track,
};

use crate::Skipped;

/// Characters a line of tablature runs to before the next line of bars is started.
const WIDTH: usize = 96;
/// Characters a quarter note is given, where its notes do not need more.
const QUARTER: i64 = 4;

/// The letter a note value is written as in the line of lengths.
pub(super) const LENGTHS: [(NoteValue, char); 7] = [
    (NoteValue::Whole, 'W'),
    (NoteValue::Half, 'H'),
    (NoteValue::Quarter, 'Q'),
    (NoteValue::Eighth, 'E'),
    (NoteValue::Sixteenth, 'S'),
    (NoteValue::ThirtySecond, 'T'),
    (NoteValue::SixtyFourth, 'X'),
];

/// The pieces of a drum kit as drum tab names them, and whether each is a cymbal, struck
/// with an `x`, or a drum, struck with an `o`.
pub(super) const PIECES: [(u8, &str, bool); 17] = [
    (49, "CC", true),
    (57, "C2", true),
    (55, "SC", true),
    (52, "CH", true),
    (51, "RD", true),
    (53, "RB", true),
    (42, "HH", true),
    (46, "HO", true),
    (44, "HF", true),
    (50, "T1", false),
    (48, "T2", false),
    (47, "T3", false),
    (45, "T4", false),
    (43, "FT", false),
    (41, "F2", false),
    (38, "SD", false),
    (36, "BD", false),
];

const NAMES: [&str; 12] = [
    "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
];

/// What each string is called at the start of its line: the note it is tuned to, the
/// highest string in small letters where a lower one has the same name.
pub(super) fn labels(staff: &Staff) -> Vec<String> {
    let names: Vec<&str> = (staff.tuning.strings.iter())
        .map(|pitch| NAMES[usize::from(pitch % 12)])
        .collect();
    (names.iter().enumerate())
        .map(
            |(index, name)| match index == 0 && names[1..].contains(name) {
                true => name.to_lowercase(),
                false => (*name).to_owned(),
            },
        )
        .collect()
}

/// A note's length as the line of lengths says it: its letter, a dot for each dot, and
/// the number of its tuplet.
fn length_mark(duration: Duration) -> Option<String> {
    let letter = LENGTHS
        .iter()
        .find(|(value, _)| *value == duration.value)?
        .1;
    let mut mark = String::from(letter);
    mark.push_str(&".".repeat(duration.dots.into()));
    if let Some(tuplet) = duration.tuplet.filter(|_| duration.nested.is_none()) {
        let _ = write!(mark, "{}", tuplet.actual);
    }
    (duration.nested.is_none()
        && duration
            .tuplet
            .is_none_or(|t| t.normal.get() == pow2(t.actual.get())))
    .then_some(mark)
}

/// The number of notes a tuplet of `actual` usually takes the time of: the power of two
/// below it.
pub(super) fn pow2(actual: u8) -> u8 {
    match actual {
        0..=2 => 1,
        actual => 1 << (7 - (actual - 1).leading_zeros()),
    }
}

/// One moment of a bar with something to write at it.
struct Column {
    /// Whole notes from the start of the bar.
    at: Rational,
    /// What each line says here; empty for a line that says nothing.
    tokens: Vec<String>,
    /// The length written over it, if it has one.
    length: Option<String>,
    chord: Option<String>,
    lyric: Option<String>,
}

fn note_token(note: &Note, next: Option<&Note>) -> String {
    let e = &note.effects;
    let fret = note.fret;
    let mut token = match (e.dead, e.harmonic, note.tied, e.ghost) {
        (true, ..) => "x".to_owned(),
        (_, Some(_), ..) => format!("<{fret}>"),
        (_, _, true, _) => format!("({fret})"),
        (_, _, _, true) => format!("[{fret}]"),
        _ => fret.to_string(),
    };
    if e.tapped {
        token.insert(0, 't');
    }
    match e.slide {
        Some(Slide::InFromBelow) => token.insert(0, '/'),
        Some(Slide::InFromAbove) => token.insert(0, '\\'),
        _ => {}
    }
    if let Some(bend) = &e.bend {
        // A bend is written as the fret whose note it reaches.
        let reach = i32::from(fret) + (i32::from(bend.peak()) + 1) / 2;
        let _ = write!(token, "b{reach}");
        if matches!(
            bend.kind(),
            BendKind::BendRelease | BendKind::PrebendRelease
        ) {
            let _ = write!(token, "r{fret}");
        }
    }
    if e.hammer {
        token.push(match next {
            Some(next) if next.fret < fret => 'p',
            _ => 'h',
        });
    }
    match (e.slide, next) {
        (Some(Slide::Shift | Slide::Legato), Some(next)) if next.fret < fret => token.push('\\'),
        (Some(Slide::Shift | Slide::Legato | Slide::OutUp), _) => token.push('/'),
        (Some(Slide::OutDown), _) => token.push('\\'),
        _ => {}
    }
    if e.vibrato.is_some() {
        token.push('~');
    }
    token
}

/// Says what of a beat plain text has no sign for.
fn report(beat: &Beat, skipped: &mut Skipped) {
    let e = &beat.effects;
    skipped.add_if(e.dynamic.is_some() || e.hairpin.is_some(), "dynamics");
    skipped.add_if(e.tremolo.is_some(), "tremolo picking");
    skipped.add_if(
        e.stroke.is_some() || e.strum.is_some(),
        "strums and pick strokes",
    );
    skipped.add_if(e.slap.is_some(), "slaps and pops");
    skipped.add_if(e.whammy.is_some(), "whammy bar");
    skipped.add_if(e.fade.is_some(), "fades");
    skipped.add_if(e.fermata, "fermatas");
    skipped.add_if(
        e.volume.is_some() || e.pan.is_some(),
        "changes of volume and pan",
    );
    skipped.add_if(e.text.is_some(), "text over the notes");
    skipped.add_if(e.diagram.is_some(), "chord diagrams");
    for note in &beat.notes {
        let e = &note.effects;
        skipped.add_if(e.accent.is_some() || e.staccato, "accents and staccato");
        skipped.add_if(e.trill.is_some(), "trills");
        skipped.add_if(e.grace.is_some(), "grace notes");
        skipped.add_if(
            e.left_finger.is_some() || e.right_finger.is_some(),
            "fingering",
        );
        skipped.add_if(e.palm_mute || e.let_ring, "palm mutes and let ring");
        skipped.add_if(
            e.harmonic.is_some_and(|h| h != Harmonic::Natural),
            "which kind a harmonic is",
        );
        skipped.add_if(
            e.bend.as_ref().is_some_and(|b| {
                *b != gtr_model::Bend::preset(b.kind(), b.peak())
                    || matches!(b.kind(), BendKind::Prebend | BendKind::PrebendRelease)
                    || b.peak() % 2 != 0
            }),
            "the exact shape of bends",
        );
    }
}

/// The columns of one bar of a staff: every moment a voice has a beat at, with what is
/// played there on each line.
fn columns(
    track: &Track,
    staff: &Staff,
    bar: usize,
    lines: &[u8],
    skipped: &mut Skipped,
) -> Vec<Column> {
    let drums = track.instrument.is_percussion();
    let mut found: Vec<Column> = Vec::new();
    let voices = &staff.bars[bar].voices;
    skipped.add_if(
        voices.iter().skip(1).any(|v| !v.beats.is_empty()),
        "the lengths of notes in second voices",
    );
    for (voice_index, voice) in voices.iter().enumerate() {
        let mut at = Rational::from_integer(0);
        for (index, beat) in voice.beats.iter().enumerate() {
            report(beat, skipped);
            let start = at;
            at += beat.duration.length();
            let place = found.partition_point(|column| column.at < start);
            if found.get(place).is_none_or(|column| column.at != start) {
                found.insert(
                    place,
                    Column {
                        at: start,
                        tokens: vec![String::new(); lines.len()],
                        length: None,
                        chord: None,
                        lyric: None,
                    },
                );
            }
            let column = &mut found[place];
            if voice_index == 0 {
                column.length = length_mark(beat.duration);
                skipped.add_if(
                    column.length.is_none(),
                    "note lengths plain text has no letter for",
                );
                column.chord = beat.effects.chord.clone();
                column.lyric = beat
                    .effects
                    .lyrics
                    .first()
                    .filter(|l| !l.is_empty())
                    .cloned();
                skipped.add_if(beat.effects.lyrics.len() > 1, "second lines of lyrics");
            }
            for note in &beat.notes {
                if drums {
                    // A piece is struck on its own line.
                    if let Some(line) = lines.iter().position(|piece| *piece == note.fret) {
                        let cymbal = PIECES.iter().any(|p| p.0 == note.fret && p.2);
                        column.tokens[line] = if cymbal { "x" } else { "o" }.to_owned();
                    }
                    continue;
                }
                // The note after this one on its string says which way a slide goes.
                let next = (voice.beats[index + 1..].iter())
                    .chain(
                        staff.bars[bar + 1..]
                            .iter()
                            .filter_map(|b| b.voices.get(voice_index))
                            .flat_map(|v| &v.beats),
                    )
                    .find_map(|later| later.note_on(note.string));
                if let Some(token) = column.tokens.get_mut(usize::from(note.string)) {
                    *token = note_token(note, next);
                }
            }
        }
    }
    found
}

pub(super) fn write(score: &Score, skipped: &mut Skipped) -> String {
    let info = &score.info;
    let mut out = String::new();
    for (key, value) in [
        ("Title", &info.title),
        ("Subtitle", &info.subtitle),
        ("Artist", &info.artist),
        ("Album", &info.album),
        ("Words", &info.words),
        ("Music", &info.music),
        ("Tabbed by", &info.transcriber),
        ("Copyright", &info.copyright),
    ] {
        if !value.is_empty() {
            let _ = writeln!(out, "{key}: {}", value.replace('\n', " "));
        }
    }
    let _ = writeln!(out, "Tempo: {}", score.tempo);
    out.push_str(
        "\nW whole  H half  Q quarter  E eighth  S sixteenth  T thirty-second  X sixty-fourth\n\
         .  dotted      a number after a length is its tuplet: E3 is an eighth of a triplet\n\
         h  hammer-on   p pull-off   /  slide up   \\  slide down   ~  vibrato   t  tapped\n\
         b  bend to the note of the fret after it   r  let down again   x  dead note\n\
         (n) tied to the note before   [n] ghost note   <n> harmonic\n",
    );

    let masters = &score.master_bars;
    skipped.add_if(
        masters.iter().any(|m| m.alternate_endings != 0),
        "alternate endings",
    );
    skipped.add_if(
        masters
            .iter()
            .any(|m| !m.targets.is_empty() || !m.jumps.is_empty()),
        "signs and jumps",
    );
    skipped.add_if(
        masters.iter().any(|m| m.feel.is_some()),
        "swing and other feels",
    );
    skipped.add_if(masters.iter().any(|m| m.timer), "timers");
    skipped.add_if(
        masters
            .iter()
            .any(|m| !m.tempo_changes.is_empty() || m.tempo_gradual),
        "changes of tempo inside a bar",
    );
    for track in &score.tracks {
        let staff = &track.staves[0];
        if staff.is_pitched() {
            skipped.add("tracks for instruments without strings");
            continue;
        }
        skipped.add_if(
            track.mix != gtr_model::Mix::default(),
            "the mixer's settings",
        );
        write_track(&mut out, score, track, skipped);
    }
    out
}

fn write_track(out: &mut String, score: &Score, track: &Track, skipped: &mut Skipped) {
    let staff = &track.staves[0];
    let drums = track.instrument.is_percussion();
    let _ = write!(
        out,
        "\nTrack: {} ({})\n",
        track.name.replace('\n', " "),
        track.instrument.name()
    );
    // The lines: a string each, or for a kit each piece that is played, as drum tab
    // orders them, cymbals at the top.
    let (lines, names): (Vec<u8>, Vec<String>) = match drums {
        true => {
            let played = |piece: u8| {
                (staff.bars.iter().flat_map(|b| &b.voices))
                    .flat_map(|v| &v.beats)
                    .any(|beat| beat.notes.iter().any(|n| n.fret == piece))
            };
            let used: Vec<(u8, String)> = (PIECES.iter())
                .filter(|piece| played(piece.0))
                .map(|piece| (piece.0, piece.1.to_owned()))
                .collect();
            let unnamed = (staff.bars.iter().flat_map(|b| &b.voices))
                .flat_map(|v| &v.beats)
                .flat_map(|beat| &beat.notes)
                .any(|n| PIECES.iter().all(|p| p.0 != n.fret));
            skipped.add_if(unnamed, "percussion that drum tab has no name for");
            used.into_iter().unzip()
        }
        false => {
            let pitches: Vec<String> = (staff.tuning.strings.iter().rev())
                .map(|pitch| {
                    format!(
                        "{}{}",
                        NAMES[usize::from(pitch % 12)],
                        i32::from(pitch / 12) - 1
                    )
                })
                .collect();
            let _ = writeln!(out, "Tuning: {}", pitches.join(" "));
            if staff.capo > 0 {
                let _ = writeln!(out, "Capo: {}", staff.capo);
            }
            (
                (0..staff.tuning.strings.len() as u8).collect(),
                labels(staff),
            )
        }
    };
    if lines.is_empty() {
        return;
    }
    let width = names
        .iter()
        .map(|name| name.chars().count())
        .max()
        .unwrap_or(1);

    let masters = &score.master_bars;
    let mut bar = 0;
    let mut previous: Option<&MasterBar> = None;
    while bar < masters.len() {
        // What stands over a line of bars: a section, the time and the tempo, each of
        // which starts a line of its own.
        let master = &masters[bar];
        out.push('\n');
        if let Some(section) = &master.section {
            let _ = writeln!(out, "[{}]", section.replace('\n', " "));
        }
        if previous.is_none_or(|p| p.time_signature != master.time_signature) {
            let time = master.time_signature;
            let _ = writeln!(out, "Time: {}/{}", time.numerator, time.denominator);
        }
        if let Some(tempo) = master.tempo.filter(|_| bar > 0) {
            let _ = writeln!(out, "Tempo: {tempo}");
        }

        let pad = " ".repeat(width);
        let mut rows: Vec<String> = names
            .iter()
            .map(|name| format!("{name:<width$}|"))
            .collect();
        let mut lengths = format!("{pad} ");
        let mut chords = format!("{pad} ");
        let mut lyrics = format!("{pad} ");
        let mut first = true;
        while bar < masters.len() {
            let master = &masters[bar];
            let starts_line = master.section.is_some()
                || master.tempo.is_some()
                || previous.is_some_and(|p| p.time_signature != master.time_signature);
            if !first && starts_line {
                break;
            }
            // The bar, a column for each moment, each as wide as its notes need or its
            // length asks.
            let found = columns(track, staff, bar, &lines, skipped);
            let length = master.time_signature.bar_length();
            let mut cells: Vec<String> = vec![String::new(); lines.len()];
            let (mut over, mut names_over, mut under) =
                (String::new(), String::new(), String::new());
            if master.repeat_start {
                cells.iter_mut().for_each(|cell| cell.push(':'));
                [&mut over, &mut names_over, &mut under]
                    .into_iter()
                    .for_each(|line| line.push(' '));
            }
            cells.iter_mut().for_each(|cell| cell.push('-'));
            [&mut over, &mut names_over, &mut under]
                .into_iter()
                .for_each(|line| line.push(' '));
            for (index, column) in found.iter().enumerate() {
                let until = found.get(index + 1).map_or(length, |next| next.at);
                let lasts = ((until - column.at) * 4 * QUARTER)
                    .ceil()
                    .to_integer()
                    .max(1) as usize;
                let needs =
                    |text: &Option<String>| text.as_ref().map_or(0, |t| t.chars().count() + 1);
                let widest = column
                    .tokens
                    .iter()
                    .map(|t| t.chars().count())
                    .max()
                    .unwrap_or(0);
                let wide = (widest + 1)
                    .max(lasts)
                    .max(needs(&column.length))
                    .max(needs(&column.chord));
                let wide = wide.max(needs(&column.lyric));
                for (cell, token) in cells.iter_mut().zip(&column.tokens) {
                    let _ = write!(cell, "{token:-<wide$}");
                }
                let _ = write!(over, "{:<wide$}", column.length.as_deref().unwrap_or(""));
                let _ = write!(
                    names_over,
                    "{:<wide$}",
                    column.chord.as_deref().unwrap_or("")
                );
                let _ = write!(under, "{:<wide$}", column.lyric.as_deref().unwrap_or(""));
            }
            if found.is_empty() {
                let wide = ((length * 4 * QUARTER).ceil().to_integer().max(1)) as usize;
                cells
                    .iter_mut()
                    .for_each(|cell| cell.push_str(&"-".repeat(wide)));
                [&mut over, &mut names_over, &mut under]
                    .into_iter()
                    .for_each(|line| line.push_str(&" ".repeat(wide)));
            }
            // A repeat's end, with how many times in all if that is more than twice.
            let close = match master.repeat_count {
                0 => "|".to_owned(),
                2 => ":|".to_owned(),
                count => format!(":|x{count}|"),
            };
            let adds = cells[0].chars().count() + close.len();
            if !first && rows[0].chars().count() + adds > WIDTH {
                break;
            }
            for (row, cell) in rows.iter_mut().zip(&cells) {
                row.push_str(cell);
                row.push_str(&close);
            }
            for (line, text) in [
                (&mut lengths, over),
                (&mut chords, names_over),
                (&mut lyrics, under),
            ] {
                line.push_str(&text);
                line.push_str(&" ".repeat(close.len()));
            }
            first = false;
            previous = Some(master);
            bar += 1;
        }
        for line in [&chords, &lengths] {
            if !line.trim().is_empty() {
                let _ = writeln!(out, "{}", line.trim_end());
            }
        }
        for row in rows {
            let _ = writeln!(out, "{row}");
        }
        if !lyrics.trim().is_empty() {
            let _ = writeln!(out, "{}", lyrics.trim_end());
        }
    }
}
