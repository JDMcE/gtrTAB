//! Guitar Pro 3, 4 and 5 (`.gp3`, `.gp4`, `.gp5`): a little-endian binary stream read front
//! to back. Nothing in it is length-prefixed as a whole, so every field has to be read or
//! skipped, including the ones the importer has no use for. The three versions are one
//! format that grew: each added fields, and here and there changed what one means, so the
//! reader carries the version as a number (300, 400, 406, 500, 510) and asks it as it goes.

use gtr_model::{
    Accent, Beat, BeatEffects, Bend, ChordDiagram, Dynamic, Fade, Feel, Finger, Grace, Harmonic,
    Instrument, Jump, MasterBar, Mix, Note, NoteEffects, NoteValue, PickStroke, Score, ScoreInfo,
    Slap, Slide, Staff, Strum, Target, TimeSignature, Track, Trill, Tuning, Vibrato, Voice,
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

/// The directions a file can hold, in the order it lists their bars.
const DIRECTIONS: [Direction; 19] = [
    Direction::Target(Target::Coda),
    Direction::Target(Target::DoubleCoda),
    Direction::Target(Target::Segno),
    Direction::Target(Target::SegnoSegno),
    Direction::Target(Target::Fine),
    Direction::Jump(Jump::DaCapo),
    Direction::Jump(Jump::DaCapoAlCoda),
    Direction::Jump(Jump::DaCapoAlDoubleCoda),
    Direction::Jump(Jump::DaCapoAlFine),
    Direction::Jump(Jump::DalSegno),
    Direction::Jump(Jump::DalSegnoAlCoda),
    Direction::Jump(Jump::DalSegnoAlDoubleCoda),
    Direction::Jump(Jump::DalSegnoAlFine),
    Direction::Jump(Jump::DalSegnoSegno),
    Direction::Jump(Jump::DalSegnoSegnoAlCoda),
    Direction::Jump(Jump::DalSegnoSegnoAlDoubleCoda),
    Direction::Jump(Jump::DalSegnoSegnoAlFine),
    Direction::Jump(Jump::ToCoda),
    Direction::Jump(Jump::ToDoubleCoda),
];

pub(super) fn read(bytes: &[u8], skipped: &mut Skipped) -> Result<Score> {
    let mut r = Reader {
        data: bytes,
        position: 0,
    };
    let version = r.fixed_string(30).map_err(|_| ImportError::UnknownFormat)?;
    let number = (version.strip_prefix("FICHIER GUITAR PRO v"))
        .or_else(|| version.strip_prefix("FICHIER GUITAR PRO L"))
        .or_else(|| version.strip_prefix("FICHIER GUITARE PRO v"))
        .ok_or(ImportError::UnknownFormat)?;
    let digits: String = number.chars().filter(char::is_ascii_digit).collect();
    let v: u32 = digits.parse().unwrap_or(0);
    if !(300..600).contains(&v) {
        return Err(ImportError::UnsupportedVersion(format!(
            "Guitar Pro {number}"
        )));
    }
    // Version 5.10 added a few fields over 5.00.
    let newer = v > 500;

    let title = r.string()?;
    let subtitle = r.string()?;
    let artist = r.string()?;
    let album = r.string()?;
    let words = r.string()?;
    // Before version 5 one name stood for the words and the music.
    let music = match v >= 500 {
        true => r.string()?,
        false => words.clone(),
    };
    let copyright = r.string()?;
    let transcriber = r.string()?;
    r.string()?; // instructions
    for _ in 0..r.count("notice count", 10_000)? {
        r.string()?;
    }

    // Before version 5 the whole piece is swung or it is not.
    let swung = v < 500 && r.byte()? != 0;

    // Lyrics, from version 4: the track they are sung to, then five lines, each with the
    // bar it starts in and its words.
    let mut lyrics = Lyrics::default();
    if v >= 400 {
        lyrics.track = r.int()?;
        for line in &mut lyrics.lines {
            line.0 = r.int()?;
            let length = r.count("lyrics length", 1 << 24)?;
            line.1 = Reader::text(r.take(length)?);
        }
    }

    if newer {
        r.skip(19)?; // master effects
    }
    if v >= 500 {
        r.skip(30)?; // page setup
        for _ in 0..11 {
            r.string()?; // header and footer templates, tempo label
        }
    }
    let tempo = r.int()?.clamp(1, 1000) as u32;
    if newer {
        r.skip(1)?; // hide tempo
    }
    r.skip(4)?; // key signature
    if v >= 400 {
        r.skip(1)?; // octave
    }
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
    if v >= 500 {
        for bar in &mut directions {
            let bytes = r.take(2)?;
            *bar = usize::try_from(i16::from_le_bytes([bytes[0], bytes[1]]) - 1).ok();
        }
        r.skip(4)?; // master reverb
    }

    let bar_count = r.count("bar count", 100_000)?;
    let track_count = r.count("track count", 1_000)?;

    let mut master_bars: Vec<MasterBar> = Vec::with_capacity(bar_count);
    for index in 0..bar_count {
        if index > 0 && v >= 500 {
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
            // How many times the passage is played in all; before version 5, how many
            // times after the first.
            let times = r.byte()?;
            master.repeat_count = match v >= 500 {
                true => times.max(2),
                false => times.saturating_add(1).max(2),
            };
        }
        if flags & 0x10 != 0 && v < 500 {
            // Before version 5 an ending says how many times through it is for, up to
            // which: all of them that no ending since the repeat began has taken.
            let upto = u32::from(r.byte()?.min(8));
            let taken = (master_bars.iter().rev())
                .take_while(|m: &&MasterBar| !m.repeat_start)
                .chain(master_bars.iter().rev().find(|m| m.repeat_start))
                .fold(0, |taken, m| taken | m.alternate_endings);
            master.alternate_endings = (((1u32 << upto) - 1) as u8) & !taken;
        }
        if flags & 0x20 != 0 {
            let name = r.string()?;
            r.skip(4)?; // marker colour
            master.section =
                Some(name.chars().take(120).collect()).filter(|n: &String| !n.is_empty());
        }
        if flags & 0x10 != 0 && v >= 500 {
            master.alternate_endings = r.byte()?;
        }
        if flags & 0x40 != 0 {
            master.key_signature = r.signed()?.clamp(-MasterBar::MAX_KEY, MasterBar::MAX_KEY);
            r.skip(1)?; // major or minor
        }
        if v >= 500 {
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
        } else if index == 0 && swung {
            master.feel = Some(Feel::SwingEighths);
        }
        if time_signature.is_valid() {
            master.time_signature = time_signature;
        }
        master_bars.push(master);
    }
    if v >= 500 {
        super::keep_feel_changes(&mut master_bars);
    }

    for (direction, bar) in DIRECTIONS.iter().zip(directions) {
        let Some(master) = bar.and_then(|bar| master_bars.get_mut(bar)) else {
            continue;
        };
        match *direction {
            Direction::Target(target) => master.targets.push(target),
            Direction::Jump(jump) => master.jumps.push(jump),
        }
    }

    let mut tracks = Vec::with_capacity(track_count);
    let mut infos = Vec::with_capacity(track_count);
    for index in 0..track_count {
        let flags = r.byte()?;
        if v >= 500 && (index == 0 || !newer) {
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
        if v >= 500 {
            r.skip(if newer { 49 } else { 44 })?;
        }
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
    if v >= 500 {
        r.skip(if newer { 1 } else { 2 })?;
    }

    // Version 5 gave a bar a second voice.
    let voice_count = if v >= 500 { 2 } else { 1 };
    // Tempos that are reached by degrees: the bar and the moment in it each starts from,
    // the tempo, and the quarter notes it takes to get there.
    let mut arriving: Vec<(usize, gtr_model::Rational, u32, u8)> = Vec::new();
    for (index, master) in master_bars.iter_mut().enumerate() {
        for (track, info) in tracks.iter_mut().zip(&mut infos) {
            let mut voices = Vec::with_capacity(voice_count);
            for _ in 0..voice_count {
                let mut beats = Vec::new();
                let mut any_real = false;
                let mut time = gtr_model::Rational::from_integer(0);
                for _ in 0..r.count("beat count", 10_000)? {
                    let (beat, empty, tempo) = read_beat(&mut r, info, v, skipped)?;
                    match tempo {
                        Some((tempo, 0)) => super::set_tempo(master, time, tempo, false),
                        Some((tempo, over)) => arriving.push((index, time, tempo, over)),
                        None => {}
                    }
                    time += beat.duration.length();
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
            // From version 5 a line break flag follows each bar; some files leave it off
            // the very last one.
            if v >= 500 && r.position < r.data.len() {
                r.skip(1)?;
            }
            track.staves[0].bars.push(bar(voices));
        }
    }

    for track in &mut tracks {
        keep_dynamic_changes(&mut track.staves[0]);
    }
    // A tempo reached by degrees is, in the model, a gradual change that starts where the
    // file has it, from the tempo in force there, and a change to the new tempo where it
    // arrives.
    arriving.sort_by_key(|a| (a.0, a.1));
    for (bar, position, to, over) in arriving {
        let zero = gtr_model::Rational::from_integer(0);
        let mut from = tempo;
        for (index, master) in master_bars.iter().enumerate().take(bar + 1) {
            let before = |at: gtr_model::Rational| index < bar || at <= position;
            from = master.tempo.filter(|_| before(zero)).unwrap_or(from);
            let mut inside = master.tempo_changes.iter().filter(|c| before(c.position));
            from = inside.next_back().map_or(from, |c| c.tempo);
        }
        super::set_tempo(&mut master_bars[bar], position, from, true);
        let (mut end, mut at) = (bar, position + gtr_model::Rational::new(over.into(), 4));
        while end + 1 < master_bars.len() && at >= master_bars[end].time_signature.bar_length() {
            at -= master_bars[end].time_signature.bar_length();
            end += 1;
        }
        // Past the end of the piece, it arrives at the start of its last bar.
        let at = match at < master_bars[end].time_signature.bar_length() {
            true => at,
            false => zero,
        };
        if (end, at) != (bar, position) {
            super::set_tempo(&mut master_bars[end], at, to, false);
        }
    }
    let sung = usize::try_from(lyrics.track.saturating_sub(1))
        .ok()
        .and_then(|track| tracks.get_mut(track));
    if let Some(track) = sung {
        for (line, (start, words)) in lyrics.lines.iter().enumerate() {
            let start = usize::try_from(start.saturating_sub(1)).unwrap_or(0);
            super::set_lyrics(&mut track.staves[0], line, start, words);
        }
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
        effects: Vec::new(),
        style: Default::default(),
        audio: None,
    })
}

/// Effects Guitar Pro 5 stores on the beat that the model keeps on each note, and the
/// beat's own effects.
#[derive(Default)]
struct BeatWide {
    vibrato: Option<Vibrato>,
    tapped: bool,
    /// A harmonic that version 3 marks on the beat, for all its notes.
    harmonic: Option<Harmonic>,
    natural_harmonic: bool,
    effects: BeatEffects,
}

/// The lyrics of a file: the track they belong to, counted from one, and five lines, each
/// with the bar it starts in, counted from one, and its words.
#[derive(Default)]
struct Lyrics {
    track: i32,
    lines: [(i32, String); 5],
}

/// Reads a chord: its name, and the fret of each string from the highest, with -1 for a
/// string that is not played. The three versions each have a layout of their own, and the
/// older two also the layout of the version before them.
fn read_chord(r: &mut Reader, v: u32, strings: usize) -> Result<(String, Option<ChordDiagram>)> {
    let frets = |r: &mut Reader, count: usize| -> Result<Vec<i8>> {
        let mut read = Vec::with_capacity(count);
        for _ in 0..count {
            read.push(r.int()?.clamp(-1, i32::from(ChordDiagram::MAX_FRET)) as i8);
        }
        Ok(read)
    };
    let (name, mut frets) = if v >= 500 {
        r.skip(17)?;
        let name = r.fixed_string(21)?;
        r.skip(8)?; // padding and the first fret shown
        let frets = frets(r, 7)?;
        r.skip(32)?; // barres and fingering
        (name, frets)
    } else if r.byte()? != 0 {
        if v >= 400 {
            r.skip(16)?;
            let name = r.fixed_string(21)?;
            r.skip(8)?;
            let frets = frets(r, 7)?;
            r.skip(32)?;
            (name, frets)
        } else {
            r.skip(25)?;
            let name = r.fixed_string(34)?;
            r.skip(4)?;
            let frets = frets(r, 6)?;
            r.skip(36)?;
            (name, frets)
        }
    } else {
        // The oldest layout: a name, and frets only if the chord has a diagram.
        let name = r.string()?;
        let first = r.int()?;
        let frets = match first > 0 {
            true => frets(r, if v >= 406 { 7 } else { 6 })?,
            false => Vec::new(),
        };
        (name, frets)
    };
    frets.truncate(strings);
    let diagram = Some(ChordDiagram { frets })
        .filter(|d| d.frets.len() == strings && d.frets.iter().any(|f| *f >= 0));
    Ok((
        name.trim().chars().take(BeatEffects::MAX_TEXT).collect(),
        diagram,
    ))
}

/// A beat as read: the beat, whether it is a placeholder rather than a note or written
/// rest, and a tempo that starts on it with the quarter notes it takes to get there.
type ReadBeat = (Beat, bool, Option<(u32, u8)>);

/// Reads one beat.
fn read_beat(
    r: &mut Reader,
    track: &mut TrackInfo,
    v: u32,
    skipped: &mut Skipped,
) -> Result<ReadBeat> {
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
        5 => Some(NoteValue::HundredTwentyEighth),
        6 => Some(NoteValue::TwoHundredFiftySixth),
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

    let chord = match flags & 0x02 != 0 {
        true => Some(read_chord(r, v, track.string_count)?),
        false => None,
    };
    let text = match flags & 0x04 != 0 {
        true => Some(r.string()?),
        false => None,
    };
    let mut wide = match flags & 0x08 != 0 {
        true => read_beat_effects(r, v, skipped)?,
        false => BeatWide::default(),
    };
    let mut tempo = None;
    if flags & 0x10 != 0 {
        let mix = read_mix_change(r, v)?;
        tempo = mix.tempo.map(|tempo| (tempo, mix.over));
        (wide.effects.volume, wide.effects.pan) = (mix.volume, mix.pan);
    }
    if let Some((name, diagram)) = chord {
        wide.effects.chord = Some(name).filter(|name| !name.is_empty());
        wide.effects.diagram = diagram;
    }
    wide.effects.text = text
        .map(|text| text.trim().chars().take(BeatEffects::MAX_TEXT).collect())
        .filter(|text: &String| !text.is_empty());

    // One bit per string, the highest string in bit 6.
    let strings = r.byte()?;
    let mut notes = Vec::new();
    for string in 0..7u8 {
        if strings & (0x40 >> string) != 0 {
            let read = read_note(r, string, track, v, skipped)?;
            wide.natural_harmonic |= read.note.is_some() && wide.harmonic.is_some();
            wide.effects.tremolo = wide.effects.tremolo.or(read.tremolo);
            // The first note's dynamic stands for the beat.
            wide.effects.dynamic = wide.effects.dynamic.or(Some(read.dynamic));
            if let Some(mut note) = read.note {
                note.effects.vibrato = note.effects.vibrato.or(wide.vibrato);
                note.effects.tapped |= wide.tapped;
                note.effects.harmonic = note.effects.harmonic.or(wide.harmonic);
                notes.push(note);
            }
        }
    }

    if v >= 500 {
        r.skip(1)?;
        if r.byte()? & 0x08 != 0 {
            r.skip(1)?;
        }
    }
    let mut read = beat(read_duration, notes, track.string_count);
    if read.is_rest() {
        // A rest keeps what is written over it, and nothing that is played.
        read.effects = BeatEffects {
            chord: wide.effects.chord,
            diagram: wide.effects.diagram,
            text: wide.effects.text,
            volume: wide.effects.volume,
            pan: wide.effects.pan,
            ..BeatEffects::default()
        };
    } else {
        read.effects = wide.effects;
    }
    Ok((read, empty, tempo))
}

fn read_beat_effects(r: &mut Reader, v: u32, _skipped: &mut Skipped) -> Result<BeatWide> {
    let flags1 = r.byte()?;
    let flags2 = if v >= 400 { r.byte()? } else { 0 };
    let mut wide = BeatWide {
        // Bit 0 is finger vibrato and bit 1 vibrato with the tremolo bar; in version 3,
        // vibrato and wide vibrato.
        vibrato: match (v >= 400, flags1 & 0x03) {
            (_, 0) => None,
            (false, bits) if bits & 0x02 != 0 => Some(Vibrato::Wide),
            _ => Some(Vibrato::Slight),
        },
        ..BeatWide::default()
    };
    if v < 400 {
        // Version 3 marks harmonics on the beat, for all its notes.
        wide.harmonic = match flags1 {
            f if f & 0x04 != 0 => Some(Harmonic::Natural),
            f if f & 0x08 != 0 => Some(Harmonic::Artificial),
            _ => None,
        };
    }
    if flags1 & 0x10 != 0 {
        wide.effects.fade = Some(Fade::In);
    }
    if flags1 & 0x20 != 0 {
        let kind = r.byte()?;
        match kind {
            1 => wide.tapped = true,
            2 => wide.effects.slap = Some(Slap::Slap),
            3 => wide.effects.slap = Some(Slap::Pop),
            _ => {}
        }
        if v < 400 {
            // Version 3 has the tremolo bar here too, as a dip of so many hundredths of
            // a tone, where the later versions have a curve.
            let depth = r.int()?;
            if kind == 0 {
                let dip = [(0, 0), (30, -depth), (60, 0)].into_iter();
                wide.effects.whammy =
                    super::bend_from_points(dip).filter(|w| w.is_valid(Bend::WHAMMY_RANGE));
            }
        }
    }
    if flags2 & 0x04 != 0 {
        wide.effects.whammy = read_bend(r)?.filter(|w| w.is_valid(Bend::WHAMMY_RANGE));
    }
    if flags1 & 0x40 != 0 {
        // Strum speeds; zero means no strum that way. Version 5 has the upstroke first.
        let (first, second) = (r.byte()?, r.byte()?);
        let (up, down) = match v >= 500 {
            true => (first, second),
            false => (second, first),
        };
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

/// What a change at the mixing desk sets, of the things the model can hold.
#[derive(Default)]
struct MixChange {
    tempo: Option<u32>,
    /// Quarter notes over which the tempo is reached; zero for at once.
    over: u8,
    volume: Option<u8>,
    pan: Option<i8>,
}

fn read_mix_change(r: &mut Reader, v: u32) -> Result<MixChange> {
    r.skip(1)?; // instrument
    if v >= 500 {
        r.skip(16)?;
    }
    // Volume, pan, chorus, reverb, phaser, tremolo: -1 where there is no change.
    let mut values = [0i8; 6];
    for value in &mut values {
        *value = r.signed()?;
    }
    if v >= 500 {
        r.string()?; // tempo name
    }
    let tempo = r.int()?;
    for value in values {
        if value >= 0 {
            r.skip(1)?; // transition length
        }
    }
    let mut over = 0;
    if tempo >= 0 {
        // Over how many beats the tempo gets there; none, and it changes at once.
        over = r.signed()?.max(0) as u8;
        if v > 500 {
            r.skip(1)?;
        }
    }
    if v >= 400 {
        r.skip(1)?; // which tracks it applies to
    }
    if v >= 500 {
        r.skip(1)?; // wah
    }
    if v > 500 {
        r.string()?;
        r.string()?;
    }
    // Guitar Pro counts both in sixteenths, with 13 its usual volume and 8 the centre.
    let [volume, pan, ..] = values;
    Ok(MixChange {
        tempo: u32::try_from(tempo).ok().filter(|t| (1..=1000).contains(t)),
        over,
        volume: u8::try_from(volume).ok().map(|volume| {
            (u32::from(volume) * u32::from(Mix::UNITY) / 13).min(u32::from(Mix::MAX_VOLUME)) as u8
        }),
        pan: (pan >= 0).then(|| ((i32::from(pan) - 8) * 25 / 2).clamp(-100, 100) as i8),
    })
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
    v: u32,
    skipped: &mut Skipped,
) -> Result<ReadNote> {
    let flags = r.byte()?;
    let kind = if flags & 0x20 != 0 { r.byte()? } else { 1 };
    if flags & 0x01 != 0 && v < 500 {
        r.skip(2)?; // a length of its own, apart from the beat's
    }
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
    if v >= 500 {
        if flags & 0x01 != 0 {
            r.skip(8)?;
        }
        r.skip(1)?;
    }
    let (mut effects, tremolo) = match flags & 0x08 != 0 {
        true => read_note_effects(r, v, skipped)?,
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
    v: u32,
    _skipped: &mut Skipped,
) -> Result<(NoteEffects, Option<NoteValue>)> {
    let flags1 = r.byte()?;
    let flags2 = if v >= 400 { r.byte()? } else { 0 };
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
        // Whether it is on the beat came with version 5.
        let grace_flags = if v >= 500 { r.byte()? } else { 0 };
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
        effects.slide = match v >= 500 {
            true => super::slide_from_flags(r.byte()?),
            // Version 4 has one kind of slide to a note, numbered.
            false => match r.signed()? {
                -2 => Some(Slide::InFromAbove),
                -1 => Some(Slide::InFromBelow),
                1 => Some(Slide::Shift),
                2 => Some(Slide::Legato),
                3 => Some(Slide::OutDown),
                4 => Some(Slide::OutUp),
                _ => None,
            },
        };
    } else if v < 400 && flags1 & 0x04 != 0 {
        // Version 3 has the one slide.
        effects.slide = Some(Slide::Shift);
    }
    if flags2 & 0x10 != 0 && v < 500 {
        // Before version 5 the kind says it all: the artificial harmonics are numbered
        // by how far up the string they are touched.
        effects.harmonic = match r.byte()? {
            3 => Some(Harmonic::Tapped),
            4 => Some(Harmonic::Pinch),
            5 => Some(Harmonic::Semi),
            15 | 17 | 22 => Some(Harmonic::Artificial),
            _ => Some(Harmonic::Natural),
        };
    } else if flags2 & 0x10 != 0 {
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
