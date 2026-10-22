import Base
import ./check.bend as C
import ../src/core/storedb.bend as S
import ../src/core/model.bend as M
import ../src/core/json.bend as J

def show.rows(xs: List<&2, Sigma<&2, &2, String, _ => String>>) -> String:
  match xs:
    case Nil{}:
      ""
    case (k, v) <> t:
      k ++ " => " ++ v ++ "\n" ++ show.rows(t)

def show(m: Map<&2, String>) -> String:
  show.rows(Map.to_list(&2, String, m))

def ids(es: List<&2, M.Entry>) -> String:
  match es:
    case Nil{}:
      ""
    case +e <> t:
      M.Idx.id(e) ++ "@" ++ M.Idx.thread(e) ++ " " ++ ids(t)

def mat() -> S.Mat:
  S.Mat.replay(S.Demo.log(), S.Mat.empty())

def fold() -> M.State:
  M.State.replay(S.Demo.changes(), M.State.empty())

def half(+n: Nat) -> S.Mat:
  S.Mat.replay(List.drop(&2, S.LL, S.Demo.log(), n), S.Mat.replay(List.take(&2, S.LL, S.Demo.log(), n), S.Mat.empty()))

def lines(m: Map<&2, String>) -> Nat:
  List.length(&2, String, List.filter(String, k => String.starts_with(k, "line\u{1f}"), Map.keys(&2, String, m)))

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("records equal the fold's dump", show(S.Mat.recs(mat())), show(S.Tab.dump(fold())))
    C.Check.str("thread t2's entries are mine", ids(S.Tab.mine(S.Mat.facts(mat()), "t2")), ids(M.State.mine(fold(), "t2")))
    C.Check.str("thread t1's entries are mine", ids(S.Tab.mine(S.Mat.facts(mat()), "t1")), ids(M.State.mine(fold(), "t1")))
    C.Check.str("a torn line has no number", Nat.show(lines(S.Mat.facts(mat()))), Nat.show(1n+List.length(&2, M.Change, S.Demo.changes())))
    C.Check.str("store from a cut equals whole: records", show(S.Mat.recs(half(30n))), show(S.Mat.recs(mat())))
    C.Check.str("store from a cut equals whole: facts", show(S.Mat.facts(half(30n))), show(S.Mat.facts(mat())))
    C.Check.str("record sql", String.join(S.Sql.of(S.Put{"thread", ["t", "p"]}), "|"), "INSERT OR REPLACE INTO thread(id, project, title, created, activity, turn, pending, archived, pinned, settled_at, provider, model, session, deleted, json) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)|t|p")
    C.Check.str("fact sql", String.join(S.Sql.of(S.Put{"term", ["w", "#3"]}), "|"), "INSERT INTO term(word, ord) VALUES (?, ?) ON CONFLICT DO NOTHING|w|#3")
    C.Check.str("obj delete sql", String.join(S.Sql.of(S.Del{"obj", ["mem", "b/k"]}), "|"), "DELETE FROM obj WHERE domain = ? AND key = ?|mem|b/k")
