# gtrTab

A guitar tablature editor for desktop and web. See [plan.md](plan.md) for the roadmap.

Scores are kept in gtrTab's own format. Guitar Pro files of every version, MusicXML, MIDI and tabs typed as plain text open through File > Open; File > Export writes Guitar Pro (`.gp`), MusicXML, MIDI, text, PDF, PNG, SVG and WAV.

For practice, Score has the loop, the speed trainer and the playback speed; View has the fretboard, keyboard or drum kit under the score and the visual metronome; and Tools has the tuner, the chord and scale libraries and the scale finder.

For the sound, View > Pedalboard puts each track, and the whole mix, through a chain of pedals: compressor, wah, overdrive, distortion, amp, cabinet, EQ, chorus, delay and reverb, from presets or set by hand. A track can have other sounds, each an instrument's sound through pedals of its own, which Beat > Change of sound switches to part-way through; Tools > Line in plays a guitar through the pedals as it is played.

## Requirements

- Rust (stable) with the `wasm32-unknown-unknown` target
- Node.js 22 or later

## Development

Install dependencies once:

```sh
cd app/ui && npm install
cd ../desktop && npm install
```

| Task | Command | Run from |
|---|---|---|
| Core tests, and the TypeScript types generated from Rust | `cargo test` | repo root |
| Lint | `cargo clippy --all-targets -- -D warnings` | repo root |
| Web app (dev server) | `npm run dev` | `app/ui` |
| Web app (production build) | `npm run build` | `app/ui` |
| Type-check the UI | `npm run check` | `app/ui` |
| Unit tests of the UI's plain functions | `npm test` | `app/ui` |
| End-to-end tests, of the production build | `npm run build`, then `npm run e2e` | `app/ui` |
| Desktop app (dev) | `npm run dev` | `app/desktop` |
| Desktop app (installers) | `npm run build` | `app/desktop` |

The end-to-end tests need Playwright's browser once: `npx playwright install chromium` in `app/ui`. The notation is also tested against pictures in `tests/golden`; when a change to the engraving is meant, look at the new pictures in `target/golden`, then run `GTR_BLESS=1 cargo test -p gtr-render` and commit them.

To see what an importer and the layout make of a file without opening the app, draw it: `cargo run -p gtr-render --example draw -- song.gp5 song.png` writes a PNG, a PDF or an SVG by the name it is given, and takes `tab`, `standard`, `both` or `slash` and `all` for the view.

Playback sounds through the GeneralUser GS SoundFont, which is too big for the repository. Fetch it once with `npm run soundfont` in `app/ui`; without it the app plays through its built-in models.

The interface's types for the score, the editor's state and the laid-out page are generated from the Rust ones into `app/ui/src/lib/generated` whenever the tests run. Commit them with the change that altered them; CI fails if they are out of date.

The UI scripts rebuild the WASM core into `app/ui/src/wasm` before starting: the editor through `wasm-pack`, and the synth for the audio thread (`crates/gtr-worklet`) with a plain `cargo build`.

## Keyboard

| Key | Action |
|---|---|
| Arrow keys | Move between beats and strings; Shift extends the selection |
| Ctrl+Left / Ctrl+Right | Previous / next bar |
| Ctrl+Home / Ctrl+End | First / last bar |
| Ctrl+Up / Ctrl+Down | Previous / next track, or staff of a piano |
| Alt+1 to Alt+4 | Voice 1 to 4 |
| 0-9 | Enter a fret; two digits typed quickly make one number |
| + / - | Shorter / longer note value |
| . | Dotted |
| / | Triplet |
| R | Rest |
| L | Tie |
| H | Hammer-on or pull-off to the next note |
| S | Shift slide to the next note |
| B | Full bend |
| Shift+B | Bend editor |
| W | Whammy bar editor |
| < / > | Crescendo / decrescendo |
| V | Vibrato |
| P | Palm mute |
| I | Let ring |
| X | Dead note |
| G | Ghost note |
| A | Accent |
| N | Natural harmonic |
| T | Tapping |
| Delete | Remove the note, or the beat if it is a rest |
| Ctrl+Delete | Remove the beat or the selected beats |
| Insert | Insert a rest before the cursor |
| Ctrl+Insert | Insert a bar |
| Ctrl+C / Ctrl+X / Ctrl+V | Copy, cut, paste |
| Ctrl+Z / Ctrl+Y | Undo, redo |
| Space | Play from the cursor's bar, or the selected bars; stop |
| [ / ] | Play slower / faster, also while playing |
| Ctrl+N / Ctrl+O / Ctrl+S / Ctrl+Shift+S | New, open or import, save, save as |
| Ctrl+P | Print |
| ? or F1 | List every shortcut |
| Tab / Shift+Tab | Move between the bars, the palette, the score and the inspector |
| Arrow keys in a bar, the palette or a menu | Move between its controls |
| In the track pane: Left / Right | Along a track's row: name, mute, solo, volume, pan |
| In the track pane: Up / Down on a slider | Set the volume or pan |
| In the track pane: Alt+Up / Alt+Down on a name | Move the track up or down |
| Esc | Back to the score |

Technique keys act on the note under the cursor, or on every selected note; palm mute, let ring and accents cover the whole chord.

## Interface

- **Bars** along the top: menus, file and edit buttons, then playback, zoom, panel switches and the theme switch. They share one row in a wide window and stack in a narrow one.
- **Palette** on the left: what is used while writing, grouped into note values, techniques on the note, techniques on the beat, and bars and score. Each group folds away, and the palette narrows to two columns in a small window or from the View menu.
- **Menus** hold everything, including the less common techniques: other slides and harmonics, tuplets, arpeggios, rasgueado, slap and pop, and fingering.
- **Track pane** under the score: a row per track with its name, mute, solo, volume and pan, and a grid with a cell for every bar, filled where the track has notes. Click a name to edit that track, a cell to go to that bar, and double-click a name for the track's settings: instrument, tuning, capo and mix. The mixer works while the score is playing. The buttons above the names add, set up, move and remove tracks. Drag the pane's top edge to resize it; the View menu hides it.
- **Views** (View menu): each track as notation and tab together, tab only, notation only, or rhythm slashes; and every track at once or only the one being edited. Notes are entered on the tab and the notation follows.
- **Inspector** on the right: everything set on the beat and note under the cursor, each with a button to remove it and, for bends, the whammy bar, trills and grace notes, one to edit it. Fingering is set here too. It opens by default when the window is wide enough to show it beside a full page.
- Every button has a tooltip with its name and shortcut. The theme follows the system until it is switched to light or dark.

Repeats, alternate endings, segno, coda and fine, section names, tempo changes, timers and swing are set on the cursor's bar from the Score menu, and playback follows them. Chord names and diagrams, text, lyrics, fermatas and changes of volume or pan are set on the cursor's beat from the Beat menu. The chord dialog finds ways to finger a chord in the track's tuning.

On a drum track a note's fret is the number of a piece of the kit, such as 36 for the bass drum, 38 for the snare and 42 for the closed hi-hat; Note > Drum piece lists them by name. A piano track has a staff for each hand, and its notes are entered on tab lines like any other track's.

Score > Playback speed plays the score at anything from a quarter of its tempo to twice it, without changing the pitch; the speed in the transport bar shows it and asks for another. File > Export audio (WAV)… writes the score to a sound file as it plays, at the tempo written.

Moving right past the end of a full bar starts a new one. Guitar Pro files of every version (`.gp3`, `.gp4`, `.gp5`, `.gpx`, `.gp`) open through Open or by dropping them on the window.

## Layout

- `crates/` — Rust core: score model, editor, notation and tab layout, playback, file formats and the WASM API.
- `app/ui/` — Svelte UI shared by the web and desktop apps.
- `app/desktop/` — Tauri shell.

## Third-party assets

- `app/ui/public/fonts/Bravura.woff2`: the Bravura music font by Steinberg, under the SIL Open Font License (`Bravura-OFL.txt` alongside it).
- Interface icons from [Lucide](https://lucide.dev) (`@lucide/svelte`), under the ISC License. Music symbols in the interface are Bravura glyphs; the guitar technique icons are drawn in `app/ui/src/lib/ui/icons.ts`.
- `tests/corpus/alphatab/`: Guitar Pro files of every version from the test data of [alphaTab](https://github.com/CoderLine/alphaTab), under the Mozilla Public License 2.0 (`LICENSE` alongside them; `tests/corpus/README.md` says which and from where). They are used by the tests only.
