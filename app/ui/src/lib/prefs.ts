// Interface preferences, kept in localStorage: theme and which panels are open.

import { isTauri } from './files';
import type { Notation } from './model';

export type Theme = 'system' | 'light' | 'dark';
export const THEMES: Theme[] = ['system', 'light', 'dark'];

export interface Prefs {
  theme: Theme;
  /** Palette as a narrow strip; `null` follows the window width. */
  compact: boolean | null;
  /** Inspector shown; `null` follows the window width. */
  inspector: boolean | null;
  /** Ids of the palette sections that are folded away. */
  collapsed: string[];
  /** Staves each track is shown on. */
  notation: Notation;
  /** Every track shown in the score, not just the one being edited. */
  multitrack: boolean;
  /** Track pane shown under the score. */
  tracks: boolean;
  /** Height of the track pane in pixels; `null` fits the tracks. */
  tracksHeight: number | null;
  /**
   * What playback sounds through: the SoundFont that ships with the app, one the user
   * picked, or the synth's own models of the instruments.
   */
  sound: 'default' | 'custom' | 'models';
}

const KEY = 'gtrtab.ui';
const DEFAULTS: Prefs = {
  theme: 'system',
  compact: null,
  inspector: null,
  collapsed: [],
  notation: 'both',
  multitrack: false,
  tracks: true,
  tracksHeight: null,
  sound: 'default',
};

export function loadPrefs(): Prefs {
  try {
    return { ...DEFAULTS, ...JSON.parse(localStorage.getItem(KEY) ?? '{}') };
  } catch {
    return { ...DEFAULTS };
  }
}

export function savePrefs(prefs: Prefs): void {
  try {
    localStorage.setItem(KEY, JSON.stringify(prefs));
  } catch {
    // Storage may be unavailable; the interface then starts from its defaults.
  }
}

/** Switches the page and, on desktop, the window frame to a theme. */
export function applyTheme(theme: Theme): void {
  if (theme === 'system') delete document.documentElement.dataset.theme;
  else document.documentElement.dataset.theme = theme;
  if (isTauri) {
    import('@tauri-apps/api/window')
      .then(({ getCurrentWindow }) => getCurrentWindow().setTheme(theme === 'system' ? null : theme))
      // The page has the theme either way; say so if the window's frame did not follow.
      .catch((error) => console.warn('The window frame did not take the theme:', error));
  }
}
