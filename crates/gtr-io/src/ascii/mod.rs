//! Tablature as plain text, the way it is typed and passed around.

mod read;
mod write;

use gtr_model::Score;

use crate::util::decode;
use crate::{Exported, ImportError, Imported, Skipped};

/// Reads tablature typed as plain text.
pub fn import(bytes: &[u8]) -> Result<Imported, ImportError> {
    // Text has no bytes that are nothing; a file with some is another kind of file.
    if bytes.contains(&0) && !matches!(bytes, [0xFF, 0xFE, ..] | [0xFE, 0xFF, ..]) {
        return Err(ImportError::UnknownFormat);
    }
    let mut skipped = Skipped::default();
    let score = read::read(&decode(bytes), &mut skipped)?;
    score.validate()?;
    Ok(Imported {
        score,
        skipped: skipped.list(),
    })
}

/// Writes the tracks of a score that are played on strings, and its drums, as text.
pub fn export(score: &Score) -> Exported {
    let mut skipped = Skipped::default();
    let text = write::write(score, &mut skipped);
    Exported {
        bytes: text.into_bytes(),
        skipped: skipped.list(),
    }
}
