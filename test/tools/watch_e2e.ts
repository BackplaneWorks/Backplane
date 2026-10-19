// Watched work, end to end (item 8: hub.bend's Watch.*, the work_* MCP
// tools, the server's watchers and launcher). A headless hub with the
// stand-in `claude` (test/tools/claude_standin.ts); the test calls the
// thread's MCP URL as its agent would (the stand-in logs it):
//   1. work_run "sleep 3; exit 4", then the turn ends: the thread wakes
//      with exit 4 and the end of the output
//   2. work_watch of a file that gets the token: woken, done
//   3. a work_run that finishes while the hub is down (killed by PID): the
//      outcome comes from its retained code file after the restart
//   4. work_watch of a pid that goes away: "outcome unknown"
//   5. work_cancel: cancelling, then cancelled
//   6. the person's Stop (the work.cancel RPC a Subagents row sends)
//   7. a timeout: "timed out, still running, outcome unknown"
//
//   bun test/tools/watch_e2e.ts [BINARY] [WIREDIR]
import { chmodSync, existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const [bin = "build/backplane", wire = "build/wire"] = process.argv.slice(2);
for (const f of readdirSync(wire).filter((f) => f.endsWith(".js"))) (0, eval)(readFileSync(`${wire}/${f}`, "utf8"));
const W = (globalThis as any).Wire;
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

const root = mkdtempSync(join(tmpdir(), "bp-watch-"));
const fake = join(root, "bin");
const log = join(root, "claude.log");
mkdirSync(fake);
writeFileSync(join(fake, "claude"), `#!/bin/sh\nexec ${process.execPath} ${resolve("test/tools/claude_standin.ts")} "$@"\n`);
chmodSync(join(fake, "claude"), 0o755);
for (const n of ["codex", "grok", "muse", "omp"]) {
  writeFileSync(join(fake, n), "#!/bin/sh\nexit 1\n");
  chmodSync(join(fake, n), 0o755);
}
const home = join(root, "home");
const proj = join(root, "proj");
mkdirSync(proj);
mkdirSync(home);
writeFileSync(join(home, "events.jsonl"), "");

function freePort(): number {
  const s = Bun.listen({ hostname: "127.0.0.1", port: 0, socket: { data() {} } });
  const p = s.port;
  s.stop(true);
  return p;
}
const until = async <T>(ms: number, f: () => T | undefined) => {
  const end = Date.now() + ms;
  for (;;) {
    const v = f();
    if (v) return v;
    if (Date.now() > end) return undefined;
    await sleep(100);
  }
};
const lines = () => (existsSync(log) ? readFileSync(log, "utf8").split("\n").filter(Boolean) : []);
let fail = 0;
const check = (ok: boolean, what: string) => {
  console.log(`${ok ? "ok  " : "FAIL"} ${what}`);
  if (!ok) fail++;
};

const port = freePort();
type Hub = { proc: ReturnType<typeof Bun.spawn> };
const start = async (): Promise<Hub> => {
  const proc = Bun.spawn([resolve(bin), "--home", home, "--port", String(port), "--no-tailscale", "--foreground"], {
    env: { ...process.env, DISPLAY: "", WAYLAND_DISPLAY: "", BACKPLANE_NO_UPDATE: "1", BACKPLANE_KEYRING_LOCKED: "0", BACKPLANE_PEERS: "", BACKPLANE_GOOGLE_BAKE: "0", FAKE_LOG: log,
      BP_WORK_MS: "200", BACKPLANE_AGENTS: fake, PATH: `${fake}:${process.env.PATH}` },
    stdout: "ignore",
    stderr: "ignore",
  });
  for (let i = 0; i < 200; i++) {
    try { if ((await fetch(`http://127.0.0.1:${port}/hello`)).status === 200) break; } catch {}
    await sleep(100);
  }
  return { proc };
};
const kill = async (h: Hub) => {
  process.kill(h.proc.pid, "SIGKILL");
  await h.proc.exited;
};
type Client = { seen: any[]; send: (m: string, p: object) => void };
const connect = async (): Promise<Client> => {
  const seen: any[] = [];
  const ws = new WebSocket(`ws://127.0.0.1:${port}/ws`);
  ws.binaryType = "arraybuffer";
  ws.onmessage = (e) => {
    let o: any = null;
    try { o = JSON.parse(typeof e.data === "string" ? (e.data as string) : W.decode(new Uint8Array(e.data as ArrayBuffer))); } catch {}
    for (const c of o?.items ?? []) seen.push(c);
  };
  await new Promise((r, j) => { ws.onopen = r; ws.onerror = j; });
  let id = 0;
  return { seen, send: (m, p) => ws.send(W.encode(JSON.stringify({ id: ++id, m, p }))) };
};
const runs: string[] = [];
const call = async (url: string, name: string, args: object) => {
  const r = await fetch(url, { method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify({ jsonrpc: "2.0", id: 1, method: "tools/call", params: { name, arguments: args } }) });
  const t = await r.text();
  let inner: any = {};
  try { inner = JSON.parse(JSON.parse(t).result.content[0].text); } catch {}
  if (inner.id && name === "work_run") runs.push(inner.id);
  return { text: t, inner };
};

let hub: Hub | null = null;
try {
  hub = await start();
  let c = await connect();
  c.send("project.add", { path: proj });
  const pc = await until(10000, () => c.seen.find((x) => x.$ === "ProjectCreated"));
  const made = async (title: string) => {
    const n = c.seen.filter((x) => x.$ === "ThreadCreated").length;
    c.send("thread.create", { project: pc.id, title, env: "local", provider: "claude" });
    return (await until(10000, () => c.seen.filter((x) => x.$ === "ThreadCreated")[n]))?.id as string;
  };
  const turns = (th: string) => c.seen.filter((x) => x.$ === "TurnChanged" && x.thread === th).map((x) => String(x.state));
  // a thread with a live process, and its MCP URL
  const ready = async (title: string) => {
    const th = await made(title);
    const before = lines().filter((l) => l.startsWith("MCP ")).length;
    c.send("turn.start", { thread: th, text: "mode:plain hello", msg: `${title}-1` });
    await until(10000, () => turns(th).at(-1) === "completed" ? true : undefined);
    const url = lines().filter((l) => l.startsWith("MCP ")).slice(before).find((l) => l.includes(`/mcp/${th}.`))?.split(" ")[2] ?? "";
    return { th, url };
  };
  const told = (th: string, word: string) => c.seen.find((x) => x.$ === "MessagePosted" && x.thread === th && String(x.text).includes(word));

  // 1. work_run, woken with the outcome
  const a = await ready("run");
  const r1 = await call(a.url, "work_run", { command: "echo building; sleep 3; echo boom; exit 4", note: "slow build" });
  check(r1.inner.state === "launching", `work_run answers launching (${r1.text.slice(0, 100)})`);
  const w1 = await until(20000, () => told(a.th, "Watched work \"slow build\" ended"));
  check(!!w1 && String(w1.text).includes("exit code 4") && String(w1.text).includes("boom"), `woken with exit 4 and the output's end (${String(w1?.text ?? "").slice(0, 120)})`);
  await until(10000, () => turns(a.th).at(-1) === "completed" && turns(a.th).length >= 4 ? true : undefined);
  check(turns(a.th).slice(-2).join(",") === "running,completed", `the wake runs a turn (${turns(a.th).join(",")})`);

  // 2. a file that gets the token
  const b = await ready("file");
  const fp = join(root, "done.txt");
  const r2 = await call(b.url, "work_watch", { kind: "file", target: fp, note: "remote job" });
  writeFileSync(fp, `all good ${r2.inner.token}\n`);
  const w2 = await until(15000, () => told(b.th, "Watched work \"remote job\" ended: done"));
  check(!!w2, "a file that gets its token: woken, done");

  // 3. finishes while the hub is down
  const d = await ready("down");
  const r3 = await call(d.url, "work_run", { command: "sleep 4; echo survived; exit 0", note: "outlives the hub" });
  check(!!r3.inner.id, "work_run before the kill");
  await sleep(1000);
  await kill(hub);
  await sleep(5000);
  hub = await start();
  c = await connect();
  const w3 = await until(20000, () => told(d.th, "Watched work \"outlives the hub\" ended"));
  check(!!w3 && String(w3.text).includes("exit code 0") && String(w3.text).includes("survived"), `its outcome comes after the restart (${String(w3?.text ?? "").slice(0, 120)})`);

  // 4. a pid that goes away
  const e = await ready("pid");
  const sl = Bun.spawn(["sleep", "2"]);
  await call(e.url, "work_watch", { kind: "pid", target: String(sl.pid), note: "a process" });
  const w4 = await until(15000, () => told(e.th, "Watched work \"a process\" ended"));
  check(!!w4 && String(w4.text).includes("outcome unknown"), `a pid gone: outcome unknown (${String(w4?.text ?? "").slice(0, 120)})`);

  // 5. work_cancel
  const f = await ready("cancel");
  const r5 = await call(f.url, "work_run", { command: "sleep 30", note: "to cancel" });
  await sleep(1500);
  const c5 = await call(f.url, "work_cancel", { id: r5.inner.id });
  check(c5.inner.state === "cancelling", "work_cancel: cancelling");
  const w5 = await until(15000, () => told(f.th, "Watched work \"to cancel\" ended: cancelled"));
  check(!!w5, "then cancelled");

  // 6. the person's Stop
  const g = await ready("stop");
  const r6 = await call(g.url, "work_run", { command: "sleep 30", note: "stopped by the person" });
  await sleep(1500);
  c.send("work.cancel", { thread: g.th, id: r6.inner.id });
  const w6 = await until(15000, () => told(g.th, "Watched work \"stopped by the person\" ended: cancelled"));
  check(!!w6, "a Subagents row's Stop cancels it");

  // 7. a timeout
  const h7 = await ready("timeout");
  const r7 = await call(h7.url, "work_run", { command: "sleep 20", note: "too slow", timeoutSec: 3 });
  const w7 = await until(15000, () => told(h7.th, "Watched work \"too slow\" ended: timed out, still running, outcome unknown"));
  check(!!w7, "a timeout: still running, outcome unknown");
  void r7;
} finally {
  if (hub) await kill(hub);
  // what a run left going (its process group, the pid the launch wrote)
  for (const id of runs) {
    const pf = join(home, "work", id, "pid");
    const p = existsSync(pf) ? Number(readFileSync(pf, "utf8").trim()) : 0;
    if (p > 1) { try { process.kill(-p, "SIGTERM"); } catch {} try { process.kill(p, "SIGTERM"); } catch {} }
  }
  if (process.env.KEEP) console.log(root); else rmSync(root, { recursive: true, force: true });
}
console.log(fail ? `${fail} failed` : "all ok");
process.exit(fail ? 1 : 0);
