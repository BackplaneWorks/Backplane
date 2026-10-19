// End to end: threads elsewhere (far.bend, docs/bots.md "Threads
// elsewhere") on two headless hubs that pair by themselves (no invite:
// client.bend's Mate.*). Each hub lists the
// other as one of the owner's machines (BACKPLANE_PEERS), so each shares
// its projects, bots and threads with the other. Checks that a bot made on
// alpha appears on beta, mirrored and named by the link; that writing to
// it from beta reaches alpha and alpha's answer comes back; that a client
// joining beta later gets the mirror whole; that when alpha stops, beta
// keeps it, away; and that beta, restarted while alpha is down, shows it
// from disk, away. Agents are stand-ins (claude answers "pong from the
// bot"). Prints "ok ..." / "FAIL ..." lines and timings.
//
//   bun test/tools/far_e2e.ts [BINARY] [WIREDIR]
//
// BINARY defaults to build/backplane; WIREDIR to build/wire (the hub's
// CBOR codec: bend test/wire/index.html -o build/wire). With
// FAR_E2E_DUMP=DIR, beta's log and the messages a client got (the
// directory, the mirror) are written there for test/native/ui_snap.bend.
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
for (const n of ["claude", "codex", "grok", "muse", "omp"]) {
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
  // each lists the other among the owner's machines (it answered /hello;
  // the list is probed every 2 minutes)
  const knows = (c: Client, port: number) => String(c.info?.machines ?? "").includes(`127.0.0.1:${port}`);
  const tm = Date.now();
  const both = await until(130000, () => knows(ca, pb) && knows(cb, pa));
  check("each hub lists the other as the owner's", !!both, [ca.info?.machines, cb.info?.machines]);
  console.log(`  machines lists ready after ${Date.now() - tm} ms`);

  // they pair by themselves, no invite pasted: the hub with the lower
  // address asks the other (POST /bots/pair) at the minute tick
  // (client.bend's Mate.*), and each keeps one link
  const tp = Date.now();
  const peer = await until(150000, () => cb.seen.find((c) => c.$ === "PeerSet" && !c.revoked));
  const peerA = await until(8000, () => ca.seen.find((c) => c.$ === "PeerSet" && !c.revoked));
  const link = peer?.id as string;
  check("they pair by themselves", typeof link === "string" && link.length > 0 && peerA?.id === link, [peer, peerA]);
  console.log(`  paired ${Date.now() - tp} ms after both listed each other`);
  await sleep(65000);
  check("and only once (the next minute asks nothing)", cb.seen.filter((c) => c.$ === "PeerSet").length === 1 && ca.seen.filter((c) => c.$ === "PeerSet").length === 1,
    [cb.seen.filter((c) => c.$ === "PeerSet"), ca.seen.filter((c) => c.$ === "PeerSet")]);
  const asked = await fetch(`http://127.0.0.1:${pa}/bots/pair?name=beta&url=http://127.0.0.1:${pb}`, { method: "POST" });
  const askedB = await fetch(`http://127.0.0.1:${pb}/bots/pair?name=alpha&url=http://127.0.0.1:${pa}`, { method: "POST" });
  check("a linked machine asking again is refused", asked.status === 409 && askedB.status === 409, [asked.status, askedB.status]);

  // a bot on alpha appears on beta, mirrored
  const t0 = Date.now();
  const miso = await rpc(ca, "bots.create", { name: "miso", persona: "a test bot" });
  check("bots.create on alpha", !!miso?.ok, miso);
  // (the first sync can wait for the machines list and the minute tick; a
  // push arrives as soon as both know each other as the owner's)
  const got = await until(90000, () => farItems(cb, link).find((c) => c.$ === "BotSet" && c.name === "miso"));
  const tMirror = Date.now() - t0;
  check("beta gets alpha's bot, in alpha's ids, from its link", !!got && got.id === miso?.bot, got);
  const th = got?.thread as string;
  check("and the bot's thread", !!farItems(cb, link).find((c) => c.$ === "ThreadCreated" && c.id === th), farItems(cb, link).map((c) => c.$));
  console.log(`  mirror of a new bot reached beta's client in ${tMirror} ms`);
  check("never a setting, a peer or a device", !farItems(cb, link).some((c) => ["SettingSet", "PeerSet", "DeviceRegistered", "HookSet", "RoomPosted"].includes(c.$)),
    farItems(cb, link).map((c) => c.$));

  // beta writes to it: the request goes to alpha in alpha's ids, and the answer comes back
  const t1 = Date.now();
  const sent = await rpc(cb, "turn.start", { thread: `${link}~${th}`, text: "hello from beta", msg: "far-m1", mode: "queue" });
  check("turn.start on the far thread is answered ok", !!sent?.ok, sent);
  const onA = await until(10000, () => ca.seen.find((c) => c.$ === "MessagePosted" && c.thread === th && c.text.includes("hello from beta")));
  check("alpha's thread gets beta's message", !!onA, ca.seen.filter((c) => c.$ === "MessagePosted"));
  const back = await until(20000, () => farItems(cb, link).find((c) => c.$ === "MessagePosted" && c.thread === th && c.text === answer));
  check("alpha's answer comes back to beta", !!back, farItems(cb, link).filter((c) => c.$ === "MessagePosted"));
  console.log(`  beta's message to alpha's answer mirrored on beta: ${Date.now() - t1} ms`);
  const mine = farItems(cb, link).filter((c) => c.$ === "MessagePosted" && c.thread === th && c.text.includes("hello from beta"));
  check("beta sees its own message once, with its msg id", mine.length === 1 && mine[0].msg === "far-m1", mine);

  // Ordinary project threads use the same mirror and routing path.
  const projectPath = join(root, "board");
  mkdirSync(projectPath);
  const project = await rpc(ca, "project.add", { path: projectPath });
  const pc = await until(5000, () => ca.seen.find((c) => c.$ === "ProjectCreated" && c.root === projectPath));
  const ordinary = await rpc(ca, "thread.create", { project: pc?.id ?? project?.id, title: "REMOTE BOARD REVIEW" });
  const created = await until(5000, () => ca.seen.find((c) => c.$ === "ThreadCreated" && c.project === pc?.id && c.title === "REMOTE BOARD REVIEW"));
  const pt = created?.id;
  check("ordinary project thread created", !!ordinary?.ok && !!pt, [ordinary, created]);
  const catalog = await until(8000, () => farItems(cb, link).find((c) => c.$ === "ThreadCreated" && c.id === pt));
  // a client joining beta later gets alpha's catalog with the mirror (the
  // one the last pull brought: a project made since arrives by its push),
  // each project's last number at or below the mirror's head
  const cbc = await connect(b);
  clients.push(cbc);
  const cat = await until(8000, () => cbc.far.find((f) => f.link === link && Array.isArray(f.catalog) && f.catalog.length > 0));
  check("a joining client gets the catalog, caught up", !!cat && cat.catalog.every((p: any) => p.n <= cat.head), cat ? { head: cat.head, catalog: cat.catalog } : cbc.far.map((f) => Object.keys(f)));
  check("ordinary project and thread are mirrored together", !!catalog && catalog.project === pc?.id && farItems(cb, link).some((c) => c.$ === "ProjectCreated" && c.id === pc?.id), catalog);
  const psent = await rpc(cb, "turn.start", { thread: `${link}~${pt}`, text: "review from beta", msg: "ordinary-m1", mode: "queue" });
  check("ordinary thread send reaches its owner", !!psent?.ok && !!await until(8000, () => ca.seen.find((c) => c.$ === "MessagePosted" && c.thread === pt && c.text.includes("review from beta"))), psent);
  check("ordinary thread answer returns through the mirror", !!await until(8000, () => farItems(cb, link).find((c) => c.$ === "MessagePosted" && c.thread === pt && c.text === answer)));

  // work the owner's agent left running travels with its thread (WorkSet),
  // so the mirror shows it monitoring
  await rpc(cb, "turn.start", { thread: `${link}~${pt}`, text: "start bg work", msg: "ordinary-bg", mode: "queue" });
  const bgRow = await until(8000, () => farItems(cb, link).find((c) => c.$ === "WorkSet" && c.thread === pt && String(c.rows).includes("sleep 60")));
  check("a yielded background task reaches the mirror", !!bgRow, farItems(cb, link).filter((c) => c.thread === pt).map((c) => c.$));
  const doneAfter = await until(8000, () => { const ts = farItems(cb, link).filter((c) => c.$ === "TurnChanged" && c.thread === pt); return ts.at(-1)?.state === "completed" ? true : undefined; });
  check("the mirror has the turn over with the task still at work", !!doneAfter && !farItems(cb, link).some((c) => c.$ === "WorkSet" && c.thread === pt && c.rows === ""));

  // Stop goes to the owner's live agent, not a process on the mirror.
  await rpc(cb, "turn.start", { thread: `${link}~${th}`, text: "hold until stopped", msg: "far-hold", mode: "queue" });
  const running = await until(8000, () => ca.seen.find((c) => c.$ === "MessagePosted" && c.msg === "far-hold"));
  check("remote hold starts on alpha", !!running);
  const stopped = await rpc(cb, "turn.interrupt", { thread: `${link}~${th}` });
  const stopOnA = await until(8000, () => ca.seen.find((c) => c.$ === "TurnChanged" && c.thread === th && c.state === "interrupted"));
  check("remote Stop reaches alpha", !!stopped?.ok && !!stopOnA, [stopped, stopOnA]);
  check("remote Stop is mirrored", !!await until(8000, () => farItems(cb, link).find((c) => c.$ === "TurnChanged" && c.thread === th && c.state === "interrupted")));

  // A real approval waits on alpha; beta answers the namespaced ask.
  const approval = fetch(`http://127.0.0.1:${a.port}/mcp/${th}`, {
    method: "POST", headers: { "content-type": "application/json" },
    body: JSON.stringify({ jsonrpc: "2.0", id: 9, method: "tools/call", params: { name: "project_create", arguments: { path: join(root, "declined-project") } } }),
  }).then((r) => r.json());
  const ask = await until(8000, () => farItems(cb, link).find((c) => c.$ === "AskOpened" && c.tool === "project_create"));
  check("alpha's approval appears on beta", !!ask, ask);
  const approved = await rpc(cb, "ask.answer", { id: `${link}~${ask?.id}`, answer: "decline" });
  const approvalResult = await approval;
  check("remote approval response reaches alpha", !!approved?.ok && !!await until(8000, () => ca.seen.find((c) => c.$ === "AskClosed" && c.id === ask?.id && c.answer === "decline")), [approved, approvalResult]);
  check("declined approval does not create a folder", !existsSync(join(root, "declined-project")));

  // a request beta may not send on
  const del = await rpc(cb, "thread.delete", { thread: `${link}~${th}` });
  check("a delete of a far thread is refused", !!del && !del.ok, del);
  check("and alpha's thread is still there", !ca.seen.some((c) => c.$ === "ThreadDeleted" && c.id === th));

  // a client joining later gets the mirror whole
  const t2 = Date.now();
  const cb2 = await connect(b);
  clients.push(cb2);
  const snap = await until(5000, () => lastFar(cb2, link));
  console.log(`  a new client on beta has the mirror ${Date.now() - t2} ms after connecting`);
  check("a new client gets the mirror whole", !!snap && snap.since === 0 && snap.items.some((i: any) => i.c.$ === "BotSet") && !snap.away, snap && { ...snap, items: snap.items.length });
  const dupes = new Set<number>();
  let twice = false;
  for (const i of snap?.items ?? []) { if (dupes.has(i.n)) twice = true; dupes.add(i.n); }
  check("each item once", !twice);
  check("the mirror is kept on disk", existsSync(join(b.home, "far", `${link}.jsonl`)));
  const dump = process.env.FAR_E2E_DUMP;
  if (dump) {
    mkdirSync(dump, { recursive: true });
    copyFileSync(join(b.home, "events.jsonl"), join(dump, "events.jsonl"));
    const info = JSON.stringify({ t: "info", info: { "bots.remote": cb.info["bots.remote"] ?? "[]" } });
    writeFileSync(join(dump, "info.txt"), [info, ...cb2.raw].join("\n") + "\n");
    writeFileSync(join(dump, "ids.txt"), `${link}~${th}\n${link}~${miso?.bot}\n${link}~${pt}\n`);
    copyFileSync(join(a.home, "events.jsonl"), join(dump, "owner-events.jsonl"));
    mkdirSync(join(dump, "far"), { recursive: true });
    copyFileSync(join(b.home, "far", `${link}.jsonl`), join(dump, "far", `${link}.jsonl`));
  }

  // alpha stops: beta keeps it, away (within the minute's directory)
  const t3 = Date.now();
  a.proc!.kill();
  await a.proc!.exited;
  a.proc = null;
  const away = await until(75000, () => { const f = lastFar(cb, link); return f && f.away ? f : undefined; });
  check("beta marks alpha away", !!away, lastFar(cb, link));
  console.log(`  alpha away on beta ${Date.now() - t3} ms after it stopped`);
  const cb3 = await connect(b);
  clients.push(cb3);
  const stale = await until(5000, () => lastFar(cb3, link));
  check("still listed while away", !!stale && stale.away && stale.items.some((i: any) => i.c.$ === "BotSet" && i.c.name === "miso"), stale && { ...stale, items: stale.items.length });
  const refused = await rpc(cb3, "turn.start", { thread: `${link}~${th}`, text: "are you there", msg: "far-m2", mode: "queue" });
  check("writing to it while away says so", !!refused && !refused.ok && String(refused.error).includes("away"), refused);

  // beta restarts while alpha is down: the mirror comes back from disk, away
  for (const c of [cb, cb2, cb3]) c.ws.close();
  b.proc!.kill();
  await b.proc!.exited;
  spawn(b);
  await up(b);
  const cb4 = await connect(b);
  clients.push(cb4);
  const disk = await until(8000, () => lastFar(cb4, link));
  check("after a restart the mirror shows from disk, away", !!disk && disk.away && disk.items.some((i: any) => i.c.$ === "BotSet"), disk && { ...disk, items: disk.items.length });
  // Finish beta's first discovery while alpha is still down. This makes
  // the initial owner-return hint arrive before beta recognizes alpha,
  // rather than letting startup timing accidentally hide that case.
  const absent = await until(10000, () => cb4.info?.machines && !String(cb4.info.machines).includes(`127.0.0.1:${a.port}`));
  check("restart discovery observes the stopped owner", !!absent, cb4.info?.machines);

  // alpha comes back: beta pulls and is here again
  spawn(a);
  await up(a);
  const ca2 = await connect(a);
  clients.push(ca2);
  const catchup = await rpc(ca2, "turn.start", { thread: th, text: "after partition", msg: "catchup-m1", mode: "queue" });
  check("owner can post after return", !!catchup?.ok, catchup);
  const here = await until(75000, () => { const f = lastFar(cb4, link); return f && !f.away ? f : undefined; });
  check("alpha back: here again", !!here, lastFar(cb4, link));
  check("missed message catches up after return", !!await until(10000, () => farItems(cb4, link).find((c) => c.$ === "MessagePosted" && c.msg === "catchup-m1")));

  // Replace only this temporary owner's history. A new epoch must clear
  // its old mirror even though the replacement has a lower sequence.
  const oldEpoch = lastFar(cb4, link)?.epoch;
  ca2.ws.close();
  a.proc!.kill();
  await a.proc!.exited;
  const peerChange = ca.seen.find((c) => c.$ === "PeerSet" && c.id === link);
  const resetAt = Math.floor(Date.now() / 1000) + 1;
  const replacement = [
    { ...peerChange, at: resetAt },
    { $: "ThreadCreated", id: "reset-thread", project: "", title: "New history", env: "local", provider: "claude", at: resetAt },
    { $: "BotSet", id: "reset-bot", name: "new-history", thread: "reset-thread", look: 2, persona: "", at: resetAt },
  ];
  check("replacement keeps the linked peer", !!peerChange, peerChange);
  writeFileSync(join(a.home, "events.jsonl"), replacement.map((c) => JSON.stringify(c)).join("\n") + "\n");
  spawn(a);
  await up(a);
  const ca3 = await connect(a);
  clients.push(ca3);
  const resetTurn = await rpc(ca3, "turn.start", { thread: "reset-thread", text: "new history", msg: "reset-message", mode: "queue" });
  check("replacement owner accepts a new turn", !!resetTurn?.ok && !!await until(5000, () => ca3.seen.find((c) => c.$ === "MessagePosted" && c.msg === "reset-message")), resetTurn);
  const reset = await until(75000, () => cb4.far.find((f) => f.link === link && f.epoch && f.epoch !== oldEpoch && f.items.some((i: any) => i.c.$ === "BotSet" && i.c.id === "reset-bot")));
  check("history replacement takes its lower sequence", !!reset && reset.head < disk.head,
    reset ? { epoch: reset.epoch, head: reset.head, old: disk.head } : { machines: { owner: ca3.info?.machines, mirror: cb4.info?.machines }, last: lastFar(cb4, link), owner: ca3.seen.slice(-8) });
  const mirrorPath = join(b.home, "far", `${link}.jsonl`);
  check("history replacement clears old disk projection", !!reset && !readFileSync(mirrorPath, "utf8").includes(miso.bot));

  // A late old push is only a hint to pull the current authority. It
  // cannot put the old history back, or advance the new mirror to 999.
  // (a push only counts from a machine this hub lists as the owner's, and
  // that list is probed every 2 minutes: wait for the replacement owner, or
  // the hub rightly refuses the push and the check sees a 403, not the epoch)
  const listed = await until(150000, () => String(cb4.info?.machines ?? "").includes(`127.0.0.1:${pa}`));
  check("the replacement owner is listed again", !!listed, cb4.info?.machines);
  const secret = JSON.parse(readFileSync(join(a.home, "secrets", "peers", link), "utf8")).secret;
  const delayed = JSON.stringify({ name: "alpha", epoch: oldEpoch, since: 0, head: 999, items: [{ n: 999, c: { $: "BotSet", id: miso.bot, name: "stale-history", thread: th, look: 1, at: resetAt } }] });
  const ts = Math.floor(Date.now() / 1000);
  const sig = "sha256=" + createHmac("sha256", Buffer.from(secret, "hex")).update(`${ts}.${delayed}`).digest("hex");
  const late = await fetch(`http://127.0.0.1:${b.port}/far/push`, { method: "POST", body: delayed,
    headers: { "content-type": "application/json", "x-backplane-peer": link, "x-backplane-timestamp": String(ts), "x-backplane-signature": sig } });
  await sleep(500);
  check("late push cannot restore an old epoch", late.ok && lastFar(cb4, link)?.epoch === reset?.epoch && lastFar(cb4, link)?.head < 999 && !readFileSync(mirrorPath, "utf8").includes("stale-history"));
} finally {
  for (const c of clients) { try { c.ws.close(); } catch {} }
  for (const h of [a, b]) if (h.proc) { h.proc.kill(); await h.proc.exited; }
  rmSync(root, { recursive: true, force: true });
}
console.log(failed ? "far e2e: FAIL" : "far e2e: ok");
process.exit(failed ? 1 : 0);
