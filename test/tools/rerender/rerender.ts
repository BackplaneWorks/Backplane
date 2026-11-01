// The re-render harness: what each ordinary operation makes the web client
// (and the hub behind it) redo. See README.md.
//
//   bun test/tools/rerender/rerender.ts --bin HUB --web WEBDIR
//     [--fixture synthetic|real] [--log EVENTS.jsonl --long ID --tools ID --bg ID]
//     [--scenarios open,idle,type1,burst,paste,scroll,clock,bg,send,stream,toolgroup,switch]
//     [--targets long,tools] [--out DIR] [--deep] [--gate budgets.json] [--write-budgets FILE]
//     [--port 3811] [--keep] [--headed]
//
// A hub on a temporary home (the fixture's log, stand-in Claude that streams,
// BACKPLANE_COPY=1 for a real log), the real web page in Chromium with
// instrument.js installed before it loads, and a second page (a "driver",
// not measured) that makes other threads work. Each scenario is one window;
// its numbers come from the page (DOM, sockets, frames, input timing, the
// page's own counters), from Chromium (CDP Performance metrics: layouts,
// style recalcs, script time; with --deep a CPU profile and a trace of
// forced layouts, the code paths), and from the hub (its CPU time, its
// /debug/perf rows). Writes DIR/report.json and DIR/report.md; --gate
// fails (exit 1) when a scenario passes its budget.
import { chmodSync, existsSync, mkdirSync, mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { homedir, tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { real, synthetic, type Fixture, type Target } from "./fixture";

const argv = process.argv.slice(2);
const opt = (name: string, d: string) => { const i = argv.indexOf(`--${name}`); return i >= 0 ? argv[i + 1] : d; };
const flag = (name: string) => argv.includes(`--${name}`);
const here = resolve(import.meta.dir, "../../..");
const say = (s: string) => console.error(`[rerender] ${s}`);
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

const bin = resolve(opt("bin", join(here, "build/rr/backplane-serve")));
const web = resolve(opt("web", join(here, "build/rr/web")));
const port = Number(opt("port", "3811"));
const fixtureKind = opt("fixture", "synthetic");
const outDir = resolve(opt("out", join(here, "build/rr/out")));
const deep = flag("deep");
const ALL = ["open", "idle", "type1", "burst", "paste", "scroll", "clock", "bg", "send", "stream", "toolgroup", "switch"];
const scenarios = opt("scenarios", ALL.join(",")).split(",");
const roles = opt("targets", "long,tools").split(",");
if (port === 3787) throw new Error("3787 is the live hub's port");
if (bin.startsWith(join(homedir(), ".local/share/backplane"))) throw new Error("refusing to run the installed binary");
for (const s of scenarios) if (!ALL.includes(s)) throw new Error(`unknown scenario ${s}`);
if (!existsSync(bin)) throw new Error(`no hub at ${bin}`);
if (!existsSync(join(web, "index.html"))) throw new Error(`no web client at ${web}`);
try { await fetch(`http://127.0.0.1:${port}/hello`, { signal: AbortSignal.timeout(800) }); throw new Error(`port ${port} is taken`); }
catch (e: any) { if (String(e.message).includes("taken")) throw e; }
const pw = [join(here, "tools/browser/node_modules/playwright-core"), join(homedir(), "Documents/i2cjak/backplane-bend/tools/browser/node_modules/playwright-core")].find((p) => existsSync(p)) ?? "playwright-core";
const { chromium } = await import(pw);

// The hub
// -------
const TOKEN = "7a1c0e5d948ab62350cfa19e6d834b22";
const root = mkdtempSync(join(process.env.RR_TMP ?? tmpdir(), "bp-rerender-"));
const home = join(root, "home"), agents = join(root, "agents");
for (const d of [home, agents, join(root, "locks"), join(root, "user"), join(root, "proj")]) mkdirSync(d, { recursive: true });
writeFileSync(join(home, "token"), TOKEN, { mode: 0o600 });
let fx: Fixture;
if (fixtureKind === "real") {
  const log = resolve(opt("log", join(homedir(), ".backplane-bend/events.jsonl")));
  fx = real(home, root, { log, long: opt("long", ""), tools: opt("tools", ""), bg: opt("bg", "") });
} else fx = synthetic(home, root, { projects: Number(opt("projects", "30")), long: Number(opt("long-n", "900")) });
say(`fixture: ${fx.source}, ${fx.lines} lines, ${(fx.bytes / 1e6).toFixed(1)} MB; targets ${fx.targets.map((t) => `${t.role}=${t.id}`).join(" ")}`);
writeFileSync(join(agents, "claude"), `#!/bin/sh\nexec ${process.execPath} ${resolve(here, "test/tools/claude_standin.ts")} "$@"\n`);
for (const n of ["codex", "grok", "muse", "opencode"]) writeFileSync(join(agents, n), "#!/bin/sh\nexit 1\n");
for (const n of ["claude", "codex", "grok", "muse", "opencode"]) chmodSync(join(agents, n), 0o755);
const hub = Bun.spawn([bin, "--home", home, "--port", String(port), "--host", "127.0.0.1", "--no-tailscale", "--headless", "--foreground", "--web", web], {
  cwd: root,
  env: { PATH: "/usr/bin:/bin", HOME: join(root, "user"), USER: process.env.USER ?? "h", LANG: "C.UTF-8", DISPLAY: "", WAYLAND_DISPLAY: "", DBUS_SESSION_BUS_ADDRESS: "unix:path=/nonexistent",
    BACKPLANE_AGENTS: agents, BACKPLANE_LOCK_DIR: join(root, "locks"), BACKPLANE_DEPLOY_LOCKS: join(root, "locks/a.lock"), BACKPLANE_NO_UPDATE: "1", BACKPLANE_KISTACK_SYNC: "0",
    BACKPLANE_ORDER_SERVER: "http://127.0.0.1:9", BACKPLANE_TELEMETRY: "off", BACKPLANE_PEERS: "", BACKPLANE_GOOGLE_BAKE: "0", BACKPLANE_APNS_URL: "http://127.0.0.1:9",
    ...(fixtureKind === "real" ? { BACKPLANE_COPY: "1" } : {}),
    BUN_JSC_maxPerThreadStackUsage: "4000000000",
    BP_STANDIN_MODE: "stream", BP_STREAM_N: opt("stream-n", "40"), BP_STREAM_MS: opt("stream-ms", "150"), FAKE_LOG: join(root, "agents.log") },
  stdout: Bun.file(join(root, "hub.out")), stderr: Bun.file(join(root, "hub.err")) });

const hubCpu = () => {
  // utime + stime of the hub process, in ms (clock ticks are 10 ms)
  try { const f = readFileSync(`/proc/${hub.pid}/stat`, "utf8").split(") ")[1].split(" "); return (Number(f[11]) + Number(f[12])) * 10; } catch { return 0; }
};
const hubRss = () => { try { return Number(/VmRSS:\s+(\d+)/.exec(readFileSync(`/proc/${hub.pid}/status`, "utf8"))?.[1] ?? 0) / 1024; } catch { return 0; } };
type PerfRow = { n: number; total: number; max: number };
async function hubPerf(): Promise<Record<string, PerfRow>> {
  const t = await (await fetch(`http://127.0.0.1:${port}/debug/perf`)).text();
  const out: Record<string, PerfRow> = {};
  for (const l of t.split("\n").slice(1)) { const c = l.trim().split(/\s+/); if (c.length >= 7 && !out[c[0]]) out[c[0]] = { n: Number(c[1]), total: Number(c[2]), max: Number(c[6]) }; }
  return out;
}
function perfDelta(a: Record<string, PerfRow>, b: Record<string, PerfRow>) {
  const d: Record<string, { n: number; ms: number }> = {};
  for (const [k, v] of Object.entries(b)) { const n = v.n - (a[k]?.n ?? 0); if (n > 0) d[k] = { n, ms: Math.round(v.total - (a[k]?.total ?? 0)) }; }
  return d;
}

// The page
// --------
const results: any[] = [];
let browser: any;
let failed = false;
const t00 = Date.now();
try {
  let up = false;
  for (let i = 0; i < 1800 && !up; i++) { try { up = (await fetch(`http://127.0.0.1:${port}/hello`)).ok; } catch {} if (!up) await sleep(100); }
  if (!up) throw new Error(`the hub never answered (${join(root, "hub.err")})`);
  say(`hub up in ${((Date.now() - t00) / 1000).toFixed(1)} s, ${hubRss().toFixed(0)} MB`);
  // a full Chromium (headless), not the headless shell: Long Animation
  // Frames and Event Timing behave as in the desktop app's Electron
  const pwCache = join(homedir(), ".cache/ms-playwright");
  const chrome = opt("chrome", process.env.RR_CHROME ?? (existsSync(pwCache) ? readdirSync(pwCache).filter((d) => /^chromium-\d+$/.test(d)).sort().reverse()
    .map((d) => [join(pwCache, d, "chrome-linux64/chrome"), join(pwCache, d, "chrome-linux/chrome")]).flat().find((p) => existsSync(p)) ?? "" : ""));
  browser = await chromium.launch({ headless: !flag("headed"), ...(chrome ? { executablePath: chrome } : {}), args: ["--enable-blink-features=LongAnimationFrameTiming"] });
  const ctx = await browser.newContext({ viewport: { width: 1500, height: 950 } });
  await ctx.addInitScript({ path: join(import.meta.dir, "instrument.js") });
  const page = await ctx.newPage();
  page.on("pageerror", (e: any) => say(`page error: ${e.message}`));
  const cdp = await ctx.newCDPSession(page);
  await cdp.send("Performance.enable", { timeDomain: "timeTicks" });
  const driverCtx = await browser.newContext({ viewport: { width: 1200, height: 800 } });
  const driver = await driverCtx.newPage();

  const ready = async (p: any) => {
    await p.goto(`http://127.0.0.1:${port}/#token=${TOKEN}`);
    await p.waitForFunction(() => document.documentElement.dataset.backplaneRemoteReady === "true", null, { timeout: 120000 });
    if (await p.locator(".tour-back").count()) { await p.keyboard.press("Escape"); await sleep(300); }
    // machine notices a copied home brings along (KiCad's health on this
    // machine): put away with their own close button, as the person would
    for (let i = 0; i < 4; i++) {
      const shut = await p.evaluate(() => {
        for (const el of document.querySelectorAll("[role=alertdialog], [role=dialog], [role=alert], .notice, .kicad-notice, .toast, .banner")) {
          if (!(el as HTMLElement).offsetParent || !/KiCad/.test(el.textContent || "")) continue;
          const b = [...el.querySelectorAll("button")].find((x) => /^(×|✕|x)$/i.test((x.textContent || "").trim()) || /close|dismiss|hide/i.test(x.getAttribute("aria-label") || x.title || ""));
          if (b) { (b as HTMLElement).click(); return true; }
        }
        return false;
      });
      if (!shut) break;
      await sleep(400);
    }
  };
  await ready(page);
  await ready(driver);
  const quiet = async (p: any, ms = 400, max = 20000) => {
    const t0 = Date.now();
    while (Date.now() - t0 < max) {
      const ok = await p.evaluate((q: number) => window.__bpRR.quiet() >= q && window.__bpRR.pending() === 0, ms).catch(() => false);
      if (ok) return true;
      await sleep(60);
    }
    return false;
  };
  // a thread opened as a sidebar row opens it: a click on an element whose
  // data-on-click is "select" (host.js's delegated handler). The palette
  // is hub-drawn and on a big copied home took longer than a person waits.
  const open = async (p: any, t: Target) => {
    await p.evaluate((id: string) => {
      const b = document.createElement("button");
      b.setAttribute("data-on-click", "select"); b.setAttribute("data-value", id); b.style.display = "none";
      document.body.appendChild(b); b.click(); b.remove();
    }, t.id);
    await p.waitForFunction(() => !!document.querySelector(".timeline-virtual-row") && !!document.getElementById("composer"), null, { timeout: 60000 }).catch(() => {});
  };
  const metrics = async () => Object.fromEntries((await cdp.send("Performance.getMetrics")).metrics.map((m: any) => [m.name, m.value]));
  const composer = () => page.locator("#composer");
  const timeline = () => page.locator("#timeline");
  const sendFrom = async (p: any, text: string) => {
    await p.locator("#composer").click();
    await p.keyboard.insertText(text);
    await sleep(150);
    await p.keyboard.press("Enter");
  };
  const waitText = (p: any, text: string, timeout = 60000) => p.waitForFunction((t: string) => (document.getElementById("timeline")?.textContent || "").includes(t), text, { timeout });

  // one measured window: act, then wait until it is over
  async function measure(name: string, target: Target, act: () => Promise<void>, o: { blank?: boolean; quietMs?: number; max?: number; minMs?: number; noSettle?: boolean } = {}) {
    const perf0 = await hubPerf(), cpu0 = hubCpu(), m0 = await metrics();
    if (deep) {
      await cdp.send("Profiler.enable"); await cdp.send("Profiler.setSamplingInterval", { interval: 200 }); await cdp.send("Profiler.start");
      await cdp.send("Tracing.start", { categories: "devtools.timeline,disabled-by-default-devtools.timeline.stack", transferMode: "ReturnAsStream" }).catch(() => {});
    }
    await page.evaluate((a: any) => window.__bpRR.begin(a.n, { blank: a.b }), { n: `${name}:${target.role}`, b: !!o.blank });
    const t0 = Date.now();
    let error = "";
    try { await act(); } catch (e: any) { error = String(e.message).split("\n")[0]; }
    // at least past the page's debounces (a draft's 75 ms refresh) and a hub round trip
    if (!o.noSettle) { await sleep(o.minMs ?? 400); await quiet(page, o.quietMs ?? 500, o.max ?? 20000); }
    const r = await page.evaluate(() => window.__bpRR.end());
    const m1 = await metrics(), cpu1 = hubCpu(), perf1 = await hubPerf();
    r.wallMs = Date.now() - t0;
    r.error = error;
    r.target = { role: target.role, id: fixtureKind === "real" ? target.id : target.id };
    r.chrome = { layouts: m1.LayoutCount - m0.LayoutCount, styleRecalcs: m1.RecalcStyleCount - m0.RecalcStyleCount,
      layoutMs: Math.round((m1.LayoutDuration - m0.LayoutDuration) * 1000), styleMs: Math.round((m1.RecalcStyleDuration - m0.RecalcStyleDuration) * 1000),
      scriptMs: Math.round((m1.ScriptDuration - m0.ScriptDuration) * 1000), taskMs: Math.round((m1.TaskDuration - m0.TaskDuration) * 1000), domNodes: m1.Nodes, heapMB: Math.round(m1.JSHeapUsedSize / 1e6) };
    r.hub = { cpuMs: cpu1 - cpu0, rssMB: Math.round(hubRss()), perf: perfDelta(perf0, perf1) };
    if (deep) {
      const { profile } = await cdp.send("Profiler.stop");
      r.profile = topFunctions(profile);
      r.forcedLayouts = await traceForced();
    }
    results.push(r);
    say(`${r.name}: ${r.activeMs} ms, ${r.dom.records} mutations (+${r.dom.added}/-${r.dom.removed} nodes, ${r.dom.recreatedIdentical} recreated identical), rows touched ${r.rows.touched} (unchanged ${r.rows.touchedUnchanged}), ws in ${r.ws.inBytes} B, hub ${r.hub.cpuMs} ms CPU${error ? `, ERROR ${error}` : ""}`);
    return r;
  }
  function topFunctions(profile: any) {
    const self = new Map<string, number>();
    const byId = new Map(profile.nodes.map((n: any) => [n.id, n]));
    const dt = profile.timeDeltas as number[];
    for (let i = 0; i < profile.samples.length; i++) {
      const n: any = byId.get(profile.samples[i]);
      const f = n.callFrame;
      const k = `${f.functionName || "(anon)"} ${(f.url || "").split("/").pop()}:${f.lineNumber + 1}`;
      self.set(k, (self.get(k) || 0) + (dt[i] || 0));
    }
    return [...self].filter(([k]) => !k.startsWith("(idle)") && !k.startsWith("(program)")).sort((a, b) => b[1] - a[1]).slice(0, 12).map(([k, us]) => ({ fn: k, ms: Math.round(us / 100) / 10 }));
  }
  async function traceForced() {
    const done = new Promise<any>((res) => cdp.once("Tracing.tracingComplete", res));
    await cdp.send("Tracing.end").catch(() => {});
    const { stream } = await done;
    let text = "";
    for (;;) { const c = await cdp.send("IO.read", { handle: stream }); text += c.base64Encoded ? Buffer.from(c.data, "base64").toString() : c.data; if (c.eof) break; }
    await cdp.send("IO.close", { handle: stream });
    let events: any[] = [];
    try { const j = JSON.parse(text); events = Array.isArray(j) ? j : j.traceEvents; } catch { return { error: "trace unreadable" }; }
    const layouts = events.filter((e) => e.name === "Layout" && e.ph === "B" || e.name === "Layout" && e.ph === "X");
    const forced = new Map<string, { n: number; ms: number }>();
    let nForced = 0;
    for (const e of layouts) {
      const st = e.args?.beginData?.stackTrace;
      if (!st?.length) continue;
      nForced++;
      const f = st[0];
      const k = `${f.functionName || "(anon)"} ${(f.url || "").split("/").pop()}:${f.lineNumber + 1}`;
      const x = forced.get(k) || { n: 0, ms: 0 }; x.n++; x.ms += (e.dur || 0) / 1000; forced.set(k, x);
    }
    return { layouts: layouts.length, forced: nForced, where: [...forced].sort((a, b) => b[1].n - a[1].n).slice(0, 8).map(([k, v]) => ({ at: k, n: v.n, ms: Math.round(v.ms * 10) / 10 })) };
  }

  const targets = fx.targets.filter((t) => roles.includes(t.role));
  const bg = fx.targets.find((t) => t.role === "bg");
  let tag = 0;
  for (const target of targets) {
    // where each run starts: a thread other than the target, then the target opened
    const other = fx.targets.find((t) => t !== target) ?? target;
    await open(page, other); await quiet(page, 800, 60000);
    if (scenarios.includes("open")) await measure("open", target, () => open(page, target), { quietMs: 800, max: 60000 });
    else { await open(page, target); await quiet(page, 800, 60000); }
    if (scenarios.includes("idle")) await quiet(page, 1500, 20000);
    if (scenarios.includes("idle")) await measure("idle", target, () => sleep(4000));
    if (scenarios.includes("type1")) {
      await composer().click(); await quiet(page);
      await measure("type1", target, () => page.keyboard.type("a"));
      await measure("type1b", target, () => page.keyboard.type("b"));
    }
    if (scenarios.includes("burst")) { await composer().click(); await measure("burst", target, () => page.keyboard.type("the quick brown fox jumps ov", { delay: 30 })); }
    if (scenarios.includes("paste")) {
      await composer().click();
      const big = Array.from({ length: 400 }, (_, i) => `line ${i}: ${"pasted text ".repeat(4)}`).join("\n");
      await measure("paste", target, () => page.keyboard.insertText(big));
    }
    // clear the draft (not measured)
    await composer().click(); await page.keyboard.press("Control+A"); await page.keyboard.press("Delete"); await quiet(page, 600);
    if (scenarios.includes("scroll")) {
      const box = await timeline().boundingBox();
      if (box) await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
      await measure("scroll-up", target, async () => { for (let i = 0; i < 20; i++) { await page.mouse.wheel(0, -400); await sleep(50); } }, { blank: true, quietMs: 700 });
      await measure("scroll-down", target, async () => { for (let i = 0; i < 24; i++) { await page.mouse.wheel(0, 400); await sleep(50); } }, { blank: true, quietMs: 700 });
      // back at the newest (not measured)
      for (let i = 0; i < 40; i++) await page.mouse.wheel(0, 2000);
      await quiet(page, 600);
    }
    // (after a longer quiet: a view still on its way is not the tick's)
    if (scenarios.includes("clock")) await quiet(page, 1500, 20000);
    if (scenarios.includes("clock")) await measure("clock", target, () => page.evaluate(() => document.dispatchEvent(new Event("visibilitychange"))));
    if (scenarios.includes("bg") && bg && bg !== target) {
      await open(driver, bg); await quiet(driver, 600, 60000);
      const w = `zqbgtag${++tag}`;
      await measure("bg-stream", target, async () => { await sendFrom(driver, `background work ${w}`); await waitText(driver, `${w}39`, 90000); }, { quietMs: 800, max: 30000 });
    }
    if (scenarios.includes("send") || scenarios.includes("stream")) {
      const w = `zqsend${++tag}`;
      await composer().click(); await page.keyboard.insertText(`a message to measure ${w}`); await quiet(page, 500);
      await measure("send", target, async () => { await page.keyboard.press("Enter"); await waitText(page, w, 20000); }, { noSettle: true });
      if (scenarios.includes("stream")) await measure("stream", target, () => waitText(page, `${w}39`, 90000), { quietMs: 1000, max: 30000 });
      else await waitText(page, `${w}39`, 90000).catch(() => {});
      await quiet(page, 800);
    }
    if (scenarios.includes("toolgroup")) {
      const w = `zqtg${++tag}`;
      await composer().click(); await page.keyboard.insertText(`run some tools tools:8 ${w}`); await quiet(page, 500);
      await measure("toolgroup", target, async () => { await page.keyboard.press("Enter"); await waitText(page, `${w}39`, 90000); }, { quietMs: 1000, max: 30000 });
    }
    if (scenarios.includes("switch")) {
      await measure("switch-away", target, () => open(page, other), { quietMs: 800, max: 60000 });
      await measure("switch-back", target, () => open(page, target), { quietMs: 800, max: 60000 });
    }
  }
} catch (e: any) {
  failed = true;
  say(`FAILED: ${e.stack ?? e.message}`);
  // where it stood (a real run's picture stays in its --out folder)
  try { mkdirSync(outDir, { recursive: true }); for (const p of browser?.contexts().flatMap((c: any) => c.pages()) ?? []) await p.screenshot({ path: join(outDir, `failed-${browser.contexts().flatMap((c: any) => c.pages()).indexOf(p)}.png`) }); } catch {}
} finally {
  await browser?.close().catch(() => {});
  hub.kill("SIGTERM");
  await Promise.race([hub.exited, sleep(8000)]);
  if (!flag("keep")) rmSync(root, { recursive: true, force: true }); else say(`kept ${root}`);
}

// The report
// ----------
const expected: Record<string, string[]> = {
  "type1": ["composer", "asks"], "type1b": ["composer", "asks"], "burst": ["composer", "asks"], "paste": ["composer", "asks"],
  "scroll-up": ["timeline-row", "timeline"], "scroll-down": ["timeline-row", "timeline"],
  "clock": ["workbar"], "idle": [], "bg-stream": ["sidebar"],
};
for (const r of results) {
  const kind = r.name.split(":")[0];
  const want = expected[kind];
  // waste: writes outside the parts the operation is about, rows touched
  // that end as they were, nodes thrown away and made again identical
  const outside = want ? Object.entries(r.dom.byRegion as Record<string, any>).filter(([k]) => !want.includes(k)).reduce((s, [, v]) => s + v.records, 0) : 0;
  const scrollStyle = kind.startsWith("scroll") ? r.dom.styleOnly : 0;
  r.waste = { outsideRecords: outside, touchedUnchangedRows: r.rows.touchedUnchanged, recreatedIdentical: r.dom.recreatedIdentical, netNoop: r.dom.netNoop,
    nonTransformScrollRecords: kind.startsWith("scroll") ? r.dom.records - scrollStyle : 0,
    score: outside + 10 * r.rows.touchedUnchanged + r.dom.recreatedIdentical + r.dom.netNoop + (kind.startsWith("scroll") ? r.dom.records - scrollStyle : 0) };
}
mkdirSync(outDir, { recursive: true });
const report = { at: new Date().toISOString(), fixture: { kind: fixtureKind, source: fx.source, lines: fx.lines, bytes: fx.bytes, targets: fx.targets.map((t) => ({ role: t.role, id: fixtureKind === "real" ? t.id : t.id })) },
  hub: bin, web, deep, failed, results };
writeFileSync(join(outDir, "report.json"), JSON.stringify(report, null, 1));
writeFileSync(join(outDir, "report.md"), markdown(report));
say(`wrote ${join(outDir, "report.md")}`);

function markdown(rep: any) {
  const L: string[] = [];
  L.push(`# Re-render report — ${rep.at}`, "", `Fixture: ${rep.fixture.source} (${rep.fixture.lines} lines). Hub \`${rep.hub}\`, web \`${rep.web}\`.${rep.failed ? " **The run failed part way.**" : ""}`, "");
  L.push("| scenario | active ms | mutations | +nodes | −nodes | recreated same | rows touched / unchanged | ws in KB (frames) | hub CPU ms | hub rpc (n, ms) | layouts | style | script ms | long frames | key→paint p50/p95 | wheel→frame p95 | blank / loading | renders full/part | dom.patch | memo.make |",
    "|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|");
  for (const r of rep.results) {
    const k = r.events["paint:keydown"], w = r.scroll.wheelToFrame, c = r.counts, rpc = r.hub.perf["hub.rpc"];
    L.push(`| ${r.name} | ${r.activeMs} | ${r.dom.records} | ${r.dom.added} | ${r.dom.removed} | ${r.dom.recreatedIdentical} | ${r.rows.touched} / ${r.rows.touchedUnchanged} | ${(r.ws.inBytes / 1024).toFixed(1)} (${r.ws.inFrames}) | ${r.hub.cpuMs} | ${rpc ? `${rpc.n}, ${rpc.ms}` : "–"} | ${r.chrome.layouts} | ${r.chrome.styleRecalcs} | ${r.chrome.scriptMs} | ${r.frames.long} | ${k ? `${k.p50}/${k.p95}` : "–"} | ${w.n ? w.p95 : "–"} | ${r.scroll.checkedFrames ? r.scroll.blankFrames : "–"} / ${r.scroll.loadingCount} | ${c["render.full"] ?? 0}/${c["render.part"] ?? 0} | ${c["dom.patch"] ?? 0} | ${c["memo.make"] ?? 0} |`);
  }
  L.push("", "## Worst offenders (waste score: writes outside the operation's region + 10 × rows touched but unchanged + nodes recreated identical + net no-op writes + non-transform writes while scrolling)", "");
  const ranked = [...rep.results].sort((a, b) => b.waste.score - a.waste.score).slice(0, 12);
  for (const r of ranked) {
    const regions = Object.entries(r.dom.byRegion as Record<string, any>).sort((a, b) => b[1].records - a[1].records).slice(0, 5).map(([k, v]) => `${k} ${v.records}`).join(", ");
    const sent = Object.entries(r.hubMsgs as Record<string, any>).filter(([k]) => k.startsWith("rtt:")).map(([k, v]) => `${k.slice(4)}×${v.n} (reply p50 ${v.p50} ms)`).join(", ");
    const got = Object.entries(r.hubMsgs as Record<string, any>).filter(([k]) => !k.startsWith("rtt:")).sort((a, b) => (b[1].bytes || 0) - (a[1].bytes || 0)).slice(0, 4).map(([k, v]) => `${k}×${v.n} ${(v.bytes / 1024).toFixed(0)} KB`).join(", ");
    L.push(`- **${r.name}** — score ${r.waste.score}: ${r.dom.records} mutations (${regions}); ${r.rows.touched} rows touched, ${r.rows.touchedUnchanged} unchanged; ${r.dom.recreatedIdentical} nodes recreated identical; sent ${sent || "nothing"}; got ${got || "nothing"}; hub ${r.hub.cpuMs} ms CPU; page ${c(r)}`);
    if (r.profile) L.push(`  - hot: ${r.profile.slice(0, 5).map((p: any) => `${p.fn} ${p.ms} ms`).join("; ")}`);
    if (r.forcedLayouts?.where?.length) L.push(`  - forced layouts ${r.forcedLayouts.forced}/${r.forcedLayouts.layouts}: ${r.forcedLayouts.where.slice(0, 4).map((x: any) => `${x.at} ×${x.n}`).join("; ")}`);
    if (r.frames.loaf?.length) L.push(`  - longest frame ${r.frames.loaf[0].ms} ms: ${r.frames.loaf[0].scripts.slice(0, 3).map((s: any) => `${s.invoker} ${s.fn || ""} ${s.ms} ms`).join("; ")}`);
  }
  return L.join("\n") + "\n";
  function c(r: any) { return Object.entries(r.counts).sort((a: any, b: any) => b[1] - a[1]).slice(0, 8).map(([k, v]) => `${k} ${v}`).join(", "); }
}

// The gate
// --------
const budgets = opt("gate", "");
if (budgets) {
  const b = JSON.parse(readFileSync(budgets, "utf8"));
  let over = 0;
  for (const r of results) {
    const lim = b[r.name.split(":")[0]];
    if (!lim) continue;
    const got: Record<string, number> = { mutations: r.dom.records, recreatedIdentical: r.dom.recreatedIdentical, touchedUnchangedRows: r.rows.touchedUnchanged,
      outsideRecords: r.waste.outsideRecords, wsInBytes: r.ws.inBytes, renderFull: r.counts["render.full"] ?? 0, loading: r.scroll.loadingCount, blankFrames: r.scroll.blankFrames, domPatch: r.counts["dom.patch"] ?? 0 };
    for (const [k, max] of Object.entries(lim as Record<string, number>)) if (k in got && got[k] > max) { console.log(`OVER ${r.name} ${k} ${got[k]} > ${max}`); over++; }
  }
  if (failed) { console.log("the run failed"); over++; }
  console.log(over ? `${over} over budget` : "all within budget");
  process.exit(over ? 1 : 0);
}
const wb = opt("write-budgets", "");
if (wb) {
  const b: Record<string, Record<string, number>> = {};
  for (const r of results) {
    const k = r.name.split(":")[0];
    const lim = (x: number) => Math.ceil(x * 1.25 + 2);
    const cur = { mutations: lim(r.dom.records), recreatedIdentical: lim(r.dom.recreatedIdentical), touchedUnchangedRows: lim(r.rows.touchedUnchanged), outsideRecords: lim(r.waste.outsideRecords),
      wsInBytes: lim(r.ws.inBytes), renderFull: lim(r.counts["render.full"] ?? 0), domPatch: lim(r.counts["dom.patch"] ?? 0), loading: 0, blankFrames: lim(r.scroll.blankFrames) };
    b[k] = b[k] ? Object.fromEntries(Object.entries(cur).map(([n, v]) => [n, Math.max(v, b[k][n as keyof typeof cur] ?? 0)])) : cur;
  }
  writeFileSync(wb, JSON.stringify(b, null, 1) + "\n");
  say(`wrote budgets ${wb}`);
}
process.exit(failed ? 1 : 0);
