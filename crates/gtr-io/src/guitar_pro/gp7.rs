//! Guitar Pro 7 and 8 (`.gp`): a zip archive whose `Content/score.gpif` is an XML document.
//! Bars, voices, beats, notes and rhythms are stored in flat lists and refer to each other
//! by id.

use std::collections::HashMap;
use std::io::{Cursor, Read};

use gtr_model::{
    Accent, BeatEffects, Harmonic, MasterBar, Note, NoteEffects, NoteValue, PickStroke, Score,
    ScoreInfo, Staff, TimeSignature, Track, Vibrato, Voice,
};
use roxmltree::{Document, Node};
use zip::ZipArchive;

use super::{
    ImportError, Skipped, bar, beat, bend_from_heights, duration, slide_from_flags, tuning,
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
        artist: text(score, "Artist").into(),
        album: text(score, "Album").into(),
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
    for node in child(root, "Tracks").into_iter().flat_map(|t| t.children()) {
        if !node.has_tag_name("Track") {
            continue;
        }
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
        tracks.push(Track {
            name: text(node, "Name").into(),
            staves,
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
        skipped.add_if(child(node, "Repeat").is_some(), "repeats");
        skipped.add_if(
            child(node, "AlternateEndings").is_some(),
            "alternate endings",
        );
        skipped.add_if(child(node, "Section").is_some(), "section markers");
        let (numerator, denominator) = text(node, "Time").split_once('/').unwrap_or(("4", "4"));
        let time_signature = TimeSignature {
            numerator: numerator.trim().parse().unwrap_or(4),
            denominator: denominator.trim().parse().unwrap_or(4),
        };
        master_bars.push(MasterBar {
            time_signature: match time_signature.is_valid() {
                true => time_signature,
                false => TimeSignature::COMMON,
            },
        });

        // One bar id per staff, in track order.
        let mut ids = text(node, "Bars").split_whitespace();
        for staff in tracks.iter_mut().flat_map(|t| &mut t.staves) {
            let string_count = staff.tuning.strings.len();
            let voice_ids = ids
                .next()
                .and_then(|id| bars.get(id))
                .map(|bar| text(*bar, "Voices"))
                .unwrap_or("");
            let read_voices = voice_ids
                .split_whitespace()
                .filter_map(|id| voices.get(id))
                .map(|voice| Voice {
                    beats: text(*voice, "Beats")
                        .split_whitespace()
                        .filter_map(|id| beats.get(id))
                        .map(|b| read_beat(*b, &notes, &rhythms, string_count, skipped))
                        .collect(),
                })
                .collect();
            staff.bars.push(bar(read_voices));
        }
    }

    Ok(Score {
        info,
        tempo,
        master_bars,
        tracks,
    })
}

fn read_beat(
    node: Node,
    notes: &HashMap<&str, Node>,
    rhythms: &HashMap<&str, Node>,
    string_count: usize,
    skipped: &mut Skipped,
) -> gtr_model::Beat {
    skipped.add_if(child(node, "GraceNotes").is_some(), "grace notes");
    skipped.add_if(child(node, "Whammy").is_some(), "whammy bar");
    skipped.add_if(child(node, "Chord").is_some(), "chord names");
    skipped.add_if(child(node, "FreeText").is_some(), "text");
    skipped.add_if(child(node, "Lyrics").is_some(), "lyrics");

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
            let tuplet = child(*rhythm, "PrimaryTuplet").and_then(|t| {
                Some((
                    t.attribute("num")?.parse().ok()?,
                    t.attribute("den")?.parse().ok()?,
                ))
            });
            duration(value, dots, tuplet, skipped)
        }
        None => gtr_model::Duration::new(NoteValue::Quarter),
    };

    let read_notes = text(node, "Notes")
        .split_whitespace()
        .filter_map(|id| notes.get(id))
        .filter_map(|note| {
            let fret: u8 = text(property(*note, "Fret")?, "Fret").parse().ok()?;
            // Strings are numbered from the lowest; the model counts from the highest.
            let from_low: usize = text(property(*note, "String")?, "String").parse().ok()?;
            let string = string_count.checked_sub(from_low + 1)?;
            Some(Note {
                string: string as u8,
                fret,
                tied: child(*note, "Tie").and_then(|t| t.attribute("destination")) == Some("true"),
                effects: read_note_effects(*note),
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
    let mut read = beat(read_duration, read_notes, string_count);
    if !read.is_rest() {
        read.effects = BeatEffects {
            tremolo: match text(node, "Tremolo") {
                "1/2" => Some(NoteValue::Eighth),
                "1/4" => Some(NoteValue::Sixteenth),
                "1/8" => Some(NoteValue::ThirtySecond),
                _ => None,
            },
            stroke: match property(node, "PickStroke").map(|p| text(p, "Direction")) {
                Some("Up") => Some(PickStroke::Up),
                Some("Down") => Some(PickStroke::Down),
                _ => None,
            },
        };
    }
    read
}

fn read_note_effects(note: Node) -> NoteEffects {
    let enabled = |name| property(note, name).is_some_and(|p| child(p, "Enable").is_some());
    let float = |name| {
        property(note, name)
            .and_then(|p| text(p, "Float").parse::<f64>().ok())
            .unwrap_or(0.0)
            .round() as i32
    };
    let accent_bits: u8 = text(note, "Accent").parse().unwrap_or(0);
    NoteEffects {
        hammer: enabled("HopoOrigin"),
        slide: property(note, "Slide")
            .and_then(|p| text(p, "Flags").parse().ok())
            .and_then(slide_from_flags),
        bend: enabled("Bended")
            .then(|| {
                bend_from_heights(&[
                    float("BendOriginValue"),
                    float("BendMiddleValue"),
                    float("BendDestinationValue"),
                ])
            })
            .flatten(),
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
        accent: match accent_bits {
            bits if bits & 0x04 != 0 => Some(Accent::Heavy),
            bits if bits & 0x08 != 0 => Some(Accent::Normal),
            _ => None,
        },
        palm_mute: enabled("PalmMuted"),
        let_ring: child(note, "LetRing").is_some(),
        dead: enabled("Muted"),
        ghost: child(note, "AntiAccent").is_some(),
        staccato: accent_bits & 0x01 != 0,
        tapped: enabled("Tapped"),
    }
}
