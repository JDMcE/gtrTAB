// Mirrors the serde JSON of the `gtr-model` and `gtr-edit` crates. Keep in sync by hand
// until the types are generated from Rust.

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
  accent?: 'normal' | 'heavy';
  trill?: { fret: number; speed: NoteValue };
  grace?: { fret: number; on_beat: boolean; hammer: boolean };
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

export interface Track {
  name: string;
  staves: Staff[];
}

export interface TimeSignature {
  numerator: number;
  denominator: number;
}

export interface MasterBar {
  time_signature: TimeSignature;
}

export interface ScoreInfo {
  title: string;
  artist: string;
  album: string;
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
      align: 'left' | 'center';
      knockout: boolean;
      c: Color;
    };

export interface SystemBox {
  top: number;
  y: number;
  bottom: number;
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
