import Base
import ./check.bend as C
import ../src/core/model.bend as M
import ../src/core/hub.bend as H
import ../src/core/client.bend as U
import ../src/core/subs.bend as SB
import ../src/mobile/screen.bend as MS
import ../src/core/json.bend as J

# Claude's subagents and background shells: a background one started, said what it does, and
# ended (lines as Claude Code 2.1 writes them); the thread waits on it
# after its own turn ends, and an agent that exits leaves none at work.

def base() -> List<&2, M.Change>:
  [M.ProjectCreated{"p1", "P", "/r/board", 50n}]

def state(es: List<&2, M.Change>) -> M.State:
  M.State.replay(es, M.State.empty())

def after.req(+es: List<&2, M.Change>, +now: Nat, +text: String) -> List<&2, M.Change>:
  List.append(&2, M.Change, es, H.Queue.changes(H.Hub.request_text(state(es), now, 1, text)))

# the log after one line of the agent's output
def after(+es: List<&2, M.Change>, +now: Nat, +line: String) -> List<&2, M.Change>:
  List.append(&2, M.Change, es, H.Queue.changes(H.Hub.agent_line(state(es), now, "t100-1", line)))

def made() -> List<&2, M.Change>:
  List.append(&2, M.Change, after.req(base(), 100n, "{\"id\":1,\"m\":\"thread.create\",\"p\":{\"project\":\"p1\",\"env\":\"local\"}}"),
    [M.TurnChanged{"t100-1", M.Running{}, 101n}])

def started() -> List<&2, M.Change>:
  after(made(), 102n, "{\"type\":\"system\",\"subtype\":\"task_started\",\"task_id\":\"a847\",\"tool_use_id\":\"toolu_1\",\"description\":\"Run bash command and report output\",\"subagent_type\":\"general-purpose\",\"is_backgrounded\":true,\"task_type\":\"local_agent\",\"session_id\":\"s\"}")

# the agent's own turn ends while the subagent works on
def done() -> List<&2, M.Change>:
  List.append(&2, M.Change, started(), [M.TurnChanged{"t100-1", M.Completed{}, 103n}])

def progressed() -> List<&2, M.Change>:
  after(done(), 104n, "{\"type\":\"system\",\"subtype\":\"task_progress\",\"task_id\":\"a847\",\"tool_use_id\":\"toolu_1\",\"description\":\"Running Sleep for 20 seconds\",\"last_tool_name\":\"Bash\",\"session_id\":\"s\"}")

def ended() -> List<&2, M.Change>:
  after(progressed(), 105n, "{\"type\":\"system\",\"subtype\":\"task_notification\",\"task_id\":\"a847\",\"tool_use_id\":\"toolu_1\",\"status\":\"completed\",\"session_id\":\"s\"}")

# a shell the agent put in the background (or Claude moved there when it
# ran past its timeout)
def shell() -> List<&2, M.Change>:
  after(made(), 102n, "{\"type\":\"system\",\"subtype\":\"task_started\",\"task_id\":\"b3\",\"description\":\"scripts/check.sh\",\"task_type\":\"local_bash\",\"is_backgrounded\":true}")

def shell.done() -> List<&2, M.Change>:
  List.append(&2, M.Change, shell(), [M.TurnChanged{"t100-1", M.Completed{}, 103n}])

# a foreground shell (Claude names it once it has run 30 s) that ran past
# its timeout and was moved to the background (lines as Claude Code 2.1.285
# writes them)
def moved() -> List<&2, M.Change>:
  after(after(made(), 102n, "{\"type\":\"system\",\"subtype\":\"task_started\",\"task_id\":\"b5\",\"task_type\":\"local_bash\",\"is_backgrounded\":false,\"description\":\"Run checks\"}"),
    103n, "{\"type\":\"system\",\"subtype\":\"task_updated\",\"task_id\":\"b5\",\"patch\":{\"is_backgrounded\":true}}")

# the subagent's own lines: a tool call, then what it wrote
def stepped() -> List<&2, M.Change>:
  after(after(started(), 102n, "{\"type\":\"assistant\",\"parent_tool_use_id\":\"toolu_1\",\"message\":{\"id\":\"m9\",\"content\":[{\"type\":\"tool_use\",\"id\":\"toolu_2\",\"name\":\"Bash\",\"input\":{\"command\":\"ls /etc/ssh\"}}]}}"),
    103n, "{\"type\":\"assistant\",\"parent_tool_use_id\":\"toolu_1\",\"message\":{\"id\":\"m10\",\"content\":[{\"type\":\"text\",\"text\":\"10 files\\nmore\"}]}}")

def entries(+es: List<&2, M.Change>, +th: String) -> String:
  Nat.show(List.length(&2, M.Entry, M.State.mine(state(es), th)))

def row.show(r: SB.SubRow) -> String:
  match r:
    case SB.SubRow{k, _, ti, _, w, d, _, o, n, ss}:
      k ++ "|" ++ ti ++ "|" ++ w ++ "|" ++ d ++ "|" ++ Bool.pick(String, o, "open", "shut") ++ "|" ++ Nat.show(n) ++ "|" ++ Nat.show(List.length(&2, SB.SubStep, ss))

def rows.show(rs: List<&2, SB.SubRow>) -> String:
  match rs:
    case Nil{}:
      ""
    case r <> t:
      row.show(r) ++ ";" ++ rows.show(t)

def rows(+es: List<&2, M.Change>) -> String:
  rows.show(SB.Subs.rows(state(es), Map.new(&2, String), "t100-1", 110n))

# the panel as a whole: open (1) or shut (0), its word, and its rows shown
def panel.k(+info: Map<&2, String>, +rs: List<&2, SB.SubRow>) -> String:
  +open = SB.Subs.panel.open(info, "t100-1", rs)
  Bool.pick(String, open, "1", "0") ++ "|" ++ SB.Subs.panel.word(rs) ++ "|" ++ Nat.show(List.length(&2, SB.SubRow, SB.Subs.panel.rows(open, rs)))

def panel.with(+es: List<&2, M.Change>, +info: Map<&2, String>) -> String:
  panel.k(info, SB.Subs.rows(state(es), info, "t100-1", 110n))

def panel(+es: List<&2, M.Change>) -> String:
  panel.with(es, Map.new(&2, String))

# the info after a click on the panel's head (the "fold" action's value)
def clicked(+v: String) -> Map<&2, String>:
  Map.set(&2, String, Map.new(&2, String), "fold:" ++ String.drop(v, 1n), String.take(v, 1n))

def stepped.ended() -> List<&2, M.Change>:
  after(stepped(), 105n, "{\"type\":\"system\",\"subtype\":\"task_notification\",\"task_id\":\"a847\",\"tool_use_id\":\"toolu_1\",\"status\":\"completed\",\"session_id\":\"s\"}")

def status(+es: List<&2, M.Change>) -> String:
  M.Status.name(U.Side.status(state(es), H.Hub.thread(state(es), "t100-1")))

def phase(+es: List<&2, M.Change>) -> String:
  M.Phase.label(U.Side.phase(state(es), H.Hub.thread(state(es), "t100-1")))

def doings(ls: List<&2, String>) -> List<&2, String>:
  match ls:
    case Nil{}:
      Nil{}
    case +l <> t:
      M.Bg.doing(l) <> doings(t)

def doing(+es: List<&2, M.Change>) -> String:
  String.join(doings(M.Bg.of(state(es), "t100-1")), "|")

def prog1() -> String:
  "{\"type\":\"system\",\"subtype\":\"task_progress\",\"task_id\":\"a847\",\"description\":\"Sleep 1\"}"

def prog2() -> String:
  "{\"type\":\"system\",\"subtype\":\"task_progress\",\"task_id\":\"a847\",\"description\":\"Sleep 2\"}"

def updated() -> List<&2, M.Change>:
  after(progressed(), 105n, "{\"type\":\"system\",\"subtype\":\"task_updated\",\"task_id\":\"a847\",\"patch\":{\"status\":\"completed\"}}")

def texts(es: List<&2, M.Entry>) -> List<&2, String>:
  match es:
    case Nil{}:
      Nil{}
    case M.Msg{_, _, _, +t, _} <> rest:
      t <> texts(rest)
    case _ <> rest:
      texts(rest)

def head(ls: List<&2, String>) -> String:
  Maybe.default(&2, String, List.head(&2, String, ls), "")

# delegation two deep: root t0's task k1 runs on t1, whose task k2 runs on t2
def tree() -> List<&2, M.Change>:
  [M.ProjectCreated{"p1", "P", "/r/board", 50n}, M.ThreadCreated{"t0", "p1", "root", M.Local{}, "", "", "claude", "", 50n},
   M.ThreadCreated{"t1", "p1", "child", M.Local{}, "", "", "claude", "", 50n}, M.ThreadCreated{"t2", "p1", "grandchild", M.Local{}, "", "", "claude", "", 50n},
   M.TaskDelegated{"k1", "t0", "t1", 1n, "claude", "", 51n}, M.TaskStateSet{"k1", M.TaskRunning{}, "", 51n},
   M.TaskDelegated{"k2", "t1", "t2", 2n, "claude", "", 52n}, M.TaskStateSet{"k2", M.TaskRunning{}, "", 52n},
   M.MessagePosted{"t2", "g1", M.User{}, "do it", 52n}, M.TurnChanged{"t1", M.Running{}, 52n}, M.TurnChanged{"t2", M.Running{}, 52n}]

def said(+th: String, +text: String) -> String:
  "{\"type\":\"assistant\",\"message\":{\"id\":\"m-" ++ th ++ "-" ++ text ++ "\",\"content\":[{\"type\":\"text\",\"text\":\"" ++ text ++ "\"}]}}"

def result() -> String:
  "{\"type\":\"result\",\"is_error\":false,\"result\":\"ok\"}"

# a thread's turn ends with its answer
def ends(+es: List<&2, M.Change>, +now: Nat, +th: String, +text: String) -> H.Step:
  H.Hub.agent_line(state(List.append(&2, M.Change, es, H.Queue.changes(H.Hub.agent_line(state(es), now, th, said(th, text))))), Nat.add(now, 1n), th, result())

def after.end(+es: List<&2, M.Change>, +now: Nat, +th: String, +text: String) -> List<&2, M.Change>:
  List.append(&2, M.Change, List.append(&2, M.Change, es, H.Queue.changes(H.Hub.agent_line(state(es), now, th, said(th, text)))), H.Queue.changes(ends(es, now, th, text)))

def marked(+es: List<&2, M.Change>, +id: String) -> String:
  Bool.show(H.Task.marked(es, id))

def act.ui(a: U.Act) -> U.Ui:
  match a:
    case U.Act{u, _}:
      u

def sample(+key: String, +word: String, live: Bool) -> SB.SubRow:
  SB.SubRow{key, True{}, "Check the board", "subagent", word, "", live, False{}, 0n, Nil{}}

def grouped(+info: Map<&2, String>) -> List<&2, SB.SubRow>:
  SB.Subs.group([sample("run", "running", True{}), sample("bad", "failed", False{}), sample("ok", "done", False{}), sample("stop", "stopped", False{})], info, "t1")

def keys(rs: List<&2, SB.SubRow>) -> String:
  match rs:
    case Nil{}:
      ""
    case r <> t:
      SB.Subs.key(r) ++ ";" ++ keys(t)

def shut(+rs: List<&2, SB.SubRow>) -> Map<&2, String>:
  U.Ui.infos(act.ui(U.Ui.act(U.Ui.init(110n), "subs-shut", SB.Subs.panel.ask(True{}, "t1", rs))))

def recent.tasks() -> M.State:
  state(List.append(&2, M.Change, tree(), [M.TaskStateSet{"k1", M.TaskDone{}, "ok", 100n}, M.TaskStateSet{"k2", M.TaskFailed{}, "error", 100n}]))

def at.now(+es: List<&2, M.Change>, +now: Nat) -> String:
  rows.show(SB.Subs.rows(state(es), Map.new(&2, String), "t100-1", now))

def launch.ask() -> M.State:
  state([M.ThreadCreated{"child", "p1", "Inspect power", M.Local{}, "", "", "claude", "claude-opus-5-5", 1n}, M.ThreadLaunched{"child", "t0", False{}, 1n}, M.TurnChanged{"child", M.Completed{}, 2n}, M.AskOpened{"a1", "child", "input", "", "Which rail?", "", 3n}])

def main() -> IO(Unit):
  do IO<Unit>:
    +nf : Map<&2, String> = Map.new(&2, String)
    C.Check.str("both phone heads shut with the shared action", J.Json.get_str(MS.Mob.subs(nf, "t1", grouped(nf)), "act", ""), "subs-shut")
    C.Check.str("both phone heads reopen with the shared action", J.Json.get_str(MS.Mob.subs(shut(grouped(nf)), "t1", grouped(nf)), "act", ""), "subs-open")
    C.Check.str("both phones render the done toggle as an agent row", J.Json.get_str(MS.Mob.sub(SB.Subs.done.toggle("t1", 2n, False{})), "value", ""), "1subs.done:t1")
    C.Check.str("both phones omit an empty panel", J.Json.show(MS.Mob.subs(nf, "t1", Nil{})), "null")
    C.Check.str("done toggle follows work and failure", keys(grouped(nf)), "run;bad;subs.done:t1;")
    C.Check.str("done toggle opens with the existing fold action", SB.Subs.ask(False{}, "subs.done:t1"), "1subs.done:t1")
    +expanded : Map<&2, String> = U.Ui.infos(act.ui(U.Ui.act(U.Ui.init(110n), "fold", SB.Subs.ask(False{}, "subs.done:t1"))))
    C.Check.str("expanded finishes follow the toggle", keys(grouped(expanded)), "run;bad;subs.done:t1;ok;stop;")
    C.Check.str("expansion never doubles the finished count", SB.Subs.panel.word(SB.Subs.group([sample("ok", "done", False{}), sample("stop", "stopped", False{})], expanded, "t1")), "2 done")
    C.Check.str("failure keeps the panel open", Bool.show(SB.Subs.panel.open(nf, "t1", SB.Subs.group([sample("bad", "failed", False{})], nf, "t1"))), "True")
    C.Check.str("24 hours retained at boundary", Bool.show(SB.Subs.old(86500n, 100n)), "False")
    C.Check.str("24 hours plus one expires", at.now(stepped.ended(), 86506n), "")
    C.Check.str("old active agents are kept", Bool.show(String.contains(at.now(stepped(), 999999n), "running")), "True")
    C.Check.str("unknown times alone do not expire", Bool.show(SB.Subs.old(999999n, 0n)), "False")
    C.Check.str("finished agent missing its start is hidden", at.now([M.ActivityLogged{"@sub:t100-1", "c/end", "c", "end", "completed", 100n}], 110n), "")
    C.Check.str("untitled live agent uses kind and step", rows([M.SettingSet{"bg.t100-1", "a\tWorking\tc\t\tsubagent", 100n}, M.ActivityLogged{"@sub:t100-1", "c/u1", "c", "Bash", "ls\nmore", 101n}]), "c|subagent: ls|running|Working|open|1|1;")
    C.Check.str("provider placeholder never becomes a bare Subagent", SB.Subs.title("subagent", "Subagent", [SB.SubStep{"Bash", "inspect"}], ""), "subagent: inspect")
    C.Check.str("known start beats provider placeholder", SB.Subs.asked("Subagent", "Inspect power"), "Inspect power")
    C.Check.str("no work and no retained finishes has no panel", keys(SB.Subs.rows(M.State.empty(), nf, "t1", 110n)), "")
    C.Check.str("work sorts before input and failures", keys(SB.Subs.group([sample("bad", "error", False{}), sample("ask", "needs you", True{}), sample("run", "starting", True{})], nf, "t1")), "run;ask;bad;")
    C.Check.str("prompt title uses first line", rows([M.SettingSet{"bg.t100-1", "a\tWorking\tc\tInspect power\tsubagent", 100n}, M.ActivityLogged{"@sub:t100-1", "c/start", "c", "start:subagent", "Inspect power\nmore", 100n}]), "c|Inspect power|running|Working|open|0|0;")
    C.Check.str("Claude alias and full id normalize", SB.Subs.model("claude", "claude-opus-5-5"), SB.Subs.model("claude", "opus"))
    C.Check.str("all Claude families and default", SB.Subs.model("claude", "claude-sonnet-4-5") ++ "/" ++ SB.Subs.model("claude", "haiku") ++ "/" ++ SB.Subs.model("claude", "claude-fable-5") ++ "/" ++ SB.Subs.model("claude", ""), "sonnet/haiku/fable/default")
    C.Check.str("Claude full ids normalize any family", SB.Subs.model("claude", "claude-custom-2-1"), SB.Subs.model("claude", "custom"))
    C.Check.str("other provider label retained", SB.Subs.model("codex", "gpt-6"), "codex · gpt-6")
    C.Check.str("task end event controls age", keys(SB.Subs.rows(recent.tasks(), nf, "t0", 86501n)), "")
    C.Check.str("a finished task stays a day after its thread last moved", keys(SB.Subs.rows(recent.tasks(), nf, "t0", 86452n)), "subs.done:t0;")
    C.Check.str("a finished task goes a second later", keys(SB.Subs.rows(recent.tasks(), nf, "t0", 86453n)), "")
    C.Check.str("end times fold nothing into settings", M.Setting.get(recent.tasks(), "task.end.k1", "") ++ M.Setting.get(launch.ask(), "thread.end.child", ""), "")
    +saved : Map<&2, String> = shut([sample("a", "running", True{}), sample("b", "running", True{})])
    C.Check.str("shut choice stores current signatures", U.Str.get(saved, "subs:t1"), "0live:a\u{001f}live:b")
    C.Check.str("same live set stays shut", Bool.show(SB.Subs.panel.open(saved, "t1", [sample("a", "running", True{}), sample("b", "running", True{})])), "False")
    C.Check.str("shrinking live set stays shut", Bool.show(SB.Subs.panel.open(saved, "t1", [sample("b", "running", True{})])), "False")
    C.Check.str("new agent reopens", Bool.show(SB.Subs.panel.open(saved, "t1", [sample("c", "running", True{})])), "True")
    C.Check.str("existing agent now needing you reopens", Bool.show(SB.Subs.panel.open(saved, "t1", [sample("a", "needs you", True{})])), "True")
    C.Check.str("another thread has its own choice", Bool.show(SB.Subs.panel.open(saved, "t2", [sample("a", "running", True{})])), "True")
    C.Check.str("launched input is attention even without pending tools", SB.Subs.word(Maybe.default(&2, SB.SubRow, List.head(&2, SB.SubRow, SB.Subs.rows(launch.ask(), nf, "t0", 110n)), sample("", "", False{}))), "needs you")
    C.Check.str("launched labels are short", SB.Subs.launch.who(U.Ui.thread(launch.ask(), "child")), "launched · opus")
    C.Check.str("launched completed thread ages out", keys(SB.Subs.rows(M.State.apply(launch.ask(), M.AskClosed{"a1", "ok", 4n}), nf, "t0", 86403n)), "")
    C.Check.str("a person's later message in a launched thread keeps it listed", keys(SB.Subs.rows(M.State.replay([M.AskClosed{"a1", "ok", 4n}, M.MessagePosted{"child", "m-later", M.User{}, "note", 86402n}], launch.ask()), nf, "t0", 86403n)), "subs.done:t0;")
    C.Check.str("launched monitoring uses a live word", SB.Subs.launch.word(M.PhMonitoring{1n}, "done"), "watching")
    C.Check.str("saved signatures split in order", String.join(SB.Subs.saved("live:a\u{001f}attention:b", "", Nil{}), "/"), "live:a/attention:b")
    C.Check.str("a launched thread ended when its turn last changed", Nat.show(SB.Subs.end(launch.ask(), "child")), "2")
    C.Check.str("an unknown thread has no end time", Nat.show(SB.Subs.end(launch.ask(), "nobody")), "0")
    C.Check.str("a live row without a tool id is not also listed as done", keys(SB.Subs.rows(state([M.SettingSet{"bg.t1", "b\tWorking\t\tInspect\tshell", 100n}, M.ActivityLogged{"@sub:t1", "b/start", "b", "start:shell", "Inspect", 100n}]), nf, "t1", 110n)), "b;")
    C.Check.str("a freshly launched idle turn is still launching", SB.Subs.launch.word(M.PhIdle{}, "idle") ++ "/" ++ Bool.show(SB.Subs.phase.live(M.PhIdle{})), "launching/True")
    C.Check.str("resolving an acknowledged ask leaves the panel shut", Bool.show(SB.Subs.panel.open(shut([sample("a", "needs you", True{})]), "t1", [sample("a", "running", True{})])), "False")
    +needs_task : M.State = M.State.apply(recent.tasks(), M.AskOpened{"a-task", "t1", "input", "", "Which rail?", "", 110n})
    +needs_row : SB.SubRow = Maybe.default(&2, SB.SubRow, List.head(&2, SB.SubRow, SB.Subs.rows(needs_task, nf, "t0", 999999n)), sample("", "", False{}))
    C.Check.str("a finished task can still link to an ask without a bogus Stop", SB.Subs.word(needs_row) ++ "/" ++ SB.Subs.cancel(needs_row), "needs you/")
    C.Check.str("a task model cannot collide with the launched marker", SB.Subs.cancel(SB.SubRow{"kid", False{}, "Inspect", "launched", "running", "", True{}, False{}, 0n, Nil{}}), "task-cancel")
    C.Check.str("started", doing(started()), "Run bash command and report output")
    C.Check.str("a failed turn shows failed, work or not", phase(List.append(&2, M.Change, shell(), [M.TurnChanged{"t100-1", M.Failed{}, 103n}])) ++ "/" ++ status(List.append(&2, M.Change, shell(), [M.TurnChanged{"t100-1", M.Failed{}, 103n}])), "Failed/failed")
    # child -> grandchild -> root: each task ends only once its thread is done
    +c1 : List<&2, M.Change> = after.end(tree(), 60n, "t1", "waiting on t2")
    C.Check.str("a child with its own child at work: its task stays open", marked(H.Queue.changes(ends(tree(), 60n, "t1", "waiting on t2")), "k1"), "False")
    +g1 : List<&2, M.Change> = after.end(c1, 70n, "t2", "grandchild result")
    C.Check.str("the grandchild ends: its task ends", marked(H.Queue.changes(ends(c1, 70n, "t2", "grandchild result")), "k2"), "True")
    C.Check.str("and the child hears it", Bool.show(String.contains(H.Task.last(state(g1), "t1") ++ M.Queue.of(state(g1), "t1") ++ String.join(texts(M.State.mine(state(g1), "t1")), "|"), "grandchild result")), "True")
    +r1 : List<&2, M.Change> = after.end(List.append(&2, M.Change, g1, [M.TurnChanged{"t1", M.Running{}, 80n}]), 81n, "t1", "child result")
    C.Check.str("the child ends after: its task ends", marked(H.Queue.changes(ends(List.append(&2, M.Change, g1, [M.TurnChanged{"t1", M.Running{}, 80n}]), 81n, "t1", "child result")), "k1"), "True")
    C.Check.str("and the root hears the child's result", Bool.show(String.contains(String.join(texts(M.State.mine(state(r1), "t0")), "|"), "child result")), "True")
    +q1 : List<&2, M.Change> = List.append(&2, M.Change, tree(), [M.TaskStateSet{"k2", M.TaskDone{}, "", 55n}, M.TaskDelivered{"k2", 55n}, M.TurnQueued{"t1", "q9", "one more thing", 56n}])
    C.Check.str("a child with a queued follow-up: its task stays open", marked(H.Queue.changes(ends(q1, 60n, "t1", "first answer")), "k1"), "False")
    C.Check.str("its steps stay off the thread", entries(stepped(), "t100-1"), entries(started(), "t100-1"))
    C.Check.str("its steps go to its log", entries(stepped(), M.Bg.log("t100-1")), "3")
    C.Check.str("at work: open with its steps", rows(stepped()), "toolu_1|Run bash command and report output|running|Run bash command and report output|open|2|2;")
    C.Check.str("at work: its title kept through progress", rows(after(stepped(), 104n, "{\"type\":\"system\",\"subtype\":\"task_progress\",\"task_id\":\"a847\",\"description\":\"Running ls\"}")),
      "toolu_1|Run bash command and report output|running|Running ls|open|2|2;")
    C.Check.str("ended: listed, shut", rows(stepped.ended()), "subs.done:t100-1|1 done|||shut|1|0;")
    # the panel: open while one works, folded to its head once all are done
    C.Check.str("panel at work: open", panel(stepped()), "1|1 at work|1")
    C.Check.str("panel all done: folded to its head", panel(stepped.ended()), "0|1 done|0")
    C.Check.str("a click on a done panel's head asks to open it", SB.Subs.panel.ask(False{}, "t100-1", Nil{}), "t100-1")
    C.Check.str("opened by hand, a done panel stays open", panel.with(stepped.ended(), U.Ui.infos(act.ui(U.Ui.act(U.Ui.init(110n), "subs-open", "t100-1")))), "1|1 done|1")
    C.Check.str("shut by hand, a working panel stays shut", panel.with(stepped(), U.Ui.infos(act.ui(U.Ui.act(U.Ui.init(110n), "subs-shut", SB.Subs.panel.ask(True{}, "t100-1", SB.Subs.rows(state(stepped()), Map.new(&2, String), "t100-1", 110n)))))), "0|1 at work|0")
    C.Check.str("a background shell is listed by its kind", rows(shell()), "b3|shell: scripts/check.sh|running|scripts/check.sh|open|0|0;")
    C.Check.str("a background shell is not an agent", SB.Subs.badge(SB.Subs.count(state(shell()), "t100-1")), "")
    C.Check.str("the sidebar's count", SB.Subs.badge(SB.Subs.count(state(stepped()), "t100-1")) ++ "/" ++ SB.Subs.badge(SB.Subs.count(state(stepped.ended()), "t100-1")), "1 subthread/")
    C.Check.str("working while the turn runs", status(started()), "working")
    C.Check.str("waiting after the turn", status(done()), "waiting")
    C.Check.str("phase after the turn", phase(done()), "Monitoring 1 task")
    C.Check.str("progress", doing(progressed()), "Running Sleep for 20 seconds")
    C.Check.str("progress again writes nothing", Nat.show(List.length(&2, M.Change, progressed())), Nat.show(List.length(&2, M.Change,
      after(progressed(), 106n, "{\"type\":\"system\",\"subtype\":\"task_progress\",\"task_id\":\"a847\",\"description\":\"Running Sleep for 20 seconds\",\"session_id\":\"s\"}"))))
    C.Check.str("a subagent's bash is not listed", doing(after(done(), 104n, "{\"type\":\"system\",\"subtype\":\"task_started\",\"task_id\":\"b1\",\"description\":\"Sleep\",\"task_type\":\"local_bash\",\"owned_by_subagent\":true}")), "Run bash command and report output")
    C.Check.str("a foreground shell is not listed", doing(after(made(), 102n, "{\"type\":\"system\",\"subtype\":\"task_started\",\"task_id\":\"b2\",\"description\":\"sleep 4\",\"task_type\":\"local_bash\",\"is_backgrounded\":false}")), "")
    C.Check.str("a background shell is listed", doing(shell()), "scripts/check.sh")
    C.Check.str("the turn over, it monitors the shell", status(shell.done()), "waiting")
    C.Check.str("monitoring phase", phase(shell.done()), "Monitoring 1 task")
    C.Check.str("the shell ends: ready", status(after(shell.done(), 105n, "{\"type\":\"system\",\"subtype\":\"task_notification\",\"task_id\":\"b3\",\"status\":\"completed\"}")), "ready")
    C.Check.str("a shell moved to the background past its timeout is listed", doing(moved()), "Background shell")
    C.Check.str("moved, the turn over: monitoring", phase(List.append(&2, M.Change, moved(), [M.TurnChanged{"t100-1", M.Completed{}, 104n}])), "Monitoring 1 task")
    C.Check.str("moved, then done: ended, its outcome owed", M.Bg.status(head(M.Bg.of(state(after(moved(), 105n, "{\"type\":\"system\",\"subtype\":\"task_updated\",\"task_id\":\"b5\",\"patch\":{\"status\":\"completed\",\"end_time\":1}}")), "t100-1"))), "completed")
    C.Check.str("a monitor is listed", doing(after(made(), 102n, "{\"type\":\"system\",\"subtype\":\"task_started\",\"task_id\":\"b4\",\"description\":\"CI\",\"task_type\":\"monitor_mcp\"}")), "CI")
    C.Check.str("ended", doing(ended()), "")
    C.Check.str("ready after", status(ended()), "ready")
    C.Check.str("updated to completed ends it", M.Bg.status(head(M.Bg.of(state(updated()), "t100-1"))), "completed")
    C.Check.str("an ended row shows finished, reporting", rows(updated()), "toolu_1|Run bash command and report output|reporting|finished, reporting|open|0|0;")
    C.Check.str("an ended row is still work", phase(updated()), "Monitoring 1 task")
    C.Check.str("progress leaves an ended row", M.Bg.status(head(M.Bg.of(state(after(updated(), 106n, "{\"type\":\"system\",\"subtype\":\"task_progress\",\"task_id\":\"a847\",\"description\":\"late\"}")), "t100-1"))), "completed")
    C.Check.str("its notification takes it", doing(after(updated(), 106n, "{\"type\":\"system\",\"subtype\":\"task_notification\",\"task_id\":\"a847\",\"tool_use_id\":\"toolu_1\",\"status\":\"completed\"}")), "")
    C.Check.str("gone from the open list: ended", M.Bg.status(head(M.Bg.of(state(after(progressed(), 105n, "{\"type\":\"system\",\"subtype\":\"background_tasks_changed\",\"tasks\":[{\"task_id\":\"zz\"}]}")), "t100-1"))), "done")
    C.Check.str("still on the open list: at work", doing(after(progressed(), 105n, "{\"type\":\"system\",\"subtype\":\"background_tasks_changed\",\"tasks\":[{\"task_id\":\"a847\"}]}")), "Running Sleep for 20 seconds")
    C.Check.str("the open list adds nothing", doing(after(made(), 105n, "{\"type\":\"system\",\"subtype\":\"background_tasks_changed\",\"tasks\":[{\"task_id\":\"a847\"}]}")), "")
    C.Check.str("progress within the gap is held", Nat.show(List.length(&2, String, H.PStep.flush(H.Prog.line(100n, [], 102n, prog1(), H.Prog.o(prog1()))))), "0")
    C.Check.str("progress after the gap goes through", Nat.show(List.length(&2, String, H.PStep.flush(H.Prog.line(100n, [], 105n, prog1(), H.Prog.o(prog1()))))), "1")
    C.Check.str("the held newest lands on the beat", doing(List.append(&2, M.Change, done(), H.Queue.changes(H.Prog.flush(state(done()), "t100-1", 106n, H.PStep.flush(H.Prog.beat(100n, H.PStep.pend(H.Prog.line(100n, H.PStep.pend(H.Prog.line(100n, [], 101n, prog1(), H.Prog.o(prog1()))), 102n, prog2(), H.Prog.o(prog2()))), 106n)))))), "Sleep 2")
    C.Check.str("a beat inside the gap lands nothing", Nat.show(List.length(&2, String, H.PStep.flush(H.Prog.beat(100n, [prog1()], 103n)))), "0")
    C.Check.str("exit clears", doing(List.append(&2, M.Change, progressed(), H.Queue.changes(H.Hub.agent_gone(state(progressed()), 107n, "t100-1", 0)))), "")
    C.Check.str("restart tells the agent and clears", M.Bg.show(M.Bg.of(H.Carry.after(state(progressed()), 107n), "t100-1")), "")
