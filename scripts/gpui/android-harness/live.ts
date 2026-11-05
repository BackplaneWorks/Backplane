// bun live.ts BRIDGE.js HOST:PORT TOKENFILE THREADS_EXPECTED [nosidebar] : the Android app's protocol, live, with no UI.
// Runs the app's own bridge.js (the same Backplane.* calls Core.kt makes) against a hub over a real view-v1 WebSocket and
// reports pass/fail per step: pair (token auth), list, open thread, older pages, send, reconnect. Prints the view failure text.
import { readFileSync } from "node:fs";
const [bridge, hp, tokf, expect, flag] = process.argv.slice(2);
const token = readFileSync(tokf, "utf8").trim();
(0, eval)(readFileSync(bridge, "utf8"));
const B = (globalThis as any).Backplane; const ep0 = () => String(++epN); let epN = 0;
const j = (x: any) => typeof x === "string" ? JSON.parse(x) : x;
const key = hp; const results: string[] = []; let failure = "";
const pass = (s: string, ok: boolean, d = "") => { results.push(`${ok ? "PASS" : "FAIL"} ${s}${d ? " — " + d : ""}`); console.log(results[results.length - 1]); };
let ws: WebSocket | null = null, ep = "", opened = false, frames = 0, screen: any = null;
function run(o: any) {
  if (o.screen) screen = typeof o.screen === "string" ? JSON.parse(o.screen) : o.screen;
  for (const c of o.cmds || []) if (c.type === "send") ws?.readyState === 1 && ws.send(Buffer.from(c.data, "base64"));
  const v = o.viewFailure || o.failure; if (v) failure = typeof v === "string" ? v : JSON.stringify(v);
}
function connect() {
  ep = ep0(); opened = false;
  ws = new WebSocket(`ws://${hp}/ws?token=${token}&view=1&mobile=1`); ws.binaryType = "arraybuffer";
  B.connection(key, ep, "view-v1");
  ws.onopen = () => { opened = true; run(j(B.online(key, true, ep))); };
  ws.onmessage = (e: any) => { frames++; try { run(j(B.recv(key, Buffer.from(e.data).toString("base64"), ep, false, false))); } catch (x: any) { failure = "bridge threw: " + x.message; } };
  ws.onclose = () => { try { B.online(key, false, true); } catch {} };
}
const until = async (f: () => boolean, ms: number) => { const t = Date.now(); while (Date.now() - t < ms) { if (f()) return true; run(j(B.tick(Date.now()))); await Bun.sleep(100); } return f(); };
const rows = () => (screen?.projects || []).flatMap((p: any) => p.threads || []);
if (flag !== "nosidebar") B.nativeSidebar?.(1);
B.start("live", "{}"); B.hubs([key]); connect();
const listed = await until(() => rows().length > 0, 25000);
pass("pair (token auth, view-v1 socket opens, greeting accepted)", opened && frames > 0, `frames ${frames}`);
const n = rows().length; const pages = screen?.sidebarPages;
pass(`list (>= ${Math.min(+expect, 48)} rows; bounded sidebar at ${expect})`, listed && n >= Math.min(+expect, 48), `rows ${n}${pages ? " pager " + JSON.stringify(pages).slice(0, 120) : ""}${failure ? " failure: " + failure : ""}`);
// pager: advance the thread list page by page with the pager controls the screen offers (Pager.next/previous: action native-sidebar-page)
const nextCtl = () => { const pp = (screen?.projects || []).map((p: any) => p.threadsPage?.next).find(Boolean); return pp || (screen?.sidebarPages || []).flatMap((h: any) => [h.projects?.next, h.active?.next]).find(Boolean); };
const prevCtl = () => { const pp = (screen?.projects || []).map((p: any) => p.threadsPage?.previous).find(Boolean); return pp || (screen?.sidebarPages || []).flatMap((h: any) => [h.projects?.previous, h.active?.previous]).find(Boolean); };
{ const firsts = [rows()[0]?.id]; let moved = 0;
  for (let k = 0; k < 8 && nextCtl(); k++) { const c = nextCtl(); const f = rows()[0]?.id; run(j(B.act(c.action, c.value))); if (!(await until(() => rows()[0]?.id !== f, 8000))) break; firsts.push(rows()[0]?.id); moved++; }
  let back = 0; const start = firsts[0];
  for (let k = 0; k < moved && prevCtl(); k++) { const c = prevCtl(); const f = rows()[0]?.id; run(j(B.act(c.action, c.value))); if (!(await until(() => rows()[0]?.id !== f, 8000))) break; back++; }
  const want = Math.min(Math.floor((+expect - 1) / 48), 8);
  pass(`pager (advance ${want} page(s), then back)`, moved >= want && back === moved && rows()[0]?.id === start && new Set(firsts).size === firsts.length, `forward ${moved}/${want}, back ${back}, first rows ${firsts.length}${failure ? " failure: " + failure : ""}`); }
let id = rows()[0]?.id;
if (id) { run(j(B.act("select", id))); }
const opened2 = await until(() => screen?.thread?.id === id, 15000);
pass("open thread", !!id && opened2 && !failure, failure || `thread ${id}`);
const entries = () => screen?.thread?.entries || [];
const fill = +(process.env.FILL || 0);
for (let i = 0; i < fill; i++) { const t = `fill ${i}`; run(j(B.act("draft", t))); run(j(B.act("send", ""))); await until(() => JSON.stringify(screen?.thread?.entries || []).includes(t), 8000); await Bun.sleep(300); }
await until(() => !screen?.thread?.working, 20000);
console.log("failure:", failure, "sel", screen?.sel, "keys", Object.keys(screen?.thread || {}).length);
console.log("after fill: entries", entries().length, "earlier", screen?.thread?.earlier, "working", screen?.thread?.working, "id", screen?.thread?.id);
const first = entries().length, e0 = screen?.thread?.earlier ?? 0;
if (e0 > 0) { run(j(B.act("earlier", ""))); await until(() => (screen?.thread?.earlier ?? 0) < e0 || entries().length > first, 15000); }
pass("older pages", fill === 0 || (e0 > 0 && (entries().length > first || (screen?.thread?.earlier ?? 0) < e0)), `entries ${first} -> ${entries().length}, earlier ${e0} -> ${screen?.thread?.earlier ?? 0}${fill ? "" : " (short thread: not exercised)"}`);
const text = "live replay " + Date.now();
run(j(B.act("draft", text))); run(j(B.act("send", "")));
const sent = await until(() => JSON.stringify(screen?.thread?.entries || []).includes(text), +(process.env.SEND_MS || 15000));
pass("send", sent, sent ? "" : failure || `own message not in the view (queued ${JSON.stringify(screen?.thread?.queued ?? screen?.thread?.queue ?? null).slice(0,80)}, working ${screen?.thread?.working})`);
ws?.close(); await Bun.sleep(1500); const before = frames; connect();
const back = await until(() => frames > before && rows().length > 0 && !failure, 25000);
pass("reconnect", back, failure);
console.log("done", results.filter(r => r.startsWith("PASS")).length + "/" + results.length);
process.exit(results.some(r => r.startsWith("FAIL")) ? 1 : 0);
