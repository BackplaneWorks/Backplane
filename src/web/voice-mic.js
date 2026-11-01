// Dictation from this page's own microphone (docs/voice.md): when the page
// is on another machine than its hub, the hub cannot hear the person, so the
// page records 16 kHz mono s16le and uploads it, in order, in ~200 ms
// pieces to the address the hub's start reply named (/voice/in?s=<key>).
// Recording starts on the press (inside the click, so the browser allows
// it) and what is heard before the address comes waits for it. Nothing here
// decides anything about dictation: Bend asks the hub, the hub answers.

const RATE = 16000;
const PIECE_MS = 200;

const WORKLET = `
class Tap extends AudioWorkletProcessor {
  process(inputs) {
    const ch = inputs[0] && inputs[0][0];
    if (ch) this.port.postMessage(ch.slice(0));
    return true;
  }
}
registerProcessor("bp-tap", Tap);
`;

// whether the hub is on this machine (it hears the person itself)
export function hubHere(loc = globalThis.location) {
  const h = (loc && loc.hostname) || "";
  return h === "127.0.0.1" || h === "localhost" || h === "::1" || h === "[::1]";
}

// float samples at `from` Hz to s16le at RATE (linear; the context usually
// runs at RATE already, so this is a copy)
export function toPcm(samples, from) {
  const step = from / RATE;
  const n = Math.floor(samples.length / step);
  const out = new Int16Array(n);
  for (let i = 0; i < n; i++) {
    const x = i * step, k = Math.floor(x), f = x - k;
    const a = samples[k] || 0, b = samples[k + 1] ?? a;
    const v = a * (1 - f) + b * f;
    out[i] = Math.max(-32768, Math.min(32767, Math.round(v * 32767)));
  }
  return new Uint8Array(out.buffer);
}

export class VoiceMic {
  constructor({ fetch: f = globalThis.fetch.bind(globalThis), media = globalThis.navigator?.mediaDevices } = {}) {
    this.fetch = f;
    this.media = media;
    this.reset();
  }
  reset() {
    this.active = false;
    this.url = "";
    this.held = []; // pieces waiting for the address
    this.chain = Promise.resolve();
    this.buf = [];
    this.bufBytes = 0;
    this.stream = null;
    this.ctx = null;
    this.timer = 0;
    this.ending = false;
  }
  async start() {
    if (this.active) return;
    this.reset();
    this.active = true;
    try {
      this.stream = await this.media.getUserMedia({ audio: { channelCount: 1, echoCancellation: true, noiseSuppression: true, autoGainControl: true } });
      const Ctx = globalThis.AudioContext || globalThis.webkitAudioContext;
      try { this.ctx = new Ctx({ sampleRate: RATE }); } catch { this.ctx = new Ctx(); }
      const url = URL.createObjectURL(new Blob([WORKLET], { type: "text/javascript" }));
      await this.ctx.audioWorklet.addModule(url);
      URL.revokeObjectURL(url);
      if (!this.active) return this.release();
      const src = this.ctx.createMediaStreamSource(this.stream);
      const tap = new AudioWorkletNode(this.ctx, "bp-tap");
      const rate = this.ctx.sampleRate;
      tap.port.onmessage = (e) => this.take(toPcm(e.data, rate));
      src.connect(tap);
      this.timer = setInterval(() => this.flush(false), PIECE_MS);
    } catch (err) {
      this.active = false;
      this.release();
      throw err;
    }
  }
  take(bytes) {
    if (!this.active) return;
    this.buf.push(bytes);
    this.bufBytes += bytes.length;
  }
  // where the audio goes (the hub's start reply)
  address(url) {
    if (!url || (!this.active && !this.ending)) return;
    this.url = url;
    for (const p of this.held.splice(0)) this.post(p, false);
    if (this.ending) { this.ending = false; this.post(new Uint8Array(0), true); }
  }
  piece() {
    const b = new Uint8Array(this.bufBytes);
    let at = 0;
    for (const x of this.buf) { b.set(x, at); at += x.length; }
    this.buf = [];
    this.bufBytes = 0;
    return b;
  }
  flush(last) {
    const b = this.piece();
    if (!b.length && !last) return;
    if (!this.url) { this.held.push(b); return; }
    this.post(b, last);
  }
  post(b, last) {
    const url = this.url + (last ? "&end=1" : "");
    this.chain = this.chain.then(() => this.fetch(url, { method: "POST", body: b, credentials: "same-origin", headers: { "content-type": "application/octet-stream" } }).catch(() => {}));
  }
  // the person stopped: what is left goes, with the end mark
  stop() {
    if (!this.active) return;
    clearInterval(this.timer);
    this.timer = 0;
    if (this.url) this.flush(true);
    else { this.held.push(this.piece()); this.ending = true; }
    this.active = false;
    this.release();
  }
  // the hub ended the session (idle, an error): nothing more to send
  gone() {
    clearInterval(this.timer);
    this.active = false;
    this.release();
  }
  release() {
    try { this.stream?.getTracks().forEach((t) => t.stop()); } catch {}
    try { this.ctx?.close(); } catch {}
    this.stream = null;
    this.ctx = null;
  }
}
