// A peer that skipped lines inside a far_line hole ends consistent after one
// snapshot pull (far.bend's Fr.view.tag view:3, storefar.bend's refill).
//
//   bun test/tools/far_line_peer_e2e.ts OLD_HUB NEW_HUB [WIREDIR]
//
// OLD_HUB: a hub from before stobuild.bend wrote far_line (and with view:2);
// NEW_HUB: this tree's. Two hubs that pair by themselves (BACKPLANE_PEERS),
// alpha the owner, beta its mirror:
// 1. both on OLD: beta mirrors alpha's project and thread;
// 2. beta away; alpha renames the thread three times and makes a thread;
// 3. the loss: alpha's store is rebuilt by OLD (far_line loses its history),
//    beta comes back on OLD and pulls from its head: it skips what happened
//    in 2 (the hole);
// 4. both on NEW: alpha refills far_line in the background, beta's first
//    pull meets the new epoch and gets one snapshot of what is live: its
//    mirror ends consistent; the snapshot's size is printed. A pull after
//    that brings only what is new.
import { chmodSync, existsSync, mkdirSync, mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { Database } from "bun:sqlite";

const [oldBin, newBin, wire = "build/wire"] = process.argv.slice(2);
if (!oldBin || !newBin) throw new Error("usage: far_line_peer_e2e.ts OLD_HUB NEW_HUB [WIREDIR]");
for (const f of readdirSync(wire).filter((f) => f.endsWith(".js"))) (0, eval)(readFileSync(`${wire}/${f}`, "utf8"));
const W = (globalThis as any).Wire;
let fail = 0;
const check = (ok: unknown, what: string, got?: unknown) => { console.log(`${ok ? "ok" : "FAIL"} ${what}${ok || got === undefined ? "" : ": " + JSON.stringify(got).slice(0, 400)}`); if (!ok) fail++; };
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
const freePort = () => { const s = Bun.listen({ hostname: "127.0.0.1", port: 0, socket: { data() {} } }); const p = s.port; s.stop(true); return p; };
const root = mkdtempSync(join(process.env.STORE_UP_DIR ?? tmpdir(), "bp-far-peer-"));
const fake = join(root, "bin");
mkdirSync(fake);
for (const n of ["claude", "codex", "grok", "muse"]) { writeFileSync(join(fake, n), "#!/bin/sh\nexit 1\n"); chmodSync(join(fake, n), 0o755); }

type Hub = { name: string; home: string; port: number; peers: string; proc: ReturnType<typeof Bun.spawn> | null };
type Client = { ws: WebSocket; seen: any[]; far: any[]; replies: Map<number, any>; n: number; info: any };
const spawn = (h: Hub, bin: string) => {
  h.proc = Bun.spawn([resolve(bin), "--home", h.home, "--port", String(h.port), "--host", "127.0.0.1", "--no-tailscale", "--headless", "--foreground"], {
    env: { PATH: `${fake}:/usr/bin:/bin`, HOME: root, DISPLAY: "", BACKPLANE_NAME: h.name, BACKPLANE_NO_UPDATE: "1", BACKPLANE_ORDER_SERVER: "http://127.0.0.1:9", BACKPLANE_KEYRING_LOCKED: "0", BACKPLANE_PEERS: h.peers, BACKPLANE_AGENTS: fake, BACKPLANE_KISTACK_SYNC: "0", BACKPLANE_GOOGLE_BAKE: "0" },
    stdout: "ignore", stderr: Bun.file(join(root, `${h.name}-${Date.now()}.log`)),
  });
};
const up = async (h: Hub) => { for (let i = 0; i < 600; i++) { try { if ((await fetch(`http://127.0.0.1:${h.port}/hello`)).ok) return; } catch {} await sleep(100); } throw new Error(`${h.name} did not start`); };
const stop = async (h: Hub) => { h.proc?.kill("SIGTERM"); await h.proc?.exited; h.proc = null; };
const connect = async (h: Hub): Promise<Client> => {
  const c: Client = { ws: null as any, seen: [], far: [], replies: new Map(), n: 0, info: {} };
  const ws = new WebSocket(`ws://127.0.0.1:${h.port}/ws`);
  ws.binaryType = "arraybuffer";
  ws.onmessage = (e) => {
    const o = JSON.parse(W.decode(new Uint8Array(e.data as ArrayBuffer)));
    for (const x of o.items ?? []) c.seen.push(x);
    if (o.t === "far") c.far.push(o);
    if (o.t === "reply") c.replies.set(Number(o.id), o);
    if (o.info) c.info = { ...c.info, ...o.info };
  };
  await new Promise((r, j) => { ws.onopen = r; ws.onerror = j; });
  c.ws = ws;
  return c;
};
const until = async <T>(ms: number, f: () => T | undefined | null | false): Promise<T | undefined> => { const end = Date.now() + ms; for (;;) { const v = f(); if (v) return v; if (Date.now() > end) return undefined; await sleep(100); } };
const rpc = async (c: Client, m: string, p: object) => { const id = ++c.n; c.ws.send(W.encode(JSON.stringify({ id, m, p }))); return await until(20000, () => c.replies.get(id)); };
const farItems = (c: Client, link: string) => c.far.filter((o) => o.link === link).flatMap((o) => (o.items ?? []).map((i: any) => i.c));
// the title beta's mirror shows for a thread: its last rename, or its creation
const title = (c: Client, link: string, th: string) => {
  let t: string | undefined;
  for (const x of farItems(c, link)) { if (x.$ === "ThreadCreated" && x.id === th) t = x.title; if (x.$ === "ThreadRenamed" && x.id === th) t = x.title; }
  return t;
};
const pa = freePort(), pb = freePort();
const a: Hub = { name: "alpha", home: join(root, "alpha"), port: pa, peers: `http://127.0.0.1:${pb}`, proc: null };
const b: Hub = { name: "beta", home: join(root, "beta"), port: pb, peers: `http://127.0.0.1:${pa}`, proc: null };
// a live thread (one stamped long ago is settled, and a snapshot carries
// only what is live: settled threads are answered on demand)
const now = Math.floor(Date.now() / 1000);
const ev = ($: string, f: object) => JSON.stringify({ $, ...f, at: now });
try {
  for (const h of [a, b]) mkdirSync(h.home);
  writeFileSync(join(a.home, "events.jsonl"), [ev("ProjectCreated", { id: "p", title: "Shared", root: join(root, "proj") }),
    ev("ThreadCreated", { id: "t1", project: "p", title: "first", env: "local", branch: "", worktree: "", provider: "claude", model: "" }),
    ev("SettingSet", { key: "restart.continue", value: "off" })].join("\n") + "\n");
  mkdirSync(join(root, "proj"));
  // 1. both on OLD; they pair, beta mirrors
  spawn(a, oldBin); spawn(b, oldBin); await up(a); await up(b);
  let ca = await connect(a), cb = await connect(b);
  const peer = await until(300000, () => cb.seen.find((c) => c.$ === "PeerSet" && !c.revoked));
  const link = peer?.id as string;
  check(!!link, "the hubs pair by themselves");
  check(await until(120000, () => title(cb, link, "t1") === "first"), "1. beta mirrors alpha's thread", farItems(cb, link).map((x) => x.$));
  // 2. beta away; alpha changes what beta has not seen
  cb.ws.close(); await stop(b);
  for (const t of ["second", "third", "fourth"]) { const r = await rpc(ca, "thread.rename", { thread: "t1", title: t }); check(!!r?.ok, `2. alpha renames t1 to ${t}`, r); }
  const made = await rpc(ca, "thread.create", { project: "p", title: "made while beta was away" });
  check(!!made?.ok, "2. alpha makes a thread while beta is away", made);
  await sleep(1000);
  ca.ws.close(); await stop(a);
  // 3. the loss: OLD rebuilds alpha's store; beta pulls on OLD from its head
  for (const s of ["", "-wal", "-shm"]) rmSync(join(a.home, "store.db" + s), { force: true });
  spawn(a, oldBin); await up(a);
  const lost = new Database(join(a.home, "store.db"), { readonly: true });
  const rowsAfterLoss = Number(lost.query("SELECT count(*) FROM far_line").values()[0][0]);
  lost.close();
  console.log(`  alpha's far_line after OLD's rebuild: ${rowsAfterLoss} rows`);
  spawn(b, oldBin); await up(b);
  cb = await connect(b);
  await sleep(90000);
  const holed = title(cb, link, "t1");
  console.log(`  beta's mirror after pulling across the hole: t1 is "${holed}"`);
  check(holed !== "fourth", "3. on OLD, beta skipped the renames inside the hole (the bug)", holed);
  cb.ws.close(); await stop(b); await stop(a);
  // 4. both on NEW: alpha refills, beta gets one snapshot and is consistent
  spawn(a, newBin); await up(a);
  ca = await connect(a);
  const filled = await until(120000, () => { const d = new Database(join(a.home, "store.db"), { readonly: true }); try { return (d.query("SELECT v FROM meta WHERE k = 'far_line'").values()[0] ?? [])[0] === "1"; } finally { d.close(); } });
  check(filled, "4. alpha refills far_line in the background and marks it");
  const fdb = new Database(join(a.home, "store.db"), { readonly: true });
  const rowsNow = Number(fdb.query("SELECT count(*) FROM far_line").values()[0][0]);
  const lines = Number(fdb.query("SELECT count(*) FROM line").values()[0][0]);
  fdb.close();
  console.log(`  alpha's far_line after the refill: ${rowsNow} rows of ${lines} lines`);
  // beta's mirror as it keeps it (its file: what a client gets on joining)
  const mirror = () => { const f = join(b.home, "far", `${link}.jsonl`); return existsSync(f) ? readFileSync(f, "utf8") : ""; };
  const items = (text: string) => text.split("\n").filter((l) => l.startsWith('{"n":')).map((l) => JSON.parse(l).c);
  const titleIn = (xs: any[], th: string) => { let t: string | undefined; for (const x of xs) { if (x.$ === "ThreadCreated" && x.id === th) t = x.title; if (x.$ === "ThreadRenamed" && x.id === th) t = x.title; } return t; };
  spawn(b, newBin); await up(b);
  cb = await connect(b);
  // with nothing changing on alpha: beta comes back when it finds alpha again
  // (the machines list is probed every 2 minutes), meets the new epoch and
  // takes one snapshot
  const t4 = Date.now();
  const fixed = await until(330000, () => titleIn(items(mirror()), "t1") === "fourth" && items(mirror()).some((x) => x.$ === "ThreadCreated" && x.title === "made while beta was away"));
  console.log(`  beta consistent ${Math.round((Date.now() - t4) / 1000)} s after it started`);
  const snapText = mirror(), snapItems = items(snapText).length;
  check(fixed, "4. beta's mirror ends consistent: t1 is \"fourth\", and it has the thread made in the hole", titleIn(items(snapText), "t1"));
  console.log(`  the snapshot beta keeps: ${snapItems} items, ${snapText.length} bytes, against ${lines} lines in alpha's log`);
  check(snapItems > 0 && snapItems < lines, "4. the snapshot is the live state, smaller than the history", { snapItems, lines });
  // a later change arrives alone
  const seen = cb.far.length;
  const r5 = await rpc(ca, "thread.rename", { thread: "t1", title: "fifth" });
  check(!!r5?.ok && await until(60000, () => titleIn(items(mirror()), "t1") === "fifth"), "4. a later change reaches beta's mirror");
  const later = cb.far.slice(seen).filter((o) => o.link === link).reduce((n, o) => n + (o.items ?? []).length, 0);
  check(later > 0 && later <= 3, "4. and comes alone, not with the history", later);
} finally {
  for (const h of [a, b]) await stop(h);
  if (process.env.KEEP) console.log(root); else rmSync(root, { recursive: true, force: true });
}
console.log(fail ? `${fail} failed` : "all ok");
process.exit(fail ? 1 : 0);
