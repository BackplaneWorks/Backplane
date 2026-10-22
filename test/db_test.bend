import Base
import ./check.bend as C
import ../src/server/db.bend as D

# The Db effect (src/server/db.bend over bun:sqlite), run by scripts/test.sh.
# Files go in a fresh mktemp directory, removed at the end. Throughput:
# test/native/db_bench.bend.

# the handle in an "ok <handle>" answer
def hd(a: String) -> String:
  String.drop(a, 3n)

def Esc.put.k(z: Bool, c: Char, acc: String) -> String:
  match z:
    case True{}: SCon{'@', SCon{'^', acc}}
    case False{}: SCon{c, acc}

def Esc.put(c: Char, acc: String) -> String:
  match c:
    case Chr{+x}: Esc.put.k(U32.is_eq(x, 0), Chr{x}, acc)

# a cell with its NULs shown as ^@
def Esc.go(s: String, acc: String) -> String:
  match s:
    case SNil{}: String.reverse(acc)
    case SCon{c, t}: Esc.go(t, Esc.put(c, acc))

def Show.esc(xs: List<&2, String>) -> List<&2, String>:
  match xs:
    case Nil{}: Nil{}
    case h <> t: Esc.go(h, "") <> Show.esc(t)

# rows as "a|b;c|d;"
def Show.rows(rs: List<&2, List<&2, String>>) -> String:
  match rs:
    case Nil{}: ""
    case r <> t: String.join(Show.esc(r), "|") ++ ";" ++ Show.rows(t)

def count(rs: List<&2, List<&2, String>>) -> String:
  Nat.show(List.length(&2, List<&2, String>, rs))

# n rows [k, "row k", k*2] for k < n, in order
def rows.go(+n: Nat, acc: List<&2, List<&2, D.DbVal>>) -> List<&2, List<&2, D.DbVal>>:
  match n:
    case 0n: acc
    case 1n+k: rows.go(k, [D.Db.nat(k), D.DbText{"row " ++ Nat.show(k)}, D.Db.nat(Nat.mul(k, 2n))] <> acc)

# n point lookups by id, answering how many found their row
def lookups(+n: Nat, +h: String, found: Nat) -> IO(Nat):
  match n:
    case 0n: IO.pure(Nat, found)
    case 1n+k:
      do IO<Nat>:
        r : List<&2, List<&2, String>> <- D.Db.query.t(h, "SELECT name FROM big WHERE id = ?", [D.Db.nat(Nat.mul(k, 7n))], 1)
        lookups(k, h, Nat.add(found, List.length(&2, List<&2, String>, r)))

def dir.of(r: Result<&1, &1, U32 & String, U32 & (String & String)>) -> IO(String):
  match r:
    case Done{(_, (out, _))}: IO.pure(String, String.trim(out))
    case Fail{(_, e)}: IO.die(String, 1, "db_test: mktemp failed: " ++ e)

# Counted and status-only scripts must produce the same rows and FTS
# contents, while leaving count-returning callers and rollback intact.
def Status.check(+h: String) -> IO(Unit):
  do IO<Unit>:
    ddl : String <- D.Db.exec(h, "CREATE TABLE counted_entry(id INTEGER PRIMARY KEY, text TEXT); CREATE VIRTUAL TABLE counted_fts USING fts5(text, content='counted_entry', content_rowid='id'); CREATE TRIGGER counted_ai AFTER INSERT ON counted_entry BEGIN INSERT INTO counted_fts(rowid,text) VALUES(new.id,new.text); END; CREATE TABLE status_entry(id INTEGER PRIMARY KEY, text TEXT); CREATE VIRTUAL TABLE status_fts USING fts5(text, content='status_entry', content_rowid='id'); CREATE TRIGGER status_ai AFTER INSERT ON status_entry BEGIN INSERT INTO status_fts(rowid,text) VALUES(new.id,new.text); END")
    C.Check.str("status fixture schema", ddl, "ok")
    counted : String <- D.Db.script(h, [["INSERT INTO counted_entry VALUES (?, ?)", "1", "日本語 routed #42"], ["INSERT INTO counted_entry VALUES (?, ?)", "2", "héllo routed"], ["INSERT INTO counted_entry VALUES (?, ?) ON CONFLICT DO NOTHING", "2", "ignored"]])
    status : String <- D.Db.script_status(h, [["INSERT INTO status_entry VALUES (?, ?)", "1", "日本語 routed #42"], ["INSERT INTO status_entry VALUES (?, ?)", "2", "héllo routed"], ["INSERT INTO status_entry VALUES (?, ?) ON CONFLICT DO NOTHING", "2", "ignored"]])
    C.Check.str("counted script retains trigger-excluding count", counted, "ok 2")
    C.Check.str("status script returns only status", status, "ok")
    +cr : List<&2, List<&2, String>> <- D.Db.query(h, "SELECT id,text FROM counted_entry ORDER BY id", [], 10)
    +sr : List<&2, List<&2, String>> <- D.Db.query(h, "SELECT id,text FROM status_entry ORDER BY id", [], 10)
    C.Check.str("status rows equal counted script", Show.rows(sr), Show.rows(cr))
    C.Check.str("status keeps Unicode and hash text parameters", Show.rows(cr), "1|日本語 routed #42;2|héllo routed;")
    cf : List<&2, List<&2, String>> <- D.Db.query(h, "SELECT rowid,text FROM counted_fts WHERE counted_fts MATCH 'routed' ORDER BY rowid", [], 10)
    +sf : List<&2, List<&2, String>> <- D.Db.query(h, "SELECT rowid,text FROM status_fts WHERE status_fts MATCH 'routed' ORDER BY rowid", [], 10)
    C.Check.str("status FTS rows equal counted script", Show.rows(sf), Show.rows(cf))
    C.Check.str("status FTS contains both real rows", Show.rows(sf), "1|日本語 routed #42;2|héllo routed;")
    noops : String <- D.Db.script_status(h, [["INSERT INTO status_entry VALUES (2, 'ignored again') ON CONFLICT DO NOTHING"], ["UPDATE status_entry SET text='absent' WHERE id=90"]])
    empty : String <- D.Db.script_status(h, [])
    C.Check.str("status duplicate and no-op writes succeed", noops, "ok")
    C.Check.str("status empty script succeeds", empty, "ok")
    begin : String <- D.Db.exec(h, "BEGIN; INSERT INTO status_entry VALUES(99, 'outer sentinel')")
    nested : String <- D.Db.script_status(h, [["INSERT INTO status_entry VALUES(100, 'inner rolledback')"], ["INSERT INTO status_entry VALUES(99, 'clash')"]])
    C.Check.str("status nested script fails identically", nested, "fail UNIQUE constraint failed: status_entry.id")
    +inside : List<&2, List<&2, String>> <- D.Db.query(h, "SELECT id,text FROM status_entry WHERE id>=99 ORDER BY id", [], 10)
    C.Check.str("status rollback preserves caller transaction", Show.rows(inside), "99|outer sentinel;")
    innerfts : List<&2, List<&2, String>> <- D.Db.query(h, "SELECT rowid FROM status_fts WHERE status_fts MATCH 'inner'", [], 10)
    C.Check.str("status rollback also removes FTS trigger rows", count(innerfts), "0")
    rollback : String <- D.Db.exec(h, "ROLLBACK")
    outer : List<&2, List<&2, String>> <- D.Db.query(h, "SELECT id FROM status_entry WHERE id>=99", [], 10)
    C.Check.str("caller rollback remains effective after status failure", begin ++ " " ++ rollback ++ " " ++ count(outer), "ok ok 0")
    badparams : String <- D.Db.script_status(h, [["INSERT INTO status_entry VALUES(101, 'parameter rollback')"], ["INSERT INTO status_entry VALUES (?, ?)", "102"]])
    C.Check.str("status validates exact parameter count", badparams, "fail 1 params for 2 placeholders")
    paramsrows : List<&2, List<&2, String>> <- D.Db.query(h, "SELECT id FROM status_entry WHERE id>=99", [], 10)
    paramsfts : List<&2, List<&2, String>> <- D.Db.query(h, "SELECT rowid FROM status_fts WHERE status_fts MATCH 'parameter'", [], 10)
    C.Check.str("parameter failure rolls back rows and FTS", count(paramsrows) ++ " " ++ count(paramsfts), "0 0")
    baditem : String <- D.Db.script_status(h, [["INSERT INTO status_entry VALUES(103, 'empty rollback')"], []])
    badsql : String <- D.Db.script_status(h, [["INSERT INTO status_entry VALUES(104, 'sql rollback')"], ["INSERT INTO no_status_table VALUES(1)"]])
    blank : String <- D.Db.script_status(h, [["  "]])
    many : String <- D.Db.script_status(h, [["SELECT ?", "one", "two"]])
    C.Check.str("status rejects empty item", baditem, "fail an empty item")
    C.Check.str("status preserves SQL error", badsql, "fail no such table: no_status_table")
    C.Check.str("status rejects empty statement", blank, "fail no statement")
    C.Check.str("status rejects extra parameters", many, "fail 2 params for 1 placeholders")
    finalrows : List<&2, List<&2, String>> <- D.Db.query(h, "SELECT id FROM status_entry WHERE id>=99", [], 10)
    C.Check.str("status failures leave no partial rows", count(finalrows), "0")
    integrity : String <- D.Db.exec(h, "INSERT INTO status_fts(status_fts,rank) VALUES('integrity-check',1)")
    C.Check.str("status FTS remains consistent with external content", integrity, "ok")
    finalcount : String <- D.Db.script(h, [["INSERT INTO counted_entry VALUES(3, 'count still exact')"]])
    C.Check.str("counted API stays exact after status calls", finalcount, "ok 1")
    invalid : String <- D.Db.script_status("x", [])
    C.Check.str("status preserves handle errors", invalid, "fail no handle x")
    return Unit{}

def main() -> IO(Unit):
  do IO<Unit>:
    made : Result<&1, &1, U32 & String, U32 & (String & String)> <- Process.run("mktemp", ["-d"], "", 4096, 10000)
    +dir : String <- dir.of(made)
    +a0 : String <- D.Db.open(dir ++ "/a.db")
    C.Check.str("open", a0, "ok 1")
    +a : String = hd(a0)
    mode : List<&2, List<&2, String>> <- D.Db.query(a, "PRAGMA journal_mode", [], 10)
    C.Check.str("wal", Show.rows(mode), "wal;")
    sync : List<&2, List<&2, String>> <- D.Db.query(a, "PRAGMA synchronous", [], 10)
    C.Check.str("synchronous normal", Show.rows(sync), "1;")
    busy : List<&2, List<&2, String>> <- D.Db.query(a, "PRAGMA busy_timeout", [], 10)
    C.Check.str("busy timeout", Show.rows(busy), "5000;")
    ddl : String <- D.Db.exec(a, "CREATE TABLE t(id INTEGER PRIMARY KEY, name TEXT, n, z TEXT); CREATE TABLE big(id INTEGER PRIMARY KEY, name TEXT NOT NULL, twice INTEGER)")
    C.Check.str("exec ddl", ddl, "ok")
    r1 : String <- D.Db.run.t(a, "INSERT INTO t(name, n) VALUES (?, ?)", [D.DbText{"héllo ✓ 'q'"}, D.DbInt{"42"}])
    C.Check.str("run with params", r1, "ok 1")
    r2 : String <- D.Db.run(a, "INSERT INTO t(name, n, z) VALUES (?, ?, ?)", ["two", "42", "#x"])
    C.Check.str("run text params", r2, "ok 1")
    r3 : String <- D.Db.run.t(a, "INSERT INTO t(name, n) VALUES (?, ?)", [D.DbText{"big"}, D.DbInt{"9223372036854775807"}])
    C.Check.str("run int64 max", r3, "ok 1")
    q1 : List<&2, List<&2, String>> <- D.Db.query(a, "SELECT id, name, n, typeof(n), z, typeof(z) FROM t ORDER BY id", [], 100)
    C.Check.str("query rows, NULL as empty", Show.rows(q1), "1|héllo ✓ 'q'|42|integer||null;2|two|42|text|#x|text;3|big|9223372036854775807|integer||null;")
    up : String <- D.Db.run.t(a, "UPDATE t SET z = ? WHERE n = ?", [D.DbText{"seen"}, D.Db.nat(42n)])
    C.Check.str("integer param matches", up, "ok 1")
    reals : List<&2, List<&2, String>> <- D.Db.query(a, "SELECT 1.5, 2.0, 3.0e20, 1.0/3, 1e-7, 0.1, -2.5, 123456789012345.0, NULL, x'6869'", [], 10)
    C.Check.str("column text", Show.rows(reals), "1.5|2.0|3.0e+20|0.333333333333333|1.0e-07|0.1|-2.5|123456789012345.0||hi;")
    dup : List<&2, List<&2, String>> <- D.Db.query(a, "SELECT 1 AS x, 2 AS x", [], 10)
    C.Check.str("repeated column names", Show.rows(dup), "1|2;")
    # 10k rows in one batch
    +all : List<&2, List<&2, D.DbVal>> = rows.go(10000n, [])
    b1 : String <- D.Db.batch.t(a, "INSERT INTO big(id, name, twice) VALUES (?, ?, ?)", all)
    C.Check.str("batch 10k", b1, "ok 10000")
    n1 : List<&2, List<&2, String>> <- D.Db.query(a, "SELECT count(*), sum(twice) FROM big", [], 1)
    C.Check.str("batch rows landed", Show.rows(n1), "10000|99990000;")
    every : List<&2, List<&2, String>> <- D.Db.query(a, "SELECT id, name, twice FROM big ORDER BY id", [], 20000)
    C.Check.str("query all 10k", count(every), "10000")
    found : Nat <- lookups(1000n, a, 0n)
    C.Check.str("1000 point lookups", Nat.show(found), "1000")
    lim : List<&2, List<&2, String>> <- D.Db.query(a, "SELECT id FROM big ORDER BY id DESC", [], 3)
    C.Check.str("query limit", Show.rows(lim), "9999;9998;9997;")
    lim0 : List<&2, List<&2, String>> <- D.Db.query(a, "SELECT id FROM big", [], 0)
    C.Check.str("query limit 0", count(lim0), "0")
    pq : List<&2, List<&2, String>> <- D.Db.query.t(a, "SELECT name FROM big WHERE id = ? OR name = ?", [D.Db.nat(77n), D.DbText{"row 78"}], 10)
    C.Check.str("query params", Show.rows(pq), "row 77;row 78;")
    # errors
    bad : List<&2, List<&2, String>> <- D.Db.query(a, "SELEC 1", [], 10)
    C.Check.str("query fail row", Show.rows(bad), "^@fail|near \"SELEC\": syntax error;")
    badrun : String <- D.Db.run(a, "INSERT INTO nope VALUES (?)", ["x"])
    C.Check.str("run fail", badrun, "fail no such table: nope")
    badex : String <- D.Db.exec(a, "CREATE TABLE t(x)")
    C.Check.str("exec fail", badex, "fail table t already exists")
    # a batch that fails part way leaves nothing behind
    b2 : String <- D.Db.batch.t(a, "INSERT INTO big(id, name) VALUES (?, ?)", [[D.Db.nat(20000n), D.DbText{"new"}], [D.Db.nat(20001n), D.DbText{"new"}], [D.Db.nat(5n), D.DbText{"clash"}]])
    C.Check.str("batch fail", b2, "fail UNIQUE constraint failed: big.id")
    n2 : List<&2, List<&2, String>> <- D.Db.query(a, "SELECT count(*) FROM big", [], 1)
    C.Check.str("batch rolled back", Show.rows(n2), "10000;")
    b3 : String <- D.Db.batch(a, "INSERT INTO big(id, name) VALUES (?, ?)", [])
    C.Check.str("empty batch", b3, "ok 0")
    tx : String <- D.Db.exec(a, "BEGIN; INSERT INTO t(name) VALUES ('in tx'); COMMIT")
    C.Check.str("exec transaction", tx, "ok")
    # a String param is text, whatever it reads: "#42" and "#007" come back
    # exactly, through run, batch, script and query
    rt : String <- D.Db.exec(a, "CREATE TABLE rt(k TEXT, v, w TEXT, i INTEGER)")
    rt1 : String <- D.Db.run(a, "INSERT INTO rt VALUES (?, ?, ?, ?)", ["run", "#42", "#007", "#7"])
    rt2 : String <- D.Db.batch(a, "INSERT INTO rt VALUES (?, ?, ?, ?)", [["batch", "#42", "#007", "#-1"]])
    rt3 : String <- D.Db.script(a, [["INSERT INTO rt VALUES (?, ?, ?, ?)", "script", "#42", "#007", "#9223372036854775807"]])
    C.Check.str("hash text params", rt1 ++ " " ++ rt2 ++ " " ++ rt3, "ok 1 ok 1 ok 1")
    rtq : List<&2, List<&2, String>> <- D.Db.query(a, "SELECT k, v, typeof(v), w, typeof(w), i, typeof(i) FROM rt WHERE v = ? AND w = ? ORDER BY rowid", ["#42", "#007"], 10)
    C.Check.str("hash text round trip", Show.rows(rtq), "run|#42|text|#007|text|#7|text;batch|#42|text|#007|text|#-1|text;script|#42|text|#007|text|#9223372036854775807|text;")
    rtt : String <- D.Db.run.t(a, "INSERT INTO rt VALUES (?, ?, ?, ?)", [D.DbText{"typed"}, D.DbText{"#42"}, D.DbText{"#007"}, D.DbText{"007"}])
    rtn : String <- D.Db.run.t(a, "INSERT INTO rt VALUES (?, ?, ?, ?)", [D.DbText{"ints"}, D.DbInt{"007"}, D.DbInt{"-9223372036854775808"}, D.DbNull{}])
    rtq2 : List<&2, List<&2, String>> <- D.Db.query.t(a, "SELECT k, v, typeof(v), w, typeof(w), i, typeof(i) FROM rt WHERE k IN (?, ?) ORDER BY rowid", [D.DbText{"typed"}, D.DbText{"ints"}], 10)
    C.Check.str("typed params", rtt ++ " " ++ rtn ++ " " ++ Show.rows(rtq2), "ok 1 ok 1 typed|#42|text|#007|text|7|integer;ints|7|integer|-9223372036854775808|text||null;")
    bi1 : String <- D.Db.run.t(a, "INSERT INTO rt(k) VALUES (?)", [D.DbInt{"x"}])
    bi2 : String <- D.Db.run.t(a, "INSERT INTO rt(k) VALUES (?)", [D.DbInt{"9223372036854775808"}])
    bi3 : List<&2, List<&2, String>> <- D.Db.query.t(a, "SELECT ?", [D.DbInt{""}], 1)
    bi4 : String <- D.Db.script.t(a, [D.DbStmt{"INSERT INTO rt(k) VALUES ('kept?')", []}, D.DbStmt{"INSERT INTO rt(k) VALUES (?)", [D.DbInt{"1.5"}]}])
    bi5 : String <- D.Db.batch.t(a, "INSERT INTO rt(k) VALUES (?)", [[D.DbText{"kept?"}], [D.DbInt{"+3"}]])
    bic : List<&2, List<&2, String>> <- D.Db.query(a, "SELECT count(*) FROM rt WHERE k = 'kept?' OR k IS NULL", [], 1)
    C.Check.str("bad integers", bi1 ++ "; " ++ bi2 ++ "; " ++ Show.rows(bi3) ++ " " ++ bi4 ++ "; " ++ bi5 ++ "; " ++ Show.rows(bic), "fail not an integer: x; fail not an integer: 9223372036854775808; ^@fail|not an integer: ; fail not an integer: 1.5; fail not an integer: +3; 0;")
    # script: mixed statements, one transaction
    sc : String <- D.Db.script.t(a, [D.DbStmt{"INSERT INTO t(name, n) VALUES (?, ?)", [D.DbText{"s1"}, D.Db.nat(1n)]}, D.DbStmt{"UPDATE t SET z = ? WHERE name = ?", [D.DbText{"after s1"}, D.DbText{"s1"}]}, D.DbStmt{"DELETE FROM t WHERE name = 'two'", []}, D.DbStmt{"CREATE TABLE kv(k TEXT PRIMARY KEY, v)", []}, D.DbStmt{"INSERT INTO kv VALUES (?, ?), (?, ?)", [D.DbText{"a"}, D.Db.nat(1n), D.DbText{"b"}, D.DbText{"x"}]}])
    C.Check.str("script", sc, "ok 5")
    sq : List<&2, List<&2, String>> <- D.Db.query(a, "SELECT name, z FROM t WHERE name IN ('s1', 'two') UNION ALL SELECT k, v FROM kv", [], 10)
    C.Check.str("script landed", Show.rows(sq), "s1|after s1;a|1;b|x;")
    sbad : String <- D.Db.script(a, [["INSERT INTO kv VALUES ('c', 1)"], ["DELETE FROM t"], ["INSERT INTO kv VALUES ('a', 2)"]])
    C.Check.str("script fail", sbad, "fail UNIQUE constraint failed: kv.k")
    sback : List<&2, List<&2, String>> <- D.Db.query(a, "SELECT (SELECT count(*) FROM kv), (SELECT count(*) FROM t)", [], 1)
    C.Check.str("script rolled back", Show.rows(sback), "2|4;")
    sempty : String <- D.Db.script(a, [["INSERT INTO kv VALUES ('c', 1)"], []])
    C.Check.str("script empty item", sempty, "fail an empty item")
    snone : String <- D.Db.script(a, [])
    C.Check.str("script of nothing", snone, "ok 0")
    # a script inside the caller's own transaction joins it
    b4 : String <- D.Db.exec(a, "BEGIN")
    sin : String <- D.Db.script(a, [["INSERT INTO kv VALUES ('c', 3)"]])
    sinbad : String <- D.Db.script(a, [["INSERT INTO kv VALUES ('d', 4)"], ["INSERT INTO kv VALUES ('c', 5)"]])
    rb : String <- D.Db.exec(a, "ROLLBACK")
    C.Check.str("script inside BEGIN", b4 ++ " " ++ sin ++ " " ++ sinbad ++ " " ++ rb, "ok ok 1 fail UNIQUE constraint failed: kv.k ok")
    sout : List<&2, List<&2, String>> <- D.Db.query(a, "SELECT count(*) FROM kv", [], 1)
    C.Check.str("outer rollback", Show.rows(sout), "2;")
    # statements given the wrong number of params, or none
    pfew : String <- D.Db.run(a, "INSERT INTO kv VALUES (?, ?)", ["x"])
    C.Check.str("too few params", pfew, "fail 1 params for 2 placeholders")
    pmany : List<&2, List<&2, String>> <- D.Db.query(a, "SELECT ?", ["#1", "#2"], 1)
    C.Check.str("too many params", Show.rows(pmany), "^@fail|2 params for 1 placeholders;")
    pnone : String <- D.Db.run(a, "  ", [])
    C.Check.str("no statement", pnone, "fail no statement")
    bnone : String <- D.Db.batch(a, "-- nothing", [["x"]])
    C.Check.str("batch no statement", bnone, "fail no statement")
    # the same statement again, from the cache, after the schema changed
    re1 : String <- D.Db.run(a, "INSERT INTO kv VALUES (?, ?)", ["e", "#1"])
    alt : String <- D.Db.exec(a, "ALTER TABLE kv ADD COLUMN w DEFAULT 'w'")
    re2 : String <- D.Db.run(a, "INSERT INTO kv VALUES (?, ?)", ["f", "#2"])
    C.Check.str("cached statement, new schema", re1 ++ " " ++ alt ++ " " ++ re2, "ok 1 ok fail table kv has 3 columns but 2 values were supplied")
    # FTS5
    fts : String <- D.Db.exec(a, "CREATE VIRTUAL TABLE doc USING fts5(body)")
    C.Check.str("fts5 available", fts, "ok")
    fb : String <- D.Db.batch.t(a, "INSERT INTO doc(rowid, body) VALUES (?, ?)", [[D.Db.nat(1n), D.DbText{"the quick brown fox"}], [D.Db.nat(2n), D.DbText{"a lazy dog sleeps"}], [D.Db.nat(3n), D.DbText{"quick thinking dog"}]])
    C.Check.str("fts5 insert", fb, "ok 3")
    fq : List<&2, List<&2, String>> <- D.Db.query(a, "SELECT rowid, body FROM doc WHERE doc MATCH ? ORDER BY rowid", ["quick"], 10)
    C.Check.str("fts5 match", Show.rows(fq), "1|the quick brown fox;3|quick thinking dog;")
    fq2 : List<&2, List<&2, String>> <- D.Db.query(a, "SELECT rowid FROM doc WHERE doc MATCH ? ORDER BY rank", ["dog NOT lazy"], 10)
    C.Check.str("fts5 query syntax", Show.rows(fq2), "3;")
    # FTS5 with external content, kept by a trigger (the store's entry_fts)
    ext : String <- D.Db.exec(a, "CREATE TABLE entry(seq INTEGER PRIMARY KEY, thread TEXT, text TEXT); CREATE VIRTUAL TABLE entry_fts USING fts5(text, content='entry', content_rowid='seq', tokenize='unicode61 remove_diacritics 0'); CREATE TRIGGER entry_ai AFTER INSERT ON entry BEGIN INSERT INTO entry_fts(rowid, text) VALUES (new.seq, new.text); END")
    C.Check.str("fts5 external content", ext, "ok")
    ei : String <- D.Db.batch.t(a, "INSERT INTO entry(seq, thread, text) VALUES (?, ?, ?)", [[D.Db.nat(10n), D.DbText{"t1"}, D.DbText{"Board routed, DRC clean"}], [D.Db.nat(11n), D.DbText{"t2"}, D.DbText{"schematic: añadir el regulador"}], [D.Db.nat(12n), D.DbText{"t1"}, D.DbText{"routed the USB pair"}]])
    C.Check.str("trigger rows not counted", ei, "ok 3")
    eq : List<&2, List<&2, String>> <- D.Db.query(a, "SELECT e.seq, e.thread, e.text FROM entry_fts f JOIN entry e ON e.seq = f.rowid WHERE entry_fts MATCH ? ORDER BY e.seq DESC", ["routed"], 10)
    C.Check.str("fts5 external match", Show.rows(eq), "12|t1|routed the USB pair;10|t1|Board routed, DRC clean;")
    eu : List<&2, List<&2, String>> <- D.Db.query(a, "SELECT rowid, highlight(entry_fts, 0, '[', ']') FROM entry_fts WHERE entry_fts MATCH ?", ["añadir"], 10)
    C.Check.str("fts5 unicode", Show.rows(eu), "11|schematic: [añadir] el regulador;")
    ep : List<&2, List<&2, String>> <- D.Db.query(a, "SELECT rowid FROM entry_fts WHERE entry_fts MATCH ? ORDER BY rowid", ["\"usb pair\" OR drc*"], 10)
    C.Check.str("fts5 phrase and prefix", Show.rows(ep), "10;12;")
    Status.check(a)
    # a second handle on another file, and a third on the same file
    +b0 : String <- D.Db.open(dir ++ "/b.db")
    C.Check.str("second handle", b0, "ok 2")
    +b : String = hd(b0)
    bt : String <- D.Db.exec(b, "CREATE TABLE only_b(x)")
    C.Check.str("second handle exec", bt, "ok")
    ina : List<&2, List<&2, String>> <- D.Db.query(a, "SELECT name FROM sqlite_master WHERE name = 'only_b'", [], 10)
    C.Check.str("handles are separate", count(ina), "0")
    +c0 : String <- D.Db.open(dir ++ "/a.db")
    C.Check.str("third handle", c0, "ok 3")
    +c : String = hd(c0)
    inc : List<&2, List<&2, String>> <- D.Db.query(c, "SELECT count(*) FROM big", [], 1)
    C.Check.str("same file, other handle", Show.rows(inc), "10000;")
    # busy: c waits its timeout for a's write lock, then fails
    bt1 : String <- D.Db.exec(c, "PRAGMA busy_timeout = 100")
    lk : String <- D.Db.exec(a, "BEGIN IMMEDIATE")
    lw : String <- D.Db.run(c, "INSERT INTO kv(k) VALUES ('busy')", [])
    lr : List<&2, List<&2, String>> <- D.Db.query(c, "SELECT count(*) FROM kv", [], 1)
    lc : String <- D.Db.exec(a, "COMMIT")
    lw2 : String <- D.Db.run(c, "INSERT INTO kv(k) VALUES ('busy')", [])
    C.Check.str("busy", bt1 ++ " " ++ lk ++ " " ++ lw ++ " " ++ Show.rows(lr) ++ " " ++ lc ++ " " ++ lw2, "ok ok fail database is locked 3; ok ok 1")
    # close
    ca : String <- D.Db.close(a)
    C.Check.str("close", ca, "ok")
    gone : String <- D.Db.exec(a, "SELECT 1")
    C.Check.str("closed handle", gone, "fail no handle 1")
    gq : List<&2, List<&2, String>> <- D.Db.query(a, "SELECT 1", [], 1)
    C.Check.str("closed handle query", Show.rows(gq), "^@fail|no handle 1;")
    junk : String <- D.Db.run("x", "SELECT 1", [])
    C.Check.str("junk handle", junk, "fail no handle x")
    +a2 : String <- D.Db.open(dir ++ "/b.db")
    C.Check.str("slot reused", a2, "ok 1")
    cb : String <- D.Db.close(b)
    cc : String <- D.Db.close(c)
    ca2 : String <- D.Db.close(hd(a2))
    C.Check.str("close all", cb ++ cc ++ ca2, "okokok")
    nodir : String <- D.Db.open(dir ++ "/no/such/dir/x.db")
    C.Check.str("open fail", nodir, "fail unable to open database file")
    rm : Result<&1, &1, U32 & String, U32 & (String & String)> <- Process.run("rm", ["-rf", dir], "", 4096, 10000)
    return Unit{}
