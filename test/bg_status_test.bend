import Base
import ./check.bend as C
import ../src/core/model.bend as M
import ../src/core/hub.bend as B
import ../src/core/mcp.bend as MC
import ../src/core/json.bend as J

# A thread whose turn ended while work it owns still runs (mcp.bend's
# Mcp.phase, hub.bend's Wake.how; laws mcp_phase_*, wake_says_*): agents
# read it as monitoring, never complete, and it settles only once all of it
# is done.

# t1's turn runs, started by a message at 2
def log(cs: List<&2, M.Change>) -> M.State:
  B.Shelve.demo(cs)

# the same with "Settle when done" on (settle.done, off by default), where
# the settle rule applies
def logon(cs: List<&2, M.Change>) -> M.State:
  B.Shelve.demo.on(cs)

def agent() -> String:
  "a1\tRunning Probe the build\ttoolu_1\tBuild the archive\tsubagent"

def shell() -> String:
  "b1\tnpm run build\ttoolu_2\tBuild\tshell"

def done() -> M.Change:
  M.TurnChanged{"t1", M.Completed{}, 5n}

def bg(+rows: String, +at: Nat) -> M.Change:
  M.WorkSet{"t1", "bg", rows, at}

def status(+s: M.State) -> J.Json:
  MC.Mcp.thread_json.w(s, M.State.thread.get(s, "t1"))

def phase(+s: M.State) -> String:
  J.Json.get_str(status(s), "phase", "")

def waiting(+s: M.State) -> String:
  Nat.show(J.Json.get_nat(status(s), "waiting", 99n))

def first(xs: List<&2, J.Json>) -> String:
  match xs:
    case Nil{}: ""
    case x <> _: J.Json.as_str(Some{x}, "")

def work(+s: M.State) -> String:
  first(J.Json.elems(J.Json.get_obj(status(s), "work")))

def body(r: MC.Res) -> J.Json:
  match r:
    case MC.Res{_, b, _}: b

# the phase thread_list gives the thread
def listed(+s: M.State) -> String:
  J.Json.get_str(Maybe.default(&2, J.Json, List.head(&2, J.Json, J.Json.elems(J.Json.get_obj(body(MC.Mcp.list(s, "t1")), "threads"))), J.Null{}), "phase", "")

def one(c: M.Change) -> String:
  match c:
    case M.ThreadSettled{id, _, at}: "settle:" ++ id ++ "@" ++ Nat.show(at)
    case M.ThreadWoke{id, at}: "wake:" ++ id ++ "@" ++ Nat.show(at)
    case _: "other"

def ones(cs: List<&2, M.Change>) -> List<&2, String>:
  match cs:
    case Nil{}: Nil{}
    case c <> rest: one(c) <> ones(rest)

def step(+s: M.State, +cs: List<&2, M.Change>) -> String:
  String.join(ones(B.Shelve.step(s, cs)), ",")

def note(+how: String) -> J.Json:
  J.Json.obj([J.KV{"task_id", J.Str{"a1"}}, J.KV{"status", J.Str{how}}])

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("turn ended, subagent at work: monitoring, not complete", phase(log([bg(agent(), 3n), done()])), "monitoring")
    C.Check.str("it counts the subagent", waiting(log([bg(agent(), 3n), done()])), "1")
    C.Check.str("and names it", work(log([bg(agent(), 3n), done()])), "Build the archive")
    C.Check.str("thread_list says the same", listed(log([bg(agent(), 3n), done()])), "monitoring")
    C.Check.str("a background shell holds it too", phase(log([bg(shell(), 3n), done()])), "monitoring")
    C.Check.str("a watched job holds it too", phase(log([M.SettingSet{"watch.t1", "w1\twatching", 3n}, done()])), "monitoring")
    C.Check.str("a delegated task at work holds it too", phase(log([M.TaskDelegated{"k1", "t1", "t2", 1n, "claude", "", 3n}, done()])), "monitoring")
    C.Check.str("all done: complete", phase(log([bg(agent(), 3n), done(), bg("", 6n)])), "complete")
    C.Check.str("and nothing waits", waiting(log([bg(agent(), 3n), done(), bg("", 6n)])), "0")
    C.Check.str("the turn still running: working", phase(log([bg(agent(), 3n)])), "working")
    C.Check.str("an approval still comes first", phase(log([bg(agent(), 3n), done(), M.PendingChanged{"t1", 1n, 6n}])), "approval")
    C.Check.str("a question comes before the work", phase(log([bg(agent(), 3n), done(), M.AskOpened{"q1", "t1", "input", "", "?", "", 6n}])), "input")
    C.Check.str("a failed turn says so before its work", phase(log([bg(agent(), 3n), M.TurnChanged{"t1", M.Failed{}, 5n}])), "failed")
    C.Check.str("a Stop with work left: monitoring", phase(log([bg(agent(), 3n), M.TurnChanged{"t1", M.Interrupted{}, 5n}])), "monitoring")
    C.Check.str("a Stop with nothing left: stopped", phase(log([M.TurnChanged{"t1", M.Interrupted{}, 5n}])), "stopped")
    C.Check.str("turn ended with the subagent at work: no settle", step(logon([bg(agent(), 3n)]), [done()]), "")
    C.Check.str("its progress lines: still no settle", step(logon([bg(agent(), 3n), done()]), [bg("a1\tRunning Write README\ttoolu_1\tBuild the archive\tsubagent", 7n)]), "")
    C.Check.str("killed, row not yet cleared: no settle", step(logon([bg(agent(), 3n), done()]), [bg("a1\t" ++ M.Bg.ended.mark() ++ "killed\ttoolu_1\tBuild the archive\tsubagent", 7n)]), "")
    C.Check.str("all of it done: settles", step(logon([bg(agent(), 3n), done()]), [bg("", 8n)]), "settle:t1@8")
    C.Check.str("settled by the person while it monitors: kept settled", step(log([bg(agent(), 3n), done(), M.ThreadSettled{"t1", True{}, 6n}]), [bg("a1\tRunning Write README\ttoolu_1\tBuild the archive\tsubagent", 7n)]), "")
    C.Check.str("a wake by a killed subagent says so", B.Wake.text(False{}, log([bg(agent(), 3n)]), "t1", note("killed")), "Woken: Build the archive (killed)")
    C.Check.str("a wake by a failed one too", B.Wake.text(False{}, log([bg(agent(), 3n)]), "t1", note("failed")), "Woken: Build the archive (failed)")
    C.Check.str("a wake by finished work reads as before", B.Wake.text(False{}, log([bg(agent(), 3n)]), "t1", note("completed")), "Woken: Build the archive")
