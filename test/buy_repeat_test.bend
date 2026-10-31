import Base
import ./check.bend as C
import ../src/core/model.bend as M
import ../src/core/hub.bend as H
import ../src/core/client.bend as U

def state() -> M.State:
  M.State.replay([M.ProjectCreated{"p", "Lamp", "/tmp/lamp", 1n}, M.ThreadCreated{"s", "p", "Source", M.Local{}, "", "", "claude", "", 1n}], M.State.empty())

def changes(step: H.Step) -> List<&2, M.Change>:
  match step:
    case H.Step{cs, _}: cs

def created(cs: List<&2, M.Change>) -> Nat:
  match cs:
    case Nil{}: 0n
    case M.ThreadCreated{_, _, _, _, _, _, _, _, _} <> t: 1n+created(t)
    case _ <> t: created(t)

def after() -> M.State:
  M.State.replay(changes(H.Buy.start(True{}, state(), 100n, 0, "one", "s")), state())

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("first click makes one thread", Nat.show(created(changes(H.Buy.start(True{}, state(), 100n, 0, "one", "s")))), "1")
    C.Check.str("rapid duplicate makes none", Nat.show(created(changes(H.Buy.start(True{}, after(), 101n, 1, "two", "s")))), "0")
    C.Check.str("later purchase remains possible", Nat.show(created(changes(H.Buy.start(True{}, after(), 104n, 0, "three", "s")))), "1")
    C.Check.str("pending client click sends nothing", Nat.show(List.length(&2, U.Cmd, U.Act.cmds(U.Fab.buy.start(True{}, U.Ui.init(1n), "s")))), "0")
    C.Check.str("queued click on the new buy thread makes none", Nat.show(created(changes(H.Buy.start(True{}, after(), 101n, 0, "stale", H.Buy.last(after(), "s", 100n))))), "0")
