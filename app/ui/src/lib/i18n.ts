// Translation of the interface. Each text is written in English where it is used, wrapped in
// `t()`, and the English is what a translation is looked up by, so that a text without one
// stays in English. The language is chosen once, before the app starts, so that every text
// is translated where it is made; a change of language takes effect when the app starts again.
//
// `npm run messages` lists every text wrapped so, for translators to start from.

/**
 * A translation, by the English it translates. A text that changes with a number has a form
 * for each of the language's plural categories, by the English for many.
 */
export type Catalog = Record<string, string | Partial<Record<Intl.LDMLPluralRule, string>>>;

export type Params = Record<string, string | number>;

/** The pseudo-language, in which every translated text is visibly changed, to find those that are not. */
export const PSEUDO = 'qps';

/** Languages written from right to left. */
const RIGHT_TO_LEFT = new Set(['ar', 'fa', 'he', 'ur', 'yi']);

let language = 'en';
let catalog: Catalog = {};
let plurals = new Intl.PluralRules('en');

/** Uses a language and its translation from now on. */
export function useLanguage(code: string, translation: Catalog = {}): void {
  language = code;
  catalog = translation;
  plurals = new Intl.PluralRules(code === PSEUDO ? 'en' : code);
}

export const currentLanguage = () => language;
export const rightToLeft = (code: string) => RIGHT_TO_LEFT.has(code.split('-')[0]);

/** Fills `{name}` in a text with the parameter of that name. */
function fill(text: string, params?: Params): string {
  if (!params) return text;
  return text.replace(/\{(\w+)\}/g, (whole, name: string) => (name in params ? String(params[name]) : whole));
}

/** A text in the language in use: `t('Bar {bar}', { bar: 3 })`. */
export function t(text: string, params?: Params): string {
  const found = catalog[text];
  const translated = typeof found === 'string' && found ? found : text;
  return fill(language === PSEUDO ? pseudo(translated) : translated, params);
}

/**
 * A text that changes with a number, given in English for one and for many:
 * `tn(count, '{count} bar', '{count} bars')`. `count` is among the parameters.
 */
export function tn(count: number, one: string, other: string, params?: Params): string {
  const all = { count, ...params };
  const found = catalog[other];
  const forms = typeof found === 'object' ? found : undefined;
  const category = plurals.select(count);
  const translated = forms?.[category] ?? forms?.other ?? (category === 'one' ? one : other);
  return fill(language === PSEUDO ? pseudo(translated) : translated, all);
}

const ACCENTED: Record<string, string> = {
  a: 'á', b: 'ƀ', c: 'ç', d: 'ð', e: 'é', g: 'ĝ', h: 'ĥ', i: 'í', j: 'ĵ', k: 'ķ', l: 'ļ', n: 'ñ',
  o: 'ö', r: 'ŕ', s: 'š', t: 'ţ', u: 'ü', w: 'ŵ', y: 'ý', z: 'ž',
  A: 'Å', C: 'Ç', D: 'Ð', E: 'É', G: 'Ĝ', I: 'Í', L: 'Ļ', N: 'Ñ', O: 'Ö', R: 'Ŕ', S: 'Š', T: 'Ţ', U: 'Ü', Y: 'Ý', Z: 'Ž',
};

/**
 * A text as the pseudo-language has it: in brackets, with its letters accented and made a
 * third longer, as many languages are longer than English, but still readable. Parameters
 * are left as they are.
 */
export function pseudo(text: string): string {
  const accented = text
    .split(/(\{\w+\})/)
    .map((part, i) => (i % 2 ? part : part.replace(/[a-zA-Z]/g, (c) => ACCENTED[c] ?? c)))
    .join('');
  const letters = text.replace(/\{\w+\}/g, '').replace(/\s/g, '').length;
  return `[${accented}${'·'.repeat(Math.ceil(letters / 3))}]`;
}

/**
 * The language to use of those there are translations for, from those wanted in order, as
 * the browser lists them: the same language and region, or else the same language.
 */
export function pick(wanted: readonly string[], available: readonly string[]): string {
  const lower = available.map((code) => code.toLowerCase());
  for (const code of wanted) {
    const exact = lower.indexOf(code.toLowerCase());
    if (exact >= 0) return available[exact];
    const base = lower.indexOf(code.toLowerCase().split('-')[0]);
    if (base >= 0) return available[base];
  }
  return 'en';
}
