//! Reading and writing scores: the native format, and the formats of other programs.

pub mod ascii;
pub mod guitar_pro;
pub mod midi;
pub mod musicxml;
pub mod native;
mod report;
mod util;

use gtr_model::Score;

pub(crate) use report::Skipped;
pub use report::{ExportError, Exported, ImportError, Imported};

/// A format of another program's that a score can be written in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    /// Guitar Pro 7 and 8.
    GuitarPro,
    MusicXml,
    Midi,
    /// Tablature as plain text.
    Ascii,
}

impl Format {
    pub const ALL: [Format; 4] = [
        Format::GuitarPro,
        Format::MusicXml,
        Format::Midi,
        Format::Ascii,
    ];

    /// What the format is called.
    pub fn name(self) -> &'static str {
        match self {
            Format::GuitarPro => "Guitar Pro",
            Format::MusicXml => "MusicXML",
            Format::Midi => "MIDI",
            Format::Ascii => "ASCII tab",
        }
    }

    /// How a file of the format ends its name.
    pub fn extension(self) -> &'static str {
        match self {
            Format::GuitarPro => "gp",
            Format::MusicXml => "musicxml",
            Format::Midi => "mid",
            Format::Ascii => "txt",
        }
    }

    /// The media type a file of the format is served as.
    pub fn media_type(self) -> &'static str {
        match self {
            Format::GuitarPro => "application/octet-stream",
            Format::MusicXml => "application/vnd.recordare.musicxml+xml",
            Format::Midi => "audio/midi",
            Format::Ascii => "text/plain",
        }
    }

    /// The format a file's name ends as, if it is one of these.
    pub fn from_extension(extension: &str) -> Option<Format> {
        match extension.to_lowercase().as_str() {
            "gp" | "gpx" | "gp5" | "gp4" | "gp3" => Some(Format::GuitarPro),
            "musicxml" | "mxl" | "xml" => Some(Format::MusicXml),
            "mid" | "midi" => Some(Format::Midi),
            "txt" | "tab" | "crd" => Some(Format::Ascii),
            _ => None,
        }
    }
}

/// Writes a score as another program's file. What the score holds that the format has no
/// place for is listed in [`Exported::skipped`].
pub fn export(score: &Score, format: Format) -> Result<Exported, ExportError> {
    Ok(match format {
        Format::GuitarPro => guitar_pro::export(score)?,
        Format::MusicXml => musicxml::export(score),
        Format::Midi => midi::export(score),
        Format::Ascii => ascii::export(score),
    })
}

/// The format a file is in, from how it starts, and failing that from its name.
pub fn detect(name: &str, bytes: &[u8]) -> Option<Format> {
    let text = match bytes {
        [0xEF, 0xBB, 0xBF, rest @ ..] => rest,
        bytes => bytes,
    };
    let start = text
        .iter()
        .position(|b| !b.is_ascii_whitespace())
        .unwrap_or(0);
    let named = name
        .rsplit_once('.')
        .and_then(|(_, extension)| Format::from_extension(extension));
    if bytes.starts_with(b"MThd") {
        Some(Format::Midi)
    } else if bytes.starts_with(b"BCFZ")
        || bytes.starts_with(b"BCFS")
        || bytes.get(1..19) == Some(b"FICHIER GUITAR PRO")
    {
        Some(Format::GuitarPro)
    } else if bytes.starts_with(b"PK") {
        // Two formats are zip archives; the name says which, and each is tried.
        Some(
            named
                .filter(|f| *f == Format::MusicXml)
                .unwrap_or(Format::GuitarPro),
        )
    } else if text.get(start) == Some(&b'<') || matches!(bytes, [0xFF, 0xFE, ..] | [0xFE, 0xFF, ..])
    {
        Some(Format::MusicXml)
    } else {
        named
    }
}

/// Puts files in one zip archive under the names given, as for downloading many at once.
/// Files that are compressed already (PDF, PNG, Guitar Pro, the native format) are stored
/// as they are; the rest are compressed.
pub fn bundle<'a>(
    files: impl IntoIterator<Item = (&'a str, &'a [u8])>,
) -> Result<Vec<u8>, ExportError> {
    use std::io::{Cursor, Write};
    use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};

    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    for (name, bytes) in files {
        let packed = [".gtrtab", ".gp", ".pdf", ".png", ".mxl"]
            .iter()
            .any(|end| name.to_lowercase().ends_with(end));
        let options = SimpleFileOptions::default()
            .compression_method(if packed {
                CompressionMethod::Stored
            } else {
                CompressionMethod::Deflated
            })
            .large_file(bytes.len() >= u32::MAX as usize);
        zip.start_file(name, options)?;
        zip.write_all(bytes)?;
    }
    Ok(zip.finish()?.into_inner())
}

/// Reads a file written by another program, working out which from its contents and its
/// name. What the file holds that there is no place for is listed in
/// [`Imported::skipped`].
pub fn import(name: &str, bytes: &[u8]) -> Result<Imported, ImportError> {
    let by = |format: Format| match format {
        Format::GuitarPro => guitar_pro::import(bytes),
        Format::MusicXml => musicxml::import(bytes),
        Format::Midi => midi::import(bytes),
        Format::Ascii => ascii::import(bytes),
    };
    match detect(name, bytes) {
        // An archive that is not the one kind may be the other.
        Some(first) if bytes.starts_with(b"PK") => {
            let other = match first {
                Format::MusicXml => Format::GuitarPro,
                _ => Format::MusicXml,
            };
            by(first).or_else(|error| match error {
                ImportError::UnknownFormat => by(other),
                error => Err(error),
            })
        }
        Some(format) => by(format),
        // Nothing says what it is: tablature typed as text, if it is anything.
        None => ascii::import(bytes),
    }
}
