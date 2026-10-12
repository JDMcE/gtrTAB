<script lang="ts">
  import { onMount } from 'svelte';
  import type { Cursor, EditorState, Item, Layout } from './model';
  import { pitchOfStep, stepOfPitch } from './pitch';

  interface Props {
    /** Builds the display list for a page of the given width in score pixels. */
    getLayout: (width: number) => Layout;
    /** Changes whenever the score or track does, to trigger a new layout. */
    revision: unknown;
    editor: EditorState;
    /** The key signature of each bar, which decides where a pitch is written on a staff. */
    keys: number[];
    playhead: { bar: number; beat: number } | null;
    zoom: number;
    /** Changes with the colour theme, so that the canvas is repainted in the new colours. */
    theme: string;
    /** A click on the score. `staff` is set when it landed on another staff than the one
     *  being edited, of this track or another. */
    onpick: (
      bar: number,
      beat: number,
      string: number,
      extend: boolean,
      staff: { track: number; staff: number } | null,
    ) => void;
    /** Reports the first bar in view whenever it changes. */
    onview?: (bar: number) => void;
  }

  let { getLayout, revision, editor, keys, playhead, zoom, theme, onpick, onview }: Props = $props();

  const MAX_PAGE_WIDTH = 1100;
  const CURSOR_WIDTH = 20;
  /** How far right of a staff's last beat a click has to be to mean the place after it. */
  const PAST_LAST = 14;

  let scroller: HTMLDivElement;
  let canvas: HTMLCanvasElement;
  let probe: HTMLSpanElement;
  let viewWidth = $state(0);
  let viewHeight = $state(0);
  let scrollTop = $state(0);
  let fontsReady = $state(false);

  const pageWidth = $derived(Math.min(viewWidth / zoom, MAX_PAGE_WIDTH));
  const layout = $derived.by(() => {
    void revision;
    return pageWidth > 0 ? getLayout(pageWidth) : undefined;
  });
  /** Left edge of the page in view pixels, centring it when the view is wider. */
  const offsetX = $derived(Math.max(0, (viewWidth - pageWidth * zoom) / 2));

  /** Centre of a beat or slot on the page, or `undefined` if it is not laid out. */
  function locate(l: Layout, at: { bar: number; beat: number }, string = 0) {
    const bar = l.bars[at.bar];
    if (!bar) return undefined;
    const x = bar.beats[at.beat] ?? bar.slot ?? bar.x + bar.w / 2;
    const system = l.systems[bar.system];
    // On a staff without strings, `string` is a pitch, placed from the staff's middle line.
    const y =
      l.middle === null ?
        system.y + string * l.string_spacing
      : system.y - (stepOfPitch(string, keys[at.bar] ?? 0) - l.middle) * l.step;
    return { x, y, system };
  }

  function itemY(item: Item): number {
    return item.t === 'line' || item.t === 'curve' || item.t === 'beam' ? item.y1 : item.y;
  }

  function draw() {
    const context = canvas?.getContext('2d');
    if (!context || !layout) return;
    // Canvas colours must be plain values. The theme variables use light-dark(), which not
    // every WebView accepts on a canvas, so let CSS resolve each one to rgb() first.
    const color = (name: string) => {
      probe.style.color = `var(${name})`;
      return getComputedStyle(probe).color;
    };
    const colors = { ink: color('--ink'), muted: color('--staff'), error: color('--error') };
    const paper = color('--paper');
    const accent = color('--accent');

    const dpr = window.devicePixelRatio || 1;
    canvas.width = Math.round(viewWidth * dpr);
    canvas.height = Math.round(viewHeight * dpr);
    context.setTransform(1, 0, 0, 1, 0, 0);
    context.fillStyle = paper;
    context.fillRect(0, 0, canvas.width, canvas.height);
    const scale = dpr * zoom;
    context.setTransform(scale, 0, 0, scale, offsetX * dpr, -scrollTop * dpr);

    // Only draw what is on screen, with a margin for tall glyphs.
    const top = scrollTop / zoom - 80;
    const bottom = (scrollTop + viewHeight) / zoom + 80;

    for (const item of layout.items) {
      const y = itemY(item);
      if (y < top || y > bottom) continue;
      switch (item.t) {
        case 'line':
          context.strokeStyle = colors[item.c];
          context.lineWidth = item.w;
          context.beginPath();
          context.moveTo(item.x1, item.y1);
          context.lineTo(item.x2, item.y2);
          context.stroke();
          break;
        case 'rect':
          context.fillStyle = colors[item.c];
          context.fillRect(item.x, item.y, item.w, item.h);
          break;
        case 'dot':
          context.fillStyle = colors.ink;
          context.beginPath();
          context.arc(item.x, item.y, item.r, 0, Math.PI * 2);
          context.fill();
          break;
        case 'beam':
          context.fillStyle = colors.ink;
          context.beginPath();
          context.moveTo(item.x1, item.y1);
          context.lineTo(item.x2, item.y2);
          context.lineTo(item.x2, item.y2 + item.h);
          context.lineTo(item.x1, item.y1 + item.h);
          context.closePath();
          context.fill();
          break;
        case 'arc':
          context.strokeStyle = colors.ink;
          context.lineWidth = 1.1;
          context.beginPath();
          context.moveTo(item.x1, item.y);
          context.quadraticCurveTo((item.x1 + item.x2) / 2, item.y - item.h * 2, item.x2, item.y);
          context.stroke();
          break;
        case 'curve':
          context.strokeStyle = colors.ink;
          context.lineWidth = 1.1;
          context.beginPath();
          context.moveTo(item.x1, item.y1);
          context.quadraticCurveTo(item.cx, item.cy, item.x2, item.y2);
          context.stroke();
          break;
        case 'text': {
          context.font =
            item.font === 'music' ? `${item.size}px Bravura`
            : item.font === 'fret' ? `600 ${item.size}px system-ui, sans-serif`
            : `${item.size}px system-ui, sans-serif`;
          context.textAlign = item.align;
          context.textBaseline = item.font === 'music' ? 'alphabetic' : 'middle';
          if (item.knockout) {
            const m = context.measureText(item.s);
            const left =
              item.align === 'center' ? item.x - m.width / 2
              : item.align === 'right' ? item.x - m.width
              : item.x;
            context.fillStyle = paper;
            context.fillRect(
              left - 1.5,
              item.y - m.actualBoundingBoxAscent - 1,
              m.width + 3,
              m.actualBoundingBoxAscent + m.actualBoundingBoxDescent + 2,
            );
          }
          context.fillStyle = colors[item.c];
          context.fillText(item.s, item.x, item.y);
          break;
        }
      }
    }

    // Selections and the playhead cover every staff of the track being edited.
    const column = (at: { bar: number; beat: number }, fill: string) => {
      const p = locate(layout, at);
      if (!p) return;
      const { cursor_top: top, cursor_bottom: bottom } = p.system;
      context.fillStyle = fill;
      context.fillRect(p.x - CURSOR_WIDTH / 2, top - 8, CURSOR_WIDTH, bottom - top + 16);
    };

    if (editor.selection) {
      const [start, end] = editor.selection;
      const select = color('--select');
      for (let bar = start[0]; bar <= end[0]; bar++) {
        const count = layout.bars[bar]?.beats.length ?? 0;
        const from = bar === start[0] ? start[1] : 0;
        const to = bar === end[0] ? Math.min(end[1], count - 1) : count - 1;
        for (let beat = from; beat <= to; beat++) column({ bar, beat }, select);
      }
    }
    if (playhead) column(playhead, color('--select'));

    const at = locate(layout, editor.cursor, editor.cursor.string);
    if (at && !playhead) {
      context.strokeStyle = accent;
      context.lineWidth = 1.6;
      context.beginPath();
      if (layout.tab) {
        context.roundRect(at.x - CURSOR_WIDTH / 2, at.y - 8, CURSOR_WIDTH, 16, 3);
      } else if (layout.middle !== null) {
        // On a staff without strings the cursor sits on a line or in a space.
        const height = layout.step * 2 + 3;
        context.roundRect(at.x - CURSOR_WIDTH / 2, at.y - height / 2, CURSOR_WIDTH, height, 3);
      } else {
        // Without string lines the cursor marks the beat on the whole staff.
        const { cursor_top: top, cursor_bottom: bottom } = at.system;
        context.roundRect(at.x - CURSOR_WIDTH / 2, top - 6, CURSOR_WIDTH, bottom - top + 12, 3);
      }
      context.stroke();
    }
  }

  $effect(() => {
    void [layout, editor, keys, playhead, zoom, theme, viewWidth, viewHeight, scrollTop, fontsReady];
    draw();
  });

  /** Keeps a position on screen, scrolling only when it is out of view. */
  function reveal(target: Cursor | { bar: number; beat: number } | null) {
    if (!layout || !target || !scroller) return;
    const p = locate(layout, target);
    if (!p) return;
    const top = p.system.top * zoom;
    const bottom = p.system.bottom * zoom;
    if (top < scroller.scrollTop) scroller.scrollTop = Math.max(0, top - 12);
    else if (bottom > scroller.scrollTop + viewHeight) {
      scroller.scrollTop = Math.min(top - 12, bottom - viewHeight + 12);
    }
  }

  $effect(() => reveal(playhead ?? editor.cursor));

  /** The first bar of the first line that is at least partly in view. */
  const firstVisible = $derived.by(() => {
    if (!layout) return 0;
    const system = layout.systems.findIndex((s) => s.bottom * zoom > scrollTop);
    return Math.max(0, layout.bars.findIndex((b) => b.system === Math.max(0, system)));
  });
  $effect(() => onview?.(firstVisible));

  function onPointerDown(event: PointerEvent) {
    if (!layout || event.button !== 0) return;
    const bounds = scroller.getBoundingClientRect();
    const x = (event.clientX - bounds.left - offsetX) / zoom;
    const y = (event.clientY - bounds.top + scroller.scrollTop) / zoom;

    const system = layout.systems.findIndex((s) => y >= s.top && y <= s.bottom);
    if (system < 0) return;
    const inSystem = layout.bars.map((b, i) => ({ b, i })).filter(({ b }) => b.system === system);
    const hit =
      inSystem.find(({ b }) => x >= b.x && x < b.x + b.w) ??
      (x < inSystem[0].b.x ? inSystem[0] : inSystem[inSystem.length - 1]);

    const stops = [...hit.b.beats, ...(hit.b.slot === null ? [] : [hit.b.slot])];
    let beat = 0;
    stops.forEach((stop, i) => {
      if (Math.abs(stop - x) < Math.abs(stops[beat] - x)) beat = i;
    });
    /** The pitch written at `y` on a staff without strings whose middle line is at `lineY`. */
    const pitchAt = (lineY: number, middle: number) =>
      pitchOfStep(middle + Math.round((lineY - y) / layout.step), keys[hit.i] ?? 0);
    const rows = layout.systems[system].rows;
    const row = rows.find((r) => y >= r.top && y <= r.bottom);
    if (row && (row.track !== editor.cursor.track || row.staff !== editor.cursor.staff)) {
      // Another staff: it has its own beats, the nearest of which is the one clicked. A
      // click well past the last of them is on the empty place after it, if there is one.
      const own = row.beats[hit.i - inSystem[0].i] ?? [];
      let at = 0;
      own.forEach((stop, i) => {
        if (Math.abs(stop - x) < Math.abs(own[at] - x)) at = i;
      });
      if (own.length && x > own[own.length - 1] + PAST_LAST) at = own.length;
      const staff = { track: row.track, staff: row.staff };
      if (row.middle !== null) {
        onpick(hit.i, at, pitchAt(row.y, row.middle), false, staff);
        return;
      }
      const string = row.strings ? Math.round((y - row.y) / layout.string_spacing) : 0;
      const clamped = Math.max(0, Math.min(row.strings - 1, string));
      onpick(hit.i, at, clamped, false, staff);
      return;
    }
    const string =
      layout.tab ? Math.round((y - layout.systems[system].y) / layout.string_spacing)
      : layout.middle !== null ? pitchAt(layout.systems[system].y, layout.middle)
      : editor.cursor.string;
    const clamped = Math.max(0, Math.min(layout.string_count - 1, string));
    onpick(hit.i, beat, clamped, event.shiftKey, null);
  }

  onMount(() => {
    const observer = new ResizeObserver(() => {
      viewWidth = scroller.clientWidth;
      viewHeight = scroller.clientHeight;
    });
    observer.observe(scroller);
    document.fonts.load('30px Bravura').then(() => (fontsReady = true));
    const scheme = window.matchMedia('(prefers-color-scheme: dark)');
    scheme.addEventListener('change', draw);
    return () => {
      observer.disconnect();
      scheme.removeEventListener('change', draw);
    };
  });
</script>

<div class="score">
  <canvas bind:this={canvas}></canvas>
  <span bind:this={probe} hidden></span>
  <!-- The score takes the keyboard whenever no other control has it. -->
  <!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
  <div
    class="scroller"
    role="application"
    aria-label="Score"
    tabindex="0"
    data-score
    bind:this={scroller}
    onscroll={() => (scrollTop = scroller.scrollTop)}
    onpointerdown={onPointerDown}
  >
    <div style:height="{(layout?.height ?? 0) * zoom}px"></div>
  </div>
</div>

<style>
  .score {
    position: relative;
    flex: 1;
    min-width: 0;
    min-height: 0;
    background: var(--paper);
  }

  canvas,
  .scroller {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
  }

  .scroller {
    overflow-y: auto;
    cursor: text;
  }

  /* The score has the keyboard most of the time, so its focus mark is a line along the top
     edge rather than a ring around the whole view. */
  .scroller:focus-visible {
    outline: none;
    box-shadow: inset 0 2px 0 var(--focus);
  }
</style>
