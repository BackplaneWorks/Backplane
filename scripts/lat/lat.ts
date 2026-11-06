import { cbor } from "./cbor.ts";
// bun lat.ts URL(ws://host:port) TOKEN [N=200] [M=30]   — RPC round trip and reconnect times against a hub's phone stream (/ws?view=1&mobile=1&enc=cbor)
const [url, token, N = "200", M = "30"] = Bun.argv.slice(2);
const full = `${url}/ws?view=1&mobile=1&enc=cbor&token=${token}`;
const now = () => performance.now();
function pct(xs: number[], q: number) { const s = [...xs].sort((a, b) => a - b); return s[Math.min(s.length - 1, Math.floor(q * s.length))] }
const sum = (name: string, xs: number[]) => console.log(`${name}: n=${xs.length} p50=${pct(xs, .5).toFixed(2)}ms p95=${pct(xs, .95).toFixed(2)}ms max=${Math.max(...xs).toFixed(2)}ms`);
type Sock = { ws: WebSocket; frames: (() => any)[]; next: (pred: (v: any) => boolean) => Promise<any>; first: number; opened: number };
function open(): Promise<Sock> { return new Promise((res, rej) => {
  const t0 = now(); const ws = new WebSocket(full); ws.binaryType = "arraybuffer";
  const waiters: { pred: (v: any) => boolean; ok: (v: any) => void }[] = []; let first = 0, opened = 0;
  const s: any = { ws, first: 0, opened: 0, next: (pred: (v: any) => boolean) => new Promise(ok => waiters.push({ pred, ok })) };
  ws.onopen = () => { opened = now() - t0; s.opened = opened };
  ws.onmessage = (e) => { const raw = e.data instanceof ArrayBuffer ? new Uint8Array(e.data) : null; const v = raw ? cbor(raw) : JSON.parse(e.data as string); if (raw) { s.lastRaw = raw } if (!s.greet && raw) s.greet = v; if (!first) { first = now() - t0; s.first = first; res(s) }
    for (let i = 0; i < waiters.length; i++) if (waiters[i].pred(v)) { const w = waiters.splice(i, 1)[0]; w.ok(v); break } };
  ws.onerror = () => rej(new Error("ws error")); }); }
let id = 1;
async function rpc(s: Sock): Promise<number> { const n = id++; const t = now(); const p = s.next(v => v["1"] === 5 && String(v["2"]) === String(n)); s.ws.send(JSON.stringify({ id: n, m: "ping", p: {} })); await p; return now() - t }
const s = await open();
for (let i = 0; i < 20; i++) await rpc(s); // warm
const r: number[] = []; for (let i = 0; i < +N; i++) r.push(await rpc(s));
sum("rpc round trip", r);
s.ws.close(); await Bun.sleep(200);
const c1: number[] = [], c2: number[] = [], c3: number[] = [];
for (let i = 0; i < +M; i++) { const t = now(); const x = await open(); c1.push(x.opened); c2.push(x.first); const d = await rpc(x); c3.push(now() - t); x.ws.close(); await Bun.sleep(150) }
sum("reconnect: socket open", c1); sum("reconnect: first frame (greeting)", c2); sum("reconnect: first RPC reply", c3);

// the first view (view.open of the recent target), as the app's first request: time, bytes, and the bytes a deflate would send
import { deflateRawSync } from "zlib";
const vt: number[] = [], vb: number[] = [], vz: number[] = [];
for (let i = 0; i < 20; i++) { const x: any = await open(); const g = x.greet; const n = 140737488355333 + i; const t = now();
  const p = x.next((v: any) => v["1"] === 5 && String(v["2"]) === String(n));
  x.ws.send(JSON.stringify({ id: n, m: "view.open", p: { version: 1, hub: "http://x", generation: g.generation, epoch: g.epoch, requestRevision: 1, target: { kind: "recent", id: "" }, viewId: "", session: {}, nativeSidebar: 1 } }));
  await p; vt.push(now() - t); vb.push(x.lastRaw.length); vz.push(deflateRawSync(x.lastRaw).length); x.ws.close(); await Bun.sleep(100) }
sum("first view (view.open) round trip", vt);
console.log(`first view bytes: p50=${pct(vb, .5)} deflated p50=${pct(vz, .5)}`);
process.exit(0);
