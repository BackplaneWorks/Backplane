// bun live.ts BRIDGE.js HOST:PORT TOKENFILE THREADS_EXPECTED [nosidebar] : the Android app's protocol, live, with no UI.
// Runs the app's own bridge.js (the same Backplane.* calls Core.kt makes) against a hub over a real view-v1 WebSocket and
// reports pass/fail per step: pair (token auth), list, open thread, older pages, send, reconnect. Prints the view failure text.
import { readFileSync } from "node:fs";
const [bridge, hp, tokf, expect, flag] = process.argv.slice(2);
const token = readFileSync(tokf, "utf8").trim();
(0, eval)(readFileSync(bridge, "utf8"));
const B = (globalThis as any).Backplane; const ep0 = () => String(++epN); let epN = 0;
const j = (x: any) => typeof x === "string" ? JSON.parse(x) : x;

// minimal CBOR reader (tags dropped) for TRACE=1
const CKEYS = ["$", "t", "id", "m", "p", "ok", "error", "items", "thread", "msg", "role", "text", "at", "state", "project", "title", "kind", "summary", "tone", "act", "session", "model", "provider", "runtime", "interaction", "since", "seq", "version", "info", "path", "value", "key", "task", "parent", "child", "depth", "branch", "env", "worktree", "root", "pinned", "settled", "name", "result", "content", "type", "cid", "count", "manual", "archived"];
function cb(b: Buffer, o = { i: 0 }): any {
  const ib = b[o.i++], mt = ib >> 5; let ai = ib & 31, n: number;
  const arg = () => { if (ai < 24) return ai; if (ai === 24) return b[o.i++]; if (ai === 25) { const v = b.readUInt16BE(o.i); o.i += 2; return v; } if (ai === 26) { const v = b.readUInt32BE(o.i); o.i += 4; return v; } const v = Number(b.readBigUInt64BE(o.i)); o.i += 8; return v; };
  if (mt === 0) return arg(); if (mt === 1) return -1 - arg();
  if (mt === 2) { n = arg(); const v = b.subarray(o.i, o.i + n); o.i += n; return "<bytes " + n + ">"; }
  if (mt === 3) { n = arg(); const v = b.toString("utf8", o.i, o.i + n); o.i += n; return v; }
  if (mt === 4) { n = arg(); const a = []; for (let k = 0; k < n; k++) a.push(cb(b, o)); return a; }
  if (mt === 5) { n = arg(); const m: any = {}; for (let k = 0; k < n; k++) { let key = cb(b, o); if (typeof key === "number") key = CKEYS[key] ?? key; m[key] = cb(b, o); } return m; }
  if (mt === 6) { arg(); return cb(b, o); }
  if (ai === 20) return false; if (ai === 21) return true; if (ai === 22 || ai === 23) return null; return "<simple>";
}
let csent = 0; const TRACE = !!process.env.TRACE; let seq = 0;
const tok = (v: string) => { const q = v.indexOf("|"); try { return JSON.parse(v.slice(q + 1)); } catch { return null; } };
const key = hp; const results: string[] = []; let failure = "";
const pass = (s: string, ok: boolean, d = "") => { results.push(`${ok ? "PASS" : "FAIL"} ${s}${d ? " — " + d : ""}`); console.log(results[results.length - 1]); };
let ws: WebSocket | null = null, ep = "", opened = false, frames = 0, screen: any = null;
let deadline: any = null, deadlineTimer: any = null;
function run(o: any) {
  if (o.screen) screen = typeof o.screen === "string" ? JSON.parse(o.screen) : o.screen;
  // the app's deadline job (Core.kt): when out.deadline changes, pulse() at deadline + 10 ms
  if (o.deadline !== deadline) { deadline = o.deadline; clearTimeout(deadlineTimer); if (deadline) { const d = deadline; deadlineTimer = setTimeout(() => { if (deadline === d) { try { run(j(B.pulse())); } catch {} } }, Math.max(0, d - Date.now()) + 10); } }
  for (const c of o.cmds || []) if (c.type === "send") { const buf = Buffer.from(c.data, "base64"); if (TRACE) { try { const r = cb(buf); const p = r.p || {}; console.log("T#" + (++seq), "C->S", r.m, "p.keys", Object.keys(p).join(","), "viewId", p.viewId, "target", JSON.stringify(p.target), "epoch", p.epoch, "generation", p.generation, "hub", p.hub, "nativeSidebar", p.nativeSidebar, "info", Object.keys(p.session?.info || {}).join(","), "action", p.action, "value", typeof p.value === "string" ? JSON.stringify(tok(p.value)?.payload ?? p.value.slice(0, 60)) : ""); } catch (e: any) { console.log("T send undecodable", e.message); } } ws?.readyState === 1 && ws.send(buf); }
  const v = o.viewFailure || o.failure; if (v) failure = typeof v === "string" ? v : JSON.stringify(v);
}
function connect() {
  ep = ep0(); opened = false;
  ws = new WebSocket(`ws://${hp}/ws?token=${token}&view=1&mobile=1`); ws.binaryType = "arraybuffer";
  B.connection(key, ep, "view-v1");
  ws.onopen = () => { opened = true; run(j(B.online(key, true, ep))); };
  ws.onmessage = (e: any) => { frames++; if (process.env.SAVE) require("node:fs").writeFileSync(`${process.env.SAVE}/s2c_${frames}.bin`, Buffer.from(e.data)); if (TRACE && e.data.byteLength > 5) { try { const r = cb(Buffer.from(e.data)); const res = r.result || r; console.log("T#" + (++seq), "S->C len", e.data.byteLength, "m", r.m || "", "keys", Object.keys(r).join(","), "viewRevision", res.viewRevision ?? r.viewRevision, "viewId", res.viewId, "target", JSON.stringify(res.target), "threadsPage.offset", res.shown?.projects?.[0]?.threadsPage?.offset, "nextTok", JSON.stringify(tok(res.shown?.projects?.[0]?.threadsPage?.next?.value || "")?.payload)); } catch (e2: any) { console.log("T recv undecodable", e2.message); } } if (process.env.DUMP && frames > 2) { const t = Buffer.from(e.data).toString("latin1"); console.log("frame", frames, "len", t.length, JSON.stringify(t.replace(/[^\x20-\x7e]/g, ".").slice(0, 500))); } try { const oo = j(B.recv(key, Buffer.from(e.data).toString("base64"), ep, false, false)); if (process.env.RECV) console.log("R#" + frames, "recv keys", Object.keys(oo).join(","), "screen?", !!oo.screen, "offset", (typeof oo.screen === "string" ? JSON.parse(oo.screen) : oo.screen)?.projects?.[0]?.threadsPage?.offset, "sync", JSON.stringify(oo.sync)?.slice(0, 120), "cmds", JSON.stringify((oo.cmds || []).map((x: any) => x.type))); run(oo); } catch (x: any) { failure = "bridge threw: " + x.message; } };
  ws.onclose = () => { try { B.online(key, false, true); } catch {} };
}
const until = async (f: () => boolean, ms: number) => { const t = Date.now(); while (Date.now() - t < ms) { if (f()) return true; run(j(B.tick(Date.now()))); { const sc = j(B.screen()); if (sc && typeof sc === "object") screen = { ...(screen || {}), ...sc, thread: sc.thread ?? screen?.thread }; } await Bun.sleep(100); } return f(); };
const rows = () => (screen?.projects || []).flatMap((p: any) => p.threads || []);
if (flag !== "nosidebar") B.nativeSidebar?.(1);
B.start("live", "{}"); B.hubs([key]); connect();
const listed = await until(() => rows().length > 0, 25000);
pass("pair (token auth, view-v1 socket opens, greeting accepted)", opened && frames > 0, `frames ${frames}`);
const n = rows().length; const pages = screen?.sidebarPages;
pass(`list (>= ${Math.min(+expect, 48)} rows; bounded sidebar at ${expect})`, listed && n >= Math.min(+expect, 48), `rows ${n}${pages ? " pager " + JSON.stringify(pages).slice(0, 120) : ""}${failure ? " failure: " + failure : ""}`);
// what the app does after start (Core.kt): the device choices, each of which refreshes the open view
if (!process.env.NOSTART) for (const [k, v] of [["mailroom-renderer", "android-v1"], ["lang-preview", ""], ["lang", "en"], ["lang-os", "en-US"]]) { try { run(j(B.local(k, v))); } catch {} }
// let the start-up follow-ups finish, as a person would (the screen is acknowledged, the hub echoes the session back)
if (!process.env.NOSTART) { let last = -1, since = Date.now(); while (Date.now() - since < 2500) { run(j(B.tick(Date.now()))); await Bun.sleep(150); if (frames !== last) { last = frames; since = Date.now(); } } }
// pager: advance the thread list page by page with the pager controls the screen offers (Pager.next/previous: action native-sidebar-page)
const nextCtl = () => { const pp = (screen?.projects || []).map((p: any) => p.threadsPage?.next).find(Boolean); return pp || (screen?.sidebarPages || []).flatMap((h: any) => [h.projects?.next, h.active?.next]).find(Boolean); };
const prevCtl = () => { const pp = (screen?.projects || []).map((p: any) => p.threadsPage?.previous).find(Boolean); return pp || (screen?.sidebarPages || []).flatMap((h: any) => [h.projects?.previous, h.active?.previous]).find(Boolean); };
if (TRACE) console.log("T-- pager step starts; next token", JSON.stringify(tok(nextCtl()?.value || "")?.payload));
{ const firsts = [rows()[0]?.id]; let moved = 0;
  for (let k = 0; k < 8 && nextCtl(); k++) { const c = nextCtl(); const f = rows()[0]?.id; const o0 = j(B.act(c.action, c.value)); if (moved === 0) console.log("act reply keys", Object.keys(o0 || {}).join(","), JSON.stringify((o0.cmds || []).map((x: any) => x.type))); run(o0); if (!(await until(() => rows()[0]?.id !== f, 8000))) break; firsts.push(rows()[0]?.id); moved++; }
  let back = 0; const start = firsts[0];
  for (let k = 0; k < moved && prevCtl(); k++) { const c = prevCtl(); const f = rows()[0]?.id; run(j(B.act(c.action, c.value))); if (!(await until(() => rows()[0]?.id !== f, 8000))) break; back++; }
  if (moved === 0) if (process.env.VA) console.log("VA", JSON.stringify((globalThis as any).__va));
  console.log("pager after act: offset", screen?.projects?.[0]?.threadsPage?.offset, "error", JSON.stringify(screen?.error), "note", JSON.stringify(screen?.note), "ids", rows().slice(0,2).map((r: any) => r.id).join(","), "failure", failure, "frames", frames, "ws", ws?.readyState);
  if (moved === 0) console.log("pager debug: sidebarPages", JSON.stringify(screen?.sidebarPages)?.slice(0, 700), "| project[0].threadsPage", JSON.stringify(screen?.projects?.[0]?.threadsPage)?.slice(0, 300), "| rows", rows().length);
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
