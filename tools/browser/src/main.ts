// backplane-browser: a headless Chromium the Bend side drives over
// newline-delimited JSON on stdin/stdout. Pages are keyed by an owner
// string (a bot id; "" is the shared page every agent drives). Raw RGB
// frames of the shared page go to a file, JPEG frames of every page to a
// directory (see README.md). Only protocol lines are written to stdout;
// logs go to stderr.

import { copyFileSync, existsSync, mkdirSync, renameSync, rmSync, statSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { basename, extname, join, resolve } from "node:path";
import { createInterface } from "node:readline";
import { chromium } from "./playwright.ts";
import type { BrowserContext, CDPSession, Locator, Page } from "playwright-core";
import { CefEngine, findCef, type CefOptions } from "./cef.ts";

import { findChromium, installChrome, INSTALL_HINT } from "./chromium.ts";
import { decodePng } from "./png.ts";
import { RECORDER_SCRIPT } from "./recorder.ts";

type Json = Record<string, any>;

const VERSION = "3";
const MAX_VISIBLE_TEXT = 20_000;
const MAX_TEXT = 200_000;
const MAX_ELEMENTS = 200;
const MAX_EVAL_BYTES = 64_000;
const MAX_LOG = 100;
const DEFAULT_TIMEOUT = 15_000;
const DEFAULT_VIEWPORT = { width: 1280, height: 800 };

// Arguments
// ---------

function argValue(name: string): string | null {
  const i = process.argv.indexOf(name);
  return i >= 0 && i + 1 < process.argv.length ? process.argv[i + 1] : null;
}

if (process.argv.includes("--help") || process.argv.includes("-h")) {
  process.stderr.write(
    "usage: backplane-browser [--frames FILE] [--profile DIR] [--jpeg-dir DIR] [--jpeg-fps N] [--chromium PATH] [--headed] [--engine chrome|cef] [--cef PATH] [--parent XID] [--display DISPLAY] [--cache DIR]\n" +
      "  newline-delimited JSON requests on stdin, replies and events on stdout.\n" +
      "  raw RGB frames of the shared page are written to FILE (atomically, via rename).\n" +
      "  --profile DIR keeps one Chrome profile (logins, cookies) across pages and runs.\n" +
      "  --jpeg-dir DIR writes every page's frames to DIR/<owner>.jpg.\n" +
      "  --jpeg-fps N (4) frames a second at most; --cast-fps N (30) while a client shows the page (op cast);\n" +
      "  a page no client shows is not cast: a still every --idle-still-ms N (5000).\n",
  );
  process.exit(0);
}

const FRAMES_ARG = argValue("--frames");
const FRAMES_PATH = FRAMES_ARG ?? join(tmpdir(), `backplane-browser-${process.pid}.rgb`);
const PROFILE = argValue("--profile");
const JPEG_DIR = argValue("--jpeg-dir");
const JPEG_FPS = Math.max(1, Math.min(30, Number(argValue("--jpeg-fps") ?? 4) || 4));
// while some client shows a page (the cast op leases it), its frames go out
// at up to CAST_FPS a second instead of JPEG_FPS
const CAST_FPS = Math.max(1, Math.min(60, Number(argValue("--cast-fps") ?? 30) || 30));
// a page no client shows is not cast at all (no screencast: Chrome encodes
// nothing for it); a still is taken every IDLE_STILL_MS instead, and once
// after each request for the page, for whoever opens it next
const IDLE_STILL_MS = Math.max(1000, Number(argValue("--idle-still-ms") ?? 5000) || 5000);
const CHROMIUM_ARG = argValue("--chromium");
const HEADED = process.argv.includes("--headed");
const DEBUG = Boolean(process.env.BACKPLANE_BROWSER_DEBUG);

if (PROFILE) mkdirSync(PROFILE, { recursive: true });
if (JPEG_DIR) mkdirSync(JPEG_DIR, { recursive: true });

// Output
// ------

function send(obj: Json): void {
  process.stdout.write(JSON.stringify(obj) + "\n");
}

function log(msg: string): void {
  process.stderr.write(`backplane-browser: ${msg}\n`);
}

// Owners
// ------

// an owner's JPEG frame file name: [A-Za-z0-9_-] only, "" -> "shared"
export function safeOwner(name: string): string {
  return name === "" ? "shared" : name.replace(/[^A-Za-z0-9_-]/g, "_");
}

function shotPath(name: string): string | null {
  return JPEG_DIR ? join(JPEG_DIR, safeOwner(name) + ".jpg") : null;
}

type Shot = {
  cdp: CDPSession | null;
  seq: number;
  last: number;
  // the cast lease: until then frames go out at CAST_FPS (performance.now ms)
  fast: number;
  // when the last frame every client gets went out
  lastAll: number;
  lastHash: bigint;
  pending: Buffer | null;
  timer: ReturnType<typeof setTimeout> | null;
  // the screencast runs (only while some client shows the page)
  casting: boolean;
  capture: (() => Promise<void>) | null;
  castTask: Promise<void> | null;
  // when the last idle still was taken (performance.now ms)
  still: number;
  // fires when the cast lease runs out
  lease: ReturnType<typeof setTimeout> | null;
};

// an owner's tabs (its page and the popups it opened), the one it drives,
// its viewport, console log and JPEG frames
type Owner = {
  name: string;
  tabs: Page[];
  active: Page | null;
  viewport: { width: number; height: number };
  console: Json[];
  shot: Shot;
  opening: Promise<Page> | null;
  // the in-page recorder is on for this owner (record op)
  recording: boolean;
  lastNav: string;
};

const owners = new Map<string, Owner>();
const ownerOf = new WeakMap<Page, Owner>();

function ownerFor(name: string): Owner {
  let o = owners.get(name);
  if (!o) {
    o = {
      name,
      tabs: [],
      active: null,
      viewport: { ...DEFAULT_VIEWPORT },
      console: [],
      shot: { cdp: null, seq: 0, last: 0, fast: 0, lastAll: 0, lastHash: 0n, pending: null, timer: null, casting: false, capture: null, castTask: null, still: 0, lease: null },
      opening: null,
      recording: false,
      lastNav: "",
    };
    owners.set(name, o);
  }
  return o;
}

function pushLog(o: Owner, entry: Json): void {
  o.console.push({ ...entry, timestamp: new Date().toISOString() });
  if (o.console.length > MAX_LOG) o.console.shift();
}

// State
// -----

let engine = argValue("--engine") ?? "chrome";
if (engine !== "chrome" && engine !== "cef") throw new Error("engine must be chrome or cef");
let cefOptions: CefOptions = {
  parent: Number(argValue("--parent") ?? 0), display: argValue("--display") ?? process.env.DISPLAY ?? "",
  cef: argValue("--cef"), cache: argValue("--cache") ?? resolve(PROFILE ?? join(tmpdir(), `bp-browser-${process.pid}`, "profile"), "..", "cef-profile"),
};
let cef: CefEngine | null = null;
// The native rectangle belongs to an owner, and follows that owner's active tab.
let cefPlacement: { owner: string; x: number; y: number; w: number; h: number } | null = null;
let cefShield = false;
let tabNumber = 0;
const cefOwners = new Map<string, Owner>();
let context: BrowserContext | null = null;
let launching: Promise<BrowserContext> | null = null;
let executable: string | null = null;
// blank pages Chrome opened at launch, handed to the first owners
let spare: Page[] = [];

// raw RGB frames of the shared page
const frames = {
  on: false,
  maxFps: 15,
  seq: 0,
  lastHash: 0n as bigint,
  pending: null as Buffer | null,
  timer: null as ReturnType<typeof setInterval> | null,
  cdp: null as CDPSession | null,
  page: null as Page | null,
  capture: null as (() => Promise<void>) | null,
  busy: false,
};

// URLs: absolute http(s) passes; a schemeless host gets https, a loopback
// host http; about:, data:, file: pass as they are.
function normalizeUrl(raw: string): string {
  const url = String(raw).trim();
  if (/^[a-z][a-z0-9+.-]*:/i.test(url) && !/^[^/]+:\d+(\/|$)/.test(url)) return url;
  const host = url.split(/[/:?#]/)[0].toLowerCase();
  const loopback = host === "localhost" || host === "127.0.0.1" || host === "[::1]" || host === "0.0.0.0" || host.endsWith(".localhost");
  return (loopback ? "http://" : "https://") + url;
}

// Browser lifecycle
// -----------------

// one persistent context (one profile directory) for every page, so a
// login made on one page holds on all of them and across restarts
async function ensureContext(): Promise<BrowserContext> {
  if (context) return context;
  if (!launching) launching = launch().finally(() => { launching = null; });
  return launching;
}

async function launch(): Promise<BrowserContext> {
  if (engine === "cef") return launchCef();
  executable = CHROMIUM_ARG ?? findChromium();
  if (!executable) {
    try {
      executable = await installChrome((m) => log(m));
    } catch (e) {
      throw new Error(`${INSTALL_HINT} (${e instanceof Error ? e.message : String(e)})`);
    }
  }
  const ctx = await chromium.launchPersistentContext(PROFILE ?? "", {
    executablePath: executable,
    headless: !HEADED,
    viewport: DEFAULT_VIEWPORT,
    deviceScaleFactor: 1,
    acceptDownloads: true,
    args: ["--hide-scrollbars", "--mute-audio", "--no-first-run", "--no-default-browser-check"],
  });
  // the recorder: one binding and one init script on the context, so every
  // tab and popup has them; the page stays inert until its owner records
  await ctx.exposeBinding("__bpRec", (source, ev) => onRecord(source, ev));
  await ctx.addInitScript(RECORDER_SCRIPT);
  ctx.on("close", () => {
    if (context !== ctx) return;
    context = null;
    spare = [];
    frames.cdp = null;
    frames.page = null;
    const release = frames.capture; frames.capture = null;
    if (release) void release();
    for (const o of owners.values()) {
      o.tabs = [];
      o.active = null;
      o.recording = false;
      clearShot(o);
    }
    send({ event: "closed" });
  });
  spare = ctx.pages().filter((p) => !p.isClosed());
  context = ctx;
  return ctx;
}

// CEF creates native children; CDP only adopts them.
async function launchCef(): Promise<BrowserContext> {
  if (!Number.isSafeInteger(cefOptions.parent) || cefOptions.parent <= 0 || !cefOptions.display)
    throw new Error("cef needs parent (X window id) and display");
  if (PROFILE && resolve(cefOptions.cache) === resolve(PROFILE))
    throw new Error("CEF cache must differ from the Chrome profile");
  const host = new CefEngine(cefOptions);
  cef = host;
  host.on("host", (e) => {
    if (e.event === "closed") cefOwners.delete(e.key);
    if (e.event === "popup") {
      const o = cefOwners.get(e.opener) ?? cefOwners.get(String(e.opener).split("~")[0]);
      if (o) cefOwners.set(e.key, o);
    }
    if (e.event === "shield-click" || e.event === "focus") {
      const o = cefOwners.get(e.key) ?? cefOwners.get(String(e.key).split("~")[0]);
      // shield-click x, y: device pixels of the shown page, as the host reports them
      if (o) send({ event: e.event, page: o.name, ...(e.event === "shield-click" ? { x: e.x, y: e.y } : {}) });
    }
  });
  host.on("exit", (code) => {
    if (cef !== host) return;
    cef = null;
    context = null;
    spare = [];
    cefOwners.clear();
    frames.cdp = null; frames.page = null;
    const release = frames.capture; frames.capture = null;
    if (release) void release();
    for (const o of owners.values()) {
      o.tabs = []; o.active = null; o.recording = false; clearShot(o);
    }
    send({ event: "closed" });
    if (!host.stopping) send({ event: "engine", engine: "cef", ok: false, error: `CEF host exited (${code})` });
  });
  try {
    const ctx = await host.start();
    await ctx.exposeBinding("__bpRec", (source, ev) => onRecord(source, ev));
    await ctx.addInitScript(RECORDER_SCRIPT);
    executable = host.executable;
    context = ctx;
    send({ event: "engine", engine: "cef", ok: true });
    return ctx;
  } catch (e) {
    await host.stop();
    if (cef === host) cef = null;
    send({ event: "engine", engine: "cef", ok: false, error: String(e instanceof Error ? e.message : e) });
    throw e;
  }
}
// CEF pages: the host places and sizes X windows in device pixels; agents,
// status and the viewport speak CSS pixels (the same on a scale-1 screen)
async function dpr(p: Page): Promise<number> {
  return (await p.evaluate(() => devicePixelRatio).catch(() => 1)) || 1;
}
// the page's CSS size once the X resize has reached the renderer (both are
// asynchronous); a pixel of rounding either way counts
async function sized(p: Page, width: number, height: number): Promise<void> {
  await p.waitForFunction(({ width, height }) => Math.abs(innerWidth - width) <= 1 && Math.abs(innerHeight - height) <= 1, { width, height }, { timeout: DEFAULT_TIMEOUT });
}
async function sizePage(o: Owner, p: Page): Promise<void> {
  if (engine === "cef") {
    const r = await dpr(p);
    await cef!.command("size", await cef!.key(p), `${Math.round(o.viewport.width * r)} ${Math.round(o.viewport.height * r)}`);
    await sized(p, o.viewport.width, o.viewport.height);
  } else await p.setViewportSize(o.viewport);
}
async function closeTab(p: Page): Promise<void> {
  if (engine === "cef") {
    if (!p.isClosed()) {
      const closed = p.waitForEvent("close");
      await cef!.close(p); await closed;
    }
  } else await p.close();
}
async function stopEngine(): Promise<void> {
  for (const o of owners.values()) { o.recording = false; await stopShots(o); clearShot(o); }
  await stopScreencast();
  const ctx = context;
  context = null;
  if (cef) await cef.stop();
  else await ctx?.close();
  spare = [];
  for (const o of owners.values()) { o.tabs = []; o.active = null; }
}
async function switchEngine(a: Json, name: string): Promise<Json> {
  if (a.engine !== "chrome" && a.engine !== "cef") throw new Error("engine must be chrome or cef");
  const next: CefOptions = { ...cefOptions };
  if (a.engine === "cef") {
    next.parent = a.parent ?? next.parent; next.display = a.display ?? next.display;
    next.cef = a.cef ?? next.cef; next.cache = a.cache ?? next.cache;
    if (!Number.isSafeInteger(next.parent) || next.parent <= 0 || typeof next.display !== "string" || !next.display)
      throw new Error("cef needs parent (X window id) and display");
    if (PROFILE && resolve(next.cache) === resolve(PROFILE)) throw new Error("CEF cache must differ from the Chrome profile");
    // Resolve before shutting down the working engine.
    if (!findCef(next.cef)) {
      const { installCef } = await import("./cef.ts"); next.cef = await installCef();
    }
  }
  if (a.engine === engine && context && (engine === "chrome" || JSON.stringify(next) === JSON.stringify(cefOptions))) return status(name);
  const urls = [...owners.values()].filter((o) => o.active && !o.active.isClosed()).map((o) => ({ name: o.name, url: o.active!.url() }));
  const oldContext = context, oldCef = cef;
  cefPlacement = null; cefShield = false;
  // CEF's singleton profile lock prevents overlapping hosts using one cache.
  // Engine changes between Chrome and CEF can overlap as specified; reconfiguring
  // a live CEF host with the same cache must release that lock first.
  const sameCefCache = oldCef && a.engine === "cef" && resolve(oldCef.options.cache) === resolve(next.cache);
  // Keep the old engine alive until the replacement pages have been opened.
  context = null; cef = null; spare = [];
  for (const o of owners.values()) { o.recording = false; await stopShots(o); clearShot(o); o.tabs = []; o.active = null; }
  await stopScreencast();
  engine = a.engine; cefOptions = next;
  try {
    if (sameCefCache) await oldCef!.stop();
    await ensureContext();
    for (const u of urls) {
      try { await (await ensurePage(u.name)).goto(u.url, { waitUntil: "load", timeout: DEFAULT_TIMEOUT }); }
      catch (e) { log(`engine reopen ${u.name}: ${e}`); }
    }
  } finally {
    if (oldCef) await oldCef.stop();
    else await oldContext?.close();
  }
  return status(name);
}

// the page an owner drives, made on first use
async function ensurePage(name: string): Promise<Page> {
  const o = ownerFor(name);
  if (o.active && !o.active.isClosed()) return o.active;
  if (!o.opening) o.opening = newTab(o).finally(() => { o.opening = null; });
  return o.opening;
}

async function newTab(o: Owner): Promise<Page> {
  const ctx = await ensureContext();
  let p = spare.pop();
  while (p && p.isClosed()) p = spare.pop();
  if (engine === "cef") {
    const key = `${safeOwner(o.name).slice(0, 48)}-${++tabNumber}`;
    cefOwners.set(key, o);
    p = await cef!.create(key);
  } else if (!p) p = await ctx.newPage();
  await adopt(o, p);
  await activate(o, p);
  return p;
}

async function adopt(o: Owner, p: Page): Promise<void> {
  if (ownerOf.has(p)) return;
  if (engine === "cef") cefOwners.set(await cef!.key(p), o);
  ownerOf.set(p, o);
  o.tabs.push(p);
  p.setDefaultTimeout(DEFAULT_TIMEOUT);
  p.on("console", (m) => pushLog(o, { level: m.type(), text: m.text() }));
  p.on("pageerror", (e) => pushLog(o, { level: "error", text: String(e) }));
  p.on("framenavigated", (f) => {
    if (p === o.active && f === p.mainFrame()) send({ event: "navigated", page: o.name, url: f.url() });
    if (o.recording && p === o.active && f === p.mainFrame()) recordNavigate(o, f.url());
  });
  p.on("load", () => {
    if (p === o.active) send({ event: "load", page: o.name, url: p.url() });
  });
  // a popup (window.open, target=_blank) joins its opener's tabs and is
  // what the owner drives next
  p.on("popup", (q) => {
    void adopt(o, q).then(() => activate(o, q)).catch((e) => log(`popup: ${e}`));
  });
  p.on("close", () => {
    o.tabs = o.tabs.filter((t) => t !== p);
    if (o.active !== p) return;
    o.active = null;
    const next = o.tabs[o.tabs.length - 1];
    if (next) void activate(o, next).catch((e) => log(`tab: ${e}`));
    else clearShot(o);
  });
  if (engine === "cef") await sizePage(o, p);
  else await p.setViewportSize(o.viewport).catch(() => {});
  // a tab adopted while recording (a popup) was told "no" on its first load
  if (o.recording) await recorderSet(o, true);
}

// Recording
// ---------

const STEP_KEYS = new Set(["click", "type", "select", "press", "submit"]);
const TARGET_KEYS = ["role", "name", "label", "placeholder", "text", "testid", "css", "nth", "near", "tag", "type"];

// tell every document of the owner's tabs to record or stop
async function recorderSet(o: Owner, on: boolean): Promise<void> {
  for (const t of [...o.tabs]) {
    if (t.isClosed()) continue;
    for (const f of t.frames()) {
      await f.evaluate((v) => { (window as any).__bpRecSet?.(v); }, on).catch(() => {});
    }
  }
}

function recordNavigate(o: Owner, url: string): void {
  if (!url || url === "about:blank" || url === o.lastNav) return;
  o.lastNav = url;
  send({ event: "record", page: o.name, step: { do: "navigate", url, at: Date.now(), frame: "" } });
}

// the binding: the page asks whether to record, or reports one step. The
// step is rebuilt here from known keys, and a secret's value is dropped
// whatever the page sent.
function onRecord(source: { page: Page; frame: any }, ev: Json): any {
  const o = ownerOf.get(source.page);
  if (ev?.q === "on?") return Boolean(o?.recording);
  if (!o || !o.recording) return;
  const s = ev?.step;
  if (!s || typeof s !== "object" || !STEP_KEYS.has(s.do)) return;
  const t: Json = {};
  for (const k of TARGET_KEYS) {
    const v = s.target?.[k];
    t[k] = k === "nth" ? (Number.isFinite(v) ? v : 0) : typeof v === "string" ? v.slice(0, 200) : "";
  }
  const secret = s.secret === true;
  const step: Json = { do: s.do };
  if (s.do === "press") step.key = ["Enter", "Tab", "Escape"].includes(s.key) ? s.key : "";
  if (s.do === "press" && !step.key) return;
  step.target = t;
  if (secret && (s.do === "type" || s.do === "select")) {
    step.secret = true;
    step.label = String(s.label ?? "").slice(0, 80);
  } else if (s.do === "type") step.value = String(s.value ?? "").slice(0, 10_000);
  else if (s.do === "select") {
    step.value = String(s.value ?? "").slice(0, 500);
    step.text = String(s.text ?? "").slice(0, 200);
  }
  step.url = source.page.url();
  step.at = Date.now();
  step.frame = source.frame === source.page.mainFrame() ? "" : String(source.frame?.url?.() ?? "");
  send({ event: "record", page: o.name, step });
}

// make p the tab the owner drives, and the one its frames show
async function activate(o: Owner, p: Page): Promise<void> {
  o.active = p;
  await startShots(o);
  if (o.name === "" && frames.on) await startScreencast();
  const at = cefPlacement, host = cef;
  if (engine === "cef" && host && at?.owner === o.name) {
    const key = await host.key(p);
    // A hide or another owner's show while adoption awaited its CDP target wins.
    const current = () => cef === host && cefPlacement === at && o.active === p;
    if (current()) await host.command("show", key, `${at.x} ${at.y} ${at.w} ${at.h}`);
    if (current()) await host.command("shield", "", cefShield ? "1" : "0");
    if (current() && !cefShield) await host.command("focus", key);
  }
}

function tabOf(name: string): Page | null {
  const p = owners.get(name)?.active;
  return p && !p.isClosed() ? p : null;
}

async function status(name: string): Promise<Json> {
  const o = ownerFor(name);
  const p = tabOf(name);
  if (engine === "cef" && p) o.viewport = await p.evaluate(() => ({ width: innerWidth, height: innerHeight }));
  return {
    version: VERSION,
    engine,
    ...(cef ? { port: cef.port, hostPid: cef.proc?.pid, cache: cef.options.cache } : {}),
    page: name,
    open: p !== null,
    chromium: executable ?? CHROMIUM_ARG ?? findChromium(),
    profile: PROFILE,
    url: p ? p.url() : null,
    title: p ? await p.title().catch(() => "") : null,
    loading: p ? await p.evaluate(() => document.readyState !== "complete").catch(() => false) : false,
    width: o.viewport.width,
    height: o.viewport.height,
    tabs: o.tabs.length,
    frames: { on: frames.on, maxFps: frames.maxFps, seq: frames.seq, path: FRAMES_PATH },
    shot: JPEG_DIR ? { path: shotPath(name), n: o.shot.seq, maxFps: JPEG_FPS } : null,
  };
}

// Raw RGB frames (the shared page)
// ------------------------------

async function startScreencast(): Promise<void> {
  const p = tabOf("");
  const ctx = context;
  if (!p || !ctx) return;
  if (frames.cdp && frames.page === p) {
    await frames.cdp.send("Page.stopScreencast").catch(() => {});
  } else {
    await stopScreencast();
    const s = await ctx.newCDPSession(p);
    s.on("Page.screencastFrame", (e: any) => {
      s.send("Page.screencastFrameAck", { sessionId: e.sessionId }).catch(() => {});
      if (frames.cdp === s) frames.pending = Buffer.from(e.data, "base64");
    });
    frames.cdp = s;
    frames.page = p;
  }
  if (engine === "cef" && !frames.capture) {
    const s = frames.cdp;
    const release = await cef!.capture(p);
    if (frames.cdp !== s || !frames.on) { await release(); return; }
    // A concurrent restart may have acquired its own lease meanwhile.
    if (frames.capture) await release(); else frames.capture = release;
  }
  const vp = ownerFor("").viewport;
  await frames.cdp!.send("Page.startScreencast", {
    format: "png",
    maxWidth: vp.width,
    maxHeight: vp.height,
    everyNthFrame: 1,
  });
}

async function stopScreencast(): Promise<void> {
  const s = frames.cdp;
  frames.cdp = null;
  frames.page = null;
  const release = frames.capture; frames.capture = null;
  if (release) await release();
  if (s) {
    await s.send("Page.stopScreencast").catch(() => {});
    await s.detach().catch(() => {});
  }
}

// write the frame file atomically and announce it; `force` skips the
// unchanged-page check
function publish(png: Buffer, force: boolean): Json | null {
  const t0 = performance.now();
  const { width, height, rgb } = decodePng(png);
  const hash = Bun.hash(rgb) as bigint;
  if (!force && hash === frames.lastHash) return null;
  frames.lastHash = hash;
  frames.seq += 1;
  const header = Buffer.alloc(16);
  header.write("BPF1", 0, "latin1");
  header.writeUInt32LE(width, 4);
  header.writeUInt32LE(height, 8);
  header.writeUInt32LE(frames.seq >>> 0, 12);
  const tmp = FRAMES_PATH + ".tmp";
  writeFileSync(tmp, Buffer.concat([header, rgb]));
  renameSync(tmp, FRAMES_PATH);
  if (DEBUG) log(`frame ${frames.seq}: ${width}x${height}, png ${png.length} B, decode+write ${(performance.now() - t0).toFixed(1)} ms`);
  const ev = { event: "frame", w: width, h: height, seq: frames.seq, path: FRAMES_PATH };
  send(ev);
  return ev;
}

function tick(): void {
  const png = frames.pending;
  if (!png || frames.busy) return;
  frames.pending = null;
  try {
    publish(png, false);
  } catch (e) {
    log(`frame: ${e}`);
  }
}

function restartTimer(): void {
  if (frames.timer) clearInterval(frames.timer);
  frames.timer = null;
  if (frames.on) frames.timer = setInterval(tick, Math.max(1, Math.round(1000 / frames.maxFps)));
}

// one screenshot of the shared page now, published even when unchanged
async function captureNow(force: boolean): Promise<Json | null> {
  const p = await ensurePage("");
  frames.busy = true;
  try {
    const png = await p.screenshot({ type: "png", animations: "allow", caret: "initial", timeout: DEFAULT_TIMEOUT });
    return publish(png, force);
  } finally {
    frames.busy = false;
  }
}

// JPEG frames (every page, with --jpeg-dir)
// ---------------------------------------

async function startShots(o: Owner): Promise<void> {
  if (!JPEG_DIR || !context) return;
  await stopShots(o);
  const p = o.active;
  if (!p || p.isClosed()) return;
  const s = await context.newCDPSession(p);
  o.shot.cdp = s;
  s.on("Page.screencastFrame", (e: any) => {
    s.send("Page.screencastFrameAck", { sessionId: e.sessionId }).catch(() => {});
    if (o.shot.cdp !== s) return;
    o.shot.pending = Buffer.from(e.data, "base64");
    scheduleShot(o);
  });
  o.shot.casting = false;
  await castCheck(o);
}

// whether a client shows the page (its cast lease holds)
function watched(o: Owner): boolean {
  return performance.now() < o.shot.fast;
}

// the screencast runs exactly while the page is watched
function castCheck(o: Owner): Promise<void> {
  // Start/stop can overlap lease expiry, tab adoption or helper teardown.
  // Serialize each owner's transitions and discard leases acquired for an
  // obsolete target; a late acquisition must never keep a hidden tab busy.
  const task = (o.shot.castTask ?? Promise.resolve()).catch(() => {}).then(async () => {
    const s = o.shot.cdp;
    if (!s) return;
    const want = watched(o);
    if (want === o.shot.casting) return;
    if (want) {
      let release: (() => Promise<void>) | null = null;
      try {
        if (engine === "cef" && o.active) release = await cef!.capture(o.active);
        if (o.shot.cdp !== s || !watched(o)) { if (release) await release(); return; }
        o.shot.capture = release;
        await s.send("Page.startScreencast", { format: "jpeg", quality: 70, maxWidth: 1280, maxHeight: 2048, everyNthFrame: 1 });
        if (o.shot.cdp === s) o.shot.casting = true;
      } catch (e) {
        if (o.shot.capture === release) o.shot.capture = null;
        if (release) await release();
        throw e;
      }
    } else {
      o.shot.casting = false;
      const release = o.shot.capture; o.shot.capture = null;
      await s.send("Page.stopScreencast").catch(() => {});
      if (release) await release();
    }
  });
  o.shot.castTask = task;
  return task.finally(() => { if (o.shot.castTask === task) o.shot.castTask = null; });
}

function castQuiet(o: Owner): void {
  void castCheck(o).catch(e => log(`cast: ${e}`));
}

// once a second: leases that ran out stop their screencast; an unwatched
// page gets a still every IDLE_STILL_MS (written only when it changed)
setInterval(() => {
  const now = performance.now();
  for (const o of owners.values()) {
    castQuiet(o);
    if (!watched(o) && o.shot.cdp && now - o.shot.still >= IDLE_STILL_MS) {
      o.shot.still = now;
      void shotNow(o.name);
    }
  }
}, 1000).unref();

async function stopShots(o: Owner): Promise<void> {
  const s = o.shot.cdp;
  o.shot.cdp = null;
  o.shot.pending = null;
  o.shot.casting = false;
  const release = o.shot.capture; o.shot.capture = null;
  if (release) await release();
  if (s) {
    await s.send("Page.stopScreencast").catch(() => {});
    await s.detach().catch(() => {});
  }
}

function clearShot(o: Owner): void {
  o.shot.cdp = null;
  o.shot.pending = null;
  o.shot.casting = false;
  const release = o.shot.capture; o.shot.capture = null;
  if (release) void release();
  if (o.shot.timer) clearTimeout(o.shot.timer);
  o.shot.timer = null;
}

// a page that painted before its screencast began sends nothing until it
// paints again: after a navigation its frame is taken once by hand
async function shotNow(name: string): Promise<void> {
  const o = owners.get(name);
  const s = o?.shot.cdp;
  if (!o || !s) return;
  const r: any = await s.send("Page.captureScreenshot", { format: "jpeg", quality: 70 }).catch(() => null);
  if (!r?.data || o.shot.cdp !== s) return;
  o.shot.pending ??= Buffer.from(r.data, "base64");
  scheduleShot(o);
}

// at most JPEG_FPS writes a second (CAST_FPS while a client shows the
// page): the newest frame waits for its slot, older ones are dropped
function shotFps(o: Owner): number {
  return performance.now() < o.shot.fast ? CAST_FPS : JPEG_FPS;
}

function scheduleShot(o: Owner): void {
  if (o.shot.timer) return;
  const wait = Math.max(0, o.shot.last + 1000 / shotFps(o) - performance.now());
  o.shot.timer = setTimeout(() => {
    o.shot.timer = null;
    writeShot(o);
  }, wait);
}

function writeShot(o: Owner): void {
  const jpg = o.shot.pending;
  const path = shotPath(o.name);
  o.shot.pending = null;
  if (!jpg || !path) return;
  const hash = Bun.hash(jpg) as bigint;
  if (hash === o.shot.lastHash) return;
  try {
    const tmp = path + ".tmp";
    writeFileSync(tmp, jpg);
    renameSync(tmp, path);
  } catch (e) {
    log(`shot: ${e}`);
    return;
  }
  o.shot.lastHash = hash;
  o.shot.last = performance.now();
  o.shot.seq += 1;
  // every client gets a frame at most JPEG_FPS a second ("all"); the rest
  // only the clients showing the page
  const now = performance.now();
  const all = now - o.shot.lastAll >= 1000 / JPEG_FPS - 5;
  if (all) o.shot.lastAll = now;
  send({ event: "frame", page: o.name, n: o.shot.seq, path, all });
}

// Targets
// -------

function targetOf(p: Page, a: Json): Locator | null {
  if (a.ref) return p.locator(`aria-ref=${a.ref}`);
  if (a.locator) return p.locator(String(a.locator)).first();
  if (a.selector) return p.locator(`css=${a.selector}`).first();
  return null;
}

function timeoutOf(a: Json, fallback = DEFAULT_TIMEOUT): number {
  const t = Number(a.timeoutMs ?? fallback);
  return Math.max(1, Math.min(60_000, Number.isFinite(t) ? t : fallback));
}

function needPath(a: Json, op: string): string {
  if (!a.path || typeof a.path !== "string") throw new Error(`${op} needs path`);
  return resolve(a.path);
}

// a file name in dir that is not taken yet: name, name (1), name (2)...
function freeName(dir: string, name: string): string {
  const safe = basename(name).replace(/[\/\\\0]/g, "_") || "download";
  const ext = extname(safe);
  const stem = safe.slice(0, safe.length - ext.length);
  let p = join(dir, safe);
  for (let i = 1; existsSync(p); i++) p = join(dir, `${stem} (${i})${ext}`);
  return p;
}

async function tabList(o: Owner): Promise<Json> {
  const tabs = await Promise.all(o.tabs.map(async (t, index) => ({
    index,
    url: t.url(),
    title: await t.title().catch(() => ""),
    active: t === o.active,
  })));
  return { page: o.name, active: o.active ? o.tabs.indexOf(o.active) : -1, tabs };
}

async function closeOwner(name: string): Promise<void> {
  const o = owners.get(name);
  if (!o) return;
  owners.delete(name);
  o.recording = false;
  await stopShots(o);
  clearShot(o);
  if (name === "") await stopScreencast();
  for (const t of [...o.tabs]) await closeTab(t).catch(() => {});
  const path = shotPath(name);
  if (path) rmSync(path, { force: true });
}

const SNAPSHOT_SCRIPT = `(() => {
  const selectorFor = (element) => {
    if (element.id) return "#" + CSS.escape(element.id);
    for (const attribute of ["data-testid", "name"]) {
      const value = element.getAttribute(attribute);
      if (value) return element.tagName.toLowerCase() + "[" + attribute + "=" + JSON.stringify(value) + "]";
    }
    const parts = [];
    for (let cur = element; cur && cur.nodeType === 1 && parts.length < 8; cur = cur.parentElement) {
      const parent = cur.parentElement;
      const same = parent ? Array.from(parent.children).filter((c) => c.tagName === cur.tagName) : [];
      const base = cur.tagName.toLowerCase();
      parts.unshift(same.length > 1 ? base + ":nth-of-type(" + (same.indexOf(cur) + 1) + ")" : base);
    }
    return parts.join(" > ");
  };
  const visible = (el) => {
    const s = getComputedStyle(el);
    const r = el.getBoundingClientRect();
    return s.visibility !== "hidden" && s.display !== "none" && r.width > 0 && r.height > 0;
  };
  const elements = Array.from(document.querySelectorAll("a[href],button,input,textarea,select,[role],[tabindex]"))
    .filter(visible).slice(0, ${MAX_ELEMENTS}).map((el) => {
      const r = el.getBoundingClientRect();
      return {
        tag: el.tagName.toLowerCase(),
        role: el.getAttribute("role"),
        name: (el.getAttribute("aria-label") || el.innerText || el.getAttribute("name") || el.getAttribute("placeholder") || "").trim().slice(0, 200),
        selector: selectorFor(el),
        x: Math.round(r.x), y: Math.round(r.y), width: Math.round(r.width), height: Math.round(r.height),
      };
    });
  return {
    loading: document.readyState !== "complete",
    visibleText: (document.body ? document.body.innerText : "").slice(0, ${MAX_VISIBLE_TEXT}),
    interactiveElements: elements,
  };
})()`;

// the page's readable text: its main or article when that holds most of
// the words, else the body; blank runs squeezed
const TEXT_SCRIPT = `(() => {
  const body = document.body ? document.body.innerText : "";
  const main = document.querySelector("main, [role=main], article");
  const m = main ? main.innerText : "";
  const t = m.length > 200 && m.length > body.length / 3 ? m : body;
  return t.split("\\n").map((l) => l.replace(/[ \\t\\u00a0]+/g, " ").trim()).join("\\n").replace(/\\n{3,}/g, "\\n\\n").trim();
})()`;

// Requests
// --------

// after input the next frame goes out at once, not in its slot
function quick(name: string): void {
  const o = owners.get(name);
  if (o) o.shot.last = 0;
}

// every handler gets the request's arguments and the owner it acts for
const handlers: Record<string, (a: Json, name: string) => Promise<any>> = {
  async engine(a, name) { return switchEngine(a, name); },
  async show(a, name) {
    if (engine !== "cef") throw new Error("show needs the cef engine");
    if (![a.x, a.y, a.w, a.h].every(Number.isInteger) || a.w < 1 || a.h < 1)
      throw new Error("show needs integer x,y,w,h with positive w,h");
    const p = await ensurePage(name);
    // x, y, w, h are the parent window's (device) pixels
    const r = await dpr(p);
    const o = ownerFor(name); o.viewport = { width: Math.round(a.w / r), height: Math.round(a.h / r) };
    // Publish the viewport before mapping. A concurrent open can finish
    // creating this same page, and must size it to the pane rather than
    // restore the owner's initial 1280x800 viewport after show completes.
    cefPlacement = { owner: name, x: a.x, y: a.y, w: a.w, h: a.h };
    await cef!.command("show", await cef!.key(p), `${a.x} ${a.y} ${a.w} ${a.h}`);
    await sized(p, o.viewport.width, o.viewport.height);
    return {};
  },
  async hide() {
    if (engine !== "cef") throw new Error("hide needs the cef engine");
    cefPlacement = null;
    await ensureContext(); await cef!.command("hide"); return {};
  },
  async shield(a) {
    if (engine !== "cef") throw new Error("shield needs the cef engine");
    if (typeof a.on !== "boolean") throw new Error("shield needs on (bool)");
    cefShield = a.on;
    await ensureContext(); await cef!.command("shield", "", a.on ? "1" : "0"); return {};
  },
  async focus(_a, name) {
    if (engine !== "cef") throw new Error("focus needs the cef engine");
    const p = await ensurePage(name); await cef!.command("focus", await cef!.key(p)); return {};
  },
  async open(a, name) {
    const o = ownerFor(name);
    if (a.width && a.height) o.viewport = { width: Math.round(a.width), height: Math.round(a.height) };
    const p = await ensurePage(name);
    await sizePage(o, p);
    if (a.url) await p.goto(normalizeUrl(a.url), { waitUntil: "load", timeout: timeoutOf(a) });
    await shotNow(name);
    return status(name);
  },
  async navigate(a, name) {
    if (!a.url) throw new Error("navigate needs url");
    const p = await ensurePage(name);
    const waitUntil = a.readiness === "none" ? "commit" : a.readiness === "domContentLoaded" ? "domcontentloaded" : "load";
    await p.goto(normalizeUrl(a.url), { waitUntil, timeout: timeoutOf(a) });
    await shotNow(name);
    return status(name);
  },
  async back(a, name) {
    await (await ensurePage(name)).goBack({ timeout: timeoutOf(a) });
    return status(name);
  },
  async forward(a, name) {
    await (await ensurePage(name)).goForward({ timeout: timeoutOf(a) });
    return status(name);
  },
  async reload(a, name) {
    await (await ensurePage(name)).reload({ timeout: timeoutOf(a) });
    return status(name);
  },
  async resize(a, name) {
    const width = Math.round(Number(a.width)), height = Math.round(Number(a.height));
    if (!(width > 0 && height > 0 && width <= 4096 && height <= 4096)) throw new Error("resize needs width and height in 1..4096");
    const o = ownerFor(name);
    o.viewport = { width, height };
    const p = await ensurePage(name);
    await sizePage(o, p);
    if (name === "" && frames.on) {
      await startScreencast();
      await captureNow(true);
    }
    return status(name);
  },
  // Human input is one queued operation, including coordinate conversion.
  // The hub provides only the bounded UI protocol, never arbitrary evaluation.
  async input(a, name) {
    const op = String(a.action ?? "");
    if (!["click", "mouse", "type", "press", "scroll", "navigate", "back", "reload"].includes(op)) throw new Error("unknown input action");
    const p = await ensurePage(name);
    const viewport = p.viewportSize() ?? ownerFor(name).viewport;
    const point = (v: unknown, size: number) => Math.min(size - 1, Math.max(0, Math.round(Number(v || 0) * size / 1000)));
    let args: Json = {};
    if (op === "click" || op === "mouse") args = { x: point(a.x, viewport.width), y: point(a.y, viewport.height), type: a.type };
    else if (op === "type") {
      const text = String(a.text ?? "");
      if (text.length > 65536) throw new Error("input text too long");
      args = { text };
    } else if (op === "press") args = { key: String(a.key ?? "") };
    else if (op === "scroll") args = { dx: Number(a.dx || 0), dy: Number(a.dy || 0) };
    else if (op === "navigate") args = { url: String(a.url ?? "") };
    return handlers[op](args, name);
  },
  async click(a, name) {
    const p = await ensurePage(name);
    quick(name);
    const button = a.button ?? "left";
    const clickCount = a.clickCount ?? 1;
    const t = targetOf(p, a);
    if (t) await t.click({ button, clickCount, timeout: timeoutOf(a) });
    else if (typeof a.x === "number" && typeof a.y === "number") await p.mouse.click(a.x, a.y, { button, clickCount });
    else throw new Error("click needs x and y, selector, locator, or ref");
    return {};
  },
  async mouse(a, name) {
    const p = await ensurePage(name);
    quick(name);
    const button = a.button ?? "left";
    if (typeof a.x === "number" && typeof a.y === "number") await p.mouse.move(a.x, a.y);
    if (a.type === "down") await p.mouse.down({ button });
    else if (a.type === "up") await p.mouse.up({ button });
    else if (a.type !== "move") throw new Error("mouse type must be move, down or up");
    return {};
  },
  async type(a, name) {
    const p = await ensurePage(name);
    quick(name);
    const text = String(a.text ?? "");
    const t = targetOf(p, a);
    if (t) {
      if (a.clear) await t.fill("", { timeout: timeoutOf(a) });
      else await t.focus({ timeout: timeoutOf(a) });
    } else if (a.clear) {
      await p.keyboard.press("ControlOrMeta+A");
      await p.keyboard.press("Delete");
    }
    await p.keyboard.insertText(text);
    return {};
  },
  async press(a, name) {
    const p = await ensurePage(name);
    quick(name);
    if (!a.key) throw new Error("press needs key");
    const mods: string[] = Array.isArray(a.modifiers) ? a.modifiers : [];
    await p.keyboard.press([...mods, String(a.key)].join("+"));
    return {};
  },
  async scroll(a, name) {
    const p = await ensurePage(name);
    quick(name);
    const vp = ownerFor(name).viewport;
    const dx = Number(a.dx ?? a.deltaX ?? 0), dy = Number(a.dy ?? a.deltaY ?? 0);
    const t = targetOf(p, a);
    if (t) {
      await t.evaluate((el, d) => el.scrollBy(d[0], d[1]), [dx, dy], { timeout: timeoutOf(a) });
    } else {
      const x = typeof a.x === "number" ? a.x : vp.width / 2;
      const y = typeof a.y === "number" ? a.y : vp.height / 2;
      await p.mouse.move(x, y);
      await p.mouse.wheel(dx, dy);
    }
    return {};
  },
  async snapshot(a, name) {
    const p = await ensurePage(name);
    const o = ownerFor(name);
    const page = await p.evaluate(SNAPSHOT_SCRIPT) as Json;
    const outline = await p.ariaSnapshot({ mode: "ai", timeout: timeoutOf(a) }).catch((e) => `(no outline: ${e.message})`);
    const out: Json = {
      url: p.url(),
      title: await p.title(),
      ...page,
      outline,
      console: o.console.slice(-50),
    };
    if (a.screenshot) {
      const png = await p.screenshot({ type: "png" });
      out.screenshot = { mimeType: "image/png", data: png.toString("base64"), width: o.viewport.width, height: o.viewport.height };
    }
    return out;
  },
  async evaluate(a, name) {
    const p = await ensurePage(name);
    if (!a.expression) throw new Error("evaluate needs expression");
    const expr = String(a.expression);
    const value = a.awaitPromise === false
      ? await p.evaluate(`(() => { const v = (${expr}); return v instanceof Promise ? "[Promise]" : v; })()`)
      : await p.evaluate(expr);
    const json = JSON.stringify(value ?? null);
    if (json.length > MAX_EVAL_BYTES) return { truncated: true, json: json.slice(0, MAX_EVAL_BYTES) };
    return value ?? null;
  },
  async wait_for(a, name) {
    const p = await ensurePage(name);
    const timeout = timeoutOf(a);
    const url = a.url ?? a.urlIncludes;
    if (!a.selector && !a.locator && !a.ref && !a.text && !url) throw new Error("wait_for needs selector, locator, text or url");
    const waits: Promise<unknown>[] = [];
    const t = targetOf(p, a);
    if (t) waits.push(t.waitFor({ state: "visible", timeout }));
    if (a.text) waits.push(p.waitForFunction((s) => (document.body?.innerText ?? "").includes(s), String(a.text), { timeout }));
    if (url) waits.push(p.waitForFunction((s) => location.href.includes(s), String(url), { timeout }));
    await Promise.all(waits);
    return {};
  },
  async text(a, name) {
    const p = await ensurePage(name);
    const max = Math.max(1, Math.min(MAX_TEXT, Number(a.max ?? MAX_VISIBLE_TEXT) || MAX_VISIBLE_TEXT));
    const t = targetOf(p, a);
    const full = t
      ? String(await t.innerText({ timeout: timeoutOf(a) }))
      : String(await p.evaluate(TEXT_SCRIPT));
    return { url: p.url(), title: await p.title(), length: full.length, truncated: full.length > max, text: full.slice(0, max) };
  },
  async select(a, name) {
    const p = await ensurePage(name);
    const t = targetOf(p, a);
    if (!t) throw new Error("select needs a selector, locator, or ref");
    let values: any;
    if (Array.isArray(a.values)) values = a.values.map(String);
    else if (a.value !== undefined) values = String(a.value);
    else if (a.label !== undefined) values = { label: String(a.label) };
    else if (a.index !== undefined) values = { index: Number(a.index) };
    else throw new Error("select needs value, values, label or index");
    const selected = await t.selectOption(values, { timeout: timeoutOf(a) });
    return { selected };
  },
  async upload(a, name) {
    const p = await ensurePage(name);
    const paths: string[] = (Array.isArray(a.paths) ? a.paths : [a.path]).filter((x: any) => typeof x === "string" && x).map((x: string) => resolve(x));
    if (!paths.length) throw new Error("upload needs path or paths");
    for (const f of paths) if (!existsSync(f) || !statSync(f).isFile()) throw new Error(`no such file: ${f}`);
    const t = targetOf(p, a) ?? p.locator("css=input[type=file]").first();
    const isInput = await t.evaluate((el) => el instanceof HTMLInputElement && el.type === "file", undefined, { timeout: timeoutOf(a) });
    if (isInput) {
      await t.setInputFiles(paths, { timeout: timeoutOf(a) });
    } else {
      // a button that opens the file chooser
      const [chooser] = await Promise.all([p.waitForEvent("filechooser", { timeout: timeoutOf(a) }), t.click({ timeout: timeoutOf(a) })]);
      await chooser.setFiles(paths);
    }
    return { files: paths };
  },
  async download(a, name) {
    if (!a.dir || typeof a.dir !== "string") throw new Error("download needs dir");
    const dir = resolve(a.dir);
    mkdirSync(dir, { recursive: true });
    const p = await ensurePage(name);
    const timeout = timeoutOf(a, 30_000);
    const t = targetOf(p, a);
    if (!t && !a.url) throw new Error("download needs a target to click or a url");
    if (engine === "cef") {
      const d = await cef!.download(p, async () => {
        if (t) await t.click({ timeout });
        else void p.goto(normalizeUrl(a.url), { timeout }).catch(() => {});
      }, timeout);
      const path = freeName(dir, d.suggestedFilename);
      copyFileSync(d.file, path); rmSync(d.file, { force: true });
      return { path, url: d.url, suggestedFilename: d.suggestedFilename, bytes: statSync(path).size };
    }
    const waiting = p.waitForEvent("download", { timeout });
    if (t) await t.click({ timeout });
    // navigating to a file fails with "Download is starting"; the event is what counts
    else p.goto(normalizeUrl(a.url), { timeout }).catch(() => {});
    const d = await waiting;
    const path = freeName(dir, d.suggestedFilename());
    await d.saveAs(path);
    const failure = await d.failure();
    if (failure) throw new Error(`download failed: ${failure}`);
    return { path, url: d.url(), suggestedFilename: d.suggestedFilename(), bytes: statSync(path).size };
  },
  async screenshot(a, name) {
    const p = await ensurePage(name);
    const path = needPath(a, "screenshot");
    mkdirSync(join(path, ".."), { recursive: true });
    const type = /\.jpe?g$/i.test(path) ? "jpeg" : "png";
    const t = targetOf(p, a);
    if (t) await t.screenshot({ path, type, timeout: timeoutOf(a) });
    else await p.screenshot({ path, type, fullPage: a.fullPage !== false, timeout: timeoutOf(a) });
    return { path, bytes: statSync(path).size };
  },
  async pdf(a, name) {
    if (engine === "cef") throw new Error("pdf needs the headless engine");
    const p = await ensurePage(name);
    const path = needPath(a, "pdf");
    mkdirSync(join(path, ".."), { recursive: true });
    await p.pdf({ path, printBackground: true });
    return { path, bytes: statSync(path).size };
  },
  async cookies_clear(a, _name) {
    const ctx = await ensureContext();
    if (a.domain) await ctx.clearCookies({ domain: String(a.domain) });
    else await ctx.clearCookies();
    return {};
  },
  async tabs(a, name) {
    const o = ownerFor(name);
    await ensurePage(name);
    if (a.new !== undefined && a.new !== false) {
      const p = engine === "cef" ? await newTab(o) : await (await ensureContext()).newPage();
      if (engine !== "cef") { await adopt(o, p); await activate(o, p); }
      const url = typeof a.new === "string" ? a.new : a.url;
      if (url) await p.goto(normalizeUrl(url), { waitUntil: "load", timeout: timeoutOf(a) });
    } else if (a.select !== undefined) {
      const p = o.tabs[Number(a.select)];
      if (!p) throw new Error(`no tab ${a.select}`);
      await activate(o, p);
    } else if (a.close !== undefined) {
      const p = o.tabs[Number(a.close)];
      if (!p) throw new Error(`no tab ${a.close}`);
      await closeTab(p);
      if (!o.tabs.length) await ensurePage(name);
    }
    return tabList(o);
  },
  async record(a, name) {
    const o = ownerFor(name);
    const on = a.on === true;
    if (on) {
      await ensurePage(name);
      o.recording = true;
      o.lastNav = "";
      await recorderSet(o, true);
    } else {
      o.recording = false;
      await recorderSet(o, false);
    }
    return { on: o.recording, page: name };
  },
  async pages(_a, _name) {
    const list = await Promise.all([...owners.values()].filter((o) => o.tabs.length > 0).map(async (o) => ({
      page: o.name,
      url: o.active?.url() ?? null,
      title: o.active ? await o.active.title().catch(() => "") : null,
      tabs: o.tabs.length,
      shot: shotPath(o.name),
      n: o.shot.seq,
    })));
    return { pages: list };
  },
  async frames(a, name) {
    if (name !== "") throw new Error("raw frames are for the shared page only");
    frames.on = Boolean(a.on);
    if (a.maxFps !== undefined) frames.maxFps = Math.max(1, Math.min(60, Number(a.maxFps) || 15));
    restartTimer();
    if (frames.on) {
      await ensurePage("");
      await startScreencast();
      await captureNow(true);
    } else {
      await stopScreencast();
    }
    return { on: frames.on, maxFps: frames.maxFps, seq: frames.seq, path: FRAMES_PATH };
  },
  async frame(_a, name) {
    if (name !== "") throw new Error("raw frames are for the shared page only");
    const ev = await captureNow(true);
    return ev ?? { seq: frames.seq, path: FRAMES_PATH };
  },
  // a client shows this page: it is cast, its frames going out at CAST_FPS,
  // for ms (2500); without it the page is not cast (IDLE_STILL_MS)
  async cast(a, name) {
    const o = ownerFor(name);
    const ms = Math.max(0, Math.min(10_000, Number(a.ms ?? 2500) || 0));
    const was = performance.now() < o.shot.fast;
    o.shot.fast = performance.now() + ms;
    // the screencast stops the moment the lease runs out, not at the next tick
    if (o.shot.lease) clearTimeout(o.shot.lease);
    o.shot.lease = setTimeout(() => { o.shot.lease = null; castQuiet(o); }, ms + 5);
    await castCheck(o);
    if (!was && ms > 0) void shotNow(name);
    return { fps: shotFps(o) };
  },
  async status(_a, name) {
    return status(name);
  },
  async close_page(_a, name) {
    await closeOwner(name);
    return {};
  },
  // with "page": close that owner's page; without: shut the helper down
  async close(_a, _name) {
    await shutdown(false);
    return {};
  },
};

// Loop
// ----

// Mutations of one owner's page execute in arrival order. Independent pages
// still run concurrently, and frame leases/status never wait behind navigation.
const pageTails = new Map<string, Promise<unknown>>();
function ordered<T>(name: string, run: () => Promise<T>): Promise<T> {
  const next = (pageTails.get(name) ?? Promise.resolve()).catch(() => {}).then(run);
  pageTails.set(name, next);
  void next.finally(() => { if (pageTails.get(name) === next) pageTails.delete(name); }).catch(() => {});
  return next;
}

// requests that change nothing on the page (no still for them)
const STILL_QUIET = new Set(["cast", "status", "frames", "frame", "close", "close_page", "snapshot", "console", "tabs"]);

// Focus is needed for native hidden-page input, not for idle pages,
// snapshots or background evaluations. The scope is released even on error.
const CEF_INPUT = new Set(["input", "click", "mouse", "type", "press", "scroll", "select"]);
async function dispatch(op: string, h: (a: Json, name: string) => Promise<any>, args: Json, name: string): Promise<any> {
  if (engine !== "cef" || !CEF_INPUT.has(op)) return h(args, name);
  const p = await ensurePage(name);
  return cef!.input(p, () => h(args, name));
}

async function handle(line: string): Promise<void> {
  if (!line.trim()) return;
  let req: Json;
  try {
    req = JSON.parse(line);
  } catch {
    send({ id: null, ok: false, error: "bad JSON" });
    return;
  }
  const id = req.id ?? null;
  let op = String(req.op ?? req.type ?? req.method ?? "");
  const h0 = Object.hasOwn(handlers, op) ? handlers[op] : undefined;
  if (!h0) {
    send({ id, ok: false, error: `unknown op: ${op}` });
    return;
  }
  // the owner is the top-level "page" only, never one inside "args"
  const pageGiven = typeof req.page === "string";
  const name: string = pageGiven ? req.page : "";
  if (op === "close" && pageGiven) op = "close_page";
  const h = handlers[op];
  const args = req.args && typeof req.args === "object" ? { ...req, ...req.args } : req;
  try {
    const run = () => dispatch(op, h, args, name);
    const result = await (["cast", "status", "pages"].includes(op) ? run() : ordered(name, run));
    send({ id, ok: true, result });
  } catch (e: any) {
    send({ id, ok: false, error: String(e?.message ?? e).split("\n")[0] });
  }
  // what a request did to an unwatched page shows in a still soon after
  const o = owners.get(name);
  if (o && !watched(o) && !STILL_QUIET.has(op)) o.shot.still = Math.min(o.shot.still, performance.now() - IDLE_STILL_MS + 300);
  if (op === "close") process.exit(0);
}

let closing = false;
async function shutdown(exit: boolean): Promise<void> {
  if (closing) return;
  closing = true;
  cefPlacement = null;
  if (frames.timer) clearInterval(frames.timer);
  frames.on = false;
  for (const o of owners.values()) clearShot(o);
  try { await stopEngine(); } catch {}
  if (!FRAMES_ARG) rmSync(FRAMES_PATH, { force: true });
  rmSync(FRAMES_PATH + ".tmp", { force: true });
  if (exit) process.exit(0);
}

process.on("SIGTERM", () => void shutdown(true));
process.on("SIGINT", () => void shutdown(true));

const rl = createInterface({ input: process.stdin, crlfDelay: Infinity });
rl.on("line", (line) => void handle(line));
rl.on("close", () => void shutdown(true));
send({ event: "ready", version: VERSION, frames: FRAMES_PATH, profile: PROFILE, jpegDir: JPEG_DIR });
