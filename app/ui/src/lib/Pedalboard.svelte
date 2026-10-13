<script lang="ts">
  // The pedalboard: the chain of pedals the cursor's track goes through, or the one the
  // whole mix goes through, laid out from left to right in the order the sound meets them.
  // Pedals are added, switched on and off, moved and taken away, their knobs turned, and a
  // chain can be put on from a preset or saved as one. A turn of a knob is heard at once
  // and kept, as one step to undo, when it is let go.
  import type { Pedal } from './model';
  import {
    MAX_CHAIN,
    addPedal,
    knobValue,
    movePedal,
    sameChain,
    withKnob,
    type PedalPreset,
    type PedalSpec,
  } from './pedals';
  import Icon from './ui/Icon.svelte';
  import Knob from './ui/Knob.svelte';

  interface Props {
    specs: PedalSpec[];
    /** The presets the app comes with, and those the user has saved. */
    presets: PedalPreset[];
    userPresets: PedalPreset[];
    master: boolean;
    trackName: string;
    chain: Pedal[];
    /** Shows the track's chain, or with `true` the mix's. */
    onchoose: (master: boolean) => void;
    /** The chain as it is while a knob turns, to be heard but not yet kept. */
    onpreview: (chain: Pedal[]) => void;
    /** The chain to keep. */
    onset: (chain: Pedal[]) => void;
    onsavepreset: () => void;
    ondeletepreset: (preset: PedalPreset) => void;
    onclose: () => void;
  }

  let {
    specs,
    presets,
    userPresets,
    master,
    trackName,
    chain,
    onchoose,
    onpreview,
    onset,
    onsavepreset,
    ondeletepreset,
    onclose,
  }: Props = $props();

  /** The chain while a knob is being turned, until it is kept. */
  let draft = $state<Pedal[] | null>(null);
  $effect(() => {
    void chain;
    draft = null;
  });
  const shown = $derived(draft ?? chain);

  const specOf = (pedal: Pedal) => specs.find((s) => s.kind === pedal.kind);
  const fits = (preset: PedalPreset) => preset.master === master;
  /** The user's preset the chain is, if it is one, which can then be deleted. */
  const userPreset = $derived(userPresets.find((p) => fits(p) && sameChain(p.pedals, shown, specs)));

  let added = $state<number | null>(null);
  let board = $state<HTMLElement>();

  function add(event: Event) {
    const select = event.currentTarget as HTMLSelectElement;
    const spec = specs.find((s) => s.kind === select.value);
    select.value = '';
    if (!spec || shown.length >= MAX_CHAIN) return;
    const [next, at] = addPedal(shown, spec, specs);
    added = at;
    onset(next);
  }

  // A pedal just added is scrolled into view.
  $effect(() => {
    if (added === null || !board) return;
    board.querySelectorAll('.pedal')[added]?.scrollIntoView({ block: 'nearest', inline: 'nearest' });
    added = null;
  });

  function choosePreset(event: Event) {
    const select = event.currentTarget as HTMLSelectElement;
    const [group, index] = select.value.split(':');
    select.value = '';
    const preset = (group === 'own' ? userPresets : presets).filter(fits)[Number(index)];
    if (preset) onset(structuredClone($state.snapshot(preset.pedals)) as Pedal[]);
  }

  const replace = (index: number, pedal: Pedal) => shown.map((p, i) => (i === index ? pedal : p));

  function turn(index: number, knob: number, value: number) {
    const spec = specOf(shown[index]);
    if (!spec) return;
    draft = replace(index, withKnob(shown[index], spec, knob, value));
    onpreview(draft);
  }

  function keep() {
    if (draft) onset(draft);
  }
</script>

<section class="pedalboard" aria-label="Pedalboard">
  <header>
    <h2>Pedalboard</h2>
    <div class="target" role="group" aria-label="Effects of">
      <button type="button" class="seg" aria-pressed={!master} onclick={() => onchoose(false)}>{trackName}</button>
      <button type="button" class="seg" aria-pressed={master} onclick={() => onchoose(true)}>Master</button>
    </div>
    <span class="rule"></span>
    <select aria-label="Put on a preset" onchange={choosePreset}>
      <option value="">Preset…</option>
      <optgroup label="Built in">
        {#each presets.filter(fits) as preset, i}<option value="app:{i}">{preset.name}</option>{/each}
      </optgroup>
      {#if userPresets.some(fits)}
        <optgroup label="Saved">
          {#each userPresets.filter(fits) as preset, i}<option value="own:{i}">{preset.name}</option>{/each}
        </optgroup>
      {/if}
    </select>
    <button type="button" class="tool" aria-label="Save as a preset" title="Save as a preset" onclick={onsavepreset}>
      <Icon name="save" size={16} />
    </button>
    {#if userPreset}
      <button
        type="button"
        class="tool"
        aria-label="Delete the preset {userPreset.name}"
        title="Delete the preset {userPreset.name}"
        onclick={() => userPreset && ondeletepreset(userPreset)}
      >
        <Icon name="removeTrack" size={16} />
      </button>
    {/if}
    <select aria-label="Add a pedal" onchange={add} disabled={shown.length >= MAX_CHAIN}>
      <option value="">Add pedal…</option>
      {#each specs as spec}<option value={spec.kind}>{spec.name}</option>{/each}
    </select>
    <button type="button" class="tool close" aria-label="Close the pedalboard" onclick={onclose}>
      <Icon name="remove" size={14} />
    </button>
  </header>
  <div class="scroller" bind:this={board}>
    {#if shown.length === 0}
      <p class="empty">
        No pedals: {master ? 'the mix is heard' : `${trackName} goes to the mix`} as it is. Add a pedal, or put on a preset.
      </p>
    {/if}
    <ol class="chain" aria-label="{master ? 'The mix' : trackName}: pedals in the order the sound goes through them">
      {#each shown as pedal, index (index)}
        {@const spec = specOf(pedal)}
        {@const on = pedal.on !== false}
        {@const name = spec?.name ?? pedal.kind}
        <li class="pedal {pedal.kind}" class:off={!on}>
          <div class="top">
            <button
              type="button"
              class="switch"
              aria-pressed={on}
              aria-label="{name} on"
              title={on ? 'Switch off' : 'Switch on'}
              onclick={() => onset(replace(index, { ...pedal, on: !on }))}
            >
              <span class="led"></span>
            </button>
            <h3>{name}</h3>
            <button
              type="button"
              class="tool small"
              aria-label="Move {name} earlier"
              aria-disabled={index === 0}
              onclick={() => index > 0 && onset(movePedal(shown, index, index - 1))}
            >
              <Icon name="chevronLeft" size={14} />
            </button>
            <button
              type="button"
              class="tool small"
              aria-label="Move {name} later"
              aria-disabled={index === shown.length - 1}
              onclick={() => index < shown.length - 1 && onset(movePedal(shown, index, index + 1))}
            >
              <Icon name="chevronRight" size={14} />
            </button>
            <button
              type="button"
              class="tool small"
              aria-label="Remove {name}"
              onclick={() => onset(shown.filter((_, i) => i !== index))}
            >
              <Icon name="remove" size={14} />
            </button>
          </div>
          {#if spec}
            <div class="knobs">
              {#each spec.knobs as knob, k}
                <Knob
                  label={knob.name}
                  value={knobValue(pedal, spec, k)}
                  reset={knob.default}
                  oninput={(value) => turn(index, k, value)}
                  onchange={keep}
                />
              {/each}
            </div>
          {/if}
        </li>
      {/each}
    </ol>
  </div>
</section>

<style>
  .pedalboard {
    flex: none;
    border-top: 1px solid var(--border);
    background: var(--panel);
  }

  header {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-2);
    min-height: 34px;
    padding: var(--space-1) var(--space-2) var(--space-1) var(--space-4);
  }

  h2 {
    margin: 0 var(--space-2) 0 0;
    font-size: var(--text-sm);
    font-weight: 600;
  }

  .target {
    display: flex;
    min-width: 0;
  }

  .seg {
    max-width: 160px;
    height: 24px;
    padding: 0 var(--space-3);
    overflow: hidden;
    border: 1px solid var(--border-strong);
    background: var(--raised);
    font-size: var(--text-sm);
    text-overflow: ellipsis;
    white-space: nowrap;
    cursor: pointer;
  }

  .seg:first-child {
    border-radius: var(--radius-s) 0 0 var(--radius-s);
  }

  .seg:last-child {
    margin-left: -1px;
    border-radius: 0 var(--radius-s) var(--radius-s) 0;
  }

  .seg[aria-pressed='true'] {
    position: relative;
    border-color: var(--accent);
    background: var(--pressed);
    color: var(--accent);
  }

  .rule {
    flex: 1;
  }

  select {
    height: 26px;
    font-size: var(--text-sm);
  }

  header .tool {
    width: 26px;
    height: 26px;
  }

  .scroller {
    overflow-x: auto;
    padding: 0 var(--space-4) var(--space-3);
  }

  .empty {
    margin: var(--space-3) 0;
    color: var(--muted);
    font-size: var(--text-sm);
  }

  .chain {
    display: flex;
    gap: var(--space-3);
    width: max-content;
    margin: 0;
    padding: 0;
    list-style: none;
  }

  /* Each kind of pedal has a colour of its own, as pedals do. */
  .pedal {
    --hue: 30;
    display: grid;
    align-content: start;
    gap: var(--space-2);
    padding: var(--space-2) var(--space-3) var(--space-3);
    border: 1px solid oklch(from var(--accent) l c var(--hue) / 55%);
    border-top: 4px solid oklch(from var(--accent) l c var(--hue));
    border-radius: var(--radius-m);
    background: color-mix(in oklch, var(--raised), oklch(from var(--accent) l c var(--hue)) 7%);
  }

  .pedal.compressor { --hue: 230; }
  .pedal.wah { --hue: 300; }
  .pedal.overdrive { --hue: 145; }
  .pedal.distortion { --hue: 25; }
  .pedal.amp { --hue: 60; }
  .pedal.cabinet { --hue: 40; }
  .pedal.eq { --hue: 200; }
  .pedal.chorus { --hue: 180; }
  .pedal.delay { --hue: 260; }
  .pedal.reverb { --hue: 280; }

  .pedal.off {
    opacity: 0.6;
  }

  .top {
    display: flex;
    align-items: center;
    gap: var(--space-1);
  }

  h3 {
    flex: 1;
    margin: 0 var(--space-3) 0 var(--space-2);
    font-size: var(--text-sm);
    font-weight: 600;
    white-space: nowrap;
  }

  .tool.small {
    width: 22px;
    height: 22px;
    color: var(--muted);
  }

  .switch {
    display: grid;
    place-items: center;
    width: 22px;
    height: 22px;
    padding: 0;
    border: 1px solid var(--border-strong);
    border-radius: 50%;
    background: var(--panel);
    cursor: pointer;
  }

  .led {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--border);
  }

  .switch[aria-pressed='true'] .led {
    background: var(--error);
    box-shadow: 0 0 6px var(--error);
  }

  .knobs {
    display: flex;
    gap: var(--space-1);
  }
</style>
