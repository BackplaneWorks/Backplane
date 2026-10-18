// The Browser tab in the real window, measured: the native app (hub and
// window in one process) on a fresh home under an X server (DISPLAY), a
// thread opened in the window (ctrl+k, its title, Return), an animating
// page opened for it (preview_open over MCP, which brings its Browser tab
// forward), then SECS seconds read from the app's trace: each picture's
// handling (ui.msg.pic), the frames drawn (ui.frame, and their interval),
// the frames that reach the hub ({"t":"shot"} on a socket), and the CPU of
// the app and of the browser helper with its Chrome; then another thread
// is selected (the tab left): when the cast ends, and the rate and CPU
// after.
//
//   bun test/tools/cast_window.ts BINARY DISPLAY [SECS] [WIREDIR] [XPOKE]
//   (backplane-browser beside BINARY; port 3796; never the live hub)
import { existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const [binArg, display, secsS = "15", wire = "build/wire", xpokeArg = "build/xpoke"] = process.argv.slice(2);
if (!binArg || !display) throw new Error("usage: cast_window.ts BINARY DISPLAY [SECS] [WIREDIR] [XPOKE]");
const bin = resolve(binArg), xpoke = resolve(xpokeArg), secs = Number(secsS);
for (const f of readdirSync(wire).filter((f) => f.endsWith(".js"))) (0, eval)(readFileSync(`${wire}/${f}`, "utf8"));
const W = (globalThis as any).Wire;
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
const PORT = 3796;
const until = async <T>(ms: number, f: () => T | undefined) => {
  const end = Date.now() + ms;
  for (;;) { const v = f(); if (v) return v; if (Date.now() > end) return undefined; await sleep(50); }
};
const poke = async (...a: string[]) => { const p = Bun.spawn([xpoke, ...a], { env: { DISPLAY: display }, stdout: "ignore", stderr: "ignore" }); await p.exited; };
// a process's CPU ticks, and its children's (Chrome) found by parent pid
const ticks = (pid: number) => { try { const r = readFileSync(`/proc/${pid}/stat`, "utf8"); const f = r.slice(r.lastIndexOf(")") + 2).split(" "); return Number(f[11]) + Number(f[12]); } catch { return 0; } };
const kids = (pid: number): number[] => {
  const out: number[] = [];
  for (const d of readdirSync("/proc")) {
    if (!/^\d+$/.test(d)) continue;
    try { const r = readFileSync(`/proc/${d}/stat`, "utf8"); if (Number(r.slice(r.lastIndexOf(")") + 2).split(" ")[1]) === pid) out.push(Number(d)); } catch {}
  }
  return out;
};
const tree = (pid: number): number[] => { const k = kids(pid); return [pid, ...k.flatMap(tree)]; };

const site = Bun.serve({
  port: 0, hostname: "127.0.0.1",
  fetch() {
    return new Response(`<!doctype html><body style="margin:0"><canvas id=c width=1280 height=800></canvas><script>
      const x = c.getContext("2d"); let t = 0;
      const f = () => { t++; x.fillStyle = "hsl(" + (t * 7 % 360) + ",70%,50%)"; x.fillRect(0, 0, 1280, 800); x.fillStyle = "#000"; x.font = "120px sans-serif"; x.fillText(String(t), 500, 420); requestAnimationFrame(f); };
      f();</script>`, { headers: { "content-type": "text/html" } });
  },
});
const root = mkdtempSync(join(tmpdir(), "bp-castwin-"));
const home = join(root, "home"), proj = join(root, "proj"), trace = join(root, "trace.json");
for (const d of [home, proj]) mkdirSync(d);
writeFileSync(join(home, "events.jsonl"), "");
const app = Bun.spawn([bin, "--home", home, "--port", String(PORT), "--no-tailscale"], {
  env: { ...process.env, DISPLAY: display, WAYLAND_DISPLAY: "", BACKPLANE_NO_UPDATE: "1", BACKPLANE_KEYRING_LOCKED: "0", BACKPLANE_PEERS: "", BACKPLANE_GOOGLE_BAKE: "0", BACKPLANE_TRACE: trace, TMPDIR: root },
  stdout: "ignore", stderr: Bun.file(join(root, "app.err")),
});
try {
  let up = false;
  for (let i = 0; i < 300 && !up; i++) { try { up = (await fetch(`http://127.0.0.1:${PORT}/hello`)).status === 200; } catch {} if (!up) await sleep(100); }
  if (!up) throw new Error("the app did not come up");
  const seen: any[] = [];
  const shots: number[] = [];
  const ws = new WebSocket(`ws://127.0.0.1:${PORT}/ws`);
  ws.binaryType = "arraybuffer";
  ws.onmessage = (e) => {
    let o: any = null;
    try { o = JSON.parse(typeof e.data === "string" ? (e.data as string) : W.decode(new Uint8Array(e.data as ArrayBuffer))); } catch {}
    if (o?.t === "shot") shots.push(performance.now());
    for (const c of o?.items ?? []) seen.push(typeof c === "string" ? JSON.parse(c) : c);
  };
  await new Promise((r, j) => { ws.onopen = r; ws.onerror = j; });
  let id = 0;
  const send = (m: string, p: object) => ws.send(W.encode(JSON.stringify({ id: ++id, m, p })));
  send("project.add", { path: proj });
  const pc: any = await until(10000, () => seen.find((x) => x.$ === "ProjectCreated"));
  await sleep(500);
  send("thread.create", { project: pc.id, title: "zqcaster", env: "local", provider: "claude" });
  const T = (await until(10000, () => seen.find((x) => x.$ === "ThreadCreated" && x.title === "zqcaster")))?.id as string;
  await sleep(2000);
  // a fresh home starts the tour: Skip it (Escape), then the thread by title
  await poke("key", "Escape"); await sleep(400);
  await poke("ctrl", "k"); await sleep(400);
  await poke("type", "zqcaster"); await sleep(400);
  await poke("key", "Return"); await sleep(1500);
  const token = readFileSync(join(home, "token"), "utf8").trim();
  await fetch(`http://127.0.0.1:${PORT}/mcp/${T}?token=${token}`, { method: "POST", headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ jsonrpc: "2.0", id: 99, method: "tools/call", params: { name: "preview_open", arguments: { url: `http://127.0.0.1:${site.port}/` } } }) });
  await sleep(5000);
  if (process.env.SHOT) Bun.spawnSync(["import", "-window", "root", "-display", display, process.env.SHOT]);
  // the measured window
  const procs = () => tree(app.pid);
  const pf = async () => (await (await fetch(`http://127.0.0.1:${PORT}/debug/perf.json?token=${token}`)).json()).rows as any[];
  const before = await pf();
  const c0 = procs().map((p) => [p, ticks(p)] as const);
  const helper0 = procs().filter((p) => p !== app.pid).reduce((a, p) => a + ticks(p), 0);
  const app0 = ticks(app.pid);
  const t0 = performance.now();
  const s0 = shots.length;
  await sleep(secs * 1000);
  const wall = (performance.now() - t0) / 1000;
  const after = await pf();
  const hz = 100;
  const appCpu = (ticks(app.pid) - app0) / hz / wall * 100;
  const helperCpu = (procs().filter((p) => p !== app.pid).reduce((a, p) => a + ticks(p), 0) - helper0) / hz / wall * 100;
  const row = (rows: any[], n: string) => rows.find((r) => r.name === n) ?? { n: 0, sum: 0, max: 0 };
  const d = (n: string) => { const a = row(after, n), b = row(before, n); const k = a.n - b.n; return { n: k, mean: k ? (a.sum - b.sum) / k / 1000 : 0, max: a.max / 1000 }; };
  const pic = d("ui.msg.pic"), frame = d("ui.frame"), hub = d("ui.hub");
  const hubShots = (shots.length - s0) / wall;
  // the window's frame interval from the trace (ui.frame starts inside the window)
  let gaps: number[] = [];
  try {
    let txt = readFileSync(trace, "utf8").trim();
    if (!txt.endsWith("]")) txt = txt.replace(/,\s*$/, "") + "]";
    const ev = JSON.parse(txt).filter((e: any) => e.ph === "X" && e.name === "ui.frame").map((e: any) => e.ts as number).sort((a: number, b: number) => a - b);
    const last = ev.at(-1) ?? 0;
    const inWin = ev.filter((t: number) => t >= last - secs * 1e6);
    gaps = inWin.slice(1).map((t: number, i: number) => (t - inWin[i]) / 1000).sort((a: number, b: number) => a - b);
  } catch {}
  const q = (p: number) => gaps.length ? gaps[Math.min(gaps.length - 1, Math.floor(p * gaps.length))].toFixed(1) : "-";
  console.log(`cast_window: ${bin} on ${display}, ${wall.toFixed(1)} s with the Browser tab on an animating page`);
  console.log(`  frames reaching a socket client: ${hubShots.toFixed(1)} a second`);
  console.log(`  ui.msg.pic: n ${pic.n} (${(pic.n / wall).toFixed(1)} a second), mean ${pic.mean.toFixed(1)} ms, max ${pic.max.toFixed(1)} ms`);
  console.log(`  ui.frame: n ${frame.n} (${(frame.n / wall).toFixed(1)} a second), mean ${frame.mean.toFixed(1)} ms; interval p50 ${q(0.5)} ms, p90 ${q(0.9)} ms`);
  console.log(`  cpu: app ${appCpu.toFixed(0)}%, browser helper + Chrome ${helperCpu.toFixed(0)}%`);
  // leaving the tab (another thread selected): the window stops saying
  // seen, the lease runs out, and the page is no longer cast
  send("thread.create", { project: pc.id, title: "zqother", env: "local", provider: "claude" });
  await until(10000, () => seen.find((x) => x.$ === "ThreadCreated" && x.title === "zqother"));
  await sleep(1000);
  await poke("ctrl", "k"); await sleep(400);
  await poke("type", "zqother"); await sleep(400);
  const tLeave = performance.now();
  await poke("key", "Return");
  const pics0 = row(await pf(), "ui.msg.pic").n;
  await sleep(4000);
  const h1 = procs().filter((p) => p !== app.pid).reduce((a, p) => a + ticks(p), 0);
  const s1 = shots.length, t1 = performance.now();
  await sleep(10000);
  const wall2 = (performance.now() - t1) / 1000;
  const helperIdle = (procs().filter((p) => p !== app.pid).reduce((a, p) => a + ticks(p), 0) - h1) / hz / wall2 * 100;
  // the cast's end: the last frame that came less than a second after the one before it
  const after2 = shots.filter((t) => t >= tLeave);
  let end = tLeave;
  for (let i = 1; i < after2.length; i++) if (after2[i] - after2[i - 1] < 1000) end = after2[i];
  console.log(`  tab left: the cast ended ${((end - tLeave) / 1000).toFixed(1)} s after; then ${((shots.length - s1) / wall2).toFixed(2)} frames a second, browser helper + Chrome ${helperIdle.toFixed(0)}%`);
  console.log(`  (window pictures from the leave on: ${row(await pf(), "ui.msg.pic").n - pics0})`);
} finally {
  try { process.kill(app.pid, "SIGTERM"); } catch {}
  await Promise.race([app.exited, sleep(5000)]);
  try { process.kill(app.pid, "SIGKILL"); } catch {}
  site.stop(true);
  if (process.env.KEEP) console.log(root); else rmSync(root, { recursive: true, force: true });
}
process.exit(0);
