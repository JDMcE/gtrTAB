// Chord shapes for the chord builder: given a tuning, a root and a kind of chord, find
// the ways to finger it.

export const ROOTS = ['C', 'C#', 'D', 'Eb', 'E', 'F', 'F#', 'G', 'Ab', 'A', 'Bb', 'B'];

/** Kinds of chord: what is written after the root, a name, and the semitones above the root. */
export const CHORD_TYPES: { suffix: string; name: string; intervals: number[] }[] = [
  { suffix: '', name: 'Major', intervals: [0, 4, 7] },
  { suffix: 'm', name: 'Minor', intervals: [0, 3, 7] },
  { suffix: '5', name: 'Power chord (5)', intervals: [0, 7] },
  { suffix: '7', name: 'Dominant 7th', intervals: [0, 4, 7, 10] },
  { suffix: 'm7', name: 'Minor 7th', intervals: [0, 3, 7, 10] },
  { suffix: 'maj7', name: 'Major 7th', intervals: [0, 4, 7, 11] },
  { suffix: 'sus2', name: 'Suspended 2nd', intervals: [0, 2, 7] },
  { suffix: 'sus4', name: 'Suspended 4th', intervals: [0, 5, 7] },
  { suffix: '6', name: 'Major 6th', intervals: [0, 4, 7, 9] },
  { suffix: 'm6', name: 'Minor 6th', intervals: [0, 3, 7, 9] },
  { suffix: 'add9', name: 'Added 9th', intervals: [0, 4, 7, 2] },
  { suffix: '9', name: 'Dominant 9th', intervals: [0, 4, 7, 10, 2] },
  { suffix: 'dim', name: 'Diminished', intervals: [0, 3, 6] },
  { suffix: 'aug', name: 'Augmented', intervals: [0, 4, 8] },
];

/** Frets a hand spans in one position. */
const SPAN = 4;
/** Highest position looked at. */
const LAST_POSITION = 12;
/** Most shapes offered. */
const LIMIT = 24;

/** What a chord is called: its root and kind, and its bass note after a slash if that is not the root. */
export function chordName(root: number, suffix: string, bass: number = root): string {
  return ROOTS[root] + suffix + (bass === root ? '' : `/${ROOTS[bass]}`);
}

/**
 * Ways to finger a chord, lowest position first. `tuning` gives the open-string pitches
 * from the highest string to the lowest, as the score has them, and each shape lists a fret
 * per string in the same order: 0 for an open string and -1 for one that is not played.
 *
 * `bass` is the note the lowest string played sounds. Left out, it is the root. A note of
 * the chord other than the root gives an inversion, and any other note a slash chord, with
 * that note under the chord and nowhere else in it.
 *
 * A shape has the bass note on its lowest string, sounds the strings above it as far as
 * they can take a note of the chord, holds every note of the chord (the fifth may be left
 * out of a chord of four or more notes), and fits in one position, open strings aside. A
 * position offers a shape from each string the bass note can be on, and for each of those
 * one that keeps the other strings low in the position and one that takes them high.
 */
export function chordShapes(
  tuning: number[],
  root: number,
  intervals: number[],
  bass: number = root,
): number[][] {
  const tones = intervals.map((i) => (root + i) % 12);
  const needed = tones.filter((_, i) => !(intervals.length >= 4 && intervals[i] === 7));
  // Work from the lowest string up, the way a chord is built.
  const open = [...tuning].reverse();
  const shapes: number[][] = [];
  const seen = new Set<string>();

  for (let position = 0; position <= LAST_POSITION; position++) {
    const first = Math.max(1, position);
    /** Frets on a string in this position, with the open string where the hand is near the nut. */
    const reach = (string: number) => {
      const frets = position <= 3 ? [0] : [];
      for (let fret = first; fret < first + SPAN; fret++) frets.push(fret);
      return frets;
    };
    const toneAt = (string: number, fret: number) => (open[string] + fret) % 12;
    /** Frets on a string that give a note of the chord. */
    const choices = (string: number) => reach(string).filter((f) => tones.includes(toneAt(string, f)));

    for (let low = 0; low + 3 <= open.length || (low === 0 && open.length < 3); low++) {
      const bassFret = reach(low).find((fret) => toneAt(low, fret) === bass);
      if (bassFret === undefined) continue;
      for (const high of [false, true]) {
        const frets: number[] = open.map(() => -1);
        frets[low] = bassFret;
        // Strings above the bass each take a note of the chord; where one cannot, it and
        // the strings above it are left out.
        for (let string = low + 1; string < open.length; string++) {
          const options = choices(string);
          if (!options.length) break;
          frets[string] = options[high ? options.length - 1 : 0];
        }
        if (frets.filter((f) => f >= 0).length < Math.max(2, needed.length)) continue;

        // Where a note of the chord is missing, give it a string that doubles another note.
        for (const tone of needed) {
          const sounding = frets.map((f, s) => (f < 0 ? -1 : toneAt(s, f)));
          if (sounding.includes(tone)) continue;
          for (let string = open.length - 1; string > low; string--) {
            if (frets[string] < 0) continue;
            const doubled = sounding.filter((t) => t === sounding[string]).length > 1;
            const fret = choices(string).find((f) => toneAt(string, f) === tone);
            if (doubled && fret !== undefined) {
              frets[string] = fret;
              break;
            }
          }
        }
        const sounding = frets.flatMap((f, s) => (f < 0 ? [] : [toneAt(s, f)]));
        if (!needed.every((tone) => sounding.includes(tone))) continue;

        const shape = [...frets].reverse();
        const key = shape.join();
        if (!seen.has(key)) {
          seen.add(key);
          shapes.push(shape);
        }
      }
    }
    if (shapes.length >= LIMIT) break;
  }
  return shapes.slice(0, LIMIT);
}
