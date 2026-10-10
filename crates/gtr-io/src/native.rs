//! The native `.gtrtab` format: a zip archive holding `score.json`.

use std::io::{Cursor, Read, Write};

use gtr_model::{Bend, BendKind, ModelError, Score};
use serde::Serialize;
use serde_json::Value;
use zip::{ZipArchive, ZipWriter, write::SimpleFileOptions};

pub const EXTENSION: &str = "gtrtab";

/// Bump when the JSON layout changes, and add a step to [`migrate`].
pub const FORMAT_VERSION: u64 = 3;

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

    let score: Score = serde_json::from_value(score_json)?;
    score.validate()?;
    Ok(score)
}

/// Upgrades score JSON written by an older format version to the current layout.
fn migrate(from: u64, score: &mut Value) {
    // Version 2 added note and beat effects as optional fields, so version 1 needs nothing.
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
