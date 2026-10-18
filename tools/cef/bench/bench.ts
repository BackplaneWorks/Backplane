// The CEF child window against the Playwright helper: startup, input to
// pixels, CPU and memory, on the same pages at the same 800x600 viewport.
//
//   DISPLAY=:78 bun tools/cef/bench/bench.ts --cef build/cef/Release/backplane-cef \
//     --xlat build/xlat [--helper tools/browser/src/main.ts] [--secs 15] [--n 30] [--only cef|helper]
//
// Run it on a private Xvfb: xlat injects XTest input into whatever has focus.
// CEF: input is a real X event into the child window; latency is until the
// pixel on the X screen changes. Helper: input is the press/scroll request
// the window sends today; latency is until its JPEG frame for the page is
// written (before the window reads and decodes it, so the helper's number is
// a lower bound of what the person sees).
import { mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const arg = (k: string, d = "") => { const i = process.argv.indexOf(k); return i >= 0 ? process.argv[i + 1] : d; };
const CEF = arg("--cef");
const XLAT = arg("--xlat");
const HELPER = arg("--helper", resolve(import.meta.dir, "../../browser/src/main.ts"));
const SECS = Number(arg("--secs", "15"));
const N = Number(arg("--n", "30"));
const ONLY = arg("--only");
const W = 800, H = 600;
const step = (m: string) => process.stderr.write(`bench: ${new Date().toISOString()} ${m}\n`);

const dir = mkdtempSync(join(tmpdir(), "bp-cef-bench-"));
const page = (name: string, html: string) => { const p = join(dir, name + ".html"); writeFileSync(p, html); return "file://" + p; };
const KEY = page("key", "<html><body style='margin:0;background:#123456;height:100vh' tabindex=0><script>let a=0;addEventListener('keydown',()=>{a^=1;document.body.style.background=a?'#e0a020':'#204080'})</script></body></html>");
const WHEEL = page("wheel", "<html><body style='margin:0;height:200000px;background:repeating-linear-gradient(#ff0000 0,#0000ff 256px,#ff0000 512px)'></body></html>");
const ANIM = page("anim", "<html><body style='margin:0'><canvas id=c width=800 height=600></canvas><script>let t=0;const x=c.getContext('2d');(function f(){t++;x.fillStyle='hsl('+(t%360)+',80%,50%)';x.fillRect(0,0,800,600);x.fillStyle='#000';x.font='60px sans';x.fillText(String(t),50,300);requestAnimationFrame(f)})()</script></body></html>");
const STATIC = page("static", "<html><body style='font:16px sans-serif;padding:20px'><h1>Backplane</h1>" + "<p>The quick brown fox jumps over the lazy dog. ".repeat(200) + "<input placeholder=Search><button>Go</button></body></html>");

// Process trees, CPU and PSS from /proc (read only)
// -------------------------------------------------
const tree = (root: number): number[] => {
  const kids = new Map<number, number[]>();
  for (const d of readdirSync("/proc")) {
    if (!/^\d+$/.test(d)) continue;
    try { const st = readFileSync(`/proc/${d}/stat`, "utf8"); const pp = Number(st.slice(st.lastIndexOf(")") + 2).split(" ")[1]); (kids.get(pp) ?? kids.set(pp, []).get(pp)!).push(Number(d)); } catch {}
  }
  const out = [root]; for (let i = 0; i < out.length; i++) out.push(...(kids.get(out[i]) ?? []));
  return out;
};
const ticks = (pids: number[]) => pids.reduce((a, q) => { try { const s = readFileSync(`/proc/${q}/stat`, "utf8"); const f = s.slice(s.lastIndexOf(")") + 2).split(" "); return a + Number(f[11]) + Number(f[12]); } catch { return a; } }, 0);
const pss = (pids: number[]) => pids.reduce((a, q) => { try { const m = /Pss:\s+(\d+)/.exec(readFileSync(`/proc/${q}/smaps_rollup`, "utf8")); return a + (m ? Number(m[1]) : 0); } catch { return a; } }, 0) / 1024;
async function usage(root: number, s: number) {
  const a = ticks(tree(root)); await Bun.sleep(s * 1000); const t = tree(root); const b = ticks(t);
  return { cpu: +(((b - a) / 100 / s) * 100).toFixed(1), pssMB: Math.round(pss(t)), procs: t.length };
}

// line-JSON child processes
// -------------------------
type Proc = { p: ReturnType<typeof Bun.spawn>; on: (f: (o: any) => boolean) => Promise<any>; send: (s: string) => void };
function spawnJson(cmd: string[]): Proc {
  const p = Bun.spawn(cmd, { stdin: "pipe", stdout: "pipe", stderr: Bun.env.BENCH_DEBUG ? "inherit" : "ignore" });
  const waits: { f: (o: any) => boolean; r: (o: any) => void }[] = [];
  (async () => {
    let buf = "";
    for await (const c of p.stdout as any) {
      buf += new TextDecoder().decode(c);
      let i;
      while ((i = buf.indexOf("\n")) >= 0) {
        const line = buf.slice(0, i); buf = buf.slice(i + 1);
        let o: any; try { o = JSON.parse(line); } catch { continue; }
        o.at = Date.now();
        for (let k = waits.length - 1; k >= 0; k--) if (waits[k].f(o)) { waits[k].r(o); waits.splice(k, 1); }
      }
    }
  })();
  return {
    p,
    on: (f) => new Promise((r, j) => { waits.push({ f, r }); setTimeout(() => j(new Error("timeout")), 30000); }),
    send: (s) => { (p.stdin as any).write(s + "\n"); (p.stdin as any).flush(); },
  };
}
const xlat = async (...a: string[]) => {
  const r = Bun.spawnSync([XLAT, ...a], { stderr: "ignore" });
  return JSON.parse(r.stdout.toString().trim().split("\n").pop()!);
};
const stats = (xs: number[]) => { const s = [...xs].sort((a, b) => a - b); return { n: s.length, mean: +(s.reduce((a, b) => a + b, 0) / s.length).toFixed(2), p50: +s[s.length >> 1].toFixed(2), p95: +s[Math.min(s.length - 1, Math.floor(s.length * 0.95))].toFixed(2), max: +s[s.length - 1].toFixed(2) }; };

// CEF
// ---
async function benchCef() {
  const out: any = {};
  const port = 9300 + Math.floor(Math.random() * 500);
  const t0 = Date.now();
  const c = spawnJson([resolve(CEF), "--url", KEY, "--size", `${W}x${H}`, "--cache", join(dir, "cef-cache"), "--port", String(port)]);
  const ready = await c.on((o) => o.event === "ready");
  const first = await xlat(String(ready.parent), "400", "300", "until:123456", "1");
  step('out.startup = { init'); out.startup = { ready: ready.at - t0, firstPixels: Math.round(first.t - t0) };
  await Bun.sleep(1000);
  step('out.key = await xlat'); out.key = await xlat(String(ready.parent), "400", "300", "key", String(N));
  c.send("url " + WHEEL); await c.on((o) => o.event === "load"); await Bun.sleep(500);
  step('out.wheel = await xlat'); out.wheel = await xlat(String(ready.parent), "400", "300", "wheel", String(N));
  c.send("url " + STATIC); await c.on((o) => o.event === "load"); await Bun.sleep(2000);
  step('out.idleStatic = await usage(c'); out.idleStatic = await usage(c.p.pid, SECS);
  c.send("url " + ANIM); await c.on((o) => o.event === "load"); await Bun.sleep(2000);
  step('out.animShown = await usage(c'); out.animShown = await usage(c.p.pid, SECS);
  c.send("hide"); await Bun.sleep(2000);
  step('out.animHidden'); out.animHidden = await usage(c.p.pid, SECS);
  c.send("show"); await Bun.sleep(1000);

  // automation on the same page over CDP, and a remote watcher's frames
  step("cdp");
  const nodeBin = Bun.env.BENCH_NODE ?? "node";
  const n = spawnJson([nodeBin, resolve(import.meta.dir, "cdp.mjs"), String(port), STATIC, ANIM, String(SECS)]);
  const done = n.on((o) => o.cdp);
  const got = await Promise.race([n.on((o) => o.casting), done]);
  const castUse = got.cdp ? null : await usage(c.p.pid, SECS);
  const cdp = (await done).cdp;
  cdp.animShownPlusCast = castUse;
  out.cdp = cdp;
  c.send("quit");
  await Promise.race([c.p.exited, Bun.sleep(10000)]);
  return out;
}

// the Playwright helper, as the window uses it today (a JPEG per frame, cast)
// ---------------------------------------------------------------------------
async function benchHelper() {
  const out: any = {};
  let id = 0;
  const t0 = Date.now();
  const h = spawnJson([process.execPath, HELPER, "--profile", join(dir, "h-prof"), "--jpeg-dir", join(dir, "shots"), "--frames", join(dir, "f.rgb")]);
  const ask = async (o: any) => { const n = ++id; const r = h.on((x) => x.id === n); h.send(JSON.stringify({ id: n, page: "bench", ...o })); return r; };
  const frame = (after: number) => h.on((o) => o.event === "frame" && o.page === "bench" && o.at > after);
  const opened = ask({ op: "open", url: KEY, width: W, height: H });
  const lease = setInterval(() => void ask({ op: "cast", ms: 2500 }), 1000);
  const r = await opened;
  if (!r.ok) throw new Error("open: " + r.error);
  void ask({ op: "cast", ms: 2500 });
  const f = await frame(0);
  step('out.startup = { opened'); out.startup = { opened: r.at - t0, firstFrame: f.at - t0 };
  await Bun.sleep(1000);
  const lat = async (op: any) => {
    const xs: number[] = [];
    for (let i = 0; i < N; i++) {
      const t = Date.now();
      const fr = frame(t);
      void ask(op);
      try { await fr; xs.push(Date.now() - t); } catch {}
      await Bun.sleep(150);
    }
    return stats(xs);
  };
  step('out.key = await lat'); out.key = await lat({ op: "press", key: "a" });
  await ask({ op: "navigate", url: WHEEL }); await Bun.sleep(500);
  step('out.wheel = await lat'); out.wheel = await lat({ op: "scroll", dy: 100, x: 400, y: 300 });
  await ask({ op: "navigate", url: STATIC }); await Bun.sleep(2000);
  step('out.idleStatic = await usage(h'); out.idleStatic = await usage(h.p.pid, SECS);
  await ask({ op: "navigate", url: ANIM }); await Bun.sleep(2000);
  let fr = 0; const count = h.on((o) => { if (o.event === "frame" && o.page === "bench") fr++; return false; });
  void count.catch(() => {});
  const f0 = fr;
  out.animShown = { ...(await usage(h.p.pid, SECS)), fps: 0 };
  out.animShown.fps = +((fr - f0) / SECS).toFixed(1);
  clearInterval(lease);
  await Bun.sleep(4000);
  step('out.animUnwatched'); out.animUnwatched = await usage(h.p.pid, SECS);
  h.send(JSON.stringify({ id: ++id, op: "close" }));
  await h.p.exited;
  return out;
}

const result: any = { at: new Date().toISOString(), viewport: `${W}x${H}`, secs: SECS, n: N, load: readFileSync("/proc/loadavg", "utf8").trim() };
try {
  if (ONLY !== "helper") result.cef = await benchCef().catch((e) => ({ error: String(e?.stack ?? e) }));
  if (ONLY !== "cef") result.helper = await benchHelper().catch((e) => ({ error: String(e?.stack ?? e) }));
} finally {
  result.loadAfter = readFileSync("/proc/loadavg", "utf8").trim();
  console.log(JSON.stringify(result, null, 1));
  rmSync(dir, { recursive: true, force: true });
  process.exit(0);
}
