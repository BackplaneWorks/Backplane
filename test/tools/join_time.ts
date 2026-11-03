// How long a client joining a running test hub takes to get its whole log:
// joins /ws with nothing (the whole log, as a new web page does) and times
// the first frame, each chunk, and the frame that makes it live
// ("boot": false); then counts what it gets live for a few seconds.
//
//   bun test/tools/join_time.ts PORT [LIVE_SECS] [HOME]   (HOME: the hub's, for its token)
//
// Never the live hub (3787).
import { readFileSync } from "node:fs";
import { join, resolve } from "node:path";

const [portS, liveS = "5", home = ""] = process.argv.slice(2);
const token = home ? readFileSync(join(home, "token"), "utf8").trim() : (process.env.BACKPLANE_TOKEN ?? "");
const port = Number(portS);
if (!port || port === 3787) throw new Error("a test hub's port, never 3787");
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

const t0 = performance.now();
let first = 0, live = 0, frames = 0, bytes = 0, items = 0, gapMax = 0, lastAt = 0, liveMsgs = 0, liveDeltas = 0;
const ws = new WebSocket(`ws://127.0.0.1:${port}/ws?token=${token}`);
ws.binaryType = "arraybuffer";
ws.onmessage = (e) => {
  const u = new Uint8Array(e.data as ArrayBuffer);
  const now = performance.now();
  if (live) { liveMsgs++; try { if (cborDec(u)?.t === "delta") liveDeltas++; } catch {} return; }
  frames++; bytes += u.length;
  if (!first) first = now;
  if (lastAt) gapMax = Math.max(gapMax, now - lastAt);
  lastAt = now;
  let o: any = null;
  try { o = cborDec(u); } catch {}
  if (Array.isArray(o?.items)) items += o.items.length;
  if (o?.boot === false || (o?.t === "log" && !o?.more)) live = now;
};
await new Promise((r, j) => { ws.onopen = r; ws.onerror = j; });
const end = Date.now() + 120000;
while (!live && Date.now() < end) await new Promise((r) => setTimeout(r, 50));
const liveAt = live;
await new Promise((r) => setTimeout(r, Number(liveS) * 1000));
ws.close();
console.log(`join_time: first frame ${(first - t0).toFixed(0)} ms, live ${liveAt ? (liveAt - t0).toFixed(0) + " ms" : "never (120 s)"}; ${frames} frames, ${items} items, ${(bytes / 1048576).toFixed(1)} MB, longest gap ${gapMax.toFixed(0)} ms; then ${(liveMsgs / Number(liveS)).toFixed(1)} msgs/s live (${liveDeltas} deltas)`);
process.exit(liveAt ? 0 : 1);
