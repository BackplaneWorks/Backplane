import Base
import ./check.bend as C
import ../src/core/model.bend as M
import ../src/core/hub.bend as B

def keys(cs: List<&2, M.Change>) -> String:
  match cs:
    case Nil{}: ""
    case M.SettingSet{k, _, _} <> rest: k ++ "," ++ keys(rest)
    case _ <> rest: "other," ++ keys(rest)

def changes(s: B.Step) -> String:
  match s:
    case B.Step{cs, _}: keys(cs)

def count.outs(s: B.Step) -> String:
  match s:
    case B.Step{_, os}: Nat.show(List.length(&2, B.Out, os))

def check.rec(r: B.Rec) -> IO(Unit):
  match r:
    case B.Rec{s, cs, os}:
      do IO<Unit>:
        C.Check.str("no-op recovery keeps accumulated changes", keys(cs), "a,")
        C.Check.str("no-op recovery keeps accumulated outs", Nat.show(List.length(&2, B.Out, os)), "1")
        C.Check.str("no-op recovery leaves state untouched", Nat.show(M.State.seq(s)), "0")

def main() -> IO(Unit):
  do IO<Unit>:
    +a : B.Step = B.Step{[M.SettingSet{"a", "1", 1n}], [B.Await{"a", "id-a", "thread", "target", "ident", 1n, "/"}]}
    +b : B.Step = B.Step{[M.SettingSet{"b", "2", 2n}], [B.Await{"b", "id-b", "thread", "target", "ident", 2n, "/"}]}
    C.Check.str("empty before step preserves changes", changes(B.Step.cat(B.Step.none(), a)), "a,")
    C.Check.str("empty after step preserves changes", changes(B.Step.cat(a, B.Step.none())), "a,")
    C.Check.str("nonempty join preserves change order", changes(B.Step.cat(a, b)), "a,b,")
    C.Check.str("nonempty join preserves both outs", count.outs(B.Step.cat(a, b)), "2")
    C.Check.str("empty join keeps outs", count.outs(B.Step.cat(a, B.Step.none())), "1")
    check.rec(B.Rec.add(B.Rec{M.State.empty(), [M.SettingSet{"a", "1", 1n}], [B.Await{"a", "id-a", "thread", "target", "ident", 1n, "/"}]}, B.Step.none()))
