// Actual throwaway hubs, the hub's wire codec, stand-in agents only.
// bun test/tools/star_nudge_e2e.ts [BINARY] [WIREDIR]
// Build artifacts must already exist; this test never builds or uses a live home.
// Linux disk-fault cases require the unprivileged util-linux prlimit command.
import { appendFileSync, chmodSync, existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, statSync, writeFileSync } from "node:fs";
import { createHmac, randomBytes } from "node:crypto";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { ThinViews } from "../../src/web/thin.js";

const [binary = "build/backplane", wire = "build/wire"] = process.argv.slice(2);
for (const f of readdirSync(wire).filter(f => f.endsWith(".js"))) (0, eval)(readFileSync(join(wire, f), "utf8"));
const W = (globalThis as any).Wire;
if (!W?.encode || !W?.decode) throw Error(`No Wire codec in ${wire}`);
const key = "star.nudge.v1", offerKey = "star.nudge.offer";
const root = mkdtempSync(join(tmpdir(), "bp-star-nudge-"));
const fake = join(root, "agents"), user = join(root, "user");
for (const d of [fake, user]) mkdirSync(d);
// Exiting stand-ins keep acceptance independent of model output, auth and usage.
// BACKPLANE_AGENTS also blocks providers absent from this directory.
for (const name of ["claude", "codex", "grok", "muse", "opencode", "antigravity", "agy", "pi", "omp", "gh", "tailscale"]) {
  const path = join(fake, name);
  writeFileSync(path, '#!/bin/sh\ncase "$1" in --version) echo "fixture 1.0.0"; exit 0;; esac\nexit 1\n');
  chmodSync(path, 0o755);
}
// An accepted bots.tell uses a linked loopback peer, never another machine.
const peer = Bun.serve({ hostname: "127.0.0.1", port: 0, fetch: request => new URL(request.url).pathname === "/hello"
  ? new Response(W.encode(JSON.stringify({ backplane: "0.11.3", busy: 0 })), { headers: { "content-type": "application/cbor" } })
  : Response.json({ ok: true }) });
let failures = 0, checks = 0, originalChecks = 0, originalFailures = 0;
const check = (name: string, ok: unknown, got?: unknown) => {
  checks++;
  console.log(`${ok ? "ok" : "FAIL"} ${name}${ok ? "" : ": " + JSON.stringify(got)?.slice(0, 900)}`);
  if (!ok) failures++;
};
const same = (a: unknown, b: unknown) => JSON.stringify(a) === JSON.stringify(b);
const pause = (ms: number) => new Promise(r => setTimeout(r, ms));
async function until<T>(label: string, f: () => T | undefined | false, ms = 10000): Promise<T> {
  const end = Date.now() + ms;
  do { const v = f(); if (v !== undefined && v !== false) return v; await pause(25); } while (Date.now() < end);
  throw Error(`timed out: ${label}`);
}
const event = ($: string, fields: object) => ({ $, ...fields, at: 1 });
type Row = Record<string, any>;
type Client = { ws: WebSocket; frames: Row[]; replies: Map<number, Row>; n: number; mode: "legacy" | "web" | "mobile"; views?: ThinViews; session?: Row; error?: Error };
type Nudge = { status: string; ids: string[]; token: string; eligibleAt: number; sentAt: number; decidedAt?: number };

class Fixture {
  home: string;
  project: string;
  port: number;
  log: string;
  proc?: ReturnType<typeof Bun.spawn>;
  clients: Client[] = [];
  starts = 0;
  discoverPeer = false;
  target = { kind: "thread", id: "b" };
  wakeTickMs = "60000";
  stderrDrain?: Promise<void>;
  constructor(readonly name: string, extra: Row[] = []) {
    this.home = join(root, name);
    this.project = join(this.home, "project");
    for (const d of [this.home, this.project]) mkdirSync(d);
    this.log = join(this.home, "events.jsonl");
    const socket = Bun.listen({ hostname: "127.0.0.1", port: 0, socket: { data() {} } });
    this.port = socket.port; socket.stop(true);
    if (this.port === 3787) throw Error("refusing the live port");
    const thread = (id: string) => event("ThreadCreated", { id, project: "p", title: id, env: "local", branch: "", worktree: this.project, provider: "claude", model: "fixture" });
    writeFileSync(this.log, [
      event("ProjectCreated", { id: "p", title: "Star nudge fixture", root: this.project }), thread("a"), thread("b"), thread("bot-thread"),
      event("BotSet", { id: "bot", name: "fixture", thread: "bot-thread", look: 0, persona: "" }),
      event("RoomSet", { id: "room", name: "Fixture room", members: "you" }),
      event("PeerSet", { id: "peer", name: "fixture-peer", url: `http://127.0.0.1:${peer.port}`, revoked: false }),
      event("SettingSet", { key: "restart.continue", value: "off" }),
      event("SettingSet", { key: "fallback.on", value: "off" }),
      event("SettingSet", { key: "telemetry.share", value: "off" }),
      event("SettingSet", { key: "onboard.done", value: "1" }),
      event("SettingSet", { key: "onboard.uber", value: "1" }),
      // Pre-feature history is not evidence of five accepted client RPCs.
      ...Array.from({ length: 8 }, (_, i) => event("MessagePosted", { thread: "b", msg: `historical-${i}`, role: "user", text: "Earlier history" })),
      ...extra,
    ].map(x => JSON.stringify(x)).join("\n") + "\n");
  }
  events(): Row[] {
    const raw = readFileSync(this.log, "utf8");
    // Ignore only an incomplete final append; never hide malformed complete rows.
    return raw.slice(0, raw.lastIndexOf("\n") + 1).split("\n").filter(Boolean).map(x => JSON.parse(x));
  }
  writes(): Row[] { return this.events().filter(x => x.$ === "SettingSet" && x.key === key); }
  state(): Nudge | undefined { const row = this.writes().at(-1); return row ? JSON.parse(row.value) : undefined; }
  offers(c: Client): Row[] { return c.frames.filter(f => f.info && Object.hasOwn(f.info, offerKey)); }
  async start() {
    const out = join(this.home, `hub-${++this.starts}`);
    // No inherited auth selectors, real client profiles, telemetry, updates,
    // KiStack sync, peer discovery or Tailscale. Only generated fixture homes.
    const env = {
      HOME: user, USERPROFILE: user, XDG_CONFIG_HOME: join(user, "config"), XDG_CACHE_HOME: join(user, "cache"), XDG_DATA_HOME: join(user, "data"),
      CODEX_HOME: join(user, "codex"), CLAUDE_CONFIG_DIR: join(user, "claude"), GEMINI_HOME: join(user, "gemini"),
      PATH: `${fake}:${dirname(process.execPath)}:/usr/bin:/bin`, TMPDIR: root, DISPLAY: "", WAYLAND_DISPLAY: "", CI: "1", DO_NOT_TRACK: "1",
      BACKPLANE_AGENTS: fake, BACKPLANE_NO_UPDATE: "1", BACKPLANE_KISTACK_SYNC: "0", BACKPLANE_TELEMETRY: "0", BEND_NO_TELEMETRY: "1",
      BACKPLANE_WAKE_TICK_MS: this.wakeTickMs,
      BACKPLANE_PEERS: this.discoverPeer ? `http://127.0.0.1:${peer.port}` : "", BACKPLANE_ORDER_SERVER: "http://127.0.0.1:9", BACKPLANE_KEYRING_LOCKED: "0", BACKPLANE_LOCK_DIR: join(this.home, "locks"),
    };
    writeFileSync(out + ".stderr", "");
    // exec keeps the PID prlimit targets; ignoring SIGXFSZ turns the fault
    // into EFBIG. The parent drains stderr, outside the hub's file limit.
    this.proc = Bun.spawn(["/bin/sh", "-c", 'trap "" XFSZ; exec "$@"', "sh", resolve(binary), "--home", this.home, "--port", String(this.port), "--host", "127.0.0.1", "--no-tailscale", "--headless", "--foreground"], {
      env, stdout: "ignore", stderr: "pipe",
    });
    const stderr = this.proc.stderr;
    this.stderrDrain = (async () => { for await (const chunk of stderr as ReadableStream<Uint8Array>) appendFileSync(out + ".stderr", chunk); })();
    const end = Date.now() + 30000;
    do {
      if (this.proc.exitCode !== null) throw Error(`${this.name}: hub exited ${this.proc.exitCode}: ${readFileSync(out + ".stderr", "utf8").slice(-2500)}`);
      try { if ((await fetch(`http://127.0.0.1:${this.port}/hello`, { signal: AbortSignal.timeout(1000) })).ok) return; } catch {}
      await pause(50);
    } while (Date.now() < end);
    throw Error(`${this.name}: hub readiness timeout: ${readFileSync(out + ".stderr", "utf8").slice(-2500)}`);
  }
  async connect(mode: Client["mode"] = "legacy"): Promise<Client> {
    const token = existsSync(join(this.home, "token")) ? readFileSync(join(this.home, "token"), "utf8").trim() : "";
    const query = mode === "legacy" ? "" : mode === "web" ? "&view=1&frontend=1" : "&view=1&mobile=1";
    const c: Client = { ws: new WebSocket(`ws://127.0.0.1:${this.port}/ws?token=${encodeURIComponent(token)}${query}`), frames: [], replies: new Map(), n: 10000, mode };
    this.clients.push(c);
    c.ws.binaryType = "arraybuffer";
    c.ws.onmessage = e => {
      try {
        const f = JSON.parse(typeof e.data === "string" ? e.data : W.decode(new Uint8Array(e.data as ArrayBuffer)));
        c.frames.push(f);
        if (f.t === "reply") c.replies.set(Number(f.id), f);
      } catch (error) { c.error = error as Error; }
    };
    await new Promise<void>((yes, no) => {
      const timer = setTimeout(() => no(Error("websocket open timeout")), 10000);
      c.ws.onopen = () => { clearTimeout(timer); yes(); };
      c.ws.onerror = () => { clearTimeout(timer); no(Error("websocket open error")); };
    });
    // A reply is also a deterministic readiness/arrival-order barrier.
    await this.barrier(c);
    if (mode !== "legacy") {
      const greeting = await until(`${this.name}: ${mode} greeting`, () => c.frames.find(f => f.t === "view"));
      c.views = new ThinViews({ now: () => performance.now(), send: (_hub: string, command: Row) => command, command: (command: Row) => command });
      if (!c.views.connect("fixture", greeting).connected) throw Error(`${mode} thin greeting rejected`);
      c.session = { sel: this.target.kind === "bot" ? "bot-thread" : "b", rid: 1, drafts: {}, open: {}, adding: false, panel: false, info: {} };
      await this.picture(c, true);
    }
    return c;
  }
  send(c: Client, m: string, p: Row = {}): number {
    const id = ++c.n;
    // Updated request shape, with an explicitly legacy (unnamed) owner. These
    // legacy wire observers need no thin-view account capability handshake.
    c.ws.send(W.encode(JSON.stringify({ id, m, p: { accountProfiles: 1, accountBinding: "", ...p } })));
    return id;
  }
  async reply(c: Client, id: number, ms = 10000): Promise<Row> {
    return until(`${this.name}: reply ${id}`, () => { if (c.error) throw c.error; return c.replies.get(id); }, ms);
  }
  async rpc(c: Client, m: string, p: Row = {}): Promise<Row> {
    const id = this.send(c, m, p);
    return this.reply(c, id);
  }
  async viewReply(c: Client, call: Row): Promise<Row> {
    if (call.failed || !call.id || call.cmds.length !== 1) throw Error(`thin request failed: ${JSON.stringify(call)}`);
    c.ws.send(W.encode(JSON.stringify(call.cmds[0])));
    const raw = await this.reply(c, call.id, 20000), taken = c.views!.reply("fixture", raw);
    if (!taken?.current || !taken.remoteReady || !taken.shown) throw Error(`thin receiver rejected picture: ${JSON.stringify(taken)}`);
    c.session = taken.session;
    return taken;
  }
  async picture(c: Client, opening = false, extra: Row = {}): Promise<Row> {
    const call = opening ? c.views!.select("fixture", this.target, c.session, `star-${c.mode}`)
      : c.views!.request("fixture", extra.action ? "view.action" : "view.update", { session: c.session, ...extra });
    return this.viewReply(c, call);
  }
  async invalidatedPicture(c: Client, start: number): Promise<Row> {
    let index = start;
    const call = await until(`${this.name}: ${c.mode} invalidation`, () => {
      if (c.error) throw c.error;
      while (index < c.frames.length) {
        const notice = c.frames[index++];
        if (notice.t !== "view.changed") continue;
        const result = c.views!.changed("fixture", notice, c.session, { id: "", offset: 0 });
        if (result.failed) throw Error(`thin invalidation rejected: ${result.failed}`);
        if (result.id) return result;
      }
    }, 15000);
    return this.viewReply(c, call);
  }
  limit(bytes: number | "unlimited") {
    if (process.platform !== "linux" || !this.proc || this.proc.exitCode !== null) throw Error("fsize fault requires a live owned Linux fixture");
    const result = Bun.spawnSync(["prlimit", "--pid", String(this.proc.pid), `--fsize=${bytes}:`], { stdout: "pipe", stderr: "pipe" });
    if (result.exitCode !== 0) throw Error(`unprivileged prlimit failed: ${new TextDecoder().decode(result.stderr)}`);
  }
  async barrier(c: Client) {
    const reply = await this.rpc(c, "setting.set", { key: "fixture.star.barrier", value: String(c.n + 1) });
    if (!reply.ok) throw Error(`fixture barrier rejected: ${JSON.stringify(reply)}`);
  }
  async accepted(c: Client, m: string, p: Row) {
    const reply = await this.rpc(c, m, p);
    if (!reply.ok) throw Error(`${m} should accept fixture message: ${JSON.stringify(reply)}`);
    return reply;
  }
  async stop(signal: "SIGTERM" | "SIGKILL" = "SIGTERM") {
    for (const c of this.clients) c.ws.close();
    if (this.proc && this.proc.exitCode === null) {
      this.proc.kill(signal);
      const timer = setTimeout(() => { if (this.proc?.exitCode === null) this.proc.kill("SIGKILL"); }, 5000);
      try { await this.proc.exited; } finally { clearTimeout(timer); }
    }
    await this.stderrDrain;
  }
}

const fixtures: Fixture[] = [];
function fixture(name: string, extra: Row[] = []) { const f = new Fixture(name, extra); fixtures.push(f); return f; }
const posts = (f: Fixture, id: string) => f.events().filter(x => (x.$ === "MessagePosted" && x.msg === id && x.role === "user") || (x.$ === "RoomPosted" && x.id === id && x.from === "you"));
const observedState = (c: Client): Nudge | undefined => {
  const row = c.frames.flatMap(frame => frame.items ?? []).filter(row => row.$ === "SettingSet" && row.key === key).at(-1);
  return row ? JSON.parse(row.value) : undefined;
};
const banner = (c: Client) => {
  const shown = c.views!.links.get("fixture")!.shown;
  return c.mode === "web" ? JSON.stringify(shown).includes('"star-nudge"') : !!shown?.starNudge;
};
async function diskFault(f: Fixture, c: Client) {
  const before = readFileSync(f.log, "utf8"), mark = c.frames.length;
  // One oversized, uncounted rename primes the real queue. The soft limit
  // affects only this fixture PID; the hard limit stays available to restore.
  f.limit(statSync(f.log).size + 16);
  await f.accepted(c, "thread.rename", { thread: "b", title: "Disk fault " + "x".repeat(512) });
  await until(`${f.name}: disk-full notification`, () => c.frames.slice(mark).some(frame => frame.t === "update" && String(frame.error ?? "").startsWith("Disk full")));
  if (readFileSync(f.log, "utf8") !== before) throw Error("fsize fault failed to leave the durable log unchanged");
}

try {
  const f = fixture("race");
  await f.start();
  let a = await f.connect(), b = await f.connect();
  const observer = await f.connect();
  check("old user history and connection/setup RPCs do not count", f.writes().length === 0 && !f.state(), f.state());
  // Some older deciders acknowledge blank/unknown turn.start; the nudge
  // must exclude them regardless. Bot/room/address errors really reject.
  await f.rpc(a, "turn.start", { thread: "a", text: " \t\n ", msg: "blank-turn" });
  await f.rpc(a, "turn.start", { thread: "nonexistent", text: "Unknown thread", msg: "unknown-turn" });
  await f.rpc(a, "bots.post", { room: "room", text: " \t\n ", msg: "blank-post" });
  for (const [m, p] of [
    ["bots.say", { bot: "nonexistent", text: "Refused bot", msg: "bad-bot" }],
    ["bots.post", { room: "nonexistent", text: "Refused room", msg: "bad-room" }],
    ["bots.tell", { to: "nobody@unlinked", text: "Refused peer", msg: "bad-peer" }],
  ] as [string, Row][]) {
    const reply = await f.rpc(a, m, p);
    check(`${m} invalid destination rejects`, reply.ok === false, reply);
  }
  check("blank and nonexistent destinations never advance the count", f.writes().length === 0, f.writes());

  const first: [string, Row][] = [
    ["turn.start", { thread: "a", text: "First accepted request", msg: "first-turn", mode: "queue" }],
    ["bots.say", { bot: "bot", text: "Second accepted request", msg: "second-say" }],
    ["bots.post", { room: "room", text: "Third accepted request", msg: "third-post" }],
    ["bots.tell", { to: "remote@fixture-peer", text: "Fourth accepted request", msg: "fourth-tell" }],
  ];
  const ids = first.map(([, p]) => p.msg);
  for (let i = 0; i < first.length; i++) {
    const [m, p] = first[i];
    await f.accepted(i % 2 ? b : a, m, p);
    check(`${m} counts exactly its accepted message id (${i + 1}/5)`, same(f.state()?.ids, ids.slice(0, i + 1)) && f.state()?.status === "counting" && f.state()?.token === p.msg, f.state());
    check(`${m} is represented once in the durable log`, posts(f, p.msg).length === 1, posts(f, p.msg));
    const before = f.writes().length;
    await f.accepted(i % 2 ? a : b, m, p);
    check(`${m} replay from another client writes neither a message nor a count`, posts(f, p.msg).length === 1 && f.writes().length === before, f.state());
  }
  const counting = f.state()!;
  check("first four never offer and timestamps stay ineligible", [a, b, observer].every(c => f.offers(c).length === 0) && counting.eligibleAt === 0 && counting.sentAt === 0, counting);
  await f.rpc(b, "star.nudge", { action: "dismiss", token: counting.token });
  check("a decision before eligibility changes nothing", same(f.state(), counting) && f.writes().length === 4, f.state());

  // Kill without graceful shutdown: the four-message checkpoint must already
  // be durable when its accepting RPC replies, independently of teardown.
  await f.stop("SIGKILL"); await f.start();
  a = await f.connect(); b = await f.connect();
  const watcher = await f.connect();
  check("restart retains the exact four-message ledger", same(f.state(), counting), f.state());
  for (const [m, p] of first) await f.accepted(a, m, p);
  check("same-message replay after restart remains below threshold", f.writes().length === 4 && same(f.state(), counting), f.state());

  // Distinct sockets race for the last slot: no assumed winner or sleeps.
  await Promise.all([
    f.accepted(a, "turn.start", { thread: "a", text: "Racing prompt A", msg: "race-a", mode: "queue" }),
    f.accepted(b, "bots.post", { room: "room", text: "Racing prompt B", msg: "race-b" }),
  ]);
  await Promise.all([f.barrier(a), f.barrier(b), f.barrier(watcher)]);
  const sent = f.state()!;
  const allOffers = [a, b, watcher].flatMap(c => f.offers(c));
  check("racing fifth/sixth both persist accepted messages", posts(f, "race-a").length === 1 && posts(f, "race-b").length === 1);
  check("race saves exactly one sent transition and five ids", f.writes().length === 5 && sent.status === "sent" && sent.ids.length === 5 && new Set(sent.ids).size === 5 && same(sent.ids.slice(0, 4), ids) && sent.ids[4] === sent.token && ["race-a", "race-b"].includes(sent.token), sent);
  check("eligibility and sent timestamps persist before the RPC replies", sent.eligibleAt > 0 && sent.sentAt === sent.eligibleAt && !sent.decidedAt, sent);
  check("race emits exactly one targeted offer with the exact envelope", allOffers.length === 1 && same(allOffers[0], { t: "info", info: { [offerKey]: sent.token } }), allOffers);
  check("only the threshold request's originating socket gets the offer", f.offers(sent.token === "race-a" ? a : b).length === 1 && f.offers(sent.token === "race-a" ? b : a).length === 0 && f.offers(watcher).length === 0, [f.offers(a), f.offers(b), f.offers(watcher)]);
  for (const c of [a, b]) {
    await f.accepted(c, "turn.start", { thread: "b", text: "After eligibility", msg: `after-${c === a ? "a" : "b"}`, mode: "queue" });
    await f.accepted(c, "bots.post", { room: "room", text: "Replay threshold", msg: sent.token });
  }
  check("later prompts and same-token replay neither write nor offer", f.writes().length === 5 && same(f.state(), sent) && [a, b, watcher].flatMap(c => f.offers(c)).length === 1, f.state());

  // Lose the offering connection without making a decision. Reconnecting,
  // including after a hub restart, deliberately cannot recover/reoffer it.
  a.ws.close(); b.ws.close();
  const reconnected = await f.connect();
  check("a reconnect never reoffers an undecided sent token", f.offers(reconnected).length === 0 && same(f.state(), sent));
  await f.stop("SIGKILL"); await f.start();
  a = await f.connect(); b = await f.connect();
  await f.accepted(a, "bots.post", { room: "room", text: "After undecided restart", msg: "post-restart-sent" });
  await f.barrier(b);
  check("restart preserves sent and never reoffers even after another prompt", same(f.state(), sent) && f.writes().length === 5 && f.offers(a).length === 0 && f.offers(b).length === 0, f.state());
  for (const p of [{ action: "dismiss", token: "stale-token" }, { action: "open", token: "" }, { action: "invalid", token: sent.token }]) await f.rpc(a, "star.nudge", p);
  check("invalid actions and tokens do not consume or rewrite the offer", f.writes().length === 5 && same(f.state(), sent), f.state());
  const dismissedReply = await f.rpc(b, "star.nudge", { action: "dismiss", token: sent.token });
  const dismissed = f.state()!;
  check("another client dismisses globally, saved before its reply", dismissedReply.ok && dismissed.status === "dismissed" && dismissed.decidedAt! >= sent.sentAt && same(dismissed.ids, sent.ids) && dismissed.token === sent.token && dismissed.eligibleAt === sent.eligibleAt && dismissed.sentAt === sent.sentAt && f.writes().length === 6, [dismissedReply, dismissed]);
  await Promise.all([f.rpc(a, "star.nudge", { action: "open", token: sent.token }), f.rpc(b, "star.nudge", { action: "dismiss", token: sent.token })]);
  check("dismissal is terminal across clients and repeated decisions", f.writes().length === 6 && same(f.state(), dismissed), f.state());
  await f.stop(); await f.start();
  a = await f.connect(); b = await f.connect();
  await f.accepted(b, "bots.post", { room: "room", text: "After dismissal", msg: "after-dismissal" });
  await f.rpc(a, "star.nudge", { action: "open", token: sent.token });
  check("dismissal survives restart, new prompts and another client's open", same(f.state(), dismissed) && f.writes().length === 6 && [a, b].every(c => f.offers(c).length === 0), f.state());
  await f.stop();

  // A compacted/deleted first entry is absent altogether. Carry the real
  // four-message checkpoint above into a fresh log, not the old entries or
  // its indexes, then accept the same id as a new message in another thread.
  const missing = fixture("missing-entry", [event("SettingSet", { key, value: JSON.stringify(counting) })]);
  await missing.start();
  const ma = await missing.connect(), mb = await missing.connect();
  check("compacted fixture really has no original counted entries", ids.every(id => posts(missing, id).length === 0));
  await missing.accepted(ma, "turn.start", { thread: "b", text: "Replay absent first entry", msg: ids[0], mode: "queue" });
  check("accepted absent-entry replay cannot inflate the durable ledger", posts(missing, ids[0]).length === 1 && same(missing.state(), counting) && missing.writes().length === 1 && missing.offers(ma).length === 0, missing.state());
  await missing.accepted(mb, "bots.post", { room: "room", text: "The real fifth distinct id", msg: "absent-fifth" });
  check("only a fresh fifth id offers after compaction", missing.state()?.status === "sent" && same(missing.state()?.ids, [...ids, "absent-fifth"]) && missing.offers(mb).length === 1 && missing.offers(ma).length === 0, missing.state());
  await missing.stop();

  // A second independent home exercises open as the winning terminal action.
  const openedFixture = fixture("opened");
  await openedFixture.start();
  let oa = await openedFixture.connect(), ob = await openedFixture.connect();
  for (let i = 1; i <= 5; i++) await openedFixture.accepted(oa, "bots.post", { room: "room", text: `Open scenario ${i}`, msg: `open-${i}` });
  const openSent = openedFixture.state()!;
  check("separate home receives its one fifth-message offer", openedFixture.offers(oa).length === 1 && openedFixture.offers(ob).length === 0 && openSent.status === "sent");
  const openReply = await openedFixture.rpc(ob, "star.nudge", { action: "open", token: openSent.token });
  const opened = openedFixture.state()!;
  check("open from another client is durable before reply", openReply.ok && opened.status === "opened" && opened.decidedAt! >= opened.sentAt && same(opened.ids, openSent.ids) && opened.token === openSent.token && opened.sentAt === openSent.sentAt && opened.eligibleAt === openSent.eligibleAt && openedFixture.writes().length === 6, opened);
  await Promise.all([openedFixture.rpc(oa, "star.nudge", { action: "dismiss", token: opened.token }), openedFixture.rpc(ob, "star.nudge", { action: "open", token: opened.token })]);
  check("opened is terminal across clients and idempotent", same(openedFixture.state(), opened) && openedFixture.writes().length === 6, openedFixture.state());
  await openedFixture.stop(); await openedFixture.start();
  oa = await openedFixture.connect(); ob = await openedFixture.connect();
  await openedFixture.accepted(ob, "bots.post", { room: "room", text: "After opening and restart", msg: "after-open" });
  await openedFixture.rpc(oa, "star.nudge", { action: "dismiss", token: opened.token });
  check("opened survives restart and suppresses prompts on all clients", same(openedFixture.state(), opened) && openedFixture.writes().length === 6 && [oa, ob].every(c => openedFixture.offers(c).length === 0), openedFixture.state());
  originalChecks = checks; originalFailures = failures;
  console.log(`receipt original40: ${originalChecks} checks, ${originalFailures} failed`);
  if (originalChecks !== 40) throw Error(`original40 receipt drifted to ${originalChecks} checks`);
  await openedFixture.stop();

  // Exercise the authenticated HTTP entry point, not the pure wrapper with
  // an invented client number. Only the generated loopback peer is discovered.
  const seedFour = () => [event("SettingSet", { key, value: JSON.stringify(counting) })];
  const retry = fixture("retry", seedFour());
  await retry.start();
  const retryClient = await retry.connect();
  await retry.accepted(retryClient, "turn.start", { thread: "a", text: "Retry accepted input", msg: "explicit-retry", retry: true });
  check("explicit Retry is accepted as a resend", posts(retry, "explicit-retry").length === 1);
  check("accepted Retry does not count as the fifth new prompt", same(retry.state(), counting) && retry.writes().length === 1 && retry.offers(retryClient).length === 0, retry.state());
  await retry.stop();

  const forwarded = fixture("forwarded", seedFour());
  forwarded.discoverPeer = true;
  const signingKey = randomBytes(32); // disposable fixture authentication only
  for (const path of [join(forwarded.home, "secrets"), join(forwarded.home, "secrets", "peers")]) mkdirSync(path, { mode: 0o700 });
  writeFileSync(join(forwarded.home, "secrets", "peers", "peer"), JSON.stringify({ secret: signingKey.toString("hex") }), { mode: 0o600 });
  await forwarded.start();
  const fa = await forwarded.connect(), fb = await forwarded.connect();
  await until("forwarded fixture recognizes its loopback peer", () => fa.frames.some(frame => {
    const machines = frame.info?.machines;
    if (typeof machines !== "string") return false;
    return JSON.parse(machines).peers?.some((machine: Row) => machine.url === `http://127.0.0.1:${peer.port}`);
  }), 30000);
  const request = { id: 9001, m: "turn.start", p: { thread: "bot-thread", text: "Forwarded human input", msg: "forwarded-message", mode: "queue", accountBinding: "" }, nonce: randomBytes(8).toString("hex") };
  const body = JSON.stringify(request), timestamp = String(Math.floor(Date.now() / 1000));
  const signature = "sha256=" + createHmac("sha256", signingKey).update(`${timestamp}.${body}`).digest("hex");
  const response = await fetch(`http://127.0.0.1:${forwarded.port}/far/rpc`, { method: "POST", body,
    headers: { "content-type": "application/json", "x-backplane-peer": "peer", "x-backplane-timestamp": timestamp, "x-backplane-signature": signature }, signal: AbortSignal.timeout(15000) });
  const forwardedText = await response.text();
  let forwardedReply: Row;
  try { forwardedReply = JSON.parse(forwardedText); }
  catch { throw Error(`forwarded /far/rpc returned HTTP ${response.status}: ${forwardedText.slice(0, 500)}`); }
  check("real forwarded synthetic request retains the normal RPC reply", response.ok && forwardedReply.t === "reply" && Number(forwardedReply.id) === request.id && forwardedReply.ok === true && !forwardedReply.info, forwardedReply);
  await Promise.all([forwarded.barrier(fa), forwarded.barrier(fb)]);
  check("forwarded message is accepted without counting or offering", posts(forwarded, request.p.msg).length === 1 && same(forwarded.state(), counting) && forwarded.writes().length === 1 && [fa, fb].every(c => forwarded.offers(c).length === 0), forwarded.state());
  await forwarded.accepted(fb, "bots.post", { room: "room", text: "Actual fifth local send", msg: "local-after-forward" });
  check("local fifth still offers only to its real socket after forwarding", forwarded.state()?.token === "local-after-forward" && forwarded.offers(fb).length === 1 && forwarded.offers(fa).length === 0, forwarded.state());
  await forwarded.stop();

  // EFBIG injection uses the existing disk_full_e2e.ts approach without
  // sudo, mounts, real homes or altering this test runner's resource limits.
  if (process.platform === "linux") {
    const disk = fixture("disk-flush", seedFour());
    // Disk retry runs on the hub tick (a minute by default). Use its
    // existing fixture knob so the bounded flush checks do not wait a minute.
    disk.wakeTickMs = "1000";
    await disk.start();
    const da = await disk.connect(), db = await disk.connect();
    await diskFault(disk, da);
    await disk.accepted(da, "bots.post", { room: "room", text: "Fifth waits for disk", msg: "disk-fifth" });
    await Promise.all([disk.barrier(da), disk.barrier(db)]);
    check("fifth is projected as sent while its durable checkpoint is still four", observedState(da)?.status === "sent" && same(disk.state(), counting), { memory: observedState(da), disk: disk.state() });
    check("SavedSend withholds the offer from every socket before flush", [da, db].every(c => disk.offers(c).length === 0));
    disk.limit("unlimited");
    await until("saved fifth and its held targeted offer", () => disk.state()?.status === "sent" && disk.offers(da).length === 1, 30000);
    await disk.barrier(db);
    const diskSent = disk.state()!;
    check("flush releases exactly one offer after saving its matching token", disk.writes().length === 2 && diskSent.token === "disk-fifth" && disk.offers(da).length === 1 && disk.offers(db).length === 0, diskSent);

    await diskFault(disk, da);
    const decisionId = disk.send(da, "star.nudge", { action: "dismiss", token: diskSent.token });
    const duplicateId = disk.send(da, "star.nudge", { action: "dismiss", token: diskSent.token });
    await disk.barrier(da); // both earlier requests have been processed
    check("first decision changes memory but stays unsaved under the fault", observedState(da)?.status === "dismissed" && same(disk.state(), diskSent), { memory: observedState(da), disk: disk.state() });
    check("decision ACK and no-change duplicate ACK both wait for flush", !da.replies.has(decisionId) && !da.replies.has(duplicateId), [da.replies.get(decisionId), da.replies.get(duplicateId)]);
    disk.limit("unlimited");
    const decisions = await Promise.all([disk.reply(da, decisionId, 30000), disk.reply(da, duplicateId, 30000)]);
    check("flush releases both ACKs for one durable terminal decision", decisions.every(reply => reply.ok === true) && disk.state()?.status === "dismissed" && disk.writes().length === 3, { decisions, state: disk.state() });
    check("flushing decisions never repeats the already delivered offer", disk.offers(da).length === 1 && disk.offers(db).length === 0);
    await disk.stop();

    const crash = fixture("disk-crash", seedFour());
    crash.wakeTickMs = "1000";
    await crash.start();
    const ca = await crash.connect();
    await diskFault(crash, ca);
    const crashPrompt = { room: "room", text: "Unsaved fifth at crash", msg: "crash-fifth" };
    await crash.accepted(ca, "bots.post", crashPrompt);
    await crash.barrier(ca);
    check("crash setup has no delivered offer and no durable fifth", crash.offers(ca).length === 0 && same(crash.state(), counting), crash.state());
    await crash.stop("SIGKILL"); await crash.start();
    const cb = await crash.connect();
    check("crash before flush discards the unsent offer and restores four", crash.offers(cb).length === 0 && same(crash.state(), counting), crash.state());
    await crash.accepted(cb, "bots.post", crashPrompt);
    check("replaying the lost fifth after recovery delivers once in total", crash.offers(ca).length + crash.offers(cb).length === 1 && crash.state()?.status === "sent" && crash.state()?.token === crashPrompt.msg, crash.state());
    // Also crash with an unsaved dismissal: its ACK must not escape, and
    // the already delivered offer must not be regenerated by the next boot.
    await diskFault(crash, cb);
    const lostDecision = crash.send(cb, "star.nudge", { action: "dismiss", token: crashPrompt.msg });
    const lostRetry = crash.send(cb, "star.nudge", { action: "dismiss", token: crashPrompt.msg });
    await crash.barrier(cb);
    check("pre-crash dismissal and duplicate ACK remain withheld", !cb.replies.has(lostDecision) && !cb.replies.has(lostRetry) && crash.state()?.status === "sent");
    await crash.stop("SIGKILL"); await crash.start();
    const cc = await crash.connect();
    await crash.accepted(cc, "bots.post", crashPrompt);
    check("crash before decision flush preserves sent without repeating offer", crash.state()?.status === "sent" && crash.offers(cc).length === 0 && crash.offers(ca).length + crash.offers(cb).length === 1, crash.state());
    const recoveredDecision = await crash.rpc(cc, "star.nudge", { action: "dismiss", token: crashPrompt.msg });
    check("unacknowledged decision can be safely retried after crash", recoveredDecision.ok === true && crash.state()?.status === "dismissed" && crash.writes().length === 3, crash.state());
    await crash.stop();
  } else console.log("skip Linux prlimit file-size fault cases (requires Linux)");

  // Each renderer originates the real view.action -> ordinary send ->
  // targeted Actor.info -> view.changed -> decoded picture chain once.
  // The other renderer is initialized too, so a leaked offer would render.
  for (const kind of ["thread", "bot"] as const) for (const mode of ["web", "mobile"] as const) {
    const label = `${kind}/${mode}`;
    const thin = fixture(`thin-${kind}-${mode}`, seedFour());
    thin.target = { kind, id: kind === "bot" ? "bot" : "b" };
    await thin.start();
    const origin = await thin.connect(mode), other = await thin.connect(mode === "web" ? "mobile" : "web");
    const control = await thin.connect();
    check(`${label}: initialized actors show no suggestion at four`, !banner(origin) && !banner(other));
    origin.session = { ...origin.session, drafts: { ...origin.session!.drafts, [kind === "bot" ? "bot-thread" : "b"]: `Fifth from actual ${label} view action` } };
    const sentAction = await thin.picture(origin, false, { action: "send", value: "" });
    const commands = sentAction.cmds.filter((command: Row) => command.type === "send");
    const prompt = commands.map((command: Row) => JSON.parse(command.text)).find((command: Row) => command.m === (kind === "bot" ? "bots.say" : "turn.start"));
    if (!prompt?.p?.msg) throw Error(`${label}: actual send action emitted no identified human prompt: ${JSON.stringify(sentAction.cmds)}`);
    const originMark = origin.frames.length, otherMark = other.frames.length;
    for (const command of commands) origin.ws.send(W.encode(command.text));
    const promptReply = await thin.reply(origin, prompt.id);
    check(`${label}: generated prompt is accepted and commits fifth`, promptReply.ok === true && thin.state()?.status === "sent" && thin.state()?.token === prompt.p.msg && posts(thin, prompt.p.msg).length === 1, thin.state());
    await Promise.all([thin.invalidatedPicture(origin, originMark), thin.invalidatedPicture(other, otherMark)]);
    check(`${label}: only originating thin actor renders its suggestion`, banner(origin) && !banner(other));
    check(`${label}: thin sockets filter raw targeted info and control gets none`, thin.offers(origin).length === 0 && thin.offers(other).length === 0 && thin.offers(control).length === 0);
    const shown = origin.views!.links.get("fixture")!.shown;
    check(`${label}: rendered suggestion carries the accepted offer token`, mode === "mobile" ? shown.starNudge?.token === prompt.p.msg : JSON.stringify(shown).includes(prompt.p.msg), shown);
    await thin.picture(origin); // refresh must preserve actor-private info
    check(`${label}: normal view refresh retains the private offer`, banner(origin));
    const rejoined = await thin.connect(mode);
    check(`${label}: new initialized actor cannot recover a previous offer`, !banner(rejoined));
    const dismissOriginMark = origin.frames.length, dismissOtherMark = other.frames.length;
    const dismissedElsewhere = await thin.rpc(other, "star.nudge", { action: "dismiss", token: prompt.p.msg });
    await Promise.all([thin.invalidatedPicture(origin, dismissOriginMark), thin.invalidatedPicture(other, dismissOtherMark)]);
    check(`${label}: another client's durable dismissal hides both rendered views`, dismissedElsewhere.ok === true && thin.state()?.status === "dismissed" && !banner(origin) && !banner(other) && thin.writes().length === 3, thin.state());
    await thin.picture(origin);
    check(`${label}: stale actor token cannot resurrect the dismissed banner`, !banner(origin) && thin.offers(origin).length === 0);
    await thin.stop();
  }
} catch (error) {
  check("bounded star nudge end-to-end run completes", false, String(error));
  for (const f of fixtures) {
    const path = join(f.home, `hub-${f.starts}.stderr`);
    if (existsSync(path)) console.error(`${f.name} hub stderr:\n${readFileSync(path, "utf8").slice(-2500)}`);
  }
} finally {
  for (const f of fixtures) await f.stop();
  peer.stop(true);
  rmSync(root, { recursive: true, force: true });
}
console.log(failures ? `${failures} failed` : "all ok");
if (originalChecks === 40) console.log(`receipt extended: ${checks - originalChecks} checks, ${failures - originalFailures} failed`);
process.exit(failures ? 1 : 0);
