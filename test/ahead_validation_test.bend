import Base
import ../src/core/client.bend as U
import ../src/core/model.bend as M
import ../src/core/json.bend as J
import ../src/core/cbor.bend as CB
import ../src/web/view.bend as V
import ../src/web/presentation.bend as P

# Production functions exposed for the browser-free driver. No alternate UI model.
def init() -> U.Ui:
  U.Ui.init(100n)

def step(u: U.Ui, action: String, value: String) -> U.Act:
  U.Ui.act(u, action, value)

def received(u: U.Ui, frame: J.Json) -> U.Act:
  U.Ui.recv_act(u, frame)

def ui(a: U.Act) -> U.Ui:
  match a:
    case U.Act{u, _}: u

def commands(a: U.Act) -> List<&2, U.Cmd>:
  match a:
    case U.Act{_, cs}: cs

def tree(u: U.Ui) -> J.Json:
  P.Node.write(V.View.page(u))

def composer(u: U.Ui) -> J.Json:
  P.Node.write(V.View.composer.u(u))

def selected(u: U.Ui) -> String:
  U.Ui.sel(u)

def draft(u: U.Ui) -> String:
  U.Ui.draft_text(u)

def pill(+u: U.Ui) -> String:
  U.Hold.pill(u, U.Ui.sel(u))

def entries(+u: U.Ui) -> List<&2, M.Entry>:
  M.State.mine(U.Ui.state(u), U.Ui.sel(u))

def fitted(u: U.Ui) -> U.Ui:
  U.Ui.info_set(u, "local.body.budget", "1")

def pending(+u: U.Ui) -> List<&2, String>:
  U.Ui.pending(u, U.Ui.sel(u))

def parse(text: String) -> J.Json:
  Maybe.default(&2, J.Json, J.Json.parse(text), J.Null{})

def encode(j: J.Json) -> List<&2, U32>:
  CB.Cbor.encode(j)

def decode(bs: List<&2, U32>) -> J.Json:
  Maybe.default(&2, J.Json, CB.Cbor.decode(bs), J.Null{})

def json_tags() -> J.Json:
  J.Arr{J.Item{J.Null{}, J.Item{J.Flag{False{}}, J.Item{J.Num{"0"}, J.Item{J.Str{""}, J.Item{J.Obj{J.Field{"", J.Null{}, J.End{}}}, J.End{}}}}}}}
