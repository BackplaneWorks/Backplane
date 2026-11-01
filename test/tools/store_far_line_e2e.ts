// A store built from the whole log equals one that caught up line by line
// (the commit's and the catch-up's path, storeio.bend's Plan.change), every
// table and far_line included: the rows the owner's other machines read
// (/far/log v3) are the same however the store was made.
//
//   bun test/tools/store_far_line_e2e.ts [test/native/store_up_probe.bend] [test/native/store_far_probe.bend]
//
// The probe (STO.Sto.boot alone, run with bend) builds one home from the
// whole synthetic log; another from its first part, then catches up the rest.
// Then a store with far_line emptied (as a rebuild before the fix left it) is
// refilled by the hub's child (storefar.bend's Fl.child, store_far_probe):
// its /far/log v3 feed must be the one the caught-up store gives.
import { appendFileSync, cpSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { Database } from "bun:sqlite";

const probe = resolve(process.argv[2] ?? "test/native/store_up_probe.bend");
const farProbe = resolve(process.argv[3] ?? "test/native/store_far_probe.bend");
const root = mkdtempSync(join(process.env.STORE_UP_DIR ?? tmpdir(), "bp-far-line-"));
const ev = ($: string, f: object) => JSON.stringify({ $, ...f });
const thread = (id: string, project: string) => ev("ThreadCreated", { id, project, title: id, env: "local", branch: "", worktree: "", provider: "claude", model: "", at: 1 });
// the first part is over 4096 bytes, so the log's id (its first 4 KB) is the
// same before and after the rest is appended
const first = [ev("ProjectCreated", { id: "p", title: "P" + "x".repeat(5000), root: "/p", at: 1 }), thread("t1", "p"), thread("t2", "p"),
  ev("MessagePosted", { thread: "t1", id: "m1", role: "user", text: "hello", at: 2 })];
const rest = [ev("ProjectCreated", { id: "q", title: "Q", root: "/q", at: 3 }), thread("t3", "q"), thread("l1~t9", "p"),
  ev("MessagePosted", { thread: "t3", id: "m2", role: "user", text: "in q", at: 4 }),
  ev("MessagePosted", { thread: "l1~t9", id: "m3", role: "user", text: "a mirror's", at: 4 }),
  ev("ActivityLogged", { thread: "t2", id: "a1", tone: "tool", kind: "Bash", text: "ls", at: 5 }),
  ev("TurnChanged", { thread: "t1", state: "running", at: 6 }), ev("TurnChanged", { thread: "t1", state: "completed", at: 7 }),
  ev("SettingSet", { key: "agent.model", value: "opus", at: 7 }),
  ev("TaskDelegated", { task: "k1", parent: "t1", child: "t2", depth: 1, provider: "claude", model: "", at: 8 }),
  ev("ThreadRenamed", { id: "t2", title: "Two", at: 9 }),
  // a line far longer than a read, and a blank one (the store numbers only
  // the lines that parse, and so must the refill)
  ev("ActivityLogged", { thread: "t1", id: "a2", tone: "tool", kind: "Bash", text: "y".repeat(3 * 1048576), at: 9 }), "",
  ev("MessagePosted", { thread: "t2", id: "m4", role: "user", text: "after the long line", at: 9 }),
  ev("ProjectRemoved", { id: "q", at: 10 })];
let fail = 0;
const check = (ok: unknown, what: string) => { console.log(`${ok ? "ok" : "FAIL"} ${what}`); if (!ok) fail++; };
const run = async (home: string, file = probe) => {
  // bend with the stack check.sh gives it (a multi-MB line recurses deep)
  const p = Bun.spawn(["sh", "-c", 'ulimit -s 1048576 2>/dev/null; exec bend "$@"', "sh", file, home], { env: { PATH: `${process.env.HOME}/.bend/bin:/usr/bin:/bin`, HOME: process.env.HOME ?? root, BEND_NO_TELEMETRY: "1", BACKPLANE_COPY: "1", BUN_JSC_maxPerThreadStackUsage: "4000000000" }, stdout: "pipe", stderr: "pipe" });
  const [o, e] = await Promise.all([new Response(p.stdout).text(), new Response(p.stderr).text()]);
  await p.exited;
  return o + e;
};
const tables = (file: string) => {
  const db = new Database(file, { readonly: true });
  try {
    const names = db.query("SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'entry_fts%' AND name NOT LIKE 'sqlite_%' ORDER BY name").values().map((r) => String(r[0]));
    const out: Record<string, string[]> = {};
    for (const t of names) out[t] = db.query(`SELECT * FROM "${t}"`).values().map((r) => JSON.stringify(r, (_k, v) => (typeof v === "bigint" ? String(v) : v))).sort();
    return out;
  } finally { db.close(); }
};
try {
  const whole = join(root, "whole"), caught = join(root, "caught");
  for (const d of [whole, caught]) mkdirSync(d);
  writeFileSync(join(whole, "events.jsonl"), [...first, ...rest].join("\n") + "\n");
  writeFileSync(join(caught, "events.jsonl"), first.join("\n") + "\n");
  await run(whole);
  await run(caught);
  appendFileSync(join(caught, "events.jsonl"), rest.join("\n") + "\n");
  const caughtOut = await run(caught);
  if (process.env.KEEP) console.log(`catch-up probe: ${caughtOut.slice(-600)}`);
  const w = tables(join(whole, "store.db")), c = tables(join(caught, "store.db"));
  check((w["far_line"] ?? []).length > 0, `the whole build wrote far_line (${(w["far_line"] ?? []).length} rows)`);
  check(JSON.stringify(w["far_line"]) === JSON.stringify(c["far_line"]), "far_line: the whole build equals the catch-up");
  let differ = 0;
  for (const t of new Set([...Object.keys(w), ...Object.keys(c)])) {
    if (JSON.stringify(w[t] ?? []) !== JSON.stringify(c[t] ?? [])) { differ++; console.log(`  ${t}: ${(w[t] ?? []).length} vs ${(c[t] ?? []).length} rows`); }
  }
  check(differ === 0, "every table: the whole build equals the catch-up");
  // the feed the owner's other machines read (server.bend's /far/log v3 query)
  const feed = (file: string) => {
    const db = new Database(file, { readonly: true });
    try { return JSON.stringify(db.query("SELECT f.seq, f.scope, l.off, l.len, l.wire FROM far_line f JOIN line l ON l.seq = f.seq ORDER BY f.seq").values()); } finally { db.close(); }
  };
  const missing = join(root, "missing");
  mkdirSync(missing);
  cpSync(join(whole, "events.jsonl"), join(missing, "events.jsonl"));
  cpSync(join(whole, "store.db"), join(missing, "store.db"));
  const mdb = new Database(join(missing, "store.db"));
  mdb.exec("DELETE FROM far_line; DELETE FROM meta WHERE k = 'far_line'");
  mdb.close();
  check(feed(join(missing, "store.db")) === "[]", "a store rebuilt before the fix has no far feed");
  const out = await run(missing, farProbe);
  console.log(`refill: ${out.trim().split("\n").pop()}`);
  check(feed(join(missing, "store.db")) === feed(join(caught, "store.db")), "after the refill the far feed is the caught-up store's, row for row");
  const again = await run(missing, farProbe);
  check(again.includes("current"), "a refilled store is not refilled again");
  // a refill a restart cut off: its rows up to the checkpoint are there, the
  // rest not, no mark; the next one writes only past the checkpoint
  const cut = join(root, "cut");
  mkdirSync(cut);
  cpSync(join(whole, "events.jsonl"), join(cut, "events.jsonl"));
  cpSync(join(whole, "store.db"), join(cut, "store.db"));
  const cdb = new Database(join(cut, "store.db"));
  const mid = Number(cdb.query("SELECT seq FROM far_line ORDER BY seq LIMIT 1 OFFSET 6").values()[0][0]);
  cdb.exec(`DELETE FROM far_line WHERE seq > ${mid}; DELETE FROM meta WHERE k = 'far_line'; INSERT OR REPLACE INTO meta(k, v) VALUES ('far_line.seq', '${mid}')`);
  // a row up to the checkpoint that differs shows nothing below it is written again
  cdb.exec(`UPDATE far_line SET scope = 'kept' WHERE seq = ${mid}`);
  cdb.close();
  const resumed = await run(cut, farProbe);
  console.log(`resumed refill: ${resumed.trim().split("\n").pop()}`);
  const rdb = new Database(join(cut, "store.db"), { readonly: true });
  const keptRow = rdb.query(`SELECT scope FROM far_line WHERE seq = ${mid}`).values()[0]?.[0];
  rdb.close();
  check(keptRow === "kept", "a resumed refill writes nothing up to its checkpoint");
  const tc = tables(join(cut, "store.db")), tw = tables(join(whole, "store.db"));
  const past = (rows: string[]) => rows.filter((r) => Number(JSON.parse(r)[0]) > mid);
  check(JSON.stringify(past(tc["far_line"] ?? [])) === JSON.stringify(past(tw["far_line"] ?? [])), "and everything past it as a whole build has it");
} finally {
  if (process.env.KEEP) console.log(root); else rmSync(root, { recursive: true, force: true });
}
console.log(fail ? `${fail} failed` : "all ok");
process.exit(fail ? 1 : 0);
