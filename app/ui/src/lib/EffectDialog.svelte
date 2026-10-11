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
  import Dialog from './ui/Dialog.svelte';

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
  const PAD = { left: 34, top: 12 };
  /** How close, in pixels, a press must be to a point to pick it up. */
  const GRAB = 10;
  const HEIGHTS = [1, 2, 4, 6, 8];
  const columns = LENGTH / STEP;
  const rows = high - low;
  const xOf = (position: number) => PAD.left + (position / STEP) * CELL_W;
  const yOf = (value: number) => PAD.top + (high - value) * CELL_H;
  const clamp = (n: number, min: number, max: number) => Math.max(min, Math.min(max, n));

  let grid = $state<SVGSVGElement>();
  /** Index of the point being dragged. */
  let dragging = $state<number | null>(null);

  /** The furthest the curve gets from the note's own pitch. */
  const peak = $derived(curve.reduce((m, p) => (Math.abs(p.value) > Math.abs(m) ? p.value : m), 0));
  const flat = $derived(peak === 0);

  /** Moves a point, keeping it between its neighbours so the curve stays in order. */
  function movePoint(index: number, position: number, value: number) {
    const min = index === 0 ? 0 : curve[index - 1].position + STEP;
    const max = index === curve.length - 1 ? LENGTH : curve[index + 1].position - STEP;
    const next = { position: clamp(position, min, max), value: clamp(value, low, high) };
    curve = curve.map((p, i) => (i === index ? next : p));
  }

  function removePoint(index: number) {
    if (curve.length > 1) curve = curve.filter((_, i) => i !== index);
  }

  /** The grid position under the pointer. */
  function cellAt(event: PointerEvent) {
    const bounds = grid!.getBoundingClientRect();
    const x = event.clientX - bounds.left;
    const y = event.clientY - bounds.top;
    return {
      x,
      y,
      position: clamp(Math.round((x - PAD.left) / CELL_W), 0, columns) * STEP,
      value: high - clamp(Math.round((y - PAD.top) / CELL_H), 0, rows),
    };
  }

  /** Picks up the point under the pointer, or adds one there and picks that up. */
  function onPointerDown(event: PointerEvent) {
    if (event.button !== 0) return;
    const at = cellAt(event);
    let index = curve.findIndex(
      (p) => Math.hypot(xOf(p.position) - at.x, yOf(p.value) - at.y) <= GRAB,
    );
    if (index < 0) {
      index = curve.findIndex((p) => p.position === at.position);
      if (index < 0) {
        curve = [...curve, { position: at.position, value: at.value }].sort(
          (a, b) => a.position - b.position,
        );
        index = curve.findIndex((p) => p.position === at.position);
      } else {
        movePoint(index, at.position, at.value);
      }
    }
    dragging = index;
    grid!.setPointerCapture(event.pointerId);
    // Keep the keyboard on the same point as the mouse.
    grid!.querySelectorAll<SVGElement>('.point')[index]?.focus({ preventScroll: true });
    event.preventDefault();
  }

  function onPointerMove(event: PointerEvent) {
    if (dragging === null) return;
    const at = cellAt(event);
    movePoint(dragging, at.position, at.value);
  }

  function onPointKeydown(event: KeyboardEvent, index: number) {
    const p = curve[index];
    switch (event.key) {
      case 'ArrowLeft': movePoint(index, p.position - STEP, p.value); break;
      case 'ArrowRight': movePoint(index, p.position + STEP, p.value); break;
      case 'ArrowUp': movePoint(index, p.position, p.value + 1); break;
      case 'ArrowDown': movePoint(index, p.position, p.value - 1); break;
      case 'Delete':
      case 'Backspace': removePoint(index); break;
      default: return;
    }
    event.preventDefault();
  }

  /** Stretches the curve so that its highest point is `height` quarter tones. */
  function setHeight(height: number) {
    if (peak === 0) return;
    curve = curve.map((p) => ({ ...p, value: Math.round((p.value * height) / peak) }));
  }

  // --- Trill and grace note ---
  let trillFret = $state(initial.trill?.fret ?? initial.fret + 2);
  let trillSpeed = $state(initial.trill?.speed ?? 'sixteenth');
  let graceFret = $state(initial.grace?.fret ?? Math.max(0, initial.fret - 2));
  let graceOnBeat = $state(initial.grace?.on_beat ?? false);
  let graceHammer = $state(initial.grace?.hammer ?? true);

  const CURVE_HINT =
    'Pitch over the length of the note. Drag a point to move it, click the grid to add one, ' +
    'and double-click a point to remove it.';
  const TITLES = { bend: 'Bend', whammy: 'Whammy bar', trill: 'Trill', grace: 'Grace note' };
  const HINTS = {
    bend: CURVE_HINT,
    whammy: CURVE_HINT,
    trill: 'Alternates quickly between the note and another fret on the same string.',
    grace: 'A quick note on the same string just before this one.',
  };

  function submit() {
    if (isCurve) onapply({ points: curve });
    else if (kind === 'trill') onapply({ fret: trillFret, speed: trillSpeed });
    else onapply({ fret: graceFret, on_beat: graceOnBeat, hammer: graceHammer });
  }
</script>

<Dialog
  title={TITLES[mode]}
  hint={HINTS[mode]}
  width={isCurve ? '380px' : '340px'}
  {onclose}
  onsubmit={submit}
>
  {#if isCurve}
    <div class="presets">
      {#each presets as [name, preset]}
        <button type="button" class="btn" onclick={() => (curve = preset.map((p) => ({ ...p })))}>
          {name}
        </button>
      {/each}
    </div>
    {#if mode === 'bend'}
      <div class="heights" role="group" aria-label="Bend height">
        <span>Height</span>
        {#each HEIGHTS as height}
          <button
            type="button"
            class="btn"
            aria-pressed={peak === height}
            onclick={() => setHeight(height)}
          >
            {intervalLabel(height)}
          </button>
        {/each}
      </div>
    {/if}
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <svg
      bind:this={grid}
      class:dragging={dragging !== null}
      width={PAD.left + columns * CELL_W + 12}
      height={PAD.top + rows * CELL_H + 12}
      onpointerdown={onPointerDown}
      onpointermove={onPointerMove}
      onpointerup={() => (dragging = null)}
      onpointercancel={() => (dragging = null)}
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
      {#each curve as p, i}
        <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
        <circle
          class="point"
          class:held={dragging === i}
          cx={xOf(p.position)}
          cy={yOf(p.value)}
          r="5"
          tabindex="0"
          role="slider"
          aria-label="Point {i + 1} of {curve.length}"
          aria-orientation="vertical"
          aria-valuemin={low}
          aria-valuemax={high}
          aria-valuenow={p.value}
          aria-valuetext="{intervalLabel(p.value)} at {Math.round((p.position / LENGTH) * 100)}% of the note"
          onkeydown={(e) => onPointKeydown(e, i)}
          ondblclick={() => removePoint(i)}
        />
      {/each}
    </svg>
    <p class="keys">With the keyboard: Tab to a point, arrow keys to move it, Delete to remove it.</p>
  {:else if kind === 'trill'}
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
  {#snippet footer()}
    {#if existed}
      <button type="button" class="btn remove" onclick={() => onapply(null)}>Remove</button>
    {/if}
    <button type="button" class="btn" onclick={onclose}>Cancel</button>
    <button type="submit" class="btn primary" disabled={isCurve && flat}>OK</button>
  {/snippet}
</Dialog>

<style>
  .presets,
  .heights {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-2);
    margin-bottom: var(--space-3);
  }

  .presets .btn,
  .heights .btn {
    height: 26px;
    padding: 0 var(--space-3);
    font-size: var(--text-sm);
  }

  .heights span {
    margin-right: var(--space-2);
    color: var(--muted);
    font-size: var(--text-sm);
  }

  .heights .btn {
    min-width: 34px;
  }

  .btn[aria-pressed='true'] {
    border-color: var(--accent);
    background: var(--pressed);
    color: var(--accent);
  }

  svg {
    display: block;
    margin: 0 auto;
    cursor: crosshair;
    user-select: none;
    touch-action: none;
  }

  svg.dragging {
    cursor: grabbing;
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
    pointer-events: none;
  }

  .point {
    fill: var(--accent);
    stroke: var(--panel);
    stroke-width: 1.5;
    cursor: grab;
    outline: none;
  }

  .point.held {
    cursor: grabbing;
  }

  .point:focus-visible,
  .point.held {
    stroke: var(--focus);
    stroke-width: 2.5;
  }

  .keys {
    margin: var(--space-3) 0 0;
    color: var(--muted);
    font-size: var(--text-sm);
  }
</style>
