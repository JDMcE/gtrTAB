// The tuner's workings: the pitch of a stretch of sound, and how far it is from a note.
// The pitch is found with YIN (de Cheveigné and Kawahara, 2002), which compares the sound
// with itself a little later and looks for the shortest delay at which it repeats. Unlike
// the loudest peak of a spectrum, that is not fooled by a string whose overtones are louder
// than the note itself, as a low string's often are.

/** Lowest and highest frequencies listened for: below a five-string bass's B, above a mandolin's E. */
export const LOWEST_HZ = 28;
export const HIGHEST_HZ = 1400;
/** How alike the sound must be to itself one period on to be heard as a note, from 0 (exactly) up. */
const THRESHOLD = 0.15;
/** Quieter than this, as the root of the mean square, is taken to be silence. */
const SILENCE = 0.005;

export interface Heard {
  /** In hertz. */
  frequency: number;
  /** How clearly it is a note, from 0 to 1. */
  clarity: number;
}

/** The pitch of `samples`, or `null` if they are too quiet or not a note. */
export function detectPitch(samples: Float32Array, sampleRate: number): Heard | null {
  const size = samples.length;
  let power = 0;
  for (let i = 0; i < size; i++) power += samples[i] * samples[i];
  if (Math.sqrt(power / size) < SILENCE) return null;

  const minLag = Math.max(2, Math.floor(sampleRate / HIGHEST_HZ));
  const maxLag = Math.min(Math.floor(size / 2), Math.ceil(sampleRate / LOWEST_HZ));
  const window = size - maxLag;
  // The difference of the sound and itself at each delay, normalised by the mean of the
  // differences at shorter delays, so that it dips under one where the sound repeats.
  const normal = new Float32Array(maxLag + 1);
  normal[0] = 1;
  let sum = 0;
  for (let lag = 1; lag <= maxLag; lag++) {
    let difference = 0;
    for (let i = 0; i < window; i++) {
      const d = samples[i] - samples[i + lag];
      difference += d * d;
    }
    sum += difference;
    normal[lag] = sum > 0 ? (difference * lag) / sum : 1;
  }

  // The first dip under the threshold, followed to its bottom.
  let lag = -1;
  for (let at = minLag; at <= maxLag; at++) {
    if (normal[at] < THRESHOLD) {
      while (at + 1 <= maxLag && normal[at + 1] < normal[at]) at++;
      lag = at;
      break;
    }
  }
  if (lag < 0) return null;

  // Between samples: the bottom of the parabola through the dip and its neighbours.
  let exact = lag;
  if (lag > 1 && lag < maxLag) {
    const [a, b, c] = [normal[lag - 1], normal[lag], normal[lag + 1]];
    const bend = a + c - 2 * b;
    if (bend > 0) exact = lag + (a - c) / (2 * bend);
  }
  return { frequency: sampleRate / exact, clarity: Math.max(0, Math.min(1, 1 - normal[lag])) };
}

/** The MIDI pitch, with a fraction, of a frequency, with A4 at `reference` hertz. */
export const pitchOf = (frequency: number, reference = 440) => 69 + 12 * Math.log2(frequency / reference);

/** The frequency of a MIDI pitch, with A4 at `reference` hertz. */
export const frequencyOf = (pitch: number, reference = 440) => reference * 2 ** ((pitch - 69) / 12);

/** The note nearest a frequency, and how many cents sharp (above 0) or flat it is of it. */
export function nearestNote(frequency: number, reference = 440): { pitch: number; cents: number } {
  const exact = pitchOf(frequency, reference);
  const pitch = Math.round(exact);
  return { pitch, cents: (exact - pitch) * 100 };
}

/**
 * The string of a tuning, given as the MIDI pitches of its open strings, that a frequency
 * is nearest, and how many cents sharp or flat of it the frequency is.
 */
export function nearestString(frequency: number, strings: number[], reference = 440): { string: number; cents: number } | null {
  if (!strings.length) return null;
  const exact = pitchOf(frequency, reference);
  let string = 0;
  for (let s = 1; s < strings.length; s++) {
    if (Math.abs(strings[s] - exact) < Math.abs(strings[string] - exact)) string = s;
  }
  return { string, cents: (exact - strings[string]) * 100 };
}

/** The middle of the last few readings, so that one stray reading does not move the needle. */
export function median(values: number[]): number {
  const sorted = [...values].sort((a, b) => a - b);
  const middle = sorted.length >> 1;
  return sorted.length % 2 ? sorted[middle] : (sorted[middle - 1] + sorted[middle]) / 2;
}
