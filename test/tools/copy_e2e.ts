// A hub on a copy of a log (BACKPLANE_COPY=1, core/copy.bend) acts on
// nothing the log names. A first hub records real watched work: a work_run
// that would touch a marker file a few seconds in, and a pid watch on a
// process this test started. It is killed (SIGKILL) before the work runs,
// as a copied log is cut off mid-work. Then the log (not the work folder)
// is copied into a fresh home:
//   - with BACKPLANE_COPY=1 the hub says so at boot, never runs the
//     work_run (no marker, no work folder), work_cancel on the pid watch
//     signals nothing (the process lives), deploy_main is refused;
//   - the same copy without it re-launches the work_run (the marker
//     appears): the control, so this test would see the danger.
// The copied log also gets a work_run whose id is written only inside a
// WorkSet's rows string (no quoted id anywhere: a harness that greps quoted
// ids for its pre-cancel misses it): neither hub may need the id found.
//
//   bun test/tools/copy_e2e.ts HUB [WIREDIR]      (port 3831, E2E_PORT)
// HUB: the server as JS (run with bun) or a single-file build.
import { chmodSync, copyFileSync, existsSync, mkdirSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";

const [hub, wire = "build/wire"] = process.argv.slice(2);
if (!hub) { console.log("usage: bun test/tools/copy_e2e.ts HUB [WIREDIR]"); process.exit(2); }
for (const f of readdirSync(wire).filter((f) => f.endsWith(".js"))) (0, eval)(readFileSync(`${wire}/${f}`, "utf8"));
const W = (globalThis as any).Wire;
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
const PORT = Number(process.env.E2E_PORT ?? 3831);
if (PORT === 3787) throw new Error("refusing the live port");
let fail = 0;
const check = (ok: boolean, what: string) => { console.log(`${ok ? "ok  " : "FAIL"} ${what}`); if (!ok) fail++; };
const until = async <T>(ms: number, f: () => T | undefined | Promise<T | undefined>) => {
  const end = Date.now() + ms;
  for (;;) { const v = await f(); if (v) return v; if (Date.now() > end) return undefined; await sleep(100); }
};
const read = (p: string) => (existsSync(p) ? readFileSync(p, "utf8") : "");
const alive = (pid: number) => { try { process.kill(pid, 0); return !/^\d+ \(.*\) Z/.test(readFileSync(`/proc/${pid}/stat`, "utf8")); } catch { return false; } };

const root = resolve(process.env.E2E_ROOT ?? "build/copy_e2e");
if (!root.includes("copy_e2e")) throw new Error(`refusing root ${root}`);
rmSync(root, { recursive: true, force: true });
const fake = join(root, "bin"), proj = join(root, "proj"), mark = join(root, "mark"), mark2 = join(root, "mark2");
for (const d of [fake, proj, join(root, "tmp")]) mkdirSync(d, { recursive: true });
for (const n of ["claude", "codex", "grok", "muse"]) { writeFileSync(join(fake, n), "#!/bin/sh\nexit 1\n"); chmodSync(join(fake, n), 0o755); }
const argv = (home: string) => [...(hub.endsWith(".js") ? [process.execPath, resolve(hub)] : [resolve(hub)]),
  "--home", home, "--port", String(PORT), "--no-tailscale", "--foreground", "--web", root];
const env = (copy: boolean) => {
  const e: Record<string, string> = { ...(process.env as Record<string, string>), DISPLAY: "", BACKPLANE_NO_UPDATE: "1", BACKPLANE_ORDER_SERVER: "http://127.0.0.1:9", BACKPLANE_TELEMETRY: "off", BACKPLANE_PEERS: "",
    BACKPLANE_GOOGLE_BAKE: "0", BACKPLANE_AGENTS: fake, BACKPLANE_LOCK_DIR: join(root, "locks"), TMPDIR: join(root, "tmp") };
  delete e.BACKPLANE_HOME;
  delete e.BACKPLANE_COPY;
  if (copy) e.BACKPLANE_COPY = "1";
  return e;
};
mkdirSync(join(root, "locks"));
const up = () => until(30000, async () => { try { return (await fetch(`http://127.0.0.1:${PORT}/hello`)).status === 200 ? true : undefined; } catch { return undefined; } });
const mcp = async (home: string, thread: string, name: string, args: object) => {
  const tok = read(join(home, "token")).trim();
  const r = await fetch(`http://127.0.0.1:${PORT}/mcp/${thread}?token=${tok}`, { method: "POST", headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ jsonrpc: "2.0", id: 1, method: "tools/call", params: { name, arguments: args } }) });
  return r.text();
};
const procs: { kill: (s?: number) => void; exited: Promise<number> }[] = [];
const sleeper = Bun.spawn(["sleep", "600"]);

try {
  // 1. a hub that records the work, then dies before it runs
  const home1 = join(root, "home1");
  mkdirSync(home1);
  writeFileSync(join(home1, "events.jsonl"), "");
  const h1 = Bun.spawn(argv(home1), { env: env(false), stdout: "ignore", stderr: Bun.file(join(root, "hub1.err")) });
  procs.push(h1);
  check(!!(await up()), "the first hub is up");
  const seen: any[] = [];
  const ws = new WebSocket(`ws://127.0.0.1:${PORT}/ws`);
  ws.binaryType = "arraybuffer";
  ws.onmessage = (e) => {
    let o: any = null;
    try { o = JSON.parse(typeof e.data === "string" ? (e.data as string) : W.decode(new Uint8Array(e.data as ArrayBuffer))); } catch {}
    for (const c of o?.items ?? []) seen.push(typeof c === "string" ? JSON.parse(c) : c);
  };
  await new Promise((r, j) => { ws.onopen = r; ws.onerror = j; });
  let id = 0;
  const send = (m: string, p: object) => ws.send(W.encode(JSON.stringify({ id: ++id, m, p })));
  send("project.add", { path: proj });
  const pc: any = await until(15000, () => seen.find((x) => x.$ === "ProjectCreated" && x.root === proj));
  send("thread.create", { project: pc?.id, title: "worker", env: "local", provider: "claude" });
  const th = (await until(15000, () => seen.find((x) => x.$ === "ThreadCreated" && x.title === "worker")))?.id as string;
  check(!!th, "a thread to own the work");
  const run = await mcp(home1, th, "work_run", { command: `sleep 4; : > '${mark}'`, note: "copy e2e marker" });
  const runId = /w-t[0-9]+-[0-9]+-[0-9]+/.exec(run)?.[0] ?? "";
  const watch = await mcp(home1, th, "work_watch", { kind: "pid", target: String(sleeper.pid), note: "copy e2e sleeper" });
  const watchId = /w-t[0-9]+-[0-9]+-[0-9]+/.exec(watch)?.[0] ?? "";
  check(!!runId && !!watchId, `a work_run and a pid watch are recorded (${runId}, ${watchId})`);
  await sleep(800);
  h1.kill(9);
  await h1.exited;
  // the work this hub launched is its own (setsid): stop it before it runs
  const wpid = Number(read(join(home1, "work", runId, "pid")).trim());
  if (wpid > 1 && read(`/proc/${wpid}/cmdline`).includes(root)) { try { process.kill(-wpid, 9); } catch {} try { process.kill(wpid, 9); } catch {} }
  await sleep(4500);
  check(!existsSync(mark), "the first hub's work never ran (killed in time)");
  ws.close();

  // a second run known only from a WorkSet row: the thread's last row set
  // again with the run's id and marker swapped
  const lines = read(join(home1, "events.jsonl")).split("\n").filter((l) => l.includes('"$":"WorkSet"') && l.includes(runId));
  const rowOnly = "w-t1792000000-1-1";
  const extra = (lines[lines.length - 1] ?? "").split(runId).join(rowOnly).split(mark).join(mark2);
  check(extra.includes(rowOnly) && !extra.includes(`"${rowOnly}"`), "a work_run whose id is only inside a WorkSet rows string");
  const copyHome = (n: string) => {
    const h = join(root, n);
    mkdirSync(h);
    writeFileSync(join(h, "events.jsonl"), read(join(home1, "events.jsonl")) + extra + "\n");
    copyFileSync(join(home1, "token"), join(h, "token"));
    return h;
  };
  // 2. the copy, with BACKPLANE_COPY=1
  const home2 = copyHome("home2");
  const h2 = Bun.spawn(argv(home2), { env: env(true), stdout: "ignore", stderr: Bun.file(join(root, "hub2.err")) });
  procs.push(h2);
  check(!!(await up()), "the copy hub is up");
  check(read(join(root, "hub2.err")).includes("BACKPLANE_COPY=1"), "it says at boot that it is a copy");
  const cancel = await mcp(home2, th, "work_cancel", { id: watchId });
  const dep = await mcp(home2, th, "deploy_main", {});
  await sleep(9000);
  check(!existsSync(mark), "it never runs the work_run the log names");
  check(!existsSync(mark2), "nor the one known only from a rows string");
  check(!existsSync(join(home2, "work", runId, "started")), "nor makes its work folder");
  check(alive(sleeper.pid), `work_cancel of the pid watch signalled nothing (the process lives; ${cancel.slice(0, 80)})`);
  check(dep.includes("does not deploy"), `deploy_main is refused (${dep.slice(0, 120)})`);
  h2.kill(9);
  await h2.exited;

  // 3. the control: the same copy without it re-launches the work
  const home3 = copyHome("home3");
  const h3 = Bun.spawn(argv(home3), { env: env(false), stdout: "ignore", stderr: Bun.file(join(root, "hub3.err")) });
  procs.push(h3);
  check(!!(await up()), "the control hub is up");
  check(!!(await until(20000, () => (existsSync(mark2) ? true : undefined))), "without BACKPLANE_COPY the copy re-runs the work, the rows-only one too (the danger this guards)");
} finally {
  for (const p of procs) { try { p.kill(9); } catch {} }
  try { sleeper.kill(9); } catch {}
  // anything still running from this test's folders (the control's work)
  for (const d of readdirSync("/proc").filter((d) => /^\d+$/.test(d))) {
    const pid = Number(d);
    if (pid <= 1 || pid === process.pid) continue;
    if (read(`/proc/${d}/cmdline`).includes(join(root, "home"))) { try { process.kill(pid, 9); } catch {} }
  }
  if (fail) console.log(`kept ${root}`); else if (!process.env.KEEP) rmSync(root, { recursive: true, force: true });
}
console.log(fail ? `${fail} failed` : "all ok");
process.exit(fail ? 1 : 0);
