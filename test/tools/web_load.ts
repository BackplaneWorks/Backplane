// Web load: the web client (headless Chromium) on a hub with agents
// streaming, phase by phase: each hub message's handling, each render's view
// and patch, the streaming row, long tasks, frame gaps, typing echo and
// thread open.
//
//   bun test/tools/web_load.ts --bin BACKPLANE --from PERFLOAD_ROOT [--web DIR]
//     [--port 3799] [--secs 15] [--phases bg,fg,type,open] [--profile PHASE]
//     [--cpu N] [--light] [--out results.json] [--shots DIR] [--pw PATH-TO-playwright-core] [--keep]
//     [--display :N]   (or LOAD_DISPLAY: the window too, on that running X server)
//
// --from is a folder perf_load.ts kept (`--hold --keep`, then touch its
// stop): its sanitized copy of the log (threads titled zqlong, zqopen*,
// zqrun*) and its stand-in agents (bin/). Here the log is copied again into
// a temp folder (paths rewritten into it, restart.continue off) and the
// binary runs on it with no display (the hub and the web server; a window
// in the same process made a joining page wait for its log), serving --web
// (else <binary's dir>/web), its PATH the stand-ins only. The page joins,
// then every zqrun thread streams and rooms with bots get a post every 4 s,
// as in perf_load's phases. host.js times itself when the page has
// window.__bpPerf, set here before it loads. Only the hub started here is
// stopped.
import { cpSync, existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { homedir, tmpdir } from "node:os";
import { join, resolve } from "node:path";

const argv = process.argv.slice(2);
const opt = (name: string, d: string) => {
  const i = argv.indexOf(`--${name}`);
  return i >= 0 ? argv[i + 1] : d;
};
const here = resolve(import.meta.dir, "../..");
const port = Number(opt("port", "3799"));
if (port === 3787) throw new Error("3787 is the live hub's port");
const secs = Number(opt("secs", "15"));
const phasesWanted = opt("phases", "bg,fg,type,open").split(",");
const outPath = opt("out", "");
const shots = opt("shots", "");
const pwPath = opt("pw", [join(here, "tools/browser/node_modules/playwright-core"), join(homedir(), "Documents/i2cjak/backplane-bend/tools/browser/node_modules/playwright-core")].find((p) => existsSync(p)) ?? "playwright-core");
const say = (s: string) => console.error(`[web_load] ${s}`);
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
const { chromium } = await import(pwPath);

// The hub
// -------
const bin = resolve(opt("bin", join(here, "build/backplane")));
const from = resolve(opt("from", ""));
const web = opt("web", "");
const keep = argv.includes("--keep");
if (bin.startsWith(join(homedir(), ".local/share/backplane"))) throw new Error("refusing to run the installed binary");
if (!existsSync(join(from, "home/events.jsonl")) || !existsSync(join(from, "bin/claude"))) throw new Error(`no perf_load folder at ${from}`);
try {
  await fetch(`http://127.0.0.1:${port}/hello`, { signal: AbortSignal.timeout(1000) });
  throw new Error(`something already answers on port ${port}`);
} catch (e: any) {
  if (String(e.message).startsWith("something")) throw e;
}
const root = mkdtempSync(join(tmpdir(), "bp-webload-"));
for (const d of ["home", "user", "locks", "proj", "elsewhere"]) mkdirSync(join(root, d), { recursive: true });
cpSync(join(from, "bin"), join(root, "bin"), { recursive: true });
const logLines = readFileSync(join(from, "home/events.jsonl"), "utf8").split("\n").filter(Boolean).map((l) => l.split(from).join(root));
logLines.push(JSON.stringify({ $: "SettingSet", key: "restart.continue", value: "off", at: Math.floor(Date.now() / 1000) }));
// --light: the light theme (screenshots of both)
if (argv.includes("--light")) logLines.push(JSON.stringify({ $: "SettingSet", key: "ui.theme", value: "light", at: Math.floor(Date.now() / 1000) }));
writeFileSync(join(root, "home/events.jsonl"), logLines.join("\n") + "\n");
for (const l of logLines) if (l.includes('"BotSet"')) { try { mkdirSync(join(root, "home/bots", JSON.parse(l).id), { recursive: true }); } catch {} }
const hub = Bun.spawn([bin, "--home", join(root, "home"), "--port", String(port), "--no-tailscale", ...(web ? ["--web", resolve(web)] : [])], {
  cwd: root, stdout: Bun.file(join(root, "app.log")), stderr: Bun.file(join(root, "app.err")),
  env: {
    PATH: `${join(root, "bin")}:/usr/bin:/bin`, HOME: join(root, "user"), USER: process.env.USER ?? "h", LANG: "C.UTF-8",
    DISPLAY: opt("display", process.env.LOAD_DISPLAY ?? ""), WAYLAND_DISPLAY: "", DBUS_SESSION_BUS_ADDRESS: "unix:path=/nonexistent",
    BACKPLANE_NO_UPDATE: "1", BACKPLANE_KEYRING_LOCKED: "0", BACKPLANE_PEERS: "", BACKPLANE_LOCK_DIR: join(root, "locks"), BACKPLANE_DEPLOY_LOCKS: join(root, "locks/a.lock"),
    BACKPLANE_APNS_URL: "http://127.0.0.1:9", BACKPLANE_HOOK_URL: "", BACKPLANE_GOOGLE_API: "http://127.0.0.1:9", BACKPLANE_GOOGLE_OAUTH: "http://127.0.0.1:9",
    FAKE_LOG: join(root, "agents.log"), FAKE_SECS: "900", FAKE_PER: "3", FAKE_CYCLE: "30",
  },
});
const stopHub = async () => {
  try { hub.kill(); } catch {}
  await Promise.race([hub.exited, sleep(5000)]);
  if (!keep) rmSync(root, { recursive: true, force: true });
};
process.on("SIGINT", async () => { await stopHub(); process.exit(130); });
let up = false;
for (let i = 0; i < 600 && hub.exitCode === null && !up; i++) {
  try { up = (await fetch(`http://127.0.0.1:${port}/hello`, { signal: AbortSignal.timeout(2000) })).status === 200; } catch {}
  if (!up) await sleep(100);
}
if (!up) { await stopHub(); throw new Error(`the hub did not start; see ${root}/app.err`); }
// no real agent under it: every child named like one must be a stand-in
const kids = (pid: number) => { try { return readFileSync(`/proc/${pid}/task/${pid}/children`, "utf8").trim().split(/\s+/).filter(Boolean).map(Number); } catch { return []; } };
for (const c of kids(hub.pid)) {
  let cmd = "";
  try { cmd = readFileSync(`/proc/${c}/cmdline`, "utf8").replace(/\0/g, " "); } catch {}
  if (/\b(claude|codex|grok|muse|agy)\b/.test(cmd) && !cmd.includes(join(root, "bin"))) { await stopHub(); throw new Error(`a real agent under the test hub: ${cmd.slice(0, 120)}`); }
}
say(`hub pid ${hub.pid} on ${port}, ${root}`);

// thread ids by their last title
const byTitle = new Map<string, string>();
for (const l of logLines) {
  if (!l.includes('"ThreadCreated"') && !l.includes('"ThreadRenamed"')) continue;
  try { const o = JSON.parse(l); if (o.title) byTitle.set(o.title.split(" ")[0], o.id); } catch {}
}

// the hub's wire (core/cbor.bend), enough to send an rpc
const cborSrc = (await import("node:fs")).readFileSync(join(here, "src/core/cbor.bend"), "utf8");
const KEYS = JSON.parse(/def Cbor\.keys\(\) -> List<&2, String>:\s*(\[[\s\S]*?\])\n/.exec(cborSrc)![1]) as string[];
const te = new TextEncoder();
function cborEnc(v: any, out: number[]) {
  const head = (mb: number, n: number) => {
    if (n < 24) out.push(mb | n);
    else if (n < 256) out.push(mb | 24, n);
    else if (n < 65536) out.push(mb | 25, n >> 8, n & 255);
    else out.push(mb | 26, (n >>> 24) & 255, (n >> 16) & 255, (n >> 8) & 255, n & 255);
  };
  const str = (s: string) => { const b = te.encode(s); head(0x60, b.length); for (const x of b) out.push(x); };
  if (typeof v === "number") head(0x00, v);
  else if (typeof v === "string") str(v);
  else {
    const ks = Object.keys(v);
    head(0xa0, ks.length);
    for (const k of ks) { const i = KEYS.indexOf(k); if (i >= 0) head(0x00, i); else str(k); cborEnc(v[k], out); }
  }
}
async function driver() {
  const lines = logLines;
  const runs: string[] = [];
  const rooms: string[] = [];
  for (const l of lines) {
    if (l.includes('"ThreadCreated"') && l.includes('"title":"zqrun')) runs.push(JSON.parse(l).id);
    if (l.includes('"RoomSet"')) { const o = JSON.parse(l); if (String(o.members ?? "").split("\n").some((m: string) => m && !m.includes("@"))) rooms.push(o.id); }
  }
  const ws = new WebSocket(`ws://127.0.0.1:${port}/ws?since=${lines.length}&enc=cbor`);
  ws.binaryType = "arraybuffer";
  await new Promise((r, j) => { ws.onopen = r; ws.onerror = j; });
  let rid = 0;
  const send = (m: string, p: object) => { const out: number[] = []; cborEnc({ id: ++rid, m, p }, out); ws.send(new Uint8Array(out)); };
  runs.forEach((t, i) => send("turn.start", { thread: t, text: `stream for the perf bench ${i % 5 < 3 ? "zqsub" : ""}` }));
  let posts = 0;
  const timer = rooms.length ? setInterval(() => send("bots.post", { room: rooms[posts++ % rooms.length], text: `zqshort a quick note for the room, post ${posts}` }), 4000) : null;
  say(`driving ${runs.length} threads, ${rooms.length} rooms`);
  return () => { if (timer) clearInterval(timer); ws.close(); };
}

const exe = (() => {
  const base = join(homedir(), ".cache/ms-playwright");
  for (const d of ["chromium_headless_shell-1234", "chromium_headless_shell-1217"]) {
    const p = join(base, d, "chrome-headless-shell-linux64/chrome-headless-shell");
    if (existsSync(p)) return p;
  }
  return undefined;
})();

const browser = await chromium.launch({ executablePath: exe, headless: true, args: ["--disable-gpu"] });
const page = await browser.newPage({ viewport: { width: 1400, height: 900 } });
await page.addInitScript(() => {
  const w = window as any;
  w.__bpPerf = { msg: [], render: [], part: [], live: [], act: [], long: [], echo: [], open: [], ev: [], frame: [] };
  // every frame's gap from the last (a frame late is a gap past 16.7 ms)
  let last = 0;
  const tick = (t: number) => { if (last) w.__bpPerf.frame.push(t - last); last = t; requestAnimationFrame(tick); };
  requestAnimationFrame(tick);
  try {
    new PerformanceObserver((l) => { for (const e of l.getEntries()) w.__bpPerf.long.push(e.duration); }).observe({ type: "longtask", buffered: true });
  } catch {}
  try {
    new PerformanceObserver((l) => { for (const e of l.getEntries() as any) w.__bpPerf.ev.push([e.name, e.duration]); }).observe({ type: "event", durationThreshold: 16, buffered: true } as any);
  } catch {}
  // after the frame that follows an input: rAF then a task (the paint is between)
  const after = (t0: number, into: any[], tag: string) => requestAnimationFrame(() => setTimeout(() => into.push([performance.now() - t0, tag]), 0));
  document.addEventListener("keydown", (e) => { if ((e.target as any)?.id === "composer") after(performance.now(), w.__bpPerf.echo, e.key); }, true);
  w.__bpOpen = (el: HTMLElement, tag: string) => {
    const t0 = performance.now();
    el.click();
    after(t0, w.__bpPerf.open, tag);
  };
});
// --cpu N: the page's CPU slowed N times (a laptop or a phone's browser)
const cpu = Number(opt("cpu", "1"));
if (cpu > 1) await (await page.context().newCDPSession(page)).send("Emulation.setCPUThrottlingRate", { rate: cpu });
page.on("pageerror", (e: any) => say(`page error: ${e.message}`));

const t0 = Date.now();
await page.goto(`http://127.0.0.1:${port}/`);
await page.waitForSelector(".sidebar .thread", { timeout: 120000 });
say(`first rows after ${Date.now() - t0} ms`);
// the log's fold: wait until messages stop arriving in bursts
let lastN = -1;
for (let i = 0; i < 60; i++) {
  const n = await page.evaluate(() => (window as any).__bpPerf.msg.length);
  if (n === lastN) break;
  lastN = n;
  await sleep(1000);
}
const take = () => page.evaluate(() => {
  const p = (window as any).__bpPerf;
  const out = JSON.parse(JSON.stringify(p));
  for (const k of Object.keys(p)) p[k].length = 0;
  return out;
});
const pct = (xs: number[], q: number) => {
  if (!xs.length) return 0;
  const s = [...xs].sort((a, b) => a - b);
  return s[Math.min(s.length - 1, Math.floor(q * s.length))];
};
const st = (xs: number[]) => ({ n: xs.length, p50: +pct(xs, 0.5).toFixed(2), p99: +pct(xs, 0.99).toFixed(2), max: +Math.max(0, ...xs).toFixed(2), sum: +xs.reduce((a, b) => a + b, 0).toFixed(1) });
const results: Record<string, any> = {};
const summarize = (name: string, p: any, wall: number) => {
  const r = {
    wall_ms: wall,
    msg: st(p.msg.map((m: any) => m[0])),
    msg_shown: st(p.msg.filter((m: any) => m[2]).map((m: any) => m[0])),
    msg_kinds: Object.fromEntries([...new Set(p.msg.map((m: any) => m[1]))].map((k: any) => [k, p.msg.filter((m: any) => m[1] === k).length])),
    render_view: st(p.render.map((r: any) => r[0])),
    render_patch: st(p.render.map((r: any) => r[1])),
    render_total: st(p.render.map((r: any) => r[0] + r[1])),
    part: st(p.part),
    live: st(p.live),
    act: st(p.act.map((a: any) => a[0])),
    longtask: st(p.long),
    echo: st(p.echo.map((e: any) => e[0])),
    open: st(p.open.map((e: any) => e[0])),
    event_timing: st(p.ev.map((e: any) => e[1])),
    frame: st(p.frame),
    late: p.frame.filter((x: number) => x > 25).length,
    busy_pct: +(100 * (p.msg.reduce((a: number, m: any) => a + m[0], 0) + p.render.reduce((a: number, r: any) => a + r[0] + r[1], 0) + p.part.reduce((a: number, x: number) => a + x, 0) + p.live.reduce((a: number, x: number) => a + x, 0) + p.act.reduce((a: number, x: any) => a + x[0], 0)) / Math.max(1, wall)).toFixed(1),
  };
  results[name] = r;
  const f = (s: any) => `n${s.n} p50 ${s.p50} p99 ${s.p99} max ${s.max}`;
  console.log(`\n== ${name} (${(wall / 1000).toFixed(1)} s, busy ${r.busy_pct}%)`);
  console.log(`  msg        ${f(r.msg)}   shown: ${f(r.msg_shown)}`);
  console.log(`  render     ${f(r.render_total)}   (view p50 ${r.render_view.p50} max ${r.render_view.max}; patch p50 ${r.render_patch.p50} max ${r.render_patch.max})`);
  if (r.part.n) console.log(`  part       ${f(r.part)}`);
  if (r.live.n) console.log(`  live       ${f(r.live)}`);
  console.log(`  act        ${f(r.act)}`);
  console.log(`  longtask   ${f(r.longtask)}`);
  if (r.echo.n) console.log(`  echo       ${f(r.echo)}`);
  if (r.open.n) console.log(`  open       ${f(r.open)}`);
  console.log(`  evt>16     ${f(r.event_timing)}`);
  console.log(`  frames     ${f(r.frame)}   late (>25 ms): ${r.late}`);
  console.log(`  kinds      ${JSON.stringify(r.msg_kinds)}`);
};
const shot = async (name: string) => { if (shots) { mkdirSync(shots, { recursive: true }); await page.screenshot({ path: join(shots, `${name}.png`) }); } };
summarize("load", await take(), Date.now() - t0);
const stopDrive = await driver();
await sleep(4000);

// a thread by its title: the page's own "select" action, through an element
// that carries it, clicked (the sidebar may have its row folded away)
async function open(title: string) {
  const id = byTitle.get(title);
  if (!id) { say(`no thread ${title}`); return false; }
  await page.evaluate(([id, t]: any) => {
    const b = document.createElement("button");
    b.setAttribute("data-on-click", "select");
    b.setAttribute("data-value", id);
    b.style.display = "none";
    document.body.appendChild(b);
    (window as any).__bpOpen(b, t);
    b.remove();
  }, [id, title]);
  await sleep(600);
  return true;
}
// --profile PHASE: a CPU profile of that phase, its top functions by self
// time printed (and the .cpuprofile written next to --out)
const profPhase = opt("profile", "");
const cdp = profPhase ? await page.context().newCDPSession(page) : null;
function topSelf(prof: any, n: number) {
  const self = new Map<string, number>();
  const byId = new Map<number, any>(prof.nodes.map((x: any) => [x.id, x]));
  const dt = prof.timeDeltas;
  for (let i = 0; i < prof.samples.length; i++) {
    const nd = byId.get(prof.samples[i]);
    const k = `${nd.callFrame.functionName || "(anon)"} ${nd.callFrame.url.split("/").pop()}:${nd.callFrame.lineNumber}`;
    self.set(k, (self.get(k) ?? 0) + (dt[i] ?? 0) / 1000);
  }
  const tot = [...self.values()].reduce((a, b) => a + b, 0);
  console.log(`  profile: ${tot.toFixed(0)} ms sampled`);
  for (const [k, v] of [...self.entries()].sort((a, b) => b[1] - a[1]).slice(0, n)) console.log(`    ${v.toFixed(1).padStart(8)} ms  ${(100 * v / tot).toFixed(1).padStart(5)}%  ${k}`);
}
async function phase(name: string, body: () => Promise<void>) {
  if (!phasesWanted.includes(name)) return;
  say(`phase ${name}`);
  await take();
  const prof = cdp && name === profPhase;
  if (prof) { await cdp.send("Profiler.enable"); await cdp.send("Profiler.setSamplingInterval", { interval: 200 }); await cdp.send("Profiler.start"); }
  const a = Date.now();
  await body();
  summarize(name, await take(), Date.now() - a);
  if (prof) {
    const { profile } = await cdp.send("Profiler.stop");
    if (outPath) writeFileSync(outPath.replace(/\.json$/, "") + `.${name}.cpuprofile`, JSON.stringify(profile));
    topSelf(profile, 40);
  }
  await shot(name);
}

await phase("bg", async () => {
  await open("zqlong");
  await take();
  await sleep(secs * 1000);
});
await phase("fg", async () => {
  await open("zqruna");
  await take();
  await sleep(secs * 1000);
});
await phase("type", async () => {
  await open("zqrunb");
  await page.click("#composer");
  await take();
  const words = "the quick brown fox jumps over the lazy dog and keeps typing ";
  for (let i = 0; i < 60; i++) { await page.keyboard.press(words[i % words.length] === " " ? "Space" : words[i % words.length]); await sleep(125); }
  for (let i = 0; i < 60; i++) { await page.keyboard.press("Backspace"); await sleep(40); }
});
await phase("open", async () => {
  for (const c of "abcdefghij") await open(`zqopen${c}`);
  await open("zqlong");
});
stopDrive();
await stopHub();
if (outPath) writeFileSync(outPath, JSON.stringify(results, null, 1));
await browser.close();
