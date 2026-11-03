import Base
import ../star_nudge_ui_test.bend as F
import ../../src/core/client.bend as U
import ../../src/core/model.bend as M
import ../../src/core/json.bend as J
import ../../src/web/view.bend as V
import ../../src/web/presentation.bend as P
import ../../src/web/dom.bend as D
import ../../src/web/icon.bend as WI

# Production page with a real eligible offer, isolated from live user data.
def fixture() -> U.Ui:
  +u = F.eligible()
  +st = M.State.replay([
    M.MessagePosted{"t", "request", M.User{}, "Make the star suggestion easier to notice while keeping the conversation usable.", 12n},
    M.MessagePosted{"t", "reply", M.Assistant{}, "The suggestion stays in its own layout row. You can keep reading, writing, and working while it is visible.", 14n}], U.Ui.state(u))
  U.Ui.info_set(U.Ui.info_set(U.Act.ui(U.Ui.draft(U.Ui.adopt.raw(u, st), "Keep the suggestion in the layout.")), "local.workspace.layout", "rail-v1"), "local.workspace.chrome", "top-v1")

def page.first(nodes: List<&2, D.Node>) -> String:
  match nodes:
    case node <> _: J.Json.show(P.Node.write(node))
    case Nil{}: "null"

def page(_: Nat) -> String:
  page.first(V.View.expand([V.View.page(fixture())]))

def dismiss(_: Nat) -> String:
  J.Json.show(P.Node.write(V.View.page(U.Act.ui(U.Ui.act(fixture(), "star-dismiss", "offer")))))

def opened(_: Nat) -> String:
  J.Json.show(P.Node.write(V.View.page(U.Act.ui(U.Ui.act(fixture(), "star-open", "offer")))))

# Frozen pre-change strip markup for the visual comparison (base 273b268cd).
def before(_: Nat) -> String:
  J.Json.show(P.Node.write(D.El{"section", "star-nudge", [D.At{"class", "star-nudge"}, D.At{"role", "status"}], [
    D.El{"p", "", [D.At{"class", "star-nudge-text"}], [D.Txt{U.Star.body()}]},
    D.El{"a", "", [D.At{"class", "star-nudge-open"}, D.At{"href", U.Star.url()}, D.At{"target", "_blank"}, D.At{"rel", "noopener noreferrer"},
      D.At{"data-value", "offer"}, D.On{"click", "star-open"}], [D.Txt{"Star on GitHub"}]},
    D.El{"button", "", [D.At{"class", "star-nudge-dismiss"}, D.At{"type", "button"}, D.At{"aria-label", "Dismiss star suggestion"}, D.At{"title", "Dismiss star suggestion"},
      D.At{"data-value", "offer"}, D.On{"click", "star-dismiss"}], [WI.WIcon.svg("x", 16)]}]}))
