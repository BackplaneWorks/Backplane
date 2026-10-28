// End to end: create anywhere (docs/bots.md "Threads elsewhere", slice
// "create anywhere"). Two headless hubs pair by themselves. A client of
// alpha creates a thread in beta's project (routed to beta, the answer
// names it `<link>~<id>`), sends a message (stand-in agent answers), forks
// it, pins, archives and brings it back, adds a project on beta (listing
// beta's folders first), and removes it again; everything appears on beta
// and comes back through the mirror to alpha's clients. Refusals: a
// project beta does not share, a delete, an unsigned request. Prints
// "ok ..." / "FAIL ..." lines and timings.
//
//   bun test/tools/create_far_e2e.ts [BINARY] [WIREDIR]
import { mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync, chmodSync, mkdirSync, existsSync, copyFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { createHmac } from "node:crypto";

const args = process.argv.slice(2);
const [bin = "build/backplane", wire = "build/wire"] = args.filter((a) => !a.startsWith("--"));
for (const f of readdirSync(wire).filter((f) => f.endsWith(".js"))) (0, eval)(readFileSync(`${wire}/${f}`, "utf8"));
const W = (globalThis as any).Wire;

let failed = false;
const check = (name: string, ok: boolean, got?: unknown) => {
  console.log(ok ? `ok ${name}` : `FAIL ${name}: ${JSON.stringify(got)?.slice(0, 600)}`);
  if (!ok) failed = true;
};
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
// a free port, from CREATE_FAR_PORT_BASE (default: any the system gives)
let nextPort = Number(process.env.CREATE_FAR_PORT_BASE ?? 0);
function freePort(): number {
  for (;;) {
    try {
      const s = Bun.listen({ hostname: "127.0.0.1", port: nextPort, socket: { data() {} } });
      const p = s.port;
      s.stop(true);
      if (nextPort) nextPort = p + 1;
      return p;
    } catch {
      if (!nextPort) throw new Error("no free port");
      nextPort++;
    }
  }
}

const root = mkdtempSync(join(tmpdir(), "bp-far-e2e-"));
const fake = join(root, "bin");
mkdirSync(fake);
const answer = "pong from the bot";
const claude = `#!/bin/sh
n=0
while IFS= read -r line; do
  case "$line" in
    *'"type":"user"'*)
      case "$line" in *'start bg work'*)
        printf '%s\\n' '{"type":"system","subtype":"task_started","task_id":"bgx","tool_use_id":"tu_bgx","description":"sleep 60","task_type":"local_bash","is_backgrounded":true}'
        printf '%s\\n' '{"type":"result","is_error":false,"result":"started"}'
        continue ;;
      esac
      case "$line" in *'hold until stopped'*)
        printf '%s\\n' '{"type":"assistant","message":{"id":"hold","content":[{"type":"text","text":"waiting for Stop"}]}}'
        continue ;;
      esac
      n=$((n + 1))
      printf '%s\\n' '{"type":"assistant","message":{"id":"m'"$$-$n"'","content":[{"type":"text","text":"${answer}"}]}}'
      printf '%s\\n' '{"type":"result","is_error":false,"result":"${answer}"}' ;;
  esac
done
`;
for (const n of ["claude", "codex", "grok", "muse"]) {
  writeFileSync(join(fake, n), n === "claude" ? claude : "#!/bin/sh\nexit 1\n");
  chmodSync(join(fake, n), 0o755);
}

type Hub = { name: string; home: string; port: number; peers: string; proc: ReturnType<typeof Bun.spawn> | null };
type Client = { ws: WebSocket; seen: any[]; far: any[]; replies: Map<number, any>; n: number; info: any; raw: string[] };

function spawn(h: Hub) {
  h.proc = Bun.spawn([resolve(bin), "--home", h.home, "--port", String(h.port), "--no-tailscale"], {
    env: { ...process.env, DISPLAY: "", WAYLAND_DISPLAY: "", BACKPLANE_NAME: h.name, BACKPLANE_NO_UPDATE: "1", BACKPLANE_ORDER_SERVER: "http://127.0.0.1:9", BACKPLANE_KEYRING_LOCKED: "0", BACKPLANE_PEERS: h.peers, BACKPLANE_AGENTS: fake, PATH: `${fake}:/usr/bin:/bin` },
    stdout: "ignore",
    stderr: "ignore",
  });
}

async function up(h: Hub) {
  for (let i = 0; i < 200; i++) {
    try {
      const r = await fetch(`http://127.0.0.1:${h.port}/hello`);
      if (r.status === 200) return;
    } catch {}
    await sleep(100);
  }
  throw new Error(`${h.name} did not start`);
}

async function connect(h: Hub): Promise<Client> {
  const c: Client = { ws: null as any, seen: [], far: [], replies: new Map(), n: 0, info: {}, raw: [] };
  const ws = new WebSocket(`ws://127.0.0.1:${h.port}/ws`);
  ws.binaryType = "arraybuffer";
  ws.onmessage = (e) => {
    const text = W.decode(new Uint8Array(e.data as ArrayBuffer));
    const o = JSON.parse(text);
    if (o.t === "far") c.raw.push(text);
    for (const x of o.items ?? []) c.seen.push(x);
    if (o.t === "far") c.far.push({ ...o, at: Date.now() });
    if (o.t === "reply") c.replies.set(Number(o.id), o);
    if (o.info) c.info = { ...c.info, ...o.info };
  };
  await new Promise((r, j) => { ws.onopen = r; ws.onerror = j; });
  c.ws = ws;
  return c;
}

async function until<T>(ms: number, f: () => T | undefined | null | false): Promise<T | undefined> {
  const end = Date.now() + ms;
  for (;;) {
    const v = f();
    if (v) return v;
    if (Date.now() > end) return undefined;
    await sleep(50);
  }
}

async function rpc(c: Client, m: string, p: object): Promise<any> {
  const id = ++c.n;
  c.ws.send(W.encode(JSON.stringify({ id, m, p })));
  return await until(20000, () => c.replies.get(id));
}

// every change a client has from a link, oldest first
const farItems = (c: Client, link: string) => c.far.filter((o) => o.link === link).flatMap((o) => o.items.map((i: any) => i.c));
const lastFar = (c: Client, link: string) => c.far.filter((o) => o.link === link).at(-1);


const pa = freePort();
const pb = freePort();
const a: Hub = { name: "alpha", home: join(root, "alpha"), port: pa, peers: `http://127.0.0.1:${pb}`, proc: null };
const b: Hub = { name: "beta", home: join(root, "beta"), port: pb, peers: `http://127.0.0.1:${pa}`, proc: null };
const clients: Client[] = [];
try {
  spawn(a);
  spawn(b);
  await up(a);
  await up(b);
  const ca = await connect(a);
  const cb = await connect(b);
  clients.push(ca, cb);
  const knows = (c: Client, port: number) => String(c.info?.machines ?? "").includes(`127.0.0.1:${port}`);
  const both = await until(130000, () => knows(ca, pb) && knows(cb, pa));
  check("each hub lists the other as the owner's", !!both, [ca.info?.machines, cb.info?.machines]);
  const peer = await until(150000, () => ca.seen.find((c) => c.$ === "PeerSet" && !c.revoked));
  const link = peer?.id as string;
  check("they pair by themselves", typeof link === "string" && link.length > 0, peer);

  // beta has a project (its own), mirrored to alpha
  const board = join(root, "board");
  mkdirSync(board);
  const added = await rpc(cb, "project.add", { path: board });
  check("project.add on beta", !!added?.ok, added);
  const pc = await until(5000, () => cb.seen.find((c) => c.$ === "ProjectCreated" && c.root === board));
  const mirrored = await until(60000, () => farItems(ca, link).find((c) => c.$ === "ProjectCreated" && c.id === pc?.id));
  check("alpha has beta's project, named by the link", !!mirrored, farItems(ca, link).map((c) => c.$));
  const fpid = `${link}~${pc?.id}`;

  // New Thread from alpha's client, in beta's project
  const t0 = Date.now();
  const made = await rpc(ca, "thread.create", { project: fpid, title: "FROM ALPHA" });
  check("thread.create on a far project is answered ok", !!made?.ok, made);
  check("the answer names the thread by its id here", typeof made?.thread === "string" && made.thread.startsWith(`${link}~`), made);
  const th = String(made?.thread ?? "").slice(link.length + 1);
  const onB = await until(8000, () => cb.seen.find((c) => c.$ === "ThreadCreated" && c.id === th));
  check("beta made it, in its own project", !!onB && onB.project === pc?.id && onB.title === "FROM ALPHA", onB);
  check("alpha made none of its own", !ca.seen.some((c) => c.$ === "ThreadCreated" && c.title === "FROM ALPHA"));
  const back = await until(8000, () => farItems(ca, link).find((c) => c.$ === "ThreadCreated" && c.id === th));
  console.log(`  new thread on beta, mirrored to alpha's client: ${Date.now() - t0} ms`);
  check("it comes back through the mirror", !!back && back.project === pc?.id, back);

  // a message; the stand-in agent on beta answers
  const sent = await rpc(ca, "turn.start", { thread: made.thread, text: "hello from alpha", msg: "ca-m1", mode: "queue" });
  check("a message to the far thread", !!sent?.ok, sent);
  check("beta's agent answers into alpha's view", !!await until(20000, () => farItems(ca, link).find((c) => c.$ === "MessagePosted" && c.thread === th && c.text === answer)));

  // fork
  const fork = await rpc(ca, "thread.fork", { thread: made.thread });
  check("thread.fork on a far thread is answered ok, naming the fork here", !!fork?.ok && String(fork.thread).startsWith(`${link}~`) && fork.thread !== made.thread, fork);
  const fth = String(fork?.thread ?? "").slice(link.length + 1);
  const forkB = await until(8000, () => cb.seen.find((c) => c.$ === "ThreadCreated" && c.id === fth));
  check("beta has the fork, in the same project", !!forkB && forkB.project === pc?.id && String(forkB.title).startsWith("Fork:"), forkB);
  check("and a ThreadForked from the source", !!cb.seen.find((c) => c.$ === "ThreadForked" && c.id === fth && c.source === th));
  check("the fork reaches alpha's mirror", !!await until(8000, () => farItems(ca, link).find((c) => c.$ === "ThreadCreated" && c.id === fth)));

  // pin, unpin, archive, unarchive
  const pin = await rpc(ca, "thread.pin", { thread: made.thread });
  check("pin is answered ok", !!pin?.ok, pin);
  check("beta pinned it", !!await until(5000, () => cb.seen.find((c) => c.$ === "ThreadPinned" && c.id === th && c.pinned === true)));
  check("the pin reaches alpha's mirror", !!await until(8000, () => farItems(ca, link).find((c) => c.$ === "ThreadPinned" && c.id === th && c.pinned === true)));
  const unpin = await rpc(ca, "thread.unpin", { thread: made.thread });
  check("unpin", !!unpin?.ok && !!await until(5000, () => cb.seen.find((c) => c.$ === "ThreadPinned" && c.id === th && c.pinned === false)));
  const arch = await rpc(ca, "thread.archive", { thread: `${link}~${fth}` });
  check("archive is answered ok", !!arch?.ok, arch);
  check("beta archived the fork", !!await until(5000, () => cb.seen.find((c) => c.$ === "ThreadArchived" && c.id === fth && c.archived === true)));
  check("the archive reaches alpha's mirror", !!await until(8000, () => farItems(ca, link).find((c) => c.$ === "ThreadArchived" && c.id === fth && c.archived === true)));
  const unarch = await rpc(ca, "thread.unarchive", { thread: `${link}~${fth}` });
  check("unarchive", !!unarch?.ok && !!await until(5000, () => cb.seen.find((c) => c.$ === "ThreadArchived" && c.id === fth && c.archived === false)));

  // add a project on beta from alpha: browse its folders, then add
  const ls = await rpc(ca, "fs.list", { on: link, path: root });
  const entries: string[] = ls?.result?.entries ?? [];
  check("beta's folders are listed to alpha's client", !!ls?.ok && entries.includes("board") && ls?.result?.dir === root, ls);
  const fresh = join(root, "fresh");
  mkdirSync(fresh);
  const t1 = Date.now();
  const padd = await rpc(ca, "project.add", { on: link, path: fresh });
  check("project.add on beta from alpha is answered ok, naming its first thread", !!padd?.ok && String(padd.thread ?? "").startsWith(`${link}~`), padd);
  const pcb = await until(8000, () => cb.seen.find((c) => c.$ === "ProjectCreated" && c.root === fresh));
  check("beta has the project", !!pcb, cb.seen.filter((c) => c.$ === "ProjectCreated").map((c) => c.root));
  check("alpha has none of its own", !ca.seen.some((c) => c.$ === "ProjectCreated" && c.root === fresh));
  check("it reaches alpha's mirror with its first thread", !!await until(8000, () => farItems(ca, link).find((c) => c.$ === "ProjectCreated" && c.id === pcb?.id))
    && !!await until(8000, () => farItems(ca, link).find((c) => c.$ === "ThreadCreated" && c.project === pcb?.id)));
  console.log(`  project added on beta, mirrored on alpha's client: ${Date.now() - t1} ms`);
  const dup = await rpc(ca, "project.add", { on: link, path: fresh });
  check("the same folder twice is refused by beta", !!dup && !dup.ok, dup);
  const mk = await rpc(ca, "project.new", { on: link, path: join(root, "made", "deep") });
  check("project.new makes the folder on beta", !!mk?.ok && existsSync(join(root, "made", "deep")), mk);
  const prm = await rpc(ca, "project.remove", { project: `${link}~${pcb?.id}` });
  check("project.remove on a far project", !!prm?.ok && !!await until(5000, () => cb.seen.find((c) => c.$ === "ProjectRemoved" && c.id === pcb?.id)), prm);

  // refusals
  const nope = await rpc(ca, "thread.create", { project: `${link}~nosuch`, title: "x" });
  check("a project beta does not have is refused", !!nope && !nope.ok, nope);
  const del = await rpc(ca, "thread.delete", { thread: made.thread });
  check("delete of a far thread is refused", !!del && !del.ok && !cb.seen.some((c) => c.$ === "ThreadDeleted" && c.id === th), del);
  const unsigned = await fetch(`http://127.0.0.1:${b.port}/far/rpc`, { method: "POST", body: JSON.stringify({ id: 1, m: "thread.create", p: { project: pc?.id, title: "intruder" } }) });
  await sleep(300);
  check("an unsigned /far/rpc is refused", unsigned.status === 401 || unsigned.status === 403, unsigned.status);
  check("and made nothing", !cb.seen.some((c) => c.$ === "ThreadCreated" && c.title === "intruder"));
  const here = await rpc(ca, "thread.create", { project: `${link}~${pc?.id}`.replace(link + "~", "zz~"), title: "x" });
  check("an unknown link answers an error", !!here && !here.ok, here);
} finally {
  for (const c of clients) { try { c.ws.close(); } catch {} }
  for (const h of [a, b]) if (h.proc) { h.proc.kill(); await h.proc.exited; }
  rmSync(root, { recursive: true, force: true });
}
console.log(failed ? "create far e2e: FAIL" : "create far e2e: ok");
process.exit(failed ? 1 : 0);
