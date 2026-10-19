import Base
import ./check.bend as C
import ../src/core/model.bend as M
import ../src/core/client.bend as U
import ../src/mobile/screen.bend as S

def add(+n: Nat, acc: M.State) -> M.State:
  match n:
    case 0n:
      acc
    case 1n+k:
      add(k, M.State.apply(acc, M.MessagePosted{"t", "m" ++ Nat.show(k), M.Assistant{}, "reply", 100n}))

def state() -> M.State:
  add(220n, M.State.apply(M.State.apply(M.State.empty(), M.ProjectCreated{"p", "P", "/tmp", 1n}),
    M.ThreadCreated{"t", "p", "T", M.Local{}, "", "", "claude", "", 1n}))

def held() -> U.Win:
  S.Mob.page(Map.set(&2, String, Map.new(&2, String), "@away.t", "m200"), "t", state())

def first.es(es: List<&2, M.Entry>) -> String:
  match es:
    case Nil{}: ""
    case e <> _: M.Idx.id(e)

def first(w: U.Win) -> String:
  first.es(U.Win.shown(w))

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("reading page retains original twenty plus arrivals", Nat.show(List.length(&2, M.Entry, U.Win.shown(held()))), "220")
    C.Check.str("original first entry survives two hundred arrivals", first(held()), "m219")
    C.Check.str("following page remains bounded", Nat.show(List.length(&2, M.Entry, U.Win.shown(S.Mob.page(Map.new(&2, String), "t", state())))), "20")
    C.Check.str("following newest page starts at recent entry", first(S.Mob.page(Map.new(&2, String), "t", state())), "m19")
