import Base
import ./check.bend as C
import ../src/core/autosettle.bend as A
import ../src/core/model.bend as M
import ../src/core/hub.bend as B
import ../src/core/tickplan.bend as T
import ../src/core/locks.bend as L
import ../src/core/signal.bend as S
import ../src/core/client.bend as U

# All fixtures are synthetic. No hub, process, filesystem or live store.
def base(+worker: Bool, more: List<&2, M.Change>) -> M.State:
  M.State.replay(List.append(&2, M.Change,
    [M.ProjectCreated{"p", "Project", "/tmp/autosettle", 1n},
     M.ThreadCreated{"parent", "p", "Parent", M.Local{}, "", "", "codex", "", 1n},
     M.ThreadCreated{"t", "p", "Worker", M.Local{}, "", "", "codex", "", 1n},
     M.MessagePosted{"t", Bool.pick(String, worker, M.Link.id("parent", "start"), "human"), M.User{}, "Do the work", 2n},
     M.TurnChanged{"t", M.Running{}, 2n}, M.MessagePosted{"t", "answer", M.Assistant{}, "Done", 4n},
     M.TurnChanged{"t", M.Completed{}, 5n}],
    List.append(&2, M.Change, Bool.pick(List<&2, M.Change>, worker,
      [M.ThreadLaunched{"t", "parent", True{}, 1n}, M.SettingSet{"autosettle.report.t", "5", 5n}], Nil{}), more)), M.State.empty())

def due(+s: M.State, now: Nat) -> Bool:
  A.Auto.due(s, M.State.thread.get(s, "t"), now, A.Auto.index(s))

def expect(name: String, s: M.State, now: Nat, want: Bool) -> IO(Unit):
  C.Check.str(name, Bool.show(due(s, now)), Bool.show(want))

def committed(+s: M.State, +cs: List<&2, M.Change>) -> M.State:
  M.State.replay(B.Shelve.auto.fix(s, cs), s)

def party(id: String) -> L.LockParty:
  L.LockParty{id, "thread", "t", "Waiting for compiler", 6n, 0n}

def shelf(s: M.State) -> String:
  Bool.pick(String, B.AutoSettled.is(M.Shelf.of(M.State.thread.get(s, "t"))), "settled", "active")


def settled() -> M.State:
  M.State.replay(A.Auto.upgrade(base(True{}, Nil{}), 905n), base(True{}, Nil{}))


def without_history(+s: M.State) -> M.State:
  match s:
    case M.State{ps, ts, _, n, tasks, settings, queue, asks, devices, nav, _}:
      M.State{ps, ts, Nil{}, n, tasks, settings, queue, asks, devices, nav,
        [M.Ext{M.State.bots(s), Map.new(&2, List<&2, M.Entry>), [M.State.mail(s)], Map.new(&2, String)}]}

def main() -> IO(Unit):
  do IO<Unit>:
    expect("worker: full 15 minute grace", base(True{}, Nil{}), 904n, False{})
    expect("worker: due at boundary", base(True{}, Nil{}), 905n, True{})
    expect("bot worker: same rule", base(True{}, [M.BotSet{"bot", "Bot", "parent", 0n, "", 1n}]), 905n, True{})
    expect("human: unread never hidden after four hours", base(False{}, Nil{}), 50000n, False{})
    expect("human: read stays visible for longer grace", base(False{}, [M.SettingSet{"autosettle.read.t", "answer", 10n}]), 905n, False{})
    expect("human: read done after four hours", base(False{}, [M.SettingSet{"autosettle.read.t", "answer", 10n}]), 14405n, True{})
    expect("human: read and settle.done uses short grace", base(False{}, [M.SettingSet{"autosettle.read.t", "answer", 10n}, M.SettingSet{"settle.done", "on", 1n}]), 905n, True{})
    expect("worker: person chatting must read reply", base(True{}, [M.MessagePosted{"t", "human-again", M.User{}, "One more thing", 6n}, M.MessagePosted{"t", "new-answer", M.Assistant{}, "Done again", 7n}, M.TurnChanged{"t", M.Completed{}, 8n}, M.SettingSet{"autosettle.report.t", "8", 8n}]), 50000n, False{})
    expect("failed never auto-settles", base(True{}, [M.TurnChanged{"t", M.Failed{}, 6n}]), 50000n, False{})
    expect("interrupted never auto-settles", base(True{}, [M.TurnChanged{"t", M.Interrupted{}, 6n}]), 50000n, False{})
    expect("failed recovery completes normally", base(True{}, [M.TurnChanged{"t", M.Failed{}, 6n}, M.TurnChanged{"t", M.Running{}, 7n}, M.TurnChanged{"t", M.Completed{}, 8n}, M.SettingSet{"autosettle.report.t", "8", 8n}]), 908n, True{})
    expect("running never settles", base(True{}, [M.TurnChanged{"t", M.Running{}, 6n}]), 50000n, False{})
    expect("idle is not a completed turn", base(True{}, [M.TurnChanged{"t", M.Idle{}, 6n}]), 50000n, False{})
    expect("shell held", base(True{}, [M.WorkSet{"t", "bg", "job\tworking\tc\tShell\tshell", 6n}]), 50000n, False{})
    expect("background agent held", base(True{}, [M.WorkSet{"t", "bg", "job\tworking\tc\tAgent\tagent", 6n}]), 50000n, False{})
    expect("monitor held", base(True{}, [M.WorkSet{"t", "watch", "watch\twatching\t\tMonitor\twatch", 6n}]), 50000n, False{})
    expect("delegated task held", base(True{}, [M.TaskDelegated{"task", "t", "child", 1n, "codex", "", 6n}]), 50000n, False{})
    expect("completed child report not delivered held", base(True{}, [M.TaskDelegated{"task", "parent", "t", 1n, "codex", "", 1n}, M.TaskStateSet{"task", M.TaskDone{}, "Done", 6n}]), 50000n, False{})
    expect("delegated child report delivered", base(True{}, [M.TaskDelegated{"task", "parent", "t", 1n, "codex", "", 1n}, M.TaskStateSet{"task", M.TaskDone{}, "Done", 6n}, M.TaskDelivered{"task", 6n}]), 905n, True{})
    expect("launched owned worker running held", base(True{}, [M.ThreadCreated{"child", "p", "Child", M.Local{}, "", "", "codex", "", 1n}, M.ThreadLaunched{"child", "t", True{}, 6n}, M.TurnChanged{"child", M.Running{}, 6n}]), 50000n, False{})
    expect("queue held", base(True{}, [M.TurnQueued{"t", "queued", "Later", 6n}]), 50000n, False{})
    expect("pending count held", base(True{}, [M.PendingChanged{"t", 1n, 6n}]), 50000n, False{})
    expect("ask held even without pending count", base(True{}, [M.AskOpened{"ask", "t", "input", "", "Choose", "", 6n}]), 50000n, False{})
    expect("secret request held", base(True{}, [M.SettingSet{"secret.ask.example", "{\"thread\":\"t\"}", 6n}]), 50000n, False{})
    expect("grandchild launch work propagates", base(True{}, [M.ThreadCreated{"child", "p", "Child", M.Local{}, "", "", "codex", "", 1n}, M.ThreadLaunched{"child", "t", True{}, 6n}, M.ThreadCreated{"grandchild", "p", "Grandchild", M.Local{}, "", "", "codex", "", 1n}, M.ThreadLaunched{"grandchild", "child", True{}, 6n}, M.TurnChanged{"grandchild", M.Running{}, 6n}]), 50000n, False{})
    expect("approval held even without pending count", base(True{}, [M.AskOpened{"ask", "t", "approval", "Bash", "Proceed", "", 6n}]), 50000n, False{})
    expect("wake on a running thread held", base(True{}, [M.ThreadCreated{"child", "p", "Child", M.Local{}, "", "", "codex", "", 1n}, M.TurnChanged{"child", M.Running{}, 6n}, M.WakeSet{"wake", "t", "thread", "child", ["end"], True{}, "Wait", 6n}]), 50000n, False{})
    expect("owned lock held", base(True{}, [M.LockChanged{"bend", L.NamedLock{Some{party("t")}, Nil{}, False{}, None{}}, 6n}]), 50000n, False{})
    expect("queued lock held", base(True{}, [M.LockChanged{"bend", L.NamedLock{None{}, [party("t")], False{}, None{}}, 6n}]), 50000n, False{})
    expect("report missing held", base(True{}, [M.SettingSet{"autosettle.report.t", "", 6n}]), 50000n, False{})
    expect("report before current turn held", base(True{}, [M.SettingSet{"autosettle.report.t", "3", 6n}]), 50000n, False{})
    expect("muted reports have no delivery obligation", base(True{}, [M.ThreadLaunched{"t", "parent", False{}, 1n}, M.SettingSet{"autosettle.report.t", "", 6n}]), 905n, True{})
    expect("explicit report in parent's inbox is legacy evidence", base(True{}, [M.SettingSet{"autosettle.report.t", "", 5n}, M.MessagePosted{"parent", M.Link.id("t", "lr-report"), M.User{}, "Done", 5n}]), 905n, True{})
    expect("pinned preserved", base(True{}, [M.ThreadPinned{"t", True{}, 6n}]), 50000n, False{})
    expect("manual bring-back preserved", base(True{}, [M.ThreadUnsettled{"t", 6n}]), 50000n, False{})
    expect("archived preserved", base(True{}, [M.ThreadArchived{"t", True{}, 6n}]), 50000n, False{})
    expect("deleted preserved", base(True{}, [M.ThreadDeleted{"t", 6n}]), 50000n, False{})
    expect("snoozed preserved", base(True{}, [M.ThreadSnoozed{"t", 50001n, 6n}]), 50000n, False{})
    expect("timer disabled", base(True{}, [M.SettingSet{"settle.idle.minutes", "0", 1n}]), 50000n, False{})
    expect("worker grace configurable", base(True{}, [M.SettingSet{"settle.idle.minutes", "1", 1n}]), 65n, True{})
    expect("personal grace configurable", base(False{}, [M.SettingSet{"settle.personal.minutes", "1", 1n}, M.SettingSet{"autosettle.read.t", "answer", 6n}]), 65n, True{})
    C.Check.str("automatic cleanup cannot settle a failed turn", shelf(committed(base(True{}, [M.TurnChanged{"t", M.Failed{}, 6n}]), [M.ThreadSettled{"t", False{}, 50000n}])), "active")
    C.Check.str("merge cannot hide a failed turn", shelf(committed(base(True{}, [M.TurnChanged{"t", M.Failed{}, 6n}]), [M.ThreadMerged{"t", "https://example.invalid/pr/1", 50000n}])), "active")
    C.Check.str("merge keeps its evidence", M.Setting.get(committed(base(True{}, Nil{}), [M.ThreadMerged{"t", "merged", 50000n}]), M.Merge.key("t"), ""), "merged")
    C.Check.str("raw auto-settle cannot hide unread personal reply", shelf(committed(base(False{}, Nil{}), [M.ThreadSettled{"t", False{}, 50000n}])), "active")
    C.Check.str("upgrade pass settles old eligible worker", shelf(settled()), "settled")
    C.Check.str("upgrade pass is idempotent", Nat.show(List.length(&2, M.Change, A.Auto.upgrade(settled(), 2000n))), "0")
    C.Check.str("history retained", Nat.show(List.length(&2, M.Entry, M.State.mine(settled(), "t"))), Nat.show(List.length(&2, M.Entry, M.State.mine(base(True{}, Nil{}), "t"))))
    C.Check.str("user message reopens", shelf(committed(settled(), [M.MessagePosted{"t", "again", M.User{}, "More", 1000n}])), "active")
    C.Check.str("assistant message reopens", shelf(committed(settled(), [M.MessagePosted{"t", "late", M.Assistant{}, "More", 1000n}])), "active")
    C.Check.str("system message reopens", shelf(committed(settled(), [M.MessagePosted{"t", "system", M.System{}, "New result", 1000n}])), "active")
    C.Check.str("new turn reopens", shelf(committed(settled(), [M.TurnChanged{"t", M.Running{}, 1000n}])), "active")
    C.Check.str("plain completion cannot bypass grace with settle.done", shelf(committed(base(True{}, [M.SettingSet{"settle.done", "on", 1n}, M.TurnChanged{"t", M.Running{}, 6n}]), [M.TurnChanged{"t", M.Completed{}, 7n}])), "active")
    +shell : M.State = base(True{}, [M.WorkSet{"t", "bg", "job\tworking", 6n}])
    +cleared : M.State = committed(shell, [M.WorkSet{"t", "bg", "", 1000n}])
    expect("cleared work starts new quiet grace", cleared, 1899n, False{})
    expect("cleared work settles after grace", cleared, 1900n, True{})
    +read : M.State = M.State.replay(A.Auto.read(base(False{}, Nil{}), "t", 10n), base(False{}, Nil{}))
    expect("read receipt acknowledges actual reply", read, 14405n, True{})
    +many : M.State = M.State.replay([M.MessagePosted{"t", "a2", M.Assistant{}, "Two", 6n}, M.MessagePosted{"t", "u3", M.User{}, "Three?", 7n}, M.MessagePosted{"parent", "p1", M.Assistant{}, "Elsewhere", 8n}, M.MessagePosted{"t", "a3", M.Assistant{}, "Three", 9n}, M.MessagePosted{"t", "u4", M.User{}, "Four?", 10n}], base(False{}, Nil{}))
    C.Check.str("receipt names the newest own reply, as the full summary does", M.Setting.get(M.State.replay(A.Auto.read(many, "t", 11n), many), "autosettle.read.t", ""), "a3")
    C.Check.str("an output watermark wins over the walk", M.Setting.get(M.State.replay(A.Auto.read(M.State.replay([M.SettingSet{"autosettle.output.t", "mark", 11n}], many), "t", 12n), many), "autosettle.read.t", ""), "mark")
    expect("same-second later reply is unread", M.State.replay([M.MessagePosted{"t", "answer-two", M.Assistant{}, "More", 4n}], read), 50000n, False{})
    +personal : M.State = committed(base(True{}, Nil{}), [M.MessagePosted{"t", "human-joined", M.User{}, "More", 6n}, M.MessagePosted{"t", "personal-result", M.Assistant{}, "Personal reply", 7n}, M.TurnChanged{"t", M.Completed{}, 8n}, M.SettingSet{"autosettle.report.t", "8", 8n}])
    expect("personal unread survives history eviction", without_history(personal), 50000n, False{})
    expect("receipt survives history eviction", M.State.replay(A.Auto.read(without_history(personal), "t", 10n), without_history(personal)), 50000n, True{})
    +mixed : M.State = committed(base(True{}, Nil{}), [M.MessagePosted{"t", "human-joined", M.User{}, "More", 6n}, M.MessagePosted{"t", M.Link.id("parent", "later"), M.User{}, "Another worker note", 7n}])
    C.Check.str("personal audience stays personal across one commit", M.Setting.get(mixed, "autosettle.audience.t", ""), "person")
    C.Check.str("settled thread absent from active rows", Bool.show(String.contains("|" ++ String.join(U.Active.ids(U.Active.folded(False{}, U.Active.parts(U.Active.rows(settled(), 905n, "")))), "|") ++ "|", "|t|")), "False")
    C.Check.str("production tick uses same pure policy", Nat.show(List.length(&2, M.Change, B.Queue.changes(T.Tick.main(base(True{}, Nil{}), 905n)))), "1")
