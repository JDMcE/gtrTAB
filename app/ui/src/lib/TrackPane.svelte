<script lang="ts">
  import type { Command } from './commands';
  import { INSTRUMENT_ICONS, type Mix, type Score } from './model';
  import Icon from './ui/Icon.svelte';
  import ToolButton from './ui/ToolButton.svelte';
  import { roving } from './ui/roving';
  import { tip } from './ui/tooltip';

  interface Props {
    score: Score;
    /** The track being edited. */
    track: number;
    /** The cursor's bar. */
    bar: number;
    /** The bar sounding now, while playing. */
    playBar: number | null;
    /** The first bar visible in the score, which the grid keeps in view. */
    viewBar: number;
    /** Height in pixels, or `null` to fit the tracks. */
    height: number | null;
    /** Track actions shown above the names; they act on the current track. */
    commands: Command[];
    onpick: (track: number, bar: number | null) => void;
    onmix: (track: number, mix: Mix) => void;
    onproperties: (track: number) => void;
    onresize: (height: number | null) => void;
  }

  let {
    score,
    track,
    bar,
    playBar,
    viewBar,
    height,
    commands,
    onpick,
    onmix,
    onproperties,
    onresize,
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
  /** Slider positions while they are being dragged, before the change is made. */
  let dragging = $state<{ track: number; mix: Mix } | null>(null);

  const barCount = $derived(score.master_bars.length);
  const fitted = $derived(HEAD + score.tracks.length * ROW + 14);

  /** For each track, the runs of consecutive bars that hold notes, as `[first, length]`. */
  const runs = $derived(
    score.tracks.map((t) => {
      const found: [number, number][] = [];
      for (let i = 0; i < barCount; i++) {
        const filled = t.staves.some((s) =>
          s.bars[i]?.voices.some((v) => v.beats.some((b) => b.notes.length > 0)),
        );
        if (!filled) continue;
        const last = found[found.length - 1];
        if (last && last[0] + last[1] === i) last[1]++;
        else found.push([i, 1]);
      }
      return found;
    }),
  );

  const numbers = $derived(
    Array.from({ length: Math.ceil(barCount / NUMBER_EVERY) }, (_, i) => i * NUMBER_EVERY),
  );
  const sections = $derived(
    score.master_bars.flatMap((m, index) => (m.section ? [{ index, name: m.section }] : [])),
  );
  const anySolo = $derived(score.tracks.some((t) => t.mix.solo));

  const mixOf = (index: number): Mix =>
    dragging?.track === index ? dragging.mix : score.tracks[index].mix;

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

  function pickBar(event: MouseEvent, index: number) {
    const cells = event.currentTarget as HTMLElement;
    const x = event.clientX - cells.getBoundingClientRect().left;
    onpick(index, Math.max(0, Math.min(barCount - 1, Math.floor(x / CELL))));
  }

  function slide(index: number, field: 'volume' | 'pan', event: Event, commit: boolean) {
    const input = event.currentTarget as HTMLInputElement;
    const mix = { ...mixOf(index), [field]: Number(input.value) };
    if (commit) {
      dragging = null;
      onmix(index, mix);
      input.blur();
    } else {
      dragging = { track: index, mix };
    }
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

<section
  class="tracks"
  aria-label="Tracks"
  bind:this={pane}
  style:height="{height ?? Math.min(fitted, 190)}px"
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
      <div class="row" class:current={index === track} class:silent>
        <div class="info">
          <button
            type="button"
            class="name"
            aria-current={index === track ? 'true' : undefined}
            use:tip={{ label: 'Edit this track; double-click for its settings' }}
            onmousedown={(e) => e.preventDefault()}
            onclick={() => onpick(index, null)}
            ondblclick={() => onproperties(index)}
          >
            <Icon name={INSTRUMENT_ICONS[t.instrument]} size={16} />
            <span>{t.name || `Track ${index + 1}`}</span>
          </button>
          <button
            type="button"
            class="flag mute"
            aria-label="Mute {t.name}"
            aria-pressed={mix.mute}
            use:tip={{ label: 'Mute' }}
            onmousedown={(e) => e.preventDefault()}
            onclick={() => onmix(index, { ...mix, mute: !mix.mute })}
          >M</button>
          <button
            type="button"
            class="flag solo"
            aria-label="Solo {t.name}"
            aria-pressed={mix.solo}
            use:tip={{ label: 'Solo' }}
            onmousedown={(e) => e.preventDefault()}
            onclick={() => onmix(index, { ...mix, solo: !mix.solo })}
          >S</button>
          <!-- The sliders are for the mouse; the track's settings hold the same values for
               the keyboard. -->
          <input
            class="volume"
            type="range"
            min="0"
            max="100"
            tabindex="-1"
            aria-label="Volume of {t.name}"
            value={mix.volume}
            use:tip={{ label: `Volume ${mix.volume}` }}
            oninput={(e) => slide(index, 'volume', e, false)}
            onchange={(e) => slide(index, 'volume', e, true)}
          />
          <input
            class="pan"
            type="range"
            min="-100"
            max="100"
            step="5"
            tabindex="-1"
            aria-label="Pan of {t.name}"
            value={mix.pan}
            style:width="{PAN_WIDTH}px"
            style:--from="{Math.min(panAt(0), panAt(mix.pan))}px"
            style:--to="{Math.max(panAt(0), panAt(mix.pan))}px"
            use:tip={{
              label: mix.pan === 0 ? 'Pan: centre' : `Pan: ${Math.abs(mix.pan)} ${mix.pan < 0 ? 'left' : 'right'}`,
            }}
            oninput={(e) => slide(index, 'pan', e, false)}
            onchange={(e) => slide(index, 'pan', e, true)}
            ondblclick={() => onmix(index, { ...mix, pan: 0 })}
          />
        </div>
        <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
        <div class="cells" style:width="{barCount * CELL}px" onclick={(e) => pickBar(e, index)}>
          {#each runs[index] as [first, length]}
            <span class="run" style:left="{first * CELL}px" style:width="{length * CELL - 1}px"></span>
          {/each}
          {#if index === track}
            <span class="at" style:left="{bar * CELL}px"></span>
          {/if}
        </div>
      </div>
    {/each}

    {#if playBar !== null}
      <span
        class="playhead"
        style:left="{infoWidth + playBar * CELL}px"
        style:height="{HEAD + score.tracks.length * ROW}px"
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

  .row.silent .run {
    opacity: 0.45;
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
