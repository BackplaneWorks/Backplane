import Base
import ../../src/core/client.bend as U
import ../../src/core/model.bend as M
import ../../src/core/hist.bend as H
import ../../src/core/i18n.bend as I
import ../../src/core/json.bend as J
import ../../src/web/view.bend as W
import ../../src/web/dom.bend as D
import ../../src/mobile/view.bend as P

# Generated history, never a real event store. Include board and schematic
# edits and user text before each step, as a long CAD conversation does.
def changes(+n: Nat, acc: List<&2, M.Change>) -> List<&2, M.Change>:
  match n:
    case 0n:
      acc
    case 1n+k:
      +id = Nat.show(1n+k)
      changes(k,
        M.MessagePosted{"t", "u" ++ id, M.User{}, "Route stage " ++ id ++ " 日本語", 1n+k} <>
        M.ActivityLogged{"t", "step:c" ++ id, "step", "kicad",
          H.Hist.record("c" ++ id ++ "\nc" ++ Nat.show(k) ++ "\n/r/.git\n/r\nM\0b.kicad_pcb\0M\0s.kicad_sch\0", "u" ++ id), 1n+k} <> acc)

def ui(a: U.Act) -> U.Ui:
  match a:
    case U.Act{u, _}: u

def make(n: Nat) -> U.Ui:
  U.Ui.view_put(ui(U.Ui.act(U.Ui.adopt(U.Ui.init(0n), M.State.replay(
    M.ThreadCreated{"t", "", "Synthetic CAD history", M.Local{}, "", "", "claude", "", 0n} <> changes(n, Nil{}), M.State.empty())), "select", "t")), "board", "/r/b.kicad_pcb")

def variant(+u: U.Ui, +v: String) -> U.Ui:
  match v:
    case "schematic":
      U.Ui.view_put(u, "schematic", "/r/s.kicad_sch")
    case "compare":
      ui(U.Ui.act(ui(U.Ui.act(u, "cmp", "")), "cmp-pick", "a\tc1\tStep 1\t/r/.git\t/r"))
    case _:
      ui(U.Ui.act(u, v, ""))

def bar(+u: U.Ui, +l: I.Loc) -> U.HBar:
  U.Hs.bar.loc(l, u)

def web(+u: U.Ui, +l: I.Loc) -> List<&2, D.Node>:
  W.View.hist.of(l, U.Hs.track(u), u)

def webmenu(+u: U.Ui, +l: I.Loc) -> D.Node:
  W.View.hist.menu(l, U.Hs.track(u), u)

def phone(+u: U.Ui, +l: I.Loc) -> String:
  J.Json.show(P.Hist.json(l, True{}, u))
