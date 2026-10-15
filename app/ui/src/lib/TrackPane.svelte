<script lang="ts" module>
  import type { Mix } from './model';

  /** The audio track, as its row shows it. */
  export interface AudioRow {
    name: string;
    mix: Mix;
    /** The bars heard over the recording. */
    bars: number[];
    /** Whether its waveform is shown under the score. */
    shown: boolean;
  }
</script>

<script lang="ts">
  import { tick } from 'svelte';
  import type { Command } from './commands';
  import { INSTRUMENT_ICONS, type Score } from './model';
  import Icon from './ui/Icon.svelte';
  import ToolButton from './ui/ToolButton.svelte';
  import { roving } from './ui/roving';
  import { tip } from './ui/tooltip';

  interface Props {
    score: Score;
    /** The score's audio track, or `null` without one. */
    audio: AudioRow | null;
    /** The track being edited. */
    track: number;
    /** The cursor's bar. */
    bar: number;
    /** The first and last bars of the selection on the track being edited, if there is one. */
    selection: [number, number] | null;
    /** The bar sounding now, while playing. */
    playBar: number | null;
    /** The first bar visible in the score, which the grid keeps in view. */
    viewBar: number;
    /** Height in pixels, or `null` to fit the tracks. */
    height: number | null;
    /** Track actions shown above the names; they act on the current track. */
    commands: Command[];
    onpick: (track: number, bar: number | null) => void;
    /** Selects the bars of a track from `from` to `to`, with the cursor at the `to` end. */
    onselect: (track: number, from: number, to: number) => void;
    onmix: (track: number, mix: Mix) => void;
    /** Puts the track at `from` where the one at `to` is. */
    onmove: (from: number, to: number) => void;
    onproperties: (track: number) => void;
    onresize: (height: number | null) => void;
    onaudiomix: (mix: Mix) => void;
    /** Shows or hides the audio track's waveform. */
    onaudio: () => void;
  }

  let {
    score,
    audio,
    track,
    bar,
    selection,
    playBar,
    viewBar,
    height,
    commands,
    onpick,
    onselect,
    onmix,
    onmove,
    onproperties,
    onresize,
    onaudiomix,
    onaudio,
  }: Props = $props();

  /** Width of one bar in the grid. */
  const CELL = 16;
  const ROW = 30;
  const HEAD = 30;
  const MIN_HEIGHT = HEAD + ROW + 8;
  /** Width of the pan control and of its handle, which decide where its fill ends. */
  const PAN_WIDTH = 52;
  const PAN_HANDLE = 8;
  /** X of the middle of the pan handle for a pan from -100 to 100. */
  const panAt = (pan: number) => PAN_HANDLE / 2 + ((pan + 100) / 200) * (PAN_WIDTH - PAN_HANDLE);
  /** Bars between the numbers along the top. */
  const NUMBER_EVERY = 4;

  let pane: HTMLElement;
  let scroller: HTMLDivElement;
  let infoWidth = $state(0);
  /** The row of the audio track, in place of a track's number. */
  const AUDIO = -1;
  /** Slider positions while they are being dragged, before the change is made. */
  let dragging = $state<{ track: number; mix: Mix } | null>(null);
  /** Whether the slider in use is being moved with the pointer, not the keyboard. */
  let pointing = false;
  /** A track being dragged to a new place: where it came from and the row it is over. */
  let moving = $state<{ from: number; to: number } | null>(null);
  /** Set when a drag has just ended, so that the click that follows it is not a pick. */
  let dragged = false;
  /** How far the pointer has to travel before a press on a name becomes a drag. */
  const DRAG_START = 5;

  const barCount = $derived(score.master_bars.length);
  const rows = $derived(score.tracks.length + (audio ? 1 : 0));
  const fitted = $derived(HEAD + rows * ROW + 14);
  /** The height the pane has: the one it was dragged to, or enough for the tracks. */
  const shown = $derived(height ?? Math.min(fitted, 190));

  /** For each track, the runs of consecutive bars that hold notes, as `[first, length]`. */
  const runs = $derived(
    score.tracks.map((t) =>
      runsOf(
        Array.from({ length: barCount }, (_, i) => i).filter((i) =>
          t.staves.some((s) => s.bars[i]?.voices.some((v) => v.beats.some((b) => b.notes.length > 0))),
        ),
      ),
    ),
  );
  const audioRuns = $derived(runsOf(audio?.bars ?? []));

  /** Bars in order, as runs of consecutive bars `[first, length]`. */
  function runsOf(bars: number[]): [number, number][] {
    const found: [number, number][] = [];
    for (const i of bars) {
      const last = found[found.length - 1];
      if (last && last[0] + last[1] === i) last[1]++;
      else if (!last || last[0] + last[1] < i) found.push([i, 1]);
    }
    return found;
  }

  const numbers = $derived(
    Array.from({ length: Math.ceil(barCount / NUMBER_EVERY) }, (_, i) => i * NUMBER_EVERY),
  );
  const sections = $derived(
    score.master_bars.flatMap((m, index) => (m.section ? [{ index, name: m.section }] : [])),
  );
  const anySolo = $derived(score.tracks.some((t) => t.mix.solo) || !!audio?.mix.solo);

  const mixOf = (index: number): Mix =>
    dragging?.track === index ? dragging.mix : index === AUDIO ? audio!.mix : score.tracks[index].mix;

  const setMix = (index: number, mix: Mix) => (index === AUDIO ? onaudiomix(mix) : onmix(index, mix));

  /** Scrolls the grid just far enough to show a bar. */
  function reveal(index: number) {
    if (!scroller) return;
    const left = index * CELL;
    const visible = scroller.clientWidth - infoWidth;
    if (left < scroller.scrollLeft) scroller.scrollLeft = Math.max(0, left - CELL * 2);
    else if (left + CELL > scroller.scrollLeft + visible) {
      scroller.scrollLeft = left + CELL * 3 - visible;
    }
  }

  $effect(() => reveal(playBar ?? bar));
  $effect(() => reveal(viewBar));
  // Keep the current track's row in view when the track changes from the keyboard.
  $effect(() => {
    const top = track * ROW;
    const visible = scroller.clientHeight - HEAD;
    if (top < scroller.scrollTop) scroller.scrollTop = top;
    else if (top + ROW > scroller.scrollTop + visible) scroller.scrollTop = top + ROW - visible;
  });

  /**
   * A press on a track's bars: a click goes to the bar, with Shift it selects from the cursor
   * to the bar, and a drag selects the bars it crosses.
   */
  function pressBars(event: PointerEvent, index: number) {
    if (event.button !== 0) return;
    const cells = event.currentTarget as HTMLElement;
    const barAt = (clientX: number) =>
      Math.max(0, Math.min(barCount - 1, Math.floor((clientX - cells.getBoundingClientRect().left) / CELL)));
    const start = barAt(event.clientX);
    // Shift carries on from the end of the selection the cursor is not at, or else from the cursor.
    const anchor = !selection ? bar : bar === selection[0] ? selection[1] : selection[0];
    const from = event.shiftKey && index === track ? anchor : start;
    if (event.shiftKey && index === track) onselect(index, from, start);
    else onpick(index, start);

    let last = start;
    cells.setPointerCapture(event.pointerId);
    const move = (e: PointerEvent) => {
      const at = barAt(e.clientX);
      if (at === last) return;
      last = at;
      onselect(index, from, at);
    };
    const end = () => {
      cells.removeEventListener('pointermove', move);
      cells.removeEventListener('pointerup', end);
      cells.removeEventListener('pointercancel', end);
    };
    cells.addEventListener('pointermove', move);
    cells.addEventListener('pointerup', end);
    cells.addEventListener('pointercancel', end);
  }

  function slide(index: number, field: 'volume' | 'pan', event: Event, commit: boolean) {
    const input = event.currentTarget as HTMLInputElement;
    const mix = { ...mixOf(index), [field]: Number(input.value) };
    if (commit) {
      dragging = null;
      setMix(index, mix);
      // After the mouse the keys go back to the score; from the keyboard they stay here.
      if (pointing) input.blur();
      pointing = false;
    } else {
      dragging = { track: index, mix };
    }
  }

  // --- Reordering ---

  /** A press on a track's name: dragged up or down, it moves the track to the row it is let go on. */
  function startMove(event: PointerEvent, index: number) {
    dragged = false;
    if (event.button !== 0 || score.tracks.length < 2) return;
    const handle = event.currentTarget as HTMLElement;
    const startY = event.clientY;
    const rowAt = (clientY: number) => {
      const top = scroller.getBoundingClientRect().top + HEAD - scroller.scrollTop;
      return Math.max(0, Math.min(score.tracks.length - 1, Math.floor((clientY - top) / ROW)));
    };
    const move = (e: PointerEvent) => {
      if (!moving && Math.abs(e.clientY - startY) < DRAG_START) return;
      if (!moving) handle.setPointerCapture(e.pointerId);
      moving = { from: index, to: rowAt(e.clientY) };
    };
    const end = (e: PointerEvent) => {
      handle.removeEventListener('pointermove', move);
      handle.removeEventListener('pointerup', end);
      handle.removeEventListener('pointercancel', end);
      const done = moving;
      moving = null;
      if (!done) return;
      dragged = true;
      if (e.type === 'pointerup' && done.to !== done.from) onmove(done.from, done.to);
    };
    handle.addEventListener('pointermove', move);
    handle.addEventListener('pointerup', end);
    handle.addEventListener('pointercancel', end);
  }

  function pickName(index: number) {
    if (dragged) dragged = false;
    else onpick(index, null);
  }

  /** Alt with the up and down arrows moves a track from the keyboard, and the focus with it. */
  function moveKey(event: KeyboardEvent, index: number) {
    const by = event.key === 'ArrowUp' ? -1 : event.key === 'ArrowDown' ? 1 : 0;
    const to = index + by;
    if (!event.altKey || !by || to < 0 || to >= score.tracks.length) return;
    event.preventDefault();
    onmove(index, to);
    tick().then(() => scroller.querySelectorAll<HTMLElement>('.name')[to]?.focus());
  }

  // --- Resizing ---

  const limit = () => Math.max(MIN_HEIGHT, window.innerHeight * 0.6);

  function startResize(event: PointerEvent) {
    const grip = event.currentTarget as HTMLElement;
    const startY = event.clientY;
    const startHeight = pane.offsetHeight;
    grip.setPointerCapture(event.pointerId);
    const move = (e: PointerEvent) => {
      onresize(Math.round(Math.max(MIN_HEIGHT, Math.min(limit(), startHeight + startY - e.clientY))));
    };
    const end = () => {
      grip.removeEventListener('pointermove', move);
      grip.removeEventListener('pointerup', end);
      grip.removeEventListener('pointercancel', end);
    };
    grip.addEventListener('pointermove', move);
    grip.addEventListener('pointerup', end);
    grip.addEventListener('pointercancel', end);
    event.preventDefault();
  }

  function resizeKey(event: KeyboardEvent) {
    const step = event.key === 'ArrowUp' ? ROW : event.key === 'ArrowDown' ? -ROW : 0;
    if (!step) return;
    event.preventDefault();
    onresize(Math.max(MIN_HEIGHT, Math.min(limit(), pane.offsetHeight + step)));
  }
</script>

{#snippet controls(index: number, name: string, mix: Mix)}
  <button
    type="button"
    class="flag mute"
    aria-label="Mute {name}"
    aria-pressed={mix.mute}
    use:tip={{ label: 'Mute' }}
    onmousedown={(e) => e.preventDefault()}
    onclick={() => setMix(index, { ...mix, mute: !mix.mute })}
  >M</button>
  <button
    type="button"
    class="flag solo"
    aria-label="Solo {name}"
    aria-pressed={mix.solo}
    use:tip={{ label: 'Solo' }}
    onmousedown={(e) => e.preventDefault()}
    onclick={() => setMix(index, { ...mix, solo: !mix.solo })}
  >S</button>
  <!-- From the keyboard the left and right arrows reach the sliders like the
       buttons beside them, and the up and down arrows set them. -->
  <input
    class="volume"
    type="range"
    min="0"
    max="100"
    aria-label="Volume of {name}"
    value={mix.volume}
    use:tip={{ label: `Volume ${mix.volume}`, keys: '↑ / ↓' }}
    onpointerdown={() => (pointing = true)}
    onkeydown={() => (pointing = false)}
    oninput={(e) => slide(index, 'volume', e, false)}
    onchange={(e) => slide(index, 'volume', e, true)}
  />
  <input
    class="pan"
    type="range"
    min="-100"
    max="100"
    step="5"
    aria-label="Pan of {name}"
    aria-valuetext={mix.pan === 0 ? 'centre' : `${Math.abs(mix.pan)} ${mix.pan < 0 ? 'left' : 'right'}`}
    value={mix.pan}
    style:width="{PAN_WIDTH}px"
    style:--from="{Math.min(panAt(0), panAt(mix.pan))}px"
    style:--to="{Math.max(panAt(0), panAt(mix.pan))}px"
    use:tip={{
      label: mix.pan === 0 ? 'Pan: centre' : `Pan: ${Math.abs(mix.pan)} ${mix.pan < 0 ? 'left' : 'right'}`,
      keys: '↑ / ↓',
    }}
    onpointerdown={() => (pointing = true)}
    onkeydown={() => (pointing = false)}
    oninput={(e) => slide(index, 'pan', e, false)}
    onchange={(e) => slide(index, 'pan', e, true)}
    ondblclick={() => setMix(index, { ...mix, pan: 0 })}
  />
{/snippet}

<section
  class="tracks"
  aria-label="Tracks"
  bind:this={pane}
  style:height="{shown}px"
  style:--cell="{CELL}px"
  style:--row="{ROW}px"
  style:--head="{HEAD}px"
>
  <!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
  <div
    class="grip"
    role="separator"
    aria-orientation="horizontal"
    aria-label="Resize the track pane"
    aria-valuenow={shown}
    aria-valuemin={MIN_HEIGHT}
    aria-valuemax={Math.round(limit())}
    aria-valuetext="{shown} pixels high"
    tabindex="0"
    use:tip={{ label: 'Drag to resize; double-click to fit the tracks' }}
    onpointerdown={startResize}
    ondblclick={() => onresize(null)}
    onkeydown={resizeKey}
  ></div>

  <div class="scroller" bind:this={scroller} use:roving>
    <div class="head" style:width="{infoWidth + barCount * CELL}px">
      <div class="corner" bind:offsetWidth={infoWidth}>
        <span class="title">Tracks</span>
        {#each commands as command}
          <ToolButton {command} size={16} />
        {/each}
      </div>
      <div class="ruler" aria-hidden="true">
        {#each numbers as index}
          <span style:left="{index * CELL}px">{index + 1}</span>
        {/each}
        {#each sections as section}
          <span class="section" style:left="{section.index * CELL}px">{section.name}</span>
        {/each}
      </div>
    </div>

    {#each score.tracks as t, index}
      {@const mix = mixOf(index)}
      {@const silent = mix.mute || (anySolo && !mix.solo)}
      <div
        class="row"
        class:current={index === track}
        class:silent
        class:moving={moving?.from === index}
        class:before={moving?.to === index && moving.to < moving.from}
        class:after={moving?.to === index && moving.to > moving.from}
      >
        <div class="info">
          <button
            type="button"
            class="name"
            aria-current={index === track ? 'true' : undefined}
            use:tip={{
              label: 'Edit this track; double-click for its settings; drag to move it',
              keys: 'Alt+↑ / Alt+↓ to move',
            }}
            onmousedown={(e) => e.preventDefault()}
            onpointerdown={(e) => startMove(e, index)}
            onclick={() => pickName(index)}
            ondblclick={() => onproperties(index)}
            onkeydown={(e) => moveKey(e, index)}
          >
            <Icon name={INSTRUMENT_ICONS[t.instrument]} size={16} />
            <span>{t.name || `Track ${index + 1}`}</span>
          </button>
          {@render controls(index, t.name, mix)}
        </div>
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div class="cells" style:width="{barCount * CELL}px" onpointerdown={(e) => pressBars(e, index)}>
          {#each runs[index] as [first, length]}
            <span class="run" style:left="{first * CELL}px" style:width="{length * CELL - 1}px"></span>
          {/each}
          {#if index === track && selection}
            <span
              class="selected"
              style:left="{selection[0] * CELL}px"
              style:width="{(selection[1] - selection[0] + 1) * CELL}px"
            ></span>
          {/if}
          {#if index === track}
            <span class="at" style:left="{bar * CELL}px"></span>
          {/if}
        </div>
      </div>
    {/each}

    {#if audio}
      {@const mix = mixOf(AUDIO)}
      <div class="row audio" class:silent={mix.mute || (anySolo && !mix.solo)}>
        <div class="info">
          <button
            type="button"
            class="name"
            aria-pressed={audio.shown}
            use:tip={{ label: audio.shown ? "Hide the audio track's waveform" : "Show the audio track's waveform" }}
            onmousedown={(e) => e.preventDefault()}
            onclick={onaudio}
          >
            <Icon name="audioTrack" size={16} />
            <span>{audio.name}</span>
          </button>
          {@render controls(AUDIO, audio.name, mix)}
        </div>
        <div class="cells" style:width="{barCount * CELL}px" aria-hidden="true">
          {#each audioRuns as [first, length]}
            <span class="run" style:left="{first * CELL}px" style:width="{length * CELL - 1}px"></span>
          {/each}
        </div>
      </div>
    {/if}

    {#if playBar !== null}
      <span
        class="playhead"
        style:left="{infoWidth + playBar * CELL}px"
        style:height="{HEAD + rows * ROW}px"
      ></span>
    {/if}
  </div>
</section>

<style>
  .tracks {
    position: relative;
    container-type: inline-size;
    display: flex;
    flex: none;
    flex-direction: column;
    border-top: 1px solid var(--border);
    background: var(--panel);
    font-size: var(--text-sm);
  }

  /* The top edge is the handle for resizing the pane. */
  .grip {
    position: absolute;
    z-index: 4;
    top: -3px;
    right: 0;
    left: 0;
    height: 6px;
    cursor: ns-resize;
    touch-action: none;
  }

  .grip:hover,
  .grip:focus-visible {
    outline: none;
    background: var(--focus);
    opacity: 0.6;
  }

  .scroller {
    position: relative;
    flex: 1;
    min-height: 0;
    overflow: auto;
    scrollbar-width: thin;
  }

  .head,
  .row {
    display: flex;
    min-width: 100%;
  }

  .head {
    position: sticky;
    z-index: 3;
    top: 0;
    height: var(--head);
    border-bottom: 1px solid var(--border);
    background: var(--panel);
  }

  .corner,
  .info {
    position: sticky;
    z-index: 2;
    left: 0;
    display: flex;
    flex: none;
    align-items: center;
    gap: var(--space-1);
    padding: 0 var(--space-3);
    border-right: 1px solid var(--border);
    background: var(--panel);
  }

  .corner :global(.tool) {
    width: 24px;
    height: 24px;
  }

  .title {
    flex: 1;
    color: var(--muted);
    font-size: var(--text-xs);
    font-weight: 600;
    letter-spacing: 0.04em;
    text-transform: uppercase;
  }

  .ruler {
    position: relative;
    flex: 1;
    color: var(--muted);
    font-size: var(--text-xs);
    font-variant-numeric: tabular-nums;
  }

  .ruler span {
    position: absolute;
    top: 0;
    bottom: 0;
    display: flex;
    align-items: center;
    padding-left: 3px;
    border-left: 1px solid var(--border);
  }

  /* A section name covers the bar numbers it starts at. */
  .ruler .section {
    z-index: 1;
    padding-right: 4px;
    border-left: 2px solid var(--accent);
    background: var(--panel);
    color: var(--text);
    font-weight: 600;
    white-space: nowrap;
  }

  .row {
    height: var(--row);
    border-bottom: 1px solid var(--border);
  }

  /* A track being dragged fades, and a line shows where it will go. */
  .row.moving {
    opacity: 0.5;
  }

  .row.moving .name {
    cursor: grabbing;
  }

  .row.before {
    box-shadow: inset 0 2px 0 var(--focus);
  }

  .row.after {
    box-shadow: inset 0 -2px 0 var(--focus);
  }

  /* The corner sets the width of the names column, so the rows follow it. */
  .info {
    width: 300px;
  }

  .corner {
    width: 300px;
  }

  .row.current .info {
    background: color-mix(in srgb, var(--panel), var(--accent) 12%);
    box-shadow: inset 3px 0 0 var(--accent);
  }

  .name {
    display: flex;
    flex: 1;
    align-items: center;
    gap: var(--space-3);
    min-width: 0;
    height: 24px;
    padding: 0 var(--space-2);
    border: 0;
    border-radius: var(--radius-s);
    background: none;
    text-align: left;
    cursor: pointer;
  }

  .name:hover {
    background: var(--hover);
  }

  .name span {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .row.current .name {
    font-weight: 600;
  }

  .row.silent .name {
    color: var(--muted);
  }

  .flag {
    flex: none;
    width: 22px;
    height: 22px;
    padding: 0;
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-s);
    background: var(--raised);
    font-size: var(--text-xs);
    font-weight: 700;
    cursor: pointer;
  }

  .flag:hover {
    background: color-mix(in srgb, var(--raised), var(--text) 10%);
  }

  .flag.mute[aria-pressed='true'] {
    border-color: var(--error);
    background: var(--error);
    color: var(--on-accent);
  }

  .flag.solo[aria-pressed='true'] {
    border-color: var(--accent);
    background: var(--accent);
    color: var(--on-accent);
  }

  input[type='range'] {
    flex: none;
    height: 22px;
    margin: 0 0 0 var(--space-2);
    padding: 0;
    border: 0;
    background: none;
    accent-color: var(--accent);
    cursor: pointer;
  }

  .volume {
    width: 72px;
  }

  /* Pan is a distance from the middle, so the control fills from the middle outwards
     and marks the middle with a tick. */
  input.pan {
    --track: linear-gradient(
      to right,
      var(--border-strong) 0 var(--from),
      var(--accent) var(--from) var(--to),
      var(--border-strong) var(--to) 100%
    );
    appearance: none;
    background: linear-gradient(var(--muted), var(--muted)) center / 1px 12px no-repeat;
  }

  input.pan::-webkit-slider-runnable-track {
    height: 4px;
    border-radius: 2px;
    background: var(--track);
  }

  input.pan::-moz-range-track {
    height: 4px;
    border-radius: 2px;
    background: var(--track);
  }

  input.pan::-webkit-slider-thumb {
    appearance: none;
    width: 8px;
    height: 14px;
    margin-top: -5px;
    border: 0;
    border-radius: 3px;
    background: var(--text);
  }

  input.pan::-moz-range-thumb {
    width: 8px;
    height: 14px;
    border: 0;
    border-radius: 3px;
    background: var(--text);
  }

  .cells {
    position: relative;
    flex: none;
    /* A faint line at every bar and a stronger one every fourth. */
    background:
      repeating-linear-gradient(
        to right,
        var(--border) 0 1px,
        transparent 1px calc(var(--cell) * 4)
      ),
      repeating-linear-gradient(
        to right,
        color-mix(in srgb, var(--border), transparent 55%) 0 1px,
        transparent 1px var(--cell)
      );
    cursor: pointer;
    /* A drag along the bars selects them rather than scrolling the grid. */
    touch-action: pan-y;
  }

  .run {
    position: absolute;
    top: 5px;
    bottom: 5px;
    margin-left: 1px;
    border-radius: 3px;
    background: color-mix(in srgb, var(--muted), transparent 35%);
  }

  .row.current .run {
    background: var(--accent);
  }

  .row.audio .run {
    background: repeating-linear-gradient(
      90deg,
      color-mix(in srgb, var(--muted), transparent 35%) 0 2px,
      transparent 2px 4px
    );
  }

  .row.silent .run {
    opacity: 0.45;
  }

  .selected {
    position: absolute;
    top: 0;
    bottom: 0;
    background: var(--select);
    pointer-events: none;
  }

  .at {
    position: absolute;
    top: 1px;
    bottom: 1px;
    width: calc(var(--cell) + 1px);
    border: 2px solid var(--focus);
    border-radius: 3px;
    pointer-events: none;
  }

  .playhead {
    position: absolute;
    z-index: 1;
    top: 0;
    width: var(--cell);
    background: var(--select);
    box-shadow: inset 2px 0 0 var(--accent);
    pointer-events: none;
  }

  /* Without room for the sliders, the names column keeps the name and the two switches. */
  @container (max-width: 640px) {
    .info,
    .corner {
      width: 190px;
    }

    input[type='range'],
    .corner .title {
      display: none;
    }
  }
</style>
