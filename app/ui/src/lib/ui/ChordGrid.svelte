<script lang="ts" module>
  /** Frets a diagram shows. */
  export const GRID_FRETS = 5;

  /**
   * The fret a diagram's top line stands for: 1 for a chord near the nut, otherwise the
   * lowest fret that is stopped. Mirrors `ChordDiagram::base_fret` in the core.
   */
  export function baseFret(frets: number[]): number {
    const stopped = frets.filter((f) => f > 0);
    return stopped.length && Math.max(...stopped) > GRID_FRETS ? Math.min(...stopped) : 1;
  }
</script>

<script lang="ts">
  interface Props {
    /** Fret of each string from the highest to the lowest; 0 is open and -1 not played. */
    frets: number[];
    /** Width in pixels of the space between two strings. */
    scale?: number;
    /** Called with a string's index and the fret clicked; 0 for the area above the nut. */
    onpick?: (string: number, fret: number) => void;
  }

  let { frets, scale = 10, onpick }: Props = $props();

  const base = $derived(baseFret(frets));
  const strings = $derived(frets.length);
  const gap = $derived(scale);
  const row = $derived(scale * 1.25);
  const width = $derived((Math.max(strings, 2) - 1) * gap);
  const height = $derived(GRID_FRETS * row);
  /** Strings are drawn lowest on the left, the reverse of how the frets are listed. */
  const column = (string: number) => (strings - 1 - string) * gap;

  function pick(event: MouseEvent) {
    if (!onpick) return;
    const box = (event.currentTarget as SVGElement).getBoundingClientRect();
    const x = event.clientX - box.left - gap * 1.5;
    const y = event.clientY - box.top - row * 1.6;
    const nearest = Math.max(0, Math.min(strings - 1, Math.round(x / gap)));
    const fret = y < 0 ? 0 : Math.min(GRID_FRETS, Math.floor(y / row) + 1) + base - 1;
    onpick(strings - 1 - nearest, fret);
  }
</script>

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<svg
  class:editable={!!onpick}
  width={width + gap * 4.2}
  height={height + row * 2.1}
  aria-hidden="true"
  onclick={pick}
>
  <g transform="translate({gap * 1.5} {row * 1.6})">
    {#each { length: strings } as _, s}
      <line x1={s * gap} y1="0" x2={s * gap} y2={height} />
    {/each}
    {#each { length: GRID_FRETS + 1 } as _, f}
      <line x1="0" y1={f * row} x2={width} y2={f * row} />
    {/each}
    {#if base === 1}
      <line class="nut" x1="0" y1="-1" x2={width} y2="-1" />
    {:else}
      <text class="base" x={width + gap * 0.5} y={row * 0.75}>{base}fr</text>
    {/if}
    {#each frets as fret, s}
      {#if fret < 0}
        <text x={column(s)} y={-row * 0.4}>×</text>
      {:else if fret === 0}
        <circle class="open" cx={column(s)} cy={-row * 0.62} r={gap * 0.26} />
      {:else}
        <circle
          cx={column(s)}
          cy={(Math.max(0, Math.min(GRID_FRETS - 1, fret - base)) + 0.5) * row}
          r={gap * 0.36}
        />
      {/if}
    {/each}
  </g>
</svg>

<style>
  svg {
    display: block;
    flex: none;
  }

  svg.editable {
    cursor: pointer;
  }

  line {
    stroke: currentColor;
    stroke-width: 1;
  }

  .nut {
    stroke-width: 3;
  }

  circle {
    fill: currentColor;
  }

  .open {
    fill: none;
    stroke: currentColor;
    stroke-width: 1;
  }

  text {
    fill: currentColor;
    font-size: 11px;
    text-anchor: middle;
  }

  .base {
    font-size: 10px;
    text-anchor: start;
  }
</style>
