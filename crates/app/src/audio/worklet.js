// AudioWorkletProcessor hosting the Stabkussion DSP (stabkussion-worklet.wasm).
//
// The main thread compiles the wasm module and hands it over through
// processorOptions; the module has no imports, so instantiation is synchronous.
//
// Messages on `port` are small arrays:
//   [0, voice, paramId, value]  set a voice parameter
//   [1, value]                  master volume
//   [2, voice, velocity, snap]  trigger; snap=1 first jumps that voice's
//                               smoothed parameters to their targets
//   [3]                         snap smoothed parameters to their targets

const GLOBAL = 255;
const GLOBAL_MASTER = 0;

class StabkussionProcessor extends AudioWorkletProcessor {
  constructor(options) {
    super();
    this.dsp = new WebAssembly.Instance(options.processorOptions.module, {}).exports;
    this.handle = this.dsp.sk_new(sampleRate);
    this.views = null;
    this.port.onmessage = (e) => this.onMessage(e.data);
  }

  onMessage(m) {
    const d = this.dsp;
    switch (m[0]) {
      case 0:
        d.sk_set_param(this.handle, m[1], m[2], m[3]);
        break;
      case 1:
        d.sk_set_param(this.handle, GLOBAL, GLOBAL_MASTER, m[1]);
        break;
      case 2:
        if (m[3]) d.sk_snap_voice(this.handle, m[1]);
        d.sk_trigger(this.handle, m[1], m[2]);
        break;
      case 3:
        d.sk_snap(this.handle);
        break;
    }
  }

  // (Re)create the Float32Array views if wasm memory was replaced by a grow.
  outputViews(frames) {
    const buffer = this.dsp.memory.buffer;
    const v = this.views;
    if (v === null || v.buffer !== buffer || v.frames !== frames) {
      this.views = {
        buffer,
        frames,
        left: new Float32Array(buffer, this.dsp.sk_left(this.handle), frames),
        right: new Float32Array(buffer, this.dsp.sk_right(this.handle), frames),
      };
    }
    return this.views;
  }

  process(_inputs, outputs) {
    const out = outputs[0];
    const frames = out[0].length;
    this.dsp.sk_render(this.handle, frames);
    const v = this.outputViews(frames);
    out[0].set(v.left);
    if (out.length > 1) out[1].set(v.right);
    return true;
  }
}

registerProcessor("stabkussion", StabkussionProcessor);
