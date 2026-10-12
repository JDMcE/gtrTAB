<script lang="ts">
  // The visual metronome: a light for each pulse of the bar, the one sounding lit, the
  // first of the bar larger. During the count-in the lights are hollow until they are lit.

  interface Props {
    /** Pulses in the bar. */
    count: number;
    /** The pulse sounding, from 0, or `null` when nothing is playing. */
    lit: number | null;
    countIn?: boolean;
  }

  let { count, lit, countIn = false }: Props = $props();
</script>

<span
  class="lights"
  class:count-in={countIn}
  role="img"
  aria-label="Visual metronome"
  data-pulse={lit === null ? '' : lit + 1}
  data-count-in={countIn}
>
  {#each { length: count } as _, i}
    <span class="light" class:first={i === 0} class:on={i === lit}></span>
  {/each}
</span>

<style>
  .lights {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    height: var(--control);
    padding: 0 var(--space-3);
  }

  .light {
    width: 10px;
    height: 10px;
    border: 1.5px solid var(--border-strong);
    border-radius: 50%;
  }

  .light.first {
    width: 14px;
    height: 14px;
  }

  .light.on {
    border-color: var(--accent);
    background: var(--accent);
    box-shadow: 0 0 8px var(--accent);
  }

  .count-in .light.on {
    border-color: var(--focus);
    background: var(--focus);
    box-shadow: 0 0 8px var(--focus);
  }
</style>
