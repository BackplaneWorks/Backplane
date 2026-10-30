import Base
import ./check.bend as C
import ../src/core/restartretry.bend as R
import ../src/core/hub.bend as H
import ../src/core/model.bend as M
import ../src/core/mcp.bend as MC
import ../src/core/json.bend as J

def base() -> M.State:
  M.State.replay([M.ProjectCreated{"p", "Test", "/tmp/restart-retry", 1n},
    M.ThreadCreated{"t", "p", "Worker", M.Local{}, "", "", "claude", "test", 1n},
    M.MessagePosted{"t", "request", M.User{}, "Finish the job once", 2n}, M.TurnChanged{"t", M.Running{}, 2n}], M.State.empty())

def fold(+s: M.State, st: H.Step) -> M.State:
  M.State.replay(H.Queue.changes(st), s)

def recovered(+s: M.State, +now: Nat) -> M.State:
  fold(s, H.Hub.recover(s, now))

def restart(+before: M.State, +now: Nat) -> M.State:
  +after = recovered(before, now)
  fold(after, H.Hub.carry(before, after, now))

def first() -> M.State:
  restart(base(), 10n)

def second() -> M.State:
  restart(first(), 20n)

def scheduled() -> M.State:
  fold(second(), R.Retry.after(first(), second(), Nil{}, 20n))

def starts(os: List<&2, H.Out>) -> Nat:
  match os:
    case Nil{}: 0n
    case H.AgentStart{_, _, _, _, _, _, _} <> rest: 1n+starts(rest)
    case H.CodexTurn{_, _, _, _, _, _} <> rest: 1n+starts(rest)
    case _ <> rest: starts(rest)

def kills(os: List<&2, H.Out>) -> Bool:
  match os:
    case Nil{}: False{}
    case H.AgentStop{_} <> _: True{}
    case H.WatchStop{_, _, _, _, _} <> _: True{}
    case _ <> rest: kills(rest)

def changed(st: H.Step) -> Nat:
  List.length(&2, M.Change, H.Queue.changes(st))

def task.got(m: Maybe<&2, M.Task>) -> M.TaskState:
  match m:
    case Some{M.Task{_, _, _, _, st, _, _, _, _}}: st
    case None{}: M.TaskCancelled{}

def task(s: M.State) -> M.TaskState:
  task.got(H.Task.of_child(s, "t"))

def named(task: M.TaskState) -> String:
  match task:
    case M.TaskRunning{}: "running"
    case M.TaskDone{}: "completed"
    case M.TaskFailed{}: "failed"
    case _: "other"

def child() -> M.State:
  M.State.replay([M.ThreadCreated{"parent", "p", "Parent", M.Local{}, "", "", "claude", "", 1n},
    M.TaskDelegated{"k", "parent", "t", 1n, "claude", "test", 2n}, M.TaskStateSet{"k", M.TaskRunning{}, "", 2n}], base())

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("first restart retains immediate carry", Bool.show(H.Hub.running(H.Hub.thread(first(), "t"))), "True")
    C.Check.str("second restart preserves immediate anti-loop", Bool.show(H.Hub.running(H.Hub.thread(second(), "t"))), "False")
    C.Check.str("existing Carry.will remains false", Bool.show(H.Carry.will(first(), "t")), "False")
    C.Check.str("repeated continuation has delayed eligibility", Bool.show(H.Carry.deferred(first(), "t")), "True")
    C.Check.str("delay has no immediate start", Nat.show(starts(H.Step.outs_of(R.Retry.after(first(), second(), Nil{}, 20n)))), "0")
    C.Check.str("first retry deadline is sixty seconds", Nat.show(R.Retry.due(scheduled(), "t")), "80")
    C.Check.str("backoff count persisted", M.Setting.get(scheduled(), R.Retry.count.key("t"), ""), "1")
    C.Check.str("schedule is idempotent", Nat.show(changed(R.Retry.after(first(), scheduled(), Nil{}, 21n))), "0")
    C.Check.str("no retry before deadline", Nat.show(starts(H.Step.outs_of(R.Retry.tick(scheduled(), 79n)))), "0")
    +resume : H.Step = R.Retry.tick(scheduled(), 80n)
    +resumed : M.State = fold(scheduled(), resume)
    C.Check.str("one start at deadline", Nat.show(starts(H.Step.outs_of(resume))), "1")
    C.Check.str("marker cleared before start", M.Setting.get(resumed, R.Retry.key("t"), "missing"), "")
    C.Check.str("deadline tick never kills", Bool.show(kills(H.Step.outs_of(resume))), "False")
    C.Check.str("second tick cannot start twice", Nat.show(starts(H.Step.outs_of(R.Retry.tick(resumed, 81n)))), "0")
    +third : M.State = restart(resumed, 90n)
    +thirdwait : M.State = fold(third, R.Retry.after(resumed, third, Nil{}, 90n))
    C.Check.str("retry continuation retains anti-loop", Bool.show(H.Carry.will(resumed, "t")), "False")
    C.Check.str("next retry backs off to two minutes", Nat.show(R.Retry.due(thirdwait, "t")), "210")
    C.Check.str("delay capped at twenty minutes", Nat.show(R.Retry.delay(900n)), "1200")
    C.Check.str("count saturated safely", Nat.show(R.Retry.next(900n)), "20")
    +stopped : M.State = fold(scheduled(), H.Hub.interrupt(scheduled(), 25n, 0, "1", "t"))
    C.Check.str("explicit Stop cancels delayed retry", Nat.show(starts(H.Step.outs_of(R.Retry.tick(stopped, 100n)))), "0")
    +mcpstopped : M.State = M.State.replay(MC.Res.changes(MC.Mcp.interrupt.tool(True{}, False{}, scheduled(), 25n, "caller", "t")), scheduled())
    C.Check.str("authorized MCP Stop cancels delayed retry", Nat.show(starts(H.Step.outs_of(R.Retry.tick(mcpstopped, 100n)))), "0")
    +denied : MC.Res = MC.Mcp.interrupt.tool(False{}, False{}, scheduled(), 25n, "foreign", "t")
    C.Check.str("foreign MCP Stop cannot cancel delayed retry", Bool.show(MC.Res.good(denied)) ++ "/" ++ Nat.show(List.length(&2, M.Change, MC.Res.changes(denied))), "False/0")
    +off : M.State = M.State.apply(scheduled(), M.SettingSet{"restart.continue", "off", 25n})
    C.Check.str("restart continue off never retries", Nat.show(starts(H.Step.outs_of(R.Retry.tick(off, 100n)))), "0")
    C.Check.str("off clears retry marker", M.Setting.get(fold(off, R.Retry.tick(off, 100n)), R.Retry.key("t"), "missing"), "")
    +ask : M.State = M.State.apply(scheduled(), M.AskOpened{"ask", "t", "approval", "Bash", "Allow", "", 25n})
    C.Check.str("pending approval blocks retry", Nat.show(starts(H.Step.outs_of(R.Retry.tick(ask, 100n)))), "0")
    +away : M.State = M.State.apply(scheduled(), M.ThreadArchived{"t", True{}, 25n})
    C.Check.str("archived thread never retries", Nat.show(starts(H.Step.outs_of(R.Retry.tick(away, 100n)))), "0")
    +changedtarget : M.State = M.State.apply(scheduled(), M.ProviderSet{"t", "codex", "test", 25n})
    C.Check.str("provider switch invalidates retry", Nat.show(starts(H.Step.outs_of(R.Retry.tick(changedtarget, 100n)))), "0")
    +newinput : M.State = M.State.apply(scheduled(), M.MessagePosted{"t", "new", M.User{}, "Stop waiting", 25n})
    C.Check.str("new input invalidates retry", Nat.show(starts(H.Step.outs_of(R.Retry.tick(newinput, 100n)))), "0")
    C.Check.str("adopted process never scheduled", Nat.show(changed(R.Retry.after(first(), second(), ["t"], 20n))), "0")
    +progress : M.State = M.State.apply(first(), M.MessagePosted{"t", "answer", M.Assistant{}, "I checked the file", 11n})
    C.Check.str("real output keeps ordinary immediate carry", Nat.show(changed(R.Retry.after(progress, restart(progress, 20n), Nil{}, 20n))), "0")
    +fake : M.State = M.State.apply(base(), M.MessagePosted{"t", "fake", M.Assistant{}, M.Carry.text(), 11n})
    C.Check.str("assistant text is never actual carry prompt", Bool.show(H.Carry.deferred(fake, "t")), "False")
    +cfirst : M.State = restart(child(), 10n)
    +csecond : M.State = restart(cfirst, 20n)
    C.Check.str("delegated ownership survives second restart", named(task(csecond)), "running")
    +cwait : M.State = fold(csecond, R.Retry.after(cfirst, csecond, Nil{}, 20n))
    +crun : M.State = fold(cwait, R.Retry.tick(cwait, 80n))
    C.Check.str("delegated task remains running through retry", named(task(crun)), "running")
    +done : M.State = fold(crun, H.Hub.agent_line(crun, 90n, "t", "{\"type\":\"result\",\"is_error\":false}"))
    C.Check.str("delegated task completes only on real result", named(task(done)), "completed")
    +intent : M.State = fold(base(), H.Locks.work.start(base(), 10n, "t", J.Json.obj([J.KV{"command", J.Str{"fixture"}}, J.KV{"lock", J.Str{"test"}}]), True{}))
    C.Check.str("prelaunch owned work intent rearmed after restart", Nat.show(H.Watch.awaits(H.Step.outs_of(H.Hub.recover(intent, 20n)))), "1")
