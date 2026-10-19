// Retiring a run: Stop, archive and a model switch leave nothing behind.
// Starts a headless hub with a stand-in `claude` that, on a message saying
// "approve", asks Backplane's permission prompt over MCP (the request parks
// until the user answers); on an interrupt it still writes a delta and a
// result for the stopped turn; and when it is stopped (SIGTERM) it writes
// a result before exiting, as a replaced or archived process might.
// Checks:
// - Stop answers the parked request exactly once with a deny, closes the
//   ask, and the thread stays Interrupted after the late result: no
//   Completed, no late delta, and nothing starts it again; a message sent
//   meanwhile (an older client's "queue" mode) is posted at once and
//   goes into the running turn, never held (hub.bend's Retire.stop,
//   Claude.result.if, Claude.stream.if; law deliver_never_waits);
// - archive of a running thread interrupts it, and after unarchive it is
//   not running (Retire.archive);
// - after a model switch the old process's late result is not heard
//   (the reader's tag, server.bend's Agent.reader).
//
//   bun test/tools/retire_e2e.ts [BINARY] [WIREDIR]
import { chmodSync, existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const [bin = "build/backplane", wire = "build/wire"] = process.argv.slice(2);
for (const f of readdirSync(wire).filter((f) => f.endsWith(".js"))) (0, eval)(readFileSync(`${wire}/${f}`, "utf8"));
const W = (globalThis as any).Wire;
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

const root = mkdtempSync(join(tmpdir(), "bp-retire-"));
const fake = join(root, "bin");
const log = join(root, "claude.log");
const perm = join(root, "perm.out");
mkdirSync(fake);
writeFileSync(join(fake, "claude"), `#!/bin/sh
# stopped (a model switch, archive), it still writes a result as it goes
trap 'echo "EOF $$" >> "$FAKE_LOG"; printf "%s\\n" "{\\"type\\":\\"result\\",\\"subtype\\":\\"success\\",\\"is_error\\":false,\\"result\\":\\"from the old process\\",\\"session_id\\":\\"x\\"}"; exit 0' TERM
url=$(printf '%s' "$*" | sed -n 's/.*"url":"\\([^"]*\\)".*/\\1/p')
echo "START $$" >> "$FAKE_LOG"
init='{"type":"system","subtype":"init","session_id":"sess-'$$'","cwd":"/tmp","tools":["Bash"],"model":"fake","permissionMode":"default"}'
while IFS= read -r line; do
  case "$line" in
    *'"subtype":"interrupt"'*)
      echo "INT $$" >> "$FAKE_LOG"
      printf '%s\\n' '{"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"LATEDELTA"}}}'
      printf '%s\\n' '{"type":"result","subtype":"success","is_error":false,"result":"late","session_id":"x"}' ;;
    *'"type":"user"'*approve*)
      echo "USER $$ $line" >> "$FAKE_LOG"
      printf '%s\\n' "$init"
      ( curl -s -X POST -H 'content-type: application/json' "$url" -d '{"jsonrpc":"2.0","id":7,"method":"tools/call","params":{"name":"permission_prompt","arguments":{"tool_name":"Bash","input":{"command":"rm -rf /tmp/nothing"}}}}' >> "$FAKE_PERM"; echo >> "$FAKE_PERM"; echo "PERMDONE" >> "$FAKE_LOG" ) & ;;
    *'"type":"user"'*)
      echo "USER $$ $line" >> "$FAKE_LOG"
      printf '%s\\n' "$init"
      printf '%s\\n' '{"type":"assistant","message":{"id":"m1","role":"assistant","content":[{"type":"text","text":"working on it"}],"usage":{"input_tokens":1}},"session_id":"x"}' ;;
  esac
done
echo "EOF $$" >> "$FAKE_LOG"
printf '%s\\n' '{"type":"result","subtype":"success","is_error":false,"result":"from the old process","session_id":"x"}'
`);
chmodSync(join(fake, "claude"), 0o755);
for (const n of ["codex", "grok", "muse", "omp"]) {
  writeFileSync(join(fake, n), "#!/bin/sh\nexit 1\n");
  chmodSync(join(fake, n), 0o755);
}
const home = join(root, "home");
const proj = join(root, "proj");
mkdirSync(proj);
mkdirSync(home);
// nothing carries on after a restart, and no real claude
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
const proc = Bun.spawn([resolve(bin), "--home", home, "--port", String(port), "--no-tailscale"], {
  env: { ...process.env, DISPLAY: "", WAYLAND_DISPLAY: "", BACKPLANE_NO_UPDATE: "1", BACKPLANE_KEYRING_LOCKED: "0", BACKPLANE_PEERS: "", BACKPLANE_GOOGLE_BAKE: "0", FAKE_LOG: log, FAKE_PERM: perm, BACKPLANE_AGENTS: fake, PATH: `${fake}:${process.env.PATH}` },
  stdout: "ignore",
  stderr: "ignore",
});
try {
  for (let i = 0; i < 200; i++) {
    try {
      if ((await fetch(`http://127.0.0.1:${port}/hello`)).status === 200) break;
    } catch {}
    await sleep(100);
  }
  const seen: any[] = [];
  const deltas: string[] = [];
  const ws = new WebSocket(`ws://127.0.0.1:${port}/ws`);
  ws.binaryType = "arraybuffer";
  ws.onmessage = (e) => {
    const text = typeof e.data === "string";
    let o: any = null;
    try { o = JSON.parse(text ? (e.data as string) : W.decode(new Uint8Array(e.data as ArrayBuffer))); } catch {}
    if (o?.t === "delta") deltas.push(o.text);
    for (const c of o?.items ?? []) seen.push(c);
  };
  await new Promise((r, j) => { ws.onopen = r; ws.onerror = j; });
  let id = 0;
  const send = (m: string, p: object) => ws.send(W.encode(JSON.stringify({ id: ++id, m, p })));
  const turns = (th: string) => seen.filter((c) => c.$ === "TurnChanged" && c.thread === th).map((c) => c.state ?? c.turn ?? JSON.stringify(c));
  const last = (th: string) => { const t = turns(th); return String(t[t.length - 1] ?? ""); };
  const newThread = async (title: string) => {
    const n = seen.filter((c) => c.$ === "ThreadCreated").length;
    send("thread.create", { project: pc.id, title, env: "local", provider: "claude" });
    const th = await until(10000, () => seen.filter((c) => c.$ === "ThreadCreated")[n]);
    if (!th) throw new Error("no thread");
    return th.id as string;
  };

  send("project.add", { path: proj });
  const pc = await until(10000, () => seen.find((c) => c.$ === "ProjectCreated"));
  if (!pc) throw new Error("no project");
  send("setting.set", { key: "restart.continue", value: "off" });

  // 1. Stop with an approval waiting
  const t1 = await newThread("stop");
  send("thread.modes", { thread: t1, runtime: "approval-required" });
  await sleep(300);
  send("turn.start", { thread: t1, text: "please approve this", msg: "c-e2e-1" });
  const ask = await until(15000, () => seen.find((c) => c.$ === "AskOpened" && c.thread === t1));
  check(!!ask, "the permission prompt opened an ask");
  // nothing is queued (law deliver_never_waits): an older client's "queue"
  // message is posted at once and goes into Claude's running turn
  send("turn.start", { thread: t1, text: "sent meanwhile", msg: "c-e2e-2", mode: "queue" });
  check(!!(await until(5000, () => seen.find((c) => c.$ === "MessagePosted" && c.thread === t1 && String(c.text) === "sent meanwhile"))), "a message sent meanwhile is posted at once");
  check(!!(await until(5000, () => lines().some((l) => l.startsWith("USER") && l.includes("sent meanwhile")) ? true : undefined)), "and Claude's running turn takes it");
  check(!seen.some((c) => c.$ === "TurnQueued" && c.thread === t1), "nothing waits in a queue");
  const users0 = lines().filter((l) => l.startsWith("USER")).length;
  send("turn.interrupt", { thread: t1 });
  check(!!(await until(10000, () => lines().some((l) => l.startsWith("PERMDONE")))), "the parked request was answered");
  await sleep(2000);
  const answers = readFileSync(perm, "utf8").split("\n").filter(Boolean);
  check(answers.length === 1 && answers[0].includes("deny") && !answers[0].includes("allow"), `exactly one deny (${answers.length}: ${answers[0]?.slice(0, 160)})`);
  check(!!seen.find((c) => c.$ === "AskClosed" && c.id === ask?.id), "the ask closed");
  check(lines().some((l) => l.startsWith("INT")), "claude got the interrupt");
  check(/Interrupted/i.test(last(t1)), `the thread stays interrupted after the late result (${turns(t1).join(",")})`);
  check(!turns(t1).some((t) => /Completed/i.test(String(t))), "no Completed from the stopped turn's result");
  check(!deltas.some((d) => d.includes("LATEDELTA")), "the stopped turn's late delta streams nowhere");
  check(lines().filter((l) => l.startsWith("USER")).length === users0, "nothing starts the stopped thread again");

  // 2. archive a running thread, then unarchive it
  const t2 = await newThread("archive");
  send("turn.start", { thread: t2, text: "work", msg: "c-e2e-3" });
  await until(10000, () => /Running/i.test(last(t2)) ? true : undefined);
  await until(10000, () => lines().filter((l) => l.startsWith("USER")).length > users0 ? true : undefined);
  send("thread.archive", { thread: t2 });
  await sleep(2000);
  check(/Interrupted/i.test(last(t2)), `archive interrupted the running turn (${turns(t2).join(",")})`);
  send("thread.unarchive", { thread: t2 });
  await sleep(1000);
  check(!/Running|Completed/i.test(last(t2)), `after unarchive it is not running (${last(t2)})`);

  // 3. a model switch: the replaced process's late result is not heard
  const t3 = await newThread("switch");
  send("turn.start", { thread: t3, text: "first", msg: "c-e2e-4" });
  await until(10000, () => /Running/i.test(last(t3)) ? true : undefined);
  const starts = () => lines().filter((l) => l.startsWith("START")).length;
  const s0 = starts();
  const e0 = lines().filter((l) => l.startsWith("EOF")).length;
  await sleep(500);
  send("thread.modes", { thread: t3, model: "other-model" });
  await sleep(300);
  send("turn.start", { thread: t3, text: "second", msg: "c-e2e-5", mode: "steer" });
  check(!!(await until(10000, () => starts() > s0 ? true : undefined)), "the switch started a new process");
  check(!!(await until(10000, () => lines().filter((l) => l.startsWith("EOF")).length > e0 ? true : undefined)), "the old process wrote its result as it ended");
  await sleep(1500);
  check(/Running/i.test(last(t3)), `the new turn still runs (${turns(t3).join(",")})`);
} finally {
  proc.kill(9);
  await proc.exited;
  if (process.env.KEEP) console.log(root); else rmSync(root, { recursive: true, force: true });
}
console.log(fail ? `${fail} failed` : "all ok");
process.exit(fail ? 1 : 0);
