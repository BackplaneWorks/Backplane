// End to end: bots on two headless hubs (docs/bots.md). Starts two hubs on
// temporary homes and free ports, then checks webhooks (signed, GitHub's,
// wrong, replayed, stale, too big; bearer tokens and forms), linking the hubs with an invite, a
// person on one hub writing to a bot on the other, a room shared by both, the signed bot
// directory, and (on the minute tick) a routine and the linked machine's
// bots in the hub info. Agents are stand-ins: `claude` answers every message
// with "pong from the bot" (so a bot's answer to a person on the other hub
// can travel back), `codex` and `grok` exit at once. Prints "ok ..." /
// "FAIL ..." lines.
//
//   bun test/tools/bots_e2e.ts [BINARY] [WIREDIR] [--quick]
//
// BINARY defaults to build/backplane (scripts/build-app.sh
// src/app/main.bend build/backplane); WIREDIR to build/wire (bend
// test/wire/index.html -o build/wire), the hub's own CBOR codec. --quick
// skips the checks that wait for the minute tick.
import { createHmac } from "node:crypto";
import { mkdtempSync, readdirSync, readFileSync, rmSync, statSync, writeFileSync, chmodSync, mkdirSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const args = process.argv.slice(2);
const quick = args.includes("--quick");
const [bin = "build/backplane", wire = "build/wire"] = args.filter((a) => !a.startsWith("--"));
for (const f of readdirSync(wire).filter((f) => f.endsWith(".js"))) (0, eval)(readFileSync(`${wire}/${f}`, "utf8"));
const W = (globalThis as any).Wire;

let failed = false;
const check = (name: string, ok: boolean, got?: unknown) => {
  console.log(ok ? `ok ${name}` : `FAIL ${name}: ${JSON.stringify(got)?.slice(0, 400)}`);
  if (!ok) failed = true;
};
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

// a port nothing listens on
function freePort(): number {
  const s = Bun.listen({ hostname: "127.0.0.1", port: 0, socket: { data() {} } });
  const p = s.port;
  s.stop(true);
  return p;
}

// stand-in agents, so no real agent runs: claude answers each message it
// reads (Claude's stream-json: an assistant message, then the result);
// codex and grok exit at once
const root = mkdtempSync(join(tmpdir(), "bp-bots-e2e-"));
const fake = join(root, "bin");
mkdirSync(fake);
const answer = "pong from the bot";
const claude = `#!/bin/sh
n=0
while IFS= read -r line; do
  case "$line" in
    *'"type":"user"'*)
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

type Hub = { name: string; home: string; port: number; proc: ReturnType<typeof Bun.spawn>; ws: WebSocket; seen: any[]; replies: Map<number, any>; info: any; n: number; hb: number };

async function start(name: string): Promise<Hub> {
  const home = join(root, name);
  const port = freePort();
  const proc = Bun.spawn([resolve(bin), "--home", home, "--port", String(port), "--no-tailscale"], {
    env: { ...process.env, DISPLAY: "", WAYLAND_DISPLAY: "", BACKPLANE_NAME: name, BACKPLANE_NO_UPDATE: "1", BACKPLANE_KEYRING_LOCKED: "0", BACKPLANE_PEERS: "", BACKPLANE_AGENTS: fake, PATH: `${fake}:${process.env.PATH}` },
    stdout: "ignore",
    stderr: "pipe",
  });
  const h: Hub = { name, home, port, proc, ws: null as any, seen: [], replies: new Map(), info: null, n: 0, hb: 0 };
  for (let i = 0; i < 100; i++) {
    try {
      const r = await fetch(`http://127.0.0.1:${port}/hello`);
      if (r.status === 200) break;
    } catch {}
    await sleep(100);
  }
  const ws = new WebSocket(`ws://127.0.0.1:${port}/ws`);
  ws.binaryType = "arraybuffer";
  ws.onmessage = (e) => {
    const o = JSON.parse(W.decode(new Uint8Array(e.data as ArrayBuffer)));
    for (const c of o.items ?? []) h.seen.push(c);
    if (o.t === "hb") h.hb++;
    if (o.t === "reply") h.replies.set(Number(o.id), o);
    if (o.info) h.info = o.info;
  };
  await new Promise((r, j) => { ws.onopen = r; ws.onerror = j; });
  h.ws = ws;
  return h;
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

async function rpc(h: Hub, m: string, p: object): Promise<any> {
  const id = ++h.n;
  h.ws.send(W.encode(JSON.stringify({ id, m, p })));
  return await until(10000, () => h.replies.get(id));
}

const change = (h: Hub, tag: string, f: (c: any) => boolean = () => true) => until(8000, () => h.seen.find((c) => c.$ === tag && f(c)));

const hmac = (key: Buffer | string, text: string) => createHmac("sha256", key).update(text).digest("hex");
const sign = (secret: string, ts: number, body: string) => "sha256=" + hmac(Buffer.from(secret, "hex"), `${ts}.${body}`);
const now = () => Math.floor(Date.now() / 1000);

async function post(url: string, body: string, headers: Record<string, string>) {
  const r = await fetch(url, { method: "POST", body, headers: { "content-type": "application/json", ...headers } });
  return { status: r.status, text: await r.text() };
}

const hubs: Hub[] = [];
try {
  const a = await start("alpha");
  hubs.push(a);
  const b = await start("beta");
  hubs.push(b);
  const A = `http://127.0.0.1:${a.port}`;

  // bots
  const miso = await rpc(a, "bots.create", { name: "miso", persona: "a test bot" });
  const nori = await rpc(a, "bots.create", { name: "nori" });
  check("bots.create answers a bot id", miso?.ok && typeof miso.bot === "string" && nori?.ok, [miso, nori]);
  const home = await change(a, "WorktreeSet", (c) => String(c.worktree ?? c.path ?? JSON.stringify(c)).includes(`/bots/${miso?.bot}`));
  // the outbox sends a request again after a dropped link, with the same
  // msg: the hub stores and wakes once, and answers both
  const say1 = await rpc(a, "bots.say", { bot: miso.bot, text: "once only", msg: "e2e-say-1" });
  const say2 = await rpc(a, "bots.say", { bot: miso.bot, text: "once only", msg: "e2e-say-1" });
  await sleep(500);
  check("bots.say with the same msg twice is answered ok both times", !!say1?.ok && !!say2?.ok, [say1, say2]);
  check("and stored once", a.seen.filter((c) => c.$ === "MessagePosted" && c.msg === "e2e-say-1").length === 1, a.seen.filter((c) => c.msg === "e2e-say-1"));
  check("a bot's home folder becomes its thread's folder", !!home, a.seen.filter((c) => c.$ === "WorktreeSet"));
  check("the home folder is a git repository", (() => { try { return statSync(join(a.home, "bots", miso.bot, ".git")).isDirectory(); } catch { return false; } })());

  // webhooks
  const hk = await rpc(a, "bots.hook", { bot: miso.bot, name: "ci" });
  check("bots.hook answers a secret once", hk?.ok && /^[0-9a-f]{64}$/.test(hk.secret) && hk.path === `/hook/${hk.hook}`, hk);
  const secret: string = hk?.secret ?? "";
  const hookFile = join(a.home, "secrets", "hooks", hk?.hook ?? "x");
  check("the hook's secret file is 0600 in a 0700 folder",
    (statSync(hookFile).mode & 0o777) === 0o600 && (statSync(join(a.home, "secrets", "hooks")).mode & 0o777) === 0o700 && (statSync(join(a.home, "secrets")).mode & 0o777) === 0o700,
    [(statSync(hookFile).mode & 0o777).toString(8)]);
  check("the hook's secret file holds the secret", readFileSync(hookFile, "utf8").trim() === secret);
  const url = `${A}/hook/${hk?.hook}`;
  const body = JSON.stringify({ action: "opened", n: 1 });
  const ts = now();
  const good = { "x-backplane-timestamp": String(ts), "x-backplane-signature": sign(secret, ts, body) };
  const r1 = await post(url, body, good);
  check("a signed webhook is accepted (202)", r1.status === 202, r1);
  check("a HookHit change follows", !!(await change(a, "HookHit")), a.seen.map((c) => c.$).slice(-8));
  check("the hook wakes its bot", !!(await change(a, "BotWoke", (c) => c.bot === miso.bot)));
  const r2 = await post(url, body, good);
  check("the same request again is refused (401)", r2.status === 401, r2);
  const r3 = await post(url, body, { "x-backplane-timestamp": String(now()), "x-backplane-signature": sign("00".repeat(32), now(), body) });
  check("a wrong signature is refused (401)", r3.status === 401, r3);
  const old = now() - 1000;
  const r4 = await post(url, body, { "x-backplane-timestamp": String(old), "x-backplane-signature": sign(secret, old, body) });
  check("a stale timestamp is refused (401)", r4.status === 401, r4);
  const r5 = await post(url, body, {});
  check("an unsigned webhook is refused (401)", r5.status === 401, r5);
  const r6 = await post(`${A}/hook/h0-0`, body, good);
  check("an unknown webhook is refused (401)", r6.status === 401, r6);
  const big = JSON.stringify({ x: "y".repeat(300000) });
  const r7 = await post(url, big, { "x-backplane-timestamp": String(now()), "x-backplane-signature": sign(secret, now(), big) });
  check("a body over 256 KB is refused (413)", r7.status === 413, r7.status);
  const gbody = JSON.stringify({ zen: "Keep it logically awesome." });
  const gh = { "x-hub-signature-256": "sha256=" + hmac(secret, gbody), "x-github-delivery": "d-" + Date.now() };
  const r8 = await post(url, gbody, gh);
  check("GitHub's signature is accepted (202)", r8.status === 202, r8);
  const r9 = await post(url, gbody, gh);
  check("the same GitHub delivery again is refused (401)", r9.status === 401, r9);
  const r10 = await post(url, gbody, { ...gh, "x-github-delivery": "d-other", "x-hub-signature-256": "sha256=" + hmac("wrong", gbody) });
  check("a wrong GitHub signature is refused (401)", r10.status === 401, r10);

  // bearer tokens: for senders that cannot sign (a Pebble Index ring)
  const form = (fields: Record<string, string>, audio?: Uint8Array) => {
    const fd = new FormData();
    for (const [k, v] of Object.entries(fields)) fd.append(k, v);
    if (audio) fd.append("audio", new Blob([audio], { type: "audio/mp4" }), "note.m4a");
    return fd;
  };
  const postForm = async (u: string, fd: FormData, headers: Record<string, string>) => {
    const r = await fetch(u, { method: "POST", body: fd, headers });
    return { status: r.status, text: await r.text() };
  };
  const b0 = await postForm(url, form({ transcription: "no token yet" }), { authorization: "Bearer 0123" });
  check("a bearer before the hook has a token is refused (401)", b0.status === 401 && b0.text.includes("no bearer token"), b0);
  const tk = await rpc(a, "bots.hook_token", { hook: hk.hook });
  const token: string = tk?.token ?? "";
  check("bots.hook_token makes a token and gives the URL", tk?.ok && /^[0-9a-f]{32}$/.test(token) && tk.tokenHook === hk.hook && String(tk.tokenUrl).endsWith(`/hook/${hk.hook}`), tk);
  const tokenFile = join(a.home, "secrets", "hooks", `${hk?.hook}.token`);
  check("the token file is 0600 and holds the token", (statSync(tokenFile).mode & 0o777) === 0o600 && readFileSync(tokenFile, "utf8").trim() === token);
  const tk2 = await rpc(a, "bots.hook_token", { hook: hk.hook });
  check("asking again shows the same token", tk2?.ok && tk2.token === token, tk2);
  const before = a.seen.filter((c) => c.$ === "MessagePosted").length;
  const bf = await postForm(url, form({ transcription: "hello pebble", recordedAt: "1790300000000", client: "ring" }), { authorization: `Bearer ${token}` });
  check("a form with the right bearer token is accepted (202)", bf.status === 202, bf);
  const msg = await change(a, "MessagePosted", (c) => JSON.stringify(c).includes("hello pebble"));
  const mtext = JSON.stringify(msg ?? "");
  check("the bot gets the form as JSON", mtext.includes('\\"source\\":\\"pebble-index-01\\"') && mtext.includes('\\"recordedAt\\":1790300000000'), mtext.slice(0, 600));
  check("framed as untrusted data", mtext.includes("untrusted data"), mtext.slice(0, 300));
  const bf2 = await postForm(url, form({ transcription: "hello again", recordedAt: "1790300000001" }), { authorization: `Bearer ${token}` });
  check("a second call with the same token is accepted too (202)", bf2.status === 202, bf2);
  const ba = await postForm(url, form({ transcription: "with audio", recordedAt: "1790300000002", client: "ring" }, new Uint8Array(4000).fill(7)), { authorization: `Bearer ${token}`, "x-audio-size": "4000" });
  check("a form with audio is accepted (202)", ba.status === 202, ba);
  const am = await change(a, "MessagePosted", (c) => JSON.stringify(c).includes("with audio"));
  check("the audio is left out, with its size", JSON.stringify(am ?? "").includes("omitted, 4000 bytes"), JSON.stringify(am ?? "").slice(0, 400));
  const bj = await post(url, JSON.stringify({ text: "json body" }), { authorization: `Bearer ${token}` });
  check("JSON with the bearer token is accepted (202)", bj.status === 202, bj);
  const jm = await change(a, "MessagePosted", (c) => JSON.stringify(c).includes("json body"));
  check("JSON reaches the bot as it came", JSON.stringify(jm ?? "").includes('{\\"text\\":\\"json body\\"}'), JSON.stringify(jm ?? "").slice(0, 400));
  const bw = await postForm(url, form({ transcription: "forged" }), { authorization: `Bearer ${"0".repeat(32)}` });
  check("a wrong bearer token is refused (401)", bw.status === 401 && bw.text.includes("wrong bearer token"), bw);
  const bn = await postForm(url, form({ transcription: "anonymous" }), {});
  check("no credentials is refused (401) and says so", bn.status === 401 && bn.text.includes("no credentials"), bn);
  const bsig = await post(url, body, { authorization: `Bearer ${token}`, "x-backplane-timestamp": String(now()), "x-backplane-signature": sign("00".repeat(32), now(), body) });
  check("a wrong signature is refused even with the token (401)", bsig.status === 401 && bsig.text.includes("signature"), bsig);
  const bbig = await postForm(url, form({ transcription: "big" }, new Uint8Array(2100000)), { authorization: `Bearer ${token}` });
  check("a form over 2 MB is refused (413)", bbig.status === 413, bbig.status);
  const tk3 = await rpc(a, "bots.hook_token", { hook: hk.hook, op: "new" });
  check("a new token replaces the old one", tk3?.ok && /^[0-9a-f]{32}$/.test(tk3.token) && tk3.token !== token, tk3);
  const bold = await postForm(url, form({ transcription: "old token" }), { authorization: `Bearer ${token}` });
  check("the old token is refused (401)", bold.status === 401, bold);
  const bnew = await postForm(url, form({ transcription: "new token" }), { authorization: `bearer ${tk3?.token}` });
  check("the new token is accepted (202)", bnew.status === 202, bnew);
  const off = await rpc(a, "bots.hook_token", { hook: hk.hook, op: "off" });
  const boff = await postForm(url, form({ transcription: "after off" }), { authorization: `Bearer ${tk3?.token}` });
  check("turned off, the token is refused (401)", off?.ok && boff.status === 401, [off, boff]);
  const ts2 = now();
  const b2 = JSON.stringify({ action: "closed", n: 2 });
  const rs = await post(url, b2, { "x-backplane-timestamp": String(ts2), "x-backplane-signature": sign(secret, ts2, b2) });
  check("signed calls still work beside tokens (202)", rs.status === 202, rs);
  const tk4 = await rpc(a, "bots.hook_token", { hook: hk.hook });
  check("the token is back on for the revoke check", tk4?.ok && tk4.token.length === 32, tk4);
  const unknownTok = await rpc(a, "bots.hook_token", { hook: "h0-0" });
  check("no token for an unknown hook", unknownTok && !unknownTok.ok, unknownTok);
  check("no token in the event log", !readFileSync(join(a.home, "events.jsonl"), "utf8").includes(tk4?.token ?? "zzz") && !readFileSync(join(a.home, "events.jsonl"), "utf8").includes(tk3?.token ?? "zzz"));

  // linking the hubs
  const inv = await rpc(a, "bots.invite", { url: A });
  check("bots.invite answers an invite", inv?.ok && typeof inv.invite === "string" && inv.invite.startsWith(`bp1:${A}:`), inv);
  const joined = await rpc(b, "bots.join", { invite: inv?.invite ?? "" });
  check("bots.join links", joined?.ok && joined.name === "alpha", joined);
  const peerA = await change(a, "PeerSet", (c) => !c.revoked);
  const peerB = await change(b, "PeerSet", (c) => !c.revoked);
  check("both hubs have a PeerSet", !!peerA && !!peerB, [peerA, peerB]);
  check("each names the other", peerA?.name === "beta" && peerB?.name === "alpha", [peerA, peerB]);
  const again = await rpc(b, "bots.join", { invite: inv?.invite ?? "" });
  check("an invite links once", again && !again.ok, again);
  const pid: string = inv?.peer ?? "";
  const peerSecret = JSON.parse(readFileSync(join(a.home, "secrets", "peers", pid), "utf8")).secret;
  check("both keep the same peer secret, 0600",
    peerSecret === JSON.parse(readFileSync(join(b.home, "secrets", "peers", pid), "utf8")).secret && (statSync(join(b.home, "secrets", "peers", pid)).mode & 0o777) === 0o600);

  // a person on beta writes to a bot on alpha
  const told = await rpc(b, "bots.tell", { to: "miso@alpha", text: "hello from beta" });
  check("bots.tell to name@machine answers ok", told?.ok, told);
  const posted = await change(a, "RoomPosted", (c) => c.text === "hello from beta");
  check("alpha logs the message as RoomPosted", !!posted, a.seen.filter((c) => c.$ === "RoomPosted"));
  check("from the person on beta", String(posted?.from ?? "").endsWith("@beta"), posted);
  // the bot answers in its own thread; the answer goes back to the person
  const sent = await change(a, "RoomPosted", (c) => c.from === "miso" && c.text === answer);
  check("alpha logs miso's answer in the person's direct room", sent?.room === posted?.room, sent ?? a.seen.filter((c) => c.$ === "RoomPosted"));
  const answered = await change(b, "RoomPosted", (c) => c.from === "miso@alpha" && c.text === answer);
  check("the answer reaches the person on beta, in the room they wrote in", answered?.room === "dm:miso@alpha:you", answered ?? b.seen.filter((c) => c.$ === "RoomPosted"));
  // captured once, when the turn that finished the work ended
  await sleep(1500);
  const once = a.seen.filter((c) => c.$ === "RoomPosted" && c.from === "miso" && c.text === answer && c.room === posted?.room).length;
  check("the answer goes back exactly once", once === 1, once);

  // a thread on beta mentions the bot on alpha (@name@machine): alpha's bot
  // hears it as beta's person, and its answer reaches beta's conversation
  mkdirSync(join(root, "proj-beta"), { recursive: true });
  const bp = await rpc(b, "project.add", { path: join(root, "proj-beta") });
  const bpc = await change(b, "ProjectCreated");
  const bth = await rpc(b, "thread.create", { project: bpc?.id ?? bp?.id ?? "", title: "Mention far" });
  const btc = await change(b, "ThreadCreated", (c) => c.title === "Mention far");
  check("a thread on beta", !!btc, [bp, bth]);
  // (a second on: the stand-in answers alike, and one body signed twice in
  // one second is refused as a replay)
  await sleep(1100);
  const ment = await rpc(b, "turn.start", { thread: btc?.id ?? "", text: "@miso@alpha please look", msg: "m-far-1" });
  check("a message naming miso@alpha is sent", ment?.ok, ment);
  const mlog = await change(b, "RoomPosted", (c) => c.room === "dm:miso@alpha:you" && String(c.text).includes("please look"));
  check("beta logs it in the person's conversation with miso", !!mlog && String(mlog.text).includes("Mention far"), mlog ?? b.seen.filter((c) => c.$ === "RoomPosted"));
  const mhear = await change(a, "RoomPosted", (c) => String(c.text).includes("mentioned you in the thread") && String(c.text).includes("please look"));
  check("alpha's miso hears it from beta's person", !!mhear && String(mhear.from).endsWith("@beta"), mhear ?? a.seen.filter((c) => c.$ === "RoomPosted"));
  const mback = await until(10000, () => b.seen.filter((c) => c.$ === "RoomPosted" && c.from === "miso@alpha" && c.text === answer).length >= 2);
  check("miso's answer reaches beta's conversation", !!mback, b.seen.filter((c) => c.$ === "RoomPosted"));

  // a room shared by both machines: alpha's person makes it with a bot on
  // each; beta learns it from the first post, and its side can post back
  const kelp = await rpc(b, "bots.create", { name: "kelp" });
  const mk = await rpc(a, "bots.room", { room: "r-shared", name: "both", members: `${miso?.bot}\nkelp@beta` });
  check("a room with a bot on beta", kelp?.ok && mk?.ok, [kelp, mk]);
  const sp = await rpc(a, "bots.post", { room: "r-shared", text: "hello both machines" });
  check("a person posts in it", sp?.ok, sp);
  const bset = await change(b, "RoomSet", (c) => c.id === "r-shared");
  check("beta keeps the room", !!bset && bset.name === "both", b.seen.filter((c) => c.$ === "RoomSet"));
  const bmem = String(bset?.members ?? "").split("\n").sort();
  check("as beta names its members", bmem.join() === [kelp?.bot, "miso@alpha"].sort().join(), bset);
  const bpost = await change(b, "RoomPosted", (c) => c.room === "r-shared" && c.text === "hello both machines");
  check("beta logs the post in the room", !!bpost && bpost.from === "you@alpha", bpost);
  check("the bot on beta hears it", !!(await change(b, "BotWoke", (c) => c.bot === kelp?.bot)), b.seen.filter((c) => c.$ === "BotWoke"));
  const back = await rpc(b, "bots.post", { room: "r-shared", text: "hello back" });
  check("beta's person posts back", back?.ok, back);
  const apost = await change(a, "RoomPosted", (c) => c.room === "r-shared" && c.text === "hello back");
  check("alpha logs it in the same room", !!apost && apost.from === "you@beta", a.seen.filter((c) => c.$ === "RoomPosted"));
  check("the bot on alpha hears it", !!(await change(a, "MessagePosted", (c) => JSON.stringify(c).includes("hello back"))));
  check("alpha's room is unchanged", !a.seen.some((c) => c.$ === "RoomSet" && c.id === "r-shared" && c.members !== `${miso?.bot}\nkelp@beta`), a.seen.filter((c) => c.$ === "RoomSet"));

  // the bot directory, signed
  const dts = now();
  const dir = await fetch(`${A}/bots/dir`, { headers: { "x-backplane-peer": pid, "x-backplane-timestamp": String(dts), "x-backplane-signature": sign(peerSecret, dts, "") } });
  const listed = dir.status === 200 ? await dir.json() : await dir.text();
  check("GET /bots/dir signed lists alpha's bots", dir.status === 200 && Array.isArray(listed) && listed.map((x: any) => x.name).sort().join() === "miso,nori", listed);
  const d2 = await fetch(`${A}/bots/dir`);
  check("GET /bots/dir unsigned is refused (401)", d2.status === 401, d2.status);
  const d3 = await fetch(`${A}/bots/dir`, { headers: { "x-backplane-peer": pid, "x-backplane-timestamp": String(dts), "x-backplane-signature": sign(peerSecret, dts, "") } });
  check("the same directory request again is refused (401)", d3.status === 401, d3.status);
  const d4 = await post(`${A}/bots/deliver`, JSON.stringify({ from: "x", to: "miso", text: "forged", hop: 0 }), { "x-backplane-peer": pid, "x-backplane-timestamp": String(now()), "x-backplane-signature": sign("11".repeat(32), now(), "x") });
  check("a forged delivery is refused (401)", d4.status === 401, d4);
  const d5 = await post(`${A}/bots/link`, JSON.stringify({ url: "http://127.0.0.1:1", name: "evil" }), { "x-backplane-peer": "../../token", "x-backplane-timestamp": String(now()), "x-backplane-signature": "sha256=00" });
  check("a peer id that is not a plain id is refused (401)", d5.status === 401, d5);

  // screens are for clients only
  const s1 = await fetch(`${A}/bots/${miso.bot}/screen.jpg`);
  check("a bot's screen from loopback: 404 until it has one", s1.status === 404, s1.status);

  // secrets never reach the logs or a broadcast
  const logA = readFileSync(join(a.home, "events.jsonl"), "utf8");
  const logB = readFileSync(join(b.home, "events.jsonl"), "utf8");
  check("no secret in either event log", !logA.includes(secret) && !logA.includes(peerSecret) && !logB.includes(peerSecret));
  check("no secret in a broadcast change", !JSON.stringify(a.seen).includes(secret) && !JSON.stringify(b.seen).includes(peerSecret));

  // revoking a hook: its secret goes, its calls are refused
  await rpc(a, "bots.hook_revoke", { hook: hk.hook });
  await sleep(300);
  const r11 = await post(url, body, { "x-backplane-timestamp": String(now()), "x-backplane-signature": sign(secret, now(), body) });
  check("a revoked webhook is refused (401)", r11.status === 401, r11);
  check("a revoked webhook's secret file is gone", (() => { try { statSync(hookFile); return false; } catch { return true; } })());
  check("a revoked webhook's token file is gone", (() => { try { statSync(join(a.home, "secrets", "hooks", `${hk.hook}.token`)); return false; } catch { return true; } })());
  const rtok = await postForm(url, form({ transcription: "revoked" }), { authorization: `Bearer ${tk4?.token}` });
  check("a revoked webhook's token is refused (401)", rtok.status === 401, rtok);

  if (!quick) {
    // the minute tick: a routine fires; beta lists alpha's bots
    const ro = await rpc(a, "bots.routine", { bot: nori.bot, name: "tick", cron: "* * * * *", prompt: "tick" });
    check("bots.routine", ro?.ok, ro);
    const fired = await until(75000, () => a.seen.find((c) => c.$ === "RoutineFired"));
    check("the routine fires on the minute tick", !!fired, a.seen.map((c) => c.$).slice(-10));
    const remote = await until(75000, () => {
      try { const xs = JSON.parse(b.info?.["bots.remote"] ?? "[]"); return xs.length === 2 ? xs : undefined; } catch { return undefined; }
    });
    check("beta's info lists alpha's bots (bots.remote)", !!remote && remote.every((x: any) => x.peer === pid && x.peerName === "alpha"), b.info?.["bots.remote"]);
  }
  // the hub's heartbeat (every 15 s) reaches a client that asks for nothing
  check("the hub sends a heartbeat", !!await until(20000, () => a.hb > 0 && b.hb > 0), [a.hb, b.hb]);
} catch (e) {
  check("no exception", false, String(e));
} finally {
  for (const h of hubs) {
    try { h.ws?.close(); } catch {}
    h.proc.kill();
  }
  await Promise.all(hubs.map((h) => h.proc.exited));
  if (process.env.KEEP) console.log(root); else rmSync(root, { recursive: true, force: true });
}
console.log(failed ? "bots e2e: FAIL" : "bots e2e: ok");
process.exit(failed ? 1 : 0);
