# gtrTab — Plan

A guitar tablature editor aiming for feature parity with Guitar Pro 8, shipped as a standalone desktop app and a web version from one codebase.

## Decisions

| # | Decision | Outcome |
|---|---|---|
| 1 | Stack | Rust core + Svelte/TypeScript UI + Tauri 2 |
| 2 | Licence | Will be open source eventually. Only bundle assets with open licences (e.g. Bravura under OFL, freely licensed SoundFonts). Specific licence still to be chosen |
| 3 | Guitar Pro import | Simple read-only importer in phase 2, full import and export in phase 7 |
| 4 | Simplifications | Every shortcut taken in a phase is written down under that phase with the phase that will remove it. Phase 6 exists to clear the ones that later phases would otherwise build on |

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
- **Layout code:** `gtr-layout` measures each bar across every staff and voice shown (`spacing.rs`), breaks the score into lines and stacks the staves (`score.rs`), and has each staff drawn by `tab.rs` or `notation.rs`.
- **Editing:** every edit is a command object with apply and revert. This gives undo/redo, makes the editor testable without a UI, and leaves room for collaboration later.
- **Layout:** the engraving engine takes the model plus a stylesheet and produces a display list of positioned glyphs, lines and paths. It handles horizontal spacing, beaming, stem direction, line and page breaking, and placement of effects above and below the staff. This is the hardest part of the project.
- **Rendering:** the UI draws the display list to Canvas and only redraws bars that changed. Export backends consume the same display list.
- **Playback:** the score compiles to a timed event stream. Techniques become synth events: bends and whammy as pitch curves, slides as pitch ramps, vibrato as modulation, palm mute and dead notes as sample or envelope changes, let-ring as extended note-offs.
- **Audio threading:** the synth runs in an AudioWorklet as WASM. The UI thread sends events and reads the playback position; it never touches audio buffers.
- **Native file format:** zipped JSON with a schema version and migrations from the first release.

## Phases

Each phase ends with something usable. Phases 1–3 make a useful riff-writing tool; phase 4 tidies the interface they left behind; phase 5 makes it a score editor; phase 6 clears the shortcuts those phases took; later phases are the long tail towards parity.

### Phase 1 — Foundations

- [x] Cargo workspace and crate skeletons
- [x] Score model with rational durations and tunings
- [x] Command system with undo/redo
- [x] Native file format (`.gtrtab`): save, load, version field
- [x] WASM build and TypeScript bindings (score and command types in `app/ui/src/lib/model.ts` are hand-written; generating them from Rust is in phase 6)
- [x] Svelte app running in the browser and inside Tauri
- [x] CI: build, test, lint for Rust and TypeScript on Windows, macOS, Linux (`.github/workflows/ci.yml`; the desktop shell compiles on all three)
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
- [ ] SoundFont synth. Playback uses built-in models with no sample files, and renders the passage to buffers up front instead of streaming from an AudioWorklet. This was to be revisited in phase 5 and was not; it is now the first item of phase 6
- [x] Metronome and count-in
- [x] File open, save, recent files (desktop only), autosave with recovery on start
- [x] Simple read-only Guitar Pro import (`.gp` and `.gp5`): notes, rests, ties, rhythms, tunings, starting tempo and time signatures for all tracks, with one track shown at a time. Anything else is skipped and listed in the status bar after import

Still to check by hand, in phase 6: how playback sounds, recent files, and drag-and-drop and the unsaved-changes prompt in the desktop app. (Autosave recovery was found broken during phase 5, where the saved copy was cleared before it could be read, and fixed.)

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

Known simplifications, all scheduled for phase 6:

- A custom bend is drawn as the nearest common shape, labelled with its highest point; playback follows the real curve.
- Only natural harmonics play at their true pitch. The other kinds sound an octave above the fretted note.
- Tapping is notation only. (Pick strokes and fingering are too, and stay that way: they have nothing to add to how a note sounds.)
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

Still to check by hand, in phase 6: the desktop title bar stayed dark when the app was switched to the light theme on Windows, so `setTheme` on the window may not be taking effect; and the interface has not been tried with a screen reader.

Every command (name, icon, shortcut, action) is defined once in `App.svelte` and the bars, palette, menus, tooltips and shortcut list are built from that set. Add new features there.

Score area at the same window size, before and after: 1280×618 → 1131×736 at 1280×800; 1024×601 → 875×706 at 1024×800, where the bars stack; 640×129 → 555×306 at 640×400. The old toolbar took 153 to 242 pixels of height; the new bars take 39 to 69. The inspector takes another 216 pixels of width when open, so it only opens by default from 1480 pixels, where a full-width page still fits beside it.

Customisable shortcuts, the command palette and further themes stay in phase 10.

**Done when:** every phase 2 and 3 feature is reachable from the new interface without a text-only button, and the score has more room than it does now at the same window size.

### Phase 5 — Full scores

- [x] Standard notation staff linked to tab: clefs, key signatures, accidentals, stems, beams, ledger lines. Notes are entered on the tab and the notation follows
- [x] Tab-only, notation-only, combined and slash notation views (View menu; combined is the default)
- [x] Multiple tracks; single-track and multitrack views. Every staff and voice shown shares one set of beat positions, so parts line up down the page
- [x] Track pane along the bottom of the window, in the dock phase 4 left under the score, laid out like Guitar Pro's: one row per track with its instrument icon and name on the left, and to the right a grid with one cell per bar, filled where the track has notes. Clicking a row switches track and clicking a cell jumps to that bar; the current bar and the playback position are marked; the grid scrolls with the score. Adding, removing, renaming and reordering tracks happens here. The pane can be resized and hidden, and replaces the track dropdown in the transport bar
- [x] Up to four voices per staff (Alt+1 to Alt+4). A bar gets a voice when something is first entered in it
- [x] Drum tracks: percussion notation, drum tab and a drum kit map. A note's fret is the General MIDI number of a piece of the kit (36 bass drum, 38 snare, 42 closed hi-hat), typed like a fret or chosen by name from Note > Drum piece; `gtr-model/src/drums.rs` says where each piece goes on the staff and which notehead it gets
- [x] Bass, keyboard (grand staff), ukulele, banjo and other fretted instruments. A piano has a staff for each hand, bracketed together; Ctrl+Up and Ctrl+Down move between staves as well as tracks
- [x] Repeats, alternate endings, directions (coda, segno, D.S., D.C., fine), followed in playback
- [x] Tempo, volume and pan automation; fermatas; swing and triplet feel. Tempo changes at a bar; volume and pan change at a beat (Beat menu) and are played back; swing eighths and sixteenths
- [x] Sections, rehearsal marks, free text, timers
- [x] Lyrics with multiple lines (entered beat by beat, up to five lines)
- [x] Chord diagrams and names with a chord builder: pick a root and a kind of chord and it offers the ways to finger it in the track's tuning, or click the frets yourself, or take the notes on the beat
- [x] Mixer: volume, pan, mute, solo, instrument selection per track, with the mute, solo and volume controls on each row of the track pane
- [x] Score information and page headers: subtitle, words, music, transcriber and copyright alongside title, artist and album

Known simplifications, each with the phase that removes it:

- Beams are level, never slanted, and accidentals are spelled by the key alone: sharps in sharp keys, flats in flat keys. (Phase 6)
- In the combined view the technique markings, chord names and diagrams sit above the tab staff, so between the two staves. The notation-only view shows the markings above the staff but not the ones written on strings (bends, slides). (Phase 6)
- A piano's notes are entered as tablature on six made-up strings per hand, since every note in the model is a string and a fret; there is no entry by pitch or from a keyboard. In the combined view each hand's tab sits under its notation, so the two notation staves are only adjacent in the notation-only view. Changing an existing track to a piano does not give it a second staff. (Phase 6)
- Drum notation uses one voice with stems as the notes fall; it does not split hands and feet into two voices by itself (use voice 2 for the feet). The kit is fixed: 23 General MIDI pieces. (Phase 6)
- A fermata is drawn but does not hold the note in playback, and a tempo change applies from the start of a bar, not from a beat inside it. (Phase 6)
- After a D.C. or D.S. the repeats are played again. Playing a selected range of bars ignores repeats and jumps. (Phase 6)
- A tempo or feel change applies to the bars after it on the page, also when a repeat or jump goes back past it. A timer shows the time at which playback first reaches its bar. (Phase 6)
- Once a passage has a volume or pan change written in it, the score has the say over that track's volume or pan for the passage, and the mixer's slider only sets where it starts. Mute and solo always work. (Phase 6)
- Instruments differ in tone only through built-in models: plucked strings (nylon, steel, clean, overdrive, distortion, bass, piano) and synthesised drums. Ukulele, banjo and mandolin borrow the nearest string tone. (Phase 6)
- Playback renders each track to its own buffer up front and mixes them as they play, which is what lets mute, solo, volume and pan change mid-playback. Memory therefore grows with tracks times length; edits to the notes only take effect on the next play. (Phase 6)
- The mixer sliders on the track rows are for the mouse; the keyboard reaches the same values through the track settings dialog. Tracks are reordered with the move up and move down buttons, not by dragging. (Phase 6)
- Clicking another staff in the multitrack view goes to the start of the bar clicked, not the beat. (Phase 6)
- The chord builder finds one shape per position with the root in the bass; it does not know slash chords or inversions, which can still be clicked in by hand. (Phase 6)
- Guitar Pro import skips lyrics, chord names and diagrams, tempo changes and `.gp` drum tracks. (Phase 7)

How things stand, for reference:

- Tuning and capo are part of a track's settings (the settings button in the track pane, or double-click the track's name), alongside its instrument and mix. Changing the kind of instrument there switches to that instrument's usual tuning.
- Guitar Pro import brings in instruments, volume and pan (`.gp5`), drum tracks (`.gp5`), key signatures, repeats, alternate endings, sections, swing feel, credits and, from `.gp`, jumps and free text.
- The native format is at version 4. The new fields are all optional, so older files open unchanged.

Still to check by hand, in phase 6: how each instrument and the drum kit sound, and the stereo mix; the track pane and the notation views in the desktop window and in a narrow window (only a wide browser window was tried); and Guitar Pro files with repeats, jumps or drum tracks, since none are in the repository.

**Done when:** a full band arrangement can be written and played back.

### Phase 6 — Consolidation

Phases 2 to 5 each took shortcuts to get something usable. This phase removes them before import and export, the practice tools and the sound work are built on top: those lean on playback timing, on the note model and on the engraving, and it is cheaper to put those right before files and features depend on how they behave now. Nothing here is a new feature.

Playback engine:

- [ ] Streaming synth: the synth runs in an AudioWorklet as WASM and plays from the event stream, as the architecture section describes, instead of rendering every track to a buffer first. Edits are heard without stopping, memory no longer grows with the length of the score, and looping, speed changes and audio export in later phases get the engine they need
- [ ] SoundFont playback: sampled instruments from a freely licensed SoundFont for every instrument and the drum kit, with the built-in models kept as the fallback when no SoundFont is loaded. Choosing the SoundFont, and checking its licence against decision 2, is part of this item
- [ ] Each instrument with its own sound: ukulele, banjo and mandolin stop borrowing a guitar's

Playback behaviour:

- [ ] A tempo map inside the bar: tempo changes on a beat, and fermatas that hold
- [ ] Tempo and feel follow the order bars are played in, so a repeat or jump back past a change restores what was in force there
- [ ] Repeats after a D.C. or D.S. are taken once through by default, as convention has it, with a setting to play them again; playing a selected range can follow its repeats
- [ ] Artificial, pinch, tapped and semi harmonics at their true pitch
- [ ] Rasgueado as the strokes it is made of; tapped notes sounded by the fretting hand
- [ ] Grace notes of several notes, and on other strings than the note they decorate
- [ ] Volume and pan written in the score and the mixer work together: the slider moves the whole passage up or down instead of being overridden

Notes and entry:

- [ ] Notes that are a pitch, not a string and a fret, for instruments without strings: the model, the commands, the native format (with a migration) and the importers
- [ ] Piano entry by pitch on the notation staff; no tab staff for a piano; the two staves adjacent in every view; changing a track to or from a piano adds or removes its second staff
- [ ] Drum notation that puts hands and feet in two voices by itself, and a kit map that can be edited
- [ ] TypeScript types generated from the Rust model instead of written by hand

Engraving:

- [ ] Slanted beams
- [ ] Accidentals spelled by context as well as by key: by the direction of the line and the chord the note belongs to
- [ ] In the combined view, chord names, diagrams, text and technique markings above the notation staff, not between the staves
- [ ] In the notation-only view, the markings that are written on strings: bends and slides
- [ ] A custom bend drawn as its own curve

Interface:

- [ ] Tracks reordered by dragging in the track pane
- [ ] Mixer sliders in the track pane that the keyboard can reach
- [ ] A click on another staff goes to the beat clicked
- [ ] Chord builder: slash chords, inversions, and more than one shape per position

Checks owed from earlier phases:

- [ ] Listen to every instrument, the drum kit and the stereo mix, and fix what sounds wrong
- [ ] The desktop app: track pane, notation views, recent files, drag-and-drop, the unsaved-changes prompt, and the title bar that stayed dark in the light theme on Windows
- [ ] A narrow window, with the track pane and the notation views
- [ ] A screen reader
- [ ] Guitar Pro files with repeats, jumps and drum tracks, found or made and added to `tests/corpus/`
- [ ] End-to-end tests for the entry workflow and golden images for the notation, which the testing section calls for and which do not exist yet

**Done when:** no phase from 1 to 5 has a simplification or a check by hand left against it, and what replaced each one is covered by a test.

### Phase 7 — Import and export

- [ ] Full Guitar Pro import, extending the phase 2 importer: `.gp3`, `.gp4`, `.gp5`, `.gpx`, `.gp`, covering every technique and score element from phases 3 and 5, including what is skipped today: lyrics, chord names and diagrams, tempo changes and `.gp` drum tracks
- [ ] Guitar Pro export: `.gp`
- [ ] MusicXML import and export
- [ ] MIDI import and export
- [ ] ASCII tab import and export
- [ ] PDF, SVG and PNG export; printing
- [ ] Audio export: WAV, MP3, FLAC, OGG
- [ ] Import test corpus with rendering comparisons against reference output

**Done when:** the import corpus opens without errors and renders recognisably.

### Phase 8 — Practice tools

- [ ] Loop a selection, with optional tempo increase per pass (speed trainer)
- [ ] Relative playback speed
- [ ] Visual metronome
- [ ] Tuner using microphone or line-in
- [ ] Fretboard, keyboard and drum kit views that follow playback and accept input
- [ ] Chord library and scale library with fretboard diagrams
- [ ] Scale finder from a selection

### Phase 9 — Sound

The synth itself and its SoundFonts come in phase 6; this phase is what is built around them.

- [ ] Per-track effects chain: amp simulation, cabinet, overdrive, distortion, compressor, EQ, chorus, delay, reverb, wah
- [ ] Pedalboard view and presets
- [ ] Per-track sound banks with switching mid-score via automation
- [ ] Master track effects
- [ ] Audio track: import an audio file and sync the score to it with tempo mapping
- [ ] Line-in: play a real guitar through the track's effects
- [ ] Native audio backend on desktop if AudioWorklet latency is not good enough

### Phase 10 — Polish and release

- [ ] Stylesheets: fonts, spacing, symbol sizes, notation conventions
- [ ] Design mode: manual bar widths, line breaks, system layout
- [ ] Page and screen layouts: page, parchment, horizontal and vertical scroll
- [ ] Command palette
- [ ] Customisable keyboard shortcuts
- [ ] Batch file conversion
- [ ] Preferences, themes, localisation scaffolding
- [ ] Signed installers and auto-update for Windows, macOS, Linux. macOS installers have to be built on a Mac, so they come from a release workflow on GitHub's macOS runners, set up with the first release; nothing has been run on a Mac beyond the CI compile
- [ ] Offline-capable web app with local file access

## Testing

- **Model and editing:** unit tests plus property tests that apply random command sequences and check undo returns the original score.
- **Layout and rendering:** golden-image tests per notation feature; a diff means review, not automatic failure. None exist yet: layout is tested by what it places where. They are a phase 6 item.
- **File formats:** round-trip tests for the native format; a corpus of real files for each import format. No Guitar Pro files are in the repository yet: `crates/gtr-io/tests/gp_samples.rs` runs against a directory named by `GTR_GP_SAMPLES`. Add files we are free to redistribute to `tests/corpus/`.
- **Audio:** snapshot tests on the event stream rather than on audio samples.
- **UI:** end-to-end tests for the core entry workflow. None exist yet; the interface has been checked by hand in a browser. They are a phase 6 item.

## Risks

- **Scope:** Guitar Pro 8 is years of work by a commercial team. The phase order front-loads the parts needed for writing riffs and songs.
- **Shortcuts piling up:** each phase so far shipped with simplifications, and phase 5 with many. Decision 4 and phase 6 are the answer: a shortcut is only taken with the phase that removes it written beside it.
- **Engraving quality:** spacing and collision avoidance are open-ended. Budget for ongoing refinement rather than a one-off implementation.
- **Sound quality:** Guitar Pro's sound comes from proprietary sample banks. Feature parity is achievable; tonal parity depends on the freely licensed SoundFonts available.
- **Web audio latency:** fine for playback, possibly not for line-in monitoring. Phase 9 has a native fallback for desktop.
- **File format coverage:** Guitar Pro formats are undocumented and have many edge cases. Coverage will improve with the corpus over time.

## Reference

- alphaTab: open-source library that renders and plays Guitar Pro files. Viewer, not editor. Useful for file format details and rendering comparison.
- SMuFL specification and the Bravura font.
- MusicXML 4.0 specification.
