import Base
import ./check.bend as C
import ../src/server/db.bend as D
import ../src/server/review-window.bend as R

def count(rows: List<&2, List<&2, String>>, n: Nat) -> Nat:
  Nat.add(n, List.length(&2, List<&2, String>, rows))

def answer(value: Maybe<&1, Nat>) -> String:
  match value:
    case None{}: "failed"
    case Some{n}: Nat.show(n)

def main() -> IO(Unit):
  do IO<Unit>:
    opened : String <- D.Db.open(":memory:")
    +h : String = String.drop(opened, 3n)
    setup : String <- D.Db.exec(h, "CREATE TABLE entry(seq INTEGER PRIMARY KEY,ord INTEGER,id TEXT,thread TEXT,src TEXT,kind TEXT,role TEXT,akind TEXT,text TEXT,at INTEGER); CREATE INDEX entry_thread ON entry(thread,seq); WITH RECURSIVE x(n) AS (SELECT 1 UNION ALL SELECT n+1 FROM x WHERE n<260) INSERT INTO entry SELECT n,n,'id-'||n,'a','','act','review','add','text',n FROM x")
    C.Check.str("review fixture", setup, "ok")
    all : Maybe<&1, Nat> <- R.ReviewWindow.fold(Nat, h, "a", 0n, 260n, 0n, count)
    C.Check.str("fold spans three real SQLite pages", answer(all), "260")
    pinned : Maybe<&1, Nat> <- R.ReviewWindow.fold(Nat, h, "a", 0n, 130n, 0n, count)
    C.Check.str("through sequence is inclusive and fixed", answer(pinned), "130")
    empty : Maybe<&1, Nat> <- R.ReviewWindow.fold(Nat, h, "other", 0n, 260n, 17n, count)
    C.Check.str("empty disk result preserves the accumulator", answer(empty), "17")
    late : Maybe<&1, Nat> <- R.ReviewWindow.fold.page(Nat, h, "a", 128n, 260n, 128n, count, D.HistoryPage{Nil{}, 128n, False{}, "read failed"})
    C.Check.str("late failure discards the entire partial fold", answer(late), "failed")
    stuck : Maybe<&1, Nat> <- R.ReviewWindow.fold.page(Nat, h, "a", 128n, 260n, 128n, count, D.HistoryPage{Nil{}, 128n, False{}, ""})
    C.Check.str("nonadvancing continuation cannot loop", answer(stuck), "failed")
    closed : String <- D.Db.close(h)
    C.Check.str("review reader releases its savepoint and statements", closed, "ok")
