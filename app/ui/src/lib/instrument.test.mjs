// Run with `npm test`: Node reads the TypeScript module directly.
import assert from 'node:assert/strict';
import { test } from 'node:test';
import { fretWidths, freeLine, isBlack, pianoKeys, pitchName, PIANO_RANGE } from './instrument.ts';

test('each fret is narrower than the last, and the twelfth half as wide as the first', () => {
  const widths = fretWidths(24);
  assert.equal(widths.length, 25);
  for (let fret = 2; fret <= 24; fret++) assert.ok(widths[fret] < widths[fret - 1]);
  assert.ok(Math.abs(widths[13] - widths[1] / 2) < 1e-9);
});

test('a piano has 52 white keys and 36 black ones, each black key between two white', () => {
  const keys = pianoKeys(...PIANO_RANGE);
  assert.equal(keys.length, 88);
  assert.equal(keys.filter((k) => !k.black).length, 52);
  // A0, B♭0, B0, C1: the black key falls on the line between the first two white keys.
  assert.deepEqual(keys.slice(0, 4).map((k) => [k.black, k.x]), [[false, 0], [true, 1], [false, 1], [false, 2]]);
  assert.equal(keys.at(-1).pitch, 108);
});

test('pitches are named as the chord builder names roots', () => {
  assert.deepEqual([60, 61, 63, 64, 70].map(pitchName), ['C', 'C#', 'Eb', 'E', 'Bb']);
  assert.deepEqual([60, 61, 66, 71].map(isBlack), [false, true, true, false]);
});

test('a new piece of the kit goes on the first free line', () => {
  assert.equal(freeLine([], 5), 0);
  assert.equal(freeLine([0, 1, 3], 5), 2);
  assert.equal(freeLine([0, 1, 2, 3, 4], 5), null);
});
