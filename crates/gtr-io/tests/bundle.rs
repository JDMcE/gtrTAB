//! Many files put in one archive, as batch conversion downloads them.

use std::io::{Cursor, Read};

#[test]
fn bundled_files_come_out_under_their_names_as_they_went_in() {
    let text = "e|--0--3--5--|\n".repeat(200);
    let picture = [0x89, b'P', b'N', b'G', 1, 2, 3];
    let files: [(&str, &[u8]); 3] = [
        ("riff.txt", text.as_bytes()),
        ("riff.png", &picture),
        ("Solo – take 2.musicxml", b"<score-partwise/>"),
    ];
    let bytes = gtr_io::bundle(files).unwrap();

    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    assert_eq!(archive.len(), 3);
    for (name, contents) in files {
        let mut entry = archive.by_name(name).unwrap();
        // Text is made smaller; a picture, compressed already, is kept as it is.
        let method = entry.compression();
        assert_eq!(
            method == zip::CompressionMethod::Stored,
            name.ends_with(".png"),
            "{name} was {method:?}"
        );
        let mut read = Vec::new();
        entry.read_to_end(&mut read).unwrap();
        assert_eq!(read, contents);
    }
}

#[test]
fn nothing_bundled_is_an_empty_archive() {
    let bytes = gtr_io::bundle([]).unwrap();
    assert_eq!(zip::ZipArchive::new(Cursor::new(bytes)).unwrap().len(), 0);
}
