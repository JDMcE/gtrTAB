// The shapes of what crosses from the Rust core: the score, the editor's state and the
// laid-out page. The types are generated from the Rust ones, by `cargo test` into
// `./generated`, and gathered here together with what the interface adds to them: the
// names and the order it shows things in. Do not write a type here that Rust has.

import type { BeatTime } from './generated/BeatTime';
import type { Pulse } from './generated/Pulse';
import type { Fade } from './generated/Fade';
import type { Feel } from './generated/Feel';
import type { Instrument } from './generated/Instrument';
import type { Jump } from './generated/Jump';
import type { KitPiece } from './generated/KitPiece';
import type { Notation } from './generated/Notation';
import type { NoteValue } from './generated/NoteValue';
import type { Target } from './generated/Target';
import type { IconName } from './ui/icons';

export type { Accent } from './generated/Accent';
export type { Fade } from './generated/Fade';
export type { Align } from './generated/Align';
export type { Bar } from './generated/Bar';
export type { BarBox } from './generated/BarBox';
export type { BarStatus } from './generated/BarStatus';
export type { Beat } from './generated/Beat';
export type { BeatEffects } from './generated/BeatEffects';
export type { BeatTime } from './generated/BeatTime';
export type { Pulse } from './generated/Pulse';
export type { Bend } from './generated/Bend';
export type { BendKind } from './generated/BendKind';
export type { ChordDiagram } from './generated/ChordDiagram';
export type { Clef } from './generated/Clef';
export type { Color } from './generated/Color';
export type { Cursor } from './generated/Cursor';
export type { CurvePoint } from './generated/CurvePoint';
export type { DrumHead } from './generated/DrumHead';
export type { Duration } from './generated/Duration';
export type { Dynamic } from './generated/Dynamic';
export type { EditorState } from './generated/EditorState';
export type { Feel } from './generated/Feel';
export type { Finger } from './generated/Finger';
export type { Font } from './generated/Font';
export type { Grace } from './generated/Grace';
export type { GraceNote } from './generated/GraceNote';
export type { Hairpin } from './generated/Hairpin';
export type { Harmonic } from './generated/Harmonic';
export type { Instrument } from './generated/Instrument';
export type { Item } from './generated/Item';
export type { Jump } from './generated/Jump';
export type { KitPiece } from './generated/KitPiece';
export type { Layout } from './generated/Layout';
export type { MasterBar } from './generated/MasterBar';
export type { Mix } from './generated/Mix';
export type { Notation } from './generated/Notation';
export type { Note } from './generated/Note';
export type { NoteEffects } from './generated/NoteEffects';
export type { NoteValue } from './generated/NoteValue';
export type { PickStroke } from './generated/PickStroke';
export type { Pedal } from './generated/Pedal';
export type { PedalKind } from './generated/PedalKind';
export type { RowBox } from './generated/RowBox';
export type { Score } from './generated/Score';
export type { ScoreInfo } from './generated/ScoreInfo';
export type { Slap } from './generated/Slap';
export type { Sound } from './generated/Sound';
export type { Slide } from './generated/Slide';
export type { Staff } from './generated/Staff';
export type { Strum } from './generated/Strum';
export type { SystemBox } from './generated/SystemBox';
export type { Target } from './generated/Target';
export type { TempoChange } from './generated/TempoChange';
export type { TimeSignature } from './generated/TimeSignature';
export type { Track } from './generated/Track';
export type { Trill } from './generated/Trill';
export type { Tuning } from './generated/Tuning';
export type { Tuplet } from './generated/Tuplet';
export type { Vibrato } from './generated/Vibrato';
export type { Voice } from './generated/Voice';

/** A piece of the drum kit: on a drum track a note's fret is the piece's number. */
export type DrumPiece = KitPiece;

/** What the player needs of a passage besides its sound; see `Passage.timelineJson`. */
export interface Timeline {
  beats: BeatTime[];
  pulses: Pulse[];
  duration: number;
}

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

export const TARGETS: [Target, string][] = [
  ['segno', 'Segno'],
  ['segno_segno', 'Second segno'],
  ['coda', 'Coda'],
  ['double_coda', 'Second coda'],
  ['fine', 'Fine'],
];

export const JUMPS: [Jump, string][] = [
  ['da_capo', 'D.C. (back to the start)'],
  ['da_capo_al_fine', 'D.C. al Fine'],
  ['da_capo_al_coda', 'D.C. al Coda'],
  ['dal_segno', 'D.S. (back to the segno)'],
  ['dal_segno_al_fine', 'D.S. al Fine'],
  ['dal_segno_al_coda', 'D.S. al Coda'],
  ['to_coda', 'To Coda'],
  ['da_capo_al_double_coda', 'D.C. al Double Coda'],
  ['dal_segno_al_double_coda', 'D.S. al Double Coda'],
  ['dal_segno_segno', 'D.S.S. (back to the second segno)'],
  ['dal_segno_segno_al_fine', 'D.S.S. al Fine'],
  ['dal_segno_segno_al_coda', 'D.S.S. al Coda'],
  ['dal_segno_segno_al_double_coda', 'D.S.S. al Double Coda'],
  ['to_double_coda', 'To Double Coda'],
];

export const FEELS: [Feel, string][] = [
  ['straight', 'Straight'],
  ['swing_eighths', 'Swing eighths'],
  ['swing_sixteenths', 'Swing sixteenths'],
  ['dotted_eighths', 'Dotted eighths'],
  ['dotted_sixteenths', 'Dotted sixteenths'],
  ['scottish_eighths', 'Scottish eighths (short, long)'],
  ['scottish_sixteenths', 'Scottish sixteenths'],
];

export const FADES: [Fade, string][] = [
  ['in', 'Fade in'],
  ['out', 'Fade out'],
  ['swell', 'Volume swell'],
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

export const NOTATIONS: [Notation, string][] = [
  ['both', 'Notation and tab'],
  ['tab', 'Tab only'],
  ['standard', 'Notation only'],
  ['slash', 'Rhythm slashes'],
];

/** The most voices a bar can hold. */
export const MAX_VOICES = 4;

export const NOTE_VALUES: NoteValue[] = [
  'whole',
  'half',
  'quarter',
  'eighth',
  'sixteenth',
  'thirty_second',
  'sixty_fourth',
  'hundred_twenty_eighth',
  'two_hundred_fifty_sixth',
];
