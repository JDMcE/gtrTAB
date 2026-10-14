# Corpus

Files in other programs' formats that the importers are tested against. Only files we are
free to redistribute belong here, each directory with a note of where its files came from
and the licence they are under.

## `alphatab/`

Guitar Pro files from the test data of [alphaTab](https://github.com/CoderLine/alphaTab)
(`packages/alphatab/test-data`, at commit `aa824f269e2f12339c6254dc16d6aff911c0f075`), copyright Daniel Kuschny and
contributors, under the Mozilla Public License 2.0; see `alphatab/LICENSE`. They are
unchanged and keep alphaTab's directories, one for each version of the format:

| Directory | Files | Version |
|---|---|---|
| `guitarpro3/` | `.gp3` | 3 |
| `guitarpro4/` | `.gp4` | 4 |
| `guitarpro5/` | `.gp5` | 5 |
| `guitarpro6/` | `.gpx` | 6 |
| `guitarpro7/`, `guitarpro8/` | `.gp` | 7 and 8 |
| `visual-tests/` | `.gp` | three more with repeats, endings and drums |

Each file is a few bars made to show one thing, and many are the same few bars saved by
each version, under the same name in each directory.

Eleven of alphaTab's files are left out: they are transcriptions of other people's songs,
or carry their words, which alphaTab's licence does not make ours to pass on. They are
`canon`, `chord-name-overflow`, `nightwish`, `serenade` and `beat-text-lyrics` (`.gp5`),
`serenade` (`.gpx` and `.gp`), `fade-to-black` (`.gp4`), and `canon-audio-track`, `faulty`
and `orphan-tempo-automation` (`.gp`). Fetched into a directory of your own they are a
good test of whole pieces: name it in `GTR_GP_SAMPLES` and run the tests.

`crates/gtr-io/tests/gp_samples.rs` imports every file here, checks that the versions of
one piece read alike, and that a damaged file is never more than refused.
`crates/gtr-io/tests/corpus.rs` says what particular files should import as, and
`gp_export.rs` writes each out as a `.gp` file and reads it back.

alphaTab's MusicXML files are here too, from the same commit:

| Directory | Files | What they are |
|---|---|---|
| `musicxml3/`, `musicxml4/` | `.musicxml`, `.xml`, `.mxl` | alphaTab's own, under the MPL like its Guitar Pro files: tablature, bends, chord diagrams, percussion, and the same bars written partwise and timewise |
| `musicxml-testsuite/` | `.xml`, `.mxl` | the unofficial MusicXML test suite, copyright Reinhold Kainhofer, under the MIT licence in `musicxml-testsuite/LICENSE`: a file or several for every corner of the format |

Left out are alphaTab's `musicxml-samples`, which are Recordare's encodings of whole
pieces and theirs to pass on, not ours. Fetched into a directory of your own, they are
read by the tests when `GTR_MUSICXML_SAMPLES` names it. `crates/gtr-io/tests/musicxml.rs`
reads every file, writes it out again and reads that.

## `mutopia/`

Five pieces for the guitar from the [Mutopia Project](https://www.mutopiaproject.org/),
as MIDI files written by LilyPond. Each is in the public domain, the music and the
typesetting both, as its page there says:

| File | Piece | From |
|---|---|---|
| `anna-magdalena-04-guitar-tab.mid` | J. S. Bach, Menuet in G, BWV Anh. 114 | `ftp/BachJS/BWVAnh114/anna-magdalena-04-guitar-tab/` |
| `Bach_Prelude_BWV999.mid` | J. S. Bach, Prelude in D minor, BWV 999 | `ftp/BachJS/BWV999/Bach_Prelude_BWV999/` |
| `carcassi-op60-01.mid` | M. Carcassi, Etude 1, Op. 60 | `ftp/CarcassiM/O60/carcassi-op60-01/` |
| `saltarello.mid` | V. Galilei, Saltarello | `ftp/GalileiV/saltarello/` |
| `wtk1-prelude1-guitar-duo.mid` | J. S. Bach, Praeludium I, BWV 846, for two guitars | `ftp/BachJS/BWV846/wtk1-prelude1-guitar-duo/` |

`crates/gtr-io/tests/midi.rs` reads them.

## `ascii/`

Tablature typed as plain text, the way it is passed around: without note lengths, with
the signs for techniques typed tight against the frets, as drum tab and as bass tab.
They were typed for this corpus, the tune of one being Beethoven's and long out of
copyright, and are ours to give away. `crates/gtr-io/tests/ascii.rs` says what each
should read as.

## `reference/`

What another program makes of the corpus, to hold the importers to. `alphatab.json` is
alphaTab's reading of every Guitar Pro and MusicXML file here: for each track and bar,
the pitches of its notes. `alphatab.mjs` wrote it and says how to write it again.
`crates/gtr-io/tests/reference.rs` compares, and lists the files the two read
differently with the reason for each.

Every file of every format is also drawn, in `crates/gtr-render/tests/corpus.rs`, and a
few are compared with the pictures of them in `tests/golden/corpus`.
