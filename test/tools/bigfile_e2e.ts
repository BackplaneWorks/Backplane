// Large files end to end (core/attach.bend's Att.max, core/range.bend,
// host.js's Sock.save and Sock.send_path): a headless hub on a fresh home.
//   upload    a file of --mb MB (default 200) as the body of POST /attach:
//             it lands whole on disk, the event log grows by no more than
//             a few KB (a message carries the path, never the bytes)
//   limit     with "files.max.mb" under the file's size its body is
//             refused (413) before it is read; at 512 (4000 for a file of
//             512 MB or more) it is taken
//   serve     GET /img streams it back whole and byte for byte, and a
//             Range gets that part alone (206), the last bytes, or a 416
//             past the end
//   loop      all along, /hello is asked every 50 ms: the slowest answer
//             says how long the hub's loop was held; the hub's RSS is read
//             from /proc every 50 ms
//
//   bun test/tools/bigfile_e2e.ts --bin build/dev/backplane-serve [--wire build/wire] [--port 3833] [--mb 200] [--baseline] [--keep]
// --baseline measures an older hub (its caps refuse a big file): the
// limit and serve checks are skipped and only the numbers are printed.
// Only the hub started here is stopped.
import { chmodSync, closeSync, mkdirSync, mkdtempSync, openSync, readFileSync, readdirSync, rmSync, statSync, writeSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const argv = process.argv.slice(2);
const opt = (name: string, d: string) => { const i = argv.indexOf(`--${name}`); return i >= 0 ? argv[i + 1] : d; };
const bin = resolve(opt("bin", "build/dev/backplane-serve"));
const port = Number(opt("port", "3833"));
if (port === 3787 || port === 3788) throw new Error("that port is a live hub's");
const MB = Number(opt("mb", "200"));
const baseline = argv.includes("--baseline");
const keep = argv.includes("--keep");
const wire = opt("wire", "build/wire");
for (const f of readdirSync(wire).filter((f) => f.endsWith(".js"))) (0, eval)(readFileSync(`${wire}/${f}`, "utf8"));
const W = (globalThis as any).Wire;
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
let fail = 0;
const check = (ok: boolean, what: string) => { console.log(`${ok ? "ok  " : "FAIL"} ${what}`); if (!ok) fail++; };
const until = async <T>(ms: number, f: () => T | undefined | Promise<T | undefined>) => {
  const end = Date.now() + ms;
  for (;;) { const v = await f(); if (v) return v; if (Date.now() > end) return undefined; await sleep(50); }
};

const root = mkdtempSync(join(tmpdir(), "bp-bigfile-"));
const fake = join(root, "bin");
mkdirSync(fake);
for (const n of ["claude", "codex", "grok", "muse", "agy", "opencode", "pi", "omp"]) { writeFileSync(join(fake, n), "#!/bin/sh\nexit 1\n"); chmodSync(join(fake, n), 0o755); }
const home = join(root, "home"), proj = join(root, "proj"), web = join(root, "web");
mkdirSync(home); mkdirSync(proj); mkdirSync(web);
writeFileSync(join(web, "index.html"), "<!doctype html>");
writeFileSync(join(home, "events.jsonl"), "");
const TOKEN = crypto.randomUUID().replace(/-/g, "");
writeFileSync(join(home, "token"), TOKEN, { mode: 0o600 });
const auth = { Authorization: `Bearer ${TOKEN}` };

// the file: a pattern no two MB alike, so a misplaced piece shows
const src = join(root, "big.png");
{
  const fd = openSync(src, "w");
  const b = new Uint8Array(1 << 20);
  for (let m = 0; m < MB; m++) {
    for (let i = 0; i < b.length; i += 4) { const v = (m * 2654435761 + i * 40503) >>> 0; b[i] = v; b[i + 1] = v >>> 8; b[i + 2] = v >>> 16; b[i + 3] = v >>> 24; }
    writeSync(fd, b);
  }
  closeSync(fd);
}
const size = statSync(src).size;
const piece = (at: number, n: number) => { const fd = openSync(src, "r"); const b = Buffer.alloc(n); require("node:fs").readSync(fd, b, 0, n, at); closeSync(fd); return b; };

const proc = Bun.spawn([bin, "--home", home, "--port", String(port), "--host", "127.0.0.1", "--no-tailscale", "--headless", "--foreground", "--web", web], {
  env: { PATH: `${fake}:/usr/bin:/bin`, HOME: join(root, "user"), DISPLAY: "", WAYLAND_DISPLAY: "", BACKPLANE_AGENTS: fake, BACKPLANE_NO_UPDATE: "1", BACKPLANE_KISTACK_SYNC: "0",
    BACKPLANE_ORDER_SERVER: "http://127.0.0.1:9", BACKPLANE_PEERS: "", BACKPLANE_GOOGLE_BAKE: "0", BACKPLANE_TELEMETRY: "off", BACKPLANE_LOCK_DIR: join(root, "locks"),
    BUN_JSC_maxPerThreadStackUsage: "4000000000" },
  stdout: "ignore", stderr: Bun.file(join(root, "hub.err")) });

// the loop's and the memory's watch
let watching = true;
let worst = 0, worstAt = "", phase = "start";
let rss0 = 0, rssMax = 0;
const rss = () => { try { return Number(/VmRSS:\s+(\d+)/.exec(readFileSync(`/proc/${proc.pid}/status`, "utf8"))?.[1] ?? 0) / 1024; } catch { return 0; } };
const watch = (async () => {
  while (watching) {
    const t = performance.now();
    try { await fetch(`http://127.0.0.1:${port}/hello`); } catch {}
    const dt = performance.now() - t;
    if (dt > worst) { worst = dt; worstAt = phase; }
    rssMax = Math.max(rssMax, rss());
    await sleep(50);
  }
})();

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
  send("setting.set", { key: "restart.continue", value: "off" });
  send("thread.create", { project: pc.id, title: "big file", env: "local", provider: "claude" });
  const th = (await until(10000, () => changes.find((c) => c.$ === "ThreadCreated")))?.id as string;
  if (!th) throw new Error("no thread");
  await sleep(1500);
  rss0 = rss(); rssMax = rss0; worst = 0;
  const log0 = statSync(join(home, "events.jsonl")).size;

  const upload = (key: string) => fetch(`http://127.0.0.1:${port}/attach?thread=${encodeURIComponent(th)}&key=${key}&name=big.png`,
    { method: "POST", headers: { ...auth, "Content-Type": "application/octet-stream", "Content-Length": String(size) }, body: Bun.file(src) });

  // a limit under the file refuses it; then one it fits under takes it
  const under = MB > 128 ? "128" : "1";
  const over = MB >= 512 ? "4000" : "512";
  if (!baseline) {
    phase = "limit";
    send("setting.set", { key: "files.max.mb", value: under });
    await sleep(500);
    const t = performance.now();
    const r = await upload("small").catch(() => null);
    const ms = performance.now() - t;
    check(r?.status === 413, `over "files.max.mb" ${under} a ${MB} MB body is refused (${r?.status}, ${ms.toFixed(0)} ms)`);
    send("setting.set", { key: "files.max.mb", value: over });
    await sleep(500);
  }

  phase = "upload";
  const t0 = performance.now();
  const up = await upload("k1");
  const upMs = performance.now() - t0;
  const ans: any = await up.json().catch(() => ({}));
  const path = String(ans.attached ?? "");
  console.log(`     upload ${MB} MB: ${up.status} in ${(upMs / 1000).toFixed(2)} s (${(MB / (upMs / 1000)).toFixed(0)} MB/s) ${path ? "" : JSON.stringify(ans)}`);
  if (!baseline || path) {
    check(up.status === 200 && path !== "", "the upload is taken");
    check(path !== "" && statSync(path).size === size, "it is whole on disk");
  }

  if (path !== "") {
    phase = "serve";
    const t1 = performance.now();
    const g = await fetch(`http://127.0.0.1:${port}/img?path=${encodeURIComponent(path)}`, { headers: auth });
    let n = 0, same = true;
    const reader = g.body!.getReader();
    for (;;) {
      const { done, value } = await reader.read();
      if (done) break;
      if (same && (n % (16 << 20)) < value.length) same = Buffer.compare(Buffer.from(value.subarray(0, 4096)), piece(n, Math.min(4096, value.length))) === 0;
      n += value.length;
    }
    const getMs = performance.now() - t1;
    console.log(`     download ${MB} MB: ${g.status} in ${(getMs / 1000).toFixed(2)} s, Accept-Ranges ${g.headers.get("accept-ranges")}`);
    check(g.status === 200 && n === size && same, `/img streams it back whole (${n} of ${size} bytes)`);
    if (!baseline) {
      phase = "range";
      const at = 123456789 % size;
      const r1 = await fetch(`http://127.0.0.1:${port}/img?path=${encodeURIComponent(path)}`, { headers: { ...auth, Range: `bytes=${at}-${at + 9999}` } });
      const b1 = Buffer.from(await r1.arrayBuffer());
      check(r1.status === 206 && r1.headers.get("content-range") === `bytes ${at}-${at + 9999}/${size}` && Buffer.compare(b1, piece(at, 10000)) === 0, "a Range gets that part alone (206)");
      const r2 = await fetch(`http://127.0.0.1:${port}/img?path=${encodeURIComponent(path)}`, { headers: { ...auth, Range: "bytes=-100" } });
      const b2 = Buffer.from(await r2.arrayBuffer());
      check(r2.status === 206 && Buffer.compare(b2, piece(size - 100, 100)) === 0, "bytes=-100 gets the last 100");
      const r3 = await fetch(`http://127.0.0.1:${port}/img?path=${encodeURIComponent(path)}`, { headers: { ...auth, Range: `bytes=${size}-` } });
      await r3.arrayBuffer();
      check(r3.status === 416 && r3.headers.get("content-range") === `bytes */${size}`, "a range past the end is a 416");
    }
  }

  // three at once: the loop still answers
  if (!baseline && path !== "") {
    phase = "three at once";
    const t2 = performance.now();
    const rs = await Promise.all([1, 2, 3].map(async (i) => {
      const g = await fetch(`http://127.0.0.1:${port}/img?path=${encodeURIComponent(path)}`, { headers: auth });
      return (await g.arrayBuffer()).byteLength;
    }));
    console.log(`     three downloads at once: ${((performance.now() - t2) / 1000).toFixed(2)} s`);
    check(rs.every((x) => x === size), "three downloads at once are each whole");
  }

  const log1 = statSync(join(home, "events.jsonl")).size;
  phase = "end";
  await sleep(300);
  watching = false;
  await watch;
  console.log(`     hub RSS ${rss0.toFixed(0)} MB at rest, ${rssMax.toFixed(0)} MB at most (+${(rssMax - rss0).toFixed(0)} MB) for a ${MB} MB file`);
  console.log(`     slowest /hello during it: ${worst.toFixed(0)} ms (${worstAt})`);
  console.log(`     events.jsonl grew ${log1 - log0} bytes`);
  if (!baseline) {
    check(log1 - log0 < 16384, "the event log holds no file bytes");
    check(rssMax - rss0 < Math.max(256, MB / 2), "the hub's memory does not follow the file's size");
    check(worst < 1000, "the loop is never held a second (no heartbeat lost)");
  }
  ws.close();
} catch (e) {
  console.log(`FAIL ${e}`);
  fail++;
} finally {
  watching = false;
  proc.kill();
  await proc.exited;
  if (!keep) rmSync(root, { recursive: true, force: true });
  else console.log(`kept ${root}`);
}
console.log(fail === 0 ? "all ok" : `${fail} failed`);
process.exit(fail === 0 ? 0 : 1);
