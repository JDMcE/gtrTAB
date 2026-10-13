//! MIDI import. A MIDI file says when notes sound, not how they are written, so the notes
//! are put on the nearest thirty-second or triplet sixteenth, gathered into chords, and
//! written as one voice in which a chord lasts until the next. A note that goes on ringing
//! past the next is marked to ring. Tracks for a guitar or a bass go on strings, nearest
//! the nut; a track for anything else becomes a piano, split between the hands at
//! middle C.

use std::collections::BTreeMap;

use gtr_model::{
    Bar, Beat, Dynamic, Instrument, MasterBar, Mix, Note, Rational, Score, ScoreInfo, Staff,
    TimeSignature, Track, Voice,
};

use crate::util::{durations_for, place};
use crate::{ImportError, Skipped};

/// The most bars a file is read as: far more than any piece, and few enough that a file
/// that claims to last for days is refused instead of read.
const MAX_BARS: usize = 5000;

fn corrupt(what: &str) -> ImportError {
    ImportError::Corrupt(what.into())
}

struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, count: usize) -> Result<&'a [u8], ImportError> {
        let end = self
            .at
            .checked_add(count)
            .filter(|end| *end <= self.bytes.len());
        let end = end.ok_or_else(|| corrupt("it ends before it should"))?;
        let taken = &self.bytes[self.at..end];
        self.at = end;
        Ok(taken)
    }

    fn byte(&mut self) -> Result<u8, ImportError> {
        Ok(self.take(1)?[0])
    }

    fn number(&mut self, bytes: usize) -> Result<u32, ImportError> {
        Ok(self
            .take(bytes)?
            .iter()
            .fold(0, |value, byte| value << 8 | u32::from(*byte)))
    }

    /// A number written seven bits to a byte.
    fn quantity(&mut self) -> Result<u32, ImportError> {
        let mut value = 0u32;
        for _ in 0..4 {
            let byte = self.byte()?;
            value = value << 7 | u32::from(byte & 0x7F);
            if byte & 0x80 == 0 {
                return Ok(value);
            }
        }
        Err(corrupt("a length runs on too long"))
    }
}

/// A note as the file plays it, in ticks.
struct Sounded {
    start: u64,
    end: u64,
    pitch: u8,
    velocity: u8,
}

/// The notes of one channel of one track of the file, and what the channel is set to.
#[derive(Default)]
struct Part {
    name: String,
    channel: u8,
    program: Option<u8>,
    volume: Option<u8>,
    pan: Option<u8>,
    notes: Vec<Sounded>,
}

#[derive(Default)]
struct Conductor {
    title: Option<String>,
    copyright: String,
    /// Microseconds to the quarter note from each tick on.
    tempos: BTreeMap<u64, u32>,
    times: BTreeMap<u64, TimeSignature>,
    keys: BTreeMap<u64, i8>,
    bends: bool,
}

fn text(bytes: &[u8]) -> String {
    (String::from_utf8_lossy(bytes).trim().chars())
        .filter(|c| !c.is_control())
        .take(120)
        .collect()
}

fn read_track(
    data: &[u8],
    conductor: &mut Conductor,
    parts: &mut Vec<Part>,
) -> Result<(), ImportError> {
    let mut r = Reader { bytes: data, at: 0 };
    let mut tick = 0u64;
    let mut status = 0u8;
    let mut name = String::new();
    // The parts this track has, by channel, and the notes still sounding in each.
    let mut mine: BTreeMap<u8, Part> = BTreeMap::new();
    let mut open: Vec<(u8, u8, u64, u8)> = Vec::new();
    while r.at < data.len() {
        tick += u64::from(r.quantity()?);
        let mut first = r.byte()?;
        if first < 0x80 {
            // The same kind of event as the last: this byte is already its first.
            if status < 0x80 {
                return Err(corrupt("an event has no kind"));
            }
            r.at -= 1;
            first = status;
        } else if first < 0xF0 {
            status = first;
        }
        let channel = first & 0x0F;
        match first & 0xF0 {
            0x80 | 0x90 => {
                let pitch = r.byte()? & 0x7F;
                let velocity = r.byte()? & 0x7F;
                let on = first & 0xF0 == 0x90 && velocity > 0;
                // A note that starts again while it sounds ends where it starts again.
                if let Some(at) = open.iter().position(|o| (o.0, o.1) == (channel, pitch)) {
                    let (_, _, start, velocity) = open.swap_remove(at);
                    let part = mine.entry(channel).or_default();
                    part.notes.push(Sounded {
                        start,
                        end: tick.max(start + 1),
                        pitch,
                        velocity,
                    });
                }
                if on {
                    open.push((channel, pitch, tick, velocity));
                    mine.entry(channel).or_default();
                }
            }
            0xA0 => {
                r.take(2)?;
            }
            0xB0 => {
                let controller = r.byte()?;
                let value = r.byte()? & 0x7F;
                let part = mine.entry(channel).or_default();
                // What the channel is set to before it plays is how the track is mixed.
                if part.notes.is_empty() && !open.iter().any(|o| o.0 == channel) {
                    match controller {
                        7 => part.volume = Some(value),
                        10 => part.pan = Some(value),
                        _ => {}
                    }
                }
            }
            0xC0 => {
                let program = r.byte()? & 0x7F;
                mine.entry(channel)
                    .or_default()
                    .program
                    .get_or_insert(program);
            }
            0xD0 => {
                r.byte()?;
            }
            0xE0 => {
                let bent = r.take(2)? != [0x00, 0x40];
                conductor.bends |= bent;
            }
            _ => match first {
                0xFF => {
                    let kind = r.byte()?;
                    let length = r.quantity()? as usize;
                    let data = r.take(length)?;
                    match (kind, data) {
                        (0x03, _) if name.is_empty() => name = text(data),
                        (0x02, _) if conductor.copyright.is_empty() => {
                            conductor.copyright = text(data);
                        }
                        (0x51, [a, b, c]) => {
                            let tempo = u32::from_be_bytes([0, *a, *b, *c]);
                            conductor.tempos.entry(tick).or_insert(tempo.max(1));
                        }
                        (0x58, [numerator, power, ..]) => {
                            let time = TimeSignature {
                                numerator: *numerator,
                                denominator: 1u8.checked_shl((*power).into()).unwrap_or(0),
                            };
                            if time.is_valid() {
                                conductor.times.entry(tick).or_insert(time);
                            }
                        }
                        (0x59, [sharps, ..]) => {
                            let key =
                                (*sharps as i8).clamp(-MasterBar::MAX_KEY, MasterBar::MAX_KEY);
                            conductor.keys.entry(tick).or_insert(key);
                        }
                        (0x2F, _) => break,
                        _ => {}
                    }
                }
                0xF0 | 0xF7 => {
                    let length = r.quantity()? as usize;
                    r.take(length)?;
                }
                _ => return Err(corrupt("an event of no known kind")),
            },
        }
    }
    // Notes that never end, end with the track.
    for (channel, pitch, start, velocity) in open {
        mine.entry(channel).or_default().notes.push(Sounded {
            start,
            end: tick.max(start + 1),
            pitch,
            velocity,
        });
    }
    // A track often has a second channel for the notes whose pitch moves, set to the
    // same sound: those are one part. Channels with sounds of their own are parts apart.
    let mut found: Vec<Part> = Vec::new();
    for (channel, mut part) in mine {
        if part.notes.is_empty() {
            continue;
        }
        part.channel = channel;
        let same = (found.iter_mut())
            .find(|other| channel != 9 && other.channel != 9 && other.program == part.program);
        match same {
            Some(other) => other.notes.append(&mut part.notes),
            None => found.push(part),
        }
    }
    // LilyPond calls the track that keeps time this, and ends its names with a colon.
    let name = name.trim_end_matches(':').to_owned();
    if found.is_empty() {
        // A track without notes is the conductor's, and its name the piece's.
        if !name.is_empty() && !name.eq_ignore_ascii_case("control track") {
            conductor.title.get_or_insert(name);
        }
        return Ok(());
    }
    let several = found.len() > 1;
    for mut part in found {
        part.name = match (name.is_empty(), several) {
            (true, _) => format!("Channel {}", part.channel + 1),
            (false, true) => format!("{name} ({})", part.channel + 1),
            (false, false) => name.clone(),
        };
        parts.push(part);
    }
    Ok(())
}

pub(super) fn read(bytes: &[u8], skipped: &mut Skipped) -> Result<Score, ImportError> {
    let mut r = Reader { bytes, at: 0 };
    if r.take(4).ok() != Some(b"MThd".as_slice()) {
        return Err(ImportError::UnknownFormat);
    }
    let length = r.number(4)? as usize;
    let header = r.take(length)?;
    let [_, format, _, _, division_high, division_low, ..] = header else {
        return Err(corrupt("its header is too short"));
    };
    if division_high & 0x80 != 0 {
        return Err(ImportError::UnsupportedVersion(
            "MIDI files timed in frames".into(),
        ));
    }
    let quarter = u64::from(u16::from_be_bytes([*division_high, *division_low]));
    if quarter == 0 {
        return Err(corrupt("it has no ticks to the quarter note"));
    }
    skipped.add_if(*format == 2, "that its tracks are separate pieces");

    let mut conductor = Conductor::default();
    let mut parts = Vec::new();
    while r.at < bytes.len() {
        // A file cut short keeps the tracks that are whole.
        let (Ok(kind), Ok(length)) = (r.take(4), r.number(4)) else {
            break;
        };
        let Ok(data) = r.take(length as usize) else {
            let rest = &bytes[r.at..];
            if kind == b"MTrk" {
                let _ = read_track(rest, &mut conductor, &mut parts);
            }
            break;
        };
        if kind == b"MTrk" {
            read_track(data, &mut conductor, &mut parts)?;
        }
    }
    if parts.is_empty() {
        return Err(corrupt("it has no notes"));
    }
    skipped.add_if(conductor.bends, "pitch bends");

    // The bars, from the time signatures: where each starts, in ticks.
    let end = (parts.iter().flat_map(|p| &p.notes))
        .map(|n| n.end)
        .max()
        .unwrap_or(0);
    let whole = quarter * 4;
    let mut masters = Vec::new();
    let mut starts = Vec::new();
    let mut time = TimeSignature::COMMON;
    let mut tick = 0u64;
    while tick < end || masters.is_empty() {
        if masters.len() >= MAX_BARS {
            return Err(corrupt("it is longer than any piece"));
        }
        // A change of time takes hold at the first bar that starts on or after it.
        if let Some((_, new)) = conductor.times.range(..=tick).next_back() {
            time = *new;
        }
        let mut master = MasterBar::new(time);
        master.key_signature = (conductor.keys.range(..=tick).next_back()).map_or(0, |k| *k.1);
        masters.push(master);
        starts.push(tick);
        tick += (whole * u64::from(time.numerator) / u64::from(time.denominator)).max(1);
    }
    starts.push(tick);

    // The tempo: what the piece starts at, and each change that is one to the ear.
    let bpm = |micros: u32| {
        (60_000_000.0 / f64::from(micros))
            .round()
            .clamp(1.0, 1000.0) as u32
    };
    let mut tempo = (conductor.tempos.range(..=0).next_back()).map_or(120, |t| bpm(*t.1));
    let start_tempo = tempo;
    for (at, micros) in conductor.tempos.range(1..) {
        let new = bpm(*micros);
        let bar = starts
            .partition_point(|start| start <= at)
            .saturating_sub(1);
        if new == tempo || bar >= masters.len() {
            continue;
        }
        tempo = new;
        let position = Rational::new(((at - starts[bar]) * 1920 / whole) as i64, 1920);
        crate::guitar_pro::set_tempo(&mut masters[bar], position, new, false);
    }

    let mut tracks = Vec::new();
    for part in &parts {
        tracks.push(read_part(part, &starts, whole, masters.len(), skipped));
    }
    Ok(Score {
        info: ScoreInfo {
            title: conductor.title.unwrap_or_default(),
            copyright: conductor.copyright,
            ..ScoreInfo::default()
        },
        tempo: start_tempo,
        master_bars: masters,
        tracks,
        effects: Vec::new(),
        audio: None,
    })
}

/// A moment in whole notes from the start of the piece, on the nearest thirty-second or
/// triplet sixteenth.
fn snap(tick: u64, whole: u64) -> Rational {
    let nearest = |grid: u64| {
        let steps = (tick * grid + whole / 2) / whole;
        (
            Rational::new(steps as i64, grid as i64),
            (tick * grid).abs_diff(steps * whole) * 96 / grid,
        )
    };
    let (straight, off_straight) = nearest(32);
    let (triplet, off_triplet) = nearest(24);
    match off_triplet < off_straight {
        true => triplet,
        false => straight,
    }
}

/// Notes that start together, with how long the longest of them lasts.
struct Chord {
    start: Rational,
    /// When the last of its notes ends.
    end: Rational,
    /// Pitches with their velocities, and when each ends.
    notes: Vec<(u8, u8, Rational)>,
}

fn read_part(part: &Part, starts: &[u64], whole: u64, bars: usize, skipped: &mut Skipped) -> Track {
    let drums = part.channel == 9;
    let instrument = match (drums, part.program) {
        (true, _) => Instrument::Drums,
        (_, Some(program @ (24..=39 | 105))) => Instrument::from_midi_program(program),
        (_, program) => {
            skipped.add_if(
                program.is_some_and(|p| p > 7),
                "the sounds of instruments without strings, which are played as a piano",
            );
            Instrument::Piano
        }
    };
    let mut track = Track::empty(part.name.clone(), instrument, bars);
    if let Some(volume) = part.volume {
        let level = (f64::from(volume) * f64::from(Mix::MAX_VOLUME) / 127.0).round();
        track.mix.volume = level as u8;
    }
    if let Some(pan) = part.pan {
        track.mix.pan = ((f64::from(pan) - 64.0) * 100.0 / 63.0)
            .round()
            .clamp(-100.0, 100.0) as i8;
    }

    // A piano's notes go to the hand that would play them.
    let hands: Vec<Box<dyn Fn(u8) -> bool>> = match instrument {
        Instrument::Piano => vec![Box::new(|pitch| pitch >= 60), Box::new(|pitch| pitch < 60)],
        _ => vec![Box::new(|_| true)],
    };
    let grid = Rational::new(1, 32);
    for (staff, mine) in track.staves.iter_mut().zip(&hands) {
        // The notes on the grid, gathered into chords.
        let mut notes: Vec<&Sounded> = part.notes.iter().filter(|n| mine(n.pitch)).collect();
        notes.sort_by_key(|n| (n.start, n.pitch));
        let mut chords: Vec<Chord> = Vec::new();
        for note in notes {
            let start = snap(note.start, whole);
            let end = snap(note.end, whole).max(start + grid);
            match chords.last_mut() {
                Some(chord) if chord.start == start => {
                    chord.end = chord.end.max(end);
                    if chord.notes.iter().all(|n| n.0 != note.pitch) {
                        chord.notes.push((note.pitch, note.velocity, end));
                    }
                }
                _ => chords.push(Chord {
                    start,
                    end,
                    notes: vec![(note.pitch, note.velocity, end)],
                }),
            }
        }
        write_staff(staff, &chords, starts, whole, instrument, skipped);
    }
    track
}

/// Writes chords into the bars of a staff as one voice.
fn write_staff(
    staff: &mut Staff,
    chords: &[Chord],
    starts: &[u64],
    whole: u64,
    instrument: Instrument,
    skipped: &mut Skipped,
) {
    let drums = instrument.is_percussion();
    // Where each bar starts and ends, in whole notes from the start of the piece.
    let edge = |bar: usize| Rational::new(starts[bar] as i64, whole as i64);
    let zero = Rational::from_integer(0);
    let mut bar = 0;
    let mut cursor = zero;
    let mut level = Dynamic::F;
    let mut beats: Vec<Vec<Beat>> = vec![Vec::new(); staff.bars.len()];
    // Fills the voice with `notes`, or rests, from the cursor up to `until`, a bar at a
    // time: what crosses a bar line is tied over it.
    let mut fill = |until: Rational,
                    notes: &[Note],
                    marks: Option<Dynamic>,
                    cursor: &mut Rational,
                    bar: &mut usize| {
        let mut first = true;
        while *cursor < until && *bar < beats.len() {
            let stop = until.min(edge(*bar + 1));
            for duration in durations_for(stop - *cursor) {
                let mut beat = Beat::new(duration, notes.to_vec());
                for note in &mut beat.notes {
                    note.tied = !first && !drums;
                }
                if !first && drums {
                    beat.notes.clear();
                }
                if first && !beat.notes.is_empty() {
                    beat.effects.dynamic = marks;
                }
                first = false;
                beats[*bar].push(beat);
            }
            *cursor = stop;
            if stop >= edge(*bar + 1) {
                *bar += 1;
            }
        }
    };
    for (index, chord) in chords.iter().enumerate() {
        if chord.start < cursor {
            continue;
        }
        fill(chord.start, &[], None, &mut cursor, &mut bar);
        // The chord lasts until its last note ends or the next chord starts.
        let next = chords.get(index + 1).map(|next| next.start);
        let until = next.map_or(chord.end, |next| chord.end.min(next));
        let mut notes: Vec<Note> = Vec::new();
        // On strings the highest note is placed first, so that a chord takes a shape a
        // hand can make; the pieces of a kit keep the order they came in.
        let mut ordered: Vec<&(u8, u8, Rational)> = chord.notes.iter().collect();
        if !drums {
            ordered.sort_by_key(|note| std::cmp::Reverse(note.0));
        }
        for (pitch, _, end) in ordered {
            let rings = next.is_some_and(|next| *end > next);
            let placed = if drums {
                (*pitch <= Instrument::Drums.max_fret()
                    && *pitch > 0
                    && notes.len() < staff.places())
                .then_some((notes.len() as u8, *pitch))
            } else if staff.is_pitched() {
                Some((*pitch, 0))
            } else {
                let taken: Vec<u8> = notes.iter().map(|n| n.string).collect();
                place(*pitch, &staff.tuning, staff.capo, &taken)
            };
            let Some((string, fret)) = placed else {
                skipped.add("notes that do not fit on the strings");
                continue;
            };
            let mut note = Note::new(string, fret);
            note.effects.let_ring = rings && !drums;
            notes.push(note);
        }
        notes.sort_by_key(|n| n.string);
        // How loud, said where it changes by more than a shade.
        let loudest = chord.notes.iter().map(|n| n.1).max().unwrap_or(80);
        let gain = f32::from(loudest) / 80.0;
        let heard = (Dynamic::ALL.into_iter())
            .min_by(|a, b| (a.gain() - gain).abs().total_cmp(&(b.gain() - gain).abs()))
            .unwrap_or(Dynamic::F);
        let marks = ((heard as i32 - level as i32).abs() >= 2).then_some(heard);
        level = marks.unwrap_or(level);
        fill(until, &notes, marks, &mut cursor, &mut bar);
    }
    // The bar the music stops in is rested out to its end.
    if bar < staff.bars.len() && cursor > edge(bar) {
        fill(edge(bar + 1), &[], None, &mut cursor, &mut bar);
    }
    for (bar, mut beats) in staff.bars.iter_mut().zip(beats) {
        // A bar of nothing but rests is an empty bar.
        if beats.iter().all(Beat::is_rest) {
            beats.clear();
        }
        *bar = Bar {
            voices: vec![Voice { beats }],
        };
    }
}
