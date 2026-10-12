//! Guitar Pro 5 (`.gp5`): a little-endian binary stream read front to back. Nothing in it
//! is length-prefixed as a whole, so every field has to be read or skipped, including the
//! ones the importer has no use for.

use gtr_model::{
    Accent, Beat, BeatEffects, Bend, Dynamic, Feel, Finger, Grace, Harmonic, Instrument, Jump,
    MasterBar, Mix, Note, NoteEffects, NoteValue, PickStroke, Score, ScoreInfo, Slap, Staff, Strum,
    Target, TimeSignature, Track, Trill, Tuning, Vibrato, Voice,
};

use super::{ImportError, Skipped, bar, beat, duration, keep_dynamic_changes, tuning};

struct Reader<'a> {
    data: &'a [u8],
    position: usize,
}

type Result<T> = std::result::Result<T, ImportError>;

impl<'a> Reader<'a> {
    fn take(&mut self, count: usize) -> Result<&'a [u8]> {
        let end = self
            .position
            .checked_add(count)
            .filter(|&e| e <= self.data.len());
        let Some(end) = end else {
            return Err(ImportError::Corrupt(format!(
                "unexpected end of file at byte {}",
                self.position
            )));
        };
        let bytes = &self.data[self.position..end];
        self.position = end;
        Ok(bytes)
    }

    fn skip(&mut self, count: usize) -> Result<()> {
        self.take(count).map(drop)
    }

    fn byte(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }

    fn signed(&mut self) -> Result<i8> {
        Ok(self.byte()? as i8)
    }

    fn int(&mut self) -> Result<i32> {
        let bytes = self.take(4)?;
        Ok(i32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    /// A count that must be small and non-negative, such as a number of bars.
    fn count(&mut self, what: &str, limit: i32) -> Result<usize> {
        let value = self.int()?;
        if (0..=limit).contains(&value) {
            Ok(value as usize)
        } else {
            Err(ImportError::Corrupt(format!("implausible {what}: {value}")))
        }
    }

    fn text(bytes: &[u8]) -> String {
        // Guitar Pro 5 writes strings in the system code page; treat them as Latin-1.
        bytes.iter().map(|&b| char::from(b)).collect()
    }

    /// A string in a fixed-size field: its length in one byte, then `size` bytes.
    fn fixed_string(&mut self, size: usize) -> Result<String> {
        let length = usize::from(self.byte()?);
        let field = self.take(size)?;
        Ok(Self::text(&field[..length.min(size)]))
    }

    /// A string preceded by a four-byte field size and a one-byte length.
    fn string(&mut self) -> Result<String> {
        let size = self.count("string size", 1 << 20)?;
        if size == 0 {
            return Ok(String::new());
        }
        let length = usize::from(self.byte()?);
        let field = self.take(size - 1)?;
        Ok(Self::text(&field[..length.min(field.len())]))
    }
}

struct TrackInfo {
    percussion: bool,
    string_count: usize,
    /// Last fret played on each string, which is what a tied note continues.
    last_fret: [u8; 7],
}

#[derive(Clone, Copy)]
enum Direction {
    Target(Target),
    Jump(Jump),
}

/// The directions a file can hold, in the order it lists their bars. The second segno and
/// coda Guitar Pro has are read as the first, as from its newer format.
const DIRECTIONS: [Direction; 19] = [
    Direction::Target(Target::Coda),
    Direction::Target(Target::Coda),
    Direction::Target(Target::Segno),
    Direction::Target(Target::Segno),
    Direction::Target(Target::Fine),
    Direction::Jump(Jump::DaCapo),
    Direction::Jump(Jump::DaCapoAlCoda),
    Direction::Jump(Jump::DaCapoAlCoda),
    Direction::Jump(Jump::DaCapoAlFine),
    Direction::Jump(Jump::DalSegno),
    Direction::Jump(Jump::DalSegnoAlCoda),
    Direction::Jump(Jump::DalSegnoAlCoda),
    Direction::Jump(Jump::DalSegnoAlFine),
    Direction::Jump(Jump::DalSegno),
    Direction::Jump(Jump::DalSegnoAlCoda),
    Direction::Jump(Jump::DalSegnoAlCoda),
    Direction::Jump(Jump::DalSegnoAlFine),
    Direction::Jump(Jump::ToCoda),
    Direction::Jump(Jump::ToCoda),
];

pub(super) fn read(bytes: &[u8], skipped: &mut Skipped) -> Result<Score> {
    let mut r = Reader {
        data: bytes,
        position: 0,
    };
    let version = r.fixed_string(30).map_err(|_| ImportError::UnknownFormat)?;
    let Some(number) = version.strip_prefix("FICHIER GUITAR PRO v") else {
        return Err(ImportError::UnknownFormat);
    };
    if !number.starts_with('5') {
        return Err(ImportError::UnsupportedVersion(format!(
            "Guitar Pro {number}"
        )));
    }
    // Version 5.10 added a few fields over 5.00.
    let newer = number != "5.00";

    let title = r.string()?;
    let subtitle = r.string()?;
    let artist = r.string()?;
    let album = r.string()?;
    let words = r.string()?;
    let music = r.string()?;
    let copyright = r.string()?;
    let transcriber = r.string()?;
    r.string()?; // instructions
    for _ in 0..r.count("notice count", 10_000)? {
        r.string()?;
    }

    // Lyrics: a track number, then five lines of start bar and text.
    r.int()?;
    for _ in 0..5 {
        r.int()?;
        let length = r.count("lyrics length", 1 << 24)?;
        skipped.add_if(length > 0, "lyrics");
        r.skip(length)?;
    }

    if newer {
        r.skip(19)?; // master effects
    }
    r.skip(30)?; // page setup
    for _ in 0..11 {
        r.string()?; // header and footer templates, tempo label
    }
    let tempo = r.int()?.clamp(1, 1000) as u32;
    if newer {
        r.skip(1)?; // hide tempo
    }
    r.skip(4)?; // key signature
    r.skip(1)?; // octave
    // MIDI channel settings: program, volume and balance, then effect levels.
    let mut channels = Vec::with_capacity(64);
    for _ in 0..64 {
        let program = r.int()?.clamp(0, 127) as u8;
        let volume = r.byte()?;
        let balance = r.byte()?;
        r.skip(6)?;
        channels.push((program, volume, balance));
    }
    // The bar each direction is written on, counted from one, or -1 for none. There is one
    // of each at most, in a fixed order: the signs first, then the jumps.
    let mut directions = [None; DIRECTIONS.len()];
    for bar in &mut directions {
        let bytes = r.take(2)?;
        *bar = usize::try_from(i16::from_le_bytes([bytes[0], bytes[1]]) - 1).ok();
    }
    r.skip(4)?; // master reverb

    let bar_count = r.count("bar count", 100_000)?;
    let track_count = r.count("track count", 1_000)?;

    let mut master_bars: Vec<MasterBar> = Vec::with_capacity(bar_count);
    for index in 0..bar_count {
        if index > 0 {
            r.skip(1)?;
        }
        let flags = r.byte()?;
        let mut time_signature = master_bars
            .last()
            .map_or(TimeSignature::COMMON, |m| m.time_signature);
        if flags & 0x01 != 0 {
            time_signature.numerator = r.byte()?;
        }
        if flags & 0x02 != 0 {
            time_signature.denominator = r.byte()?;
        }
        let mut master = MasterBar::new(TimeSignature::COMMON);
        // The key carries on from the bar before unless this bar changes it.
        master.key_signature = master_bars.last().map_or(0, |m| m.key_signature);
        master.repeat_start = flags & 0x04 != 0;
        if flags & 0x08 != 0 {
            // How many times the passage is played in all. (Older formats, which are not
            // read here, count the times after the first.)
            master.repeat_count = r.byte()?.max(2);
        }
        if flags & 0x20 != 0 {
            let name = r.string()?;
            r.skip(4)?; // marker colour
            master.section =
                Some(name.chars().take(120).collect()).filter(|n: &String| !n.is_empty());
        }
        if flags & 0x10 != 0 {
            master.alternate_endings = r.byte()?;
        }
        if flags & 0x40 != 0 {
            master.key_signature = r.signed()?.clamp(-MasterBar::MAX_KEY, MasterBar::MAX_KEY);
            r.skip(1)?; // major or minor
        }
        if flags & 0x03 != 0 {
            r.skip(4)?; // beaming
        }
        if flags & 0x10 == 0 {
            r.skip(1)?;
        }
        master.feel = match r.byte()? {
            1 => Some(Feel::SwingEighths),
            2 => Some(Feel::SwingSixteenths),
            _ => None,
        };
        if time_signature.is_valid() {
            master.time_signature = time_signature;
        }
        master_bars.push(master);
    }

    for (direction, bar) in DIRECTIONS.iter().zip(directions) {
        let Some(master) = bar.and_then(|bar| master_bars.get_mut(bar)) else {
            continue;
        };
        // A bar here has one sign and one jump: the first of each.
        let taken = match *direction {
            Direction::Target(target) => master
                .target
                .replace(master.target.unwrap_or(target))
                .is_some(),
            Direction::Jump(jump) => master.jump.replace(master.jump.unwrap_or(jump)).is_some(),
        };
        skipped.add_if(taken, "all but the first sign and jump on a bar");
    }

    let mut tracks = Vec::with_capacity(track_count);
    let mut infos = Vec::with_capacity(track_count);
    for index in 0..track_count {
        let flags = r.byte()?;
        if index == 0 || !newer {
            r.skip(1)?;
        }
        let name = r.fixed_string(40)?;
        let string_count = r.count("string count", 7)?;
        let mut strings = Vec::with_capacity(string_count);
        for string in 0..7 {
            let pitch = r.int()?;
            if string < string_count {
                strings.push(pitch.clamp(0, 127) as u8);
            }
        }
        let port = r.int()?;
        let channel = r.int()?;
        r.skip(4)?; // effects channel
        r.skip(4)?; // fret count
        let capo = r.int()?.clamp(0, 24) as u8;
        r.skip(4)?; // colour
        r.skip(if newer { 49 } else { 44 })?;
        if newer {
            r.string()?;
            r.string()?;
        }
        // Not every file sets the flag: a track on MIDI channel 10 is drums either way.
        let percussion = flags & 0x01 != 0 || channel == 10;
        // A drum track's lines have no pitch: its notes say which piece of the kit they are.
        let staff_tuning = match percussion {
            true => Tuning::new(vec![0; strings.len().clamp(1, Tuning::MAX_STRINGS)]),
            false => tuning(strings),
        };
        let capo = if percussion { 0 } else { capo };
        infos.push(TrackInfo {
            percussion,
            string_count: staff_tuning.strings.len(),
            last_fret: [0; 7],
        });
        let mut track = Track::new(
            name,
            Instrument::default(),
            vec![Staff {
                tuning: staff_tuning,
                capo,
                bars: Vec::with_capacity(bar_count),
            }],
        );
        let index = (port.saturating_sub(1).saturating_mul(16))
            .saturating_add(channel)
            .saturating_sub(1);
        let settings = usize::try_from(index)
            .ok()
            .and_then(|index| channels.get(index));
        if let Some(&(program, volume, balance)) = settings {
            track.instrument = Instrument::from_midi_program(program);
            // Guitar Pro counts both in sixteenths, with 13 its usual volume and 8 the centre.
            track.mix.volume = (u32::from(volume) * u32::from(Mix::UNITY) / 13)
                .min(u32::from(Mix::MAX_VOLUME)) as u8;
            track.mix.pan = ((i32::from(balance) - 8) * 25 / 2).clamp(-100, 100) as i8;
        }
        if percussion {
            track.instrument = Instrument::Drums;
        }
        tracks.push(track);
    }
    r.skip(if newer { 1 } else { 2 })?;

    for _ in 0..bar_count {
        for (track, info) in tracks.iter_mut().zip(&mut infos) {
            let mut voices = Vec::with_capacity(2);
            for _ in 0..2 {
                let mut beats = Vec::new();
                let mut any_real = false;
                for _ in 0..r.count("beat count", 10_000)? {
                    let (beat, empty) = read_beat(&mut r, info, newer, skipped)?;
                    any_real |= !empty;
                    beats.push(beat);
                }
                // Placeholder beats pad out unused time. Alongside real beats they act as
                // rests; a voice holding nothing else is simply empty.
                if !any_real {
                    beats.clear();
                }
                voices.push(Voice { beats });
            }
            // A line break flag follows each bar; some files leave it off the very last one.
            if r.position < r.data.len() {
                r.skip(1)?;
            }
            track.staves[0].bars.push(bar(voices));
        }
    }

    for track in &mut tracks {
        keep_dynamic_changes(&mut track.staves[0]);
    }

    Ok(Score {
        info: ScoreInfo {
            title,
            subtitle,
            artist,
            album,
            words,
            music,
            transcriber,
            copyright,
        },
        tempo,
        master_bars,
        tracks,
    })
}

/// Effects Guitar Pro 5 stores on the beat that the model keeps on each note, and the
/// beat's own effects.
#[derive(Default)]
struct BeatWide {
    vibrato: Option<Vibrato>,
    tapped: bool,
    effects: BeatEffects,
}

/// Reads one beat, and whether it is a placeholder rather than a note or written rest.
fn read_beat(
    r: &mut Reader,
    track: &mut TrackInfo,
    newer: bool,
    skipped: &mut Skipped,
) -> Result<(Beat, bool)> {
    let flags = r.byte()?;
    let empty = flags & 0x40 != 0 && r.byte()? == 0;
    let value = match r.signed()? {
        -2 => Some(NoteValue::Whole),
        -1 => Some(NoteValue::Half),
        0 => Some(NoteValue::Quarter),
        1 => Some(NoteValue::Eighth),
        2 => Some(NoteValue::Sixteenth),
        3 => Some(NoteValue::ThirtySecond),
        4 => Some(NoteValue::SixtyFourth),
        _ => None,
    };
    let tuplet = if flags & 0x20 != 0 {
        match r.int()? {
            3 => Some((3, 2)),
            5 => Some((5, 4)),
            6 => Some((6, 4)),
            7 => Some((7, 4)),
            9 => Some((9, 8)),
            10 => Some((10, 8)),
            11 => Some((11, 8)),
            12 => Some((12, 8)),
            13 => Some((13, 8)),
            _ => None,
        }
    } else {
        None
    };
    let read_duration = duration(value, u8::from(flags & 0x01 != 0), tuplet, skipped);

    if flags & 0x02 != 0 {
        r.skip(107)?; // chord diagram
        skipped.add("chord names");
    }
    if flags & 0x04 != 0 {
        r.string()?;
        skipped.add("text");
    }
    let mut wide = match flags & 0x08 != 0 {
        true => read_beat_effects(r, skipped)?,
        false => BeatWide::default(),
    };
    if flags & 0x10 != 0 {
        read_mix_change(r, newer, skipped)?;
    }

    // One bit per string, the highest string in bit 6.
    let strings = r.byte()?;
    let mut notes = Vec::new();
    for string in 0..7u8 {
        if strings & (0x40 >> string) != 0 {
            let read = read_note(r, string, track, skipped)?;
            wide.effects.tremolo = wide.effects.tremolo.or(read.tremolo);
            // The first note's dynamic stands for the beat.
            wide.effects.dynamic = wide.effects.dynamic.or(Some(read.dynamic));
            if let Some(mut note) = read.note {
                note.effects.vibrato = note.effects.vibrato.or(wide.vibrato);
                note.effects.tapped |= wide.tapped;
                notes.push(note);
            }
        }
    }

    r.skip(1)?;
    if r.byte()? & 0x08 != 0 {
        r.skip(1)?;
    }
    let mut read = beat(read_duration, notes, track.string_count);
    if !read.is_rest() {
        read.effects = wide.effects;
    }
    Ok((read, empty))
}

fn read_beat_effects(r: &mut Reader, skipped: &mut Skipped) -> Result<BeatWide> {
    let flags1 = r.byte()?;
    let flags2 = r.byte()?;
    let mut wide = BeatWide {
        // Bit 0 is finger vibrato and bit 1 vibrato with the tremolo bar.
        vibrato: (flags1 & 0x03 != 0).then_some(Vibrato::Slight),
        ..BeatWide::default()
    };
    skipped.add_if(flags1 & 0x10 != 0, "fade in");
    if flags1 & 0x20 != 0 {
        match r.byte()? {
            1 => wide.tapped = true,
            2 => wide.effects.slap = Some(Slap::Slap),
            3 => wide.effects.slap = Some(Slap::Pop),
            _ => {}
        }
    }
    if flags2 & 0x04 != 0 {
        wide.effects.whammy = read_bend(r)?.filter(|w| w.is_valid(Bend::WHAMMY_RANGE));
    }
    if flags1 & 0x40 != 0 {
        // Strum speeds, upstroke first; zero means no strum that way.
        let (up, down) = (r.byte()?, r.byte()?);
        wide.effects.strum = match (up, down) {
            (0, 0) => None,
            (0, _) => Some(Strum::BrushDown),
            _ => Some(Strum::BrushUp),
        };
    }
    if flags2 & 0x01 != 0 {
        wide.effects.strum = Some(Strum::Rasgueado);
    }
    if flags2 & 0x02 != 0 {
        wide.effects.stroke = match r.byte()? {
            1 => Some(PickStroke::Up),
            2 => Some(PickStroke::Down),
            _ => None,
        };
    }
    Ok(wide)
}

/// Reads a bend or whammy bar curve.
fn read_bend(r: &mut Reader) -> Result<Option<Bend>> {
    r.skip(5)?; // type and height
    let mut points = Vec::new();
    for _ in 0..r.count("bend point count", 1_000)? {
        let position = r.int()?;
        let height = r.int()?;
        r.skip(1)?; // vibrato
        points.push((position, height));
    }
    Ok(super::bend_from_points(points.into_iter()))
}

fn read_mix_change(r: &mut Reader, newer: bool, skipped: &mut Skipped) -> Result<()> {
    r.skip(1)?; // instrument
    r.skip(16)?;
    let mut changed = [false; 6]; // volume, pan, chorus, reverb, phaser, tremolo
    for change in &mut changed {
        *change = r.signed()? >= 0;
    }
    r.string()?; // tempo name
    let tempo = r.int()?;
    for change in changed {
        if change {
            r.skip(1)?; // transition length
        }
    }
    if tempo >= 0 {
        skipped.add("tempo changes");
        r.skip(1)?;
        if newer {
            r.skip(1)?;
        }
    }
    r.skip(2)?;
    if newer {
        r.string()?;
        r.string()?;
    }
    Ok(())
}

struct ReadNote {
    /// `None` for a note on a fret that does not exist.
    note: Option<Note>,
    tremolo: Option<NoteValue>,
    dynamic: Dynamic,
}

fn finger(code: i8) -> Option<Finger> {
    Some(match code {
        0 => Finger::Thumb,
        1 => Finger::Index,
        2 => Finger::Middle,
        3 => Finger::Ring,
        4 => Finger::Pinky,
        _ => return None,
    })
}

fn read_note(
    r: &mut Reader,
    string: u8,
    track: &mut TrackInfo,
    skipped: &mut Skipped,
) -> Result<ReadNote> {
    let flags = r.byte()?;
    let kind = if flags & 0x20 != 0 { r.byte()? } else { 1 };
    let mut dynamic = Dynamic::F;
    if flags & 0x10 != 0 {
        // Guitar Pro 5 folds articulation into the stored level: a ghost note is written
        // one level quieter than the passage it is in, an accented note louder.
        let adjust = match flags {
            f if f & 0x04 != 0 => 1,
            f if f & 0x02 != 0 => -2,
            f if f & 0x40 != 0 => -1,
            _ => 0,
        };
        let level = i32::from(r.byte()?.clamp(1, 8)) - 1 + adjust;
        dynamic = Dynamic::ALL[level.clamp(0, 7) as usize];
    }
    let mut fret = 0;
    if flags & 0x20 != 0 {
        fret = r.signed()?.clamp(0, 99) as u8;
    }
    let mut fingers = (None, None);
    if flags & 0x80 != 0 {
        fingers = (finger(r.signed()?), finger(r.signed()?));
    }
    if flags & 0x01 != 0 {
        r.skip(8)?;
    }
    r.skip(1)?;
    let (mut effects, tremolo) = match flags & 0x08 != 0 {
        true => read_note_effects(r, skipped)?,
        false => (NoteEffects::default(), None),
    };
    effects.ghost = flags & 0x04 != 0;
    for grace in effects.grace.iter_mut().flat_map(|g| &mut g.notes) {
        grace.string = string;
    }
    effects.dead = kind == 3;
    effects.accent = match flags {
        f if f & 0x02 != 0 => Some(Accent::Heavy),
        f if f & 0x40 != 0 => Some(Accent::Normal),
        _ => None,
    };
    (effects.left_finger, effects.right_finger) = fingers;

    let tied = kind == 2;
    let memory = &mut track.last_fret[usize::from(string)];
    if tied {
        fret = *memory;
    } else {
        *memory = fret;
    }
    // On a percussion track the fret is the number of a piece of the drum kit.
    let highest = match track.percussion {
        true => Instrument::Drums.max_fret(),
        false => Instrument::default().max_fret(),
    };
    let note = (fret <= highest).then_some(Note {
        string,
        fret,
        tied,
        effects,
    });
    Ok(ReadNote {
        note,
        tremolo,
        dynamic,
    })
}

fn read_note_effects(
    r: &mut Reader,
    _skipped: &mut Skipped,
) -> Result<(NoteEffects, Option<NoteValue>)> {
    let flags1 = r.byte()?;
    let flags2 = r.byte()?;
    let mut effects = NoteEffects {
        hammer: flags1 & 0x02 != 0,
        let_ring: flags1 & 0x08 != 0,
        staccato: flags2 & 0x01 != 0,
        palm_mute: flags2 & 0x02 != 0,
        vibrato: (flags2 & 0x40 != 0).then_some(Vibrato::Slight),
        ..NoteEffects::default()
    };
    let mut tremolo = None;
    if flags1 & 0x01 != 0 {
        effects.bend = read_bend(r)?.filter(|b| b.is_valid(Bend::BEND_RANGE));
    }
    if flags1 & 0x10 != 0 {
        let fret = r.byte()?;
        r.skip(1)?; // dynamic
        let transition = r.byte()?;
        r.skip(1)?; // duration
        let grace_flags = r.byte()?;
        // The string is the note's own, which the caller fills in.
        effects.grace = Some(Grace::single(
            0,
            fret.min(99),
            grace_flags & 0x02 != 0,
            transition == 3,
        ));
    }
    if flags2 & 0x04 != 0 {
        tremolo = match r.byte()? {
            1 => Some(NoteValue::Eighth),
            2 => Some(NoteValue::Sixteenth),
            _ => Some(NoteValue::ThirtySecond),
        };
    }
    if flags2 & 0x08 != 0 {
        effects.slide = super::slide_from_flags(r.byte()?);
    }
    if flags2 & 0x10 != 0 {
        effects.harmonic = match r.byte()? {
            2 => {
                r.skip(3)?;
                Some(Harmonic::Artificial)
            }
            3 => {
                r.skip(1)?;
                Some(Harmonic::Tapped)
            }
            4 => Some(Harmonic::Pinch),
            5 => Some(Harmonic::Semi),
            _ => Some(Harmonic::Natural),
        };
    }
    if flags2 & 0x20 != 0 {
        let fret = r.byte()?;
        effects.trill = Some(Trill {
            fret,
            speed: match r.byte()? {
                1 => NoteValue::Sixteenth,
                2 => NoteValue::ThirtySecond,
                _ => NoteValue::SixtyFourth,
            },
        });
    }
    Ok((effects, tremolo))
}
