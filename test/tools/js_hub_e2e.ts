// The whole hub on Bun's JS runtime, end to end, on an empty home kept on
// disk (build/js_hub_e2e, E2E_ROOT to move it): the effects the C hub gets
// from host.c, hublock.c and adopt.c, through their JS twins.
//   - /hello answers, the web client is served from --web, a WebSocket joins
//   - a project and a thread are made; a thread asked for a worktree gets
//     one from git (Proc.run: posix_spawn, a pipe, the exit status)
//   - Sock.peer: the loopback lock API (it trusts only 127.*) answers
//   - a deploy restart in place (deploy_restart from a stand-in agent):
//     the package is swapped, the hub's PID stays (an exec), the streaming
//     stand-in is the same process afterwards (its fds handed over,
//     adopt.js), its text keeps coming and its turn ends
//   - the home's flock is held across the exec (hublock.js hands its fd in
//     BACKPLANE_HUB_LOCK_FD): flock -n fails, and a second hub on the home
//     exits 1 saying a hub already serves it
//
//   bun test/tools/js_hub_e2e.ts HUB [WIREDIR]      (port 3824, E2E_PORT)
// HUB is the server emitted as JS (bend src/server/main.bend -o hub.js) or
// the single file `bun build --compile` makes of it (scripts/build-js.sh).
import { chmodSync, copyFileSync, existsSync, mkdirSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";

const [hub, wire = "build/wire"] = process.argv.slice(2);
if (!hub) { console.log("usage: bun test/tools/js_hub_e2e.ts HUB [WIREDIR]"); process.exit(2); }
for (const f of readdirSync(wire).filter((f) => f.endsWith(".js"))) (0, eval)(readFileSync(`${wire}/${f}`, "utf8"));
const W = (globalThis as any).Wire;
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
const PORT = Number(process.env.E2E_PORT ?? 3824);
if (PORT === 3787) throw new Error("refusing the live port");

let fail = 0;
const check = (ok: boolean, what: string) => {
  console.log(`${ok ? "ok  " : "FAIL"} ${what}`);
  if (!ok) fail++;
};
const until = async <T>(ms: number, f: () => T | undefined | Promise<T | undefined>) => {
  const end = Date.now() + ms;
  for (;;) {
    const v = await f();
    if (v) return v;
    if (Date.now() > end) return undefined;
    await sleep(100);
  }
};
const alive = (pid: number) => {
  try { process.kill(pid, 0); } catch { return false; }
  try { return !/^\d+ \(.*\) Z/.test(readFileSync(`/proc/${pid}/stat`, "utf8")); } catch { return false; }
};
const read = (p: string) => (existsSync(p) ? readFileSync(p, "utf8") : "");
const run = (cmd: string[], cwd?: string) => Bun.spawnSync(cmd, { cwd, stdout: "pipe", stderr: "pipe" });

try { if ((await fetch(`http://127.0.0.1:${PORT}/hello`)).status === 200) { console.log(`port ${PORT} is in use`); process.exit(2); } } catch {}

const root = resolve(process.env.E2E_ROOT ?? "build/js_hub_e2e");
if (root === "/" || !root.includes("js_hub_e2e")) throw new Error(`refusing root ${root}`);
rmSync(root, { recursive: true, force: true });
const fake = join(root, "bin"), log = join(root, "claude.log"), home = join(root, "home"), proj = join(root, "proj"), prefix = join(root, "prefix"), stage = join(root, "stage"), web = join(root, "web");
for (const d of [fake, home, proj, prefix, join(stage, "web"), web, join(root, "locks"), join(root, "tmp")]) mkdirSync(d, { recursive: true });

// one executable file, as a package holds it
const exe = join(root, "backplane");
if (hub.endsWith(".js")) {
  const b = run([process.execPath, "build", "--compile", resolve(hub), "--outfile", exe]);
  if (b.exitCode !== 0) { console.log(String(b.stderr)); process.exit(1); }
} else copyFileSync(resolve(hub), exe);
chmodSync(exe, 0o755);
copyFileSync(exe, join(prefix, "backplane"));
copyFileSync(exe, join(stage, "backplane"));
chmodSync(join(prefix, "backplane"), 0o755);
chmodSync(join(stage, "backplane"), 0o755);
writeFileSync(join(prefix, "LICENSE"), "orig");
writeFileSync(join(stage, "LICENSE"), "new");
writeFileSync(join(web, "index.html"), "<!doctype html><title>js-hub-e2e-web</title>");

writeFileSync(join(fake, "claude"), `#!/bin/sh\nexec ${process.execPath} ${resolve("test/tools/claude_standin.ts")} "$@"\n`);
chmodSync(join(fake, "claude"), 0o755);
for (const n of ["codex", "grok", "muse"]) { writeFileSync(join(fake, n), "#!/bin/sh\nexit 1\n"); chmodSync(join(fake, n), 0o755); }
writeFileSync(join(home, "events.jsonl"), "");
// a repository for the worktree
writeFileSync(join(proj, "README"), "e2e\n");
for (const c of [["git", "init", "-q", "-b", "main"], ["git", "add", "README"], ["git", "-c", "user.name=e2e", "-c", "user.email=e2e@example.invalid", "commit", "-q", "-m", "first"]]) run(c, proj);

const N = 200, MS = 100; // a 20 s turn
const env = {
  ...process.env, DISPLAY: "", WAYLAND_DISPLAY: "", BACKPLANE_NO_UPDATE: "1", BACKPLANE_TELEMETRY: "off", BACKPLANE_KEYRING_LOCKED: "0", BACKPLANE_PEERS: "",
  BACKPLANE_GOOGLE_BAKE: "0", FAKE_LOG: log, BP_EOF_FINISH: "1", BP_STREAM_N: String(N), BP_STREAM_MS: String(MS),
  BP_DEPLOY_ARGS: JSON.stringify({ stage, note: "js hub e2e" }), BACKPLANE_WAKE_TICK_MS: "2000", BACKPLANE_DEPLOY_LOCKS: join(root, "none.lock"),
  BACKPLANE_LOCK_DIR: join(root, "locks"), BACKPLANE_AGENTS: fake, PATH: `${fake}:${process.env.PATH}`, TMPDIR: join(root, "tmp"),
};
delete (env as any).BACKPLANE_HOME;
const args = (port: number) => ["--home", home, "--port", String(port), "--no-tailscale", "--foreground", "--web", web];
const proc = Bun.spawn([join(prefix, "backplane"), ...args(PORT)], { env, stdout: Bun.file(join(root, "hub.out")), stderr: Bun.file(join(root, "hub.err")) });
// /hello answers CBOR
const hello = async () => { try { const r = await fetch(`http://127.0.0.1:${PORT}/hello`); return r.status === 200 ? JSON.parse(W.decode(new Uint8Array(await r.arrayBuffer()))) : undefined; } catch { return undefined; } };

try {
  const h0: any = await until(30000, hello);
  check(!!h0, `/hello answers (${JSON.stringify(h0)?.slice(0, 120)})`);
  const page = await fetch(`http://127.0.0.1:${PORT}/`).then((r) => r.text()).catch(() => "");
  check(page.includes("js-hub-e2e-web"), "the web client is served from --web");

  const seen: any[] = [];
  const deltas: { thread: string; at: number }[] = [];
  const connect = async () => {
    const ws = new WebSocket(`ws://127.0.0.1:${PORT}/ws`);
    ws.binaryType = "arraybuffer";
    ws.onmessage = (e) => {
      let o: any = null;
      try { o = JSON.parse(typeof e.data === "string" ? (e.data as string) : W.decode(new Uint8Array(e.data as ArrayBuffer))); } catch {}
      if (o?.t === "delta") deltas.push({ thread: o.thread, at: Date.now() });
      for (const c of o?.items ?? []) seen.push(typeof c === "string" ? JSON.parse(c) : c);
    };
    await new Promise((r, j) => { ws.onopen = r; ws.onerror = j; });
    let id = 0;
    return { ws, send: (m: string, p: object) => ws.send(W.encode(JSON.stringify({ id: ++id, m, p }))) };
  };
  let c = await connect();
  check(true, "a WebSocket client joins");
  c.send("project.add", { path: proj });
  const pc: any = await until(15000, () => seen.find((x) => x.$ === "ProjectCreated" && x.root === proj));
  check(!!pc, "a project is added");
  const made = async (title: string, wt: string) => {
    await sleep(500);
    c.send("thread.create", { project: pc?.id, title, env: wt, provider: "claude" });
    return (await until(15000, () => seen.find((x) => x.$ === "ThreadCreated" && x.title === title)))?.id as string;
  };
  const S = await made("streamer", "local");
  const D = await made("deployer", "local");
  check(!!S && !!D, "threads are created");
  const G = await made("branched", "worktree");
  // made at the thread's first turn
  c.send("turn.start", { thread: G, text: "hello", msg: "g-1" });
  if (process.env.E2E_HOLD) await sleep(Number(process.env.E2E_HOLD));
  const wt: any = await until(10000, () => seen.find((x) => x.$ === "WorktreeSet" && x.id === G && x.worktree));
  // advisory: main's C hub records no WorktreeSet here either (J1, 2026-10-21)
  const wts = run(["git", "worktree", "list"], proj);
  check(String(wts.stdout).includes("/.backplane/worktrees/"), "git makes a worktree for the thread (Proc.run)");
  console.log(`     WorktreeSet recorded: ${!!wt} (advisory, as C)`);

  const tok = read(join(home, "token")).trim();
  const lockList = await fetch(`http://127.0.0.1:${PORT}/lock/list`, { method: "POST", body: "{}" }).then((r) => r.status).catch(() => 0);
  check(lockList !== 403 && lockList !== 0, `a loopback client is trusted by the lock API (Sock.peer; ${lockList})`);

  c.send("turn.start", { thread: S, text: "mode:stream go w", msg: "s-1" });
  const pidsOf = (th: string) => [...read(log).matchAll(/^MCP (\d+) \S*\/mcp\/([^.?/]+)/gm)].filter((m) => m[2] === th).map((m) => Number(m[1]));
  await until(20000, () => deltas.find((d) => d.thread === S));
  const before = pidsOf(S);
  check(before.length === 1 && alive(before[0]), `one stand-in streams for S (${before})`);
  await fetch(`http://127.0.0.1:${PORT}/mcp/${S}?token=${tok}`, { method: "POST", headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ jsonrpc: "2.0", id: 1, method: "tools/call", params: { name: "lock_acquire", arguments: { name: "jsguard", note: "e2e" } } }) });
  await sleep(1000);

  c.send("turn.start", { thread: D, text: "mode:deploy go", msg: "d-1" });
  const swapped = await until(90000, () => (read(join(prefix, "LICENSE")) === "new" ? true : undefined));
  check(!!swapped, "the deploy swaps the package in");
  // the new image has read the log again ("replayed" twice) and answers
  const h1: any = await until(60000, async () => (read(join(root, "hub.err")).split("info hub: replayed").length === 3 ? await hello() : undefined));
  check(!!h1, "the hub answers again after its restart in place");
  const tRestart = Date.now();
  c = await connect();
  check(proc.exitCode === null, "the hub's process is the same one (an exec, not a new process)");
  check(read(join(root, "hub.err")).split("restarting in place").length === 2, "it restarted in place once");

  const held = run(["flock", "-n", "-E", "9", join(home, "hub.lock"), "true"]);
  check(held.exitCode === 9, `the home's flock is still held after the exec (flock -n: ${held.exitCode})`);
  const second = Bun.spawnSync([exe, ...args(PORT + 1)], { env, stdout: "pipe", stderr: "pipe", timeout: 30000 });
  check(second.exitCode === 1 && String(second.stderr).includes("a hub already serves"), `a second hub on the home exits 1 (${second.exitCode}: ${String(second.stderr).trim().slice(0, 100)})`);

  await sleep(1000);
  const live = pidsOf(S).filter(alive);
  check(live.length === 1 && live[0] === before[0], `the streaming stand-in is the same process (adopted; ${live}, all ${pidsOf(S)})`);
  check(!!(await until(15000, () => deltas.find((d) => d.thread === S && d.at > tRestart))), "its text keeps streaming after the restart");
  const done = await until(60000, () => seen.find((x) => x.$ === "MessagePosted" && x.thread === S && String(x.text).includes(`w${N - 1}`)));
  check(!!done, "its whole reply lands in the thread");
  check(!!(await until(15000, () => seen.find((x) => x.$ === "TurnChanged" && x.thread === S && x.state === "completed"))), "and its turn ends");
  check(!read(log).includes(`EOF ${before[0]}`), "the stand-in never saw its stdin end");
} finally {
  try { process.kill(proc.pid, "SIGKILL"); } catch {}
  await proc.exited;
  for (const m of read(log).matchAll(/^START (\d+)/gm)) { try { process.kill(Number(m[1]), "SIGKILL"); } catch {} }
  if (fail) console.log(`kept ${root} (hub.err there)`); else if (!process.env.KEEP) rmSync(root, { recursive: true, force: true });
}
console.log(fail ? `${fail} failed` : "all ok");
process.exit(fail ? 1 : 0);
