import Base
import ./check.bend as C
import ./watchdog_test.bend as T
import ./liveness_recovery_test.bend as R
import ../src/core/watchdog.bend as W
import ../src/core/liveness.bend as L
import ../src/core/model.bend as M
import ../src/core/hub.bend as B
import ../src/core/signal.bend as SG

# A persisted idle owner with old subscriptions reproduces the live bar:
# every watchdog nudge used Lr.send, starting a turn and reopening its shelf.
def passive(turn: M.TurnState) -> M.State:
  M.State.replay([M.TurnChanged{"worker", turn, 11n}, M.ThreadSettled{"worker", False{}, 12n},
    M.WakeSet{"file", "worker", "file", "/tmp/future-result", ["changed"], True{}, "keep notification", 13n},
    M.WakeSet{"work", "worker", "work", "old-work", ["end"], True{}, "old work", 13n},
    M.WakeSet{"thread", "worker", "thread", "idle", ["end"], False{}, "keep subscription", 13n},
    M.SettingSet{L.Liveness.key("worker"), L.Liveness.write(L.Live{10n, 0n, False{}, False{}, "old output"}), 1270n}], T.base())

def stale(s: M.State) -> M.State:
  M.State.apply(s, M.SettingSet{L.Liveness.key("worker"), L.Liveness.write(L.Live{10n, 1210n, True{}, True{}, "old output"}), 1270n})

def count(s: M.State) -> String:
  Nat.show(T.notice_count(W.Watchdog.tick(s, 2000n), "worker"))

def post(+s: M.State) -> M.State:
  T.apply(s, W.Watchdog.tick(s, 2000n))

def work(s: M.State) -> M.State:
  M.State.apply(s, M.WorkSet{"worker", "watch", "w-live\twatching\t\tActive monitor\twatch\trun\tcmd\t-\t0", 14n})

def settled(sh: M.Shelf) -> Bool:
  match sh:
    case M.Settled{}: True{}
    case _: False{}

def RetryOpen(xs: List<&2, M.Ask>) -> Bool:
  match xs:
    case Nil{}: False{}
    case M.Ask{_, _, _, _, _, _, open, _} <> rest: Bool.or(open, RetryOpen(rest))

def main() -> IO(Unit):
  do IO<Unit>:
    +done : M.State = passive(M.Completed{})
    C.Check.str("completed owner with stale subscriptions gets no nudge", count(done), "0")
    C.Check.str("failed owner with stale subscriptions gets no nudge", count(passive(M.Failed{})), "0")
    C.Check.str("interrupted owner with stale subscriptions stays stopped", count(passive(M.Interrupted{})), "0")
    C.Check.str("idle owner with passive subscriptions gets no nudge", count(passive(M.Idle{})), "0")
    C.Check.str("stale stall clears on reconciliation tick", Bool.show(SG.Signal.display.stalled(post(stale(done)), "worker")), "False")
    C.Check.str("reconciliation keeps terminal status", Bool.show(B.Hub.running(B.Hub.thread(post(done), "worker"))), "False")
    C.Check.str("reconciliation leaves settled shelf intact", Bool.show(settled(M.Shelf.of(B.Hub.thread(post(done), "worker")))), "True")
    C.Check.str("all subscriptions preserved", String.join(M.Wake.ids(post(done), "wakes.by.worker"), ","), String.join(M.Wake.ids(done, "wakes.by.worker"), ","))
    C.Check.str("repeat tick has no extra state or notices", Nat.show(List.length(&2, M.Change, B.Queue.changes(W.Watchdog.tick(post(done), 2060n)))), "0")
    # Replaying saved events after restart gives the same passive result.
    +replayed : M.State = M.State.replay([M.TurnChanged{"worker", M.Completed{}, 15n}], post(done))
    C.Check.str("restart replay never turns subscription into work", count(replayed), "0")
    +monitor : M.State = work(done)
    C.Check.str("completed owner with real monitor stays watched", Bool.show(W.Watchdog.active(monitor, B.Hub.thread(monitor, "worker"))), "True")
    C.Check.str("monitor stays owned after watchdog tick", Nat.show(M.Work.busy(post(monitor), "worker")), "1")
    C.Check.str("watchdog emits no cancellation", Nat.show(T.stop_count(W.Watchdog.tick(monitor, 2000n))), "0")
    +stoppedWork : M.State = work(passive(M.Interrupted{}))
    C.Check.str("interrupted monitor owner is not resumed by nudge", count(stoppedWork), "0")
    C.Check.str("interrupted owner keeps running monitor", Nat.show(M.Work.busy(post(stoppedWork), "worker")), "1")
    +approval : M.State = M.State.replay([M.AskOpened{"approval", "worker", "approval", "shell", "Need consent", "", 14n}, M.PendingChanged{"worker", 1n, 14n}], T.base())
    C.Check.str("running provider awaiting approval gets no nudge", count(approval), "0")
    C.Check.str("approval remains open", Bool.show(RetryOpen(M.Asks.all(post(approval)))), "True")
    +answered : M.State = T.apply(approval, W.Watchdog.after(approval, 2000n, B.Step{[M.AskClosed{"approval", "yes", 2000n}, M.PendingChanged{"worker", 0n, 2000n}], Nil{}}))
    C.Check.str("approval resolution starts fresh execution clock", Nat.show(L.Liveness.last(W.Watchdog.live(answered, "worker"))), "2000")
    C.Check.str("old human wait does not cause immediate nudge", Nat.show(T.notice_count(W.Watchdog.tick(answered, 2001n), "worker")), "0")
    C.Check.str("running turn after approval remains watched", Bool.show(W.Watchdog.active(answered, B.Hub.thread(answered, "worker"))), "True")
    +askOnly : M.State = M.State.apply(T.base(), M.AskOpened{"approval", "worker", "approval", "shell", "Need consent", "", 14n})
    C.Check.str("open ask guards stale pending count", count(askOnly), "0")
    +queued : M.State = M.State.replay([M.TurnQueued{"worker", "old", "Preserved context", 14n}], passive(M.Interrupted{}))
    C.Check.str("held queue is not running execution", count(queued), "0")
    C.Check.str("held queued message preserved", M.Queue.of(post(queued), "worker"), M.Queue.of(queued, "worker"))
    +starting : M.State = M.State.replay([M.ThreadCreated{"start", "p", "Queued worktree", M.Worktree{}, "branch", "", "codex", "", 14n}, M.TurnQueued{"start", "go", "Run", 14n}], T.base())
    C.Check.str("worktree creation queue remains watched", Bool.show(W.Watchdog.active(starting, B.Hub.thread(starting, "start"))), "True")
    C.Check.str("ordinary running turn still gets one nudge", Nat.show(T.notice_count(W.Watchdog.tick(T.base(), 1210n), "worker")), "1")
    C.Check.str("watched result never restarts interrupted owner", Nat.show(R.starts(B.Step.outs_of(B.Watch.end(stoppedWork, 2000n, "worker", "w-live", "exit 0", "done")))), "0")
    +expired : M.State = M.State.replay([M.TurnChanged{"t", M.Interrupted{}, 12n}], R.bg(""))
    +timeout : B.Step = W.Watchdog.tick(expired, 1210n)
    C.Check.str("expired background row never restarts interrupted owner", Nat.show(R.starts(B.Step.outs_of(timeout))), "0")
    C.Check.str("timeout still recorded for interrupted owner", M.Setting.get(T.apply(expired, timeout), "work.deadline.t.m.noted", ""), "yes")
    C.Check.str("expired background work stays owned", Nat.show(M.Work.busy(T.apply(expired, timeout), "t")), "1")
    C.Check.str("timeout keeps owner interrupted", Bool.show(B.ProviderResume.interrupted(M.Thread.turn(B.Hub.thread(T.apply(expired, timeout), "t")))), "True")
    C.Check.str("timeout notification retained in history", Bool.show(B.Watch.told(M.State.mine(T.apply(expired, timeout), "t"), "timed out")), "True")
    +ended : M.State = M.State.apply(monitor, M.WorkSet{"worker", "watch", "", 16n})
    C.Check.str("monitor completion retires execution eligibility", Bool.show(W.Watchdog.active(ended, B.Hub.thread(ended, "worker"))), "False")
    C.Check.str("completed monitor clears old stall", Bool.show(SG.Signal.display.stalled(post(stale(ended)), "worker")), "False")
