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
| Delete | Remove the note, or the beat if it is a rest |
| Ctrl+Delete | Remove the beat or the selected beats |
| Insert | Insert a rest before the cursor |
| Ctrl+Insert | Insert a bar |
| Ctrl+C / Ctrl+X / Ctrl+V | Copy, cut, paste |
| Ctrl+Z / Ctrl+Y | Undo, redo |
| Space | Play from the cursor's bar, or the selected bars; stop |
| Ctrl+N / Ctrl+O / Ctrl+S / Ctrl+Shift+S | New, open or import, save, save as |

Moving right past the end of a full bar starts a new one. Guitar Pro files (`.gp`, `.gp5`) open through Open or by dropping them on the window.

## Layout

- `crates/` — Rust core: score model, editor, tab layout, playback, file formats and the WASM API.
- `app/ui/` — Svelte UI shared by the web and desktop apps.
- `app/desktop/` — Tauri shell.

## Third-party assets

- `app/ui/public/fonts/Bravura.woff2`: the Bravura music font by Steinberg, under the SIL Open Font License (`Bravura-OFL.txt` alongside it).
