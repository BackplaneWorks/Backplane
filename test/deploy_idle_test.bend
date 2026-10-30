import Base
import ./check.bend as C
import ../src/core/model.bend as M
import ../src/core/hub.bend as H
import ../src/core/deploy.bend as D

def busy() -> M.State:
  H.Deploy.demo(True{}, H.Deploy.demo.line("t0"))

def idle() -> M.State:
  M.State.replay([M.TurnChanged{"t1", M.Idle{}, 8n}], busy())

def check(st: M.State, free: Bool, open: List<&2, String>) -> H.Step:
  H.Deploy.check.idle(st, 9n, free, open)

def goes(st: M.State, free: Bool, open: List<&2, String>) -> String:
  Nat.show(H.Deploy.demo.go(check(st, free, open)))

def newer() -> M.State:
  M.State.replay([M.ThreadCreated{"t2", "p1", "new", M.Local{}, "", "", "claude", "", 8n}, M.TurnChanged{"t2", M.Running{}, 8n}], idle())

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("between-call gap never interrupts a running turn", goes(busy(), True{}, Nil{}), "0")
    C.Check.str("open tool also holds restart", goes(busy(), True{}, H.Deploy.demo.open(True{})), "0")
    C.Check.str("idle turn still waits for outstanding MCP answer", goes(idle(), True{}, H.Deploy.demo.open(True{})), "0")
    C.Check.str("all idle still waits for build lock", goes(idle(), False{}, Nil{}), "0")
    C.Check.str("all idle and calls locks clear permits restart", goes(idle(), True{}, Nil{}), "1")
    C.Check.str("a new turn after preflight holds actual handoff", goes(newer(), True{}, Nil{}), "0")
    C.Check.str("last turn completion permits pending handoff", goes(M.State.replay([M.TurnChanged{"t2", M.Idle{}, 9n}], newer()), True{}, Nil{}), "1")
    C.Check.str("handoff fires only once", goes(M.State.replay(H.Deploy.demo.cs(check(idle(), True{}, Nil{})), idle()), True{}, Nil{}), "0")
    C.Check.str("held handoff does not mark source turn complete", D.Deploy.running(M.State.replay(H.Deploy.demo.cs(check(busy(), True{}, Nil{})), busy())), "t1")
