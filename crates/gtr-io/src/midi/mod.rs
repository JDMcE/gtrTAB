//! Standard MIDI files. A score is written as it plays, and a file is read as the notes
//! it sounds, put back on bars and beats.

mod read;
mod write;

use gtr_model::Score;

use crate::{Exported, ImportError, Imported, Skipped};

/// Ticks to a quarter note in the files written here: every note value down to a 256th,
/// and triplets and quintuplets of most of them, fall on a tick.
pub(crate) const TICKS_PER_QUARTER: u32 = 960;

/// Reads a standard MIDI file.
pub fn import(bytes: &[u8]) -> Result<Imported, ImportError> {
    let mut skipped = Skipped::default();
    let score = read::read(bytes, &mut skipped)?;
    score.validate()?;
    Ok(Imported {
        score,
        skipped: skipped.list(),
    })
}

/// Writes a score as a standard MIDI file, a track for each of the score's that is heard,
/// playing what the score plays: its repeats and jumps taken, its techniques sounded.
pub fn export(score: &Score) -> Exported {
    let mut skipped = Skipped::default();
    let bytes = write::write(score, &mut skipped);
    Exported {
        bytes,
        skipped: skipped.list(),
    }
}
