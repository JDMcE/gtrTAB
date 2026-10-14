// Lists every text of the interface wrapped in t() or tn(), for translators to start from.
//
//   npm run messages            writes src/locales/messages.json
//
// A text has to be written out where it is wrapped, as a string in quotes, for it to be
// found here; `messages.test.mjs` fails on one that is not, and on a translation of a text
// that is no longer in the interface.

import { readdirSync, readFileSync, writeFileSync } from 'node:fs';
import { join, relative } from 'node:path';
import { fileURLToPath } from 'node:url';

const ui = fileURLToPath(new URL('..', import.meta.url));
export const SOURCE = join(ui, 'src');
export const LOCALES = join(SOURCE, 'locales');

function* sources(dir) {
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const path = join(dir, entry.name);
    if (entry.isDirectory()) {
      if (!['wasm', 'locales', 'generated'].includes(entry.name)) yield* sources(path);
    } else if (/\.(ts|svelte)$/.test(entry.name) && !/\.test\./.test(entry.name) && entry.name !== 'i18n.ts') {
      yield path;
    }
  }
}

/** Reads a string in quotes starting at `at`; `null` if there is none, or it has `${}` in it. */
function literal(text, at) {
  const quote = text[at];
  if (!`'"\``.includes(quote)) return null;
  let value = '';
  for (let i = at + 1; i < text.length; i++) {
    const c = text[i];
    if (c === '\\') {
      const next = text[++i];
      value += { n: '\n', t: '\t' }[next] ?? next;
    } else if (c === quote) return { value, end: i + 1 };
    else if (quote === '`' && c === '$' && text[i + 1] === '{') return null;
    else value += c;
  }
  return null;
}

const skipSpace = (text, at) => {
  while (/\s/.test(text[at] ?? '')) at++;
  return at;
};

/** Skips one argument of a call, to the comma after it. */
function skipArgument(text, at) {
  let depth = 0;
  for (let i = at; i < text.length; i++) {
    const c = text[i];
    if ('([{'.includes(c)) depth++;
    else if (')]}'.includes(c)) depth--;
    else if (c === ',' && depth === 0) return i + 1;
  }
  return text.length;
}

/**
 * The texts in the interface: `messages` by their English (for many, with `tn`), and
 * `problems`, each a place where t() or tn() is given something other than a text in quotes.
 */
export function extract() {
  const messages = new Map();
  const problems = [];
  for (const path of sources(SOURCE)) {
    const text = readFileSync(path, 'utf8');
    const file = relative(ui, path);
    for (const match of text.matchAll(/(?<![\w$.])(tn?)\(/g)) {
      const line = text.slice(0, match.index).split('\n').length;
      let at = skipSpace(text, match.index + match[0].length);
      const where = `${file}:${line}`;
      if (match[1] === 't') {
        const found = literal(text, at);
        if (!found) problems.push(where);
        else messages.set(found.value, { where });
      } else {
        at = skipSpace(text, skipArgument(text, at));
        const one = literal(text, at);
        const other = one && literal(text, skipSpace(text, skipSpace(text, one.end) + 1));
        if (!other) problems.push(where);
        else messages.set(other.value, { one: one.value, where });
      }
    }
  }
  return { messages, problems };
}

/** The translations there are: their codes and what is in them. */
export function catalogs() {
  return readdirSync(LOCALES)
    .filter((name) => name.endsWith('.json') && name !== 'messages.json')
    .map((name) => ({ code: name.slice(0, -5), catalog: JSON.parse(readFileSync(join(LOCALES, name), 'utf8')) }));
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const { messages, problems } = extract();
  for (const where of problems) console.warn(`${where}: not a text in quotes, so it cannot be translated`);
  const out = Object.fromEntries(
    [...messages].sort(([a], [b]) => a.localeCompare(b)).map(([text, { one, where }]) => [text, one ? { one, other: '', where } : { where }]),
  );
  writeFileSync(join(LOCALES, 'messages.json'), `${JSON.stringify(out, null, 2)}\n`);
  console.log(`${messages.size} texts written to ${relative(process.cwd(), join(LOCALES, 'messages.json'))}`);
}
