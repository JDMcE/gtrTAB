// Run with `npm test`: Node reads the TypeScript module directly.
import assert from 'node:assert/strict';
import { test } from 'node:test';
import { detectPitch, frequencyOf, median, nearestNote, nearestString, pitchOf } from './tuner.ts';

const RATE = 48_000;

/**
 * A plucked string, roughly: a note with overtones, the second louder than the note
 * itself as on a low string, dying away, with a little noise.
 */
function pluck(frequency, { size = 4096, overtones = [1, 1.6, 0.8, 0.5, 0.3], noise = 0.01 } = {}) {
  const samples = new Float32Array(size);
  let seed = 1;
  const random = () => ((seed = (seed * 16807) % 2147483647) / 2147483647 - 0.5) * 2;
  for (let i = 0; i < size; i++) {
    const t = i / RATE;
    let value = 0;
    overtones.forEach((level, n) => (value += level * Math.sin(2 * Math.PI * frequency * (n + 1) * t + n)));
    samples[i] = 0.2 * value * Math.exp(-t * 3) + noise * random();
  }
  return samples;
}

/** Cents between two frequencies. */
const cents = (a, b) => 1200 * Math.log2(a / b);

test('the note of every open string of a guitar, a bass and a mandolin is heard to within a cent', () => {
  // B0 of a five-string bass, E2 to E4 of a guitar, E5 of a mandolin.
  for (const pitch of [23, 28, 40, 45, 50, 55, 59, 64, 76]) {
    const frequency = frequencyOf(pitch);
    const heard = detectPitch(pluck(frequency), RATE);
    assert.ok(heard, `nothing heard at ${frequency}`);
    assert.ok(Math.abs(cents(heard.frequency, frequency)) < 1, `${heard.frequency} for ${frequency}`);
    assert.ok(heard.clarity > 0.8);
  }
});

test('a string a little out of tune is heard as out by as much', () => {
  const sharp = frequencyOf(45) * 2 ** (12 / 1200);
  const heard = detectPitch(pluck(sharp), RATE);
  assert.equal(nearestNote(heard.frequency).pitch, 45);
  assert.ok(Math.abs(nearestNote(heard.frequency).cents - 12) < 1);
});

test('silence and noise are not notes', () => {
  assert.equal(detectPitch(new Float32Array(4096), RATE), null);
  assert.equal(detectPitch(pluck(110, { overtones: [], noise: 0.3 }), RATE), null);
});

test('a note is named, and a string found, against the reference pitch', () => {
  assert.equal(pitchOf(440), 69);
  assert.ok(Math.abs(frequencyOf(40) - 82.407) < 0.001);
  assert.deepEqual(nearestNote(432, 432), { pitch: 69, cents: 0 });
  const flatA = nearestNote(frequencyOf(69) * 2 ** (-30 / 1200));
  assert.equal(flatA.pitch, 69);
  assert.ok(Math.abs(flatA.cents + 30) < 1e-6);

  // A low E a semitone flat is still the low E string, ninety-nine cents flat of it.
  const guitar = [64, 59, 55, 50, 45, 40];
  const found = nearestString(frequencyOf(39.01), guitar);
  assert.equal(found.string, 5);
  assert.ok(Math.abs(found.cents + 99) < 1e-6);
  assert.equal(nearestString(440, []), null);
});

test('the middle reading is kept', () => {
  assert.equal(median([3, 100, 2]), 3);
  assert.equal(median([4, 1, 2, 3]), 2.5);
});
