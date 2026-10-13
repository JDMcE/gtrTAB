// Run with `npm test`: Node reads the TypeScript module directly.
import assert from 'node:assert/strict';
import { test } from 'node:test';
import { addPedal, knobValue, movePedal, sameChain, withKnob, withPreset } from './pedals.ts';

// A few kinds in the order the core lists them.
const specs = [
  { kind: 'compressor', name: 'Compressor', knobs: [{ name: 'Threshold', default: 50 }] },
  { kind: 'overdrive', name: 'Overdrive', knobs: [{ name: 'Drive', default: 40 }, { name: 'Tone', default: 50 }] },
  { kind: 'amp', name: 'Amp', knobs: [{ name: 'Gain', default: 50 }] },
  { kind: 'delay', name: 'Delay', knobs: [{ name: 'Time', default: 35 }] },
  { kind: 'reverb', name: 'Reverb', knobs: [{ name: 'Size', default: 50 }] },
];
const spec = (kind) => specs.find((s) => s.kind === kind);
const kinds = (chain) => chain.map((p) => p.kind);

test('a pedal is added where its kind usually goes', () => {
  let chain = [];
  for (const kind of ['amp', 'reverb', 'compressor', 'delay', 'overdrive']) {
    [chain] = addPedal(chain, spec(kind), specs);
  }
  assert.deepEqual(kinds(chain), ['compressor', 'overdrive', 'amp', 'delay', 'reverb']);
  // A second of a kind goes after the first; it says where it went.
  const [more, at] = addPedal(chain, spec('overdrive'), specs);
  assert.equal(at, 2);
  assert.deepEqual(kinds(more), ['compressor', 'overdrive', 'overdrive', 'amp', 'delay', 'reverb']);
  // A chain put in another order is not reordered: the new pedal goes after what stands
  // before it in the usual order, counting from the end.
  const [odd] = addPedal([{ kind: 'reverb', knobs: [] }, { kind: 'amp', knobs: [] }], spec('delay'), specs);
  assert.deepEqual(kinds(odd), ['reverb', 'amp', 'delay']);
});

test('pedals move, and a move off either end does nothing', () => {
  const chain = ['amp', 'delay', 'reverb'].map((kind) => ({ kind, knobs: [] }));
  assert.deepEqual(kinds(movePedal(chain, 0, 1)), ['delay', 'amp', 'reverb']);
  assert.deepEqual(kinds(movePedal(chain, 2, 0)), ['reverb', 'amp', 'delay']);
  assert.equal(movePedal(chain, 0, -1), chain);
  assert.equal(movePedal(chain, 2, 3), chain);
});

test('a knob is read with its default, and set within its range', () => {
  const bare = { kind: 'overdrive', knobs: [] };
  assert.equal(knobValue(bare, spec('overdrive'), 1), 50);
  assert.deepEqual(withKnob(bare, spec('overdrive'), 1, 72.4).knobs, [40, 72]);
  assert.deepEqual(withKnob(bare, spec('overdrive'), 0, 140).knobs, [100, 50]);
  // The same settings written out or left to their defaults are the same chain.
  assert.ok(sameChain([bare], [{ kind: 'overdrive', on: true, knobs: [40, 50] }], specs));
  assert.ok(!sameChain([bare], [{ kind: 'overdrive', on: false, knobs: [] }], specs));
});

test('a preset saved under a name takes the place of the one it had', () => {
  const one = { name: 'Lead', master: false, pedals: [] };
  const two = { name: 'Lead', master: false, pedals: [{ kind: 'amp', knobs: [] }] };
  const hall = { name: 'Lead', master: true, pedals: [] };
  const saved = withPreset(withPreset(withPreset([], one), hall), two);
  assert.deepEqual(saved, [hall, two]);
});
