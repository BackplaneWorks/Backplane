// A store at an older codec upgraded in place equals one rebuilt from the
// same log (storeio.bend's Sto.upgrade, storedb.bend's "Upgrades in place").
//
//   bun test/tools/store_upgrade_e2e.ts OLD_HUB test/native/store_up_probe.bend [EVENTS.jsonl | OLD_HOME]
//
// OLD_HUB is a hub binary from before the codec bump (it makes the old
// store); PROBE is test/native/store_up_probe.bend (STO.Sto.boot alone, run
// with bend). Without EVENTS a synthetic log
// with every way thread.parent.* is set is used. The old hub runs once on
// a throwaway home; then the probe upgrades one copy of that home in place
// and rebuilds another from the same log, and every table must match, row
// for row. OLD_HOME (a folder with events.jsonl and an old store.db, e.g. a
// copy of a real home) skips the old hub. The e2e prints which path each
// probe took, the time and the counts.
import { cpSync, existsSync, mkdirSync, mkdtempSync, readdirSync, readFileSync, rmSync, statSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { Database } from "bun:sqlite";

const [oldHub, probe, events] = process.argv.slice(2);
if (!oldHub || !probe) throw new Error("usage: store_upgrade_e2e.ts OLD_HUB PROBE [EVENTS.jsonl]");
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
const root = mkdtempSync(join(process.env.STORE_UP_DIR ?? tmpdir(), "bp-store-up-"));
const ev = ($: string, f: object) => JSON.stringify({ $, ...f });
const delegated = (task: string, parent: string, child: string, at: number) => ev("TaskDelegated", { task, parent, child, depth: 1, provider: "claude", model: "", at });
const thread = (id: string, title: string) => ev("ThreadCreated", { id, project: "p", title, env: "local", branch: "", worktree: "", provider: "claude", model: "", at: 1 });
// storedb.bend's Up.demo, as lines, with the threads made
const synthetic = [ev("ProjectCreated", { id: "p", title: "P", root: "/p", at: 1 }), thread("tp", "Parent"), thread("tb", "Bot"), thread("tq", "Q"),
  ...["c1", "c2", "c3", "c4", "c5", "c6", "c7"].map((c) => thread(c, c)),
  ev("SettingSet", { key: "restart.continue", value: "off", at: 1 }),
  ev("BotSet", { id: "b1", name: "Bub", thread: "tb", look: 1, persona: "", at: 2 }),
  delegated("k1", "tp", "c1", 3), delegated("k2", "tb", "c2", 3), delegated("k3", "tq", "c3", 4),
  ev("BotSet", { id: "b2", name: "Two", thread: "tq", look: 1, persona: "", at: 5 }),
  delegated("k4", "tp", "c4", 6), ev("ThreadParentSet", { id: "c4", parent: "tx", at: 7 }),
  ev("ThreadParentSet", { id: "c5", parent: "ty", at: 7 }), delegated("k5", "tp", "c5", 8),
  ev("SettingSet", { key: "thread.parent.c6", value: "tz", at: 8 }),
  ev("MessagePosted", { thread: "tp", id: "m1", role: "user", text: 'BotSet "TaskDelegated"', at: 9 }),
  ev("BotDeleted", { id: "b1", at: 10 }), delegated("k6", "tb", "c7", 11),
  ev("TaskStateSet", { task: "k1", state: "done", summary: "done", at: 12 }), ev("TaskDelivered", { task: "k1", at: 12 }),
  ev("SettingSet", { key: "thread.parentx", value: "no", at: 12 }),
  ev("ActivityLogged", { thread: "tp", id: "a1", tone: "tool", kind: "BotDeleted", text: "x", at: 13 }),
  ev("ThreadParentSet", { id: "c2", parent: "", at: 14 })].join("\n") + "\n";

let fail = 0;
const check = (ok: unknown, what: string) => { console.log(`${ok ? "ok" : "FAIL"} ${what}`); if (!ok) fail++; };
const freePort = () => { const s = Bun.listen({ hostname: "127.0.0.1", port: 0, socket: { data() {} } }); const p = s.port; s.stop(true); return p; };
const env = { PATH: "/usr/bin:/bin", HOME: root, BACKPLANE_COPY: "1", BACKPLANE_AGENTS: join(root, "stand"), BACKPLANE_KISTACK_SYNC: "0", BACKPLANE_ORDER_SERVER: "http://127.0.0.1:9", DISPLAY: "", BACKPLANE_NO_UPDATE: "1", BACKPLANE_PEERS: "", BACKPLANE_GOOGLE_BAKE: "0" };
const tables = (file: string) => {
  const db = new Database(file, { readonly: true });
  try {
    const names = db.query("SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'entry_fts%' AND name NOT LIKE 'sqlite_%' ORDER BY name").values().map((r) => String(r[0]));
    const out: Record<string, string[]> = {};
    for (const t of names) out[t] = db.query(`SELECT * FROM "${t}"`).values().map((r) => JSON.stringify(r, (_k, v) => (typeof v === "bigint" ? String(v) : v))).sort();
    out["entry_fts"] = db.query("SELECT rowid, text FROM entry_fts").values().map((r) => JSON.stringify(r)).sort();
    return out;
  } finally { db.close(); }
};
const meta = (file: string) => Object.fromEntries(new Database(file, { readonly: true }).query("SELECT k, v FROM meta").values().map(([k, v]) => [String(k), String(v)]));

try {
  mkdirSync(join(root, "stand"));
  const given = !!events && statSync(events).isDirectory();
  const old = given ? resolve(events) : join(root, "old");
  if (!given) {
  mkdirSync(old);
  writeFileSync(join(old, "events.jsonl"), events ? readFileSync(events) : synthetic);
  // the old hub builds its store and stops
  const port = freePort();
  const hub = Bun.spawn([resolve(oldHub), "--home", old, "--port", String(port), "--host", "127.0.0.1", "--no-tailscale", "--headless", "--foreground"], { env, stdout: "ignore", stderr: Bun.file(join(root, "old.log")) });
  let up = false;
  for (let i = 0; i < 6000 && !up; i++) { try { up = (await fetch(`http://127.0.0.1:${port}/hello`)).ok; } catch {} if (!up) await sleep(100); }
  check(up, "the old hub made its store");
  await sleep(1500);
  hub.kill("SIGTERM");
  await hub.exited;
  }
  const was = meta(join(old, "store.db"));
  console.log(`old store: codec ${was.codec}, ${was.applied_seq} lines`);
  // one copy upgraded in place, one rebuilt from the same log
  const a = join(root, "upgraded"), b = join(root, "rebuilt");
  for (const d of [a, b]) mkdirSync(d);
  cpSync(join(old, "events.jsonl"), join(a, "events.jsonl"));
  cpSync(join(old, "events.jsonl"), join(b, "events.jsonl"));
  for (const s of ["", "-wal", "-shm"]) if (existsSync(join(old, "store.db" + s))) cpSync(join(old, "store.db" + s), join(a, "store.db" + s));
  const run = async (home: string) => {
    const t0 = performance.now();
    // a .bend probe runs under bend itself (its IO main), a built one as it is
    const cmd = probe.endsWith(".bend") ? ["bend", resolve(probe), home] : [resolve(probe), home];
    const p = Bun.spawn(cmd, { env: { ...env, PATH: `${process.env.HOME}/.bend/bin:${env.PATH}`, BEND_NO_TELEMETRY: "1", HOME: process.env.HOME ?? root }, stdout: "pipe", stderr: "pipe" });
    const [out, err] = await Promise.all([new Response(p.stdout).text(), new Response(p.stderr).text()]);
    await p.exited;
    return { ms: Math.round(performance.now() - t0), log: out + err };
  };
  const ra = await run(a), rb = await run(b);
  const path = (log: string) => (log.includes("upgraded store.db in place") ? "upgraded in place" : log.includes("in-place upgrade failed") ? "upgrade failed, rebuilt" : "rebuilt or opened");
  console.log(`upgrade: ${path(ra.log)} in ${ra.ms} ms; rebuild: ${path(rb.log)} in ${rb.ms} ms`);
  console.log((ra.log.match(/upgraded store\.db[^\n]*/) ?? [""])[0]);
  check(path(ra.log) === "upgraded in place", "the old store was upgraded in place, not rebuilt");
  const ta = tables(join(a, "store.db")), tb = tables(join(b, "store.db"));
  // far_line is written by commits and the catch-up (storeio.bend's Plan.far)
  // but not by the full rebuild (stobuild.bend), so a rebuilt store has none:
  // the upgrade must leave the old store's rows as they were instead
  const to = tables(join(old, "store.db"));
  check(JSON.stringify(ta["far_line"] ?? []) === JSON.stringify(to["far_line"] ?? []), `far_line kept as the old store had it (${(to["far_line"] ?? []).length} rows)`);
  let differ = 0;
  for (const t of new Set([...Object.keys(ta), ...Object.keys(tb)].filter((t) => t !== "far_line"))) {
    const x = ta[t] ?? [], y = tb[t] ?? [];
    const sx = new Set(x), sy = new Set(y);
    const only = (p: string[], q: Set<string>) => p.filter((r) => !q.has(r));
    const oa = only(x, sy), ob = only(y, sx);
    if (oa.length || ob.length || x.length !== y.length) {
      differ++;
      console.log(`  ${t}: ${x.length} vs ${y.length} rows; only upgraded ${oa.length}, only rebuilt ${ob.length}`);
      for (const r of oa.slice(0, 3)) console.log(`    upgraded: ${r.slice(0, 160)}`);
      for (const r of ob.slice(0, 3)) console.log(`    rebuilt:  ${r.slice(0, 160)}`);
    }
  }
  // an upgrade that fails is rolled back whole and the store rebuilt from
  // the log: a trigger refuses the upgrade's writes on a third copy
  const c = join(root, "failing");
  mkdirSync(c);
  cpSync(join(old, "events.jsonl"), join(c, "events.jsonl"));
  for (const x of ["", "-wal", "-shm"]) if (existsSync(join(old, "store.db" + x))) cpSync(join(old, "store.db" + x), join(c, "store.db" + x));
  const tdb = new Database(join(c, "store.db"));
  tdb.exec("CREATE TRIGGER refuse_up BEFORE INSERT ON setting WHEN substr(NEW.k, 1, 14) = 'thread.parent.' BEGIN SELECT RAISE(ABORT, 'injected upgrade failure'); END");
  tdb.close();
  const rc = await run(c);
  console.log(`failing upgrade: ${path(rc.log)} in ${rc.ms} ms`);
  check(path(rc.log) === "upgrade failed, rebuilt", "a failed upgrade falls back to the rebuild, and says so");
  const tc = tables(join(c, "store.db"));
  check(JSON.stringify(Object.keys(tc).filter((t) => t !== "far_line").map((t) => tc[t])) === JSON.stringify(Object.keys(tb).filter((t) => t !== "far_line").map((t) => tb[t])), "the fallback's store equals the rebuilt one");
  const kept = readdirSync(c).filter((n) => n.startsWith("store.db.previous-") && !n.includes("-wal") && !n.includes("-shm"));
  check(kept.length === 1 && meta(join(c, kept[0])).codec === was.codec, "the store it set aside is the old one, unchanged (no half upgrade)");
  const parents = (ta["setting"] ?? []).filter((r) => r.startsWith('["thread.parent.')).length;
  console.log(`thread.parent rows: ${parents} upgraded, ${(tb["setting"] ?? []).filter((r) => r.startsWith('["thread.parent.')).length} rebuilt`);
  check(differ === 0, "every other table of the upgraded store equals the rebuilt one, row for row");
} finally {
  if (process.env.KEEP) console.log(root); else rmSync(root, { recursive: true, force: true });
}
console.log(fail ? `${fail} failed` : "all ok");
process.exit(fail ? 1 : 0);
