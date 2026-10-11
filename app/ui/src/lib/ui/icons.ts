// The interface's icons. Music symbols come from Bravura (SMuFL code points), general
// actions from Lucide, and guitar techniques that neither covers are drawn here on Lucide's
// 24-unit grid so that the three sets sit together.

import {
  Check,
  ChevronDown,
  ChevronRight,
  CircleQuestionMark,
  ClipboardPaste,
  Copy,
  FilePlus,
  FileText,
  FolderOpen,
  Gauge,
  Guitar,
  Hand,
  Keyboard,
  Metronome,
  Monitor,
  Moon,
  PanelLeft,
  PanelRight,
  Pencil,
  Play,
  Redo2,
  Save,
  SaveAll,
  Scissors,
  Square,
  Sun,
  Undo2,
  X,
  ZoomIn,
  ZoomOut,
} from '@lucide/svelte';
import type { Component } from 'svelte';

export type IconDef =
  | { lucide: Component<{ size?: number | string; strokeWidth?: number | string }> }
  /** A Bravura glyph: font size and baseline position on the 24-unit grid. */
  | { glyph: string; size: number; x?: number; y: number }
  /** SVG content, stroked in the current colour unless it sets its own fill. */
  | { svg: string };

const glyph = (code: number, size: number, y: number, x = 12): IconDef => ({
  glyph: String.fromCodePoint(code),
  size,
  x,
  y,
});
const svg = (content: string): IconDef => ({ svg: content });

const FILL = 'fill="currentColor" stroke="none"';
const letters = (text: string, x: number, y: number, size: number) =>
  `<text x="${x}" y="${y}" text-anchor="middle" font-size="${size}" font-weight="700" ` +
  `font-family="system-ui, sans-serif" ${FILL}>${text}</text>`;
const dot = (x: number, y: number, r = 2.2) => `<circle cx="${x}" cy="${y}" r="${r}" ${FILL}/>`;
const head = (x: number, y: number) =>
  `<ellipse cx="${x}" cy="${y}" rx="4" ry="3" transform="rotate(-20 ${x} ${y})" ${FILL}/>`;
const strings = '<path d="M11 6h10M11 12h10M11 18h10" stroke-width="1.2"/>';

export const ICONS = {
  // General actions (Lucide)
  new: { lucide: FilePlus },
  open: { lucide: FolderOpen },
  save: { lucide: Save },
  saveAs: { lucide: SaveAll },
  undo: { lucide: Undo2 },
  redo: { lucide: Redo2 },
  cut: { lucide: Scissors },
  copy: { lucide: Copy },
  paste: { lucide: ClipboardPaste },
  play: { lucide: Play },
  stop: { lucide: Square },
  metronome: { lucide: Metronome },
  speed: { lucide: Gauge },
  zoomIn: { lucide: ZoomIn },
  zoomOut: { lucide: ZoomOut },
  palette: { lucide: PanelLeft },
  inspector: { lucide: PanelRight },
  shortcuts: { lucide: Keyboard },
  help: { lucide: CircleQuestionMark },
  themeSystem: { lucide: Monitor },
  themeLight: { lucide: Sun },
  themeDark: { lucide: Moon },
  tuning: { lucide: Guitar },
  scoreInfo: { lucide: FileText },
  fingering: { lucide: Hand },
  editValue: { lucide: Pencil },
  remove: { lucide: X },
  check: { lucide: Check },
  chevronDown: { lucide: ChevronDown },
  chevronRight: { lucide: ChevronRight },

  // Note values, rests and ornaments (Bravura)
  whole: glyph(0xe1d2, 32, 13),
  half: glyph(0xe1d3, 21, 20),
  quarter: glyph(0xe1d5, 21, 20),
  eighth: glyph(0xe1d7, 21, 20, 10),
  sixteenth: glyph(0xe1d9, 21, 20, 10),
  thirty_second: glyph(0xe1db, 19, 21, 10),
  sixty_fourth: glyph(0xe1dd, 16, 21.5, 10),
  rest: glyph(0xe4e5, 22, 12.5),
  trill: glyph(0xe566, 34, 17),
  grace: glyph(0xe560, 34, 21),
  tempo: glyph(0xe1d5, 17, 18),
  ppp: glyph(0xe52a, 22, 16),
  pp: glyph(0xe52b, 30, 17),
  p: glyph(0xe520, 34, 17.5),
  mp: glyph(0xe52c, 26, 16.5),
  mf: glyph(0xe52d, 26, 16.5),
  f: glyph(0xe522, 34, 17.5),
  ff: glyph(0xe52f, 30, 17),
  fff: glyph(0xe530, 22, 16),
  timeSignature: svg(
    `<text x="12" y="7" text-anchor="middle" font-size="22" font-family="Bravura" ${FILL}></text>` +
      `<text x="12" y="17.5" text-anchor="middle" font-size="22" font-family="Bravura" ${FILL}></text>`,
  ),

  // Rhythm
  dot: svg(head(9, 13) + dot(18, 13, 2.1)),
  triplet: svg(`<path d="M3 13V8h5M16 8h5v5"/>${letters('3', 12, 12, 11)}`),
  tuplet: svg(`<path d="M3 13V8h5M16 8h5v5"/>${letters('n', 12, 11.5, 11)}`),
  tie: svg(head(5.5, 9) + head(18.5, 9) + '<path d="M5 15c4 4.5 10 4.5 14 0"/>'),

  // Techniques on the note
  hammer: svg(dot(5, 18) + dot(19, 18) + `<path d="M5 13c3-5 11-5 14 0"/>${letters('H', 12, 7.5, 8)}`),
  slideShift: svg(dot(4.5, 17.5) + dot(19.5, 6.5) + '<path d="M8 15l8-6"/>'),
  slideLegato: svg(
    dot(4.5, 18.5) + dot(19.5, 9.5) + '<path d="M8 16.5l8-5"/><path d="M4 12c3-7 10-9 15-7"/>',
  ),
  slideInBelow: svg(dot(18, 8, 2.6) + '<path d="M4 18l10-8"/>'),
  slideInAbove: svg(dot(18, 16, 2.6) + '<path d="M4 6l10 8"/>'),
  slideOutDown: svg(dot(6, 8, 2.6) + '<path d="M10 10l10 8"/>'),
  slideOutUp: svg(dot(6, 16, 2.6) + '<path d="M10 14l10-8"/>'),
  bend: svg('<path d="M3 19h3c7 0 11-5 11-13"/><path d="M13 9.5L17 5.5l4 4"/>'),
  vibrato: svg('<path d="M2 12c1.7-4 3.3-4 5 0s3.3 4 5 0 3.3-4 5 0 3.3 4 5 0"/>'),
  wideVibrato: svg(
    '<path d="M2 12c1.7-8 3.3-8 5 0s3.3 8 5 0 3.3-8 5 0 3.3 8 5 0" stroke-width="2.6"/>',
  ),
  palmMute: svg(
    `${letters('PM', 9.5, 11, 10)}<path d="M2 17h17" stroke-dasharray="3 2.4"/><path d="M21 14v6"/>`,
  ),
  letRing: svg(
    dot(5.5, 12, 2.6) + '<path d="M11 8a6 6 0 0 1 0 8"/><path d="M15 5a11 11 0 0 1 0 14"/>',
  ),
  dead: svg('<path d="M6 6l12 12M18 6L6 18" stroke-width="2.2"/>'),
  ghost: svg(dot(12, 12, 2.6) + '<path d="M8 5c-3.5 4-3.5 10 0 14"/><path d="M16 5c3.5 4 3.5 10 0 14"/>'),
  staccato: svg(head(12, 16) + dot(12, 7, 1.9)),
  accent: svg(head(12, 18) + '<path d="M6 4.5l12 3.5-12 3.5"/>'),
  heavyAccent: svg(head(12, 18) + '<path d="M7.5 12l4.5-9 4.5 9"/>'),
  harmonic: svg('<path d="M12 4l6.5 8-6.5 8-6.5-8z"/>'),
  tapping: svg(`<circle cx="12" cy="12" r="9"/>${letters('T', 12, 16, 11)}`),

  // Techniques on the beat
  crescendo: svg('<path d="M20 6L4 12l16 6"/>'),
  decrescendo: svg('<path d="M4 6l16 6-16 6"/>'),
  strokeDown: svg('<path d="M6 19V7h12v12"/><path d="M6 7.5h12" stroke-width="4" stroke-linecap="butt"/>'),
  strokeUp: svg('<path d="M6 5l6 14 6-14"/>'),
  // Arrows point the way the score draws them: a downstroke runs up the page.
  strumDown: svg(`${strings}<path d="M5 20V4M2 7l3-3 3 3"/>`),
  strumUp: svg(`${strings}<path d="M5 4v16M2 17l3 3 3-3"/>`),
  arpeggioDown: svg(`${strings}<path d="M5 20l-1.8-2 3.6-3-3.6-3 3.6-3L5 7V4M2 7l3-3 3 3"/>`),
  arpeggioUp: svg(`${strings}<path d="M5 4l1.8 2-3.6 3 3.6 3-3.6 3L5 17v3M2 17l3 3 3-3"/>`),
  tremolo: svg('<path d="M12 3v18"/><path d="M7 10l10-3.5M7 14.5L17 11M7 19l10-3.5" stroke-width="2.2"/>'),
  whammy: svg('<path d="M2 7h5l5 11 5-11h5"/>'),
  slap: svg(`<circle cx="12" cy="12" r="9"/>${letters('S', 12, 16, 11)}`),
  pop: svg(`<circle cx="12" cy="12" r="9"/>${letters('P', 12, 16, 11)}`),

  // Bars and playback
  insertBar: svg('<path d="M4 4v16M20 4v16"/><path d="M12 8v8M8 12h8"/>'),
  removeBar: svg('<path d="M4 4v16M20 4v16"/><path d="M8 12h8"/>'),
  countIn: svg(dot(3.5, 12, 1.7) + dot(8.5, 12, 1.7) + dot(13.5, 12, 1.7) + `<path d="M17.5 7.5l5 4.5-5 4.5z" ${FILL}/>`),
} satisfies Record<string, IconDef>;

export type IconName = keyof typeof ICONS;
