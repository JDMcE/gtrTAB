//! The native `.gtrtab` format: a zip archive holding `score.json`.

use std::io::{Cursor, Read, Write};

use gtr_model::{Bend, BendKind, ModelError, Score};
use serde::Serialize;
use serde_json::Value;
use zip::{ZipArchive, ZipWriter, write::SimpleFileOptions};

pub const EXTENSION: &str = "gtrtab";

/// Bump when the JSON layout changes, and add a step to [`migrate`].
pub const FORMAT_VERSION: u64 = 9;

const SCORE_ENTRY: &str = "score.json";

#[derive(Debug, thiserror::Error)]
pub enum NativeError {
    #[error("not a valid gtrTab file: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("score data is malformed: {0}")]
    Json(#[from] serde_json::Error),
    #[error("file has no format version")]
    MissingVersion,
    #[error("file uses format version {found}, but this build only reads up to {FORMAT_VERSION}")]
    UnsupportedVersion { found: u64 },
    #[error("score is inconsistent: {0}")]
    Invalid(#[from] ModelError),
}

#[derive(Serialize)]
struct Envelope<'a> {
    format_version: u64,
    score: &'a Score,
}

pub fn save(score: &Score) -> Result<Vec<u8>, NativeError> {
    let json = serde_json::to_vec(&Envelope {
        format_version: FORMAT_VERSION,
        score,
    })?;
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    zip.start_file(SCORE_ENTRY, SimpleFileOptions::default())?;
    zip.write_all(&json)?;
    Ok(zip.finish()?.into_inner())
}

pub fn load(bytes: &[u8]) -> Result<Score, NativeError> {
    let mut json = Vec::new();
    ZipArchive::new(Cursor::new(bytes))?
        .by_name(SCORE_ENTRY)?
        .read_to_end(&mut json)?;

    let mut envelope: Value = serde_json::from_slice(&json)?;
    let version = envelope
        .get("format_version")
        .and_then(Value::as_u64)
        .ok_or(NativeError::MissingVersion)?;
    if version == 0 || version > FORMAT_VERSION {
        return Err(NativeError::UnsupportedVersion { found: version });
    }
    let mut score_json = envelope
        .get_mut("score")
        .map(Value::take)
        .unwrap_or_default();
    migrate(version, &mut score_json);

    let mut score: Score = serde_json::from_value(score_json)?;
    score.validate()?;
    if version < 6 {
        // Version 6 writes a piano's notes as pitches. Before, each hand had six made-up
        // strings to hold them.
        for track in score
            .tracks
            .iter_mut()
            .filter(|t| t.instrument.is_pitched())
        {
            for staff in &mut track.staves {
                *staff = staff.to_pitched();
            }
        }
    }
    Ok(score)
}

/// Upgrades score JSON written by an older format version to the current layout.
fn migrate(from: u64, score: &mut Value) {
    // Version 2 added note and beat effects as optional fields, so version 1 needs nothing.
    // Version 4 likewise only added optional fields, for instruments and the mixer.
    if from < 3 {
        // Version 3 stores a bend as a curve instead of a shape and a height.
        for_each_note(score, |note| {
            let Some(bend) = note.pointer_mut("/effects/bend") else {
                return;
            };
            let kind = serde_json::from_value::<BendKind>(bend["kind"].take());
            let height = bend["quarter_tones"].as_i64().unwrap_or(4).clamp(1, 12) as i8;
            if let Ok(curve) =
                serde_json::to_value(Bend::preset(kind.unwrap_or(BendKind::Bend), height))
            {
                *bend = curve;
            }
        });
    }
    if from < 5 {
        // Version 5 lets a grace be several notes, each on a string of its own. It also
        // added tempo changes inside a bar and where a harmonic is touched, as optional
        // fields.
        for_each_note(score, |note| {
            let string = note["string"].clone();
            let Some(grace) = note.pointer_mut("/effects/grace") else {
                return;
            };
            if let Some(fields) = grace.as_object_mut()
                && let Some(fret) = fields.remove("fret")
            {
                fields.insert(
                    "notes".into(),
                    serde_json::json!([{ "string": string, "fret": fret }]),
                );
            }
        });
    }
    if from < 7 {
        // Version 7 also added shorter notes, a third dot, more feels, fades and gradual
        // changes of tempo, none of which an older file can have.
        list_signs_and_jumps(score);
    }
    // Version 8 added the pedals of each track and of the mix, and version 9 a track's
    // other sounds and the beats that switch to them, all as optional fields.
}

/// Version 7 lets a bar be several signs and have several jumps: what was one of each, if
/// there was one, becomes a list of one.
fn list_signs_and_jumps(score: &mut Value) {
    let bars = score.get_mut("master_bars").and_then(Value::as_array_mut);
    for bar in bars.into_iter().flatten().filter_map(Value::as_object_mut) {
        for (one, several) in [("target", "targets"), ("jump", "jumps")] {
            if let Some(value) = bar.remove(one).filter(|value| !value.is_null()) {
                bar.insert(several.into(), Value::Array(vec![value]));
            }
        }
    }
}

fn for_each_note(score: &mut Value, mut visit: impl FnMut(&mut Value)) {
    let each = |value: &mut Value, key: &str, f: &mut dyn FnMut(&mut Value)| {
        if let Some(items) = value.get_mut(key).and_then(Value::as_array_mut) {
            items.iter_mut().for_each(f);
        }
    };
    each(score, "tracks", &mut |track| {
        each(track, "staves", &mut |staff| {
            each(staff, "bars", &mut |bar| {
                each(bar, "voices", &mut |voice| {
                    each(voice, "beats", &mut |beat| each(beat, "notes", &mut visit));
                });
            });
        });
    });
}

#[cfg(test)]
mod tests {
    use gtr_model::demo_score;

    use super::*;

    fn archive(json: &str) -> Vec<u8> {
        let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
        zip.start_file(SCORE_ENTRY, SimpleFileOptions::default())
            .unwrap();
        zip.write_all(json.as_bytes()).unwrap();
        zip.finish().unwrap().into_inner()
    }

    #[test]
    fn round_trip() {
        let score = demo_score();
        assert_eq!(load(&save(&score).unwrap()).unwrap(), score);
    }

    #[test]
    fn version_2_bends_become_curves() {
        let mut score = serde_json::to_value(demo_score()).unwrap();
        score["tracks"][0]["staves"][0]["bars"][0]["voices"][0]["beats"][0]["notes"][0]["effects"] =
            serde_json::json!({ "bend": { "kind": "bend_release", "quarter_tones": 2 } });
        let json = serde_json::json!({ "format_version": 2, "score": score }).to_string();
        let loaded = load(&archive(&json)).unwrap();
        let note = &loaded.tracks[0].staves[0].bars[0].voices[0].beats[0].notes[0];
        assert_eq!(
            note.effects.bend,
            Some(Bend::preset(BendKind::BendRelease, 2))
        );
    }

    #[test]
    fn version_4_grace_notes_become_lists() {
        let mut score = serde_json::to_value(demo_score()).unwrap();
        let note =
            &mut score["tracks"][0]["staves"][0]["bars"][0]["voices"][0]["beats"][0]["notes"][0];
        let string = note["string"].as_u64().unwrap() as u8;
        note["effects"] =
            serde_json::json!({ "grace": { "fret": 3, "on_beat": true, "hammer": true } });
        let json = serde_json::json!({ "format_version": 4, "score": score }).to_string();
        let loaded = load(&archive(&json)).unwrap();
        let note = &loaded.tracks[0].staves[0].bars[0].voices[0].beats[0].notes[0];
        assert_eq!(
            note.effects.grace,
            Some(gtr_model::Grace::single(string, 3, true, true))
        );
        // And what is saved now is read back as it is.
        assert_eq!(load(&save(&loaded).unwrap()).unwrap(), loaded);
    }

    #[test]
    fn version_6_signs_and_jumps_become_lists() {
        // A bar had one sign and one jump, each under its own name.
        let mut score = serde_json::to_value(demo_score()).unwrap();
        score["master_bars"][0]["target"] = "segno".into();
        score["master_bars"][1]["jump"] = "dal_segno_al_fine".into();
        score["master_bars"][1]["target"] = serde_json::Value::Null;
        let json = serde_json::json!({ "format_version": 6, "score": score }).to_string();
        let loaded = load(&archive(&json)).unwrap();
        assert_eq!(loaded.master_bars[0].targets, [gtr_model::Target::Segno]);
        assert!(loaded.master_bars[0].jumps.is_empty());
        assert_eq!(
            loaded.master_bars[1].jumps,
            [gtr_model::Jump::DalSegnoAlFine]
        );
        assert!(loaded.master_bars[1].targets.is_empty());

        // What version 7 added goes out and comes back as it was.
        let mut score = demo_score();
        let bar = &mut score.master_bars[0];
        bar.targets = vec![gtr_model::Target::SegnoSegno, gtr_model::Target::DoubleCoda];
        bar.jumps = vec![
            gtr_model::Jump::DalSegnoSegno,
            gtr_model::Jump::ToDoubleCoda,
        ];
        bar.feel = Some(gtr_model::Feel::ScottishSixteenths);
        bar.tempo_gradual = true;
        bar.tempo_changes = vec![gtr_model::TempoChange {
            position: gtr_model::Rational::new(1, 2),
            tempo: 90,
            gradual: true,
        }];
        let beat = &mut score.tracks[0].staves[0].bars[0].voices[0].beats[0];
        beat.effects.fade = Some(gtr_model::Fade::Swell);
        beat.duration.dots = 3;
        beat.duration.value = gtr_model::NoteValue::TwoHundredFiftySixth;
        assert_eq!(load(&save(&score).unwrap()).unwrap(), score);

        // And what version 8 added.
        use gtr_model::{Pedal, PedalKind};
        score.tracks[0].effects = vec![
            Pedal::with(PedalKind::Amp, &[80, 40]),
            Pedal {
                on: false,
                ..Pedal::new(PedalKind::Delay)
            },
        ];
        score.effects = vec![Pedal::new(PedalKind::Reverb)];
        assert_eq!(load(&save(&score).unwrap()).unwrap(), score);

        // And what version 9 added.
        score.tracks[0].sounds = vec![gtr_model::Sound {
            name: "Lead".into(),
            instrument: gtr_model::Instrument::DistortionGuitar,
            effects: vec![Pedal::new(PedalKind::Delay)],
        }];
        score.tracks[0].staves[0].bars[0].voices[0].beats[1]
            .effects
            .sound = Some(1);
        assert_eq!(load(&save(&score).unwrap()).unwrap(), score);
    }

    #[test]
    fn version_5_pianos_lose_their_strings() {
        use gtr_model::{Beat, Duration, Instrument, Note, NoteValue, Staff, Track, Tuning};

        let mut score = demo_score();
        let bars = score.tracks[0].staves[0].bars.clone();
        let mut right = Staff {
            tuning: Tuning::new([76, 71, 67, 62, 57, 52]),
            capo: 0,
            bars,
        };
        // E5 on the open first string, twice over with the fifth fret of the second, and
        // middle C on the fourth.
        right.bars[0].voices[0].beats = vec![Beat::new(
            Duration::new(NoteValue::Quarter),
            vec![Note::new(0, 0), Note::new(1, 5), Note::new(4, 3)],
        )];
        score
            .tracks
            .push(Track::new("Piano", Instrument::Piano, vec![right]));
        let json = serde_json::json!({ "format_version": 5, "score": score }).to_string();
        let loaded = load(&archive(&json)).unwrap();
        let staff = &loaded.tracks[1].staves[0];
        assert!(staff.is_pitched());
        let pitches: Vec<(u8, u8)> = (staff.bars[0].voices[0].beats[0].notes.iter())
            .map(|n| (n.string, n.fret))
            .collect();
        assert_eq!(pitches, [(60, 0), (76, 0)]);
        // The guitar keeps its strings, and what is saved now is read back as it is.
        assert_eq!(loaded.tracks[0], score.tracks[0]);
        assert_eq!(load(&save(&loaded).unwrap()).unwrap(), loaded);
    }

    #[test]
    fn rejects_garbage() {
        assert!(matches!(load(b"not a zip"), Err(NativeError::Zip(_))));
    }

    #[test]
    fn rejects_newer_version() {
        let bytes = archive(r#"{"format_version": 999, "score": {}}"#);
        assert!(matches!(
            load(&bytes),
            Err(NativeError::UnsupportedVersion { found: 999 })
        ));
    }

    #[test]
    fn rejects_missing_version() {
        let bytes = archive(r#"{"score": {}}"#);
        assert!(matches!(load(&bytes), Err(NativeError::MissingVersion)));
    }

    #[test]
    fn rejects_inconsistent_score() {
        let mut score = demo_score();
        score.master_bars.pop();
        let json = serde_json::to_string(&Envelope {
            format_version: FORMAT_VERSION,
            score: &score,
        })
        .unwrap();
        assert!(matches!(
            load(&archive(&json)),
            Err(NativeError::Invalid(_))
        ));
    }
}
