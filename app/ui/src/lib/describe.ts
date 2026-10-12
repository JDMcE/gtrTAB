// The score in words: what is under the cursor, for a screen reader, which cannot see what
// is drawn on the canvas.

import type { EditorState, Note, Score } from './model';

const NOTE_NAMES = ['C', 'C sharp', 'D', 'E flat', 'E', 'F', 'F sharp', 'G', 'A flat', 'A', 'B flat', 'B'];
const VALUES: Record<string, string> = {
  whole: 'whole',
  half: 'half',
  quarter: 'quarter',
  eighth: 'eighth',
  sixteenth: 'sixteenth',
  thirty_second: 'thirty-second',
  sixty_fourth: 'sixty-fourth',
  hundred_twenty_eighth: 'hundred and twenty-eighth',
  two_hundred_fifty_sixth: 'two hundred and fifty-sixth',
};
const SLIDES: Record<string, string> = {
  shift: 'shift slide',
  legato: 'legato slide',
  in_from_below: 'slide in from below',
  in_from_above: 'slide in from above',
  out_down: 'slide out downwards',
  out_up: 'slide out upwards',
};

/** A MIDI pitch as a spoken note name with its octave: 40 is "E 2". */
export function spokenPitch(pitch: number): string {
  return `${NOTE_NAMES[pitch % 12]} ${Math.floor(pitch / 12) - 1}`;
}

/** The techniques on a note, in words. */
function techniques(note: Note): string[] {
  const e = note.effects ?? {};
  const found: string[] = [];
  if (note.tied) found.push('tied');
  if (e.hammer) found.push('hammer-on or pull-off');
  if (e.slide) found.push(SLIDES[e.slide]);
  if (e.bend) found.push('bend');
  if (e.vibrato) found.push(e.vibrato === 'wide' ? 'wide vibrato' : 'vibrato');
  if (e.harmonic) found.push(`${e.harmonic} harmonic`);
  if (e.palm_mute) found.push('palm mute');
  if (e.let_ring) found.push('let ring');
  if (e.dead) found.push('dead note');
  if (e.ghost) found.push('ghost note');
  if (e.staccato) found.push('staccato');
  if (e.accent) found.push(e.accent === 'heavy' ? 'heavy accent' : 'accent');
  if (e.tapped) found.push('tapped');
  if (e.trill) found.push('trill');
  if (e.grace) found.push(e.grace.notes.length > 1 ? 'grace notes' : 'grace note');
  return found;
}

/**
 * Says where the cursor is and what is there: the bar and beat, how long the beat is and
 * how many notes it has, and the note on the cursor's string with its fret, pitch and
 * techniques. `drumNames` names the pieces of the kit by their numbers, for a drum track.
 */
export function describeCursor(
  score: Score,
  editor: EditorState,
  drumNames: Map<number, string> = new Map(),
): string {
  const { cursor } = editor;
  const track = score.tracks[cursor.track];
  const staff = track?.staves[cursor.staff];
  if (!track || !staff) return '';
  const beats = staff.bars[cursor.bar]?.voices[cursor.voice]?.beats ?? [];
  const beat = beats[cursor.beat];
  const pitched = staff.tuning.strings.length === 0;
  const drums = track.instrument === 'drums';

  const parts = [`Bar ${cursor.bar + 1}`];
  if (!beat) {
    parts.push(beats.length ? `after the last of ${beats.length} beats, empty` : 'empty');
  } else {
    parts.push(`beat ${cursor.beat + 1} of ${beats.length}`);
    const d = beat.duration;
    const length = [['', 'dotted', 'double dotted', 'triple dotted'][d.dots] ?? '', VALUES[d.value]]
      .filter(Boolean)
      .join(' ');
    const tuplet = d.tuplet ? `, ${d.tuplet.actual} in the time of ${d.tuplet.normal}` : '';
    const count = beat.notes.length;
    parts.push(
      count === 0 ? `${length} rest${tuplet}`
      : count === 1 ? `${length} note${tuplet}`
      : `${length} chord of ${count} notes${tuplet}`,
    );
    const fade = editor.beat_effects?.fade;
    if (fade) parts.push(fade === 'in' ? 'fading in' : fade === 'out' ? 'fading out' : 'swelling');
  }

  const note = editor.note;
  if (pitched) {
    const there = note ? [`note ${spokenPitch(note.string)}`, ...techniques(note)] : [`at ${spokenPitch(cursor.string)}, no note`];
    parts.push(there.join(', '));
  } else {
    const where = `${drums ? 'line' : 'string'} ${cursor.string + 1}`;
    if (!note) {
      parts.push(`${where}, no note`);
    } else if (drums) {
      parts.push([`${where}, ${drumNames.get(note.fret) ?? `piece ${note.fret}`}`, ...techniques(note)].join(', '));
    } else {
      const pitch = staff.tuning.strings[note.string] + staff.capo + note.fret;
      parts.push([`${where}, fret ${note.fret}, ${spokenPitch(pitch)}`, ...techniques(note)].join(', '));
    }
  }
  return `${parts.join(', ')}.`;
}
