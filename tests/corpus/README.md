# Corpus

Files in other programs' formats that the importers are tested against. Only files we are
free to redistribute belong here, each directory with a note of where its files came from
and the licence they are under.

## `alphatab/`

Guitar Pro files from the test data of [alphaTab](https://github.com/CoderLine/alphaTab)
(`packages/alphatab/test-data`, at commit `aa824f269e2f12339c6254dc16d6aff911c0f075`), copyright Daniel Kuschny and
contributors, under the Mozilla Public License 2.0; see `alphatab/LICENSE`. They are
unchanged, apart from `directions-gp8.gp`, which is alphaTab's `guitarpro8/directions.gp`
renamed so as not to be taken for the same piece as `directions.gp5`. Each is a few bars
made to show one feature:

| File | Shows |
|---|---|
| `repeat-close.gp`, `.gp5` | A repeat played twice |
| `repeat-close-multi.gp`, `.gp5` | A repeat played four times |
| `repeat-close-without-start-at-beginning.gp5` | A repeat with no sign to open it |
| `repeat-close-alternate-endings.gp`, `.gp5` | Alternate endings inside a repeat |
| `repeats.gp`, `alternate-endings.gp` | More of both, in bars without notes |
| `directions.gp5` | Every sign and jump Guitar Pro 5 has, one to a bar |
| `directions-gp8.gp` | The same from Guitar Pro 8, several to a bar |
| `percussion-all.gp5` | A drum track playing every General MIDI percussion number |
| `drum-tabs.gp`, `drum-custom-lines.gp`, `percussion.gp` | Drum kits of Guitar Pro 7 and 8 |

`crates/gtr-io/tests/corpus.rs` says what each should import as.
