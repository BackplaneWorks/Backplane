import Base
import ./check.bend as C
import ../src/core/storedb.bend as S
import ../src/server/db.bend as D

# Values through SQLite and back (D.Db, bun:sqlite): text that reads like
# a number stays the text it was, numbers land in INTEGER columns as
# integers, and a delete finds the key it was given.

def handle(+r: String) -> String:
  String.drop(r, 3n)

def items(os: List<&2, S.Op>, acc: List<&2, List<&2, String>>) -> List<&2, List<&2, String>>:
  match os:
    case Nil{}:
      List.reverse(&2, List<&2, String>, acc)
    case o <> t:
      items(t, S.Sql.of(o) <> acc)

def row(rs: List<&2, List<&2, String>>) -> String:
  match rs:
    case r <> _:
      String.join(r, "|")
    case Nil{}:
      "<none>"

def main() -> IO(Unit):
  do IO<Unit>:
    o : String <- D.Db.open(":memory:")
    +h : String = handle(o)
    e : String <- D.Db.exec(h, S.Sdb.schema())
    C.Check.str("schema made", e, "ok")
    w : String <- D.Db.script(h, items([S.Put{"setting", ["#42", "#007", "0"]}, S.Put{"setting", ["plain", "007", "0"]},
      S.Put{"thread", ["#7", "p", "#007", "5", "6", "idle", "0", "0", "1", "0", "", "", "", "0", "{}"]},
      S.Put{"obj", ["mem", "#9", "\"#9\""]}, S.Put{"line", ["1", "0", "12", ""]}], Nil{}))
    C.Check.str("written", String.take(w, 2n), "ok")
    a : List<&2, List<&2, String>> <- D.Db.query(h, "SELECT k, v, typeof(v) FROM setting WHERE k = '#42'", [], 5)
    C.Check.str("#42 / #007 round-trip as text", row(a), "#42|#007|text")
    b : List<&2, List<&2, String>> <- D.Db.query(h, "SELECT v, typeof(v) FROM setting WHERE k = 'plain'", [], 5)
    C.Check.str("007 stays text in a TEXT column", row(b), "007|text")
    c : List<&2, List<&2, String>> <- D.Db.query(h, "SELECT id, title, created, typeof(created), pinned FROM thread", [], 5)
    C.Check.str("thread: text kept, numbers integers", row(c), "#7|#007|5|integer|1")
    l : List<&2, List<&2, String>> <- D.Db.query(h, "SELECT seq + 1, len, typeof(off) FROM line", [], 5)
    C.Check.str("line numbers are integers", row(l), "2|12|integer")
    d : String <- D.Db.script(h, items([S.Del{"setting", ["#42"]}, S.Del{"obj", ["mem", "#9"]}], Nil{}))
    C.Check.str("deleted", String.take(d, 2n), "ok")
    x : List<&2, List<&2, String>> <- D.Db.query(h, "SELECT count(*) FROM setting WHERE k = '#42'", [], 5)
    C.Check.str("delete by a #-key", row(x), "0")
    y : List<&2, List<&2, String>> <- D.Db.query(h, "SELECT count(*) FROM obj", [], 5)
    C.Check.str("obj delete by (domain, #-key)", row(y), "0")
    z : String <- D.Db.close(h)
    C.Check.str("closed", z, "ok")
