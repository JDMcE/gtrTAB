<script lang="ts">
  import { onMount } from 'svelte';
  import Dialogs, { type DialogKind } from './lib/Dialogs.svelte';
  import EffectDialog, { bendPreset, type EffectKind } from './lib/EffectDialog.svelte';
  import {
    EXTENSION,
    addRecentFile,
    confirmDialog,
    openFile,
    readAutosave,
    readPath,
    recentFiles,
    removeRecentFile,
    saveFile,
    setWindowTitle,
    writeAutosave,
    type OpenedFile,
  } from './lib/files';
  import {
    NOTE_VALUES,
    type BendKind,
    type EditorState,
    type Layout,
    type Score,
    type Timeline,
  } from './lib/model';
  import { Player } from './lib/player';
  import ScoreView from './lib/ScoreView.svelte';
  import init, { Document } from './wasm/gtr_wasm.js';

  const UNTITLED = `untitled.${EXTENSION}`;
  const VALUE_LABELS = ['1', '2', '4', '8', '16', '32', '64'];
  const VALUE_NAMES = ['Whole', 'Half', 'Quarter', 'Eighth', '16th', '32nd', '64th'];
  const SLIDES = [
    ['slide_shift', 'Shift slide'],
    ['slide_legato', 'Legato slide'],
    ['slide_in_below', 'Slide in from below'],
    ['slide_in_above', 'Slide in from above'],
    ['slide_out_down', 'Slide out downwards'],
    ['slide_out_up', 'Slide out upwards'],
  ];
  const HARMONICS = [
    ['harmonic_natural', 'Natural (N.H.)'],
    ['harmonic_artificial', 'Artificial (A.H.)'],
    ['harmonic_pinch', 'Pinch (P.H.)'],
    ['harmonic_tapped', 'Tapped (T.H.)'],
    ['harmonic_semi', 'Semi (S.H.)'],
  ];
  const STRUMS = [
    ['brush_down', 'Strum down'],
    ['brush_up', 'Strum up'],
    ['arpeggio_down', 'Arpeggio down'],
    ['arpeggio_up', 'Arpeggio up'],
    ['rasgueado', 'Rasgueado'],
  ];
  const DYNAMICS = ['ppp', 'pp', 'p', 'mp', 'mf', 'f', 'ff', 'fff'];
  const FINGERS = ['thumb', 'index', 'middle', 'ring', 'pinky'];
  /** Tuplets as notes played in the time of how many. */
  const TUPLETS = [
    [3, 2],
    [5, 4],
    [6, 4],
    [7, 4],
    [9, 8],
    [10, 8],
    [11, 8],
    [12, 8],
    [13, 8],
  ];
  const BEND_KINDS: [BendKind, string][] = [
    ['bend', 'Bend'],
    ['bend_release', 'Bend and release'],
    ['prebend', 'Pre-bend'],
    ['prebend_release', 'Pre-bend and release'],
  ] as [BendKind, string][];
  const BEND_HEIGHTS = [
    [1, '¼'],
    [2, '½'],
    [4, 'full'],
    [6, '1½'],
    [8, '2'],
  ] as const;
  /** Longest pause between two digits that still makes one fret number. */
  const DIGIT_WINDOW_MS = 900;
  const AUTOSAVE_DELAY_MS = 1500;

  let doc = $state.raw<Document>();
  const player = new Player();

  let score = $state.raw<Score>();
  let editor = $state.raw<EditorState>();
  let fileName = $state(UNTITLED);
  let filePath = $state<string | null>(null);
  let message = $state('');
  let failed = $state(false);
  let recent = $state(recentFiles());
  let dialog = $state<DialogKind | null>(null);
  let effectDialog = $state<EffectKind | null>(null);
  let zoom = $state(1);

  let playing = $state(false);
  let playhead = $state.raw<{ bar: number; beat: number } | null>(null);
  let metronome = $state(false);
  let countIn = $state(false);
  let speed = $state(1);

  let revision = -1;
  let lastDigitAt = 0;

  function setDocument(next: Document, name: string, path: string | null) {
    stop();
    doc?.free();
    doc = next;
    revision = -1;
    fileName = name;
    filePath = path;
    refresh();
  }

  /** Pulls the editor state from the core, and the score too if it changed. */
  function refresh() {
    if (!doc) return;
    const state: EditorState = JSON.parse(doc.stateJson());
    if (state.revision !== revision) {
      revision = state.revision;
      score = JSON.parse(doc.scoreJson());
    }
    editor = state;
  }

  /** Runs an action, showing any failure in the status bar. */
  async function run(action: () => unknown) {
    message = '';
    failed = false;
    try {
      await action();
    } catch (e) {
      message = e instanceof Error ? e.message : String(e);
      failed = true;
    }
    refresh();
  }

  /** Runs an edit on the open document. */
  const edit = (action: (doc: Document) => unknown) => run(() => doc && action(doc));

  // --- Files ---

  async function discardChanges(): Promise<boolean> {
    return !editor?.dirty || confirmDialog(`Discard unsaved changes to ${fileName}?`);
  }

  const newScore = () =>
    run(async () => {
      if (await discardChanges()) setDocument(new Document(), UNTITLED, null);
    });

  function load(file: OpenedFile) {
    if (file.name.toLowerCase().endsWith(`.${EXTENSION}`)) {
      setDocument(Document.load(file.bytes), file.name, file.path);
      if (file.path) recent = addRecentFile(file.path);
      return;
    }
    // Imported files are saved as a new gtrTab score, never over the original.
    const imported = Document.importFile(file.name, file.bytes);
    const report = imported.importReport;
    imported.markUnsaved();
    setDocument(imported, file.name.replace(/\.[^.]+$/, '') + `.${EXTENSION}`, null);
    message = report ? `Imported ${file.name}. Not imported: ${report}.` : `Imported ${file.name}.`;
  }

  const open = () =>
    run(async () => {
      if (!(await discardChanges())) return;
      const file = await openFile();
      if (file) load(file);
    });

  /** Opens a file dropped onto the window. */
  function onDrop(event: DragEvent) {
    const file = event.dataTransfer?.files[0];
    if (!file) return;
    event.preventDefault();
    run(async () => {
      if (!(await discardChanges())) return;
      load({ name: file.name, path: null, bytes: new Uint8Array(await file.arrayBuffer()) });
    });
  }

  const openRecent = (path: string) =>
    run(async () => {
      if (!(await discardChanges())) return;
      try {
        load(await readPath(path));
      } catch (e) {
        recent = removeRecentFile(path);
        throw e;
      }
    });

  const save = (saveAs = false) =>
    run(async () => {
      if (!doc) return;
      const saved = await saveFile(fileName, saveAs ? null : filePath, doc.save());
      if (!saved) return;
      doc.markSaved();
      fileName = saved.name;
      filePath = saved.path;
      if (saved.path) recent = addRecentFile(saved.path);
      message = `Saved ${saved.name}`;
    });

  // Keep a copy of unsaved work so that a crash or closed window does not lose it.
  $effect(() => {
    if (!editor?.dirty) {
      writeAutosave(null);
      return;
    }
    void score;
    const timer = setTimeout(() => {
      if (doc) writeAutosave({ name: fileName, path: filePath, bytes: doc.save() });
    }, AUTOSAVE_DELAY_MS);
    return () => clearTimeout(timer);
  });

  $effect(() => {
    const title = `${editor?.dirty ? '• ' : ''}${fileName} — gtrTab`;
    setWindowTitle(title).catch(() => {});
  });

  // --- Playback ---

  const play = () =>
    run(async () => {
      if (!doc || !editor) return;
      const selection = editor.selection;
      const options = {
        from_bar: selection ? selection[0][0] : editor.cursor.bar,
        to_bar: selection ? selection[1][0] + 1 : null,
        metronome,
        count_in: countIn,
        speed,
      };
      const audio = doc.renderAudio(JSON.stringify(options), player.sampleRate);
      const timeline: Timeline = JSON.parse(audio.timelineJson);
      const samples = audio.takeSamples();
      audio.free();
      await player.play(samples, timeline, () => {
        playing = false;
        playhead = null;
      });
      playing = true;
      follow();
    });

  function follow() {
    if (!playing) return;
    const now = player.position();
    if (now && (now.bar !== playhead?.bar || now.beat !== playhead?.beat)) {
      playhead = { bar: now.bar, beat: now.beat };
    }
    requestAnimationFrame(follow);
  }

  /** Stops playback, leaving the cursor where the music stopped. */
  function stop() {
    if (!playing) return;
    player.stop();
    playing = false;
    const at = playhead;
    playhead = null;
    if (at && doc && editor) {
      doc.setCursor(at.bar, at.beat, editor.cursor.string, false);
      refresh();
    }
  }

  const togglePlay = () => (playing ? stop() : play());

  // --- Techniques ---

  const technique = (name: string) => edit((d) => d.toggleTechnique(name));
  const effects = $derived(editor?.note?.effects ?? {});
  const slideValue = $derived(
    effects.slide ? `slide_${effects.slide.replace('_from', '')}` : '',
  );
  const harmonicValue = $derived(effects.harmonic ? `harmonic_${effects.harmonic}` : '');
  const beatEffects = $derived(editor?.beat_effects ?? {});
  const strumValue = $derived(beatEffects.strum ?? '');

  const setNoteEffect = (field: string, value: unknown) =>
    edit((d) => d.setNoteEffect(field, JSON.stringify(value)));
  const setBeatEffect = (field: string, value: unknown) =>
    edit((d) => d.setBeatEffect(field, JSON.stringify(value)));

  /** The bend dropdown's value: a preset as `kind:height`, or `custom` for any other curve. */
  const bendValue = $derived.by(() => {
    if (!effects.bend) return '';
    const current = JSON.stringify(effects.bend);
    for (const [kind] of BEND_KINDS) {
      for (const [height] of BEND_HEIGHTS) {
        if (JSON.stringify(bendPreset(kind, height)) === current) return `${kind}:${height}`;
      }
    }
    return 'custom';
  });

  function setBend(value: string) {
    if (value === 'custom') {
      effectDialog = 'bend';
      return;
    }
    const [kind, height] = value.split(':');
    setNoteEffect('bend', value === '' ? null : bendPreset(kind as BendKind, Number(height)));
  }

  const tupletValue = (t: { actual: number; normal: number } | null | undefined) =>
    t ? `${t.actual}:${t.normal}` : '';

  function setTuplet(value: string, nested: boolean) {
    const [actual, normal] = value === '' ? [0, 0] : value.split(':').map(Number);
    edit((d) => (nested ? d.setNestedTuplet(actual, normal) : d.setTuplet(actual, normal)));
  }

  /** Chooses one of a set of exclusive techniques from a dropdown; '' clears it. */
  function chooseTechnique(current: string, chosen: string) {
    if (chosen !== '') technique(chosen);
    else if (current !== '') technique(current);
  }

  // --- Keyboard ---

  function onKeydown(event: KeyboardEvent) {
    if (dialog || effectDialog || !doc) return;
    const target = event.target instanceof Element ? event.target : null;
    if (target?.closest('input, select, textarea')) return;

    const action = keyAction(event);
    if (!action) return;
    event.preventDefault();
    if (target instanceof HTMLButtonElement) target.blur();
    action();
  }

  function keyAction(event: KeyboardEvent): (() => unknown) | undefined {
    const { key, shiftKey: shift } = event;
    const ctrl = event.ctrlKey || event.metaKey;
    const move = (direction: string) => () => edit((d) => d.moveCursor(direction, shift));

    if (ctrl) {
      switch (key.toLowerCase()) {
        case 'z': return shift ? () => edit((d) => d.redo()) : () => edit((d) => d.undo());
        case 'y': return () => edit((d) => d.redo());
        case 'c': return () => edit((d) => d.copy());
        case 'x': return () => edit((d) => d.cut());
        case 'v': return () => edit((d) => d.paste());
        case 's': return () => save(shift);
        case 'o': return open;
        case 'n': return newScore;
        case 'arrowleft': return move('prev_bar');
        case 'arrowright': return move('next_bar');
        case 'home': return move('first');
        case 'end': return move('last');
        case 'delete': return () => edit((d) => d.deleteBeats());
        case 'insert': return () => edit((d) => d.insertBar());
      }
      return undefined;
    }
    if (event.altKey) return undefined;

    if (/^[0-9]$/.test(key)) {
      return () => {
        const now = performance.now();
        const combine = now - lastDigitAt < DIGIT_WINDOW_MS;
        lastDigitAt = now;
        edit((d) => d.typeDigit(Number(key), combine));
      };
    }
    switch (key) {
      case 'ArrowLeft': return move('left');
      case 'ArrowRight': return move('right');
      case 'ArrowUp': return move('up');
      case 'ArrowDown': return move('down');
      case 'Home': return move('first');
      case 'End': return move('last');
      case 'Delete':
      case 'Backspace': return () => edit((d) => d.delete());
      case 'Insert': return () => edit((d) => d.insertBeat());
      case '+':
      case '=': return () => edit((d) => d.shorter());
      case '-': return () => edit((d) => d.longer());
      case '.': return () => edit((d) => d.toggleDot());
      case '/': return () => edit((d) => d.toggleTriplet());
      case 'r':
      case 'R': return () => edit((d) => d.rest());
      case 'l':
      case 'L': return () => edit((d) => d.toggleTie());
      case 'h': return () => technique('hammer');
      case 's': return () => technique('slide_shift');
      case 'b': return () => setBend(effects.bend ? '' : 'bend:4');
      case 'B': return () => (effectDialog = 'bend');
      case 'w': return () => (effectDialog = 'whammy');
      case '<': return () => technique('crescendo');
      case '>': return () => technique('decrescendo');
      case 'v': return () => technique('vibrato');
      case 'p': return () => technique('palm_mute');
      case 'i': return () => technique('let_ring');
      case 'x': return () => technique('dead');
      case 'g': return () => technique('ghost');
      case 'a': return () => technique('accent');
      case 'n': return () => technique('harmonic_natural');
      case 't': return () => technique('tapped');
      case ' ': return togglePlay;
    }
    return undefined;
  }

  onMount(() => {
    run(async () => {
      await init();
      const autosave = readAutosave();
      if (autosave) {
        try {
          const recovered = Document.load(autosave.bytes);
          recovered.markUnsaved();
          setDocument(recovered, autosave.name, autosave.path);
          message = `Recovered unsaved changes to ${autosave.name}`;
          return;
        } catch {
          writeAutosave(null);
        }
      }
      setDocument(new Document(), UNTITLED, null);
    });
    return () => {
      player.stop();
      doc?.free();
    };
  });
</script>

<svelte:window onkeydown={onKeydown} />

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="app" ondragover={(e) => e.preventDefault()} ondrop={onDrop}>
  <header>
    <strong>gtrTab</strong>
    <div class="group">
      <button onclick={newScore} title="New (Ctrl+N)">New</button>
      <button onclick={open} title="Open or import (Ctrl+O)">Open…</button>
      {#if recent.length > 0}
        <select
          title="Recent files"
          aria-label="Recent files"
          value=""
          onchange={(e) => {
            const path = e.currentTarget.value;
            e.currentTarget.value = '';
            if (path) openRecent(path);
          }}
        >
          <option value="">Recent</option>
          {#each recent as path}
            <option value={path}>{path.split(/[\\/]/).pop()}</option>
          {/each}
        </select>
      {/if}
      <button onclick={() => save()} disabled={!score} title="Save (Ctrl+S)">Save</button>
      <button onclick={() => save(true)} disabled={!score} title="Save as (Ctrl+Shift+S)">
        Save as…
      </button>
    </div>

    {#if score && editor}
      <div class="group">
        <button onclick={() => edit((d) => d.undo())} disabled={!editor.can_undo} title="Undo (Ctrl+Z)">
          Undo
        </button>
        <button onclick={() => edit((d) => d.redo())} disabled={!editor.can_redo} title="Redo (Ctrl+Y)">
          Redo
        </button>
      </div>

      <div class="group">
        {#each NOTE_VALUES as value, i}
          <button
            class="tight"
            class:active={editor.duration.value === value}
            onclick={() => edit((d) => d.setNoteValue(value))}
            title="{VALUE_NAMES[i]} note (+ and − step through)"
          >
            {VALUE_LABELS[i]}
          </button>
        {/each}
        <button
          class="tight"
          class:active={editor.duration.dots > 0}
          onclick={() => edit((d) => d.toggleDot())}
          title="Dotted (.)"
        >
          •
        </button>
        <button
          class="tight"
          class:active={editor.duration.tuplet !== null}
          onclick={() => edit((d) => d.toggleTriplet())}
          title="Triplet (/)"
        >
          3
        </button>
        <button onclick={() => edit((d) => d.rest())} title="Rest (R)">Rest</button>
        <button onclick={() => edit((d) => d.toggleTie())} title="Tie (L)">Tie</button>
      </div>

      <div class="group">
        <button onclick={() => edit((d) => d.insertBar())} title="Insert bar (Ctrl+Insert)">
          + Bar
        </button>
        <button onclick={() => edit((d) => d.removeBar())} title="Remove bar">− Bar</button>
        <button onclick={() => (dialog = 'time')}>
          {score.master_bars[editor.cursor.bar].time_signature.numerator}/{score.master_bars[
            editor.cursor.bar
          ].time_signature.denominator}
        </button>
        <button onclick={() => (dialog = 'tuning')}>Tuning…</button>
        <button onclick={() => (dialog = 'score')}>Score…</button>
        {#if score.tracks.length > 1}
          <select
            aria-label="Track"
            title="Track"
            bind:value={() => editor!.cursor.track, (track) => edit((d) => d.setTrack(track))}
          >
            {#each score.tracks as track, i}
              <option value={i}>{track.name || `Track ${i + 1}`}</option>
            {/each}
          </select>
        {/if}
      </div>

      <div class="group">
        <button class="play" onclick={togglePlay} title="Play or stop (Space)">
          {playing ? 'Stop' : 'Play'}
        </button>
        <button class:active={metronome} onclick={() => (metronome = !metronome)} title="Metronome">
          Click
        </button>
        <button class:active={countIn} onclick={() => (countIn = !countIn)} title="Count-in bar">
          Count-in
        </button>
        <select aria-label="Playback speed" title="Playback speed" bind:value={speed}>
          {#each [0.5, 0.75, 1] as option}
            <option value={option}>{option * 100}%</option>
          {/each}
        </select>
      </div>

      <div class="group techniques">
        <button class:active={effects.hammer} onclick={() => technique('hammer')} title="Hammer-on or pull-off to the next note (H)">
          H/P
        </button>
        <select
          aria-label="Slide"
          title="Slide (S for a shift slide)"
          class:active={slideValue !== ''}
          bind:value={() => slideValue, (v) => chooseTechnique(slideValue, v)}
        >
          <option value="">Slide</option>
          {#each SLIDES as [value, label]}
            <option {value}>{label}</option>
          {/each}
        </select>
        <select
          aria-label="Bend"
          title="Bend (B for a full bend)"
          class:active={bendValue !== ''}
          bind:value={() => bendValue, (v) => setBend(v)}
        >
          <option value="">Bend</option>
          <option value="custom">Custom curve…</option>
          {#each BEND_KINDS as [kind, kindLabel]}
            <optgroup label={kindLabel}>
              {#each BEND_HEIGHTS as [height, heightLabel]}
                <option value="{kind}:{height}">{kindLabel} {heightLabel}</option>
              {/each}
            </optgroup>
          {/each}
        </select>
        <button class:active={effects.vibrato === 'slight'} onclick={() => technique('vibrato')} title="Vibrato (V)">
          Vib
        </button>
        <button class:active={effects.vibrato === 'wide'} onclick={() => technique('wide_vibrato')} title="Wide vibrato">
          Wide vib
        </button>
        <button class:active={effects.palm_mute} onclick={() => technique('palm_mute')} title="Palm mute (P)">
          P.M.
        </button>
        <button class:active={effects.let_ring} onclick={() => technique('let_ring')} title="Let ring (I)">
          Let ring
        </button>
        <button class="tight" class:active={effects.dead} onclick={() => technique('dead')} title="Dead note (X)">
          X
        </button>
        <button class="tight" class:active={effects.ghost} onclick={() => technique('ghost')} title="Ghost note (G)">
          ( )
        </button>
        <button class:active={effects.staccato} onclick={() => technique('staccato')} title="Staccato">
          Stacc
        </button>
        <button class="tight" class:active={effects.accent === 'normal'} onclick={() => technique('accent')} title="Accent (A)">
          &gt;
        </button>
        <button class="tight" class:active={effects.accent === 'heavy'} onclick={() => technique('heavy_accent')} title="Heavy accent">
          ∧
        </button>
        <select
          aria-label="Harmonic"
          title="Harmonic (N for a natural harmonic)"
          class:active={harmonicValue !== ''}
          bind:value={() => harmonicValue, (v) => chooseTechnique(harmonicValue, v)}
        >
          <option value="">Harmonic</option>
          {#each HARMONICS as [value, label]}
            <option {value}>{label}</option>
          {/each}
        </select>
        <button class="tight" class:active={effects.tapped} onclick={() => technique('tapped')} title="Tapping (T)">
          T
        </button>
        <button class:active={!!editor.beat_effects?.tremolo} onclick={() => technique('tremolo')} title="Tremolo picking">
          Trem
        </button>
        <button class="tight" class:active={editor.beat_effects?.stroke === 'down'} onclick={() => technique('stroke_down')} title="Downstroke">
          ↓
        </button>
        <button class="tight" class:active={editor.beat_effects?.stroke === 'up'} onclick={() => technique('stroke_up')} title="Upstroke">
          ↑
        </button>
      </div>

      <div class="group techniques">
        <button class:active={!!beatEffects.whammy} onclick={() => (effectDialog = 'whammy')} title="Whammy bar (W)">
          Whammy…
        </button>
        <button class:active={!!effects.trill} onclick={() => (effectDialog = 'trill')} disabled={!editor.note} title="Trill">
          Trill…
        </button>
        <button class:active={!!effects.grace} onclick={() => (effectDialog = 'grace')} disabled={!editor.note} title="Grace note">
          Grace…
        </button>
        <select
          aria-label="Strum"
          title="Strum or arpeggio"
          class:active={strumValue !== ''}
          bind:value={() => strumValue, (v) => chooseTechnique(strumValue, v)}
        >
          <option value="">Strum</option>
          {#each STRUMS as [value, label]}
            <option {value}>{label}</option>
          {/each}
        </select>
        <button class:active={beatEffects.slap === 'slap'} onclick={() => technique('slap')} title="Slap">
          Slap
        </button>
        <button class:active={beatEffects.slap === 'pop'} onclick={() => technique('pop')} title="Pop">
          Pop
        </button>
        <select
          aria-label="Dynamic"
          title="Dynamic: loudness from this beat on"
          class:active={!!beatEffects.dynamic}
          bind:value={() => beatEffects.dynamic ?? '', (v) => setBeatEffect('dynamic', v || null)}
        >
          <option value="">Dynamic</option>
          {#each DYNAMICS as value}
            <option {value}>{value}</option>
          {/each}
        </select>
        <button class="tight" class:active={beatEffects.hairpin === 'crescendo'} onclick={() => technique('crescendo')} title="Crescendo (<)">
          &lt;
        </button>
        <button class="tight" class:active={beatEffects.hairpin === 'decrescendo'} onclick={() => technique('decrescendo')} title="Decrescendo (>)">
          &gt;
        </button>
        <select
          aria-label="Tuplet"
          title="Tuplet (/ for a triplet)"
          class:active={editor.duration.tuplet !== null}
          bind:value={() => tupletValue(editor?.duration.tuplet), (v) => setTuplet(v, false)}
        >
          <option value="">Tuplet</option>
          {#each TUPLETS as [actual, normal]}
            <option value="{actual}:{normal}">{actual}:{normal}</option>
          {/each}
        </select>
        <select
          aria-label="Nested tuplet"
          title="A second tuplet inside the first"
          class:active={!!editor.duration.nested}
          disabled={editor.duration.tuplet === null}
          bind:value={() => tupletValue(editor?.duration.nested), (v) => setTuplet(v, true)}
        >
          <option value="">Nested</option>
          {#each TUPLETS.slice(0, 4) as [actual, normal]}
            <option value="{actual}:{normal}">{actual}:{normal}</option>
          {/each}
        </select>
        <select
          aria-label="Fretting-hand finger"
          title="Fretting-hand finger"
          class:active={!!effects.left_finger}
          disabled={!editor.note}
          bind:value={() => effects.left_finger ?? '', (v) => setNoteEffect('left_finger', v || null)}
        >
          <option value="">LH</option>
          {#each FINGERS as finger, i}
            <option value={finger}>{'T1234'[i]}</option>
          {/each}
        </select>
        <select
          aria-label="Picking-hand finger"
          title="Picking-hand finger"
          class:active={!!effects.right_finger}
          disabled={!editor.note}
          bind:value={() => effects.right_finger ?? '', (v) => setNoteEffect('right_finger', v || null)}
        >
          <option value="">RH</option>
          {#each FINGERS as finger, i}
            <option value={finger}>{'pimac'[i]}</option>
          {/each}
        </select>
      </div>

      <div class="group end">
        <select aria-label="Zoom" title="Zoom" bind:value={zoom}>
          {#each [0.75, 1, 1.25, 1.5, 2] as option}
            <option value={option}>{option * 100}%</option>
          {/each}
        </select>
      </div>
    {/if}
  </header>

  {#if score && editor && doc}
    <ScoreView
      getLayout={(width) => JSON.parse(doc!.layoutJson(width)) as Layout}
      revision={`${editor.revision}:${editor.cursor.track}:${fileName}`}
      {editor}
      {playhead}
      {zoom}
      onpick={(bar, beat, string, extend) => edit((d) => d.setCursor(bar, beat, string, extend))}
    />
  {:else}
    <main>{failed ? '' : 'Loading…'}</main>
  {/if}

  <footer class:failed>
    <span>{message || fileName}</span>
    {#if editor}
      <span>Bar {editor.cursor.bar + 1} · Beat {editor.cursor.beat + 1}</span>
    {/if}
  </footer>
</div>

{#if dialog && score && editor}
  <Dialogs
    kind={dialog}
    {score}
    track={editor.cursor.track}
    bar={editor.cursor.bar}
    onscore={(info, tempo) => {
      dialog = null;
      edit((d) => {
        const old = score!;
        const changed =
          info.title !== old.info.title || info.artist !== old.info.artist || info.album !== old.info.album;
        if (changed) d.setInfo(info.title, info.artist, info.album);
        if (tempo !== old.tempo) d.setTempo(tempo);
      });
    }}
    ontime={(numerator, denominator) => {
      dialog = null;
      edit((d) => d.setTimeSignature(numerator, denominator));
    }}
    ontuning={(strings, capo) => {
      dialog = null;
      edit((d) => d.setTuning(new Uint8Array(strings), capo));
    }}
    onclose={() => (dialog = null)}
  />
{/if}

{#if effectDialog && editor}
  <EffectDialog
    kind={effectDialog}
    note={editor.note}
    beatEffects={editor.beat_effects}
    onapply={(value) => {
      // Read which effect this is before closing the dialog clears it.
      const kind = effectDialog;
      effectDialog = null;
      if (kind === 'whammy') setBeatEffect('whammy', value);
      else if (kind) setNoteEffect(kind, value);
    }}
    onclose={() => (effectDialog = null)}
  />
{/if}

<style>
  .app {
    display: flex;
    flex-direction: column;
    height: 100%;
  }

  header,
  footer {
    background: var(--panel);
    border: 0 solid var(--border);
    padding: 6px 12px;
  }

  header {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px 14px;
    border-bottom-width: 1px;
  }

  header strong {
    color: var(--accent);
  }

  .group {
    display: flex;
    align-items: center;
    gap: 4px;
  }

  .group.end {
    margin-left: auto;
  }

  button,
  select {
    font: inherit;
    padding: 4px 9px;
    border: 1px solid var(--border);
    border-radius: 4px;
    background: var(--bg);
    color: var(--text);
    cursor: pointer;
  }

  select {
    max-width: 150px;
  }

  button.tight {
    min-width: 28px;
    padding: 4px 6px;
  }

  button.active,
  select.active {
    border-color: var(--accent);
    color: var(--accent);
  }

  button.play {
    min-width: 54px;
    border-color: var(--accent);
    background: var(--accent);
    color: var(--paper);
  }

  button:disabled {
    opacity: 0.45;
    cursor: default;
  }

  main {
    flex: 1;
    padding: 24px;
    color: var(--muted);
  }

  footer {
    display: flex;
    justify-content: space-between;
    gap: 16px;
    border-top-width: 1px;
    color: var(--muted);
    font-size: 12px;
  }

  footer.failed span:first-child {
    color: var(--error);
  }
</style>
