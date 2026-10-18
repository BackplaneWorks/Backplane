// The Browser tab's stream, end to end (tools/browser's cast op, the hub's
// castio.bend): a headless hub (BINARY, with backplane-browser beside it)
// opens a page for a thread (preview_open over MCP) and a client counts
// that page's frames ({"t":"shot"}) a second:
//   1. an animating page with no client showing it: not cast, a still
//      every --idle-still-ms (5 s)
//   2. the client says browser.seen every second: up to --cast-fps (30)
//   3. it stops: not cast again within a few seconds
//   4. a still page clicked (preview_click) while shown: the frame that
//      shows the click arrives soon after (input to frame, ms)
//
//   bun test/tools/cast_e2e.ts [BINARY] [WIREDIR]      (port 3795)
// MIN_FAST=N sets the rate step 2 must reach (default 15).
import { mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const [bin = "build/backplane", wire = "build/wire"] = process.argv.slice(2);
for (const f of readdirSync(wire).filter((f) => f.endsWith(".js"))) (0, eval)(readFileSync(`${wire}/${f}`, "utf8"));
const W = (globalThis as any).Wire;
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
const PORT = Number(process.env.E2E_PORT ?? 3795);
if (PORT === 3787) throw new Error("refusing the live port");
const MIN_FAST = Number(process.env.MIN_FAST ?? 15);
let fail = 0;
const check = (ok: boolean, what: string) => { console.log(`${ok ? "ok  " : "FAIL"} ${what}`); if (!ok) fail++; };
const until = async <T>(ms: number, f: () => T | undefined) => {
  const end = Date.now() + ms;
  for (;;) { const v = f(); if (v) return v; if (Date.now() > end) return undefined; await sleep(50); }
};

const site = Bun.serve({
  port: 0, hostname: "127.0.0.1",
  fetch(req) {
    const u = new URL(req.url);
    const html = (b: string) => new Response(`<!doctype html><body style="margin:0">${b}</body>`, { headers: { "content-type": "text/html" } });
    if (u.pathname === "/anim") return html(`<canvas id=c width=800 height=500></canvas><script>
      const x = c.getContext("2d"); let t = 0;
      const f = () => { t++; x.fillStyle = "hsl(" + (t * 7 % 360) + ",70%,50%)"; x.fillRect(0, 0, 800, 500); x.fillStyle = "#000"; x.font = "80px sans-serif"; x.fillText(String(t), 300, 280); requestAnimationFrame(f); };
      f();</script>`);
    if (u.pathname === "/still") return html(`<div id=b style="width:100vw;height:100vh;background:#fff" onclick="this.style.background = this.style.background.includes('255') ? '#c00' : '#fff'">click</div>`);
    return html("home");
  },
});

const root = mkdtempSync(join(tmpdir(), "bp-cast-"));
const home = join(root, "home"), proj = join(root, "proj");
for (const d of [home, proj]) mkdirSync(d);
writeFileSync(join(home, "events.jsonl"), "");
const hub = Bun.spawn([resolve(bin), "--home", home, "--port", String(PORT), "--no-tailscale", "--foreground"], {
  env: { ...process.env, DISPLAY: "", WAYLAND_DISPLAY: "", BACKPLANE_NO_UPDATE: "1", BACKPLANE_KEYRING_LOCKED: "0", BACKPLANE_PEERS: "", BACKPLANE_GOOGLE_BAKE: "0", TMPDIR: root },
  stdout: "ignore", stderr: Bun.file(join(root, "hub.err")),
});
try {
  let up = false;
  for (let i = 0; i < 200 && !up; i++) { try { up = (await fetch(`http://127.0.0.1:${PORT}/hello`)).status === 200; } catch {} if (!up) await sleep(100); }
  check(up, "the hub is up");
  const seen: any[] = [];
  const shots: { page: string; n: number; at: number }[] = [];
  const ws = new WebSocket(`ws://127.0.0.1:${PORT}/ws`);
  ws.binaryType = "arraybuffer";
  ws.onmessage = (e) => {
    let o: any = null;
    try { o = JSON.parse(typeof e.data === "string" ? (e.data as string) : W.decode(new Uint8Array(e.data as ArrayBuffer))); } catch {}
    if (o?.t === "shot") shots.push({ page: o.bot, n: Number(o.n), at: performance.now() });
    for (const c of o?.items ?? []) seen.push(typeof c === "string" ? JSON.parse(c) : c);
  };
  await new Promise((r, j) => { ws.onopen = r; ws.onerror = j; });
  let id = 0;
  const send = (m: string, p: object) => ws.send(W.encode(JSON.stringify({ id: ++id, m, p })));
  send("project.add", { path: proj });
  const pc: any = await until(10000, () => seen.find((x) => x.$ === "ProjectCreated"));
  await sleep(500);
  send("thread.create", { project: pc.id, title: "caster", env: "local", provider: "claude" });
  const T = (await until(10000, () => seen.find((x) => x.$ === "ThreadCreated" && x.title === "caster")))?.id as string;
  const token = readFileSync(join(home, "token"), "utf8").trim();
  const tool = async (name: string, args: object) => {
    const r = await fetch(`http://127.0.0.1:${PORT}/mcp/${T}?token=${token}`, { method: "POST", headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ jsonrpc: "2.0", id: ++id, method: "tools/call", params: { name, arguments: args } }) });
    return r.text();
  };
  const opened = await tool("preview_open", { url: `http://127.0.0.1:${site.port}/anim` });
  check(!/isError":true/.test(opened), `preview_open answers (${opened.slice(0, 120)})`);
  await until(30000, () => shots.find((s) => s.page === T));
  const rate = async (secs: number, seeing: boolean) => {
    const t0 = performance.now();
    const n0 = shots.filter((s) => s.page === T).length;
    for (let i = 0; i < secs * 2; i++) {
      if (seeing && i % 2 === 0) send("browser.seen", { page: T });
      await sleep(500);
    }
    const ts = shots.filter((s) => s.page === T && s.at >= t0).map((s) => s.at);
    const gaps = ts.slice(1).map((t, i) => t - ts[i]).sort((a, b) => a - b);
    return { fps: (shots.filter((s) => s.page === T).length - n0) / secs, p50: gaps[Math.floor(gaps.length / 2)] ?? 0, p90: gaps[Math.floor(gaps.length * 0.9)] ?? 0 };
  };
  const a = await rate(10, false);
  check(a.fps > 0 && a.fps <= 0.5, `not shown: not cast, a still every few seconds (${a.fps.toFixed(2)} frames a second)`);
  const b = await rate(6, true);
  check(b.fps >= MIN_FAST, `shown (browser.seen): ${b.fps.toFixed(1)} frames a second (interval p50 ${b.p50.toFixed(0)} ms, p90 ${b.p90.toFixed(0)} ms)`);
  await sleep(4000);
  const c = await rate(10, false);
  check(c.fps <= 0.5, `no longer shown: not cast again (${c.fps.toFixed(2)} a second)`);
  // input to frame on a still page, shown
  await tool("preview_navigate", { url: `http://127.0.0.1:${site.port}/still` });
  await sleep(1500);
  const lats: number[] = [];
  for (let i = 0; i < 6; i++) {
    send("browser.seen", { page: T });
    await sleep(300);
    const before = shots.filter((s) => s.page === T).length;
    const t0 = performance.now();
    await tool("preview_click", { x: 300, y: 300 });
    const got = await until(3000, () => shots.filter((s) => s.page === T).slice(before).find((s) => s.at >= t0));
    if (got) lats.push(got.at - t0);
    await sleep(400);
  }
  lats.sort((x, y) => x - y);
  check(lats.length >= 5, `a click's frame arrives: input to frame p50 ${lats[Math.floor(lats.length / 2)]?.toFixed(0)} ms, max ${lats.at(-1)?.toFixed(0)} ms (${lats.length} of 6)`);
} finally {
  try { process.kill(hub.pid, "SIGTERM"); } catch {}
  await Promise.race([hub.exited, sleep(5000)]);
  try { process.kill(hub.pid, "SIGKILL"); } catch {}
  site.stop(true);
  if (process.env.KEEP) console.log(root); else rmSync(root, { recursive: true, force: true });
}
console.log(fail ? `${fail} failed` : "all ok");
process.exit(fail ? 1 : 0);
