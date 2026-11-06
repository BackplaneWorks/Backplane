// db.js's read cache (Db.query / Db.query.t answers kept until this process writes what they read): every answer
// equals a fresh read of the database, every write path and open/close drops what was kept that names the written
// table or what derives from it (views, triggers, FTS content, references), a write it cannot name drops everything,
// failures and impure SQL are never kept.
//   bun test/tools/db_read_cache_test.ts
import { readFileSync, mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
const src = readFileSync(join(import.meta.dir, "../../src/server/effects/db.js"), "utf8")
  .replace(/CID\(Db\.([A-Za-z_.]+)\)/g, (_, n) => JSON.stringify("Db." + n))
  .replace(/CID\((DbText|DbNull|DbInt|HistoryPage)\)/g, (_, n) => JSON.stringify("db." + n))
  .replace(/CID\((Con|Nil)\)/g, (_, n) => JSON.stringify(n));
const eff: Record<string, Function> = {};
new Function("io_eff", "require", src)((k: string, f: Function) => { eff[k] = f; }, require);
const L = (xs: any[]) => xs.reduceRight((t, x) => ({ $: "Con", head: x, tail: t }), { $: "Nil" });
const A = (l: any): any[] => { const r = []; for (; l.$ === "Con"; l = l.tail) r.push(l.head.$ === "Con" || l.head.$ === "Nil" ? A(l.head) : l.head); return r; };
const T = (s: string) => ({ $: "db.DbText", text: s });
const dir = mkdtempSync(join(tmpdir(), "bp-db-cache-"));
let fail = 0; const ok = (c: boolean, what: string) => { if (!c) { fail++; console.log("FAIL " + what); } };
try {
  const h = eff["Db.open"](join(dir, "s.db")).replace(/^ok /, "");
  eff["Db.exec"](h, "CREATE TABLE e(thread TEXT, n INTEGER)");
  const q = (sql = "SELECT n FROM e WHERE thread = ?1 ORDER BY n", p = [T("a")]) => eff["Db.query.t"](h, sql, L(p), 100);
  const fresh = () => { eff["Db.close"](h); };
  ok(A(q()).length === 0, "empty");
  const r1 = q(), r2 = q(); ok(r1 === r2, "a repeated read is kept");
  const writes: [string, () => void][] = [
    ["exec", () => eff["Db.exec"](h, "INSERT INTO e VALUES ('a', 1)")],
    ["run", () => eff["Db.run"](h, "INSERT INTO e VALUES (?1, ?2)", L(["a", "2"]))],
    ["run.t", () => eff["Db.run.t"](h, "INSERT INTO e VALUES (?1, 3)", L([T("a")]))],
    ["batch", () => eff["Db.batch"](h, "INSERT INTO e VALUES (?1, ?2)", L([L(["a", "4"])]))],
    ["batch.t", () => eff["Db.batch.t"](h, "INSERT INTO e VALUES (?1, 5)", L([L([T("a")])]))],
    ["script", () => eff["Db.script"](h, L([L(["INSERT INTO e VALUES ('a', 6)"])]))],
    ["script.t", () => eff["Db.script.t"](h, L([{ sql: "INSERT INTO e VALUES ('a', 7)", params: L([]) }]))],
  ];
  let n = 0;
  for (const [name, w] of writes) { const before = q(); w(); n++; const after = q(); ok(after !== before && A(after).length === n, name + " drops kept reads (" + A(after).length + " rows)"); }
  ok(A(q(undefined, [T("b")])).length === 0, "other params are their own read");
  const bad = eff["Db.query.t"](h, "SELECT nope FROM e", L([]), 10), bad2 = eff["Db.query.t"](h, "SELECT nope FROM e", L([]), 10);
  ok(bad !== bad2, "a failure is never kept");
  const rnd = eff["Db.query.t"](h, "SELECT random()", L([]), 1), rnd2 = eff["Db.query.t"](h, "SELECT random()", L([]), 1);
  ok(rnd !== rnd2, "random() is never kept");
  const now = eff["Db.query.t"](h, "SELECT datetime('now')", L([]), 1), now2 = eff["Db.query.t"](h, "SELECT datetime('now')", L([]), 1);
  ok(now !== now2, "'now' is never kept");
  const lim1 = eff["Db.query.t"](h, "SELECT n FROM e WHERE thread = ?1 ORDER BY n", L([T("a")]), 1);
  ok(A(lim1).length === 1, "the limit is part of the read");
  // writes to other tables keep an answer; to its table, or what derives from it, drop it
  let fr = 1000; const fresh1 = (sql: string) => JSON.stringify(A(eff["Db.query.t"](h, sql, L([]), fr++)));
  const kept = (sql: string) => eff["Db.query.t"](h, sql, L([]), 100);
  const same = (sql: string, w: () => void, keep: boolean, what: string) => {
    const before = kept(sql); w(); const after = kept(sql);
    ok((after === before) === keep, what + (keep ? " keeps" : " drops") + " the answer of " + sql);
    ok(JSON.stringify(A(after)) === fresh1(sql), what + ": the answer is what the database holds");
  };
  eff["Db.exec"](h, "CREATE TABLE f(a TEXT); CREATE TABLE g(a TEXT); CREATE VIEW ve AS SELECT n FROM e WHERE n > 2;" +
    "CREATE TABLE pa(id TEXT PRIMARY KEY); CREATE TABLE ch(id TEXT, p TEXT REFERENCES pa(id) ON DELETE CASCADE);" +
    "CREATE TRIGGER f_to_g AFTER INSERT ON f BEGIN INSERT INTO g VALUES (new.a); END;" +
    "CREATE VIRTUAL TABLE ef USING fts5(thread, content='e', content_rowid='n')");
  const E = "SELECT count(*) FROM e", G = "SELECT count(*) FROM g", V = "SELECT count(*) FROM ve", C = "SELECT count(*) FROM ch", F = "SELECT count(*) FROM ef";
  same(E, () => eff["Db.run"](h, "INSERT INTO f VALUES (?1)", L(["x"])), true, "run into f");
  same(E, () => eff["Db.batch.t"](h, "INSERT OR REPLACE INTO main.g VALUES (?1)", L([L([T("y")])])), true, "batch into main.g");
  same(E, () => eff["Db.script"](h, L([L(["INSERT INTO f VALUES ('s')"]), L(["UPDATE g SET a = 'z' WHERE a = 'y'"]), L(["DELETE FROM pa WHERE id = 'none'"])])), true, "script into f, g, pa");
  same(E, () => eff["Db.run"](h, "INSERT INTO e VALUES ('c', 9)", L([])), false, "run into e");
  same(G, () => eff["Db.run"](h, "INSERT INTO f VALUES ('t')", L([])), false, "f's trigger writes g: run into f");
  same(V, () => eff["Db.run"](h, "INSERT INTO e VALUES ('c', 10)", L([])), false, "a view over e: run into e");
  same(V, () => eff["Db.run"](h, "INSERT INTO g VALUES ('q')", L([])), true, "a view over e: run into g");
  eff["Db.run"](h, "INSERT INTO pa VALUES ('p1')", L([])); eff["Db.run"](h, "INSERT INTO ch VALUES ('c1', 'p1')", L([]));
  same(C, () => eff["Db.run"](h, "DELETE FROM pa WHERE id = 'p1'", L([])), false, "ch references pa: delete from pa");
  same(F, () => eff["Db.run"](h, "INSERT INTO e VALUES ('d', 11)", L([])), false, "ef's content is e: run into e");
  same(E, () => eff["Db.run"](h, "WITH x AS (SELECT 1) INSERT INTO f SELECT 'w' FROM x", L([])), false, "a CTE-led write (not named)");
  same(E, () => eff["Db.run"](h, "/* c */ INSERT INTO f VALUES ('v')", L([])), false, "a write led by a comment (not named)");
  same(E, () => eff["Db.exec"](h, "INSERT INTO f VALUES ('u')"), false, "exec");
  // a trigger made after answers were kept (DDL through run) is seen by the next write
  kept(E);
  eff["Db.run"](h, "CREATE TRIGGER g_to_e AFTER INSERT ON g BEGIN INSERT INTO e VALUES ('trig', 99); END", L([]));
  same(E, () => eff["Db.run"](h, "INSERT INTO g VALUES ('fires')", L([])), false, "g's new trigger writes e: run into g");
  same(E, () => eff["Db.batch"](h, "INSERT INTO f VALUES (?1)", L([L(["a"]), L(["b"])])), false, "f -> g -> e by triggers: a batch into f");
  same(E, () => eff["Db.batch"](h, "INSERT INTO pa VALUES (?1)", L([L(["a"]), L(["b"])])), true, "a batch into pa");
  const fb = kept(E); eff["Db.script"](h, L([L(["INSERT INTO pa VALUES ('ok')"]), L(["INSERT INTO nope VALUES (1)"])])); ok(kept(E) === fb, "a failed batch into pa/nope keeps e's answer");
  ok(JSON.stringify(A(kept(E))) === fresh1(E), "and it is still right");
  const k = q(); eff["Db.close"](h); const h2 = eff["Db.open"](join(dir, "s.db")).replace(/^ok /, "");
  const k2 = eff["Db.query.t"](h2, "SELECT n FROM e WHERE thread = ?1 ORDER BY n", L([T("a")]), 100);
  ok(k2 !== k && JSON.stringify(A(k2)) === JSON.stringify(A(k)), "close/open drops kept reads; the answer is the same");
  const hot = eff["Db.query.t"](h2, "SELECT ?1", L([T("hot")]), 1);
  for (let i = 0; i < 300; i++) { eff["Db.query.t"](h2, "SELECT ?1", L([T(String(i))]), 1); ok(eff["Db.query.t"](h2, "SELECT ?1", L([T("hot")]), 1) === hot, "an answer used again stays over the cap (" + i + ")"); }
  const old0 = A(eff["Db.query.t"](h2, "SELECT ?1", L([T("5")]), 1)); ok(old0[0][0] === "5", "one dropped for the cap reads again, right");
  ok(A(eff["Db.query.t"](h2, "SELECT ?1", L([T("299")]), 1))[0][0] === "299", "bounded and still right");
  const big = "x".repeat(17 << 20), b1 = eff["Db.query.t"](h2, "SELECT ?1", L([T(big)]), 1), b2 = eff["Db.query.t"](h2, "SELECT ?1", L([T(big)]), 1);
  ok(b1 !== b2 && A(b2)[0][0] === big, "an answer over 32 MiB is not kept");
  const hb = eff["Db.query.t"](h2, "SELECT ?1 || 'h'", L([T("y".repeat(1 << 20))]), 1);
  for (let i = 0; i < 40; i++) { eff["Db.query.t"](h2, "SELECT ?1 || " + i, L([T("z".repeat(1 << 20))]), 1); ok(eff["Db.query.t"](h2, "SELECT ?1 || 'h'", L([T("y".repeat(1 << 20))]), 1) === hb, "a big answer used again stays over the 64 MiB cap (" + i + ")"); }
  const s1 = eff["Db.query.t"](h2, "SELECT ?1", L([T("small")]), 1), s2 = eff["Db.query.t"](h2, "SELECT ?1", L([T("small")]), 1);
  ok(s1 === s2, "a small one still is");
  // reads about one thread (every use "FROM t WHERE thread = ?1 AND/end"): rows of other threads keep them
  eff["Db.exec"](h2, "CREATE TABLE en(seq INTEGER PRIMARY KEY, thread TEXT, n INTEGER); CREATE TABLE ci(thread TEXT COLLATE NOCASE, n INTEGER);" +
    "CREATE TABLE sw(thread TEXT, n INTEGER); CREATE TRIGGER sw_more AFTER INSERT ON sw WHEN new.n < 5 BEGIN INSERT INTO sw VALUES ('a', new.n + 10); END");
  let fr2 = 5000; const fresh2 = (sql: string, v: string) => JSON.stringify(A(eff["Db.query.t"](h2, sql, L([T(v)]), fr2++)));
  const rd = (sql: string, v: string) => eff["Db.query.t"](h2, sql, L([T(v)]), 100);
  const keeps = (sql: string, v: string, w: () => void, keep: boolean, what: string) => {
    const before = rd(sql, v); w(); const after = rd(sql, v);
    ok((after === before) === keep, what + (keep ? " keeps" : " drops") + " [" + v + "] " + sql);
    ok(JSON.stringify(A(after)) === fresh2(sql, v), what + ": the answer is what the database holds");
  };
  const S1 = "SELECT n FROM en WHERE thread = ?1 ORDER BY n";
  const S2 = "WITH a AS (SELECT seq FROM en WHERE thread = ?1 AND n > 0 ORDER BY seq DESC LIMIT 5), b AS (SELECT seq FROM en WHERE thread=?1 AND n IN (SELECT n FROM en WHERE thread = ?1 AND (n = 1 OR n = 2))) SELECT count(*) FROM en WHERE thread = ?1 AND seq IN (SELECT seq FROM a UNION SELECT seq FROM b)";
  const ins = "INSERT INTO en(seq, thread, n) VALUES (substr(?, 2), substr(?, 2), substr(?, 2)) ON CONFLICT DO NOTHING";
  let seq = 1; const row = (th: string, n = 1) => L(["v" + seq++, "v" + th, "v" + n]);
  eff["Db.batch"](h2, ins, L([row("a"), row("b")]));
  keeps(S1, "a", () => eff["Db.batch"](h2, ins, L([row("b"), row("c", 2)])), true, "a batch of other threads' rows");
  keeps(S2, "a", () => eff["Db.batch"](h2, ins, L([row("b", 2)])), true, "(the CTE read) another thread's row");
  keeps(S1, "a", () => eff["Db.batch"](h2, ins, L([row("b"), row("a", 3)])), false, "a batch with one row of its thread");
  keeps(S2, "a", () => eff["Db.batch"](h2, ins, L([row("a", 2)])), false, "(the CTE read) a row of its thread");
  keeps(S1, "a", () => eff["Db.run"](h2, "INSERT INTO en(seq, thread, n) VALUES (?, ?, ?)", L([String(seq++), "b", "4"])), true, "a plain-mark insert of another thread");
  keeps(S1, "a", () => eff["Db.run.t"](h2, "INSERT INTO en(seq, thread, n) VALUES (?, ?, ?)", L([{ $: "db.DbInt", digits: String(seq++) }, T("b"), { $: "db.DbInt", digits: "4" }])), true, "a typed insert of another thread");
  keeps(S1, "a", () => eff["Db.script"](h2, L([L([ins, "v" + seq++, "vz", "v1"])])), true, "a script insert of another thread");
  keeps(S1, "a", () => eff["Db.run"](h2, "INSERT OR REPLACE INTO en(seq, thread, n) VALUES (?, ?, ?)", L([String(seq++), "b", "5"])), false, "INSERT OR REPLACE");
  keeps(S1, "a", () => eff["Db.run"](h2, "UPDATE en SET n = n + 1 WHERE thread = 'b'", L([])), false, "an UPDATE");
  keeps(S1, "a", () => eff["Db.run"](h2, "DELETE FROM en WHERE thread = 'b'", L([])), false, "a DELETE");
  keeps("SELECT n FROM en WHERE thread = ?1 OR n > 100 ORDER BY n", "a", () => eff["Db.batch"](h2, ins, L([row("b", 200)])), false, "OR beside it");
  keeps("SELECT n FROM en WHERE thread = ?1 COLLATE NOCASE ORDER BY n", "a", () => eff["Db.batch"](h2, ins, L([row("A", 6)])), false, "COLLATE on it");
  keeps("SELECT n FROM en WHERE thread = ?1 AND n <= (SELECT max(n) FROM en) ORDER BY n", "a", () => eff["Db.batch"](h2, ins, L([row("b", 300)])), false, "the table used again");
  keeps("SELECT n FROM en WHERE n > 0 AND thread = ?1 ORDER BY n", "a", () => eff["Db.batch"](h2, ins, L([row("b", 7)])), false, "not first in its WHERE (not known)");
  eff["Db.run"](h2, "INSERT INTO ci VALUES ('b', 1)", L([]));
  keeps("SELECT n FROM ci WHERE thread = ?1", "b", () => eff["Db.run"](h2, "INSERT INTO ci(thread, n) VALUES (?, ?)", L(["B", "2"])), false, "a column with a collation");
  eff["Db.run"](h2, "INSERT INTO sw VALUES ('a', 100)", L([]));
  keeps("SELECT n FROM sw WHERE thread = ?1 ORDER BY n", "a", () => eff["Db.run"](h2, "INSERT INTO sw(thread, n) VALUES (?, ?)", L(["b", "1"])), false, "a table whose trigger writes it");
  eff["Db.exec"](h2, "CREATE VIEW env AS SELECT n - 1000 AS n FROM en WHERE n > 1000");
  keeps("SELECT n FROM en WHERE thread = ?1 AND n NOT IN (SELECT n FROM env) ORDER BY n", "a", () => eff["Db.batch"](h2, ins, L([row("b", 1001)])), false, "a view over the table beside it");
  keeps(S1, "a", () => eff["Db.batch"](h2, ins, L([row("b", 8)])), true, "and after all that, another thread's row again");
  // a read's tables are where tables go; a column that shares a table's name is not one
  eff["Db.exec"](h2, "CREATE TABLE thread(id TEXT, x INTEGER); CREATE TABLE ta(n INTEGER); CREATE TABLE tb(n INTEGER); CREATE TABLE tc(n INTEGER); CREATE TABLE \"td q\"(n INTEGER)");
  eff["Db.exec"](h2, "CREATE INDEX en_th ON en(thread)");
  keeps(S1, "a", () => eff["Db.run"](h2, "INSERT OR REPLACE INTO thread(id, x) VALUES (?, ?)", L(["a", "1"])), true, "a write to the table named like its column");
  const tw = (t: string) => () => eff["Db.run"](h2, "INSERT INTO " + t + " VALUES (1)", L([]));
  for (const [sql, t] of [
    ["SELECT count(*) FROM ta, tb WHERE ta.n = tb.n AND ?1 <> ''", "tb"],
    ["SELECT count(*) FROM ta AS x, tb y, tc WHERE ?1 <> ''", "tc"],
    ["SELECT count(*) FROM ta LEFT JOIN tb ON ta.n = tb.n WHERE ?1 <> ''", "tb"],
    ["SELECT count(*) FROM ta WHERE n IN tc AND ?1 <> ''", "tc"],
    ["SELECT count(*) FROM (SELECT n FROM ta) s, tb WHERE ?1 <> ''", "tb"],
    ["SELECT count(*) FROM (SELECT n FROM ta) s, tb WHERE ?1 <> ''", "ta"],
    ["SELECT count(*) FROM main.tc WHERE ?1 <> ''", "tc"],
    ["SELECT count(*) FROM en INDEXED BY en_th, tb WHERE ?1 <> ''", "tb"],
    ["SELECT count(*) FROM en AS e NOT INDEXED, tc WHERE ?1 <> ''", "tc"],
    ["SELECT count(*) FROM \"td q\" WHERE ?1 <> ''", "\"td q\""],
    ["SELECT (SELECT count(*) FROM tb WHERE n > 0) + (SELECT count(*) FROM tc) WHERE ?1 <> ''", "tc"],
  ]) keeps(sql, "a", tw(t), false, "a write to " + t);
  keeps("SELECT count(*) FROM ta, tb WHERE ta.n = tb.n AND ?1 <> ''", "a", tw("tc"), true, "a write to a table it does not read");
  // the review history's pages: kept per thread like a scoped read
  eff["Db.exec"](h2, "CREATE TABLE entry(seq INTEGER PRIMARY KEY, ord INTEGER UNIQUE, id TEXT, thread TEXT, src TEXT, kind TEXT, role TEXT, akind TEXT, text TEXT, at INTEGER); CREATE INDEX entry_thread ON entry(thread, seq)");
  const eins = "INSERT INTO entry(seq, ord, id, thread, src, kind, role, akind, text, at) VALUES (substr(?, 2), substr(?, 2), substr(?, 2), substr(?, 2), substr(?, 2), substr(?, 2), substr(?, 2), substr(?, 2), substr(?, 2), substr(?, 2)) ON CONFLICT DO NOTHING";
  let es = 1; const erow = (th: string) => { const n = es++; return L(["v" + n, "v" + n, "vi" + n, "v" + th, "v", "vact", "vreview", "v", "v{}", "v1"]); };
  eff["Db.batch"](h2, eins, L([erow("ra"), erow("rb")]));
  const budget = { rows: 128, bytes: 1048576 };
  const page = (th: string) => eff["Db.review_page"](h2, th, 0n, 1000n, budget);
  const pa = page("ra"); ok(page("ra") === pa && A(pa.rows).length === 1, "a review page is kept");
  eff["Db.batch"](h2, eins, L([erow("rb")])); ok(page("ra") === pa, "another thread's entry keeps it");
  eff["Db.batch"](h2, eins, L([erow("ra")])); const pa2 = page("ra"); ok(pa2 !== pa && A(pa2.rows).length === 2, "its thread's entry drops it; the new page has the row");
  eff["Db.run"](h2, "UPDATE entry SET text = '{\"x\":1}' WHERE thread = 'rb'", L([])); ok(page("ra") !== pa2, "an UPDATE of entry drops it");
} finally { rmSync(dir, { recursive: true, force: true }); }
console.log(fail ? fail + " failed" : "db_read_cache: ok"); process.exit(fail ? 1 : 0);
