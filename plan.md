# gtrTab — Plan

A guitar tablature editor aiming for feature parity with Guitar Pro 8, shipped as a standalone desktop app and a web version from one codebase.

## Decisions

| # | Decision | Outcome |
|---|---|---|
| 1 | Stack | Rust core + Svelte/TypeScript UI + Tauri 2 |
| 2 | Licence | Will be open source eventually. Only bundle assets with open licences (e.g. Bravura under OFL, freely licensed SoundFonts). Specific licence still to be chosen |
| 3 | Guitar Pro import | Simple read-only importer in phase 2, full import and export in phase 6 |

## Stack

| Layer | Choice | Why |
|---|---|---|
| Core (score model, editing, layout, file I/O, synth) | Rust workspace | One codebase runs natively and as WASM; fast enough for layout and real-time audio |
| UI (toolbars, panels, mixer, dialogs) | TypeScript + Svelte | Same UI on desktop and web |
| Desktop shell | Tauri 2 | Small installers, native file dialogs, Rust backend in-process |
| Score drawing | Rust layout engine emits a display list, drawn to Canvas | The same display list feeds PDF/SVG/PNG export |
| Music glyphs | SMuFL font (Bravura) | Open standard, full notation coverage |
| Audio | Rust SoundFont synth + effects in an AudioWorklet | One audio path for web and desktop; native backend later if latency demands it |

## Repository layout

```
crates/
  gtr-model/     score data model, rational durations, tunings
  gtr-edit/      commands, undo/redo, selection and cursor logic
  gtr-layout/    engraving engine -> display list
  gtr-render/    display list -> SVG / PDF / PNG
  gtr-audio/     score -> event stream, SoundFont synth, effects
  gtr-io/        native format, Guitar Pro, MusicXML, MIDI, ASCII
  gtr-wasm/      wasm-bindgen API over the crates above
app/
  ui/            Svelte app (shared by web and desktop)
  desktop/       Tauri shell
tests/
  corpus/        sample scores for import and rendering tests
  golden/        reference images for rendering regression tests
```

## Architecture

- **Score model:** Score → Tracks → Staves → Bars → Voices → Beats → Notes. Bar-level properties shared across tracks (time signature, key, repeats, tempo, sections) live in a separate master-bar list. Durations are exact rationals, never floats.
- **Editing:** every edit is a command object with apply and revert. This gives undo/redo, makes the editor testable without a UI, and leaves room for collaboration later.
- **Layout:** the engraving engine takes the model plus a stylesheet and produces a display list of positioned glyphs, lines and paths. It handles horizontal spacing, beaming, stem direction, line and page breaking, and placement of effects above and below the staff. This is the hardest part of the project.
- **Rendering:** the UI draws the display list to Canvas and only redraws bars that changed. Export backends consume the same display list.
- **Playback:** the score compiles to a timed event stream. Techniques become synth events: bends and whammy as pitch curves, slides as pitch ramps, vibrato as modulation, palm mute and dead notes as sample or envelope changes, let-ring as extended note-offs.
- **Audio threading:** the synth runs in an AudioWorklet as WASM. The UI thread sends events and reads the playback position; it never touches audio buffers.
- **Native file format:** zipped JSON with a schema version and migrations from the first release.

## Phases

Each phase ends with something usable. Phases 1–3 make a useful riff-writing tool; phase 4 tidies the interface they left behind; later phases are the long tail towards parity.

### Phase 1 — Foundations

- [x] Cargo workspace and crate skeletons
- [x] Score model with rational durations and tunings
- [x] Command system with undo/redo
- [x] Native file format (`.gtrtab`): save, load, version field
- [x] WASM build and TypeScript bindings (score and command types in `app/ui/src/lib/model.ts` are hand-written; generate them from Rust later)
- [x] Svelte app running in the browser and inside Tauri
- [ ] CI: build, test, lint for Rust and TypeScript on Windows, macOS, Linux (workflow written in `.github/workflows/ci.yml`, not yet run on GitHub)
- [x] Check open and save through the native file dialogs in the desktop app (automated once through Windows UI Automation; not part of the test suite)

**Done when:** a hard-coded score round-trips through save and load on web and desktop.

### Phase 2 — Tab editor MVP

- [x] Layout: single tab staff, bar lines, time signature, rhythm stems and beams, line breaking
- [x] Canvas renderer with scrolling and zoom
- [x] Cursor movement across strings, beats and bars
- [x] Keyboard note entry: fret numbers, durations, dots, rests, ties
- [x] Insert, delete, copy, cut and paste of beats and bars (cut empties bars rather than removing them, so other tracks keep theirs)
- [x] Time signature, tempo and tuning dialogs, including capo and 6/7/8-string guitars and 4/5-string bass
- [x] Bar validation: highlight incomplete and overfull bars
- [x] Playback with a moving cursor, from the cursor's bar or over the selected bars
- [ ] SoundFont synth. Playback currently uses a built-in plucked-string model with no sample files, and renders the passage to a buffer up front instead of streaming from an AudioWorklet. Revisit when other instruments arrive in phase 5
- [x] Metronome and count-in
- [x] File open, save, recent files (desktop only), autosave with recovery on start
- [x] Simple read-only Guitar Pro import (`.gp` and `.gp5`): notes, rests, ties, rhythms, tunings, starting tempo and time signatures for all tracks, with one track shown at a time. Anything else is skipped and listed in the status bar after import

Still to check by hand: how playback sounds, autosave recovery, recent files, and drag-and-drop and the unsaved-changes prompt in the desktop app.

**Done when:** a riff can be written, played back, saved and reopened without touching the mouse, and an existing Guitar Pro file opens with its notes and rhythms intact.

### Phase 3 — Guitar techniques

Each item needs notation, input and playback.

- [x] Hammer-on, pull-off, legato slides, shift slides, slide in and out
- [x] Bends with a bend editor: presets (bend, release, pre-bend, pre-bend and release) and custom curves
- [x] Vibrato (slight, wide) and whammy bar with a curve editor
- [x] Palm mute, let ring, dead notes, ghost notes, staccato
- [x] Accents, heavy accents, dynamics, crescendo and decrescendo
- [x] Natural, artificial, pinch, tapped and semi harmonics
- [x] Tapping, slapping, popping, pick strokes, strum directions, arpeggios, rasgueado
- [x] Tremolo picking, trills, grace notes
- [x] Tuplets, including nested tuplets
- [x] Fingering for both hands
- [x] Guitar Pro import brings in all of the above

Known simplifications, to revisit:

- A custom bend is drawn as the nearest common shape, labelled with its highest point; playback follows the real curve.
- Only natural harmonics play at their true pitch. The other kinds sound an octave above the fretted note.
- Tapping, pick strokes and fingering are notation only.
- A grace note is a single note on the same string as the note it decorates.
- Rasgueado plays as a slow strum.

**Done when:** a typical rock or metal lead part can be transcribed with every technique heard in playback.

### Phase 4 — Interface

The toolbar grew one text button at a time through phases 2 and 3 and is now four crowded rows. This phase redesigns the interface before phase 5 adds the track list, mixer and notation options, so that those arrive into a layout with room for them.

- [x] Layout: a slim top bar for file and edit actions, a transport bar, a tool palette beside the score, and a status bar. The score keeps as much of the window as possible (the two bars share one row when the window is wide enough)
- [x] Palette grouped by purpose (note values, techniques on the note, techniques on the beat, bars and score), with sections that collapse
- [x] Icons in place of text labels: music symbols from Bravura for note values, rests, dynamics and articulations; Lucide (ISC) for general actions; guitar technique icons drawn to match in `app/ui/src/lib/ui/icons.ts`
- [x] Tooltips on every control giving its name and shortcut
- [x] Inspector panel showing everything set on the note and beat under the cursor, replacing the dropdowns that only exist to show state
- [x] Menus for the less common actions, so the palette holds only what is used while writing
- [x] Keyboard shortcut overlay listing every key (`?` or F1)
- [x] Design tokens for colour, spacing and type in `app/ui/src/app.css`, applied to the bars, palette, dialogs and score; light and dark themes with a manual switch
- [x] Dialogs restyled to match, and the curve editor made draggable
- [x] Keyboard access and visible focus for every control; contrast checked in both themes (text and icons at 4.5:1 or better, control outlines and the focus ring at 3:1 or better)
- [x] Narrow windows: the palette collapses to two columns and the bars stack
- [x] Space reserved for the track list and mixer that phase 5 adds (an empty dock under the score)
- [x] Checked in the desktop window as well as in browsers, since the two have differed before (layout, note entry, the bend editor, the inspector and both themes looked the same in each)

Still to check by hand: the desktop title bar stayed dark when the app was switched to the light theme on Windows, so `setTheme` on the window may not be taking effect; and the interface has not been tried with a screen reader.

Every command (name, icon, shortcut, action) is defined once in `App.svelte` and the bars, palette, menus, tooltips and shortcut list are built from that set. Add new features there.

Score area at the same window size, before and after: 1280×618 → 1131×736 at 1280×800; 1024×601 → 875×706 at 1024×800, where the bars stack; 640×129 → 555×306 at 640×400. The old toolbar took 153 to 242 pixels of height; the new bars take 39 to 69. The inspector takes another 216 pixels of width when open, so it only opens by default from 1480 pixels, where a full-width page still fits beside it.

Customisable shortcuts, the command palette and further themes stay in phase 9.

**Done when:** every phase 2 and 3 feature is reachable from the new interface without a text-only button, and the score has more room than it does now at the same window size.

### Phase 5 — Full scores

- [ ] Standard notation staff linked to tab: clefs, key signatures, accidentals, stems, beams, ledger lines
- [ ] Tab-only, notation-only, combined and slash notation views
- [ ] Multiple tracks; single-track and multitrack views
- [ ] Track pane along the bottom of the window, in the dock phase 4 left under the score, laid out like Guitar Pro's: one row per track with its instrument icon and name on the left, and to the right a grid with one cell per bar, filled where the track has notes. Clicking a row switches track and clicking a cell jumps to that bar; the current bar and the playback position are marked; the grid scrolls with the score. Adding, removing, renaming and reordering tracks happens here. The pane can be resized and hidden, and replaces the track dropdown in the transport bar
- [ ] Up to four voices per staff
- [ ] Drum tracks: percussion notation, drum tab and a drum kit map
- [ ] Bass, keyboard (grand staff), ukulele, banjo and other fretted instruments
- [ ] Repeats, alternate endings, directions (coda, segno, D.S., D.C., fine)
- [ ] Tempo, volume and pan automation; fermatas; swing and triplet feel
- [ ] Sections, rehearsal marks, free text, timers
- [ ] Lyrics with multiple lines
- [ ] Chord diagrams and names with a chord builder
- [ ] Mixer: volume, pan, mute, solo, instrument selection per track, with the mute, solo and volume controls on each row of the track pane
- [ ] Score information and page headers

**Done when:** a full band arrangement can be written and played back.

### Phase 6 — Import and export

- [ ] Full Guitar Pro import, extending the phase 2 importer: `.gp3`, `.gp4`, `.gp5`, `.gpx`, `.gp`, covering every technique and score element from phases 3 and 5
- [ ] Guitar Pro export: `.gp`
- [ ] MusicXML import and export
- [ ] MIDI import and export
- [ ] ASCII tab import and export
- [ ] PDF, SVG and PNG export; printing
- [ ] Audio export: WAV, MP3, FLAC, OGG
- [ ] Import test corpus with rendering comparisons against reference output

**Done when:** the import corpus opens without errors and renders recognisably.

### Phase 7 — Practice tools

- [ ] Loop a selection, with optional tempo increase per pass (speed trainer)
- [ ] Relative playback speed
- [ ] Visual metronome
- [ ] Tuner using microphone or line-in
- [ ] Fretboard, keyboard and drum kit views that follow playback and accept input
- [ ] Chord library and scale library with fretboard diagrams
- [ ] Scale finder from a selection

### Phase 8 — Sound

- [ ] Per-track effects chain: amp simulation, cabinet, overdrive, distortion, compressor, EQ, chorus, delay, reverb, wah
- [ ] Pedalboard view and presets
- [ ] Per-track sound banks with switching mid-score via automation
- [ ] Master track effects
- [ ] Audio track: import an audio file and sync the score to it with tempo mapping
- [ ] Line-in: play a real guitar through the track's effects
- [ ] Native audio backend on desktop if AudioWorklet latency is not good enough

### Phase 9 — Polish and release

- [ ] Stylesheets: fonts, spacing, symbol sizes, notation conventions
- [ ] Design mode: manual bar widths, line breaks, system layout
- [ ] Page and screen layouts: page, parchment, horizontal and vertical scroll
- [ ] Command palette
- [ ] Customisable keyboard shortcuts
- [ ] Batch file conversion
- [ ] Preferences, themes, localisation scaffolding
- [ ] Signed installers and auto-update for Windows, macOS, Linux
- [ ] Offline-capable web app with local file access

## Testing

- **Model and editing:** unit tests plus property tests that apply random command sequences and check undo returns the original score.
- **Layout and rendering:** golden-image tests per notation feature; a diff means review, not automatic failure.
- **File formats:** round-trip tests for the native format; a corpus of real files for each import format. No Guitar Pro files are in the repository yet: `crates/gtr-io/tests/gp_samples.rs` runs against a directory named by `GTR_GP_SAMPLES`. Add files we are free to redistribute to `tests/corpus/`.
- **Audio:** snapshot tests on the event stream rather than on audio samples.
- **UI:** end-to-end tests for the core entry workflow.

## Risks

- **Scope:** Guitar Pro 8 is years of work by a commercial team. The phase order front-loads the parts needed for writing riffs and songs.
- **Engraving quality:** spacing and collision avoidance are open-ended. Budget for ongoing refinement rather than a one-off implementation.
- **Sound quality:** Guitar Pro's sound comes from proprietary sample banks. Feature parity is achievable; tonal parity depends on the freely licensed SoundFonts available.
- **Web audio latency:** fine for playback, possibly not for line-in monitoring. Phase 8 has a native fallback for desktop.
- **File format coverage:** Guitar Pro formats are undocumented and have many edge cases. Coverage will improve with the corpus over time.

## Reference

- alphaTab: open-source library that renders and plays Guitar Pro files. Viewer, not editor. Useful for file format details and rendering comparison.
- SMuFL specification and the Bravura font.
- MusicXML 4.0 specification.
