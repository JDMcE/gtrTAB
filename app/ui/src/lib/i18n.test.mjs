// Run with `npm test`: Node reads the TypeScript module directly.
import assert from 'node:assert/strict';
import { afterEach, test } from 'node:test';
import { PSEUDO, pick, pseudo, rightToLeft, t, tn, useLanguage } from './i18n.ts';

afterEach(() => useLanguage('en'));

test('in English a text is as written, with its parameters filled in', () => {
  assert.equal(t('Save'), 'Save');
  assert.equal(t('Bar {bar} of {count}', { bar: 3, count: 8 }), 'Bar 3 of 8');
  assert.equal(t('Left {alone}'), 'Left {alone}');
});

test('a translated text is looked up by its English, and one without a translation stays in English', () => {
  useLanguage('fr', { Save: 'Enregistrer', 'Bar {bar}': 'Mesure {bar}', Empty: '' });
  assert.equal(t('Save'), 'Enregistrer');
  assert.equal(t('Bar {bar}', { bar: 2 }), 'Mesure 2');
  assert.equal(t('Undo'), 'Undo');
  assert.equal(t('Empty'), 'Empty');
});

test('a text that changes with a number takes the form the language has for it', () => {
  assert.equal(tn(1, '{count} bar', '{count} bars'), '1 bar');
  assert.equal(tn(4, '{count} bar', '{count} bars'), '4 bars');
  // Polish has a form for a few and another for many.
  useLanguage('pl', { '{count} bars': { one: '{count} takt', few: '{count} takty', many: '{count} taktów' } });
  assert.equal(tn(1, '{count} bar', '{count} bars'), '1 takt');
  assert.equal(tn(3, '{count} bar', '{count} bars'), '3 takty');
  assert.equal(tn(5, '{count} bar', '{count} bars'), '5 taktów');
});

test('the pseudo-language changes every text visibly, and leaves its parameters', () => {
  assert.equal(pseudo('Save'), '[Šávé··]');
  assert.equal(pseudo('Bar {bar}'), '[Báŕ {bar}·]');
  useLanguage(PSEUDO);
  assert.equal(t('Bar {bar}', { bar: 7 }).includes('7'), true);
  assert.notEqual(t('Save'), 'Save');
  assert.equal(tn(2, '{count} bar', '{count} bars').startsWith('[2 '), true);
});

test('the language is the first wanted that there is a translation for, else English', () => {
  assert.equal(pick(['de-AT', 'fr'], ['en', 'fr', 'de']), 'de');
  assert.equal(pick(['pt-BR'], ['en', 'pt-BR', 'pt']), 'pt-BR');
  assert.equal(pick(['ja'], ['en', 'fr']), 'en');
});

test('some languages are written from right to left', () => {
  assert.equal(rightToLeft('ar-EG'), true);
  assert.equal(rightToLeft('he'), true);
  assert.equal(rightToLeft('en-GB'), false);
});
