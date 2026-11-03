// What a hub sends one client, by kind: joins a running test hub at its
// log's head (no log taken) and for SECS seconds counts each message's
// kind ("t"; "changes" split by the changes' kinds) and bytes, then prints
// messages and bytes per second.
//
//   bun test/tools/ws_tally.ts PORT HOME SECS
//
// HOME is the test hub's home (its events.jsonl gives the head). Never
// point it at the live hub (3787).
import { readFileSync } from "node:fs";
import { join, resolve } from "node:path";
const hubToken = (dir: string): string => require('node:fs').readFileSync(dir + '/token', 'utf8').trim();

const [portS, home, secsS = "20"] = process.argv.slice(2);
const port = Number(portS);
if (!port || port === 3787) throw new Error("a test hub's port, never 3787");
const secs = Number(secsS);
const here = resolve(import.meta.dir, "../..");
const cborSrc = readFileSync(join(here, "src/core/cbor.bend"), "utf8");
const dict = (name: string) => JSON.parse(new RegExp(`def Cbor\\.${name}\\(\\) -> List<&2, String>:\\s*(\\[[\\s\\S]*?\\])\\n`).exec(cborSrc)![1]) as string[];
const KEYS = dict("keys"), WORDS = dict("words");
const td = new TextDecoder();
function cborDec(b: Uint8Array): any {
  let p = 0;
  const arg = (ai: number) => {
    if (ai < 24) return ai;
    if (ai === 24) return b[p++];
    if (ai === 25) { const v = (b[p] << 8) | b[p + 1]; p += 2; return v; }
    if (ai === 26) { const v = ((b[p] << 24) >>> 0) + (b[p + 1] << 16) + (b[p + 2] << 8) + b[p + 3]; p += 4; return v; }
    throw new Error("cbor arg");
  };
  const item = (key: boolean): any => {
    const ib = b[p++], mt = ib >> 5, ai = ib & 31;
    if (mt === 7) return ai === 21 ? true : ai === 20 ? false : null;
    if (mt === 6) {
      const tag = arg(ai);
      if (tag === 6) return WORDS[item(false)];
      return Number(item(false));
    }
    const n = arg(ai);
    if (mt === 0) return key ? KEYS[n] : n;
    if (mt === 1) return -1 - n;
    if (mt === 2 || mt === 3) { const s = td.decode(b.subarray(p, p + n)); p += n; return s; }
    if (mt === 4) { const a = []; for (let i = 0; i < n; i++) a.push(item(false)); return a; }
    const o: any = {};
    for (let i = 0; i < n; i++) { const k = item(true); o[k] = item(false); }
    return o;
  };
  return item(false);
}

const logLines = readFileSync(join(home, "events.jsonl"), "utf8").split("\n").filter((l) => l.trim());
const since = logLines.length;
const origin = String(JSON.parse(logLines[0]).at ?? 0); // the hub resumes only into the log it came from
const tally = new Map<string, { n: number; bytes: number }>();
const add = (k: string, b: number) => { const r = tally.get(k) ?? { n: 0, bytes: 0 }; r.n++; r.bytes += b; tally.set(k, r); };
let total = 0, bytes = 0;
const ws = new WebSocket(`ws://127.0.0.1:${port}/ws?token=${hubToken(home)}&since=${since}&origin=${origin}`);
ws.binaryType = "arraybuffer";
let start = 0;
ws.onmessage = (e) => {
  const u = new Uint8Array(e.data as ArrayBuffer);
  if (!start) return;
  total++; bytes += u.length;
  let o: any;
  try { o = cborDec(u); } catch { add("?", u.length); return; }
  let k = String(o?.t ?? "?");
  if (k === "changes" && Array.isArray(o.items)) {
    const kinds = new Set<string>();
    for (const c of o.items) { try { kinds.add(String((typeof c === "string" ? JSON.parse(c) : c)?.$)); } catch {} }
    k += ":" + [...kinds].sort().join("+");
  }
  add(k, u.length);
};
await new Promise((r, j) => { ws.onopen = r; ws.onerror = j; });
await new Promise((r) => setTimeout(r, 1500)); // the hello and info first
start = Date.now();
await new Promise((r) => setTimeout(r, secs * 1000));
const wall = (Date.now() - start) / 1000;
ws.close();
console.log(`ws_tally: ${wall.toFixed(1)} s, ${(total / wall).toFixed(1)} msgs/s, ${(bytes / wall / 1024).toFixed(1)} KB/s`);
for (const [k, r] of [...tally.entries()].sort((a, b) => b[1].bytes - a[1].bytes).slice(0, 20))
  console.log(`  ${k.padEnd(48)} ${(r.n / wall).toFixed(1).padStart(7)} msgs/s ${(r.bytes / wall / 1024).toFixed(1).padStart(8)} KB/s  avg ${Math.round(r.bytes / r.n)} B`);
process.exit(0);
