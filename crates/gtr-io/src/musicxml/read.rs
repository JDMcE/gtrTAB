//! MusicXML import. A part becomes a track. Where a part has a tablature staff, or its
//! notes say their strings and frets, those are kept; a part for a guitar or a bass
//! that says neither has its notes put on the strings nearest the nut. A part for
//! anything else becomes a piano, whose notes are pitches.

use std::collections::HashMap;
use std::io::{Cursor, Read};

use gtr_model::{
    Accent, Bar, Beat, BeatEffects, Bend, BendKind, ChordDiagram, Duration, Feel, Finger, Grace,
    GraceNote, Hairpin, Harmonic, Instrument, Jump, MasterBar, Note, NoteEffects, NoteValue,
    PickStroke, Rational, Score, ScoreInfo, Slap, Slide, Staff, Strum, Target, TimeSignature,
    Track, Trill, Tuning, Tuplet, Vibrato, Voice, standard_kit,
};
use roxmltree::{Document, Node, ParsingOptions};
use zip::ZipArchive;

use super::write::{
    DYNAMICS, FEELS, KINDS, PALM_MUTE, POP, SLAP, TRILL_TO, TYPES, WIDE_VIBRATO, harmonic_name,
};
use crate::util::{decode, durations_for, pitch_of, place};
use crate::{ImportError, Skipped};

fn corrupt(what: impl Into<String>) -> ImportError {
    ImportError::Corrupt(what.into())
}

/// The score's XML out of a compressed file: the file its container names, or failing
/// that the first that is not the container's own.
fn unzip(bytes: &[u8]) -> Result<String, ImportError> {
    let mut archive =
        ZipArchive::new(Cursor::new(bytes)).map_err(|_| ImportError::UnknownFormat)?;
    let mut entry = |name: &str| -> Option<Vec<u8>> {
        let mut out = Vec::new();
        archive.by_name(name).ok()?.read_to_end(&mut out).ok()?;
        Some(out)
    };
    let named = entry("META-INF/container.xml").and_then(|container| {
        let text = decode(&container);
        let document = Document::parse(&text).ok()?;
        let root = document
            .descendants()
            .find(|n| n.has_tag_name("rootfile"))?;
        Some(root.attribute("full-path")?.to_owned())
    });
    let name = match named {
        Some(name) => name,
        None => (archive.file_names())
            .find(|name| {
                !name.starts_with("META-INF")
                    && (name.ends_with(".xml") || name.ends_with(".musicxml"))
            })
            .map(str::to_owned)
            .ok_or(ImportError::UnknownFormat)?,
    };
    let mut out = Vec::new();
    archive
        .by_name(&name)
        .map_err(|_| ImportError::UnknownFormat)?
        .read_to_end(&mut out)
        .map_err(|e| corrupt(e.to_string()))?;
    Ok(decode(&out))
}

fn child<'a>(node: Node<'a, 'a>, name: &str) -> Option<Node<'a, 'a>> {
    node.children().find(|n| n.has_tag_name(name))
}

fn children<'a>(node: Node<'a, 'a>, name: &'a str) -> impl Iterator<Item = Node<'a, 'a>> {
    node.children().filter(move |n| n.has_tag_name(name))
}

fn text<'a>(node: Node<'a, 'a>, name: &str) -> &'a str {
    child(node, name)
        .and_then(|n| n.text())
        .unwrap_or("")
        .trim()
}

fn number<T: std::str::FromStr>(node: Node, name: &str) -> Option<T> {
    text(node, name).parse().ok()
}

/// A node somewhere below another, by name.
fn find<'a>(node: Node<'a, 'a>, name: &str) -> Option<Node<'a, 'a>> {
    node.descendants().find(|n| n.has_tag_name(name))
}

fn clip(text: &str) -> String {
    text.chars().take(BeatEffects::MAX_TEXT).collect()
}

pub(super) fn read(bytes: &[u8], skipped: &mut Skipped) -> Result<Score, ImportError> {
    let xml = match bytes.starts_with(b"PK") {
        true => unzip(bytes)?,
        false => decode(bytes),
    };
    let options = ParsingOptions {
        allow_dtd: true,
        ..ParsingOptions::default()
    };
    let document =
        Document::parse_with_options(&xml, options).map_err(|e| match xml.contains("<score-") {
            true => corrupt(e.to_string()),
            false => ImportError::UnknownFormat,
        })?;
    let root = document.root_element();
    let timewise = root.has_tag_name("score-timewise");
    if !timewise && !root.has_tag_name("score-partwise") {
        return Err(ImportError::UnknownFormat);
    }

    let mut info = ScoreInfo::default();
    let work = find(root, "work-title")
        .and_then(|n| n.text())
        .unwrap_or("")
        .trim();
    let movement = text(root, "movement-title");
    match (movement, work) {
        ("", title) => info.title = title.into(),
        (title, album) => {
            info.title = title.into();
            info.album = album.into();
        }
    }
    if let Some(identification) = child(root, "identification") {
        for creator in children(identification, "creator") {
            let who = creator.text().unwrap_or("").trim().to_owned();
            match creator.attribute("type").unwrap_or("") {
                "composer" => info.music = who,
                "lyricist" | "poet" => info.words = who,
                "artist" => info.artist = who,
                "transcriber" | "arranger" => info.transcriber = who,
                "subtitle" => info.subtitle = who,
                _ => {}
            }
        }
        info.copyright = text(identification, "rights").into();
    }

    // The bars of each part, in order. A timewise score lists its parts bar by bar.
    let mut parts: Vec<(Option<&str>, Vec<Node>)> = Vec::new();
    if timewise {
        for part in children(root, "measure").flat_map(|measure| children(measure, "part")) {
            let id = part.attribute("id");
            match parts.iter_mut().find(|found| found.0 == id) {
                Some(found) => found.1.push(part),
                None => parts.push((id, vec![part])),
            }
        }
    } else {
        parts.extend(
            children(root, "part").map(|p| (p.attribute("id"), children(p, "measure").collect())),
        );
    }
    let count = parts.iter().map(|p| p.1.len()).max().unwrap_or(0);
    if parts.is_empty() || count == 0 {
        return Err(corrupt("the score has no music"));
    }
    let mut global = Global {
        masters: vec![MasterBar::new(TimeSignature::COMMON); count],
        tempo: None,
        set: vec![Set::default(); count],
    };
    let listed: HashMap<&str, Node> = (find(root, "part-list").into_iter())
        .flat_map(|list| children(list, "score-part"))
        .filter_map(|part| Some((part.attribute("id")?, part)))
        .collect();
    let mut tracks = Vec::new();
    for (id, measures) in &parts {
        let listed = id.and_then(|id| listed.get(id)).copied();
        tracks.push(read_part(measures, listed, count, &mut global, skipped));
    }
    // A key or a time lasts until the next.
    let (mut time, mut key) = (TimeSignature::COMMON, 0);
    for (master, set) in global.masters.iter_mut().zip(&global.set) {
        match set.time {
            true => time = master.time_signature,
            false => master.time_signature = time,
        }
        match set.key {
            true => key = master.key_signature,
            false => master.key_signature = key,
        }
        // A change of tempo that falls outside its bar, now that the bar's length is
        // known, is left out.
        let length = master.time_signature.bar_length();
        master
            .tempo_changes
            .retain(|change| change.position < length);
    }
    Ok(Score {
        info,
        tempo: global.tempo.unwrap_or(120),
        master_bars: global.masters,
        tracks,
    })
}

/// What the parts say between them of the bars they share.
struct Global {
    masters: Vec<MasterBar>,
    /// The tempo the piece starts at, once a part has said it.
    tempo: Option<u32>,
    set: Vec<Set>,
}

/// What of a bar has been said already, so that the first part to say it is believed.
#[derive(Clone, Default)]
struct Set {
    time: bool,
    key: bool,
    tempo: bool,
    marks: bool,
}

/// What a part turns out to be for.
enum Kind {
    Drums,
    /// On strings, each staff with its tuning and capo.
    Strings,
    Pitched,
    /// The staff of notation that a staff of tablature repeats: its notes are read as
    /// pitches, with everything a note on strings can have.
    Shadow,
}

/// What is waiting for the next note of a staff.
#[derive(Default)]
struct Pending {
    text: Option<String>,
    dynamic: Option<gtr_model::Dynamic>,
    chord: Option<String>,
    diagram: Option<ChordDiagram>,
}

/// A hairpin that has opened and not yet closed.
#[derive(Clone)]
struct Wedge {
    kind: Hairpin,
    /// The bar it opened in, and how far through it.
    bar: usize,
    from: Rational,
    /// The voice it is for, if it says.
    voice: Option<String>,
}

/// Gives a chord's grace notes to the note on the string of the first of them, or
/// failing that to the chord's first note.
fn rehome_grace(beat: &mut Beat) {
    let Some(grace) = beat.notes.iter_mut().find_map(|n| n.effects.grace.take()) else {
        return;
    };
    let string = grace.notes.first().map(|g| g.string);
    let owner = (beat.notes.iter().position(|n| Some(n.string) == string)).unwrap_or(0);
    beat.notes[owner].effects.grace = Some(grace);
}

/// The fret a trill on a staff of notation goes to when it does not say: the note of the
/// tablature it is given to decides.
const UNSAID: u8 = u8::MAX;

/// Gives what a staff of notation says to the tablature under it: each of its beats to
/// the beat of the tablature that starts at the same moment, and each of its notes to
/// the note there that sounds the same pitch.
fn merge_shadow(staff: &Staff, lines: &mut [Line], shadow: Vec<Line>) {
    for line in shadow {
        let mut start = Rational::from_integer(0);
        for said in line.beats {
            let at = start;
            start += said.duration.length();
            if said.is_rest() && said.effects.is_none() {
                continue;
            }
            let pitches: Vec<u8> = said.notes.iter().map(|n| n.string).collect();
            let sounds = |beat: &Beat| {
                (beat.notes.iter()).any(|n| {
                    (staff.pitch(n.string, n.fret)).is_some_and(|pitch| pitches.contains(&pitch))
                })
            };
            // The beats that start at that moment, one in each voice at most; the one
            // that shares a note with what was said, if any does.
            let mut found: Vec<&mut Beat> = (lines.iter_mut())
                .filter_map(|line| {
                    let mut time = Rational::from_integer(0);
                    line.beats.iter_mut().find(|beat| {
                        let begins = time;
                        time += beat.duration.length();
                        begins == at
                    })
                })
                .collect();
            let chosen = found.iter().position(|beat| sounds(beat)).unwrap_or(0);
            if found.is_empty() {
                continue;
            }
            let beat = found.swap_remove(chosen);

            let (to, from) = (&mut beat.effects, said.effects);
            to.tremolo = to.tremolo.or(from.tremolo);
            to.stroke = to.stroke.or(from.stroke);
            to.strum = to.strum.or(from.strum);
            to.slap = to.slap.or(from.slap);
            to.dynamic = to.dynamic.or(from.dynamic);
            to.hairpin = to.hairpin.or(from.hairpin);
            to.fermata |= from.fermata;
            to.chord = to.chord.take().or(from.chord);
            to.diagram = to.diagram.take().or(from.diagram);
            to.text = to.text.take().or(from.text);
            if to.lyrics.is_empty() {
                to.lyrics = from.lyrics;
            }
            let mut given = vec![false; beat.notes.len()];
            for note in said.notes {
                let same = (0..beat.notes.len()).find(|index| {
                    let there = &beat.notes[*index];
                    !given[*index] && staff.pitch(there.string, there.fret) == Some(note.string)
                });
                let Some(index) = same else {
                    continue;
                };
                given[index] = true;
                let there = &mut beat.notes[index];
                there.tied |= note.tied;
                let (to, from) = (&mut there.effects, note.effects);
                to.hammer |= from.hammer;
                to.slide = to.slide.or(from.slide);
                to.bend = to.bend.take().or(from.bend);
                to.vibrato = to.vibrato.or(from.vibrato);
                to.harmonic = to.harmonic.or(from.harmonic);
                to.accent = to.accent.or(from.accent);
                to.trill = to.trill.or(from.trill.map(|trill| Trill {
                    fret: match trill.fret {
                        UNSAID => there.fret.saturating_add(2),
                        fret => fret,
                    },
                    ..trill
                }));
                to.left_finger = to.left_finger.or(from.left_finger);
                to.right_finger = to.right_finger.or(from.right_finger);
                to.palm_mute |= from.palm_mute;
                to.let_ring |= from.let_ring;
                to.dead |= from.dead;
                to.ghost |= from.ghost;
                to.staccato |= from.staccato;
                to.tapped |= from.tapped;
            }
        }
    }
}

/// One voice of one staff while a bar is read.
#[derive(Default)]
struct Line {
    beats: Vec<Beat>,
    /// How far through the bar the voice has got, in whole notes.
    cursor: Rational,
    /// Grace notes waiting for the note they lead into.
    graces: Vec<(GraceNote, bool, bool)>,
}

fn read_part(
    measures: &[Node],
    listed: Option<Node>,
    count: usize,
    global: &mut Global,
    skipped: &mut Skipped,
) -> Track {
    let name = listed.map_or("", |l| text(l, "part-name"));
    let program = listed
        .and_then(|l| find(l, "midi-program"))
        .and_then(|n| n.text()?.trim().parse::<u32>().ok())
        .map(|program| program.clamp(1, 128) as u8 - 1);
    let channel = listed
        .and_then(|l| find(l, "midi-channel"))
        .and_then(|n| n.text()?.trim().parse::<u32>().ok());
    // The General MIDI number each instrument of a drum part sounds.
    let unpitched: HashMap<&str, u8> = (listed.into_iter())
        .flat_map(|l| children(l, "midi-instrument"))
        .filter_map(|midi| {
            let number = number::<u32>(midi, "midi-unpitched")?;
            Some((
                midi.attribute("id")?,
                u8::try_from(number.checked_sub(1)?).ok()?,
            ))
        })
        .collect();

    // What the part's staves are: their clefs, their strings, and whether notes say
    // where on the strings they are played.
    let mut staves = 1;
    let mut tab: Vec<usize> = Vec::new();
    let mut percussion = channel == Some(10);
    let mut tunings: HashMap<usize, (Vec<u8>, u8)> = HashMap::new();
    let mut fretted = false;
    let mut transpose = 0i32;
    for node in measures.iter().flat_map(|measure| measure.descendants()) {
        match node.tag_name().name() {
            "staves" => {
                staves = staves.max(node.text().and_then(|t| t.trim().parse().ok()).unwrap_or(1))
            }
            "staff" => {
                staves = staves.max(node.text().and_then(|t| t.trim().parse().ok()).unwrap_or(1))
            }
            "clef" => {
                let staff = node
                    .attribute("number")
                    .and_then(|n| n.parse().ok())
                    .unwrap_or(1);
                match text(node, "sign") {
                    "TAB" if !tab.contains(&staff) => tab.push(staff),
                    "percussion" => percussion = true,
                    _ => {}
                }
            }
            "unpitched" => percussion = true,
            "staff-details" => {
                let staff = node
                    .attribute("number")
                    .and_then(|n| n.parse().ok())
                    .unwrap_or(1);
                // Listed by line from the lowest string; here the highest comes first.
                let mut strings: Vec<(u32, u8)> = children(node, "staff-tuning")
                    .filter_map(|tuning| {
                        let line = tuning.attribute("line")?.parse().ok()?;
                        let step = text(tuning, "tuning-step").chars().next()?;
                        let alter = number::<f64>(tuning, "tuning-alter").unwrap_or(0.0) as i32;
                        let octave = number(tuning, "tuning-octave")?;
                        Some((line, pitch_of(step, alter, octave)?))
                    })
                    .collect();
                strings.sort_by_key(|(line, _)| std::cmp::Reverse(*line));
                let capo = number::<u8>(node, "capo").unwrap_or(0).min(24);
                if !strings.is_empty() && strings.len() <= Tuning::MAX_STRINGS {
                    let strings = strings.into_iter().map(|(_, pitch)| pitch).collect();
                    tunings.entry(staff).or_insert((strings, capo));
                }
            }
            "transpose" if transpose == 0 => {
                transpose = number::<f64>(node, "chromatic").unwrap_or(0.0) as i32
                    + 12 * number::<i32>(node, "octave-change").unwrap_or(0);
            }
            "technical" => {
                fretted |= child(node, "fret").is_some() && child(node, "string").is_some()
            }
            _ => {}
        }
    }

    // An instrument named as gtrTab names it is that one: General MIDI has no number
    // for a ukulele or a mandolin, so the program alone would not say.
    let by_name = listed
        .and_then(|l| find(l, "instrument-name"))
        .and_then(|n| {
            let name = n.text()?.trim();
            Instrument::ALL
                .into_iter()
                .find(|i| i.name().eq_ignore_ascii_case(name))
        });
    let from_program = by_name.or(program.map(Instrument::from_midi_program));
    // A part that does not say what plays it is taken at its name, or by its strings.
    let lowest = tunings
        .values()
        .filter_map(|(strings, _)| strings.last())
        .min();
    let named = name.to_lowercase();
    let guessed = [
        ("bass", Instrument::Bass),
        ("banjo", Instrument::Banjo),
        ("ukulele", Instrument::Ukulele),
        ("mandolin", Instrument::Mandolin),
        ("nylon", Instrument::NylonGuitar),
        ("classical", Instrument::NylonGuitar),
    ]
    .into_iter()
    .find(|(word, _)| named.contains(word))
    .map(|found| found.1)
    .unwrap_or(match lowest {
        Some(pitch) if *pitch < 36 => Instrument::Bass,
        _ => Instrument::default(),
    });
    let stringed = matches!(program, Some(24..=39 | 105));
    let has_tab = !tab.is_empty();
    let (kind, instrument, shown): (Kind, Instrument, Vec<usize>) = if percussion {
        (Kind::Drums, Instrument::Drums, vec![1])
    } else if !tab.is_empty() {
        // The tablature says everything its notation does, and where it is played.
        tab.sort_unstable();
        tab.truncate(1);
        let instrument = from_program.filter(|i| !i.is_pitched()).unwrap_or(guessed);
        (Kind::Strings, instrument, tab)
    } else if fretted || !tunings.is_empty() || (stringed && staves == 1) {
        let instrument = from_program.filter(|i| !i.is_pitched()).unwrap_or(guessed);
        (Kind::Strings, instrument, vec![1])
    } else {
        skipped.add_if(
            program.is_some_and(|p| p > 7),
            "the sounds of instruments without strings, which are played as a piano",
        );
        skipped.add_if(staves > 2, "more than two staves in a part");
        (Kind::Pitched, Instrument::Piano, vec![1, 2])
    };
    let mut track = Track::new(clip(name), instrument, Vec::new());
    for number in &shown {
        let (tuning, capo) = match kind {
            Kind::Drums => (Tuning::new([0; 5]), 0),
            Kind::Pitched | Kind::Shadow => (Tuning::none(), 0),
            Kind::Strings => match tunings.get(number) {
                Some((strings, capo)) => (Tuning::new(strings.clone()), *capo),
                None => (instrument.default_tuning(), 0),
            },
        };
        track.staves.push(Staff {
            tuning,
            capo,
            bars: Vec::new(),
        });
    }
    if let Some(listed) = listed
        && let Some(midi) = find(listed, "midi-instrument")
    {
        if let Some(volume) = number::<f64>(midi, "volume") {
            track.mix.volume = volume.round().clamp(0.0, 100.0) as u8;
        }
        if let Some(pan) = number::<f64>(midi, "pan") {
            track.mix.pan = (pan / 0.9).round().clamp(-100.0, 100.0) as i8;
        }
    }

    // A staff of tablature usually stands under a staff of notation that says the same
    // notes, and says alone what is written around them. That staff is read too, and
    // what it says is given to the notes of the tablature.
    let shadow = (1..=staves).find(|number| has_tab && !percussion && !shown.contains(number));
    let slots: Vec<usize> = shown.iter().copied().chain(shadow).collect();
    let shadow_staff = Staff {
        tuning: Tuning::none(),
        capo: 0,
        bars: Vec::new(),
    };

    let kit = standard_kit();
    let mut divisions = 1i64;
    // The voices of each staff by the names the file gives them. Most files number the
    // voices of the first staff from 1, of the second from 5 and so on, and then a voice
    // is the one its number says; otherwise they are taken in order of their names.
    let mut voices: Vec<Vec<&str>> = vec![Vec::new(); slots.len()];
    let notes =
        (measures.iter().flat_map(|measure| measure.children())).filter(|n| n.has_tag_name("note"));
    for note in notes {
        let staff = number::<usize>(note, "staff").unwrap_or(1);
        if let Some(staff) = slots.iter().position(|s| *s == staff)
            && !voices[staff].contains(&text(note, "voice"))
        {
            voices[staff].push(text(note, "voice"));
        }
    }
    for (number, names) in slots.iter().zip(&mut voices) {
        let numbered = |name: &str| name.parse::<usize>().unwrap_or(usize::MAX);
        names.sort_by_key(|name| (numbered(name), *name));
        let first = (number - 1) * Bar::MAX_VOICES + 1;
        if names
            .iter()
            .all(|name| (first..first + Bar::MAX_VOICES).contains(&numbered(name)))
        {
            let mut placed = vec![""; Bar::MAX_VOICES];
            for name in names.iter() {
                placed[numbered(name) - first] = name;
            }
            // A place no voice has is left empty; no note says an empty name here.
            *names = placed;
        }
    }
    let mut wedges: Vec<Vec<Wedge>> = vec![Vec::new(); slots.len()];
    let mut ending: u8 = 0;
    for (bar, measure) in measures.iter().enumerate() {
        let mut lines: Vec<Vec<Line>> = (0..slots.len()).map(|_| Vec::new()).collect();
        let mut pending: Vec<Pending> = (0..slots.len()).map(|_| Pending::default()).collect();
        let mut position = Rational::from_integer(0);
        // Where the last note began, which is where the notes of its chord begin.
        let mut chord_at: Option<(usize, usize)> = None;
        let whole =
            |ticks: i64, divisions: i64| Rational::new(ticks.clamp(0, 1 << 24), divisions * 4);
        let mut closes = false;
        for node in measure.children().filter(Node::is_element) {
            match node.tag_name().name() {
                "attributes" => {
                    if let Some(new) =
                        number::<i64>(node, "divisions").filter(|d| (1..=1 << 20).contains(d))
                    {
                        divisions = new;
                    }
                    let master = &mut global.masters[bar];
                    if let Some(key) = child(node, "key")
                        && !global.set[bar].key
                    {
                        let fifths = number::<i32>(key, "fifths").unwrap_or(0);
                        master.key_signature = fifths.clamp(-7, 7) as i8;
                        global.set[bar].key = true;
                    }
                    if let Some(time) = child(node, "time")
                        && !global.set[bar].time
                    {
                        // A numerator can be a sum, as in 3+2.
                        let beats: u32 = (text(time, "beats").split('+'))
                            .filter_map(|part| part.trim().parse::<u32>().ok())
                            .sum();
                        let signature = TimeSignature {
                            numerator: u8::try_from(beats).unwrap_or(4),
                            denominator: number(time, "beat-type").unwrap_or(4),
                        };
                        if signature.is_valid() {
                            master.time_signature = signature;
                            global.set[bar].time = true;
                        }
                    }
                }
                "backup" => position -= whole(number(node, "duration").unwrap_or(0), divisions),
                "forward" => position += whole(number(node, "duration").unwrap_or(0), divisions),
                "barline" => {
                    let master = &mut global.masters[bar];
                    if let Some(repeat) = child(node, "repeat") {
                        match repeat.attribute("direction") {
                            Some("forward") => master.repeat_start = true,
                            _ => {
                                let times =
                                    repeat.attribute("times").and_then(|t| t.parse::<u8>().ok());
                                master.repeat_count = times.unwrap_or(2).max(2);
                            }
                        }
                    }
                    if let Some(mark) = child(node, "ending") {
                        match mark.attribute("type") {
                            Some("start") => {
                                // The passes are listed by number.
                                ending = (mark.attribute("number").unwrap_or("").split([',', ' ']))
                                    .filter_map(|pass| pass.trim().parse::<u32>().ok())
                                    .filter(|pass| (1..=8).contains(pass))
                                    .fold(0, |mask, pass| mask | 1 << (pass - 1));
                            }
                            _ => closes = true,
                        }
                    }
                }
                "direction" => {
                    let staff = number::<usize>(node, "staff").unwrap_or(1);
                    let staff = slots.iter().position(|s| *s == staff);
                    let offset = whole(number(node, "offset").unwrap_or(0), divisions);
                    read_direction(
                        node,
                        bar,
                        position + offset,
                        staff.map(|s| (&mut pending[s], &mut wedges[s])),
                        global,
                    );
                }
                "harmony" => {
                    let staff = number::<usize>(node, "staff").unwrap_or(1);
                    if let Some(staff) = slots.iter().position(|s| *s == staff) {
                        let (name, diagram) = read_harmony(node);
                        pending[staff].chord = name;
                        pending[staff].diagram = diagram
                            .filter(|d| d.frets.len() == track.staves[0].tuning.strings.len());
                    }
                }
                "note" => {
                    let length = whole(number(node, "duration").unwrap_or(0), divisions);
                    let is_chord = child(node, "chord").is_some();
                    let grace = child(node, "grace");
                    let start = position;
                    if !is_chord && grace.is_none() {
                        position += length;
                    }
                    let staff_number = number::<usize>(node, "staff").unwrap_or(1);
                    let Some(staff) = slots.iter().position(|s| *s == staff_number) else {
                        if !is_chord {
                            chord_at = None;
                        }
                        continue;
                    };
                    if child(node, "cue").is_some() {
                        skipped.add("cue notes");
                        continue;
                    }
                    let voice_name = text(node, "voice");
                    let known = voices[staff].iter().position(|v| *v == voice_name);
                    let voice = known.unwrap_or(Bar::MAX_VOICES);
                    if voice >= Bar::MAX_VOICES {
                        skipped.add("more than four voices on a staff");
                        continue;
                    }
                    if lines[staff].len() <= voice {
                        lines[staff].resize_with(voice + 1, Line::default);
                    }
                    let (model_staff, kind) = match track.staves.get(staff) {
                        Some(staff) => (staff, &kind),
                        None => (&shadow_staff, &Kind::Shadow),
                    };
                    let read = read_note(node, kind, model_staff, transpose, &unpitched, &kit);

                    if let Some(grace) = grace {
                        if let Some((note, _)) = &read {
                            let hammer = note.effects.hammer;
                            let on_beat = grace.attribute("slash") != Some("yes");
                            let ornament = GraceNote {
                                string: note.string,
                                fret: note.fret,
                            };
                            lines[staff][voice].graces.push((ornament, on_beat, hammer));
                        }
                        continue;
                    }
                    if is_chord {
                        // Another note of the chord the last note began.
                        let Some((s, v)) = chord_at else { continue };
                        let Some(beat) = lines[s][v].beats.last_mut() else {
                            continue;
                        };
                        if let Some((mut note, _)) = read {
                            if matches!(kind, Kind::Drums) {
                                note.string = beat.notes.len() as u8;
                            } else if matches!(kind, Kind::Strings)
                                && beat.notes.iter().any(|n| n.string == note.string)
                            {
                                // Its string is taken: it goes on another that plays it.
                                let taken: Vec<u8> = beat.notes.iter().map(|n| n.string).collect();
                                let pitch = model_staff.pitch(note.string, note.fret);
                                let Some((string, fret)) = pitch.and_then(|p| {
                                    place(p, &model_staff.tuning, model_staff.capo, &taken)
                                }) else {
                                    skipped.add("notes that do not fit on the strings");
                                    continue;
                                };
                                (note.string, note.fret) = (string, fret);
                            }
                            if model_staff.places() > usize::from(note.string)
                                && beat.notes.iter().all(|n| n.string != note.string)
                            {
                                beat.notes.push(note);
                                beat.notes.sort_by_key(|n| n.string);
                                rehome_grace(beat);
                            }
                        }
                        continue;
                    }

                    let line = &mut lines[staff][voice];
                    if start < line.cursor {
                        skipped.add("notes that overlap in one voice");
                        chord_at = None;
                        continue;
                    }
                    // A stretch the voice says nothing of is rests.
                    for rest in durations_for(start - line.cursor) {
                        line.beats.push(Beat::rest(rest));
                    }
                    line.cursor = start + length;
                    let durations = match read_duration(node) {
                        Some(duration) => vec![duration],
                        None => durations_for(length),
                    };
                    let Some((first, more)) = durations.split_first() else {
                        continue;
                    };
                    let mut beat = Beat::rest(*first);
                    let mut note_effects = None;
                    if let Some((note, beat_effects)) = read {
                        if model_staff.places() > usize::from(note.string) {
                            beat.notes.push(note);
                        } else {
                            skipped.add("notes that do not fit on the strings");
                        }
                        note_effects = Some(beat_effects);
                    } else if child(node, "rest").is_none() {
                        skipped.add("notes that do not fit on the strings");
                    }
                    if let Some(effects) = note_effects {
                        beat.effects = effects;
                    }
                    beat.effects.fermata = find(node, "fermata").is_some();
                    read_lyrics(node, &mut beat);
                    // What was written before the note is the note's.
                    let waiting = std::mem::take(&mut pending[staff]);
                    beat.effects.text = waiting.text;
                    beat.effects.chord = waiting.chord;
                    beat.effects.diagram = waiting.diagram;
                    beat.effects.dynamic = waiting.dynamic.filter(|_| !beat.notes.is_empty());
                    // A hairpin that opened earlier, for every voice or for this one.
                    beat.effects.hairpin = (wedges[staff].iter())
                        .find(|wedge| {
                            !beat.notes.is_empty()
                                && (wedge.bar < bar || wedge.from <= start)
                                && (wedge.voice.as_deref()).is_none_or(|v| v == voice_name)
                        })
                        .map(|wedge| wedge.kind);
                    // And so are the grace notes that came before it.
                    if let Some(note) = beat.notes.first_mut() {
                        let graces: Vec<_> = line.graces.drain(..).take(Grace::MAX_NOTES).collect();
                        if let Some((_, on_beat, _)) = graces.first() {
                            note.effects.grace = Some(Grace {
                                notes: graces.iter().map(|g| g.0).collect(),
                                on_beat: *on_beat,
                                hammer: graces.iter().any(|g| g.2),
                            });
                        }
                    }
                    line.graces.clear();
                    line.beats.push(beat);
                    // A length no one note has is the note and rests after it.
                    line.beats.extend(more.iter().map(|rest| Beat::rest(*rest)));
                    chord_at = Some((staff, voice));
                }
                _ => {}
            }
        }
        global.masters[bar].alternate_endings |= ending;
        if closes {
            ending = 0;
        }
        for shadow in lines.split_off(track.staves.len()) {
            merge_shadow(&track.staves[0], &mut lines[0], shadow);
        }
        for (staff, lines) in track.staves.iter_mut().zip(lines) {
            let mut voices: Vec<Voice> = (lines.into_iter())
                .map(|line| Voice { beats: line.beats })
                .collect();
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
            // A bar that is one long rest is an empty bar.
            if voices.len() == 1
                && voices[0]
                    .beats
                    .iter()
                    .all(|b| b.is_rest() && b.effects.is_none())
            {
                voices[0].beats.clear();
            }
            staff.bars.push(Bar { voices });
        }
    }
    for staff in &mut track.staves {
        staff.bars.resize_with(count, Bar::empty);
    }
    track
}

/// What a direction says: of the bar, if it is a tempo, a sign, a jump or a section, and
/// otherwise of the next note of its staff.
fn read_direction(
    node: Node,
    bar: usize,
    position: Rational,
    staff: Option<(&mut Pending, &mut Vec<Wedge>)>,
    global: &mut Global,
) {
    let sound = child(node, "sound");
    let attribute = |name| sound.and_then(|s| s.attribute(name));
    let first = !global.set[bar].marks;
    let mut said = false;

    // A tempo: what the playback is told, or else what the metronome mark says.
    let tempo = (attribute("tempo").and_then(|t| t.parse::<f64>().ok())).or_else(|| {
        let mark = find(node, "metronome")?;
        let per_minute: f64 = text(mark, "per-minute")
            .trim_start_matches(|c: char| !c.is_ascii_digit())
            .parse()
            .ok()?;
        let unit = TYPES
            .iter()
            .find(|(_, name)| *name == text(mark, "beat-unit"))?
            .0;
        let dotted = child(mark, "beat-unit-dot").is_some();
        let quarters = unit.length()
            * 4
            * if dotted {
                Rational::new(3, 2)
            } else {
                Rational::from_integer(1)
            };
        Some(per_minute * (*quarters.numer() as f64 / *quarters.denom() as f64))
    });
    if let Some(tempo) = tempo.filter(|t| t.is_finite() && *t >= 1.0) {
        let tempo = tempo.round().clamp(1.0, 1000.0) as u32;
        let start = position <= Rational::from_integer(0);
        if bar == 0 && start {
            global.tempo.get_or_insert(tempo);
        } else if !global.set[bar].tempo || !start {
            crate::guitar_pro::set_tempo(&mut global.masters[bar], position, tempo, false);
        }
        if start {
            global.set[bar].tempo = true;
        }
    }

    let master = &mut global.masters[bar];
    let target = |target: Target, master: &mut MasterBar| {
        if first && !master.targets.contains(&target) {
            master.targets.push(target);
        }
    };
    if let Some(sign) = attribute("segno") {
        target(
            if sign == "segnosegno" {
                Target::SegnoSegno
            } else {
                Target::Segno
            },
            master,
        );
        said = true;
    } else if find(node, "segno").is_some() {
        target(Target::Segno, master);
        said = true;
    }
    if let Some(sign) = attribute("coda") {
        target(
            if sign == "doublecoda" {
                Target::DoubleCoda
            } else {
                Target::Coda
            },
            master,
        );
        said = true;
    } else if find(node, "coda").is_some() {
        target(Target::Coda, master);
        said = true;
    }
    let written: Vec<&str> = (node.descendants())
        .filter(|n| n.has_tag_name("words"))
        .filter_map(|n| n.text())
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .collect();
    let all = written.join(" ");
    if attribute("fine").is_some() || all.eq_ignore_ascii_case("fine") {
        target(Target::Fine, master);
        said = true;
    }
    // A jump is known by its words, or else by what the playback is told.
    const JUMPS: [Jump; 14] = [
        Jump::DaCapo,
        Jump::DaCapoAlFine,
        Jump::DaCapoAlCoda,
        Jump::DaCapoAlDoubleCoda,
        Jump::DalSegno,
        Jump::DalSegnoAlFine,
        Jump::DalSegnoAlCoda,
        Jump::DalSegnoAlDoubleCoda,
        Jump::DalSegnoSegno,
        Jump::DalSegnoSegnoAlFine,
        Jump::DalSegnoSegnoAlCoda,
        Jump::DalSegnoSegnoAlDoubleCoda,
        Jump::ToCoda,
        Jump::ToDoubleCoda,
    ];
    let by_words = JUMPS
        .into_iter()
        .find(|jump| jump.label().eq_ignore_ascii_case(&all));
    let by_sound = match (
        attribute("dacapo"),
        attribute("dalsegno"),
        attribute("tocoda"),
    ) {
        (Some("yes"), ..) => Some(Jump::DaCapo),
        (_, Some("segnosegno"), _) => Some(Jump::DalSegnoSegno),
        (_, Some(_), _) => Some(Jump::DalSegno),
        (_, _, Some("doublecoda")) => Some(Jump::ToDoubleCoda),
        (_, _, Some(_)) => Some(Jump::ToCoda),
        _ => None,
    };
    if let Some(jump) = by_words.or(by_sound) {
        if first && !master.jumps.contains(&jump) {
            master.jumps.push(jump);
        }
        said = true;
    }
    if let Some(section) = find(node, "rehearsal").and_then(|n| n.text()) {
        if first {
            master.section = Some(section.trim().chars().take(120).collect());
        }
        said = true;
    }
    if let Some(swing) = sound.and_then(|s| child(s, "swing")) {
        let feel = match child(swing, "straight") {
            Some(_) => Feel::Straight,
            None => {
                let (first, second): (u8, u8) = (
                    number(swing, "first").unwrap_or(2),
                    number(swing, "second").unwrap_or(1),
                );
                let eighths = text(swing, "swing-type") != "16th";
                (FEELS.iter())
                    .find(|(_, a, b, e)| (*a, *b, *e) == (first, second, eighths))
                    .map_or(Feel::SwingEighths, |found| found.0)
            }
        };
        if first {
            master.feel = Some(feel);
        }
        said = true;
    }
    if said || tempo.is_some() {
        return;
    }

    let Some((pending, wedges)) = staff else {
        return;
    };
    if let Some(dynamics) = find(node, "dynamics") {
        let named = dynamics.children().find(Node::is_element);
        pending.dynamic = named.and_then(|n| {
            (DYNAMICS
                .iter()
                .find(|(_, name)| *name == n.tag_name().name()))
            .map(|found| found.0)
        });
    }
    if let Some(mark) = find(node, "wedge") {
        let voice = Some(text(node, "voice")).filter(|v| !v.is_empty());
        let kind = match mark.attribute("type") {
            Some("crescendo") => Some(Hairpin::Crescendo),
            Some("diminuendo") => Some(Hairpin::Decrescendo),
            _ => None,
        };
        // One hairpin at a time for each voice that has one of its own, and one for
        // the staff.
        wedges.retain(|wedge| wedge.voice.as_deref() != voice);
        if let Some(kind) = kind {
            wedges.push(Wedge {
                kind,
                bar,
                from: position,
                voice: voice.map(str::to_owned),
            });
        }
    }
    if !all.is_empty() {
        pending.text = Some(clip(&all));
    }
}

/// A chord symbol's name, and the fingering drawn with it if there is one.
fn read_harmony(node: Node) -> (Option<String>, Option<ChordDiagram>) {
    let note = |node: Node, step: &str, alter: &str| {
        let mut name = text(node, step).to_owned();
        match number::<f64>(node, alter).unwrap_or(0.0) as i32 {
            1 => name.push('#'),
            -1 => name.push('b'),
            _ => {}
        }
        name
    };
    let kind = child(node, "kind");
    let value = kind.and_then(|k| k.text()).unwrap_or("").trim();
    // What follows the root is the file's own words for it, or else the usual ones.
    let suffix = match kind.and_then(|k| k.attribute("text")) {
        Some(text) => text.to_owned(),
        None => (KINDS.iter().find(|(_, named)| *named == value))
            .map_or("", |k| k.0)
            .to_owned(),
    };
    let mut name = match (value, child(node, "root")) {
        ("none", _) | (_, None) => suffix,
        (_, Some(root)) => note(root, "root-step", "root-alter") + &suffix,
    };
    if let Some(bass) = child(node, "bass") {
        name.push('/');
        name.push_str(&note(bass, "bass-step", "bass-alter"));
    }
    let diagram = child(node, "frame").and_then(|frame| {
        let strings: usize = number(frame, "frame-strings")?;
        let mut frets = vec![-1i8; strings.clamp(1, Tuning::MAX_STRINGS)];
        for marked in children(frame, "frame-note") {
            let string: usize = number(marked, "string")?;
            let fret: i8 = number(marked, "fret")?;
            if let Some(slot) = string.checked_sub(1).and_then(|s| frets.get_mut(s)) {
                *slot = fret.clamp(0, ChordDiagram::MAX_FRET);
            }
        }
        Some(ChordDiagram { frets })
    });
    (Some(clip(&name)).filter(|name| !name.is_empty()), diagram)
}

/// The length a note is written as, if it says: its type, its dots and its tuplet.
fn read_duration(node: Node) -> Option<Duration> {
    let value = TYPES
        .iter()
        .find(|(_, name)| *name == text(node, "type"))?
        .0;
    let dots = children(node, "dot").count().min(Duration::MAX_DOTS.into()) as u8;
    let tuplet = child(node, "time-modification").and_then(|modification| {
        Some(Tuplet {
            actual: number::<u8>(modification, "actual-notes")?
                .try_into()
                .ok()?,
            normal: number::<u8>(modification, "normal-notes")?
                .try_into()
                .ok()?,
        })
    });
    Some(Duration {
        value,
        dots,
        tuplet: tuplet.filter(|t| t.actual != t.normal),
        nested: None,
    })
}

fn read_lyrics(node: Node, beat: &mut Beat) {
    for (index, lyric) in children(node, "lyric").enumerate() {
        let line = (lyric
            .attribute("number")
            .and_then(|n| n.parse::<usize>().ok()))
        .map_or(index, |number| number.saturating_sub(1));
        if line >= BeatEffects::MAX_LYRIC_LINES || beat.notes.is_empty() {
            continue;
        }
        let mut sung: String = children(lyric, "text").filter_map(|t| t.text()).collect();
        if matches!(text(lyric, "syllabic"), "begin" | "middle") {
            sung.push('-');
        }
        if beat.effects.lyrics.len() <= line {
            beat.effects.lyrics.resize(line + 1, String::new());
        }
        beat.effects.lyrics[line] = clip(sung.trim());
    }
    while beat.effects.lyrics.last().is_some_and(String::is_empty) {
        beat.effects.lyrics.pop();
    }
}

/// A bend from the levels MusicXML gives it, in quarter tones: one of the common shapes
/// if the levels are one's, and otherwise the levels spread evenly over the note.
fn bend_of(levels: &[i8], prebend: bool) -> Option<Bend> {
    let peak = levels.iter().copied().max()?;
    let kind = match (prebend, levels) {
        (false, [_]) => Some(BendKind::Bend),
        (false, [_, 0]) => Some(BendKind::BendRelease),
        (true, [_]) => Some(BendKind::Prebend),
        (true, [_, 0]) => Some(BendKind::PrebendRelease),
        _ => None,
    };
    let bend = match kind {
        Some(kind) => Bend::preset(kind, peak),
        None => {
            let levels = &levels[..levels.len().min(Bend::MAX_POINTS - 1)];
            let step = Bend::LENGTH as usize / levels.len();
            let start = match prebend {
                true => Vec::new(),
                false => vec![(0, 0)],
            };
            let rest = (levels.iter().enumerate()).map(|(index, level)| {
                let at = if prebend { index } else { index + 1 };
                ((at * step) as u8, *level)
            });
            Bend::new(start.into_iter().chain(rest))
        }
    };
    bend.is_valid(Bend::BEND_RANGE).then_some(bend)
}

/// Reads a note: where it is played, with what is its own, and what it says of its beat.
/// `None` for a rest, and for a note there is no place for.
fn read_note(
    node: Node,
    kind: &Kind,
    staff: &Staff,
    transpose: i32,
    unpitched: &HashMap<&str, u8>,
    kit: &[gtr_model::KitPiece],
) -> Option<(Note, BeatEffects)> {
    let notations: Vec<Node> = children(node, "notations").collect();
    let within = |name: &'static str| {
        (notations.iter()).flat_map(move |n| n.descendants().filter(move |d| d.has_tag_name(name)))
    };
    let has = |name: &'static str| within(name).next().is_some();
    let others: Vec<&str> = within("other-technical")
        .filter_map(|n| n.text())
        .map(str::trim)
        .collect();
    let head = child(node, "notehead");

    let (string, fret) =
        match kind {
            Kind::Drums => {
                // The piece is the one the part list gives the note's instrument. Without
                // that, a note with a pitch is the General MIDI number of its piece, and
                // one without is the piece that is written where the note is, or nearest.
                let listed = child(node, "instrument")
                    .and_then(|i| i.attribute("id"))
                    .and_then(|id| unpitched.get(id).copied());
                let numbered = child(node, "pitch").and_then(|pitch| {
                    let step = text(pitch, "step").chars().next()?;
                    let alter = number::<f64>(pitch, "alter").unwrap_or(0.0).round() as i32;
                    pitch_of(step, alter, number(pitch, "octave")?)
                });
                let written = child(node, "unpitched").and_then(|place| {
                    let step = text(place, "display-step").chars().next()?;
                    let octave = number::<i32>(place, "display-octave")?;
                    let index = "CDEFGAB".find(step.to_ascii_uppercase())? as i32;
                    let position = octave * 7 + index - 34;
                    let cross = head
                        .and_then(|h| h.text())
                        .is_some_and(|h| h.trim() != "normal");
                    (kit.iter())
                        .min_by_key(|p| {
                            let off = (i32::from(p.position) - position).abs();
                            (off, (p.head != gtr_model::DrumHead::Normal) != cross)
                        })
                        .map(|p| p.number)
                });
                let piece = listed
                    .or(numbered)
                    .or(written)
                    .filter(|p| (1..=Instrument::Drums.max_fret()).contains(p))?;
                (0, piece)
            }
            _ => {
                let pitch = child(node, "pitch")?;
                let step = text(pitch, "step").chars().next()?;
                let alter = number::<f64>(pitch, "alter").unwrap_or(0.0).round() as i32;
                let sounding = pitch_of(step, alter + transpose, number(pitch, "octave")?);
                match kind {
                    Kind::Pitched | Kind::Shadow => (sounding?, 0),
                    _ => {
                        // Where the note says it is played, or else nearest the nut.
                        let said = within("string").next().zip(within("fret").next()).and_then(
                            |(s, f)| {
                                let string =
                                    s.text()?.trim().parse::<usize>().ok()?.checked_sub(1)?;
                                let fret = f.text()?.trim().parse::<u8>().ok()?;
                                (string < staff.tuning.strings.len())
                                    .then_some((string as u8, fret))
                            },
                        );
                        said.or_else(|| place(sounding?, &staff.tuning, staff.capo, &[]))?
                    }
                }
            }
        };

    let drums = matches!(kind, Kind::Drums);
    let tied = children(node, "tie").any(|t| t.attribute("type") == Some("stop")) && !drums;
    let started = |name: &'static str| within(name).any(|n| n.attribute("type") == Some("start"));
    let harmonic = within("harmonic").next().map(|harmonic| {
        let named = [Harmonic::Pinch, Harmonic::Tapped, Harmonic::Semi]
            .into_iter()
            .find(|kind| harmonic_name(*kind).is_some_and(|name| others.contains(&name)));
        match (named, child(harmonic, "artificial")) {
            (Some(kind), _) => kind,
            (None, Some(_)) => Harmonic::Artificial,
            _ => Harmonic::Natural,
        }
    });
    let finger = |name: &'static str, labels: fn(Finger) -> &'static str| {
        let written = within(name).next()?.text()?.trim();
        [
            Finger::Thumb,
            Finger::Index,
            Finger::Middle,
            Finger::Ring,
            Finger::Pinky,
        ]
        .into_iter()
        .find(|finger| labels(*finger).eq_ignore_ascii_case(written))
    };
    let mut levels = Vec::new();
    let mut prebend = false;
    for bend in within("bend") {
        let level = (number::<f64>(bend, "bend-alter").unwrap_or(0.0) * 2.0)
            .round()
            .clamp(0.0, 12.0);
        let before = child(bend, "pre-bend").is_some();
        // A string bent by nothing before it is struck is not bent.
        if levels.is_empty() && before && level == 0.0 {
            continue;
        }
        prebend |= levels.is_empty() && before;
        levels.push(level as i8);
    }
    let slide = if started("slide") || started("glissando") {
        Some(if started("slur") {
            Slide::Legato
        } else {
            Slide::Shift
        })
    } else if has("scoop") {
        Some(Slide::InFromBelow)
    } else if has("plop") {
        Some(Slide::InFromAbove)
    } else if has("doit") {
        Some(Slide::OutUp)
    } else if has("falloff") {
        Some(Slide::OutDown)
    } else {
        None
    };
    let trill = has("trill-mark").then(|| {
        let to = others
            .iter()
            .find_map(|other| other.strip_prefix(TRILL_TO)?.parse::<u8>().ok());
        Trill {
            // Without a fret to trill to, the note a tone above.
            fret: to.unwrap_or(match kind {
                Kind::Shadow => UNSAID,
                _ => fret.saturating_add(2),
            }),
            speed: NoteValue::Sixteenth,
        }
    });
    let strings = matches!(kind, Kind::Strings | Kind::Shadow);
    let effects = NoteEffects {
        hammer: (started("hammer-on") || started("pull-off")) && strings,
        slide: slide.filter(|_| strings),
        bend: bend_of(&levels, prebend).filter(|_| strings),
        vibrato: within("wavy-line")
            .next()
            .map(|_| match others.contains(&WIDE_VIBRATO) {
                true => Vibrato::Wide,
                false => Vibrato::Slight,
            }),
        harmonic: harmonic.filter(|_| strings),
        harmonic_touch: None,
        accent: match (has("strong-accent"), has("accent")) {
            (true, _) => Some(Accent::Heavy),
            (_, true) => Some(Accent::Normal),
            _ => None,
        },
        trill: trill.filter(|_| strings),
        grace: None,
        left_finger: finger("fingering", Finger::left_label),
        right_finger: finger("pluck", Finger::right_label),
        palm_mute: others.contains(&PALM_MUTE),
        let_ring: within("tied").any(|t| t.attribute("type") == Some("let-ring")),
        dead: !drums
            && head
                .and_then(|h| h.text())
                .is_some_and(|h| matches!(h.trim(), "x" | "cross")),
        ghost: head.is_some_and(|h| h.attribute("parentheses") == Some("yes")),
        staccato: has("staccato") || has("staccatissimo"),
        tapped: has("tap") && strings,
    };
    let beat = BeatEffects {
        tremolo: within("tremolo")
            .find(|t| t.attribute("type").unwrap_or("single") == "single")
            .and_then(|t| match t.text()?.trim() {
                "1" => Some(NoteValue::Eighth),
                "2" => Some(NoteValue::Sixteenth),
                _ => Some(NoteValue::ThirtySecond),
            }),
        stroke: match (has("down-bow"), has("up-bow")) {
            (true, _) => Some(PickStroke::Down),
            (_, true) => Some(PickStroke::Up),
            _ => None,
        },
        strum: within("arpeggiate")
            .next()
            .map(|a| match a.attribute("direction") {
                Some("down") => Strum::ArpeggioUp,
                _ => Strum::ArpeggioDown,
            }),
        slap: match (others.contains(&SLAP), others.contains(&POP)) {
            (true, _) => Some(Slap::Slap),
            (_, true) => Some(Slap::Pop),
            _ => None,
        },
        ..BeatEffects::default()
    };
    Some((
        Note {
            string,
            fret,
            tied,
            effects,
        },
        beat,
    ))
}
