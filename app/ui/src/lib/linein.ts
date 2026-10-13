import synthUrl from '../wasm/gtr_worklet.wasm?url';
import workletUrl from './synth.worklet.js?url&no-inline';

/** How loud the line is going in and coming out, from 0 to 1 at full scale. */
export interface Levels {
  input: number;
  output: number;
}

/**
 * A guitar plugged into a line in, or played into a microphone, heard through the pedals
 * of a track and of the mix as it is played. The pedals run in an AudioWorklet, in the
 * same module as the synth (`gtr-line` in synth.worklet.js).
 */
export class LineIn {
  private context?: AudioContext;
  private stream?: MediaStream;
  private node?: AudioWorkletNode;
  /** Seconds between a string being played and its sound coming out, as far as is known. */
  latency = 0;
  onlevels?: (levels: Levels) => void;

  get running(): boolean {
    return this.node !== undefined;
  }

  /**
   * Opens the input (`''` for the system's default) and starts playing it through the
   * pedals given as `Document.chainBytes` gives them. Throws if the input cannot be opened.
   */
  async start(deviceId: string, track: Uint8Array, master: Uint8Array, gain: number): Promise<void> {
    this.stop();
    const stream = await navigator.mediaDevices.getUserMedia({
      audio: {
        deviceId: deviceId ? { exact: deviceId } : undefined,
        // An instrument wants its sound as it is, not cleaned up for speech, and as soon
        // as it can be had.
        echoCancellation: false,
        noiseSuppression: false,
        autoGainControl: false,
        latency: 0,
      } as MediaTrackConstraints,
    });
    const context = new AudioContext({ latencyHint: 'interactive' });
    try {
      const [wasm] = await Promise.all([
        fetch(synthUrl).then((response) => response.arrayBuffer()),
        context.audioWorklet.addModule(workletUrl),
      ]);
      const node = new AudioWorkletNode(context, 'gtr-line', {
        numberOfInputs: 1,
        numberOfOutputs: 1,
        outputChannelCount: [2],
        processorOptions: { wasm },
      });
      node.port.onmessage = (event: MessageEvent<{ type: 'level' } & Levels>) =>
        this.onlevels?.({ input: event.data.input, output: event.data.output });
      context.createMediaStreamSource(stream).connect(node).connect(context.destination);
      await context.resume();
      [this.context, this.stream, this.node] = [context, stream, node];
    } catch (error) {
      stream.getTracks().forEach((t) => t.stop());
      void context.close();
      throw error;
    }
    this.setEffects(false, track);
    this.setEffects(true, master);
    this.setGain(gain);
    const settings = stream.getAudioTracks()[0]?.getSettings() as { latency?: number } | undefined;
    this.latency = (settings?.latency ?? 0) + context.baseLatency + (context.outputLatency || 0);
  }

  /** Changes the pedals of the track, or with `master` those of the mix. */
  setEffects(master: boolean, chain: Uint8Array): void {
    const bytes = chain.slice().buffer as ArrayBuffer;
    this.node?.port.postMessage({ type: 'effects', master, chain: bytes }, [bytes]);
  }

  /** Brings the line up or down by a factor before the pedals. */
  setGain(gain: number): void {
    this.node?.port.postMessage({ type: 'gain', gain });
  }

  stop(): void {
    this.stream?.getTracks().forEach((t) => t.stop());
    this.node?.disconnect();
    void this.context?.close();
    [this.context, this.stream, this.node] = [undefined, undefined, undefined];
  }
}
