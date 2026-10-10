import type { BeatTime, Timeline } from './model';

/** Plays a pre-rendered buffer and reports which beat is sounding. */
export class Player {
  private context?: AudioContext;
  private source?: AudioBufferSourceNode;
  private startedAt = 0;
  private beats: BeatTime[] = [];

  /** Sample rate audio should be rendered at. */
  get sampleRate(): number {
    this.context ??= new AudioContext();
    return this.context.sampleRate;
  }

  get playing(): boolean {
    return this.source !== undefined;
  }

  /** Starts playback, stopping anything already playing. `onEnd` fires when it runs out. */
  async play(samples: Float32Array, timeline: Timeline, onEnd: () => void): Promise<void> {
    this.stop();
    const context = (this.context ??= new AudioContext());
    await context.resume();
    const buffer = context.createBuffer(1, Math.max(samples.length, 1), context.sampleRate);
    buffer.getChannelData(0).set(samples);
    const source = context.createBufferSource();
    source.buffer = buffer;
    source.connect(context.destination);
    source.onended = () => {
      if (this.source !== source) return;
      this.source = undefined;
      onEnd();
    };
    this.beats = timeline.beats;
    this.startedAt = context.currentTime;
    this.source = source;
    source.start();
  }

  stop(): void {
    const source = this.source;
    this.source = undefined;
    source?.stop();
  }

  /** The beat sounding now, or `null` before the first beat or when stopped. */
  position(): BeatTime | null {
    if (!this.source || !this.context) return null;
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
