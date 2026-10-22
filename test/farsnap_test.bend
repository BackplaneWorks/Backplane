import Base
import ./check.bend as C
import ../src/core/model.bend as M
import ../src/core/hot.bend as H
import ../src/core/codec.bend as K
import ../src/core/json.bend as J
import ../src/core/farsnap.bend as S

# A far mirror's snapshot (farsnap.bend): the live part of a machine's
# state as changes it already folds, so a mirror keeps that and not the
# machine's whole history.

# a change list as its log lines
def text(cs: List<&2, M.Change>) -> String:
  match cs:
    case Nil{}:
      ""
    case c <> t:
      J.Json.show(K.Change.encode(c)) ++ "\n" ++ text(t)

def main() -> IO(Unit):
  do IO<Unit>:
    +d : M.State = S.Snap.demo()
    +snap : List<&2, M.Change> = S.Snap.of.at(d, 2n)
    +back : M.State = S.Snap.fold(snap)
    C.Check.str("snapshot: the live threads only", S.Snap.made(snap), "t1 t10 t2 t3 t4 t5 ")
    C.Check.str("fold: the threads it holds", Nat.show(Map.size(&2, M.Thread, H.Hot.threads(back))), "6")
    C.Check.str("fixpoint: a snapshot of the fold is the snapshot", text(S.Snap.of.at(back, 2n)), text(snap))
    C.Check.str("view: as many live rows", Nat.show(List.length(&2, H.HRow, H.Hot.view(back, 2n))), Nat.show(List.length(&2, H.HRow, H.Hot.view(d, 2n))))
    C.Check.str("window: the last two entries of t1", Nat.show(List.length(&2, M.Entry, M.State.mine(back, "t1"))), "2")
    C.Check.str("asks: only the open one", Nat.show(List.length(&2, M.Ask, M.Asks.all(back))), "1")
    C.Check.str("live: settled, archived, deleted and elsewhere are not",
      Bool.pick(String, Bool.or(H.Hot.thread.of(d, "t6"), Bool.or(H.Hot.thread.of(d, "t7"), H.Hot.thread.of(d, "t8"))), "some", "none"), "none")
    C.Check.str("live: kept by a task, a bot, a running turn", Bool.pick(String, Bool.and(H.Hot.thread.of(d, "t10"), Bool.and(H.Hot.thread.of(d, "t2"), H.Hot.thread.of(d, "t4"))), "all", "not all"), "all")
    C.Check.str("changes: fewer than the log", Bool.pick(String, Nat.is_lt(List.length(&2, M.Change, snap), M.State.seq(d)), "fewer", "more"), "fewer")
    IO.print(text(snap))
