import assert from 'node:assert/strict';
import { test } from 'node:test';
import { describeCursor, spokenPitch } from './describe.ts';

const duration = (value, extra = {}) => ({ value, dots: 0, tuplet: null, nested: null, ...extra });
const note = (string, fret, effects = {}, tied = false) => ({ string, fret, tied, effects });
const staffOf = (strings, beats) => ({ tuning: { strings }, capo: 0, bars: [{ voices: [{ beats }] }] });
const GUITAR = [64, 59, 55, 50, 45, 40];

function at(staff, beat, string, instrument = 'steel_guitar') {
  const score = { tracks: [{ name: 'Guitar', instrument, staves: [staff] }] };
  const found = staff.bars[0].voices[0].beats[beat]?.notes.find((n) => n.string === string) ?? null;
  const editor = { cursor: { track: 0, staff: 0, bar: 0, beat, string, voice: 0 }, note: found };
  return [score, editor];
}

test('a note is read with its place, length, fret, pitch and techniques', () => {
  const staff = staffOf(GUITAR, [
    { duration: duration('eighth'), notes: [note(1, 5, { hammer: true, palm_mute: true })] },
    { duration: duration('quarter', { dots: 1 }), notes: [note(0, 0), note(1, 0), note(2, 1, {}, true)] },
    { duration: duration('eighth', { tuplet: { actual: 3, normal: 2 } }), notes: [] },
  ]);
  assert.equal(
    describeCursor(...at(staff, 0, 1)),
    'Bar 1, beat 1 of 3, eighth note, string 2, fret 5, E 4, hammer-on or pull-off, palm mute.',
  );
  assert.equal(
    describeCursor(...at(staff, 1, 2)),
    'Bar 1, beat 2 of 3, dotted quarter chord of 3 notes, string 3, fret 1, A flat 3, tied.',
  );
  assert.equal(describeCursor(...at(staff, 1, 5)), 'Bar 1, beat 2 of 3, dotted quarter chord of 3 notes, string 6, no note.');
  assert.equal(
    describeCursor(...at(staff, 2, 0)),
    'Bar 1, beat 3 of 3, eighth rest, 3 in the time of 2, string 1, no note.',
  );
  // The empty place after the last beat, and a bar with nothing in it.
  assert.equal(describeCursor(...at(staff, 3, 0)), 'Bar 1, after the last of 3 beats, empty, string 1, no note.');
  assert.equal(describeCursor(...at(staffOf(GUITAR, []), 0, 3)), 'Bar 1, empty, string 4, no note.');
});

test('a piano note is its pitch and a drum note its piece of the kit', () => {
  const piano = staffOf([], [{ duration: duration('half'), notes: [note(60, 0)] }]);
  assert.equal(describeCursor(...at(piano, 0, 60, 'piano')), 'Bar 1, beat 1 of 1, half note, note C 4.');
  assert.equal(describeCursor(...at(piano, 0, 62, 'piano')), 'Bar 1, beat 1 of 1, half note, at D 4, no note.');
  const drums = staffOf([0, 0, 0, 0, 0], [{ duration: duration('quarter'), notes: [note(4, 36)] }]);
  const names = new Map([[36, 'Bass drum']]);
  assert.equal(
    describeCursor(...at(drums, 0, 4, 'drums'), names),
    'Bar 1, beat 1 of 1, quarter note, line 5, Bass drum.',
  );
  assert.equal(spokenPitch(40), 'E 2');
});
