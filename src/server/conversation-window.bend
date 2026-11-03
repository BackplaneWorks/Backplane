import Base
import ../core/json.bend as J
import ../core/client.bend as U
import ../web/dom.bend as D
import ../web/presentation.bend as W
import ./thin-view.bend as V

# A completed normal render is one atomic, read-only presentation region.
# It is separately framed like exact session/control values. Source window
# and body budgets are enforced before rendering; wire packet budgets are
# independent of aggregate region/control bytes. No row renderer, clipping,
# stream, read boundary, continuation or viewer lifecycle is changed here.
# Framing transports the complete canonical picture, including project and
# thread navigation. Packet budgets are enforced by control chunking; they
# must never replace the sidebar with a reduced navigation stub.
def Window.root(j: J.Json) -> J.Json:
  j

def Window.picture() -> J.Json:
  W.Node.write(D.El{"div", "workspace-root", [D.At{"data-window-region", "root"}], Nil{}})

def Window.make(+result: J.Json) -> J.Json:
  +root = Window.root(J.Json.get_obj(result, "shown"))
  +projection = J.Json.obj([J.KV{"root", root}, J.KV{"logicalBytes", J.Json.nat(J.Json.bytes(root))}])
  V.Obj.put(V.Obj.put(V.Obj.put(result, "shown", Window.picture()), "windowProjection", projection), "boundedFrame", J.Num{"1"})

# Legacy clients receive the normal whole canonical tree.
# Region/control framing is used only after explicit request negotiation.
def Window.drop(j: J.Json, key: String) -> J.Json:
  match j:
    case J.Obj{fs}: J.Obj{V.Obj.drop(fs, key)}
    case other: other

def Window.prepare.clean(regions: Bool, +clean: J.Json) -> J.Json:
  match regions:
    case True{}: Window.make(clean)
    case False{}: V.Obj.put(V.Obj.put(clean, "shown", Window.root(J.Json.get_obj(clean, "shown"))), "boundedFrame", J.Num{"1"})

# A navigation/control ACK without shown must stay an ACK, not acquire a
# synthetic picture. The final framing protocol already supports no shown.
def Window.prepare.visible(visible: Bool, regions: Bool, clean: J.Json) -> J.Json:
  match visible:
    case False{}: V.Obj.put(clean, "boundedFrame", J.Num{"1"})
    case True{}: Window.prepare.clean(regions, clean)

def Window.prepare(regions: Bool, result: J.Json) -> J.Json:
  +clean = Window.drop(Window.drop(result, "sidebarRegion"), "viewDelta")
  Window.prepare.visible(Maybe.is_some(&2, J.Json, J.Json.get(clean, "shown")), regions, clean)
