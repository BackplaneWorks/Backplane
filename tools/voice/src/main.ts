// backplane-voice: speech to text with OpenAI for Backplane.
// NDJSON events on stdout, diagnostics on stderr. See ../README.md.
import { writeSync, unlinkSync } from "node:fs";
import { tmpdir } from "node:os";
import { basename, join } from "node:path";
import { baseUrl, DELAYS, msg, Realtime, transcribeFile, type Opts } from "./openai";
import { listDevices } from "./devices";
import { BYTES_PER_MS, Rechunker, Segmenter, type Action } from "./vad";
import * as L from "./local";

const USAGE = `usage:
  backplane-voice live --engine local --local-dir D [--model parakeet-v2] [--source ...] [--device NAME]
  backplane-voice file PATH --engine local --local-dir D [--model ID] [--content-type CT]
  backplane-voice local-status --local-dir D
  backplane-voice local-install --local-dir D [--model ID]
  backplane-voice local-remove --local-dir D [--model ID]
  backplane-voice live [--keywords-file F] [--prompt P] [--language xx]... [--delay low]
                       [--source auto|pw-record|parecord|arecord|stdin] [--device NAME] [--key-file F]
  backplane-voice file PATH [--content-type CT] [--keywords-file F] [--prompt P]
                       [--language xx]... [--delay low] [--key-file F]
  backplane-voice devices
The key comes from --key-file, else OPENAI_API_KEY. OPENAI_BASE_URL overrides the API base.
`;

const LIVE_LIMIT_MS = 10 * 60 * 1000;
const FILE_LIMIT_MS = 90 * 1000;
const WAIT_MS = 8000;
const CHUNK = 100 * BYTES_PER_MS; // 100 ms of audio
const FILE_SEGMENT = 15000 * BYTES_PER_MS; // at most 15 s per commit

// ---- output

let failed = false;
let ended = false;

function emit(ev: Record<string, unknown>) {
  if (ended) return;
  writeSync(1, JSON.stringify(ev) + "\n");
}

function error(message: string) {
  failed = true;
  emit({ event: "error", error: message });
}

function end(): never {
  emit({ event: "end" });
  ended = true;
  process.exit(failed ? 1 : 0);
}

function fail(message: string): never {
  error(message);
  end();
}

function log(s: string) {
  process.stderr.write(`backplane-voice: ${s}\n`);
}

// ---- arguments

type Args = {
  cmd: string;
  pos: string[];
  keywordsFile?: string;
  keyFile?: string;
  prompt: string;
  languages: string[];
  delay?: string;
  source: string;
  device: string;
  contentType: string;
  engine: string;
  rate: number;
  localDir: string;
  model: string;
};

function usage(why?: string): never {
  if (why) process.stderr.write(`backplane-voice: ${why}\n`);
  process.stderr.write(USAGE);
  process.exit(2);
}

function parse(argv: string[]): Args {
  const a: Args = { cmd: argv[0] ?? "", pos: [], prompt: "", languages: [], source: "auto", device: "", contentType: "", engine: "openai", rate: 0, localDir: "", model: "parakeet-v2" };
  if (!["live", "file", "devices", "local-status", "local-install", "local-remove"].includes(a.cmd)) usage(a.cmd ? `unknown command: ${a.cmd}` : undefined);
  for (let i = 1; i < argv.length; i++) {
    const f = argv[i];
    if (!f.startsWith("--")) {
      a.pos.push(f);
      continue;
    }
    const v = argv[++i];
    if (v === undefined) usage(`${f} needs a value`);
    switch (f) {
      case "--keywords-file": a.keywordsFile = v; break;
      case "--key-file": a.keyFile = v; break;
      case "--prompt": a.prompt = v; break;
      case "--language": a.languages.push(v); break;
      case "--delay": a.delay = v; break;
      case "--source": a.source = v; break;
      case "--device": a.device = v; break;
      case "--content-type": a.contentType = v; break;
      case "--engine": a.engine = v; break;
      case "--rate": a.rate = Number(v); break;
      case "--local-dir": a.localDir = v; break;
      case "--model": a.model = v; break;
      default: usage(`unknown flag: ${f}`);
    }
  }
  if (a.delay !== undefined && !DELAYS.includes(a.delay)) usage(`--delay is one of ${DELAYS.join("|")}`);
  if (!["auto", "pw-record", "parecord", "arecord", "stdin"].includes(a.source)) usage(`bad --source: ${a.source}`);
  if (!["openai", "local"].includes(a.engine)) usage(`bad --engine: ${a.engine}`);
  if ((a.engine === "local" || a.cmd.startsWith("local-")) && !a.localDir) usage("--local-dir is needed");
  if (a.cmd === "file" && a.pos.length !== 1) usage("file needs one PATH");
  if (a.cmd !== "file" && a.pos.length) usage(`unexpected argument: ${a.pos[0]}`);
  return a;
}

export function keywordsOf(text: string): string[] {
  return text.split(/\r?\n/).map((l) => l.trim()).filter(Boolean).slice(0, 100);
}

async function opts(a: Args): Promise<Opts> {
  let key = "";
  if (a.keyFile) {
    try {
      key = (await Bun.file(a.keyFile).text()).trim();
    } catch {
      fail(`cannot read the key file ${a.keyFile}`);
    }
  } else key = (process.env.OPENAI_API_KEY ?? "").trim();
  if (!key) fail("no OpenAI API key (set one in Backplane's settings)");
  let keywords: string[] = [];
  if (a.keywordsFile) {
    try {
      keywords = keywordsOf(await Bun.file(a.keywordsFile).text());
    } catch {
      log(`cannot read keywords file ${a.keywordsFile}; going without`);
    }
  }
  return { key, base: baseUrl(), prompt: a.prompt, keywords, languages: a.languages, delay: a.delay ?? "low" };
}

// ---- live

const RECORDERS: Record<string, (device: string, rate: number) => string[]> = {
  "pw-record": (d, r) => ["pw-record", "--raw", "--rate", `${r}`, "--channels", "1", "--format", "s16", ...(d ? ["--target", d] : []), "-"],
  parecord: (d, r) => ["parecord", "--raw", `--rate=${r}`, "--channels=1", "--format=s16le", ...(d ? [`--device=${d}`] : [])],
  arecord: (d, r) => ["arecord", "-q", "-t", "raw", "-f", "S16_LE", "-r", `${r}`, "-c", "1", ...(d ? ["-D", d] : [])],
};
const ORDER = ["pw-record", "parecord", "arecord"];
const OPEN_GRACE_MS = 500; // a recorder alive this long with no audio yet counts as started

type Source = {
  name: string;
  reader: ReadableStreamDefaultReader<Uint8Array>;
  pending: Promise<ReadableStreamReadResult<Uint8Array>>;
  kill: () => void;
  why: () => Promise<string>;
};

async function recorder(name: string, device: string, rate: number): Promise<Source | string> {
  const cmd = RECORDERS[name](device, rate);
  const exe = Bun.which(cmd[0]);
  if (!exe) return `${name} not found`;
  const p = Bun.spawn([exe, ...cmd.slice(1)], { stdin: "ignore", stdout: "pipe", stderr: "pipe" });
  const errText = new Response(p.stderr).text();
  const reader = p.stdout.getReader();
  const why = async () => `${name}: ${(await errText).trim().split("\n").pop() || `exit ${await p.exited}`}`;
  const pending = reader.read();
  const r = await Promise.race([pending, Bun.sleep(OPEN_GRACE_MS).then(() => "wait" as const)]);
  if (r !== "wait" && r.done) return await why();
  return { name, reader, pending, kill: () => p.kill("SIGTERM"), why };
}

async function openSource(which: string, device: string, rate: number): Promise<Source> {
  if (which === "stdin") {
    const reader = Bun.stdin.stream().getReader();
    return { name: "stdin", reader, pending: reader.read(), kill: () => void reader.cancel().catch(() => {}), why: async () => "stdin closed" };
  }
  const names = which === "auto" ? ORDER : [which];
  const whys: string[] = [];
  for (const n of names) {
    const s = await recorder(n, device, rate);
    if (typeof s !== "string") return s;
    whys.push(s);
    log(s);
  }
  fail(`could not record from the microphone (${whys.filter((w) => !w.endsWith("not found")).join("; ")})`);
}

// RMS (int16) to 0..100: -60 dBFS and below is 0, full scale 100.
export function levelOf(rms: number): number {
  if (rms <= 0) return 0;
  const db = 20 * Math.log10(rms / 32768);
  return Math.max(0, Math.min(100, Math.round(((db + 60) * 100) / 60)));
}

const LEVEL_MS = 250;
const LABEL_WAIT_MS = 150; // how long "ready" waits for the microphone's name
const NO_AUDIO_MS = 3000;
const SILENT_MS = 4000;

async function live(a: Args) {
  const local = a.engine === "local";
  const rate = local ? L.RATE : 24000;
  if (local) {
    const why = L.ready(a.localDir, a.model);
    if (why) fail(why);
  }
  const o = local ? undefined : await opts(a);
  if (a.source !== "stdin") {
    const names = a.source === "auto" ? ORDER : [a.source];
    if (!names.some((n) => Bun.which(n))) {
      fail(a.source === "auto" ? "no recorder found (pw-record, parecord or arecord)" : `${a.source} not found`);
    }
  }

  let stopping = false;
  let onStop!: () => void;
  const stopped = new Promise<"stop">((ok) => (onStop = () => ok("stop")));
  const stop = () => {
    if (stopping) return;
    stopping = true;
    emit({ event: "stopping" });
    onStop();
  };
  process.on("SIGTERM", stop);
  process.on("SIGINT", stop);
  setTimeout(stop, LIVE_LIMIT_MS);
  if (a.source !== "stdin") watchStdin(stop);

  const listing = a.source === "stdin" ? Promise.resolve(undefined) : listDevices().catch(() => undefined);
  let finishing = false;

  // The recorder and the OpenAI session open at once: audio heard before
  // the session is up waits in `queued` and goes the moment it opens, so
  // recording starts (and "ready" goes out) without waiting on the network.
  let rt: Realtime | undefined;
  const queued: Action[] = [];
  const send = (acts: Action[]) => {
    for (const x of acts) {
      if ("append" in x) rt!.append(x.append);
      else if ("commit" in x) rt!.commit();
      else rt!.clear();
    }
  };
  const apply = (acts: Action[]) => {
    if (rt) send(acts);
    else queued.push(...acts);
  };
  // the local engine: the model loads while the microphone already records
  let sess: L.Session | undefined;
  const held: Uint8Array[] = [];
  const t0load = performance.now();
  const loading = !local
    ? Promise.resolve(undefined)
    : L.Engine.create(a.localDir, a.model).then(
        (e) => {
          sess = new L.Session(e, a.localDir, {
            hyp: (item, words, end) => emit({ event: "hyp", item, words, end }),
            done: (item, words, ms) =>
              emit({ event: "done", item, text: words.map((w) => w[0]).join(" "), words, decode_ms: ms.decode, final_ms: ms.speechEnd }),
            speech: () => {},
          });
          emit({ event: "loaded", ms: Math.round(performance.now() - t0load) });
          for (const b of held.splice(0)) sess.push(b);
          return sess;
        },
        (e) => {
          error(`could not load the local model (${msg(e)})`);
          stop();
          return undefined;
        },
      );
  const connecting = local ? Promise.resolve(undefined) : Realtime.connect(o!, {
    delta: (item, text) => emit({ event: "delta", item, text }),
    done: (item, text) => emit({ event: "done", item, text }),
    error: (m) => error(m),
    closed: () => {
      if (!finishing) {
        error("the connection to OpenAI closed");
        stop();
      }
    },
  }).then(
    (r) => {
      rt = r;
      send(queued.splice(0));
      return r;
    },
    (e) => {
      error(msg(e));
      stop();
      return undefined;
    },
  );

  const seg = new Segmenter();
  const re = new Rechunker(CHUNK);
  // the level meter and the no-audio / silent hints
  let bytes = 0;
  let peak = 0;
  let heard = false;
  const hints = new Set<string>();
  const hint = (h: string) => {
    if (hints.has(h)) return;
    hints.add(h);
    emit({ event: "hint", hint: h });
  };
  // audio from a client (--source stdin --rate R) at another rate is
  // converted to the engine's
  const conv = a.source === "stdin" && a.rate > 0 && a.rate !== rate ? new Resampler(a.rate, rate) : undefined;
  const feed = (b0: Uint8Array) => {
    const b = conv ? conv.push(b0) : b0;
    if (!b.length) return;
    bytes += b.length;
    if (local) {
      if (sess) sess.push(b);
      else held.push(b);
    }
    for (const c of re.push(b)) {
      const acts = seg.push(c);
      if (!local) apply(acts);
      if (seg.lastRms > peak) peak = seg.lastRms;
      if (seg.lastLoud && !heard) {
        heard = true;
        if (hints.size) hint("ok");
      }
    }
  };

  if (!stopping) {
    const opening = openSource(a.source, a.device, rate);
    const src = await Promise.race([opening, stopped]);
    if (src === "stop") opening.then((s) => s.kill(), () => {});
    else {
      // the device's name is a nicety: "ready" goes now with what is known,
      // and again with the name if the listing comes in a little later
      let devs: Awaited<typeof listing>;
      let listed = false;
      listing.then((d) => ((devs = d), (listed = true)));
      await Promise.race([listing, Bun.sleep(LABEL_WAIT_MS)]);
      const ready = () => {
        const device = a.device || devs?.default || "";
        const label = devs?.devices.find((d) => d.name === device)?.label ?? "";
        emit({ event: "ready", recorder: src.name, device, label });
      };
      ready();
      if (!listed) {
        listing.then(() => {
          if (!stopping && !hints.size && (devs?.default || devs?.devices.length)) ready();
        });
      }
      const t0 = Date.now();
      const meter = setInterval(() => {
        if (stopping) return clearInterval(meter);
        emit({ event: "level", v: levelOf(peak) });
        peak = 0;
        const el = Date.now() - t0;
        if (el >= NO_AUDIO_MS && bytes === 0) hint("no-audio");
        if (el >= SILENT_MS && bytes > 0 && !heard) hint("silent");
      }, LEVEL_MS);
      let killed = false;
      let late: Promise<"late"> | undefined;
      let pending = src.pending;
      for (;;) {
        const r = await Promise.race([pending, late ?? stopped]);
        if (r === "late") break;
        if (r === "stop") {
          // let the recorder hand over what it has, briefly
          killed = true;
          late = new Promise((ok) => setTimeout(() => ok("late"), 1000));
          src.kill();
          continue;
        }
        if (r.done) {
          if (!stopping && a.source !== "stdin") error(`the recorder stopped (${await src.why()})`);
          break;
        }
        feed(r.value);
        pending = src.reader.read();
      }
      clearInterval(meter);
      if (!killed) src.kill();
    }
  }
  if (!stopping) stop();
  finishing = true;
  if (local) {
    const s = await loading;
    if (s) await s.finish();
    end();
  }
  for (const c of re.rest()) apply(seg.push(c));
  apply(seg.flush());
  const live = await connecting;
  if (!live) end();
  if (!(await live.waitAll(WAIT_MS))) log("gave up waiting for transcripts");
  live.close();
  end();
}

// s16le mono from one rate to another, linearly (a client's 16 kHz for the
// cloud's 24 kHz); keeps the odd byte and the last sample between chunks
export class Resampler {
  private carry = new Uint8Array(0);
  private pos = 0; // the next output sample, in input samples from `last`
  private last = 0;
  constructor(private from: number, private to: number) {}
  push(b0: Uint8Array): Uint8Array {
    let b = b0;
    if (this.carry.length) {
      b = new Uint8Array(this.carry.length + b0.length);
      b.set(this.carry);
      b.set(b0, this.carry.length);
    }
    const n = b.length >> 1;
    this.carry = b.slice(n * 2);
    const v = new DataView(b.buffer, b.byteOffset, n * 2);
    const step = this.from / this.to;
    const out: number[] = [];
    // input sample i (-1 is `last`)
    const at = (i: number) => (i < 0 ? this.last : v.getInt16(i * 2, true));
    while (this.pos <= n - 1) {
      const i = Math.floor(this.pos) - 1;
      const f = this.pos - Math.floor(this.pos);
      out.push(Math.round(at(i) * (1 - f) + at(i + 1) * f));
      this.pos += step;
    }
    if (n > 0) {
      this.last = at(n - 1);
      this.pos -= n;
    }
    const o = new Uint8Array(out.length * 2);
    const ov = new DataView(o.buffer);
    out.forEach((x, k) => ov.setInt16(k * 2, Math.max(-32768, Math.min(32767, x)), true));
    return o;
  }
}

// ---- devices

async function devices() {
  const d = await listDevices();
  emit({ event: "devices", default: d.default, devices: d.devices });
  if (!d.via) error("no way to list audio devices (pactl, pw-dump or arecord)");
  end();
}

// "stop" on stdin, or stdin's end, stops a live session.
function watchStdin(stop: () => void) {
  (async () => {
    const dec = new TextDecoder();
    let buf = "";
    try {
      for await (const b of Bun.stdin.stream()) {
        buf += dec.decode(b, { stream: true });
        const lines = buf.split("\n");
        buf = lines.pop()!;
        if (lines.some((l) => l.trim() === "stop")) return stop();
      }
    } catch {}
    stop();
  })();
}

// ---- file

async function decode(path: string, rate = 24000): Promise<Uint8Array> {
  const p = Bun.spawn(["ffmpeg", "-nostdin", "-loglevel", "error", "-i", path, "-f", "s16le", "-ac", "1", "-ar", `${rate}`, "-"], {
    stdin: "ignore",
    stdout: "pipe",
    stderr: "pipe",
  });
  const [out, err, code] = await Promise.all([new Response(p.stdout).arrayBuffer(), new Response(p.stderr).text(), p.exited]);
  if (code !== 0) throw new Error(`ffmpeg: ${err.trim().split("\n").pop() || `exit ${code}`}`);
  return new Uint8Array(out);
}

// Whole PCM through gpt-live-transcribe: one commit per <= 15 s piece.
async function liveFile(o: Opts, pcm: Uint8Array, deadline: number): Promise<string> {
  if (pcm.length < 100 * BYTES_PER_MS) return "";
  const errs: string[] = [];
  const rt = await Realtime.connect(o, { error: (m) => errs.push(m) });
  try {
    const n = Math.ceil(pcm.length / FILE_SEGMENT);
    const size = Math.ceil(pcm.length / n / 2) * 2;
    for (let s = 0; s < pcm.length; s += size) {
      const piece = pcm.subarray(s, Math.min(pcm.length, s + size));
      for (let c = 0; c < piece.length; c += 10 * CHUNK) rt.append(piece.subarray(c, Math.min(piece.length, c + 10 * CHUNK)));
      rt.commit();
    }
    const ok = await rt.waitAll(Math.max(1000, deadline - Date.now() - 20000));
    if (errs.length) throw new Error(errs[0]);
    if (!ok) throw new Error("no transcript from the realtime session");
    return rt.joined();
  } finally {
    rt.close();
  }
}

async function file(a: Args) {
  if (a.engine === "local") return localFile(a);
  const o = await opts(a);
  const path = a.pos[0];
  const deadline = Date.now() + FILE_LIMIT_MS;
  setTimeout(() => fail("timed out after 90 s"), FILE_LIMIT_MS);
  emit({ event: "ready" });

  const f = Bun.file(path);
  if (!(await f.exists())) fail(`no such file: ${path}`);
  let audio: Blob;
  let name: string;
  let onDisk = path;
  let temp = "";
  if (a.contentType.toLowerCase().startsWith("multipart/form-data")) {
    let part: unknown;
    try {
      // bytes, not the BunFile: Bun 1.3 fails to parse a file-backed body
      part = (await new Response(await f.bytes(), { headers: { "content-type": a.contentType } }).formData()).get("audio");
    } catch (e) {
      fail(`bad multipart body: ${msg(e)}`);
    }
    if (!(part instanceof Blob)) fail("no audio part");
    audio = part;
    name = (part as File).name || "audio.m4a";
    if (!/\.[a-z0-9]+$/i.test(name)) name += "." + extOf(part.type);
    temp = onDisk = join(tmpdir(), `backplane-voice-${process.pid}-${basename(name)}`);
  } else {
    audio = a.contentType ? Bun.file(path, { type: a.contentType }) : f;
    name = basename(path);
  }

  let text: string | undefined;
  if (Bun.which("ffmpeg")) {
    try {
      if (temp) await Bun.write(temp, audio);
      text = await liveFile(o, await decode(onDisk), deadline);
    } catch (e) {
      log(`realtime path failed (${msg(e)}); trying gpt-transcribe`);
    } finally {
      if (temp) try { unlinkSync(temp); } catch {}
    }
  }
  if (text === undefined) {
    try {
      text = await transcribeFile(o, audio, name, AbortSignal.timeout(Math.max(1000, deadline - Date.now())));
    } catch (e) {
      fail(msg(e));
    }
  }
  emit({ event: "done", item: "file", text: text.trim() });
  end();
}

// a recording through the local engine: ffmpeg to 16 kHz PCM, then the
// segments one by one
async function localFile(a: Args) {
  const why = L.ready(a.localDir, a.model);
  if (why) fail(why);
  setTimeout(() => fail("timed out after 90 s"), FILE_LIMIT_MS);
  emit({ event: "ready" });
  const path = a.pos[0];
  if (!(await Bun.file(path).exists())) fail(`no such file: ${path}`);
  if (!Bun.which("ffmpeg")) fail("ffmpeg is needed to read recordings");
  let onDisk = path;
  let temp = "";
  if (a.contentType.toLowerCase().startsWith("multipart/form-data")) {
    const part = (await new Response(await Bun.file(path).bytes(), { headers: { "content-type": a.contentType } }).formData()).get("audio");
    if (!(part instanceof Blob)) fail("no audio part");
    temp = onDisk = join(tmpdir(), `backplane-voice-${process.pid}.audio`);
    await Bun.write(temp, part);
  }
  try {
    const pcm = await decode(onDisk, L.RATE);
    const e = await L.Engine.create(a.localDir, a.model);
    emit({ event: "done", item: "file", text: (await L.transcribe(e, a.localDir, pcm)).trim() });
  } catch (e) {
    fail(msg(e));
  } finally {
    if (temp) try { unlinkSync(temp); } catch {}
  }
  end();
}

async function localCmd(a: Args) {
  if (a.cmd === "local-status") {
    emit({ event: "local", ...L.status(a.localDir), bytes: L.diskBytes(a.localDir) });
  } else if (a.cmd === "local-remove") {
    L.remove(a.localDir, a.model);
    emit({ event: "local", ...L.status(a.localDir), bytes: L.diskBytes(a.localDir) });
  } else {
    try {
      await L.install(a.localDir, a.model, (what, done, total) => emit({ event: "progress", what, done, total }));
    } catch (e) {
      error(msg(e));
    }
    emit({ event: "local", ...L.status(a.localDir), bytes: L.diskBytes(a.localDir) });
  }
  end();
}

function extOf(type: string): string {
  const t = type.toLowerCase();
  if (t.includes("mp4") || t.includes("m4a") || t.includes("aac")) return "m4a";
  if (t.includes("wav")) return "wav";
  if (t.includes("mpeg") || t.includes("mp3")) return "mp3";
  if (t.includes("ogg")) return "ogg";
  if (t.includes("webm")) return "webm";
  if (t.includes("flac")) return "flac";
  return "m4a";
}

// ---- main

if (import.meta.main) {
  const a = parse(process.argv.slice(2));
  (a.cmd === "live" ? live(a) : a.cmd === "file" ? file(a) : a.cmd.startsWith("local-") ? localCmd(a) : devices()).catch((e) => fail(msg(e)));
}
