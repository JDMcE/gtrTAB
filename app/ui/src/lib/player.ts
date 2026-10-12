import synthUrl from '../wasm/gtr_worklet.wasm?url';
import type { BeatTime, Timeline, Track } from './model';
import workletUrl from './synth.worklet.js?url&no-inline';

/** Where a track sits in the mix. */
export interface TrackMix {
  /** False for a track that is muted, or left out by another track's solo. */
  audible: boolean;
  gain: number;
  /** From -1 (left) to 1 (right). */
  pan: number;
}

/** Volume setting at which a track plays at its natural level, as in `gtr_model::Mix`. */
const UNITY_VOLUME = 75;

/** The mix the tracks' settings ask for. */
export function mixOf(tracks: Track[]): TrackMix[] {
  const anySolo = tracks.some((t) => t.mix.solo);
  return tracks.map(({ mix }) => {
    const audible = !mix.mute && (mix.solo || !anySolo);
    return { audible, gain: (mix.volume / UNITY_VOLUME) ** 2, pan: mix.pan / 100 };
  });
}

/** What the synth reports back from the audio thread (see synth.worklet.js). */
type Report =
  | { type: 'position'; id: number; seconds: number; at: number }
  | { type: 'ended'; id: number }
  | { type: 'pass'; id: number }
  | { type: 'soundfont'; id: number; error: number };

/** Why the synth would not take a SoundFont, by the number it answers with. */
const SOUNDFONT_ERRORS = [
  '',
  'That is not a SoundFont file.',
  'Compressed SoundFonts (.sf3) are not supported; use an .sf2 file.',
  'That SoundFont is damaged.',
];

/**
 * Plays a passage through the synth, which runs on the audio thread and makes the sound as
 * it goes. The passage can be swapped for a newer version of itself and the tracks mixed
 * while it plays. Reports which beat is sounding.
 */
export class Player {
  private context?: AudioContext;
  private synth?: Promise<AudioWorkletNode>;
  /** Numbers each passage sent to the synth, so its reports can be told apart. */
  private sent = 0;
  /** The passage playing: the number of its latest version, or 0 when stopped. */
  private current = 0;
  private beats: BeatTime[] = [];
  /** Beats of a version the synth has been sent but has not started on yet. */
  private next?: { id: number; beats: BeatTime[] };
  /** Beats of the passage a loop goes on to the next time round, if it has been sent one. */
  private queued?: { id: number; beats: BeatTime[] };
  /** The last the synth said of how far it had got, and when by the context's clock. */
  private heard?: { seconds: number; at: number };
  private onEnd?: () => void;
  private onPass?: (fromQueue: boolean) => void;
  /** SoundFonts sent to the synth that it has yet to answer for. */
  private fonts = new Map<number, (error: number) => void>();

  get playing(): boolean {
    return this.current !== 0;
  }

  /** Starts the synth on the audio thread, the first time it is needed. */
  private start(context: AudioContext): Promise<AudioWorkletNode> {
    return (this.synth ??= (async () => {
      const [wasm] = await Promise.all([
        fetch(synthUrl).then((response) => response.arrayBuffer()),
        context.audioWorklet.addModule(workletUrl),
      ]);
      const node = new AudioWorkletNode(context, 'gtr-synth', {
        numberOfInputs: 0,
        outputChannelCount: [2],
        processorOptions: { wasm },
      });
      node.port.onmessage = (event: MessageEvent<Report>) => this.receive(event.data);
      node.connect(context.destination);
      return node;
    })());
  }

  private receive(report: Report): void {
    if (report.type === 'soundfont') {
      this.fonts.get(report.id)?.(report.error);
      this.fonts.delete(report.id);
      return;
    }
    const fromQueue = report.id === this.queued?.id;
    // The synth has started on a version it was sent: a newer one, or the one queued.
    if (report.id === this.next?.id) {
      this.current = report.id;
      this.beats = this.next.beats;
      this.next = undefined;
    } else if (report.id === this.queued?.id) {
      this.current = report.id;
      this.beats = this.queued.beats;
      this.queued = undefined;
    }
    if (report.id !== this.current) return;
    if (report.type === 'position') {
      this.heard = { seconds: report.seconds, at: report.at };
      return;
    }
    if (report.type === 'pass') {
      this.onPass?.(fromQueue);
      return;
    }
    const onEnd = this.onEnd;
    this.halt();
    onEnd?.();
  }

  private halt(): void {
    this.current = 0;
    this.next = this.queued = undefined;
    this.heard = undefined;
    this.onEnd = this.onPass = undefined;
  }

  /**
   * Has the synth play from the samples of a SoundFont, given as the bytes of an `.sf2`
   * file, or with `null` from its built-in models. It takes effect from the next note.
   * Resolves to what was wrong with the file, or `null` if it was taken.
   */
  async setSoundFont(bytes: ArrayBuffer | null): Promise<string | null> {
    const synth = await this.start((this.context ??= new AudioContext()));
    const id = ++this.sent;
    const error = await new Promise<number>((resolve) => {
      this.fonts.set(id, resolve);
      synth.port.postMessage({ type: 'soundfont', id, bytes }, bytes ? [bytes] : []);
    });
    return SOUNDFONT_ERRORS[error] || null;
  }

  /**
   * Starts playback, stopping anything already playing. `events` is the passage for the
   * synth, as `Passage.events` gives it. `onEnd` fires when the passage runs out, and
   * `onPass` each time a looped one goes back to its start, saying whether it went on to
   * the version queued for it.
   */
  async play(
    events: Uint8Array,
    timeline: Timeline,
    onEnd: () => void,
    onPass?: (fromQueue: boolean) => void,
  ): Promise<void> {
    this.stop();
    const context = (this.context ??= new AudioContext());
    const [synth] = await Promise.all([this.start(context), context.resume()]);
    this.current = ++this.sent;
    this.beats = timeline.beats;
    this.onEnd = onEnd;
    this.onPass = onPass;
    const bytes = events.buffer as ArrayBuffer;
    synth.port.postMessage({ type: 'play', id: this.current, events: bytes }, [bytes]);
  }

  /**
   * Swaps what is playing for a newer version of the same passage, as after an edit.
   * Playback carries on from the beat it had reached.
   */
  async update(events: Uint8Array, timeline: Timeline): Promise<void> {
    if (!this.playing || !this.synth) return;
    const id = ++this.sent;
    this.next = { id, beats: timeline.beats };
    const bytes = events.buffer as ArrayBuffer;
    (await this.synth).port.postMessage({ type: 'replace', id, events: bytes }, [bytes]);
  }

  /**
   * Has a looped passage go on to another version of itself the next time it comes round,
   * as when it is to be played faster, or with `null` to the same version again.
   */
  async queue(next: { events: Uint8Array; timeline: Timeline } | null): Promise<void> {
    if (!this.playing || !this.synth) return;
    const synth = await this.synth;
    if (!next) {
      this.queued = undefined;
      synth.port.postMessage({ type: 'queue', events: null });
      return;
    }
    const id = ++this.sent;
    this.queued = { id, beats: next.timeline.beats };
    const bytes = next.events.buffer as ArrayBuffer;
    synth.port.postMessage({ type: 'queue', id, events: bytes }, [bytes]);
  }

  /** Changes the mix of what is playing. */
  async setMix(mix: TrackMix[]): Promise<void> {
    if (this.playing) (await this.synth)?.port.postMessage({ type: 'mix', mix });
  }

  stop(): void {
    if (!this.playing) return;
    this.halt();
    void this.synth?.then((synth) => synth.port.postMessage({ type: 'stop' }));
  }

  /** The beat sounding now, or `null` before the first beat or when stopped. */
  position(): BeatTime | null {
    if (!this.playing || !this.context || !this.heard) return null;
    const time = this.heard.seconds + (this.context.currentTime - this.heard.at);
    let low = 0;
    let high = this.beats.length;
    while (low < high) {
      const mid = (low + high) >> 1;
      if (this.beats[mid].time <= time) low = mid + 1;
      else high = mid;
    }
    return low > 0 ? this.beats[low - 1] : null;
  }
}
