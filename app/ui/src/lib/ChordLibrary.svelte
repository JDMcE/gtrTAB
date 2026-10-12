<script lang="ts">
  // The chord library: every kind of chord on every root, with the ways to play it in the
  // track's tuning drawn as diagrams. A shape can be heard, shown on the fretboard, or put
  // on the beat at the cursor with its name and diagram.
  import { CHORD_TYPES, ROOTS, chordName, chordShapes } from './chords';
  import type { Marks } from './InstrumentView.svelte';
  import ChordGrid from './ui/ChordGrid.svelte';
  import Dialog from './ui/Dialog.svelte';

  interface Props {
    /** Pitches of the open strings with the capo on, from the highest string; empty for a
     *  track without strings, which can only hear a chord and see its notes. */
    open: number[];
    onshow: (marks: Marks) => void;
    onplay: (pitches: number[]) => void;
    /** Puts the shape, frets from the highest string, on the beat with the chord's name. */
    onput: (frets: number[], name: string) => void;
    onclose: () => void;
  }

  let { open, onshow, onplay, onput, onclose }: Props = $props();

  let root = $state(0);
  let type = $state(0);
  let chosen = $state(0);

  const kind = $derived(CHORD_TYPES[type]);
  const name = $derived(chordName(root, kind.suffix));
  const shapes = $derived(open.length ? chordShapes(open, root, kind.intervals) : []);
  const shape = $derived(shapes[Math.min(chosen, shapes.length - 1)]);
  const tones = $derived(kind.intervals.map((i) => (root + i) % 12));
  const FORMULA = ['1', '♭2', '2', '♭3', '3', '4', '♭5', '5', '♯5', '6', '♭7', '7'];
  /** A ninth is written as one, not as a second. */
  const step = (interval: number) => (interval === 2 && kind.intervals.length > 3 ? '9' : FORMULA[interval]);

  // A new chord starts from its first shape.
  $effect(() => {
    void [root, type];
    chosen = 0;
  });

  /** Frets from the lowest string up, as a shape is read aloud. */
  const spoken = (frets: number[]) => [...frets].reverse().map((f) => (f < 0 ? 'x' : f)).join(' ');

  /** The pitches a shape sounds, lowest first; without a shape, the chord on middle C's octave. */
  const pitches = $derived(
    shape ?
      shape.flatMap((f, s) => (f < 0 ? [] : [open[s] + f])).sort((a, b) => a - b)
    : kind.intervals.map((i) => 48 + root + i).sort((a, b) => a - b),
  );
</script>

<Dialog title="Chord library" width="620px" {onclose}>
  <div class="row">
    <label>
      Root
      <select bind:value={root}>
        {#each ROOTS as note, i}<option value={i}>{note}</option>{/each}
      </select>
    </label>
    <label class="wide">
      Chord
      <select bind:value={type}>
        {#each CHORD_TYPES as kind, i}<option value={i}>{kind.name}</option>{/each}
      </select>
    </label>
  </div>

  <p class="notes">
    <strong>{name}</strong>
    <span>{tones.map((t) => ROOTS[t]).join(' ')}</span>
    <span class="muted">({kind.intervals.map(step).join(' ')})</span>
  </p>

  {#if open.length}
    <div class="shapes" role="group" aria-label="Ways to play {name}">
      {#each shapes as option, i}
        <button
          type="button"
          class="option"
          aria-label="{name}: {spoken(option)}"
          aria-pressed={i === chosen}
          onclick={() => (chosen = i)}
        >
          <ChordGrid frets={option} scale={9} />
        </button>
      {:else}
        <p class="muted">No way to play {name} was found in this tuning.</p>
      {/each}
    </div>
  {:else}
    <p class="muted">This track has no strings, so there are no shapes to show; the chord can be heard.</p>
  {/if}

  {#snippet footer()}
    <button type="button" class="btn" onclick={() => onplay(pitches)}>Hear it</button>
    <button type="button" class="btn" onclick={() => onshow({ label: name, root, classes: tones })}>
      Show on the fretboard
    </button>
    <button type="button" class="btn" disabled={!shape} onclick={() => shape && onput($state.snapshot(shape), name)}>
      Put on the beat
    </button>
    <button type="button" class="btn primary" onclick={onclose}>Close</button>
  {/snippet}
</Dialog>

<style>
  .row {
    display: flex;
    gap: var(--space-4);
  }

  .row label {
    flex: 1;
  }

  .row .wide {
    flex: 2;
  }

  .notes {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: var(--space-3);
    margin: 0 0 var(--space-4);
  }

  .notes strong {
    font-size: var(--text-lg);
  }

  .muted {
    color: var(--muted);
  }

  .shapes {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-2);
    max-height: 300px;
    overflow-y: auto;
  }

  .option {
    padding: var(--space-1);
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-s);
    background: var(--raised);
    color: var(--text);
    cursor: pointer;
  }

  .option:hover {
    background: var(--hover);
  }

  .option[aria-pressed='true'] {
    border-color: var(--accent);
    background: var(--pressed);
    box-shadow: 0 0 0 1px var(--accent);
  }
</style>
