// Durable steps, end to end (item 4: one write per step, TaskDelivered
// after the parent's message, Task.repair at start-up, a torn last line).
// A headless hub with the stand-in `claude` (test/tools/claude_standin.ts):
//   1. a parent delegates a child (mode delegate); the hub is SIGKILLed (by
//      its PID) the moment the child's task is seen finishing, then started
//      again on the same home: the parent holds the child's result exactly
//      once and the task is delivered.
//   2. a torn last line: with the hub down, half a line is appended to the
//      log; the hub starts, writes a change, is stopped and started again:
//      the change after the fragment is read back.
//
//   bun test/tools/durable_e2e.ts [BINARY] [WIREDIR]
import { appendFileSync, chmodSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const [bin = "build/backplane", wire = "build/wire"] = process.argv.slice(2);
for (const f of readdirSync(wire).filter((f) => f.endsWith(".js"))) (0, eval)(readFileSync(`${wire}/${f}`, "utf8"));
const W = (globalThis as any).Wire;
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

const root = mkdtempSync(join(tmpdir(), "bp-durable-"));
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
    await sleep(25);
  }
};
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
      BP_WORK_MS: "300", BACKPLANE_AGENTS: fake, PATH: `${fake}:${process.env.PATH}` },
    stdout: "ignore",
    stderr: "ignore",
  });
  for (let i = 0; i < 200; i++) {
    try {
      if ((await fetch(`http://127.0.0.1:${port}/hello`)).status === 200) break;
    } catch {}
    await sleep(100);
  }
  return { proc };
};
const kill = async (h: Hub) => {
  process.kill(h.proc.pid, "SIGKILL");
  await h.proc.exited;
};
type Client = { ws: WebSocket; seen: any[]; send: (m: string, p: object) => void };
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
  return { ws, seen, send: (m, p) => ws.send(W.encode(JSON.stringify({ id: ++id, m, p }))) };
};

let hub: Hub | null = null;
try {
  hub = await start();
  let c = await connect();
  c.send("project.add", { path: proj });
  const pc = await until(10000, () => c.seen.find((x) => x.$ === "ProjectCreated"));
  c.send("setting.set", { key: "restart.continue", value: "off" });
  c.send("thread.create", { project: pc.id, title: "parent", env: "local", provider: "claude" });
  const parent = (await until(10000, () => c.seen.find((x) => x.$ === "ThreadCreated")))?.id as string;
  c.send("turn.start", { thread: parent, text: "mode:delegate go", msg: "d-1" });
  // the child's task finishing: kill the hub there and then
  const task = await until(20000, () => c.seen.find((x) => x.$ === "TaskDelegated"));
  const done = await until(20000, () => c.seen.find((x) => x.$ === "TaskStateSet" && x.task === task?.task && x.state === "done"));
  await kill(hub);
  hub = null;
  check(!!task && !!done, `the child's task finished before the kill (${task?.task})`);
  const expect = `t:${task?.child}:td-${task?.task}`;

  hub = await start();
  c = await connect();
  await until(5000, () => c.seen.length > 0 ? true : undefined);
  await sleep(1500);
  const results = c.seen.filter((x) => x.$ === "MessagePosted" && x.thread === parent && x.msg === expect);
  check(results.length === 1, `the parent holds the child's result once (${results.length})`);
  check(c.seen.some((x) => x.$ === "TaskDelivered" && x.task === task?.task), "the task is delivered");
  check(c.seen.filter((x) => x.$ === "TaskDelivered" && x.task === task?.task).length === 1, "and delivered once");

  // a torn last line
  await kill(hub);
  hub = null;
  appendFileSync(join(home, "events.jsonl"), '{"$":"SettingSet","key":"torn","val');
  hub = await start();
  c = await connect();
  await sleep(500);
  c.send("setting.set", { key: "after.torn", value: "yes" });
  await until(5000, () => c.seen.find((x) => x.$ === "SettingSet" && x.key === "after.torn"));
  await kill(hub);
  hub = null;
  hub = await start();
  c = await connect();
  const back = await until(5000, () => c.seen.find((x) => x.$ === "SettingSet" && x.key === "after.torn" && x.value === "yes"));
  check(!!back, "the change written after a torn line reads back");
  check(!c.seen.some((x) => x.$ === "SettingSet" && x.key === "torn"), "the torn fragment is left out");
} finally {
  if (hub) await kill(hub);
  if (process.env.KEEP) console.log(root); else rmSync(root, { recursive: true, force: true });
}
console.log(fail ? `${fail} failed` : "all ok");
process.exit(fail ? 1 : 0);
