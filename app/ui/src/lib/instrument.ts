// The plain workings of the instrument view: where the frets and keys go, and which line of
// a drum staff a new piece of the kit goes on.

import { ROOTS } from './chords.ts';

/** Frets the fretboard shows past the nut, or the capo, when no note asks for more. */
export const FRETS = 24;

/** Frets with a dot inlaid on the neck, whose numbers stand out under it. */
export const INLAYS = [3, 5, 7, 9, 12, 15, 17, 19, 21, 24];

/**
 * How wide each column of the fretboard is, from the open strings to fret `count`, as a
 * share of the neck: each fret is narrower than the one before it, as on a guitar, where
 * each is shorter by the twelfth root of two. The open strings get a narrow column.
 */
export function fretWidths(count: number): number[] {
  const widths = [0.6];
  for (let fret = 1; fret <= count; fret++) widths.push(2 ** (-(fret - 1) / 12));
  return widths;
}

/** The name of a pitch without its octave, as the chord builder spells roots. */
export const pitchName = (pitch: number) => ROOTS[((pitch % 12) + 12) % 12];

/** True for the pitches played on the black keys of a piano. */
export const isBlack = (pitch: number) => [1, 3, 6, 8, 10].includes(((pitch % 12) + 12) % 12);

/** A key of the piano: its pitch, and where it starts counted in white keys from the lowest. */
export interface PianoKey {
  pitch: number;
  black: boolean;
  /** For a white key its index among the white keys; for a black key where its middle falls. */
  x: number;
}

/** The keys from `low` to `high`, both white, laid out left to right. */
export function pianoKeys(low: number, high: number): PianoKey[] {
  const keys: PianoKey[] = [];
  let white = 0;
  for (let pitch = low; pitch <= high; pitch++) {
    const black = isBlack(pitch);
    keys.push({ pitch, black, x: black ? white : white++ });
  }
  return keys;
}

/** The lowest and highest key a piano has: A0 and C8. */
export const PIANO_RANGE: [number, number] = [21, 108];

/**
 * The line of a drum staff a piece goes on in a beat that has notes on `taken` lines: the
 * first of `lines` that is free, or `null` if the beat is full.
 */
export function freeLine(taken: number[], lines: number): number | null {
  for (let line = 0; line < lines; line++) if (!taken.includes(line)) return line;
  return null;
}
