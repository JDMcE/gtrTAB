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
  gtr-worklet/   the synth alone, as a plain WASM module for the AudioWorklet
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
- [x] WASM build and TypeScript bindings (the score, editor and layout types are generated from Rust since phase 6)
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
- [x] SoundFont synth (done in phase 6, where it had been put off to)
- [x] Metronome and count-in
- [x] File open, save, recent files (desktop only), autosave with recovery on start
- [x] Simple read-only Guitar Pro import (`.gp` and `.gp5`): notes, rests, ties, rhythms, tunings, starting tempo and time signatures for all tracks, with one track shown at a time. Anything else is skipped and listed in the status bar after import

Checked in phase 6: how playback sounds (by measurement; see there), and recent files, drag-and-drop and the unsaved-changes prompt in the desktop app. (Autosave recovery was found broken during phase 5, where the saved copy was cleared before it could be read, and fixed.)

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

Pick strokes and fingering are notation only, and stay that way: they have nothing to add to how a note sounds. (A custom bend used to be drawn as the nearest common shape; since phase 6 it is drawn as its own curve.)

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

Still to check by hand, on Windows: the desktop title bar stayed dark when the app was switched to the light theme, so `setTheme` on the window may not be taking effect. Phase 6 could not look at it, having no Windows machine; a failure there is now logged to the console instead of being swallowed. The interface has had the automated part of a screen reader check in phase 6, and has not been used with one by a person.

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
- [x] Tempo, volume and pan automation; fermatas; swing and triplet feel. Tempo changes on the beat under the cursor (Score menu); volume and pan change at a beat (Beat menu) and are played back; swing eighths and sixteenths
- [x] Sections, rehearsal marks, free text, timers
- [x] Lyrics with multiple lines (entered beat by beat, up to five lines)
- [x] Chord diagrams and names with a chord builder: pick a root and a kind of chord and it offers the ways to finger it in the track's tuning, or click the frets yourself, or take the notes on the beat
- [x] Mixer: volume, pan, mute, solo, instrument selection per track, with the mute, solo and volume controls on each row of the track pane
- [x] Score information and page headers: subtitle, words, music, transcriber and copyright alongside title, artist and album

Known simplifications, each with the phase that removes it:

- Beams were level and accidentals spelled by the key alone; in the combined view the markings sat between the two staves, and the notation-only view left out bends and slides. (Removed in phase 6)
- The mixer sliders were for the mouse alone, tracks were reordered with buttons only, a click on another staff went to the start of its bar, and the chord builder found one shape per position with the root in the bass. (Removed in phase 6)
- Guitar Pro import skips lyrics, chord names and diagrams, and tempo changes. (Phase 7)

How things stand, for reference:

- Tuning and capo are part of a track's settings (the settings button in the track pane, or double-click the track's name), alongside its instrument and mix. Changing the kind of instrument there switches to that instrument's usual tuning.
- Guitar Pro import brings in instruments, volume and pan (`.gp5`), drum tracks, key signatures, repeats, alternate endings, sections, swing feel, credits, signs and jumps and, from `.gp`, free text. A bar holds one sign and one jump: where a file has more, or a second segno or coda, the first is kept and the import says so.
- The native format is at version 7 since phase 7, which made a bar's sign and jump lists and added optional fields. At the end of phase 5 it was at version 6. Version 4 only added optional fields; version 5 turned a grace note into a list of them, with a migration, and added tempo changes inside a bar and where a harmonic is touched; version 6 took the strings off a piano, with a migration, and added a track's own drum kit.

Checked in phase 6: each instrument, the drum kit and the stereo mix, by measurement; the track pane and the notation views in the desktop window and in a narrow one; and Guitar Pro files with repeats, jumps and drum tracks, which are now in `tests/corpus/`.

**Done when:** a full band arrangement can be written and played back.

### Phase 6 — Consolidation

Phases 2 to 5 each took shortcuts to get something usable. This phase removes them before import and export, the practice tools and the sound work are built on top: those lean on playback timing, on the note model and on the engraving, and it is cheaper to put those right before files and features depend on how they behave now. Nothing here is a new feature.

Playback engine:

- [x] Streaming synth: the synth runs in an AudioWorklet as WASM and plays from the event stream, as the architecture section describes, instead of rendering every track to a buffer first. Edits are heard without stopping, memory no longer grows with the length of the score, and looping, speed changes and audio export in later phases get the engine they need. `gtr_audio::Engine` makes the sound a block at a time; `crates/gtr-worklet` wraps it as a module the worklet (`app/ui/src/lib/synth.worklet.js`) can load without `wasm-bindgen`. After an edit the passage is compiled again and the engine carries on from the same beat of it; notes already sounding ring out as they were
- [x] SoundFont playback: sampled instruments from a freely licensed SoundFont for every instrument and the drum kit, with the built-in models kept as the fallback when no SoundFont is loaded. The bank is GeneralUser GS 2.0.3 by S. Christian Collins, fetched by `npm run soundfont` in `app/ui` (32 MB, so it is not in the repository) and loaded in the background once the app is up. Score > Sound switches between it, any `.sf2` file of the user's own, which is kept in the browser's storage for next time, and the models. How things stand:
  - The licence allows any use, changes and redistribution, which meets decision 2. Its author adds that he cannot vouch for where every sample came from; that is worth reading again before a release, and is why the licence file is fetched to sit beside the bank
  - `gtr_audio::SoundFont` follows what a plucked or struck instrument needs: key and velocity zones, tuning, loops, level, pan, the volume envelope, exclusive classes (a closed hi-hat stops an open one), and the filter in full: its cutoff, the modulation envelope that opens and closes it, the modulators that move it and that envelope's depth with the key and the velocity, and envelope times that shorten up the keyboard. GeneralUser GS keeps its brightness there, and read without it (as it was until the check at the end of this phase) the steel guitar sat under a 500 Hz filter and the tambourine could not be heard. It does not follow modulators of anything but the filter, the envelope's hold on pitch, the bank's own vibrato or its reverb and chorus. Compressed banks (`.sf3`) are refused
  - Palm mutes and dead notes are made from the ordinary sample, damped and dulled, and a harmonic is the ordinary sample at the harmonic's pitch
  - The whole bank goes to the audio thread, about 64 MB of memory there, and reading it stops the sound for a moment if something is playing. Cutting the bank down to the instruments gtrTab has would shrink the download and the memory several times over, and is still to do
- [x] Each instrument with its own sound: ukulele, banjo and mandolin stop borrowing a guitar's. The banjo is the bank's; General MIDI has no ukulele or mandolin, so those two are always played by models of their own, also when a SoundFont is loaded

Playback behaviour:

- [x] Fermatas that hold: the beat under one lasts twice as long, in every track, and timers and the metronome wait with it (`gtr_model::BarTiming`)
- [x] Tempo changes on a beat inside the bar: Score > Tempo change puts one on the cursor's beat, the mark stands over that beat, and it lasts for the bars played after it. They are kept with the bar (`MasterBar::tempo_changes`), not with a track
- [x] Tempo and feel follow the order bars are played in, so a repeat or jump back past a change restores what was in force there. A bar is always played at the tempo it had the first time it was reached, so playing from the cursor sounds the same as playing through
- [x] Repeats after a D.C. or D.S. are taken once through by default, on their last ending, as convention has it, with a setting to play them again (Score menu; it lasts for the session and timers always assume the convention); playing a selected range follows the repeats inside it, but takes no jumps
- [x] Artificial, pinch, tapped and semi harmonics at their true pitch: the overtone of the stopped note that sounds where the string is touched. That place is the octave unless Note > Harmonic says otherwise (7, 5, 4 or 3 frets up); a pinch harmonic, made near the pickups, sounds two octaves up unless it says otherwise. Read from `.gp` files
- [x] Rasgueado as the strokes it is made of: four quick downstrokes, the last the hardest. Tapped notes sound hammered on, not picked
- [x] Grace notes of several notes, and on other strings than the note they decorate: up to four, played in turn, each drawn on its own string
- [x] Volume and pan written in the score and the mixer work together: the slider moves the whole passage up or down instead of being overridden. A written volume is measured from the slider (75 is the slider's own level) and a written pan from where the slider puts the track, so scores made with the sliders at their defaults sound as before

Notes and entry:

- [x] Notes that are a pitch, not a string and a fret, for instruments without strings: the model, the commands, the native format (with a migration) and the importers. A staff for such an instrument has a tuning with no strings, and a note on it says its MIDI pitch where a note on strings says its string; a beat can hold any number of them. `Note` keeps its two fields, `string` and `fret`, with `string` holding the pitch: everything that tells the notes of a beat apart by string, from ties to the cursor, then works for pitches as it stands
- [x] Piano entry by pitch on the notation staff; no tab staff for a piano; the two staves adjacent in every view; changing a track to or from a piano adds or removes its second staff. The letters A to G enter the note of that name nearest the cursor, as the key has it, and Enter a note where the cursor is; the arrows move the cursor through the notes of the key, Alt with them moves the note under it a semitone, and a click on the staff puts the cursor on the line or space clicked. Changing a guitar to a piano keeps its notes as the pitches they sound; changing back puts each on the string that plays it nearest the nut, and what the strings cannot reach, and the left hand's staff, is lost (it is one undo step). Still to do: entry from a MIDI keyboard
- [x] Drum notation that puts hands and feet in two voices by itself, and a kit map that can be edited. A bar written in one voice is drawn in two wherever it has both: the hands as written with stems up, the feet with stems down, each note lasting to the next or to the end of its beat, and without the rests between. Track > Drum kit… gives a track a kit of its own: which numbers it has, what each is called, where and with which notehead it is written, and whether a foot plays it. A piece's number still decides its sound
- [x] TypeScript types generated from the Rust model instead of written by hand: `ts-rs` writes them to `app/ui/src/lib/generated` when the tests run, `model.ts` gathers them, and CI fails if what is committed is out of date

Engraving:

- [x] Slanted beams: a beam on the notation staff leans the way its notes go, a quarter of a space for each step between the first note and the last, up to a space and a quarter and never steeper than 3 in 10. It stays level when the two are on the same step, or when a note between them comes nearer the beam than either. Every stem is at least its usual length and ends on the beam. The beams under a tab staff and over rhythm slashes have no pitches to follow and stay level. The display list has a `beam` item for them
- [x] Accidentals spelled by context as well as by key (`notation::spell`). A note of the key is written as the key has it, so the F in F sharp major is an E sharp. One outside it that could be a sharp or a flat goes, in this order, by the chord it is in (a third is written as a third, and two pitches never share a line), by where its line goes (a sharp if it rises a semitone, a flat if it falls one, looking past repeats of the note), by how it was last written in the bar, and then by the key as before. A tied note keeps the name of the note it is tied to
- [x] In the combined view, chord names, diagrams, text and technique markings above the notation staff, not between the staves. Bends stay over the tab, since they are drawn from the string; dynamics and lyrics stay under it
- [x] In the notation-only view, the markings that are written on strings: bends rise from the notehead into the same lane as over a tab staff, and slides are lines between noteheads or short ones into and out of a note
- [x] A custom bend drawn as its own curve: a bend that is not one of the four common shapes is drawn as a step for every level it moves to, each as high in its lane as that level is of the curve's highest, with an arrow and its size; the beat is given the room. How fast a bend gets from one level to the next is still not shown, as it is not in the common shapes

Interface:

- [x] Tracks reordered by dragging in the track pane: a track's name is the handle, the row fades while it is dragged and a line shows where it will land. Alt+Up and Alt+Down on a name do the same from the keyboard, and the move up and move down buttons stay
- [x] Mixer sliders in the track pane that the keyboard can reach: the left and right arrows walk a row from the name through mute and solo to the volume and pan sliders, and on a slider the up and down arrows, Page Up, Page Down, Home and End set it (`roving.ts`). The pane is still one Tab stop. Every step is an undo step of its own
- [x] A click on another staff goes to the beat clicked: each row of the layout carries its own beat positions (`RowBox::beats`), and a click well to the right of a staff's last beat goes to the empty place after it
- [x] Chord builder: slash chords, inversions, and more than one shape per position. A bass note can be chosen beside the root and kind; a note of the chord gives an inversion and any other a slash chord, named with a slash either way. Each position offers a shape from every string the bass note can be on, with the other strings low in the position and again with them high, up to 24 in all. `npm test` runs the builder's tests under Node, and CI with it

Checks owed from earlier phases:

- [x] Every instrument, the drum kit and the stereo mix, measured, and what measured wrong fixed. Each instrument was played across its range, and each piece of the kit, through the SoundFont and through the models, and its level, tuning, peak, steady offset and place between the speakers taken. Three things were wrong. The SoundFont reader left out what GeneralUser GS uses to open its filters (see above): the top of the steel guitar's neck was 25 dB under the bottom of it, and is now within 3; the tambourine was 20 dB under the rest of the kit. The string models carried a steady offset that outlasted the note and, at the top of a piano, was most of what sounded. And with its filter open the sampled distortion guitar was louder than its model, and the bass three times its model's level, so their trims came down. Tuning is within a few cents throughout, panning keeps the power constant, and a full band does not clip. `crates/gtr-audio/tests/soundfont_file.rs` holds the bank to this when `GTR_SOUNDFONT` names it
- [ ] The same by ear. Nobody has listened: a measurement finds a filter that is shut, not a guitar that sounds like a toy
- [x] The desktop app, on Linux: the track pane with dragging, the notation views, the native save dialog, recent files and the unsaved-changes prompt, each answered both ways, by running the built app and working it through the X server. A file dropped on the window is opened by the page itself (`dragDropEnabled` is off), and that is tested in the browser; a drop from a file manager was not tried. Closing the window does not ask about unsaved work, and is not meant to: autosave brings it back
- [ ] The title bar that stayed dark in the light theme on Windows. Not looked at: it needs a Windows machine
- [x] A narrow window, with the track pane and the notation views: at 640 pixels, the least the desktop window goes to, and at 420. Below 640 the top bar's buttons ran off the side, and now wrap. `e2e/narrow.spec.ts`
- [x] A screen reader, as far as a machine can check. The score was a picture and nothing else; it is now also put in words, in a live region that says what is under the cursor whenever that changes ("Bar 2, beat 3 of 8, eighth note, string 5, fret 2, B 2, hammer-on or pull-off"). axe finds nothing wrong in the app, its dialogs or its menus, after the track pane's resize handle was given its value, the work area made the main landmark and the name a heading. Every part of the window is one Tab stop. `e2e/accessibility.spec.ts`, `src/lib/describe.ts`
- [ ] The same with a screen reader and a person. Whether the words are the right ones, and come at the right moments, is not something a test can say
- [x] Guitar Pro files with repeats, jumps and drum tracks, found and added to `tests/corpus/`: fifteen small files from alphaTab's test data, under the MPL 2.0, each made to show one thing (`tests/corpus/README.md`). They found four faults in the importer. A `.gp5` repeat was played once too often. A `.gp5` drum track that is on MIDI channel 10 without saying it is percussion came in as a piano. The signs and jumps of `.gp5` were not read at all. And a `.gp` drum track came in as nonsense, without a word: it is now read, each note as the General MIDI number its articulation sounds. `crates/gtr-io/tests/corpus.rs` says what each file holds
- [x] End-to-end tests for the entry workflow and golden images for the notation. Playwright drives the built app (`npm run e2e` in `app/ui`): a riff written, changed, undone, copied, played, saved, reopened, dropped and recovered, from the keyboard alone. `gtr-render` writes a laid-out score as SVG, and `crates/gtr-render/tests/golden.rs` compares ten scores with the pictures in `tests/golden`. Looking at the first pictures found a key change at the start of a line written twice, the labels over narrow bars running into each other, bar numbers under key signatures and track names under clefs, all fixed

**Done when:** no phase from 1 to 5 has a simplification or a check by hand left against it, and what replaced each one is covered by a test. Three checks are left, and none of them can be done by a program: listening, a person with a screen reader, and the title bar on Windows.

### Phase 7 — Import and export

- [x] Full Guitar Pro import, extending the phase 2 importer: `.gp3`, `.gp4`, `.gp5`, `.gpx`, `.gp`, covering every technique and score element from phases 3 and 5, including what was skipped until now: lyrics, chord names and diagrams, text, and tempo changes. How things stand:
  - Versions 3 and 4 are read by the reader for version 5, which carries the version as a number and asks it as it goes (`guitar_pro/gp5.rs`): the three are one format that grew. Version 6 keeps the XML score of version 7 in a container of its own, a small file system that is usually compressed (`guitar_pro/gpx.rs`)
  - Also read: track volume and pan, fermatas and timers from the XML formats, and changes of volume and pan from the binary ones. A change of tempo inside a bar is put at its place in the bar to the nearest sixty-fourth of it. A tempo that version 5 reaches over so many beats becomes a gradual change from the tempo before it, and a change to the new tempo where it arrives
  - Nothing a file holds is left out for want of a place in the model any more. What the first pass of this item skipped was given one: fades (in, out and swell, on a beat), a tempo that changes by degrees, the dotted and Scottish feels, a second segno and coda with the jumps that use them, any number of signs and jumps on a bar, notes down to a 256th and a third dot, and the drum notes of version 6, which name a piece of its kit and a way of hitting it. Each is drawn, played, saved (the native format is at version 7, with a migration for the signs and jumps, which became lists) and can be written in the app: Beat > Fade, Score > Segno, coda and fine, Jumps at the end of the bar, Feel, and Change tempo by degrees. The exception is dots: the app still enters one, and shows the two or three a file brings
  - A jump is taken once, in the order a bar lists them, and a way on to a coda is open only after the jump that names that coda. A gradual change runs from one place the tempo is set to the next as the score is written, and is played as steps a sixteenth of a whole note long. Only notes shorter than a 256th and more than three dots are still skipped, and listed after the import
  - The corpus is alphaTab's test data for every version, 214 files (`tests/corpus/README.md`). Of the 104 pairs of files that are the same bars saved by two versions, 97 read alike and the other 7 are not the same bars. A file cut short or with bytes changed is refused or read as far as it goes, and never panics. Eleven of alphaTab's files are transcriptions of other people's songs and are not in the repository; kept beside it, they import whole, the ten tracks and 216 bars of one in version 4 with every bar adding up
- [ ] Guitar Pro export: `.gp`
- [ ] MusicXML import and export
- [ ] MIDI import and export
- [ ] ASCII tab import and export
- [ ] PDF, SVG and PNG export; printing
- [x] Audio export: WAV. File > Export audio (WAV)… writes the whole score as it plays, with its repeats, every track that is not muted, through the sound chosen for playback, as 16-bit stereo at 44.1 kHz. It is made four seconds at a time on the page's own thread, which goes on answering and says how far it has got (`Document::export_audio`, `gtr_audio::wav`). MP3 and OGG are not wanted: their encoders would mean bundling a library under another licence, or leaning on what each browser offers. FLAC can be written in Rust without either, and is left until it is asked for
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
- **Layout and rendering:** layout is tested by what it places where, and by golden images: `crates/gtr-render/tests/golden.rs` lays out a score for each area of the notation, writes it as SVG and compares it with the picture in `tests/golden`. A difference fails the test and leaves the new picture in `target/golden`; look at the two, and if the change is meant run the tests with `GTR_BLESS=1` and commit the pictures. They open in a browser straight from the repository.
- **File formats:** round-trip tests for the native format; a corpus of real files for each import format in `tests/corpus/`, with a note of where each came from and its licence. `crates/gtr-io/tests/corpus.rs` says what each Guitar Pro file there holds, and `gp_samples.rs` imports them all, and any in a directory named by `GTR_GP_SAMPLES`, and checks a piece saved in two formats comes out the same. Only add files we are free to redistribute.
- **Audio:** snapshot tests on the event stream rather than on audio samples. The synth is tested by what it measures: pitch, level, and that playing in blocks gives the same samples as rendering at once. The SoundFont reader is tested on a small bank made up in the test; `crates/gtr-audio/tests/soundfont_file.rs` plays every instrument from a real one named by `GTR_SOUNDFONT`.
- **UI:** end-to-end tests of the built app in `app/ui/e2e`, run by Playwright (`npm run build`, then `npm run e2e`): the entry workflow from the keyboard, a narrow window, and what a screen reader and a keyboard are given. They read the score from the words the app puts it in for a screen reader. The parts of the UI that are plain functions (the chord builder, those words) have unit tests beside them, run by `npm test`.

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
