<script lang="ts">
  // The tuner: listens to a microphone or a line in, and says which note it hears and how
  // far sharp or flat it is, of the nearest note or of the nearest string of the track.
  import { onMount, untrack } from 'svelte';
  import { pitchName } from './instrument';
  import { detectPitch, median, nearestNote, nearestString } from './tuner';
  import Dialog from './ui/Dialog.svelte';

  interface Props {
    /** Open strings of the track as they sound, with the capo on, from the highest. */
    strings: number[];
    onclose: () => void;
  }

  let { strings, onclose }: Props = $props();

  /** Within this many cents of the note, it is in tune. */
  const IN_TUNE = 3;
  /** Readings taken a second, and how many of the last are kept to steady the needle. */
  const READINGS_PER_SECOND = 20;
  const KEPT = 5;
  const REFERENCE_KEY = 'gtrtab.tuner.reference';

  let inputs = $state<MediaDeviceInfo[]>([]);
  let input = $state('');
  let reference = $state(readReference());
  let target = $state<'note' | 'string'>(untrack(() => (strings.length ? 'string' : 'note')));
  let problem = $state('');
  /** The frequency heard, steadied, or `null` while nothing is. */
  let frequency = $state<number | null>(null);
  /** What a screen reader is told, at most once a second. */
  let spoken = $state('');

  let context: AudioContext | undefined;
  let stream: MediaStream | undefined;
  let timer: ReturnType<typeof setInterval> | undefined;
  let lastSpoken = 0;

  function readReference(): number {
    try {
      const stored = Number(localStorage.getItem(REFERENCE_KEY));
      return stored >= 400 && stored <= 480 ? stored : 440;
    } catch {
      return 440;
    }
  }

  const name = (pitch: number) => `${pitchName(pitch)}${Math.floor(pitch / 12) - 1}`;

  /** What is heard, against the note or the string it is nearest. */
  const reading = $derived.by(() => {
    if (frequency === null) return null;
    const a4 = reference;
    const onString = target === 'string' ? nearestString(frequency, strings, a4) : null;
    if (onString) return { label: name(strings[onString.string]), string: onString.string, cents: onString.cents };
    const note = nearestNote(frequency, a4);
    return { label: name(note.pitch), string: null, cents: note.cents };
  });
  const tuned = $derived(reading !== null && Math.abs(reading.cents) <= IN_TUNE);
  const words = $derived(
    !reading ? 'Play a note'
    : tuned ? 'In tune'
    : `${Math.round(Math.abs(reading.cents))} cents ${reading.cents > 0 ? 'sharp' : 'flat'}`,
  );
  /** The needle, from -50 cents at the left to 50 at the right. */
  const needle = $derived(reading ? Math.max(-50, Math.min(50, reading.cents)) : 0);

  $effect(() => {
    try {
      localStorage.setItem(REFERENCE_KEY, String(reference));
    } catch {
      // Kept for the session only.
    }
  });

  async function listen(deviceId: string) {
    stopListening();
    problem = '';
    try {
      stream = await navigator.mediaDevices.getUserMedia({
        audio: {
          deviceId: deviceId ? { exact: deviceId } : undefined,
          // A tuner wants the sound as it is, not cleaned up for speech.
          echoCancellation: false,
          noiseSuppression: false,
          autoGainControl: false,
        },
      });
    } catch (error) {
      problem = `The input could not be opened: ${error instanceof Error ? error.message : String(error)}`;
      return;
    }
    // The names of the inputs are only given once one has been allowed.
    inputs = (await navigator.mediaDevices.enumerateDevices()).filter((d) => d.kind === 'audioinput');
    context = new AudioContext();
    const analyser = context.createAnalyser();
    analyser.fftSize = 4096;
    context.createMediaStreamSource(stream).connect(analyser);
    const samples = new Float32Array(analyser.fftSize);
    const recent: number[] = [];
    timer = setInterval(() => {
      if (!context) return;
      analyser.getFloatTimeDomainData(samples);
      const heard = detectPitch(samples, context.sampleRate);
      if (!heard || heard.clarity < 0.85) {
        recent.length = 0;
        frequency = null;
      } else {
        recent.push(heard.frequency);
        if (recent.length > KEPT) recent.shift();
        frequency = median(recent);
      }
      const now = performance.now();
      if (now - lastSpoken > 1000) {
        lastSpoken = now;
        spoken = reading ? `${reading.label}, ${words}` : '';
      }
    }, 1000 / READINGS_PER_SECOND);
  }

  function stopListening() {
    clearInterval(timer);
    stream?.getTracks().forEach((track) => track.stop());
    void context?.close();
    [stream, context, frequency] = [undefined, undefined, null];
  }

  onMount(() => {
    void listen('');
    return stopListening;
  });
</script>

<Dialog title="Tuner" width="460px" {onclose}>
  <div class="row">
    <label>
      Input
      <select bind:value={input} onchange={() => listen(input)}>
        <option value="">The system's default</option>
        {#each inputs as device (device.deviceId)}
          {#if device.deviceId && device.deviceId !== 'default'}
            <option value={device.deviceId}>{device.label || 'An input without a name'}</option>
          {/if}
        {/each}
      </select>
    </label>
    <label class="narrow">
      A4 (Hz)
      <input type="number" min="400" max="480" bind:value={reference} />
    </label>
  </div>

  {#if strings.length}
    <fieldset class="target">
      <legend>Tune to</legend>
      <label class="check"><input type="radio" bind:group={target} value="string" /> The track's strings</label>
      <label class="check"><input type="radio" bind:group={target} value="note" /> The nearest note</label>
    </fieldset>
  {/if}

  {#if problem}
    <p class="problem">{problem}</p>
  {:else}
    <div class="dial" class:tuned class:silent={!reading} data-cents={reading ? Math.round(reading.cents) : ''}>
      <div class="note" data-note>{reading?.label ?? '–'}</div>
      <div class="scale" aria-hidden="true">
        <span class="mark flat">−50</span>
        <span class="mark centre">0</span>
        <span class="mark sharp">+50</span>
        <span class="needle" style:left="{50 + needle}%"></span>
      </div>
      <div class="words" data-words>{words}</div>
      <div class="hz">{frequency ? `${frequency.toFixed(1)} Hz` : ''}</div>
    </div>
    {#if target === 'string' && strings.length}
      <ol class="strings" aria-label="Strings, from the lowest">
        {#each [...strings.keys()].reverse() as string}
          <li class:near={reading?.string === string} class:tuned={tuned && reading?.string === string}>
            {name(strings[string])}
          </li>
        {/each}
      </ol>
    {/if}
  {/if}
  <p class="sr-only" aria-live="polite">{spoken}</p>

  {#snippet footer()}
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
    min-width: 0;
  }

  .row .narrow {
    flex: none;
    width: 90px;
  }

  .target {
    display: flex;
    gap: var(--space-5);
    margin: 0 0 var(--space-4);
    padding: 0;
    border: 0;
  }

  .target legend {
    float: left;
    margin-right: var(--space-3);
    color: var(--muted);
    font-size: var(--text-sm);
  }

  .target label {
    margin-bottom: 0;
  }

  .dial {
    display: grid;
    justify-items: center;
    gap: var(--space-3);
    padding: var(--space-4) 0;
  }

  .note {
    font-size: 44px;
    font-weight: 700;
    line-height: 1;
    font-variant-numeric: tabular-nums;
  }

  .silent .note {
    color: var(--muted);
  }

  .tuned .note,
  .tuned .words {
    color: light-dark(#1d7a3a, #6fd08c);
  }

  .scale {
    position: relative;
    width: 100%;
    height: 34px;
    border-bottom: 2px solid var(--border-strong);
    background: linear-gradient(90deg, transparent 47%, light-dark(rgb(29 122 58 / 18%), rgb(111 208 140 / 22%)) 47% 53%, transparent 53%);
  }

  .mark {
    position: absolute;
    bottom: 2px;
    color: var(--muted);
    font-size: var(--text-xs);
  }

  .flat {
    left: 0;
  }

  .centre {
    left: 50%;
    transform: translateX(-50%);
  }

  .sharp {
    right: 0;
  }

  .needle {
    position: absolute;
    top: 0;
    bottom: -2px;
    width: 3px;
    margin-left: -1.5px;
    border-radius: 2px;
    background: var(--accent);
    transition: left 80ms linear;
  }

  .silent .needle {
    display: none;
  }

  .tuned .needle {
    background: light-dark(#1d7a3a, #6fd08c);
  }

  .words {
    font-size: var(--text-lg);
    font-weight: 600;
  }

  .hz {
    min-height: 1.2em;
    color: var(--muted);
    font-size: var(--text-sm);
    font-variant-numeric: tabular-nums;
  }

  .strings {
    display: flex;
    justify-content: center;
    gap: var(--space-2);
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .strings li {
    min-width: 40px;
    padding: var(--space-2) var(--space-3);
    border: 1px solid var(--border);
    border-radius: var(--radius-s);
    color: var(--muted);
    text-align: center;
  }

  .strings li.near {
    border-color: var(--accent);
    color: var(--text);
    font-weight: 600;
  }

  .strings li.tuned {
    border-color: light-dark(#1d7a3a, #6fd08c);
    background: light-dark(rgb(29 122 58 / 12%), rgb(111 208 140 / 16%));
  }

  .problem {
    margin: 0;
    color: var(--error);
  }
</style>
