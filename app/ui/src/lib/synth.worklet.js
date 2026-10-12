// The audio thread's side of playback: runs the synth (crates/gtr-worklet) and copies the
// samples it makes to the output. Plain JavaScript without imports, since that is all an
// AudioWorklet is sure to load. The other side is player.ts.

/** Blocks between reports of how far playback has got. */
const REPORT_EVERY = 8;

class SynthProcessor extends AudioWorkletProcessor {
  constructor(options) {
    super();
    const module = new WebAssembly.Module(options.processorOptions.wasm);
    this.wasm = new WebAssembly.Instance(module, {}).exports;
    this.synth = this.wasm.gtr_new(sampleRate);
    this.playing = false;
    /** Which passage is playing, as player.ts numbers them. */
    this.id = 0;
    /** Which passage a loop goes on to the next time round; 0 for none. */
    this.queued = 0;
    /** Times the passage playing has gone round its loop. */
    this.passes = 0;
    this.blocks = 0;
    this.port.onmessage = (event) => this.receive(event.data);
  }

  receive(message) {
    switch (message.type) {
      case 'play':
      case 'replace': {
        // A newer version of a passage that has since ended has nothing to replace.
        if (message.type === 'replace' && !this.playing) return;
        const events = new Uint8Array(message.events);
        const at = this.wasm.gtr_input(this.synth, events.length);
        new Uint8Array(this.wasm.memory.buffer, at, events.length).set(events);
        if (!this.wasm.gtr_load(this.synth, message.type === 'replace' ? 1 : 0)) return;
        this.id = message.id;
        if (message.type === 'play') {
          this.queued = 0;
          this.passes = 0;
        }
        this.playing = true;
        this.report();
        break;
      }
      case 'queue': {
        // The passage a loop goes on to, or nothing for the same one again.
        if (!this.playing) return;
        if (!message.events) {
          this.wasm.gtr_queue(this.synth, 1);
          this.queued = 0;
          return;
        }
        const events = new Uint8Array(message.events);
        const at = this.wasm.gtr_input(this.synth, events.length);
        new Uint8Array(this.wasm.memory.buffer, at, events.length).set(events);
        if (this.wasm.gtr_queue(this.synth, 0)) this.queued = message.id;
        break;
      }
      case 'mix':
        message.mix.forEach((track, index) => {
          this.wasm.gtr_set_mix(this.synth, index, track.audible ? 1 : 0, track.gain, track.pan);
        });
        break;
      case 'soundfont': {
        // The bank to play from, as the bytes of an .sf2 file, or nothing for the models.
        let error = 0;
        if (message.bytes) {
          const bytes = new Uint8Array(message.bytes);
          const at = this.wasm.gtr_input(this.synth, bytes.length);
          new Uint8Array(this.wasm.memory.buffer, at, bytes.length).set(bytes);
          error = this.wasm.gtr_soundfont(this.synth);
        } else {
          this.wasm.gtr_no_soundfont(this.synth);
        }
        this.port.postMessage({ type: 'soundfont', id: message.id, error });
        break;
      }
      case 'stop':
        this.playing = false;
        break;
    }
  }

  /** Tells the player where in the passage the block about to sound is. */
  report() {
    this.port.postMessage({
      type: 'position',
      id: this.id,
      seconds: this.wasm.gtr_seconds(this.synth),
      at: currentTime,
    });
  }

  process(_inputs, outputs) {
    const [left, right] = outputs[0];
    if (!this.playing || !left) return true;
    if (this.blocks++ % REPORT_EVERY === 0) this.report();
    const more = this.wasm.gtr_process(this.synth, left.length);
    const passes = this.wasm.gtr_passes(this.synth);
    if (passes !== this.passes) {
      // Round the loop, and on to the passage queued if there was one.
      this.passes = passes;
      if (this.queued && !this.wasm.gtr_queued(this.synth)) this.id = this.queued;
      this.queued = 0;
      this.port.postMessage({ type: 'pass', id: this.id });
      // Say where it has got to before the next block, as playback has gone back.
      this.blocks = 0;
    }
    // The module's memory moves when it grows, so look the samples up afresh each time.
    const { buffer } = this.wasm.memory;
    left.set(new Float32Array(buffer, this.wasm.gtr_left(this.synth), left.length));
    (right ?? left).set(new Float32Array(buffer, this.wasm.gtr_right(this.synth), left.length));
    if (!more) {
      this.playing = false;
      this.port.postMessage({ type: 'ended', id: this.id });
    }
    return true;
  }
}

registerProcessor('gtr-synth', SynthProcessor);
