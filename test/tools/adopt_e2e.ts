// Agents kept through a deploy restart (core/adopt.bend), end to end. A
// headless hub installed in a temp prefix (a copy of the build) with the
// stand-in `claude` (test/tools/claude_standin.ts), BP_EOF_FINISH=1: like
// Claude Code, a stand-in whose stdin ends finishes its turn first.
//   1. Thread S streams a long turn (mode:stream); thread D calls
//      deploy_restart with a staged package; the hub swaps it in and
//      restarts in place (exec). Afterwards exactly one stand-in runs for
//      S, the one that was streaming (no --resume of its session), its text
//      keeps reaching a client, and its turn ends with all of it.
//   2. The same with BACKPLANE_ADOPT=0: the streaming stand-in is stopped
//      (SIGTERM) before anything starts for S, and S's turn carries on in
//      exactly one new process.
//   3. The hub is killed (SIGKILL) while S streams and a new hub starts on
//      the same home: nothing was handed off, so the stand-in still running
//      (found by the record, same start identity) is stopped, and S carries
//      on in exactly one new process.
//   ADOPT_ONLY=on|off|crash runs one.
//
//   bun test/tools/adopt_e2e.ts [BINARY] [WIREDIR]      (port 3793)
import { chmodSync, copyFileSync, existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const [bin = "build/backplane", wire = "build/wire"] = process.argv.slice(2);
for (const f of readdirSync(wire).filter((f) => f.endsWith(".js"))) (0, eval)(readFileSync(`${wire}/${f}`, "utf8"));
const W = (globalThis as any).Wire;
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
const PORT = Number(process.env.E2E_PORT ?? 3793);
if (PORT === 3787) throw new Error("refusing the live port");

let fail = 0;
const check = (ok: boolean, what: string) => {
  console.log(`${ok ? "ok  " : "FAIL"} ${what}`);
  if (!ok) fail++;
};
const until = async <T>(ms: number, f: () => T | undefined) => {
  const end = Date.now() + ms;
  for (;;) {
    const v = f();
    if (v) return v;
    if (Date.now() > end) return undefined;
    await sleep(100);
  }
};
// running (a zombie not reaped yet is not)
const alive = (pid: number) => {
  try { process.kill(pid, 0); } catch { return false; }
  try { return !/^\d+ \(.*\) Z/.test(readFileSync(`/proc/${pid}/stat`, "utf8")); } catch { return false; }
};
// the hub's children that ended and were never waited for
const zombies = (pid: number) => {
  const out: number[] = [];
  for (const d of readdirSync("/proc")) {
    if (!/^\d+$/.test(d)) continue;
    try {
      const st = readFileSync(`/proc/${d}/stat`, "utf8");
      const m = /\) (\S) (\d+)/.exec(st);
      if (m && m[1] === "Z" && Number(m[2]) === pid) out.push(Number(d));
    } catch {}
  }
  return out;
};
const read = (p: string) => (existsSync(p) ? readFileSync(p, "utf8") : "");

try { if ((await fetch(`http://127.0.0.1:${PORT}/hello`)).status === 200) { console.log(`port ${PORT} is in use`); process.exit(2); } } catch {}

async function scenario(kind: "on" | "off" | "crash") {
  const adopt = kind !== "off";
  console.log(`-- ${kind === "on" ? "adoption on" : kind === "off" ? "adoption off (BACKPLANE_ADOPT=0)" : "a crash (SIGKILL), then a new hub"}`);
  const root = mkdtempSync(join(tmpdir(), "bp-adopt-"));
  const fake = join(root, "bin"), log = join(root, "claude.log"), home = join(root, "home"), proj = join(root, "proj"), prefix = join(root, "prefix");
  for (const d of [fake, home, proj, prefix, join(prefix, "web")]) mkdirSync(d);
  writeFileSync(join(fake, "claude"), `#!/bin/sh\nexec ${process.execPath} ${resolve("test/tools/claude_standin.ts")} "$@"\n`);
  chmodSync(join(fake, "claude"), 0o755);
  for (const n of ["codex", "grok", "muse"]) { writeFileSync(join(fake, n), "#!/bin/sh\nexit 1\n"); chmodSync(join(fake, n), 0o755); }
  writeFileSync(join(home, "events.jsonl"), "");
  copyFileSync(resolve(bin), join(prefix, "backplane"));
  chmodSync(join(prefix, "backplane"), 0o755);
  writeFileSync(join(prefix, "LICENSE"), "orig");
  const stage = join(root, "stage");
  mkdirSync(join(stage, "web"), { recursive: true });
  copyFileSync(resolve(bin), join(stage, "backplane"));
  chmodSync(join(stage, "backplane"), 0o755);
  writeFileSync(join(stage, "LICENSE"), "new");
  const N = 260, MS = 100; // a 26 s turn
  const hubEnv = () => ({
      ...process.env, DISPLAY: "", WAYLAND_DISPLAY: "", BACKPLANE_NO_UPDATE: "1", BACKPLANE_ORDER_SERVER: "http://127.0.0.1:9", BACKPLANE_KEYRING_LOCKED: "0", BACKPLANE_PEERS: "", BACKPLANE_GOOGLE_BAKE: "0", FAKE_LOG: log,
      BP_EOF_FINISH: "1", BP_STREAM_N: String(N), BP_STREAM_MS: String(MS), BP_DEPLOY_ARGS: JSON.stringify({ stage, note: "adopt e2e" }),
      BACKPLANE_WAKE_TICK_MS: "2000", BACKPLANE_DEPLOY_LOCKS: join(root, "none.lock"), BACKPLANE_LOCK_DIR: join(root, "locks"), BACKPLANE_ADOPT: adopt ? "" : "0",
      BACKPLANE_AGENTS: fake, PATH: `${fake}:${process.env.PATH}`, TMPDIR: root });
  const spawnHub = (n: number) => Bun.spawn([join(prefix, "backplane"), "--home", home, "--port", String(PORT), "--no-tailscale", "--foreground"],
    { env: hubEnv(), stdout: "ignore", stderr: Bun.file(join(root, `hub${n}.err`)) });
  let proc = spawnHub(1);
  mkdirSync(join(root, "locks"));
  const up = async () => {
    for (let i = 0; i < 200; i++) { try { if ((await fetch(`http://127.0.0.1:${PORT}/hello`)).status === 200) return true; } catch {} await sleep(100); }
    return false;
  };
  try {
    check(await up(), "the hub is up");
    const seen: any[] = [];
    const deltas: { thread: string; text: string; at: number }[] = [];
    const connect = async () => {
      const ws = new WebSocket(`ws://127.0.0.1:${PORT}/ws`);
      ws.binaryType = "arraybuffer";
      ws.onmessage = (e) => {
        let o: any = null;
        try { o = JSON.parse(typeof e.data === "string" ? (e.data as string) : W.decode(new Uint8Array(e.data as ArrayBuffer))); } catch {}
        if (o?.t === "delta") deltas.push({ thread: o.thread, text: o.text, at: Date.now() });
        for (const c of o?.items ?? []) seen.push(typeof c === "string" ? JSON.parse(c) : c);
      };
      await new Promise((r, j) => { ws.onopen = r; ws.onerror = j; });
      let id = 0;
      return { ws, send: (m: string, p: object) => ws.send(W.encode(JSON.stringify({ id: ++id, m, p }))) };
    };
    let c = await connect();
    c.send("project.add", { path: proj });
    const pc: any = await until(10000, () => seen.find((x) => x.$ === "ProjectCreated"));
    const made = async (title: string) => {
      await sleep(1000);
      c.send("thread.create", { project: pc.id, title, env: "local", provider: "claude" });
      return (await until(10000, () => seen.find((x) => x.$ === "ThreadCreated" && x.title === title)))?.id as string;
    };
    const S = await made("streamer");
    const D = await made("deployer");
    c.send("turn.start", { thread: S, text: "mode:stream go w", msg: "s-1" });
    // which stand-in serves which thread: its MCP URL names it
    const pidsOf = (th: string) => [...read(log).matchAll(/^MCP (\d+) \S*\/mcp\/([^.?/]+)/gm)].filter((m) => m[2] === th).map((m) => Number(m[1]));
    await until(15000, () => deltas.find((d) => d.thread === S));
    const before = pidsOf(S);
    check(before.length === 1 && alive(before[0]), `one stand-in streams for S (${before})`);
    await sleep(2000);
    // a lock held for S: its guard (a child of the hub) ends with the exec
    // and is handed to the new image, which must still wait for it
    if (kind !== "crash") {
      const tok = read(join(home, "token")).trim();
      await fetch(`http://127.0.0.1:${PORT}/mcp/${S}?token=${tok}`, { method: "POST", headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ jsonrpc: "2.0", id: 1, method: "tools/call", params: { name: "lock_acquire", arguments: { name: "adoptguard", note: "e2e" } } }) });
      await sleep(500);
    }
    if (kind === "crash") {
      process.kill(proc.pid, "SIGKILL");
      await proc.exited;
      await sleep(500);
      check(alive(before[0]), "the stand-in outlives the killed hub (as Claude Code does)");
      proc = spawnHub(2);
      check(await up(), "a new hub is up");
    } else {
      c.send("turn.start", { thread: D, text: "mode:deploy go", msg: "d-1" });
      const swapped = await until(60000, () => read(join(prefix, "LICENSE")) === "new" ? true : undefined);
      check(!!swapped, "the deploy swaps the package in");
      await sleep(1500);
      check(await up(), "the hub is up again after its restart in place");
      check(proc.exitCode === null, "the hub's process is the same one (an exec, not a new process)");
    }
    const tRestart = Date.now();
    c = await connect();
    await sleep(4000);
    const live = pidsOf(S).filter(alive);
    check(live.length === 1, `exactly one stand-in process for S after the restart (${live}; all ${pidsOf(S)})`);
    if (kind === "on") {
      check(live[0] === before[0], "it is the one that was streaming (adopted)");
      check(pidsOf(S).length === 1, `no other process was started for S (no --resume; ${pidsOf(S)})`);
      const after = await until(15000, () => deltas.find((d) => d.thread === S && d.at > tRestart));
      check(!!after, "S's text keeps streaming to a client after the restart");
      const done = await until(60000, () => seen.find((x) => x.$ === "MessagePosted" && x.thread === S && String(x.text).includes(`w${N - 1}`)));
      check(!!done, "S's whole reply lands in its thread");
      const ended = await until(15000, () => seen.find((x) => x.$ === "TurnChanged" && x.thread === S && x.state === "completed"));
      check(!!ended, "and its turn ends");
      check(!seen.find((x) => x.$ === "MessagePosted" && x.thread === S && String(x.text).startsWith("Backplane restarted while you were working")), "S was not told to continue (it never stopped)");
      check(!read(log).includes(`EOF ${before[0]}`), "the stand-in never saw its stdin end");
    } else {
      check(!alive(before[0]) && read(log).includes(`TERM ${before[0]}`), "the streaming stand-in was stopped");
      check(!!(await until(5000, () => (!existsSync(`/proc/${before[0]}`) ? true : undefined))), "and reaped: no zombie is left");
      check(live.length === 1 && live[0] !== before[0], "S carries on in one new process");
    }
    if (kind !== "crash") {
      const none = await until(15000, () => (zombies(proc.pid).length === 0 ? true : undefined));
      check(!!none, `no child of the hub is left a zombie (${zombies(proc.pid)})`);
    }
    const lines = read(log).split("\n").filter((l) => l.startsWith("START"));
    console.log(`   (${lines.length} stand-ins started in all)`);
  } finally {
    try { process.kill(proc.pid, "SIGKILL"); } catch {}
    await proc.exited;
    // the stand-ins this run started (their pids from its log), never others
    for (const m of read(log).matchAll(/^START (\d+)/gm)) { try { process.kill(Number(m[1]), "SIGKILL"); } catch {} }
    if (process.env.KEEP) console.log(root); else rmSync(root, { recursive: true, force: true });
  }
}

const only = process.env.ADOPT_ONLY ?? ""; // "on", "off" or "crash": one scenario
for (const k of ["on", "off", "crash"] as const) if (!only || only === k) await scenario(k);
console.log(fail ? `${fail} failed` : "all ok");
process.exit(fail ? 1 : 0);
