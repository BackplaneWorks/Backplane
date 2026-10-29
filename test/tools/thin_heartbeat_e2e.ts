// A view (thin) client hears the hub's heartbeat and no broadcast.
// Thin clients take only their own view's frames; before the heartbeat
// reached them an idle web page or phone took a healthy link for dead after
// Alive.limit (40 s) and reconnected (server.bend's Wire.all.thin, Wire.beat).
// Starts a headless hub on a temporary home, opens a view socket and a
// legacy one, makes a change (a broadcast every legacy client hears), and
// waits past one heartbeat (Alive.every, 15 s). Prints "ok ..."/"FAIL ...".
//
//   bun test/tools/thin_heartbeat_e2e.ts [BINARY]
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { cbor, plain } from "../../src/web/wire.js";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const bin = resolve(process.argv[2] ?? "build/backplane-serve");
const PORT = Number(process.env.E2E_PORT ?? 3931);
if (PORT === 3787) throw new Error("3787 is the live hub's port");
const TOKEN = "5f0c2e7d948ab62350cfa19e6d834b10";
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
let fail = 0;
const check = (ok: boolean, what: string) => { console.log(`${ok ? "ok  " : "FAIL"} ${what}`); if (!ok) fail++; };

const root = mkdtempSync(join(process.env.E2E_TMP ?? tmpdir(), "bp-thinhb-"));
const home = join(root, "home"), proj = join(root, "proj");
mkdirSync(home);mkdirSync(proj);
writeFileSync(join(home, "events.jsonl"), "");
writeFileSync(join(home, "token"), TOKEN, { mode: 0o600 });
const hub = Bun.spawn([bin, "--home", home, "--port", String(PORT), "--host", "127.0.0.1", "--no-tailscale", "--headless", "--foreground"], {
  env: { ...process.env, DISPLAY: "", BACKPLANE_NO_UPDATE: "1", BACKPLANE_KISTACK_SYNC: "0", BACKPLANE_ORDER_SERVER: "http://127.0.0.1:9", BACKPLANE_TELEMETRY: "off", BACKPLANE_PEERS: "", BACKPLANE_GOOGLE_BAKE: "0" },
  stdout: "ignore", stderr: Bun.file(join(root, "hub.err")) });

// Frames decoded as the clients do (wire.js), with the hub's dictionaries
// read from core/cbor.bend (Cbor.keys, Cbor.words)
const dict = (name: string) => {
  const src = readFileSync(resolve(import.meta.dir, "../../src/core/cbor.bend"), "utf8");
  const at = src.indexOf(`def Cbor.${name}()`);
  const list = src.slice(src.indexOf("[", at), src.indexOf("]", at) + 1);
  return JSON.parse(list) as string[];
};
const KEYS = dict("keys"), WORDS = dict("words");
type Frame = { at: number; text: string; bytes: number };
const open = (q: string) => {
  const frames: Frame[] = [];
  const ws = new WebSocket(`ws://127.0.0.1:${PORT}/ws?token=${TOKEN}&${q}`);
  ws.binaryType = "arraybuffer";
  ws.onmessage = (e: MessageEvent) => {
    let text = typeof e.data === "string" ? e.data : "";
    if (!text) try { text = JSON.stringify(plain(cbor(new Uint8Array(e.data as ArrayBuffer), KEYS, WORDS))); } catch { text = JSON.stringify({ t: "undecodable" }); }
    frames.push({ at: Date.now(), text, bytes: typeof e.data === "string" ? e.data.length : (e.data as ArrayBuffer).byteLength });
  };
  let closed = false;ws.onclose = () => { closed = true; };
  return { ws, frames, closed: () => closed, opened: new Promise((r, j) => { ws.onopen = r;ws.onerror = j; }) };
};
// A request as the hub reads it: CBOR (the hub drops JSON text requests),
// plain text keys (its dictionary is optional on the way in).
const cborOf = (v: any): number[] => {
  const head = (major: number, n: number): number[] =>
    n < 24 ? [major << 5 | n] : n < 256 ? [major << 5 | 24, n] : n < 65536 ? [major << 5 | 25, n >> 8, n & 255] : [major << 5 | 26, n >>> 24 & 255, n >> 16 & 255, n >> 8 & 255, n & 255];
  if (typeof v === "number") return head(0, v);
  if (typeof v === "string") { const b = [...new TextEncoder().encode(v)]; return [...head(3, b.length), ...b]; }
  const ks = Object.keys(v);
  return [...head(5, ks.length), ...ks.flatMap((k) => [...cborOf(k), ...cborOf(v[k])])];
};
const isHb = (f: Frame) => JSON.parse(f.text).t === "hb";
const kind = (f: Frame) => String(JSON.parse(f.text).t);

try {
  let up = false;
  for (let i = 0; i < 300 && !up; i++) { try { up = (await fetch(`http://127.0.0.1:${PORT}/hello`)).ok; } catch {} if (!up) await sleep(100); }
  check(up, "hub listening");
  const thin = open("view=1&mobile=1&since=0&origin=");
  const legacy = open("since=0");
  await Promise.all([thin.opened, legacy.opened]);
  await sleep(1500);
  const greeted = thin.frames.length;
  check(greeted > 0, `the view socket is greeted (${thin.frames.map(kind).join(", ")})`);
  let t0 = Infinity;
  const later = (fs: Frame[]) => fs.filter((f) => f.at >= t0);
  // the legacy client takes its log first (a slow runner's join can outlast a
  // fixed wait, and a request sent mid-join is not the case under test)
  for (let i = 0; i < 300 && !legacy.frames.some((f) => kind(f) === "log"); i++) await sleep(100);
  check(legacy.frames.some((f) => kind(f) === "log"), `the legacy client took its log (${legacy.frames.map(kind).join(", ")})`);
  // a change: every legacy client hears it as "changes"
  t0 = Date.now();
  legacy.ws.send(new Uint8Array(cborOf({ id: 1, m: "project.add", p: { path: proj } })));
  for (let i = 0; i < 100 && !later(legacy.frames).some((f) => kind(f) === "reply"); i++) await sleep(100);
  await sleep(17000); // past one Alive.every (15 s)
  // the change itself, as every full client hears it: a "changes" frame with the project
  const legacyChanges = later(legacy.frames).filter((f) => kind(f) === "changes" && f.text.includes("ProjectCreated"));
  check(later(legacy.frames).some((f) => kind(f) === "reply"), "the hub answered the request");
  check(legacyChanges.length > 0, `the legacy client heard the change (${legacyChanges.length} frames: ${later(legacy.frames).map((f) => kind(f) === "reply" ? f.text.slice(0, 120) : kind(f)).join(", ") || "none"})`);
  check(later(legacy.frames).some(isHb), "the legacy client heard a heartbeat");
  const thinAfter = later(thin.frames);
  check(thinAfter.some(isHb), `the view client heard a heartbeat (${thinAfter.map(kind).join(", ") || "nothing"})`);
  const leaked = thinAfter.filter((f) => ["changes", "log", "delta", "undecodable"].includes(kind(f)));
  check(leaked.length === 0, `the view client heard no broadcast (${leaked.map(kind).join(", ") || "none"})`);
  check(!thin.closed() && !legacy.closed(), "both links stay up");
  thin.ws.close();legacy.ws.close();
} finally {
  hub.kill("SIGTERM");
  await hub.exited;
  if (!process.env.E2E_KEEP) rmSync(root, { recursive: true, force: true });
}
if (fail) { console.log(`${fail} failed`);process.exit(1); }
console.log("all ok");
