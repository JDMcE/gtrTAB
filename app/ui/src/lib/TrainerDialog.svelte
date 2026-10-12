<script lang="ts">
  import { untrack } from 'svelte';
  import type { Trainer } from './trainer';
  import Dialog from './ui/Dialog.svelte';

  interface Props {
    trainer: Trainer;
    /** The speed it starts from, as a percentage of the tempo written. */
    speed: number;
    min: number;
    max: number;
    /** True while it is switched on, when it can also be switched off. */
    on: boolean;
    onapply: (trainer: Trainer | null) => void;
    onclose: () => void;
  }

  let { trainer, speed, min, max, on, onapply, onclose }: Props = $props();

  const initial = untrack(() => trainer);
  let step = $state<number | null>(initial.step);
  let every = $state<number | null>(initial.every);
  let target = $state<number | null>(initial.target);

  const clamp = (value: number | null, low: number, high: number, fallback: number) =>
    Math.max(low, Math.min(high, Math.round(value ?? fallback)));

  function submit() {
    onapply({
      step: clamp(step, 1, 100, initial.step),
      every: clamp(every, 1, 99, initial.every),
      target: clamp(target, min, max, initial.target),
    });
    onclose();
  }

  function turnOff() {
    onapply(null);
    onclose();
  }
</script>

<Dialog
  title="Speed trainer"
  hint="Loops the selected bars, or from the cursor to the end, starting at the speed now set ({speed}%) and getting faster as it goes round."
  {onclose}
  onsubmit={submit}
>
  <label>Faster by (%) <input type="number" min="1" max="100" bind:value={step} /></label>
  <label>After this many times round <input type="number" min="1" max="99" bind:value={every} /></label>
  <label>Up to (%) <input type="number" {min} {max} bind:value={target} /></label>
  {#snippet footer()}
    {#if on}<button type="button" class="btn" onclick={turnOff}>Turn off</button>{/if}
    <button type="button" class="btn" onclick={onclose}>Cancel</button>
    <button type="submit" class="btn primary">OK</button>
  {/snippet}
</Dialog>
