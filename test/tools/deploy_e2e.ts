// Safe deploy and restart, end to end (hub.bend's Deploy.*, docs/deploy.md).
// A headless hub installed in a temp prefix (a copy of the build) with the
// stand-in `claude` (test/tools/claude_standin.ts):
//   1. A holds a tool call (mode:toolhold, 7 s), C works (mode:stop), and B
//      calls deploy_restart with a staged package: nothing is swapped or
//      restarted while A's tool runs; when it ends the package is swapped
//      into the prefix (LICENSE, web/ and web.old), the hub restarts, B is
//      told once, and C's process, kept through the restart
//      (core/adopt.bend), ends its turn itself (test/tools/adopt_e2e.ts).
//   2. A build lock (BACKPLANE_DEPLOY_LOCKS) is held by flock for 7 s: B2's
//      deploy_restart waits for it, then goes.
//   3. D runs two tool calls in a row with no text between and sleeps 30 s
//      between them (outside any tool call): B3's deploy_restart goes in that
//      gap, not after D's turn.
//
//   bun test/tools/deploy_e2e.ts [BINARY] [WIREDIR]      (port 3792)
import { chmodSync, copyFileSync, existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const [bin = "build/backplane", wire = "build/wire"] = process.argv.slice(2);
for (const f of readdirSync(wire).filter((f) => f.endsWith(".js"))) (0, eval)(readFileSync(`${wire}/${f}`, "utf8"));
const W = (globalThis as any).Wire;
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

const PORT = Number(process.env.E2E_PORT ?? 3792);
const root = mkdtempSync(join(tmpdir(), "bp-deploy-"));
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
const prefix = join(root, "prefix");
const lock = join(root, "build.lock");
for (const d of [home, proj, prefix, join(prefix, "web")]) mkdirSync(d);
writeFileSync(join(home, "events.jsonl"), "");
writeFileSync(lock, "");
copyFileSync(resolve(bin), join(prefix, "backplane"));
chmodSync(join(prefix, "backplane"), 0o755);
writeFileSync(join(prefix, "LICENSE"), "orig");
writeFileSync(join(prefix, "web", "marker"), "orig");

// a staged package: the same binary, a LICENSE and a web/ saying which
const stage = (tag: string) => {
  const d = join(root, `stage-${tag}`);
  mkdirSync(join(d, "web"), { recursive: true });
  copyFileSync(resolve(bin), join(d, "backplane"));
  chmodSync(join(d, "backplane"), 0o755);
  writeFileSync(join(d, "LICENSE"), tag);
  writeFileSync(join(d, "web", "marker"), tag);
  return d;
};
const read = (p: string) => (existsSync(p) ? readFileSync(p, "utf8") : "");

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
let deployArgs = "{}";
const start = async (): Promise<Hub> => {
  const proc = Bun.spawn([join(prefix, "backplane"), "--home", home, "--port", String(PORT), "--no-tailscale", "--foreground"], {
    env: { ...process.env, DISPLAY: "", WAYLAND_DISPLAY: "", BACKPLANE_NO_UPDATE: "1", BACKPLANE_KEYRING_LOCKED: "0", BACKPLANE_PEERS: "", BACKPLANE_GOOGLE_BAKE: "0", FAKE_LOG: log,
      BP_WORK_MS: "300", BP_TOOL_MS: "7000", ...(process.env.DEBUG ? { BACKPLANE_LOG: "debug" } : {}), BP_DEPLOY_ARGS: deployArgs, BACKPLANE_DEPLOY_LOCKS: lock, BACKPLANE_AGENTS: fake, PATH: `${fake}:${process.env.PATH}` },
    stdout: process.env.DEBUG ? Bun.file(join(root, "hub.out")) : "ignore",
    stderr: process.env.DEBUG ? Bun.file(join(root, "hub.err")) : "ignore",
  });
  for (let i = 0; i < 200; i++) {
    try {
      if ((await fetch(`http://127.0.0.1:${PORT}/hello`)).status === 200) break;
    } catch {}
    await sleep(100);
  }
  return { proc };
};
const kill = async (h: Hub) => {
  try { process.kill(h.proc.pid, "SIGKILL"); } catch {}
  await h.proc.exited;
};
type Client = { ws: WebSocket; seen: any[]; send: (m: string, p: object) => void };
const connect = async (): Promise<Client> => {
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
  return { ws, seen, send: (m, p) => ws.send(W.encode(JSON.stringify({ id: ++id, m, p }))) };
};

let hub: Hub | null = null;
let c: Client;
const restart = async () => {
  if (hub) await kill(hub);
  hub = await start();
  c = await connect();
};
let lockHolder: ReturnType<typeof Bun.spawn> | null = null;
try {
  const S1 = stage("S1");
  deployArgs = JSON.stringify({ stage: S1, note: "e2e one" });
  hub = await start();
  c = await connect();
  const changes = () => c.seen;
  c.send("project.add", { path: proj });
  const pc: any = await until(10000, () => changes().find((x) => x.$ === "ProjectCreated"));
  const made = async (title: string) => {
    await sleep(1500);
    c.send("thread.create", { project: pc.id, title, env: "local", provider: "claude" });
    return (await until(10000, () => changes().find((x) => x.$ === "ThreadCreated" && x.title === title)))?.id as string;
  };
  const notes = (th: string) => changes().filter((x) => x.$ === "MessagePosted" && x.thread === th && String(x.text).includes("restarted for a deploy"));

  // 1. a tool call in flight holds the deploy
  const A = await made("tool holder");
  const C = await made("worker");
  const B = await made("deployer");
  c.send("turn.start", { thread: A, text: "mode:toolhold go", msg: "a-1" });
  await until(15000, () => changes().find((x) => x.$ === "ActivityLogged" && x.thread === A && x.tone === "tool"));
  c.send("turn.start", { thread: C, text: "mode:stop go", msg: "c-1" });
  await sleep(1000);
  const t0 = Date.now();
  c.send("turn.start", { thread: B, text: "mode:deploy go", msg: "b-1" });
  const scheduled = await until(15000, () => /DEPLOY .*scheduled/.test(read(log)) ? true : undefined);
  check(!!scheduled, "deploy_restart answers at once: scheduled");
  const waitSet = await until(5000, () => changes().find((x) => x.$ === "SettingSet" && x.key === "deploy.wait" && String(x.value).includes("e2e one")));
  check(!!waitSet, "the wait is recorded");
  await sleep(3000);
  check(Date.now() - t0 < 6500 && read(join(prefix, "LICENSE")) === "orig", `nothing is swapped while A's tool runs (LICENSE ${read(join(prefix, "LICENSE"))})`);
  check(!read(log).includes("TOOL ended"), "A's tool is still running at that point");
  const swapped = await until(40000, () => read(join(prefix, "LICENSE")) === "S1" ? true : undefined);
  check(!!swapped, "the package is swapped in once the tool ended");
  check(/TOOL ended/.test(read(log)), "and the tool had ended first");
  check(read(join(prefix, "web", "marker")) === "S1" && read(join(prefix, "web.old", "marker")) === "orig", "web/ swapped, the old one kept as web.old");
  check(!existsSync(S1), "the staged folder is gone");
  // the hub restarts into it: the old connection drops, the new hub answers
  await sleep(1500);
  for (let i = 0; i < 100; i++) { try { if ((await fetch(`http://127.0.0.1:${PORT}/hello`)).status === 200) break; } catch {} await sleep(100); }
  c = await connect();
  await until(20000, () => notes(B).length ? true : undefined);
  await sleep(2500);
  check(notes(B).length === 1, `B is told once (${notes(B).length}: ${String(notes(B)[0]?.text).slice(0, 80)})`);
  // C's process came through the restart (adopt.bend): its turn was never
  // cut, so nothing tells it to continue, and it ends in that process
  const cont = changes().filter((x) => x.$ === "MessagePosted" && x.thread === C && String(x.text).startsWith("Backplane restarted while you were working"));
  check(cont.length === 0, `C's turn, kept through the restart, is not told to continue (${cont.length})`);
  const cEnds = await until(40000, () => changes().find((x) => x.$ === "TurnChanged" && x.thread === C && x.state === "completed"));
  check(!!cEnds, "C's turn ends in the process that was running it");
  check(notes(C).length === 0 && notes(A).length === 0, "A, whose turn had ended, and C, never cut, get no note");
  const cleared = changes().filter((x) => x.$ === "SettingSet" && x.key === "deploy.done");
  check(cleared.length >= 2 && String(cleared.at(-1).value) === "", "deploy.done is cleared at start");

  // 2. a held build lock holds it too
  const S2 = stage("S2");
  deployArgs = JSON.stringify({ stage: S2, note: "e2e two" });
  await restart();
  await sleep(500);
  lockHolder = Bun.spawn(["flock", lock, "sleep", "7"], { stdout: "ignore", stderr: "ignore" });
  await sleep(800);
  const B2 = await made("deployer two");
  const t1 = Date.now();
  c.send("turn.start", { thread: B2, text: "mode:deploy go", msg: "b-2" });
  await until(15000, () => changes().find((x) => x.$ === "SettingSet" && x.key === "deploy.wait" && String(x.value).includes("e2e two")));
  await sleep(4000);
  check(read(join(prefix, "LICENSE")) === "S1" && Date.now() - t1 < 7000, "nothing is swapped while the build lock is held");
  const swapped2 = await until(40000, () => read(join(prefix, "LICENSE")) === "S2" ? true : undefined);
  check(!!swapped2 && Date.now() - t1 > 5500, "it goes once the lock is free");
  await sleep(1500);
  for (let i = 0; i < 100; i++) { try { if ((await fetch(`http://127.0.0.1:${PORT}/hello`)).status === 200) break; } catch {} await sleep(100); }
  c = await connect();
  await until(20000, () => notes(B2).length ? true : undefined);
  await sleep(2000);
  check(notes(B2).length === 1, `B2 is told once (${notes(B2).length})`);
  const again = await fetch(`http://127.0.0.1:${PORT}/hello`);
  check(again.status === 200, "the hub is listening");

  // 3. between two tool calls of one turn the deploy goes
  const S3 = stage("S3");
  deployArgs = JSON.stringify({ stage: S3, note: "e2e three" });
  await restart();
  await sleep(500);
  const D = await made("gap runner");
  const B3 = await made("deployer three");
  c.send("turn.start", { thread: D, text: "mode:toolgap go", msg: "d-1" });
  const gapStart = await until(15000, () => /GAP tool 1 ended/.test(read(log)) ? true : undefined);
  check(!!gapStart, "D's first tool call ran and ended");
  const t3 = Date.now();
  c.send("turn.start", { thread: B3, text: "mode:deploy go", msg: "b-3" });
  const swapped3 = await until(25000, () => read(join(prefix, "LICENSE")) === "S3" ? true : undefined);
  check(!!swapped3 && !read(log).includes("GAP over"), `the deploy goes in the gap, before D's second call (${Date.now() - t3} ms, newest entry is a tool Act)`);
} finally {
  if (lockHolder) { try { process.kill(lockHolder.pid, "SIGKILL"); } catch {} }
  if (hub) await kill(hub);
  if (process.env.KEEP) console.log(root); else rmSync(root, { recursive: true, force: true });
}
console.log(fail ? `${fail} failed` : "all ok");
process.exit(fail ? 1 : 0);
