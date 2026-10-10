//! Guitar Pro 5 (`.gp5`): a little-endian binary stream read front to back. Nothing in it
//! is length-prefixed as a whole, so every field has to be read or skipped, including the
//! ones the importer has no use for.

use gtr_model::{
    Beat, MasterBar, Note, NoteValue, Score, ScoreInfo, Staff, TimeSignature, Track, Voice,
};

use super::{ImportError, Skipped, bar, beat, duration, tuning};

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
    r.string()?; // subtitle
    let artist = r.string()?;
    let album = r.string()?;
    for _ in 0..5 {
        r.string()?; // words, music, copyright, tab author, instructions
    }
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
    r.skip(64 * 12)?; // MIDI channel settings
    r.skip(42)?; // bar numbers of codas, segnos and similar directions

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
        skipped.add_if(flags & 0x04 != 0, "repeats");
        if flags & 0x08 != 0 {
            r.skip(1)?; // repeat count
            skipped.add("repeats");
        }
        if flags & 0x20 != 0 {
            r.string()?; // marker name
            r.skip(4)?; // marker colour
            skipped.add("section markers");
        }
        if flags & 0x10 != 0 {
            r.skip(1)?;
            skipped.add("alternate endings");
        }
        if flags & 0x40 != 0 {
            r.skip(2)?; // key signature
        }
        if flags & 0x03 != 0 {
            r.skip(4)?; // beaming
        }
        if flags & 0x10 == 0 {
            r.skip(1)?;
        }
        skipped.add_if(r.byte()? != 0, "triplet feel");
        master_bars.push(MasterBar {
            time_signature: match time_signature.is_valid() {
                true => time_signature,
                false => TimeSignature::COMMON,
            },
        });
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
        r.skip(12)?; // MIDI port and channels
        r.skip(4)?; // fret count
        let capo = r.int()?.clamp(0, 24) as u8;
        r.skip(4)?; // colour
        r.skip(if newer { 49 } else { 44 })?;
        if newer {
            r.string()?;
            r.string()?;
        }
        let percussion = flags & 0x01 != 0;
        skipped.add_if(percussion, "percussion and untuned tracks");
        let staff_tuning = tuning(strings);
        infos.push(TrackInfo {
            percussion,
            string_count: staff_tuning.strings.len(),
            last_fret: [0; 7],
        });
        tracks.push(Track {
            name,
            staves: vec![Staff {
                tuning: staff_tuning,
                capo,
                bars: Vec::with_capacity(bar_count),
            }],
        });
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

    Ok(Score {
        info: ScoreInfo {
            title,
            artist,
            album,
        },
        tempo,
        master_bars,
        tracks,
    })
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
    if flags & 0x08 != 0 {
        read_beat_effects(r, skipped)?;
    }
    if flags & 0x10 != 0 {
        read_mix_change(r, newer, skipped)?;
    }

    // One bit per string, the highest string in bit 6.
    let strings = r.byte()?;
    let mut notes = Vec::new();
    for string in 0..7u8 {
        if strings & (0x40 >> string) != 0
            && let Some(note) = read_note(r, string, track, skipped)?
        {
            notes.push(note);
        }
    }

    r.skip(1)?;
    if r.byte()? & 0x08 != 0 {
        r.skip(1)?;
    }
    Ok((beat(read_duration, notes, track.string_count), empty))
}

fn read_beat_effects(r: &mut Reader, skipped: &mut Skipped) -> Result<()> {
    let flags1 = r.byte()?;
    let flags2 = r.byte()?;
    skipped.add_if(flags1 & 0x03 != 0, "vibrato");
    if flags1 & 0x20 != 0 {
        r.skip(1)?;
        skipped.add("tapping");
    }
    if flags2 & 0x04 != 0 {
        read_bend(r)?;
        skipped.add("whammy bar");
    }
    if flags1 & 0x40 != 0 {
        r.skip(2)?;
        skipped.add("strum directions");
    }
    if flags2 & 0x02 != 0 {
        r.skip(1)?;
        skipped.add("pick strokes");
    }
    Ok(())
}

/// Skips a bend or whammy bar curve.
fn read_bend(r: &mut Reader) -> Result<()> {
    r.skip(5)?; // type and height
    let points = r.count("bend point count", 1_000)?;
    r.skip(points * 9)
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

fn read_note(
    r: &mut Reader,
    string: u8,
    track: &mut TrackInfo,
    skipped: &mut Skipped,
) -> Result<Option<Note>> {
    let flags = r.byte()?;
    skipped.add_if(flags & 0x42 != 0, "accents");
    skipped.add_if(flags & 0x04 != 0, "ghost notes");
    let kind = if flags & 0x20 != 0 { r.byte()? } else { 1 };
    if flags & 0x10 != 0 {
        r.skip(1)?; // dynamic
    }
    let mut fret = 0;
    if flags & 0x20 != 0 {
        fret = r.signed()?.clamp(0, 99) as u8;
    }
    if flags & 0x80 != 0 {
        r.skip(2)?; // fingering
    }
    if flags & 0x01 != 0 {
        r.skip(8)?;
    }
    r.skip(1)?;
    if flags & 0x08 != 0 {
        read_note_effects(r, skipped)?;
    }

    let tied = kind == 2;
    skipped.add_if(kind == 3, "dead notes");
    let memory = &mut track.last_fret[usize::from(string)];
    if tied {
        fret = *memory;
    } else {
        *memory = fret;
    }
    // Percussion tracks store drum numbers as frets, which mean nothing as tablature.
    Ok((!track.percussion).then_some(Note { string, fret, tied }))
}

fn read_note_effects(r: &mut Reader, skipped: &mut Skipped) -> Result<()> {
    let flags1 = r.byte()?;
    let flags2 = r.byte()?;
    skipped.add_if(flags1 & 0x02 != 0, "hammer-ons and pull-offs");
    skipped.add_if(flags1 & 0x08 != 0, "let ring");
    skipped.add_if(flags2 & 0x01 != 0, "staccato");
    skipped.add_if(flags2 & 0x02 != 0, "palm muting");
    skipped.add_if(flags2 & 0x40 != 0, "vibrato");
    if flags1 & 0x01 != 0 {
        read_bend(r)?;
        skipped.add("bends");
    }
    if flags1 & 0x10 != 0 {
        r.skip(5)?;
        skipped.add("grace notes");
    }
    if flags2 & 0x04 != 0 {
        r.skip(1)?;
        skipped.add("tremolo picking");
    }
    if flags2 & 0x08 != 0 {
        r.skip(1)?;
        skipped.add("slides");
    }
    if flags2 & 0x10 != 0 {
        match r.byte()? {
            2 => r.skip(3)?,
            3 => r.skip(1)?,
            _ => {}
        }
        skipped.add("harmonics");
    }
    if flags2 & 0x20 != 0 {
        r.skip(2)?;
        skipped.add("trills");
    }
    Ok(())
}
