<script lang="ts" module>
  export type DialogKind = 'score' | 'time' | 'tuning';

  const NOTE_NAMES = ['C', 'C#', 'D', 'D#', 'E', 'F', 'F#', 'G', 'G#', 'A', 'A#', 'B'];
  const FLATS: Record<string, string> = { DB: 'C#', EB: 'D#', GB: 'F#', AB: 'G#', BB: 'A#' };

  /** MIDI pitch as a note name with octave, e.g. 40 → "E2". */
  export function noteName(pitch: number): string {
    return `${NOTE_NAMES[pitch % 12]}${Math.floor(pitch / 12) - 1}`;
  }

  /** Parses names such as "E2", "f#3" or "Bb1"; `null` if the text is not a note. */
  export function parseNote(text: string): number | null {
    const match = /^([A-Ga-g][#b]?)(-?\d)$/.exec(text.trim());
    if (!match) return null;
    const name = match[1].toUpperCase();
    const index = NOTE_NAMES.indexOf(FLATS[name] ?? name);
    const pitch = index + 12 * (Number(match[2]) + 1);
    return index < 0 || pitch < 0 || pitch > 127 ? null : pitch;
  }

  const PRESETS: { name: string; strings: number[] }[] = [
    { name: 'Guitar: standard', strings: [64, 59, 55, 50, 45, 40] },
    { name: 'Guitar: drop D', strings: [64, 59, 55, 50, 45, 38] },
    { name: 'Guitar: D standard', strings: [62, 57, 53, 48, 43, 38] },
    { name: 'Guitar: drop C', strings: [62, 57, 53, 48, 43, 36] },
    { name: 'Guitar: open G', strings: [62, 59, 55, 50, 43, 38] },
    { name: 'Guitar: DADGAD', strings: [62, 57, 55, 50, 45, 38] },
    { name: '7-string: standard', strings: [64, 59, 55, 50, 45, 40, 35] },
    { name: '7-string: drop A', strings: [64, 59, 55, 50, 45, 40, 33] },
    { name: '8-string: standard', strings: [64, 59, 55, 50, 45, 40, 35, 30] },
    { name: 'Bass: standard', strings: [43, 38, 33, 28] },
    { name: 'Bass: drop D', strings: [43, 38, 33, 26] },
    { name: '5-string bass: standard', strings: [43, 38, 33, 28, 23] },
  ];
</script>

<script lang="ts">
  import { untrack } from 'svelte';
  import type { Score } from './model';

  interface Props {
    kind: DialogKind;
    score: Score;
    track: number;
    bar: number;
    onscore: (info: { title: string; artist: string; album: string }, tempo: number) => void;
    ontime: (numerator: number, denominator: number) => void;
    ontuning: (strings: number[], capo: number) => void;
    onclose: () => void;
  }

  let { kind, score, track, bar, onscore, ontime, ontuning, onclose }: Props = $props();

  // The dialog edits a copy of the values it opened with.
  const initial = untrack(() => ({
    info: score.info,
    tempo: score.tempo,
    time: score.master_bars[bar].time_signature,
    staff: score.tracks[track].staves[0],
  }));
  let title = $state(initial.info.title);
  let artist = $state(initial.info.artist);
  let album = $state(initial.info.album);
  let tempo = $state(initial.tempo);
  let numerator = $state(initial.time.numerator);
  let denominator = $state(initial.time.denominator);
  // Shown low string first, the way tunings are usually written.
  let tuningText = $state([...initial.staff.tuning.strings].reverse().map(noteName).join(' '));
  let capo = $state(initial.staff.capo);
  let problem = $state('');

  const preset = $derived(
    PRESETS.findIndex((p) => [...p.strings].reverse().map(noteName).join(' ') === tuningText),
  );

  let dialog: HTMLDialogElement;
  $effect(() => dialog.showModal());

  function submit(event: SubmitEvent) {
    event.preventDefault();
    if (kind === 'score') {
      onscore({ title: title.trim(), artist: artist.trim(), album: album.trim() }, tempo);
    } else if (kind === 'time') {
      ontime(numerator, denominator);
    } else {
      const strings = tuningText.trim().split(/[\s,]+/).map(parseNote);
      if (strings.includes(null)) {
        problem = 'Write each string as a note and octave, for example E2 A2 D3 G3 B3 E4.';
        return;
      }
      ontuning((strings as number[]).reverse(), capo);
    }
  }
</script>

<dialog bind:this={dialog} {onclose}>
  <form onsubmit={submit}>
    {#if kind === 'score'}
      <h2>Score</h2>
      <label>Title <input bind:value={title} /></label>
      <label>Artist <input bind:value={artist} /></label>
      <label>Album <input bind:value={album} /></label>
      <label>
        Tempo (bpm)
        <input type="number" min="20" max="400" required bind:value={tempo} />
      </label>
    {:else if kind === 'time'}
      <h2>Time signature</h2>
      <p>Applies from bar {bar + 1} up to the next time signature change.</p>
      <div class="row">
        <input type="number" min="1" max="32" required aria-label="Beats" bind:value={numerator} />
        <span>/</span>
        <select aria-label="Beat unit" bind:value={denominator}>
          {#each [1, 2, 4, 8, 16, 32] as unit}
            <option value={unit}>{unit}</option>
          {/each}
        </select>
      </div>
    {:else}
      <h2>Tuning</h2>
      <label>
        Preset
        <select
          value={preset}
          onchange={(e) => {
            const chosen = PRESETS[Number(e.currentTarget.value)];
            if (chosen) tuningText = [...chosen.strings].reverse().map(noteName).join(' ');
          }}
        >
          <option value={-1}>Custom</option>
          {#each PRESETS as p, i}
            <option value={i}>{p.name}</option>
          {/each}
        </select>
      </label>
      <label>
        Strings, lowest first
        <input bind:value={tuningText} spellcheck="false" />
      </label>
      <label>
        Capo
        <input type="number" min="0" max="12" required bind:value={capo} />
      </label>
      {#if problem}<p class="problem">{problem}</p>{/if}
    {/if}
    <footer>
      <button type="button" onclick={onclose}>Cancel</button>
      <button type="submit" class="primary">OK</button>
    </footer>
  </form>
</dialog>

<style>
  dialog {
    width: min(360px, 90vw);
    padding: 20px;
    border: 1px solid var(--border);
    border-radius: 8px;
    background: var(--panel);
    color: var(--text);
  }

  dialog::backdrop {
    background: rgb(0 0 0 / 40%);
  }

  h2 {
    margin: 0 0 14px;
    font-size: 16px;
  }

  p {
    margin: 0 0 12px;
    color: var(--muted);
  }

  .problem {
    color: var(--error);
  }

  label {
    display: grid;
    gap: 4px;
    margin-bottom: 12px;
    color: var(--muted);
    font-size: 12px;
  }

  input,
  select {
    font: inherit;
    font-size: 14px;
    padding: 5px 8px;
    border: 1px solid var(--border);
    border-radius: 4px;
    background: var(--bg);
    color: var(--text);
  }

  .row {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 12px;
  }

  .row input {
    width: 70px;
  }

  footer {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 6px;
  }

  button {
    font: inherit;
    padding: 5px 14px;
    border: 1px solid var(--border);
    border-radius: 4px;
    background: var(--bg);
    color: var(--text);
    cursor: pointer;
  }

  button.primary {
    border-color: var(--accent);
    background: var(--accent);
    color: var(--paper);
  }
</style>
