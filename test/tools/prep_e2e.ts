// Turn start never waits on the hub's loop (a Claude process is started by
// a job: its project walk and keys are read there). A headless hub with the
// stand-in `claude` and a `find` that takes 3 s:
//   - a request made right after a turn starts is answered at once, not
//     after the walk
//   - the turn still completes, and so does one naming a `$skill`
//
//   bun test/tools/prep_e2e.ts [BINARY] [WIREDIR]
import { chmodSync, existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const [bin = "build/backplane", wire = "build/wire"] = process.argv.slice(2);
for (const f of readdirSync(wire).filter((f) => f.endsWith(".js"))) (0, eval)(readFileSync(`${wire}/${f}`, "utf8"));
const W = (globalThis as any).Wire;
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

const root = mkdtempSync(join(tmpdir(), "bp-prep-"));
const fake = join(root, "bin");
const log = join(root, "claude.log");
mkdirSync(fake);
writeFileSync(join(fake, "claude"), `#!/bin/sh\nexec ${process.execPath} ${resolve("test/tools/claude_standin.ts")} "$@"\n`);
chmodSync(join(fake, "claude"), 0o755);
writeFileSync(join(fake, "grok"), "#!/bin/sh\nexit 1\n");
chmodSync(join(fake, "grok"), 0o755);
writeFileSync(join(fake, "muse"), "#!/bin/sh\nexit 1\n");
chmodSync(join(fake, "muse"), 0o755);
writeFileSync(join(fake, "omp"), "#!/bin/sh\nexit 1\n");
chmodSync(join(fake, "omp"), 0o755);
// a slow project walk: the hub must not wait for it
writeFileSync(join(fake, "find"), "#!/bin/sh\nsleep 3\nexec /usr/bin/find \"$@\"\n");
chmodSync(join(fake, "find"), 0o755);

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
const proc = Bun.spawn([resolve(bin), "--home", home, "--port", String(port), "--no-tailscale", "--foreground"], {
  env: { ...process.env, DISPLAY: "", WAYLAND_DISPLAY: "", BACKPLANE_NO_UPDATE: "1", BACKPLANE_KEYRING_LOCKED: "0", BACKPLANE_PEERS: "", BACKPLANE_GOOGLE_BAKE: "0", FAKE_LOG: log,
    BP_WORK_MS: "300", XDG_CACHE_HOME: join(root, "cache"), BACKPLANE_AGENTS: fake, PATH: `${fake}:${process.env.PATH}` },
  stdout: "ignore",
  stderr: "ignore",
});
try {
  for (let i = 0; i < 200; i++) {
    try { if ((await fetch(`http://127.0.0.1:${port}/hello`)).status === 200) break; } catch {}
    await sleep(100);
  }
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
  const send = (m: string, p: object) => ws.send(W.encode(JSON.stringify({ id: ++id, m, p })));
  const changes = () => seen.map((x) => x.c);
  const turns = (th: string) => changes().filter((x) => x.$ === "TurnChanged" && x.thread === th).map((x) => String(x.state));
  send("project.add", { path: proj });
  const pc = await until(10000, () => changes().find((x) => x.$ === "ProjectCreated"));
  const made = async (title: string) => {
    const n = changes().filter((x) => x.$ === "ThreadCreated").length;
    send("thread.create", { project: pc.id, title, env: "local", provider: "claude" });
    return (await until(10000, () => changes().filter((x) => x.$ === "ThreadCreated")[n]))?.id as string;
  };

  const t1 = await made("one");
  send("turn.start", { thread: t1, text: "mode:plain hello", msg: "p-1" });
  const at = Date.now();
  const t2 = await made("two");
  const took = Date.now() - at;
  check(!!t2 && took < 1500, `a request after a turn starts is answered at once (${took} ms; the walk takes 3 s)`);
  await until(15000, () => turns(t1).at(-1) === "completed" ? true : undefined);
  check(turns(t1).at(-1) === "completed", `the turn completes (${turns(t1).join(",")})`);

  const t3 = await made("three");
  send("turn.start", { thread: t3, text: "mode:plain use $pcb please", msg: "p-2" });
  await until(15000, () => turns(t3).at(-1) === "completed" ? true : undefined);
  check(turns(t3).at(-1) === "completed", `a turn naming a $skill completes (${turns(t3).join(",")})`);
  // a second line while one runs reaches the process
  send("turn.start", { thread: t3, text: "mode:plain again", msg: "p-3" });
  await until(15000, () => turns(t3).filter((s) => s === "completed").length >= 2 ? true : undefined);
  check(turns(t3).filter((s) => s === "completed").length >= 2, `a second message completes too (${turns(t3).join(",")})`);
} finally {
  proc.kill(9);
  await proc.exited;
  if (process.env.KEEP) console.log(root); else rmSync(root, { recursive: true, force: true });
}
console.log(fail ? `${fail} failed` : "all ok");
process.exit(fail ? 1 : 0);
