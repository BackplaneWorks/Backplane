import Base
import ./check.bend as C
import ../src/server/db.bend as D
import ../src/server/design-history.bend as H
import ../src/core/hist.bend as HI
import ../src/core/model.bend as M

def ids(es: List<&2, M.Entry>) -> String:
  match es:
    case Nil{}: ""
    case +e <> rest: M.Idx.id(e) ++ " " ++ ids(rest)

def captured(n: Nat, st: M.State) -> M.State:
  match st:
    case M.State{ps, ts, es, _, tk, sg, qu, ak, dv, nav, bw}: M.State{ps, ts, es, n, tk, sg, qu, ak, dv, nav, bw}

def main() -> IO(Unit):
  do IO<Unit>:
    +opened : String <- D.Db.open(":memory:")
    +db : String = String.drop(opened, 3n)
    +schema : String <- D.Db.exec(db, "CREATE TABLE entry(seq INTEGER,ord INTEGER,id TEXT,thread TEXT,src TEXT,kind TEXT,role TEXT,akind TEXT,text TEXT,at INTEGER)")
    C.Check.str("history fixture uses SQLite", schema, "ok")
    +inserted : String <- D.Db.run.t(db, "INSERT INTO entry VALUES(1,1,'tool','t','','act','tool','Bash',?1,1),(2,2,'s1','t','','act','step','kicad','{\"e\":\"tool\",\"c\":\"a\",\"p\":\"\",\"f\":[]}',2),(3,3,'hidden','other','','act','step','kicad','{\"e\":\"tool\",\"c\":\"private\"}',3),(4,4,'new-message','t','','msg','assistant','','new chat page',4),(5,5,'s2','t','','act','step','kicad','{\"e\":\"tool\",\"c\":\"b\",\"p\":\"a\",\"f\":[]}',5)", [D.DbText{String.repeat("x", 400n)}])
    C.Check.str("history fixture inserted", String.take(inserted, 2n), "ok")
    +page : M.State = captured(5n, M.State.post(M.State.empty(), M.Msg{"new-message", "t", M.Assistant{}, "new chat page", 4n}))
    +loaded : M.State <- H.History.restore(db, "t", page)
    C.Check.str("old steps survive a hot state with no step bodies", ids(M.State.steps(loaded, "t")), "s2 s1 ")
    C.Check.str("only selected thread step payloads are loaded", ids(M.State.steps(loaded, "other")), "")
    C.Check.str("chat stays on its bounded page", ids(M.State.mine(loaded, "t")), "new-message ")
    C.Check.str("repeated tool references load one source", ids(M.State.mine(loaded, HI.Hist.sources.key("t"))), "tool ")
    C.Check.str("tool source text is bounded before leaving SQLite", Nat.show(String.length(HI.Hist.label(M.State.mine(loaded, HI.Hist.sources.key("t")), "tool"))), "246")
    +rendered : M.State = HI.Hist.window(loaded, "t", M.State.empty())
    C.Check.str("a second display projection preserves off-page sources", ids(M.State.mine(rendered, HI.Hist.sources.key("t"))), "tool ")
    +earlier : M.State <- H.History.restore(db, "t", captured(2n, page))
    C.Check.str("a newer snapshot never crosses the captured revision", ids(M.State.steps(earlier, "t")), "s1 ")
    +before : M.State <- H.History.restore(db, "t", captured(1n, page))
    C.Check.str("future source labels never cross the captured revision", ids(M.State.mine(before, HI.Hist.sources.key("t"))), "")
    +none : M.State <- H.History.restore(db, "absent", page)
    C.Check.str("a thread without history stays empty", ids(M.State.steps(none, "absent")), "")
    +closed : String <- D.Db.close(db)
    C.Check.str("history fixture closed", closed, "ok")
