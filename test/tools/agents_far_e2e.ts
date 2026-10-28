// End to end: agents across machines (core/fmcp.bend, docs/bots.md
// "Agents across machines") on two headless hubs that pair by themselves
// (as far_e2e.ts). An agent of alpha (a stand-in: the test calls alpha's
// MCP endpoint /mcp/<thread> as the agent would) lists beta's machine and
// projects, lists its threads, sends to one (beta's stand-in answers),
// waits for it, reads it, interrupts a running one, launches a new thread in
// beta's project and waits for that, and gives its own conversation to a
// thread and a project there with /give. Also checks that errors surface
// (an unknown machine, a thread not shared, an unlinked name) and that the
// signed route refuses what it should (unsigned, an unlisted tool).
// Prints "ok ..." / "FAIL ..." lines and timings.
//
//   bun test/tools/agents_far_e2e.ts [BINARY] [WIREDIR]
//
// BINARY defaults to build/backplane; WIREDIR to build/wire.
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
function freePort(): number {
  const s = Bun.listen({ hostname: "127.0.0.1", port: 0, socket: { data() {} } });
  const p = s.port;
  s.stop(true);
  return p;
}

const root = mkdtempSync(join(tmpdir(), "bp-agents-far-e2e-"));
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


let mcpId = 0;
async function mcp(h: Hub, thread: string, name: string, args: object): Promise<{ isError: boolean; body: any }> {
  const r: any = await fetch(`http://127.0.0.1:${h.port}/mcp/${thread}`, {
    method: "POST", headers: { "content-type": "application/json" },
    body: JSON.stringify({ jsonrpc: "2.0", id: ++mcpId, method: "tools/call", params: { name, arguments: args } }),
  }).then((x) => x.json());
  const text = r.result?.content?.[0]?.text;
  let body: any = r;
  try { body = text ? JSON.parse(text) : r; } catch { body = text; }
  return { isError: !!r.result?.isError || !!r.error, body };
}

const pa = 39410;
const pb = 39411;
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
  const peer = await until(150000, () => cb.seen.find((c) => c.$ === "PeerSet" && !c.revoked));
  const peerA = await until(8000, () => ca.seen.find((c) => c.$ === "PeerSet" && !c.revoked));
  const link = peer?.id as string;
  check("they pair by themselves", typeof link === "string" && link.length > 0 && peerA?.id === link, [peer, peerA]);

  // beta has a project with a thread; alpha has a project with the agent's thread
  const boardPath = join(root, "board");
  mkdirSync(boardPath);
  await rpc(cb, "project.add", { path: boardPath });
  const pcB = await until(5000, () => cb.seen.find((c) => c.$ === "ProjectCreated" && c.root === boardPath));
  await rpc(cb, "thread.create", { project: pcB?.id, title: "REMOTE REVIEW" });
  const tB = await until(5000, () => cb.seen.find((c) => c.$ === "ThreadCreated" && c.project === pcB?.id && c.title === "REMOTE REVIEW"));
  const pt = tB?.id as string;
  check("beta has a project and a thread", !!pcB && !!pt, [pcB, tB]);
  // ids are time + counter: alpha makes its things a second later so the two hubs' ids never coincide
  await sleep(1100);
  const homePath = join(root, "home");
  mkdirSync(homePath);
  await rpc(ca, "project.add", { path: homePath });
  const pcA = await until(5000, () => ca.seen.find((c) => c.$ === "ProjectCreated" && c.root === homePath));
  await rpc(ca, "thread.create", { project: pcA?.id, title: "ALPHA AGENT" });
  const tA = await until(5000, () => ca.seen.find((c) => c.$ === "ThreadCreated" && c.project === pcA?.id && c.title === "ALPHA AGENT"));
  const ta = tA?.id as string;
  check("alpha has the agent's thread", !!ta, tA);
  await rpc(ca, "turn.start", { thread: ta, text: "a conversation to give", msg: "ag-1", mode: "queue" });
  await until(20000, () => ca.seen.find((c) => c.$ === "MessagePosted" && c.thread === ta && c.text === answer));

  // the agent asks which machines there are, until beta's project has reached alpha's mirror
  const t0 = Date.now();
  let ml: any;
  for (let i = 0; i < 600; i++) {
    ml = await mcp(a, ta, "machine_list", {});
    const m = ml.body?.machines?.find?.((x: any) => x.machine === "beta");
    if (m?.projects?.some?.((p: any) => p.id === `${link}~${pcB?.id}`)) break;
    await sleep(200);
  }
  const mach = ml?.body?.machines?.find?.((x: any) => x.machine === "beta");
  check("machine_list shows this machine first and beta with its project's far id", !ml.isError && ml.body.machines[0].machine === "this" && mach?.state === "here" && mach?.link === link
    && mach?.projects?.some?.((p: any) => p.id === `${link}~${pcB?.id}` && p.name === "board"), ml?.body);
  console.log(`  beta's project reached alpha's agent's view in ${Date.now() - t0} ms`);

  // thread_list on beta
  const lst = await mcp(a, ta, "thread_list", { machine: "beta" });
  const farId = `${link}~${pt}`;
  check("thread_list machine=beta lists its threads in far ids", !lst.isError && lst.body.threads?.some?.((t: any) => t.id === farId && t.title === "REMOTE REVIEW" && t.project === `${link}~${pcB?.id}`), lst.body);
  const lstP = await mcp(a, ta, "thread_list", { machine: "beta", project: "board" });
  check("and narrowed to a project", !lstP.isError && lstP.body.threads?.some?.((t: any) => t.title === "REMOTE REVIEW") && lstP.body.threads.every((t: any) => t.project === `${link}~${pcB?.id}`), lstP.body);
  const local = await mcp(a, ta, "thread_list", {});
  check("thread_list with no machine is alpha's own", !local.isError && local.body.threads?.some?.((t: any) => t.id === ta) && !local.body.threads.some((t: any) => String(t.id).includes("~")), local.body);

  // send, and beta's stand-in answers; wait; read
  const t1 = Date.now();
  const sent = await mcp(a, ta, "thread_send", { threadId: farId, text: "hello from alpha's agent" });
  check("thread_send to a thread on beta is ok", !sent.isError && sent.body.state === "sent" && sent.body.threadId === farId, sent.body);
  const onB = await until(10000, () => cb.seen.find((c) => c.$ === "MessagePosted" && c.thread === pt && c.text.includes("hello from alpha's agent")));
  check("beta's thread got it", !!onB, cb.seen.filter((c) => c.$ === "MessagePosted" && c.thread === pt).map((c) => c.text));
  check("and it says whose thread it came from", !!onB && String(onB.msg).includes(`${link}~${ta}`), onB);
  const waited = await mcp(a, ta, "thread_wait", { threadId: farId, timeoutMs: 40000 });
  check("thread_wait on it ends done with beta's answer", !waited.isError && waited.body.done === true && waited.body.lastReply === answer && waited.body.threadId === farId, waited.body);
  console.log(`  send to answer, waited: ${Date.now() - t1} ms`);
  let read: any;
  for (let i = 0; i < 100; i++) {
    read = await mcp(a, ta, "thread_read", { threadId: farId });
    if (read.body?.entries?.some?.((e: any) => e.text === answer)) break;
    await sleep(100);
  }
  check("thread_read shows the exchange from the mirror", read.body?.entries?.some?.((e: any) => e.text === "hello from alpha's agent") && read.body.entries.some((e: any) => e.text === answer), read.body);

  // a running turn is interrupted from afar
  const hold = await mcp(a, ta, "thread_send", { threadId: farId, text: "hold until stopped" });
  check("send of a turn that holds", !hold.isError, hold.body);
  await until(8000, () => cb.seen.find((c) => c.$ === "TurnChanged" && c.thread === pt && c.state === "running" && c.at >= Math.floor(t1 / 1000)));
  const w0 = await mcp(a, ta, "thread_wait", { threadId: farId, timeoutMs: 2000 });
  check("thread_wait times out while it runs (not done)", !w0.isError && w0.body.done === false, w0.body);
  const intr = await mcp(a, ta, "thread_interrupt", { threadId: farId });
  check("thread_interrupt stops it", !intr.isError && intr.body.state === "interrupted", intr.body);
  check("beta sees the interrupt", !!await until(8000, () => cb.seen.find((c) => c.$ === "TurnChanged" && c.thread === pt && c.state === "interrupted")));
  const ren = await mcp(a, ta, "thread_update", { threadId: farId, title: "RENAMED BY ALPHA" });
  check("thread_update renames it there", !ren.isError && !!await until(8000, () => cb.seen.find((c) => c.$ === "ThreadRenamed" && c.id === pt && c.title === "RENAMED BY ALPHA")), ren.body);

  // launch a new thread in beta's project
  const t2 = Date.now();
  const launched = await mcp(a, ta, "thread_launch", { machine: "beta", project: "board", prompt: "start from alpha's agent", title: "LAUNCHED FROM ALPHA" });
  const newId = String(launched.body?.threadId ?? "");
  check("thread_launch machine=beta makes a thread there, answered in a far id", !launched.isError && newId.startsWith(`${link}~`), launched.body);
  const madeB = await until(8000, () => cb.seen.find((c) => c.$ === "ThreadCreated" && c.title === "LAUNCHED FROM ALPHA" && c.project === pcB?.id));
  check("it is a thread of beta's project", !!madeB && `${link}~${madeB.id}` === newId, madeB);
  const w1 = await mcp(a, ta, "thread_wait", { threadId: newId, timeoutMs: 40000 });
  check("thread_wait on it ends done with the answer", !w1.isError && w1.body.done === true && w1.body.lastReply === answer, w1.body);
  console.log(`  launch to answer, waited: ${Date.now() - t2} ms`);
  const viaId = await mcp(a, ta, "thread_launch", { project: `${link}~${pcB?.id}`, prompt: "by project id", title: "BY PROJECT ID" });
  check("thread_launch with the project's far id works too", !viaId.isError && String(viaId.body.threadId).startsWith(`${link}~`), viaId.body);

  // errors are surfaced
  const nope = await mcp(a, ta, "thread_send", { threadId: `${link}~nosuch`, text: "x" });
  check("a thread beta does not have is an error", nope.isError && /not shared|not on this machine/.test(JSON.stringify(nope.body)), nope.body);
  const gamma = await mcp(a, ta, "thread_list", { machine: "gamma" });
  check("a machine not linked is an error", gamma.isError && JSON.stringify(gamma.body).includes("gamma"), gamma.body);
  const noproj = await mcp(a, ta, "thread_launch", { machine: "beta", project: "no such project", prompt: "x" });
  check("a project beta does not have is an error", noproj.isError, noproj.body);
  const wrongLink = await mcp(a, ta, "thread_send", { threadId: "zzzz~t1", text: "x" });
  check("a far id of no linked machine is an error, nothing sent", wrongLink.isError, wrongLink.body);

  // /give to a thread and a project on beta
  await sleep(2000); // the rename reaches alpha's mirror
  const given = await rpc(ca, "bots.give", { thread: ta, text: "/give >beta:renamed-by-alpha with a note" });
  check("/give >beta:slug is ok", !!given?.ok, given);
  // a handoff waits in the thread's queue for its next message, as a local /give does
  check("beta's thread got the conversation queued", !!await until(15000, () => cb.seen.find((c) => c.$ === "TurnQueued" && c.thread === pt && String(c.msg).includes("give:") && c.text.includes("a conversation to give"))), cb.seen.filter((c) => c.$ === "TurnQueued" && c.thread === pt).map((c) => String(c.text).slice(0, 80)));
  check("and it did not start a turn by itself", !cb.seen.find((c) => c.$ === "MessagePosted" && c.thread === pt && c.text.includes("a conversation to give")));
  const givenP = await rpc(ca, "bots.give", { thread: ta, text: "/give %beta:board" });
  check("/give %beta:project is ok", !!givenP?.ok, givenP);
  check("beta's project got a new thread with the conversation queued", !!await until(15000, () => cb.seen.find((c) => c.$ === "TurnQueued" && c.text.includes("a conversation to give") && c.thread !== pt && cb.seen.find((d) => d.$ === "ThreadCreated" && d.id === c.thread && d.project === pcB?.id))));
  const givenX = await rpc(ca, "bots.give", { thread: ta, text: "/give >beta:no-such-thread" });
  check("/give to a thread beta does not have is an error", givenX && givenX.ok === false, givenX);

  // the signed route's limits
  const secret = JSON.parse(readFileSync(join(a.home, "secrets", "peers", link), "utf8")).secret;
  const signed = async (h: Hub, bodyObj: object, sign = true) => {
    const body = JSON.stringify(bodyObj);
    const ts = Math.floor(Date.now() / 1000);
    const sig = "sha256=" + createHmac("sha256", Buffer.from(secret, "hex")).update(`${ts}.${body}`).digest("hex");
    return await fetch(`http://127.0.0.1:${h.port}/far/rpc`, { method: "POST", body,
      headers: { "content-type": "application/json", "x-backplane-peer": link, "x-backplane-timestamp": String(ts), "x-backplane-signature": sign ? sig : "sha256=00" } });
  };
  const call = (tool: string, args: object) => ({ id: 0, m: "mcp.call", p: { tool, from: "x~y", args } });
  const unsigned = await signed(b, call("thread_list", {}), false);
  check("an unsigned mcp.call is refused", unsigned.status === 401 || unsigned.status === 403, unsigned.status);
  const okSigned = await signed(b, call("thread_list", {}));
  const okBody: any = await okSigned.json().catch(() => null);
  check("a signed mcp.call from the owner's machine is answered", okSigned.status === 200 && okBody?.mcp?.ok === true && okBody.mcp.body.threads.length > 0, okBody);
  const bad = await signed(b, call("work_run", { command: "touch /tmp/pwned" }));
  check("a tool off the list is refused", bad.status === 403, bad.status);
  const unshared = await signed(b, call("thread_send", { threadId: "nosuch", text: "x" }));
  const unsharedBody: any = await unshared.json().catch(() => null);
  check("a thread beta does not share is refused, with a message", unshared.status === 200 && unsharedBody?.mcp?.ok === false, unsharedBody);
} finally {
  for (const c of clients) { try { c.ws.close(); } catch {} }
  for (const h of [a, b]) if (h.proc) { h.proc.kill(); await h.proc.exited; }
  rmSync(root, { recursive: true, force: true });
}
console.log(failed ? "agents far e2e: FAIL" : "agents far e2e: ok");
process.exit(failed ? 1 : 0);
