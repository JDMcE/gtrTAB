// Run with `npm test`. Every text of the interface that is to be translated is written out
// where it is wrapped, and every translation is of texts the interface still has.
import assert from 'node:assert/strict';
import { test } from 'node:test';
import { catalogs, extract } from '../../scripts/messages.mjs';

const { messages, problems } = extract();

test('every text given to t() and tn() is written out in quotes, so that it can be found', () => {
  assert.deepEqual(problems, []);
  assert.ok(messages.has('Save'));
  assert.ok(messages.has('Keyboard shortcuts'));
});

test('a translation has only texts the interface has, each with the same parameters', () => {
  const params = (text) => [...text.matchAll(/\{(\w+)\}/g)].map((m) => m[1]).sort();
  for (const { code, catalog } of catalogs()) {
    for (const [text, translation] of Object.entries(catalog)) {
      if (text.startsWith('$')) continue;
      assert.ok(messages.has(text), `${code}: "${text}" is no longer in the interface`);
      for (const form of typeof translation === 'string' ? [translation] : Object.values(translation)) {
        if (form) assert.deepEqual(params(form), params(text), `${code}: "${text}"`);
      }
    }
  }
});
