// Scales for the scale library and the scale finder: what each is, how its notes are
// spelled, and which of them fit a set of notes.

import { ROOTS } from './chords.ts';

export interface Scale {
  name: string;
  /** Semitones above the root, from the root up. */
  intervals: number[];
}

/** The scales offered, the most used first; the finder prefers them in this order too. */
export const SCALES: Scale[] = [
  { name: 'Major', intervals: [0, 2, 4, 5, 7, 9, 11] },
  { name: 'Natural minor', intervals: [0, 2, 3, 5, 7, 8, 10] },
  { name: 'Major pentatonic', intervals: [0, 2, 4, 7, 9] },
  { name: 'Minor pentatonic', intervals: [0, 3, 5, 7, 10] },
  { name: 'Blues', intervals: [0, 3, 5, 6, 7, 10] },
  { name: 'Major blues', intervals: [0, 2, 3, 4, 7, 9] },
  { name: 'Dorian', intervals: [0, 2, 3, 5, 7, 9, 10] },
  { name: 'Mixolydian', intervals: [0, 2, 4, 5, 7, 9, 10] },
  { name: 'Phrygian', intervals: [0, 1, 3, 5, 7, 8, 10] },
  { name: 'Lydian', intervals: [0, 2, 4, 6, 7, 9, 11] },
  { name: 'Locrian', intervals: [0, 1, 3, 5, 6, 8, 10] },
  { name: 'Harmonic minor', intervals: [0, 2, 3, 5, 7, 8, 11] },
  { name: 'Melodic minor', intervals: [0, 2, 3, 5, 7, 9, 11] },
  { name: 'Phrygian dominant', intervals: [0, 1, 4, 5, 7, 8, 10] },
  { name: 'Whole tone', intervals: [0, 2, 4, 6, 8, 10] },
  { name: 'Diminished (whole-half)', intervals: [0, 2, 3, 5, 6, 8, 9, 11] },
  { name: 'Diminished (half-whole)', intervals: [0, 1, 3, 4, 6, 7, 9, 10] },
];

const pitchClass = (pitch: number) => ((pitch % 12) + 12) % 12;

/** The pitch classes of a scale on `root`. */
export const scaleClasses = (root: number, scale: Scale) => scale.intervals.map((i) => pitchClass(root + i));

/** What each step of a scale is to its root, as a scale is usually written: 1, ♭3, 5 and so on. */
const DEGREES = ['1', '♭2', '2', '♭3', '3', '4', '♭5', '5', '♭6', '6', '♭7', '7'];
export function degrees(scale: Scale): string[] {
  // Six semitones up is a raised fourth in a scale that has a fifth and no fourth, as the
  // Lydian has, and otherwise a flattened fifth.
  return scale.intervals.map((i) => (i === 6 && !scale.intervals.includes(5) && scale.intervals.includes(7) ? '♯4' : DEGREES[i]));
}

const LETTERS = 'CDEFGAB';
const NATURALS = [0, 2, 4, 5, 7, 9, 11];

/**
 * The names of a scale's notes. A scale of seven notes uses each letter once, as it is
 * written, so that F major has a B♭ and not an A♯; any other is spelled as the chord
 * builder spells roots.
 */
export function spell(root: number, scale: Scale): string[] {
  if (scale.intervals.length !== 7) return scaleClasses(root, scale).map((c) => ROOTS[c]);
  const rootName = ROOTS[pitchClass(root)];
  const first = LETTERS.indexOf(rootName[0]);
  return scale.intervals.map((interval, step) => {
    const letter = (first + step) % 7;
    const off = (((pitchClass(root + interval) - NATURALS[letter]) % 12) + 18) % 12 - 6;
    const accidental = off > 0 ? '#'.repeat(off) : 'b'.repeat(-off);
    return LETTERS[letter] + accidental;
  });
}

/** A scale that fits some notes, and how well. */
export interface Fit {
  root: number;
  scale: Scale;
  /** Notes played that the scale does not have; a scale that fits has none. */
  outside: number[];
  /** Notes of the scale that were not played. */
  unused: number;
}

/**
 * The scales that fit `pitches` best, the best first. Those that hold every note come
 * first, and of them those with the fewest notes left over; then those that were most
 * likely the key: on the note played most, or the first or the last; then the scales that
 * are used most. When no scale holds every note, those holding the most are given.
 */
export function findScales(pitches: number[], limit = 12): Fit[] {
  if (!pitches.length) return [];
  const counts = new Map<number, number>();
  for (const pitch of pitches) counts.set(pitchClass(pitch), (counts.get(pitchClass(pitch)) ?? 0) + 1);
  const played = [...counts.keys()];
  const most = Math.max(...counts.values());
  /** How likely a note is to be the key's: played most, last, first. */
  const tonic = (root: number) =>
    (counts.get(root) === most ? 1 : 0) +
    (pitchClass(pitches[pitches.length - 1]) === root ? 2 : 0) +
    (pitchClass(pitches[0]) === root ? 1 : 0) +
    (pitchClass(Math.min(...pitches)) === root ? 1 : 0);

  const fits: (Fit & { order: number; likely: number })[] = [];
  SCALES.forEach((scale, order) => {
    for (let root = 0; root < 12; root++) {
      const classes = scaleClasses(root, scale);
      const outside = played.filter((c) => !classes.includes(c));
      const unused = classes.filter((c) => !counts.has(c)).length;
      fits.push({ root, scale, outside, unused, order, likely: tonic(root) });
    }
  });
  fits.sort(
    (a, b) =>
      a.outside.length - b.outside.length ||
      a.unused - b.unused ||
      b.likely - a.likely ||
      a.order - b.order ||
      a.root - b.root,
  );
  return fits.slice(0, limit).map(({ root, scale, outside, unused }) => ({ root, scale, outside, unused }));
}

/** The pitches of a scale going up an octave from `root`, for hearing it. */
export const ascending = (root: number, scale: Scale) => [...scale.intervals.map((i) => root + i), root + 12];
