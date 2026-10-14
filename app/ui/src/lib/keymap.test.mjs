// Run with `npm test`: Node reads the TypeScript module directly.
import assert from 'node:assert/strict';
import { test } from 'node:test';
import { bind, chordOf, fixedKey, keymap, keysOf, reset, unbind } from './keymap.ts';

const press = (key, extra = {}) => ({
  key,
  code: extra.code ?? '',
  ctrlKey: !!extra.ctrl,
  metaKey: !!extra.meta,
  altKey: !!extra.alt,
  shiftKey: !!extra.shift,
});

test('a key press is named as the keys are shown', () => {
  assert.equal(chordOf(press('s', { code: 'KeyS', ctrl: true })), 'Ctrl+S');
  assert.equal(chordOf(press('S', { code: 'KeyS', ctrl: true, shift: true })), 'Ctrl+Shift+S');
  assert.equal(chordOf(press('B', { code: 'KeyB', shift: true })), 'Shift+B');
  assert.equal(chordOf(press('r', { code: 'KeyR' })), 'R');
  assert.equal(chordOf(press(' ', { code: 'Space' })), 'Space');
  assert.equal(chordOf(press('ArrowUp', { code: 'ArrowUp', ctrl: true })), 'Ctrl+↑');
  assert.equal(chordOf(press('Delete', { code: 'Delete', ctrl: true })), 'Ctrl+Delete');
  assert.equal(chordOf(press('F1', { code: 'F1' })), 'F1');
});

test('Caps Lock does not change a letter, and a character typed with Shift is the character', () => {
  assert.equal(chordOf(press('R', { code: 'KeyR' })), 'R');
  assert.equal(chordOf(press('?', { code: 'Slash', shift: true })), '?');
  assert.equal(chordOf(press('<', { code: 'Comma', shift: true })), '<');
  assert.equal(chordOf(press('[', { code: 'BracketLeft' })), '[');
});

test('with Ctrl or Alt, a letter or digit is named by where it is on the keyboard', () => {
  // Cmd counts as Ctrl.
  assert.equal(chordOf(press('k', { code: 'KeyK', meta: true })), 'Ctrl+K');
  // A Russian layout types к where K is.
  assert.equal(chordOf(press('к', { code: 'KeyK', ctrl: true })), 'Ctrl+K');
  // Alt+1 on a Mac types ¡.
  assert.equal(chordOf(press('¡', { code: 'Digit1', alt: true })), 'Alt+1');
});

test('a modifier on its own is not a chord', () => {
  assert.equal(chordOf(press('Control', { code: 'ControlLeft', ctrl: true })), null);
  assert.equal(chordOf(press('Shift', { code: 'ShiftLeft', shift: true })), null);
});

const commands = [
  { id: 'undo', keys: 'Ctrl+Z' },
  { id: 'redo', keys: 'Ctrl+Y / Ctrl+Shift+Z' },
  { id: 'rest', keys: 'R' },
  { id: 'loop' },
];

test('the keymap has each command\'s own keys until they are changed', () => {
  const map = keymap(commands, {});
  assert.equal(map.get('Ctrl+Shift+Z'), 'redo');
  assert.equal(map.get('R'), 'rest');
  assert.equal(map.size, 4);
  assert.equal(keymap(commands, { rest: ['Shift+R'] }).get('R'), undefined);
});

test('a key given to a command is taken from the one that had it', () => {
  const { bindings, from } = bind(commands, {}, 'loop', 'R');
  assert.equal(from, 'rest');
  assert.deepEqual(bindings, { loop: ['R'], rest: [] });
  assert.equal(keymap(commands, bindings).get('R'), 'loop');
  assert.deepEqual(bind(commands, {}, 'loop', 'Ctrl+L'), { bindings: { loop: ['Ctrl+L'] }, from: null });
});

test('only the changes are kept: a command given back its own keys is left out', () => {
  let custom = unbind(commands, {}, 'redo', 'Ctrl+Shift+Z');
  assert.deepEqual(custom, { redo: ['Ctrl+Y'] });
  custom = bind(commands, custom, 'redo', 'Ctrl+Shift+Z').bindings;
  assert.deepEqual(custom, {});
});

test('reset gives a command its own keys back, from whichever command has them now', () => {
  let custom = bind(commands, {}, 'loop', 'R').bindings;
  custom = bind(commands, custom, 'rest', 'Shift+R').bindings;
  custom = reset(commands, custom, 'rest');
  assert.deepEqual(keysOf(commands[2], custom), ['R']);
  assert.deepEqual(keysOf(commands[3], custom), []);
  assert.deepEqual(custom, {});
});

test('the keys that move the cursor and enter notes cannot be given to a command', () => {
  for (const chord of ['←', 'Shift+→', 'Ctrl+←', 'Ctrl+Shift+→', 'Home', 'Shift+End', 'Alt+↑', '7', '+', '-', 'Enter', 'Escape', 'Tab']) {
    assert.ok(fixedKey(chord), chord);
  }
  for (const chord of ['Ctrl+↑', 'Ctrl+↓', 'Alt+1', 'R', 'Shift+B', 'Ctrl+Delete', 'Space', 'F2']) {
    assert.ok(!fixedKey(chord), chord);
  }
});
