// Reading while a thread grows, the web client end to end (client.bend's
// Hold, host.js's held): a headless hub with the stand-in `claude` in mode
// stream, lines a delta (test/tools/claude_standin.ts), and the page in
// headless Chromium.
//   mid-thread   scrolled up with the wheel, a reply of 200 lines streams
//                in below and lands: the first message on screen stays
//                exactly where it was (sampled through the stream), and
//                the "N new" pill shows
//   pill         a click on it goes to the end
//   end          at the end, the next reply is followed to its last line
//
//   bun test/tools/hold_e2e.ts --bin BACKPLANE [--web DIR] [--port 3801] [--pw PLAYWRIGHT-CORE] [--wire DIR]
// The binary runs with no display on a fresh temp home (--no-tailscale);
// only the hub started here is stopped.
import { chmodSync, existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { homedir, tmpdir } from "node:os";
import { join, resolve } from "node:path";

const argv = process.argv.slice(2);
const opt = (name: string, d: string) => {
  const i = argv.indexOf(`--${name}`);
  return i >= 0 ? argv[i + 1] : d;
};
const here = resolve(import.meta.dir, "../..");
const bin = resolve(opt("bin", "build/backplane"));
const port = Number(opt("port", "3801"));
if (port === 3787) throw new Error("3787 is the live hub's port");
const web = opt("web", "");
const wire = opt("wire", "build/wire");
const out = resolve(opt("out", join(tmpdir(), `bp-hold-web-${Date.now()}`)));
mkdirSync(out, { recursive: true });
const pwPath = opt("pw", [join(here, "tools/browser/node_modules/playwright-core"), join(homedir(), "Documents/i2cjak/backplane-bend/tools/browser/node_modules/playwright-core")].find((p) => existsSync(p)) ?? "playwright-core");
const { chromium } = await import(pwPath);
for (const f of readdirSync(wire).filter((f) => f.endsWith(".js"))) (0, eval)(readFileSync(`${wire}/${f}`, "utf8"));
const W = (globalThis as any).Wire;
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
let fail = 0;
const check = (ok: boolean, what: string) => {
  console.log(`${ok ? "ok  " : "FAIL"} ${what}`);
  if (!ok) fail++;
};

const root = mkdtempSync(join(tmpdir(), "bp-hold-"));
const fake = join(root, "bin");
mkdirSync(fake);
writeFileSync(join(fake, "claude"), `#!/bin/sh\nexec ${process.execPath} ${join(here, "test/tools/claude_standin.ts")} "$@"\n`);
chmodSync(join(fake, "claude"), 0o755);
for (const n of ["codex", "grok", "muse", "agy"]) {
  writeFileSync(join(fake, n), "#!/bin/sh\nexit 1\n");
  chmodSync(join(fake, n), 0o755);
}
const home = join(root, "home");
const proj = join(root, "proj");
mkdirSync(proj);
mkdirSync(home);
writeFileSync(join(home, "events.jsonl"), "");
const args = [bin, "--home", home, "--port", String(port), "--no-tailscale", "--foreground"];
if (web) args.push("--web", resolve(web));
const proc = Bun.spawn(args, {
  env: { ...process.env, DISPLAY: "", WAYLAND_DISPLAY: "", BACKPLANE_NO_UPDATE: "1", BACKPLANE_ORDER_SERVER: "http://127.0.0.1:9", BACKPLANE_PEERS: "", BACKPLANE_GOOGLE_BAKE: "0",
    BP_STANDIN_MODE: "stream", BP_STREAM_N: "200", BP_STREAM_MS: "15", BP_STREAM_SEP: "\n\n", BACKPLANE_AGENTS: fake, PATH: `${fake}:/usr/bin:/bin` },
  stdout: "ignore",
  stderr: "ignore",
});
const until = async <T>(ms: number, f: () => T | undefined | Promise<T | undefined>) => {
  const end = Date.now() + ms;
  for (;;) {
    const v = await f();
    if (v) return v;
    if (Date.now() > end) return undefined;
    await sleep(40);
  }
};
let browser: any = null;
let page: any = null;
let context: any = null;
try {
  await until(20000, async () => { try { return (await fetch(`http://127.0.0.1:${port}/hello`)).status === 200; } catch { return undefined; } });
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
  send("setting.set", { key: "restart.continue", value: "off" });
  // no first-run tour over the page
  send("setting.set", { key: "onboard.done", value: "1" });
  send("thread.create", { project: pc.id, title: "hold", env: "local", provider: "claude" });
  const th = (await until(10000, () => changes.find((c) => c.$ === "ThreadCreated")))?.id as string;
  if (!th) throw new Error("no thread");
  const replies = () => changes.filter((c) => c.$ === "MessagePosted" && c.thread === th && String(c.role).toLowerCase() === "assistant").length;
  const turn = async (word: string, n: number) => {
    send("turn.start", { thread: th, text: `go ${word}`, msg: `h-${word}` });
    return until(60000, () => replies() >= n ? true : undefined);
  };
  // history to read: three replies of 200 lines
  for (const [i, w] of ["a", "b", "c"].entries()) {
    if (!(await turn(w, i + 1))) throw new Error(`history ${w} did not land`);
  }
  // A synthetic provider work record: it runs no real agent.
  send("setting.set", { key: `bg.${th}`, value: "a\tWorking\tc\tInspect anchoring\tsubagent" });

  const exe = [join(homedir(), ".cache/ms-playwright/chromium_headless_shell-1234/chrome-headless-shell-linux64/chrome-headless-shell"),
    join(homedir(), ".cache/ms-playwright/chromium_headless_shell-1217/chrome-headless-shell-linux64/chrome-headless-shell")].find((p) => existsSync(p));
  browser = await chromium.launch({ executablePath: exe, headless: true, args: ["--disable-gpu"] });
  context = await browser.newContext({ viewport: { width: 1200, height: 800 }, serviceWorkers: "block" });
  await context.tracing.start({ screenshots: true, snapshots: true, sources: true });
  page = await context.newPage();
  await page.addInitScript(() => {
    (window as any).__holdEvents = [];
    for (const type of ["scroll", "wheel", "pointerdown", "click"]) document.addEventListener(type, (e: Event) => {
      const tl = document.getElementById("timeline");
      if (!tl) return;
      const target = e.target as HTMLElement;
      (window as any).__holdEvents.push({ at: performance.now(), type, target: target?.className, top: tl.scrollTop, height: tl.scrollHeight, view: tl.clientHeight });
    }, true);
  });
  await page.goto(`http://127.0.0.1:${port}/`);
  await page.waitForSelector(".sidebar .thread", { timeout: 60000 });
  await page.evaluate((id: string) => {
    const b = document.createElement("button");
    b.setAttribute("data-on-click", "select");
    b.setAttribute("data-value", id);
    document.body.appendChild(b);
    b.click();
    b.remove();
  }, th);
  await page.waitForSelector("#timeline [data-id]", { timeout: 20000 });
  await page.waitForSelector(".subs-head", { timeout: 10000 });
  // the first-run tour, skipped
  await sleep(500);
  const skip = await page.$('[data-on-click="ob-skip"]');
  if (skip) { await skip.click(); await sleep(500); }
  await sleep(800);
  const gap = () => page.evaluate(() => { const t = document.getElementById("timeline")!; return t.scrollHeight - t.scrollTop - t.clientHeight; });
  check((await gap()) <= 4, `opens at the end (gap ${(await gap()).toFixed(1)})`);

  // scroll up with the wheel, over the timeline, to mid-thread
  const box = await page.evaluate(() => { const r = document.getElementById("timeline")!.getBoundingClientRect(); return { x: r.left + r.width / 2, y: r.top + r.height / 2 }; });
  await page.mouse.move(box.x, box.y);
  for (let i = 0; i < 12; i++) {
    await page.mouse.wheel(0, -400);
    await sleep(60);
    if (process.env.HOLD_DEBUG) console.log("wheel", i, await gap(), await page.evaluate(() => { const t = document.getElementById("timeline")!; return [t.scrollTop, t.scrollHeight, t.clientHeight, getComputedStyle(t).overflowY, document.elementFromPoint(600, 400)?.className]; }));
  }
  await sleep(500);
  const at = () => page.evaluate(() => {
    const tl = document.getElementById("timeline")!;
    const top = tl.getBoundingClientRect().top;
    for (const el of tl.querySelectorAll("[data-id]")) {
      const r = el.getBoundingClientRect();
      if (r.bottom > top + 1) return { id: el.getAttribute("data-id")!, y: r.top };
    }
    return null;
  });
  const where = (id: string) => page.evaluate((id: string) => {
    const el = document.querySelector(`#timeline [data-id="${CSS.escape(id)}"]`);
    return el ? el.getBoundingClientRect().top : null;
  }, id);
  let a0 = await at();
  check(!!a0 && (await gap()) > 200, `scrolled up mid-thread (gap ${(await gap()).toFixed(0)})`);
  if (!a0) throw new Error("nothing on screen");

  const detail = () => page.evaluate(() => {
    const tl = document.getElementById("timeline")!;
    const box = (el: Element | null) => { const r = el?.getBoundingClientRect(); return r ? [r.x, r.y, r.width, r.height] : null; };
    return { tl: box(tl), scroll: [tl.scrollTop, tl.scrollHeight, tl.clientHeight], head: box(tl.querySelector(".subs-head")), work: box(tl.querySelector(".work")), entries: [...tl.querySelectorAll("[data-id]")].map(el => [el.getAttribute("data-id"), ...box(el)!]) };
  });
  const capture = async (name: string) => {
    console.log(name, JSON.stringify(await detail()));
    await page.screenshot({ path: join(out, `${name}.png`) });
  };
  await capture("away");
  // A real pointer click avoids Playwright scrolling its target into view
  // before the click, which itself changes the position under test.
  const panelClick = async () => {
    const r = await page.locator(".subs-head").boundingBox();
    if (!r || r.y < 0 || r.y + r.height > 800) throw new Error(`subagents header not visible: ${JSON.stringify(r)}`);
    await page.mouse.click(r.x + r.width / 2, r.y + r.height / 2);
  };
  // Chromium quantizes scrollTop to device pixels; a .5px font/card
  // height can leave half a pixel of unavoidable rounding at DPR 1.
  // Keep the original anchor through every mutation: this bound must
  // never accumulate across repeated corrections.
  const sameY = async (a: {id: string; y: number}) => Math.abs((await where(a.id) ?? Infinity) - a.y) <= 0.5;
  await panelClick();
  await sleep(200);
  await capture("subagents-shut");
  check(await sameY(a0), `collapsing subagents keeps the entry offset (${await where(a0.id)} vs ${a0.y})`);
  // A collapsed panel intentionally stops sticking and scrolls away.
  // Its existing focused button can be toggled again with Enter without
  // scrolling the timeline back to the top to find it.
  check(await page.evaluate(() => document.activeElement?.classList.contains("subs-head")), "the subagents toggle keeps keyboard focus");
  await page.keyboard.press("Enter");
  await sleep(200);
  await capture("subagents-open");
  check(await sameY(a0), `expanding subagents keeps the entry offset (${await where(a0.id)} vs ${a0.y})`);
  let panelDrift = 0;
  for (let i = 0; i < 40; i++) {
    await page.evaluate(() => (document.querySelector(".subs-head") as HTMLElement).click());
    await page.evaluate(() => new Promise<void>(r => requestAnimationFrame(() => requestAnimationFrame(() => r()))));
    panelDrift = Math.max(panelDrift, Math.abs((await where(a0!.id) ?? Infinity) - a0!.y));
  }
  check(panelDrift <= 0.5, `40 panel layouts do not accumulate rounding drift (${panelDrift} px)`);
  // Keep later cases independent if a preceding assertion failed.
  a0 = await at();
  if (!a0) throw new Error("no reading anchor after the panel test");

  // A late image above the first visible entry changes an already drawn
  // entry's height. The host's ResizeObserver must correct before paint.
  let releaseImage!: () => void;
  const imageGate = new Promise<void>((r) => { releaseImage = r; });
  await page.route("**/hold-image.svg", async (route: any) => {
    await imageGate;
    await route.fulfill({ contentType: "image/svg+xml", body: '<svg xmlns="http://www.w3.org/2000/svg" width="200" height="400"><rect width="200" height="400" fill="#c84"/></svg>' });
  });
  const imagePlaced = await page.evaluate(() => {
    const tl = document.getElementById("timeline")!;
    const top = tl.getBoundingClientRect().top;
    const before = [...tl.querySelectorAll("[data-id]")].find((e) => e.getBoundingClientRect().bottom < top);
    if (!before) return false;
    const img = document.createElement("img");
    img.id = "hold-image";
    img.style.display = "block";
    img.src = "/hold-image.svg";
    before.appendChild(img);
    return true;
  });
  check(imagePlaced, "a delayed image exists above the view");
  releaseImage();
  if (imagePlaced) await page.waitForFunction(() => (document.getElementById("hold-image") as HTMLImageElement)?.naturalHeight === 400, null, { timeout: 5000 });
  await sleep(150);
  await capture("image-loaded");
  check(await sameY(a0), `a loaded image above keeps the entry offset (${await where(a0.id)} vs ${a0.y})`);
  // Near the bottom, removing the image above also makes the browser
  // clamp scrollTop. That layout scroll must not turn holding into follow.
  await page.mouse.move(box.x, box.y);
  await page.mouse.wheel(0, (await gap()) - 100);
  await sleep(200);
  const near = await at();
  const nearGap = await gap();
  check(!!near && nearGap > 4 && nearGap < 400, `away near the end before image shrink (gap ${nearGap})`);
  await page.evaluate(() => document.getElementById("hold-image")?.remove());
  await sleep(150);
  check(!!near && await sameY(near), "image shrink and browser scroll clamping keep the entry offset");
  check(Math.abs((await gap()) - nearGap) <= 1, `clamping does not start following (gap ${await gap()} vs ${nearGap})`);
  await capture("image-shrunk");
  await page.mouse.wheel(0, -4800);
  await sleep(250);
  a0 = await at();
  if (!a0) throw new Error("no reading anchor after image shrink");

  // a reply of 200 lines streams in below and lands
  send("turn.start", { thread: th, text: "go d", msg: "h-d" });
  let worst = 0;
  let samples = 0;
  let pillSeen = "";
  const end = Date.now() + 30000;
  while (Date.now() < end) {
    const y = await where(a0.id);
    if (y !== null) { worst = Math.max(worst, Math.abs(y - a0.y)); samples++; }
    if (process.env.HOLD_DEBUG && y !== a0.y) console.log("moved", samples, y, a0.y, await gap(), replies());
    if (!pillSeen) pillSeen = await page.evaluate(() => document.querySelector(".hold-pill")?.textContent ?? "");
    if (replies() >= 4) break;
    await sleep(50);
  }
  await sleep(600);
  const yEnd = await where(a0.id);
  if (yEnd !== null) worst = Math.max(worst, Math.abs(yEnd - a0.y));
  check(replies() >= 4, "the streamed reply landed");
  check(samples > 20 && worst <= 0.5, `the first message on screen stays put through 200 streamed lines (${samples} samples, worst ${worst} px)`);
  const pill = await page.evaluate(() => document.querySelector(".hold-pill")?.textContent ?? "");
  check(pillSeen.length > 0, `a pill while it streamed ("${pillSeen}")`);
  check(/^[0-9]+ new ↓$/.test(pill), `the pill counts what came ("${pill}")`);

  // the pill goes to the end
  await page.click(".hold-pill");
  await sleep(400);
  check((await gap()) <= 4, `the pill goes to the end (gap ${(await gap()).toFixed(1)})`);
  check(!(await page.evaluate(() => !!document.querySelector(".hold-pill"))), "and goes away");

  // at the end, the next reply is followed
  send("turn.start", { thread: th, text: "go e", msg: "h-e" });
  let far = 0;
  let n2 = 0;
  const end2 = Date.now() + 30000;
  while (Date.now() < end2) {
    await sleep(80);
    far = Math.max(far, await gap());
    n2++;
    if (replies() >= 5) break;
  }
  await sleep(400);
  const g2 = await gap();
  check(replies() >= 5, "the followed reply landed");
  check(g2 <= 4, `at the end it follows to the last line (gap ${g2.toFixed(1)}, worst while streaming ${far.toFixed(0)} over ${n2} samples)`);
  ws.close();
} finally {
  if (page) {
    try { await page.screenshot({ path: join(out, "final.png") }); writeFileSync(join(out, "events.json"), JSON.stringify(await page.evaluate(() => (window as any).__holdEvents), null, 2)); } catch {}
  }
  if (context) await context.tracing.stop({ path: join(out, "trace.zip") });
  console.log(`artifacts: ${out}`);
  if (browser) await browser.close();
  proc.kill();
  await proc.exited;
  rmSync(root, { recursive: true, force: true });
}
console.log(fail ? `${fail} failed` : "all passed");
process.exit(fail ? 1 : 0);
