// Dictation latency, measured on the helper as the hub runs it.
//   bun test/tools/voice_latency.ts HELPER LOCAL_DIR WAV... [--engine openai --key-file F]
// Plays each 16 kHz mono WAV into `backplane-voice live --source stdin` in
// real time (100 ms pieces, 1.2 s of silence after each, as a person pausing),
// and reports, per segment:
//   first word  ms from the start of speech to the first hypothesis word
//   final       ms from the end of speech (the WAV's end) to its `done`
//   decode      the helper's own decode time for the final pass
//   load        the model load (local), once
// The words shown live are timed as the hub settles them (Settle): a word is
// committed when two hypotheses agree on it; "commit lag" is how long after
// a word ends in the audio it is committed.
import { readFileSync } from "node:fs";

const args = process.argv.slice(2);
const flag = (n: string) => {
  const i = args.indexOf(n);
  if (i < 0) return undefined;
  const v = args[i + 1];
  args.splice(i, 2);
  return v;
};
const engine = flag("--engine") ?? "local";
const keyFile = flag("--key-file");
const [helper, localDir, ...wavs] = args;
if (!helper || !wavs.length) {
  console.error("usage: bun test/tools/voice_latency.ts HELPER LOCAL_DIR WAV... [--engine openai --key-file F]");
  process.exit(2);
}

const RATE = 16000;
const pcmOf = (f: string) => {
  const b = readFileSync(f);
  // a plain 44-byte header, 16-bit mono 16 kHz (what sherpa's writeWave makes)
  return new Uint8Array(b.buffer, b.byteOffset + 44, b.length - 44);
};

const gap = new Uint8Array(Math.round(RATE * 1.2) * 2);
const pieces = wavs.map(pcmOf);
const argv =
  engine === "local"
    ? [helper, "live", "--engine", "local", "--local-dir", localDir, "--source", "stdin"]
    : [helper, "live", "--source", "stdin", "--rate", "16000", ...(keyFile ? ["--key-file", keyFile] : [])];
const p = Bun.spawn(argv, { stdin: "pipe", stdout: "pipe", stderr: "inherit" });

const t0 = performance.now();
const speechAt: { start: number; end: number }[] = [];
const events: { t: number; e: any }[] = [];
(async () => {
  const dec = new TextDecoder();
  let buf = "";
  for await (const c of p.stdout) {
    buf += dec.decode(c, { stream: true });
    const ls = buf.split("\n");
    buf = ls.pop()!;
    for (const l of ls) if (l.trim()) events.push({ t: performance.now() - t0, e: JSON.parse(l) });
  }
})();

// real time, measured against the wall clock
let sent = 0;
const write = async (b: Uint8Array) => {
  for (let i = 0; i < b.length; i += 3200) {
    p.stdin.write(b.subarray(i, Math.min(b.length, i + 3200)));
    sent += Math.min(3200, b.length - i);
    const due = (sent / 2 / RATE) * 1000;
    const wait = due - (performance.now() - t0);
    if (wait > 0) await Bun.sleep(wait);
  }
};
for (const pc of pieces) {
  const start = (sent / 2 / RATE) * 1000;
  await write(pc);
  speechAt.push({ start, end: (sent / 2 / RATE) * 1000 });
  await write(gap);
}
p.stdin.end();
await p.exited;
await Bun.sleep(50);

const dones = events.filter((x) => x.e.event === "done" && x.e.text);
const hyps = events.filter((x) => x.e.event === "hyp");
const load = events.find((x) => x.e.event === "loaded")?.e.ms;
const rows: string[] = [];
const finals: number[] = [];
const firsts: number[] = [];
const lags: number[] = [];
dones.forEach((d, i) => {
  const s = speechAt[i];
  if (!s) return;
  const first = hyps.find((h) => h.e.item === d.e.item && h.e.words.length);
  const fin = d.t - s.end;
  finals.push(fin);
  if (first) firsts.push(first.t - s.start);
  // commit lag: replay Settle's rule (agreement of two hypotheses) on this item
  const hs = hyps.filter((h) => h.e.item === d.e.item);
  const committed = new Map<number, number>();
  for (let k = 1; k < hs.length; k++) {
    const a = hs[k - 1].e.words, b = hs[k].e.words;
    for (let j = 0; j < Math.min(a.length, b.length) - 1; j++) {
      if (a[j][0] !== b[j][0]) break;
      if (b[j][2] > hs[k].e.end - 40) break;
      if (!committed.has(j)) committed.set(j, hs[k].t - (s.start + b[j][2] * 10));
    }
  }
  for (const v of committed.values()) lags.push(v);
  rows.push(`${String(i).padStart(2)}  first word ${first ? Math.round(first.t - s.start) : "-"} ms  final ${Math.round(fin)} ms  decode ${d.e.decode_ms ?? "-"} ms  "${d.e.text}"`);
});
const pct = (xs: number[], q: number) => {
  if (!xs.length) return NaN;
  const s = [...xs].sort((a, b) => a - b);
  return Math.round(s[Math.min(s.length - 1, Math.floor(q * s.length))]);
};
console.log(`engine ${engine}${load !== undefined ? `, model load ${load} ms` : ""}, ${dones.length} segments`);
for (const r of rows) console.log(r);
console.log(`final after speech ends: p50 ${pct(finals, 0.5)} ms, p95 ${pct(finals, 0.95)} ms (includes the VAD's 500 ms of silence)`);
console.log(`first word after speech starts: p50 ${pct(firsts, 0.5)} ms`);
console.log(`commit lag (word end in audio -> committed): p50 ${pct(lags, 0.5)} ms, p95 ${pct(lags, 0.95)} ms over ${lags.length} words`);
