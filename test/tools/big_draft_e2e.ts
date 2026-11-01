// A 300 KB draft, the web client end to end (hub view protocol H: the view
// delta, core/draftbound.bend, thin-host.js's boundedDraft): a headless hub
// on a fresh home and the page in headless Chromium.
//   paste     300 KB into the composer: the page's socket stays up, views
//             keep arriving, every view request stays under the 256 KB
//             limit, no reply carries the draft back, and the composer
//             says the draft is over a message's bound (with both sizes)
//   reload    the page comes back on the same draft, every character of it,
//             and still says so
//   send      Enter posts nothing: the hub refuses a bounded draft and the
//             draft stays
//
//   bun test/tools/big_draft_e2e.ts --bin BACKPLANE --web DIR [--port 3826] [--wire DIR]
// Only the hub started here is stopped.
import { chmodSync, existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, writeFileSync } from "node:fs";
import { homedir, tmpdir } from "node:os";
import { join, resolve } from "node:path";

const argv = process.argv.slice(2);
const opt = (name: string, d: string) => { const i = argv.indexOf(`--${name}`); return i >= 0 ? argv[i + 1] : d; };
const here = resolve(import.meta.dir, "../..");
const bin = resolve(opt("bin", "build/backplane"));
const web = resolve(opt("web", "dist/web"));
const port = Number(opt("port", "3826"));
if (port === 3787) throw new Error("3787 is the live hub's port");
const wire = opt("wire", "build/wire");
const pwPath = [join(here, "tools/browser/node_modules/playwright-core"), join(homedir(), "Documents/i2cjak/backplane-bend/tools/browser/node_modules/playwright-core")].find((p) => existsSync(p)) ?? "playwright-core";
const { chromium } = await import(pwPath);
for (const f of readdirSync(wire).filter((f) => f.endsWith(".js"))) (0, eval)(readFileSync(`${wire}/${f}`, "utf8"));
const W = (globalThis as any).Wire;
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
let fail = 0;
const check = (ok: boolean, what: string) => { console.log(`${ok ? "ok  " : "FAIL"} ${what}`); if (!ok) fail++; };
const until = async <T>(ms: number, f: () => T | undefined | Promise<T | undefined>) => {
  const end = Date.now() + ms;
  for (;;) { const v = await f(); if (v) return v; if (Date.now() > end) return undefined; await sleep(50); }
};

const root = mkdtempSync(join(tmpdir(), "bp-bigdraft-"));
const fake = join(root, "bin");
mkdirSync(fake);
for (const n of ["claude", "codex", "grok", "muse", "agy", "opencode"]) { writeFileSync(join(fake, n), "#!/bin/sh\nexit 1\n"); chmodSync(join(fake, n), 0o755); }
const home = join(root, "home"), proj = join(root, "proj");
mkdirSync(home);mkdirSync(proj);
writeFileSync(join(home, "events.jsonl"), "");
// the page signs in with the home's token (a browser is not trusted by address)
const TOKEN = crypto.randomUUID().replace(/-/g, "");
writeFileSync(join(home, "token"), TOKEN, { mode: 0o600 });
const proc = Bun.spawn([bin, "--home", home, "--port", String(port), "--host", "127.0.0.1", "--no-tailscale", "--headless", "--foreground", "--web", web], {
  env: { PATH: `${fake}:/usr/bin:/bin`, HOME: join(root, "user"), DISPLAY: "", WAYLAND_DISPLAY: "", BACKPLANE_AGENTS: fake, BACKPLANE_NO_UPDATE: "1", BACKPLANE_KISTACK_SYNC: "0",
    BACKPLANE_ORDER_SERVER: "http://127.0.0.1:9", BACKPLANE_PEERS: "", BACKPLANE_GOOGLE_BAKE: "0", BACKPLANE_TELEMETRY: "off", BACKPLANE_LOCK_DIR: join(root, "locks"),
    BUN_JSC_maxPerThreadStackUsage: "4000000000" },
  stdout: "ignore", stderr: Bun.file(join(root, "hub.err")) });
let browser: any = null;
let page: any = null;
try {
  if (!(await until(60000, async () => { try { return (await fetch(`http://127.0.0.1:${port}/hello`)).status === 200; } catch { return undefined; } }))) throw new Error("the hub never answered");
  const ws = new WebSocket(`ws://127.0.0.1:${port}/ws`);
  ws.binaryType = "arraybuffer";
  const changes: any[] = [];
  ws.onmessage = (e) => {
    let o: any = null;
    try { o = JSON.parse(typeof e.data === "string" ? (e.data as string) : W.decode(new Uint8Array(e.data as ArrayBuffer))); } catch {}
    for (const ch of o?.items ?? []) if (o.t !== "live") changes.push(ch);
  };
  await new Promise((r, j) => { ws.onopen = r; ws.onerror = j; });
  let id = 0;
  const send = (m: string, p: object) => ws.send(W.encode(JSON.stringify({ id: ++id, m, p })));
  send("project.add", { path: proj });
  const pc = await until(10000, () => changes.find((c) => c.$ === "ProjectCreated"));
  if (!pc) throw new Error("no project");
  send("setting.set", { key: "onboard.done", value: "1" });
  send("setting.set", { key: "restart.continue", value: "off" });
  send("thread.create", { project: pc.id, title: "big draft", env: "local", provider: "claude" });
  const th = (await until(10000, () => changes.find((c) => c.$ === "ThreadCreated")))?.id as string;
  if (!th) throw new Error("no thread");

  const exe = [join(homedir(), ".cache/ms-playwright/chromium_headless_shell-1234/chrome-headless-shell-linux64/chrome-headless-shell"),
    join(homedir(), ".cache/ms-playwright/chromium_headless_shell-1217/chrome-headless-shell-linux64/chrome-headless-shell")].find((p) => existsSync(p));
  browser = await chromium.launch({ executablePath: exe, headless: true, args: ["--disable-gpu"] });
  const context = await browser.newContext({ viewport: { width: 1200, height: 800 }, serviceWorkers: "block" });
  await context.grantPermissions(["clipboard-read", "clipboard-write"], { origin: `http://127.0.0.1:${port}` });
  page = await context.newPage();
  if (process.env.BIG_DRAFT_DEBUG) { page.on("console", (m: any) => console.log("console:", m.text().slice(0, 300)));page.on("pageerror", (e: any) => console.log("pageerror:", String(e).slice(0, 300))); }
  // every frame the page's socket sends and gets, and whether it closed
  const wire = { out: [] as number[], in: [] as number[], closed: 0, draftBack: 0 };
  const big = "first words " + "pasted text, line after line\n".repeat(Number(process.env.BIG_N || 10500)) + " ends here";
  page.on("websocket", (s: any) => {
    s.on("framesent", (f: any) => wire.out.push(typeof f.payload === "string" ? Buffer.byteLength(f.payload) : f.payload.length));
    s.on("framereceived", (f: any) => {
      const n = typeof f.payload === "string" ? Buffer.byteLength(f.payload) : f.payload.length;
      wire.in.push(n);
      if (n > 100000) wire.draftBack++;
    });
    s.on("close", () => wire.closed++);
  });
  const select = () => page.evaluate((id: string) => {
    const b = document.createElement("button");
    b.setAttribute("data-on-click", "select");b.setAttribute("data-value", id);
    document.body.appendChild(b);b.click();b.remove();
  }, th);
  // the thread's own view on show (the composer of an earlier view is replaced)
  const ready = async () => {
    await sleep(1000);
    // the first-run tour, skipped (it holds every key while up)
    await sleep(500);
    for (let i = 0; i < 3; i++) { const skip = await page.$('[data-on-click="ob-skip"]');if (!skip) break;await skip.click();await sleep(400); }
    await select();
    await sleep(3000);
    if (process.env.BIG_DRAFT_DEBUG) { await page.screenshot({ path: "/tmp/h-delta/bd-sel.png" });console.log(await page.evaluate(() => [...document.documentElement.attributes].map((a) => a.name + "=" + a.value).join(" "))); }
    await page.waitForSelector('html[data-backplane-remote-ready="true"] #composer', { timeout: 30000 });
    await page.waitForFunction((id: string) => document.querySelector(`[data-thread="${id}"], .thread-head, #timeline`) !== null, th, { timeout: 30000 }).catch(() => {});
    await sleep(800);
  };
  await page.goto(`http://127.0.0.1:${port}/#token=${TOKEN}`);
  await page.waitForSelector(".sidebar .thread", { timeout: 60000 });
  await ready();
  check(Buffer.byteLength(big) > 300000, `draft is ${Buffer.byteLength(big)} bytes`);

  // paste
  const out0 = wire.out.length, in0 = wire.in.length;
  if (process.env.BIG_DRAFT_PROF) {
    const cdp = await context.newCDPSession(page);
    await cdp.send("Profiler.enable");await cdp.send("Profiler.setSamplingInterval", { interval: 1000 });await cdp.send("Profiler.start");
    const ms = await page.evaluate((t: string) => { const c = document.getElementById("composer") as any;const a = performance.now();c.value = t;c.dispatchEvent(new Event("input", { bubbles: true }));return performance.now() - a; }, big.slice(0, Number(process.env.BIG_DRAFT_PROF)));
    await sleep(3000);
    const { profile } = await cdp.send("Profiler.stop");
    const self = new Map<string, number>(), byId = new Map(profile.nodes.map((n: any) => [n.id, n]));
    for (const s of profile.samples) { const n: any = byId.get(s);const k = n.callFrame.functionName + ":" + n.callFrame.lineNumber;self.set(k, (self.get(k) || 0) + 1); }
    console.log("input event ms", ms.toFixed(0));
    console.log([...self].sort((a, b) => b[1] - a[1]).slice(0, 25).map(([k, v]) => v + " " + k).join("\n"));
    throw new Error("profiled");
  }
  // input → paint: from the input event to the task after the next frame
  // (the re-render harness's measure, test/tools/rerender/instrument.js)
  await page.evaluate(() => {
    const w = window as any;
    w.__bdPaint = [] as number[];
    document.addEventListener("input", (e) => {
      if ((e.target as any)?.id !== "composer") return;
      const t = e.timeStamp;
      requestAnimationFrame(() => setTimeout(() => w.__bdPaint.push(performance.now() - t), 0));
    }, true);
  });
  const paints = () => page.evaluate(() => (window as any).__bdPaint.splice(0));
  const p95 = (xs: number[]) => xs.slice().sort((a, b) => a - b)[Math.min(xs.length - 1, Math.floor(xs.length * 0.95))] ?? Infinity;
  // a paste as a person makes one, the clipboard then Ctrl+V: one input
  // event (Playwright's fill inserts a text of many lines as two input
  // events a line, 21,000 here)
  await page.locator("#composer").click();
  await page.evaluate((t: string) => navigator.clipboard.writeText(t), big);
  await page.keyboard.press("Control+V");
  const pasted = await until(10000, async () => { const x = await paints(); return x.length ? x : undefined; }) ?? [];
  check(pasted.length > 0 && pasted[0] <= 100, `the paste paints within 100 ms (${pasted.map((x: number) => Math.round(x)).join(", ")} ms)`);
  if (process.env.BIG_DRAFT_DEBUG) await page.screenshot({ path: "/tmp/h-delta/bd-fill.png" });
  if (process.env.BIG_DRAFT_DEBUG) console.log("composer", await page.evaluate(() => { const c = document.getElementById("composer") as any; return { tag: c?.tagName, len: c?.value?.length, max: c?.maxLength, n: document.querySelectorAll("#composer").length }; }));
  const said = await until(15000, async () => (await page.locator(".draft-limit").count()) ? await page.locator(".draft-limit").innerText() : undefined);
  check(!!said && said.includes("KB") && said.includes("128"), `the composer says so: ${JSON.stringify(said)}`);
  await sleep(1500);
  check(wire.closed === 0, "the socket stays up after the paste");
  check(wire.in.length > in0, `views keep arriving (${wire.in.length - in0} frames)`);
  check(Math.max(0, ...wire.out.slice(out0)) < 262144, `every request under the limit (largest ${Math.max(0, ...wire.out.slice(out0))} B)`);
  check(wire.draftBack === 0, "no reply carries the draft back");
  check((await page.locator("#composer").inputValue()) === big, "the composer holds the whole draft");

  // typing in it: 20 keys at its end, then 20 in its middle, each painted
  // as soon as a bare textarea holding the same text paints it (p95, within
  // a frame or 4 ms more than the bare one: the browser's own editing of a
  // 300 KB textarea is the floor, about 17 ms at p95 in headless Chromium)
  const control = await context.newPage();
  await control.setContent(`<textarea id="composer" style="width:700px;height:300px;font:14px sans-serif"></textarea>`);
  await control.evaluate((t: string) => {
    const c = document.getElementById("composer") as HTMLTextAreaElement, w = window as any;
    c.value = t;w.__bdPaint = [];
    c.addEventListener("input", (e) => { const t0 = e.timeStamp;requestAnimationFrame(() => setTimeout(() => w.__bdPaint.push(performance.now() - t0), 0)); });
  }, big);
  const floor = async (at: "end" | "mid") => {
    await control.bringToFront();
    await control.evaluate((m: string) => { const c = document.getElementById("composer") as HTMLTextAreaElement;c.focus();const n = m === "end" ? c.value.length : c.value.length >> 1;c.setSelectionRange(n, n);(window as any).__bdPaint = []; }, at);
    await control.keyboard.type("the quick brown fox ", { delay: 40 });
    await sleep(300);
    const xs: number[] = await control.evaluate(() => (window as any).__bdPaint);
    await page.bringToFront();
    return p95(xs);
  };
  const typed = async (at: "end" | "mid") => {
    const bare = await floor(at);
    await page.evaluate((m: string) => { const c = document.getElementById("composer") as HTMLTextAreaElement;c.focus();const n = m === "end" ? c.value.length : c.value.length >> 1;c.setSelectionRange(n, n); }, at);
    await paints();
    const cdp = process.env.BIG_DRAFT_TYPEPROF ? await context.newCDPSession(page) : null;
    if (cdp) { await cdp.send("Profiler.enable");await cdp.send("Profiler.setSamplingInterval", { interval: 100 });await cdp.send("Profiler.start"); }
    await page.keyboard.type("the quick brown fox ", { delay: 40 });
    if (cdp) {
      // inclusive time by function: each function once per sample's stack
      const { profile } = await cdp.send("Profiler.stop");
      const byId = new Map(profile.nodes.map((n: any) => [n.id, n])), parent = new Map<number, number>();
      for (const n of profile.nodes) for (const c of n.children || []) parent.set(c, n.id);
      const incl = new Map<string, number>(), dt = profile.timeDeltas as number[];
      profile.samples.forEach((id: number, i: number) => {
        const seen = new Set<string>();
        for (let x: any = id; x !== undefined; x = parent.get(x)) { const f = (byId.get(x) as any).callFrame;const k = `${f.functionName || "(anon)"}:${f.lineNumber + 1}`;if (!seen.has(k)) { seen.add(k);incl.set(k, (incl.get(k) || 0) + (dt[i] || 0)); } }
      });
      console.log(`typing at the ${at}, inclusive ms:`);
      console.log([...incl].filter(([k]) => !/^\((root|idle|program)\)/.test(k)).sort((a, b) => b[1] - a[1]).slice(0, 40).map(([k, us]) => `  ${(us / 1000).toFixed(1)} ${k}`).join("\n"));
    }
    await sleep(300);
    const xs: number[] = await paints();
    check(xs.length >= 20 && p95(xs) <= Math.max(16.7, bare + 4), `typing at the ${at}: key → paint p95 ${Math.round(p95(xs))} ms (${xs.length} keys; a bare textarea ${Math.round(bare)} ms)`);
  };
  await typed("end");
  await typed("mid");
  await control.close();
  // the draft as typed, for the reload and Enter checks below
  const whole = await page.locator("#composer").inputValue();
  check(whole.length === big.length + 40, "the typed keys are in the draft");
  await sleep(1500);

  // reload: the draft comes back whole, the limit is said again
  const closed = wire.closed;
  if (process.env.BIG_DRAFT_DEBUG) console.log("stored", await page.evaluate(() => Object.keys(localStorage).filter((k) => k.includes("draft")).map((k) => k + ":" + (localStorage.getItem(k) || "").length)));
  await page.reload();
  await page.waitForSelector(".sidebar .thread", { timeout: 60000 });
  await ready();
  if (process.env.BIG_DRAFT_DEBUG) console.log("stored after", await page.evaluate(() => Object.keys(localStorage).filter((k) => k.includes("draft")).map((k) => k + ":" + (localStorage.getItem(k) || "").length)), await page.evaluate(() => (document.getElementById("composer") as any)?.value?.length));
  const back = await until(15000, async () => { const v = await page.locator("#composer").inputValue(); return v === whole ? v : undefined; });
  check(back === whole, `after a reload the draft is intact (${back?.length ?? 0} of ${whole.length} chars)`);
  check(!!(await until(15000, async () => (await page.locator(".draft-limit").count()) || undefined)), "and the limit is said again");
  await sleep(1000);
  check(wire.closed === closed, "the new socket stays up");

  // send: nothing is posted, the draft stays
  const posted = () => changes.filter((c) => c.$ === "MessagePosted" && c.thread === th).length;
  const before = posted();
  await page.locator("#composer").press("Enter");
  await sleep(2500);
  check(posted() === before, "Enter posts nothing over a message's bound");
  check((await page.locator("#composer").inputValue()) === whole, "the draft stays after Enter");
  check(wire.closed === closed, "the socket is still up");
  ws.close();
} catch (e) {
  console.log(`FAIL ${(e as Error).message}`);
  fail++;
  if (page && process.env.BIG_DRAFT_SHOT) {
    await page.screenshot({ path: process.env.BIG_DRAFT_SHOT }).catch(() => {});
    console.log((await page.content().catch(() => "")).slice(0, 3000));
  }
} finally {
  await browser?.close();
  proc.kill();
}
console.log(fail ? `big draft: ${fail} failed` : "big draft: all passed");
process.exit(fail ? 1 : 0);
