// Run with `npm test`: Node reads the TypeScript module directly.
import assert from 'node:assert/strict';
import { test } from 'node:test';
import { nextSpeed } from './trainer.ts';

/** The speeds of the first `count` times round, from `speed`. */
function speeds(trainer, speed, count) {
  const played = [speed];
  for (let pass = 1; pass < count; pass++) played.push(nextSpeed(trainer, played[pass - 1], pass));
  return played;
}

test('the speed goes up a step each time round, and stops at the target', () => {
  assert.deepEqual(speeds({ step: 10, every: 1, target: 100 }, 70, 6), [70, 80, 90, 100, 100, 100]);
});

test('a step that would pass the target lands on it', () => {
  assert.deepEqual(speeds({ step: 25, every: 1, target: 90 }, 50, 4), [50, 75, 90, 90]);
});

test('the speed can be held for several times round before each step', () => {
  assert.deepEqual(speeds({ step: 5, every: 3, target: 100 }, 60, 8), [60, 60, 60, 65, 65, 65, 70, 70]);
});

test('a speed already at or past the target stays as it is', () => {
  assert.deepEqual(speeds({ step: 5, every: 1, target: 80 }, 80, 3), [80, 80, 80]);
  assert.deepEqual(speeds({ step: 5, every: 1, target: 80 }, 120, 3), [120, 120, 120]);
});
