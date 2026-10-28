// Auto panel: an agent starting in a KiCad project opens its thread's
// panel on the board. Starts a headless hub with a stand-in `claude`, runs
// a turn in a folder with a .kicad_pro and .kicad_pcb and checks the hub
// sends that thread a "view" with "auto" on the board; then a turn in a
// plain folder, which sends none (hub.bend's Auto.view, server.bend's
// Hub.auto; the client's side is laws viewer_auto_*).
//
//   bun test/tools/auto_panel_e2e.ts [BINARY] [WIREDIR]
import { chmodSync, existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const [bin = "build/backplane", wire = "build/wire"] = process.argv.slice(2);
for (const f of readdirSync(wire).filter((f) => f.endsWith(".js"))) (0, eval)(readFileSync(`${wire}/${f}`, "utf8"));
const W = (globalThis as any).Wire;
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

const root = mkdtempSync(join(tmpdir(), "bp-autopanel-"));
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
for (const n of ["codex", "grok", "muse"]) {
  writeFileSync(join(fake, n), "#!/bin/sh\nexit 1\n");
  chmodSync(join(fake, n), 0o755);
}
const home = join(root, "home");
const proj = join(root, "board");
mkdirSync(proj);
writeFileSync(join(proj, "kb.kicad_pro"), "{}\n");
writeFileSync(join(proj, "kb.kicad_pcb"), "(kicad_pcb (version 20240108) (generator test))\n");
const plain = join(root, "plain");
mkdirSync(plain);

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
    env: { ...process.env, DISPLAY: "", WAYLAND_DISPLAY: "", BACKPLANE_NO_UPDATE: "1", BACKPLANE_ORDER_SERVER: "http://127.0.0.1:9", BACKPLANE_KEYRING_LOCKED: "0", BACKPLANE_PEERS: "", FAKE_LOG: log, BACKPLANE_AGENTS: fake, PATH: `${fake}:${process.env.PATH}` },
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
  const views: any[] = [];
  const ws = new WebSocket(`ws://127.0.0.1:${port}/ws`);
  ws.binaryType = "arraybuffer";
  ws.onmessage = (e) => {
    const text = typeof e.data === "string";
    let o: any = null;
    try { o = JSON.parse(text ? (e.data as string) : W.decode(new Uint8Array(e.data as ArrayBuffer))); } catch {}
    if (o) views.push(o);
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
  return { seen, views, send, stop };
}

let fail = 0;
const check = (ok: boolean, what: string) => {
  console.log(`${ok ? "ok  " : "FAIL"} ${what}`);
  if (!ok) fail++;
};

async function turn(h: any, path: string, title: string) {
  h.send("project.add", { path });
  const pc = await until(10000, () => h.seen.find((c: any) => c.$ === "ProjectCreated" && c.root.endsWith(path.split("/").pop())));
  if (!pc) throw new Error("no project");
  h.send("thread.create", { project: pc.id, title, env: "local" });
  const th = await until(10000, () => h.seen.find((c: any) => c.$ === "ThreadCreated" && c.title === title));
  if (!th) throw new Error("no thread");
  h.send("turn.start", { thread: th.id, text: "look at the board" });
  return th.id as string;
}

try {
  const h = await hub();
  const t1 = await turn(h, proj, "kicad");
  const v = await until(10000, () => h.views.find((o: any) => o.t === "view" && o.thread === t1));
  check(!!v && v.auto === true && v.kind === "board" && String(v.path).endsWith("kb.kicad_pcb"), "a KiCad project's agent opens the board: " + JSON.stringify(v));
  const t2 = await turn(h, plain, "plain");
  await until(10000, () => lines().filter((l) => l.startsWith("USER")).length >= 2);
  await sleep(1000);
  check(!h.views.some((o: any) => o.t === "view" && o.thread === t2), "a plain project's agent opens nothing");
  await h.stop();
} finally {
  if (process.env.KEEP) console.log(root); else rmSync(root, { recursive: true, force: true });
}
console.log(fail ? `${fail} failed` : "all ok");
process.exit(fail ? 1 : 0);
