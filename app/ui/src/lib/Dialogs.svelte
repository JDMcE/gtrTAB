<script lang="ts" module>
  export type DialogKind = 'score' | 'time' | 'key' | 'track' | 'addTrack';

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

  /** Tunings to choose from; each is offered for the kind of instrument its name starts with. */
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
    { name: 'Ukulele: standard', strings: [69, 64, 60, 67] },
    { name: 'Banjo: open G', strings: [62, 59, 55, 50, 67] },
    { name: 'Mandolin: standard', strings: [76, 69, 62, 55] },
  ];
</script>

<script lang="ts">
  import { untrack } from 'svelte';
  import {
    DEFAULT_TUNINGS,
    FAMILIES,
    INSTRUMENTS,
    KEYS,
    type Family,
    type Instrument,
    type Mix,
    type Score,
    type ScoreInfo,
  } from './model';
  import Dialog from './ui/Dialog.svelte';

  interface Props {
    kind: DialogKind;
    score: Score;
    track: number;
    /** The staff of the track whose tuning is shown. */
    staff: number;
    bar: number;
    onscore: (info: ScoreInfo, tempo: number) => void;
    ontime: (numerator: number, denominator: number) => void;
    onkey: (key: number) => void;
    /** A new track, or new settings for the current one; a new track has no mix. `strings`
     *  are open-string pitches from the highest string to the lowest. */
    ontrack: (
      name: string,
      instrument: Instrument,
      strings: number[],
      capo: number,
      mix: Mix | null,
    ) => void;
    onclose: () => void;
  }

  let { kind, score, track, staff, bar, onscore, ontime, onkey, ontrack, onclose }: Props =
    $props();

  // The dialog edits a copy of the values it opened with.
  const initial = untrack(() => ({
    info: score.info,
    tempo: score.tempo,
    time: score.master_bars[bar].time_signature,
    key: score.master_bars[bar].key_signature,
    staff: score.tracks[track].staves[staff] ?? score.tracks[track].staves[0],
    track: score.tracks[track],
  }));
  // Files written before these fields existed leave some of them out.
  const info = $state<ScoreInfo>({
    title: initial.info.title ?? '',
    subtitle: initial.info.subtitle ?? '',
    artist: initial.info.artist ?? '',
    album: initial.info.album ?? '',
    words: initial.info.words ?? '',
    music: initial.info.music ?? '',
    transcriber: initial.info.transcriber ?? '',
    copyright: initial.info.copyright ?? '',
  });
  let tempo = $state(initial.tempo);
  let numerator = $state(initial.time.numerator);
  let denominator = $state(initial.time.denominator);
  let key = $state(initial.key);
  const adding = untrack(() => kind === 'addTrack');
  let instrument = $state<Instrument>(adding ? 'steel_guitar' : initial.track.instrument);
  // Shown low string first, the way tunings are usually written.
  const written = (strings: number[]) => [...strings].reverse().map(noteName).join(' ');
  const usual = (family: Family) => written(DEFAULT_TUNINGS[family]);
  let tuningText = $state(adding ? usual('guitar') : written(initial.staff.tuning.strings));
  let capo = $state(adding ? 0 : initial.staff.capo);
  let problem = $state('');

  /** Changes the instrument, and the tuning with it unless one was chosen that still fits. */
  function chooseInstrument(next: Instrument) {
    const [from, to] = [FAMILIES[instrument], FAMILIES[next]];
    if (from !== to || tuningText === usual(from)) tuningText = usual(to);
    instrument = next;
  }

  const presetFamily = (name: string): Family =>
    (['bass', 'ukulele', 'banjo', 'mandolin'] as const).find((f) => name.toLowerCase().includes(f)) ??
    'guitar';
  const presets = $derived(PRESETS.filter((p) => presetFamily(p.name) === FAMILIES[instrument]));
  /** A piano and a drum kit have no tuning to choose. */
  const tuned = $derived(DEFAULT_TUNINGS[FAMILIES[instrument]].length > 0);
  // A new track is named after its instrument until a name is typed.
  let typedName = $state(adding ? null : initial.track.name);
  const instrumentName = $derived(INSTRUMENTS.find(([id]) => id === instrument)?.[1] ?? '');
  const trackName = $derived(typedName ?? instrumentName);
  let volume = $state(initial.track.mix.volume);
  let pan = $state(initial.track.mix.pan);
  let mute = $state(initial.track.mix.mute);
  let solo = $state(initial.track.mix.solo);

  const preset = $derived(presets.findIndex((p) => written(p.strings) === tuningText));

  const TITLES = {
    score: 'Score',
    time: 'Time signature',
    key: 'Key signature',
    track: 'Track',
    addTrack: 'Add a track',
  };

  function submit() {
    if (kind === 'score') {
      const trimmed = Object.fromEntries(
        Object.entries($state.snapshot(info)).map(([field, value]) => [field, value.trim()]),
      ) as unknown as ScoreInfo;
      onscore(trimmed, tempo);
    } else if (kind === 'time') {
      ontime(numerator, denominator);
    } else if (kind === 'key') {
      onkey(key);
    } else {
      const strings = tuned ? tuningText.trim().split(/[\s,]+/).map(parseNote) : [];
      if (strings.includes(null)) {
        problem = 'Write each string as a note and octave, for example E2 A2 D3 G3 B3 E4.';
        return;
      }
      const mix = kind === 'track' ? { volume, pan, mute, solo } : null;
      const name = trackName.trim() || instrumentName;
      ontrack(name, instrument, (strings as number[]).reverse(), capo, mix);
    }
  }
</script>

<Dialog
  title={TITLES[kind]}
  hint={kind === 'time'
    ? `Applies from bar ${bar + 1} up to the next time signature change.`
    : kind === 'key'
      ? `Applies from bar ${bar + 1} up to the next change of key. It is written on the notation staff and decides how notes are spelled.`
      : kind === 'addTrack'
      ? `The new track goes below ${score.tracks[track].name || 'the current track'}.`
      : undefined}
  {onclose}
  onsubmit={submit}
>
  {#if kind === 'score'}
    <label>Title <input bind:value={info.title} /></label>
    <label>Subtitle <input bind:value={info.subtitle} /></label>
    <div class="pair">
      <label>Artist <input bind:value={info.artist} /></label>
      <label>Album <input bind:value={info.album} /></label>
      <label>Words by <input bind:value={info.words} /></label>
      <label>Music by <input bind:value={info.music} /></label>
    </div>
    <label>Tabbed by <input bind:value={info.transcriber} /></label>
    <label>Copyright <input bind:value={info.copyright} /></label>
    <label>
      Tempo (bpm)
      <input type="number" min="20" max="400" required bind:value={tempo} />
    </label>
  {:else if kind === 'time'}
    <div class="row">
      <input type="number" min="1" max="32" required aria-label="Beats" bind:value={numerator} />
      <span>/</span>
      <select aria-label="Beat unit" bind:value={denominator}>
        {#each [1, 2, 4, 8, 16, 32] as unit}
          <option value={unit}>{unit}</option>
        {/each}
      </select>
    </div>
  {:else if kind === 'key'}
    <label>
      Key
      <select bind:value={key}>
        {#each KEYS as [fifths, name]}
          <option value={fifths}>{name}</option>
        {/each}
      </select>
    </label>
  {:else}
    <label>
      Instrument
      <select value={instrument} onchange={(e) => chooseInstrument(e.currentTarget.value as Instrument)}>
        {#each INSTRUMENTS as [id, name]}
          <option value={id}>{name}</option>
        {/each}
      </select>
    </label>
    <label>
      Name
      <input value={trackName} oninput={(e) => (typedName = e.currentTarget.value)} />
    </label>
    {#if !tuned}
      <p class="plain">
        {instrument === 'drums'
          ? 'Each note on a drum track is a piece of the kit: its number goes where a fret would, for example 36 for the bass drum, 38 for the snare and 42 for the closed hi-hat. The Note menu lists them all.'
          : 'A piano has a staff for each hand. Its notes are entered on tab lines like any other track, and the notation shows them.'}
      </p>
    {:else}
    <div class="pair">
      <label>
        Tuning
        <select
          value={preset}
          onchange={(e) => {
            const chosen = presets[Number(e.currentTarget.value)];
            if (chosen) tuningText = written(chosen.strings);
          }}
        >
          <option value={-1}>Custom</option>
          {#each presets as p, i}
            <option value={i}>{p.name}</option>
          {/each}
        </select>
      </label>
      <label>
        Capo
        <input type="number" min="0" max="12" required bind:value={capo} />
      </label>
    </div>
    <label>
      Strings, lowest first
      <input bind:value={tuningText} spellcheck="false" />
    </label>
    {#if problem}<p class="problem">{problem}</p>{/if}
    {/if}
    {#if kind === 'track'}
      <div class="pair mix">
        <label>
          Volume (0 to 100)
          <input type="number" min="0" max="100" required bind:value={volume} />
        </label>
        <label>
          Pan (-100 left to 100 right)
          <input type="number" min="-100" max="100" required bind:value={pan} />
        </label>
      </div>
      <label class="check"><input type="checkbox" bind:checked={mute} /> Mute</label>
      <label class="check"><input type="checkbox" bind:checked={solo} /> Solo</label>
    {/if}
  {/if}
  {#snippet footer()}
    <button type="button" class="btn" onclick={onclose}>Cancel</button>
    <button type="submit" class="btn primary">OK</button>
  {/snippet}
</Dialog>

<style>
  .row {
    display: flex;
    align-items: center;
    gap: var(--space-3);
  }

  .row input {
    width: 70px;
  }

  .pair {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: var(--space-4);
  }

  .pair input,
  .pair select {
    width: 100%;
    min-width: 0;
  }

  .plain {
    margin: 0 0 var(--space-4);
    color: var(--muted);
  }

  .mix {
    margin-top: var(--space-2);
  }
</style>
