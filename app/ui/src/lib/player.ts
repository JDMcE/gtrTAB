import type { BeatTime, Timeline, Track } from './model';

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
/** Seconds a change to the mix takes, so that it does not click. */
const MIX_GLIDE = 0.015;
/** How far above full level the mix may go before the limiter flattens it completely. */
const HEADROOM = 4;

/** The mix the tracks' settings ask for. */
export function mixOf(tracks: Track[]): TrackMix[] {
  const anySolo = tracks.some((t) => t.mix.solo);
  return tracks.map(({ mix }) => {
    const audible = !mix.mute && (mix.solo || !anySolo);
    return { audible, gain: (mix.volume / UNITY_VOLUME) ** 2, pan: mix.pan / 100 };
  });
}

interface Strip {
  /** Open for a track that is heard, shut for one that is muted. */
  gate: GainNode;
  /** The track's volume: the mixer's, until the score changes it. */
  level: GainNode;
  pan: StereoPannerNode;
  /** The score sets these itself part-way through, so the mixer leaves them alone. */
  scored: { level: boolean; pan: boolean };
}

/**
 * Plays a passage rendered as one buffer per track, mixing the tracks as they play so
 * that volume, pan, mute and solo can change mid-passage, and reports which beat is
 * sounding.
 */
export class Player {
  private context?: AudioContext;
  private output?: AudioNode;
  private sources: AudioBufferSourceNode[] = [];
  /** One per track of the passage; `null` for a track with nothing to play. */
  private strips: (Strip | null)[] = [];
  private startedAt = 0;
  private beats: BeatTime[] = [];

  /** Sample rate audio should be rendered at. */
  get sampleRate(): number {
    this.context ??= new AudioContext();
    return this.context.sampleRate;
  }

  get playing(): boolean {
    return this.sources.length > 0;
  }

  /** The end of the chain: a soft limiter, so that loud passages round off instead of clipping. */
  private master(context: AudioContext): AudioNode {
    if (this.output) return this.output;
    const input = context.createGain();
    input.gain.value = 1 / HEADROOM;
    const limiter = context.createWaveShaper();
    const curve = new Float32Array(4097);
    for (let i = 0; i < curve.length; i++) {
      curve[i] = Math.tanh(((i / (curve.length - 1)) * 2 - 1) * HEADROOM);
    }
    limiter.curve = curve;
    input.connect(limiter).connect(context.destination);
    return (this.output = input);
  }

  /**
   * Starts playback, stopping anything already playing. `stems` holds each track's
   * samples, or `null` for a track without notes; `clicks` the metronome. `onEnd` fires
   * when the passage runs out.
   */
  async play(
    stems: (Float32Array | null)[],
    clicks: Float32Array | null,
    timeline: Timeline,
    mix: TrackMix[],
    onEnd: () => void,
  ): Promise<void> {
    this.stop();
    const context = (this.context ??= new AudioContext());
    await context.resume();
    const master = this.master(context);

    const source = (samples: Float32Array) => {
      const buffer = context.createBuffer(1, samples.length, context.sampleRate);
      buffer.getChannelData(0).set(samples);
      const node = context.createBufferSource();
      node.buffer = buffer;
      return node;
    };
    const sources: AudioBufferSourceNode[] = [];
    const startAt = context.currentTime + 0.02;
    this.strips = stems.map((samples, index) => {
      if (!samples) return null;
      const node = source(samples);
      const gate = context.createGain();
      const level = context.createGain();
      const pan = context.createStereoPanner();
      gate.gain.value = (mix[index]?.audible ?? true) ? 1 : 0;
      // The panner halves the power of a centred track; make up for it.
      level.gain.value = (mix[index]?.gain ?? 1) * Math.SQRT2;
      pan.pan.value = mix[index]?.pan ?? 0;
      // Changes of volume and pan written in the score happen on their beats.
      const scored = { level: false, pan: false };
      for (const change of timeline.automation) {
        if (change.channel !== index) continue;
        if (change.gain !== null) {
          level.gain.setValueAtTime(change.gain * Math.SQRT2, startAt + change.time);
          scored.level = true;
        }
        if (change.pan !== null) {
          pan.pan.setValueAtTime(change.pan, startAt + change.time);
          scored.pan = true;
        }
      }
      node.connect(gate).connect(level).connect(pan).connect(master);
      sources.push(node);
      return { gate, level, pan, scored };
    });
    if (clicks) {
      const node = source(clicks);
      node.connect(master);
      sources.push(node);
    }
    if (sources.length === 0) {
      // Nothing to hear, but the passage still takes its time.
      const length = Math.max(1, Math.ceil(timeline.duration * context.sampleRate));
      sources.push(source(new Float32Array(length)));
    }

    // Every buffer is the same length, so the first to finish ends the passage.
    sources[0].onended = () => {
      if (this.sources !== sources) return;
      this.sources = [];
      this.strips = [];
      onEnd();
    };
    this.beats = timeline.beats;
    this.sources = sources;
    this.startedAt = startAt;
    for (const node of sources) node.start(startAt);
  }

  /**
   * Changes the mix of what is playing. Ignored if the tracks are not the ones playing.
   * Mute and solo always apply; volume and pan do unless the score itself changes them
   * during the passage, in which case the score has the say.
   */
  setMix(mix: TrackMix[]): void {
    if (!this.context || mix.length !== this.strips.length) return;
    const now = this.context.currentTime;
    this.strips.forEach((strip, index) => {
      if (!strip) return;
      strip.gate.gain.setTargetAtTime(mix[index].audible ? 1 : 0, now, MIX_GLIDE);
      if (!strip.scored.level) {
        strip.level.gain.setTargetAtTime(mix[index].gain * Math.SQRT2, now, MIX_GLIDE);
      }
      if (!strip.scored.pan) strip.pan.pan.setTargetAtTime(mix[index].pan, now, MIX_GLIDE);
    });
  }

  stop(): void {
    const sources = this.sources;
    this.sources = [];
    this.strips = [];
    for (const node of sources) node.stop();
  }

  /** The beat sounding now, or `null` before the first beat or when stopped. */
  position(): BeatTime | null {
    if (!this.playing || !this.context) return null;
    const time = this.context.currentTime - this.startedAt;
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
