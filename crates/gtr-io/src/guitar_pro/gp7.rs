//! Versions 7 and 8 (`.gp`): a zip archive whose `Content/score.gpif` is an XML document.
//! Bars, voices, beats, notes and rhythms are stored in flat lists and refer to each other
//! by id.

use std::collections::HashMap;
use std::io::{Cursor, Read};

use gtr_model::{
    Accent, Beat, BeatEffects, Bend, ChordDiagram, Duration, Dynamic, Fade, Feel, Finger, Grace,
    GraceNote, Hairpin, Harmonic, Instrument, Jump, MasterBar, Mix, Note, NoteEffects, NoteValue,
    PickStroke, Score, ScoreInfo, Slap, Sound, Staff, Strum, Target, TimeSignature, Track, Trill,
    Tuplet, Vibrato, Voice,
};
use roxmltree::{Document, Node};
use zip::ZipArchive;

use super::{
    ImportError, Skipped, bar, beat, bend_from_points, duration, keep_dynamic_changes, rse,
    slide_from_flags, tuning,
};

const SCORE_ENTRY: &str = "Content/score.gpif";

fn corrupt(what: impl Into<String>) -> ImportError {
    ImportError::Corrupt(what.into())
}

pub(super) fn read(bytes: &[u8], skipped: &mut Skipped) -> Result<Score, ImportError> {
    let mut xml = String::new();
    ZipArchive::new(Cursor::new(bytes))
        .map_err(|_| ImportError::UnknownFormat)?
        .by_name(SCORE_ENTRY)
        .map_err(|_| ImportError::UnknownFormat)?
        .read_to_string(&mut xml)
        .map_err(|e| corrupt(e.to_string()))?;
    read_xml(&xml, skipped)
}

/// Reads the score from its XML, which a `.gpx` file has too, in a container of its own.
pub(super) fn read_xml(xml: &str, skipped: &mut Skipped) -> Result<Score, ImportError> {
    let document = Document::parse(xml).map_err(|e| corrupt(e.to_string()))?;
    read_document(&document, skipped)
}

fn child<'a>(node: Node<'a, 'a>, name: &str) -> Option<Node<'a, 'a>> {
    node.children().find(|n| n.has_tag_name(name))
}

fn text<'a>(node: Node<'a, 'a>, name: &str) -> &'a str {
    child(node, name)
        .and_then(|n| n.text())
        .unwrap_or("")
        .trim()
}

/// The elements of a flat list such as `<Beats>`, keyed by their `id` attribute.
fn by_id<'a>(root: Node<'a, 'a>, list: &str) -> HashMap<&'a str, Node<'a, 'a>> {
    child(root, list)
        .into_iter()
        .flat_map(|l| l.children())
        .filter_map(|n| Some((n.attribute("id")?, n)))
        .collect()
}

/// A `<Property name="...">` anywhere below `node`.
fn property<'a>(node: Node<'a, 'a>, name: &str) -> Option<Node<'a, 'a>> {
    node.descendants()
        .find(|n| n.has_tag_name("Property") && n.attribute("name") == Some(name))
}

fn read_document(document: &Document, skipped: &mut Skipped) -> Result<Score, ImportError> {
    let root = document.root_element();
    let bars = by_id(root, "Bars");
    let voices = by_id(root, "Voices");
    let beats = by_id(root, "Beats");
    let notes = by_id(root, "Notes");
    let rhythms = by_id(root, "Rhythms");

    let info = child(root, "Score").map_or_else(ScoreInfo::default, |score| ScoreInfo {
        title: text(score, "Title").into(),
        subtitle: text(score, "SubTitle").into(),
        artist: text(score, "Artist").into(),
        album: text(score, "Album").into(),
        words: text(score, "Words").into(),
        music: text(score, "Music").into(),
        transcriber: text(score, "Tabber").into(),
        copyright: text(score, "Copyright").into(),
    });

    // Tempo automations hold "<bpm> <unit>", the unit being the note the beats are
    // counted in. The first is the starting tempo, and the rest change it where they stand.
    let tempos: Vec<(usize, f64, u32, bool)> = (root.descendants())
        .filter(|n| n.has_tag_name("Automation") && text(*n, "Type") == "Tempo")
        .map(|automation| {
            let mut parts = text(automation, "Value").split_whitespace();
            let bpm: f64 = parts.next().and_then(|v| v.parse().ok()).unwrap_or(120.0);
            let per_quarter = match parts.next() {
                Some("1") => 0.5,
                Some("3") => 1.5,
                Some("4") => 2.0,
                Some("5") => 3.0,
                _ => 1.0,
            };
            (
                text(automation, "Bar").parse().unwrap_or(0),
                text(automation, "Position").parse().unwrap_or(0.0),
                (bpm * per_quarter).round().clamp(1.0, 1000.0) as u32,
                // The tempo moves steadily from here to the next automation.
                text(automation, "Linear") == "true",
            )
        })
        .collect();
    let tempo = tempos.first().map_or(120, |first| first.2);

    let mut tracks = Vec::new();
    // What each track's beats and notes refer to.
    let mut parts: Vec<Part> = Vec::new();
    for node in child(root, "Tracks").into_iter().flat_map(|t| t.children()) {
        if !node.has_tag_name("Track") {
            continue;
        }
        // A `.gp` file gives a drum track a kit of articulations; a `.gpx` only says
        // that the track is percussion, and has its notes name their pieces themselves.
        let old_drums = (child(node, "GeneralMidi"))
            .is_some_and(|midi| midi.attribute("table") == Some("Percussion"));
        let kit = child(node, "InstrumentSet")
            .filter(|set| text(*set, "Type") == "drumKit")
            .map(|set| {
                (set.descendants())
                    .filter(|n| n.has_tag_name("Articulation"))
                    .map(|a| text(a, "OutputMidiNumber").parse().unwrap_or(0))
                    .collect::<Vec<u8>>()
            })
            .or(old_drums.then(Vec::new));
        // Version 7 nests properties in <Staff> elements; older files put one set of
        // properties on the track itself.
        let staff_nodes: Vec<Node> = match child(node, "Staves") {
            Some(staves) => staves
                .children()
                .filter(|n| n.has_tag_name("Staff"))
                .collect(),
            None => vec![node],
        };
        let staves = staff_nodes
            .into_iter()
            .map(|staff| {
                // Pitches are listed lowest string first.
                let pitches = property(staff, "Tuning")
                    .map(|p| text(p, "Pitches"))
                    .unwrap_or("")
                    .split_whitespace()
                    .filter_map(|p| p.parse().ok())
                    .rev()
                    .collect::<Vec<u8>>();
                if kit.is_some() {
                    // A drum track's lines have no pitch, as in the older format.
                    return Staff {
                        tuning: gtr_model::Tuning::new(vec![0; DRUM_LINES]),
                        capo: 0,
                        bars: Vec::new(),
                    };
                }
                Staff {
                    // A staff that names no strings is for an instrument that has none.
                    tuning: match pitches.is_empty() {
                        true => gtr_model::Tuning::none(),
                        false => tuning(pitches),
                    },
                    capo: property(staff, "CapoFret")
                        .and_then(|p| text(p, "Fret").parse().ok())
                        .unwrap_or(0),
                    bars: Vec::new(),
                }
            })
            .collect::<Vec<Staff>>();
        // The sound's General MIDI program, wherever this version of the format keeps it.
        let program = (node.descendants())
            .find(|n| n.has_tag_name("Program"))
            .and_then(|n| n.text()?.trim().parse::<u8>().ok());
        let instrument = match kit {
            Some(_) => Instrument::Drums,
            // The one instrument here without strings is the piano.
            None if staves.iter().any(Staff::is_pitched) => {
                skipped.add_if(
                    program.is_some_and(|p| p > 7),
                    "the sounds of instruments without strings, which are played as a piano",
                );
                Instrument::Piano
            }
            None => program.map_or_else(Instrument::default, Instrument::from_midi_program),
        };
        // A piano's staves are all without strings.
        let staves: Vec<Staff> = match instrument {
            Instrument::Piano => (staves.into_iter())
                .map(|staff| Staff {
                    tuning: gtr_model::Tuning::none(),
                    capo: 0,
                    ..staff
                })
                .collect(),
            _ => staves,
        };
        // The track's chords, which its beats name by a number, and the lines of its
        // lyrics, each with the bar it starts in.
        let chords = property(node, "DiagramCollection")
            .into_iter()
            .flat_map(|collection| collection.descendants())
            .filter(|n| n.has_tag_name("Item"))
            .filter_map(|item| {
                let name: String = (item.attribute("name").unwrap_or("").trim().chars())
                    .take(BeatEffects::MAX_TEXT)
                    .collect();
                Some((
                    item.attribute("id")?,
                    (name, child(item, "Diagram").and_then(diagram)),
                ))
            })
            .collect();
        let lines = child(node, "Lyrics")
            .into_iter()
            .flat_map(|lyrics| lyrics.children())
            .filter(|n| n.has_tag_name("Line"))
            .map(|line| {
                (
                    text(line, "Offset").parse().unwrap_or(0),
                    text(line, "Text"),
                )
            })
            .collect();
        let mut track = Track::new(text(node, "Name"), instrument, staves);
        let switches = read_sounds(node, &mut track, skipped);
        // The mixing desk's settings for the track, each from 0 to 1: the twelfth is its
        // place between the speakers and the thirteenth its volume, which stands at
        // 0.795 until it is moved.
        let desk: Vec<f64> = (node.descendants())
            .find(|n| n.has_tag_name("ChannelStrip"))
            .map(|strip| text(strip, "Parameters"))
            .unwrap_or("")
            .split_whitespace()
            .filter_map(|value| value.parse().ok())
            .collect();
        if let (Some(balance), Some(volume)) = (desk.get(11), desk.get(12)) {
            let level = volume / super::write::DESK_UNITY * f64::from(Mix::UNITY);
            track.mix.volume = level.round().clamp(0.0, f64::from(Mix::MAX_VOLUME)) as u8;
            track.mix.pan = ((balance - 0.5) * 200.0).round().clamp(-100.0, 100.0) as i8;
        }
        match text(node, "PlaybackState") {
            "Mute" => track.mix.mute = true,
            "Solo" => track.mix.solo = true,
            _ => {}
        }
        // Changes to the same two settings as the piece goes: the bar, how far through
        // it, and the new value. One at the very start that says what the desk says is
        // the desk's own.
        let changes = (node.descendants())
            .filter(|n| n.has_tag_name("Automation"))
            .filter_map(|automation| {
                let pan = match text(automation, "Type") {
                    "DSPParam_11" => true,
                    "DSPParam_12" => false,
                    _ => return None,
                };
                let bar: usize = text(automation, "Bar").parse().ok()?;
                let position: f64 = text(automation, "Position").parse().ok()?;
                let value: f64 = text(automation, "Value").parse().ok()?;
                let desk = desk.get(if pan { 11 } else { 12 }).copied();
                let same = desk.is_some_and(|desk| (desk - value).abs() < 1e-4);
                (bar > 0 || position > 0.0 || !same).then_some(Change {
                    bar,
                    position,
                    pan,
                    value,
                })
            })
            .collect();
        tracks.push(track);
        parts.push(Part {
            kit,
            chords,
            lines,
            changes,
            switches,
        });
    }

    let mut master_bars = Vec::new();
    for node in child(root, "MasterBars")
        .into_iter()
        .flat_map(|m| m.children())
    {
        if !node.has_tag_name("MasterBar") {
            continue;
        }
        let (numerator, denominator) = text(node, "Time").split_once('/').unwrap_or(("4", "4"));
        let time_signature = TimeSignature {
            numerator: numerator.trim().parse().unwrap_or(4),
            denominator: denominator.trim().parse().unwrap_or(4),
        };
        let mut master = MasterBar::new(match time_signature.is_valid() {
            true => time_signature,
            false => TimeSignature::COMMON,
        });
        master.key_signature = child(node, "Key")
            .and_then(|key| text(key, "AccidentalCount").parse::<i8>().ok())
            .map_or(0, |count| {
                count.clamp(-MasterBar::MAX_KEY, MasterBar::MAX_KEY)
            });
        if let Some(repeat) = child(node, "Repeat") {
            master.repeat_start = repeat.attribute("start") == Some("true");
            if repeat.attribute("end") == Some("true") {
                let count = repeat.attribute("count").and_then(|c| c.parse::<u8>().ok());
                master.repeat_count = count.unwrap_or(2).max(2);
            }
        }
        // The passes an ending is played on are listed by number.
        master.alternate_endings = text(node, "AlternateEndings")
            .split_whitespace()
            .filter_map(|pass| pass.parse::<u32>().ok())
            .filter(|pass| (1..=8).contains(pass))
            .fold(0, |mask, pass| mask | 1 << (pass - 1));
        master.section = child(node, "Section")
            .map(|section| match text(section, "Text") {
                "" => text(section, "Letter"),
                name => name,
            })
            .filter(|name| !name.is_empty())
            .map(|name| name.chars().take(120).collect());
        if let Some(directions) = child(node, "Directions") {
            for direction in directions.children() {
                let named = direction.text().unwrap_or("").trim();
                if direction.has_tag_name("Target") {
                    master.targets.extend(match named {
                        "Segno" => Some(Target::Segno),
                        "SegnoSegno" => Some(Target::SegnoSegno),
                        "Coda" => Some(Target::Coda),
                        "DoubleCoda" => Some(Target::DoubleCoda),
                        "Fine" => Some(Target::Fine),
                        _ => None,
                    });
                } else if direction.has_tag_name("Jump") {
                    master.jumps.extend(match named {
                        "DaCapo" => Some(Jump::DaCapo),
                        "DaCapoAlFine" => Some(Jump::DaCapoAlFine),
                        "DaCapoAlCoda" => Some(Jump::DaCapoAlCoda),
                        "DaCapoAlDoubleCoda" => Some(Jump::DaCapoAlDoubleCoda),
                        "DaSegno" => Some(Jump::DalSegno),
                        "DaSegnoAlFine" => Some(Jump::DalSegnoAlFine),
                        "DaSegnoAlCoda" => Some(Jump::DalSegnoAlCoda),
                        "DaSegnoAlDoubleCoda" => Some(Jump::DalSegnoAlDoubleCoda),
                        "DaSegnoSegno" => Some(Jump::DalSegnoSegno),
                        "DaSegnoSegnoAlFine" => Some(Jump::DalSegnoSegnoAlFine),
                        "DaSegnoSegnoAlCoda" => Some(Jump::DalSegnoSegnoAlCoda),
                        "DaSegnoSegnoAlDoubleCoda" => Some(Jump::DalSegnoSegnoAlDoubleCoda),
                        "DaCoda" => Some(Jump::ToCoda),
                        "DaDoubleCoda" => Some(Jump::ToDoubleCoda),
                        _ => None,
                    });
                }
            }
        }
        master.feel = match text(node, "TripletFeel") {
            "Triplet8th" => Some(Feel::SwingEighths),
            "Triplet16th" => Some(Feel::SwingSixteenths),
            "Dotted8th" => Some(Feel::DottedEighths),
            "Dotted16th" => Some(Feel::DottedSixteenths),
            "Scottish8th" => Some(Feel::ScottishEighths),
            "Scottish16th" => Some(Feel::ScottishSixteenths),
            _ => None,
        };
        // Fermatas are kept with the bar, each at so many quarter notes into it, and go on
        // every beat that starts there.
        let fermatas: Vec<gtr_model::Rational> = child(node, "Fermatas")
            .into_iter()
            .flat_map(|list| list.children())
            .filter(|n| n.has_tag_name("Fermata"))
            .filter_map(|fermata| {
                let (quarters, of) = text(fermata, "Offset").split_once('/')?;
                let (quarters, of): (i64, i64) =
                    (quarters.trim().parse().ok()?, of.trim().parse().ok()?);
                (of > 0 && (0..=4096).contains(&quarters) && of <= 4096)
                    .then(|| gtr_model::Rational::new(quarters, of * 4))
            })
            .collect();
        master_bars.push(master);
        let mut timed = false;

        // One bar id per staff, in track order.
        let mut ids = text(node, "Bars").split_whitespace();
        let staves = (tracks.iter_mut().zip(&parts))
            .flat_map(|(track, part)| track.staves.iter_mut().map(move |s| (s, part)));
        for (staff, part) in staves {
            let voice_ids = ids
                .next()
                .and_then(|id| bars.get(id))
                .map(|bar| text(*bar, "Voices"))
                .unwrap_or("");
            let read_voices = voice_ids
                .split_whitespace()
                .filter_map(|id| voices.get(id))
                .map(|voice| {
                    let nodes = text(*voice, "Beats")
                        .split_whitespace()
                        .filter_map(|id| beats.get(id).copied());
                    timed |= nodes.clone().any(|beat| child(beat, "Timer").is_some());
                    let mut read = read_voice(nodes, &notes, &rhythms, staff, part, skipped);
                    let mut time = gtr_model::Rational::from_integer(0);
                    for beat in &mut read.beats {
                        beat.effects.fermata |= fermatas.contains(&time);
                        time += beat.duration.length();
                    }
                    read
                })
                .collect();
            staff.bars.push(bar(read_voices));
        }
        // A timer shows how long the piece has been playing; here it belongs to the bar.
        if let Some(master) = master_bars.last_mut() {
            master.timer = timed;
        }
    }
    for staff in tracks.iter_mut().flat_map(|t| &mut t.staves) {
        keep_dynamic_changes(staff);
    }
    // Lyrics that the file has not already put on the beats are put there now.
    for (track, part) in tracks.iter_mut().zip(&parts) {
        let staff = &mut track.staves[0];
        let dispatched = (staff.bars.iter().flat_map(|b| &b.voices))
            .flat_map(|v| &v.beats)
            .any(|b| !b.effects.lyrics.is_empty());
        for (line, (offset, words)) in part.lines.iter().enumerate() {
            if !dispatched {
                super::set_lyrics(staff, line, *offset, words);
            }
        }
    }
    for (track, part) in tracks.iter_mut().zip(&parts) {
        set_mix_changes(track, &part.changes, &master_bars);
        for &(bar, position, sound) in &part.switches {
            match beat_at(track, &master_bars, bar, position) {
                Some(beat) => beat.effects.sound = Some(sound),
                None => skipped.add("switches of sound in bars with no beat to carry them"),
            }
        }
    }
    // The pedals of the whole mix.
    let master = (root.descendants()).find(|n| n.has_tag_name("MasterTrack"));
    let (effects, unmatched) =
        (master.and_then(|m| child(m, "RSE"))).map_or((Vec::new(), false), rse::chain);
    skipped.add_if(unmatched, "pedals and amplifiers that have no match here");
    super::keep_feel_changes(&mut master_bars);
    if let (Some(&(_, _, _, true)), Some(first)) = (tempos.first(), master_bars.first_mut()) {
        first.tempo_gradual = true;
    }
    for &(bar, position, tempo, gradual) in tempos.iter().skip(1) {
        let Some(master) = master_bars.get_mut(bar) else {
            continue;
        };
        let at = moment(position, master);
        if bar > 0 || position > 0.0 {
            super::set_tempo(master, at, tempo, gradual);
        }
    }

    Ok(Score {
        info,
        tempo,
        master_bars,
        tracks,
        effects,
        audio: None,
        style: Default::default(),
    })
}

/// Reads the beats of a voice. Grace notes are stored as beats of their own before the beat
/// they decorate; the model attaches them to that beat's notes instead.
/// Lines a drum staff is given.
const DRUM_LINES: usize = 5;

/// The kit of `.gpx` files, whose notes name a piece by its place here and one of up to
/// three ways of hitting it, as the General MIDI numbers that sound: the bass drum; the
/// snare, its rim shot and side stick; three cowbells; five toms; the hi-hat closed, half
/// open and open, and by its pedal; two crashes, a splash, the ride with its edge and its
/// bell, and a china. (The table is alphaTab's, worked out from a file with every piece.)
const OLD_KIT: [[u8; 3]; 17] = [
    [35, 35, 35],
    [38, 38, 37],
    [56, 56, 56],
    [56, 56, 56],
    [56, 56, 56],
    [43, 43, 43],
    [45, 45, 45],
    [47, 47, 47],
    [48, 48, 48],
    [50, 50, 50],
    [42, 46, 46],
    [44, 44, 44],
    [57, 57, 57],
    [49, 49, 49],
    [55, 55, 55],
    [51, 51, 53],
    [52, 52, 52],
];

/// What the beats and notes of a track refer to.
struct Part<'a> {
    /// For a drum kit, the General MIDI number each of its articulations sounds: a note
    /// says which articulation it is by its place in this list.
    kit: Option<Vec<u8>>,
    /// The track's chords by the number a beat names them with: a name, and a fingering.
    chords: HashMap<&'a str, (String, Option<ChordDiagram>)>,
    /// The lines of its lyrics: the bar each starts in, and its words.
    lines: Vec<(usize, &'a str)>,
    /// Changes of the track's volume and pan.
    changes: Vec<Change>,
    /// Switches to another of the track's sounds: the bar, how far through it from 0 to
    /// 1, and the sound, as [`BeatEffects::sound`] numbers it.
    switches: Vec<(usize, f64, u8)>,
}

/// Reads a track's sounds, each with its chain of pedals and amplifiers, onto the track:
/// the one it starts with is its own, and the others are those it can switch to. Returns
/// where it switches from one to another after it starts.
fn read_sounds(node: Node, track: &mut Track, skipped: &mut Skipped) -> Vec<(usize, f64, u8)> {
    let sounds: Vec<Node> = (child(node, "Sounds").into_iter())
        .flat_map(|s| s.children())
        .filter(|n| n.has_tag_name("Sound"))
        .collect();
    // A `.gpx` file has no sounds of its own: a track has a chain, and variations of it.
    if sounds.is_empty() {
        let mut chains = (child(node, "RSE").into_iter())
            .flat_map(|rse| rse.descendants())
            .filter(|n| n.has_tag_name("EffectChain"))
            .map(rse::chain);
        if let Some((chain, unmatched)) = chains.next() {
            skipped.add_if(unmatched, "pedals and amplifiers that have no match here");
            track.effects = chain;
        }
        skipped.add_if(
            chains.any(|(chain, _)| !chain.is_empty()),
            "the variations of a track's effect chain",
        );
        return Vec::new();
    }
    // A switch names a sound by its path, its name and its role, which together tell it
    // from the others.
    let key = |sound: Node| {
        format!(
            "{};{};{}",
            text(sound, "Path"),
            text(sound, "Name"),
            text(sound, "Role")
        )
    };
    let mut switches: Vec<(usize, f64, &str)> = (node.children())
        .filter(|n| n.has_tag_name("Automations"))
        .flat_map(|a| a.children())
        .filter(|n| n.has_tag_name("Automation") && text(*n, "Type") == "Sound")
        .filter_map(|a| {
            Some((
                text(a, "Bar").parse().ok()?,
                text(a, "Position").parse().ok()?,
                text(a, "Value"),
            ))
        })
        .collect();
    switches.sort_by(|a, b| {
        (a.0, a.1)
            .partial_cmp(&(b.0, b.1))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let place = |value: &str| {
        sounds.iter().position(|&s| {
            key(s) == value || text(s, "Name") == value.split(';').nth(1).unwrap_or("")
        })
    };
    // The track's own sound is the one it starts with; the rest follow it in their order.
    let own = match switches.first() {
        Some(&(0, 0.0, value)) => place(value).unwrap_or(0),
        _ => 0,
    };
    let mut order: Vec<usize> = std::iter::once(own)
        .chain((0..sounds.len()).filter(|&i| i != own))
        .collect();
    skipped.add_if(
        order.len() > Sound::MAX_SOUNDS + 1,
        "sounds past the eighth of a track",
    );
    order.truncate(Sound::MAX_SOUNDS + 1);
    let mut others = Vec::new();
    for (n, &i) in order.iter().enumerate() {
        let sound = sounds[i];
        let (chain, unmatched) =
            (descendant(sound, "EffectChain")).map_or((Vec::new(), false), rse::chain);
        skipped.add_if(unmatched, "pedals and amplifiers that have no match here");
        if n == 0 {
            track.effects = chain;
            continue;
        }
        let program =
            (descendant(sound, "Program")).and_then(|p| p.text()?.trim().parse::<u8>().ok());
        let instrument = match track.instrument {
            Instrument::Drums | Instrument::Piano => track.instrument,
            own => program.map_or(own, Instrument::from_midi_program),
        };
        let name: String = match text(sound, "Name").trim() {
            "" => format!("Sound {n}"),
            name => name.chars().take(Sound::MAX_NAME).collect(),
        };
        let fallback = Sound {
            name: name.clone(),
            instrument: track.instrument,
            effects: chain.clone(),
        };
        let read = Sound {
            name,
            instrument,
            effects: chain,
        };
        others.push(
            match Sound::list_is_valid(std::slice::from_ref(&read), track.instrument) {
                true => read,
                false => fallback,
            },
        );
    }
    track.sounds = others;
    // Each switch after the start, to the number the sound now has.
    (switches.into_iter())
        .filter(|&(bar, position, _)| bar > 0 || position > 0.0)
        .filter_map(|(bar, position, value)| {
            let to = order.iter().position(|&i| Some(i) == place(value))?;
            Some((bar, position, u8::try_from(to).ok()?))
        })
        .collect()
}

/// The first element named `name` anywhere below `node`.
fn descendant<'a>(node: Node<'a, 'a>, name: &str) -> Option<Node<'a, 'a>> {
    node.descendants().find(|n| n.has_tag_name(name))
}

/// A change of a track's volume or pan at a moment of a bar, as the mixing desk counts
/// them: from 0 to 1.
struct Change {
    bar: usize,
    /// How far through the bar, from 0 to 1.
    position: f64,
    pan: bool,
    value: f64,
}

/// A moment of a bar given as a share of it, in whole notes from its start. Files say it
/// as a decimal; it is taken to the nearest 1920th of a whole note, which every beat of
/// a note value or a common tuplet falls on.
fn moment(share: f64, master: &MasterBar) -> gtr_model::Rational {
    let length = master.time_signature.bar_length();
    let whole = share.clamp(0.0, 1.0) * (*length.numer() as f64 / *length.denom() as f64);
    gtr_model::Rational::new((whole * 1920.0).round() as i64, 1920)
}

/// The beat of a track at a moment of a bar, from 0 to 1 through it: the beat of the first
/// staff's first voice that starts there, or the last to start before.
fn beat_at<'a>(
    track: &'a mut Track,
    masters: &[MasterBar],
    bar: usize,
    position: f64,
) -> Option<&'a mut Beat> {
    beat_in(
        track.staves[0].bars.get_mut(bar)?,
        masters.get(bar)?,
        position,
    )
}

fn beat_in<'a>(
    bar: &'a mut gtr_model::Bar,
    master: &MasterBar,
    position: f64,
) -> Option<&'a mut Beat> {
    let at = moment(position, master);
    let mut time = gtr_model::Rational::from_integer(0);
    let mut found = None;
    for (index, beat) in bar.voices[0].beats.iter().enumerate() {
        if time > at {
            break;
        }
        found = Some(index);
        time += beat.duration.length();
    }
    found.map(|index| &mut bar.voices[0].beats[index])
}

/// Puts a track's changes of volume and pan on the beats they happen at.
fn set_mix_changes(track: &mut Track, changes: &[Change], masters: &[MasterBar]) {
    for change in changes {
        let (Some(master), Some(bar)) = (
            masters.get(change.bar),
            track.staves[0].bars.get_mut(change.bar),
        ) else {
            continue;
        };
        let Some(beat) = beat_in(bar, master, change.position) else {
            continue;
        };
        if change.pan {
            let pan = ((change.value - 0.5) * 200.0).round().clamp(-100.0, 100.0);
            beat.effects.pan = Some(pan as i8);
        } else {
            let level = change.value / super::write::DESK_UNITY * f64::from(Mix::UNITY);
            beat.effects.volume = Some(level.round().clamp(0.0, f64::from(Mix::MAX_VOLUME)) as u8);
        }
    }
}

/// A chord's fingering. The file lists the strings that are stopped or open, counted from
/// the lowest, with frets counted from the diagram's first.
fn diagram(node: Node) -> Option<ChordDiagram> {
    let number = |n: Node, name| n.attribute(name).and_then(|v| v.parse::<i32>().ok());
    let strings = usize::try_from(number(node, "stringCount")?).ok()?;
    let base = number(node, "baseFret").unwrap_or(0);
    let mut frets = vec![-1i8; strings.min(gtr_model::Tuning::MAX_STRINGS)];
    for fret in node.children().filter(|n| n.has_tag_name("Fret")) {
        let string = usize::try_from(number(fret, "string")?).ok()?;
        let slot = frets.len().checked_sub(string + 1)?;
        let at = (base + number(fret, "fret")?).clamp(0, ChordDiagram::MAX_FRET.into());
        frets[slot] = at as i8;
    }
    Some(ChordDiagram { frets }).filter(|d| d.frets.iter().any(|f| *f >= 0))
}

fn read_voice<'a>(
    nodes: impl Iterator<Item = Node<'a, 'a>>,
    notes: &HashMap<&str, Node>,
    rhythms: &HashMap<&str, Node>,
    staff: &Staff,
    part: &Part,
    skipped: &mut Skipped,
) -> Voice {
    let mut beats = Vec::new();
    // Grace notes waiting for the beat they lead into: each with whether it is on the
    // beat and whether it is hammered.
    let mut graces: Vec<(GraceNote, bool, bool)> = Vec::new();
    for node in nodes {
        let mut beat = read_beat(node, notes, rhythms, staff, part, skipped);
        if let Some(kind) = child(node, "GraceNotes") {
            let on_beat = kind.text().map(str::trim) == Some("OnBeat");
            graces.extend(beat.notes.iter().map(|n| {
                let grace = GraceNote {
                    string: n.string,
                    fret: n.fret,
                };
                (grace, on_beat, n.effects.hammer)
            }));
            continue;
        }
        for (grace, on_beat, hammer) in graces.drain(..) {
            // A grace note belongs to the note on its string, or failing that to the
            // first note of the beat.
            let at = (beat.notes.iter()).position(|n| n.string == grace.string);
            let Some(note) = beat.notes.get_mut(at.unwrap_or(0)) else {
                continue;
            };
            let ornament = note.effects.grace.get_or_insert(Grace {
                notes: Vec::new(),
                on_beat,
                hammer: false,
            });
            if ornament.notes.len() < Grace::MAX_NOTES {
                ornament.notes.push(grace);
                ornament.hammer |= hammer;
            }
        }
        beats.push(beat);
    }
    skipped.add_if(!graces.is_empty(), "grace notes with no note after them");
    Voice { beats }
}

/// A pitch curve from the format's origin, middle and destination values, which are in
/// hundredths of a whole tone at offsets in hundredths of the note's length. The middle
/// value is held between two offsets; a `.gpx` file leaves the offsets out where they are
/// the usual ones, and then the middle value is reached half way through.
fn curve(values: [Option<f64>; 3], offsets: [Option<f64>; 4]) -> Option<Bend> {
    let [origin, middle, destination] = values;
    let mut points = vec![(offsets[0].unwrap_or(0.0), origin.unwrap_or(0.0))];
    match (middle, offsets[1], offsets[2]) {
        (Some(middle), None, None) => points.push((50.0, middle)),
        (middle, first, second) => {
            let middle = middle.unwrap_or(0.0);
            points.extend(first.map(|offset| (offset, middle)));
            points.extend(second.map(|offset| (offset, middle)));
        }
    }
    points.push((offsets[3].unwrap_or(100.0), destination.unwrap_or(0.0)));
    let mut bend = bend_from_points(
        (points.into_iter())
            .map(|(offset, value)| ((offset * 0.6).round() as i32, value.round() as i32)),
    )?;
    // The level a curve ends on is held to the end of the note; saying so keeps the
    // shapes the editor offers recognisable.
    if let Some(last) = bend.points.last().copied()
        && last.position < Bend::LENGTH
        && bend.points.len() < Bend::MAX_POINTS
    {
        bend.points.push(gtr_model::CurvePoint {
            position: Bend::LENGTH,
            value: last.value,
        });
    }
    Some(bend)
}

fn read_beat(
    node: Node,
    notes: &HashMap<&str, Node>,
    rhythms: &HashMap<&str, Node>,
    staff: &Staff,
    part: &Part,
    skipped: &mut Skipped,
) -> Beat {
    let kit = part.kit.as_deref();
    let string_count = staff.places();

    let rhythm = child(node, "Rhythm")
        .and_then(|r| r.attribute("ref"))
        .and_then(|id| rhythms.get(id));
    let read_duration = match rhythm {
        Some(rhythm) => {
            let value = match text(*rhythm, "NoteValue") {
                "Whole" | "Long" | "DoubleWhole" => Some(NoteValue::Whole),
                "Half" => Some(NoteValue::Half),
                "Quarter" => Some(NoteValue::Quarter),
                "Eighth" => Some(NoteValue::Eighth),
                "16th" => Some(NoteValue::Sixteenth),
                "32nd" => Some(NoteValue::ThirtySecond),
                "64th" => Some(NoteValue::SixtyFourth),
                "128th" => Some(NoteValue::HundredTwentyEighth),
                "256th" => Some(NoteValue::TwoHundredFiftySixth),
                _ => None,
            };
            let dots = child(*rhythm, "AugmentationDot")
                .and_then(|d| d.attribute("count")?.parse().ok())
                .unwrap_or(0);
            let tuplet = |name| {
                let t = child(*rhythm, name)?;
                Some((
                    t.attribute("num")?.parse().ok()?,
                    t.attribute("den")?.parse().ok()?,
                ))
            };
            let mut read = duration(value, dots, tuplet("PrimaryTuplet"), skipped);
            read.nested = tuplet("SecondaryTuplet").and_then(|(actual, normal): (u8, u8)| {
                Some(Tuplet {
                    actual: actual.try_into().ok()?,
                    normal: normal.try_into().ok()?,
                })
            });
            read
        }
        None => Duration::new(NoteValue::Quarter),
    };

    let read_notes = text(node, "Notes")
        .split_whitespace()
        .filter_map(|id| notes.get(id))
        .enumerate()
        .filter_map(|(line, note)| {
            if let Some(kit) = kit {
                // The piece of the kit is the note's fret, as on any drum staff here, and
                // the notes of a beat take its lines from the top.
                let number = |name| {
                    let named = property(*note, name)?;
                    text(named, name).parse::<usize>().ok()
                };
                let piece = (text(*note, "InstrumentArticulation").parse::<usize>().ok())
                    .and_then(|index| kit.get(index).copied())
                    .or_else(|| {
                        // In a `.gpx` file, a piece of its kit and a way of hitting it.
                        let variations = OLD_KIT.get(number("Element")?)?;
                        Some(variations[number("Variation").filter(|v| *v < 3).unwrap_or(0)])
                    })
                    .filter(|piece| (1..=Instrument::Drums.max_fret()).contains(piece));
                skipped.add_if(
                    piece.is_none(),
                    "percussion that is not in the General MIDI kit",
                );
                skipped.add_if(
                    line >= string_count,
                    "more drums at once than a staff has lines",
                );
                return Some(Note {
                    string: line as u8,
                    fret: piece?,
                    tied: false,
                    effects: read_note_effects(*note),
                });
            }
            let mut effects = read_note_effects(*note);
            if staff.is_pitched() {
                // A note of an instrument without strings is its pitch: an octave and
                // a step of it, or the MIDI number.
                let number = |name, inner| text(property(*note, name)?, inner).parse::<u32>().ok();
                let pitch = match (number("Octave", "Number"), number("Tone", "Step")) {
                    (Some(octave), Some(tone)) => octave * 12 + tone,
                    _ => number("Midi", "Number")?,
                };
                return Some(Note {
                    string: u8::try_from(pitch).ok().filter(|p| *p < 128)?,
                    fret: 0,
                    tied: child(*note, "Tie").and_then(|t| t.attribute("destination"))
                        == Some("true"),
                    effects,
                });
            }
            let fret: u8 = text(property(*note, "Fret")?, "Fret").parse().ok()?;
            // Strings are numbered from the lowest; the model counts from the highest.
            let from_low: usize = text(property(*note, "String")?, "String").parse().ok()?;
            let string = string_count.checked_sub(from_low + 1)? as u8;
            // A trill names the other note by its MIDI pitch.
            effects.trill = text(*note, "Trill").parse::<u8>().ok().and_then(|pitch| {
                let open = staff.tuning.pitch(string, 0, staff.capo)?;
                // How long each of its notes lasts is kept apart, with 480 to the
                // quarter note.
                let ticks = (note.descendants())
                    .find(|n| n.has_tag_name("XProperty") && n.attribute("id") == Some("688062467"))
                    .and_then(|speed| text(speed, "Int").parse::<u32>().ok());
                Some(Trill {
                    fret: pitch.checked_sub(open)?,
                    speed: match ticks {
                        Some(30) => NoteValue::SixtyFourth,
                        Some(60) => NoteValue::ThirtySecond,
                        _ => NoteValue::Sixteenth,
                    },
                })
            });
            Some(Note {
                string,
                fret,
                tied: child(*note, "Tie").and_then(|t| t.attribute("destination")) == Some("true"),
                effects,
            })
        })
        .collect::<Vec<Note>>();
    // Vibrato made with the tremolo bar is stored on the beat; the model keeps it per note.
    let bar_vibrato = match property(node, "VibratoWTremBar").map(|p| text(p, "Strength")) {
        Some("Slight") => Some(Vibrato::Slight),
        Some("Wide") => Some(Vibrato::Wide),
        _ => None,
    };
    let read_notes = read_notes
        .into_iter()
        .map(|mut note| {
            note.effects.vibrato = note.effects.vibrato.or(bar_vibrato);
            note
        })
        .collect();

    let enabled = |name| property(node, name).is_some_and(|p| child(p, "Enable").is_some());
    let direction = |name| property(node, name).map(|p| text(p, "Direction"));
    let mut read = beat(read_duration, read_notes, string_count);
    read.effects.dynamic = match text(node, "Dynamic") {
        "PPP" => Some(Dynamic::Ppp),
        "PP" => Some(Dynamic::Pp),
        "P" => Some(Dynamic::P),
        "MP" => Some(Dynamic::Mp),
        "MF" => Some(Dynamic::Mf),
        "F" => Some(Dynamic::F),
        "FF" => Some(Dynamic::Ff),
        "FFF" => Some(Dynamic::Fff),
        _ => None,
    };
    if !read.is_rest() {
        let whammy = child(node, "Whammy").and_then(|w| {
            let number = |name| w.attribute(name).and_then(|v| v.parse::<f64>().ok());
            curve(
                [
                    number("originValue"),
                    number("middleValue"),
                    number("destinationValue"),
                ],
                [
                    number("originOffset"),
                    number("middleOffset1"),
                    number("middleOffset2"),
                    number("destinationOffset"),
                ],
            )
        });
        read.effects = BeatEffects {
            tremolo: match text(node, "Tremolo") {
                "1/2" => Some(NoteValue::Eighth),
                "1/4" => Some(NoteValue::Sixteenth),
                "1/8" => Some(NoteValue::ThirtySecond),
                _ => None,
            },
            stroke: match direction("PickStroke") {
                Some("Up") => Some(PickStroke::Up),
                Some("Down") => Some(PickStroke::Down),
                _ => None,
            },
            strum: match (text(node, "Arpeggio"), direction("Brush")) {
                ("Up", _) => Some(Strum::ArpeggioUp),
                ("Down", _) => Some(Strum::ArpeggioDown),
                (_, Some("Up")) => Some(Strum::BrushUp),
                (_, Some("Down")) => Some(Strum::BrushDown),
                _ if enabled("Rasgueado") || child(node, "Rasgueado").is_some() => {
                    Some(Strum::Rasgueado)
                }
                _ => None,
            },
            slap: match (enabled("Slapped"), enabled("Popped")) {
                (true, _) => Some(Slap::Slap),
                (_, true) => Some(Slap::Pop),
                _ => None,
            },
            whammy: whammy.filter(|w| w.is_valid(Bend::WHAMMY_RANGE)),
            hairpin: match text(node, "Hairpin") {
                "Crescendo" => Some(Hairpin::Crescendo),
                "Decrescendo" => Some(Hairpin::Decrescendo),
                _ => None,
            },
            dynamic: read.effects.dynamic,
            fade: match text(node, "Fadding") {
                "FadeIn" => Some(Fade::In),
                "FadeOut" => Some(Fade::Out),
                "VolumeSwell" => Some(Fade::Swell),
                _ => None,
            },
            ..BeatEffects::default()
        };
        // The syllable of each line of the lyrics that is sung here.
        let mut lyrics: Vec<String> = child(node, "Lyrics")
            .into_iter()
            .flat_map(|lyrics| lyrics.children())
            .filter(|n| n.has_tag_name("Line"))
            .take(BeatEffects::MAX_LYRIC_LINES)
            .map(|line| {
                let sung = line.text().unwrap_or("").trim();
                sung.chars().take(BeatEffects::MAX_TEXT).collect()
            })
            .collect();
        while lyrics.last().is_some_and(String::is_empty) {
            lyrics.pop();
        }
        read.effects.lyrics = lyrics;
    }
    // What is written over a beat can stand over a rest as well: text, and a chord.
    read.effects.text = Some(text(node, "FreeText"))
        .filter(|t| !t.is_empty())
        .map(|t| t.chars().take(BeatEffects::MAX_TEXT).collect());
    if let Some((name, diagram)) = part.chords.get(text(node, "Chord")) {
        read.effects.chord = Some(name.clone()).filter(|name| !name.is_empty());
        read.effects.diagram =
            (diagram.clone()).filter(|d| d.frets.len() == staff.tuning.strings.len());
    }
    read
}

fn read_note_effects(note: Node) -> NoteEffects {
    let enabled = |name| property(note, name).is_some_and(|p| child(p, "Enable").is_some());
    let float = |name| property(note, name).and_then(|p| text(p, "Float").parse::<f64>().ok());
    let finger = |name| match text(note, name) {
        "P" => Some(Finger::Thumb),
        "I" => Some(Finger::Index),
        "M" => Some(Finger::Middle),
        "A" => Some(Finger::Ring),
        "C" => Some(Finger::Pinky),
        _ => None,
    };
    let accent_bits: u8 = text(note, "Accent").parse().unwrap_or(0);
    let harmonic = property(note, "HarmonicType").and_then(|p| match text(p, "HType") {
        "Natural" => Some(Harmonic::Natural),
        "Artificial" => Some(Harmonic::Artificial),
        "Pinch" => Some(Harmonic::Pinch),
        "Tap" => Some(Harmonic::Tapped),
        "Semi" => Some(Harmonic::Semi),
        _ => None,
    });
    NoteEffects {
        hammer: enabled("HopoOrigin"),
        slide: property(note, "Slide")
            .and_then(|p| text(p, "Flags").parse().ok())
            .and_then(slide_from_flags),
        bend: enabled("Bended")
            .then(|| {
                curve(
                    [
                        float("BendOriginValue"),
                        float("BendMiddleValue"),
                        float("BendDestinationValue"),
                    ],
                    [
                        float("BendOriginOffset"),
                        float("BendMiddleOffset1"),
                        float("BendMiddleOffset2"),
                        float("BendDestinationOffset"),
                    ],
                )
            })
            .flatten()
            .filter(|b| b.is_valid(Bend::BEND_RANGE)),
        vibrato: match text(note, "Vibrato") {
            "Slight" => Some(Vibrato::Slight),
            "Wide" => Some(Vibrato::Wide),
            _ => None,
        },
        harmonic,
        harmonic_touch: property(note, "HarmonicType")
            .filter(|p| matches!(text(*p, "HType"), "Artificial" | "Pinch" | "Tap" | "Semi"))
            .and_then(|_| property(note, "HarmonicFret"))
            .and_then(|p| text(p, "HFret").parse::<f32>().ok())
            .map(|fret| fret.round() as u8)
            .filter(|fret| (1..=Harmonic::MAX_TOUCH).contains(fret))
            // The usual place for its kind need not be said.
            .filter(|fret| harmonic.is_none_or(|kind| kind.usual_touch() != *fret)),
        accent: match accent_bits {
            bits if bits & 0x04 != 0 => Some(Accent::Heavy),
            bits if bits & 0x08 != 0 => Some(Accent::Normal),
            _ => None,
        },
        trill: None,
        grace: None,
        left_finger: finger("LeftFingering"),
        right_finger: finger("RightFingering"),
        palm_mute: enabled("PalmMuted"),
        let_ring: child(note, "LetRing").is_some(),
        dead: enabled("Muted"),
        ghost: child(note, "AntiAccent").is_some(),
        staccato: accent_bits & 0x01 != 0,
        tapped: enabled("Tapped"),
    }
}

#[cfg(test)]
mod tests {
    use gtr_model::Rational;

    use super::*;

    /// A score of one track and one bar, as the XML formats write it: `track` goes inside
    /// the track, `bar` inside the bar, `beats` and `notes` are the lists of those, and
    /// the bar's one voice plays the beats numbered up to `count`.
    fn document(
        track: &str,
        bar: &str,
        count: usize,
        beats: &str,
        notes: &str,
        more: &str,
    ) -> String {
        let ids: Vec<String> = (0..count).map(|id| id.to_string()).collect();
        format!(
            r#"<GPIF><Score><Title>Test</Title></Score>
            <MasterTrack><Automations>{more}</Automations></MasterTrack>
            <Tracks><Track id="0"><Name>One</Name>{track}
              <Properties><Property name="Tuning"><Pitches>40 45 50 55 59 64</Pitches></Property></Properties>
            </Track></Tracks>
            <MasterBars><MasterBar><Time>4/4</Time><Bars>0</Bars>{bar}</MasterBar></MasterBars>
            <Bars><Bar id="0"><Voices>0</Voices></Bar></Bars>
            <Voices><Voice id="0"><Beats>{}</Beats></Voice></Voices>
            <Beats>{beats}</Beats><Notes>{notes}</Notes>
            <Rhythms>
              <Rhythm id="0"><NoteValue>Quarter</NoteValue></Rhythm>
              <Rhythm id="1"><NoteValue>128th</NoteValue></Rhythm>
              <Rhythm id="2"><NoteValue>256th</NoteValue></Rhythm>
              <Rhythm id="3"><NoteValue>Half</NoteValue><AugmentationDot count="3"/></Rhythm>
            </Rhythms></GPIF>"#,
            ids.join(" ")
        )
    }

    fn read(xml: &str) -> (Score, Vec<&'static str>) {
        let mut skipped = Skipped::default();
        let score = read_xml(xml, &mut skipped).unwrap();
        score.validate().unwrap();
        (score, skipped.0.into_iter().collect())
    }

    const NOTE: &str = r#"<Note id="0"><Properties>
        <Property name="String"><String>0</String></Property>
        <Property name="Fret"><Fret>3</Fret></Property></Properties></Note>"#;

    #[test]
    fn short_notes_dots_fades_and_feels_are_read() {
        let beats = r#"
            <Beat id="0"><Rhythm ref="1"/><Notes>0</Notes><Fadding>FadeOut</Fadding></Beat>
            <Beat id="1"><Rhythm ref="2"/><Notes>0</Notes><Fadding>VolumeSwell</Fadding></Beat>
            <Beat id="2"><Rhythm ref="3"/><Notes>0</Notes><Fadding>FadeIn</Fadding></Beat>"#;
        for (written, feel) in [
            ("Dotted8th", Feel::DottedEighths),
            ("Dotted16th", Feel::DottedSixteenths),
            ("Scottish8th", Feel::ScottishEighths),
            ("Scottish16th", Feel::ScottishSixteenths),
        ] {
            let bar = format!("<TripletFeel>{written}</TripletFeel>");
            let (score, skipped) = read(&document("", &bar, 3, beats, NOTE, ""));
            assert_eq!(skipped, [] as [&str; 0]);
            assert_eq!(score.master_bars[0].feel, Some(feel));
            let read: Vec<(NoteValue, u8, Option<Fade>)> =
                (score.tracks[0].staves[0].bars[0].voices[0].beats.iter())
                    .map(|beat| (beat.duration.value, beat.duration.dots, beat.effects.fade))
                    .collect();
            assert_eq!(
                read,
                [
                    (NoteValue::HundredTwentyEighth, 0, Some(Fade::Out)),
                    (NoteValue::TwoHundredFiftySixth, 0, Some(Fade::Swell)),
                    (NoteValue::Half, 3, Some(Fade::In)),
                ]
            );
        }
    }

    #[test]
    fn a_tempo_that_changes_by_degrees_is_read_as_one() {
        let tempo = |bar: usize, position: &str, value: u32, linear: bool| {
            format!(
                "<Automation><Type>Tempo</Type><Linear>{linear}</Linear><Bar>{bar}</Bar>\
                 <Position>{position}</Position><Value>{value} 2</Value></Automation>"
            )
        };
        let more = [
            tempo(0, "0", 100, true),
            tempo(0, "0.5", 140, true),
            tempo(0, "0.75", 90, false),
        ];
        let (score, skipped) = read(&document("", "", 0, "", "", &more.concat()));
        assert_eq!(skipped, [] as [&str; 0]);
        assert_eq!(score.tempo, 100);
        let bar = &score.master_bars[0];
        assert!(bar.tempo_gradual);
        let changes: Vec<(Rational, u32, bool)> = (bar.tempo_changes.iter())
            .map(|change| (change.position, change.tempo, change.gradual))
            .collect();
        assert_eq!(
            changes,
            [
                (Rational::new(1, 2), 140, true),
                (Rational::new(3, 4), 90, false)
            ]
        );
    }

    #[test]
    fn the_drums_of_guitar_pro_6_name_their_pieces_themselves() {
        // The track only says that it is percussion; each note names a piece of the kit
        // and a way of hitting it: the snare's side stick, the hi-hat open, the ride's bell.
        let piece = |id: usize, element: usize, variation: usize| {
            format!(
                r#"<Note id="{id}"><Properties>
                   <Property name="Element"><Element>{element}</Element></Property>
                   <Property name="Variation"><Variation>{variation}</Variation></Property>
                   </Properties></Note>"#
            )
        };
        let notes = [
            piece(0, 1, 2),
            piece(1, 10, 2),
            piece(2, 15, 2),
            piece(3, 0, 0),
        ]
        .concat();
        let beats = r#"<Beat id="0"><Rhythm ref="0"/><Notes>0 1</Notes></Beat>
                       <Beat id="1"><Rhythm ref="0"/><Notes>2 3</Notes></Beat>"#;
        let track = r#"<GeneralMidi table="Percussion"/>"#;
        let (score, skipped) = read(&document(track, "", 2, beats, &notes, ""));
        assert_eq!(skipped, [] as [&str; 0]);
        assert_eq!(score.tracks[0].instrument, Instrument::Drums);
        let played: Vec<Vec<u8>> = (score.tracks[0].staves[0].bars[0].voices[0].beats.iter())
            .map(|beat| beat.notes.iter().map(|note| note.fret).collect())
            .collect();
        assert_eq!(played, [[37, 46], [53, 35]]);
    }
}
