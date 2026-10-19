import Base
import ../core/client.bend as U
import ../core/codec.bend as C
import ../core/model.bend as M
import ../core/json.bend as J
import ../core/i18n.bend as I
import ../core/msgs.bend as MSG

# Use the ordinary thread.modes RPC. The menu never changes a global default,
# and the screen continues to show the persisted mode until its event arrives.
def Permissions.valid(+value: String) -> Bool:
  Nat.is_lt(C.Codec.pos(["approval-required", "auto-accept-edits", "auto", "full-access"], value), 4n)

def Permissions.set.k(valid: Bool, +u: U.Ui, value: String) -> U.Act:
  match valid:
    case True{}:
      U.Ui.on_thread.p(u, "thread.modes", [J.KV{"runtime", J.Json.str(value)}])
    case False{}:
      U.Act{u, Nil{}}

def Permissions.set(+u: U.Ui, +value: String) -> U.Act:
  Permissions.set.k(Bool.and(Permissions.valid(value), Bool.not(String.is_empty(U.Ui.sel(u)))), u, value)

def Permissions.act.k(hit: Bool, +u: U.Ui, action: String, value: String) -> U.Act:
  match hit:
    case True{}: Permissions.set(u, value)
    case False{}: U.Ui.act(u, action, value)

def Permissions.act(+u: U.Ui, +action: String, value: String) -> U.Act:
  Permissions.act.k(String.eq(action, "thread-permissions"), u, action, value)

def Permissions.label(lc: I.Loc, r: M.RuntimeMode) -> String:
  match r:
    case M.Supervised{}: MSG.T.native.thread.permissions.approval(lc)
    case M.AcceptEdits{}: MSG.T.native.thread.permissions.edits(lc)
    case M.AutoMode{}: MSG.T.native.thread.permissions.auto(lc)
    case M.FullAccess{}: MSG.T.native.thread.permissions.full(lc)

def Permissions.choice(+lc: I.Loc, +r: M.RuntimeMode, selected: String) -> J.Json:
  J.Json.obj([J.KV{"label", J.Json.str(Permissions.label(lc, r))}, J.KV{"value", J.Json.str(C.Runtime.name(r))}, J.KV{"on", J.Json.bool(String.eq(C.Runtime.name(r), selected))}])

def Permissions.json(+lc: I.Loc, +r: M.RuntimeMode) -> J.Json:
  J.Json.obj([J.KV{"label", J.Json.str(MSG.T.native.thread.permissions(lc))},
    J.KV{"current", J.Json.str(MSG.T.native.thread.permissions.current(lc, Permissions.label(lc, r)))},
    J.KV{"choices", J.Json.arr([Permissions.choice(lc, M.Supervised{}, C.Runtime.name(r)), Permissions.choice(lc, M.AcceptEdits{}, C.Runtime.name(r)), Permissions.choice(lc, M.AutoMode{}, C.Runtime.name(r)), Permissions.choice(lc, M.FullAccess{}, C.Runtime.name(r))])}])
