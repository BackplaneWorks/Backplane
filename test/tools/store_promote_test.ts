// The store's build and promotion on synthetic homes. macOS's SQLite
// keeps a database's -wal and -shm after its connection closes
// (persistent WAL), so the rebuild folder still held them, rmdir failed
// with ENOTEMPTY and a hub with no store.db never started there.
// storeio.js's store_promote now moves them with the database. They must
// not be dropped instead: store_validate opens the store read-only at
// every boot, and on macOS a read-only connection cannot open a WAL
// database whose -wal and -shm are gone (the store would be set aside and
// rebuilt from the log at every start).
//   build        a store built through db.js in a rebuild folder is
//                promoted whole: the folder gone, every row there, and it
//                opens read-only as store_validate opens it
//   side files   the build's -wal and -shm move with its database; the old
//                database and its WAL are kept, the rebuild folder goes
//   none         without side files (Linux) nothing else changes
//   wal rows     rows committed only to the WAL are still there afterwards
//
//   bun test/tools/store_promote_test.ts
import { Database } from "bun:sqlite";
import * as fs from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const source = fs.readFileSync("src/server/effects/storeio.js", "utf8").replace(/CID\(([^)]+)\)/g, '"$1"');
const effects = new Map<string, Function>();
new Function("require", "io_eff", "CID", source)(require, (id: string, f: Function) => effects.set(id, f), (n: unknown) => n);
const temp = effects.get("Disk.temp")!, promote = effects.get("Disk.promote")!;
const dbsource = fs.readFileSync("src/server/effects/db.js", "utf8").replace(/CID\(([^)]+)\)/g, '"$1"');
new Function("require", "io_eff", "CID", dbsource)(require, (id: string, f: Function) => effects.set(id, f), (n: unknown) => n);
const open = effects.get("Db.open")!, exec = effects.get("Db.exec")!, close = effects.get("Db.close")!;
// a database built the hub's way: opened, written and closed through db.js
const build = (file: string, rows: number) => {
  const o = String(open(file)), h = o.slice(3);
  if (!o.startsWith("ok ")) return o;
  const steps = [exec(h, "CREATE TABLE line(seq INTEGER)"), exec(h, "BEGIN"), ...Array.from({ length: rows }, (_, i) => exec(h, `INSERT INTO line VALUES (${i + 1})`)), exec(h, "COMMIT"), close(h)];
  return steps.find((r) => r !== "ok") ?? "ok";
};
const rows = (file: string, readonly = false) => { const d = new Database(file, { readonly }); try { return (d.query("SELECT count(*) FROM line").values()[0] as number[])[0]; } finally { d.close(); } };

let bad = 0;
const ok = (v: boolean, s: string) => { console.log(`${v ? "ok  " : "FAIL"} ${s}`); if (!v) bad++; };
const read = (p: string) => (fs.existsSync(p) ? fs.readFileSync(p, "utf8") : "(none)");
const home = () => { const h = fs.mkdtempSync(join(tmpdir(), "bp-promote-")); fs.writeFileSync(join(h, "events.jsonl"), "{}\n"); return h; };
const homes: string[] = [];

try {
  // the hub's way end to end: built in a rebuild folder, promoted whole
  {
    const h = home(); homes.push(h);
    const t = temp(h);
    const b = build(join(t, "store.db"), 30);
    const r = promote(h, t);
    ok(b === "ok" && r === "ok" && !fs.existsSync(t), `build: built and promoted, the folder gone (${b}, ${r})`);
    let n: unknown;
    try { n = rows(join(h, "store.db"), true); } catch (e: any) { n = e.message; }
    ok(n === 30, `build: every row, read-only as store_validate reads it (${n}; ${fs.readdirSync(h).join(" ")})`);
  }
  // the shape macOS leaves: the build's side files, and an old store with its own
  {
    const h = home(); homes.push(h);
    for (const [s, v] of [["", "OLD-DB"], ["-wal", "OLD-WAL"], ["-shm", "OLD-SHM"]]) fs.writeFileSync(join(h, "store.db" + s), v);
    const t = temp(h);
    for (const [s, v] of [["", "NEW-DB"], ["-wal", "NEW-WAL"], ["-shm", "NEW-SHM"]]) fs.writeFileSync(join(t, "store.db" + s), v);
    const r = promote(h, t);
    ok(r === "ok", `side files: promoted (${r})`);
    ok(!fs.existsSync(t), "side files: the rebuild folder is gone");
    ok(read(join(h, "store.db")) === "NEW-DB" && read(join(h, "store.db-wal")) === "NEW-WAL" && read(join(h, "store.db-shm")) === "NEW-SHM",
      "side files: the new database's WAL and shared memory moved with it");
    const kept = fs.readdirSync(h).filter((f) => f.startsWith("store.db.previous-"));
    const base = kept.find((f) => /^store\.db\.previous-\d+$/.test(f)) ?? "";
    ok(read(join(h, base)) === "OLD-DB" && read(join(h, base + "-wal.retired")) === "OLD-WAL", `side files: the old database and its WAL are kept (${kept.join(", ")})`);
  }
  // the shape Linux leaves: no side files
  {
    const h = home(); homes.push(h);
    const t = temp(h);
    fs.writeFileSync(join(t, "store.db"), "NEW-DB");
    const r = promote(h, t);
    ok(r === "ok" && !fs.existsSync(t) && read(join(h, "store.db")) === "NEW-DB", `none: promoted, the folder gone (${r})`);
    ok(!fs.existsSync(join(h, "store.db-wal")) && !fs.existsSync(join(h, "store.db-shm")), "none: no side files appear");
  }
  // committed rows that are only in the WAL survive the move
  {
    const h = home(); homes.push(h);
    const scratch = fs.mkdtempSync(join(tmpdir(), "bp-promote-db-")); homes.push(scratch);
    const db = new Database(join(scratch, "store.db"));
    db.run("PRAGMA journal_mode=WAL"); db.run("PRAGMA wal_autocheckpoint=0");
    db.run("CREATE TABLE line(seq INTEGER)");
    db.run("PRAGMA wal_checkpoint(TRUNCATE)");
    for (let i = 1; i <= 50; i++) db.run("INSERT INTO line VALUES (?)", [i]);
    const t = temp(h);
    // the database and its WAL as a build that stopped before a checkpoint
    // leaves them: the rows are in the WAL alone
    fs.copyFileSync(join(scratch, "store.db"), join(t, "store.db"));
    fs.copyFileSync(join(scratch, "store.db-wal"), join(t, "store.db-wal"));
    db.close();
    const r = promote(h, t);
    ok(r === "ok" && !fs.existsSync(t), `wal rows: promoted (${r})`);
    let n: unknown = "(not promoted)";
    if (r === "ok") {
      const after = new Database(join(h, "store.db"), { readonly: true });
      n = (after.query("SELECT count(*) FROM line").values()[0] as number[])[0];
      after.close();
    }
    ok(n === 50, `wal rows: the 50 rows committed to the WAL are in the promoted store (${n})`);
  }
} finally {
  for (const h of homes) fs.rmSync(h, { recursive: true, force: true });
}
process.exit(bad ? 1 : 0);
