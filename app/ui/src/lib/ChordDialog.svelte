<script lang="ts">
  import { untrack } from 'svelte';
  import { CHORD_TYPES, ROOTS, chordShapes } from './chords';
  import type { ChordDiagram, Note } from './model';
  import ChordGrid, { GRID_FRETS, baseFret } from './ui/ChordGrid.svelte';
  import Dialog from './ui/Dialog.svelte';

  interface Props {
    /** Open-string pitches with the capo on, from the highest string to the lowest. */
    tuning: number[];
    /** The name and fingering the beat has now. */
    name: string;
    diagram: ChordDiagram | null;
    /** The notes on the beat, to take a fingering from. */
    notes: Note[];
    /** The new name and fingering; either may be left out. */
    onapply: (name: string | null, diagram: ChordDiagram | null) => void;
    onclose: () => void;
  }

  let { tuning, name, diagram, notes, onapply, onclose }: Props = $props();

  const unplayed = () => tuning.map(() => -1);
  let chordName = $state(untrack(() => name));
  let frets = $state(untrack(() => diagram?.frets.slice(0, tuning.length) ?? unplayed()));
  let showDiagram = $state(untrack(() => !!diagram || !name));
  let root = $state(0);
  let type = $state(0);

  const shapes = $derived(chordShapes(tuning, root, CHORD_TYPES[type].intervals));
  const built = $derived(ROOTS[root] + CHORD_TYPES[type].suffix);
  const empty = $derived(frets.every((f) => f < 0));

  function useShape(shape: number[]) {
    frets = shape;
    chordName = built;
    showDiagram = true;
  }

  function fromNotes() {
    const taken = unplayed();
    for (const note of notes) taken[note.string] = note.fret;
    frets = taken;
    showDiagram = true;
  }

  /** A click on the grid: stop the string there, or open or silence it above the nut. */
  function pick(string: number, fret: number) {
    const next = [...frets];
    if (fret === 0) next[string] = next[string] === 0 ? -1 : 0;
    else next[string] = next[string] === fret ? -1 : fret;
    frets = next;
    showDiagram = true;
  }

  /** Moves the whole shape up or down the neck. */
  function shift(by: number) {
    if (frets.some((f) => f > 0 && (f + by < 1 || f + by > 24))) return;
    frets = frets.map((f) => (f > 0 ? f + by : f));
  }

  function submit() {
    const fingering = showDiagram && !empty ? { frets: $state.snapshot(frets) } : null;
    onapply(chordName.trim() || null, fingering);
    onclose();
  }
</script>

<Dialog
  title="Chord"
  hint="The name is written above the beat, with the diagram under it if there is one."
  width="520px"
  {onclose}
  onsubmit={submit}
>
  <label>Name <input bind:value={chordName} maxlength="120" placeholder="For example Am7 or D/F#" /></label>

  <div class="builder">
    <div class="shape">
      <ChordGrid {frets} scale={16} onpick={pick} />
      <div class="moves">
        <button type="button" class="btn" onclick={() => shift(-1)}>− fret</button>
        <button type="button" class="btn" onclick={() => shift(1)}>+ fret</button>
      </div>
      <p class="note">
        Click a fret to stop a string there, and above the top line to play it open or leave
        it out. Frets {baseFret(frets)} to {baseFret(frets) + GRID_FRETS - 1} are shown.
      </p>
    </div>

    <div class="find">
      <div class="pair">
        <label>
          Root
          <select bind:value={root}>
            {#each ROOTS as note, i}
              <option value={i}>{note}</option>
            {/each}
          </select>
        </label>
        <label>
          Kind
          <select bind:value={type}>
            {#each CHORD_TYPES as kind, i}
              <option value={i}>{kind.name}</option>
            {/each}
          </select>
        </label>
      </div>
      <div class="shapes" role="group" aria-label="Ways to play {built}">
        {#each shapes as shape}
          <button
            type="button"
            class="option"
            aria-label="{built}: {[...shape].reverse().map((f) => (f < 0 ? 'x' : f)).join(' ')}"
            onclick={() => useShape(shape)}
          >
            <ChordGrid frets={shape} scale={7} />
          </button>
        {:else}
          <p class="note">No way to play {built} was found in this tuning.</p>
        {/each}
      </div>
    </div>
  </div>

  <label class="check">
    <input type="checkbox" bind:checked={showDiagram} disabled={empty} /> Show the diagram in the score
  </label>

  {#snippet footer()}
    <button type="button" class="btn remove" disabled={!notes.length} onclick={fromNotes}>
      Use the notes on this beat
    </button>
    <button type="button" class="btn" onclick={onclose}>Cancel</button>
    <button type="submit" class="btn primary">OK</button>
  {/snippet}
</Dialog>

<style>
  .builder {
    display: flex;
    gap: var(--space-5);
    margin-bottom: var(--space-4);
  }

  .shape {
    flex: none;
    width: 170px;
  }

  .moves {
    display: flex;
    gap: var(--space-3);
    margin: var(--space-3) 0;
  }

  .find {
    flex: 1;
    min-width: 0;
  }

  .pair {
    display: grid;
    grid-template-columns: 1fr 2fr;
    gap: var(--space-4);
  }

  .pair select {
    width: 100%;
    min-width: 0;
  }

  .shapes {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-2);
    max-height: 190px;
    overflow-y: auto;
  }

  .option {
    padding: var(--space-1);
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-s);
    background: var(--raised);
    cursor: pointer;
  }

  .option:hover {
    border-color: var(--accent);
    background: var(--pressed);
  }

  .note {
    margin: 0;
    color: var(--muted);
    font-size: var(--text-sm);
  }
</style>
