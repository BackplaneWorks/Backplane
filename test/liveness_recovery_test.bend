import Base
import ./check.bend as C
import ../src/core/hub.bend as H
import ../src/core/model.bend as M
import ../src/core/json.bend as J
import ../src/core/mcp.bend as MC

def base() -> M.State:
  M.State.replay([M.ProjectCreated{"p", "Test", "/tmp/liveness-test", 1n},
    M.ThreadCreated{"t", "p", "Worker", M.Local{}, "", "/tmp/liveness-test", "codex", "test", 1n},
    M.MessagePosted{"t", "request", M.User{}, "Finish the job", 2n}, M.TurnChanged{"t", M.Running{}, 2n}], M.State.empty())

def json(s: String) -> J.Json:
  Maybe.default(&2, J.Json, J.Json.parse(s), J.Null{})

def fold(+s: M.State, st: H.Step) -> M.State:
  M.State.replay(H.Queue.changes(st), s)

def stopped(os: List<&2, H.Out>) -> Bool:
  match os:
    case Nil{}: False{}
    case H.AgentStop{_} <> _: True{}
    case H.WatchStop{_, _, _, _, _} <> _: True{}
    case _ <> t: stopped(t)

def limit(+reset: Nat) -> String:
  "{\"type\":\"turn.failed\",\"error\":{\"code\":\"quota_exceeded\",\"message\":\"usage limit reached\",\"resets_at\":" ++ Nat.show(reset) ++ "}}"

def wait(+reset: Nat) -> M.State:
  fold(base(), H.Hub.agent_line(base(), 10n, "t", limit(reset)))

def bg(+args: String) -> M.State:
  M.State.replay(H.Bg.system(base(), "t", 10n, json("{\"subtype\":\"task_started\",\"task_type\":\"monitor_mcp\",\"task_id\":\"m\",\"description\":\"fixture\"" ++ args ++ "}")), base())

def changed(st: H.Step) -> Nat:
  List.length(&2, M.Change, H.Queue.changes(st))

def starts(os: List<&2, H.Out>) -> Nat:
  match os:
    case Nil{}: 0n
    case H.AgentStart{_, _, _, _, _, _, _} <> rest: 1n+starts(rest)
    case H.CodexTurn{_, _, _, _, _, _} <> rest: 1n+starts(rest)
    case _ <> rest: starts(rest)

def two() -> M.State:
  +first = bg("")
  +both = M.State.replay(H.Bg.system(first, "t", 10n, json("{\"subtype\":\"task_started\",\"task_type\":\"monitor_mcp\",\"task_id\":\"m2\",\"description\":\"second fixture\"}")), first)
  M.State.apply(both, M.TurnChanged{"t", M.Completed{}, 11n})

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("default work timeout", Nat.show(H.WorkTimeout.seconds(base(), J.Null{})), "1200")
    C.Check.str("explicit timeout opt out", Nat.show(H.WorkTimeout.seconds(base(), json("{\"timeoutSec\":0}"))), "0")
    C.Check.str("malformed timeout keeps default", Nat.show(H.WorkTimeout.seconds(base(), json("{\"timeoutSec\":-1}"))), "1200")
    C.Check.str("configured timeout", Nat.show(H.WorkTimeout.seconds(M.State.replay([M.SettingSet{"work.timeout.seconds", "60", 3n}], base()), J.Null{})), "60")
    C.Check.str("quota persists with fallback disabled", Bool.show(H.ProviderResume.interrupted(M.Thread.turn(H.Hub.thread(wait(100n), "t")))), "True")
    C.Check.str("quota never kills", Bool.show(stopped(H.Step.outs_of(H.Hub.agent_line(base(), 10n, "t", limit(100n))))), "False")
    C.Check.str("no resume before reset", Nat.show(changed(H.ProviderResume.tick(wait(100n), 99n))), "0")
    C.Check.str("resume at reset", Bool.show(H.Hub.running(H.Hub.thread(fold(wait(100n), H.ProviderResume.tick(wait(100n), 100n)), "t"))), "True")
    +resumed : M.State = fold(wait(100n), H.ProviderResume.tick(wait(100n), 100n))
    C.Check.str("resume once", Nat.show(changed(H.ProviderResume.tick(resumed, 101n))), "0")
    +paused : M.State = fold(wait(100n), H.Hub.interrupt(wait(100n), 20n, 0, "1", "t"))
    C.Check.str("explicit Stop cancels automatic quota recovery", Nat.show(changed(H.ProviderResume.tick(paused, 101n))), "0")
    C.Check.str("unknown reset never guessed", Nat.show(changed(H.ProviderResume.tick(wait(0n), 9000n))), "0")
    C.Check.str("expired reset never loops", Nat.show(changed(H.ProviderResume.tick(wait(9n), 9000n))), "0")
    C.Check.str("archive never resumes", Nat.show(changed(H.ProviderResume.tick(M.State.replay([M.ThreadArchived{"t", True{}, 20n}], wait(100n)), 101n))), "0")
    C.Check.str("busy turn never resumed", Nat.show(changed(H.ProviderResume.tick(M.State.replay([M.TurnChanged{"t", M.Running{}, 20n}], wait(100n)), 101n))), "0")
    C.Check.str("monitor default deadline", M.Setting.get(bg(""), "work.deadline.t.m", ""), "1210")
    C.Check.str("monitor before deadline", Nat.show(changed(H.WorkTimeout.tick(bg(""), 1209n))), "0")
    +timed : H.Step = H.WorkTimeout.tick(bg(""), 1210n)
    C.Check.str("monitor timeout reported", Bool.show(Nat.is_gt(changed(timed), 0n)), "True")
    C.Check.str("monitor timeout never kills", Bool.show(stopped(H.Step.outs_of(timed))), "False")
    +post : M.State = fold(bg(""), timed)
    C.Check.str("monitor remains owned", Nat.show(M.Work.busy(post, "t")), "1")
    C.Check.str("monitor timeout reported once", Nat.show(changed(H.WorkTimeout.tick(post, 1300n))), "0")
    C.Check.str("monitor opt out", Nat.show(changed(H.WorkTimeout.tick(bg(",\"timeoutSec\":0"), 9000n))), "0")
    +locked : M.State = fold(base(), H.Locks.work.start(base(), 10n, "t", json("{\"command\":\"fixture\",\"lock\":\"test\"}"), True{}))
    +wid : String = H.Watch.id(base(), "t")
    +lockedpost : M.State = fold(locked, H.Watch.end(locked, 1210n, "t", wid, "timeout", ""))
    C.Check.str("locked timeout persists cleared deadline", Nat.show(H.Watch.deadline(H.Watch.find(M.Work.watch(lockedpost, "t"), wid))), "0")
    C.Check.str("locked timeout reported once", Nat.show(changed(H.Watch.end(lockedpost, 1211n, "t", wid, "timeout", ""))), "0")
    C.Check.str("locked timeout retains lock", Bool.show(H.Locks.work.has(lockedpost, wid)), "True")
    +twotimeouts : H.Step = H.WorkTimeout.tick(two(), 1210n)
    C.Check.str("simultaneous timeouts start their parent once", Nat.show(starts(H.Step.outs_of(twotimeouts))), "1")
    +twopost : M.State = fold(two(), twotimeouts)
    C.Check.str("both timeout reports recorded", M.Setting.get(twopost, "work.deadline.t.m.noted", "") ++ M.Setting.get(twopost, "work.deadline.t.m2.noted", ""), "yesyes")
