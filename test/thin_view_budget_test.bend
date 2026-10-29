import Base
import ./check.bend as C
import ../src/server/thin-view.bend as V
import ../src/core/thin.bend as T
import ../src/core/model.bend as M
import ../src/core/json.bend as J
import ../src/core/thin-wire.bend as W

def request() -> J.Json:
  Maybe.default(&2, J.Json, J.Json.parse("{\"hub\":\"h\",\"cid\":\"c\",\"bodyPages\":1,\"target\":{\"kind\":\"thread\",\"id\":\"a\"},\"session\":{\"sel\":\"a\",\"rid\":12,\"drafts\":{\"a\":\"kept 日本語\"},\"open\":{},\"info\":{}},\"action\":\"\",\"value\":\"\"}"), J.Null{})

def seed() -> M.State:
  M.State.apply(M.State.empty(), M.ThreadCreated{"a", "", "Thread", M.Local{}, "", "", "codex", "", 1n})

def rows(+n: Nat, +text: String, st: M.State) -> M.State:
  match n:
    case 0n: st
    case 1n+k: rows(k, text, M.State.apply(st, M.MessagePosted{"a", "m" ++ Nat.show(n), M.Assistant{}, text, Nat.add(n, 2n)}))

def text(n: Nat, acc: String) -> String:
  match n:
    case 0n: acc
    case 1n+k: text(k, "<>&\"日本語😀 bounded content\n" ++ acc)

def render(+st: M.State) -> J.Json:
  +i = T.Index.of(st)
  V.Render.view(True{}, st, i, J.Json.obj(Nil{}), J.Json.obj(Nil{}), 100n, request(), "e", T.Index.page(i, "a", 0n, 32n))

def main() -> IO(Unit):
  do IO<Unit>:
    +big : M.State = rows(1n, text(9000n, "END-OF-HUGE-MESSAGE"), seed())
    +one : J.Json = render(big)
    C.Check.str("one legitimate huge message fits unchanged reply cap", Bool.show(W.Wire.within(J.Json.show(one), 250000n)), "True")
    C.Check.str("huge message has an accessible lazy body continuation", Bool.show(Nat.is_gt(List.length(&2, J.Json, J.Json.elems(J.Json.get_obj(J.Json.get_obj(one, "window"), "partial"))), 0n)), "True")
    C.Check.str("oversized view preserves current draft", J.Json.get_str(J.Json.get_obj(J.Json.get_obj(one, "session"), "drafts"), "a", ""), "kept 日本語")
    +dense : M.State = rows(32n, text(260n, "ROW-END"), seed())
    +many : J.Json = render(dense)
    C.Check.str("32 rich messages fit without losing message identities", Bool.show(W.Wire.within(J.Json.show(many), 250000n)), "True")
    C.Check.str("latest ordinary window retains every selected entry", Nat.show(List.length(&2, J.Json, J.Json.elems(J.Json.get_obj(J.Json.get_obj(many, "window"), "ids")))), "32")
    C.Check.str("read-only body projection produces no commands", Nat.show(List.length(&2, J.Json, J.Json.elems(J.Json.get_obj(one, "cmds")))), "0")
