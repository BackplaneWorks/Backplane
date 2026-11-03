import Base
import ./check.bend as C
import ../src/core/model.bend as M
import ../src/core/client.bend as U
import ../src/core/i18n.bend as I
import ../src/core/json.bend as J
import ../src/web/view.bend as V
import ../src/web/dom.bend as D
import ../src/web/presentation.bend as W

# A red sidebar row says what it wants where its age goes (mockup n1103
# r01): an approval or a question in red, a failed turn in grey with a
# hollow square; the age moves to the tooltip. Other rows keep their age.

def st() -> M.State:
  M.State.replay([M.ProjectCreated{"p", "Test", "/tmp/sidebar-want", 1n},
    M.ThreadCreated{"ap", "p", "Battery life estimate", M.Local{}, "", "", "claude", "", 1n}, M.TurnChanged{"ap", M.Running{}, 2n}, M.PendingChanged{"ap", 1n, 3n},
    M.ThreadCreated{"q", "p", "Grounding plan", M.Local{}, "", "", "claude", "", 1n}, M.TurnChanged{"q", M.Running{}, 2n}, M.AskOpened{"a1", "q", "input", "AskUserQuestion", "Which ground?", "", 3n}, M.PendingChanged{"q", 1n, 3n},
    M.ThreadCreated{"f", "p", "Firmware pin map", M.Local{}, "", "", "claude", "", 1n}, M.TurnChanged{"f", M.Running{}, 2n}, M.TurnChanged{"f", M.Failed{}, 3n},
    M.ThreadCreated{"w", "p", "I2C pull-ups", M.Local{}, "", "", "claude", "", 1n}, M.TurnChanged{"w", M.Running{}, 2n},
    M.ThreadCreated{"d", "p", "Bias the tube stage", M.Local{}, "", "", "claude", "", 1n}, M.TurnChanged{"d", M.Running{}, 2n}, M.TurnChanged{"d", M.Completed{}, 3n}], M.State.empty())

def ui() -> U.Ui:
  U.Side.with_signals(U.Ui.online(U.Ui.adopt.raw(U.Ui.init(1300n), st()), True{}))

def ix() -> U.SIx:
  U.Side.signals.ix(U.Side.ix(st()), ui())

def attrs(n: D.Node) -> List<&2, D.Attr>:
  match n:
    case D.El{_, _, as, _}: as
    case _: Nil{}

def row(+lc: I.Loc, +id: String) -> D.Node:
  V.View.row(lc, U.Ui.thread(st(), id), ix(), st(), "", 1300n, False{}, False{})

def want(+id: String) -> String:
  V.View.attr(attrs(row(I.En{}, id)), "want")

def html(+id: String) -> String:
  J.Json.show(W.Node.write(V.View.make(row(I.En{}, id))))

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("an approval says approval", want("ap"), "approval")
    C.Check.str("a question says question", want("q"), "question")
    C.Check.str("a failed turn says failed", want("f"), "failed")
    C.Check.str("a working row keeps its age", want("w"), "")
    C.Check.str("a finished row keeps its age", want("d"), "")
    C.Check.str("japanese words", V.View.attr(attrs(row(I.Ja{}, "ap")), "want") ++ "|" ++ V.View.attr(attrs(row(I.Ja{}, "q")), "want") ++ "|" ++ V.View.attr(attrs(row(I.Ja{}, "f")), "want"), "承認|質問|失敗")
    C.Check.str("the failed dot is its own", V.View.attr(attrs(row(I.En{}, "f")), "dot"), "failed")
    C.Check.str("the word takes the age's place", Bool.show(String.contains(html("q"), "ago want")), "True")
    C.Check.str("the age moves to the tooltip", Bool.show(String.contains(html("q"), "question · ")), "True")
    C.Check.str("a working row draws its age", Bool.show(String.contains(html("w"), "ago want")), "False")
    C.Check.str("a question counted pending still says question", U.Side.want.ask([M.Ask{"a", "q", "input", "AskUserQuestion", "?", "", True{}, ""}], 1n), "question")
    C.Check.str("an approval before a question", U.Side.want.ask([M.Ask{"a", "q", "input", "AskUserQuestion", "?", "", True{}, ""}, M.Ask{"b", "q", "approval", "Bash", "ls", "", True{}, ""}], 2n), "approval")
    C.Check.str("a red dot without an ask of its own says nothing", U.Side.want(I.En{}, "blocked", Nil{}, U.Ui.thread(st(), "w")), "")
    C.Check.str("a green dot says nothing", U.Side.want(I.En{}, "finished", Nil{}, U.Ui.thread(st(), "ap")), "")
