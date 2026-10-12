<script lang="ts">
  // The whole neck as a diagram, with the notes of a scale or a chord marked on it and its
  // root set apart. A picture only: what it shows is said in words beside it.
  import { FRETS, INLAYS } from '../instrument';

  interface Props {
    /** Pitches of the open strings, with the capo on, from the highest string. */
    open: number[];
    /** Pitch classes marked. */
    classes: number[];
    root: number;
    /** What a mark says, by pitch class. */
    label: (pitchClass: number) => string;
    frets?: number;
  }

  let { open, classes, root, label, frets = FRETS }: Props = $props();

  const CELL = 26;
  const ROW = 20;
  const LEFT = 16;
  const TOP = 12;
  const width = $derived(LEFT + (frets + 1) * CELL + 4);
  const height = $derived(TOP + open.length * ROW + 18);
  /** The middle of a fret's space, or for the open string left of the nut. */
  const x = (fret: number) => LEFT + fret * CELL + CELL / 2;
  const y = (string: number) => TOP + string * ROW + ROW / 2;
  const nut = LEFT + CELL;
  const pc = (pitch: number) => ((pitch % 12) + 12) % 12;
</script>

<svg {width} {height} aria-hidden="true">
  {#each { length: frets + 1 } as _, fret}
    <line
      class:nut={fret === 0}
      x1={LEFT + (fret + 1) * CELL}
      y1={y(0)}
      x2={LEFT + (fret + 1) * CELL}
      y2={y(open.length - 1)}
    />
    <text class="number" class:inlay={INLAYS.includes(fret)} x={x(fret)} y={height - 4}>{fret}</text>
  {/each}
  {#each open as _, string}
    <line class="string" x1={nut} y1={y(string)} x2={width - 4} y2={y(string)} />
  {/each}
  {#each open as pitch, string}
    {#each { length: frets + 1 } as _, fret}
      {@const at = pc(pitch + fret)}
      {#if classes.includes(at)}
        <circle class:root={at === root} cx={x(fret)} cy={y(string)} r="8.5" />
        <text class="mark" class:root={at === root} x={x(fret)} y={y(string) + 3.5}>{label(at)}</text>
      {/if}
    {/each}
  {/each}
</svg>

<style>
  svg {
    display: block;
    flex: none;
  }

  line {
    stroke: var(--staff);
    stroke-width: 1.5;
  }

  .nut {
    stroke: var(--ink);
    stroke-width: 4;
  }

  .string {
    stroke: var(--ink);
    stroke-width: 1;
  }

  circle {
    fill: var(--raised);
    stroke: var(--focus);
    stroke-width: 1.5;
  }

  circle.root {
    fill: var(--focus);
  }

  text {
    fill: var(--text);
    font-size: 9px;
    font-weight: 600;
    text-anchor: middle;
  }

  .mark.root {
    fill: var(--panel);
  }

  .number {
    fill: var(--muted);
    font-weight: 400;
  }

  .number.inlay {
    fill: var(--text);
    font-weight: 700;
  }
</style>
