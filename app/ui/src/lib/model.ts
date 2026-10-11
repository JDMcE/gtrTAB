// Mirrors the serde JSON of the `gtr-model` and `gtr-edit` crates. Keep in sync by hand
// until the types are generated from Rust.

import type { IconName } from './ui/icons';

export type NoteValue =
  | 'whole'
  | 'half'
  | 'quarter'
  | 'eighth'
  | 'sixteenth'
  | 'thirty_second'
  | 'sixty_fourth';

export interface Tuplet {
  actual: number;
  normal: number;
}

export interface Duration {
  value: NoteValue;
  dots: number;
  tuplet: Tuplet | null;
  /** A second tuplet inside the first. */
  nested?: Tuplet;
}

export type Slide =
  | 'shift'
  | 'legato'
  | 'in_from_below'
  | 'in_from_above'
  | 'out_down'
  | 'out_up';

export type BendKind = 'bend' | 'bend_release' | 'prebend' | 'prebend_release';

export interface CurvePoint {
  /** From 0 at the start of the note to 60 at its end. */
  position: number;
  /** Pitch offset in quarter tones: 4 is a full step. */
  value: number;
}

/** A pitch curve over a note, for string bends and the whammy bar. */
export interface Bend {
  points: CurvePoint[];
}

export type Finger = 'thumb' | 'index' | 'middle' | 'ring' | 'pinky';
export type Dynamic = 'ppp' | 'pp' | 'p' | 'mp' | 'mf' | 'f' | 'ff' | 'fff';
export type Strum = 'brush_down' | 'brush_up' | 'arpeggio_down' | 'arpeggio_up' | 'rasgueado';

export type Harmonic = 'natural' | 'artificial' | 'pinch' | 'tapped' | 'semi';

/** Techniques on a note. Fields that are off are left out of the JSON. */
export interface NoteEffects {
  hammer?: boolean;
  slide?: Slide;
  bend?: Bend;
  vibrato?: 'slight' | 'wide';
  harmonic?: Harmonic;
  /** Frets above the note at which the string is touched for the harmonic. */
  harmonic_touch?: number;
  accent?: 'normal' | 'heavy';
  trill?: { fret: number; speed: NoteValue };
  grace?: { notes: { string: number; fret: number }[]; on_beat: boolean; hammer: boolean };
  left_finger?: Finger;
  right_finger?: Finger;
  palm_mute?: boolean;
  let_ring?: boolean;
  dead?: boolean;
  ghost?: boolean;
  staccato?: boolean;
  tapped?: boolean;
}

export interface BeatEffects {
  tremolo?: NoteValue;
  stroke?: 'down' | 'up';
  strum?: Strum;
  slap?: 'slap' | 'pop';
  whammy?: Bend;
  dynamic?: Dynamic;
  hairpin?: 'crescendo' | 'decrescendo';
  fermata?: boolean;
  /** The name of the chord from this beat on. */
  chord?: string;
  diagram?: ChordDiagram;
  /** The track's volume and pan from this beat on, on the scale of the mixer. */
  volume?: number;
  pan?: number;
  /** Free text above the beat. */
  text?: string;
  /** The syllable sung on this beat in each line of the lyrics. */
  lyrics?: string[];
}

export interface Note {
  /** Index into the staff tuning; 0 is the highest string. */
  string: number;
  fret: number;
  tied: boolean;
  effects?: NoteEffects;
}

export interface Beat {
  duration: Duration;
  notes: Note[];
  effects?: BeatEffects;
}

export interface Voice {
  beats: Beat[];
}

export interface Bar {
  voices: Voice[];
}

export interface Tuning {
  /** Open-string MIDI pitches from the highest string to the lowest. */
  strings: number[];
}

export interface Staff {
  tuning: Tuning;
  capo: number;
  bars: Bar[];
}

export type Instrument =
  | 'steel_guitar'
  | 'nylon_guitar'
  | 'clean_guitar'
  | 'overdrive_guitar'
  | 'distortion_guitar'
  | 'bass'
  | 'ukulele'
  | 'banjo'
  | 'mandolin'
  | 'piano'
  | 'drums';

/** Instruments in the order they are offered, with the names `gtr_model::Instrument` gives. */
export const INSTRUMENTS: [Instrument, string][] = [
  ['steel_guitar', 'Steel guitar'],
  ['nylon_guitar', 'Nylon guitar'],
  ['clean_guitar', 'Clean guitar'],
  ['overdrive_guitar', 'Overdrive guitar'],
  ['distortion_guitar', 'Distortion guitar'],
  ['bass', 'Bass'],
  ['ukulele', 'Ukulele'],
  ['banjo', 'Banjo'],
  ['mandolin', 'Mandolin'],
  ['piano', 'Piano'],
  ['drums', 'Drums'],
];

export const INSTRUMENT_ICONS: Record<Instrument, IconName> = {
  steel_guitar: 'acousticGuitar',
  nylon_guitar: 'acousticGuitar',
  clean_guitar: 'electricGuitar',
  overdrive_guitar: 'electricGuitar',
  distortion_guitar: 'electricGuitar',
  bass: 'bassGuitar',
  ukulele: 'ukulele',
  banjo: 'banjo',
  mandolin: 'ukulele',
  piano: 'piano',
  drums: 'drums',
};

/** Instruments that share tunings. */
export type Family = 'guitar' | 'bass' | 'ukulele' | 'banjo' | 'mandolin' | 'keyboard' | 'drums';
export const FAMILIES: Record<Instrument, Family> = {
  steel_guitar: 'guitar',
  nylon_guitar: 'guitar',
  clean_guitar: 'guitar',
  overdrive_guitar: 'guitar',
  distortion_guitar: 'guitar',
  bass: 'bass',
  ukulele: 'ukulele',
  banjo: 'banjo',
  mandolin: 'mandolin',
  piano: 'keyboard',
  drums: 'drums',
};

/** The tuning a track for each kind of instrument starts in, highest string first. A piano
 *  and a drum kit are not tuned: the core gives them the staves they need. */
export const DEFAULT_TUNINGS: Record<Family, number[]> = {
  guitar: [64, 59, 55, 50, 45, 40],
  bass: [43, 38, 33, 28],
  ukulele: [69, 64, 60, 67],
  banjo: [62, 59, 55, 50, 67],
  mandolin: [76, 69, 62, 55],
  keyboard: [],
  drums: [],
};

/** A piece of the drum kit: on a drum track a note's fret is the piece's number. */
export interface DrumPiece {
  number: number;
  name: string;
}

/** How a chord is fingered: a fret per string from the highest; 0 is open, -1 not played. */
export interface ChordDiagram {
  frets: number[];
}

/** A track's mixer settings. */
export interface Mix {
  /** From 0 to 100; 75 is the track's natural level. */
  volume: number;
  /** From -100 (left) to 100 (right). */
  pan: number;
  mute: boolean;
  solo: boolean;
}

export interface Track {
  name: string;
  instrument: Instrument;
  mix: Mix;
  staves: Staff[];
}

export interface TimeSignature {
  numerator: number;
  denominator: number;
}

export interface MasterBar {
  time_signature: TimeSignature;
  /** Sharps in the key signature if positive, flats if negative. */
  key_signature: number;
  repeat_start?: boolean;
  /** Times a repeat ending with this bar is played in all. */
  repeat_count?: number;
  /** Passes of a repeat on which the bar is played, one bit per pass. */
  alternate_endings?: number;
  section?: string;
  target?: Target;
  jump?: Jump;
  /** A new tempo from this bar on. */
  tempo?: number;
  /** New tempos from beats inside the bar; `position` is `[numerator, denominator]` of whole notes. */
  tempo_changes?: { position: [number, number]; tempo: number }[];
  feel?: Feel;
  /** The time elapsed is written above the bar. */
  timer?: boolean;
}

export type Target = 'segno' | 'coda' | 'fine';
export const TARGETS: [Target, string][] = [
  ['segno', 'Segno'],
  ['coda', 'Coda'],
  ['fine', 'Fine'],
];

export type Jump =
  | 'da_capo'
  | 'da_capo_al_fine'
  | 'da_capo_al_coda'
  | 'dal_segno'
  | 'dal_segno_al_fine'
  | 'dal_segno_al_coda'
  | 'to_coda';
export const JUMPS: [Jump, string][] = [
  ['da_capo', 'D.C. (back to the start)'],
  ['da_capo_al_fine', 'D.C. al Fine'],
  ['da_capo_al_coda', 'D.C. al Coda'],
  ['dal_segno', 'D.S. (back to the segno)'],
  ['dal_segno_al_fine', 'D.S. al Fine'],
  ['dal_segno_al_coda', 'D.S. al Coda'],
  ['to_coda', 'To Coda'],
];

export type Feel = 'straight' | 'swing_eighths' | 'swing_sixteenths';
export const FEELS: [Feel, string][] = [
  ['straight', 'Straight'],
  ['swing_eighths', 'Swing eighths'],
  ['swing_sixteenths', 'Swing sixteenths'],
];

/** Key signatures from seven flats to seven sharps, named by their major and minor keys. */
export const KEYS: [number, string][] = [
  [-7, 'C♭ major / A♭ minor (7 flats)'],
  [-6, 'G♭ major / E♭ minor (6 flats)'],
  [-5, 'D♭ major / B♭ minor (5 flats)'],
  [-4, 'A♭ major / F minor (4 flats)'],
  [-3, 'E♭ major / C minor (3 flats)'],
  [-2, 'B♭ major / G minor (2 flats)'],
  [-1, 'F major / D minor (1 flat)'],
  [0, 'C major / A minor'],
  [1, 'G major / E minor (1 sharp)'],
  [2, 'D major / B minor (2 sharps)'],
  [3, 'A major / F♯ minor (3 sharps)'],
  [4, 'E major / C♯ minor (4 sharps)'],
  [5, 'B major / G♯ minor (5 sharps)'],
  [6, 'F♯ major / D♯ minor (6 sharps)'],
  [7, 'C♯ major / A♯ minor (7 sharps)'],
];

/** Which staves a track is shown on; mirrors `gtr_layout::Notation`. */
export type Notation = 'tab' | 'standard' | 'both' | 'slash';
export const NOTATIONS: [Notation, string][] = [
  ['both', 'Notation and tab'],
  ['tab', 'Tab only'],
  ['standard', 'Notation only'],
  ['slash', 'Rhythm slashes'],
];

/** The most voices a bar can hold. */
export const MAX_VOICES = 4;

export interface ScoreInfo {
  title: string;
  subtitle: string;
  artist: string;
  album: string;
  /** Who wrote the words. */
  words: string;
  /** Who wrote the music. */
  music: string;
  /** Who wrote the score down. */
  transcriber: string;
  copyright: string;
}

export interface Score {
  info: ScoreInfo;
  tempo: number;
  master_bars: MasterBar[];
  tracks: Track[];
}

export interface BeatRef {
  track: number;
  staff: number;
  bar: number;
  voice: number;
  beat: number;
}

export type Command =
  | { type: 'put_note'; at: BeatRef; note: Note }
  | { type: 'remove_note'; at: BeatRef; string: number }
  | { type: 'insert_beat'; at: BeatRef; beat: Beat }
  | { type: 'remove_beat'; at: BeatRef }
  | { type: 'set_duration'; at: BeatRef; duration: Duration }
  | { type: 'insert_bar'; index: number; master_bar: MasterBar; bars: Bar[][] }
  | { type: 'remove_bar'; index: number }
  | { type: 'set_time_signature'; bar: number; time_signature: TimeSignature }
  | { type: 'set_tempo'; tempo: number };

export interface Cursor {
  track: number;
  bar: number;
  beat: number;
  string: number;
  /** The voice being edited, counted from zero. */
  voice: number;
  /** The staff of the track being edited: a piano has one for each hand. */
  staff: number;
}

export interface EditorState {
  cursor: Cursor;
  /** First and last selected `[bar, beat]`, inclusive. */
  selection: [[number, number], [number, number]] | null;
  duration: Duration;
  /** The note under the cursor. */
  note: Note | null;
  /** Effects of the beat under the cursor. */
  beat_effects: BeatEffects | null;
  /** The tempo written to change on the cursor's beat. */
  tempo_change: number | null;
  can_undo: boolean;
  can_redo: boolean;
  can_paste: boolean;
  dirty: boolean;
  /** Changes whenever the score does. */
  revision: number;
}

export type Color = 'ink' | 'muted' | 'error';

export type Item =
  | { t: 'line'; x1: number; y1: number; x2: number; y2: number; w: number; c: Color }
  | { t: 'rect'; x: number; y: number; w: number; h: number; c: Color }
  | { t: 'dot'; x: number; y: number; r: number }
  | { t: 'arc'; x1: number; x2: number; y: number; h: number }
  | { t: 'curve'; x1: number; y1: number; cx: number; cy: number; x2: number; y2: number }
  | {
      t: 'text';
      x: number;
      y: number;
      s: string;
      size: number;
      font: 'text' | 'fret' | 'music';
      align: 'left' | 'center' | 'right';
      knockout: boolean;
      c: Color;
    };

export interface RowBox {
  track: number;
  /** Which of the track's staves this is, counted from the top. */
  staff: number;
  top: number;
  bottom: number;
  /** Y of the track's first string line, or of the top line of its notation. */
  y: number;
  /** Number of string lines shown; zero without a tab staff. */
  strings: number;
}

export interface SystemBox {
  top: number;
  /** Y of the first string line of the track being edited, or of the top line of its
   *  notation when no tab is shown. */
  y: number;
  bottom: number;
  /** Top and bottom staff lines of the track being edited. */
  cursor_top: number;
  cursor_bottom: number;
  rows: RowBox[];
}

export interface BarBox {
  system: number;
  x: number;
  w: number;
  beats: number[];
  slot: number | null;
  status: 'empty' | 'incomplete' | 'complete' | 'overfull';
}

export interface Layout {
  width: number;
  height: number;
  string_spacing: number;
  string_count: number;
  /** Whether tab staves are shown, which is where the cursor picks out a string. */
  tab: boolean;
  items: Item[];
  systems: SystemBox[];
  bars: BarBox[];
}

export interface BeatTime {
  bar: number;
  beat: number;
  time: number;
}

export interface Timeline {
  beats: BeatTime[];
  duration: number;
}

export const NOTE_VALUES: NoteValue[] = [
  'whole',
  'half',
  'quarter',
  'eighth',
  'sixteenth',
  'thirty_second',
  'sixty_fourth',
];
