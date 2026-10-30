// A hub-drawn page whose fixed chrome alone is over the view budget stays
// connected while the person opens a long thread, switches threads and
// sends. server.bend's Hub.thin.web_fit once rendered such a view eleven
// times in one hub step (each "shrink" bottomed out, as a sidebar that never
// pages cannot shrink); on a real catalog that was 5-6 s per update, past the
// page's 5 s view deadline, so every message ended in "Conversation
// unavailable" and a reconnect. The fit now takes at most three renders,
// recorded per view as /debug/perf's "thin.fit.renders".
//
// Starts a headless hub on a temporary home holding many projects (each a
// live thread, so the sidebar alone passes the budget) and one long thread,
// opens the real web page in headless Chromium, opens the thread, switches
// to another thread and back and sends twice. Prints "ok ..."/"FAIL ...".
//
//   bun test/tools/thin_fit_e2e.ts [BINARY] [WEB DIR]
import { existsSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { homedir, tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";

const here = resolve(import.meta.dir, "../..");
const bin = resolve(process.argv[2] ?? join(here, "build/backplane-serve"));
const web = resolve(process.argv[3] ?? join(dirname(bin), "web"));
const PORT = Number(process.env.E2E_PORT ?? 3933);
if (PORT === 3787) throw new Error("3787 is the live hub's port");
// enough live projects that the sidebar alone passes the fit budget
// (224 KB), few enough that the floor stays under the transport cap (256 KB)
const PROJECTS = Number(process.env.E2E_PROJECTS ?? 36);
const TOKEN = "5f0c2e7d948ab62350cfa19e6d834b10";
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
let fail = 0;
const check = (ok: boolean, what: string) => { console.log(`${ok ? "ok  " : "FAIL"} ${what}`); if (!ok) fail++; };
const pw = [join(here, "tools/browser/node_modules/playwright-core"), join(homedir(), "Documents/i2cjak/backplane-bend/tools/browser/node_modules/playwright-core")].find((p) => existsSync(p)) ?? "playwright-core";
const { chromium } = await import(pw);

const root = mkdtempSync(join(process.env.E2E_TMP ?? tmpdir(), "bp-thinfit-"));
const home = join(root, "home"), agents = join(root, "agents");
for (const d of [home, agents, join(root, "locks"), join(root, "user")]) mkdirSync(d, { recursive: true });
writeFileSync(join(home, "token"), TOKEN, { mode: 0o600 });

// the log: PROJECTS projects with a thread each, and the long thread
const at = Math.floor(Date.now() / 1000) - 7200;
const lines: any[] = [{ $: "SettingSet", key: "restart.continue", value: "off", at }, { $: "SettingSet", key: "onboard.done", value: "1", at }, { $: "SettingSet", key: "onboard.uber", value: "1", at }];
const words = (n: number, seed: number) => Array.from({ length: n }, (_, i) => `word${(i * 7 + seed) % 97}`).join(" ");
for (let p = 0; p < PROJECTS; p++) {
  const dir = join(root, "proj", `p${p}`);mkdirSync(dir, { recursive: true });
  lines.push({ $: "ProjectCreated", id: `p${p}`, title: `Project ${p} ${words(4, p)}`, root: dir, at: at + p });
  lines.push({ $: "ThreadCreated", id: `t${p}`, project: `p${p}`, title: `Thread ${p}: ${words(8, p)}`, env: "local", branch: "", worktree: "", provider: "claude", model: "", at: at + p });
  lines.push({ $: "MessagePosted", thread: `t${p}`, msg: `m${p}`, role: "user", text: words(20, p), at: at + p });
}
const long = "tlong";
lines.push({ $: "ThreadCreated", id: long, project: "p0", title: "The long thread under test", env: "local", branch: "", worktree: "", provider: "claude", model: "", at: at + 1000 });
for (let i = 0; i < 400; i++) {
  lines.push({ $: "MessagePosted", thread: long, msg: `l${i}`, role: i % 2 ? "assistant" : "user", text: `${i}: ${words(120, i)}`, at: at + 1000 + i });
  if (i % 4 === 1) lines.push({ $: "ActivityLogged", thread: long, act: `a${i}`, tone: "tool", kind: "Bash", summary: words(60, i), at: at + 1000 + i });
}
writeFileSync(join(home, "events.jsonl"), lines.map((l) => JSON.stringify(l)).join("\n") + "\n");

const hub = Bun.spawn([bin, "--home", home, "--port", String(PORT), "--host", "127.0.0.1", "--no-tailscale", "--headless", "--foreground", "--web", web], {
  env: { ...process.env, HOME: join(root, "user"), DISPLAY: "", BACKPLANE_AGENTS: agents, BACKPLANE_LOCK_DIR: join(root, "locks"), BACKPLANE_NO_UPDATE: "1",
    BACKPLANE_KISTACK_SYNC: "0", BACKPLANE_ORDER_SERVER: "http://127.0.0.1:9", BACKPLANE_TELEMETRY: "off", BACKPLANE_PEERS: "", BACKPLANE_GOOGLE_BAKE: "0",
    BUN_JSC_maxPerThreadStackUsage: "4000000000" },
  stdout: "ignore", stderr: Bun.file(join(root, "hub.err")) });

let browser: any;
try {
  let up = false;
  for (let i = 0; i < 600 && !up; i++) { try { up = (await fetch(`http://127.0.0.1:${PORT}/hello`)).ok; } catch {} if (!up) await sleep(100); }
  check(up, "hub listening");
  browser = await chromium.launch({ headless: true, ...(process.env.E2E_CHROME ? { executablePath: process.env.E2E_CHROME } : {}) });
  const page = await (await browser.newContext({ viewport: { width: 1500, height: 950 } })).newPage();
  const conn: any[] = [];
  let step = "load";
  page.on("console", (m: any) => { const t = m.text(); if (t.includes("bp-connection")) try { conn.push({ ...JSON.parse(t.slice(t.indexOf("{"))), step }); } catch {} });
  await page.goto(`http://127.0.0.1:${PORT}/#token=${TOKEN}`);
  for (let i = 0; i < 300 && !conn.some((c) => c.event === "ready"); i++) await sleep(100);
  check(conn.some((c) => c.event === "ready"), "the page's view is ready");
  if (await page.locator(".tour-back").count()) { await page.getByRole("button", { name: /^Skip/ }).first().click().catch(() => page.keyboard.press("Escape"));await sleep(500); }
  await page.getByText("Jump or run a command").first().click();await sleep(400);
  await page.keyboard.type("The long thread under test");await sleep(2000);
  await page.getByText("The long thread under test").last().click({ timeout: 15000 }).catch(async (e: any) => {
    if (process.env.E2E_SHOT) await page.screenshot({ path: process.env.E2E_SHOT });
    console.log("conn:", conn.map((c) => `${c.event}:${c.cause ?? ""}`).join(" "));throw e;
  });await sleep(4000);
  // another thread and back: each a full view over the budget
  for (const title of ["Thread 1:", "The long thread under test"]) {
    step = title;await page.getByText("Jump or run a command").first().click();await sleep(400);
    await page.keyboard.type(title);await sleep(2000);
    await page.getByText(title).last().click().catch(() => {});await sleep(3000);
  }
  for (const text of ["first message", "second message"]) {
    step = text;await page.click("#composer");await page.keyboard.type(text);
    await page.getByRole("button", { name: /^(Send|Queue|Give)$/ }).first().click().catch(() => page.keyboard.press("Control+Enter"));
    await sleep(6000);
  }
  const perf = await (await fetch(`http://127.0.0.1:${PORT}/debug/perf`)).text();
  const row = perf.split("\n").find((l) => l.startsWith("thin.fit.renders"))?.trim().split(/\s+/) ?? [];
  // name n total mean p50 p99 max
  const n = Number(row[1] ?? 0), max = Number(row[6] ?? 0);
  const rpcMax = Number(perf.split("\n").find((l) => l.startsWith("hub.rpc "))?.trim().split(/\s+/)[6] ?? 0);
  check(n > 0, `views were fitted (${n})`);
  check(max >= 2, `the sidebar alone passed the budget, so a view was shrunk (most renders for one view: ${max})`);
  check(max <= 3, `no view took more than three renders (${max})`);
  const errors = conn.filter((c) => ["view-error", "closing", "closed"].includes(c.event));
  check(errors.length === 0, `the page stayed connected with its view (${errors.map((c) => `${c.event}:${c.cause} at ${c.step}`).join(", ") || "no errors"}; slowest hub.rpc ${rpcMax} ms)`);
} finally {
  await browser?.close().catch(() => {});
  hub.kill("SIGTERM");
  await hub.exited;
  if (!process.env.E2E_KEEP) rmSync(root, { recursive: true, force: true });
}
if (fail) { console.log(`${fail} failed`);process.exit(1); }
console.log("all ok");
