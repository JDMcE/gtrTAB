// Run with `npm test`: Node reads the TypeScript module directly.
import assert from 'node:assert/strict';
import { test } from 'node:test';
import { leftOut, outputName } from './batch.ts';

test('a converted file takes the name it came from with the new ending', () => {
  const taken = new Set();
  assert.equal(outputName('Riff.gp5', 'pdf', taken), 'Riff.pdf');
  assert.equal(outputName('solo.final.musicxml', 'gp', taken), 'solo.final.gp');
  assert.equal(outputName('no ending', 'txt', taken), 'no ending.txt');
  assert.equal(outputName('.hidden', 'mid', taken), '.hidden.mid');
});

test('two files of the same name are numbered, whatever their case', () => {
  const taken = new Set(['riff.pdf']);
  assert.equal(outputName('riff.gp', 'pdf', taken), 'riff (2).pdf');
  assert.equal(outputName('RIFF.mid', 'pdf', taken), 'RIFF (3).pdf');
  assert.equal(outputName('riff.txt', 'gp', taken), 'riff.gp');
  assert.ok(taken.has('riff (3).pdf'));
});

test('what was left out says which way it was lost', () => {
  assert.equal(leftOut('', ''), '');
  assert.equal(leftOut('lyrics', ''), 'not imported: lyrics');
  assert.equal(leftOut('lyrics', 'bends, slides'), 'not imported: lyrics; not exported: bends, slides');
});
