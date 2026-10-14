//! MusicXML, the format most notation programs share: `.musicxml` and `.xml` as they
//! stand, and `.mxl`, which is the same in a zip archive.

mod read;
mod write;

use gtr_model::Score;

use crate::{Exported, ImportError, Imported, Skipped};

/// Reads a MusicXML file, compressed or not.
pub fn import(bytes: &[u8]) -> Result<Imported, ImportError> {
    let mut skipped = Skipped::default();
    let score = read::read(bytes, &mut skipped)?;
    score.validate()?;
    Ok(Imported {
        score,
        skipped: skipped.list(),
    })
}

/// Writes a score as an uncompressed MusicXML file.
pub fn export(score: &Score) -> Exported {
    let mut skipped = Skipped::default();
    let xml = write::write(score, &mut skipped);
    skipped.add_if(!score.style.is_default(), "the stylesheet");
    Exported {
        bytes: xml.into_bytes(),
        skipped: skipped.list(),
    }
}
