// A turn of a provider that is not adopted across a restart (Codex here;
// Grok, Muse and Antigravity take the same path) cut by a restart in place,
// or by a crash, end to end. A headless hub installed in a
// temp prefix, with the stand-in `codex` (test/tools/codex_mcp_standin.ts)
// and the stand-in `claude` (test/tools/claude_standin.ts):
//   - thread Z (Claude) streams a long turn;
//   - thread C (Codex) opens an MCP tool call, thread_wait on Z, and once
//     it returns keeps its turn running;
//   - thread D (Claude) calls deploy_restart with a staged package: the
//     hub waits for C's call (deploy_mcp_e2e.ts), then swaps it in and
//     restarts in place (exec), cutting C's turn.
// Afterwards C's old process was stopped, exactly one new one resumed C's
// session, told that Backplane restarted and that a tool call in flight
// lost its result, and C's turn ends. Then the same for a crash: the hub
// is killed (SIGKILL) while thread C2 is in its tool call, and the hub
// that starts next stops C2's old process before resuming it once.
//
//   bun test/tools/resume_e2e.ts [BINARY] [WIREDIR]
import { chmodSync, copyFileSync, existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const [bin = "build/backplane", wire = "build/wire"] = process.argv.slice(2);
for (const f of readdirSync(wire).filter((f) => f.endsWith(".js"))) (0, eval)(readFileSync(`${wire}/${f}`, "utf8"));
const W = (globalThis as any).Wire;
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
let fail = 0;
const check = (ok: boolean, what: string) => { console.log(`${ok ? "ok  " : "FAIL"} ${what}`); if (!ok) fail++; };
const until = async <T>(ms: number, f: () => T | undefined) => {
  const end = Date.now() + ms;
  for (;;) { const v = f(); if (v) return v; if (Date.now() > end) return undefined; await sleep(100); }
};
const alive = (pid: number) => {
  try { process.kill(pid, 0); } catch { return false; }
  try { return !/^\d+ \(.*\) Z/.test(readFileSync(`/proc/${pid}/stat`, "utf8")); } catch { return false; }
};
const read = (p: string) => (existsSync(p) ? readFileSync(p, "utf8") : "");
const freePort = () => { const s = Bun.listen({ hostname: "127.0.0.1", port: 0, socket: { data() {} } }); const p = s.port; s.stop(true); return p; };

const root = mkdtempSync(join(tmpdir(), "bp-resume-"));
const fake = join(root, "bin"), clog = join(root, "claude.log"), xlog = join(root, "codex.log"), home = join(root, "home"), proj = join(root, "proj"), prefix = join(root, "prefix");
for (const d of [fake, home, proj, prefix, join(prefix, "web")]) mkdirSync(d);
writeFileSync(join(fake, "claude"), `#!/bin/sh\nexec ${process.execPath} ${resolve("test/tools/claude_standin.ts")} "$@"\n`);
writeFileSync(join(fake, "codex"), `#!/bin/sh\nexec ${process.execPath} ${resolve("test/tools/codex_mcp_standin.ts")} "$@"\n`);
for (const n of ["grok", "muse"]) writeFileSync(join(fake, n), "#!/bin/sh\nexit 1\n");
for (const n of ["claude", "codex", "grok", "muse"]) chmodSync(join(fake, n), 0o755);
writeFileSync(join(home, "events.jsonl"), "");
copyFileSync(resolve(bin), join(prefix, "backplane"));
chmodSync(join(prefix, "backplane"), 0o755);
writeFileSync(join(prefix, "LICENSE"), "orig");
const stage = join(root, "stage");
mkdirSync(join(stage, "web"), { recursive: true });
copyFileSync(resolve(bin), join(stage, "backplane"));
chmodSync(join(stage, "backplane"), 0o755);
writeFileSync(join(stage, "LICENSE"), "new");
const events = () => read(xlog).split("\n").filter(Boolean).map((l) => JSON.parse(l));

const PORT = freePort();
if (PORT === 3787) throw new Error("refusing the live port");
const spawnHub = (n: number) => Bun.spawn([join(prefix, "backplane"), "--home", home, "--port", String(PORT), "--no-tailscale", "--foreground"], {
  env: { ...process.env, DISPLAY: "", WAYLAND_DISPLAY: "", BACKPLANE_NO_UPDATE: "1", BACKPLANE_ORDER_SERVER: "http://127.0.0.1:9", BACKPLANE_KEYRING_LOCKED: "0", BACKPLANE_PEERS: "", BACKPLANE_GOOGLE_BAKE: "0",
    FAKE_LOG: clog, RIDE_LOG: xlog, BP_EOF_FINISH: "1", BP_STREAM_N: "60", BP_STREAM_MS: "100", BP_DEPLOY_ARGS: JSON.stringify({ stage, note: "resume e2e" }),
    BACKPLANE_WAKE_TICK_MS: "2000", BACKPLANE_DEPLOY_LOCKS: join(root, "none.lock"), BACKPLANE_LOCK_DIR: join(root, "locks"),
    BACKPLANE_AGENTS: fake, PATH: `${fake}:${process.env.PATH}`, TMPDIR: root },
  stdout: "ignore", stderr: Bun.file(join(root, `hub${n}.err`)),
});
let proc = spawnHub(1);
mkdirSync(join(root, "locks"));
const up = async () => {
  for (let i = 0; i < 200; i++) { try { if ((await fetch(`http://127.0.0.1:${PORT}/hello`)).status === 200) return true; } catch {} await sleep(100); }
  return false;
};
const seen: any[] = [];
const connect = async () => {
  const ws = new WebSocket(`ws://127.0.0.1:${PORT}/ws`);
  ws.binaryType = "arraybuffer";
  ws.onmessage = (e) => {
    let o: any = null;
    try { o = JSON.parse(typeof e.data === "string" ? (e.data as string) : W.decode(new Uint8Array(e.data as ArrayBuffer))); } catch {}
    for (const c of o?.items ?? []) seen.push(typeof c === "string" ? JSON.parse(c) : c);
  };
  await new Promise((r, j) => { ws.onopen = r; ws.onerror = j; });
  let id = 0;
  return { ws, send: (m: string, p: object) => ws.send(W.encode(JSON.stringify({ id: ++id, m, p }))) };
};
try {
  check(await up(), "the hub is up");
  let c = await connect();
  c.send("project.add", { path: proj });
  const pc: any = await until(10000, () => seen.find((x) => x.$ === "ProjectCreated"));
  const made = async (title: string, provider: string) => {
    await sleep(800);
    c.send("thread.create", { project: pc.id, title, env: "local", provider });
    return (await until(10000, () => seen.find((x) => x.$ === "ThreadCreated" && x.title === title)))?.id as string;
  };
  const Z = await made("sleeper", "claude");
  const C = await made("cut", "codex");
  const D = await made("deployer", "claude");
  const turns = (th: string) => seen.filter((x) => x.$ === "TurnChanged" && x.thread === th).map((x) => String(x.state));
  c.send("turn.start", { thread: Z, text: "mode:stream go w", msg: "z-1" });
  await until(15000, () => turns(Z).includes("running") ? true : undefined);
  c.send("turn.start", { thread: C, text: `ride-cut:${Z} go`, msg: "c-1" });
  const first: any = await until(15000, () => events().find((e) => e.ev === "start" && String(e.prompt).includes("ride-cut")));
  check(!!first, "the codex turn runs");
  await sleep(1500);
  c.send("turn.start", { thread: D, text: "mode:deploy go", msg: "d-1" });
  const swapped = await until(60000, () => read(join(prefix, "LICENSE")) === "new" ? true : undefined);
  check(!!swapped, "the deploy swaps the package in and restarts in place, mid-turn, once its tool call returned");
  await sleep(1500);
  check(await up(), "the hub is up again");
  check(proc.exitCode === null, "the same process (an exec)");
  c = await connect();
  const resumed: any = await until(30000, () => events().find((e) => e.ev === "start" && e.pid !== first.pid));
  check(!!resumed, "a new codex process started for C");
  check(!alive(first.pid), `the cut process is gone (${first.pid})`);
  const args: string[] = resumed?.args ?? [];
  check(args.includes("resume") && args.includes("cx-ride"), `it resumed C's session (${args.slice(0, 4).join(" ")})`);
  const p = String(resumed?.prompt ?? "");
  check(p.includes("Backplane restarted while you were working"), "it was told Backplane restarted");
  check(/tool call was running when it stopped, its result was lost/.test(p), `and that a tool call in flight lost its result (${p.slice(0, 300)})`);
  const done = await until(30000, () => (turns(C).at(-1) === "completed" ? true : undefined));
  check(!!done, `C's turn ends (${turns(C).join(",")})`);
  await sleep(2000);
  const starts = events().filter((e) => e.ev === "start").length;
  check(starts === 2, `exactly one new process (${starts} in all)`);

  // a crash: the record, not a handoff, finds the process still running
  const Z2 = await made("sleeper2", "claude");
  c.send("turn.start", { thread: Z2, text: "mode:stream go w", msg: "z2-1" });
  await until(15000, () => turns(Z2).includes("running") ? true : undefined);
  const C2 = await made("crash", "codex");
  c.send("turn.start", { thread: C2, text: `ride-cut:${Z2} again`, msg: "c2-1" });
  const old: any = await until(15000, () => events().find((e) => e.ev === "start" && String(e.prompt).includes("ride-cut") && String(e.prompt).includes("again")));
  check(!!old, "C2's codex turn runs");
  await sleep(1500);
  process.kill(proc.pid, "SIGKILL");
  await proc.exited;
  await sleep(500);
  check(alive(old.pid), "its process outlives the killed hub");
  proc = spawnHub(2);
  check(await up(), "a new hub is up");
  c = await connect();
  const again: any = await until(30000, () => events().find((e) => e.ev === "start" && e.pid !== old.pid && e.pid !== first.pid && e.pid !== resumed?.pid));
  check(!!again && (again.args ?? []).includes("resume"), "C2 resumed in a new process");
  check(events().some((e) => e.ev === "term" && e.pid === old.pid) && !alive(old.pid), "after its old process was stopped");
  const n2 = events().filter((e) => e.ev === "start").length;
  await sleep(2000);
  check(events().filter((e) => e.ev === "start").length === n2 && n2 === 4, `one new process for C2 (${n2} in all)`);
} finally {
  try { process.kill(proc.pid, "SIGKILL"); } catch {}
  await proc.exited;
  // the stand-ins this run started (their pids from its logs), never others
  for (const e of events()) if (e.ev === "start") { try { process.kill(Number(e.pid), "SIGKILL"); } catch {} }
  for (const m of read(clog).matchAll(/^START (\d+)/gm)) { try { process.kill(Number(m[1]), "SIGKILL"); } catch {} }
  if (process.env.KEEP) console.log(root); else rmSync(root, { recursive: true, force: true });
}
console.log(fail ? `${fail} failed` : "all ok");
process.exit(fail ? 1 : 0);
