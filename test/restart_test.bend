import Base
import ./check.bend as C
import ../src/core/model.bend as M
import ../src/core/hub.bend as H

# A restart with background work out (hub.bend's Rec.lost, Hub.carry,
# Rec.release, Bg.lost): ten threads at once start four at a time; a
# restart repeated before the work was told counts and, the second time,
# only logs it; with restart.continue off nothing starts and the rows stay
# with their outcome unknown; a child whose turn ended with work out keeps
# its task open, is told what it lost, and delivers once when it is done.

def state(cs: List<&2, M.Change>) -> M.State:
  M.State.replay(cs, M.State.empty())

def apply(+s: M.State, st: H.Step) -> M.State:
  M.State.replay(H.Queue.changes(st), s)

def ids(+n: Nat) -> List<&2, String>:
  match n:
    case 0n:
      Nil{}
    case 1n+p:
      ("w" ++ Nat.show(n)) <> ids(p)

def lost(ts: List<&2, String>) -> List<&2, M.Change>:
  match ts:
    case Nil{}:
      Nil{}
    case +t <> rest:
      List.append(&2, M.Change, H.Lost.demo.cs(True{}, False{}, t), lost(rest))

def ten() -> M.State:
  state(M.ProjectCreated{"p1", "p", "/p", 1n} <> lost(ids(10n)))

def recovered(+s: M.State, +now: Nat) -> M.State:
  apply(s, H.Hub.recover(s, now))

def carried.k(+before: M.State, +after: M.State, +now: Nat) -> M.State:
  apply(after, H.Hub.carry(before, after, now))

# a restart: recover, then carry, as the server does
def restart(+s: M.State, +now: Nat) -> M.State:
  carried.k(s, recovered(s, now), now)

def released(+s: M.State, +now: Nat) -> M.State:
  apply(s, H.Rec.release(s, now))

# every running thread stops (their turns end: each notifies, Release)
def ended(ts: List<&2, String>, +s: M.State, +now: Nat) -> M.State:
  match ts:
    case Nil{}:
      s
    case +t <> rest:
      ended(rest, Bool.pick(M.State, H.Hub.running(H.Hub.thread(s, t)), M.State.replay([M.TurnChanged{t, M.Completed{}, now}], s), s), now)

def acts(es: List<&2, M.Entry>, +kind: String) -> Nat:
  match es:
    case Nil{}:
      0n
    case M.Act{_, _, _, +k, _, _} <> t:
      Nat.add(Bool.pick(Nat, String.eq(k, kind), 1n, 0n), acts(t, kind))
    case _ <> t:
      acts(t, kind)

# a child t1 whose turn ended with a background shell out: its task k1 open
def child() -> M.State:
  state([M.ProjectCreated{"p1", "p", "/p", 1n}, M.ThreadCreated{"t0", "p1", "root", M.Local{}, "", "", "claude", "", 1n},
    M.ThreadCreated{"t1", "p1", "child", M.Local{}, "", "", "claude", "", 1n}, M.TaskDelegated{"k1", "t0", "t1", 1n, "claude", "", 1n},
    M.TaskStateSet{"k1", M.TaskRunning{}, "", 1n}, M.MessagePosted{"t1", "u1", M.User{}, "do it", 2n}, M.TurnChanged{"t1", M.Running{}, 2n},
    M.WorkSet{"t1", "bg", "b1\tsleep 60\tc1\tsleep 60\tshell", 3n}, M.MessagePosted{"t1", "a1", M.Assistant{}, "started it", 3n},
    M.TurnChanged{"t1", M.Completed{}, 4n}])

# t1's turn cut off mid-reply by a restart (a1 so far)
def partial() -> M.State:
  state([M.ProjectCreated{"p1", "p", "/p", 1n}, M.ThreadCreated{"t1", "p1", "one", M.Local{}, "", "", "claude", "", 1n},
    M.MessagePosted{"t1", "u1", M.User{}, "do it", 2n}, M.TurnChanged{"t1", M.Running{}, 2n}, M.MessagePosted{"t1", "a1", M.Assistant{}, "half of it", 3n}])

def task.k(m: Maybe<&2, M.Task>) -> M.TaskState:
  match m:
    case Some{M.Task{_, _, _, _, st, _, _, _, _}}:
      st
    case None{}:
      M.TaskQueued{}

def task(+s: M.State) -> M.TaskState:
  task.k(H.Task.of_child(s, "t1"))

def delivers(cs: List<&2, M.Change>) -> Nat:
  match cs:
    case Nil{}:
      0n
    case M.TaskDelivered{_, _} <> t:
      1n+delivers(t)
    case _ <> t:
      delivers(t)

def main() -> IO(Unit):
  do IO<Unit>:
    +r1 : M.State = restart(ten(), 10n)
    # a turn the restart cut off carries on, and its partial reply is not "Finished"
    C.Check.str("a cut-off turn carries on, marked stopped", Bool.show(H.Hub.running(H.Hub.thread(restart(partial(), 20n), "t1"))) ++ "/" ++ M.Setting.get(restart(partial(), 20n), "stopped.u1", ""), "True/1")
    C.Check.str("ten threads: four start at once", Nat.show(H.Lost.running(r1)), "4")
    C.Check.str("the rest are marked", Nat.show(List.length(&2, String, H.Resume.ids(r1))), "6")
    +r2 : M.State = released(r1, 11n)
    C.Check.str("a release starts four more", Nat.show(H.Lost.running(r2)), "8")
    +r3 : M.State = released(ended(ids(10n), r2, 12n), 13n)
    C.Check.str("and the next the last two", Nat.show(H.Lost.running(r3)) ++ "/" ++ Nat.show(List.length(&2, String, H.Resume.ids(r3))), "2/0")
    C.Check.str("each told once", Nat.show(H.Lost.told(M.State.mine(r3, "w1"))) ++ Nat.show(H.Lost.told(M.State.mine(r3, "w10"))), "11")
    # restarts again before the work was told
    +a1 : M.State = recovered(H.Lost.demo(True{}, False{}), 10n)
    C.Check.str("marked at the first restart", H.Resume.why(a1, "t1"), "restart")
    +a2 : M.State = recovered(a1, 20n)
    C.Check.str("counted at the second", H.Resume.why(a2, "t1") ++ "/" ++ Nat.show(H.Resume.n(a2, "t1")), "restart/1")
    +a3 : M.State = recovered(a2, 30n)
    C.Check.str("only logged at the third", H.Resume.why(a3, "t1") ++ "/" ++ Nat.show(acts(M.State.mine(a3, "t1"), "lost")) ++ "/" ++ Bool.show(H.Hub.running(H.Hub.thread(restart(a3, 40n), "t1"))), "/1/False")
    # restart.continue off
    +o1 : M.State = restart(H.Lost.demo(False{}, False{}), 10n)
    C.Check.str("off: logged, nothing starts", Nat.show(acts(M.State.mine(o1, "t1"), "lost")) ++ "/" ++ Bool.show(H.Hub.running(H.Hub.thread(o1, "t1"))), "1/False")
    C.Check.str("off: the rows stay, outcome unknown", M.Bg.status(Maybe.default(&2, String, List.head(&2, String, M.Bg.of(o1, "t1")), "")), "unknown")
    C.Check.str("off: logged once", Nat.show(acts(M.State.mine(restart(o1, 20n), "t1"), "lost")), "1")
    # a child that ended its turn with work out
    +c1 : M.State = restart(child(), 10n)
    C.Check.str("the child is told and runs; its task stays open", Bool.show(H.Hub.running(H.Hub.thread(c1, "t1"))) ++ "/" ++ Nat.show(H.Lost.told(M.State.mine(c1, "t1"))) ++ "/" ++ Bool.show(M.TaskState.terminal(task(c1))), "True/1/False")
    +d1 : H.Step = H.Task.finish.w(c1, 20n, "t1", H.Step{[M.TurnChanged{"t1", M.Completed{}, 20n}], []})
    C.Check.str("its next end delivers once", Nat.show(delivers(H.Queue.changes(d1))) ++ "/" ++ Nat.show(delivers(H.Queue.changes(H.Task.finish.w(apply(c1, d1), 21n, "t1", H.Step{[M.TurnChanged{"t1", M.Completed{}, 21n}], []})))), "1/0")

