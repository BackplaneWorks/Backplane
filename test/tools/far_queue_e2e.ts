// End to end: the queue for machines that are away (hub.bend's Fq.*,
// fmcp.bend's PLater) on two headless hubs that pair by themselves
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

const root = mkdtempSync(join(tmpdir(), "bp-far-queue-e2e-"));
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
    env: { ...process.env, DISPLAY: "", WAYLAND_DISPLAY: "", BACKPLANE_NAME: h.name, BACKPLANE_NO_UPDATE: "1", BACKPLANE_KEYRING_LOCKED: "0", BACKPLANE_PEERS: h.peers, BACKPLANE_AGENTS: fake, PATH: `${fake}:/usr/bin:/bin` },
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
  let cb = await connect(b);
  clients.push(ca, cb);
  // as far_e2e: the machines lists first (probed every 2 minutes), then the
  // pairing ask at a minute's tick
  const knows = (c: Client, port: number) => String(c.info?.machines ?? "").includes(`127.0.0.1:${port}`);
  check("each hub lists the other", !!await until(150000, () => knows(ca, pb) && knows(cb, pa)), [ca.info?.machines, cb.info?.machines]);
  const peer = await until(150000, () => cb.seen.find((c) => c.$ === "PeerSet" && !c.revoked));
  const link = peer?.id as string;
  check("they pair by themselves", typeof link === "string" && link.length > 0, peer);
  if (!link) throw new Error("no link: nothing after this can be tested");

  const boardPath = join(root, "board");
  mkdirSync(boardPath);
  await rpc(cb, "project.add", { path: boardPath });
  const pcB = await until(5000, () => cb.seen.find((c) => c.$ === "ProjectCreated" && c.root === boardPath));
  await rpc(cb, "thread.create", { project: pcB?.id, title: "REMOTE REVIEW" });
  const tB = await until(5000, () => cb.seen.find((c) => c.$ === "ThreadCreated" && c.project === pcB?.id && c.title === "REMOTE REVIEW"));
  const pt = tB?.id as string;
  await sleep(1100);
  const homePath = join(root, "home");
  mkdirSync(homePath);
  await rpc(ca, "project.add", { path: homePath });
  const pcA = await until(5000, () => ca.seen.find((c) => c.$ === "ProjectCreated" && c.root === homePath));
  await rpc(ca, "thread.create", { project: pcA?.id, title: "ALPHA AGENT" });
  const tA = await until(5000, () => ca.seen.find((c) => c.$ === "ThreadCreated" && c.project === pcA?.id && c.title === "ALPHA AGENT"));
  const ta = tA?.id as string;
  check("each hub has its thread", !!pt && !!ta, [tB, tA]);
  let ml: any;
  for (let i = 0; i < 600; i++) {
    ml = await mcp(a, ta, "machine_list", {});
    const m = ml.body?.machines?.find?.((x: any) => x.machine === "beta");
    if (m?.projects?.some?.((p: any) => p.id === `${link}~${pcB?.id}`)) break;
    await sleep(200);
  }
  const farId = `${link}~${pt}`;

  // beta goes away
  cb.ws.close();
  b.proc!.kill();
  await b.proc!.exited;
  let away: any;
  for (let i = 0; i < 400; i++) {
    away = await mcp(a, ta, "machine_list", {});
    if (away.body?.machines?.find?.((x: any) => x.machine === "beta")?.state === "away") break;
    await sleep(250);
  }
  check("alpha sees beta away", away.body?.machines?.find?.((x: any) => x.machine === "beta")?.state === "away", away.body);

  // a send waits; an interrupt is still refused
  const q = await mcp(a, ta, "thread_send", { threadId: farId, text: "queued while away" });
  check("thread_send to an away machine is queued, not refused", !q.isError && q.body?.queued === true && q.body?.machine === "beta", q.body);
  const intr = await mcp(a, ta, "thread_interrupt", { threadId: farId });
  check("thread_interrupt to an away machine is still refused", intr.isError && JSON.stringify(intr.body).includes("away"), intr.body);
  const kept = await until(5000, () => ca.seen.find((c) => c.$ === "SettingSet" && c.key === "far.queue" && String(c.value).includes("thread_send")));
  check("the call is kept in the queue (far.queue)", !!kept, kept);

  // beta is back: the call goes there once, and alpha's thread is told
  spawn(b);
  await up(b);
  cb = await connect(b);
  clients.push(cb);
  const t0 = Date.now();
  const got = await until(200000, () => cb.seen.find((c) => c.$ === "MessagePosted" && c.thread === pt && String(c.text).includes("queued while away")));
  check("the queued send reaches beta's thread", !!got, cb.seen.filter((c) => c.$ === "MessagePosted" && c.thread === pt).map((c) => c.text));
  console.log(`  delivered ${Date.now() - t0} ms after beta came back`);
  const told = await until(30000, () => ca.seen.find((c) => c.$ === "MessagePosted" && c.thread === ta && String(c.text).includes("Your queued thread_send for beta reached it")));
  check("alpha's thread is told it reached beta", !!told, ca.seen.filter((c) => c.$ === "MessagePosted" && c.thread === ta).map((c) => c.text));
  const cleared = await until(10000, () => ca.seen.find((c) => c.$ === "SettingSet" && c.key === "far.queue" && c.value === ""));
  check("and the queue is empty again", !!cleared, cleared);
  // another minute: nothing is sent twice
  await sleep(70000);
  const r2 = await fetch(`http://127.0.0.1:${pb}/hello`).then(() => true).catch(() => false);
  const ev = readFileSync(join(b.home, "events.jsonl"), "utf8").split("\n").filter((l) => l.includes("queued while away") && l.includes('"MessagePosted"'));
  check("delivered exactly once", r2 && ev.length === 1, ev.length);
  const toldN = readFileSync(join(a.home, "events.jsonl"), "utf8").split("\n").filter((l) => l.includes("Your queued thread_send for beta") && l.includes('"MessagePosted"'));
  check("told exactly once", toldN.length === 1, toldN.length);

  // --- at most once end to end: the queue key is the request's p.key, and
  // beta keeps its answer under it (fmcp.bend's Fm.seen.*) ---
  const secret = JSON.parse(readFileSync(join(a.home, "secrets", "peers", link), "utf8")).secret;
  let nonce = 0;
  const signed = async (raw: string) => {
    // as the sender signs every attempt: a fresh nonce first (far.bend's Fr.nonced)
    const bodyText = `{"n":"t${Date.now()}-${++nonce}",` + raw.slice(1);
    const ts = Math.floor(Date.now() / 1000);
    const sig = "sha256=" + createHmac("sha256", Buffer.from(secret, "hex")).update(`${ts}.${bodyText}`).digest("hex");
    const r = await fetch(`http://127.0.0.1:${pb}/far/rpc`, { method: "POST", body: bodyText,
      headers: { "content-type": "application/json", "x-backplane-peer": link, "x-backplane-timestamp": String(ts), "x-backplane-signature": sig } });
    return { status: r.status, text: await r.text() };
  };
  const onBeta = (needle: string) => readFileSync(join(b.home, "events.jsonl"), "utf8").split("\n").filter((l) => l.includes(needle) && l.includes('"MessagePosted"')).length;

  // (a) a reply lost, then the same call again: delivered once, the same answer
  const keyed = JSON.stringify({ id: 0, m: "mcp.call", p: { key: "lost-1", tool: "thread_send", from: `${link}~${ta}`, args: { threadId: pt, text: "lost reply once" } } });
  const la1 = await signed(keyed);
  const la2 = await signed(keyed);
  await sleep(1500);
  check("(a) a keyed call sent again after its reply was lost is answered the same", la1.status === 200 && la2.status === 200 && la1.text === la2.text && JSON.parse(la1.text).mcp?.ok === true, [la1, la2]);
  check("(a) and delivered once", onBeta("lost reply once") === 1, onBeta("lost reply once"));
  const unkeyed = JSON.stringify({ id: 0, m: "mcp.call", p: { tool: "thread_list", from: `${link}~${ta}`, args: {} } });
  const u1 = await signed(unkeyed);
  check("(a) an unkeyed call is served as before", u1.status === 200 && JSON.parse(u1.text).mcp?.ok === true, u1);

  // (b) the sender crashes after its call got there, then sends it again
  cb.ws.close();
  b.proc!.kill();
  await b.proc!.exited;
  for (let i = 0; i < 400; i++) {
    const m = await mcp(a, ta, "machine_list", {});
    if (m.body?.machines?.find?.((x: any) => x.machine === "beta")?.state === "away") break;
    await sleep(250);
  }
  const qb = await mcp(a, ta, "thread_send", { threadId: farId, text: "crash after send once" });
  check("(b) queued while beta is away", !qb.isError && qb.body?.queued === true, qb.body);
  const qline = await until(5000, () => [...ca.seen].reverse().find((c) => c.$ === "SettingSet" && c.key === "far.queue" && String(c.value).includes("crash after send once")));
  const line = String(qline?.value ?? "").split("\n").find((l: string) => l.includes("crash after send once")) ?? "";
  const qkey = line.split("\t")[0];
  const qbody = line.split("\t").slice(5).join("\t");
  check("(b) the queued request carries its queue key", qkey.length > 0 && JSON.parse(qbody || "{}").p?.key === qkey, [qkey, qbody.slice(0, 200)]);
  // beta comes back; the call reaches it (sent as alpha would, with its key)
  // and alpha dies before it learns the outcome
  spawn(b);
  await up(b);
  const cbx = await connect(b);
  clients.push(cbx);
  await until(150000, () => String(cbx.info?.machines ?? "").includes(`127.0.0.1:${pa}`));
  const delivered = await signed(qbody);
  a.proc!.kill();
  await a.proc!.exited;
  check("(b) the call got there, then the sender died", delivered.status === 200 && JSON.parse(delivered.text).mcp?.ok === true, delivered);
  // alpha restarts with the call still in its queue (unless it took the answer first) and sends it again
  spawn(a);
  await up(a);
  const ca3 = await connect(a);
  clients.push(ca3);
  const toldB = await until(240000, () => readFileSync(join(a.home, "events.jsonl"), "utf8").split("\n").some((l) => l.includes("Your queued thread_send for beta reached it") && l.includes(qkey)) || ca3.seen.find((c) => c.$ === "MessagePosted" && c.thread === ta && String(c.msg).includes(qkey)));
  check("(b) the restarted sender sends it again and is told", !!toldB, toldB);
  await sleep(2000);
  check("(b) delivered once", onBeta("crash after send once") === 1, onBeta("crash after send once"));

  // (c) the queue is bounded: 100 calls wait, the 101st is refused
  b.proc!.kill();
  await b.proc!.exited;
  for (let i = 0; i < 400; i++) {
    const m = await mcp(a, ta, "machine_list", {});
    if (m.body?.machines?.find?.((x: any) => x.machine === "beta")?.state === "away") break;
    await sleep(250);
  }
  // the bound is the stored call's UTF-8 bytes, keyed (fmcp.bend's Fm.later.res)
  const big = await mcp(a, ta, "thread_launch", { machine: "beta", project: "board", prompt: "x".repeat(70000) });
  check("(c) a call over 64 KiB is refused", big.isError && JSON.stringify(big.body).includes("full"), JSON.stringify(big.body).slice(0, 200));
  const emoji = await mcp(a, ta, "thread_send", { threadId: farId, text: "\u{1F600}".repeat(16400) });
  check("(c) multibyte: 16400 emoji (65600 UTF-8 bytes, far fewer characters) are refused", emoji.isError && JSON.stringify(emoji.body).includes("full"), JSON.stringify(emoji.body).slice(0, 160));
  const quotes = await mcp(a, ta, "thread_send", { threadId: farId, text: '"'.repeat(33000) });
  check("(c) escaped: 33000 quotes (66000 bytes once JSON-escaped) are refused", quotes.isError && JSON.stringify(quotes.body).includes("full"), JSON.stringify(quotes.body).slice(0, 160));
  const qlines = () => String([...ca3.seen].reverse().find((c) => c.$ === "SettingSet" && c.key === "far.queue")?.value ?? "").split("\n").filter(Boolean);
  // the exact edge: the longest ASCII text whose keyed call still fits
  let lo = 60000, hi = 65536, best = -1;
  while (lo <= hi) {
    const mid = (lo + hi) >> 1;
    const r = await mcp(a, ta, "thread_send", { threadId: farId, text: `E${mid}:` + "a".repeat(mid) });
    if (!r.isError && r.body?.queued === true) { best = mid; lo = mid + 1; } else hi = mid - 1;
  }
  await sleep(1000);
  const edge = qlines().find((l) => l.includes(`E${best}:`)) ?? "";
  const edgeBody = edge.split("\t").slice(5).join("\t");
  const edgeBytes = Buffer.byteLength(edgeBody, "utf8");
  check("(c) envelope: the largest call kept is at most 64 KiB as stored, keyed, and the bound is tight", best > 0 && edgeBytes <= 65536 && edgeBytes > 65536 - 16 && JSON.parse(edgeBody).p?.key?.length > 0, { best, edgeBytes });
  const past = await mcp(a, ta, "thread_send", { threadId: farId, text: `E${best + 1}:` + "a".repeat(best + 1) });
  check("(c) one byte past that edge is refused", past.isError && JSON.stringify(past.body).includes("full"), JSON.stringify(past.body).slice(0, 160));
  check("(c) a refused call is never kept", !qlines().some((l) => l.includes("\u{1F600}\u{1F600}") || l.includes(`E${best + 1}:`)), qlines().length);
  let queued = qlines().length;
  for (let i = queued; i < 100; i++) {
    const q = await mcp(a, ta, "thread_send", { threadId: farId, text: `fill ${i}` });
    if (!q.isError && q.body?.queued === true) queued++;
  }
  const over = await mcp(a, ta, "thread_send", { threadId: farId, text: "one too many" });
  check("(c) 100 calls wait for an away machine", queued === 100, queued);
  check("(c) the 101st is refused: the queue is full", over.isError && JSON.stringify(over.body).includes("full"), over.body);
} catch (e) {
  console.log("FAIL threw: " + String(e));
  failed = true;
} finally {
  for (const c of clients) { try { c.ws.close(); } catch {} }
  for (const h of [a, b]) { try { h.proc?.kill(); } catch {} }
  rmSync(root, { recursive: true, force: true });
}
console.log(failed ? "far queue e2e: FAIL" : "far queue e2e: ok");
process.exit(failed ? 1 : 0);
