import Base
import ./check.bend as C
import ../src/server/review-history.bend as R
import ../src/server/review-window.bend as RW
import ../src/server/db.bend as D
import ../src/core/model.bend as M
import ../src/core/review.bend as RV

# Actual production adapter and M.State fold; no renderer import or substitute.
def Fixture.cold() -> M.State:
  M.State.apply(M.State.empty(), M.ThreadCreated{"a", "", "Settled", M.Local{}, "", "", "codex", "", 1n})

def Fixture.advance(left: Nat, st: M.State) -> M.State:
  match left:
    case 0n: st
    case 1n+n: Fixture.advance(n, M.State.tick(st))

def Fixture.text() -> String:
  String.repeat("Full review context 🛶 日本語. ", 40n)

def Fixture.root() -> M.Entry:
  M.Act{"root", "a", "review", "add", RV.Review.op.add("root", RV.Anchor{"board.kicad_pcb", "", "board", "", 1.0, 2.0, ""}, Fixture.text(), "person", ""), 2n}

def Fixture.reply() -> M.Entry:
  M.Act{"reply", "a", "review", "add", RV.Review.op.add("reply", RV.Anchor{"", "", "", "", 0.0, 0.0, ""}, "Reply", "person", "root"), 4n}

def Fixture.last() -> M.Entry:
  M.Act{"resolve-260", "a", "review", "resolve", RV.Review.op.resolve("root", "person"), 520n}

def Fixture.known() -> M.State:
  M.State.apply(M.State.apply(Fixture.advance(519n, Fixture.cold()),
    M.ActivityLogged{"a", "queued-new", "review", "reopen", RV.Review.op.reopen("root", "person"), 521n}),
    M.ActivityLogged{"a", "resolve-260", "review", "reopen", RV.Review.op.reopen("root", "queued-wins"), 522n})

def Fixture.row(e: M.Entry) -> List<&2, String>:
  match e:
    case M.Act{id, th, tone, kind, summary, +at}:
      [Nat.show(at), Nat.show(at), id, th, "", "act", tone, kind, summary, Nat.show(at)]
    case _: Nil{}

def Fixture.ids(es: List<&2, M.Entry>) -> String:
  match es:
    case Nil{}: ""
    case e <> rest: M.Idx.id(e) ++ " " ++ Fixture.ids(rest)

def Fixture.first_summary(es: List<&2, M.Entry>) -> String:
  match es:
    case M.Act{_, _, _, _, text, _} <> _: text
    case _: ""

def Fixture.put(h: String, e: M.Entry) -> IO(Unit):
  match e:
    case M.Act{id, th, tone, kind, summary, +at}:
      do IO<Unit>:
        out : String <- D.Db.run.t(h, "INSERT INTO entry VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
          [D.Db.nat(at), D.Db.nat(at), D.DbText{id}, D.DbText{th}, D.DbText{""}, D.DbText{"act"}, D.DbText{tone}, D.DbText{kind}, D.DbText{summary}, D.Db.nat(at)])
        C.Check.str("adapter fixture insert", out, "ok 1")
    case _: IO.pure(Unit, Unit{})

def main() -> IO(Unit):
  do IO<Unit>:
    +opened : String <- D.Db.open(":memory:")
    C.Check.str("adapter fixture database opened", String.take(opened, 3n), "ok ")
    +h : String = String.drop(opened, 3n)
    setup : String <- D.Db.exec(h, "CREATE TABLE entry(seq INTEGER PRIMARY KEY,ord INTEGER,id TEXT,thread TEXT,src TEXT,kind TEXT,role TEXT,akind TEXT,text TEXT,at INTEGER); CREATE INDEX entry_thread ON entry(thread,seq)")
    C.Check.str("adapter fixture schema", setup, "ok")
    Fixture.put(h, Fixture.root())
    Fixture.put(h, Fixture.reply())
    bulk : String <- D.Db.run.t(h, "WITH RECURSIVE x(n) AS (SELECT 3 UNION ALL SELECT n+1 FROM x WHERE n<260) INSERT INTO entry SELECT n*2,n*2,'resolve-'||n,'a','','act','review','resolve',?1,n*2 FROM x", [D.DbText{RV.Review.op.resolve("root", "person")}])
    C.Check.str("adapter fixture spans three pages", bulk, "ok 258")
    Fixture.put(h, M.Act{"foreign", "other", "review", "add", "private", 3n})
    Fixture.put(h, M.Act{"tool", "a", "tool", "Bash", "not a review", 5n})
    Fixture.put(h, M.Act{"future", "a", "review", "add", "beyond pinned sequence", 524n})

    +known : M.State = Fixture.known()
    +stored : M.State = R.ReviewHistory.post([Fixture.row(Fixture.root()), Fixture.row(Fixture.reply()), Fixture.row(Fixture.last())], M.State.empty())
    C.Check.str("adapter None keeps original resident review", Fixture.ids(M.State.reviews(R.ReviewHistory.complete(None{}, known, "a"), "a")), "resolve-260 queued-new ")
    +some : M.State = R.ReviewHistory.complete(Some{stored}, known, "a")
    C.Check.str("adapter Some merges stored baseline after queued ops", Fixture.ids(M.State.reviews(some, "a")), "resolve-260 queued-new reply root ")
    C.Check.str("adapter Some queued duplicate retains resident payload", Bool.show(String.contains(Fixture.first_summary(M.State.reviews(some, "a")), "queued-wins")), "True")
    C.Check.str("adapter empty Some keeps resident review", Fixture.ids(M.State.reviews(R.ReviewHistory.complete(Some{M.State.empty()}, known, "a"), "a")), "resolve-260 queued-new ")
    lost : Maybe<&1, M.State> = RW.ReviewWindow.check(M.State, 128n, 128n, True{}, "late read failed", stored)
    C.Check.str("adapter late error discards entire partial M.State fold", Fixture.ids(M.State.reviews(R.ReviewHistory.complete(lost, known, "a"), "a")), "resolve-260 queued-new ")

    +shown : M.State <- R.ReviewHistory.read(h, known, "a")
    C.Check.str("actual M.State adapter folds all three SQLite pages", Nat.show(List.length(&2, M.Entry, M.State.reviews(shown, "a"))), "261")
    C.Check.str("actual adapter preserves newest queued and sequence order", Fixture.ids(List.take(&2, M.Entry, M.State.reviews(shown, "a"), 3n)), "resolve-260 queued-new resolve-259 ")
    C.Check.str("actual adapter preserves oldest root and reply", Fixture.ids(List.take(&2, M.Entry, List.reverse(&2, M.Entry, M.State.reviews(shown, "a")), 2n)), "root reply ")
    C.Check.str("actual adapter queued duplicate payload wins", Bool.show(String.contains(Fixture.first_summary(M.State.reviews(shown, "a")), "queued-wins")), "True")
    C.Check.str("actual adapter preserves root and reply semantics", Nat.show(List.length(&2, RV.Comment, RV.Review.of_state(shown, "a"))), "2")
    C.Check.str("actual adapter queued reopen wins old stored resolutions", Nat.show(RV.Review.count.open(RV.Review.of_state(shown, "a"))), "1")
    C.Check.str("actual adapter preserves whole long review text", Bool.show(String.contains(RV.Review.md(RV.Review.of_state(shown, "a")), Fixture.text())), "True")
    C.Check.str("actual adapter excludes other thread review", Nat.show(List.length(&2, M.Entry, M.State.reviews(shown, "other"))), "0")
    C.Check.str("actual adapter excludes post-snapshot row", Bool.show(String.contains(Fixture.ids(M.State.reviews(shown, "a")), "future")), "False")
    C.Check.str("actual adapter keeps original sequence", Nat.show(M.State.seq(shown)), "522")
    C.Check.str("actual adapter does not hydrate canonical entries", Nat.show(List.length(&2, M.Entry, M.State.entries.all(shown))), Nat.show(List.length(&2, M.Entry, M.State.entries.all(known))))
    +again : M.State <- R.ReviewHistory.read(h, shown, "a")
    C.Check.str("actual adapter repeated read deduplicates all stored IDs", Nat.show(List.length(&2, M.Entry, M.State.reviews(again, "a"))), "261")
    +empty : M.State <- R.ReviewHistory.read(h, known, "missing")
    C.Check.str("actual adapter empty disk result preserves resident review", Fixture.ids(M.State.reviews(empty, "a")), "resolve-260 queued-new ")
    +failed : M.State <- R.ReviewHistory.read("not-a-handle", known, "a")
    C.Check.str("actual adapter read failure returns original review state", Fixture.ids(M.State.reviews(failed, "a")), "resolve-260 queued-new ")
    +local : M.State <- R.ReviewHistory.read("", known, "a")
    C.Check.str("actual adapter no-store skip preserves resident review", Fixture.ids(M.State.reviews(local, "a")), "resolve-260 queued-new ")
    +far : M.State <- R.ReviewHistory.read("not-a-handle", known, "remote~a")
    C.Check.str("actual adapter remote skip preserves resident review", Fixture.ids(M.State.reviews(far, "a")), "resolve-260 queued-new ")
    +blank : M.State <- R.ReviewHistory.read(h, known, "")
    C.Check.str("actual adapter blank-thread skip preserves resident review", Fixture.ids(M.State.reviews(blank, "a")), "resolve-260 queued-new ")
    closed : String <- D.Db.close(h)
    C.Check.str("adapter fixture closes after all reads", closed, "ok")
