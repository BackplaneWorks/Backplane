// A big paste into a real terminal on the hub, as the web client sends it
// (term.open, term.input): 64 KB into a program that does not read yet. The
// hub stays responsive meanwhile (/hello under a second) and, once the
// program reads, every byte arrives. A headless hub, an empty home.
//
//   bun test/tools/term_paste_e2e.ts BINARY WIREDIR
import { mkdirSync, mkdtempSync, readdirSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const [bin = "build/hub", wire = "build/wire"] = process.argv.slice(2);
for (const f of readdirSync(wire).filter((f) => f.endsWith(".js"))) (0, eval)(readFileSync(`${wire}/${f}`, "utf8"));
const W = (globalThis as any).Wire;
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
let fail = 0;
const check = (ok: boolean, what: string, more: unknown = "") => {
  console.log(`${ok ? "ok  " : "FAIL"} ${what}${ok || more === "" ? "" : ": " + JSON.stringify(more).slice(0, 200)}`);
  if (!ok) fail++;
};
const until = async <T>(ms: number, f: () => T | undefined | false) => {
  const end = Date.now() + ms;
  for (;;) { const v = f(); if (v) return v as T; if (Date.now() > end) return undefined; await sleep(50); }
};
const root = mkdtempSync(join(tmpdir(), "bp-paste-"));
const home = join(root, "home"), proj = join(root, "proj");
mkdirSync(proj);
const port = (() => { const s = Bun.listen({ hostname: "127.0.0.1", port: 0, socket: { data() {} } }); const p = s.port; s.stop(true); return p; })();
const hub = Bun.spawn([resolve(bin), "--home", home, "--port", String(port), "--no-tailscale", "--foreground"], {
  env: { ...process.env, DISPLAY: "", BACKPLANE_NO_UPDATE: "1", BACKPLANE_PEERS: "", BACKPLANE_TELEMETRY: "off", SHELL: "/bin/sh" },
  stdout: "ignore", stderr: "ignore",
});
try {
  for (let i = 0; i < 300; i++) { try { if ((await fetch(`http://127.0.0.1:${port}/hello`)).ok) break; } catch {} await sleep(100); }
  const seen: any[] = [], msgs: any[] = [];
  const ws = new WebSocket(`ws://127.0.0.1:${port}/ws`);
  ws.binaryType = "arraybuffer";
  ws.onmessage = (e) => { try { const o = JSON.parse(typeof e.data === "string" ? e.data : W.decode(new Uint8Array(e.data as ArrayBuffer))); msgs.push(o); if (o.t !== "live") for (const c of o.items ?? []) seen.push(c); } catch {} };
  await new Promise((r) => (ws.onopen = r));
  let n = 0;
  const send = (m: string, p: object) => ws.send(W.encode(JSON.stringify({ id: ++n, m, p })));
  send("project.add", { path: proj });
  const pc: any = await until(10000, () => seen.find((c) => c.$ === "ProjectCreated" && c.root === proj));
  send("thread.create", { project: pc.id, title: "paste", env: "local", provider: "claude" });
  const th = (await until(10000, () => seen.find((c) => c.$ === "ThreadCreated" && c.title === "paste")) as any).id;
  const out = () => msgs.filter((o) => o.t === "term" && o.thread === th).map((o) => new TextDecoder().decode(new Uint8Array(o.b ?? []))).join("");
  const input = (s: string | Uint8Array) => send("term.input", { thread: th, b: [...(typeof s === "string" ? new TextEncoder().encode(s) : s)] });
  send("term.open", { thread: th, cols: 80, rows: 24 });
  check(!!(await until(10000, () => out().length > 0)), "the terminal's shell is up");
  // the program reads nothing for 2 s, then counts what it was given
  input("stty raw -echo; sleep 2; head -c 65536 | wc -c\n");
  await sleep(300);
  const t0 = performance.now();
  input(new Uint8Array(65536).fill(97));
  let worst = 0;
  for (let i = 0; i < 6; i++) {
    const a = performance.now();
    const ok = await fetch(`http://127.0.0.1:${port}/hello`).then((r) => r.ok).catch(() => false);
    worst = Math.max(worst, performance.now() - a);
    if (!ok) { worst = 99999; break; }
    await sleep(200);
  }
  check(worst < 1000, `the hub answers /hello while the paste waits (worst ${Math.round(worst)} ms)`);
  const got = await until(30000, () => /\b65536\b/.test(out()));
  check(!!got, `every byte reaches the program (${Math.round(performance.now() - t0)} ms)`, out().slice(-80));
} catch (e) {
  check(false, "run", String(e));
} finally {
  hub.kill();
  await hub.exited;
  rmSync(root, { recursive: true, force: true });
}
console.log(fail ? `term_paste: ${fail} failed` : "term_paste: ok");
process.exit(fail ? 1 : 0);
