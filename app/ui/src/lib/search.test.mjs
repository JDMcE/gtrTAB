// Run with `npm test`: Node reads the TypeScript module directly.
import assert from 'node:assert/strict';
import { test } from 'node:test';
import { listCommands, search, used } from './search.ts';

const command = (label, extra = {}) => ({ label, run: () => {}, ...extra });
const save = command('Save');
const saveAs = command('Save as…');
const pdf = command('PDF…');
const pinch = command('Pinch harmonic');
const natural = command('Natural harmonic');
const hammer = command('Hammer-on or pull-off to the next note');
const paste = command('Paste', { disabled: () => true });
const dark = command('Dark');
const play = command('Play from the cursor or the selection');

const menus = [
  { label: 'File', items: [save, saveAs, '-', { label: 'Export', items: [pdf] }] },
  { label: 'Edit', items: [paste] },
  { label: 'Note', items: [hammer, { label: 'Harmonic', items: [natural, pinch] }] },
  { label: 'View', items: [{ label: 'Theme', items: [dark] }] },
  { label: 'Score', items: [play] },
  { label: 'Playback', items: [play] },
];
const items = listCommands(menus);
const labels = (found) => found.map((f) => f.command.label);

test('every command in the menus is listed once, with the menus it is under', () => {
  assert.equal(items.length, 9);
  assert.deepEqual(items.find((f) => f.command === pdf).path, ['File', 'Export']);
  assert.equal(items.find((f) => f.command === dark).id, 'View › Theme › Dark');
  assert.deepEqual(items.find((f) => f.command === play).path, ['Score']);
});

test('a few letters of each word find a command', () => {
  assert.deepEqual(labels(search(items, 'pin harm')), ['Pinch harmonic']);
  assert.deepEqual(labels(search(items, 'pull')), [hammer.label]);
  assert.deepEqual(labels(search(items, 'nothing like it')), []);
});

test('a name that begins as the query comes first', () => {
  assert.deepEqual(labels(search(items, 'sa')), ['Save', 'Save as…']);
  assert.deepEqual(labels(search(items, 'harmonic')), ['Natural harmonic', 'Pinch harmonic']);
});

test('the menus a command is under find it too, after the names that match', () => {
  assert.deepEqual(labels(search(items, 'export')), ['PDF…']);
  assert.deepEqual(labels(search(items, 'theme dark')), ['Dark']);
});

test('case, accents and the dots of a dialog do not matter', () => {
  assert.deepEqual(labels(search(items, 'PDF')), ['PDF…']);
  assert.deepEqual(labels(search(items, 'pdf…')), ['PDF…']);
  assert.deepEqual(labels(search(items, 'dárk')), ['Dark']);
});

test('what cannot be used now comes last', () => {
  assert.deepEqual(labels(search(items, 'pa')).at(-1), 'Paste');
  assert.equal(labels(search(items, '')).at(-1), 'Paste');
});

test('with nothing typed, the commands used lately come first', () => {
  const recent = ['View › Theme › Dark', 'File › Export › PDF…'];
  assert.deepEqual(labels(search(items, '', recent)).slice(0, 3), ['Dark', 'PDF…', 'Save']);
});

test('a command used goes to the front of the recent list, which is kept short', () => {
  assert.deepEqual(used(['a', 'b', 'c'], 'b'), ['b', 'a', 'c']);
  assert.deepEqual(used(['a', 'b', 'c'], 'd', 3), ['d', 'a', 'b']);
});
