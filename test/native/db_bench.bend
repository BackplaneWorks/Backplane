import Base
import ../../src/server/db.bend as D

# Db throughput in the store's shapes: entries with an
# external-content FTS5 index kept by a trigger, written by one big batch and
# by many small scripts (one per hub commit), read back whole, by key, a
# page at a time and by search: `bend test/native/db_bench.bend` (Bun).
# Each line: "bench <what> n=<operations> ms=<ms>".

def hd(a: String) -> String:
  String.drop(a, 3n)

def words(+k: Nat) -> String:
  "routed the board " ++ Nat.show(k) ++ " drc clean usb pair regulator net" ++ Nat.show(Nat.mod(k, 97n))

# n entry rows [k, thread, text] for k < n
def rows.go(+n: Nat, acc: List<&2, List<&2, D.DbVal>>) -> List<&2, List<&2, D.DbVal>>:
  match n:
    case 0n: acc
    case 1n+k: rows.go(k, [D.Db.nat(k), D.DbText{"t" ++ Nat.show(Nat.mod(k, 50n))}, D.DbText{words(k)}] <> acc)

# one commit's items, as the store writes them: c entries from seq s, a
# record upsert and the meta row
def commit.go(+c: Nat, +s: Nat, acc: List<&2, D.DbStmt>) -> List<&2, D.DbStmt>:
  match c:
    case 0n: D.DbStmt{"INSERT INTO thread(id, json) VALUES (?, ?) ON CONFLICT(id) DO UPDATE SET json = excluded.json", [D.DbText{"t" ++ Nat.show(Nat.mod(s, 50n))}, D.DbText{"{\"seq\":" ++ Nat.show(s) ++ "}"}]} <> (D.DbStmt{"INSERT OR REPLACE INTO meta(k, v) VALUES ('applied_seq', ?)", [D.Db.nat(s)]} <> acc)
    case 1n+k: commit.go(k, s, D.DbStmt{"INSERT INTO entry(seq, thread, text) VALUES (?, ?, ?) ON CONFLICT DO NOTHING", [D.Db.nat(Nat.add(s, k)), D.DbText{"t" ++ Nat.show(Nat.mod(s, 50n))}, D.DbText{words(Nat.add(s, k))}]} <> acc)

def ok.not.k(b: Bool) -> Nat:
  match b:
    case True{}: 0n
    case False{}: 1n

def ok.not(r: String) -> Nat:
  ok.not.k(String.starts_with(r, "ok"))

# n scripts of c entries each, seqs from s
def commits(+n: Nat, +h: String, +c: Nat, +s: Nat, bad: Nat) -> IO(Nat):
  match n:
    case 0n: IO.pure(Nat, bad)
    case 1n+k:
      do IO<Nat>:
        r : String <- D.Db.script.t(h, commit.go(c, s, []))
        commits(k, h, c, Nat.add(s, c), Nat.add(bad, ok.not(r)))

# rows counted with an accumulator (Base's List.length is not a tail call
# and overflows the JS stack at tens of thousands)
def got.go(rs: List<&2, List<&2, String>>, n: Nat) -> Nat:
  match rs:
    case Nil{}: n
    case _ <> t: got.go(t, Nat.add(n, 1n))

def got(rs: List<&2, List<&2, String>>) -> Nat:
  got.go(rs, 0n)

# n point lookups by seq: rows found
def lookups(+n: Nat, +h: String, found: Nat) -> IO(Nat):
  match n:
    case 0n: IO.pure(Nat, found)
    case 1n+k:
      do IO<Nat>:
        r : List<&2, List<&2, String>> <- D.Db.query.t(h, "SELECT thread, text FROM entry WHERE seq = ?", [D.Db.nat(Nat.mod(Nat.mul(k, 7919n), 50000n))], 1)
        lookups(k, h, Nat.add(found, got(r)))

# n pages of 50 newest-first entries of a thread before a seq: rows read
def pages(+n: Nat, +h: String, found: Nat) -> IO(Nat):
  match n:
    case 0n: IO.pure(Nat, found)
    case 1n+k:
      do IO<Nat>:
        r : List<&2, List<&2, String>> <- D.Db.query.t(h, "SELECT seq, text FROM entry WHERE thread = ? AND seq < ? ORDER BY seq DESC LIMIT 50", [D.DbText{"t" ++ Nat.show(Nat.mod(k, 50n))}, D.Db.nat(Nat.sub(50000n, Nat.mul(k, 37n)))], 50)
        pages(k, h, Nat.add(found, got(r)))

# n searches, 20 hits at most each: rows read
def searches(+n: Nat, +h: String, found: Nat) -> IO(Nat):
  match n:
    case 0n: IO.pure(Nat, found)
    case 1n+k:
      do IO<Nat>:
        r : List<&2, List<&2, String>> <- D.Db.query(h, "SELECT e.seq, e.text FROM entry_fts f JOIN entry e ON e.seq = f.rowid WHERE entry_fts MATCH ? ORDER BY f.rowid DESC LIMIT 20", ["usb net" ++ Nat.show(Nat.mod(k, 97n))], 20)
        searches(k, h, Nat.add(found, got(r)))

def say(what: String, n: Nat, a: Nat, b: Nat) -> IO(Unit):
  IO.print("bench " ++ what ++ " n=" ++ Nat.show(n) ++ " ms=" ++ Nat.show(Nat.sub(b, a)))

def dir.of(r: Result<&1, &1, U32 & String, U32 & (String & String)>) -> IO(String):
  match r:
    case Done{(_, (out, _))}: IO.pure(String, String.trim(out))
    case Fail{(_, e)}: IO.die(String, 1, "db_bench: mktemp failed: " ++ e)

def main() -> IO(Unit):
  do IO<Unit>:
    made : Result<&1, &1, U32 & String, U32 & (String & String)> <- Process.run("mktemp", ["-d", "-p", "."], "", 4096, 10000)
    +dir : String <- dir.of(made)
    +a0 : String <- D.Db.open(dir ++ "/bench.db")
    +h : String = hd(a0)
    +ddl : String <- D.Db.exec(h, "CREATE TABLE meta(k TEXT PRIMARY KEY, v); CREATE TABLE thread(id TEXT PRIMARY KEY, json TEXT); CREATE TABLE entry(seq INTEGER PRIMARY KEY, thread TEXT, text TEXT); CREATE INDEX entry_thread ON entry(thread, seq); CREATE VIRTUAL TABLE entry_fts USING fts5(text, content='entry', content_rowid='seq'); CREATE TRIGGER entry_ai AFTER INSERT ON entry BEGIN INSERT INTO entry_fts(rowid, text) VALUES (new.seq, new.text); END")
    IO.print("setup " ++ a0 ++ " " ++ ddl)
    +all : List<&2, List<&2, D.DbVal>> = rows.go(50000n, [])
    t0 : Nat <- IO.now()
    +b1 : String <- D.Db.batch.t(h, "INSERT INTO entry(seq, thread, text) VALUES (?, ?, ?)", all)
    +t1 : Nat <- IO.now()
    say("batch_insert_fts(" ++ b1 ++ ")", 50000n, t0, t1)
    +bad : Nat <- commits(1000n, h, 10n, 100000n, 0n)
    +t2 : Nat <- IO.now()
    say("script_commits_of_10(failed " ++ Nat.show(bad) ++ ")", 1000n, t1, t2)
    +every : List<&2, List<&2, String>> <- D.Db.query(h, "SELECT seq, thread, text FROM entry ORDER BY seq", [], 100000)
    +t3 : Nat <- IO.now()
    say("query_all_rows(" ++ Nat.show(got(every)) ++ ")", got(every), t2, t3)
    +found : Nat <- lookups(10000n, h, 0n)
    +t4 : Nat <- IO.now()
    say("point_lookups(found " ++ Nat.show(found) ++ ")", 10000n, t3, t4)
    +paged : Nat <- pages(1000n, h, 0n)
    +t5 : Nat <- IO.now()
    say("pages_of_50(rows " ++ Nat.show(paged) ++ ")", 1000n, t4, t5)
    +hits : Nat <- searches(1000n, h, 0n)
    t6 : Nat <- IO.now()
    say("fts_searches(rows " ++ Nat.show(hits) ++ ")", 1000n, t5, t6)
    cl : String <- D.Db.close(h)
    rm : Result<&1, &1, U32 & String, U32 & (String & String)> <- Process.run("rm", ["-rf", dir], "", 4096, 10000)
    return Unit{}
