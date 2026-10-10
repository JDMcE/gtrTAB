<script lang="ts" module>
  import type { Bend, BendKind, CurvePoint } from './model';

  export type EffectKind = 'bend' | 'whammy' | 'trill' | 'grace';

  /** Position of the end of the note on a curve; matches `Bend::LENGTH` in the core. */
  const LENGTH = 60;
  const STEP = 5;

  const points = (...pairs: [number, number][]): CurvePoint[] =>
    pairs.map(([position, value]) => ({ position, value }));

  /** The common bend shapes; matches `Bend::preset` in the core. */
  export function bendPreset(kind: BendKind, height: number): Bend {
    switch (kind) {
      case 'bend':
        return { points: points([0, 0], [15, height], [60, height]) };
      case 'bend_release':
        return { points: points([0, 0], [15, height], [30, height], [45, 0], [60, 0]) };
      case 'prebend':
        return { points: points([0, height], [60, height]) };
      case 'prebend_release':
        return { points: points([0, height], [20, height], [40, 0], [60, 0]) };
    }
  }

  /** An interval in quarter tones the way guitarists write it. */
  export function intervalLabel(quarterTones: number): string {
    const size = Math.abs(quarterTones);
    const fraction = ['', '¼', '½', '¾'][size % 4];
    const whole = Math.floor(size / 4);
    const sign = quarterTones < 0 ? '−' : '';
    if (whole === 1 && fraction === '' && quarterTones > 0) return 'full';
    return sign + (whole === 0 ? fraction || '0' : `${whole}${fraction}`);
  }

  const WHAMMY_PRESETS: [string, CurvePoint[]][] = [
    ['Dip', points([0, 0], [10, -4], [20, 0], [60, 0])],
    ['Dive', points([0, 0], [60, -8])],
    ['Dive and return', points([0, 0], [25, -8], [50, 0], [60, 0])],
    ['Pull up', points([0, 0], [20, 4], [60, 4])],
    ['Release down', points([0, 4], [30, 0], [60, 0])],
  ];
  const BEND_PRESETS: [string, CurvePoint[]][] = [
    ['Bend', bendPreset('bend', 4).points],
    ['Bend and release', bendPreset('bend_release', 4).points],
    ['Pre-bend', bendPreset('prebend', 4).points],
    ['Pre-bend and release', bendPreset('prebend_release', 4).points],
  ];
</script>

<script lang="ts">
  import { untrack } from 'svelte';
  import type { BeatEffects, Note } from './model';

  interface Props {
    kind: EffectKind;
    /** The note under the cursor. */
    note: Note | null;
    beatEffects: BeatEffects | null;
    /** Called with the effect's new value, or `null` to remove it. */
    onapply: (value: unknown) => void;
    onclose: () => void;
  }

  let { kind, note, beatEffects, onapply, onclose }: Props = $props();

  // The dialog is created for one effect and edits a copy of the values it opened with.
  const mode = untrack(() => kind);
  const initial = untrack(() => ({
    curve: (kind === 'whammy' ? beatEffects?.whammy : note?.effects?.bend)?.points,
    trill: note?.effects?.trill,
    grace: note?.effects?.grace,
    fret: note?.fret ?? 0,
  }));
  const isCurve = mode === 'bend' || mode === 'whammy';
  const existed =
    mode === 'trill' ? !!initial.trill
    : mode === 'grace' ? !!initial.grace
    : !!initial.curve;

  // --- Curve editor ---
  const [low, high] = mode === 'whammy' ? [-12, 8] : [0, 12];
  const presets = mode === 'whammy' ? WHAMMY_PRESETS : BEND_PRESETS;
  let curve = $state<CurvePoint[]>(
    (initial.curve ?? presets[0][1]).map((p) => ({
      position: p.position,
      // Imported curves can go beyond what the grid shows.
      value: Math.max(low, Math.min(high, p.value)),
    })),
  );

  const CELL_W = 22;
  const CELL_H = mode === 'whammy' ? 11 : 15;
  const PAD = { left: 34, top: 10 };
  const columns = LENGTH / STEP;
  const rows = high - low;
  const xOf = (position: number) => PAD.left + (position / STEP) * CELL_W;
  const yOf = (value: number) => PAD.top + (high - value) * CELL_H;

  /** Adds a point where the grid was clicked, moves the one in that column, or removes it. */
  function onGridClick(event: MouseEvent) {
    const bounds = (event.currentTarget as SVGElement).getBoundingClientRect();
    const column = Math.round((event.clientX - bounds.left - PAD.left) / CELL_W);
    const row = Math.round((event.clientY - bounds.top - PAD.top) / CELL_H);
    if (column < 0 || column > columns || row < 0 || row > rows) return;
    const position = column * STEP;
    const value = high - row;
    const at = curve.findIndex((p) => p.position === position);
    if (at >= 0 && curve[at].value === value) {
      if (curve.length > 1) curve = curve.filter((_, i) => i !== at);
    } else if (at >= 0) {
      curve = curve.map((p, i) => (i === at ? { position, value } : p));
    } else {
      curve = [...curve, { position, value }].sort((a, b) => a.position - b.position);
    }
  }

  const flat = $derived(curve.every((p) => p.value === 0));

  // --- Trill and grace note ---
  let trillFret = $state(initial.trill?.fret ?? initial.fret + 2);
  let trillSpeed = $state(initial.trill?.speed ?? 'sixteenth');
  let graceFret = $state(initial.grace?.fret ?? Math.max(0, initial.fret - 2));
  let graceOnBeat = $state(initial.grace?.on_beat ?? false);
  let graceHammer = $state(initial.grace?.hammer ?? true);

  let dialog: HTMLDialogElement;
  $effect(() => dialog.showModal());

  function submit(event: SubmitEvent) {
    event.preventDefault();
    if (isCurve) onapply({ points: curve });
    else if (kind === 'trill') onapply({ fret: trillFret, speed: trillSpeed });
    else onapply({ fret: graceFret, on_beat: graceOnBeat, hammer: graceHammer });
  }
</script>

<dialog bind:this={dialog} {onclose}>
  <form onsubmit={submit}>
    {#if isCurve}
      <h2>{kind === 'bend' ? 'Bend' : 'Whammy bar'}</h2>
      <p>
        Pitch over the length of the note. Click the grid to add or move a point; click a point
        to remove it.
      </p>
      <div class="presets">
        {#each presets as [name, preset]}
          <button type="button" onclick={() => (curve = preset.map((p) => ({ ...p })))}>
            {name}
          </button>
        {/each}
      </div>
      <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_noninteractive_element_interactions -->
      <svg
        width={PAD.left + columns * CELL_W + 12}
        height={PAD.top + rows * CELL_H + 12}
        onclick={onGridClick}
        role="img"
        aria-label="Pitch curve"
      >
        {#each { length: rows + 1 } as _, row}
          {@const value = high - row}
          <line
            class:tone={value % 4 === 0}
            class:zero={value === 0}
            x1={PAD.left}
            x2={xOf(LENGTH)}
            y1={yOf(value)}
            y2={yOf(value)}
          />
          {#if value % 4 === 0}
            <text x={PAD.left - 6} y={yOf(value) + 3} text-anchor="end">
              {value === 0 ? '0' : intervalLabel(value)}
            </text>
          {/if}
        {/each}
        {#each { length: columns + 1 } as _, column}
          <line
            class:tone={column % 3 === 0}
            x1={xOf(column * STEP)}
            x2={xOf(column * STEP)}
            y1={yOf(high)}
            y2={yOf(low)}
          />
        {/each}
        <polyline class="curve" points={curve.map((p) => `${xOf(p.position)},${yOf(p.value)}`).join(' ')} />
        {#each curve as p}
          <circle cx={xOf(p.position)} cy={yOf(p.value)} r="4" />
        {/each}
      </svg>
    {:else if kind === 'trill'}
      <h2>Trill</h2>
      <p>Alternates quickly between the note and another fret on the same string.</p>
      <label>
        Trill to fret
        <input type="number" min="0" max="36" required bind:value={trillFret} />
      </label>
      <label>
        Speed
        <select bind:value={trillSpeed}>
          <option value="sixteenth">16th notes</option>
          <option value="thirty_second">32nd notes</option>
          <option value="sixty_fourth">64th notes</option>
        </select>
      </label>
    {:else}
      <h2>Grace note</h2>
      <p>A quick note on the same string just before this one.</p>
      <label>
        Fret
        <input type="number" min="0" max="36" required bind:value={graceFret} />
      </label>
      <label class="check">
        <input type="checkbox" bind:checked={graceHammer} />
        Hammer on or pull off into the main note
      </label>
      <label class="check">
        <input type="checkbox" bind:checked={graceOnBeat} />
        On the beat, taking time from the main note
      </label>
    {/if}
    <footer>
      {#if existed}
        <button type="button" class="remove" onclick={() => onapply(null)}>Remove</button>
      {/if}
      <button type="button" onclick={onclose}>Cancel</button>
      <button type="submit" class="primary" disabled={isCurve && flat}>OK</button>
    </footer>
  </form>
</dialog>

<style>
  dialog {
    padding: 20px;
    border: 1px solid var(--border);
    border-radius: 8px;
    background: var(--panel);
    color: var(--text);
    max-width: 92vw;
  }

  dialog::backdrop {
    background: rgb(0 0 0 / 40%);
  }

  h2 {
    margin: 0 0 10px;
    font-size: 16px;
  }

  p {
    max-width: 340px;
    margin: 0 0 12px;
    color: var(--muted);
  }

  .presets {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin-bottom: 10px;
  }

  svg {
    display: block;
    cursor: crosshair;
    user-select: none;
  }

  svg line {
    stroke: var(--border);
    stroke-width: 1;
    opacity: 0.6;
  }

  svg line.tone {
    opacity: 1;
  }

  svg line.zero {
    stroke: var(--muted);
  }

  svg text {
    fill: var(--muted);
    font-size: 10px;
  }

  .curve {
    fill: none;
    stroke: var(--accent);
    stroke-width: 2;
  }

  circle {
    fill: var(--accent);
  }

  label {
    display: grid;
    gap: 4px;
    margin-bottom: 12px;
    color: var(--muted);
    font-size: 12px;
  }

  label.check {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 13px;
    color: var(--text);
  }

  input[type='number'],
  select {
    font: inherit;
    font-size: 14px;
    padding: 5px 8px;
    border: 1px solid var(--border);
    border-radius: 4px;
    background: var(--bg);
    color: var(--text);
  }

  footer {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 14px;
  }

  button {
    font: inherit;
    padding: 5px 12px;
    border: 1px solid var(--border);
    border-radius: 4px;
    background: var(--bg);
    color: var(--text);
    cursor: pointer;
  }

  button.remove {
    margin-right: auto;
  }

  button.primary {
    border-color: var(--accent);
    background: var(--accent);
    color: var(--paper);
  }

  button:disabled {
    opacity: 0.45;
    cursor: default;
  }
</style>
