import Base
import ./check.bend as C
import ../src/core/watchdog.bend as W
import ../src/core/liveness.bend as L
import ../src/core/model.bend as M
import ../src/core/hub.bend as B
import ../src/core/json.bend as J

def shown(b: Bool) -> String:
  Bool.pick(String, b, "true", "false")

def base() -> M.State:
  M.State.replay([M.ProjectCreated{"p", "project", "/tmp/project", 1n},
    M.ThreadCreated{"worker", "p", "worker", M.Local{}, "", "", "codex", "", 1n},
    M.ThreadCreated{"launcher", "p", "launcher", M.Local{}, "", "", "codex", "", 1n},
    M.ThreadCreated{"idle", "p", "idle", M.Local{}, "", "", "codex", "", 1n},
    M.ThreadLaunched{"worker", "launcher", True{}, 1n},
    M.TurnChanged{"worker", M.Running{}, 10n},
    M.SettingSet{L.Liveness.key("worker"), L.Liveness.write(L.Live{10n, 0n, False{}, False{}, "blocking shell"}), 10n}], M.State.empty())

def apply(+s: M.State, step: B.Step) -> M.State:
  M.State.replay(B.Queue.changes(step), s)

def tick(+s: M.State, +now: Nat) -> M.State:
  apply(s, W.Watchdog.after(s, now, W.Watchdog.tick(s, now)))

def nudged() -> M.State:
  tick(base(), 1210n)

def stalled() -> M.State:
  tick(nudged(), 1270n)

def harmful(os: List<&2, B.Out>) -> Nat:
  match os:
    case Nil{}: 0n
    case B.AgentStop{_} <> rest: 1n+harmful(rest)
    case B.Restart{} <> rest: 1n+harmful(rest)
    case B.WatchStop{_, _, _, _, _} <> rest: 1n+harmful(rest)
    case B.TermKill{_} <> rest: 1n+harmful(rest)
    case B.AgentSend{_, +text} <> rest: Nat.add(Bool.pick(Nat, String.contains(text, "interrupt"), 1n, 0n), harmful(rest))
    case _ <> rest: harmful(rest)

def stop_count(step: B.Step) -> Nat:
  match step:
    case B.Step{_, os}: harmful(os)

def notice_count(step: B.Step, th: String) -> Nat:
  B.Lr.count(B.Queue.changes(step), th)

def json(text: String) -> J.Json:
  Maybe.default(&2, J.Json, J.Json.parse(text), J.Null{})

def observed(+s: M.State, +now: Nat, +th: String) -> M.State:
  apply(s, W.Watchdog.observed([J.Json.obj([J.KV{"thread", J.Str{th}}, J.KV{"at", J.Json.nat(now)}, J.KV{"reason", J.Str{"work output grew: w-running"}}])], s, now, B.Step.none()))

def dependency() -> M.State:
  M.State.replay([M.ThreadCreated{"child", "p", "child", M.Local{}, "", "", "codex", "", 10n},
    M.TaskDelegated{"task", "worker", "child", 1n, "codex", "", 10n}, M.TurnChanged{"child", M.Running{}, 10n},
    M.TurnChanged{"worker", M.Completed{}, 11n}], base())

def waiting() -> M.State:
  M.State.apply(base(), M.WakeSet{"wait", "idle", "thread", "worker", ["end"], True{}, "Waiting for worker", 10n})

def asking() -> M.State:
  M.State.replay([M.AskOpened{"ask", "idle", "input", "question", "Need the user", "", 10n}, M.PendingChanged{"idle", 1n, 10n}], base())

def main() -> IO(Unit):
  do IO<Unit>:
    +legacy : M.State = M.State.apply(base(), M.TurnQueued{"worker", "old", "legacy context", 11n})
    +legacyNudge : B.Step = W.Watchdog.tick(legacy, 1210n)
    C.Check.str("legacy queue cannot make watchdog interrupt work", Nat.show(stop_count(legacyNudge)), "0")
    C.Check.str("watchdog preserves legacy queued context", M.Queue.of(apply(legacy, legacyNudge), "worker"), M.Queue.of(legacy, "worker"))
    +busyLauncher : M.State = M.State.replay([M.TurnChanged{"launcher", M.Running{}, 10n}, M.TurnQueued{"launcher", "old-parent", "parent context", 11n}], nudged())
    C.Check.str("launcher notice cannot interrupt a legacy queued turn", Nat.show(stop_count(W.Watchdog.tick(busyLauncher, 1270n))), "0")
    +quota : M.State = M.State.apply(base(), M.SettingSet{"provider.resume.idle", "{\"reset\":0}", 10n})
    C.Check.str("a provider hold with no reset waits, not stalled", shown(W.Watchdog.active(quota, B.Hub.thread(quota, "idle"))), "false")
    +reset : M.State = M.State.apply(base(), M.SettingSet{"provider.resume.idle", "{\"reset\":5000}", 10n})
    C.Check.str("known provider reset may wait for scheduled recovery", shown(W.Watchdog.active(reset, B.Hub.thread(reset, "idle"))), "false")
    # a turn that ended on the session limit with its busy work still out
    # waits for the reset: no nudge, no stall
    +heldBusy : M.State = M.State.replay([M.WorkSet{"worker", "bg", "b1\tsleep\tc1\tsleep\tshell", 11n}, M.TurnChanged{"worker", M.Interrupted{}, 12n},
      M.SettingSet{"provider.resume.worker", "{\"target\":\"claude/x\",\"reset\":0,\"reason\":\"You've hit your session limit\"}", 12n}], base())
    C.Check.str("held on the limit with busy work: not watched", shown(W.Watchdog.active(heldBusy, B.Hub.thread(heldBusy, "worker"))), "false")
    C.Check.str("held on the limit: no nudge", Nat.show(L.Liveness.nudged(W.Watchdog.live(tick(heldBusy, 1210n), "worker"))), "0")
    C.Check.str("held on the limit: never stalled", shown(L.Liveness.stalled(W.Watchdog.live(tick(tick(heldBusy, 1210n), 1270n), "worker"))), "false")
    C.Check.str("held on the limit: no notice", Nat.show(notice_count(W.Watchdog.tick(tick(heldBusy, 1210n), 1270n), "launcher")), "0")
    # ephemeral mail runs and coordinator back offices are never watched
    +runs : M.State = M.State.replay([M.ThreadCreated{"run1793393228-869127", "p", "run", M.Local{}, "", "", "claude", "", 10n}, M.TurnChanged{"run1793393228-869127", M.Running{}, 10n},
      M.ThreadCreated{"mailcoord-b1-turn1", "p", "coord", M.Local{}, "", "", "claude", "", 10n}, M.TurnChanged{"mailcoord-b1-turn1", M.Running{}, 10n}], base())
    C.Check.str("a mail run is not watched", shown(W.Watchdog.active(runs, B.Hub.thread(runs, "run1793393228-869127"))), "false")
    C.Check.str("a coordinator back office is not watched", shown(W.Watchdog.active(runs, B.Hub.thread(runs, "mailcoord-b1-turn1"))), "false")
    C.Check.str("a mail run is never nudged or stalled", L.Liveness.write(W.Watchdog.live(tick(tick(runs, 1210n), 1270n), "run1793393228-869127")), L.Liveness.write(W.Watchdog.live(runs, "run1793393228-869127")))
    C.Check.str("an ordinary thread still is", shown(L.Liveness.stalled(W.Watchdog.live(tick(tick(runs, 1210n), 1270n), "worker"))), "true")
    C.Check.str("ephemeral ids", String.join([shown(B.Ephemeral.is(runs, "run1")), shown(B.Ephemeral.is(runs, "mailcoord-x")), shown(B.Ephemeral.is(runs, "t1"))], ","), "true,true,false")
    C.Check.str("before gap is unchanged", Nat.show(L.Liveness.nudged(W.Watchdog.live(tick(base(), 1209n), "worker"))), "0")
    C.Check.str("at gap one nudge", Nat.show(notice_count(W.Watchdog.tick(base(), 1210n), "worker")), "1")
    C.Check.str("nudge has not stalled yet", shown(L.Liveness.stalled(W.Watchdog.live(nudged(), "worker"))), "false")
    C.Check.str("synthetic delivery cannot count as progress", Nat.show(L.Liveness.last(W.Watchdog.live(nudged(), "worker"))), "10")
    C.Check.str("nudge retains episode", Nat.show(L.Liveness.nudged(W.Watchdog.live(nudged(), "worker"))), "1210")
    C.Check.str("grace has not elapsed", shown(L.Liveness.stalled(W.Watchdog.live(tick(nudged(), 1269n), "worker"))), "false")
    C.Check.str("stall after grace", shown(L.Liveness.stalled(W.Watchdog.live(stalled(), "worker"))), "true")
    C.Check.str("launcher notified once", Nat.show(notice_count(W.Watchdog.tick(nudged(), 1270n), "launcher")), "1")
    C.Check.str("no repeat nudge", Nat.show(notice_count(W.Watchdog.tick(stalled(), 1330n), "worker")), "0")
    C.Check.str("no repeat launcher notice", Nat.show(notice_count(W.Watchdog.tick(stalled(), 1330n), "launcher")), "0")
    C.Check.str("watchdog never stops busy work", Nat.show(stop_count(W.Watchdog.tick(nudged(), 1270n))), "0")
    C.Check.str("idle threads get no synthetic state", M.Setting.get(tick(base(), 1210n), L.Liveness.key("idle"), "absent"), "absent")
    C.Check.str("real output clears visible stall", shown(L.Liveness.stalled(W.Watchdog.live(apply(stalled(), W.Watchdog.progress(stalled(), 1300n, "worker", "tool output")), "worker"))), "false")
    C.Check.str("subagent output belongs to parent", W.Watchdog.owner("@sub:worker"), "worker")
    C.Check.str("provider pings cannot be progress", shown(W.Watchdog.output(J.Json.obj([J.KV{"type", J.Str{"ping"}}]))), "false")
    C.Check.str("quota polling cannot be progress", shown(W.Watchdog.output(J.Json.obj([J.KV{"type", J.Str{"rate_limit_event"}}]))), "false")
    C.Check.str("stream output is progress", shown(W.Watchdog.output(json("{\"type\":\"stream_event\",\"event\":{\"type\":\"content_block_delta\",\"delta\":{\"type\":\"text_delta\",\"text\":\"working\"}}}"))), "true")
    C.Check.str("provider echo of watchdog nudge is not progress", shown(W.Watchdog.output(json("{\"type\":\"user\",\"message\":{\"content\":[{\"type\":\"text\",\"text\":\"You appear stalled; report status\"}]}}"))), "false")
    C.Check.str("wrapped provider ping is not progress", shown(W.Watchdog.output(json("{\"type\":\"stream_event\",\"event\":{\"type\":\"ping\"}}"))), "false")
    C.Check.str("empty stream wrapper is not progress", shown(W.Watchdog.output(json("{\"type\":\"stream_event\"}"))), "false")
    C.Check.str("raw ACP delta is progress", shown(W.Watchdog.output(json("{\"type\":\"item.delta\",\"text\":\"new output\"}"))), "true")
    C.Check.str("empty ACP delta is not progress", shown(W.Watchdog.output(json("{\"type\":\"item.delta\",\"text\":\"\"}"))), "false")
    C.Check.str("completed tool result is progress", shown(W.Watchdog.output(json("{\"type\":\"user\",\"message\":{\"content\":[{\"type\":\"tool_result\",\"tool_use_id\":\"tool\",\"content\":\"done\"}]}}"))), "true")
    C.Check.str("raw output clears overdue episode immediately", Nat.show(L.Liveness.nudged(W.Watchdog.live(apply(nudged(), W.Watchdog.line(nudged(), 1211n, "worker", json("{\"type\":\"item.delta\",\"text\":\"continuing\"}"))), "worker"))), "0")
    C.Check.str("echo cannot clear overdue episode", Nat.show(L.Liveness.nudged(W.Watchdog.live(apply(nudged(), W.Watchdog.line(nudged(), 1211n, "worker", json("{\"type\":\"user\",\"message\":{\"content\":[{\"type\":\"text\",\"text\":\"You appear stalled\"}]}}"))), "worker"))), "1210")
    C.Check.str("late synthetic delivery cannot reseed gap", Nat.show(L.Liveness.last(W.Watchdog.live(apply(nudged(), W.Watchdog.after(nudged(), 1211n, B.Step{[M.TurnChanged{"worker", M.Running{}, 1211n}], Nil{}})), "worker"))), "10")
    C.Check.str("late queued watchdog message cannot reseed gap", Nat.show(L.Liveness.last(W.Watchdog.live(apply(nudged(), W.Watchdog.after(nudged(), 1211n, B.Step{[M.MessagePosted{"worker", M.Link.id("watchdog:worker", "lr-note"), M.User{}, "You appear stalled", 1211n}], Nil{}})), "worker"))), "10")
    C.Check.str("real user message starts a fresh episode", Nat.show(L.Liveness.last(W.Watchdog.live(apply(stalled(), W.Watchdog.after(stalled(), 1300n, B.Step{[M.MessagePosted{"worker", "user-request", M.User{}, "Please continue", 1300n}], Nil{}})), "worker"))), "1300")
    C.Check.str("owned work output clears visible stall", shown(L.Liveness.stalled(W.Watchdog.live(observed(stalled(), 1300n, "worker"), "worker"))), "false")
    C.Check.str("owned work output advances progress", Nat.show(L.Liveness.last(W.Watchdog.live(observed(stalled(), 1300n, "worker"), "worker"))), "1300")
    C.Check.str("idle parent with owned child remains watched", shown(W.Watchdog.active(dependency(), B.Hub.thread(dependency(), "worker"))), "true")
    C.Check.str("owned child progress advances idle parent", Nat.show(L.Liveness.last(W.Watchdog.live(observed(dependency(), 1300n, "child"), "worker"))), "1300")
    C.Check.str("child notice propagation never kills work", Nat.show(stop_count(W.Watchdog.progress(dependency(), 1300n, "child", "new child output"))), "0")
    C.Check.str("thread waiting only on another thread remains watched", shown(W.Watchdog.active(waiting(), B.Hub.thread(waiting(), "idle"))), "true")
    C.Check.str("thread waiting only on user remains watched", shown(W.Watchdog.active(asking(), B.Hub.thread(asking(), "idle"))), "true")
    C.Check.str("waiting thread initializes one progress baseline", Nat.show(L.Liveness.last(W.Watchdog.live(tick(waiting(), 10n), "idle"))), "10")
    C.Check.str("waiting thread eventually nudged", Nat.show(L.Liveness.nudged(W.Watchdog.live(tick(tick(waiting(), 10n), 1210n), "idle"))), "1210")
    C.Check.str("archived dependency is never nudged", Nat.show(notice_count(W.Watchdog.tick(M.State.apply(waiting(), M.ThreadArchived{"idle", True{}, 11n}), 2000n), "idle")), "0")
