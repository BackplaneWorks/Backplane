import Base
import ./check.bend as C
import ../src/core/model.bend as M
import ../src/core/thin.bend as T
import ../src/core/json.bend as J
import ../src/core/thin-live.bend as L
import ../src/server/thin-view.bend as V

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
