# gtrTab

A guitar tablature editor for desktop and web. See [plan.md](plan.md) for the roadmap.

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
| Core tests | `cargo test` | repo root |
| Lint | `cargo clippy --all-targets -- -D warnings` | repo root |
| Web app (dev server) | `npm run dev` | `app/ui` |
| Web app (production build) | `npm run build` | `app/ui` |
| Type-check the UI | `npm run check` | `app/ui` |
| Desktop app (dev) | `npm run dev` | `app/desktop` |
| Desktop app (installers) | `npm run build` | `app/desktop` |

The UI scripts rebuild the WASM core into `app/ui/src/wasm` before starting.

## Keyboard

| Key | Action |
|---|---|
| Arrow keys | Move between beats and strings; Shift extends the selection |
| Ctrl+Left / Ctrl+Right | Previous / next bar |
| Ctrl+Home / Ctrl+End | First / last bar |
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
| Ctrl+N / Ctrl+O / Ctrl+S / Ctrl+Shift+S | New, open or import, save, save as |
| ? or F1 | List every shortcut |
| Tab / Shift+Tab | Move between the bars, the palette, the score and the inspector |
| Arrow keys in a bar, the palette or a menu | Move between its controls |
| Esc | Back to the score |

Technique keys act on the note under the cursor, or on every selected note; palm mute, let ring and accents cover the whole chord.

## Interface

- **Bars** along the top: menus, file and edit buttons, then playback, zoom, panel switches and the theme switch. They share one row in a wide window and stack in a narrow one.
- **Palette** on the left: what is used while writing, grouped into note values, techniques on the note, techniques on the beat, and bars and score. Each group folds away, and the palette narrows to two columns in a small window or from the View menu.
- **Menus** hold everything, including the less common techniques: other slides and harmonics, tuplets, arpeggios, rasgueado, slap and pop, and fingering.
- **Inspector** on the right: everything set on the beat and note under the cursor, each with a button to remove it and, for bends, the whammy bar, trills and grace notes, one to edit it. Fingering is set here too. It opens by default when the window is wide enough to show it beside a full page.
- Every button has a tooltip with its name and shortcut. The theme follows the system until it is switched to light or dark.

Moving right past the end of a full bar starts a new one. Guitar Pro files (`.gp`, `.gp5`) open through Open or by dropping them on the window.

## Layout

- `crates/` — Rust core: score model, editor, tab layout, playback, file formats and the WASM API.
- `app/ui/` — Svelte UI shared by the web and desktop apps.
- `app/desktop/` — Tauri shell.

## Third-party assets

- `app/ui/public/fonts/Bravura.woff2`: the Bravura music font by Steinberg, under the SIL Open Font License (`Bravura-OFL.txt` alongside it).
- Interface icons from [Lucide](https://lucide.dev) (`@lucide/svelte`), under the ISC License. Music symbols in the interface are Bravura glyphs; the guitar technique icons are drawn in `app/ui/src/lib/ui/icons.ts`.
