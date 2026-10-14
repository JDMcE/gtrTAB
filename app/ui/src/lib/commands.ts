// Everything the interface can do is a command. The bars, palette, menus, tooltips and
// shortcut list are all built from the same set, so a name or key is written once.

import type { IconName } from './ui/icons';

export interface Command {
  /** Names the command for the keys the user gives it; one without an id cannot be given any. */
  id?: string;
  label: string;
  icon?: IconName;
  /** The keys it comes with, as shown to the user, e.g. `Ctrl+Z`; several are separated by ` / `. */
  keys?: string;
  run: () => unknown;
  /** For commands that switch something on and off. */
  active?: () => boolean;
  disabled?: () => boolean;
}

/** A menu holds commands, `'-'` separators and one level of submenus. */
export type MenuEntry = Command | '-' | { label: string; items: MenuEntry[] };

export interface Menu {
  label: string;
  items: MenuEntry[];
}

export interface PaletteSection {
  id: string;
  title: string;
  commands: Command[];
}

export interface ShortcutGroup {
  title: string;
  rows: { keys: string; label: string }[];
}
