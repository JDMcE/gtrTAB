// Interface preferences, kept in localStorage: theme and which panels are open.

import { isTauri } from './files';
import type { Notation } from './model';

export type Theme = 'system' | 'light' | 'dark';
export const THEMES: Theme[] = ['system', 'light', 'dark'];
/** High contrast, or the system's setting for it. */
export type Contrast = 'system' | 'normal' | 'high';
/** What the score is drawn on: the theme's paper, or white whatever the theme. */
export type Paper = 'theme' | 'white';

export interface Prefs {
  theme: Theme;
  contrast: Contrast;
  paper: Paper;
  /** The language of the interface, by its code; `system` follows the browser's. */
  language: string;
  /** Palette as a narrow strip; `null` follows the window width. */
  compact: boolean | null;
  /** Inspector shown; `null` follows the window width. */
  inspector: boolean | null;
  /** Ids of the palette sections that are folded away. */
  collapsed: string[];
  /** Staves each track is shown on. */
  notation: Notation;
  /**
   * What the score is shown on: pages one under another or side by side, one sheet as
   * wide as the window, or one line holding every bar.
   */
  layout: 'pages' | 'pagesAcross' | 'parchment' | 'screen';
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
  /** The visual metronome shown in the transport bar. */
  beatLights: boolean;
  /** The fretboard, keyboard or drum kit shown under the score. */
  instrument: boolean;
  /** The pedalboard shown under the score. */
  pedalboard: boolean;
  /** The audio track shown under the score, if the score has one. */
  audio: boolean;
  /** Ids of the commands last run from the command palette, the latest first. */
  recentCommands: string[];
  /** The keys the user has given commands, or taken from them, by command id. */
  shortcuts: Record<string, string[]>;
}

const KEY = 'gtrtab.ui';
const DEFAULTS: Prefs = {
  theme: 'system',
  contrast: 'system',
  paper: 'theme',
  language: 'system',
  compact: null,
  inspector: null,
  collapsed: [],
  notation: 'both',
  layout: 'parchment',
  multitrack: false,
  tracks: true,
  tracksHeight: null,
  sound: 'default',
  beatLights: false,
  instrument: false,
  pedalboard: false,
  audio: true,
  recentCommands: [],
  shortcuts: {},
};

/** The preferences the app starts with. */
export const defaultPrefs = (): Prefs => structuredClone(DEFAULTS);

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

const moreContrast = () => window.matchMedia?.('(prefers-contrast: more)');

/**
 * Switches the page to high contrast or back, and the score to white paper or the theme's.
 * Following the system, it changes when the system's setting does; the function returned
 * stops that.
 */
export function applyLook(contrast: Contrast, paper: Paper, onchange?: () => void): () => void {
  const root = document.documentElement;
  const query = moreContrast();
  const set = () => {
    const high = contrast === 'high' || (contrast === 'system' && !!query?.matches);
    if (high) root.dataset.contrast = 'high';
    else delete root.dataset.contrast;
  };
  set();
  if (paper === 'white') root.dataset.paper = 'white';
  else delete root.dataset.paper;
  if (contrast !== 'system' || !query) return () => {};
  const changed = () => {
    set();
    onchange?.();
  };
  query.addEventListener('change', changed);
  return () => query.removeEventListener('change', changed);
}

/** Whether the page is in high contrast now. */
export const highContrast = () => document.documentElement.dataset.contrast === 'high';
