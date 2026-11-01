// The local engine: sherpa-onnx's Node addon (loaded at run time from the
// downloaded runtime, never bundled), NVIDIA Parakeet TDT and Silero VAD.
//
// One model does both the live words and the final ones: while a segment is
// open its audio so far is decoded again every DECODE_MS, and each decode is
// sent as a `hyp` with every word's start, end and probability. Backplane
// (Bend: core/voice.bend's Settle) freezes the words two decodes agree on,
// so what is shown never changes; the segment's last decode is its `done`.
import { createRequire } from "node:module";
import { existsSync, mkdirSync, readdirSync, renameSync, rmSync, statSync, writeFileSync } from "node:fs";
import { createHash } from "node:crypto";
import { arch, platform } from "node:os";
import { join } from "node:path";

export const RATE = 16000;
const DECODE_MS = 300; // a live decode at most this often
const PRE_ROLL = 0.4; // seconds kept before speech starts
const TAIL = 0.25; // seconds kept after the VAD's end
const MAX_SEG = 20; // a segment is cut after this many seconds
const MIN_SPEECH = 0.25; // shorter speech is a click, not words

// ---- what is downloaded, pinned

export const RUNTIME_VERSION = "2.0.1";
type Pin = { url: string; size?: number; sha256?: string; sha512?: string };
const RUNTIMES: Record<string, Pin> = {
  "linux-x64": { url: "https://registry.npmjs.org/sherpa-onnx-linux-x64/-/sherpa-onnx-linux-x64-2.0.1.tgz", sha512: "w5mRRrIvPvcNWac7rQ8AN/t/cwElPGoRpH+ceCRBV7XfjCIQ9Rcr3Ig/kXcJLNOxgK4bGmef+Y47lIWiesDODw==" },
  "linux-arm64": { url: "https://registry.npmjs.org/sherpa-onnx-linux-arm64/-/sherpa-onnx-linux-arm64-2.0.1.tgz", sha512: "5CJvLjFYIg23oELCsT7l07fMLT3PdquZ3tqh7KZK3Pzi+c2YiPKRe1DDUxoNdtCaw3ZiS4aQSQPBblB9JStuDA==" },
  "darwin-arm64": { url: "https://registry.npmjs.org/sherpa-onnx-darwin-arm64/-/sherpa-onnx-darwin-arm64-2.0.1.tgz", sha512: "agXmbbwiVPUSMViMhWhYAt59iG21ui7x6HxQFU35XzHOfQe9x6Pg9V160Pzak/66x0ePMjixsLUtMaaUrLfCwQ==" },
  "darwin-x64": { url: "https://registry.npmjs.org/sherpa-onnx-darwin-x64/-/sherpa-onnx-darwin-x64-2.0.1.tgz", sha512: "SBnHa3LDOSI/5/rpBHGxKQDFq+yIdxowYY0InLzfojTUx+yGgqu7cYBrRxjzvHOiOKi0O4rmlFVZboyhIqpDgA==" },
  "win-x64": { url: "https://registry.npmjs.org/sherpa-onnx-win-x64/-/sherpa-onnx-win-x64-2.0.1.tgz", sha512: "7vwdAw9PGEb0JLSpK7UFxAdp1vabBHK9Nhw2jFRBWSFgtslUPHIB0T6syJfRgiUGyCeNW9fJvcFAk2F2u7hBEw==" },
};
const GH = "https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/";
export const MODELS: Record<string, { label: string; dir: string; pin: Pin }> = {
  "parakeet-v2": {
    label: "Parakeet TDT 0.6B v2 (English)",
    dir: "sherpa-onnx-nemo-parakeet-tdt-0.6b-v2-int8",
    pin: { url: GH + "sherpa-onnx-nemo-parakeet-tdt-0.6b-v2-int8.tar.bz2", size: 482468385, sha256: "157c157bc51155e03e37d2466522a3a737dd9c72bb25f36eb18912964161e1ad" },
  },
  "parakeet-v3": {
    label: "Parakeet TDT 0.6B v3 (25 languages)",
    dir: "sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8",
    pin: { url: GH + "sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8.tar.bz2", size: 487170055, sha256: "5793d0fd397c5778d2cf2126994d58e9d56b1be7c04d13c7a15bb1b4eafb16bf" },
  },
};
const VAD: Pin = { url: GH + "silero_vad.onnx", size: 643854, sha256: "9e2449e1087496d8d4caba907f23e0bd3f78d91fa552479bb9c23ac09cbb1fd6" };

export function plat(): string {
  return `${platform() === "win32" ? "win" : platform()}-${arch()}`;
}

const runtimeDir = (dir: string) => join(dir, `runtime-${RUNTIME_VERSION}-${plat()}`);
const vadPath = (dir: string) => join(dir, "silero_vad.onnx");
const modelDir = (dir: string, id: string) => join(dir, MODELS[id].dir);

// What is installed under DIR (<home>/voice/local).
export function status(dir: string) {
  const models = Object.entries(MODELS).map(([id, m]) => ({
    id,
    label: m.label,
    size: m.pin.size ?? 0,
    installed: existsSync(join(modelDir(dir, id), "encoder.int8.onnx")) && existsSync(join(modelDir(dir, id), "tokens.txt")),
  }));
  return {
    platform: plat(),
    supported: plat() in RUNTIMES,
    runtime: existsSync(join(runtimeDir(dir), "sherpa-onnx.node")),
    vad: existsSync(vadPath(dir)),
    models,
  };
}

export function ready(dir: string, id: string): string | undefined {
  const s = status(dir);
  if (!s.supported) return `local dictation is not available on ${s.platform}`;
  if (!(id in MODELS)) return `unknown model ${id}`;
  if (!s.runtime || !s.vad || !s.models.find((m) => m.id === id)?.installed) return "the local model is not downloaded (Settings → Voice)";
  return undefined;
}

// ---- install

type Progress = (what: string, done: number, total: number) => void;

async function fetchTo(pin: Pin, path: string, what: string, progress: Progress) {
  const res = await fetch(pin.url, { redirect: "follow" });
  if (!res.ok || !res.body) throw new Error(`${what}: HTTP ${res.status}`);
  const total = pin.size ?? Number(res.headers.get("content-length") ?? 0);
  const h256 = createHash("sha256");
  const h512 = createHash("sha512");
  const out = Bun.file(path).writer();
  let done = 0;
  let last = 0;
  for await (const chunk of res.body) {
    h256.update(chunk);
    h512.update(chunk);
    out.write(chunk);
    done += chunk.length;
    if (Date.now() - last > 250) {
      last = Date.now();
      progress(what, done, total);
    }
  }
  await out.end();
  progress(what, done, total);
  if (pin.size !== undefined && done !== pin.size) throw new Error(`${what}: ${done} bytes, expected ${pin.size}`);
  if (pin.sha256 && h256.digest("hex") !== pin.sha256) throw new Error(`${what}: checksum mismatch`);
  if (pin.sha512 && h512.digest("base64") !== pin.sha512) throw new Error(`${what}: checksum mismatch`);
}

async function untar(archive: string, into: string, flag: string) {
  mkdirSync(into, { recursive: true });
  const p = Bun.spawn(["tar", flag, archive, "-C", into], { stdout: "ignore", stderr: "pipe" });
  const err = await new Response(p.stderr).text();
  if ((await p.exited) !== 0) throw new Error(`tar: ${err.trim().split("\n").pop()}`);
}

// Fetch whatever is missing for model ID into DIR; each piece lands in a
// staging folder and is moved into place only once its checksum matched.
export async function install(dir: string, id: string, progress: Progress) {
  if (!(plat() in RUNTIMES)) throw new Error(`local dictation is not available on ${plat()}`);
  if (!(id in MODELS)) throw new Error(`unknown model ${id}`);
  mkdirSync(dir, { recursive: true });
  const stage = join(dir, `.stage-${process.pid}`);
  rmSync(stage, { recursive: true, force: true });
  mkdirSync(stage, { recursive: true });
  try {
    const s = status(dir);
    if (!s.vad) {
      await fetchTo(VAD, join(stage, "silero_vad.onnx"), "voice activity model", progress);
      renameSync(join(stage, "silero_vad.onnx"), vadPath(dir));
    }
    if (!s.runtime) {
      const tgz = join(stage, "runtime.tgz");
      await fetchTo(RUNTIMES[plat()], tgz, "speech runtime", progress);
      await untar(tgz, join(stage, "rt"), "-xzf");
      rmSync(runtimeDir(dir), { recursive: true, force: true });
      renameSync(join(stage, "rt", "package"), runtimeDir(dir));
    }
    if (!s.models.find((m) => m.id === id)?.installed) {
      const m = MODELS[id];
      const tb = join(stage, "model.tar.bz2");
      await fetchTo(m.pin, tb, m.label, progress);
      await untar(tb, join(stage, "m"), "-xjf");
      rmSync(join(stage, "m", m.dir, "test_wavs"), { recursive: true, force: true });
      rmSync(modelDir(dir, id), { recursive: true, force: true });
      renameSync(join(stage, "m", m.dir), modelDir(dir, id));
    }
  } finally {
    rmSync(stage, { recursive: true, force: true });
  }
  // the hub reads this to know the model is usable (server/voice.bend)
  writeFileSync(join(dir, `ready-${id}`), "ok");
}

export function remove(dir: string, id: string) {
  if (id in MODELS) {
    rmSync(join(dir, `ready-${id}`), { force: true });
    rmSync(modelDir(dir, id), { recursive: true, force: true });
  }
  // the runtime and VAD go with the last model
  if (!status(dir).models.some((m) => m.installed)) {
    rmSync(runtimeDir(dir), { recursive: true, force: true });
    rmSync(vadPath(dir), { force: true });
  }
}

export function diskBytes(dir: string): number {
  let n = 0;
  const walk = (p: string) => {
    for (const e of readdirSync(p, { withFileTypes: true })) {
      const q = join(p, e.name);
      if (e.isDirectory()) walk(q);
      else n += statSync(q).size;
    }
  };
  if (existsSync(dir)) walk(dir);
  return n;
}

// ---- the engine

// a word: text, start and end in centiseconds from the segment's start, and
// the probability (percent) of its least sure piece
export type Word = [string, number, number, number];

type Addon = any;
let addon: Addon | undefined;

function load(dir: string): Addon {
  if (!addon) addon = createRequire(import.meta.url)(join(runtimeDir(dir), "sherpa-onnx.node"));
  return addon;
}

export function wordsOf(r: { tokens?: string[]; timestamps?: number[]; durations?: number[]; ys_log_probs?: number[] }): Word[] {
  const toks = r.tokens ?? [];
  const ts = r.timestamps ?? [];
  const ds = r.durations ?? [];
  const lp = r.ys_log_probs ?? [];
  const out: Word[] = [];
  let gap = true; // the next piece starts a word
  for (let i = 0; i < toks.length; i++) {
    const t = toks[i];
    const start = ts[i] ?? 0;
    const end = start + (ds[i] || 0.08);
    const p = lp[i] === undefined ? 1 : Math.exp(lp[i]);
    // a piece of only spaces (" " before a digit) opens the next word
    if (t.trim() === "") {
      gap = true;
      continue;
    }
    const last = out[out.length - 1];
    // a piece of only punctuation (" ." at the end) closes the last word
    const punct = /^[\s.,!?;:]+$/.test(t);
    if (!last || (!punct && (gap || /^\s/.test(t)))) out.push([t.trim(), cs(start), cs(end), pct(p)]);
    else {
      last[0] += t.trim();
      last[2] = Math.max(last[2], cs(end));
      last[3] = Math.min(last[3], pct(p));
    }
    gap = /\s$/.test(t);
  }
  return out.filter((w) => w[0] !== "");
}

const cs = (sec: number) => Math.max(0, Math.round(sec * 100));
const pct = (p: number) => Math.max(0, Math.min(100, Math.round(p * 100)));

export class Engine {
  private constructor(private a: Addon, private rec: unknown) {}
  // loading the model takes about a second; it runs off the event loop so
  // the microphone keeps being read meanwhile
  static async create(dir: string, id: string, threads = 4): Promise<Engine> {
    const a = load(dir);
    const m = modelDir(dir, id);
    const config = {
      featConfig: { sampleRate: RATE, featureDim: 80 },
      modelConfig: {
        transducer: { encoder: join(m, "encoder.int8.onnx"), decoder: join(m, "decoder.int8.onnx"), joiner: join(m, "joiner.int8.onnx") },
        tokens: join(m, "tokens.txt"),
        numThreads: threads,
        provider: "cpu",
        modelType: "nemo_transducer",
        debug: 0,
      },
      decodingMethod: "greedy_search",
    };
    const rec = a.createOfflineRecognizerAsync ? await a.createOfflineRecognizerAsync(config) : a.createOfflineRecognizer(config);
    return new Engine(a, rec);
  }
  async decode(samples: Float32Array): Promise<Word[]> {
    const s = this.a.createOfflineStream(this.rec);
    this.a.acceptWaveformOffline(s, { samples, sampleRate: RATE });
    if (this.a.decodeOfflineStreamAsync) await this.a.decodeOfflineStreamAsync(this.rec, s);
    else this.a.decodeOfflineStream(this.rec, s);
    return wordsOf(JSON.parse(this.a.getOfflineStreamResultAsJson(s)));
  }
  vad(dir: string): Vad {
    return new Vad(this.a, vadPath(dir));
  }
}

class Vad {
  private h: unknown;
  constructor(private a: Addon, model: string) {
    this.h = a.createVoiceActivityDetector(
      { sileroVad: { model, threshold: 0.5, minSilenceDuration: 0.5, minSpeechDuration: MIN_SPEECH, windowSize: 512, maxSpeechDuration: MAX_SEG }, sampleRate: RATE, numThreads: 1, debug: 0 },
      60,
    );
  }
  accept(w: Float32Array) {
    this.a.voiceActivityDetectorAcceptWaveform(this.h, w);
  }
  speaking(): boolean {
    return this.a.voiceActivityDetectorIsDetected(this.h);
  }
  // segments the VAD has closed since the last call (it keeps them queued)
  ended(): number {
    let n = 0;
    while (!this.a.voiceActivityDetectorIsEmpty(this.h)) {
      this.a.voiceActivityDetectorPop(this.h);
      n++;
    }
    return n;
  }
  flush() {
    this.a.voiceActivityDetectorFlush(this.h);
  }
}

export type Out = {
  hyp: (item: string, words: Word[], end: number) => void;
  done: (item: string, words: Word[], ms: { decode: number; speechEnd: number }) => void;
  speech: (on: boolean) => void;
};

// A live session: s16le mono PCM at RATE in, hyp/done out. The VAD opens a
// segment (with PRE_ROLL before it) and closes it after half a second of
// quiet; while open, its audio so far is decoded again whenever the last
// decode is done and DECODE_MS have passed.
export class Session {
  private buf = new Float32Array(RATE * 120);
  private n = 0; // samples held in buf (from base)
  private base = 0; // absolute sample index of buf[0]
  private segStart = -1; // absolute sample where the open segment starts
  private win = new Float32Array(512);
  private winN = 0;
  private busy: Promise<void> = Promise.resolve();
  private decoding = false;
  private lastDecode = 0;
  private seq = 0;
  private vad: Vad;
  private carry = new Uint8Array(0);
  constructor(private e: Engine, dir: string, private out: Out, private live = true, private id = `l${Date.now().toString(36)}`) {
    this.vad = e.vad(dir);
  }
  private now() {
    return this.base + this.n;
  }
  push(pcm: Uint8Array) {
    let b = pcm;
    if (this.carry.length) {
      b = new Uint8Array(this.carry.length + pcm.length);
      b.set(this.carry);
      b.set(pcm, this.carry.length);
    }
    const even = b.length & ~1;
    this.carry = b.slice(even);
    const v = new DataView(b.buffer, b.byteOffset, even);
    for (let i = 0; i < even; i += 2) this.sample(v.getInt16(i, true) / 32768);
    this.tick();
  }
  private sample(x: number) {
    if (this.n === this.buf.length) this.compact();
    this.buf[this.n++] = x;
    this.win[this.winN++] = x;
    if (this.winN === 512) {
      this.winN = 0;
      this.vad.accept(this.win);
      this.step(this.vad.speaking(), this.vad.ended());
    }
  }
  // drop audio no open segment needs (keep the pre-roll)
  private compact() {
    const keepFrom = this.segStart >= 0 ? this.segStart : this.now() - Math.round(PRE_ROLL * RATE);
    const drop = Math.max(0, Math.min(this.n, keepFrom - this.base));
    if (drop === 0) {
      const bigger = new Float32Array(this.buf.length * 2);
      bigger.set(this.buf);
      this.buf = bigger;
      return;
    }
    this.buf.copyWithin(0, drop, this.n);
    this.n -= drop;
    this.base += drop;
  }
  private step(speaking: boolean, ended: number) {
    if (this.segStart < 0 && speaking) {
      this.segStart = Math.max(this.base, this.now() - Math.round((PRE_ROLL + MIN_SPEECH) * RATE));
      this.out.speech(true);
    }
    if (this.segStart >= 0 && ended > 0) {
      // a segment cut at MAX_SEG while still speaking goes on as the next
      this.close(speaking ? 0.5 : TAIL);
      if (speaking) {
        this.segStart = this.now();
        this.out.speech(true);
      }
    }
  }
  private item() {
    return `${this.id}-${this.seq}`;
  }
  private audio(from: number, to: number) {
    return this.buf.slice(from - this.base, to - this.base);
  }
  private tick() {
    if (!this.live || this.segStart < 0 || this.decoding || Date.now() - this.lastDecode < DECODE_MS) return;
    const from = this.segStart;
    const to = this.now();
    if (to - from < RATE * 0.3) return;
    const item = this.item();
    this.decoding = true;
    this.lastDecode = Date.now();
    this.busy = this.busy.then(async () => {
      const w = await this.e.decode(this.audio(from, to));
      // a decode that finished after its segment closed says nothing new
      if (this.item() === item && this.segStart === from) this.out.hyp(item, w, cs((to - from) / RATE));
      this.decoding = false;
    });
  }
  private close(extra = TAIL) {
    const from = this.segStart;
    const to = Math.min(this.now(), this.now() - Math.round(0.5 * RATE) + Math.round(extra * RATE));
    const item = this.item();
    this.segStart = -1;
    this.seq++;
    this.out.speech(false);
    const speechEnd = Date.now();
    const samples = this.audio(from, Math.max(from + 1, to));
    this.busy = this.busy.then(async () => {
      const t = performance.now();
      const w = samples.length > RATE * 0.2 ? await this.e.decode(samples) : [];
      this.out.done(item, w, { decode: Math.round(performance.now() - t), speechEnd: Date.now() - speechEnd });
    });
  }
  // the end: whatever segment is open closes with all its audio
  async finish() {
    if (this.segStart >= 0) {
      const from = this.segStart;
      const to = this.now();
      const item = this.item();
      this.segStart = -1;
      this.seq++;
      const samples = this.audio(from, to);
      this.busy = this.busy.then(async () => {
        const t = performance.now();
        const w = samples.length > RATE * MIN_SPEECH ? await this.e.decode(samples) : [];
        this.out.done(item, w, { decode: Math.round(performance.now() - t), speechEnd: 0 });
      });
    }
    await this.busy;
  }
}

// a whole recording (s16le at RATE): segments by the VAD, decoded one by one
export async function transcribe(e: Engine, dir: string, pcm: Uint8Array): Promise<string> {
  const parts: string[] = [];
  const s = new Session(e, dir, { hyp: () => {}, done: (_i, w) => parts.push(w.map((x) => x[0]).join(" ")), speech: () => {} }, false);
  for (let i = 0; i < pcm.length; i += 32000) s.push(pcm.subarray(i, Math.min(pcm.length, i + 32000)));
  await s.finish();
  return parts.filter(Boolean).join(" ");
}
