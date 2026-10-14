import Base
import ../../src/web/dom.bend as D
import ../../src/web/view.bend as V
import ../../src/core/client.bend as U
import ../../src/core/codec.bend as C
import ../../src/core/json.bend as J
import ../../src/core/model.bend as M
import ../trb_seed.bend as S
import ./space_preview.bend as H
import ../../src/web/cat.bend as WC

# A reproducible web view of the same fictional log as trb_snap.bend.
def screen(+view: String, +theme: String) -> U.Ui:
  +u = S.ui(theme)
  Bool.pick(U.Ui, String.eq(view, "rules-form"), S.settings(u, True{}), Bool.pick(U.Ui, String.eq(view, "rules-closed"), S.settings(u, False{}),
    Bool.pick(U.Ui, String.eq(view, "status"), S.status(u), S.board(u))))

def page(u: U.Ui) -> String:
  H.Html.nodes([V.View.page(u)])

def html(view: String, theme: String) -> String:
  page(screen(view, theme))

def act(u: U.Ui, a: String, v: String) -> U.Act:
  U.Ui.act(u, a, v)

def body(u: U.Ui) -> String:
  page(u)

# Persistable event JSON for an isolated test home.
def seed.lines(cs: List<&2, M.Change>) -> List<&2, String>:
  match cs:
    case Nil{}:
      Nil{}
    case c <> t:
      J.Json.show(C.Change.encode(c)) <> seed.lines(t)

def seed() -> String:
  String.join(seed.lines(S.log()), "\n")

def cat(name: String) -> String:
  WC.WCat.uri(name)
