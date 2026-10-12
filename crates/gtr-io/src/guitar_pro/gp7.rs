//! Guitar Pro 7 and 8 (`.gp`): a zip archive whose `Content/score.gpif` is an XML document.
//! Bars, voices, beats, notes and rhythms are stored in flat lists and refer to each other
//! by id.

use std::collections::HashMap;
use std::io::{Cursor, Read};

use gtr_model::{
    Accent, Beat, BeatEffects, Bend, Duration, Dynamic, Feel, Finger, Grace, GraceNote, Hairpin,
    Harmonic, Instrument, Jump, MasterBar, Note, NoteEffects, NoteValue, PickStroke, Score,
    ScoreInfo, Slap, Staff, Strum, Target, TimeSignature, Track, Trill, Tuplet, Vibrato, Voice,
};
use roxmltree::{Document, Node};
use zip::ZipArchive;

use super::{
    ImportError, Skipped, bar, beat, bend_from_points, duration, keep_dynamic_changes,
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
    let document = Document::parse(&xml).map_err(|e| corrupt(e.to_string()))?;
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

    // Tempo automations hold "<bpm> <unit>"; the first one is the starting tempo.
    let tempos: Vec<Node> = (root.descendants())
        .filter(|n| n.has_tag_name("Automation") && text(*n, "Type") == "Tempo")
        .collect();
    skipped.add_if(tempos.len() > 1, "tempo changes");
    let tempo = tempos.first().map_or(120, |automation| {
        let mut parts = text(*automation, "Value").split_whitespace();
        let bpm: f64 = parts.next().and_then(|v| v.parse().ok()).unwrap_or(120.0);
        let per_quarter = match parts.next() {
            Some("1") => 0.5,
            Some("3") => 1.5,
            Some("4") => 2.0,
            Some("5") => 3.0,
            _ => 1.0,
        };
        (bpm * per_quarter).round().clamp(1.0, 1000.0) as u32
    });

    let mut tracks = Vec::new();
    // For each track that is a drum kit, the General MIDI number each of its articulations
    // sounds: a note on such a track says which articulation it is by its place in this list.
    let mut kits: Vec<Option<Vec<u8>>> = Vec::new();
    for node in child(root, "Tracks").into_iter().flat_map(|t| t.children()) {
        if !node.has_tag_name("Track") {
            continue;
        }
        let kit = child(node, "InstrumentSet")
            .filter(|set| text(*set, "Type") == "drumKit")
            .map(|set| {
                (set.descendants())
                    .filter(|n| n.has_tag_name("Articulation"))
                    .map(|a| text(a, "OutputMidiNumber").parse().unwrap_or(0))
                    .collect::<Vec<u8>>()
            });
        // Guitar Pro 7 nests properties in <Staff> elements; older files put one set of
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
                skipped.add_if(pitches.is_empty(), "percussion and untuned tracks");
                Staff {
                    tuning: tuning(pitches),
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
            None => program.map_or_else(Instrument::default, Instrument::from_midi_program),
        };
        tracks.push(Track::new(text(node, "Name"), instrument, staves));
        kits.push(kit);
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
            // A bar here has one sign and one jump: the first of each.
            let several = |name| {
                directions
                    .children()
                    .filter(|n| n.has_tag_name(name))
                    .count()
                    > 1
            };
            skipped.add_if(
                several("Target") || several("Jump"),
                "all but the first sign and jump on a bar",
            );
            master.target = match text(directions, "Target") {
                "Segno" | "SegnoSegno" => Some(Target::Segno),
                "Coda" | "DoubleCoda" => Some(Target::Coda),
                "Fine" => Some(Target::Fine),
                _ => None,
            };
            master.jump = match text(directions, "Jump") {
                "DaCapo" => Some(Jump::DaCapo),
                "DaCapoAlFine" => Some(Jump::DaCapoAlFine),
                "DaCapoAlCoda" | "DaCapoAlDoubleCoda" => Some(Jump::DaCapoAlCoda),
                "DaSegno" | "DaSegnoSegno" => Some(Jump::DalSegno),
                "DaSegnoAlFine" | "DaSegnoSegnoAlFine" => Some(Jump::DalSegnoAlFine),
                "DaSegnoAlCoda"
                | "DaSegnoAlDoubleCoda"
                | "DaSegnoSegnoAlCoda"
                | "DaSegnoSegnoAlDoubleCoda" => Some(Jump::DalSegnoAlCoda),
                "DaCoda" | "DaDoubleCoda" => Some(Jump::ToCoda),
                _ => None,
            };
        }
        master.feel = match text(node, "TripletFeel") {
            "Triplet8th" => Some(Feel::SwingEighths),
            "Triplet16th" => Some(Feel::SwingSixteenths),
            "" | "NoTripletFeel" => None,
            _ => {
                skipped.add("dotted and scottish feels");
                None
            }
        };
        master_bars.push(master);

        // One bar id per staff, in track order.
        let mut ids = text(node, "Bars").split_whitespace();
        let staves = (tracks.iter_mut().zip(&kits))
            .flat_map(|(track, kit)| track.staves.iter_mut().map(move |s| (s, kit.as_deref())));
        for (staff, kit) in staves {
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
                    read_voice(nodes, &notes, &rhythms, staff, kit, skipped)
                })
                .collect();
            staff.bars.push(bar(read_voices));
        }
    }
    for staff in tracks.iter_mut().flat_map(|t| &mut t.staves) {
        keep_dynamic_changes(staff);
    }

    Ok(Score {
        info,
        tempo,
        master_bars,
        tracks,
    })
}

/// Reads the beats of a voice. Grace notes are stored as beats of their own before the beat
/// they decorate; the model attaches them to that beat's notes instead.
/// Lines a drum staff is given.
const DRUM_LINES: usize = 5;

fn read_voice<'a>(
    nodes: impl Iterator<Item = Node<'a, 'a>>,
    notes: &HashMap<&str, Node>,
    rhythms: &HashMap<&str, Node>,
    staff: &Staff,
    kit: Option<&[u8]>,
    skipped: &mut Skipped,
) -> Voice {
    let mut beats = Vec::new();
    // Grace notes waiting for the beat they lead into: each with whether it is on the
    // beat and whether it is hammered.
    let mut graces: Vec<(GraceNote, bool, bool)> = Vec::new();
    for node in nodes {
        let mut beat = read_beat(node, notes, rhythms, staff, kit, skipped);
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
    Voice { beats }
}

/// A pitch curve from Guitar Pro's origin, middle and destination values, which are in
/// hundredths of a whole tone at offsets in hundredths of the note's length.
fn curve(values: [f64; 3], offsets: [Option<f64>; 4]) -> Option<Bend> {
    let [origin, middle, destination] = values;
    let points = [
        (offsets[0].unwrap_or(0.0), origin),
        (offsets[1].unwrap_or(f64::NAN), middle),
        (offsets[2].unwrap_or(f64::NAN), middle),
        (offsets[3].unwrap_or(100.0), destination),
    ];
    bend_from_points(
        (points.into_iter())
            .filter(|(offset, _)| !offset.is_nan())
            .map(|(offset, value)| ((offset * 0.6).round() as i32, value.round() as i32)),
    )
}

fn read_beat(
    node: Node,
    notes: &HashMap<&str, Node>,
    rhythms: &HashMap<&str, Node>,
    staff: &Staff,
    kit: Option<&[u8]>,
    skipped: &mut Skipped,
) -> Beat {
    skipped.add_if(child(node, "Chord").is_some(), "chord names");
    skipped.add_if(child(node, "FreeText").is_some(), "text");
    skipped.add_if(child(node, "Lyrics").is_some(), "lyrics");
    skipped.add_if(child(node, "Fadding").is_some(), "fade in");
    let string_count = staff.tuning.strings.len();

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
                let piece = (text(*note, "InstrumentArticulation").parse::<usize>().ok())
                    .and_then(|index| kit.get(index).copied())
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
            let fret: u8 = text(property(*note, "Fret")?, "Fret").parse().ok()?;
            // Strings are numbered from the lowest; the model counts from the highest.
            let from_low: usize = text(property(*note, "String")?, "String").parse().ok()?;
            let string = string_count.checked_sub(from_low + 1)? as u8;
            let mut effects = read_note_effects(*note);
            // A trill names the other note by its MIDI pitch.
            effects.trill = text(*note, "Trill").parse::<u8>().ok().and_then(|pitch| {
                let open = staff.tuning.pitch(string, 0, staff.capo)?;
                Some(Trill {
                    fret: pitch.checked_sub(open)?,
                    speed: NoteValue::Sixteenth,
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
                    number("originValue").unwrap_or(0.0),
                    number("middleValue").unwrap_or(0.0),
                    number("destinationValue").unwrap_or(0.0),
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
            // Free text on a beat; chord names and lyrics are not read yet.
            text: Some(text(node, "FreeText"))
                .filter(|t| !t.is_empty())
                .map(|t| t.chars().take(BeatEffects::MAX_TEXT).collect()),
            ..BeatEffects::default()
        };
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
    NoteEffects {
        hammer: enabled("HopoOrigin"),
        slide: property(note, "Slide")
            .and_then(|p| text(p, "Flags").parse().ok())
            .and_then(slide_from_flags),
        bend: enabled("Bended")
            .then(|| {
                curve(
                    [
                        float("BendOriginValue").unwrap_or(0.0),
                        float("BendMiddleValue").unwrap_or(0.0),
                        float("BendDestinationValue").unwrap_or(0.0),
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
        harmonic: property(note, "HarmonicType").and_then(|p| match text(p, "HType") {
            "Natural" => Some(Harmonic::Natural),
            "Artificial" => Some(Harmonic::Artificial),
            "Pinch" => Some(Harmonic::Pinch),
            "Tap" => Some(Harmonic::Tapped),
            "Semi" => Some(Harmonic::Semi),
            _ => None,
        }),
        harmonic_touch: property(note, "HarmonicType")
            .filter(|p| matches!(text(*p, "HType"), "Artificial" | "Pinch" | "Tap" | "Semi"))
            .and_then(|_| property(note, "HarmonicFret"))
            .and_then(|p| text(p, "HFret").parse::<f32>().ok())
            .map(|fret| fret.round() as u8)
            .filter(|fret| (1..=Harmonic::MAX_TOUCH).contains(fret)),
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
