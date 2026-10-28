// Lost work, end to end (item 5: hub.bend's Bg.lost, Rec.lost, Hub.carry,
// Rec.release; the install guard). A headless hub with the stand-in
// `claude` (test/tools/claude_standin.ts):
//   1. t1 leaves a background shell running after its turn; t2 delegates a
//      child that is still working. /hello says busy and
//      `scripts/dev.sh install` refuses. The hub is SIGKILLed (its PID) and
//      started again: t1 runs a turn whose message names its task and the
//      restart, and completes; t2's child carries on, its task stays open,
//      and the parent gets the child's result once.
//   2. t3 leaves a shell running, its model changes and a message follows:
//      t3 is told the work was lost to the replacement, and the new
//      process's own shell stays listed.
//   3. with restart.continue off, t4's lost shell is only logged (an Act
//      "lost"), nothing starts, and its row stays with outcome unknown.
//
//   bun test/tools/restart_work_e2e.ts [BINARY] [WIREDIR]
import { chmodSync, existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const [bin = "build/backplane", wire = "build/wire"] = process.argv.slice(2);
for (const f of readdirSync(wire).filter((f) => f.endsWith(".js"))) (0, eval)(readFileSync(`${wire}/${f}`, "utf8"));
const W = (globalThis as any).Wire;
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

const root = mkdtempSync(join(tmpdir(), "bp-lost-"));
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
    env: { ...process.env, DISPLAY: "", WAYLAND_DISPLAY: "", BACKPLANE_NO_UPDATE: "1", BACKPLANE_ORDER_SERVER: "http://127.0.0.1:9", BACKPLANE_KEYRING_LOCKED: "0", BACKPLANE_PEERS: "", BACKPLANE_GOOGLE_BAKE: "0", FAKE_LOG: log,
      BP_WORK_MS: "300", BP_BG_MS: "600000", BP_CHILD_PROMPT: "mode:stop child work", BACKPLANE_AGENTS: fake, PATH: `${fake}:${process.env.PATH}` },
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
type Client = { ws: WebSocket; seen: { c: any; t: number }[]; send: (m: string, p: object) => void };
const connect = async (): Promise<Client> => {
  const seen: { c: any; t: number }[] = [];
  const ws = new WebSocket(`ws://127.0.0.1:${port}/ws`);
  ws.binaryType = "arraybuffer";
  ws.onmessage = (e) => {
    let o: any = null;
    try { o = JSON.parse(typeof e.data === "string" ? (e.data as string) : W.decode(new Uint8Array(e.data as ArrayBuffer))); } catch {}
    for (const c of o?.items ?? []) seen.push({ c, t: Date.now() });
  };
  await new Promise((r, j) => { ws.onopen = r; ws.onerror = j; });
  let id = 0;
  return { ws, seen, send: (m, p) => ws.send(W.encode(JSON.stringify({ id: ++id, m, p }))) };
};
const busy = async () => {
  const b = new Uint8Array(await (await fetch(`http://127.0.0.1:${port}/hello`)).arrayBuffer());
  const k = [0x64, 0x62, 0x75, 0x73, 0x79];
  for (let i = 0; i + 5 < b.length; i++) if (k.every((x, j) => b[i + j] === x)) return b[i + 5] < 24 ? b[i + 5] : -1;
  return 0;
};

let hub: Hub | null = null;
try {
  hub = await start();
  let c = await connect();
  const changes = () => c.seen.map((x) => x.c);
  const turns = (th: string) => changes().filter((x) => x.$ === "TurnChanged" && x.thread === th).map((x) => String(x.state));
  const rows = (th: string) => { const w = changes().filter((x) => x.$ === "WorkSet" && x.thread === th && x.kind === "bg"); return w.length ? String(w[w.length - 1].rows) : ""; };
  c.send("project.add", { path: proj });
  const pc = await until(10000, () => changes().find((x) => x.$ === "ProjectCreated"));
  const made = async (title: string) => {
    const n = changes().filter((x) => x.$ === "ThreadCreated").length;
    c.send("thread.create", { project: pc.id, title, env: "local", provider: "claude" });
    return (await until(10000, () => changes().filter((x) => x.$ === "ThreadCreated")[n]))?.id as string;
  };

  // 1. a shell left running, a child still working
  const t1 = await made("bg");
  c.send("turn.start", { thread: t1, text: "mode:bg start", msg: "l-1" });
  await until(10000, () => turns(t1).at(-1) === "completed" && rows(t1) ? true : undefined);
  check(rows(t1) !== "", `t1 lists its shell after its turn (${rows(t1).slice(0, 40)})`);
  const t2 = await made("parent");
  c.send("turn.start", { thread: t2, text: "mode:delegate go", msg: "l-2" });
  const task = await until(15000, () => changes().find((x) => x.$ === "TaskDelegated" && x.parent === t2));
  await until(15000, () => lines().some((l) => l.startsWith("USER") && l.includes("mode:stop child work")) ? true : undefined);
  await sleep(500);
  // one read, no retries: the count follows the commit, not the minute's tick
  const b = await busy();
  check(b > 0, `/hello says busy right after the commit (${b})`);
  const dev = Bun.spawnSync(["sh", "scripts/dev.sh", "install"], { env: { ...process.env, BACKPLANE_INSTALL_PORT: String(port) }, stdout: "pipe", stderr: "pipe" });
  const said = dev.stdout.toString() + dev.stderr.toString();
  check(dev.exitCode !== 0 && said.includes("at work"), `dev.sh install refuses while busy (${dev.exitCode}: ${said.trim().slice(0, 80)})`);

  await kill(hub);
  hub = await start();
  c = await connect();
  const told = await until(20000, () => lines().find((l) => l.startsWith("IN") && l.includes("ended your background work")));
  check(!!told && told.includes("restart") && told.includes("shell"), `t1 is told its shell and the restart (${(told ?? "").slice(0, 120)})`);
  await until(15000, () => turns(t1).at(-1) === "completed" && turns(t1).includes("running") ? true : undefined);
  check(turns(t1).includes("running") && turns(t1).at(-1) === "completed", `t1 works, then completes (${turns(t1).join(",")})`);
  check(rows(t1) === "", "t1's lost row is gone");
  const failed = changes().find((x) => x.$ === "TaskStateSet" && x.task === task?.task && x.state === "failed");
  check(!failed, "the child's task stays open across the restart");
  const expect = `t:${task?.child}:td-${task?.task}`;
  await until(40000, () => changes().find((x) => x.$ === "MessagePosted" && x.thread === t2 && x.msg === expect));
  await sleep(1000);
  const res = changes().filter((x) => x.$ === "MessagePosted" && x.thread === t2 && x.msg === expect);
  check(res.length === 1 && changes().some((x) => x.$ === "TaskStateSet" && x.task === task?.task && x.state === "done"), `the parent gets the child's result once (${res.length})`);

  // 2. a replacement for a new model
  const t3 = await made("replace");
  c.send("turn.start", { thread: t3, text: "mode:bg first", msg: "l-3" });
  await until(10000, () => turns(t3).at(-1) === "completed" && rows(t3) ? true : undefined);
  const first = rows(t3).split("\t")[0];
  c.send("thread.modes", { thread: t3, model: "claude-other" });
  await sleep(300);
  c.send("turn.start", { thread: t3, text: "mode:bg again", msg: "l-4" });
  const rep = await until(15000, () => changes().find((x) => x.$ === "MessagePosted" && x.thread === t3 && String(x.text).includes("for a new model or mode")));
  check(!!rep, "t3 is told its work was lost to the replacement");
  await until(15000, () => rows(t3) && rows(t3).split("\t")[0] !== first ? true : undefined);
  await sleep(1500);
  const now3 = rows(t3);
  check(now3 !== "" && !now3.includes(first) , `the new process's shell stays listed (${now3.slice(0, 40)})`);

  // 3. restart.continue off
  c.send("setting.set", { key: "restart.continue", value: "off" });
  const t4 = await made("off");
  c.send("turn.start", { thread: t4, text: "mode:bg off", msg: "l-5" });
  await until(10000, () => turns(t4).at(-1) === "completed" && rows(t4) ? true : undefined);
  await kill(hub);
  hub = await start();
  c = await connect();
  await sleep(2000);
  const act = changes().find((x) => x.$ === "ActivityLogged" && x.thread === t4 && x.kind === "lost");
  check(!!act, "off: an Act says the work was lost");
  check(turns(t4).at(-1) === "completed", `off: nothing starts (${turns(t4).at(-1)})`);
  check(rows(t4).includes("\u0001ended:unknown"), "off: the row stays, outcome unknown");
} finally {
  if (hub) await kill(hub);
  if (process.env.KEEP) console.log(root); else rmSync(root, { recursive: true, force: true });
}
console.log(fail ? `${fail} failed` : "all ok");
process.exit(fail ? 1 : 0);
