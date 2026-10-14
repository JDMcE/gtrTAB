// The translations there are, and choosing one before the app starts.

import { PSEUDO, pick, rightToLeft, t, useLanguage, type Catalog } from './i18n';

/**
 * Each translation is `src/locales/<code>.json`: the English of each text and its translation,
 * and under `$language` the language's name in that language. `npm run messages` writes
 * the texts to translate to `src/locales/messages.json`, which is not a translation.
 */
const files = import.meta.glob<Catalog & { $language?: string }>(['../locales/*.json', '!../locales/messages.json'], {
  import: 'default',
});
const codeOf = (path: string) => path.replace(/^.*\/|\.json$/g, '');

export interface Language {
  code: string;
  /** Its name in itself. */
  name: string;
}

/** The languages that can be chosen, English first. Names are read once the files are. */
export const languages = async (): Promise<Language[]> => {
  const found = await Promise.all(
    Object.entries(files).map(async ([path, load]) => ({ code: codeOf(path), name: (await load()).$language ?? codeOf(path) })),
  );
  return [
    { code: 'en', name: 'English' },
    ...found.sort((a, b) => a.name.localeCompare(b.name)),
    { code: PSEUDO, name: t('Pseudo-language, to find texts not translated') },
  ];
};

/**
 * Uses the language chosen, or with `system` the first of the browser's that there is a
 * translation for, and marks the page with it so that it is read out and laid out as it is
 * written. A translation that cannot be read leaves the interface in English.
 */
export async function startLanguage(chosen: string): Promise<void> {
  const available = ['en', PSEUDO, ...Object.keys(files).map(codeOf)];
  const code = chosen === 'system' ? pick(navigator.languages ?? [], available) : chosen;
  const load = Object.entries(files).find(([path]) => codeOf(path) === code)?.[1];
  try {
    useLanguage(load ? code : code === PSEUDO ? PSEUDO : 'en', load ? await load() : {});
  } catch (error) {
    console.warn(`The ${code} translation could not be read:`, error);
    useLanguage('en');
  }
  document.documentElement.lang = code === PSEUDO ? 'en' : load ? code : 'en';
  document.documentElement.dir = rightToLeft(code) ? 'rtl' : 'ltr';
}
