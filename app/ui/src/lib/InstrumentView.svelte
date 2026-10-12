<script lang="ts" module>
  /** Pitch classes marked on the instrument, such as a scale's, with its root set apart. */
  export interface Marks {
    /** What they are, said over the instrument. */
    label: string;
    root: number;
    classes: number[];
  }
</script>

<script lang="ts">
  // The instrument of the cursor's track drawn as it is played: a fretboard for a track on
  // strings, a keyboard for a piano and pads for a drum kit. It lights the notes of the
  // beat at the cursor, or of the beat sounding while the music plays, and a fret, key or
  // pad clicked puts that note in the beat at the cursor or takes it out.
  import { tick } from 'svelte';
  import type { DrumPiece, Note } from './model';
  import {
    FRETS,
    INLAYS,
    PIANO_RANGE,
    fretWidths,
    isBlack,
    pianoKeys,
    pitchName,
  } from './instrument';
  import Icon from './ui/Icon.svelte';

  interface Props {
    kind: 'strings' | 'keys' | 'drums';
    /** Open strings from the highest to the lowest, without the capo. */
    tuning: number[];
    capo: number;
    kit: DrumPiece[];
    /** The notes lit: of the beat at the cursor, or of the one sounding. */
    notes: Note[];
    /** True while the notes lit are the ones sounding. */
    sounding: boolean;
    /** The cursor's string, or on a piano its pitch. */
    cursor: number;
    marks: Marks | null;
    /** A string and fret, a key's pitch or a piece's number was clicked. */
    onpick: (string: number, fret: number) => void;
    onclose: () => void;
  }

  let { kind, tuning, capo, kit, notes, sounding, cursor, marks, onpick, onclose }: Props = $props();

  const marked = (pitch: number) => marks?.classes.includes(((pitch % 12) + 12) % 12) ?? false;
  const isRoot = (pitch: number) => marks !== null && ((pitch % 12) + 12) % 12 === marks.root;

  // --- Fretboard ---

  const fretCount = $derived(Math.max(FRETS, ...notes.map((n) => n.fret)));
  const columns = $derived(
    fretWidths(fretCount)
      .map((share, fret) => `minmax(${fret === 0 ? 26 : 24}px, ${share.toFixed(3)}fr)`)
      .join(' '),
  );
  const fretted = (string: number, fret: number) =>
    notes.some((n) => n.string === string && n.fret === fret);

  // --- Keyboard ---

  const KEY_WIDTH = 22;
  const keys = pianoKeys(...PIANO_RANGE);
  const whiteCount = keys.filter((k) => !k.black).length;
  const pressed = (pitch: number) => notes.some((n) => n.string === pitch);

  /** The keyboard scrolls to keep the notes lit, or the cursor's pitch, in view. */
  let keyboard = $state<HTMLElement>();
  $effect(() => {
    if (kind !== 'keys' || !keyboard) return;
    const pitches = notes.length ? notes.map((n) => n.string) : [cursor];
    const at = (pitch: number) => (keys.find((k) => k.pitch === pitch)?.x ?? 0) * KEY_WIDTH;
    const [low, high] = [at(Math.min(...pitches)), at(Math.max(...pitches)) + KEY_WIDTH];
    const view = keyboard;
    void tick().then(() => {
      if (low < view.scrollLeft || high > view.scrollLeft + view.clientWidth) {
        view.scrollLeft = (low + high) / 2 - view.clientWidth / 2;
      }
    });
  });

  // --- Drums ---

  const struck = (piece: number) => notes.some((n) => n.fret === piece);

  const title = $derived(
    kind === 'strings' ? 'Fretboard'
    : kind === 'keys' ? 'Keyboard'
    : 'Drum kit',
  );
</script>

<section class="instrument" aria-label={title} class:sounding>
  <header>
    <h2>{title}</h2>
    {#if kind === 'strings' && capo > 0}<span class="note">Capo {capo}: frets counted from it</span>{/if}
    {#if marks}<span class="note marks">{marks.label}</span>{/if}
    <span class="rule"></span>
    <button type="button" class="tool" aria-label="Close the {title.toLowerCase()}" onclick={onclose}>
      <Icon name="remove" size={14} />
    </button>
  </header>

  {#if kind === 'strings'}
    <div class="neck-scroll">
      <div class="neck" style:grid-template-columns={columns} style:--strings={tuning.length}>
        {#each tuning as open, string}
          {#each { length: fretCount + 1 } as _, fret}
            {@const pitch = open + capo + fret}
            {@const on = fretted(string, fret)}
            <button
              type="button"
              tabindex="-1"
              class="fret"
              class:open={fret === 0}
              class:cursor-string={string === cursor && !sounding}
              aria-label="String {string + 1}, fret {fret}, {pitchName(pitch)}"
              aria-pressed={on}
              onmousedown={(e) => e.preventDefault()}
              onclick={() => onpick(string, fret)}
            >
              {#if on}
                <span class="dot on">{fret}</span>
              {:else if marked(pitch)}
                <span class="dot mark" class:root={isRoot(pitch)}>{pitchName(pitch)}</span>
              {/if}
            </button>
          {/each}
        {/each}
        {#each { length: fretCount + 1 } as _, fret}
          <span class="number" class:inlay={INLAYS.includes(fret)}>{fret}</span>
        {/each}
      </div>
    </div>
  {:else if kind === 'keys'}
    <div class="keys-scroll" bind:this={keyboard}>
      <div class="keys" style:width="{whiteCount * KEY_WIDTH}px">
        {#each keys as key (key.pitch)}
          {@const on = pressed(key.pitch)}
          <button
            type="button"
            tabindex="-1"
            class="key"
            class:black={key.black}
            class:on
            class:mark={!on && marked(key.pitch)}
            class:root={!on && isRoot(key.pitch)}
            class:cursor-key={key.pitch === cursor && !sounding}
            style:left="{(key.black ? key.x - 0.32 : key.x) * KEY_WIDTH}px"
            aria-label="{pitchName(key.pitch)}{Math.floor(key.pitch / 12) - 1}"
            aria-pressed={on}
            onmousedown={(e) => e.preventDefault()}
            onclick={() => onpick(key.pitch, 0)}
          >
            {#if key.pitch % 12 === 0 && !isBlack(key.pitch)}<span>C{Math.floor(key.pitch / 12) - 1}</span>{/if}
          </button>
        {/each}
      </div>
    </div>
  {:else}
    <div class="pads">
      {#each kit as piece (piece.number)}
        {@const on = struck(piece.number)}
        <button
          type="button"
          tabindex="-1"
          class="pad"
          class:on
          aria-pressed={on}
          onmousedown={(e) => e.preventDefault()}
          onclick={() => onpick(0, piece.number)}
        >
          {piece.name}
        </button>
      {/each}
    </div>
  {/if}
</section>

<style>
  .instrument {
    flex: none;
    border-top: 1px solid var(--border);
    background: var(--panel);
  }

  header {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    height: 28px;
    padding: 0 var(--space-2) 0 var(--space-4);
  }

  h2 {
    margin: 0;
    font-size: var(--text-sm);
    font-weight: 600;
  }

  .note {
    color: var(--muted);
    font-size: var(--text-sm);
  }

  .rule {
    flex: 1;
  }

  .tool {
    display: inline-grid;
    place-items: center;
    width: 24px;
    height: 24px;
    border: 0;
    border-radius: var(--radius-s);
    background: none;
    color: var(--muted);
    cursor: pointer;
  }

  .tool:hover {
    background: var(--hover);
  }

  /* Fretboard */

  .neck-scroll {
    overflow-x: auto;
    padding: 0 var(--space-4) var(--space-3);
  }

  .neck {
    display: grid;
    min-width: 0;
  }

  .fret {
    position: relative;
    display: grid;
    place-items: center;
    height: 22px;
    padding: 0;
    border: 0;
    border-right: 2px solid var(--staff);
    background: linear-gradient(transparent calc(50% - 0.75px), var(--ink) calc(50% - 0.75px), var(--ink) calc(50% + 0.75px), transparent calc(50% + 0.75px));
    cursor: pointer;
  }

  .fret.open {
    border-right: 5px solid var(--ink);
    background: linear-gradient(transparent calc(50% - 0.5px), var(--muted) calc(50% - 0.5px), var(--muted) calc(50% + 0.5px), transparent calc(50% + 0.5px));
  }

  .fret:hover {
    background-color: var(--hover);
  }

  .fret.cursor-string {
    background-color: var(--select);
  }

  .dot {
    display: grid;
    place-items: center;
    min-width: 18px;
    height: 18px;
    padding: 0 3px;
    border-radius: 9px;
    font-size: var(--text-xs);
    font-weight: 600;
    line-height: 1;
  }

  .dot.on {
    background: var(--accent);
    color: var(--on-accent);
  }

  .sounding .dot.on {
    box-shadow: 0 0 8px var(--accent);
  }

  .dot.mark {
    border: 1.5px solid var(--focus);
    background: var(--panel);
    color: var(--text);
  }

  .dot.mark.root {
    background: var(--focus);
    color: var(--panel);
  }

  .number {
    padding-top: 2px;
    color: var(--muted);
    font-size: var(--text-xs);
    text-align: center;
  }

  .number.inlay {
    color: var(--text);
    font-weight: 700;
  }

  /* Keyboard */

  .keys-scroll {
    overflow-x: auto;
    padding: 0 var(--space-4) var(--space-3);
  }

  .keys {
    position: relative;
    height: 96px;
  }

  .key {
    position: absolute;
    top: 0;
    width: 22px;
    height: 96px;
    padding: 0 0 4px;
    border: 1px solid var(--border-strong);
    border-radius: 0 0 3px 3px;
    background: #fdfdfb;
    color: #555;
    font-size: 9px;
    cursor: pointer;
    display: flex;
    align-items: flex-end;
    justify-content: center;
  }

  .key.black {
    z-index: 1;
    width: 14px;
    height: 58px;
    border-color: #111;
    background: #1d1d1f;
  }

  .key.cursor-key {
    box-shadow: inset 0 -6px 0 var(--focus);
  }

  .key.mark {
    background: color-mix(in srgb, var(--focus) 30%, #fdfdfb);
  }

  .key.black.mark {
    background: color-mix(in srgb, var(--focus) 55%, #1d1d1f);
  }

  .key.root {
    background: var(--focus);
  }

  .key.on,
  .key.black.on {
    background: var(--accent);
  }

  /* Drums */

  .pads {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-2);
    max-height: 132px;
    overflow-y: auto;
    padding: 0 var(--space-4) var(--space-3);
  }

  .pad {
    min-width: 88px;
    height: 38px;
    padding: 0 var(--space-3);
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-m);
    background: var(--raised);
    color: var(--text);
    font-size: var(--text-sm);
    cursor: pointer;
  }

  .pad:hover {
    background: var(--hover);
  }

  .pad.on {
    border-color: var(--accent);
    background: var(--accent);
    color: var(--on-accent);
  }
</style>
