// End to end: streamed text and turn state of threads elsewhere (core/
// farlive.bend, docs/bots.md "Live text of threads elsewhere") on two
// headless hubs that pair by themselves (as far_e2e.ts). A stand-in claude
// on alpha streams a reply a delta at a time; a client of beta must see
// the deltas live (under the far id), then the durable message, and never
// a delta after it. Also: a client that joins mid-stream, a stream that
// starts while beta has no client at all (the sender goes quiet, then the
// receiver asks for the whole text), the owner's alert reaching beta's desk
// once under the owner's key, and the push latency of durable messages.
// Prints "ok ..." / "FAIL ..." lines and timings.
//
//   bun test/tools/live_far_e2e.ts [BINARY] [WIREDIR]
//
// BINARY defaults to build/backplane; WIREDIR to build/wire (the hub's
// CBOR codec: bend test/wire/index.html -o build/wire).
import { mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync, chmodSync, mkdirSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const args = process.argv.slice(2);
const [bin = "build/backplane", wire = "build/wire"] = args.filter((a) => !a.startsWith("--"));
for (const f of readdirSync(wire).filter((f) => f.endsWith(".js"))) (0, eval)(readFileSync(`${wire}/${f}`, "utf8"));
const W = (globalThis as any).Wire;

let failed = false;
const check = (name: string, ok: boolean, got?: unknown) => {
  console.log(ok ? `ok ${name}` : `FAIL ${name}: ${JSON.stringify(got)?.slice(0, 800)}`);
  if (!ok) failed = true;
};
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
function freePort(): number {
  const s = Bun.listen({ hostname: "127.0.0.1", port: 0, socket: { data() {} } });
  const p = s.port;
  s.stop(true);
  return p;
}

const root = mkdtempSync(join(tmpdir(), "bp-live-e2e-"));
const fake = join(root, "bin");
mkdirSync(fake);
// the stand-in claude: "stream N" streams N text deltas of "wK " 0.2 s
// apart and then posts the whole text; anything else answers "pong" at once
const claude = `#!/bin/sh
n=0
while IFS= read -r line; do
  case "$line" in
    *'"type":"user"'*)
      case "$line" in *'stream '*)
        count=$(printf '%s' "$line" | sed -n 's/.*stream \\([0-9][0-9]*\\).*/\\1/p')
        text=""
        i=0
        while [ "$i" -lt "$count" ]; do
          printf '%s\\n' '{"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"w'"$i"' "}}}'
          text="$text"w"$i "
          sleep 0.2
          i=$((i + 1))
        done
        n=$((n + 1))
        printf '%s\\n' '{"type":"assistant","message":{"id":"s'"$$-$n"'","content":[{"type":"text","text":"'"$text"'"}]}}'
        printf '%s\\n' '{"type":"result","is_error":false,"result":"'"$text"'"}'
        continue ;;
      esac
      n=$((n + 1))
      printf '%s\\n' '{"type":"assistant","message":{"id":"m'"$$-$n"'","content":[{"type":"text","text":"pong"}]}}'
      printf '%s\\n' '{"type":"result","is_error":false,"result":"pong"}' ;;
  esac
done
`;
for (const n of ["claude", "codex", "grok", "muse", "omp"]) {
  writeFileSync(join(fake, n), n === "claude" ? claude : "#!/bin/sh\nexit 1\n");
  chmodSync(join(fake, n), 0o755);
}

type Hub = { name: string; home: string; port: number; peers: string; proc: ReturnType<typeof Bun.spawn> | null };
type Ev = { at: number; o: any };
type Client = { ws: WebSocket; seen: any[]; far: any[]; deltas: Ev[]; lives: Ev[]; notifies: Ev[]; replies: Map<number, any>; n: number; info: any };

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
  const c: Client = { ws: null as any, seen: [], far: [], deltas: [], lives: [], notifies: [], replies: new Map(), n: 0, info: {} };
  const ws = new WebSocket(`ws://127.0.0.1:${h.port}/ws`);
  ws.binaryType = "arraybuffer";
  ws.onmessage = (e) => {
    const at = performance.now();
    const text = W.decode(new Uint8Array(e.data as ArrayBuffer));
    const o = JSON.parse(text);
    for (const x of o.items ?? []) if (o.t !== "live") c.seen.push({ ...x, _at: at });
    if (o.t === "far") c.far.push({ ...o, at });
    if (o.t === "delta") c.deltas.push({ at, o });
    if (o.t === "live") c.lives.push({ at, o });
    if (o.t === "notify") c.notifies.push({ at, o });
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
    await sleep(20);
  }
}

async function rpc(c: Client, m: string, p: object): Promise<any> {
  const id = ++c.n;
  c.ws.send(W.encode(JSON.stringify({ id, m, p })));
  return await until(20000, () => c.replies.get(id));
}

const farItems = (c: Client, link: string) => c.far.filter((o) => o.link === link).flatMap((o) => o.items.map((i: any) => ({ ...i.c, _at: o.at })));

// the live text a client shows for a thread: a snapshot replaces, a delta
// appends (client.bend's Ui.live.set / Ui.delta), up to a time
function liveText(c: Client, thread: string, upTo = Infinity): string {
  const evs: { at: number; snap?: string; add?: string }[] = [];
  for (const l of c.lives) {
    const it = (l.o.items ?? []).find((i: any) => i.thread === thread);
    evs.push({ at: l.at, snap: it ? (it.trunc ? "…" : "") + it.text : "" });
  }
  for (const d of c.deltas) if (d.o.thread === thread) evs.push({ at: d.at, add: d.o.text });
  evs.sort((x, y) => x.at - y.at);
  let t = "";
  for (const e of evs) {
    if (e.at > upTo) break;
    if (e.snap !== undefined) t = e.snap;
    else t += e.add;
  }
  return t;
}

const expected = (n: number) => Array.from({ length: n }, (_, i) => `w${i} `).join("");

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
  const peer = await until(150000, () => cb.seen.find((c) => c.$ === "PeerSet" && !c.revoked));
  const link = peer?.id as string;
  check("they pair by themselves", typeof link === "string" && link.length > 0, peer);

  // a project thread on alpha, mirrored on beta
  const projectPath = join(root, "board");
  mkdirSync(projectPath);
  await rpc(ca, "project.add", { path: projectPath });
  const pc = await until(5000, () => ca.seen.find((c) => c.$ === "ProjectCreated" && c.root === projectPath));
  await rpc(ca, "thread.create", { project: pc?.id, title: "LIVE STREAM" });
  const created = await until(5000, () => ca.seen.find((c) => c.$ === "ThreadCreated" && c.project === pc?.id && c.title === "LIVE STREAM"));
  const th = created?.id as string;
  const far = `${link}~${th}`;
  check("the thread is created on alpha", !!th, created);
  check("and mirrored on beta", !!await until(60000, () => farItems(cb, link).find((c) => c.$ === "ThreadCreated" && c.id === th)));

  // desks: one on each hub, watching nothing, so an alert may reach them
  const da = await connect(a);
  const db = await connect(b);
  clients.push(da, db);
  await rpc(da, "desk.look", { thread: "", focused: true });
  await rpc(db, "desk.look", { thread: "", focused: true });

  // 1. a stream, watched from the start: 25 deltas, 0.2 s apart
  const N = 25;
  const t1 = performance.now();
  const sent = await rpc(ca, "turn.start", { thread: th, text: `stream ${N}`, msg: "s1", mode: "queue" });
  check("turn.start stream is answered ok", !!sent?.ok, sent);
  const msg1 = await until(30000, () => farItems(cb, link).find((c) => c.$ === "MessagePosted" && c.thread === th && c.role === "assistant" && String(c.text).startsWith("w0 ")));
  check("beta gets the durable message", !!msg1 && msg1.text === expected(N), msg1);
  const bd = cb.deltas.filter((d) => d.o.thread === far);
  const ad = ca.deltas.filter((d) => d.o.thread === th);
  check("beta's client got deltas under the far id while it streamed", bd.length >= 5, bd.length);
  check("the deltas are the whole text, in order", bd.map((d) => d.o.text).join("") === expected(N), bd.map((d) => d.o.text).join(""));
  check("alpha's own client got them too", ad.map((d) => d.o.text).join("") === expected(N), ad.length);
  const first = bd[0]?.at - t1;
  console.log(`  first far delta ${first.toFixed(0)} ms after the send; ${bd.length} deltas on beta, ${ad.length} on alpha`);
  // lag: when beta had the text alpha's client had at time t
  let cumA = 0, lagSum = 0, lagMax = 0, lagN = 0;
  const marks = ad.map((d) => ({ at: d.at, len: (cumA += d.o.text.length) }));
  let cumB = 0;
  for (const d of bd) {
    cumB += d.o.text.length;
    const m = marks.find((x) => x.len >= cumB);
    if (m) { const lag = d.at - m.at; lagSum += lag; lagMax = Math.max(lagMax, lag); lagN++; }
  }
  console.log(`  delta lag beta vs alpha: avg ${(lagSum / Math.max(lagN, 1)).toFixed(1)} ms, max ${lagMax.toFixed(1)} ms`);
  check("lag stays well under a second", lagN > 0 && lagMax < 1000, { lagMax });
  check("no delta after the message arrived (superseded)", !bd.some((d) => d.at > msg1!._at + 1), { msgAt: msg1?._at, late: bd.filter((d) => d.at > msg1!._at + 1).length });
  check("the live text is empty once it is posted", liveText(cb, far) === "" || liveText(cb, far) === expected(N), liveText(cb, far));
  // (clients clear live text by themselves when the far message folds: client.bend's Ui.far)

  // 2. a client that joins mid-stream gets what streamed so far, then the rest
  await sleep(500);
  const before = performance.now();
  await rpc(ca, "turn.start", { thread: th, text: `stream ${N}`, msg: "s2", mode: "queue" });
  await sleep(2200);
  const cj = await connect(b);
  clients.push(cj);
  const msg2 = await until(30000, () => farItems(cb, link).find((c) => c.$ === "MessagePosted" && c.thread === th && c.role === "assistant" && c._at > before));
  const midText = liveText(cj, far, msg2 ? msg2._at - 1 : Infinity);
  check("a mid-stream joiner has the whole reply live before it posts", !!msg2 && midText === expected(N), { midText, want: expected(N) });
  const snapItem = cj.lives.flatMap((l) => l.o.items ?? []).find((i: any) => i.thread === far);
  check("its snapshot held the text so far", !!snapItem && snapItem.text.length > 0 && expected(N).startsWith(snapItem.text), snapItem);

  // 3. beta has no client when the stream starts: alpha learns it is
  // unwatched, goes quiet, and the text still reaches the client that
  // joins (the receiver asks for the whole)
  for (const c of [cb, db, cj]) c.ws.close();
  await sleep(1500);
  const before3 = performance.now();
  const M = 60;
  await rpc(ca, "turn.start", { thread: th, text: `stream ${M}`, msg: "s3", mode: "queue" });
  await sleep(3000);
  const cq = await connect(b);
  const dq = await connect(b);
  clients.push(cq, dq);
  await rpc(dq, "desk.look", { thread: "", focused: true });
  const msg3 = await until(40000, () => farItems(cq, link).find((c) => c.$ === "MessagePosted" && c.thread === th && c.role === "assistant" && String(c.text).length > expected(30).length));
  const t3text = liveText(cq, far, msg3 ? msg3._at - 1 : Infinity);
  // (the join may be before or after the message; what must hold is that
  // by the time the message arrives the live text was complete)
  console.log(`  quiet case: live text just before the message ${t3text.length} of ${expected(M).length} chars`);
  check("a stream that began unwatched is whole for a client that joined later", !!msg3 && t3text === expected(M), { got: t3text.slice(-60), want: expected(M).slice(-60) });
  await sleep(300);

  // 4. alerts: the owner's turn ended with nobody watching it. Each desk
  // gets one notification under the owner's key, beta's by its far id
  const notes = (c: Client, thread: string) => c.notifies.filter((n) => n.o.thread === thread);
  const na = await until(8000, () => notes(da, th).length > 0 && notes(da, th));
  const nb = await until(8000, () => notes(dq, far).length > 0 && notes(dq, far));
  check("alpha's desk is told once", !!na && na.length >= 1, da.notifies.map((n) => n.o));
  check("beta's desk is told, by the far id and the owner's key", !!nb && nb.every((n) => n.o.key === `turn-${th}` && n.o.kind === "done"), dq.notifies.map((n) => n.o));
  check("and each turn once (three streams so far, beta joined for the last)", !!nb && nb.length <= 3, nb?.length);

  // 5. durable push latency: ten quick turns, alpha client sees the answer
  // vs beta's client sees it mirrored
  const lat: number[] = [];
  for (let i = 0; i < 10; i++) {
    const k = cq.far.length;
    const m = `lat-${i}`;
    await rpc(ca, "turn.start", { thread: th, text: `ping ${i}`, msg: m, mode: "queue" });
    const onA = await until(10000, () => ca.seen.find((c) => c.$ === "MessagePosted" && c.thread === th && c.role === "assistant" && c.text === "pong" && !c._used && (c._used = true)));
    const onB = await until(10000, () => farItems(cq, link).find((c) => c.$ === "MessagePosted" && c.thread === th && c.text === "pong" && c._at > (onA?._at ?? 0) - 1000 && !c._used && (c._used = true)));
    if (onA && onB) lat.push(onB._at - onA._at);
    void k;
  }
  lat.sort((x, y) => x - y);
  console.log(`  durable push latency alpha->beta client: ${lat.map((x) => x.toFixed(0)).join(" ")} ms (median ${lat[Math.floor(lat.length / 2)]?.toFixed(0)})`);
  check("durable messages mirror quickly", lat.length === 10 && lat[Math.floor(lat.length / 2)] < 500, lat);
} finally {
  for (const c of clients) { try { c.ws.close(); } catch {} }
  for (const h of [a, b]) if (h.proc) { h.proc.kill(); await h.proc.exited; }
  rmSync(root, { recursive: true, force: true });
}
console.log(failed ? "live far e2e: FAIL" : "live far e2e: ok");
process.exit(failed ? 1 : 0);
