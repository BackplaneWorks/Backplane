// End to end: a project added or made from a client starts with a thread
// (Hub.project_add), sent before the reply so the client that asked can
// select it; a removed project's folder brings it back with no new thread.
// Prints "ok ..." / "FAIL ..." lines.
//
//   bun test/tools/proj_thread_e2e.ts [BINARY] [WIREDIR]
//
// BINARY defaults to build/backplane; WIREDIR to build/wire (bend
// test/wire/index.html -o build/wire).
import { existsSync, mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync, chmodSync, mkdirSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const [bin = "build/backplane", wire = "build/wire"] = process.argv.slice(2);
for (const f of readdirSync(wire).filter((f) => f.endsWith(".js"))) (0, eval)(readFileSync(`${wire}/${f}`, "utf8"));
const W = (globalThis as any).Wire;

let failed = false;
const check = (name: string, ok: boolean, got?: unknown) => {
  console.log(ok ? `ok ${name}` : `FAIL ${name}: ${JSON.stringify(got)?.slice(0, 400)}`);
  if (!ok) failed = true;
};
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
async function until<T>(ms: number, f: () => T | undefined | null | false): Promise<T | undefined> {
  const end = Date.now() + ms;
  for (;;) {
    const v = f();
    if (v) return v;
    if (Date.now() > end) return undefined;
    await sleep(50);
  }
}
function freePort(): number {
  const s = Bun.listen({ hostname: "127.0.0.1", port: 0, socket: { data() {} } });
  const p = s.port;
  s.stop(true);
  return p;
}

// no real agents: every provider exits at once
const root = mkdtempSync(join(tmpdir(), "bp-projthread-e2e-"));
const fake = join(root, "bin");
mkdirSync(fake);
for (const n of ["claude", "codex", "grok", "muse"]) {
  writeFileSync(join(fake, n), "#!/bin/sh\nexit 1\n");
  chmodSync(join(fake, n), 0o755);
}
const home = join(root, "home");
const user = join(root, "user");
mkdirSync(join(user, "kart"), { recursive: true });
const port = freePort();
const proc = Bun.spawn([resolve(bin), "--home", home, "--port", String(port), "--no-tailscale"], {
  env: { ...process.env, HOME: user, DISPLAY: "", WAYLAND_DISPLAY: "", BACKPLANE_NO_UPDATE: "1", BACKPLANE_ORDER_SERVER: "http://127.0.0.1:9", BACKPLANE_KEYRING_LOCKED: "0", BACKPLANE_PEERS: "", BACKPLANE_AGENTS: fake, PATH: `${fake}:${process.env.PATH}` },
  stdout: "ignore",
  stderr: process.env.E2E_LOG ? "inherit" : "ignore",
});

try {
  for (let i = 0; i < 100; i++) {
    try {
      if ((await fetch(`http://127.0.0.1:${port}/hello`)).status === 200) break;
    } catch {}
    await sleep(100);
  }
  // every message in arrival order: changes and replies
  const order: any[] = [];
  const replies = new Map<number, any>();
  const ws = new WebSocket(`ws://127.0.0.1:${port}/ws`);
  ws.binaryType = "arraybuffer";
  ws.onmessage = (e) => {
    const o = JSON.parse(W.decode(new Uint8Array(e.data as ArrayBuffer)));
    for (const c of o.items ?? []) order.push(c);
    if (o.t === "reply") { replies.set(Number(o.id), o); order.push({ $: "reply", id: Number(o.id) }); }
  };
  await new Promise((r, j) => { ws.onopen = r; ws.onerror = j; });
  await sleep(300);
  let n = 0;
  const rpc = async (m: string, p: object) => {
    const id = ++n;
    ws.send(W.encode(JSON.stringify({ id, m, p })));
    return [id, await until(10000, () => replies.get(id))] as const;
  };
  const project = (r: string) => order.find((c) => c.$ === "ProjectCreated" && c.root === r);
  const threads = (pid: string) => order.filter((c) => c.$ === "ThreadCreated" && c.project === pid);
  const at = (f: (c: any) => boolean) => order.findIndex(f);

  // made: a new folder, its project, its first thread, then the answer
  const [id1, r1] = await rpc("project.new", { path: join(user, "fan") });
  const fan = project(join(user, "fan"));
  check("made: answered ok", r1?.ok === true, r1);
  check("made: folder", existsSync(join(user, "fan")));
  check("made: one thread there", !!fan && threads(fan.id).length === 1, order);
  check("made: the answer names its thread", !!fan && r1?.thread === threads(fan.id)[0]?.id, r1);
  check("made: the thread comes before the answer", !!fan && at((c) => c.$ === "ThreadCreated" && c.project === fan.id) < at((c) => c.$ === "reply" && c.id === id1), order);

  // added: an existing folder the same
  const [, r2] = await rpc("project.add", { path: join(user, "kart") });
  const kart = project(join(user, "kart"));
  check("added: answered ok", r2?.ok === true, r2);
  check("added: one thread there", !!kart && threads(kart.id).length === 1, order);

  // the same folder again is refused and starts nothing
  const [, r3] = await rpc("project.add", { path: join(user, "kart") });
  check("again: refused", r3?.ok === false, r3);
  check("again: no second thread", !!kart && threads(kart.id).length === 1, order);

  // removed and added again: back with its thread, no new one
  await rpc("project.remove", { project: kart?.id });
  const [, r4] = await rpc("project.add", { path: join(user, "kart") });
  check("back: answered ok", r4?.ok === true, r4);
  check("back: no new thread", !!kart && threads(kart.id).length === 1, order);
  check("back: the answer names none", r4?.thread === undefined, r4);

  // two clients create at once: each answer names that client's thread
  // (client.bend's Made.reply selects only it; laws made_*)
  const other: any = { replies: new Map<number, any>() };
  const ws2 = new WebSocket(`ws://127.0.0.1:${port}/ws`);
  ws2.binaryType = "arraybuffer";
  ws2.onmessage = (e) => {
    const o = JSON.parse(W.decode(new Uint8Array(e.data as ArrayBuffer)));
    if (o.t === "reply") other.replies.set(Number(o.id), o);
  };
  await new Promise((r, j) => { ws2.onopen = r; ws2.onerror = j; });
  await sleep(300);
  const before = threads(fan!.id).length;
  const mine = ++n;
  ws.send(W.encode(JSON.stringify({ id: mine, m: "thread.create", p: { project: fan!.id, title: "mine" } })));
  ws2.send(W.encode(JSON.stringify({ id: 1, m: "thread.create", p: { project: fan!.id, title: "theirs" } })));
  const ra = await until(10000, () => replies.get(mine));
  const rb = await until(10000, () => other.replies.get(1));
  await until(5000, () => threads(fan!.id).length >= before + 2);
  const tMine = threads(fan!.id).find((c) => c.title === "mine")?.id;
  const tTheirs = threads(fan!.id).find((c) => c.title === "theirs")?.id;
  check("at once: each answer names its own thread", !!tMine && !!tTheirs && ra?.thread === tMine && rb?.thread === tTheirs, [ra, rb, tMine, tTheirs]);
  // a fork's answer names the fork
  const [, rf] = await rpc("thread.fork", { thread: tMine });
  const fork = order.filter((c) => c.$ === "ThreadCreated" && c.title === "Fork: mine").pop();
  check("fork: the answer names the fork", !!fork && rf?.thread === fork.id, [rf, fork]);
  ws2.close();
  ws.close();
} finally {
  proc.kill();
  await proc.exited;
  rmSync(root, { recursive: true, force: true });
}
process.exit(failed ? 1 : 0);
