// End to end: the far feed is made only when a linked machine first asks
// (far.bend Fh.feed.ask, server.bend Far.feed.want), a slice at a time
// (Fr.feed.some, Far.feed.run). Alpha starts on a synthetic log big enough
// that the feed takes seconds to make; alpha and beta pair by themselves
// (BACKPLANE_PEERS, client.bend's Mate.*). A pull signed as beta while the
// feed is being made is told 503 "not ready"; pulled again until it is
// ready, the pages (followed while "more") hold every shared line of the
// log exactly once, under its own number, in order, with its text: the
// pages the whole-log feed gave. Prints "ok ..." / "FAIL ..." lines.
//
//   bun test/tools/far_feed_lazy_e2e.ts [BINARY] [WIREDIR] [LINES]
//
// BINARY defaults to build/backplane-serve (scripts/build-js.sh
// src/server/main.bend build/backplane-serve); WIREDIR to build/wire (bend
// test/wire/index.html -o build/wire); LINES (messages in the log) 60000.
import { mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync, chmodSync, mkdirSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { createHmac } from "node:crypto";

const args = process.argv.slice(2);
const [bin = "build/backplane-serve", wire = "build/wire", nstr = "60000"] = args;
const N = Number(nstr);
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
async function until<T>(ms: number, f: () => T | undefined | null | false | Promise<T | undefined | null | false>): Promise<T | undefined> {
  const end = Date.now() + ms;
  for (;;) {
    const v = await f();
    if (v) return v;
    if (Date.now() > end) return undefined;
    await sleep(50);
  }
}

const root = mkdtempSync(join(tmpdir(), "bp-far-lazy-"));
const fake = join(root, "bin");
mkdirSync(fake);
for (const n of ["claude", "codex", "grok", "muse"]) {
  writeFileSync(join(fake, n), "#!/bin/sh\nexit 1\n");
  chmodSync(join(fake, n), 0o755);
}

type Hub = { name: string; home: string; port: number; peers: string; proc: ReturnType<typeof Bun.spawn> | null };
function spawn(h: Hub) {
  h.proc = Bun.spawn([resolve(bin), "--home", h.home, "--port", String(h.port), "--no-tailscale"], {
    env: { ...process.env, HOME: join(root, "fakehome"), DISPLAY: "", WAYLAND_DISPLAY: "", BACKPLANE_NAME: h.name, BACKPLANE_NO_UPDATE: "1", BACKPLANE_KEYRING_LOCKED: "0",
      BACKPLANE_PEERS: h.peers, BACKPLANE_AGENTS: fake, PATH: `${fake}:/usr/bin:/bin` },
    stdout: "ignore",
    stderr: "ignore",
  });
}
async function up(h: Hub) {
  const ok = await until(120000, async () => {
    try { return (await fetch(`http://127.0.0.1:${h.port}/hello`)).status === 200; } catch { return false; }
  });
  if (!ok) throw new Error(`${h.name} did not start`);
}

// alpha's log: a project and a thread in it, then N messages (shared),
// a setting after every tenth (not shared)
const pa = freePort();
const pb = freePort();
const a: Hub = { name: "alpha", home: join(root, "alpha"), port: pa, peers: `http://127.0.0.1:${pb}`, proc: null };
const b: Hub = { name: "beta", home: join(root, "beta"), port: pb, peers: `http://127.0.0.1:${pa}`, proc: null };
mkdirSync(a.home, { recursive: true });
mkdirSync(join(root, "proj"));
mkdirSync(join(root, "fakehome"));
const at0 = Math.floor(Date.now() / 1000) - 100000;
const seed: any[] = [
  { $: "SettingSet", key: "restart.continue", value: "off", at: at0 },
  { $: "ProjectCreated", id: "pz", title: "Lazy", root: join(root, "proj"), at: at0 },
  { $: "ThreadCreated", id: "tz", project: "pz", title: "Feed", env: "local", branch: "", worktree: "", provider: "claude", model: "", at: at0 },
];
for (let i = 0; i < N; i++) {
  seed.push({ $: "MessagePosted", thread: "tz", msg: `m${i}`, role: i % 2 ? "assistant" : "user", text: `message ${i} ` + "x".repeat(80), at: at0 + 1 + Math.floor(i / 10) });
  if (i % 10 === 9) seed.push({ $: "SettingSet", key: "noise", value: String(i), at: at0 + 1 + Math.floor(i / 10) });
}
const lines = seed.map((c) => JSON.stringify(c));
writeFileSync(join(a.home, "events.jsonl"), lines.join("\n") + "\n");
// what the whole-log feed shares of the seed: every line but the settings,
// numbered by its place in the log (from 1)
const want = new Map<number, string>();
seed.forEach((c, i) => { if (c.$ !== "SettingSet") want.set(i + 1, lines[i]); });

function signed(secret: string, link: string, body: string) {
  const ts = Math.floor(Date.now() / 1000);
  const sig = "sha256=" + createHmac("sha256", Buffer.from(secret, "hex")).update(`${ts}.${body}`).digest("hex");
  return { "content-type": "application/json", "x-backplane-peer": link, "x-backplane-timestamp": String(ts), "x-backplane-signature": sig };
}

try {
  const t0 = Date.now();
  spawn(a);
  spawn(b);
  await up(a);
  await up(b);
  console.log(`  both up after ${Date.now() - t0} ms (alpha replays ${lines.length} lines)`);
  // paired: beta's link to alpha has a secret on both sides
  const peersDir = join(a.home, "secrets", "peers");
  const link = await until(300000, () => { try { return readdirSync(peersDir)[0]; } catch { return undefined; } });
  check("they pair by themselves", !!link, link);
  if (!link) throw new Error("no pairing");
  const secret = JSON.parse(readFileSync(join(peersDir, link), "utf8")).secret as string;
  // (a nonce in each body: the same signed body twice in a second is
  // refused as a replay)
  let nonce = 0;
  // the history's epoch the owner names, echoed as a peer does (a pull
  // with another epoch is answered from 0: its history changed)
  let epoch = "";
  const pull = async (since: number) => {
    const body = JSON.stringify({ since, epoch, nonce: `${Date.now()}-${++nonce}` });
    const r = await fetch(`http://127.0.0.1:${pa}/far/log`, { method: "POST", body, headers: signed(secret, link, body) });
    return { status: r.status, text: await r.text() };
  };

  // a pull is let in once alpha lists beta among the owner's machines
  // (probed every 2 minutes; 403 until then); the first pull let in
  // (beta's or this one) starts the feed: while it is being made, a pull
  // is told "not ready"
  const first = (await until(300000, async () => { const r = await pull(0); return r.status !== 403 ? r : undefined; })) ?? { status: 403, text: "never let in" };
  check("a pull while the feed is being made is told 503 not ready", first.status === 503 && first.text.includes("not ready"), first);

  // pulled again until it is ready
  const t1 = Date.now();
  const ready = await until(180000, async () => { const r = await pull(0); return r.status === 200 ? r : undefined; });
  check("then the feed is ready", !!ready, ready);
  console.log(`  feed ready ${Date.now() - t1} ms after the first 503`);

  // every page, from 0, while "more"
  const got = new Map<number, string>();
  let since = 0, pages = 0, ascending = true, dup = 0, last = 0;
  const tp = Date.now();
  for (;;) {
    const tq = Date.now();
    const r = await pull(since);
    if (r.status !== 200) { check("pages after ready answer 200", false, r); break; }
    const o = JSON.parse(r.text);
    epoch = o.epoch ?? epoch;
    pages++;
    console.log(`  page ${pages}: since ${since} -> head ${o.head}, ${o.items?.length ?? 0} items, more ${o.more}, ${r.text.length} bytes, ${Date.now() - tq} ms`);
    if (o.more && !(o.head > since)) { check("a page with more moves its head on", false, { since, head: o.head }); break; }
    for (const it of o.items ?? []) {
      if (it.n <= last) ascending = false;
      last = it.n;
      if (got.has(it.n)) dup++;
      got.set(it.n, JSON.stringify(it.c));
    }
    if (!o.more) break;
    since = o.head;
  }
  let missing = 0, differ = 0;
  for (const [n, text] of want) {
    const g = got.get(n);
    if (g === undefined) missing++;
    else if (JSON.stringify(JSON.parse(text)) !== g) differ++;
  }
  const stray = [...got.keys()].filter((n) => n <= lines.length && !want.has(n)).length;
  console.log(`  ${pages} pages, ${got.size} items (the seed shares ${want.size}) in ${Date.now() - tp} ms`);
  check("every shared line once, under its own number, in order, with its text", missing === 0 && differ === 0 && dup === 0 && ascending && stray === 0,
    { missing, differ, dup, ascending, stray });
} finally {
  for (const h of [a, b]) if (h.proc) { h.proc.kill(); await h.proc.exited; }
  rmSync(root, { recursive: true, force: true });
}
console.log(failed ? "far feed lazy e2e: FAIL" : "far feed lazy e2e: ok");
process.exit(failed ? 1 : 0);
