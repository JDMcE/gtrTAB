<script lang="ts" module>
  /** Where a bar of the play-through starts in the recording, as `Document.recordingBarsJson` gives it. */
  export interface RecordingBar {
    bar: number;
    place: number;
    start: number;
    length: number;
  }

  /** How loud the recording is along its length: the lowest and highest sample of each stretch. */
  export interface Peaks {
    /** Stretches a second. */
    rate: number;
    low: Float32Array;
    high: Float32Array;
    /** Seconds the recording lasts. */
    duration: number;
  }

  /** The peaks of a recording, `rate` stretches a second, of its two sides together. */
  export function peaksOf(recording: AudioBuffer, rate = 200): Peaks {
    const left = recording.getChannelData(0);
    const right = recording.numberOfChannels > 1 ? recording.getChannelData(1) : left;
    const per = Math.max(1, Math.round(recording.sampleRate / rate));
    const count = Math.ceil(recording.length / per);
    const low = new Float32Array(count);
    const high = new Float32Array(count);
    for (let i = 0; i < count; i++) {
      let [min, max] = [0, 0];
      for (let s = i * per; s < Math.min(recording.length, (i + 1) * per); s++) {
        const sample = (left[s] + right[s]) / 2;
        if (sample < min) min = sample;
        if (sample > max) max = sample;
      }
      [low[i], high[i]] = [min, max];
    }
    return { rate: recording.sampleRate / per, low, high, duration: recording.duration };
  }

  /** Seconds as minutes, seconds and thousandths, such as 1:02.345. */
  export function clock(seconds: number): string {
    const sign = seconds < 0 ? '−' : '';
    const whole = Math.abs(seconds);
    const minutes = Math.floor(whole / 60);
    return `${sign}${minutes}:${(whole - minutes * 60).toFixed(3).padStart(6, '0')}`;
  }
</script>

<script lang="ts">
  // The audio track under the score: the recording drawn as its waveform, with a line where
  // each bar starts in it. A bar line dragged along the waveform says where that bar starts,
  // and the bars between it and the next bar so held are fitted evenly in between. A click
  // elsewhere goes to the bar there.
  import { onMount } from 'svelte';
  import type { AudioTrack, Mix } from './model';
  import Icon from './ui/Icon.svelte';

  interface Props {
    audio: AudioTrack;
    bars: RecordingBar[];
    /** The recording's waveform, or `null` while it is read or if its file is not at hand. */
    peaks: Peaks | null;
    /** Why there is no waveform, if there is none to come. */
    problem: string;
    /** The bar at the cursor. */
    cursorBar: number;
    /** Seconds into the recording now sounding, while it plays. */
    now: number | null;
    /** Bar `place` of the play-through starts at `millis` in the recording. */
    onsync: (place: number, millis: number) => void;
    /** Bar `place` is no longer held to a place in the recording. */
    onunsync: (place: number) => void;
    onseek: (bar: number) => void;
    onmix: (mix: Mix) => void;
    onreplace: () => void;
    onremove: () => void;
    onclose: () => void;
  }

  let { audio, bars, peaks, problem, cursorBar, now, onsync, onunsync, onseek, onmix, onreplace, onremove, onclose }: Props =
    $props();

  const HEIGHT = 96;
  /** Pixels a second at each step of zoom. */
  const ZOOMS = [10, 20, 40, 80, 160, 320];
  let zoom = $state(2);
  const scale = $derived(ZOOMS[zoom]);
  /** How near a bar line a press has to be to take hold of it. */
  const GRAB = 6;

  const held = $derived(new Set(audio.sync.map((p) => p.bar)));
  const duration = $derived(Math.max(peaks?.duration ?? 0, ...bars.map((b) => b.start + b.length), 1));
  const width = $derived(Math.ceil(duration * scale) + 40);

  let scroller = $state<HTMLElement>();
  let canvas = $state<HTMLCanvasElement>();
  let left = $state(0);
  let view = $state(0);
  /** A bar line being dragged, and where it has got to. */
  let dragging = $state<{ place: number; seconds: number; moved: boolean } | null>(null);

  /** The first place of the cursor's bar, which the buttons hold to the recording. */
  const cursorPlace = $derived(bars.find((b) => b.bar === cursorBar)?.place ?? null);
  const cursorHeld = $derived(cursorPlace !== null && held.has(cursorPlace));

  const x = (seconds: number) => seconds * scale - left + 20;
  const secondsAt = (clientX: number) => {
    const box = canvas!.getBoundingClientRect();
    return (clientX - box.left + left - 20) / scale;
  };

  function colour(name: string): string {
    return getComputedStyle(canvas!).getPropertyValue(name).trim() || '#888';
  }

  function draw() {
    if (!canvas || !scroller) return;
    const ratio = window.devicePixelRatio || 1;
    view = scroller.clientWidth;
    canvas.width = Math.round(view * ratio);
    canvas.height = Math.round(HEIGHT * ratio);
    const g = canvas.getContext('2d')!;
    g.setTransform(ratio, 0, 0, ratio, 0, 0);
    g.clearRect(0, 0, view, HEIGHT);
    const middle = HEIGHT / 2 + 6;

    // The cursor's bar, wherever it is played.
    g.fillStyle = colour('--select');
    for (const bar of bars.filter((b) => b.bar === cursorBar)) {
      g.fillRect(x(bar.start), 14, bar.length * scale, HEIGHT - 14);
    }

    // The waveform.
    if (peaks) {
      g.strokeStyle = colour('--muted');
      g.beginPath();
      for (let px = 0; px < view; px++) {
        const from = Math.floor(((px + left - 20) / scale) * peaks.rate);
        const to = Math.max(from + 1, Math.floor(((px + 1 + left - 20) / scale) * peaks.rate));
        if (to <= 0 || from >= peaks.low.length) continue;
        let [low, high] = [0, 0];
        for (let i = Math.max(0, from); i < Math.min(peaks.low.length, to); i++) {
          low = Math.min(low, peaks.low[i]);
          high = Math.max(high, peaks.high[i]);
        }
        const half = (HEIGHT - 20) / 2;
        g.moveTo(px + 0.5, middle - high * half);
        g.lineTo(px + 0.5, middle - low * half + 1);
      }
      g.stroke();
    }

    // A line where each bar starts, numbered where there is room; heavier where the bar
    // is held to the recording.
    const every = Math.max(1, Math.ceil(28 / Math.max(1, (bars[0]?.length ?? 2) * scale)));
    g.font = '10px system-ui, sans-serif';
    for (const bar of bars) {
      const start = dragging?.place === bar.place ? dragging.seconds : bar.start;
      const at = Math.round(x(start)) + 0.5;
      if (at < -40 || at > view + 40) continue;
      const fixed = held.has(bar.place);
      g.strokeStyle = colour(fixed ? '--accent' : '--staff');
      g.lineWidth = fixed ? 2 : 1;
      g.beginPath();
      g.moveTo(at, 14);
      g.lineTo(at, HEIGHT);
      g.stroke();
      if (fixed) {
        g.fillStyle = colour('--accent');
        g.beginPath();
        g.moveTo(at - 5, 14);
        g.lineTo(at + 5, 14);
        g.lineTo(at, 20);
        g.fill();
      }
      if (fixed || bar.place % every === 0) {
        g.fillStyle = colour(fixed ? '--accent' : '--muted');
        g.fillText(String(bar.bar + 1), at + 3, 11);
      }
    }
    g.lineWidth = 1;

    if (now !== null) {
      g.strokeStyle = colour('--focus');
      g.lineWidth = 2;
      g.beginPath();
      g.moveTo(Math.round(x(now)), 0);
      g.lineTo(Math.round(x(now)), HEIGHT);
      g.stroke();
    }
  }

  $effect(() => {
    void [bars, peaks, scale, left, cursorBar, now, dragging, held];
    draw();
  });

  // The playhead is kept in view while the recording plays.
  $effect(() => {
    if (now === null || !scroller || dragging) return;
    const at = now * scale + 20;
    if (at < scroller.scrollLeft || at > scroller.scrollLeft + scroller.clientWidth - 40) {
      scroller.scrollLeft = Math.max(0, at - 60);
    }
  });

  onMount(() => {
    const observer = new ResizeObserver(draw);
    if (scroller) observer.observe(scroller);
    return () => observer.disconnect();
  });

  function setZoom(to: number) {
    if (!scroller) return;
    // The middle of what is shown stays in the middle.
    const middle = (scroller.scrollLeft + scroller.clientWidth / 2 - 20) / scale;
    zoom = Math.max(0, Math.min(ZOOMS.length - 1, to));
    requestAnimationFrame(() => {
      if (scroller) scroller.scrollLeft = Math.max(0, middle * ZOOMS[zoom] + 20 - scroller.clientWidth / 2);
    });
  }

  function down(event: PointerEvent) {
    if (event.button !== 0) return;
    const seconds = secondsAt(event.clientX);
    const near = bars
      .map((bar) => ({ bar, off: Math.abs(bar.start - seconds) * scale }))
      .filter((b) => b.off <= GRAB)
      .sort((a, b) => a.off - b.off)[0];
    if (near) {
      (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
      dragging = { place: near.bar.place, seconds: near.bar.start, moved: false };
      return;
    }
    // Elsewhere: the bar there.
    const bar = bars.find((b) => b.start <= seconds && seconds < b.start + b.length);
    if (bar) onseek(bar.bar);
  }

  function move(event: PointerEvent) {
    if (!dragging) return;
    dragging = { ...dragging, seconds: Math.max(0, secondsAt(event.clientX)), moved: true };
  }

  function up() {
    if (dragging?.moved) onsync(dragging.place, Math.round(dragging.seconds * 1000));
    else if (dragging) onseek(bars[dragging.place]?.bar ?? cursorBar);
    dragging = null;
  }

  function doubleClick(event: MouseEvent) {
    const seconds = secondsAt(event.clientX);
    const bar = bars.find((b) => held.has(b.place) && Math.abs(b.start - seconds) * scale <= GRAB);
    if (bar && audio.sync.length > 1) onunsync(bar.place);
  }

  const mix = $derived(audio.mix);
</script>

<section class="audio" aria-label="Audio track">
  <header>
    <h2>Audio track</h2>
    <span class="name" title={audio.name}>{audio.name}</span>
    <button type="button" class="toggle" aria-pressed={mix.mute} aria-label="Mute the recording" onclick={() => onmix({ ...mix, mute: !mix.mute })}>M</button>
    <button type="button" class="toggle" aria-pressed={mix.solo} aria-label="Solo the recording" onclick={() => onmix({ ...mix, solo: !mix.solo })}>S</button>
    <input
      type="range"
      min="0"
      max="100"
      value={mix.volume}
      aria-label="Volume of the recording"
      onchange={(e) => onmix({ ...mix, volume: Number(e.currentTarget.value) })}
    />
    <span class="rule"></span>
    {#if cursorPlace !== null}
      <span class="note" data-cursor-start>
        Bar {cursorBar + 1}: {clock(bars[cursorPlace]?.start ?? 0)}{cursorHeld ? ', held' : ''}
      </span>
      {#if now !== null}
        <button type="button" class="btn small" onclick={() => onsync(cursorPlace!, Math.round(now! * 1000))}>
          Bar {cursorBar + 1} starts here
        </button>
      {/if}
      {#if cursorHeld && audio.sync.length > 1}
        <button type="button" class="btn small" onclick={() => onunsync(cursorPlace!)}>Let bar {cursorBar + 1} go</button>
      {/if}
    {/if}
    <button type="button" class="tool" aria-label="Zoom the waveform out" disabled={zoom === 0} onclick={() => setZoom(zoom - 1)}>
      <Icon name="zoomOut" size={15} />
    </button>
    <button type="button" class="tool" aria-label="Zoom the waveform in" disabled={zoom === ZOOMS.length - 1} onclick={() => setZoom(zoom + 1)}>
      <Icon name="zoomIn" size={15} />
    </button>
    <button type="button" class="tool" aria-label="Replace the recording…" title="Replace the recording…" onclick={onreplace}>
      <Icon name="open" size={15} />
    </button>
    <button type="button" class="tool" aria-label="Remove the audio track" title="Remove the audio track" onclick={onremove}>
      <Icon name="removeTrack" size={15} />
    </button>
    <button type="button" class="tool" aria-label="Close the audio track" onclick={onclose}>
      <Icon name="remove" size={14} />
    </button>
  </header>
  {#if problem}<p class="problem" role="alert">{problem}</p>{/if}
  <div class="scroller" bind:this={scroller} onscroll={() => (left = scroller?.scrollLeft ?? 0)}>
    <div class="track" style:width="{width}px">
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <canvas
        bind:this={canvas}
        style:width="{view}px"
        style:height="{HEIGHT}px"
        class:dragging
        onpointerdown={down}
        onpointermove={move}
        onpointerup={up}
        onpointercancel={() => (dragging = null)}
        ondblclick={doubleClick}
      ></canvas>
    </div>
  </div>
</section>

<style>
  .audio {
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

  .name {
    max-width: 180px;
    overflow: hidden;
    color: var(--muted);
    font-size: var(--text-sm);
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .toggle {
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

  .toggle[aria-pressed='true'] {
    border-color: var(--accent);
    background: var(--accent);
    color: var(--on-accent);
  }

  input[type='range'] {
    width: 80px;
    height: auto;
    padding: 0;
    border: 0;
    background: none;
    accent-color: var(--accent);
  }

  .rule {
    flex: 1;
  }

  .note {
    color: var(--muted);
    font-size: var(--text-sm);
    font-variant-numeric: tabular-nums;
  }

  .small {
    height: 24px;
    padding: 0 var(--space-3);
    font-size: var(--text-sm);
  }

  header .tool {
    width: 26px;
    height: 26px;
  }

  .tool:disabled {
    opacity: 0.4;
    cursor: default;
  }

  .problem {
    margin: 0 var(--space-4) var(--space-2);
    color: var(--error);
    font-size: var(--text-sm);
  }

  .scroller {
    overflow-x: auto;
    padding-bottom: var(--space-2);
  }

  .track {
    height: 96px;
  }

  canvas {
    position: sticky;
    left: 0;
    display: block;
    cursor: pointer;
    touch-action: pan-x;
  }

  canvas.dragging {
    cursor: ew-resize;
  }
</style>
