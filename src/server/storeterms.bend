import Base
import ../core/storedb.bend as SDB
import ../core/model.bend as M
import ../core/exe.bend as EX
import ./db.bend as D
import ./proc.bend as P
import ./host.bend as H
import ./perf.bend as PS
import ./storefar.bend as SFL

# Search terms made again in the background
# -----------------------------------------
# A store whose search terms are behind this code's (SDB.Sdb.terms.due:
# a bump of Sdb.terms.v, not of the codec) is still used at boot, and its
# term table is derived again from the entry table while the hub serves:
#
# 1. the child (`backplane --store-terms HOME`, at storefar.bend's Fl.low:
#    nice 19, ionice idle, a 1 GB JS heap) clears any term_old
#    a previous swap left, then fills term_next from the entries in short
#    transactions (Terms.batch() a time, ordinal order; it resumes from
#    term_next's last ordinal after a restart, and starts afresh when
#    term_next was for another version, meta "terms.next"), and says
#    "built <last ordinal>";
# 2. the hub, in one script on its own handle (so between two commits),
#    adds the terms of the entries committed since, renames term to
#    term_old and term_next to term, and sets meta "terms";
# 3. the child runs again and deletes term_old in batches.
#
# Search keeps answering from the old terms until the swap. No write holds
# the store for long: the hub's connection waits at most 5 s for a lock.

def Terms.batch() -> Nat:
  2000n

def Terms.clear.batch() -> Nat:
  20000n

def Terms.ok(+r: String) -> Bool:
  String.starts_with(r, "ok")

# a decimal answer (SQL's text of an integer; M.Switch.num's walk)
def Terms.num(+s: String) -> Nat:
  M.Switch.num(s, 0n)

def Terms.cell.k(r: Maybe<&2, List<&2, String>>) -> String:
  match r:
    case Some{cs}:
      Maybe.default(&2, String, List.head(&2, String, cs), "")
    case None{}:
      ""

def Terms.cell(rows: List<&2, List<&2, String>>) -> String:
  Terms.cell.k(List.head(&2, List<&2, String>, rows))

def Terms.one(+h: String, +sql: String, +ps: List<&2, String>) -> IO(String):
  IO.bind(List<&2, List<&2, String>>, String, D.Db.query(h, sql, ps, 1), rows => IO.pure(String, Terms.cell(rows)))

def Terms.meta(+h: String, +k: String) -> IO(String):
  Terms.one(h, "SELECT v FROM meta WHERE k = ?", [k])

def Terms.table(+h: String, +t: String) -> IO(Bool):
  IO.bind(String, Bool, Terms.one(h, "SELECT name FROM sqlite_master WHERE type = 'table' AND name = ?", [t]), n => IO.pure(Bool, String.eq(n, t)))

# Statements
# ----------

# a row [ord, text] of the entry table
def Terms.ord(r: List<&2, String>) -> Nat:
  Terms.num(Maybe.default(&2, String, List.head(&2, String, r), ""))

def Terms.text(r: List<&2, String>) -> String:
  Maybe.default(&2, String, List.get(&2, String, r, 1n), "")

def Terms.rows(rows: List<&2, List<&2, String>>, +t: String, acc: List<&2, List<&2, String>>) -> List<&2, List<&2, String>>:
  match rows:
    case Nil{}:
      acc
    case +r <> rest:
      Terms.rows(rest, t, List.append(&2, List<&2, String>, SDB.Sdb.terms.sql(t, Terms.ord(r), Terms.text(r)), acc))

def Terms.last(rows: List<&2, List<&2, String>>, +n: Nat) -> Nat:
  match rows:
    case Nil{}:
      n
    case +r <> rest:
      Terms.last(rest, Terms.ord(r))

def Terms.entries.sql() -> String:
  "SELECT ord, text FROM entry WHERE ord > ? ORDER BY ord LIMIT ?"

def Terms.meta.sql(+k: String, +v: String) -> List<&2, String>:
  ["INSERT OR REPLACE INTO meta(k, v) VALUES (?, ?)", k, v]

# The child
# ---------

def Terms.cleared(empty: Bool, ok: Bool, +r: String, +h: String, +t: String, k: Unit -> IO(String)) -> IO(String):
  match empty ok:
    case True{} _:
      D.Db.exec(h, "DROP TABLE " ++ t)
    case False{} True{}:
      IO.bind(Unit, String, IO.sleep(20), k)
    case False{} False{}:
      IO.pure(String, r)

# a table emptied a batch at a time, then dropped (fuel: batches)
def Terms.clear.go(fuel: Nat, +h: String, +t: String) -> IO(String):
  match fuel:
    case 0n:
      IO.pure(String, "fail clearing " ++ t ++ " ran out of batches")
    case 1n+f:
      do IO<String>:
        +r : String <- D.Db.run(h, "DELETE FROM " ++ t ++ " WHERE (word, ord) IN (SELECT word, ord FROM " ++ t ++ " LIMIT " ++ Nat.show(Terms.clear.batch()) ++ ")", Nil{})
        Terms.cleared(Bool.and(Terms.ok(r), String.eq(r, "ok 0")), Terms.ok(r), r, h, t, u => Terms.clear.go(f, h, t))

def Terms.clear.k(has: Bool, +h: String, +t: String) -> IO(String):
  match has:
    case True{}:
      Terms.clear.go(1000000n, h, t)
    case False{}:
      IO.pure(String, "ok")

def Terms.clear(+h: String, +t: String) -> IO(String):
  IO.bind(Bool, String, Terms.table(h, t), has => Terms.clear.k(has, h, t))

def Terms.filled.k(ok: Bool, +r: String, +last: Nat, k: Nat -> IO(String)) -> IO(String):
  match ok:
    case True{}:
      k(last)
    case False{}:
      IO.pure(String, r)

def Terms.filled(done: Bool, +rows: List<&2, List<&2, String>>, +h: String, +from: Nat, k: Nat -> IO(String)) -> IO(String):
  match done:
    case True{}:
      IO.pure(String, "built " ++ Nat.show(from))
    case False{}:
      do IO<String>:
        +last : Nat = Terms.last(rows, from)
        +r : String <- D.Db.script(h, Terms.rows(rows, "term_next", Nil{}))
        Terms.filled.k(Terms.ok(r), r, last, k)

def Terms.filled.rows(+rows: List<&2, List<&2, String>>, +h: String, +from: Nat, k: Nat -> IO(String)) -> IO(String):
  Terms.filled(List.is_empty(&2, List<&2, String>, rows), rows, h, from, k)

# term_next filled from ordinal `from` on, a batch a transaction; answers
# the last ordinal read
def Terms.fill(fuel: Nat, +h: String, +from: Nat) -> IO(String):
  match fuel:
    case 0n:
      IO.pure(String, "fail term_next ran out of batches")
    case 1n+f:
      IO.bind(List<&2, List<&2, String>>, String, D.Db.query(h, Terms.entries.sql(), [Nat.show(from), Nat.show(Terms.batch())], 4000000000),
        rows => Terms.filled.rows(rows, h, from, n => Terms.fill(f, h, n)))

# a term_next for another version is cleared first
def Terms.restart(same: Bool, +h: String) -> IO(String):
  match same:
    case True{}:
      IO.pure(String, "ok")
    case False{}:
      Terms.clear(h, "term_next")

def Terms.make(+h: String) -> IO(String):
  do IO<String>:
    nv : String <- Terms.meta(h, "terms.next")
    c : String <- Terms.restart(String.eq(nv, Nat.show(SDB.Sdb.terms.v())), h)
    mk : String <- D.Db.exec(h, "CREATE TABLE IF NOT EXISTS term_next(word TEXT, ord INTEGER, PRIMARY KEY(word, ord)) WITHOUT ROWID")
    mt : String <- D.Db.script(h, [Terms.meta.sql("terms.next", Nat.show(SDB.Sdb.terms.v()))])
    from : String <- Terms.one(h, "SELECT coalesce(max(ord), 0) FROM term_next", Nil{})
    Terms.fill(1000000n, h, Terms.num(from))

def Terms.go(due: Bool, +h: String) -> IO(String):
  match due:
    case True{}:
      Terms.make(h)
    case False{}:
      IO.pure(String, "current")

def Terms.child.h(+h: String) -> IO(Unit):
  do IO<Unit>:
    +c : String <- Terms.clear(h, "term_old")
    have : String <- Terms.meta(h, "terms")
    r : String <- Terms.go(SDB.Sdb.terms.due(have), h)
    cl : String <- D.Db.close(h)
    IO.print(Bool.pick(String, Terms.ok(c), r, c))

def Terms.child.k(ok: Bool, +o: String) -> IO(Unit):
  match ok:
    case True{}:
      Terms.child.h(String.drop(o, 3n))
    case False{}:
      IO.print(o)

# `backplane --store-terms HOME`: prints "current", "built <ordinal>" or
# "fail <why>"
def Terms.opened(+o: String) -> IO(Unit):
  Terms.child.k(String.starts_with(o, "ok "), o)

def Terms.child(+home: String) -> IO(Unit):
  IO.bind(String, Unit, D.Db.open(home ++ "/store.db"), o => Terms.opened(o))

def Terms.back(xs: List<&2, String>) -> List<String>:
  match xs:
    case Nil{}:
      Nil{}
    case x <> t:
      x <> Terms.back(t)

# the program's arguments: the child's, or passed on (go)
def Terms.main.k(xs: List<&2, String>, go: List<String> -> IO(Unit)) -> IO(Unit):
  match xs:
    case "--store-terms" <> +home <> _:
      Terms.child(home)
    case x:
      go(Terms.back(x))

def Terms.copy(xs: List<String>) -> List<&2, String>:
  match xs:
    case Nil{}:
      Nil{}
    case x <> t:
      x <> Terms.copy(t)

def Terms.main(xs: List<String>, go: List<String> -> IO(Unit)) -> IO(Unit):
  Terms.main.k(Terms.copy(xs), go)

# The hub's side
# --------------

# the entries since the child's last, their terms into term_next, and the
# swap, all in one script on the hub's handle
def Terms.swap.sql(rows: List<&2, List<&2, String>>) -> List<&2, List<&2, String>>:
  List.append(&2, List<&2, String>, List.reverse(&2, List<&2, String>, Terms.rows(rows, "term_next", Nil{})),
    [["ALTER TABLE term RENAME TO term_old"], ["ALTER TABLE term_next RENAME TO term"],
     Terms.meta.sql("terms", Nat.show(SDB.Sdb.terms.v())), ["DELETE FROM meta WHERE k = 'terms.next'"]])

def Terms.swap(+h: String, +last: Nat) -> IO(String):
  do IO<String>:
    rows : List<&2, List<&2, String>> <- D.Db.query(h, "SELECT ord, text FROM entry WHERE ord > ? ORDER BY ord", [Nat.show(last)], 4000000000)
    D.Db.script(h, Terms.swap.sql(rows))

def Terms.run(+home: String) -> IO(P.Ran):
  IO.bind(String, P.Ran, H.Sys.exe_path(), exe => P.Proc.run("sh", ["-c", SFL.Fl.low(), "sh", EX.Exe.child(exe), "--store-terms", home], "/"))

def Terms.out(r: P.Ran) -> String:
  match r:
    case P.Ran{_, +out}:
      String.trim(out)

def Terms.said(+what: String, +r: String) -> IO(Unit):
  PS.Log.info("store", "search terms " ++ what ++ ": " ++ r)

def Terms.swapped(+home: String, +r: String) -> IO(Unit):
  do IO<Unit>:
    Terms.said("made again", r)
    c : P.Ran <- Terms.run(home)
    Terms.said("old table cleared", Terms.out(c))

def Terms.after.swap(ok: Bool, +home: String, +r: String) -> IO(Unit):
  match ok:
    case True{}:
      Terms.swapped(home, r)
    case False{}:
      Terms.said("swap failed", r)

def Terms.built(+home: String, +h: String, +out: String) -> IO(Unit):
  do IO<Unit>:
    +r : String <- Terms.swap(h, Terms.num(String.drop(out, 6n)))
    Terms.after.swap(Terms.ok(r), home, r)

def Terms.quiet(current: Bool, +out: String) -> IO(Unit):
  match current:
    case True{}:
      IO.pure(Unit, Unit{})
    case False{}:
      Terms.said("not made again", out)

def Terms.after(built: Bool, +home: String, +h: String, +out: String) -> IO(Unit):
  match built:
    case True{}:
      Terms.built(home, h, out)
    case False{}:
      Terms.quiet(String.eq(out, "current"), out)

def Terms.job.k(need: Bool, +home: String, +h: String) -> IO(Unit):
  match need:
    case False{}:
      IO.pure(Unit, Unit{})
    case True{}:
      do IO<Unit>:
        Terms.said("behind or left over", "making them again in the background")
        r : P.Ran <- Terms.run(home)
        +out : String = Terms.out(r)
        Terms.after(String.starts_with(out, "built "), home, h, out)

# once the hub serves, read on its own handle: the child runs only when the
# store's terms are behind or a term_old was left; most boots run nothing
def Terms.job(+home: String, +h: String) -> IO(Unit):
  do IO<Unit>:
    have : String <- Terms.meta(h, "terms")
    old : Bool <- Terms.table(h, "term_old")
    Terms.job.k(Bool.or(SDB.Sdb.terms.due(have), old), home, h)

def Terms.start(+home: String, +h: String) -> IO(Unit):
  IO.spawn(Unit, Terms.job(home, h))
