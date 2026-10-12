// Run with `npm test`: Node reads the TypeScript module directly.
import assert from 'node:assert/strict';
import { test } from 'node:test';
import { CHORD_TYPES, chordName, chordShapes } from './chords.ts';

/** Standard guitar tuning, from the highest string to the lowest. */
const GUITAR = [64, 59, 55, 50, 45, 40];
const [C, D, E, G, A] = [0, 2, 4, 7, 9];
const kind = (suffix) => CHORD_TYPES.find((t) => t.suffix === suffix).intervals;

/** The pitch classes a shape sounds, from the lowest string played to the highest. */
const sounded = (shape, tuning = GUITAR) =>
  shape.flatMap((fret, string) => (fret < 0 ? [] : [(tuning[string] + fret) % 12])).reverse();
const has = (shapes, shape) => shapes.some((s) => s.join() === shape.join());

test('the open shapes everyone learns first are found', () => {
  assert.ok(has(chordShapes(GUITAR, C, kind('')), [0, 1, 0, 2, 3, -1]));
  assert.ok(has(chordShapes(GUITAR, E, kind('m')), [0, 0, 0, 2, 2, 0]));
  assert.ok(has(chordShapes(GUITAR, A, kind('m7')), [0, 1, 0, 2, 0, -1]));
});

test('a position offers more than one shape', () => {
  const shapes = chordShapes(GUITAR, G, kind(''));
  // Open G with the top strings low in the position, and with them taken high.
  assert.ok(has(shapes, [3, 0, 0, 0, 2, 3]));
  assert.ok(has(shapes, [3, 3, 4, 0, 2, 3]));
  // And the same chord from a root on another string.
  const lowest = shapes.map((shape) => shape.findLastIndex((fret) => fret >= 0));
  assert.ok(new Set(lowest).size > 1);
  assert.equal(new Set(shapes.map(String)).size, shapes.length, 'no shape is offered twice');
});

test('every shape holds its chord, with the root in the bass unless told otherwise', () => {
  for (const { intervals } of CHORD_TYPES) {
    for (let root = 0; root < 12; root++) {
      const tones = intervals.map((i) => (root + i) % 12);
      const shapes = chordShapes(GUITAR, root, intervals);
      assert.ok(shapes.length > 0 && shapes.length <= 24);
      for (const shape of shapes) {
        const notes = sounded(shape);
        assert.equal(notes[0], root);
        assert.ok(notes.every((note) => tones.includes(note)));
        const fifth = intervals.length >= 4 ? (root + 7) % 12 : -1;
        assert.ok(tones.every((tone) => tone === fifth || notes.includes(tone)));
        // One position: the stopped strings are within four frets of each other.
        const stopped = shape.filter((fret) => fret > 0);
        assert.ok(Math.max(...stopped, 0) - Math.min(...stopped, 24) < 4);
      }
    }
  }
});

test('an inversion puts another note of the chord in the bass', () => {
  const shapes = chordShapes(GUITAR, C, kind(''), E);
  assert.ok(has(shapes, [0, 1, 0, 2, 3, 0]));
  for (const shape of shapes) {
    const notes = sounded(shape);
    assert.equal(notes[0], E);
    assert.ok([C, E, G].every((tone) => notes.includes(tone)));
  }
  assert.equal(chordName(C, '', E), 'C/E');
  assert.equal(chordName(C, 'maj7'), 'Cmaj7');
});

test('a slash chord has its bass note under the chord and nowhere else', () => {
  const shapes = chordShapes(GUITAR, A, kind('m'), G);
  assert.ok(has(shapes, [0, 1, 2, 2, 0, 3]));
  for (const shape of shapes) {
    const notes = sounded(shape);
    assert.equal(notes[0], G);
    assert.ok(!notes.slice(1).includes(G));
    assert.ok([A, C, E].every((tone) => notes.includes(tone)));
  }
  assert.equal(chordName(D, '', 6), 'D/F#');
});

test('other tunings get shapes of their own', () => {
  const ukulele = [69, 64, 60, 67];
  assert.ok(chordShapes(ukulele, C, kind('')).length > 0);
  const bass = [43, 38, 33, 28];
  for (const shape of chordShapes(bass, E, kind('5'))) assert.equal(sounded(shape, bass)[0], E);
});
