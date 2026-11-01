import Base
import ./check.bend as C
import ./subagent_history_test.bend as F
import ../src/server/thin-view.bend as V
import ../src/core/thin.bend as T
import ../src/core/model.bend as M
import ../src/core/subs.bend as SB

def window(+st: M.State, bounded: Bool) -> M.State:
  V.State.window.body(bounded, st, T.Index.of(st), V.Target.json("thread", "t1"), Nil{})

def check(+bounded: Bool) -> IO(Unit):
  do IO<Unit>:
    +st : M.State = M.State.apply(F.finished(), M.ActivityLogged{"@sub:foreign", "secret/start", "secret", "start:subagent", "Private other review", 204n})
    +shown : M.State = window(st, bounded)
    C.Check.str("selected inspector preserves completed counts " ++ Bool.show(bounded), SB.Subs.panel.word(F.rows(shown, Map.new(&2, String))), "2 done")
    C.Check.str("selected inspector preserves old review steps " ++ Bool.show(bounded), F.fingerprint(F.find(F.rows(shown, F.expanded()), "c1")), F.fingerprint(F.find(F.rows(st, F.expanded()), "c1")))
    C.Check.str("shared activity page stays bounded " ++ Bool.show(bounded), Nat.show(List.length(&2, M.Entry, M.State.mine(shown, "@sub:t1"))), "400")
    C.Check.str("own busy call stays bounded " ++ Bool.show(bounded), Nat.show(List.length(&2, M.Entry, M.State.mine(shown, M.Idx.sub.call("@sub:t1", "c2")))), "400")
    C.Check.str("foreign starts stay outside projection " ++ Bool.show(bounded), Nat.show(List.length(&2, M.Entry, M.State.mine(shown, M.Idx.sub.starts("@sub:foreign")))), "0")
    C.Check.str("foreign call bodies stay outside projection " ++ Bool.show(bounded), Nat.show(List.length(&2, M.Entry, M.State.mine(shown, M.Idx.sub.call("@sub:foreign", "secret")))), "0")

def main() -> IO(Unit):
  do IO<Unit>:
    check(False{})
    check(True{})
