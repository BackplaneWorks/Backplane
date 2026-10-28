import Base
import ./check.bend as C
import ../src/core/model.bend as M
import ../src/core/client.bend as U
import ../src/core/json.bend as J
import ../src/core/hier.bend as H
import ../src/mobile/view.bend as MV

# The phone's viewer in a project with several boards (mobile/view.bend):
# opening it asks the hub for the project card, whose boards the Sheet menu
# offers; a board picked shows its own schematic and asks for its own
# hierarchy, another tab stays on it, and a sheet picked there is that
# board's sheet.

def log() -> List<&2, M.Change>:
  [M.ProjectCreated{"p1", "multi", "/m", 100n},
   M.ThreadCreated{"a", "p1", "Boards", M.Local{}, "", "", "claude", "", 300n}]

def ui(a: U.Act) -> U.Ui:
  match a:
    case U.Act{u, _}:
      u

def sent.all(cs: List<&2, U.Cmd>) -> String:
  match cs:
    case Nil{}:
      ""
    case U.Send{t} <> r:
      t ++ "\n" ++ sent.all(r)
    case _ <> r:
      sent.all(r)

def sent(a: U.Act) -> String:
  match a:
    case U.Act{_, cs}:
      sent.all(cs)

def has(+a: U.Act, +s: String) -> String:
  Bool.show(String.contains(sent(a), s))

# a thread as the hub-drawn view opens one: chosen, no action run
def opened() -> U.Ui:
  U.Bots.sel.set(U.Ui.fold(log(), U.Ui.init(1000n)), "a")

# the card a hub answers: the two boards, alpha the project's own
def carded() -> U.Ui:
  U.Ui.info_set(U.Ui.info_set(opened(), "fab.boards:a", "boards/alpha/alpha.kicad_pcb\nboards/beta/beta.kicad_pcb"), "fab.board:a", "boards/alpha/alpha.kicad_pcb")

def labels(js: List<&2, J.Json>) -> String:
  match js:
    case Nil{}:
      ""
    case +j <> t:
      J.Json.get_str(j, "label", "") ++ Bool.pick(String, J.Json.get_bool(j, "on", False{}), "*", "") ++ "|" ++ labels(t)

# beta's own hierarchy, as the plot actor sends it for the project's root
def tree() -> J.Json:
  J.Json.obj([J.KV{"t", J.Str{"sheets"}}, J.KV{"root", J.Str{"/m"}}, J.KV{"slot", J.Str{""}}, J.KV{"gen", J.Str{"1"}},
    J.KV{"rows", J.Str{H.Hier.show(H.Hier.rows([H.Kids{"/m/boards/beta/beta.kicad_sch", [H.Sheet{"Power", "/m/boards/beta/power.kicad_sch"}]}], "/m/boards/beta/beta.kicad_sch"))}},
    J.KV{"boxes", J.Str{""}}])

def beta() -> U.Ui:
  ui(MV.View.sheet.pick(ui(MV.View.open.k(carded(), "schematic")), "b:boards/beta/beta.kicad_pcb"))

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("opening the viewer asks for the project card", has(MV.View.open.k(opened(), "board"), "fab.look"), "True")
    C.Check.str("once", has(MV.View.open.k(ui(MV.View.open.k(opened(), "board")), "schematic"), "fab.look"), "False")
    C.Check.str("the Board tab offers each board, the project's own lit", labels(MV.View.board.list(ui(MV.View.open.k(carded(), "board")))), "alpha*|beta|")
    C.Check.str("one board offers none", labels(MV.View.board.list(ui(MV.View.open.k(opened(), "board")))), "")
    C.Check.str("a board picked on Schematic shows its schematic", MV.View.file(beta()), "/m/boards/beta/beta.kicad_sch")
    C.Check.str("and asks for its hierarchy", has(MV.View.sheet.pick(ui(MV.View.open.k(carded(), "schematic")), "b:boards/beta/beta.kicad_pcb"), "\"tree\":\"/m/boards/beta/beta.kicad_sch\""), "True")
    C.Check.str("it is lit", labels(MV.View.board.list(beta())), "alpha|beta*|")
    C.Check.str("Board stays on it", MV.View.file(ui(MV.View.open.k(beta(), "board"))), "/m/boards/beta/beta.kicad_pcb")
    C.Check.str("the menu lists its sheets, its root on show", labels(MV.View.sheet.list(U.Sch.got(beta(), tree()))), "beta.kicad_sch*|  Power|")
    C.Check.str("a sheet picked is that board's", MV.View.file(ui(MV.View.sheet.pick(U.Sch.got(beta(), tree()), "1"))), "/m/boards/beta/power.kicad_sch")
    C.Check.str("and is the one lit", labels(MV.View.sheet.list(ui(MV.View.sheet.pick(U.Sch.got(beta(), tree()), "1")))), "↑ Up|beta.kicad_sch|  Power*|")
    C.Check.str("the project's own needs no hierarchy named", has(MV.View.open.k(opened(), "schematic"), "\"tree\":\"\""), "True")
