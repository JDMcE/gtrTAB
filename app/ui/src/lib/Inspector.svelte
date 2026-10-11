<script lang="ts">
  import { noteName } from './Dialogs.svelte';
  import { bendPreset, intervalLabel, type EffectKind } from './EffectDialog.svelte';
  import type { BendKind, CurvePoint, DrumPiece, EditorState, Finger, Score } from './model';
  import Icon from './ui/Icon.svelte';
  import type { IconName } from './ui/icons';
  import { tip } from './ui/tooltip';

  interface Props {
    score: Score;
    editor: EditorState;
    technique: (name: string) => void;
    setNoteEffect: (field: string, value: unknown) => void;
    setBeatEffect: (field: string, value: unknown) => void;
    openEffect: (kind: EffectKind) => void;
    openDialog: (kind: 'time') => void;
    /** The pieces of the drum kit, for naming the notes of a drum track. */
    drumKit: DrumPiece[];
  }

  let {
    score,
    editor,
    technique,
    setNoteEffect,
    setBeatEffect,
    openEffect,
    openDialog,
    drumKit,
  }: Props = $props();

  /** One thing set on the note or beat, with how to change and remove it. */
  interface Row {
    icon: IconName;
    label: string;
    value?: string;
    edit?: () => void;
    remove: () => void;
  }

  const VALUE_NAMES: Record<string, string> = {
    whole: 'Whole',
    half: 'Half',
    quarter: 'Quarter',
    eighth: 'Eighth',
    sixteenth: '16th',
    thirty_second: '32nd',
    sixty_fourth: '64th',
  };
  const SLIDES: Record<string, [IconName, string]> = {
    shift: ['slideShift', 'Shift slide'],
    legato: ['slideLegato', 'Legato slide'],
    in_from_below: ['slideInBelow', 'Slide in from below'],
    in_from_above: ['slideInAbove', 'Slide in from above'],
    out_down: ['slideOutDown', 'Slide out downwards'],
    out_up: ['slideOutUp', 'Slide out upwards'],
  };
  const STRUMS: Record<string, [IconName, string]> = {
    brush_down: ['strumDown', 'Strum down'],
    brush_up: ['strumUp', 'Strum up'],
    arpeggio_down: ['arpeggioDown', 'Arpeggio down'],
    arpeggio_up: ['arpeggioUp', 'Arpeggio up'],
    rasgueado: ['strumDown', 'Rasgueado'],
  };
  const BEND_KINDS: [BendKind, string][] = [
    ['bend', 'Bend'],
    ['bend_release', 'Bend and release'],
    ['prebend', 'Pre-bend'],
    ['prebend_release', 'Pre-bend and release'],
  ];
  const FINGERS: Finger[] = ['thumb', 'index', 'middle', 'ring', 'pinky'];
  const HANDS = [
    { field: 'left_finger', title: 'Fretting hand', marks: 'T1234' },
    { field: 'right_finger', title: 'Picking hand', marks: 'pimac' },
  ] as const;

  const peakOf = (points: CurvePoint[]) =>
    points.reduce((m, p) => (Math.abs(p.value) > Math.abs(m) ? p.value : m), 0);

  /** A bend as its preset name and height, or its height alone for a custom curve. */
  function bendText(points: CurvePoint[]): [string, string] {
    const peak = peakOf(points);
    const shape = JSON.stringify(points);
    const kind = BEND_KINDS.find(([k]) => JSON.stringify(bendPreset(k, peak).points) === shape);
    return [kind ? kind[1] : 'Custom bend', intervalLabel(peak)];
  }

  const cursor = $derived(editor.cursor);
  const staff = $derived(score.tracks[cursor.track]?.staves[cursor.staff]);
  const drums = $derived(score.tracks[cursor.track]?.instrument === 'drums');
  const beat = $derived(staff?.bars[cursor.bar]?.voices[0]?.beats[cursor.beat]);
  const time = $derived(score.master_bars[cursor.bar]?.time_signature);
  const note = $derived(editor.note);

  const durationText = $derived.by(() => {
    const d = editor.duration;
    const parts = [VALUE_NAMES[d.value]];
    if (d.dots) parts.push(d.dots > 1 ? 'double dotted' : 'dotted');
    if (d.tuplet) parts.push(`${d.tuplet.actual}:${d.tuplet.normal}`);
    if (d.nested) parts.push(`${d.nested.actual}:${d.nested.normal}`);
    return parts.join(', ');
  });

  /** What the note is: its pitch, or on a drum track the piece of the kit. */
  const pitch = $derived.by(() => {
    if (!note || !staff) return '';
    if (drums) return drumKit.find((p) => p.number === note.fret)?.name ?? 'not in the kit';
    return noteName(staff.tuning.strings[note.string] + staff.capo + note.fret);
  });

  const noteRows = $derived.by(() => {
    const e = note?.effects ?? {};
    const rows: Row[] = [];
    const clear = (field: string) => () => setNoteEffect(field, null);
    if (e.hammer) rows.push({ icon: 'hammer', label: 'Hammer-on or pull-off', remove: clear('hammer') });
    if (e.slide) rows.push({ icon: SLIDES[e.slide][0], label: SLIDES[e.slide][1], remove: clear('slide') });
    if (e.bend) {
      const [label, value] = bendText(e.bend.points);
      rows.push({ icon: 'bend', label, value, edit: () => openEffect('bend'), remove: clear('bend') });
    }
    if (e.vibrato) {
      rows.push({
        icon: e.vibrato === 'wide' ? 'wideVibrato' : 'vibrato',
        label: e.vibrato === 'wide' ? 'Wide vibrato' : 'Vibrato',
        remove: clear('vibrato'),
      });
    }
    if (e.harmonic) {
      rows.push({
        icon: 'harmonic',
        label: `${e.harmonic[0].toUpperCase()}${e.harmonic.slice(1)} harmonic`,
        remove: clear('harmonic'),
      });
    }
    if (e.palm_mute) rows.push({ icon: 'palmMute', label: 'Palm mute', remove: () => technique('palm_mute') });
    if (e.let_ring) rows.push({ icon: 'letRing', label: 'Let ring', remove: () => technique('let_ring') });
    if (e.dead) rows.push({ icon: 'dead', label: 'Dead note', remove: clear('dead') });
    if (e.ghost) rows.push({ icon: 'ghost', label: 'Ghost note', remove: clear('ghost') });
    if (e.staccato) rows.push({ icon: 'staccato', label: 'Staccato', remove: clear('staccato') });
    if (e.accent) {
      const heavy = e.accent === 'heavy';
      rows.push({
        icon: heavy ? 'heavyAccent' : 'accent',
        label: heavy ? 'Heavy accent' : 'Accent',
        remove: () => technique(heavy ? 'heavy_accent' : 'accent'),
      });
    }
    if (e.tapped) rows.push({ icon: 'tapping', label: 'Tapping', remove: clear('tapped') });
    if (e.trill) {
      rows.push({
        icon: 'trill',
        label: 'Trill',
        value: `fret ${e.trill.fret}, ${VALUE_NAMES[e.trill.speed].toLowerCase()}s`,
        edit: () => openEffect('trill'),
        remove: clear('trill'),
      });
    }
    if (e.grace) {
      rows.push({
        icon: 'grace',
        label: 'Grace note',
        value: `fret ${e.grace.fret}, ${e.grace.on_beat ? 'on' : 'before'} the beat`,
        edit: () => openEffect('grace'),
        remove: clear('grace'),
      });
    }
    return rows;
  });

  const beatRows = $derived.by(() => {
    const e = editor.beat_effects ?? {};
    const rows: Row[] = [];
    const clear = (field: string) => () => setBeatEffect(field, null);
    if (e.chord) rows.push({ icon: 'chordName', label: 'Chord', value: e.chord, remove: clear('chord') });
    if (e.diagram) {
      const shape = [...e.diagram.frets].reverse().map((f) => (f < 0 ? 'x' : f)).join(' ');
      rows.push({ icon: 'chordDiagram', label: 'Chord diagram', value: shape, remove: clear('diagram') });
    }
    if (e.volume !== undefined) {
      rows.push({ icon: 'volumeChange', label: 'Volume from here', value: `${e.volume}`, remove: clear('volume') });
    }
    if (e.pan !== undefined) {
      const side = e.pan === 0 ? 'centre' : `${Math.abs(e.pan)} ${e.pan < 0 ? 'left' : 'right'}`;
      rows.push({ icon: 'panChange', label: 'Pan from here', value: side, remove: clear('pan') });
    }
    if (e.text) rows.push({ icon: 'beatText', label: 'Text', value: e.text, remove: clear('text') });
    if (e.lyrics?.length) {
      rows.push({ icon: 'lyrics', label: 'Lyrics', value: e.lyrics.join(' / '), remove: clear('lyrics') });
    }
    if (e.fermata) rows.push({ icon: 'fermata', label: 'Fermata', remove: clear('fermata') });
    if (e.dynamic) rows.push({ icon: e.dynamic, label: 'Dynamic', value: e.dynamic, remove: clear('dynamic') });
    if (e.hairpin) {
      rows.push({
        icon: e.hairpin,
        label: e.hairpin === 'crescendo' ? 'Crescendo' : 'Decrescendo',
        remove: clear('hairpin'),
      });
    }
    if (e.stroke) {
      rows.push({
        icon: e.stroke === 'down' ? 'strokeDown' : 'strokeUp',
        label: e.stroke === 'down' ? 'Downstroke' : 'Upstroke',
        remove: clear('stroke'),
      });
    }
    if (e.strum) rows.push({ icon: STRUMS[e.strum][0], label: STRUMS[e.strum][1], remove: clear('strum') });
    if (e.slap) {
      rows.push({ icon: e.slap, label: e.slap === 'slap' ? 'Slap' : 'Pop', remove: clear('slap') });
    }
    if (e.tremolo) rows.push({ icon: 'tremolo', label: 'Tremolo picking', remove: clear('tremolo') });
    if (e.whammy) {
      rows.push({
        icon: 'whammy',
        label: 'Whammy bar',
        value: intervalLabel(peakOf(e.whammy.points)),
        edit: () => openEffect('whammy'),
        remove: clear('whammy'),
      });
    }
    return rows;
  });
</script>

{#snippet list(rows: Row[], empty: string)}
  {#if rows.length === 0}
    <p class="empty">{empty}</p>
  {:else}
    <ul>
      {#each rows as row}
        <li>
          <span class="glyph"><Icon name={row.icon} size={16} /></span>
          <span class="what">
            {row.label}{#if row.value}<small>{row.value}</small>{/if}
          </span>
          {#if row.edit}
            <button
              type="button"
              class="tool"
              aria-label="Edit {row.label.toLowerCase()}"
              use:tip={{ label: 'Edit' }}
              onclick={row.edit}
            >
              <Icon name="editValue" size={14} />
            </button>
          {/if}
          <button
            type="button"
            class="tool"
            aria-label="Remove {row.label.toLowerCase()}"
            use:tip={{ label: 'Remove' }}
            onclick={row.remove}
          >
            <Icon name="remove" size={14} />
          </button>
        </li>
      {/each}
    </ul>
  {/if}
{/snippet}

<aside class="inspector" aria-label="Inspector">
  <section>
    <h3>Position</h3>
    <dl>
      <dt>Bar</dt>
      <dd>{cursor.bar + 1} of {score.master_bars.length}</dd>
      <dt>Beat</dt>
      <dd>{cursor.beat + 1}</dd>
      <dt>String</dt>
      <dd>{cursor.string + 1}{#if staff && !drums}&ensp;({noteName(staff.tuning.strings[cursor.string] + staff.capo)}){/if}</dd>
      <dt>Time</dt>
      <dd>
        {#if time}
          <button type="button" class="link" onclick={() => openDialog('time')}>
            {time.numerator}/{time.denominator}
          </button>
        {/if}
      </dd>
    </dl>
  </section>

  <section>
    <h3>Beat</h3>
    <dl>
      <dt>Value</dt>
      <dd>{durationText}</dd>
      <dt>Holds</dt>
      <dd>
        {#if !beat}
          Nothing yet
        {:else if beat.notes.length === 0}
          Rest
        {:else}
          {beat.notes.length} {beat.notes.length === 1 ? 'note' : 'notes'}
        {/if}
      </dd>
    </dl>
    {#if beat}{@render list(beatRows, 'No techniques on this beat.')}{/if}
  </section>

  <section>
    <h3>Note</h3>
    {#if note}
      <dl>
        <dt>Fret</dt>
        <dd>{note.fret}&ensp;({pitch}){note.tied ? ', tied' : ''}</dd>
      </dl>
      {@render list(noteRows, 'No techniques on this note.')}
      {#each HANDS as hand}
        <div class="fingers" role="group" aria-label="{hand.title} finger">
          <span>{hand.title}</span>
          {#each FINGERS as finger, i}
            <button
              type="button"
              aria-label="{hand.title}: {finger}"
              aria-pressed={note.effects?.[hand.field] === finger}
              use:tip={{ label: `${finger[0].toUpperCase()}${finger.slice(1)}` }}
              onclick={() =>
                setNoteEffect(hand.field, note.effects?.[hand.field] === finger ? null : finger)}
            >
              {hand.marks[i]}
            </button>
          {/each}
        </div>
      {/each}
    {:else}
      <p class="empty">No note on this string.</p>
    {/if}
  </section>
</aside>

<style>
  .inspector {
    flex: none;
    width: 216px;
    overflow-y: auto;
    scrollbar-width: thin;
    border-left: 1px solid var(--border);
    background: var(--panel);
    font-size: var(--text-sm);
  }

  section {
    padding: var(--space-3) var(--space-4) var(--space-4);
    border-bottom: 1px solid var(--border);
  }

  h3 {
    margin: 0 0 var(--space-3);
    color: var(--muted);
    font-size: var(--text-xs);
    font-weight: 600;
    letter-spacing: 0.04em;
    text-transform: uppercase;
  }

  dl {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: var(--space-2) var(--space-4);
    margin: 0;
  }

  dt {
    color: var(--muted);
  }

  dd {
    margin: 0;
  }

  .link {
    padding: 0;
    border: 0;
    background: none;
    color: var(--accent);
    text-decoration: underline;
    cursor: pointer;
  }

  ul {
    margin: var(--space-3) 0 0;
    padding: 0;
    list-style: none;
  }

  li {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    min-height: 28px;
  }

  .glyph {
    display: grid;
    place-items: center;
    width: 20px;
    color: var(--accent);
  }

  .what {
    flex: 1;
    min-width: 0;
  }

  .what small {
    display: block;
    color: var(--muted);
    font-size: var(--text-xs);
  }

  li .tool {
    width: 22px;
    height: 22px;
    color: var(--muted);
  }

  .empty {
    margin: var(--space-3) 0 0;
    color: var(--muted);
  }

  .fingers {
    display: flex;
    align-items: center;
    gap: var(--space-1);
    margin-top: var(--space-3);
  }

  .fingers span {
    flex: 1;
    color: var(--muted);
    white-space: nowrap;
  }

  .fingers button {
    width: 20px;
    height: 22px;
    padding: 0;
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-s);
    background: var(--raised);
    cursor: pointer;
  }

  .fingers button:hover {
    background: var(--hover);
  }

  .fingers button[aria-pressed='true'] {
    border-color: var(--accent);
    background: var(--pressed);
    color: var(--accent);
    font-weight: 700;
  }
</style>
