// Which keys run which commands. Each command has the keys it comes with; the user can give
// it others, or take them away, and only those changes are kept.

/** A key with its modifiers, as shown to the user: `Ctrl+Shift+S`, `Alt+1`, `?`, `Space`, `Ctrl+↑`. */
export type Chord = string;

/** The keys of the commands the user has changed, by command id. A command left out has its own. */
export type Bindings = Record<string, Chord[]>;

/** What the keymap needs of a command. */
export interface Bindable {
  id: string;
  /** The keys it comes with, several separated by ` / `. */
  keys?: string;
}

/** The fields of a `KeyboardEvent` that say which key it is. */
export interface KeyPress {
  key: string;
  code: string;
  ctrlKey: boolean;
  metaKey: boolean;
  altKey: boolean;
  shiftKey: boolean;
}

const NAMES: Record<string, string> = {
  ' ': 'Space',
  ArrowLeft: '←',
  ArrowRight: '→',
  ArrowUp: '↑',
  ArrowDown: '↓',
};

/** Keys that are only ever held with another, or that do not name a key. */
const NOT_KEYS = new Set(['Control', 'Shift', 'Alt', 'AltGraph', 'Meta', 'OS', 'CapsLock', 'Dead', 'Process', 'Unidentified', 'Compose']);

/**
 * The chord a key press is, or `null` for a modifier on its own. Cmd on a Mac counts as
 * Ctrl. A character typed with Shift is the character alone, as `?` is, except for letters,
 * where Shift is part of the chord. With Ctrl or Alt held, a letter or digit is named by
 * where it is on the keyboard, so that Ctrl+K is Ctrl+K in any layout and Alt+1 on a Mac
 * is not `¡`.
 */
export function chordOf(press: KeyPress): Chord | null {
  const { key, code } = press;
  if (NOT_KEYS.has(key)) return null;
  const ctrl = press.ctrlKey || press.metaKey;
  const alt = press.altKey;
  let name = NAMES[key] ?? key;
  let shift = press.shiftKey;
  if ((ctrl || alt) && /^Key[A-Z]$/.test(code)) name = code.slice(3);
  else if ((ctrl || alt) && /^Digit[0-9]$/.test(code) && !shift) name = code.slice(5);
  else if (key.length === 1) {
    const upper = key.toUpperCase();
    if (upper !== key.toLowerCase()) name = upper;
    else shift = false;
  }
  return [ctrl && 'Ctrl', alt && 'Alt', shift && 'Shift', name].filter(Boolean).join('+');
}

/** The keys a command comes with. */
export const defaultsOf = (command: Bindable): Chord[] => (command.keys ? command.keys.split(' / ') : []);

/** The keys a command has now. */
export const keysOf = (command: Bindable, custom: Bindings): Chord[] => custom[command.id] ?? defaultsOf(command);

/** Which command each key runs. */
export function keymap(commands: Bindable[], custom: Bindings): Map<Chord, string> {
  const map = new Map<Chord, string>();
  for (const command of commands) {
    for (const chord of keysOf(command, custom)) if (!map.has(chord)) map.set(chord, command.id);
  }
  return map;
}

/** Keeps only the commands whose keys differ from those they come with. */
function tidy(commands: Bindable[], custom: Bindings): Bindings {
  const kept: Bindings = {};
  for (const command of commands) {
    const keys = custom[command.id];
    if (keys && keys.join(' / ') !== defaultsOf(command).join(' / ')) kept[command.id] = keys;
  }
  return kept;
}

/**
 * Gives a command a key, taking it from any other command that had it. Returns the new
 * bindings and the id of the command the key was taken from, if any.
 */
export function bind(
  commands: Bindable[],
  custom: Bindings,
  id: string,
  chord: Chord,
): { bindings: Bindings; from: string | null } {
  const next: Bindings = { ...custom };
  let from: string | null = null;
  for (const command of commands) {
    const keys = keysOf(command, next);
    if (command.id !== id && keys.includes(chord)) {
      from = command.id;
      next[command.id] = keys.filter((k) => k !== chord);
    }
  }
  const target = commands.find((c) => c.id === id);
  if (target) {
    const keys = keysOf(target, next);
    if (!keys.includes(chord)) next[id] = [...keys, chord];
  }
  return { bindings: tidy(commands, next), from };
}

/** Takes a key away from a command. */
export function unbind(commands: Bindable[], custom: Bindings, id: string, chord: Chord): Bindings {
  const target = commands.find((c) => c.id === id);
  if (!target) return custom;
  return tidy(commands, { ...custom, [id]: keysOf(target, custom).filter((k) => k !== chord) });
}

/**
 * Gives a command back the keys it comes with, taking them from any command the user has
 * given them to since.
 */
export function reset(commands: Bindable[], custom: Bindings, id: string): Bindings {
  const target = commands.find((c) => c.id === id);
  if (!target) return custom;
  let next: Bindings = { ...custom };
  delete next[id];
  for (const chord of defaultsOf(target)) next = bind(commands, next, id, chord).bindings;
  return tidy(commands, next);
}

/**
 * Keys that move the cursor, enter notes or move the focus, which are not commands and
 * cannot be given to one: the arrows, Home and End with or without Shift and Ctrl, the
 * digits, + and −, Enter, Tab and Esc. Ctrl+↑ and Ctrl+↓ are commands: the track above or
 * below.
 */
export function fixedKey(chord: Chord): boolean {
  if (/^Ctrl\+(Shift\+)?[↑↓]$/.test(chord)) return false;
  return (
    /^(Ctrl\+)?(Shift\+)?([←→↑↓]|Home|End)$/.test(chord) ||
    /^Alt\+[↑↓]$/.test(chord) ||
    /^[0-9+=-]$/.test(chord) ||
    /^(Shift\+)?(Enter|Tab|Escape)$/.test(chord)
  );
}

/** The symbols a Mac shows for the modifiers, in the order it shows them, and for some keys. */
const MAC_MODIFIERS: [string, string][] = [
  ['Alt', '⌥'],
  ['Shift', '⇧'],
  ['Ctrl', '⌘'],
];
const MAC_KEYS: Record<string, string> = { Delete: '⌦', Backspace: '⌫', Enter: '↩', Tab: '⇥' };

/**
 * Keys as they are shown on the system: as written on Windows and Linux, and on a Mac with
 * its symbols, the modifiers in the Mac's order and run together, as `⇧⌘S`. Takes one chord
 * or several separated by ` / `, and leaves ranges such as `0–9` as they are.
 */
export function keyLabel(keys: string, mac: boolean): string {
  if (!mac) return keys;
  return keys
    .split(' / ')
    .map((chord) => {
      // Ranges and lists, such as `Alt+1 – Alt+4` or `← → ↑ ↓`, are labelled a part at a time.
      if (/\s/.test(chord)) return chord.split(/(\s+)/).map((part) => (/\s/.test(part) ? part : keyLabel(part, mac))).join('');
      const parts = chord.split('+');
      // `+` on its own, or as the key of a chord, is the last part left empty.
      const key = chord.endsWith('+') ? '+' : parts[parts.length - 1];
      const held = new Set(chord.endsWith('+') ? parts.slice(0, -2) : parts.slice(0, -1));
      const modifiers = MAC_MODIFIERS.filter(([name]) => held.has(name)).map(([, symbol]) => symbol).join('');
      const name = MAC_KEYS[key] ?? key;
      // A word, such as `arrows`, stands a space apart from the symbols.
      return modifiers && /^[a-z]{2,}/.test(name) ? `${modifiers} ${name}` : modifiers + name;
    })
    .join(' / ');
}

const ARIA_KEYS: Record<string, string> = { '←': 'ArrowLeft', '→': 'ArrowRight', '↑': 'ArrowUp', '↓': 'ArrowDown' };

/**
 * Keys as `aria-keyshortcuts` has them, for a screen reader: the names of the keys as a
 * browser gives them, Ctrl being the Command key, `Meta`, on a Mac, and several chords
 * separated by spaces.
 */
export function ariaKeys(keys: string, mac: boolean): string {
  return keys
    .split(' / ')
    .map((chord) =>
      chord
        .split(/\+(?=.)/)
        .map((part) => (part === 'Ctrl' ? (mac ? 'Meta' : 'Control') : (ARIA_KEYS[part] ?? part)))
        .join('+'),
    )
    .join(' ');
}
