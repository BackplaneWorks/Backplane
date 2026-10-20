import Base
import ../../src/core/client.bend as U
import ../../src/core/i18n.bend as I
import ../../src/web/view.bend as V
import ../../src/web/dom.bend as D

def state(a: U.Act) -> U.Ui:
  match a:
    case U.Act{u, _}: u

def init() -> U.Ui:
  state(U.Ui.act(U.Ui.init(0n), "flag", "settings"))

def act(u: U.Ui, a: String, v: String) -> U.Ui:
  state(U.Ui.act(u, a, v))

def view(+u: U.Ui, +ja: Bool) -> D.Node:
  V.View.settings.loc(Bool.pick(I.Loc, ja, I.Ja{}, I.En{}), u, U.Ui.state.of(u), Map.new(&2, String), "review", "")
