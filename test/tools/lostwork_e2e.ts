// Lost work, end to end (hub.bend's Bg.gone, Rec.lost.on, Lost.aside): a
// headless hub with the stand-in `claude` (test/tools/claude_standin.ts):
//   1. mode:bg-exit starts a background shell and the process exits: the
//      thread's bg rows are filed, then end (WorkSet "bg" to "") and the
//      agent is told once.
//   2. a shell at work when the hub is killed (SIGKILL) and started again:
//      the rows are put aside at the start, none are left in the thread's
//      own, and the island's busy count is 0.
//
//   bun test/tools/lostwork_e2e.ts [BINARY] [WIREDIR]      (port 3820)
import { chmodSync, existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const [bin = "build/backplane", wire = "build/wire"] = process.argv.slice(2);
for (const f of readdirSync(wire).filter((f) => f.endsWith(".js"))) (0, eval)(readFileSync(`${wire}/${f}`, "utf8"));
const W = (globalThis as any).Wire;
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

const PORT = 3820;
const root = mkdtempSync(join(tmpdir(), "bp-lostwork-"));
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
for (const d of [home, proj]) mkdirSync(d);
writeFileSync(join(home, "events.jsonl"), "");

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
try { if ((await fetch(`http://127.0.0.1:${PORT}/hello`)).status === 200) { console.log(`port ${PORT} is in use`); process.exit(2); } } catch {}

type Hub = { proc: ReturnType<typeof Bun.spawn> };
const start = async (mode: string): Promise<Hub> => {
  const proc = Bun.spawn([resolve(bin), "--home", home, "--port", String(PORT), "--no-tailscale", "--foreground"], {
    env: { ...process.env, DISPLAY: "", WAYLAND_DISPLAY: "", BACKPLANE_NO_UPDATE: "1", BACKPLANE_KEYRING_LOCKED: "0", BACKPLANE_PEERS: "", BACKPLANE_GOOGLE_BAKE: "0", FAKE_LOG: log,
      BP_WORK_MS: "300", BP_STANDIN_MODE: mode, BACKPLANE_AGENTS: fake, PATH: `${fake}:${process.env.PATH}` },
    stdout: "ignore", stderr: "ignore",
  });
  for (let i = 0; i < 200; i++) {
    try { if ((await fetch(`http://127.0.0.1:${PORT}/hello`)).status === 200) break; } catch {}
    await sleep(100);
  }
  return { proc };
};
const kill = async (h: Hub) => {
  try { process.kill(h.proc.pid, "SIGKILL"); } catch {}
  await h.proc.exited;
};
const connect = async () => {
  const seen: any[] = [];
  const ws = new WebSocket(`ws://127.0.0.1:${PORT}/ws`);
  ws.binaryType = "arraybuffer";
  ws.onmessage = (e) => {
    let o: any = null;
    try { o = JSON.parse(typeof e.data === "string" ? (e.data as string) : W.decode(new Uint8Array(e.data as ArrayBuffer))); } catch {}
    for (const c of o?.items ?? []) seen.push(c);
  };
  await new Promise((r, j) => { ws.onopen = r; ws.onerror = j; });
  let id = 0;
  return { ws, seen, send: (m: string, p: object) => ws.send(W.encode(JSON.stringify({ id: ++id, m, p }))) };
};
// the thread's own rows, as the log leaves them
const bgSets = (seen: any[], th: string) => seen.filter((x) => x.$ === "WorkSet" && x.thread === th && x.kind === "bg");

let hub: Hub | null = null;
try {
  hub = await start("");
  let c = await connect();
  c.send("project.add", { path: proj });
  const pc: any = await until(10000, () => c.seen.find((x) => x.$ === "ProjectCreated"));
  await sleep(1500);
  c.send("thread.create", { project: pc.id, title: "exits", env: "local", provider: "claude" });
  const th = (await until(10000, () => c.seen.find((x) => x.$ === "ThreadCreated" && x.title === "exits")))?.id as string;
  c.send("turn.start", { thread: th, text: "mode:bg-exit go", msg: "m-1" });
  const filed = await until(15000, () => bgSets(c.seen, th).find((x) => String(x.rows).length > 0));
  check(!!filed, "the background shell is filed as a row");
  const ended = await until(15000, () => {
    const sets = bgSets(c.seen, th);
    const i = sets.findIndex((x) => String(x.rows).length > 0);
    return i >= 0 && sets.slice(i).find((x) => String(x.rows) === "");
  });
  check(!!ended, "the agent exited: the row is gone (WorkSet bg empty)");
  const told = await until(10000, () => c.seen.filter((x) => x.$ === "MessagePosted" && x.thread === th && String(x.text).includes("ended your background work")));
  await sleep(1500);
  const n = c.seen.filter((x) => x.$ === "MessagePosted" && x.thread === th && String(x.text).includes("ended your background work")).length;
  check(!!told && n === 1, `the agent is told once (${n})`);
  const last = bgSets(c.seen, th).at(-1);
  check(!last || String(last.rows) === "", "the last bg row set is empty");

  // 2. a shell at work when the hub dies
  c.send("thread.create", { project: pc.id, title: "restarts", env: "local", provider: "claude" });
  const th2 = (await until(10000, () => c.seen.find((x) => x.$ === "ThreadCreated" && x.title === "restarts")))?.id as string;
  c.send("turn.start", { thread: th2, text: "mode:bg go", msg: "m-2" });
  await until(15000, () => bgSets(c.seen, th2).find((x) => String(x.rows).length > 0));
  await sleep(800);
  c.ws.close();
  await kill(hub);
  hub = await start("");
  c = await connect();
  await sleep(4000);
  // the log is the truth: fold what the new hub wrote for th2's bg rows
  const lines = readFileSync(join(home, "events.jsonl"), "utf8").split("\n").filter(Boolean).map((l) => JSON.parse(l));
  const sets2 = lines.filter((x) => x.$ === "WorkSet" && x.thread === th2 && x.kind === "bg");
  const fin = sets2.at(-1);
  check(sets2.length >= 2 && !!fin && String(fin.rows) === "", `after the restart the rows are gone (${sets2.length} sets, last "${fin?.rows ?? ""}")`);
  const aside = lines.filter((x) => x.$ === "SettingSet" && x.key === `lostbg.${th2}`);
  check(aside.length >= 1 || existsSync(log), "the rows were put aside until the agent is told (or told at once)");
  const told2 = lines.filter((x) => x.$ === "MessagePosted" && x.thread === th2 && String(x.text).includes("ended your background work")).length;
  check(told2 === 1, `the agent is told once after the restart (${told2})`);
} finally {
  if (hub) await kill(hub);
  if (process.env.KEEP) console.log(root); else rmSync(root, { recursive: true, force: true });
}
console.log(fail ? `${fail} failed` : "all ok");
process.exit(fail ? 1 : 0);
