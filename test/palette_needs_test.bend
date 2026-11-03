import Base
import ./check.bend as C
import ../src/core/client.bend as U
import ../src/core/model.bend as M
import ../src/core/json.bend as J
import ../src/web/workspace.bend as WS
import ../src/web/dom.bend as D
import ../src/web/presentation.bend as W

# Opened with nothing typed, Commands lists the threads waiting on the
# person first (mockup n1103 r05), in the sidebar's order (Urg.rank: an
# approval, a question, then a failed turn; newest first), each with its
# dot and what it wants, the first selected; every thread row shows its
# dot in place of the speech bubble.

def log() -> List<&2, M.Change>:
  [M.ProjectCreated{"p", "Sensor-node", "/fixture", 1n},
   M.ThreadCreated{"t", "p", "Open one", M.Local{}, "", "/fixture", "claude", "", 1n},
   M.ThreadCreated{"f", "p", "Firmware pin map", M.Local{}, "", "/fixture", "claude", "", 2n}, M.TurnChanged{"f", M.Running{}, 3n}, M.TurnChanged{"f", M.Failed{}, 40n},
   M.ThreadCreated{"q", "p", "Grounding plan", M.Local{}, "", "/fixture", "claude", "", 2n}, M.TurnChanged{"q", M.Running{}, 3n}, M.AskOpened{"a1", "q", "input", "AskUserQuestion", "Which ground?", "", 30n},
   M.ThreadCreated{"ap", "p", "Battery life estimate", M.Local{}, "", "/fixture", "claude", "", 2n}, M.TurnChanged{"ap", M.Running{}, 3n}, M.PendingChanged{"ap", 1n, 20n},
   M.ThreadCreated{"f2", "p", "Older failure", M.Local{}, "", "/fixture", "claude", "", 2n}, M.TurnChanged{"f2", M.Running{}, 3n}, M.TurnChanged{"f2", M.Failed{}, 10n},
   M.ThreadCreated{"w", "p", "I2C pull-ups", M.Local{}, "", "/fixture", "claude", "", 2n}, M.TurnChanged{"w", M.Running{}, 3n},
   M.ThreadCreated{"z", "p", "Snoozed question", M.Local{}, "", "/fixture", "claude", "", 2n}, M.TurnChanged{"z", M.Running{}, 3n}, M.AskOpened{"a2", "z", "input", "AskUserQuestion", "?", "", 31n}, M.ThreadSnoozed{"z", 5000n, 32n},
   M.ThreadCreated{"ar", "p", "Archived approval", M.Local{}, "", "/fixture", "claude", "", 2n}, M.TurnChanged{"ar", M.Running{}, 3n}, M.PendingChanged{"ar", 1n, 33n}, M.ThreadArchived{"ar", True{}, 34n},
   M.ThreadCreated{"s", "p", "Settled failure", M.Local{}, "", "/fixture", "claude", "", 2n}, M.TurnChanged{"s", M.Running{}, 3n}, M.TurnChanged{"s", M.Failed{}, 35n}, M.ThreadSettled{"s", True{}, 36n}]

def ui() -> U.Ui:
  U.Side.with_signals(U.Ui.select.k(U.Ui.fold(log(), U.Ui.init(1000n)), "t"))

def labels(xs: List<&2, U.PItem>) -> List<&2, String>:
  match xs:
    case Nil{}: Nil{}
    case U.PItem{l, _, _} <> t: l <> labels(t)

def first(+n: Nat, +q: String) -> String:
  String.join(List.take(&2, String, labels(WS.WS.items(U.Palette.set(ui(), q, 0n))), n), ",")

def ui.of(a: U.Act) -> U.Ui:
  match a:
    case U.Act{u, _}: u

def html(+q: String) -> String:
  J.Json.show(J.Json.arr(W.Node.write.go(WS.WS.palette(True{}, U.Palette.set(ui(), q, 0n)), Nil{})))

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("nothing typed: approval, question, then failures newest first, then the commands", first(5n, ""), "Battery life estimate,Grounding plan,Firmware pin map,Older failure,New subthread of current thread")
    C.Check.str("snoozed, archived and settled threads wait on nothing now", Bool.show(Bool.or(String.contains(first(24n, ""), "Snoozed question"), Bool.or(String.contains(first(24n, ""), "Archived approval"), String.contains(first(24n, ""), "Settled failure")))), "False")
    C.Check.str("only red rows: working and open threads come later", Bool.show(String.contains(first(4n, ""), "I2C pull-ups")), "False")
    C.Check.str("typed: unchanged, no waiting list", first(1n, "new thread"), "New thread · Ctrl+N")
    C.Check.str("Ctrl+K Enter opens the first", U.Ui.sel(ui.of(WS.WS.palette.run(U.Palette.set(ui(), "", 0n), 0n))), "ap")
    C.Check.str("a thread row draws its dot, not the bubble", Bool.show(String.contains(html(""), "dot blocked")) ++ Bool.show(String.contains(html(""), "dot failed")) ++ Bool.show(String.contains(html(""), "\"svg\"")), "TrueTrueFalse")
    C.Check.str("and says what it wants", Bool.show(String.contains(html(""), "\"approval\"")) ++ Bool.show(String.contains(html(""), "\"question\"")) ++ Bool.show(String.contains(html(""), "\"failed\"")), "TrueTrueTrue")
    C.Check.str("a typed thread row has its dot and word too", Bool.show(String.contains(html("grounding"), "dot blocked")) ++ Bool.show(String.contains(html("grounding"), "\"question\"")) ++ Bool.show(String.contains(html("pull-ups"), "dot inactive")), "TrueTrueTrue")
    C.Check.str("helpers: want of a red row", U.Palette.want(U.Lang.loc(ui()), ui(), "blocked", U.Ui.thread(U.Ui.state(ui()), "q")), "question")
