// The command palette's search: every command in the menus, found by a few letters of the
// words in its name or in the menus it is under.

import type { Command, Menu, MenuEntry } from './commands';

export interface Found {
  command: Command;
  /** The menus it is under, e.g. `['File', 'Export']`. */
  path: string[];
  /** Its path and name, which stay the same from one run to the next. */
  id: string;
}

/** Every command in the menus, each once, where it first appears. */
export function listCommands(menus: Menu[]): Found[] {
  const found: Found[] = [];
  const seen = new Set<Command>();
  const walk = (items: MenuEntry[], path: string[]) => {
    for (const item of items) {
      if (item === '-') continue;
      if ('items' in item) walk(item.items, [...path, item.label]);
      else if (!seen.has(item)) {
        seen.add(item);
        found.push({ command: item, path, id: [...path, item.label].join(' › ') });
      }
    }
  };
  for (const menu of menus) walk(menu.items, [menu.label]);
  return found;
}

/** Lower case, without accents or the dots that say a dialog follows. */
const plain = (text: string) =>
  text.normalize('NFD').replace(/[̀-ͯ]/g, '').replace(/…/g, '').toLowerCase();

const words = (text: string) => plain(text).split(/[^\p{L}\p{N}%]+/u).filter(Boolean);

/**
 * How well a query matches a command; `null` if it does not. Every word of the query has
 * to begin a word of the name or of a menu it is under, the name counting for more.
 */
function score(item: Found, query: string[]): number | null {
  const name = words(item.command.label);
  const menus = item.path.flatMap(words);
  let total = 0;
  for (const word of query) {
    if (name.some((w) => w.startsWith(word))) total += name.includes(word) ? 4 : 3;
    else if (menus.some((w) => w.startsWith(word))) total += 1;
    else if (word.length >= 3 && plain(item.command.label).includes(word)) total += 0.5;
    else return null;
  }
  // A name that begins as the query does comes first: "sa" finds Save before Pinch harmonic.
  if (plain(item.command.label).startsWith(query.join(' '))) total += 5;
  else if (name[0]?.startsWith(query[0])) total += 2;
  return total;
}

/**
 * The commands a query finds, best first, those that cannot be used now last. With no query,
 * the ones used most recently and then the rest in the order of the menus.
 */
export function search(items: Found[], query: string, recent: string[] = []): Found[] {
  const terms = words(query);
  const usable = (item: Found) => (item.command.disabled?.() ? 1 : 0);
  const lately = (item: Found) => {
    const at = recent.indexOf(item.id);
    return at < 0 ? recent.length : at;
  };
  const ranked = items
    .map((item, order) => ({ item, order, score: terms.length ? score(item, terms) : 0 }))
    .filter((r): r is typeof r & { score: number } => r.score !== null);
  ranked.sort(
    (a, b) =>
      usable(a.item) - usable(b.item) ||
      b.score - a.score ||
      lately(a.item) - lately(b.item) ||
      a.order - b.order,
  );
  return ranked.map((r) => r.item);
}

/** The recent list with a command put first, kept to `limit` entries. */
export function used(recent: string[], id: string, limit = 8): string[] {
  return [id, ...recent.filter((r) => r !== id)].slice(0, limit);
}
