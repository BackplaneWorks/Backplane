// A client that stops reading cannot hold the hub. Starts a headless hub
// with a stand-in `claude` that answers a message with a flood of
// assistant messages and then a result. Client A connects (a raw socket,
// small receive buffer) and never reads; client B starts the turn. Its
// frames fill A's queue (1024) and socket, so the hub's next send to A
// would wait; the writer's deadline (server.bend's Conn.deadline, 10 s)
// fails, closes A's channel and shuts its socket. Checks B still gets the
// turn's end and an answer to a request in time, A is dropped (its socket
// ends), and a client reconnecting from the start gets exactly what B
// holds. Twice. Then, on the log that left (thousands of big changes): a
// slow reader joins and gets all of it a chunk at a time; a client that
// never reads is dropped; a ping flood gets at most a pong a second; a
// client hanging up halfway through its join and coming back from what it
// had ends with what B holds; B's requests are answered throughout; Stop
// during a flood lands at once; the hub's memory stays bounded over
// dropped clients.
//
//   bun test/tools/stall_e2e.ts [BINARY] [WIREDIR]
import { chmodSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const [bin = "build/backplane", wire = "build/wire"] = process.argv.slice(2);
for (const f of readdirSync(wire).filter((f) => f.endsWith(".js"))) (0, eval)(readFileSync(`${wire}/${f}`, "utf8"));
const W = (globalThis as any).Wire;
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

const root = mkdtempSync(join(tmpdir(), "bp-stall-"));
const fake = join(root, "bin");
mkdirSync(fake);
const pad = "x".repeat(3000);
writeFileSync(join(fake, "claude"), `#!/bin/sh
while IFS= read -r line; do
  case "$line" in
    *'"type":"user"'*)
      printf '%s\\n' '{"type":"system","subtype":"init","session_id":"sess-1","cwd":"/tmp","tools":["Bash"],"model":"fake","permissionMode":"default"}'
      i=0
      while [ $i -lt 4000 ]; do
        printf '{"type":"assistant","message":{"id":"m%s-%s","role":"assistant","content":[{"type":"text","text":"%s ${pad}"}],"usage":{"input_tokens":1}},"session_id":"sess-1"}\\n' "$$" "$i" "$i"
        i=$((i+1))
      done
      printf '%s\\n' '{"type":"result","subtype":"success","is_error":false,"result":"done","session_id":"sess-1"}' ;;
  esac
done
`);
chmodSync(join(fake, "claude"), 0o755);
for (const n of ["codex", "grok", "muse", "omp"]) {
  writeFileSync(join(fake, n), "#!/bin/sh\nexit 1\n");
  chmodSync(join(fake, n), 0o755);
}
const stalled = join(root, "stalled.py");
writeFileSync(stalled, `import socket, sys, time
s = socket.socket()
s.setsockopt(socket.SOL_SOCKET, socket.SO_RCVBUF, 4096)
s.connect(("127.0.0.1", int(sys.argv[1])))
s.sendall(b"GET /ws HTTP/1.1\\r\\nHost: x\\r\\nUpgrade: websocket\\r\\nConnection: Upgrade\\r\\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\\r\\nSec-WebSocket-Version: 13\\r\\n\\r\\n")
print("connected", flush=True)
time.sleep(float(sys.argv[2]))
s.settimeout(10)
total = 0
try:
    while True:
        d = s.recv(65536)
        if not d:
            print("EOF", total, flush=True)
            break
        total += len(d)
except Exception as e:
    print("ERR", e, total, flush=True)
`);
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

let fail = 0;
const check = (ok: boolean, what: string) => {
  console.log(`${ok ? "ok  " : "FAIL"} ${what}`);
  if (!ok) fail++;
};

const port = freePort();
const proc = Bun.spawn([resolve(bin), "--home", home, "--port", String(port), "--no-tailscale"], {
  env: { ...process.env, DISPLAY: "", WAYLAND_DISPLAY: "", BACKPLANE_NO_UPDATE: "1", BACKPLANE_KEYRING_LOCKED: "0", BACKPLANE_PEERS: "", BACKPLANE_GOOGLE_BAKE: "0", BACKPLANE_AGENTS: fake, PATH: `${fake}:${process.env.PATH}` },
  stdout: "ignore",
  stderr: process.env.HUBLOG ? Bun.file(process.env.HUBLOG) : "ignore",
});

type Client = { seen: any[]; replies: Set<number>; send: (m: string, p: object) => number; close: () => void; origin: string };
async function connect(query = ""): Promise<Client> {
  const seen: any[] = [];
  const replies = new Set<number>();
  let origin = "";
  const ws = new WebSocket(`ws://127.0.0.1:${port}/ws${query}`);
  ws.binaryType = "arraybuffer";
  ws.onmessage = (e) => {
    const text = typeof e.data === "string";
    let o: any = null;
    try { o = JSON.parse(text ? (e.data as string) : W.decode(new Uint8Array(e.data as ArrayBuffer))); } catch {}
    if (o?.origin) origin = o.origin;
    if (o?.id !== undefined && (o?.ok !== undefined || o?.error !== undefined || o?.result !== undefined)) replies.add(Number(o.id));
    for (const c of o?.items ?? []) seen.push(c);
  };
  await new Promise((r, j) => { ws.onopen = r; ws.onerror = j; });
  let id = 0;
  return {
    seen, replies,
    get origin() { return origin; },
    send: (m, p) => { ws.send(W.encode(JSON.stringify({ id: ++id, m, p }))); return id; },
    close: () => ws.close(),
  } as Client;
}

try {
  for (let i = 0; i < 200; i++) {
    try {
      if ((await fetch(`http://127.0.0.1:${port}/hello`)).status === 200) break;
    } catch {}
    await sleep(100);
  }
  const b = await connect();
  b.send("project.add", { path: proj });
  const pc = await until(10000, () => b.seen.find((c) => c.$ === "ProjectCreated"));
  if (!pc) throw new Error("no project");
  b.send("setting.set", { key: "restart.continue", value: "off" });

  for (const round of [1, 2]) {
    const n = b.seen.filter((c) => c.$ === "ThreadCreated").length;
    b.send("thread.create", { project: pc.id, title: `flood ${round}`, env: "local", provider: "claude" });
    const th = await until(10000, () => b.seen.filter((c) => c.$ === "ThreadCreated")[n]);
    if (!th) throw new Error("no thread");
    const a = Bun.spawn(["python3", stalled, String(port), "45"], { stdout: "pipe", stderr: "inherit" });
    const out = new Response(a.stdout).text();
    await sleep(1000);
    const t0 = Date.now();
    b.send("turn.start", { thread: th.id, text: "flood", msg: `c-stall-${round}` });
    // a flood alone takes about 6 s here; the stall adds the 10 s deadline, and
    // without it the hub would wait for A to read (45 s)
    const done = await until(30000, () => b.seen.find((c) => c.$ === "TurnChanged" && c.thread === th.id && /completed/i.test(String(c.state))));
    check(!!done, `round ${round}: B saw the turn complete (${((Date.now() - t0) / 1000).toFixed(1)} s)`);
    const rid = b.send("thread.pin", { thread: th.id });
    const t1 = Date.now();
    const pinned = await until(15000, () => b.seen.find((c) => c.$ === "ThreadPinned" && c.id === th.id));
    check(!!pinned, `round ${round}: a request after it is answered (${((Date.now() - t1) / 1000).toFixed(1)} s)`);
    const said = (await out).trim();
    check(said.includes("EOF") || said.includes("reset"), `round ${round}: the stalled client was dropped (${said.replace(/\n/g, " | ")})`);
    await a.exited;
    const c = await connect(`?since=0`);
    await until(30000, () => (c.seen.length >= b.seen.length ? true : undefined));
    await sleep(500);
    check(c.seen.length === b.seen.length, `round ${round}: a client from the start holds what B holds (${c.seen.length} / ${b.seen.length})`);
    c.close();
  }

  // Transport under a large log
  const wsc = resolve("test/tools/fixtures/wsclient.py");
  const py = (args: string[]) => {
    const p = Bun.spawn(["python3", wsc, String(port), ...args], { stdout: "pipe", stderr: "inherit" });
    return { p, out: new Response(p.stdout).text() };
  };
  const rss = () => Number(/VmRSS:\s+(\d+)/.exec(readFileSync(`/proc/${proc.pid}/status`, "utf8"))?.[1] ?? 0) / 1024;
  const rtt = async () => {
    const t = Date.now();
    const id = b.send("setting.set", { key: `probe.${t}`, value: "1" });
    const ok = await until(10000, () => b.replies.has(id));
    return ok ? Date.now() - t : 99999;
  };
  const total = b.seen.length;
  const slow = py(["?since=0", "slow", "1500000"]);
  const stall = py(["?since=0", "stall", "15"]);
  const pings = py(["?since=" + total + "&origin=" + b.origin, "pings", "20000"]);
  let worst = 0;
  for (let i = 0; i < 10; i++) {
    const r = await rtt();
    if (process.env.HUBLOG) console.log("rtt", r);
    worst = Math.max(worst, r);
    await sleep(300);
  }
  check(worst < 1500, `requests answered while a slow join, a stalled join and a ping flood run (worst ${worst} ms)`);
  const pong = (await pings.out).trim();
  const np = Number(/pongs (\d+)/.exec(pong)?.[1] ?? 99);
  check(np <= 5, `a ping flood gets at most a pong a second (${pong})`);
  const slowSaid = (await slow.out).trim();
  check(/done/.test(slowSaid), `a slow reader takes the whole log a chunk at a time (${slowSaid.replace(/\n/g, " | ")})`);
  const stallSaid = (await stall.out).trim();
  // (its join waits for it: at most about a megabyte is queued for a client
  // with no room, so it either is dropped or gets the rest once it reads)
  check(/EOF|reset|done/.test(stallSaid), `a joining client that does not read holds nothing up (${stallSaid.replace(/\n/g, " | ")})`);

  // halfway through a join, hang up and come back from what it had
  const h1 = await connect("?since=0");
  await until(10000, () => h1.seen.length > 0);
  const had = h1.seen.length, origin = h1.origin;
  h1.close();
  check(had < total, `a join's first frame is a chunk, not the whole log (${had} of ${total})`);
  const h2 = await connect(`?since=${had}&origin=${origin}`);
  const back = await until(30000, () => (had + h2.seen.length >= b.seen.length ? true : undefined));
  check(!!back && had + h2.seen.length === b.seen.length, `a join resumed halfway ends with what B holds (${had} + ${h2.seen.length} / ${b.seen.length})`);
  h2.close();

  // Stop during a flood
  const n3 = b.seen.filter((c) => c.$ === "ThreadCreated").length;
  b.send("thread.create", { project: pc.id, title: "stop", env: "local", provider: "claude" });
  const th3 = await until(10000, () => b.seen.filter((c) => c.$ === "ThreadCreated")[n3]);
  b.send("turn.start", { thread: th3.id, text: "flood", msg: "c-stall-stop" });
  await until(10000, () => b.seen.filter((c) => c.$ === "MessagePosted" && c.thread === th3.id).length > 200);
  const ts = Date.now();
  b.send("turn.interrupt", { thread: th3.id });
  const stopped = await until(10000, () => b.seen.find((c) => c.$ === "TurnChanged" && c.thread === th3.id && /interrupted/i.test(String(c.state))));
  check(!!stopped && Date.now() - ts < 1500, `Stop during a flood lands at once (${Date.now() - ts} ms)`);
  await sleep(8000);

  // memory over dropped clients
  const r0 = rss();
  for (let cyc = 0; cyc < 3; cyc++) {
    const cs = [0, 1, 2, 3].map(() => py(["?since=0", "stall", "13"]));
    for (const c of cs) await c.out;
  }
  const r1 = rss();
  check(r1 - r0 < 400, `memory stays bounded over 12 dropped joins (${r0.toFixed(0)} -> ${r1.toFixed(0)} MB)`);
} finally {
  proc.kill(9);
  await proc.exited;
  if (process.env.KEEP) console.log(root); else rmSync(root, { recursive: true, force: true });
}
console.log(fail ? `${fail} failed` : "all ok");
process.exit(fail ? 1 : 0);
