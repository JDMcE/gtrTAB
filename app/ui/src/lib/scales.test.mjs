// Run with `npm test`: Node reads the TypeScript module directly.
import assert from 'node:assert/strict';
import { test } from 'node:test';
import { SCALES, ascending, degrees, findScales, scaleClasses, spell } from './scales.ts';

const scale = (name) => SCALES.find((s) => s.name === name);
const [C, D, E, F, G, A, B] = [60, 62, 64, 65, 67, 69, 71];

test('a scale of seven notes is spelled with each letter once', () => {
  assert.deepEqual(spell(5, scale('Major')), ['F', 'G', 'A', 'Bb', 'C', 'D', 'E']);
  assert.deepEqual(spell(3, scale('Natural minor')), ['Eb', 'F', 'Gb', 'Ab', 'Bb', 'Cb', 'Db']);
  assert.deepEqual(spell(6, scale('Major')), ['F#', 'G#', 'A#', 'B', 'C#', 'D#', 'E#']);
  assert.deepEqual(spell(9, scale('Harmonic minor')), ['A', 'B', 'C', 'D', 'E', 'F', 'G#']);
  assert.deepEqual(spell(9, scale('Minor pentatonic')), ['A', 'C', 'D', 'E', 'G']);
});

test('the steps of a scale are said as they are to its root', () => {
  assert.deepEqual(degrees(scale('Blues')), ['1', '♭3', '4', '♭5', '5', '♭7']);
  assert.deepEqual(degrees(scale('Lydian')), ['1', '2', '3', '♯4', '5', '6', '7']);
  assert.deepEqual(scaleClasses(10, scale('Major pentatonic')), [10, 0, 2, 5, 7]);
  assert.deepEqual(ascending(57, scale('Minor pentatonic')), [57, 60, 62, 64, 67, 69]);
});

test('the scale found is the one that holds the notes with the fewest left over', () => {
  // A minor pentatonic lick, from A and back to it.
  const [best] = findScales([A, C + 12, D + 12, E + 12, G + 12, E + 12, A]);
  assert.equal(best.scale.name, 'Minor pentatonic');
  assert.equal(best.root, 9);
  assert.deepEqual([best.outside, best.unused], [[], 0]);
});

test('of the modes that hold the same notes, the one on the note it comes home to wins', () => {
  const found = findScales([C, D, E, F, G, A, B, C + 12]);
  assert.deepEqual([found[0].scale.name, found[0].root], ['Major', 0]);
  const dorian = findScales([D, E, F, G, A, B, C + 12, D + 12, D]);
  assert.deepEqual([dorian[0].scale.name, dorian[0].root], ['Dorian', 2]);
  // The same notes as another mode are among those offered, with nothing outside them.
  assert.ok(found.some((fit) => fit.scale.name === 'Natural minor' && fit.root === 9 && !fit.outside.length));
});

test('when no scale holds every note, those that hold the most are given', () => {
  const chromatic = findScales([C, C + 1, D, D + 1, E, F, F + 1, G, G + 1, A], 3);
  assert.equal(chromatic.length, 3);
  assert.ok(chromatic.every((fit) => fit.outside.length === chromatic[0].outside.length));
  assert.ok(chromatic[0].outside.length > 0);
  assert.deepEqual(findScales([]), []);
});
