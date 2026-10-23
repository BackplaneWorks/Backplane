import Base
import ./check.bend as C
import ../src/core/model.bend as M
import ../src/core/client.bend as U
import ../src/core/botui.bend as BU
import ../src/core/plane.bend as P
import ../src/core/i18n.bend as I
import ../src/core/json.bend as J
import ../src/mobile/plane.bend as MP
import ../src/web/bots.bend as WB
import ../src/web/dom.bend as D

# What the uberbot's count says needs the person, and the rows behind it.
# A launcher, an integrator it launched, and the threads they launched; a bot
# (the uberbot) whose own thread launched them.
def log() -> List<&2, M.Change>:
  [M.ProjectCreated{"project", "Test", "/tmp/attention", 1n},
    M.ThreadCreated{"bt", "", "Bub", M.Local{}, "", "", "claude", "", 1n}, M.BotSet{"b1", "Bub", "bt", 7n, "", 1n},
    M.ThreadCreated{"int", "project", "Integrator", M.Local{}, "", "", "claude", "", 2n}, M.ThreadLaunched{"int", "bt", True{}, 2n},
    M.ThreadCreated{"ds", "project", "DeepSeek smoke", M.Local{}, "", "", "claude", "", 3n}, M.ThreadLaunched{"ds", "int", True{}, 3n},
    M.ThreadCreated{"pi", "project", "Old Pi", M.Local{}, "", "", "claude", "", 4n}, M.ThreadLaunched{"pi", "bt", True{}, 4n},
    M.ThreadCreated{"pi2", "project", "Pi owner", M.Local{}, "", "", "claude", "", 5n}, M.ThreadLaunched{"pi2", "bt", True{}, 5n}]

def ui(cs: List<&2, M.Change>) -> U.Ui:
  U.Ui.online(U.Ui.fold(List.append(&2, M.Change, log(), cs), U.Ui.init(20n)), True{})

def plane(cs: List<&2, M.Change>) -> P.Plane:
  P.Plane.signals.of(ui(cs), 20n, True{})

def need(cs: List<&2, M.Change>) -> String:
  String.join(P.Plane.ids(P.Plane.need(plane(cs))), ",")

def states.go(rs: List<&2, P.PRow>) -> List<&2, String>:
  match rs:
    case Nil{}: Nil{}
    case r <> t: (P.PRow.id(r) ++ "=" ++ P.PRow.state(r)) <> states.go(t)

def states(cs: List<&2, M.Change>) -> String:
  String.join(states.go(P.Plane.need(plane(cs))), ",")

def counts(cs: List<&2, M.Change>) -> String:
  P.Plane.counts(I.En{}, plane(cs))

# the sidebar's count, made without rows
def tally(cs: List<&2, M.Change>) -> String:
  P.Plane.tally.words(ui(cs), 20n)

# the failures today: DeepSeek's turn failed (a funds decision), the old Pi
# run failed and was replaced by a working owner
def failed() -> List<&2, M.Change>:
  [M.TurnChanged{"bt", M.Completed{}, 10n}, M.TurnChanged{"int", M.Running{}, 11n},
    M.TurnChanged{"ds", M.Failed{}, 12n}, M.TurnChanged{"pi", M.Failed{}, 13n}, M.TurnChanged{"pi2", M.Running{}, 14n}]

def attrs(n: D.Node) -> List<&2, D.Attr>:
  match n:
    case D.El{_, _, as, _}: as
    case _: Nil{}

def tag(n: D.Node) -> String:
  match n:
    case D.El{t, _, _, _}: t
    case _: ""

def first(rs: List<&2, P.PRow>) -> P.PRow:
  match rs:
    case r <> _: r
    case Nil{}: P.PRow{"", "", "", "", "", "", "", "", 0n, False{}}

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("empty: nothing needs you", need(Nil{}), "")
    C.Check.str("empty: no count words", counts(Nil{}), "")
    C.Check.str("launchers do not count their launched failures again", states(failed()), "pi=failed,ds=failed")
    C.Check.str("failures counted apart from asks; the bot waits on its working launch", counts(failed()), "2 failed · 3 running")
    C.Check.str("sidebar tally agrees", tally(failed()), counts(failed()))
    C.Check.str("superseded failure settled: gone, history kept", need(List.append(&2, M.Change, failed(), [M.ThreadSettled{"pi", True{}, 15n}])), "ds")
    C.Check.str("settled count", counts(List.append(&2, M.Change, failed(), [M.ThreadSettled{"pi", True{}, 15n}])), "1 failed · 3 running")
    C.Check.str("a real approval on the launcher stays", states(List.append(&2, M.Change, failed(), [M.AskOpened{"q1", "int", "approval", "Bash", "git push", "", 16n}])), "int=approval,pi=failed,ds=failed")
    C.Check.str("ask and failures in words", counts(List.append(&2, M.Change, failed(), [M.AskOpened{"q1", "int", "approval", "Bash", "git push", "", 16n}])), "1 need you · 2 failed · 2 running")
    C.Check.str("tally with an ask agrees", tally(List.append(&2, M.Change, failed(), [M.AskOpened{"q1", "int", "approval", "Bash", "git push", "", 16n}])), counts(List.append(&2, M.Change, failed(), [M.AskOpened{"q1", "int", "approval", "Bash", "git push", "", 16n}])))
    C.Check.str("an answered ask is gone", need(List.append(&2, M.Change, failed(), [M.AskOpened{"q1", "int", "approval", "Bash", "ls", "", 16n}, M.AskClosed{"q1", "allow", 17n}])), "pi,ds")
    C.Check.str("a reply twice changes nothing", need(List.append(&2, M.Change, failed(), [M.AskOpened{"q1", "int", "approval", "Bash", "ls", "", 16n}, M.AskClosed{"q1", "allow", 17n}, M.AskClosed{"q1", "allow", 18n}])), "pi,ds")
    C.Check.str("a pending tool on the bot's own thread stays", states(List.append(&2, M.Change, failed(), [M.PendingChanged{"bt", 1n, 16n}])), "b1=approval,pi=failed,ds=failed")
    C.Check.str("a question on the bot stays a question", states(List.append(&2, M.Change, failed(), [M.AskOpened{"q2", "bt", "input", "", "Which?", "", 16n}])), "b1=question,pi=failed,ds=failed")
    C.Check.str("a delegated child's question shows at its parent", states([M.TaskDelegated{"k", "int", "kid", 1n, "claude", "", 6n}, M.ThreadCreated{"kid", "project", "Kid", M.Local{}, "", "", "claude", "", 6n}, M.AskOpened{"q3", "kid", "input", "", "Choose", "", 7n}]), "int=question")
    C.Check.str("a delegated child's failure shows at its parent", states([M.TaskDelegated{"k", "int", "kid", 1n, "claude", "", 6n}, M.ThreadCreated{"kid", "project", "Kid", M.Local{}, "", "", "claude", "", 6n}, M.TurnChanged{"kid", M.Failed{}, 7n}]), "int=failed")
    C.Check.str("a delegated child's question opens the child", P.Plane.hit(first(P.Plane.need(plane([M.TaskDelegated{"k", "int", "kid", 1n, "claude", "", 6n}, M.ThreadCreated{"kid", "project", "Kid", M.Local{}, "", "", "claude", "", 6n}, M.AskOpened{"q3", "kid", "input", "", "Choose", "", 7n}])))), "select kid")
    C.Check.str("a row opens its own thread", P.Plane.hit(first(P.Plane.need(plane(failed())))), "select pi")
    C.Check.str("why: approval", P.Plane.why(I.En{}, "approval"), "Approval")
    C.Check.str("why: question", P.Plane.why(I.En{}, "question"), "Question")
    C.Check.str("why: failed", P.Plane.why(I.En{}, "failed"), "Failed run")
    C.Check.str("why: none for running", P.Plane.why(I.En{}, "working"), "")
    C.Check.str("phone row leads with why", J.Json.get_str(MP.MPlane.row(I.En{}, first(P.Plane.need(plane(failed()))), 20n, U.Ui.state(ui(failed()))), "why", "missing"), "Failed run")
    C.Check.str("web uber row is a button", tag(WB.WB.uber.k(I.En{}, BU.BRow{"b1", "Bub", "bt", 0n, "idle", "", False{}, False{}, "", ""}, "2 failed")), "button")
