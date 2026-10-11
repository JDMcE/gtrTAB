<script lang="ts">
  import { onMount } from 'svelte';
  import type { Command, Menu, MenuEntry, PaletteSection, ShortcutGroup } from './lib/commands';
  import Dialogs, { type DialogKind } from './lib/Dialogs.svelte';
  import EffectDialog, { bendPreset, type EffectKind } from './lib/EffectDialog.svelte';
  import {
    EXTENSION,
    addRecentFile,
    confirmDialog,
    openFile,
    readAutosave,
    readPath,
    recentFiles,
    removeRecentFile,
    saveFile,
    setWindowTitle,
    writeAutosave,
    type OpenedFile,
  } from './lib/files';
  import Inspector from './lib/Inspector.svelte';
  import {
    NOTE_VALUES,
    type Dynamic,
    type EditorState,
    type Layout,
    type Score,
    type Timeline,
  } from './lib/model';
  import Palette from './lib/Palette.svelte';
  import { Player } from './lib/player';
  import { THEMES, applyTheme, loadPrefs, savePrefs, type Theme } from './lib/prefs';
  import ScoreView from './lib/ScoreView.svelte';
  import Shortcuts from './lib/Shortcuts.svelte';
  import Icon from './lib/ui/Icon.svelte';
  import type { IconName } from './lib/ui/icons';
  import MenuBar from './lib/ui/MenuBar.svelte';
  import { roving } from './lib/ui/roving';
  import ToolButton from './lib/ui/ToolButton.svelte';
  import { tip } from './lib/ui/tooltip';
  import init, { Document } from './wasm/gtr_wasm.js';

  const UNTITLED = `untitled.${EXTENSION}`;
  const VALUE_NAMES = ['Whole', 'Half', 'Quarter', 'Eighth', '16th', '32nd', '64th'];
  const DYNAMICS: [Dynamic, string][] = [
    ['ppp', 'Pianississimo'],
    ['pp', 'Pianissimo'],
    ['p', 'Piano'],
    ['mp', 'Mezzo-piano'],
    ['mf', 'Mezzo-forte'],
    ['f', 'Forte'],
    ['ff', 'Fortissimo'],
    ['fff', 'Fortississimo'],
  ];
  /** Tuplets as notes played in the time of how many. */
  const TUPLETS = [
    [3, 2],
    [5, 4],
    [6, 4],
    [7, 4],
    [9, 8],
    [10, 8],
    [11, 8],
    [12, 8],
    [13, 8],
  ];
  const FINGERS = ['thumb', 'index', 'middle', 'ring', 'pinky'];
  const SPEEDS = [1, 0.75, 0.5];
  const ZOOMS = [0.5, 0.75, 1, 1.25, 1.5, 2];
  const THEME_NAMES: Record<Theme, string> = { system: 'System', light: 'Light', dark: 'Dark' };
  const THEME_ICONS: Record<Theme, IconName> = {
    system: 'themeSystem',
    light: 'themeLight',
    dark: 'themeDark',
  };
  /** Longest pause between two digits that still makes one fret number. */
  const DIGIT_WINDOW_MS = 900;
  const AUTOSAVE_DELAY_MS = 1500;
  /** Below this window width the palette becomes a strip and the inspector floats. */
  const NARROW_WIDTH = 860;
  /** From this width the inspector fits beside a full-width page, so it opens by default. */
  const ROOMY_WIDTH = 1480;
  /** Keys that belong to a focused toolbar or palette rather than to the score. */
  const WIDGET_KEYS = ['ArrowLeft', 'ArrowRight', 'ArrowUp', 'ArrowDown', 'Home', 'End', ' ', 'Enter'];

  let doc = $state.raw<Document>();
  const player = new Player();

  let score = $state.raw<Score>();
  let editor = $state.raw<EditorState>();
  let fileName = $state(UNTITLED);
  let filePath = $state<string | null>(null);
  let message = $state('');
  let failed = $state(false);
  let recent = $state(recentFiles());
  let dialog = $state<DialogKind | null>(null);
  let effectDialog = $state<EffectKind | null>(null);
  let shortcutsOpen = $state(false);
  let zoom = $state(1);

  let playing = $state(false);
  let playhead = $state.raw<{ bar: number; beat: number } | null>(null);
  let metronome = $state(false);
  let countIn = $state(false);
  let speed = $state(1);

  const prefs = $state(loadPrefs());
  let width = $state(0);
  const narrow = $derived(width > 0 && width < NARROW_WIDTH);
  const compact = $derived(prefs.compact ?? narrow);
  const inspectorOpen = $derived(prefs.inspector ?? width >= ROOMY_WIDTH);

  let revision = -1;
  let lastDigitAt = 0;

  $effect(() => savePrefs($state.snapshot(prefs)));

  function setTheme(theme: Theme) {
    prefs.theme = theme;
    applyTheme(theme);
  }

  function toggleSection(id: string) {
    prefs.collapsed = prefs.collapsed.includes(id)
      ? prefs.collapsed.filter((s) => s !== id)
      : [...prefs.collapsed, id];
  }

  /** Gives the keyboard back to the score. */
  const focusScore = () => document.querySelector<HTMLElement>('[data-score]')?.focus();

  function setDocument(next: Document, name: string, path: string | null) {
    stop();
    doc?.free();
    doc = next;
    revision = -1;
    fileName = name;
    filePath = path;
    refresh();
  }

  /** Pulls the editor state from the core, and the score too if it changed. */
  function refresh() {
    if (!doc) return;
    const state: EditorState = JSON.parse(doc.stateJson());
    if (state.revision !== revision) {
      revision = state.revision;
      score = JSON.parse(doc.scoreJson());
    }
    editor = state;
  }

  /** Runs an action, showing any failure in the status bar. */
  async function run(action: () => unknown) {
    message = '';
    failed = false;
    try {
      await action();
    } catch (e) {
      message = e instanceof Error ? e.message : String(e);
      failed = true;
    }
    refresh();
  }

  /** Runs an edit on the open document. */
  const edit = (action: (doc: Document) => unknown) => run(() => doc && action(doc));

  // --- Files ---

  async function discardChanges(): Promise<boolean> {
    return !editor?.dirty || confirmDialog(`Discard unsaved changes to ${fileName}?`);
  }

  const newScore = () =>
    run(async () => {
      if (await discardChanges()) setDocument(new Document(), UNTITLED, null);
    });

  function load(file: OpenedFile) {
    if (file.name.toLowerCase().endsWith(`.${EXTENSION}`)) {
      setDocument(Document.load(file.bytes), file.name, file.path);
      if (file.path) recent = addRecentFile(file.path);
      return;
    }
    // Imported files are saved as a new gtrTab score, never over the original.
    const imported = Document.importFile(file.name, file.bytes);
    const report = imported.importReport;
    imported.markUnsaved();
    setDocument(imported, file.name.replace(/\.[^.]+$/, '') + `.${EXTENSION}`, null);
    message = report ? `Imported ${file.name}. Not imported: ${report}.` : `Imported ${file.name}.`;
  }

  const open = () =>
    run(async () => {
      if (!(await discardChanges())) return;
      const file = await openFile();
      if (file) load(file);
    });

  /** Opens a file dropped onto the window. */
  function onDrop(event: DragEvent) {
    const file = event.dataTransfer?.files[0];
    if (!file) return;
    event.preventDefault();
    run(async () => {
      if (!(await discardChanges())) return;
      load({ name: file.name, path: null, bytes: new Uint8Array(await file.arrayBuffer()) });
    });
  }

  const openRecent = (path: string) =>
    run(async () => {
      if (!(await discardChanges())) return;
      try {
        load(await readPath(path));
      } catch (e) {
        recent = removeRecentFile(path);
        throw e;
      }
    });

  const save = (saveAs = false) =>
    run(async () => {
      if (!doc) return;
      const saved = await saveFile(fileName, saveAs ? null : filePath, doc.save());
      if (!saved) return;
      doc.markSaved();
      fileName = saved.name;
      filePath = saved.path;
      if (saved.path) recent = addRecentFile(saved.path);
      message = `Saved ${saved.name}`;
    });

  // Keep a copy of unsaved work so that a crash or closed window does not lose it.
  $effect(() => {
    if (!editor?.dirty) {
      writeAutosave(null);
      return;
    }
    void score;
    const timer = setTimeout(() => {
      if (doc) writeAutosave({ name: fileName, path: filePath, bytes: doc.save() });
    }, AUTOSAVE_DELAY_MS);
    return () => clearTimeout(timer);
  });

  $effect(() => {
    const title = `${editor?.dirty ? '• ' : ''}${fileName} — gtrTab`;
    setWindowTitle(title).catch(() => {});
  });

  // --- Playback ---

  const play = () =>
    run(async () => {
      if (!doc || !editor) return;
      const selection = editor.selection;
      const options = {
        from_bar: selection ? selection[0][0] : editor.cursor.bar,
        to_bar: selection ? selection[1][0] + 1 : null,
        metronome,
        count_in: countIn,
        speed,
      };
      const audio = doc.renderAudio(JSON.stringify(options), player.sampleRate);
      const timeline: Timeline = JSON.parse(audio.timelineJson);
      const samples = audio.takeSamples();
      audio.free();
      await player.play(samples, timeline, () => {
        playing = false;
        playhead = null;
      });
      playing = true;
      follow();
    });

  function follow() {
    if (!playing) return;
    const now = player.position();
    if (now && (now.bar !== playhead?.bar || now.beat !== playhead?.beat)) {
      playhead = { bar: now.bar, beat: now.beat };
    }
    requestAnimationFrame(follow);
  }

  /** Stops playback, leaving the cursor where the music stopped. */
  function stop() {
    if (!playing) return;
    player.stop();
    playing = false;
    const at = playhead;
    playhead = null;
    if (at && doc && editor) {
      doc.setCursor(at.bar, at.beat, editor.cursor.string, false);
      refresh();
    }
  }

  const togglePlay = () => (playing ? stop() : play());

  // --- Techniques ---

  const technique = (name: string) => edit((d) => d.toggleTechnique(name));
  const effects = $derived(editor?.note?.effects ?? {});
  const beatEffects = $derived(editor?.beat_effects ?? {});

  const setNoteEffect = (field: string, value: unknown) =>
    edit((d) => d.setNoteEffect(field, JSON.stringify(value)));
  const setBeatEffect = (field: string, value: unknown) =>
    edit((d) => d.setBeatEffect(field, JSON.stringify(value)));

  // --- Commands ---

  const noNote = () => !editor?.note;
  const noScore = () => !score;

  /** A command that switches a technique on and off. */
  const toggle = (
    label: string,
    icon: IconName,
    name: string,
    active: () => boolean,
    keys?: string,
  ): Command => ({ label, icon, keys, active, run: () => technique(name) });

  const noteValues: Command[] = NOTE_VALUES.map((value, i) => ({
    label: `${VALUE_NAMES[i]} note`,
    icon: value,
    active: () => editor?.duration.value === value,
    run: () => edit((d) => d.setNoteValue(value)),
  }));

  const dynamics: Command[] = DYNAMICS.map(([value, name]) => ({
    label: `${name} (${value})`,
    icon: value,
    active: () => beatEffects.dynamic === value,
    run: () => setBeatEffect('dynamic', beatEffects.dynamic === value ? null : value),
  }));

  const tuplets = (nested: boolean): Command[] =>
    TUPLETS.slice(0, nested ? 4 : TUPLETS.length).map(([actual, normal]) => {
      const current = () => (nested ? editor?.duration.nested : editor?.duration.tuplet);
      const active = () => current()?.actual === actual && current()?.normal === normal;
      return {
        label: `${actual} in the time of ${normal}`,
        active,
        disabled: () => nested && !editor?.duration.tuplet,
        run: () =>
          edit((d) => {
            const [a, n] = active() ? [0, 0] : [actual, normal];
            return nested ? d.setNestedTuplet(a, n) : d.setTuplet(a, n);
          }),
      };
    });

  const fingering = (field: 'left_finger' | 'right_finger', marks: string): Command[] =>
    FINGERS.map((finger, i) => ({
      label: `${marks[i]}  ${finger}`,
      active: () => effects[field] === finger,
      disabled: noNote,
      run: () => setNoteEffect(field, effects[field] === finger ? null : finger),
    }));

  const cmd = {
    new: { label: 'New', icon: 'new', keys: 'Ctrl+N', run: newScore },
    open: { label: 'Open or import…', icon: 'open', keys: 'Ctrl+O', run: open },
    save: { label: 'Save', icon: 'save', keys: 'Ctrl+S', disabled: noScore, run: () => save() },
    saveAs: { label: 'Save as…', icon: 'saveAs', keys: 'Ctrl+Shift+S', disabled: noScore, run: () => save(true) },

    undo: { label: 'Undo', icon: 'undo', keys: 'Ctrl+Z', disabled: () => !editor?.can_undo, run: () => edit((d) => d.undo()) },
    redo: { label: 'Redo', icon: 'redo', keys: 'Ctrl+Y', disabled: () => !editor?.can_redo, run: () => edit((d) => d.redo()) },
    cut: { label: 'Cut', icon: 'cut', keys: 'Ctrl+X', run: () => edit((d) => d.cut()) },
    copy: { label: 'Copy', icon: 'copy', keys: 'Ctrl+C', run: () => edit((d) => d.copy()) },
    paste: { label: 'Paste', icon: 'paste', keys: 'Ctrl+V', disabled: () => !editor?.can_paste, run: () => edit((d) => d.paste()) },
    insertBeat: { label: 'Insert a rest before the cursor', keys: 'Insert', run: () => edit((d) => d.insertBeat()) },
    delete: { label: 'Remove the note, or the beat if it is a rest', keys: 'Delete', run: () => edit((d) => d.delete()) },
    deleteBeats: { label: 'Remove the beat or the selected beats', keys: 'Ctrl+Delete', run: () => edit((d) => d.deleteBeats()) },

    dot: { label: 'Dotted', icon: 'dot', keys: '.', active: () => (editor?.duration.dots ?? 0) > 0, run: () => edit((d) => d.toggleDot()) },
    triplet: { label: 'Triplet', icon: 'triplet', keys: '/', active: () => !!editor?.duration.tuplet, run: () => edit((d) => d.toggleTriplet()) },
    rest: { label: 'Rest', icon: 'rest', keys: 'R', run: () => edit((d) => d.rest()) },
    tie: { label: 'Tie to the previous note', icon: 'tie', keys: 'L', active: () => !!editor?.note?.tied, run: () => edit((d) => d.toggleTie()) },

    hammer: toggle('Hammer-on or pull-off to the next note', 'hammer', 'hammer', () => !!effects.hammer, 'H'),
    slideShift: toggle('Shift slide to the next note', 'slideShift', 'slide_shift', () => effects.slide === 'shift', 'S'),
    slideLegato: toggle('Legato slide to the next note', 'slideLegato', 'slide_legato', () => effects.slide === 'legato'),
    slideInBelow: toggle('Slide in from below', 'slideInBelow', 'slide_in_below', () => effects.slide === 'in_from_below'),
    slideInAbove: toggle('Slide in from above', 'slideInAbove', 'slide_in_above', () => effects.slide === 'in_from_above'),
    slideOutDown: toggle('Slide out downwards', 'slideOutDown', 'slide_out_down', () => effects.slide === 'out_down'),
    slideOutUp: toggle('Slide out upwards', 'slideOutUp', 'slide_out_up', () => effects.slide === 'out_up'),
    bend: {
      label: 'Full bend',
      icon: 'bend',
      keys: 'B',
      active: () => !!effects.bend,
      run: () => setNoteEffect('bend', effects.bend ? null : bendPreset('bend', 4)),
    },
    bendEditor: { label: 'Bend editor…', icon: 'bend', keys: 'Shift+B', disabled: noNote, run: () => (effectDialog = 'bend') },
    vibrato: toggle('Vibrato', 'vibrato', 'vibrato', () => effects.vibrato === 'slight', 'V'),
    wideVibrato: toggle('Wide vibrato', 'wideVibrato', 'wide_vibrato', () => effects.vibrato === 'wide'),
    palmMute: toggle('Palm mute', 'palmMute', 'palm_mute', () => !!effects.palm_mute, 'P'),
    letRing: toggle('Let ring', 'letRing', 'let_ring', () => !!effects.let_ring, 'I'),
    dead: toggle('Dead note', 'dead', 'dead', () => !!effects.dead, 'X'),
    ghost: toggle('Ghost note', 'ghost', 'ghost', () => !!effects.ghost, 'G'),
    staccato: toggle('Staccato', 'staccato', 'staccato', () => !!effects.staccato),
    accent: toggle('Accent', 'accent', 'accent', () => effects.accent === 'normal', 'A'),
    heavyAccent: toggle('Heavy accent', 'heavyAccent', 'heavy_accent', () => effects.accent === 'heavy'),
    harmonicNatural: toggle('Natural harmonic', 'harmonic', 'harmonic_natural', () => effects.harmonic === 'natural', 'N'),
    harmonicArtificial: toggle('Artificial harmonic', 'harmonic', 'harmonic_artificial', () => effects.harmonic === 'artificial'),
    harmonicPinch: toggle('Pinch harmonic', 'harmonic', 'harmonic_pinch', () => effects.harmonic === 'pinch'),
    harmonicTapped: toggle('Tapped harmonic', 'harmonic', 'harmonic_tapped', () => effects.harmonic === 'tapped'),
    harmonicSemi: toggle('Semi-harmonic', 'harmonic', 'harmonic_semi', () => effects.harmonic === 'semi'),
    tapped: toggle('Tapping', 'tapping', 'tapped', () => !!effects.tapped, 'T'),
    trill: { label: 'Trill…', icon: 'trill', active: () => !!effects.trill, disabled: noNote, run: () => (effectDialog = 'trill') },
    grace: { label: 'Grace note…', icon: 'grace', active: () => !!effects.grace, disabled: noNote, run: () => (effectDialog = 'grace') },

    crescendo: toggle('Crescendo', 'crescendo', 'crescendo', () => beatEffects.hairpin === 'crescendo', '<'),
    decrescendo: toggle('Decrescendo', 'decrescendo', 'decrescendo', () => beatEffects.hairpin === 'decrescendo', '>'),
    strokeDown: toggle('Downstroke', 'strokeDown', 'stroke_down', () => beatEffects.stroke === 'down'),
    strokeUp: toggle('Upstroke', 'strokeUp', 'stroke_up', () => beatEffects.stroke === 'up'),
    strumDown: toggle('Strum down', 'strumDown', 'brush_down', () => beatEffects.strum === 'brush_down'),
    strumUp: toggle('Strum up', 'strumUp', 'brush_up', () => beatEffects.strum === 'brush_up'),
    arpeggioDown: toggle('Arpeggio down', 'arpeggioDown', 'arpeggio_down', () => beatEffects.strum === 'arpeggio_down'),
    arpeggioUp: toggle('Arpeggio up', 'arpeggioUp', 'arpeggio_up', () => beatEffects.strum === 'arpeggio_up'),
    rasgueado: toggle('Rasgueado', 'strumDown', 'rasgueado', () => beatEffects.strum === 'rasgueado'),
    tremolo: toggle('Tremolo picking', 'tremolo', 'tremolo', () => !!beatEffects.tremolo),
    slap: toggle('Slap', 'slap', 'slap', () => beatEffects.slap === 'slap'),
    pop: toggle('Pop', 'pop', 'pop', () => beatEffects.slap === 'pop'),
    whammy: { label: 'Whammy bar…', icon: 'whammy', keys: 'W', active: () => !!beatEffects.whammy, run: () => (effectDialog = 'whammy') },

    insertBar: { label: 'Insert bar', icon: 'insertBar', keys: 'Ctrl+Insert', run: () => edit((d) => d.insertBar()) },
    removeBar: { label: 'Remove bar', icon: 'removeBar', run: () => edit((d) => d.removeBar()) },
    timeSignature: { label: 'Time signature…', icon: 'timeSignature', run: () => (dialog = 'time') },
    tuning: { label: 'Tuning and capo…', icon: 'tuning', run: () => (dialog = 'tuning') },
    scoreInfo: { label: 'Title, artist and tempo…', icon: 'scoreInfo', run: () => (dialog = 'score') },

    play: { label: 'Play from the cursor or the selection', icon: 'play', keys: 'Space', run: togglePlay },
    metronome: { label: 'Metronome', icon: 'metronome', active: () => metronome, run: () => (metronome = !metronome) },
    countIn: { label: 'Count-in bar', icon: 'countIn', active: () => countIn, run: () => (countIn = !countIn) },

    zoomIn: { label: 'Zoom in', icon: 'zoomIn', disabled: () => zoom >= ZOOMS[ZOOMS.length - 1], run: () => stepZoom(1) },
    zoomOut: { label: 'Zoom out', icon: 'zoomOut', disabled: () => zoom <= ZOOMS[0], run: () => stepZoom(-1) },
    compact: { label: 'Narrow palette', icon: 'palette', active: () => compact, run: () => (prefs.compact = !compact) },
    inspector: { label: 'Inspector', icon: 'inspector', active: () => inspectorOpen, run: () => (prefs.inspector = !inspectorOpen) },
    shortcuts: { label: 'Keyboard shortcuts', icon: 'shortcuts', keys: '?', run: () => (shortcutsOpen = true) },
  } satisfies Record<string, Command>;

  const themes: Command[] = THEMES.map((theme) => ({
    label: THEME_NAMES[theme],
    icon: THEME_ICONS[theme],
    active: () => prefs.theme === theme,
    run: () => setTheme(theme),
  }));

  function stepZoom(by: number) {
    const at = ZOOMS.indexOf(zoom);
    zoom = ZOOMS[Math.max(0, Math.min(ZOOMS.length - 1, at + by))];
  }

  const palette: PaletteSection[] = [
    { id: 'values', title: 'Note values', commands: [...noteValues, cmd.dot, cmd.triplet, cmd.rest, cmd.tie] },
    {
      id: 'note',
      title: 'On the note',
      commands: [
        cmd.hammer, cmd.slideShift, cmd.slideLegato, cmd.bend,
        cmd.vibrato, cmd.wideVibrato, cmd.palmMute, cmd.letRing,
        cmd.dead, cmd.ghost, cmd.harmonicNatural, cmd.tapped,
        cmd.staccato, cmd.accent, cmd.heavyAccent, cmd.trill, cmd.grace,
      ],
    },
    {
      id: 'beat',
      title: 'On the beat',
      commands: [
        ...dynamics, cmd.crescendo, cmd.decrescendo, cmd.strokeDown, cmd.strokeUp,
        cmd.strumDown, cmd.strumUp, cmd.tremolo, cmd.whammy,
      ],
    },
    {
      id: 'bars',
      title: 'Bars and score',
      commands: [cmd.insertBar, cmd.removeBar, cmd.timeSignature, cmd.tuning, cmd.scoreInfo],
    },
  ];

  const menus = $derived.by((): Menu[] => {
    const recentItems: MenuEntry[] = recent.map((path) => ({
      label: path.split(/[\\/]/).pop() ?? path,
      run: () => openRecent(path),
    }));
    const tracks: MenuEntry[] = (score?.tracks ?? []).map((track, i) => ({
      label: track.name || `Track ${i + 1}`,
      active: () => editor?.cursor.track === i,
      run: () => edit((d) => d.setTrack(i)),
    }));
    return [
      {
        label: 'File',
        items: [
          cmd.new,
          cmd.open,
          ...(recentItems.length ? [{ label: 'Open recent', items: recentItems }] : []),
          '-',
          cmd.save,
          cmd.saveAs,
        ],
      },
      {
        label: 'Edit',
        items: [
          cmd.undo, cmd.redo, '-', cmd.cut, cmd.copy, cmd.paste, '-',
          cmd.insertBeat, cmd.delete, cmd.deleteBeats,
        ],
      },
      {
        label: 'Note',
        items: [
          cmd.rest, cmd.tie, cmd.dot,
          { label: 'Tuplet', items: tuplets(false) },
          { label: 'Tuplet inside the tuplet', items: tuplets(true) },
          '-',
          cmd.hammer,
          {
            label: 'Slide',
            items: [cmd.slideShift, cmd.slideLegato, '-', cmd.slideInBelow, cmd.slideInAbove, cmd.slideOutDown, cmd.slideOutUp],
          },
          cmd.bend,
          cmd.bendEditor,
          cmd.vibrato,
          cmd.wideVibrato,
          {
            label: 'Harmonic',
            items: [cmd.harmonicNatural, cmd.harmonicArtificial, cmd.harmonicPinch, cmd.harmonicTapped, cmd.harmonicSemi],
          },
          '-',
          cmd.palmMute, cmd.letRing, cmd.dead, cmd.ghost, cmd.staccato, cmd.accent, cmd.heavyAccent, cmd.tapped,
          '-',
          cmd.trill,
          cmd.grace,
          { label: 'Fretting-hand finger', items: fingering('left_finger', 'T1234') },
          { label: 'Picking-hand finger', items: fingering('right_finger', 'pimac') },
        ],
      },
      {
        label: 'Beat',
        items: [
          { label: 'Dynamic', items: dynamics },
          cmd.crescendo,
          cmd.decrescendo,
          '-',
          cmd.strokeDown, cmd.strokeUp,
          {
            label: 'Strum',
            items: [cmd.strumDown, cmd.strumUp, cmd.arpeggioDown, cmd.arpeggioUp, cmd.rasgueado],
          },
          cmd.tremolo,
          cmd.slap,
          cmd.pop,
          cmd.whammy,
        ],
      },
      {
        label: 'Score',
        items: [
          cmd.insertBar, cmd.removeBar, cmd.timeSignature, '-', cmd.tuning, cmd.scoreInfo,
          ...(tracks.length > 1 ? ['-' as const, { label: 'Track', items: tracks }] : []),
          '-',
          cmd.play, cmd.metronome, cmd.countIn,
        ],
      },
      {
        label: 'View',
        items: [cmd.zoomIn, cmd.zoomOut, '-', cmd.inspector, cmd.compact, { label: 'Theme', items: themes }],
      },
      { label: 'Help', items: [cmd.shortcuts] },
    ];
  });

  const keyRows = (...commands: Command[]) =>
    commands.filter((c) => c.keys).map((c) => ({ keys: c.keys!, label: c.label.replace(/…$/, '') }));

  const shortcutGroups: ShortcutGroup[] = [
    {
      title: 'Moving',
      rows: [
        { keys: '← → ↑ ↓', label: 'Move between beats and strings' },
        { keys: 'Shift+arrows', label: 'Extend the selection' },
        { keys: 'Ctrl+← / Ctrl+→', label: 'Previous or next bar' },
        { keys: 'Home / End', label: 'First or last bar' },
      ],
    },
    {
      title: 'Entering notes',
      rows: [
        { keys: '0–9', label: 'Fret; two digits typed quickly make one number' },
        { keys: '+ / −', label: 'Shorter or longer note value' },
        ...keyRows(cmd.dot, cmd.triplet, cmd.rest, cmd.tie, cmd.insertBeat, cmd.delete, cmd.deleteBeats, cmd.insertBar),
      ],
    },
    {
      title: 'On the note',
      rows: keyRows(
        cmd.hammer, cmd.slideShift, cmd.bend, cmd.bendEditor, cmd.vibrato, cmd.palmMute, cmd.letRing,
        cmd.dead, cmd.ghost, cmd.accent, cmd.harmonicNatural, cmd.tapped,
      ),
    },
    { title: 'On the beat', rows: keyRows(cmd.crescendo, cmd.decrescendo, cmd.whammy) },
    {
      title: 'File and edit',
      rows: [
        ...keyRows(cmd.new, cmd.open, cmd.save, cmd.saveAs, cmd.undo),
        { keys: 'Ctrl+Y / Ctrl+Shift+Z', label: 'Redo' },
        ...keyRows(cmd.cut, cmd.copy, cmd.paste),
      ],
    },
    { title: 'Playback', rows: [{ keys: 'Space', label: 'Play from the cursor or the selection; stop' }] },
    {
      title: 'Interface',
      rows: [
        { keys: '? / F1', label: 'This list' },
        { keys: 'Tab / Shift+Tab', label: 'Move between the bars, palette and score' },
        { keys: 'Arrows', label: 'Move within a bar, the palette or a menu' },
        { keys: 'Esc', label: 'Back to the score' },
      ],
    },
  ];

  // --- Keyboard ---

  function onKeydown(event: KeyboardEvent) {
    if (event.defaultPrevented || dialog || effectDialog || shortcutsOpen || !doc) return;
    const target = event.target instanceof Element ? event.target : null;
    if (target?.closest('input, select, textarea, [role="menubar"]')) return;
    if (target?.closest('[data-roving]')) {
      if (event.key === 'Escape') return focusScore();
      if (WIDGET_KEYS.includes(event.key) && !event.ctrlKey && !event.metaKey) return;
    } else if (target?.closest('button') && (event.key === ' ' || event.key === 'Enter')) {
      return;
    }

    const action = keyAction(event);
    if (!action) return;
    event.preventDefault();
    action();
  }

  function keyAction(event: KeyboardEvent): (() => unknown) | undefined {
    const { key, shiftKey: shift } = event;
    const ctrl = event.ctrlKey || event.metaKey;
    const move = (direction: string) => () => edit((d) => d.moveCursor(direction, shift));

    if (ctrl) {
      switch (key.toLowerCase()) {
        case 'z': return shift ? cmd.redo.run : cmd.undo.run;
        case 'y': return cmd.redo.run;
        case 'c': return cmd.copy.run;
        case 'x': return cmd.cut.run;
        case 'v': return cmd.paste.run;
        case 's': return () => save(shift);
        case 'o': return open;
        case 'n': return newScore;
        case 'arrowleft': return move('prev_bar');
        case 'arrowright': return move('next_bar');
        case 'home': return move('first');
        case 'end': return move('last');
        case 'delete': return cmd.deleteBeats.run;
        case 'insert': return cmd.insertBar.run;
      }
      return undefined;
    }
    if (event.altKey) return undefined;

    if (/^[0-9]$/.test(key)) {
      return () => {
        const now = performance.now();
        const combine = now - lastDigitAt < DIGIT_WINDOW_MS;
        lastDigitAt = now;
        edit((d) => d.typeDigit(Number(key), combine));
      };
    }
    // Letters work the same with Caps Lock on.
    switch (key.length === 1 ? key.toLowerCase() : key) {
      case 'ArrowLeft': return move('left');
      case 'ArrowRight': return move('right');
      case 'ArrowUp': return move('up');
      case 'ArrowDown': return move('down');
      case 'Home': return move('first');
      case 'End': return move('last');
      case 'Delete':
      case 'Backspace': return cmd.delete.run;
      case 'Insert': return cmd.insertBeat.run;
      case '+':
      case '=': return () => edit((d) => d.shorter());
      case '-': return () => edit((d) => d.longer());
      case '.': return cmd.dot.run;
      case '/': return cmd.triplet.run;
      case 'r': return cmd.rest.run;
      case 'l': return cmd.tie.run;
      case 'h': return cmd.hammer.run;
      case 's': return cmd.slideShift.run;
      case 'b': return shift ? () => noNote() || cmd.bendEditor.run() : cmd.bend.run;
      case 'w': return cmd.whammy.run;
      case '<': return cmd.crescendo.run;
      case '>': return cmd.decrescendo.run;
      case 'v': return cmd.vibrato.run;
      case 'p': return cmd.palmMute.run;
      case 'i': return cmd.letRing.run;
      case 'x': return cmd.dead.run;
      case 'g': return cmd.ghost.run;
      case 'a': return cmd.accent.run;
      case 'n': return cmd.harmonicNatural.run;
      case 't': return cmd.tapped.run;
      case '?':
      case 'F1': return cmd.shortcuts.run;
      case ' ': return togglePlay;
    }
    return undefined;
  }

  onMount(() => {
    run(async () => {
      await init();
      const autosave = readAutosave();
      if (autosave) {
        try {
          const recovered = Document.load(autosave.bytes);
          recovered.markUnsaved();
          setDocument(recovered, autosave.name, autosave.path);
          message = `Recovered unsaved changes to ${autosave.name}`;
          return;
        } catch {
          writeAutosave(null);
        }
      }
      setDocument(new Document(), UNTITLED, null);
    });
    return () => {
      player.stop();
      doc?.free();
    };
  });
</script>

<svelte:window onkeydown={onKeydown} />

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="app"
  bind:clientWidth={width}
  ondragover={(e) => e.preventDefault()}
  ondrop={onDrop}
>
  <!-- The two bars share a row when the window is wide enough and stack when it is not. -->
  <header class="bars">
    <div class="topbar">
      <strong class="brand">gtrTab</strong>
      <MenuBar {menus} onclose={focusScore} />
      <span class="rule"></span>
      <div class="cluster" role="toolbar" aria-label="File and edit" use:roving>
        <ToolButton command={cmd.new} />
        <ToolButton command={cmd.open} />
        <ToolButton command={cmd.save} />
        <span class="rule"></span>
        <ToolButton command={cmd.undo} />
        <ToolButton command={cmd.redo} />
      </div>
      <span class="file" class:dirty={editor?.dirty}>{fileName}</span>
    </div>

    {#if score && editor}
      <div class="transport" role="toolbar" aria-label="Playback and view" use:roving>
        <button
          type="button"
          class="tool play"
          aria-label={playing ? 'Stop' : cmd.play.label}
          aria-keyshortcuts="Space"
          use:tip={{ label: playing ? 'Stop' : cmd.play.label, keys: 'Space' }}
          onmousedown={(e) => e.preventDefault()}
          onclick={togglePlay}
        >
          <Icon name={playing ? 'stop' : 'play'} size={16} />
        </button>
        <ToolButton command={cmd.metronome} />
        <ToolButton command={cmd.countIn} />
        <button
          type="button"
          class="chip"
          aria-label="Playback speed: {speed * 100}%"
          use:tip={{ label: 'Playback speed' }}
          onmousedown={(e) => e.preventDefault()}
          onclick={() => (speed = SPEEDS[(SPEEDS.indexOf(speed) + 1) % SPEEDS.length])}
        >
          <Icon name="speed" size={16} />
          {speed * 100}%
        </button>
        <button
          type="button"
          class="chip"
          aria-label="Tempo: {score.tempo} beats per minute"
          use:tip={{ label: 'Tempo' }}
          onmousedown={(e) => e.preventDefault()}
          onclick={cmd.scoreInfo.run}
        >
          <Icon name="tempo" size={16} />
          = {score.tempo}
        </button>
        {#if score.tracks.length > 1}
          <select
            aria-label="Track"
            use:tip={{ label: 'Track' }}
            bind:value={() => editor!.cursor.track, (track) => edit((d) => d.setTrack(track))}
          >
            {#each score.tracks as track, i}
              <option value={i}>{track.name || `Track ${i + 1}`}</option>
            {/each}
          </select>
        {/if}
        <span class="rule"></span>
        <ToolButton command={cmd.zoomOut} />
        <span class="zoom" aria-label="Zoom {zoom * 100}%">{zoom * 100}%</span>
        <ToolButton command={cmd.zoomIn} />
        <span class="rule"></span>
        <ToolButton command={cmd.compact} />
        <ToolButton command={cmd.inspector} />
        <ToolButton command={cmd.shortcuts} />
        <button
          type="button"
          class="tool"
          aria-label="Theme: {THEME_NAMES[prefs.theme]}"
          use:tip={{ label: `Theme: ${THEME_NAMES[prefs.theme]}` }}
          onmousedown={(e) => e.preventDefault()}
          onclick={() => setTheme(THEMES[(THEMES.indexOf(prefs.theme) + 1) % THEMES.length])}
        >
          <Icon name={THEME_ICONS[prefs.theme]} />
        </button>
      </div>
    {/if}
  </header>

  {#if score && editor && doc}
    <div class="work">
      <Palette sections={palette} {compact} collapsed={prefs.collapsed} ontoggle={toggleSection} />
      <div class="stage">
        <ScoreView
          getLayout={(pageWidth) => JSON.parse(doc!.layoutJson(pageWidth)) as Layout}
          revision={`${editor.revision}:${editor.cursor.track}:${fileName}`}
          {editor}
          {playhead}
          {zoom}
          theme={prefs.theme}
          onpick={(bar, beat, string, extend) => edit((d) => d.setCursor(bar, beat, string, extend))}
        />
        <!-- Room for the track list and mixer. Empty, and so collapsed, until phase 5. -->
        <div class="dock"></div>
      </div>
      {#if inspectorOpen}
        <div class="side" class:floating={narrow}>
          <Inspector
            {score}
            {editor}
            {technique}
            {setNoteEffect}
            {setBeatEffect}
            openEffect={(kind) => (effectDialog = kind)}
            openDialog={(kind) => (dialog = kind)}
          />
        </div>
      {/if}
    </div>
  {:else}
    <main>{failed ? '' : 'Loading…'}</main>
  {/if}

  <footer class:failed>
    <span role="status">{message || 'Press ? for the keyboard shortcuts'}</span>
    {#if editor && score}
      <span>Bar {editor.cursor.bar + 1} of {score.master_bars.length} · Beat {editor.cursor.beat + 1}</span>
    {/if}
  </footer>
</div>

{#if dialog && score && editor}
  <Dialogs
    kind={dialog}
    {score}
    track={editor.cursor.track}
    bar={editor.cursor.bar}
    onscore={(info, tempo) => {
      dialog = null;
      edit((d) => {
        const old = score!;
        const changed =
          info.title !== old.info.title || info.artist !== old.info.artist || info.album !== old.info.album;
        if (changed) d.setInfo(info.title, info.artist, info.album);
        if (tempo !== old.tempo) d.setTempo(tempo);
      });
    }}
    ontime={(numerator, denominator) => {
      dialog = null;
      edit((d) => d.setTimeSignature(numerator, denominator));
    }}
    ontuning={(strings, capo) => {
      dialog = null;
      edit((d) => d.setTuning(new Uint8Array(strings), capo));
    }}
    onclose={() => (dialog = null)}
  />
{/if}

{#if effectDialog && editor}
  <EffectDialog
    kind={effectDialog}
    note={editor.note}
    beatEffects={editor.beat_effects}
    onapply={(value) => {
      // Read which effect this is before closing the dialog clears it.
      const kind = effectDialog;
      effectDialog = null;
      if (kind === 'whammy') setBeatEffect('whammy', value);
      else if (kind) setNoteEffect(kind, value);
    }}
    onclose={() => (effectDialog = null)}
  />
{/if}

{#if shortcutsOpen}
  <Shortcuts groups={shortcutGroups} onclose={() => (shortcutsOpen = false)} />
{/if}

<style>
  .app {
    display: flex;
    flex-direction: column;
    height: 100%;
  }

  .bars {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0 var(--space-4);
    padding: var(--space-2) var(--space-3);
    border-bottom: 1px solid var(--border);
    background: var(--panel);
  }

  .topbar,
  .transport,
  .cluster {
    display: flex;
    align-items: center;
    gap: var(--space-1);
  }

  /* Sharing a row, the top bar takes the spare width; alone, each bar fills its row. */
  .topbar {
    flex: 1000 1 auto;
    min-width: 0;
  }

  .transport {
    flex: 1 1 auto;
    flex-wrap: wrap;
  }

  .brand {
    padding: 0 var(--space-3) 0 var(--space-2);
    color: var(--accent);
  }

  .rule {
    flex: none;
    width: 1px;
    height: 18px;
    margin: 0 var(--space-2);
    background: var(--border);
  }

  .file {
    flex: 1;
    min-width: 0;
    padding: 0 var(--space-3);
    overflow: hidden;
    color: var(--muted);
    font-size: var(--text-sm);
    text-align: center;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .file.dirty::before {
    content: '• ';
    color: var(--accent);
  }

  .tool.play {
    margin-right: var(--space-2);
    background: var(--accent);
    color: var(--on-accent);
  }

  .tool.play:hover {
    background: color-mix(in srgb, var(--accent), var(--text) 12%);
  }

  .chip {
    display: inline-flex;
    align-items: center;
    gap: var(--space-2);
    height: var(--control);
    padding: 0 var(--space-3) 0 var(--space-2);
    border: 0;
    border-radius: var(--radius-s);
    background: none;
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
    cursor: pointer;
  }

  .chip:hover {
    background: var(--hover);
  }

  .transport select {
    max-width: 150px;
  }

  .zoom {
    min-width: 40px;
    color: var(--muted);
    font-size: var(--text-sm);
    font-variant-numeric: tabular-nums;
    text-align: center;
  }

  .work {
    position: relative;
    display: flex;
    flex: 1;
    min-height: 0;
  }

  .stage {
    display: flex;
    flex: 1;
    flex-direction: column;
    min-width: 0;
  }

  .dock:empty {
    display: none;
  }

  .side {
    display: flex;
  }

  /* In a narrow window the inspector lies over the score instead of squeezing it. */
  .side.floating {
    position: absolute;
    top: 0;
    right: 0;
    bottom: 0;
    box-shadow: var(--shadow);
  }

  main {
    flex: 1;
    padding: var(--space-6);
    color: var(--muted);
  }

  footer {
    display: flex;
    justify-content: space-between;
    gap: var(--space-5);
    padding: var(--space-2) var(--space-4);
    border-top: 1px solid var(--border);
    background: var(--panel);
    color: var(--muted);
    font-size: var(--text-sm);
    white-space: nowrap;
  }

  footer span:first-child {
    overflow: hidden;
    text-overflow: ellipsis;
  }

  footer.failed span:first-child {
    color: var(--error);
  }
</style>
