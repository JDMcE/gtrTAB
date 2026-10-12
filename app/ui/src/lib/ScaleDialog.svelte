<script lang="ts">
  // The scale library: any scale on any root, its notes spelled and counted from the root,
  // and drawn over the whole neck in the track's tuning. Opened by the scale finder, it
  // first lists the scales that fit the notes it was given.
  import { untrack } from 'svelte';
  import { ROOTS } from './chords';
  import type { Marks } from './InstrumentView.svelte';
  import { SCALES, ascending, degrees, scaleClasses, spell, type Fit } from './scales';
  import Dialog from './ui/Dialog.svelte';
  import NeckDiagram from './ui/NeckDiagram.svelte';

  interface Props {
    /** Pitches of the open strings with the capo on, from the highest string. */
    open: number[];
    /** Said under the neck when it is not the track's own, as for a piano. */
    neckNote?: string;
    root: number;
    scale: number;
    /** Scales found to fit some notes, the best first, and what the notes were. */
    found?: { fits: Fit[]; source: string } | null;
    onshow: (marks: Marks) => void;
    onplay: (pitches: number[]) => void;
    onclose: () => void;
  }

  let { open, neckNote, root: firstRoot, scale: firstScale, found = null, onshow, onplay, onclose }: Props = $props();

  let root = $state(untrack(() => firstRoot));
  let index = $state(untrack(() => firstScale));
  let labels = $state<'names' | 'degrees'>('names');

  const scale = $derived(SCALES[index]);
  const classes = $derived(scaleClasses(root, scale));
  const names = $derived(spell(root, scale));
  const steps = $derived(degrees(scale));
  const title = $derived(`${names[0]} ${scale.name.toLowerCase()}`);
  const label = (pitchClass: number) => {
    const at = classes.indexOf(pitchClass);
    return labels === 'names' ? names[at] : steps[at];
  };

  /** What a scale found makes of the notes, in words. */
  function fitWords(fit: Fit): string {
    const outside = fit.outside.length;
    const holds = outside ? `all but ${outside} of the notes (${fit.outside.map((c) => ROOTS[c]).join(', ')})` : 'every note';
    const unused = fit.unused ? `, ${fit.unused} of its notes not played` : '';
    return `${holds}${unused}`;
  }

  function choose(fit: Fit) {
    root = fit.root;
    index = SCALES.indexOf(fit.scale);
  }

  /** Lowest root of the scale on the lowest string, for hearing it. */
  const low = $derived(Math.min(...open) + ((root - Math.min(...open)) % 12 + 12) % 12);
</script>

<Dialog title="Scale library" width="760px" {onclose}>
  {#if found}
    <fieldset class="found">
      <legend>Scales that fit the notes of {found.source}</legend>
      {#if found.fits.length === 0}
        <p class="hint">There are no notes there.</p>
      {/if}
      {#each found.fits as fit, i (i)}
        <label class="check">
          <input
            type="radio"
            name="fit"
            checked={fit.root === root && fit.scale === scale}
            onchange={() => choose(fit)}
          />
          {spell(fit.root, fit.scale)[0]} {fit.scale.name.toLowerCase()}: <span class="muted">{fitWords(fit)}</span>
        </label>
      {/each}
    </fieldset>
  {/if}

  <div class="row">
    <label>
      Root
      <select bind:value={root}>
        {#each ROOTS as name, i}<option value={i}>{name}</option>{/each}
      </select>
    </label>
    <label>
      Scale
      <select bind:value={index}>
        {#each SCALES as s, i}<option value={i}>{s.name}</option>{/each}
      </select>
    </label>
    <label>
      Marked with
      <select bind:value={labels}>
        <option value="names">Note names</option>
        <option value="degrees">Steps of the scale</option>
      </select>
    </label>
  </div>

  <table class="notes" aria-label="Notes of {title}">
    <tbody>
      <tr><th scope="row">Notes</th>{#each names as name}<td>{name}</td>{/each}</tr>
      <tr><th scope="row">Steps</th>{#each steps as step}<td>{step}</td>{/each}</tr>
    </tbody>
  </table>

  <div class="neck">
    <NeckDiagram {open} {classes} {root} {label} />
  </div>
  {#if neckNote}<p class="hint">{neckNote}</p>{/if}

  {#snippet footer()}
    <button type="button" class="btn" onclick={() => onplay(ascending(low, scale))}>Hear it</button>
    <button type="button" class="btn" onclick={() => onshow({ label: title, root, classes })}>Show on the fretboard</button>
    <button type="button" class="btn primary" onclick={onclose}>Close</button>
  {/snippet}
</Dialog>

<style>
  .row {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-4);
  }

  .row label {
    flex: 1;
    min-width: 140px;
  }

  .found {
    margin: 0 0 var(--space-4);
    padding: 0;
    border: 0;
    max-height: 160px;
    overflow-y: auto;
  }

  .found legend {
    margin-bottom: var(--space-3);
    padding: 0;
    font-weight: 600;
  }

  .found label {
    margin-bottom: var(--space-2);
  }

  .muted {
    color: var(--muted);
  }

  .hint {
    margin: var(--space-3) 0 0;
    color: var(--muted);
    font-size: var(--text-sm);
  }

  .notes {
    border-collapse: collapse;
    margin: var(--space-3) 0;
    font-variant-numeric: tabular-nums;
  }

  .notes th {
    padding-right: var(--space-4);
    color: var(--muted);
    font-weight: 400;
    text-align: left;
  }

  .notes td {
    min-width: 34px;
    padding: var(--space-1) var(--space-2);
    font-weight: 600;
    text-align: center;
  }

  .neck {
    overflow-x: auto;
    color: var(--text);
  }
</style>
