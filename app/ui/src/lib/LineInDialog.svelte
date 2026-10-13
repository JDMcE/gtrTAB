<script lang="ts" module>
  /** The input and its gain, kept for next time. */
  export interface LineSettings {
    /** The input's id, or `''` for the system's default. */
    device: string;
    /** Decibels the line is brought up or down by before the pedals. */
    gain: number;
  }

  const SETTINGS_KEY = 'gtrtab.lineIn';
  export const MIN_GAIN = -12;
  export const MAX_GAIN = 24;

  export function loadLineSettings(): LineSettings {
    try {
      const saved = JSON.parse(localStorage.getItem(SETTINGS_KEY) ?? '{}');
      const gain = Number(saved.gain);
      return {
        device: typeof saved.device === 'string' ? saved.device : '',
        gain: gain >= MIN_GAIN && gain <= MAX_GAIN ? gain : 0,
      };
    } catch {
      return { device: '', gain: 0 };
    }
  }

  export function saveLineSettings(settings: LineSettings): void {
    try {
      localStorage.setItem(SETTINGS_KEY, JSON.stringify(settings));
    } catch {
      // Kept for the session only.
    }
  }
</script>

<script lang="ts">
  // The line in: a guitar plugged into an input, or played into a microphone, heard through
  // the pedals of the cursor's track and of the mix. It carries on once this is closed, so
  // that the pedals can be set while it is played, until it is stopped.
  import { onMount, untrack } from 'svelte';
  import type { Levels } from './linein';
  import Dialog from './ui/Dialog.svelte';

  interface Props {
    running: boolean;
    trackName: string;
    /** The names of the pedals the line goes through: the track's, then the mix's. */
    trackPedals: string[];
    masterPedals: string[];
    levels: Levels;
    /** Seconds from a string to the speakers, as far as is known. */
    latency: number;
    settings: LineSettings;
    /** Starts the line on the settings; resolves to what went wrong, or `null`. */
    onstart: (settings: LineSettings) => Promise<string | null>;
    ongain: (gain: number) => void;
    onstop: () => void;
    onpedals: () => void;
    onclose: () => void;
  }

  let {
    running,
    trackName,
    trackPedals,
    masterPedals,
    levels,
    latency,
    settings,
    onstart,
    ongain,
    onstop,
    onpedals,
    onclose,
  }: Props = $props();

  let inputs = $state<MediaDeviceInfo[]>([]);
  let device = $state(untrack(() => settings.device));
  let gain = $state(untrack(() => settings.gain));
  let problem = $state('');
  let starting = $state(false);

  async function listInputs() {
    try {
      inputs = (await navigator.mediaDevices.enumerateDevices()).filter((d) => d.kind === 'audioinput');
    } catch {
      inputs = [];
    }
  }

  onMount(() => void listInputs());

  async function start() {
    starting = true;
    problem = (await onstart({ device, gain })) ?? '';
    starting = false;
    // The names of the inputs are only given once one has been allowed.
    if (!problem) await listInputs();
  }

  function changeGain() {
    gain = Math.max(MIN_GAIN, Math.min(MAX_GAIN, Math.round(gain)));
    ongain(gain);
  }

  /** A level as decibels under full scale, for the meters. */
  const decibels = (level: number) => (level > 1e-5 ? Math.max(-60, 20 * Math.log10(level)) : -60);
  const chain = (names: string[]) => (names.length ? names.join(' → ') : 'no pedals');
</script>

<Dialog
  title="Line in"
  width="440px"
  hint="Plays a guitar plugged into an input, or played into a microphone, through the pedals of the track at the cursor and then those of the mix. Use headphones: a microphone near the speakers will howl. It carries on once this is closed, until it is stopped here or from its button in the transport bar."
  {onclose}
  onsubmit={() => (running ? onclose() : void start())}
>
  <label>
    Input
    <select bind:value={device} disabled={running}>
      <option value="">The system's default</option>
      {#each inputs as input (input.deviceId)}
        {#if input.deviceId && input.deviceId !== 'default'}
          <option value={input.deviceId}>{input.label || 'An input without a name'}</option>
        {/if}
      {/each}
    </select>
  </label>
  <label>
    Gain (dB)
    <input type="number" min={MIN_GAIN} max={MAX_GAIN} bind:value={gain} onchange={changeGain} />
  </label>

  <dl class="chain">
    <dt>{trackName}</dt>
    <dd data-track-pedals>{chain(trackPedals)}</dd>
    <dt>The mix</dt>
    <dd data-master-pedals>{chain(masterPedals)}</dd>
  </dl>
  <button type="button" class="btn small" onclick={onpedals}>Show the pedalboard</button>

  {#if problem}
    <p class="problem" role="alert">{problem}</p>
  {:else if running}
    <div class="meters" data-input={levels.input.toFixed(4)} data-output={levels.output.toFixed(4)}>
      <label>
        In
        <meter min="-60" max="0" low={-12} high={-3} optimum={-18} value={decibels(levels.input)}></meter>
      </label>
      <label>
        Out
        <meter min="-60" max="0" low={-12} high={-3} optimum={-18} value={decibels(levels.output)}></meter>
      </label>
    </div>
    <p class="latency" data-latency>
      {latency > 0 ? `Heard about ${Math.round(latency * 1000)} ms after it is played.` : 'How late it is heard is not known.'}
    </p>
  {/if}

  {#snippet footer()}
    {#if running}
      <button type="button" class="btn" onclick={onstop}>Stop</button>
      <button type="submit" class="btn primary">Close</button>
    {:else}
      <button type="button" class="btn" onclick={onclose}>Cancel</button>
      <button type="submit" class="btn primary" disabled={starting}>Start</button>
    {/if}
  {/snippet}
</Dialog>

<style>
  .chain {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: var(--space-2) var(--space-4);
    margin: var(--space-2) 0 var(--space-3);
    font-size: var(--text-sm);
  }

  dt {
    color: var(--muted);
  }

  dd {
    margin: 0;
  }

  .small {
    height: 26px;
    font-size: var(--text-sm);
  }

  .meters {
    display: grid;
    gap: var(--space-2);
    margin-top: var(--space-4);
  }

  .meters label {
    display: grid;
    grid-template-columns: 32px 1fr;
    align-items: center;
    margin: 0;
  }

  meter {
    width: 100%;
  }

  .latency {
    color: var(--muted);
    font-size: var(--text-sm);
  }

  .problem {
    color: var(--error);
  }
</style>
