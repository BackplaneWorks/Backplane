// CEF runtime discovery/installation and the stdin host/CDP connection.
import { existsSync, mkdirSync, mkdtempSync, readdirSync, renameSync, rmSync } from "node:fs";
import { homedir, tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { createInterface } from "node:readline";
import { EventEmitter } from "node:events";
import { chromium } from "./playwright.ts";
import { useBunWs } from "./bunws.ts";
import type { Browser, BrowserContext, Page, CDPSession } from "playwright-core";

export const CEF_VERSION = "154.0.34";
export const CEF_URL = "";
export const CEF_SHA256 = "";
const runtimeRoot = () => join(process.env.XDG_CACHE_HOME ?? join(homedir(), ".cache"), "backplane", "cef-runtime");
export function findCef(explicit?: string | null): string | null {
  for (const p of [explicit, process.env.BACKPLANE_CEF,
    join(dirname(process.execPath), "cef", "backplane-cef"),
    join(import.meta.dir, "../../../dist/cef/backplane-cef")]) {
    if (p && existsSync(p)) return resolve(p);
  }
  let versions: string[] = [];
  try { versions = readdirSync(runtimeRoot()); } catch {}
  versions.sort((a, b) => b.localeCompare(a, undefined, { numeric: true }));
  for (const v of versions) {
    const p = join(runtimeRoot(), v, "backplane-cef");
    if (existsSync(p)) return p;
  }
  return null;
}
export async function installCef(url = CEF_URL, sha256 = CEF_SHA256): Promise<string> {
  if (!url || !sha256) throw new Error("no CEF runtime: set BACKPLANE_CEF");
  mkdirSync(runtimeRoot(), { recursive: true });
  const tmp = mkdtempSync(join(runtimeRoot(), ".install-"));
  try {
    const res = await fetch(url);
    if (!res.ok) throw new Error(`CEF download: HTTP ${res.status}`);
    const archive = join(tmp, "runtime.tar.zst");
    await Bun.write(archive, res);
    const hash = new Bun.CryptoHasher("sha256").update(await Bun.file(archive).arrayBuffer()).digest("hex");
    if (hash !== sha256.toLowerCase()) throw new Error("CEF download: SHA-256 mismatch");
    const out = join(tmp, "runtime"); mkdirSync(out);
    const unpack = Bun.spawn(["tar", "--zstd", "-xf", archive, "-C", out], { stdout: "ignore", stderr: "pipe" });
    if (await unpack.exited !== 0) throw new Error(`CEF unpack: ${await new Response(unpack.stderr).text()}`);
    if (!existsSync(join(out, "backplane-cef"))) throw new Error("CEF unpacked without backplane-cef");
    const dest = join(runtimeRoot(), CEF_VERSION);
    rmSync(dest, { recursive: true, force: true });
    renameSync(out, dest);
    return join(dest, "backplane-cef");
  } finally { rmSync(tmp, { recursive: true, force: true }); }
}
export type CefOptions = { parent: number; display: string; cef?: string | null; cache: string };
type Event = Record<string, any>;
export class CefEngine extends EventEmitter {
  proc: ReturnType<typeof Bun.spawn> | null = null;
  browser: Browser | null = null;
  context!: BrowserContext;
  port = 0;
  executable = "";
  targets = new Map<string, Event>();
  keys = new WeakMap<Page, string>();
  // Correlated by cmd+key; commands for a given key are issued sequentially.
  pending = new Map<string, { resolve: (e: Event) => void; reject: (e: Error) => void; timer: ReturnType<typeof setTimeout> }[]>();
  stopping = false;
  downloads = "";
  downloadSession: CDPSession | null = null;
  downloadError = "";
  constructor(public options: CefOptions) { super(); }
  async start(): Promise<BrowserContext> {
    this.executable = findCef(this.options.cef) ?? await installCef();
    const listener = Bun.listen({ hostname: "127.0.0.1", port: 0, socket: { data() {} } });
    this.port = listener.port; listener.stop(true);
    this.proc = Bun.spawn([this.executable, "--parent", String(this.options.parent), "--port", String(this.port), "--cache", resolve(this.options.cache)], {
      env: { ...process.env, DISPLAY: this.options.display }, stdin: "pipe", stdout: "pipe", stderr: "inherit",
    });
    const proc = this.proc;
    void (async () => {
      const lines = createInterface({ input: require("node:stream").Readable.fromWeb(proc.stdout as ReadableStream), crlfDelay: Infinity });
      for await (const line of lines) {
        let e: Event;
        try { e = JSON.parse(line); } catch { continue; }
        if (e.target) this.targets.set(e.target, e);
        if (e.event === "closed") for (const [target, info] of this.targets) {
          if (info.key === e.key) this.targets.delete(target);
        }
        if (e.cmd) {
          const q = this.pending.get(`${e.cmd}:${e.key ?? ""}`);
          const w = q?.shift();
          if (w) { clearTimeout(w.timer); e.event === "error" ? w.reject(new Error(e.error)) : w.resolve(e); }
        }
        this.emit("host", e);
      }
    })();
    void proc.exited.then((code) => {
      for (const q of this.pending.values()) for (const w of q) {
        clearTimeout(w.timer); w.reject(new Error(`CEF host exited (${code})`));
      }
      this.pending.clear();
      if (this.downloads) rmSync(this.downloads, { recursive: true, force: true });
      this.emit("exit", code);
    });
    try {
      let endpoint = "";
      const deadline = Date.now() + 20000;
      while (Date.now() < deadline && proc.exitCode === null) {
        try {
          const r = await fetch(`http://127.0.0.1:${this.port}/json/version`, { signal: AbortSignal.timeout(500) });
          endpoint = (await r.json()).webSocketDebuggerUrl;
          if (endpoint) break;
        } catch {}
        await Bun.sleep(30);
      }
      if (!endpoint) throw new Error("CEF host did not open CDP");
      // Bun's own WebSocket (bunws.ts); and a browser the host creates is a
      // CDP target of type "other" before it is a "page", which Playwright
      // skips unless told to attach those too (read at each attach, so it
      // stays set while CEF runs; the Chrome engine talks over a pipe)
      useBunWs();
      process.env.PW_CHROMIUM_ATTACH_TO_OTHER = "1";
      this.browser = await chromium.connectOverCDP(endpoint, { timeout: 15000 });
      this.context = this.browser.contexts()[0];
      this.downloads = mkdtempSync(join(tmpdir(), "bp-cef-downloads-"));
      try {
        this.downloadSession = await this.browser.newBrowserCDPSession();
        await this.downloadSession.send("Browser.setDownloadBehavior", { behavior: "allowAndName", downloadPath: this.downloads, eventsEnabled: true });
      } catch (e) { this.downloadError = `download unavailable in CEF: ${e}`; }
      if (proc.exitCode !== null || !this.browser.isConnected()) throw new Error("CEF host exited during startup");
      return this.context;
    } catch (e) { await this.stop(); throw e; }
  }
  command(cmd: string, key = "", args = ""): Promise<Event> {
    const proc = this.proc;
    if (!proc || proc.exitCode !== null) return Promise.reject(new Error("CEF host is not running"));
    if (/[\r\n]/.test(key + args)) return Promise.reject(new Error("CEF command contains a newline"));
    return new Promise((resolve, reject) => {
      const id = `${cmd}:${key}`;
      const q = this.pending.get(id) ?? []; this.pending.set(id, q);
      const w = { resolve, reject, timer: setTimeout(() => {
        const i = q.indexOf(w); if (i >= 0) q.splice(i, 1);
        reject(new Error(`CEF ${cmd} timed out`));
      }, 15000) };
      q.push(w);
      (proc.stdin as any).write([cmd, key, args].filter(Boolean).join(" ") + "\n");
      (proc.stdin as any).flush();
    });
  }
  async key(p: Page): Promise<string> {
    const cached = this.keys.get(p);
    if (cached) return cached;
    const s = await this.context.newCDPSession(p);
    let target: string;
    try { target = (await s.send("Target.getTargetInfo")).targetInfo.targetId; }
    finally { await s.detach(); }
    const deadline = Date.now() + 15000;
    while (!this.targets.has(target) && Date.now() < deadline) await Bun.sleep(10);
    const key = this.targets.get(target)?.key;
    if (!key) throw new Error("CEF page has no host key");
    this.keys.set(p, key);
    return key;
  }
  async create(key: string): Promise<Page> {
    const e = await this.command("create", key, "about:blank");
    const deadline = Date.now() + 15000;
    while (Date.now() < deadline) {
      for (const p of this.context.pages()) {
        if (p.isClosed()) continue;
        if (await this.key(p) === key) return p;
      }
      await Bun.sleep(10);
    }
    throw new Error(`CEF target ${e.target} did not attach`);
  }
  // Native hidden pages have no permanent synthetic focus/capture handle.
  // A live remote viewer or an input action temporarily needs rendering.
  // Each lease owns a separate CDP connection, so overlapping users cannot
  // release each other's handles. Disconnect releases it on error as well.
  async capture(p: Page): Promise<() => Promise<void>> {
    const key = await this.key(p);
    const targetId = [...this.targets].find(([, v]) => v.key === key)?.[0];
    const targets = await (await fetch(`http://127.0.0.1:${this.port}/json/list`, { signal: AbortSignal.timeout(5000) })).json();
    const target = targets.find((v: any) => v.id === targetId);
    if (!target) throw new Error("CEF capture target closed");
    const socket = new WebSocket(target.webSocketDebuggerUrl);
    let sequence = 0;
    const waiting = new Map<number, { done: (v: any) => void; fail: (e: Error) => void }>();
    socket.onmessage = e => {
      const reply = JSON.parse(String(e.data));
      waiting.get(reply.id)?.done(reply); waiting.delete(reply.id);
    };
    socket.onclose = () => {
      for (const w of waiting.values()) w.fail(new Error("CEF capture connection closed"));
      waiting.clear();
    };
    const focus = (enabled: boolean) => new Promise<void>((resolve, reject) => {
      const id = ++sequence;
      const timer = setTimeout(() => { waiting.delete(id); reject(new Error("CEF capture focus timed out")); }, 5000);
      const fail = (e: Error) => { clearTimeout(timer); waiting.delete(id); reject(e); };
      waiting.set(id, { fail, done: v => { clearTimeout(timer); v.error ? reject(new Error(v.error.message)) : resolve(); } });
      try { socket.send(JSON.stringify({ id, method: "Emulation.setFocusEmulationEnabled", params: { enabled } })); }
      catch (e) { fail(e as Error); }
    });
    let released = false;
    const release = async () => {
      if (released) return;
      released = true;
      try { if (socket.readyState === WebSocket.OPEN) await focus(false).catch(() => {}); }
      finally { socket.close(); }
    };
    try {
      await new Promise<void>((resolve, reject) => {
        const fail = (message: string) => { clearTimeout(timer); reject(new Error(message)); };
        const timer = setTimeout(() => fail("CEF capture connection timed out"), 5000);
        socket.onopen = () => { clearTimeout(timer); resolve(); };
        socket.onerror = () => fail("CEF capture connection failed");
      });
      await focus(true);
      return release;
    } catch (e) { await release(); throw e; }
  }
  async input<T>(p: Page, action: () => Promise<T>): Promise<T> {
    const release = await this.capture(p);
    try { return await action(); }
    finally { await release(); }
  }
  async close(p: Page): Promise<void> { await this.command("close", await this.key(p)); }
  async download(p: Page, trigger: () => Promise<unknown>, timeout: number): Promise<{ file: string; url: string; suggestedFilename: string }> {
    if (this.downloadError || !this.downloadSession) throw new Error(this.downloadError || "download unavailable in CEF");
    const s = this.downloadSession;
    const ps = await this.context.newCDPSession(p);
    let frameId: string;
    try { frameId = (await ps.send("Page.getFrameTree")).frameTree.frame.id; }
    finally { await ps.detach(); }
    return new Promise((resolve, reject) => {
      let item: any = null;
      const clean = () => { clearTimeout(timer); s.off("Browser.downloadWillBegin", begin); s.off("Browser.downloadProgress", progress); };
      const fail = (e: unknown) => { clean(); reject(e); };
      const begin = (e: any) => { if (e.frameId === frameId && !item) item = e; };
      const progress = (e: any) => {
        if (!item || item.guid !== e.guid) return;
        if (e.state === "canceled") fail(new Error("CEF download canceled"));
        if (e.state === "completed") {
          clean(); resolve({ file: join(this.downloads, item.guid), url: item.url, suggestedFilename: item.suggestedFilename });
        }
      };
      const timer = setTimeout(() => fail(new Error("CEF download timed out")), timeout);
      s.on("Browser.downloadWillBegin", begin); s.on("Browser.downloadProgress", progress);
      void trigger().catch(fail);
    });
  }
  async stop(): Promise<void> {
    if (this.stopping) { await this.proc?.exited; return; }
    this.stopping = true;
    if (this.proc && this.proc.exitCode === null) {
      (this.proc.stdin as any).write("quit\n"); (this.proc.stdin as any).end();
      await this.proc.exited;
    }
    delete process.env.PW_CHROMIUM_ATTACH_TO_OTHER;
    // The host owns Chromium. Its exit disconnects CDP; never Browser.close().
    if (this.downloads) rmSync(this.downloads, { recursive: true, force: true });
  }
}
