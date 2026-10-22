import Base
import ./check.bend as C
import ../src/server/thin-view.bend as V
import ../src/core/thin.bend as T
import ../src/core/model.bend as M
import ../src/core/json.bend as J
import ../src/core/client.bend as U
import ../src/core/thin-wire.bend as W

def obj(s: String) -> J.Json:
  Maybe.default(&2, J.Json, J.Json.parse(s), J.Null{})

def yes(b: Bool) -> String:
  Bool.pick(String, b, "yes", "no")

def state() -> M.State:
  M.State.apply(M.State.apply(M.State.empty(), M.ThreadCreated{"a", "", "Selected", M.Local{}, "", "", "codex", "", 1n}), M.MessagePosted{"a", "m", M.Assistant{}, "日本語 exact message", 2n})

def target() -> J.Json:
  V.Target.json("thread", "a")

def request() -> J.Json:
  obj("{\"hub\":\"h\",\"cid\":\"device\",\"target\":{\"kind\":\"thread\",\"id\":\"a\"},\"session\":{\"sel\":\"a\",\"rid\":100,\"drafts\":{\"a\":\"captured draft\"},\"open\":{},\"info\":{}},\"action\":\"\",\"value\":\"\"}")

def cmd.count(a: V.ActResult) -> Nat:
  match a:
    case V.ActResult{_, cs}: List.length(&2, J.Json, cs)

def has.text(es: List<&2, M.Entry>, +x: String) -> Bool:
  match es:
    case Nil{}: False{}
    case +e <> t: Bool.or(String.eq(T.Entry.text(e), x), has.text(t, x))

def main() -> IO(Unit):
  do IO<Unit>:
    +st : M.State = state()
    +i : T.Index = T.Index.of(st)
    C.Check.str("actual selected thread exists", yes(V.Target.exists(st, target())), "yes")
    C.Check.str("missing target refused before rendering", yes(V.Target.exists(st, V.Target.json("thread", "foreign"))), "no")
    C.Check.str("bounded window contains only selected entries", Nat.show(List.length(&2, M.Entry, M.State.entries.all(V.State.window(st, i, target(), ["1"])))), "1")
    C.Check.str("window never mutates authoritative state", Nat.show(List.length(&2, M.Entry, M.State.entries.all(st))), "1")
    +u : U.Ui = V.Session.input(st, obj("{}"), obj("{}"), 5n, request())
    C.Check.str("current thin adapter has no provisional-only guard", U.Ui.scratch(u, "@presentation"), "")
    C.Check.str("current thin adapter binds exact canonical selected thread", U.Ui.sel.of(u), "a")
    C.Check.str("current thin action retains original draft", U.Ui.scratch(u, "a"), "captured draft")
    C.Check.str("pure initial view has no issued commands", Nat.show(cmd.count(V.Action.run(True{}, u, request(), 5n))), "0")
    +window : J.Json = V.Page.window(i, "a", request(), "epoch", ["1"])
    C.Check.str("complete single entry has no partial marker", Nat.show(List.length(&2, J.Json, J.Json.elems(J.Json.get_obj(window, "partial")))), "0")
    C.Check.str("last page explicitly has no older entries", yes(J.Json.get_bool(window, "hasMore", True{})), "no")
    +large : T.Index = T.Index.post(i, M.Msg{"large", "a", M.Assistant{}, String.repeat("😀", 129n), 3n})
    +partial : J.Json = V.Page.window(large, "a", request(), "epoch", ["2"])
    C.Check.str("a long message is never partial", Nat.show(List.length(&2, J.Json, J.Json.elems(J.Json.get_obj(partial, "partial")))), "0")
    C.Check.str("visible preview never rewrites original body", T.Entry.text(T.Row.entry(T.Index.row(large, 2n))), String.repeat("😀", 129n))
    C.Check.str("a long message is shown whole", T.Entry.text(T.Row.preview(T.Index.row(large, 2n))), String.repeat("😀", 129n))
    +huge : T.Index = T.Index.post(large, M.Msg{"huge", "a", M.Assistant{}, String.repeat("long line of message text ", 400n), 4n})
    +hst : M.State = M.State.post(M.State.post(st, M.Msg{"large", "a", M.Assistant{}, String.repeat("😀", 129n), 3n}), M.Msg{"huge", "a", M.Assistant{}, String.repeat("long line of message text ", 400n), 4n})
    C.Check.str("the window carries a 10 KB message whole", Bool.show(has.text(M.State.entries.all(V.State.window(hst, huge, target(), ["3"])), String.repeat("long line of message text ", 400n))), "True")
    +mob : J.Json = V.Render.view(False{}, hst, huge, obj("{}"), obj("{}"), 5n, request(), "epoch", ["3"])
    C.Check.str("the phones' screen carries the 10 KB message whole", Bool.show(String.contains(J.Json.show(mob), String.repeat("long line of message text ", 400n))), "True")
    C.Check.str("the phones' screen marks no message partial", Bool.show(String.contains(J.Json.show(mob), "byte-limit")), "False")
    +act : T.Index = T.Index.post(huge, M.Act{"act", "a", "tool", "Bash", String.repeat("o", 2000n), 5n})
    C.Check.str("a long activity stays folded", Nat.show(List.length(&2, J.Json, J.Json.elems(J.Json.get_obj(V.Page.window(act, "a", request(), "epoch", ["4"]), "partial")))), "1")
    C.Check.str("a folded activity shows its first block", Nat.show(String.length(T.Entry.text(T.Row.preview(T.Index.row(act, 4n))))), "512")
    +search : J.Json = V.Search.render(i, st, V.Obj.put(request(), "query", J.Str{"exact"}), "epoch", 0n)
    C.Check.str("indexed search returns normal explicit result rows", Nat.show(List.length(&2, J.Json, J.Json.elems(J.Json.get_obj(J.Json.get_obj(search, "search"), "rows")))), "1")
    C.Check.str("search produces no commands", Nat.show(List.length(&2, J.Json, J.Json.elems(J.Json.get_obj(search, "cmds")))), "0")
    C.Check.str("search cannot confer native readiness", yes(J.Json.get_bool(search, "remoteReady", True{})), "no")
    +chunk : J.Json = V.Message.result(T.Index.row(large, 2n), 1n, request(), "epoch")
    C.Check.str("continuation chunk retains exact final Unicode scalar", J.Json.get_str(J.Json.get_obj(chunk, "message"), "text", ""), "😀")
    C.Check.str("final exact chunk reports completion", yes(J.Json.get_bool(J.Json.get_obj(chunk, "message"), "complete", False{})), "yes")
    C.Check.str("message query cannot mark main display ready", yes(J.Json.get_bool(chunk, "remoteReady", True{})), "no")
    +web : J.Json = V.Render.view(True{}, st, i, obj("{}"), obj("{}"), 5n, request(), "epoch", ["1"])
    C.Check.str("actual production web projection returns keyed element codec", J.Json.get_str(J.Json.get_obj(web, "shown"), "type", ""), "element")
    C.Check.str("actual current web projection ready is separate from log", yes(J.Json.get_bool(web, "remoteReady", False{})), "yes")
