<script lang="ts">
  import { untrack } from 'svelte';
  import type { DrumHead, DrumPiece } from './model';
  import Dialog from './ui/Dialog.svelte';

  interface Props {
    /** The track's kit as it is now. */
    kit: DrumPiece[];
    /** The kit every drum track starts with. */
    standard: DrumPiece[];
    /** Called with the new kit; an empty one means the standard kit. */
    onapply: (kit: DrumPiece[]) => void;
    onclose: () => void;
  }

  let { kit, standard, onapply, onclose }: Props = $props();

  /** The most pieces a kit can have, as `gtr_model::KitPiece::MAX_PIECES`. */
  const MAX_PIECES = 64;
  const HEADS: [DrumHead, string][] = [
    ['normal', 'Round'],
    ['cross', 'Cross'],
    ['circled_cross', 'Circled cross'],
    ['diamond', 'Diamond'],
    ['triangle', 'Triangle'],
  ];
  /** Where on the staff a piece can be written, from the top down. */
  const PLACES: [number, string][] = Array.from({ length: 21 }, (_, i): [number, string] => {
    const position = 10 - i;
    const where =
      position > 4 ? `${ordinal((position - 3) >> 1)} ${position % 2 ? 'space' : 'line'} above`
      : position < -4 ? `${ordinal((-position - 3) >> 1)} ${position % 2 ? 'space' : 'line'} below`
      : position % 2 ? `${ordinal((position + 5) >> 1)} space`
      : `${ordinal((position + 6) >> 1)} line`;
    return [position, where];
  });

  function ordinal(n: number): string {
    return ['1st', '2nd', '3rd', '4th', '5th'][n - 1] ?? `${n}th`;
  }

  // The dialog edits a copy of the kit it opened with.
  let pieces = $state(untrack(() => kit.map((piece) => ({ ...piece }))));
  let problem = $state('');

  function add() {
    const taken = new Set(pieces.map((p) => p.number));
    // The first General MIDI percussion number the kit does not have yet.
    let number = 27;
    while (taken.has(number) && number < 87) number++;
    pieces.push({ number, name: 'New piece', position: 0, head: 'normal', foot: false });
  }

  function submit() {
    const numbers = pieces.map((p) => p.number);
    if (new Set(numbers).size !== numbers.length) {
      problem = 'Two pieces have the same number.';
      return;
    }
    const sorted = [...pieces]
      .map((p) => ({ ...p, name: p.name.trim() }))
      .sort((a, b) => a.number - b.number);
    // A kit that is the standard one again is not kept as the track's own.
    onapply(JSON.stringify(sorted) === JSON.stringify(standard) ? [] : sorted);
  }
</script>

<Dialog
  title="Drum kit"
  hint="The pieces this track has and how each is written. A piece's number is what a note's fret says on the tab, and decides its sound: the numbers are General MIDI's (36 bass drum, 38 snare, 42 closed hi-hat). Pieces played with a foot are written stems down, apart from the hands."
  width="640px"
  {onclose}
  onsubmit={submit}
>
  <table>
    <thead>
      <tr>
        <th scope="col">Number</th>
        <th scope="col">Name</th>
        <th scope="col">Written on the</th>
        <th scope="col">Notehead</th>
        <th scope="col">Foot</th>
        <th scope="col"><span class="hidden">Remove</span></th>
      </tr>
    </thead>
    <tbody>
      {#each pieces as piece, i (i)}
        <tr>
          <td><input type="number" min="0" max="87" required aria-label="Number" bind:value={piece.number} /></td>
          <td><input maxlength="40" required aria-label="Name" bind:value={piece.name} /></td>
          <td>
            <select aria-label="Written on the" bind:value={piece.position}>
              {#each PLACES as [position, where] (position)}
                <option value={position}>{where}</option>
              {/each}
            </select>
          </td>
          <td>
            <select aria-label="Notehead" bind:value={piece.head}>
              {#each HEADS as [head, label] (head)}
                <option value={head}>{label}</option>
              {/each}
            </select>
          </td>
          <td class="foot"><input type="checkbox" aria-label="Played with a foot" bind:checked={piece.foot} /></td>
          <td>
            <button type="button" class="btn" aria-label="Remove {piece.name}" onclick={() => pieces.splice(i, 1)}>
              Remove
            </button>
          </td>
        </tr>
      {/each}
    </tbody>
  </table>
  {#if problem}<p class="problem" role="alert">{problem}</p>{/if}
  {#snippet footer()}
    <button type="button" class="btn" disabled={pieces.length >= MAX_PIECES} onclick={add}>Add a piece</button>
    <button type="button" class="btn" onclick={() => (pieces = standard.map((piece) => ({ ...piece })))}>
      Standard kit
    </button>
    <span class="gap"></span>
    <button type="button" class="btn" onclick={onclose}>Cancel</button>
    <button class="btn primary">OK</button>
  {/snippet}
</Dialog>

<style>
  table {
    width: 100%;
    border-collapse: collapse;
    font-size: var(--text-sm);
  }

  th {
    padding: 0 var(--space-1) var(--space-2);
    color: var(--muted);
    font-weight: 500;
    text-align: left;
  }

  td {
    padding: 2px var(--space-1);
  }

  td input:not([type='checkbox']),
  td select {
    width: 100%;
  }

  td:first-child {
    width: 72px;
  }

  .foot {
    text-align: center;
  }

  .hidden {
    position: absolute;
    clip-path: inset(50%);
  }

  .problem {
    margin: var(--space-3) 0 0;
    color: var(--error);
  }

  .gap {
    flex: 1;
  }
</style>
