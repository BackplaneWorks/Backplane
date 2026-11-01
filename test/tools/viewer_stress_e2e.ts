// Viewer stress: opening heavy boards, schematics and 3D/mech models must
// never drop the page's connection or freeze it. A throwaway hub (temp home,
// stand-in agents, its own port) adds a project, and a Chromium page on an
// isolated X display (never the person's) opens the viewer and cycles its
// tabs again and again. Measured while it runs:
//   - the page's view sockets: every one made and every close (a close the
//     test did not force is a disconnect), and the page's connection log;
//   - the page's main thread: long tasks (PerformanceObserver), the longest;
//   - the hub's event loop: GET /hello every 200 ms from here, its slowest
//     answer (the hub answers it on the loop that also sends heartbeats);
//   - heartbeat gaps: the longest time between two frames the page got.
// --drop forces a socket close mid-run and checks the page comes back by
// itself (no button pressed) with the viewer still open.
//
//   bun test/tools/viewer_stress_e2e.ts --bin HUB --web WEBDIR --project DIR
//     [--display :87] [--port 3841] [--rounds 3] [--tabs board,schematic,3d,split,mech]
//     [--dwell 6000] [--drop] [--out result.json] [--shots DIR] [--keep]
//
// Requires an isolated display (--display or BACKPLANE_TEST_DISPLAY), never
// :0. Prints one JSON summary; exits 1 when any disconnect or a missed
// heartbeat happened, or a forced drop did not come back by itself.
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync, symlinkSync } from "node:fs";
import { homedir, tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";

const argv = process.argv.slice(2);
const opt = (name: string, d: string) => { const i = argv.indexOf(`--${name}`); return i >= 0 ? argv[i + 1] : d; };
const flag = (name: string) => argv.includes(`--${name}`);
const here = resolve(import.meta.dir, "../..");
const say = (s: string) => console.error(`[viewer_stress] ${s}`);
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

const display = opt("display", process.env.BACKPLANE_TEST_DISPLAY ?? "");
if (!display || display === ":0" || display === process.env.DISPLAY) throw new Error("needs an isolated display (--display :N), never the person's");
const port = Number(opt("port", "3841"));
if (port === 3787 || port === 3788) throw new Error("that port is a real hub's");
const bin = resolve(opt("bin", ""));
if (!existsSync(bin) || bin.startsWith(join(homedir(), ".local/share/backplane"))) throw new Error("--bin must be a built hub, not the installed one");
const web = resolve(opt("web", join(dirname(bin), "web")));
const project = resolve(opt("project", ""));
if (!existsSync(project)) throw new Error("--project DIR");
const rounds = Number(opt("rounds", "3"));
const tabs = opt("tabs", "board,schematic,3d,split,mech").split(",");
const dwell = Number(opt("dwell", "6000"));
const drop = flag("drop");
// --stall MS: the hub's loop held for MS (SIGSTOP on the test hub only) right
// after the first 3D click of round 1, as a busy hub's loop stalls
const stall = Number(opt("stall", "0"));
const keep = flag("keep");
const shots = opt("shots", "");
const kicad = opt("kicad", join(homedir(), ".backplane-bend/kicad/10.99.0-bp.4/bin"));
const pwPath = [join(here, "tools/browser/node_modules/playwright-core"), join(homedir(), "Documents/i2cjak/backplane-bend/tools/browser/node_modules/playwright-core")].find((p) => existsSync(p)) ?? "playwright-core";
const { chromium } = await import(pwPath);

try { await fetch(`http://127.0.0.1:${port}/hello`, { signal: AbortSignal.timeout(800) }); throw new Error(`port ${port} is taken`); }
catch (e: any) { if (String(e.message).startsWith("port")) throw e; }

// The hub
// -------
const root = mkdtempSync(join(tmpdir(), "bp-vstress-"));
for (const d of ["home", "user", "locks", "bin"]) mkdirSync(join(root, d), { recursive: true });
for (const a of ["claude", "codex", "grok", "muse"]) { writeFileSync(join(root, "bin", a), "#!/bin/sh\nexit 0\n", { mode: 0o755 }); }
const token = [...crypto.getRandomValues(new Uint8Array(16))].map((b) => b.toString(16).padStart(2, "0")).join("");
writeFileSync(join(root, "home/token"), token, { mode: 0o600 });
writeFileSync(join(root, "home/events.jsonl"), [["restart.continue", "off"], ["onboard.done", "1"]].map(([key, value]) => JSON.stringify({ $: "SettingSet", key, value, at: Math.floor(Date.now() / 1000) })).join("\n") + "\n");
const hub = Bun.spawn([bin, "--home", join(root, "home"), "--port", String(port), "--no-tailscale", "--foreground", "--web", web], {
  cwd: root, stdout: Bun.file(join(root, "hub.log")), stderr: Bun.file(join(root, "hub.err")),
  env: {
    BACKPLANE_AGENTS: join(root, "bin"), PATH: `${join(root, "bin")}:${kicad}:/usr/bin:/bin`, HOME: join(root, "user"), USER: process.env.USER ?? "h", LANG: "C.UTF-8",
    DISPLAY: "", WAYLAND_DISPLAY: "", DBUS_SESSION_BUS_ADDRESS: "unix:path=/nonexistent",
    BACKPLANE_NO_UPDATE: "1", BACKPLANE_ORDER_SERVER: "http://127.0.0.1:9", BACKPLANE_KEYRING_LOCKED: "0", BACKPLANE_PEERS: "", BACKPLANE_LOCK_DIR: join(root, "locks"), BACKPLANE_DEPLOY_LOCKS: join(root, "locks/a.lock"),
    BACKPLANE_APNS_URL: "http://127.0.0.1:9", BACKPLANE_HOOK_URL: "", BACKPLANE_GOOGLE_API: "http://127.0.0.1:9", BACKPLANE_GOOGLE_OAUTH: "http://127.0.0.1:9", BACKPLANE_GOOGLE_BAKE: "0",
    BACKPLANE_KISTACK_SYNC: "0", BACKPLANE_FREECAD: "/nonexistent",
  },
});
let stopped = false;
const stopHub = async () => {
  if (stopped) return;stopped = true;
  try { hub.kill(); } catch {}
  await Promise.race([hub.exited, sleep(5000)]);
  if (!keep) rmSync(root, { recursive: true, force: true });
};
process.on("SIGINT", async () => { await stopHub(); process.exit(130); });
process.on("uncaughtException", async (e) => { console.error(e); await stopHub(); process.exit(2); });
process.on("unhandledRejection", async (e) => { console.error(e); await stopHub(); process.exit(2); });
let up = false;
for (let i = 0; i < 600 && hub.exitCode === null && !up; i++) {
  try { up = (await fetch(`http://127.0.0.1:${port}/hello`, { signal: AbortSignal.timeout(2000) })).status === 200; } catch {}
  if (!up) await sleep(100);
}
if (!up) { await stopHub(); throw new Error(`the hub did not start; see ${root}/hub.err`); }
say(`hub pid ${hub.pid} on ${port}, home ${root}`);

// the hub's wire (core/cbor.bend), enough for one request
const cborSrc = readFileSync(join(here, "src/core/cbor.bend"), "utf8");
const KEYS = JSON.parse(/def Cbor\.keys\(\) -> List<&2, String>:\s*(\[[\s\S]*?\])\n/.exec(cborSrc)![1]) as string[];
const te = new TextEncoder();
function cborEnc(v: any, out: number[]) {
  const head = (mb: number, n: number) => {
    if (n < 24) out.push(mb | n); else if (n < 256) out.push(mb | 24, n); else if (n < 65536) out.push(mb | 25, n >> 8, n & 255);
    else out.push(mb | 26, (n >>> 24) & 255, (n >> 16) & 255, (n >> 8) & 255, n & 255);
  };
  const str = (s: string) => { const b = te.encode(s); head(0x60, b.length); for (const x of b) out.push(x); };
  if (typeof v === "number") head(0x00, v);
  else if (typeof v === "string") str(v);
  else { const ks = Object.keys(v); head(0xa0, ks.length); for (const k of ks) { const i = KEYS.indexOf(k); if (i >= 0) head(0x00, i); else str(k); cborEnc(v[k], out); } }
}
{
  const ws = new WebSocket(`ws://127.0.0.1:${port}/ws?since=0&enc=cbor`);
  ws.binaryType = "arraybuffer";
  await new Promise((r, j) => { ws.onopen = r; ws.onerror = j; });
  const replies: string[] = [];
  ws.onmessage = (e: any) => { const b = new Uint8Array(e.data);const t = new TextDecoder().decode(b);if (t.includes("project") || t.includes("error") || t.includes("err")) replies.push(t.replace(/[^\x20-\x7e]/g, ".").slice(0, 200)); };
  const out: number[] = [];cborEnc({ id: 1, m: "project.add", p: { path: project } }, out);ws.send(new Uint8Array(out));
  await sleep(2500);ws.close();
  say("project.add: " + JSON.stringify(replies.slice(-3)));
}

// The hub's loop: how long /hello takes, every 200 ms
const hubLag: number[] = [];
let probing = true;
const probe = (async () => {
  while (probing) {
    const t0 = performance.now();
    try { await fetch(`http://127.0.0.1:${port}/hello`, { signal: AbortSignal.timeout(60000) }); } catch {}
    hubLag.push(performance.now() - t0);
    await sleep(200);
  }
})();

// The page
// --------
const chrome = opt("chrome", ["/usr/bin/google-chrome", join(homedir(), ".cache/ms-playwright/chromium-1234/chrome-linux64/chrome")].find((p) => existsSync(p)) ?? "");
const browser = await chromium.launch({
  headless: false, ...(chrome ? { executablePath: chrome } : {}),
  env: { ...process.env, DISPLAY: display, WAYLAND_DISPLAY: "" },
  args: ["--use-angle=swiftshader", "--enable-unsafe-swiftshader", "--ignore-gpu-blocklist", "--window-size=1600,1000", "--no-first-run", "--disable-background-timer-throttling", "--disable-renderer-backgrounding"],
});
const page = await browser.newPage({ viewport: { width: 1600, height: 1000 } });
// --cpu N: the page's CPU N times slower (a busy machine)
const cpu = Number(opt("cpu", "1"));
if (cpu > 1) { const cdp = await page.context().newCDPSession(page);await cdp.send("Emulation.setCPUThrottlingRate", { rate: cpu }); }
await page.addInitScript(() => {
  const w = window as any;
  const st = w.__vs = { long: [] as number[], sockets: 0, closes: [] as any[], gaps: [] as number[], forced: 0, frames: 0, last: 0 };
  try { new PerformanceObserver((l) => { for (const e of l.getEntries()) st.long.push(Math.round(e.duration)); }).observe({ type: "longtask", buffered: true }); } catch {}
  const Real = WebSocket;
  w.__vsSockets = [] as WebSocket[];
  class Watched extends Real {
    constructor(url: string | URL, p?: string | string[]) {
      super(url, p);
      if (!String(url).includes("view=1")) return;
      st.sockets++;w.__vsSockets.push(this);
      this.addEventListener("message", () => { const t = performance.now(); if (st.last) st.gaps.push(t - st.last); st.last = t; st.frames++; });
      this.addEventListener("close", (e: CloseEvent) => { st.closes.push({ at: Date.now(), code: e.code, forced: (this as any).__forced === true }); st.last = 0; });
    }
  }
  w.WebSocket = Watched;
});
const pageErrors: string[] = [];
page.on("pageerror", (e: any) => pageErrors.push(String(e?.message ?? e).slice(0, 200)));
await page.goto(`http://127.0.0.1:${port}/#token=${token}`);
try { await page.waitForFunction(() => (window as any).__vs?.frames > 0, null, { timeout: 30000 }); }
catch (e) {
  const dir = shots || root;mkdirSync(dir, { recursive: true });
  await page.screenshot({ path: join(dir, "nojoin.png") }).catch(() => {});
  say("no view frames: " + JSON.stringify(await page.evaluate(() => ({ text: document.body.innerText.slice(0, 400), vs: (window as any).__vs, url: location.href })).catch((x: any) => String(x))));
  throw e;
}
const shot = async (name: string) => { if (shots) { mkdirSync(shots, { recursive: true });await page.screenshot({ path: join(shots, name + ".png") }).catch(() => {}); } };

// the project's thread, then the viewer
await sleep(1500);
for (let i = 0; i < 3; i++) { const skip = page.getByRole("button", { name: "Skip", exact: true });if (await skip.count()) await skip.first().click().catch(() => {});else break;await sleep(300); }
const row = page.getByText("New thread", { exact: true }).first();
try { await row.click({ timeout: 8000 }); } catch { say("no thread row to click; the hub may have opened it"); }
await sleep(1500);
const label: Record<string, string> = { board: "Board", schematic: "Schematic", "3d": "3D", split: "Split", mech: "Mech" };
const tab = (kind: string) => `.workspace-rail button[aria-label="${label[kind]}"], nav.viewer-tabs button[data-on-click="${kind === "mech" ? "mech" : "view-kind"}"]${kind === "mech" ? "" : `[data-value="${kind}"]`}`;
if (!(await page.locator(tab("board")).count())) {
  const open = page.locator(`[data-on-click="panel"]`).or(page.getByRole("button", { name: /^Viewer/ })).first();
  await open.click({ timeout: 8000 }).catch(() => say("no viewer button"));
  await sleep(1000);
}
await shot("start");

// a main-thread responsiveness probe: a rAF + timer heartbeat in the page
await page.evaluate(() => {
  const w = window as any;w.__vs.ticks = [];let last = performance.now();
  setInterval(() => { const t = performance.now();w.__vs.ticks.push(t - last);last = t; }, 100);
  w.__vs.said = [];w.__vs.buttons = [];
  setInterval(() => {
    for (const id of ["view-connection", "reconnect-note"]) {
      const el = document.getElementById(id);
      if (!el || el.hidden || el.style.display === "none" || !el.textContent) continue;
      const text = (document.getElementById("view-connection-text") && id === "view-connection" ? document.getElementById("view-connection-text")!.textContent : el.textContent) || "";
      if (text && !w.__vs.said.includes(text)) w.__vs.said.push(text);
      for (const b of el.querySelectorAll("button")) if (!(b as any).hidden && (b as any).offsetParent !== null && !w.__vs.buttons.includes(b.textContent)) w.__vs.buttons.push(b.textContent);
    }
  }, 250);
});

// --profile: a CPU profile of the page over the whole run, self time by function
const profiling = flag("profile");
const prof = profiling ? await page.context().newCDPSession(page) : null;
if (prof) { await prof.send("Profiler.enable");await prof.send("Profiler.setSamplingInterval", { interval: 500 });await prof.send("Profiler.start"); }
const t0 = Date.now();
let forcedAt = 0, stalledAt = 0;
for (let r = 0; r < rounds; r++) {
  for (const kind of tabs) {
    const b = page.locator(tab(kind)).first();
    const clickAt = performance.now();
    try { await b.click({ timeout: 15000 }); } catch (e: any) { say(`round ${r} ${kind}: no tab (${String(e.message).split("\n")[0]})`);continue; }
    const clickMs = performance.now() - clickAt;
    if (clickMs > 1000) say(`round ${r} ${kind}: the click took ${Math.round(clickMs)} ms (page busy)`);
    if (stall && r === 1 && kind === "3d" && !stalledAt) {
      // the click's request is on its way; hold the hub's loop meanwhile
      stalledAt = Date.now();process.kill(hub.pid, "SIGSTOP");say(`hub held ${stall} ms`);
      await sleep(stall);process.kill(hub.pid, "SIGCONT");
    }
    if (drop && r === 1 && kind === "3d" && !forcedAt) {
      await sleep(800);
      forcedAt = Date.now();
      await page.evaluate(() => { const w = window as any;for (const s of w.__vsSockets) if (s.readyState === 1) { s.__forced = true;w.__vs.forced++;s.close(4000, "forced by test"); } });
      say("forced a socket close");
    }
    await sleep(dwell);
    await shot(`r${r}-${kind}`);
  }
}
const elapsed = Date.now() - t0;
let profileTop: string[] = [];
if (prof) {
  const { profile } = await prof.send("Profiler.stop");
  const self = new Map<string, number>();
  const dt = new Map<number, number>();
  for (let i = 0; i < profile.samples.length; i++) dt.set(profile.samples[i], (dt.get(profile.samples[i]) ?? 0) + (profile.timeDeltas[i] ?? 0));
  for (const n of profile.nodes) {
    const f = n.callFrame, key = `${f.functionName || "(anon)"} ${String(f.url).split("/").pop()}:${f.lineNumber}`;
    self.set(key, (self.get(key) ?? 0) + (dt.get(n.id) ?? 0) / 1000);
  }
  profileTop = [...self].filter(([k]) => !/^\((idle|program|garbage collector)\)/.test(k)).sort((a, b) => b[1] - a[1]).slice(0, 25).map(([k, ms]) => `${Math.round(ms)} ms ${k}`);
  const gc = [...self].find(([k]) => k.startsWith("(garbage collector)"));
  if (gc) profileTop.push(`${Math.round(gc[1])} ms (gc)`);
}
await sleep(2000);

const pageState = await page.evaluate(() => {
  const w = window as any;
  const btn = [...document.querySelectorAll("#view-connection button")].filter((b: any) => !b.hidden && b.offsetParent !== null).map((b: any) => b.textContent);
  const bar = document.getElementById("view-connection");
  let log: any[] = [];try { log = JSON.parse(localStorage.getItem("bp.connection.log.v1") || "[]"); } catch {}
  return { vs: w.__vs, buttonsVisible: btn, barText: bar && !bar.hidden && bar.style.display !== "none" ? bar.textContent : "", log,
    open: !!document.querySelector('.workspace-rail button[aria-pressed="true"], nav.viewer-tabs button.on'),
    ready: !document.documentElement.classList.contains("hub-down") && !document.querySelector("[data-backplane-held='quiet']") && !/retry explicitly|unavailable|read-only/i.test(document.getElementById("view-connection")?.textContent || "") };
});
const ticks: number[] = pageState.vs.ticks ?? [];
probing = false;await probe;
let hubPerf = "";
try { hubPerf = await (await fetch(`http://127.0.0.1:${port}/debug/perf`, { signal: AbortSignal.timeout(5000) })).text(); } catch {}
await browser.close();
await stopHub();

const max = (xs: number[]) => xs.reduce((a, b) => Math.max(a, b), 0);
const pct = (xs: number[], p: number) => { const s = [...xs].sort((a, b) => a - b);return s.length ? s[Math.min(s.length - 1, Math.floor(s.length * p))] : 0; };
const closes = pageState.vs.closes as any[];
const unforced = closes.filter((c) => !c.forced);
const reconnectedAfterForce = forcedAt ? pageState.vs.sockets >= 2 && (pageState.log as any[]).some((e) => e.event === "ready" && e.at > forcedAt) : null;
const summary = {
  build: bin, cpu, rounds, tabs, dwellMs: dwell, elapsedMs: elapsed,
  disconnects: unforced.length, closes, socketsMade: pageState.vs.sockets,
  heartbeatExpired: (pageState.log as any[]).filter((e) => e.cause === "heartbeat-expired" && e.at >= t0).length,
  longTasks: { count: pageState.vs.long.length, over200: pageState.vs.long.filter((x: number) => x > 200).length, maxMs: max(pageState.vs.long) },
  pageTimerStallMaxMs: Math.round(max(ticks)), pageTimerP99Ms: Math.round(pct(ticks, 0.99)),
  frameGapMaxMs: Math.round(max(pageState.vs.gaps)),
  hubStallMaxMs: Math.round(max(hubLag)), hubStallP99Ms: Math.round(pct(hubLag, 0.99)), hubOver1s: hubLag.filter((x) => x > 1000).length,
  forcedDrop: forcedAt ? { reconnected: reconnectedAfterForce } : null,
  hubHeldMs: stall || null,
  statusShown: pageState.vs.said ?? [], buttonsShown: pageState.vs.buttons ?? [],
  remoteReadyAtEnd: pageState.ready,
  buttonsVisibleAtEnd: pageState.buttonsVisible, barTextAtEnd: pageState.barText, viewerOpenAtEnd: pageState.open,
  pageErrors: pageErrors.slice(0, 10),
  profileTop,
  hubPerfTop: hubPerf.split("\n").filter((l) => /^\s*hub\.|plot|push|Plot|bin/i.test(l)).slice(0, 25),
  logTail: (pageState.log as any[]).filter((e) => e.at >= t0).map((e) => `${e.event}:${e.cause ?? ""}${e.closeCode ? ":" + e.closeCode : ""}`),
};
const text = JSON.stringify(summary, null, 1);
console.log(text);
const outPath = opt("out", "");
if (outPath) writeFileSync(outPath, text + "\n");
const bad = summary.disconnects > 0 || summary.heartbeatExpired > 0 || (forcedAt && !reconnectedAfterForce) || summary.buttonsVisibleAtEnd.length > 0 || summary.buttonsShown.length > 0 || !summary.remoteReadyAtEnd;
process.exit(bad ? 1 : 0);
