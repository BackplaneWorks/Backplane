import Base
import ./check.bend as C
import ../src/core/model.bend as M
import ../src/core/signal.bend as S
import ../src/core/i18n.bend as I

def made(+id: String) -> M.Change:
  M.ThreadCreated{id, "p", id, M.Local{}, "", "", "claude", "", 1n}

def state(cs: List<&2, M.Change>) -> M.State:
  M.State.replay(List.append(&2, M.Change, [made("p"), made("c"), made("g")], cs), M.State.empty())

def signal(+st: M.State, +id: String) -> String:
  S.Signal.name(S.Signal.of(st, M.State.thread.get(st, id)))

def check(name: String, cs: List<&2, M.Change>, want: String) -> IO(Unit):
  C.Check.str(name, signal(state(cs), "p"), want)

# what the State cell says (Signal.shown): the word, then its sentence
def shown.w(+st: M.State, +id: String, +n: Nat, +w: String) -> String:
  w ++ " | " ++ S.Signal.label.shown(I.En{}, n, w, S.Signal.why.if(String.eq(w, "failed"), st, id))

def shown.t(+st: M.State, +id: String, t: M.Thread) -> String:
  match t:
    case M.Thread{_, _, _, _, _, +turn, +pending, _, _, _, _, _, _, _, _, _, _, _, _, _, _}:
      shown.w(st, id, S.Signal.of(st, M.State.thread.get(st, id)), S.Signal.shown(S.Signal.of(st, M.State.thread.get(st, id)), turn, pending))

def shown(+st: M.State, +id: String) -> String:
  shown.t(st, id, M.State.thread.get(st, id))

def cell(name: String, cs: List<&2, M.Change>, want: String) -> IO(Unit):
  C.Check.str(name, shown(state(cs), "p"), want)

def asked(+at: Nat) -> M.Change:
  M.MessagePosted{"p", "u" ++ Nat.show(at), M.User{}, "go", at}

def failed(+text: String, +at: Nat) -> M.Change:
  M.ActivityLogged{"p", "e" ++ Nat.show(at), "error", "codex", text, at}

def delegated() -> M.Change:
  M.TaskDelegated{"task", "p", "c", 1n, "claude", "", 2n}

def ask(+id: String, kind: String) -> M.Change:
  M.AskOpened{"ask", id, kind, "tool", "Choose", "", 4n}

def watch(+word: String) -> M.Change:
  M.WorkSet{"p", "watch", "w\t" ++ word ++ "\t\tbuild\twatch", 4n}

def bg(+word: String) -> M.Change:
  M.WorkSet{"p", "bg", "s\t" ++ word ++ "\t\tinspect\tsubagent", 4n}

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("blue active + red interaction bits retained", S.Signal.label(I.En{}, 3n), "Blocked: needs interaction; work also active")
    cell("a stopped turn says stopped", [M.TurnChanged{"p", M.Running{}, 2n}, M.TurnChanged{"p", M.Interrupted{}, 3n}], "stopped | Stopped: nothing is running")
    cell("a stopped and settled thread says settled", [M.TurnChanged{"p", M.Interrupted{}, 3n}, M.ThreadSettled{"p", True{}, 4n}], "settled | Settled")
    cell("a stopped thread with a question says needs you", [M.TurnChanged{"p", M.Interrupted{}, 3n}, ask("p", "input")], "blocked | Blocked: needs interaction")
    cell("a failed turn says failed and why", [asked(2n), failed("old trouble", 3n), asked(4n), failed("codex exited (exit 127)", 5n), M.TurnChanged{"p", M.Failed{}, 5n}], "failed | Failed: codex exited (exit 127)")
    cell("only the last turn's error is why", [asked(2n), failed("old trouble", 3n), asked(4n), M.TurnChanged{"p", M.Failed{}, 5n}], "failed | Failed")
    cell("a failure with an approval waiting needs you", [M.TurnChanged{"p", M.Failed{}, 5n}, M.PendingChanged{"p", 1n, 6n}], "blocked | Blocked: needs interaction")
    cell("a finished turn says finished", [M.TurnChanged{"p", M.Completed{}, 5n}], "finished | Finished: ready for you to look at")
    cell("a new thread is idle", Nil{}, "inactive | Inactive: no confirmed active work")
    check("idle is gray", Nil{}, "inactive")
    check("queued without worker is gray", [M.TurnQueued{"p", "m", "go", 2n}], "inactive")
    check("running agent is blue", [M.TurnChanged{"p", M.Running{}, 2n}], "active")
    check("pending normal tool remains blue", [M.TurnChanged{"p", M.Running{}, 2n}, M.ActivityLogged{"p", "tool", "tool", "shell", "pending", 3n}], "active")
    check("complete is green until settled", [M.TurnChanged{"p", M.Completed{}, 2n}], "finished")
    check("complete then settled is gray", [M.TurnChanged{"p", M.Completed{}, 2n}, M.ThreadSettled{"p", True{}, 4n}], "settled")
    check("complete with a message queued is not finished", [M.TurnChanged{"p", M.Completed{}, 2n}, M.TurnQueued{"p", "m", "go", 3n}], "inactive")
    check("complete with work running waits", [M.TurnChanged{"p", M.Completed{}, 2n}, watch("watching")], "waiting")
    check("complete with an ask is blocked", [M.TurnChanged{"p", M.Completed{}, 2n}, ask("p", "input")], "blocked")
    check("complete parent of a running child waits", [delegated(), M.TurnChanged{"p", M.Completed{}, 3n}, M.TurnChanged{"c", M.Running{}, 4n}], "waiting")
    check("a child whose task is not closed keeps its parent waiting", [delegated(), M.TaskStateSet{"task", M.TaskRunning{}, "", 3n}, M.TurnChanged{"c", M.Completed{}, 4n}], "waiting")
    check("interrupted is gray", [M.TurnChanged{"p", M.Interrupted{}, 2n}], "inactive")
    # a coordinator whose turn ended while what it started is out waits,
    # never finished (green) before the last of it is done
    check("finished turn with a delegated task out waits", [delegated(), M.TurnChanged{"p", M.Completed{}, 3n}], "waiting")
    check("finished turn, its subagent between turns: waiting", [delegated(), M.TaskStateSet{"task", M.TaskRunning{}, "", 3n}, M.TurnChanged{"p", M.Completed{}, 3n}, M.TurnChanged{"c", M.Completed{}, 4n}], "waiting")
    check("finished turn, its background agent done but owed: waiting", [M.TurnChanged{"p", M.Completed{}, 2n}, bg(M.Bg.ended.mark() ++ "completed")], "waiting")
    check("its task done: finished again", [delegated(), M.TurnChanged{"p", M.Completed{}, 3n}, M.TaskStateSet{"task", M.TaskDone{}, "done", 4n}], "finished")
    check("a launched thread done: finished again", [M.ThreadLaunched{"c", "p", True{}, 2n}, M.TurnChanged{"p", M.Completed{}, 2n}, M.TurnChanged{"c", M.Running{}, 3n}, M.TurnChanged{"c", M.Completed{}, 4n}], "finished")
    check("a running turn with work out is working, not waiting", [delegated(), M.TurnChanged{"p", M.Running{}, 3n}], "active")
    check("a deleted thread never waits", [delegated(), M.TurnChanged{"p", M.Completed{}, 3n}, M.ThreadDeleted{"p", 4n}], "inactive")
    cell("the cell says waiting", [delegated(), M.TurnChanged{"p", M.Completed{}, 3n}], "waiting | Waiting: on subagents or work it started")
    C.Check.str("waiting on work at it is active, never blocked or finished", Nat.show(S.Signal.bits(6n)) ++ "/" ++ S.Signal.name(6n), "1/waiting")
    C.Check.str("waiting on work not confirmed has no activity bit", Nat.show(S.Signal.bits(7n)) ++ "/" ++ S.Signal.name(7n), "0/waiting")
    C.Check.str("a queued subagent: waiting without the activity bit", Nat.show(S.Signal.of(state([delegated(), M.TurnChanged{"p", M.Completed{}, 3n}]), M.State.thread.get(state(Nil{}), "p"))), "7")
    C.Check.str("a running child: waiting with it", Nat.show(S.Signal.of(state([delegated(), M.TurnChanged{"p", M.Completed{}, 3n}, M.TurnChanged{"c", M.Running{}, 4n}]), M.State.thread.get(state(Nil{}), "p"))), "6")
    check("failure requires recovery red", [M.TurnChanged{"p", M.Failed{}, 2n}], "blocked")
    check("queued subagent: the parent waits", [delegated()], "waiting")
    check("actual running child keeps a yielded parent waiting", [delegated(), M.TurnChanged{"p", M.Completed{}, 3n}, M.TurnChanged{"c", M.Running{}, 4n}], "waiting")
    check("TaskRunning token with idle child: the parent waits", [delegated(), M.TaskStateSet{"task", M.TaskRunning{}, "", 3n}], "waiting")
    check("yielded child with its task open: the parent waits", [delegated(), M.TaskStateSet{"task", M.TaskRunning{}, "", 3n}, M.TurnChanged{"c", M.Running{}, 4n}, M.TurnChanged{"c", M.Completed{}, 5n}], "waiting")
    check("descendant running owned process propagates", [delegated(), M.TaskDelegated{"grand", "c", "g", 2n, "claude", "", 3n}, M.WorkSet{"g", "bg", "s\tworking\t\tinspect\tsubagent", 4n}], "waiting")
    check("closed task no longer owns child", [delegated(), M.TaskStateSet{"task", M.TaskDone{}, "done", 3n}, M.TurnChanged{"c", M.Running{}, 4n}], "inactive")
    check("launched thread keeps its launcher waiting", [M.ThreadLaunched{"c", "p", True{}, 2n}, M.TurnChanged{"p", M.Completed{}, 2n}, M.TurnChanged{"c", M.Running{}, 3n}], "waiting")
    check("launch ownership cycle terminates and propagates", [M.ThreadLaunched{"c", "p", True{}, 2n}, M.ThreadLaunched{"p", "c", True{}, 3n}, M.TurnChanged{"c", M.Running{}, 4n}], "waiting")
    check("owned background agent: the turn over, waiting", [bg("working")], "waiting")
    check("owned background agent while the turn runs is blue", [M.TurnChanged{"p", M.Running{}, 2n}, bg("working")], "active")
    check("ended agent whose outcome is owed: waiting", [bg(M.Bg.ended.mark() ++ "completed")], "waiting")
    check("owned work launching: waiting", [watch("launching")], "waiting")
    check("owned work queued for lock: waiting", [watch("waiting for lock")], "waiting")
    check("owned work running, the turn over: waiting", [watch("watching")], "waiting")
    check("owned work cancelling: still waiting", [watch("cancelling")], "waiting")
    check("unknown watchdog: waiting until delivered", [watch(M.Bg.ended.mark() ++ "unknown")], "waiting")
    check("ended work awaits delivery: waiting", [watch(M.Bg.ended.mark() ++ "completed")], "waiting")
    check("approval beats active worker red", [M.TurnChanged{"p", M.Running{}, 2n}, ask("p", "approval"), watch("watching")], "blocked")
    check("question red", [ask("p", "input")], "blocked")
    check("plan red", [ask("p", "plan")], "blocked")
    check("browser human help red", [ask("p", "browser")], "blocked")
    check("owned child asks red", [delegated(), ask("c", "input")], "blocked")
    check("answered but inactive gray", [ask("p", "input"), M.AskClosed{"ask", "yes", 5n}], "inactive")
    check("answered and resumed blue", [ask("p", "input"), M.AskClosed{"ask", "yes", 5n}, M.TurnChanged{"p", M.Running{}, 6n}], "active")
    check("credential hold red", [M.SettingSet{"auth.wait.p", "claude", 4n}], "blocked")
    check("credential resumed blue", [M.SettingSet{"auth.wait.p", "claude", 4n}, M.SettingSet{"auth.wait.p", "", 5n}, M.TurnChanged{"p", M.Running{}, 6n}], "active")
    check("fallback hold red", [M.TurnQueued{"p", "fallback:held:1", "go", 4n}], "blocked")
    C.Check.str("blocked process alone is not separate active work", Nat.show(S.Signal.of(state([M.TurnChanged{"p", M.Running{}, 2n}, ask("p", "input")]), M.State.thread.get(state(Nil{}), "p"))), "2")
    C.Check.str("blocked plus owned process retains active bit", Nat.show(S.Signal.of(state([M.TurnChanged{"p", M.Running{}, 2n}, ask("p", "input"), watch("watching")]), M.State.thread.get(state(Nil{}), "p"))), "3")
    check("fallback resumed blue", [M.TurnQueued{"p", "fallback:held:1", "go", 4n}, M.QueueTaken{"p", 5n}, M.TurnChanged{"p", M.Running{}, 6n}], "active")
    check("old slow work never implies blocked", [M.TurnChanged{"p", M.Running{}, 2n}, M.ActivityLogged{"p", "later", "tool", "shell", "pending", 2000000000n}], "active")
    check("settled gray", [M.ThreadSettled{"p", True{}, 4n}], "settled")
    check("automatic settled gray", [M.ThreadSettled{"p", False{}, 4n}], "settled")
    check("unsettled gray", [M.ThreadSettled{"p", True{}, 4n}, M.ThreadUnsettled{"p", 5n}], "inactive")
    check("settled cannot hide running work", [M.ThreadSettled{"p", True{}, 4n}, watch("watching")], "waiting")
    check("settled cannot hide interaction", [M.ThreadSettled{"p", True{}, 4n}, ask("p", "input")], "blocked")
