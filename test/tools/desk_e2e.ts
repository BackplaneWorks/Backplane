// Desks, end to end (core/desk.bend): a headless hub with a stand-in
// `claude`, a desk client (as the web page is) and a phone-like client.
//   - the desk says desk.look on thread A, focused: every client hears
//     info "desk" naming A
//   - A's turn ends: no desktop notification (the desk is watching it)
//   - B's turn ends: the desk gets {"t":"notify"} for B, the phone does not
//   - the desk leaves: every client hears info "desk" empty
//
//   bun test/tools/desk_e2e.ts [BINARY] [WIREDIR]
// BINARY defaults to build/backplane, WIREDIR to build/wire (bend
// test/wire/index.html -o build/wire).
import { chmodSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const [bin = "build/backplane", wire = "build/wire"] = process.argv.slice(2);
for (const f of readdirSync(wire).filter((f) => f.endsWith(".js"))) (0, eval)(readFileSync(`${wire}/${f}`, "utf8"));
const W = (globalThis as any).Wire;
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

function freePort(): number {
  const s = Bun.listen({ hostname: "127.0.0.1", port: 0, socket: { data() {} } });
  const p = s.port;
  s.stop(true);
  return p;
}

const root = mkdtempSync(join(tmpdir(), "bp-desk-"));
const fake = join(root, "bin");
mkdirSync(fake);
// a turn: one answer, then the result
writeFileSync(join(fake, "claude"), `#!/bin/sh
sid="fake-$$"; n=0
while IFS= read -r line; do
  case "$line" in
    *'"type":"user"'*)
      n=$((n + 1))
      printf '%s\\n' '{"type":"system","subtype":"init","session_id":"'"$sid"'","cwd":"/tmp","tools":[],"model":"fake","permissionMode":"default"}'
      sleep 0.3
      printf '%s\\n' '{"type":"assistant","message":{"id":"m'"$$-$n"'","role":"assistant","content":[{"type":"text","text":"All done here."}],"usage":{"input_tokens":1,"output_tokens":1}},"session_id":"'"$sid"'"}'
      printf '%s\\n' '{"type":"result","subtype":"success","is_error":false,"result":"done","session_id":"'"$sid"'","modelUsage":{"fake":{"contextWindow":200000}}}' ;;
  esac
done
`);
chmodSync(join(fake, "claude"), 0o755);

let failed = 0;
const check = (name: string, ok: boolean, got?: unknown) => {
  if (ok) console.log(`ok ${name}`);
  else {
    failed++;
    console.log(`FAIL ${name}${got === undefined ? "" : ": got " + JSON.stringify(got)}`);
  }
};

type Conn = { ws: WebSocket; msgs: any[]; send: (m: string, p: object) => void };
async function connect(port: number): Promise<Conn> {
  const msgs: any[] = [];
  const ws = new WebSocket(`ws://127.0.0.1:${port}/ws`);
  ws.binaryType = "arraybuffer";
  ws.onmessage = (e) => {
    try {
      msgs.push(JSON.parse(typeof e.data === "string" ? e.data : W.decode(new Uint8Array(e.data as ArrayBuffer))));
    } catch {}
  };
  await new Promise((r, j) => { ws.onopen = r; ws.onerror = j; });
  let id = 0;
  return { ws, msgs, send: (m, p) => ws.send(W.encode(JSON.stringify({ id: ++id, m, p }))) };
}

const until = async <T>(ms: number, f: () => T | undefined) => {
  const end = Date.now() + ms;
  for (;;) {
    const v = f();
    if (v) return v;
    if (Date.now() > end) return undefined;
    await sleep(20);
  }
};

const items = (c: Conn) => c.msgs.flatMap((m) => m.items ?? []);
const deskInfo = (c: Conn) => c.msgs.filter((m) => m.t === "info" && m.info && "desk" in m.info).map((m) => m.info.desk);
const ended = (c: Conn, th: string, k: number) =>
  items(c).filter((x) => x.$ === "TurnChanged" && x.thread === th && x.state === "completed").length >= k;

const home = join(root, "home");
const proj = join(root, "proj");
mkdirSync(proj, { recursive: true });
const port = freePort();
const proc = Bun.spawn([resolve(bin), "--home", home, "--port", String(port), "--no-tailscale"], {
  env: { ...process.env, DISPLAY: "", WAYLAND_DISPLAY: "", BACKPLANE_NO_UPDATE: "1", BACKPLANE_ORDER_SERVER: "http://127.0.0.1:9", BACKPLANE_KEYRING_LOCKED: "0", BACKPLANE_PEERS: "", BACKPLANE_AGENTS: fake, PATH: `${fake}:${process.env.PATH}` },
  stdout: "ignore",
  stderr: "ignore",
});
try {
  for (let i = 0; i < 100; i++) {
    try {
      if ((await fetch(`http://127.0.0.1:${port}/hello`)).status === 200) break;
    } catch {}
    await sleep(100);
  }
  const desk = await connect(port);
  const phone = await connect(port);
  desk.send("project.add", { path: proj });
  const pc: any = await until(10000, () => items(desk).find((c) => c.$ === "ProjectCreated"));
  desk.send("thread.create", { project: pc.id, title: "A", env: "local" });
  desk.send("thread.create", { project: pc.id, title: "B", env: "local" });
  const ths: any = await until(10000, () => {
    const t = items(desk).filter((c) => c.$ === "ThreadCreated");
    return t.length >= 2 ? t : undefined;
  });
  const [a, b] = [ths.find((t: any) => t.title === "A").id, ths.find((t: any) => t.title === "B").id];

  desk.send("desk.look", { thread: a, focused: true });
  const heard = await until(5000, () => deskInfo(phone).find((d) => d === a));
  check("the phone hears the watched thread", heard === a, deskInfo(phone));

  desk.send("turn.start", { thread: a, text: "go" });
  await until(15000, () => ended(desk, a, 1) || undefined);
  await sleep(500);
  check("a watched thread's end: no desktop notification", !desk.msgs.some((m) => m.t === "notify"), desk.msgs.filter((m) => m.t === "notify"));

  desk.send("turn.start", { thread: b, text: "go" });
  const n: any = await until(15000, () => desk.msgs.find((m) => m.t === "notify"));
  check("another thread's end notifies the desk", n?.thread === b && n?.title === "B" && n?.kind === "done", n);
  check("the notification carries the answer", n?.body === "All done here.", n?.body);
  await sleep(300);
  check("a phone gets no desktop notification", !phone.msgs.some((m) => m.t === "notify"));

  desk.send("desk.look", { thread: a, focused: false });
  const cleared = await until(5000, () => deskInfo(phone).slice(-1)[0] === "" || undefined);
  check("focus lost: nothing watched", cleared === true, deskInfo(phone));

  desk.send("desk.look", { thread: a, focused: true });
  await until(5000, () => deskInfo(phone).slice(-1)[0] === a || undefined);
  desk.ws.close();
  const left = await until(5000, () => deskInfo(phone).slice(-1)[0] === "" || undefined);
  check("the desk left: nothing watched", left === true, deskInfo(phone));
  phone.ws.close();
} finally {
  proc.kill();
  await proc.exited;
  rmSync(root, { recursive: true, force: true });
}
process.exit(failed ? 1 : 0);
