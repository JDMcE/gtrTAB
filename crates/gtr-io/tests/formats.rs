//! The formats together: a file is known for what it is, and every score of the corpus
//! can be written in every format and read again.

mod common;

use common::{band, corpus, files};
use gtr_io::{Format, ImportError, detect, export, import};
use gtr_model::Score;

fn notes(score: &Score) -> usize {
    (score.tracks.iter().flat_map(|t| &t.staves))
        .flat_map(|s| &s.bars)
        .flat_map(|b| &b.voices)
        .flat_map(|v| &v.beats)
        .map(|b| b.notes.len())
        .sum()
}

#[test]
fn a_file_is_known_by_how_it_starts_and_then_by_its_name() {
    let score = band();
    for format in Format::ALL {
        let written = export(&score, format).unwrap();
        // Whatever it is called, except that text has nothing to be known by but its name.
        let name = match format {
            Format::Ascii => "song.tab",
            _ => "song.bin",
        };
        assert_eq!(detect(name, &written.bytes), Some(format), "{format:?}");
        let read = import(name, &written.bytes).unwrap_or_else(|e| panic!("{format:?}: {e}"));
        assert!(notes(&read.score) > 50, "{format:?}");
        assert_eq!(Format::from_extension(format.extension()), Some(format));
    }
    // The files of the corpus, each from its contents alone.
    for (extension, format) in [
        ("gp3", Format::GuitarPro),
        ("gp4", Format::GuitarPro),
        ("gp5", Format::GuitarPro),
        ("gpx", Format::GuitarPro),
        ("gp", Format::GuitarPro),
        ("mxl", Format::MusicXml),
        ("musicxml", Format::MusicXml),
        ("mid", Format::Midi),
    ] {
        let found = files(&corpus(), &[extension]);
        assert!(!found.is_empty(), "{extension}");
        for path in found {
            let bytes = std::fs::read(&path).unwrap();
            // The two formats that are archives are told apart by trying each.
            if extension != "mxl" {
                assert_eq!(
                    detect("unnamed", &bytes),
                    Some(format),
                    "{}",
                    path.display()
                );
            }
            import("unnamed", &bytes).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        }
    }
    // A compressed MusicXML file under a name that says nothing is still found out.
    let mxl = std::fs::read(corpus().join("alphatab/musicxml3/compressed.mxl")).unwrap();
    assert!(import("score.zip", &mxl).is_ok());
    assert!(matches!(
        import("notes.txt", b"just words"),
        Err(ImportError::UnknownFormat)
    ));
    assert!(matches!(
        import("data.bin", &[0, 1, 2, 3]),
        Err(ImportError::UnknownFormat)
    ));
}

#[test]
fn every_score_of_the_corpus_is_written_in_every_format_and_read_again() {
    let found = files(&corpus(), &["gp3", "gp4", "gp5", "gpx", "gp"]);
    for path in &found {
        let name = path.strip_prefix(corpus()).unwrap().display().to_string();
        let score = import(&name, &std::fs::read(path).unwrap()).unwrap().score;
        let had = notes(&score);
        for format in Format::ALL {
            let written = export(&score, format).unwrap_or_else(|e| panic!("{name}: {e}"));
            common::keep(
                &format!("{}.{}", name.replace(['/', '\\'], "_"), format.extension()),
                &written.bytes,
            );
            let file = format!("out.{}", format.extension());
            match import(&file, &written.bytes) {
                Ok(read) => read.score.validate().unwrap(),
                // A score with nothing to be heard is no MIDI file, and one with nothing
                // on strings no tablature.
                Err(ImportError::Corrupt(_)) if format == Format::Midi && had == 0 => {}
                Err(ImportError::UnknownFormat) if format == Format::Ascii => {}
                Err(error) => panic!("{name} as {format:?}: {error}"),
            }
        }
    }
}
