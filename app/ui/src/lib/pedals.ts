// What the pedalboard works with: the kinds of pedal and their knobs as the core lists
// them, changes to a chain, and the presets the user has saved.

import type { Pedal, PedalKind } from './model';

/** One knob of a kind of pedal: its name and the setting a new pedal has. */
export interface KnobSpec {
  name: string;
  default: number;
}

/** A kind of pedal, as `Document.pedalKindsJson` lists them. */
export interface PedalSpec {
  kind: PedalKind;
  name: string;
  knobs: KnobSpec[];
}

/** A chain ready to put on a track or, if `master`, on the whole mix. */
export interface PedalPreset {
  name: string;
  master: boolean;
  pedals: Pedal[];
}

/** The most pedals a chain can have, as in `gtr_model::Pedal::MAX_CHAIN`. */
export const MAX_CHAIN = 16;
export const MAX_KNOB = 100;

/** The setting of a pedal's knob `index`: its own, or the default if it has none. */
export function knobValue(pedal: Pedal, spec: PedalSpec, index: number): number {
  return pedal.knobs[index] ?? spec.knobs[index]?.default ?? 0;
}

/** The pedal with knob `index` set to `value`, every knob before it written out. */
export function withKnob(pedal: Pedal, spec: PedalSpec, index: number, value: number): Pedal {
  const knobs = spec.knobs.map((_, i) => knobValue(pedal, spec, i));
  knobs[index] = Math.max(0, Math.min(MAX_KNOB, Math.round(value)));
  return { ...pedal, knobs };
}

/** A new pedal of the kind, switched on with every knob at its default. */
export function newPedal(spec: PedalSpec): Pedal {
  return { kind: spec.kind, knobs: spec.knobs.map((k) => k.default) };
}

/**
 * The chain with a new pedal of the kind added where pedals of its kind usually go: the
 * kinds are listed in the order a guitarist would put them, from the compressor at the
 * guitar's end to the reverb at the amplifier's, so a delay added to a chain that ends
 * with a reverb goes before it. Returns the chain and where the pedal went.
 */
export function addPedal(chain: Pedal[], spec: PedalSpec, specs: PedalSpec[]): [Pedal[], number] {
  const order = (kind: PedalKind) => specs.findIndex((s) => s.kind === kind);
  let at = chain.length;
  while (at > 0 && order(chain[at - 1].kind) > order(spec.kind)) at--;
  return [[...chain.slice(0, at), newPedal(spec), ...chain.slice(at)], at];
}

/** The chain with the pedal at `from` moved to `to`. */
export function movePedal(chain: Pedal[], from: number, to: number): Pedal[] {
  if (to < 0 || to >= chain.length || from === to) return chain;
  const moved = [...chain];
  const [pedal] = moved.splice(from, 1);
  moved.splice(to, 0, pedal);
  return moved;
}

/** True when two chains would sound the same: the same pedals with the same settings. */
export function sameChain(a: Pedal[], b: Pedal[], specs: PedalSpec[]): boolean {
  const full = (pedal: Pedal) => {
    const spec = specs.find((s) => s.kind === pedal.kind);
    const knobs = spec ? spec.knobs.map((_, i) => knobValue(pedal, spec, i)) : pedal.knobs;
    return JSON.stringify([pedal.kind, pedal.on !== false, knobs]);
  };
  return a.length === b.length && a.every((pedal, i) => full(pedal) === full(b[i]));
}

const PRESETS_KEY = 'gtrtab.pedalPresets';

/** The presets the user has saved, for tracks and for the mix. */
export function loadUserPresets(): PedalPreset[] {
  try {
    const saved = JSON.parse(localStorage.getItem(PRESETS_KEY) ?? '[]');
    return Array.isArray(saved) ? saved : [];
  } catch {
    return [];
  }
}

/** Keeps the user's presets for next time. */
export function saveUserPresets(presets: PedalPreset[]): void {
  try {
    localStorage.setItem(PRESETS_KEY, JSON.stringify(presets));
  } catch {
    // Storage may be unavailable; the presets then last for the session.
  }
}

/** The user's presets with one saved under its name, in place of any of that name. */
export function withPreset(presets: PedalPreset[], preset: PedalPreset): PedalPreset[] {
  const others = presets.filter((p) => !(p.name === preset.name && p.master === preset.master));
  return [...others, preset].sort((a, b) => a.name.localeCompare(b.name));
}
