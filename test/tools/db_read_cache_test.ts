// db.js's read cache (Db.query / Db.query.t answers kept until this process writes): every answer equals a fresh
// read of the database, every write path and open/close drops what was kept, failures and impure SQL are never kept.
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
  const k = q(); eff["Db.close"](h); const h2 = eff["Db.open"](join(dir, "s.db")).replace(/^ok /, "");
  const k2 = eff["Db.query.t"](h2, "SELECT n FROM e WHERE thread = ?1 ORDER BY n", L([T("a")]), 100);
  ok(k2 !== k && JSON.stringify(A(k2)) === JSON.stringify(A(k)), "close/open drops kept reads; the answer is the same");
  for (let i = 0; i < 300; i++) eff["Db.query.t"](h2, "SELECT ?1", L([T(String(i))]), 1);
  ok(A(eff["Db.query.t"](h2, "SELECT ?1", L([T("299")]), 1))[0][0] === "299", "bounded and still right");
  const big = "x".repeat(600000), b1 = eff["Db.query.t"](h2, "SELECT ?1", L([T(big)]), 1), b2 = eff["Db.query.t"](h2, "SELECT ?1", L([T(big)]), 1);
  ok(b1 !== b2 && A(b2)[0][0] === big, "an answer over 1 MiB is not kept");
  const s1 = eff["Db.query.t"](h2, "SELECT ?1", L([T("small")]), 1), s2 = eff["Db.query.t"](h2, "SELECT ?1", L([T("small")]), 1);
  ok(s1 === s2, "a small one still is");
} finally { rmSync(dir, { recursive: true, force: true }); }
console.log(fail ? fail + " failed" : "db_read_cache: ok"); process.exit(fail ? 1 : 0);
