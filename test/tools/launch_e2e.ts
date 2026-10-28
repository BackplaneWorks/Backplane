// Launched threads report back, end to end (hub.bend's Launch.tell). A
// headless hub on a throwaway home with the stand-in `claude`
// (test/tools/claude_standin.ts, mode "launch": thread_launch, then end):
//   1. t1 launches a child that answers at once: t1 gets one report naming
//      the child, its state and its reply, and a turn starts for it.
//   2. t2 launches a child and holds its own turn 4 s: the report waits in
//      t2's queue and starts t2's next turn once the first ends.
//   3. t3 launches with report false: nothing comes back.
//   4. t4 launches a child, the hub is killed while the child works and
//      started again: the child's turn ends (interrupted by the restart),
//      and t4 hears of it once.
//
//   bun test/tools/launch_e2e.ts [BINARY] [WIREDIR]
import { chmodSync, existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const [bin = "build/backplane", wire = "build/wire"] = process.argv.slice(2);
for (const f of readdirSync(wire).filter((f) => f.endsWith(".js"))) (0, eval)(readFileSync(`${wire}/${f}`, "utf8"));
const W = (globalThis as any).Wire;
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

const root = mkdtempSync(join(tmpdir(), "bp-launch-"));
const fake = join(root, "bin");
const log = join(root, "claude.log");
mkdirSync(fake);
writeFileSync(join(fake, "claude"), `#!/bin/sh\nexec ${process.execPath} ${resolve("test/tools/claude_standin.ts")} "$@"\n`);
chmodSync(join(fake, "claude"), 0o755);
for (const n of ["codex", "grok", "muse"]) {
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
    await sleep(50);
  }
};
let fail = 0;
const check = (ok: boolean, what: string) => {
  console.log(`${ok ? "ok  " : "FAIL"} ${what}`);
  if (!ok) fail++;
};

const port = freePort();
type Hub = { proc: ReturnType<typeof Bun.spawn> };
let launchArgs = "{}";
let childPrompt = "mode:plain child work";
const start = async (): Promise<Hub> => {
  const proc = Bun.spawn([resolve(bin), "--home", home, "--port", String(port), "--no-tailscale", "--foreground"], {
    env: { ...process.env, DISPLAY: "", WAYLAND_DISPLAY: "", BACKPLANE_NO_UPDATE: "1", BACKPLANE_ORDER_SERVER: "http://127.0.0.1:9", BACKPLANE_KEYRING_LOCKED: "0", BACKPLANE_PEERS: "", BACKPLANE_GOOGLE_BAKE: "0", FAKE_LOG: log,
      BP_WORK_MS: "300", BP_CHILD_PROMPT: childPrompt, BP_LAUNCH_ARGS: launchArgs, BP_LAUNCH_HOLD_MS: process.env.HOLD ?? "0", BACKPLANE_AGENTS: fake, PATH: `${fake}:${process.env.PATH}` },
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
const restart = async () => {
  if (hub) await kill(hub);
  hub = await start();
  c = await connect();
};
let c: Client;
try {
  hub = await start();
  c = await connect();
  const changes = () => c.seen;
  const turns = (th: string) => changes().filter((x) => x.$ === "TurnChanged" && x.thread === th).map((x) => String(x.state));
  const reports = (th: string) => changes().filter((x) => x.$ === "MessagePosted" && x.thread === th && String(x.text).startsWith("[Backplane] Launched thread"));
  const launchedBy = (by: string) => changes().find((x) => x.$ === "ThreadLaunched" && x.by === by);
  c.send("project.add", { path: proj });
  let pc: any = await until(10000, () => changes().find((x) => x.$ === "ProjectCreated"));
  // (a reconnect replays the whole log, so threads are found by title)
  const made = async (title: string) => {
    await sleep(1500);
    c.send("thread.create", { project: pc.id, title, env: "local", provider: "claude" });
    return (await until(10000, () => changes().find((x) => x.$ === "ThreadCreated" && x.title === title)))?.id as string;
  };

  // 1. a quick child
  const t1 = await made("boss one");
  c.send("turn.start", { thread: t1, text: "mode:launch go", msg: "l-1" });
  const l1 = await until(15000, () => launchedBy(t1));
  check(!!l1 && l1.report === true, `t1's child records its launcher (${l1?.id})`);
  await until(20000, () => reports(t1).length ? true : undefined);
  await sleep(1500);
  const r1 = reports(t1);
  check(r1.length === 1, `t1 gets one report (${r1.length})`);
  check(!!r1[0] && String(r1[0].text).includes(`(${l1?.id}) completed`) && String(r1[0].text).includes("REPLY mode:plain child work"),
    `the report names the child, its state and its reply (${String(r1[0]?.text).slice(0, 120)})`);
  check(!!r1[0] && String(r1[0].msg).startsWith(`t:${l1?.id}:lr-`), "its id names the child");
  await until(10000, () => turns(t1).filter((x) => x === "running").length >= 2 ? true : undefined);
  check(turns(t1).filter((x) => x === "running").length >= 2, `the report starts t1's turn (${turns(t1).join(",")})`);

  // 2. the launcher is still working: the report waits behind its turn
  launchArgs = "{}";
  process.env.HOLD = "4000";
  await restart();
  const t2 = await made("boss two");
  c.send("turn.start", { thread: t2, text: "mode:launch go", msg: "l-2" });
  const l2 = await until(15000, () => launchedBy(t2));
  const q = await until(20000, () => changes().find((x) => x.$ === "TurnQueued" && x.thread === t2 && String(x.text).startsWith("[Backplane] Launched thread")));
  check(!!q, "t2's report waits in its queue while it works");
  await until(20000, () => turns(t2).filter((x) => x === "running").length >= 2 ? true : undefined);
  const t2turns = turns(t2);
  check(t2turns.filter((x) => x === "running").length >= 2 && t2turns.indexOf("completed") < t2turns.lastIndexOf("running"), `and runs once t2's turn ends (${t2turns.join(",")})`);
  check(reports(t2).length === 1 && !!l2, `t2 gets one report (${reports(t2).length})`);

  // 3. report false
  process.env.HOLD = "0";
  launchArgs = JSON.stringify({ report: false });
  await restart();
  const t3 = await made("boss three");
  c.send("turn.start", { thread: t3, text: "mode:launch go", msg: "l-3" });
  const l3 = await until(15000, () => launchedBy(t3));
  check(!!l3 && l3.report === false, "t3's child is launched without reports");
  await until(15000, () => l3 && turns(l3.id).at(-1) === "completed" ? true : undefined);
  await sleep(1500);
  check(!!l3 && turns(l3.id).at(-1) === "completed" && reports(t3).length === 0, `nothing comes back to t3 (${reports(t3).length})`);

  // 4. a restart while the child works
  launchArgs = "{}";
  childPrompt = "mode:stop child work";
  await restart();
  const t4 = await made("boss four");
  c.send("turn.start", { thread: t4, text: "mode:launch go", msg: "l-4" });
  const l4 = await until(15000, () => launchedBy(t4));
  await until(15000, () => l4 && turns(l4.id).includes("running") ? true : undefined);
  await sleep(800);
  c.send("setting.set", { key: "restart.continue", value: "off" });
  await sleep(500);
  await restart();
  await until(20000, () => reports(t4).length ? true : undefined);
  await sleep(1500);
  check(reports(t4).length === 1 && String(reports(t4)[0]?.text).includes("was interrupted"), `after a restart t4 hears its child was interrupted, once (${reports(t4).length}: ${String(reports(t4)[0]?.text).slice(0, 90)})`);
} finally {
  if (hub) await kill(hub);
  if (process.env.KEEP) console.log(root); else rmSync(root, { recursive: true, force: true });
}
console.log(fail ? `${fail} failed` : "all ok");
process.exit(fail ? 1 : 0);
