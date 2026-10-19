// Restart: a turn the hub's restart cut short carries on. Starts a headless
// hub with a stand-in `claude` that opens a session and never ends its
// turn, kills the hub mid-turn, starts it again on the same home, and
// checks the stand-in is started again with --resume and told to continue
// (the "restart.continue" setting, hub.bend's Carry). Then turns the
// setting off, does it again, and checks nothing starts.
//
//   bun test/tools/restart_e2e.ts [BINARY] [WIREDIR]
//
// BINARY defaults to build/backplane, WIREDIR to build/wire (bend
// test/wire/index.html -o build/wire).
import { chmodSync, existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const [bin = "build/backplane", wire = "build/wire"] = process.argv.slice(2);
for (const f of readdirSync(wire).filter((f) => f.endsWith(".js"))) (0, eval)(readFileSync(`${wire}/${f}`, "utf8"));
const W = (globalThis as any).Wire;
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

const root = mkdtempSync(join(tmpdir(), "bp-restart-"));
const fake = join(root, "bin");
const log = join(root, "claude.log");
mkdirSync(fake);
// each start logs its arguments; each user line is logged and answered
// with the init line and a message, and the turn never ends
writeFileSync(join(fake, "claude"), `#!/bin/sh
echo "ARGS $*" >> "$FAKE_LOG"
while IFS= read -r line; do
  case "$line" in
    *'"type":"user"'*)
      echo "USER $line" >> "$FAKE_LOG"
      printf '%s\\n' '{"type":"system","subtype":"init","session_id":"sess-1","cwd":"/tmp","tools":["Bash"],"model":"fake","permissionMode":"default"}'
      printf '%s\\n' '{"type":"assistant","message":{"id":"m1","role":"assistant","content":[{"type":"text","text":"working on it"}],"usage":{"input_tokens":1}},"session_id":"sess-1"}' ;;
  esac
done
`);
chmodSync(join(fake, "claude"), 0o755);
for (const n of ["codex", "grok", "muse", "omp"]) {
  writeFileSync(join(fake, n), "#!/bin/sh\nexit 1\n");
  chmodSync(join(fake, n), 0o755);
}
const home = join(root, "home");
const proj = join(root, "proj");
mkdirSync(proj);

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

async function hub() {
  const port = freePort();
  const proc = Bun.spawn([resolve(bin), "--home", home, "--port", String(port), "--no-tailscale"], {
    env: { ...process.env, DISPLAY: "", WAYLAND_DISPLAY: "", BACKPLANE_NO_UPDATE: "1", BACKPLANE_KEYRING_LOCKED: "0", BACKPLANE_PEERS: "", FAKE_LOG: log, BACKPLANE_AGENTS: fake, PATH: `${fake}:${process.env.PATH}` },
    stdout: "ignore",
    stderr: "ignore",
  });
  for (let i = 0; i < 200; i++) {
    try {
      if ((await fetch(`http://127.0.0.1:${port}/hello`)).status === 200) break;
    } catch {}
    await sleep(100);
  }
  const seen: any[] = [];
  const ws = new WebSocket(`ws://127.0.0.1:${port}/ws`);
  ws.binaryType = "arraybuffer";
  ws.onmessage = (e) => {
    const text = typeof e.data === "string";
    let o: any = null;
    try { o = JSON.parse(text ? (e.data as string) : W.decode(new Uint8Array(e.data as ArrayBuffer))); } catch {}
    for (const c of o?.items ?? []) seen.push(c);
  };
  await new Promise((r, j) => { ws.onopen = r; ws.onerror = j; });
  let id = 0;
  const send = (m: string, p: object) => ws.send(W.encode(JSON.stringify({ id: ++id, m, p })));
  const stop = async () => {
    ws.close();
    proc.kill(9);
    await proc.exited;
  };
  return { seen, send, stop };
}

let fail = 0;
const check = (ok: boolean, what: string) => {
  console.log(`${ok ? "ok  " : "FAIL"} ${what}`);
  if (!ok) fail++;
};

try {
  // a turn starts and is cut short
  let h = await hub();
  h.send("project.add", { path: proj });
  const pc = await until(10000, () => h.seen.find((c) => c.$ === "ProjectCreated"));
  if (!pc) throw new Error("no project");
  h.send("thread.create", { project: pc.id, title: "carry", env: "local" });
  const th = await until(10000, () => h.seen.find((c) => c.$ === "ThreadCreated"));
  if (!th) throw new Error("no thread");
  h.send("turn.start", { thread: th.id, text: "do the long thing" });
  check(!!(await until(10000, () => lines().find((l) => l.startsWith("USER")))), "the turn started");
  await until(5000, () => h.seen.find((c) => c.$ === "SessionSet" || JSON.stringify(c).includes("sess-1")));
  await sleep(500);
  await h.stop();

  // the hub comes back and the turn carries on
  const n0 = lines().length;
  h = await hub();
  const again = await until(15000, () => lines().slice(n0).find((l) => l.startsWith("USER")));
  check(!!again && again.includes("Continue where you left off"), "told to continue after the restart");
  // the arguments span lines (the system prompt has newlines)
  check(lines().slice(n0).join("\n").includes("--resume sess-1"), "resumed its session");

  // with the setting off, the next restart leaves it stopped
  h.send("setting.set", { key: "restart.continue", value: "off" });
  await sleep(1000);
  await h.stop();
  const n1 = lines().length;
  h = await hub();
  await sleep(4000);
  check(!lines().slice(n1).some((l) => l.startsWith("USER")), "with the setting off nothing carries on");
  await h.stop();
} finally {
  if (process.env.KEEP) console.log(root); else rmSync(root, { recursive: true, force: true });
}
console.log(fail ? `${fail} failed` : "all ok");
process.exit(fail ? 1 : 0);
