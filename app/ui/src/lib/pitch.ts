// Where a pitch sits on a staff and back again, for the cursor on a staff without strings.
// Positions are counted in steps of the scale, as `gtr_layout` counts them: C in the octave
// below MIDI's lowest is step 0, so middle C (MIDI 60) is step 28.

/** The step of the scale, from C, of each pitch class when spelled with sharps or with flats. */
const SHARP_STEPS = [0, 0, 1, 1, 2, 3, 3, 4, 4, 5, 5, 6];
const FLAT_STEPS = [0, 1, 1, 2, 2, 3, 4, 4, 5, 5, 6, 6];
/** The pitch class of each letter from C to B. */
const NATURALS = [0, 2, 4, 5, 7, 9, 11];
/** The letters a key signature sharpens, in the order it adds them; flats go the other way. */
const SHARP_ORDER = [3, 0, 4, 1, 5, 2, 6];

/** The step a pitch is written on in a key of `key` sharps, or flats if negative. */
export function stepOfPitch(pitch: number, key: number): number {
  const steps = key >= 0 ? SHARP_STEPS : FLAT_STEPS;
  return (Math.floor(pitch / 12) - 1) * 7 + steps[pitch % 12];
}

/** The pitch a step stands for in a key: the note of that name as the key has it. */
export function pitchOfStep(step: number, key: number): number {
  const letter = ((step % 7) + 7) % 7;
  const octave = Math.floor(step / 7) + 1;
  const sharpened = SHARP_ORDER.slice(0, Math.max(0, key)).includes(letter);
  const flattened = [...SHARP_ORDER].reverse().slice(0, Math.max(0, -key)).includes(letter);
  const pitch = octave * 12 + NATURALS[letter] + (sharpened ? 1 : 0) - (flattened ? 1 : 0);
  return Math.max(0, Math.min(127, pitch));
}
