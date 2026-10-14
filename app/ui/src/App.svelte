<script lang="ts">
  import { onMount, untrack } from 'svelte';
  import type { Command, Menu, MenuEntry, PaletteSection, ShortcutGroup } from './lib/commands';
  import ChordDialog from './lib/ChordDialog.svelte';
  import Dialogs, { type DialogKind } from './lib/Dialogs.svelte';
  import EffectDialog, { bendPreset, type EffectKind } from './lib/EffectDialog.svelte';
  import {
    EXTENSION,
    addRecentFile,
    confirmDialog,
    exportFile,
    canPickFolder,
    baseName,
    openAudioFile,
    openFile,
    pickFolder,
    writeInto,
    readAutosave,
    autosaveRecording,
    keepAutosaveRecording,
    readPath,
    recentFiles,
    removeRecentFile,
    saveFile,
    setWindowTitle,
    writeAutosave,
    reopenAfterRestart,
    takeReopenAfterRestart,
    type OpenedFile,
  } from './lib/files';
  import { PICTURE_WIDTH, exportFonts, paper, printPages, type Fonts } from './lib/export';
  import Inspector from './lib/Inspector.svelte';
  import {
    FADES,
    FEELS,
    JUMPS,
    MAX_VOICES,
    NOTATIONS,
    TARGETS,
    NOTE_VALUES,
    type DrumPiece,
    type Pulse,
    type Dynamic,
    type Jump,
    type Target,
    type EditorState,
    type Layout,
    type Style,
    type AudioTrack,
    type Instrument,
    type Mix,
    type Pedal,
    type Score,
    type Timeline,
  } from './lib/model';
  import BeatLights from './lib/BeatLights.svelte';
  import InstrumentView, { type Marks } from './lib/InstrumentView.svelte';
  import Pedalboard from './lib/Pedalboard.svelte';
  import AudioPanel, { peaksOf, type Peaks, type RecordingBar } from './lib/AudioPanel.svelte';
  import SoundDialog from './lib/SoundDialog.svelte';
  import LineInDialog, { loadLineSettings, saveLineSettings, type LineSettings } from './lib/LineInDialog.svelte';
  import { LineIn, type Levels } from './lib/linein';
  import {
    loadUserPresets,
    saveUserPresets,
    withPreset,
    type PedalPreset,
    type PedalSpec,
  } from './lib/pedals';
  import ChordLibrary from './lib/ChordLibrary.svelte';
  import ScaleDialog from './lib/ScaleDialog.svelte';
  import TunerDialog from './lib/TunerDialog.svelte';
  import { SCALES, findScales, type Fit } from './lib/scales';
  import { freeLine } from './lib/instrument';
  import Palette from './lib/Palette.svelte';
  import PromptDialog, { type Prompt } from './lib/PromptDialog.svelte';
  import TrainerDialog from './lib/TrainerDialog.svelte';
  import { nextSpeed, type Trainer } from './lib/trainer';
  import KitDialog from './lib/KitDialog.svelte';
  import { Player, decodeAt, interleaved, mixOf } from './lib/player';
  import {
    DEFAULT_SOUNDFONT,
    fetchDefaultSoundFont,
    pickSoundFont,
    storeSoundFont,
    storedSoundFont,
  } from './lib/soundfont';
  import { THEMES, applyLook, applyTheme, defaultPrefs, loadPrefs, savePrefs, type Prefs, type Theme } from './lib/prefs';
  import Preferences from './lib/Preferences.svelte';
  import StyleDialog from './lib/StyleDialog.svelte';
  import ScoreView from './lib/ScoreView.svelte';
  import Shortcuts from './lib/Shortcuts.svelte';
  import CommandPalette from './lib/CommandPalette.svelte';
  import { listCommands, used, type Found } from './lib/search';
  import { chordOf, keymap } from './lib/keymap';
  import { aria, keyOf, label as keyLabel, shortcuts, shownKeys } from './lib/keys.svelte';
  import TrackPane, { type AudioRow } from './lib/TrackPane.svelte';
  import { describeCursor } from './lib/describe';
  import Icon from './lib/ui/Icon.svelte';
  import type { IconName } from './lib/ui/icons';
  import MenuBar from './lib/ui/MenuBar.svelte';
  import { roving } from './lib/ui/roving';
  import ToolButton from './lib/ui/ToolButton.svelte';
  import { tip } from './lib/ui/tooltip';
  import init, { Bundle, Document } from './wasm/gtr_wasm.js';
  import BatchDialog from './lib/BatchDialog.svelte';
  import { TARGET_EXTENSIONS, leftOut, outputName, type BatchTarget, type Converted } from './lib/batch';
  import { t, tn } from './lib/i18n';
  import { onLaunch } from './lib/webfiles';

  const UNTITLED = `untitled.${EXTENSION}`;
  const VALUE_NAMES = [
    t('Whole note'), t('Half note'), t('Quarter note'), t('Eighth note'), t('16th note'),
    t('32nd note'), t('64th note'), t('128th note'), t('256th note'),
  ];
  const DYNAMICS: [Dynamic, string][] = [
    ['ppp', t('Pianississimo')],
    ['pp', t('Pianissimo')],
    ['p', t('Piano')],
    ['mp', t('Mezzo-piano')],
    ['mf', t('Mezzo-forte')],
    ['f', t('Forte')],
    ['ff', t('Fortissimo')],
    ['fff', t('Fortississimo')],
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
  const FINGER_NAMES = [t('thumb'), t('index'), t('middle'), t('ring'), t('pinky')];
  /** Playback speeds offered by name, as percentages of the tempo written. */
  const SPEEDS = [25, 50, 75, 100, 125, 150, 200];
  const [MIN_SPEED, MAX_SPEED] = [25, 200];
  /** How far the slower and faster keys move the speed. */
  const SPEED_STEP = 5;
  const ZOOMS = [0.5, 0.75, 1, 1.25, 1.5, 2];
  const THEME_NAMES: Record<Theme, string> = { system: t('System'), light: t('Light'), dark: t('Dark') };
  const THEME_ICONS: Record<Theme, IconName> = {
    system: 'themeSystem',
    light: 'themeLight',
    dark: 'themeDark',
  };
  /** Longest pause between two digits that still makes one fret number. */
  const DIGIT_WINDOW_MS = 900;
  const AUTOSAVE_DELAY_MS = 1500;
  /** How long after an edit the passage being played is handed to the synth again. */
  const EDIT_PLAYBACK_DELAY_MS = 60;
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
  let prompt = $state.raw<Prompt | null>(null);
  let chordOpen = $state(false);
  let styleOpen = $state(false);
  /** The pieces of the drum kit, read from the core once it has loaded. */
  /** The kit every drum track starts with. */
  let standardKit = $state.raw<DrumPiece[]>([]);
  let kitDialog = $state(false);
  let shortcutsOpen = $state(false);
  let paletteOpen = $state(false);
  let prefsOpen = $state(false);
  let batchOpen = $state(false);
  let zoom = $state(1);

  let playing = $state(false);
  let playhead = $state.raw<{ bar: number; beat: number } | null>(null);
  /** The pulse of the bar sounding, for the visual metronome. */
  let pulse = $state.raw<Pulse | null>(null);
  /** First bar visible in the score, which the track pane follows. */
  let viewBar = $state(0);
  let metronome = $state(false);
  let countIn = $state(false);
  /** Play the repeats again after a D.C. or D.S., instead of passing through them. */
  let replayRepeats = $state(false);
  /** How fast the score is played, as a percentage of the tempo written. */
  let speed = $state(100);
  /** Play the passage again and again until stopped. */
  let loop = $state(false);
  /** How the speed trainer raises the speed, and whether it is switched on. */
  let trainer = $state<Trainer>({ step: 5, every: 1, target: 100 });
  let training = $state(false);
  let trainerOpen = $state(false);

  const prefs = $state(loadPrefs());
  let width = $state(0);
  const narrow = $derived(width > 0 && width < NARROW_WIDTH);
  const compact = $derived(prefs.compact ?? narrow);
  const inspectorOpen = $derived(prefs.inspector ?? width >= ROOMY_WIDTH);

  let revision = -1;
  let lastDigitAt = 0;

  $effect(() => savePrefs($state.snapshot(prefs)));
  /** The language in use, which a change in the preferences only switches to on a new start. */
  const languageAtStart = untrack(() => prefs.language);
  /** Goes up when the system's contrast changes, so that the canvases are painted again. */
  let lookChanges = $state(0);
  // Before the canvases repaint for the change, which their own effects do first.
  $effect.pre(() => applyLook(prefs.contrast, prefs.paper, () => lookChanges++));
  /** Changes with everything the score's colours depend on. */
  const look = $derived(`${prefs.theme} ${prefs.contrast} ${prefs.paper} ${lookChanges}`);

  // What the menus, tooltips and palette show beside each command follows the keys given.
  $effect.pre(() => {
    shortcuts.custom = prefs.shortcuts;
  });

  function setTheme(theme: Theme) {
    prefs.theme = theme;
    applyTheme(theme);
  }

  /** Sets one preference from the preferences dialog. */
  function setPref<K extends keyof Prefs>(key: K, value: Prefs[K]) {
    if (key === 'theme') setTheme(value as Theme);
    else prefs[key] = value;
  }

  /** Every preference as it was when the app came, except the keys given and the commands used lately. */
  function resetPrefs() {
    const { shortcuts: keys, recentCommands, sound } = prefs;
    Object.assign(prefs, { ...defaultPrefs(), shortcuts: keys, recentCommands });
    applyTheme(prefs.theme);
    if (sound !== prefs.sound) useSound('default');
    message = t('Preferences as they were when gtrTab came');
  }

  /**
   * Starts the app again, for a change of language, with the score as it is: work not saved
   * is kept as it is in case the app closes, and a score that is saved is opened again.
   */
  function restart() {
    if (doc && editor) {
      if (editor.dirty || !filePath) {
        keepCopy(doc, !editor.dirty);
      } else {
        dropCopy();
        reopenAfterRestart(filePath);
      }
    }
    location.reload();
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
    return !editor?.dirty || confirmDialog(t('Discard unsaved changes to {name}?', { name: fileName }));
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
    message = report
      ? t('Imported {name}. Not imported: {report}.', { name: file.name, report })
      : t('Imported {name}.', { name: file.name });
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
      message = t('Saved {name}', { name: saved.name });
    });

  /**
   * The recording kept beside the copy of unsaved work, by its name and size, or `''`
   * for none: it is big, so it is written only when it changes, not with every edit.
   */
  let keptRecording = '';

  /** Keeps a copy of the work as it is, in case the app closes before it is saved. */
  function keepCopy(current: Document, saved = false) {
    writeAutosave({ name: fileName, path: filePath, bytes: current.saveWithoutRecording(), saved });
    const size = score?.audio ? current.recordingSize : 0;
    const key = size ? `${score!.audio!.name}:${size}` : '';
    if (key === keptRecording) return;
    keptRecording = key;
    void keepAutosaveRecording(key ? { name: score!.audio!.name, bytes: current.recordingFile()! } : null);
  }

  /** Lets go of the copy of the work, which is saved or not wanted. */
  function dropCopy() {
    writeAutosave(null);
    if (!keptRecording) return;
    keptRecording = '';
    void keepAutosaveRecording(null);
  }

  // Keep a copy of unsaved work so that a crash or closed window does not lose it.
  $effect(() => {
    // Before a document is open there is nothing to save, and a copy left by the last
    // session is still waiting to be recovered.
    if (!editor) return;
    if (!editor.dirty) {
      dropCopy();
      return;
    }
    void score;
    const timer = setTimeout(() => {
      if (doc) keepCopy(doc);
    }, AUTOSAVE_DELAY_MS);
    return () => clearTimeout(timer);
  });

  $effect(() => {
    const title = `${editor?.dirty ? '• ' : ''}${fileName} — gtrTab`;
    setWindowTitle(title).catch(() => {});
  });

  // --- Playback ---

  /** What is playing: the options it was compiled with, but for the speed and the loop, and the track being followed. */
  let passage: { options: object; track: number } | null = null;
  /** Times round the loop, counting the one playing. */
  let pass = 0;
  /** The speed of the version of the passage queued for the next time round, if any. */
  let queuedSpeed: number | null = null;

  /** Compiles the passage being played from the score as it now stands, at `percent` of the tempo. */
  function compileAt(doc: Document, { options, track }: { options: object; track: number }, percent: number) {
    const compiled = doc.compileAudio(JSON.stringify({ ...options, speed: percent / 100, looped: loop }), track);
    const timeline: Timeline = JSON.parse(compiled.timelineJson);
    const events = compiled.events;
    compiled.free();
    return { events, timeline };
  }

  /** Compiles the passage being played at the speed now set. */
  function compilePassage(doc: Document, played: { options: object; track: number }) {
    playedRevision = revision;
    playedSpeed = speed;
    playedLoop = loop;
    return compileAt(doc, played, speed);
  }

  /**
   * Hands the synth the passage to go on to when the loop next comes round: faster if the
   * speed trainer says so, and otherwise the same again.
   */
  function queueNext() {
    if (!playing || !doc || !passage) return;
    const next = training && loop ? nextSpeed(trainer, speed, pass) : speed;
    if (next === speed) {
      if (queuedSpeed !== null) void player.queue(null);
      queuedSpeed = null;
      return;
    }
    queuedSpeed = next;
    void player.queue(compileAt(doc, passage, next));
  }

  /** The loop has come round, and is played at the speed queued for it if it went on to that. */
  function onPass(fromQueue: boolean) {
    pass += 1;
    if (fromQueue && queuedSpeed !== null) {
      speed = playedSpeed = queuedSpeed;
      message = t('Time {pass} round, at {speed}% of the tempo', { pass, speed });
    }
    queuedSpeed = null;
    queueNext();
  }

  const play = () =>
    run(async () => {
      if (!doc || !editor) return;
      const selection = editor.selection;
      const options = {
        from_bar: selection ? selection[0][0] : editor.cursor.bar,
        to_bar: selection ? selection[1][0] + 1 : null,
        metronome,
        count_in: countIn,
        replay_repeats: replayRepeats,
        staff: editor.cursor.staff,
        voice: editor.cursor.voice,
      };
      passage = { options, track: editor.cursor.track };
      const { events, timeline } = compilePassage(doc, passage);
      pass = 1;
      queuedSpeed = null;
      const ended = () => {
        playing = false;
        playhead = null;
        pulse = null;
      };
      await player.play(events, timeline, ended, onPass);
      playing = true;
      queueNext();
      follow();
    });

  function follow() {
    if (!playing) return;
    const now = player.position();
    if (now && (now.bar !== playhead?.bar || now.beat !== playhead?.beat)) {
      playhead = { bar: now.bar, beat: now.beat };
    }
    const beat = player.pulse();
    if (beat?.time !== pulse?.time) pulse = beat;
    recordingNow = player.recordingTime();
    requestAnimationFrame(follow);
  }

  /** Stops playback, leaving the cursor where the music stopped. */
  function stop() {
    if (!playing) return;
    player.stop();
    playing = false;
    const at = playhead;
    playhead = null;
    pulse = null;
    recordingNow = null;
    if (at && doc && editor) {
      doc.setCursor(at.bar, at.beat, editor.cursor.string, false);
      refresh();
    }
  }

  const togglePlay = () => (playing ? stop() : play());

  function setSpeed(percent: number) {
    speed = Math.max(MIN_SPEED, Math.min(MAX_SPEED, Math.round(percent)));
    message = t('Playing at {speed}% of the tempo', { speed });
  }

  // Volume, pan, mute and solo take effect on what is already playing.
  $effect(() => {
    if (!playing || !score) return;
    const mix = mixOf(score.tracks, score.audio);
    player.setMix(mix.slice(0, score.tracks.length), mix[score.tracks.length]);
  });

  // So do an edit, a change of speed and the loop switched on or off: the synth is handed
  // the passage again and carries on from the beat it had reached. What a loop goes on to
  // next is made again to match.
  let playedRevision = -1;
  let playedSpeed = 100;
  let playedLoop = false;
  $effect(() => {
    if (!playing || !score || !doc || !passage) return;
    if (revision === playedRevision && speed === playedSpeed && loop === playedLoop) return;
    const timer = setTimeout(() => {
      if (!doc || !passage) return;
      const { events, timeline } = compilePassage(doc, passage);
      void player.update(events, timeline);
      queueNext();
    }, EDIT_PLAYBACK_DELAY_MS);
    return () => clearTimeout(timer);
  });

  // The speed trainer switched on or off, or set differently, changes what comes next.
  $effect(() => {
    void [training, trainer];
    untrack(queueNext);
  });

  function setLoop(on: boolean) {
    loop = on;
    if (!on) training = false;
    message = on ? 'Looping' : 'Not looping';
  }

  function setTrainer(next: Trainer | null) {
    training = next !== null;
    if (!next) {
      message = t('Speed trainer off');
      return;
    }
    trainer = next;
    loop = true;
    message =
      speed >= next.target
        ? t('Already at {speed}%; the speed trainer goes up to {target}%', { speed, target: next.target })
        : tn(
            next.every,
            'Speed trainer: from {speed}% up to {target}%, {step}% faster each time round',
            'Speed trainer: from {speed}% up to {target}%, {step}% faster every {count} times round',
            { speed, target: next.target, step: next.step },
          );
  }

  // --- Sound ---

  /** Name of the SoundFont the user picked, once it is known. */
  let customSound = $state('');

  /**
   * Has the synth play through what the preferences ask for. If that cannot be had, it
   * plays through the next best thing: the SoundFont that ships with the app, and failing
   * that its own models, which need no file. Returns what stood in the way, if anything.
   */
  async function applySound(): Promise<string | null> {
    if (prefs.sound === 'custom') {
      const stored = await storedSoundFont();
      if (stored && !(await player.setSoundFont(stored.bytes))) {
        customSound = stored.name;
        return null;
      }
      prefs.sound = 'default';
      return t('The SoundFont picked before is no longer there.');
    }
    if (prefs.sound === 'default') {
      const bytes = await fetchDefaultSoundFont();
      if (bytes && !(await player.setSoundFont(bytes))) return null;
      await player.setSoundFont(null);
      return t('{name} is not part of this build.', { name: DEFAULT_SOUNDFONT.name });
    }
    await player.setSoundFont(null);
    return null;
  }

  const useSound = (sound: 'default' | 'models') =>
    run(async () => {
      prefs.sound = sound;
      const problem = await applySound();
      if (problem) throw new Error(t('{problem} Playing through the built-in models.', { problem }));
      message =
        sound === 'default'
          ? t('Playing through {name}', { name: DEFAULT_SOUNDFONT.name })
          : t('Playing through the built-in models');
    });

  /** The bytes of the SoundFont the preferences ask for, or `null` for the built-in models. */
  async function soundBytes(): Promise<ArrayBuffer | null> {
    if (prefs.sound === 'custom') return (await storedSoundFont())?.bytes ?? null;
    return prefs.sound === 'default' ? fetchDefaultSoundFont() : null;
  }

  /** True while the score is being made into a sound file. */
  let exporting = $state(false);

  /**
   * Writes the whole score to a WAV file as it plays: every track that is not muted,
   * with its repeats, through the sound chosen for playback, and the audio track's
   * recording unless it is muted. The sound is made a few seconds at a time, so that the
   * window goes on answering and can say how far it is.
   */
  const exportAudio = () =>
    run(async () => {
      if (!doc || exporting) return;
      exporting = true;
      try {
        const RATE = 44_100;
        const options = JSON.stringify({ from_bar: 0, to_bar: null, replay_repeats: replayRepeats, speed: 1 });
        const font = await soundBytes();
        const recording = score?.audio ? doc.recordingFile() : undefined;
        const samples = recording && interleaved(await decodeAt(recording, RATE));
        const making = doc.exportAudio(options, RATE, font ? new Uint8Array(font) : undefined, samples);
        try {
          for (let done = 0; done < 1; ) {
            done = making.step(RATE * 4);
            message = t('Exporting audio: {percent}%', { percent: Math.round(done * 100) });
            await new Promise((next) => setTimeout(next));
          }
          const name = fileName.replace(/\.[^.]+$/, '') + '.wav';
          const written = await exportFile(name, making.wav(), 'WAV audio', 'audio/wav');
          message = written ? t('Exported {name}', { name: written }) : '';
        } finally {
          making.free();
        }
      } finally {
        exporting = false;
      }
    });

  /** The formats of other programs a score can be written in, as the save dialog names them. */
  const FORMATS = {
    guitar_pro: 'Guitar Pro file',
    musicxml: 'MusicXML file',
    midi: 'MIDI file',
    ascii: 'Text file',
  };

  /** The score's file name with another ending. */
  const named = (extension: string) => fileName.replace(/\.[^.]+$/, '') + '.' + extension;

  /**
   * Writes the score as another program's file. What the score holds that the format has
   * no place for is said in the status bar.
   */
  const exportAs = (format: keyof typeof FORMATS) =>
    run(async () => {
      if (!doc) return;
      const written = doc.exportFile(format);
      try {
        const report = written.report;
        const saved = await exportFile(named(written.extension), written.bytes, FORMATS[format], written.mediaType);
        if (saved) {
          message = report
            ? t('Exported {name}. Not exported: {report}.', { name: saved, report })
            : t('Exported {name}', { name: saved });
        }
      } finally {
        written.free();
      }
    });

  /**
   * Writes the score as something to look at, in the view it is shown in: a PDF of its
   * pages, or the whole of it as one SVG or PNG picture.
   */
  const exportPicture = (kind: 'pdf' | 'svg' | 'png') =>
    run(async () => {
      if (!doc) return;
      const fonts = await exportFonts();
      const [bytes, what, type] =
        kind === 'pdf'
          ? [doc.exportPdf(viewJson, paper, ...fonts), 'PDF document', 'application/pdf']
          : kind === 'svg'
            ? [new TextEncoder().encode(doc.exportSvg(viewJson, PICTURE_WIDTH, ...fonts)), 'SVG picture', 'image/svg+xml']
            : [doc.exportPng(viewJson, PICTURE_WIDTH, 2, ...fonts), 'PNG picture', 'image/png'];
      const saved = await exportFile(named(kind), bytes, what, type);
      if (saved) message = t('Exported {name}', { name: saved });
    });

  /**
   * Writes a score opened for conversion in a format, as `exportAs` and `exportPicture`
   * do for the score being edited. Resolves to its bytes and what the format left out.
   */
  function converted(from: Document, target: BatchTarget, fonts: Fonts | null): [Uint8Array, string] {
    switch (target) {
      case 'gtrtab':
        return [from.save(), ''];
      case 'pdf':
        return [from.exportPdf(viewJson, paper, ...fonts!), ''];
      case 'svg':
        return [new TextEncoder().encode(from.exportSvg(viewJson, PICTURE_WIDTH, ...fonts!)), ''];
      case 'png':
        return [from.exportPng(viewJson, PICTURE_WIDTH, 2, ...fonts!), ''];
      default: {
        const written = from.exportFile(target);
        try {
          return [written.bytes, written.report];
        } finally {
          written.free();
        }
      }
    }
  }

  /**
   * Converts scores of any format the app opens to one format, without opening them for
   * editing: into a folder chosen on desktop, and as one zip file on the web. A score
   * that cannot be read or written is said so, and the rest are converted.
   */
  async function batchConvert(
    files: OpenedFile[],
    target: BatchTarget,
    done: (result: Converted) => void,
  ): Promise<string | null> {
    const folder = canPickFolder() ? await pickFolder() : null;
    if (canPickFolder() && folder === null) return null;
    const fonts = ['pdf', 'png', 'svg'].includes(target) ? await exportFonts() : null;
    const extension = TARGET_EXTENSIONS[target];
    const bundle = folder === null ? new Bundle() : null;
    const taken = new Set<string>();
    let count = 0;
    try {
      for (const file of files) {
        let opened: Document | null = null;
        try {
          const native = file.name.toLowerCase().endsWith(`.${EXTENSION}`);
          opened = native ? Document.load(file.bytes) : Document.importFile(file.name, file.bytes);
          const [bytes, report] = converted(opened, target, fonts);
          const name =
            folder === null
              ? outputName(file.name, extension, taken)
              : await writeInto(folder, file.name, extension, bytes, taken);
          bundle?.add(name, bytes);
          count++;
          done({ from: file.name, name, note: leftOut(native ? '' : opened.importReport, report), failed: false });
        } catch (e) {
          done({ from: file.name, name: null, note: e instanceof Error ? e.message : String(e), failed: true });
        } finally {
          opened?.free();
        }
        // Let the window answer, and show the file done, before the next.
        await new Promise((next) => setTimeout(next));
      }
      const failed = files.length - count;
      const tally =
        tn(count, '{count} score converted.', '{count} scores converted.') +
        (failed ? ' ' + tn(failed, '{count} could not be.', '{count} could not be.') : '');
      if (!bundle || !count) return tally;
      const saved = await exportFile('converted.zip', bundle.zip(), 'Zip archive', 'application/zip');
      return saved ? `${tally} ${t('Downloaded as {name}.', { name: saved })}` : null;
    } finally {
      bundle?.free();
    }
  }

  /** Prints the score, in the view it is shown in, on the paper usual where the user is. */
  const print = () =>
    run(async () => {
      if (!doc) return;
      const fonts = await exportFonts();
      const pages: string[] = JSON.parse(doc.pagesJson(viewJson, paper, ...fonts));
      await printPages(pages, paper);
    });

  const chooseSound = () =>
    run(async () => {
      const font = await pickSoundFont();
      if (!font) return;
      // The synth takes the bytes for itself, so the copy to keep is made first.
      const kept = { name: font.name, bytes: font.bytes.slice(0) };
      const problem = await player.setSoundFont(font.bytes);
      if (problem) throw new Error(problem);
      await storeSoundFont(kept);
      customSound = font.name;
      prefs.sound = 'custom';
      message = t('Playing through {name}', { name: font.name });
    });

  // --- Techniques ---

  const technique = (name: string) => edit((d) => d.toggleTechnique(name));
  const effects = $derived(editor?.note?.effects ?? {});
  const beatEffects = $derived(editor?.beat_effects ?? {});

  const setNoteEffect = (field: string, value: unknown) =>
    edit((d) => d.setNoteEffect(field, JSON.stringify(value)));
  const setBeatEffect = (field: string, value: unknown) =>
    edit((d) => d.setBeatEffect(field, JSON.stringify(value)));

  // --- Tracks ---

  const trackIndex = () => editor?.cursor.track ?? 0;
  const trackCount = () => score?.tracks.length ?? 0;
  const currentMix = () => score?.tracks[trackIndex()]?.mix;

  /** Switches to a track, and to one of its bars if one is given. */
  /** How the score is laid out, as the fields of `gtr_layout::View` the interface chooses. */
  const viewJson = $derived(JSON.stringify({ notation: prefs.notation, multitrack: prefs.multitrack }));

  const voices: Command[] = Array.from({ length: MAX_VOICES }, (_, i) => ({
    id: `voice${i + 1}`,
    label: t('Voice {number}', { number: i + 1 }),
    icon: `voice${i + 1}` as IconName,
    keys: `Alt+${i + 1}`,
    active: () => editor?.cursor.voice === i,
    run: () => edit((d) => d.setVoice(i)),
  }));

  /** What the score is shown on: pages, one sheet wrapped to the window, or one line. */
  const LAYOUTS: [Prefs['layout'], string][] = [
    ['pages', t('Pages, one under another')],
    ['pagesAcross', t('Pages side by side')],
    ['parchment', t('Parchment: one sheet as wide as the window')],
    ['screen', t('Screen: every bar on one line')],
  ];
  const layouts: Command[] = LAYOUTS.map(([value, label]) => ({
    id: `layout.${value}`,
    label,
    active: () => prefs.layout === value,
    run: () => (prefs.layout = value),
  }));
  /** The view for the screen, which only there may be one line. */
  const screenViewJson = $derived(
    JSON.stringify({
      notation: prefs.notation,
      multitrack: prefs.multitrack,
      single_line: prefs.layout === 'screen',
      design: prefs.design,
    }),
  );
  /** The score laid out for the screen in the layout chosen, `width` wide where that matters. */
  const screenLayout = (width: number): Layout =>
    JSON.parse(
      prefs.layout === 'pages' || prefs.layout === 'pagesAcross'
        ? doc!.sheetsJson(screenViewJson, paper, prefs.layout === 'pagesAcross')
        : doc!.layoutJson(width, screenViewJson),
    );

  const notations: Command[] = NOTATIONS.map(([value, label]) => ({
    id: `notation.${value}`,
    label,
    active: () => prefs.notation === value,
    run: () => (prefs.notation = value),
  }));

  const pickTrack = (track: number, bar: number | null) =>
    edit((d) => {
      d.setTrack(track);
      if (bar !== null) d.goToBar(bar);
    });
  const setMix = (track: number, mix: Mix) => edit((d) => d.setMix(track, JSON.stringify(mix)));
  const moveTrack = (by: number) => edit((d) => d.moveTrack(trackIndex(), trackIndex() + by));
  /** Goes to the staff above or below: the track's other staff if it has one, or the next track. */
  const stepTrack = (by: number) =>
    edit((d) => {
      if (!editor || !score) return;
      const { track, staff } = editor.cursor;
      const within = staff + by;
      if (within >= 0 && within < score.tracks[track].staves.length) return d.setStaff(within);
      const next = track + by;
      if (next < 0 || next >= score.tracks.length) return;
      d.setTrack(next);
      if (by < 0) d.setStaff(score.tracks[next].staves.length - 1);
    });
  const onFirstStaff = () => trackIndex() === 0 && (editor?.cursor.staff ?? 0) === 0;
  const onLastStaff = () =>
    trackIndex() >= trackCount() - 1 &&
    (editor?.cursor.staff ?? 0) >= (score?.tracks[trackIndex()]?.staves.length ?? 1) - 1;
  const onDrums = () => score?.tracks[trackIndex()]?.instrument === 'drums';

  /** The kit of the cursor's track: its own, if it has one. */
  const drumKit = $derived.by(() => {
    const own = score?.tracks[editor?.cursor.track ?? 0]?.kit;
    return own?.length ? own : standardKit;
  });
  /** What is under the cursor, in words. */
  const position = $derived(
    score && editor ?
      describeCursor(score, editor, new Map(drumKit.map((piece) => [piece.number, piece.name])))
    : '',
  );

  /** Entering a piece of the drum kit by name, for those who do not know its number. */
  const drumPieces = $derived(
    drumKit.map((piece): Command => ({
      label: `${piece.number}  ${piece.name}`,
      active: () => onDrums() && editor?.note?.fret === piece.number,
      disabled: () => !onDrums(),
      run: () => edit((d) => d.setFret(piece.number)),
    })),
  );

  // --- The instrument view ---

  /** The staff the cursor is on. */
  const cursorStaff = $derived(score && editor ? score.tracks[editor.cursor.track]?.staves[editor.cursor.staff] : undefined);
  const instrumentKind = $derived<'strings' | 'keys' | 'drums'>(
    onDrums() ? 'drums'
    : cursorStaff?.tuning.strings.length === 0 ? 'keys'
    : 'strings',
  );
  /** The notes of a beat of the cursor's staff and voice. */
  const notesAt = (bar: number, beat: number) =>
    cursorStaff?.bars[bar]?.voices[editor?.cursor.voice ?? 0]?.beats[beat]?.notes ?? [];
  /** The notes the instrument lights: those sounding, or those at the cursor. */
  const instrumentNotes = $derived(
    !editor ? []
    : playhead ? notesAt(playhead.bar, playhead.beat)
    : notesAt(editor.cursor.bar, editor.cursor.beat),
  );
  /** Pitch classes marked on the instrument, such as a scale's. */
  let marks = $state.raw<Marks | null>(null);

  /** A fret, key or pad clicked: that note goes into the beat at the cursor, or out if it is there. */
  const pickOnInstrument = (string: number, fret: number) =>
    edit((d) => {
      if (!editor || !cursorStaff) return;
      const { bar, beat } = editor.cursor;
      const notes = notesAt(bar, beat);
      if (instrumentKind === 'drums') {
        const there = notes.find((n) => n.fret === fret);
        const line = there ? there.string : freeLine(notes.map((n) => n.string), cursorStaff.tuning.strings.length);
        if (line === null) throw new Error(t('This beat has a piece of the kit on every line.'));
        d.setCursor(bar, beat, line, false);
        return there ? d.delete() : d.setFret(fret);
      }
      d.setCursor(bar, beat, string, false);
      if (instrumentKind === 'keys') {
        return notes.some((n) => n.string === string) ? d.delete() : d.enterNote();
      }
      return notes.some((n) => n.string === string && n.fret === fret) ? d.delete() : d.setFret(fret);
    });

  // --- Chord and scale libraries ---

  /** A guitar in standard tuning, for showing a scale to a track without strings. */
  const STANDARD_GUITAR = [64, 59, 55, 50, 45, 40];
  /** The open strings of the cursor's staff as they sound, with the capo on. */
  const openStrings = $derived(
    onDrums() ? [] : (cursorStaff?.tuning.strings.map((pitch) => pitch + cursorStaff.capo) ?? []),
  );
  let chordLibraryOpen = $state(false);
  let tunerOpen = $state(false);

  // --- Pedalboard ---

  let pedalSpecs = $state.raw<PedalSpec[]>([]);
  let pedalPresets = $state.raw<PedalPreset[]>([]);
  let userPresets = $state.raw<PedalPreset[]>(loadUserPresets());
  /** The pedalboard shows the mix's pedals rather than the cursor's track's. */
  let pedalMaster = $state(false);
  /** Which of the track's sounds the pedalboard shows: 0 for its own. */
  let pedalSound = $state(0);
  const pedalTrackOf = $derived(score?.tracks[editor?.cursor.track ?? 0]);
  const trackSounds = $derived(pedalTrackOf?.sounds ?? []);
  /** The sound shown, back to the track's own on a track that does not have it. */
  const soundShown = $derived(pedalSound <= trackSounds.length ? pedalSound : 0);
  /** The chain of the sound shown on the cursor's track. */
  const soundChain = $derived(
    (soundShown === 0 ? pedalTrackOf?.effects : trackSounds[soundShown - 1]?.effects) ?? [],
  );
  const pedalChain = $derived((pedalMaster ? score?.effects : soundChain) ?? []);
  const pedalTrack = () => (pedalMaster ? undefined : trackIndex());

  /** Lets what is playing be heard through the chain as a knob turns, before it is kept. */
  function previewPedals(chain: Pedal[]) {
    const bytes = Document.chainBytes(JSON.stringify(chain));
    if (playing) void player.setEffects(pedalTrack() ?? null, pedalMaster ? 0 : soundShown, bytes);
  }

  function setPedals(chain: Pedal[]) {
    const track = pedalTrack();
    if (track === undefined || soundShown === 0) {
      edit((d) => d.setEffects(track, JSON.stringify(chain)));
      return;
    }
    const sounds = trackSounds.map((s, i) => (i === soundShown - 1 ? { ...s, effects: chain } : s));
    edit((d) => d.setSounds(track, JSON.stringify(sounds)));
  }

  /** The sound being named: 0 for a new one, or which of the track's others. */
  let soundDialog = $state<number | null>(null);

  function nameSound(name: string, instrument: Instrument) {
    const track = trackIndex();
    const editing = soundDialog ?? 0;
    const sounds =
      editing === 0
        ? [...trackSounds, { name, instrument }]
        : trackSounds.map((s, i) => (i === editing - 1 ? { ...s, name, instrument } : s));
    edit((d) => d.setSounds(track, JSON.stringify(sounds)));
    if (editing === 0) pedalSound = sounds.length;
  }

  function removeSound() {
    const removed = soundShown;
    edit((d) => d.removeSound(trackIndex(), removed));
    pedalSound = removed - 1;
  }

  function savePreset() {
    const pedals = $state.snapshot(pedalChain) as Pedal[];
    const master = pedalMaster;
    prompt = {
      title: t('Save as a preset'),
      hint: master
        ? t('The pedals of the mix, to put on the mix of another score. A preset of the same name is replaced.')
        : t('The pedals of this track, to put on another track. A preset of the same name is replaced.'),
      field: { kind: 'text', label: t('Name'), value: '' },
      apply: (name: string) => {
        name = name.trim();
        if (!name) return;
        userPresets = withPreset(userPresets, { name, master, pedals });
        saveUserPresets(userPresets);
        message = t('Saved the preset {name}', { name });
      },
    };
  }

  // --- Audio track ---

  let recordingPeaks = $state.raw<Peaks | null>(null);
  let recordingProblem = $state('');
  /** Seconds into the recording sounding, while it plays. */
  let recordingNow = $state<number | null>(null);
  const recordingBars = $derived.by((): RecordingBar[] => {
    void revision;
    return score?.audio && doc ? JSON.parse(doc.recordingBarsJson()) : [];
  });

  // The recording is read and handed to the synth whenever the score is given another one,
  // or another score is opened.
  let readFor: { doc: Document | undefined; name: string | null } = { doc: undefined, name: null };
  $effect(() => {
    const name = score?.audio?.name ?? null;
    const current = doc;
    untrack(() => void readRecording(current, name));
  });

  async function readRecording(current: Document | undefined, name: string | null) {
    if (readFor.doc === current && readFor.name === name) return;
    readFor = { doc: current, name };
    recordingPeaks = null;
    recordingSamples = null;
    recordingProblem = '';
    const bytes = name && current ? current.recordingFile() : undefined;
    if (!name || !bytes) {
      if (name) recordingProblem = t('The recording {name} is not at hand: replace it with the file it came from.', { name });
      await player.setRecording(null);
      return;
    }
    try {
      const decoded = await player.decode(bytes);
      if (readFor.doc !== current || readFor.name !== name) return;
      recordingPeaks = peaksOf(decoded);
      recordingSamples = decoded;
      await player.setRecording(decoded);
    } catch {
      recordingProblem = `${name} could not be read as a recording.`;
      await player.setRecording(null);
    }
  }

  const importRecording = () =>
    run(async () => {
      const file = await openAudioFile();
      if (!file || !doc) return;
      const replacing = !!score?.audio;
      doc.importRecording(file.name, file.bytes);
      prefs.audio = true;
      message = replacing
        ? t('The recording is now {name}', { name: file.name })
        : t('Added an audio track of {name}', { name: file.name });
    });

  const setAudio = (audio: AudioTrack | null) => edit((d) => d.setAudio(JSON.stringify(audio)));

  /** The recording as it was read, to listen to for its beat. */
  let recordingSamples: AudioBuffer | null = null;

  /** Listens for the recording's beat and holds each bar to the beat it starts on. */
  const findBars = () =>
    edit((d) => {
      const recording = recordingSamples;
      if (!recording) return;
      const mono = new Float32Array(recording.length);
      for (let channel = 0; channel < recording.numberOfChannels; channel++) {
        const side = recording.getChannelData(channel);
        for (let i = 0; i < mono.length; i++) mono[i] += side[i] / recording.numberOfChannels;
      }
      const held = d.findBars(mono, recording.sampleRate);
      message = tn(held, 'Held {count} bar to the beat it starts on', 'Held {count} bars to the beats they start on');
    });

  /** The audio track as the track pane shows it: the bars heard over the recording. */
  const audioRow = $derived.by((): AudioRow | null => {
    if (!score?.audio) return null;
    const duration = recordingPeaks?.duration ?? Infinity;
    const heard = recordingBars.filter((b) => b.start < duration && b.start + b.length > 0).map((b) => b.bar);
    return {
      name: score.audio.name,
      mix: score.audio.mix,
      bars: [...new Set(heard)].sort((a, b) => a - b),
      shown: prefs.audio,
    };
  });

  // --- Line in ---

  const lineIn = new LineIn();
  let lineOn = $state(false);
  let lineDialog = $state(false);
  let lineSettings = $state(loadLineSettings());
  let lineLevels = $state.raw<Levels>({ input: 0, output: 0 });
  let lineLatency = $state(0);
  lineIn.onlevels = (levels) => (lineLevels = levels);
  const lineTrack = $derived(score?.tracks[editor?.cursor.track ?? 0]);
  const pedalNames = (chain: Pedal[] | undefined) =>
    (chain ?? []).filter((p) => p.on !== false).map((p) => pedalSpecs.find((s) => s.kind === p.kind)?.name ?? p.kind);
  const chainBytes = (chain: Pedal[] | undefined) => Document.chainBytes(JSON.stringify(chain ?? []));
  const decibels = (db: number) => 10 ** (db / 20);

  async function startLine(settings: LineSettings): Promise<string | null> {
    lineSettings = settings;
    saveLineSettings(settings);
    try {
      await lineIn.start(
        settings.device,
        chainBytes(soundChain),
        chainBytes(score?.effects),
        decibels(settings.gain),
      );
    } catch (error) {
      return t('The input could not be opened: {problem}', { problem: error instanceof Error ? error.message : String(error) });
    }
    lineOn = true;
    lineLatency = lineIn.latency;
    message = t('Playing the line in through the pedals');
    return null;
  }

  function setLineGain(gain: number) {
    lineSettings = { ...lineSettings, gain };
    saveLineSettings(lineSettings);
    lineIn.setGain(decibels(gain));
  }

  function stopLine() {
    lineIn.stop();
    lineOn = false;
    lineLevels = { input: 0, output: 0 };
    message = t('Stopped the line in');
  }

  // The line follows the pedals of the cursor's track, in the sound shown on the
  // pedalboard, and of the mix as they change.
  $effect(() => {
    if (lineOn) lineIn.setEffects(false, chainBytes(soundChain));
  });
  $effect(() => {
    if (lineOn) lineIn.setEffects(true, chainBytes(score?.effects));
  });

  function deletePreset(preset: PedalPreset) {
    userPresets = userPresets.filter((p) => p !== preset);
    saveUserPresets(userPresets);
    message = t('Deleted the preset {name}', { name: preset.name });
  }
  let scaleLibrary = $state.raw<{ root: number; scale: number; found: { fits: Fit[]; source: string } | null } | null>(
    null,
  );

  /** Plays a few notes on the cursor's instrument, as a chord or a scale is tried out. */
  const audition = (pitches: number[], gap: number, length: number) =>
    run(async () => {
      if (!score) return;
      stop();
      const instrument = score.tracks[trackIndex()].instrument;
      const passage = Document.audition(instrument === 'drums' ? 'steel_guitar' : instrument, Uint8Array.from(pitches), gap, length);
      const timeline: Timeline = JSON.parse(passage.timelineJson);
      const events = passage.events;
      passage.free();
      await player.play(events, timeline, () => {});
    });

  /** Marks the notes of a chord or a scale on the fretboard or keyboard, and shows it. */
  function showMarks(next: Marks) {
    marks = next;
    prefs.instrument = true;
    chordLibraryOpen = false;
    scaleLibrary = null;
    message =
      instrumentKind === 'keys'
        ? t('{name} marked on the keyboard', { name: next.label })
        : t('{name} marked on the fretboard', { name: next.label });
  }

  /** The pitches of the selected notes, or of the whole track without a selection. */
  function selectedPitches(): number[] {
    if (!score || !editor) return [];
    const track = score.tracks[editor.cursor.track];
    const selection = editor.selection;
    const staves = selection ? [track.staves[editor.cursor.staff]] : track.staves;
    const pitches: number[] = [];
    for (const staff of staves) {
      const pitchOf = (string: number, fret: number) =>
        staff.tuning.strings.length ? staff.tuning.strings[string] + staff.capo + fret : string;
      staff.bars.forEach((bar, b) => {
        bar.voices.forEach((voice, v) => {
          voice.beats.forEach((beat, i) => {
            if (selection) {
              const at = [b, i];
              const within = (a: number[], z: number[]) => a[0] < z[0] || (a[0] === z[0] && a[1] <= z[1]);
              if (v !== editor!.cursor.voice || !within(selection[0], at) || !within(at, selection[1])) return;
            }
            for (const note of beat.notes) pitches.push(pitchOf(note.string, note.fret));
          });
        });
      });
    }
    return pitches;
  }

  /** Opens the scale library on the scales that fit the selected notes, or the track's. */
  function findScale() {
    if (!editor) return;
    const fits = findScales(selectedPitches());
    const best = fits[0];
    scaleLibrary = {
      root: best?.root ?? 0,
      scale: best ? SCALES.indexOf(best.scale) : 0,
      found: { fits, source: editor.selection ? 'the selection' : 'the whole track' },
    };
  }

  // --- Bar marks and beat text ---

  /** Whether the cursor is on a staff without strings, where notes are entered by name. */
  const pitched = $derived(
    !!score && !!editor && score.tracks[editor.cursor.track]?.staves[editor.cursor.staff]?.tuning.strings.length === 0,
  );
  const masterBar = $derived(score?.master_bars[editor?.cursor.bar ?? 0]);

  // --- Stylesheet and design ---

  /** The stylesheet a score comes with, read from the core once it has loaded. */
  let standardStyle = $state.raw<Style | null>(null);
  const currentStyle = (): Style => score?.style ?? standardStyle!;
  const setStyle = (style: Style) => edit((d) => d.setStyle(JSON.stringify(style)));
  const [MIN_BAR_WIDTH, MAX_BAR_WIDTH, BAR_WIDTH_STEP] = [25, 400, 10];
  /** Gives the cursor's bar a width, as a percentage; 100 is as it comes. */
  const setBarWidth = (percent: number) => {
    const width = Math.max(MIN_BAR_WIDTH, Math.min(MAX_BAR_WIDTH, Math.round(percent)));
    setBar('width', width === 100 ? null : width);
  };
  const setBar = (field: string, value: unknown) =>
    edit((d) => d.setBarProperty(field, JSON.stringify(value)));
  const barNumber = () => (editor?.cursor.bar ?? 0) + 1;

  /** A command that sets a property of the bar to one of a list of choices, or clears it. */
  const barChoices = <T extends string>(field: 'feel', choices: [T, string][]): Command[] =>
    choices.map(([value, label]) => ({
      id: `${field}.${value}`,
      label,
      active: () => masterBar?.[field] === value,
      run: () => setBar(field, masterBar?.[field] === value ? null : value),
    }));

  /** A command for each thing a bar can have several of: it is put on the bar, or taken off it. */
  const barList = <T extends Target | Jump>(field: 'targets' | 'jumps', choices: [T, string][]): Command[] =>
    choices.map(([value, label]) => {
      const there = () => ((masterBar?.[field] ?? []) as T[]).includes(value);
      return {
        id: `${field}.${value}`,
        label,
        active: there,
        run: () => {
          const now = (masterBar?.[field] ?? []) as T[];
          setBar(field, there() ? now.filter((one) => one !== value) : [...now, value]);
        },
      };
    });

  const fades: Command[] = FADES.map(([value, label]) => ({
    id: `fade.${value}`,
    label,
    active: () => beatEffects.fade === value,
    disabled: () => !editor?.beat_effects,
    run: () => setBeatEffect('fade', beatEffects.fade === value ? null : value),
  }));

  /**
   * Where above a stopped note its string can be touched for a harmonic, in frets, and what
   * then sounds. Natural harmonics have no say here: their fret is where they are touched.
   */
  const HARMONIC_TOUCHES: [number, string][] = [
    [12, 'an octave up'],
    [7, 'an octave and a fifth up'],
    [5, 'two octaves up'],
    [4, 'two octaves and a third up'],
    [3, 'two octaves and a fifth up'],
  ];
  const usualTouch = () => (effects.harmonic === 'pinch' ? 5 : 12);
  const harmonicTouches: Command[] = HARMONIC_TOUCHES.map(([frets, sounds]) => ({
    id: `harmonicTouch.${frets}`,
    label: t('Touched {frets} frets above the note: {sounds}', { frets, sounds }),
    active: () =>
      !!effects.harmonic && effects.harmonic !== 'natural' && (effects.harmonic_touch ?? usualTouch()) === frets,
    disabled: () => !effects.harmonic || effects.harmonic === 'natural',
    run: () => setNoteEffect('harmonic_touch', frets === usualTouch() ? null : frets),
  }));

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
    id: `value.${value}`,
    label: VALUE_NAMES[i],
    icon: value,
    active: () => editor?.duration.value === value,
    run: () => edit((d) => d.setNoteValue(value)),
  }));

  const dynamics: Command[] = DYNAMICS.map(([value, name]) => ({
    id: `dynamic.${value}`,
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
        id: `${nested ? 'nestedTuplet' : 'tuplet'}.${actual}:${normal}`,
        label: t('{actual} in the time of {normal}', { actual, normal }),
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
      id: `${field}.${finger}`,
      label: `${marks[i]}  ${FINGER_NAMES[i]}`,
      active: () => effects[field] === finger,
      disabled: noNote,
      run: () => setNoteEffect(field, effects[field] === finger ? null : finger),
    }));

  const cmd = {
    new: { label: t('New'), icon: 'new', keys: 'Ctrl+N', run: newScore },
    open: { label: t('Open or import…'), icon: 'open', keys: 'Ctrl+O', run: open },
    save: { label: t('Save'), icon: 'save', keys: 'Ctrl+S', disabled: noScore, run: () => save() },
    saveAs: { label: t('Save as…'), icon: 'saveAs', keys: 'Ctrl+Shift+S', disabled: noScore, run: () => save(true) },
    batchConvert: { label: t('Batch convert…'), run: () => (batchOpen = true) },
    exportAudio: { label: t('Audio (WAV)…'), disabled: () => noScore() || exporting, run: () => exportAudio() },
    exportPdf: { label: t('PDF…'), disabled: noScore, run: () => exportPicture('pdf') },
    exportPng: { label: t('Picture (PNG)…'), disabled: noScore, run: () => exportPicture('png') },
    exportSvg: { label: t('Picture (SVG)…'), disabled: noScore, run: () => exportPicture('svg') },
    exportGuitarPro: { label: t('Guitar Pro (.gp)…'), disabled: noScore, run: () => exportAs('guitar_pro') },
    exportMusicXml: { label: t('MusicXML…'), disabled: noScore, run: () => exportAs('musicxml') },
    exportMidi: { label: t('MIDI…'), disabled: noScore, run: () => exportAs('midi') },
    exportAscii: { label: t('Text tab…'), disabled: noScore, run: () => exportAs('ascii') },
    print: { label: t('Print…'), keys: 'Ctrl+P', disabled: noScore, run: print },

    undo: { label: t('Undo'), icon: 'undo', keys: 'Ctrl+Z', disabled: () => !editor?.can_undo, run: () => edit((d) => d.undo()) },
    redo: { label: t('Redo'), icon: 'redo', keys: 'Ctrl+Y / Ctrl+Shift+Z', disabled: () => !editor?.can_redo, run: () => edit((d) => d.redo()) },
    cut: { label: t('Cut'), icon: 'cut', keys: 'Ctrl+X', run: () => edit((d) => d.cut()) },
    copy: { label: t('Copy'), icon: 'copy', keys: 'Ctrl+C', run: () => edit((d) => d.copy()) },
    paste: { label: t('Paste'), icon: 'paste', keys: 'Ctrl+V', disabled: () => !editor?.can_paste, run: () => edit((d) => d.paste()) },
    insertBeat: { label: t('Insert a rest before the cursor'), keys: 'Insert', run: () => edit((d) => d.insertBeat()) },
    delete: { label: t('Remove the note, or the beat if it is a rest'), keys: 'Delete / Backspace', run: () => edit((d) => d.delete()) },
    deleteBeats: { label: t('Remove the beat or the selected beats'), keys: 'Ctrl+Delete', run: () => edit((d) => d.deleteBeats()) },

    dot: { label: t('Dotted'), icon: 'dot', keys: '.', active: () => (editor?.duration.dots ?? 0) > 0, run: () => edit((d) => d.toggleDot()) },
    triplet: { label: t('Triplet'), icon: 'triplet', keys: '/', active: () => !!editor?.duration.tuplet, run: () => edit((d) => d.toggleTriplet()) },
    rest: { label: t('Rest'), icon: 'rest', keys: 'R', run: () => edit((d) => d.rest()) },
    tie: { label: t('Tie to the previous note'), icon: 'tie', keys: 'L', active: () => !!editor?.note?.tied, run: () => edit((d) => d.toggleTie()) },

    hammer: toggle(t('Hammer-on or pull-off to the next note'), 'hammer', 'hammer', () => !!effects.hammer, 'H'),
    slideShift: toggle(t('Shift slide to the next note'), 'slideShift', 'slide_shift', () => effects.slide === 'shift', 'S'),
    slideLegato: toggle(t('Legato slide to the next note'), 'slideLegato', 'slide_legato', () => effects.slide === 'legato'),
    slideInBelow: toggle(t('Slide in from below'), 'slideInBelow', 'slide_in_below', () => effects.slide === 'in_from_below'),
    slideInAbove: toggle(t('Slide in from above'), 'slideInAbove', 'slide_in_above', () => effects.slide === 'in_from_above'),
    slideOutDown: toggle(t('Slide out downwards'), 'slideOutDown', 'slide_out_down', () => effects.slide === 'out_down'),
    slideOutUp: toggle(t('Slide out upwards'), 'slideOutUp', 'slide_out_up', () => effects.slide === 'out_up'),
    bend: {
      label: t('Full bend'),
      icon: 'bend',
      keys: 'B',
      active: () => !!effects.bend,
      run: () => setNoteEffect('bend', effects.bend ? null : bendPreset('bend', 4)),
    },
    bendEditor: { label: t('Bend editor…'), icon: 'bend', keys: 'Shift+B', disabled: noNote, run: () => (effectDialog = 'bend') },
    vibrato: toggle(t('Vibrato'), 'vibrato', 'vibrato', () => effects.vibrato === 'slight', 'V'),
    wideVibrato: toggle(t('Wide vibrato'), 'wideVibrato', 'wide_vibrato', () => effects.vibrato === 'wide'),
    palmMute: toggle(t('Palm mute'), 'palmMute', 'palm_mute', () => !!effects.palm_mute, 'P'),
    letRing: toggle(t('Let ring'), 'letRing', 'let_ring', () => !!effects.let_ring, 'I'),
    dead: toggle(t('Dead note'), 'dead', 'dead', () => !!effects.dead, 'X'),
    ghost: toggle(t('Ghost note'), 'ghost', 'ghost', () => !!effects.ghost, 'G'),
    staccato: toggle(t('Staccato'), 'staccato', 'staccato', () => !!effects.staccato),
    accent: toggle(t('Accent'), 'accent', 'accent', () => effects.accent === 'normal', 'A'),
    heavyAccent: toggle(t('Heavy accent'), 'heavyAccent', 'heavy_accent', () => effects.accent === 'heavy'),
    harmonicNatural: toggle(t('Natural harmonic'), 'harmonic', 'harmonic_natural', () => effects.harmonic === 'natural', 'N'),
    harmonicArtificial: toggle(t('Artificial harmonic'), 'harmonic', 'harmonic_artificial', () => effects.harmonic === 'artificial'),
    harmonicPinch: toggle(t('Pinch harmonic'), 'harmonic', 'harmonic_pinch', () => effects.harmonic === 'pinch'),
    harmonicTapped: toggle(t('Tapped harmonic'), 'harmonic', 'harmonic_tapped', () => effects.harmonic === 'tapped'),
    harmonicSemi: toggle(t('Semi-harmonic'), 'harmonic', 'harmonic_semi', () => effects.harmonic === 'semi'),
    tapped: toggle(t('Tapping'), 'tapping', 'tapped', () => !!effects.tapped, 'T'),
    trill: { label: t('Trill…'), icon: 'trill', active: () => !!effects.trill, disabled: noNote, run: () => (effectDialog = 'trill') },
    grace: { label: t('Grace note…'), icon: 'grace', active: () => !!effects.grace, disabled: noNote, run: () => (effectDialog = 'grace') },

    crescendo: toggle(t('Crescendo'), 'crescendo', 'crescendo', () => beatEffects.hairpin === 'crescendo', '<'),
    decrescendo: toggle(t('Decrescendo'), 'decrescendo', 'decrescendo', () => beatEffects.hairpin === 'decrescendo', '>'),
    strokeDown: toggle(t('Downstroke'), 'strokeDown', 'stroke_down', () => beatEffects.stroke === 'down'),
    strokeUp: toggle(t('Upstroke'), 'strokeUp', 'stroke_up', () => beatEffects.stroke === 'up'),
    strumDown: toggle(t('Strum down'), 'strumDown', 'brush_down', () => beatEffects.strum === 'brush_down'),
    strumUp: toggle(t('Strum up'), 'strumUp', 'brush_up', () => beatEffects.strum === 'brush_up'),
    arpeggioDown: toggle(t('Arpeggio down'), 'arpeggioDown', 'arpeggio_down', () => beatEffects.strum === 'arpeggio_down'),
    arpeggioUp: toggle(t('Arpeggio up'), 'arpeggioUp', 'arpeggio_up', () => beatEffects.strum === 'arpeggio_up'),
    rasgueado: toggle(t('Rasgueado'), 'strumDown', 'rasgueado', () => beatEffects.strum === 'rasgueado'),
    tremolo: toggle(t('Tremolo picking'), 'tremolo', 'tremolo', () => !!beatEffects.tremolo),
    slap: toggle(t('Slap'), 'slap', 'slap', () => beatEffects.slap === 'slap'),
    pop: toggle(t('Pop'), 'pop', 'pop', () => beatEffects.slap === 'pop'),
    whammy: { label: t('Whammy bar…'), icon: 'whammy', keys: 'W', active: () => !!beatEffects.whammy, run: () => (effectDialog = 'whammy') },

    insertBar: { label: t('Insert bar'), icon: 'insertBar', keys: 'Ctrl+Insert', run: () => edit((d) => d.insertBar()) },
    removeBar: { label: t('Remove bar'), icon: 'removeBar', run: () => edit((d) => d.removeBar()) },
    timeSignature: { label: t('Time signature…'), icon: 'timeSignature', run: () => (dialog = 'time') },
    keySignature: { label: t('Key signature…'), icon: 'keySignature', run: () => (dialog = 'key') },
    scoreInfo: { label: t('Title, credits and tempo…'), icon: 'scoreInfo', run: () => (dialog = 'score') },

    repeatStart: {
      label: t('Start of a repeat'),
      icon: 'repeatStart',
      active: () => !!masterBar?.repeat_start,
      run: () => setBar('repeat_start', masterBar?.repeat_start ? null : true),
    },
    repeatEnd: {
      label: t('End of a repeat…'),
      icon: 'repeatEnd',
      active: () => !!masterBar?.repeat_count,
      run: () =>
        (prompt = {
          title: t('End of a repeat'),
          hint: t('The repeat ends with bar {bar}. Leave the field empty for no repeat.', { bar: barNumber() }),
          field: { kind: 'number', label: t('Times played in all'), value: masterBar?.repeat_count || 2, min: 2, max: 99 },
          apply: (count: number | null) => setBar('repeat_count', count && count >= 2 ? count : null),
        }),
    },
    endings: {
      label: t('Alternate ending…'),
      icon: 'ending',
      active: () => !!masterBar?.alternate_endings,
      run: () =>
        (prompt = {
          title: t('Alternate ending'),
          hint: t('Bar {bar} is played only on the passes ticked; with none ticked it is played every time.', { bar: barNumber() }),
          field: { kind: 'bits', label: t('Times through the repeat'), value: masterBar?.alternate_endings ?? 0, count: 8 },
          apply: (passes: number) => setBar('alternate_endings', passes || null),
        }),
    },
    section: {
      label: t('Section or rehearsal mark…'),
      icon: 'section',
      active: () => !!masterBar?.section,
      run: () =>
        (prompt = {
          title: t('Section'),
          hint: t('Named from bar {bar}, for example Verse, Chorus or A. Leave it empty for none.', { bar: barNumber() }),
          field: { kind: 'text', label: t('Name'), value: masterBar?.section ?? '' },
          apply: (name: string) => setBar('section', name || null),
        }),
    },
    barTempo: {
      label: t('Tempo change…'),
      icon: 'tempo',
      active: () => !!editor?.tempo_change,
      run: () =>
        (prompt = {
          title: t('Tempo change'),
          hint: t("From the cursor's beat in bar {bar} on. Leave it empty to carry on at the tempo before.", { bar: barNumber() }),
          field: { kind: 'number', label: t('Tempo (bpm)'), value: editor?.tempo_change ?? null, min: 20, max: 400 },
          apply: (tempo: number | null) => edit((d) => d.setTempoChange(tempo || 0, !!editor?.tempo_gradual)),
        }),
    },
    tempoGradual: {
      label: t('Change tempo by degrees, up to the next change'),
      active: () => !!editor?.tempo_gradual,
      disabled: () => !editor?.tempo_change,
      run: () => edit((d) => d.setTempoChange(editor?.tempo_change ?? 0, !editor?.tempo_gradual)),
    },
    fermata: {
      label: t('Fermata'),
      icon: 'fermata',
      active: () => !!beatEffects.fermata,
      run: () => setBeatEffect('fermata', beatEffects.fermata ? null : true),
    },
    chordName: {
      label: t('Chord name and diagram…'),
      icon: 'chordDiagram',
      active: () => !!beatEffects.chord || !!beatEffects.diagram,
      disabled: () => !editor?.beat_effects,
      run: () => (chordOpen = true),
    },
    timer: {
      label: t('Timer: show the time elapsed'),
      icon: 'timer',
      active: () => !!masterBar?.timer,
      run: () => setBar('timer', masterBar?.timer ? null : true),
    },
    volumeChange: {
      label: t('Volume change…'),
      icon: 'volumeChange',
      active: () => beatEffects.volume !== undefined,
      run: () =>
        (prompt = {
          title: t('Volume change'),
          hint: t('The track plays at this volume from this beat on, until the next change. At 75 it is as loud as its slider in the mixer sets it; the slider still turns the whole track up and down. Leave it empty for no change here.'),
          field: { kind: 'number', label: t('Volume (0 to 100; 75 is the level of the mixer)'), value: beatEffects.volume ?? null, min: 0, max: 100 },
          apply: (volume: number | null) => setBeatEffect('volume', volume),
        }),
    },
    panChange: {
      label: t('Pan change…'),
      icon: 'panChange',
      active: () => beatEffects.pan !== undefined,
      run: () =>
        (prompt = {
          title: t('Pan change'),
          hint: t('The track moves this far from where the mixer puts it, from this beat on until the next change. Leave it empty for no change here.'),
          field: { kind: 'number', label: t('Pan (-100 left to 100 right; 0 is where the mixer puts it)'), value: beatEffects.pan ?? null, min: -100, max: 100 },
          apply: (pan: number | null) => setBeatEffect('pan', pan),
        }),
    },
    beatText: {
      label: t('Text…'),
      icon: 'beatText',
      active: () => !!beatEffects.text,
      run: () =>
        (prompt = {
          title: t('Text'),
          hint: t('Written above the beat. Leave it empty for none.'),
          field: { kind: 'text', label: t('Text'), value: beatEffects.text ?? '' },
          apply: (text: string) => setBeatEffect('text', text || null),
        }),
    },
    lyrics: {
      label: t('Lyrics…'),
      icon: 'lyrics',
      active: () => !!beatEffects.lyrics?.length,
      run: () =>
        (prompt = {
          title: t('Lyrics'),
          hint: t('What is sung on this beat: one line here for each line of the lyrics, up to five.'),
          field: { kind: 'lines', label: t('Syllables'), value: beatEffects.lyrics ?? [] },
          apply: (lines: string[]) => setBeatEffect('lyrics', lines.length ? lines.slice(0, 5) : null),
        }),
    },

    addTrack: { label: t('Add a track…'), icon: 'addTrack', run: () => (dialog = 'addTrack') },
    trackSettings: { label: t('Instrument, tuning, capo and mix…'), icon: 'trackSettings', run: () => (dialog = 'track') },
    drumKit: { label: t('Drum kit…'), disabled: () => !onDrums(), run: () => (kitDialog = true) },
    removeTrack: { label: t('Remove the track'), icon: 'removeTrack', disabled: () => trackCount() < 2, run: () => edit((d) => d.removeTrack(trackIndex())) },
    trackUp: { label: t('Move the track up'), icon: 'moveUp', disabled: () => trackIndex() === 0, run: () => moveTrack(-1) },
    trackDown: { label: t('Move the track down'), icon: 'moveDown', disabled: () => trackIndex() >= trackCount() - 1, run: () => moveTrack(1) },
    previousTrack: { label: t('Previous track or staff'), keys: 'Ctrl+↑', disabled: onFirstStaff, run: () => stepTrack(-1) },
    nextTrack: { label: t('Next track or staff'), keys: 'Ctrl+↓', disabled: onLastStaff, run: () => stepTrack(1) },
    mute: {
      label: t('Mute the track'),
      active: () => !!currentMix()?.mute,
      run: () => {
        const mix = currentMix();
        if (mix) setMix(trackIndex(), { ...mix, mute: !mix.mute });
      },
    },
    solo: {
      label: t('Solo the track'),
      active: () => !!currentMix()?.solo,
      run: () => {
        const mix = currentMix();
        if (mix) setMix(trackIndex(), { ...mix, solo: !mix.solo });
      },
    },

    play: { label: t('Play from the cursor or the selection'), icon: 'play', keys: 'Space', run: togglePlay },
    metronome: { label: t('Metronome'), icon: 'metronome', active: () => metronome, run: () => (metronome = !metronome) },
    countIn: { label: t('Count-in bar'), icon: 'countIn', active: () => countIn, run: () => (countIn = !countIn) },
    beatLights: {
      label: t('Visual metronome'),
      icon: 'beatLights',
      active: () => prefs.beatLights,
      run: () => (prefs.beatLights = !prefs.beatLights),
    },
    loop: { label: t('Loop'), icon: 'loop', active: () => loop, run: () => setLoop(!loop) },
    trainer: { label: t('Speed trainer…'), icon: 'trainer', active: () => training, run: () => (trainerOpen = true) },
    slower: { label: t('Slower'), keys: '[', disabled: () => speed <= MIN_SPEED, run: () => setSpeed(speed - SPEED_STEP) },
    faster: { label: t('Faster'), keys: ']', disabled: () => speed >= MAX_SPEED, run: () => setSpeed(speed + SPEED_STEP) },
    speedOther: {
      label: t('Other…'),
      active: () => !SPEEDS.includes(speed),
      run: () =>
        (prompt = {
          title: t('Playback speed'),
          hint: t('How fast the score is played, as a percentage of the tempo written. The pitch stays as it is.'),
          field: { kind: 'number', label: t('Speed (%)'), value: speed, min: MIN_SPEED, max: MAX_SPEED },
          apply: (percent: number | null) => setSpeed(percent ?? 100),
        }),
    },
    soundDefault: {
      label: t('{name} (comes with gtrTab)', { name: DEFAULT_SOUNDFONT.name }),
      active: () => prefs.sound === 'default',
      run: () => useSound('default'),
    },
    soundCustom: {
      label: t('SoundFont from a file…'),
      active: () => prefs.sound === 'custom',
      run: chooseSound,
    },
    soundModels: {
      label: t('Built-in models, without samples'),
      active: () => prefs.sound === 'models',
      run: () => useSound('models'),
    },
    replayRepeats: {
      label: t('Repeats again after D.C. and D.S.'),
      icon: 'repeatEnd',
      active: () => replayRepeats,
      run: () => (replayRepeats = !replayRepeats),
    },

    zoomIn: { label: t('Zoom in'), icon: 'zoomIn', disabled: () => zoom >= ZOOMS[ZOOMS.length - 1], run: () => stepZoom(1) },
    zoomOut: { label: t('Zoom out'), icon: 'zoomOut', disabled: () => zoom <= ZOOMS[0], run: () => stepZoom(-1) },
    compact: { label: t('Narrow palette'), icon: 'palette', active: () => compact, run: () => (prefs.compact = !compact) },
    inspector: { label: t('Inspector'), icon: 'inspector', active: () => inspectorOpen, run: () => (prefs.inspector = !inspectorOpen) },
    multitrack: { label: t('Show every track'), icon: 'multitrack', active: () => prefs.multitrack, run: () => (prefs.multitrack = !prefs.multitrack) },
    instrument: {
      label: t('Fretboard, keyboard or drum kit'),
      icon: 'tuning',
      active: () => prefs.instrument,
      run: () => (prefs.instrument = !prefs.instrument),
    },
    soundChange: {
      label: t('Change of sound…'),
      icon: 'soundChange',
      active: () => beatEffects.sound !== undefined,
      run: () => {
        const track = score?.tracks[trackIndex()];
        const others = track?.sounds ?? [];
        prompt = {
          title: t('Change of sound'),
          hint: others.length
            ? 'The track plays with this sound from this beat on, until the next change. What is written stays as it is.'
            : 'The track has no other sound to change to yet: add one on the pedalboard, with the + beside its sounds.',
          field: {
            kind: 'choice',
            label: t('From this beat on'),
            value: beatEffects.sound ?? null,
            options: [
              { value: null, label: t('No change here') },
              { value: 0, label: t('Its own sound ({name})', { name: track?.name ?? '' }) },
              ...others.map((s, i) => ({ value: i + 1, label: s.name })),
            ],
          },
          apply: (sound: number | null) => setBeatEffect('sound', sound),
        };
      },
    },
    audioTrack: {
      label: t('Audio track…'),
      icon: 'soundChange',
      run: () => (score?.audio ? (prefs.audio = true) : importRecording()),
    },
    audioPanel: {
      label: t('Audio track'),
      active: () => prefs.audio && !!score?.audio,
      disabled: () => !score?.audio,
      run: () => (prefs.audio = !prefs.audio),
    },
    pedalboard: {
      label: t('Pedalboard'),
      icon: 'pedalboard',
      active: () => prefs.pedalboard,
      run: () => (prefs.pedalboard = !prefs.pedalboard),
    },
    trackEffects: {
      label: t('Effects…'),
      run: () => {
        pedalMaster = false;
        prefs.pedalboard = true;
      },
    },
    masterEffects: {
      label: t('Master effects…'),
      run: () => {
        pedalMaster = true;
        prefs.pedalboard = true;
      },
    },
    lineIn: {
      label: t('Line in…'),
      icon: 'lineIn',
      run: () => (lineDialog = true),
    },
    /** In the transport bar while the line in plays. */
    lineInOn: {
      label: t('Line in: playing'),
      icon: 'lineIn',
      active: () => lineOn,
      run: () => (lineDialog = true),
    },
    tuner: {
      label: t('Tuner…'),
      run: () => {
        stop();
        tunerOpen = true;
      },
    },
    chordLibrary: { label: t('Chord library…'), run: () => (chordLibraryOpen = true) },
    scaleLibrary: { label: t('Scale library…'), run: () => (scaleLibrary = { root: 0, scale: 0, found: null }) },
    findScale: {
      label: t('Find the scale of the selection…'),
      disabled: () => onDrums(),
      run: findScale,
    },
    trackPane: { label: t('Track pane'), icon: 'trackPane', active: () => prefs.tracks, run: () => (prefs.tracks = !prefs.tracks) },
    shortcuts: { label: t('Keyboard shortcuts'), icon: 'shortcuts', keys: '? / F1', run: () => (shortcutsOpen = true) },
    designMode: {
      label: t('Design mode: set out lines, pages and bar widths'),
      icon: 'design',
      active: () => prefs.design,
      run: () => (prefs.design = !prefs.design),
    },
    stylesheet: { label: t('Stylesheet…'), icon: 'design', run: () => (styleOpen = true) },
    lineBreak: {
      label: t('Start a new line with this bar'),
      icon: 'lineBreak',
      active: () => !!masterBar?.line_break,
      disabled: () => (editor?.cursor.bar ?? 0) === 0,
      run: () => setBar('line_break', masterBar?.line_break ? null : true),
    },
    pageBreak: {
      label: t('Start a new page with this bar'),
      icon: 'pageBreak',
      active: () => !!masterBar?.page_break,
      disabled: () => (editor?.cursor.bar ?? 0) === 0,
      run: () => setBar('page_break', masterBar?.page_break ? null : true),
    },
    barWider: {
      label: t('Wider bar'),
      icon: 'barWider',
      disabled: () => (masterBar?.width ?? 100) >= MAX_BAR_WIDTH,
      run: () => setBarWidth((masterBar?.width ?? 100) + BAR_WIDTH_STEP),
    },
    barNarrower: {
      label: t('Narrower bar'),
      icon: 'barNarrower',
      disabled: () => (masterBar?.width ?? 100) <= MIN_BAR_WIDTH,
      run: () => setBarWidth((masterBar?.width ?? 100) - BAR_WIDTH_STEP),
    },
    barWidthReset: {
      label: t('Bar width as it comes'),
      disabled: () => masterBar?.width == null,
      run: () => setBarWidth(100),
    },
    barsPerLine: {
      label: t('Bars on each line…'),
      icon: 'barsPerLine',
      active: () => !!score?.style?.bars_per_line,
      run: () =>
        (prompt = {
          title: t('Bars on each line'),
          hint: t('Every line has this many bars, unless a bar starts a new one before then. Leave it empty for as many as fit.'),
          field: { kind: 'number', label: t('Bars'), value: score?.style?.bars_per_line ?? null, min: 1, max: 32 },
          apply: (count: number | null) => setStyle({ ...currentStyle(), bars_per_line: count || null }),
        }),
    },
    preferences: { label: t('Preferences…'), icon: 'preferences', keys: 'Ctrl+,', run: () => (prefsOpen = true) },
    commandPalette: { label: t('Command palette…'), icon: 'search', keys: 'Ctrl+K / Ctrl+Shift+P', run: () => (paletteOpen = true) },
  } satisfies Record<string, Command>;
  // Each is known by its name here, for the keys the user gives it.
  for (const [id, command] of Object.entries(cmd)) (command as Command).id = id;

  const speeds: Command[] = SPEEDS.map((percent) => ({
    id: `speed.${percent}`,
    label: `${percent}%`,
    active: () => speed === percent,
    run: () => setSpeed(percent),
  }));

  const themes: Command[] = THEMES.map((theme) => ({
    id: `theme.${theme}`,
    label: THEME_NAMES[theme],
    icon: THEME_ICONS[theme],
    active: () => prefs.theme === theme,
    run: () => setTheme(theme),
  }));

  function stepZoom(by: number) {
    const at = ZOOMS.indexOf(zoom);
    zoom = ZOOMS[Math.max(0, Math.min(ZOOMS.length - 1, at + by))];
  }

  const palette: PaletteSection[] = $derived([
    ...(prefs.design
      ? [
          {
            id: 'design',
            title: t('Design'),
            commands: [cmd.lineBreak, cmd.pageBreak, cmd.barWider, cmd.barNarrower, cmd.barsPerLine, cmd.stylesheet],
          },
        ]
      : []),
    { id: 'values', title: t('Note values'), commands: [...noteValues, cmd.dot, cmd.triplet, cmd.rest, cmd.tie] },
    {
      id: 'note',
      title: t('On the note'),
      commands: [
        cmd.hammer, cmd.slideShift, cmd.slideLegato, cmd.bend,
        cmd.vibrato, cmd.wideVibrato, cmd.palmMute, cmd.letRing,
        cmd.dead, cmd.ghost, cmd.harmonicNatural, cmd.tapped,
        cmd.staccato, cmd.accent, cmd.heavyAccent, cmd.trill, cmd.grace,
      ],
    },
    {
      id: 'beat',
      title: t('On the beat'),
      commands: [
        ...dynamics, cmd.crescendo, cmd.decrescendo, cmd.strokeDown, cmd.strokeUp,
        cmd.strumDown, cmd.strumUp, cmd.tremolo, cmd.whammy,
        cmd.chordName, cmd.beatText, cmd.lyrics, cmd.fermata,
      ],
    },
    {
      id: 'bars',
      title: t('Bars and score'),
      commands: [
        cmd.insertBar, cmd.removeBar, cmd.timeSignature, cmd.keySignature, cmd.scoreInfo,
        cmd.repeatStart, cmd.repeatEnd, cmd.endings, cmd.section,
      ],
    },
    { id: 'voice', title: t('Voice'), commands: voices },
  ]);

  const menus = $derived.by((): Menu[] => {
    const recentItems: MenuEntry[] = recent.map((path) => ({
      label: baseName(path),
      run: () => openRecent(path),
    }));
    const tracks: MenuEntry[] = (score?.tracks ?? []).map((track, i) => ({
      label: track.name || t('Track {number}', { number: i + 1 }),
      active: () => editor?.cursor.track === i,
      run: () => pickTrack(i, null),
    }));
    return [
      {
        label: t('File'),
        items: [
          cmd.new,
          cmd.open,
          ...(recentItems.length ? [{ label: t('Open recent'), items: recentItems }] : []),
          '-',
          cmd.save,
          cmd.saveAs,
          '-',
          {
            label: t('Export'),
            items: [
              cmd.exportPdf, cmd.exportPng, cmd.exportSvg, '-',
              cmd.exportGuitarPro, cmd.exportMusicXml, cmd.exportMidi, cmd.exportAscii, '-',
              cmd.exportAudio,
            ],
          },
          cmd.batchConvert,
          cmd.print,
        ],
      },
      {
        label: t('Edit'),
        items: [
          cmd.undo, cmd.redo, '-', cmd.cut, cmd.copy, cmd.paste, '-',
          cmd.insertBeat, cmd.delete, cmd.deleteBeats, '-',
          cmd.preferences,
        ],
      },
      {
        label: t('Note'),
        items: [
          { label: t('Note value'), items: noteValues },
          { label: t('Voice'), items: voices },
          { label: t('Drum piece'), items: drumPieces },
          '-',
          cmd.rest, cmd.tie, cmd.dot,
          { label: t('Tuplet'), items: tuplets(false) },
          { label: t('Tuplet inside the tuplet'), items: tuplets(true) },
          '-',
          cmd.hammer,
          {
            label: t('Slide'),
            items: [cmd.slideShift, cmd.slideLegato, '-', cmd.slideInBelow, cmd.slideInAbove, cmd.slideOutDown, cmd.slideOutUp],
          },
          cmd.bend,
          cmd.bendEditor,
          cmd.vibrato,
          cmd.wideVibrato,
          {
            label: t('Harmonic'),
            items: [
              cmd.harmonicNatural, cmd.harmonicArtificial, cmd.harmonicPinch, cmd.harmonicTapped, cmd.harmonicSemi,
              '-',
              ...harmonicTouches,
            ],
          },
          '-',
          cmd.palmMute, cmd.letRing, cmd.dead, cmd.ghost, cmd.staccato, cmd.accent, cmd.heavyAccent, cmd.tapped,
          '-',
          cmd.trill,
          cmd.grace,
          { label: t('Fretting-hand finger'), items: fingering('left_finger', 'T1234') },
          { label: t('Picking-hand finger'), items: fingering('right_finger', 'pimac') },
        ],
      },
      {
        label: t('Beat'),
        items: [
          { label: t('Dynamic'), items: dynamics },
          cmd.crescendo,
          cmd.decrescendo,
          '-',
          cmd.strokeDown, cmd.strokeUp,
          {
            label: t('Strum'),
            items: [cmd.strumDown, cmd.strumUp, cmd.arpeggioDown, cmd.arpeggioUp, cmd.rasgueado],
          },
          cmd.tremolo,
          cmd.slap,
          cmd.pop,
          cmd.whammy,
          '-',
          cmd.chordName, cmd.beatText, cmd.lyrics, cmd.fermata,
          '-',
          { label: t('Fade'), items: fades },
          cmd.volumeChange, cmd.panChange, cmd.soundChange,
        ],
      },
      {
        label: t('Score'),
        items: [
          cmd.insertBar, cmd.removeBar, cmd.timeSignature, cmd.keySignature,
          '-',
          cmd.repeatStart, cmd.repeatEnd, cmd.endings,
          { label: t('Segno, coda and fine'), items: barList('targets', TARGETS) },
          { label: t('Jumps at the end of the bar'), items: barList('jumps', JUMPS) },
          '-',
          cmd.section, cmd.barTempo, cmd.tempoGradual, cmd.timer,
          { label: t('Feel'), items: barChoices('feel', FEELS) },
          '-',
          cmd.scoreInfo, cmd.stylesheet,
          {
            label: t('Lines and pages'),
            items: [
              cmd.lineBreak, cmd.pageBreak, '-',
              cmd.barWider, cmd.barNarrower, cmd.barWidthReset, '-',
              cmd.barsPerLine,
            ],
          },
          '-',
          cmd.play, cmd.metronome, cmd.countIn, cmd.loop, cmd.trainer, cmd.replayRepeats,
          { label: t('Playback speed'), items: [...speeds, cmd.speedOther, '-', cmd.slower, cmd.faster] },
          { label: t('Sound'), items: [cmd.soundDefault, cmd.soundCustom, cmd.soundModels] },
          cmd.masterEffects,
        ],
      },
      {
        label: t('Track'),
        items: [
          ...tracks,
          '-',
          cmd.previousTrack, cmd.nextTrack,
          '-',
          cmd.addTrack, cmd.trackSettings, cmd.drumKit, cmd.trackEffects, cmd.trackUp, cmd.trackDown, cmd.removeTrack,
          '-',
          cmd.audioTrack,
          '-',
          cmd.mute, cmd.solo,
        ],
      },
      {
        label: t('View'),
        items: [
          cmd.zoomIn, cmd.zoomOut, '-',
          { label: t('Layout'), items: layouts },
          ...notations,
          cmd.multitrack, cmd.designMode,
          '-',
          cmd.instrument, cmd.pedalboard, cmd.audioPanel, cmd.trackPane, cmd.inspector, cmd.compact, cmd.beatLights,
          { label: t('Theme'), items: themes },
        ],
      },
      { label: t('Tools'), items: [cmd.tuner, cmd.lineIn, '-', cmd.chordLibrary, cmd.scaleLibrary, cmd.findScale] },
      { label: t('Help'), items: [cmd.commandPalette, cmd.shortcuts] },
    ];
  });

  /** The keys that are not commands, listed with the shortcuts but not to be changed. */
  const fixedKeys: ShortcutGroup[] = [
    {
      title: t('Moving'),
      rows: [
        { keys: '← → ↑ ↓', label: t('Move between beats and strings, or on a piano the notes of the key') },
        { keys: t('Shift+arrows'), label: t('Extend the selection') },
        { keys: 'Ctrl+← / Ctrl+→', label: t('Previous or next bar') },
        { keys: 'Home / End', label: t('First or last bar') },
      ],
    },
    {
      title: t('Entering notes'),
      rows: [
        { keys: '0–9', label: t('Fret; two digits typed quickly make one number') },
        { keys: 'A–G', label: t('On a piano: the note of that name nearest the cursor, before any command on those keys') },
        { keys: 'Enter', label: t('On a piano: a note where the cursor is') },
        { keys: 'Alt+↑ / Alt+↓', label: t('On a piano: the note under the cursor a semitone up or down') },
        { keys: '+ / −', label: t('Shorter or longer note value') },
      ],
    },
    {
      title: t('Interface'),
      rows: [
        { keys: 'Tab / Shift+Tab', label: t('Move between the bars, palette and score') },
        { keys: t('Arrows'), label: t('Move within a bar, the palette or a menu') },
        { keys: 'Esc', label: t('Back to the score') },
      ],
    },
  ];

  const speedKeys = $derived([shownKeys(cmd.slower), shownKeys(cmd.faster)].filter((k) => k !== undefined));

  /** Closes the command palette, and runs what was chosen there as the menus would. */
  function runFromPalette(chosen?: Found) {
    paletteOpen = false;
    focusScore();
    if (!chosen) return;
    prefs.recentCommands = used(prefs.recentCommands, chosen.id);
    chosen.command.run();
  }

  // --- Keyboard ---

  function onKeydown(event: KeyboardEvent) {
    if (event.defaultPrevented || dialog || effectDialog || prompt || chordOpen || shortcutsOpen || paletteOpen || prefsOpen || styleOpen || batchOpen) return;
    if (!doc) return;
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

  /** The commands that can be given keys, from the menus. */
  const bindable = $derived(listCommands(menus).filter((found) => found.command.id));
  const bindableKeys = $derived(bindable.map(({ command }) => ({ id: command.id!, keys: command.keys })));
  const commandById = $derived(new Map(bindable.map(({ command }) => [command.id!, command])));
  const keyMap = $derived(keymap(bindableKeys, prefs.shortcuts));

  function keyAction(event: KeyboardEvent): (() => unknown) | undefined {
    const { key, shiftKey: shift, altKey: alt } = event;
    const ctrl = event.ctrlKey || event.metaKey;
    const move = (direction: string) => () => edit((d) => d.moveCursor(direction, shift));

    // Moving the cursor and entering notes are not commands, and their keys stay as they are.
    if (ctrl && !alt) {
      switch (key) {
        case 'ArrowLeft': return move('prev_bar');
        case 'ArrowRight': return move('next_bar');
        case 'Home': return move('first');
        case 'End': return move('last');
      }
    }
    // On a staff without strings, a semitone up or down for the note under the cursor.
    if (alt && !ctrl && pitched && (key === 'ArrowUp' || key === 'ArrowDown')) {
      return () => edit((d) => d.shiftPitch(key === 'ArrowUp' ? 1 : -1));
    }
    if (!ctrl && !alt) {
      if (pitched) {
        // Notes are entered by name; the letters keep their other meanings on strings.
        const letter = key.length === 1 ? 'cdefgab'.indexOf(key.toLowerCase()) : -1;
        if (letter >= 0) return () => edit((d) => d.enterLetter(letter));
        if (key === 'Enter') return () => edit((d) => d.enterNote());
      }
      if (/^[0-9]$/.test(key)) {
        return () => {
          const now = performance.now();
          const combine = now - lastDigitAt < DIGIT_WINDOW_MS;
          lastDigitAt = now;
          edit((d) => d.typeDigit(Number(key), combine));
        };
      }
      switch (key) {
        case 'ArrowLeft': return move('left');
        case 'ArrowRight': return move('right');
        case 'ArrowUp': return move('up');
        case 'ArrowDown': return move('down');
        case 'Home': return move('first');
        case 'End': return move('last');
        case '+':
        case '=': return () => edit((d) => d.shorter());
        case '-': return () => edit((d) => d.longer());
      }
    }

    const chord = chordOf(event);
    const command = chord ? commandById.get(keyMap.get(chord) ?? '') : undefined;
    return command && (() => command.disabled?.() || command.run());
  }

  onMount(() => {
    const started = run(async () => {
      await init();
      standardKit = JSON.parse(Document.drumKitJson());
      standardStyle = JSON.parse(Document.defaultStyleJson());
      pedalSpecs = JSON.parse(Document.pedalKindsJson());
      pedalPresets = JSON.parse(Document.pedalPresetsJson());
      const autosave = readAutosave();
      if (autosave) {
        try {
          const recovered = Document.load(autosave.bytes);
          // The recording is kept apart from the copy, and comes back if it is the score's.
          const recording = await autosaveRecording();
          if (recording && recovered.restoreRecording(recording.name, recording.bytes)) {
            keptRecording = `${recording.name}:${recording.bytes.length}`;
          }
          // A score saved before the app started again is as it was; one that was not is work to save.
          if (!autosave.saved) recovered.markUnsaved();
          setDocument(recovered, autosave.name, autosave.path);
          if (!autosave.saved) message = t('Recovered unsaved changes to {name}', { name: autosave.name });
          return;
        } catch {
          writeAutosave(null);
        }
      }
      void keepAutosaveRecording(null);
      setDocument(new Document(), UNTITLED, null);
      const reopen = takeReopenAfterRestart();
      if (reopen) openRecent(reopen);
    });
    // A score the system opens with the installed web app, once the app is up.
    void started.then(() =>
      onLaunch((file) =>
        run(async () => {
          if (!(await discardChanges())) return;
          load(file);
        }),
      ),
    );
    // The samples are big, so they are fetched once the app is up; until they arrive,
    // playback uses the built-in models.
    void applySound().then((problem) => {
      if (problem && prefs.sound !== 'default') message = problem;
    });
    return () => {
      player.stop();
      doc?.free();
      // Nothing still pending, such as the autosave timer, may touch the freed document.
      doc = undefined;
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
      <h1 class="brand">gtrTab</h1>
      <MenuBar {menus} onclose={focusScore} />
      <span class="rule"></span>
      <div class="cluster" role="toolbar" aria-label={t('File and edit')} use:roving>
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
      <div class="transport" role="toolbar" aria-label={t('Playback and view')} use:roving>
        <button
          type="button"
          class="tool play"
          aria-label={playing ? t('Stop') : cmd.play.label}
          aria-keyshortcuts={aria(shownKeys(cmd.play))}
          use:tip={{ label: playing ? t('Stop') : cmd.play.label, keys: keyOf(cmd.play) }}
          onmousedown={(e) => e.preventDefault()}
          onclick={togglePlay}
        >
          <Icon name={playing ? 'stop' : 'play'} size={16} />
        </button>
        <ToolButton command={cmd.metronome} />
        <ToolButton command={cmd.countIn} />
        <ToolButton command={cmd.beatLights} />
        {#if prefs.beatLights}
          <BeatLights
            count={pulse?.count ?? masterBar?.time_signature.numerator ?? 4}
            lit={pulse?.index ?? null}
            countIn={pulse?.count_in ?? false}
          />
        {/if}
        {#if lineOn}<ToolButton command={cmd.lineInOn} />{/if}
        <ToolButton command={cmd.loop} />
        <ToolButton command={cmd.trainer} />
        <button
          type="button"
          class="chip"
          aria-label={t('Playback speed: {speed}%', { speed })}
          aria-keyshortcuts={aria(speedKeys.join(' / ') || undefined)}
          use:tip={{ label: t('Playback speed'), keys: speedKeys.length ? keyLabel(speedKeys.join(' / ')) : undefined }}
          onmousedown={(e) => e.preventDefault()}
          onclick={cmd.speedOther.run}
        >
          <Icon name="speed" size={16} />
          {speed}%
        </button>
        <button
          type="button"
          class="chip"
          aria-label={t('Tempo: {tempo} beats per minute', { tempo: score.tempo })}
          use:tip={{ label: t('Tempo') }}
          onmousedown={(e) => e.preventDefault()}
          onclick={cmd.scoreInfo.run}
        >
          <Icon name="tempo" size={16} />
          = {score.tempo}
        </button>
        <span class="rule"></span>
        <ToolButton command={cmd.zoomOut} />
        <span class="zoom" aria-label={t('Zoom {percent}%', { percent: zoom * 100 })}>{zoom * 100}%</span>
        <ToolButton command={cmd.zoomIn} />
        <span class="rule"></span>
        <ToolButton command={cmd.compact} />
        <ToolButton command={cmd.trackPane} />
        <ToolButton command={cmd.pedalboard} />
        <ToolButton command={cmd.inspector} />
        <ToolButton command={cmd.shortcuts} />
        <button
          type="button"
          class="tool"
          aria-label={t('Theme: {theme}', { theme: THEME_NAMES[prefs.theme] })}
          use:tip={{ label: t('Theme: {theme}', { theme: THEME_NAMES[prefs.theme] }) }}
          onmousedown={(e) => e.preventDefault()}
          onclick={() => setTheme(THEMES[(THEMES.indexOf(prefs.theme) + 1) % THEMES.length])}
        >
          <Icon name={THEME_ICONS[prefs.theme]} />
        </button>
      </div>
    {/if}
  </header>

  {#if score && editor && doc}
    <div class="work" role="main">
      <Palette sections={palette} {compact} collapsed={prefs.collapsed} ontoggle={toggleSection} />
      <div class="stage">
        <ScoreView
          getLayout={screenLayout}
          design={prefs.design}
          widths={score.master_bars.map((bar) => bar.width)}
          onwiden={(bar, percent) =>
            edit((d) => {
              d.goToBar(bar);
              const width = Math.max(MIN_BAR_WIDTH, Math.min(MAX_BAR_WIDTH, percent));
              d.setBarProperty('width', JSON.stringify(width === 100 ? null : width));
            })}
          revision={`${editor.revision}:${editor.cursor.track}:${editor.cursor.staff}:${editor.cursor.voice}:${viewJson}:${fileName}`}
          {editor}
          keys={score.master_bars.map((bar) => bar.key_signature)}
          {playhead}
          {zoom}
          theme={look}
          onpick={(bar, beat, string, extend, staff) =>
            edit((d) => {
              if (staff) {
                d.setTrack(staff.track);
                d.setStaff(staff.staff);
              }
              d.setCursor(bar, beat, string, extend);
            })}
          onview={(bar) => (viewBar = bar)}
        />
        {#if prefs.instrument && cursorStaff}
          <InstrumentView
            kind={instrumentKind}
            tuning={cursorStaff.tuning.strings}
            capo={cursorStaff.capo}
            kit={drumKit}
            notes={instrumentNotes}
            sounding={!!playhead}
            cursor={editor.cursor.string}
            {marks}
            onpick={pickOnInstrument}
            onclearmarks={() => (marks = null)}
            onclose={() => {
              prefs.instrument = false;
              focusScore();
            }}
          />
        {/if}
        {#if prefs.audio && score.audio}
          <AudioPanel
            audio={score.audio}
            bars={recordingBars}
            peaks={recordingPeaks}
            problem={recordingProblem}
            cursorBar={editor.cursor.bar}
            theme={look}
            now={recordingNow}
            onsync={(place, millis) => edit((d) => d.syncRecording(place, millis))}
            onunsync={(place) => edit((d) => d.unsyncRecording(place))}
            onseek={(bar) => edit((d) => d.goToBar(bar))}
            onmix={(mix) => score?.audio && setAudio({ ...score.audio, mix })}
            onfind={findBars}
            onreplace={importRecording}
            onremove={() => setAudio(null)}
            onclose={() => {
              prefs.audio = false;
              focusScore();
            }}
          />
        {/if}
        {#if prefs.pedalboard}
          <Pedalboard
            specs={pedalSpecs}
            presets={pedalPresets}
            {userPresets}
            master={pedalMaster}
            trackName={score.tracks[editor.cursor.track]?.name ?? ''}
            sounds={trackSounds}
            sound={soundShown}
            chain={pedalChain}
            onchoose={(master) => (pedalMaster = master)}
            onsound={(sound) => (pedalSound = sound)}
            onaddsound={() => (soundDialog = 0)}
            oneditsound={() => (soundDialog = soundShown)}
            onremovesound={removeSound}
            onpreview={previewPedals}
            onset={setPedals}
            onsavepreset={savePreset}
            ondeletepreset={deletePreset}
            onclose={() => {
              prefs.pedalboard = false;
              focusScore();
            }}
          />
        {/if}
        {#if prefs.tracks}
          <TrackPane
            {score}
            track={editor.cursor.track}
            bar={editor.cursor.bar}
            playBar={playhead?.bar ?? null}
            {viewBar}
            height={prefs.tracksHeight}
            commands={[cmd.addTrack, cmd.trackSettings, cmd.trackUp, cmd.trackDown, cmd.removeTrack]}
            onpick={pickTrack}
            onmix={setMix}
            onmove={(from, to) => edit((d) => d.moveTrack(from, to))}
            onproperties={(track) => pickTrack(track, null).then(() => (dialog = 'track'))}
            onresize={(height) => (prefs.tracksHeight = height)}
            audio={audioRow}
            onaudiomix={(mix) => score?.audio && setAudio({ ...score.audio, mix })}
            onaudio={() => (prefs.audio = !prefs.audio)}
          />
        {/if}
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
            {drumKit}
          />
        </div>
      {/if}
    </div>
  {:else}
    <main>{failed ? '' : t('Loading…')}</main>
  {/if}

  <!-- The score is drawn on a canvas, which says nothing to a screen reader: this says what
       is under the cursor each time it moves or the music there changes. -->
  <p class="sr-only" aria-live="polite" aria-atomic="true" data-position>{position}</p>

  <footer class:failed>
    <span role="status">{message || (keyOf(cmd.shortcuts) ? t('Press {keys} for the keyboard shortcuts', { keys: keyOf(cmd.shortcuts)! }) : '')}</span>
    {#if editor && score}
      <span>
        {score.tracks[editor.cursor.track]?.name}{#if (score.tracks[editor.cursor.track]?.staves.length ?? 1) > 1}, staff {editor.cursor.staff + 1}{/if}
        · Voice {editor.cursor.voice + 1} · Bar {editor.cursor.bar + 1} of {score.master_bars.length}
        · Beat {editor.cursor.beat + 1}
      </span>
    {/if}
  </footer>
</div>

{#if dialog && score && editor}
  <Dialogs
    kind={dialog}
    {score}
    track={editor.cursor.track}
    staff={editor.cursor.staff}
    bar={editor.cursor.bar}
    onscore={(info, tempo) => {
      dialog = null;
      edit((d) => {
        const old = score!;
        if (JSON.stringify(info) !== JSON.stringify(old.info)) d.setInfo(JSON.stringify(info));
        if (tempo !== old.tempo) d.setTempo(tempo);
      });
    }}
    ontime={(numerator, denominator) => {
      dialog = null;
      edit((d) => d.setTimeSignature(numerator, denominator));
    }}
    onkey={(key) => {
      dialog = null;
      edit((d) => d.setKeySignature(key));
    }}
    ontrack={(name, instrument, strings, capo, mix) => {
      dialog = null;
      edit((d) => {
        if (!mix) return d.addTrack(name, instrument, new Uint8Array(strings), capo);
        const index = trackIndex();
        const old = score!.tracks[index];
        const staff = old.staves[editor!.cursor.staff] ?? old.staves[0];
        // A change to or from a piano rewrites the notes for the new instrument, on its
        // usual strings, so it goes before any tuning; the tuning is then set on those.
        const piano = (i: string) => i === 'piano';
        const rewritten = piano(instrument) !== piano(old.instrument);
        if (rewritten) d.setTrackInfo(index, name, instrument);
        // Otherwise the tuning goes first: if notes rule it out, nothing else has changed
        // yet. A piano and a drum kit have none to set.
        const retuned = capo !== staff.capo || strings.join() !== staff.tuning.strings.join();
        if (strings.length && (retuned || rewritten)) {
          d.setTuning(new Uint8Array(strings), capo);
        }
        if (!rewritten && (name !== old.name || instrument !== old.instrument)) {
          d.setTrackInfo(index, name, instrument);
        }
        d.setMix(index, JSON.stringify(mix));
      });
    }}
    onclose={() => (dialog = null)}
  />
{/if}

{#if effectDialog && editor}
  <EffectDialog
    kind={effectDialog}
    note={editor.note}
    beatEffects={editor.beat_effects}
    strings={score?.tracks[editor.cursor.track]?.staves[editor.cursor.staff]?.tuning.strings.length ?? 6}
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

{#if chordOpen && score && editor}
  {@const staff = score.tracks[editor.cursor.track].staves[editor.cursor.staff]}
  <ChordDialog
    tuning={staff.tuning.strings.map((pitch) => pitch + staff.capo)}
    name={beatEffects.chord ?? ''}
    diagram={beatEffects.diagram ?? null}
    notes={staff.bars[editor.cursor.bar]?.voices[editor.cursor.voice]?.beats[editor.cursor.beat]?.notes ?? []}
    onapply={(name, diagram) =>
      edit((d) => {
        if (name !== (beatEffects.chord ?? null)) d.setBeatEffect('chord', JSON.stringify(name));
        if (JSON.stringify(diagram) !== JSON.stringify(beatEffects.diagram ?? null)) {
          d.setBeatEffect('diagram', JSON.stringify(diagram));
        }
      })}
    onclose={() => (chordOpen = false)}
  />
{/if}

{#if prompt}
  <PromptDialog {prompt} onclose={() => (prompt = null)} />
{/if}

{#if soundDialog !== null && pedalTrackOf}
  <SoundDialog
    sound={soundDialog > 0 ? (trackSounds[soundDialog - 1] ?? null) : null}
    instrument={pedalTrackOf.instrument}
    onapply={nameSound}
    onclose={() => (soundDialog = null)}
  />
{/if}

{#if lineDialog}
  <LineInDialog
    running={lineOn}
    trackName={soundShown > 0 ? `${lineTrack?.name}: ${trackSounds[soundShown - 1]?.name}` : (lineTrack?.name ?? '')}
    trackPedals={pedalNames(soundChain)}
    masterPedals={pedalNames(score?.effects)}
    levels={lineLevels}
    latency={lineLatency}
    settings={lineSettings}
    onstart={startLine}
    ongain={setLineGain}
    onstop={stopLine}
    onpedals={() => {
      lineDialog = false;
      pedalMaster = false;
      prefs.pedalboard = true;
    }}
    onclose={() => (lineDialog = false)}
  />
{/if}

{#if tunerOpen}
  <TunerDialog strings={openStrings} onclose={() => (tunerOpen = false)} />
{/if}

{#if chordLibraryOpen}
  <ChordLibrary
    open={openStrings}
    onshow={showMarks}
    onplay={(pitches) => audition(pitches, 0.03, 2)}
    onput={(frets, name) => {
      chordLibraryOpen = false;
      edit((d) => d.putShape(Int8Array.from(frets), name));
    }}
    onclose={() => (chordLibraryOpen = false)}
  />
{/if}

{#if scaleLibrary}
  <ScaleDialog
    open={openStrings.length ? openStrings : STANDARD_GUITAR}
    neckNote={openStrings.length ? undefined : 'Shown on a guitar in standard tuning, as this track has no strings.'}
    root={scaleLibrary.root}
    scale={scaleLibrary.scale}
    found={scaleLibrary.found}
    onshow={showMarks}
    onplay={(pitches) => audition(pitches, 0.3, 0.35)}
    onclose={() => (scaleLibrary = null)}
  />
{/if}

{#if trainerOpen}
  <TrainerDialog
    {trainer}
    {speed}
    min={MIN_SPEED}
    max={MAX_SPEED}
    on={training}
    onapply={setTrainer}
    onclose={() => (trainerOpen = false)}
  />
{/if}

{#if kitDialog && editor}
  <KitDialog
    kit={drumKit}
    standard={standardKit}
    onapply={(kit) => {
      kitDialog = false;
      edit((d) => d.setDrumKit(trackIndex(), JSON.stringify(kit)));
    }}
    onclose={() => (kitDialog = false)}
  />
{/if}

{#if shortcutsOpen}
  <Shortcuts
    fixed={fixedKeys}
    commands={bindable}
    custom={prefs.shortcuts}
    onchange={(custom) => (prefs.shortcuts = custom)}
    onclose={() => (shortcutsOpen = false)}
  />
{/if}

{#if styleOpen && standardStyle}
  <StyleDialog
    style={currentStyle()}
    standard={standardStyle}
    onapply={setStyle}
    onclose={() => {
      styleOpen = false;
      focusScore();
    }}
  />
{/if}

{#if batchOpen}
  <BatchDialog
    onconvert={batchConvert}
    onclose={() => {
      batchOpen = false;
      focusScore();
    }}
  />
{/if}

{#if prefsOpen}
  <Preferences
    {prefs}
    set={setPref}
    {languageAtStart}
    sounds={[cmd.soundDefault, cmd.soundCustom, cmd.soundModels]}
    shortcuts={{ ...cmd.shortcuts, run: () => ((prefsOpen = false), (shortcutsOpen = true)) }}
    palette={{ ...cmd.commandPalette, run: () => ((prefsOpen = false), (paletteOpen = true)) }}
    onrestart={restart}
    onreset={resetPrefs}
    onclose={() => {
      prefsOpen = false;
      focusScore();
    }}
  />
{/if}

{#if paletteOpen}
  <CommandPalette commands={listCommands(menus)} recent={prefs.recentCommands} onclose={runFromPalette} />
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
    flex-wrap: wrap;
    min-width: 0;
  }

  .transport {
    flex: 1 1 auto;
    flex-wrap: wrap;
  }

  .brand {
    margin: 0;
    padding: 0 var(--space-3) 0 var(--space-2);
    color: var(--accent);
    font-size: inherit;
    font-weight: 700;
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
