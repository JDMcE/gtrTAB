# Corpus

Files in other programs' formats that the importers are tested against. Only files we are
free to redistribute belong here, each directory with a note of where its files came from
and the licence they are under.

## `alphatab/`

Guitar Pro files from the test data of [alphaTab](https://github.com/CoderLine/alphaTab)
(`packages/alphatab/test-data`, at commit `aa824f269e2f12339c6254dc16d6aff911c0f075`), copyright Daniel Kuschny and
contributors, under the Mozilla Public License 2.0; see `alphatab/LICENSE`. They are
unchanged and keep alphaTab's directories, one for each version of Guitar Pro:

| Directory | Files | Version |
|---|---|---|
| `guitarpro3/` | `.gp3` | Guitar Pro 3 |
| `guitarpro4/` | `.gp4` | Guitar Pro 4 |
| `guitarpro5/` | `.gp5` | Guitar Pro 5 |
| `guitarpro6/` | `.gpx` | Guitar Pro 6 |
| `guitarpro7/`, `guitarpro8/` | `.gp` | Guitar Pro 7 and 8 |
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
`crates/gtr-io/tests/corpus.rs` says what particular files should import as.
