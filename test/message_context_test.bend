import Base
import ./check.bend as C
import ../src/core/model.bend as M
import ../src/core/thin.bend as T
import ../src/core/json.bend as J
import ../src/core/thin-live.bend as L
import ../src/server/thin-view.bend as V
import ../src/core/message-context.bend as MC
import ../src/core/fold.bend as F
import ../src/core/lazy-tools.bend as LT
import ../src/core/i18n.bend as I

def tools(+n: Nat, acc: M.State) -> M.State:
  match n:
    case 0n: acc
    case 1n+k: tools(k, M.State.post(acc, M.Act{"call-" ++ Nat.show(n), "t", "tool", "shell", "Inspect design", n}))

def state() -> M.State:
  tools(1100n, M.State.post(M.State.post(M.State.empty(), M.Msg{"human", "t", M.User{}, "Please fix it", 1n}), M.Msg{"agent", "t", M.Assistant{}, "I will check", 2n}))

def window(+st: M.State, +index: T.Index, +before: Nat, +action: String, +value: String) -> J.Json:
  V.Render.window.context(st, L.Stream.blank(), index, "t", J.Json.obj([
    J.KV{"action", J.Str{action}}, J.KV{"value", J.Str{value}}, J.KV{"target", V.Target.json("thread", "t")}]), "epoch", T.Index.page(index, "t", before, 32n))

def count(j: J.Json, key: String) -> String:
  Nat.show(List.length(&2, J.Json, J.Json.elems(J.Json.get_obj(j, key))))

def roots(+n: Nat, acc: List<&2, M.Entry>) -> List<&2, M.Entry>:
  match n:
    case 0n: acc
    case 1n+k: roots(k, M.Act{"call-" ++ Nat.show(n), "t", "context", "call-1100", "", 0n} <> acc)

def head(rs: List<&2, F.Role>) -> String:
  match rs:
    case F.RHead{id, _, open, _} <> _: id ++ ":" ++ Bool.show(open)
    case _: "missing"

def main() -> IO(Unit):
  do IO<Unit>:
    +st : M.State = state()
    +index : T.Index = T.Index.of(st)
    +newest : J.Json = window(st, index, 0n, "", "")
    +older : J.Json = window(st, index, 800n, "", "")
    C.Check.str("wire page retains its exact raw ordinals", count(newest, "ids"), "32")
    C.Check.str("1100 closed tools spend one display row", count(newest, "historyUnits"), "1")
    C.Check.str("pages inside a run share the real display identity", J.Json.show(J.Json.get_obj(newest, "historyUnits")), J.Json.show(J.Json.get_obj(older, "historyUnits")))
    C.Check.str("latest human and agent text survive a long tool tail", J.Json.show(J.Json.get_obj(newest, "contextMessages")), "[\"agent\",\"human\"]")
    C.Check.str("tools are never reported as message text", count(newest, "messageIds"), "0")
    +open : J.Json = window(st, index, 0n, "fold", "1call-1100")
    C.Check.str("explicitly expanded calls each spend a display row", count(open, "historyUnits"), "33")
    +text : J.Json = window(st, index, 3n, "", "")
    C.Check.str("the source text page identifies both committed messages", count(text, "messageIds"), "2")
    +hot : M.State = V.State.window.file(V.State.window.file(tools(200n, M.State.empty()), MC.Context.key("t"),
      [M.Msg{"agent", "t", M.Assistant{}, "context", 2n}, M.Msg{"human", "t", M.User{}, "context", 1n}]), MC.Context.runs.key("t"), roots(32n, Nil{}))
    +restored : J.Json = window(hot, index, 0n, "", "")
    C.Check.str("stored text identities survive a tool-only startup state", J.Json.show(J.Json.get_obj(restored, "contextMessages")), "[\"agent\",\"human\"]")
    C.Check.str("stored run identity survives a truncated startup context", J.Json.show(J.Json.get_obj(restored, "historyUnits")), "[\"fold:call-1100\"]")
    +ids : List<&2, String> = T.Index.page(index, "t", 0n, 32n)
    +bounded : M.State = V.State.window.body(False{}, hot, index, V.Target.json("thread", "t"), ids)
    C.Check.str("render state keeps the stored text context", String.join(MC.Context.targets(bounded, "t"), ","), "agent,human")
    +es : List<&2, M.Entry> = LT.Tools.entries(ids, index, Nil{})
    +info : Map<&2, String> = Map.set(&2, String, Map.new(&2, String), "fold:call-1100", "1")
    +roles : List<&2, F.Role> = F.Fold.roles.all(I.En{}, F.Fold.context.of(bounded, "t", es), es, info, False{})
    C.Check.str("rendered fold uses the stored identity and its explicit open choice", head(MC.Context.roles(es, roles, bounded, "t", info)), "call-1100:True")
    +allowed : Map<&2, Bool> = LT.Tools.allowed(hot, index, "t", ids, J.Json.obj([J.KV{"action", J.Str{"fold"}}, J.KV{"value", J.Str{"1call-1100"}}]))
    C.Check.str("canonical drawer expansion admits its selected tool bodies", Bool.show(LT.Tools.found(Map.get(Bool, False{}, allowed, "call-1"))), "True")
