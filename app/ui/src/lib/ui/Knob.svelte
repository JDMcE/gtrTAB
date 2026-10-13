<script lang="ts">
  // A rotary knob from 0 to 100, as on a pedal. Dragged up or right it goes up; the arrow
  // keys move it a step, Page Up and Page Down ten, and a double click puts it back to
  // where it starts. `oninput` follows it as it moves, and `onchange` says where it was
  // left, once the drag or the key is let go.

  interface Props {
    label: string;
    value: number;
    /** Where a double click puts it. */
    reset: number;
    oninput: (value: number) => void;
    onchange: (value: number) => void;
  }

  let { label, value, reset, oninput, onchange }: Props = $props();

  /** Degrees the knob turns through from 0 to 100, centred on straight up. */
  const SWEEP = 270;
  /** Pixels of drag from one end to the other. */
  const TRAVEL = 160;

  const clamp = (v: number) => Math.max(0, Math.min(100, Math.round(v)));
  const angle = $derived(-SWEEP / 2 + (SWEEP * value) / 100);

  /** A point on the knob's circle, `degrees` clockwise from straight up. */
  function at(degrees: number, radius: number): string {
    const radians = (degrees * Math.PI) / 180;
    return `${(20 + radius * Math.sin(radians)).toFixed(2)} ${(20 - radius * Math.cos(radians)).toFixed(2)}`;
  }
  /** The arc from the bottom left round to `degrees`. */
  const arc = (degrees: number) =>
    `M ${at(-SWEEP / 2, 15)} A 15 15 0 ${degrees + SWEEP / 2 > 180 ? 1 : 0} 1 ${at(degrees, 15)}`;

  let drag: { x: number; y: number; from: number; moved: boolean } | null = null;

  function down(event: PointerEvent) {
    if (event.button !== 0) return;
    (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
    (event.currentTarget as HTMLElement).focus();
    drag = { x: event.clientX, y: event.clientY, from: value, moved: false };
    event.preventDefault();
  }

  function move(event: PointerEvent) {
    if (!drag) return;
    const by = (drag.y - event.clientY + (event.clientX - drag.x)) * (100 / TRAVEL);
    const next = clamp(drag.from + by);
    if (next !== value) {
      drag.moved = true;
      oninput(next);
    }
  }

  function up() {
    if (drag?.moved) onchange(value);
    drag = null;
  }

  /** Keys that move the knob, and how far. */
  function step(key: string, shift: boolean): number | null {
    const small = shift ? 10 : 1;
    switch (key) {
      case 'ArrowUp':
      case 'ArrowRight':
        return value + small;
      case 'ArrowDown':
      case 'ArrowLeft':
        return value - small;
      case 'PageUp':
        return value + 10;
      case 'PageDown':
        return value - 10;
      case 'Home':
        return 0;
      case 'End':
        return 100;
    }
    return null;
  }

  let keyed = false;

  function keydown(event: KeyboardEvent) {
    const next = step(event.key, event.shiftKey);
    if (next === null) return;
    event.preventDefault();
    event.stopPropagation();
    keyed = true;
    if (clamp(next) !== value) oninput(clamp(next));
  }

  // A key held down repeats; what it reached is kept once it is let go.
  function keyup(event: KeyboardEvent) {
    if (!keyed || step(event.key, event.shiftKey) === null) return;
    keyed = false;
    onchange(value);
  }

  function blur() {
    if (keyed) onchange(value);
    keyed = false;
  }

  function dblclick() {
    if (value === reset) return;
    oninput(reset);
    onchange(reset);
  }
</script>

<div class="knob">
  <div
    class="dial"
    role="slider"
    tabindex="0"
    aria-label={label}
    aria-valuemin={0}
    aria-valuemax={100}
    aria-valuenow={value}
    onpointerdown={down}
    onpointermove={move}
    onpointerup={up}
    onpointercancel={up}
    onkeydown={keydown}
    onkeyup={keyup}
    onblur={blur}
    ondblclick={dblclick}
  >
    <svg viewBox="0 0 40 40" aria-hidden="true">
      <path class="track" d={arc(SWEEP / 2)} />
      {#if value > 0}<path class="level" d={arc(angle)} />{/if}
      <circle class="cap" cx="20" cy="20" r="10.5" />
      <path class="pointer" d="M {at(angle, 4)} L {at(angle, 9.5)}" />
    </svg>
  </div>
  <span class="name">{label}</span>
  <span class="value">{value}</span>
</div>

<style>
  .knob {
    display: grid;
    justify-items: center;
    gap: var(--space-1);
    width: 52px;
  }

  .dial {
    width: 42px;
    height: 42px;
    border-radius: 50%;
    cursor: ns-resize;
    touch-action: none;
  }

  .dial:focus-visible {
    outline: 2px solid var(--focus);
    outline-offset: 1px;
  }

  svg {
    display: block;
    width: 100%;
    height: 100%;
  }

  path {
    fill: none;
    stroke-linecap: round;
  }

  .track {
    stroke: var(--border);
    stroke-width: 3;
  }

  .level {
    stroke: var(--accent);
    stroke-width: 3;
  }

  .cap {
    fill: var(--raised);
    stroke: var(--border-strong);
    stroke-width: 1;
  }

  .pointer {
    stroke: var(--text);
    stroke-width: 2;
  }

  .name {
    font-size: var(--text-xs);
    white-space: nowrap;
  }

  .value {
    color: var(--muted);
    font-size: var(--text-xs);
    font-variant-numeric: tabular-nums;
  }
</style>
