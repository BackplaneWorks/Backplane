import Base
import ./check.bend as C
import ./thin_chat_receipts_test.bend as F
import ../src/core/model.bend as M
import ../src/core/client.bend as U
import ../src/core/thin-chat.bend as TC
import ../src/core/thin.bend as T
import ../src/core/storedb.bend as S
import ../src/core/json.bend as J
import ../src/core/i18n.bend as I
import ../src/core/thin-wire.bend as W
import ../src/web/view.bend as V
import ../src/web/dom.bend as D
import ../src/web/presentation.bend as P
import ../src/server/thin-view.bend as TV
import ../src/server/thinstore.bend as TS
import ../src/server/db.bend as DB

# Authored, unrun. The four empty Msg times/roles match the bounded persisted
# screenshot metadata; IDs and surrounding bodies are synthetic. Expand real
# message memos: the previous receipt-only case missed these stamp wrappers.
def state() -> M.State:
  M.State.replay(F.metadata(), M.State.empty())

def before() -> M.Entry:
  M.Msg{"real-before", "bub-chat", M.Assistant{}, "Real preceding assistant message.", 1792772545n}

def after() -> M.Entry:
  M.Msg{"real-after", "bub-chat", M.Assistant{}, "Real following assistant message.", 1792773210n}

def screenshot.empty() -> List<&2, M.Entry>:
  [M.Msg{"empty-1222", "bub-chat", M.Assistant{}, "", 1792772568n},
   M.Msg{"empty-1223", "bub-chat", M.Assistant{}, "", 1792772585n},
   M.Msg{"empty-1227", "bub-chat", M.Assistant{}, "", 1792772876n},
   M.Msg{"empty-1232", "bub-chat", M.Assistant{}, "", 1792773154n}]

def info() -> Map<&2, String>:
  Map.set(&2, String, Map.new(&2, String), "clock", "1792773600")

def dom(nodes: List<&2, D.Node>) -> String:
  J.Json.show(P.Node.write(D.Dom.div("timeline", V.View.expand(nodes))))

def shown(es: List<&2, M.Entry>) -> List<&2, D.Node>:
  V.View.entries(I.En{}, es, state(), "bub-chat", info(), False{})

def paged(es: List<&2, M.Entry>) -> List<&2, D.Node>:
  V.View.paged(I.En{}, Map.new(&2, String), List.reverse(&2, M.Entry, es), state(), "bub-chat", info(), False{}, U.Ec.none())

def count(nodes: List<&2, D.Node>, +cls: String) -> Nat:
  match nodes:
    case Nil{}: 0n
    case D.Txt{_} <> t: count(t, cls)
    case D.El{_, _, attrs, kids} <> t:
      Nat.add(Bool.pick(Nat, String.eq(V.View.attr(attrs, "class"), cls), 1n, 0n), Nat.add(count(kids, cls), count(t, cls)))

def timeline(nodes: List<&2, D.Node>) -> List<&2, D.Node>:
  match nodes:
    case Nil{}: Nil{}
    case D.Txt{_} <> t: timeline(t)
    case D.El{tag, key, +attrs, +kids} <> t:
      List.append(&2, D.Node, Bool.pick(List<&2, D.Node>, String.eq(V.View.attr(attrs, "id"), "timeline"), [D.El{tag, key, attrs, kids}], timeline(kids)), timeline(t))

def thin.dom(+st: M.State, +pg: TS.Page) -> String:
  +rendered = TV.Render.view(True{}, st, TS.Page.index(pg), J.Json.obj(Nil{}), J.Json.obj(Nil{}), 1792773600n, F.request(), "ep", TS.Page.ids(pg))
  dom(timeline([P.Node.read(J.Json.get_obj(rendered, "shown"))]))

def empty.posts(+n: Nat, acc: List<&2, M.Change>) -> List<&2, M.Change>:
  match n:
    case 0n: acc
    case 1n+k:
      empty.posts(k, M.MessagePosted{"bub-chat", "empty:" ++ Nat.show(1n+k), M.Assistant{},
        Bool.pick(String, Nat.is_eq(1n+k, 96n), String.repeat(" ", 600n), " \t\r\n\u{b}\u{c}"), 1792773500n} <> acc)

def main() -> IO(Unit):
  do IO<Unit>:
    +baseline : List<&2, M.Entry> = [before(), after()]
    +actual : List<&2, M.Entry> = before() <> List.append(&2, M.Entry, screenshot.empty(), [after()])
    C.Check.str("screenshot wrappers/stamps entirely absent after full expanded render", dom(shown(actual)), dom(shown(baseline)))
    C.Check.str("screenshot wrappers/stamps absent in loaded60-row pager", dom(paged(actual)), dom(paged(baseline)))
    C.Check.str("both real assistant wrappers remain", Nat.show(count(V.View.expand(shown(actual)), "msg assistant")), "2")
    C.Check.str("both attached real timestamps remain", Nat.show(count(V.View.expand(shown(actual)), "stamp")), "2")
    C.Check.str("empty-only group emits ZERO nodes, not a hidden div", Nat.show(List.length(&2, D.Node, shown(screenshot.empty()))), "0")
    C.Check.str("new empty arrival leaves actual DOM identical", dom(shown(List.append(&2, M.Entry, baseline, screenshot.empty()))), dom(shown(baseline)))
    C.Check.str("empty dated reply leaves no date/time container", dom(shown(List.append(&2, M.Entry, baseline, [M.Msg{"empty-yesterday", "bub-chat", M.Assistant{}, "", 1792686100n}]))), dom(shown(baseline)))
    C.Check.str("whitespace-only reply hidden", F.yes(TC.Chat.hidden(M.Msg{"plain", "bub-chat", M.Assistant{}, " \t\r\n\u{b}\u{c}", 1n})), "yes")
    C.Check.str("blank USER message remains", F.yes(TC.Chat.hidden(M.Msg{"user", "bub-chat", M.User{}, "", 1n})), "no")
    C.Check.str("nonempty Markdown remains", F.yes(TC.Chat.hidden(M.Msg{"code", "bub-chat", M.Assistant{}, "```\n\n```", 1n})), "no")
    C.Check.str("attachment-only message remains", F.yes(TC.Chat.hidden(M.Msg{"image", "bub-chat", M.Assistant{}, "[attachment: /tmp/fixture.png]", 1n})), "no")
    C.Check.str("NBSP is content, matching Markdown whitespace policy", F.yes(TC.Chat.hidden(M.Msg{"nbsp", "bub-chat", M.Assistant{}, "\u{a0}", 1n})), "no")
    C.Check.str("actual empty-text link remains", F.yes(TC.Chat.hidden(M.Msg{M.Link.id("other", "1"), "bub-chat", M.Assistant{}, "", 1n})), "no")
    C.Check.str("warn ride remains", F.yes(TC.Chat.hidden(M.Act{"ride:warn", "bub-chat", "warn", "ride", "warning", 1n})), "no")
    C.Check.str("session notice remains", F.yes(TC.Chat.hidden(M.Act{"session", "bub-chat", "info", "session", "OpenCode reset", 1n})), "no")
    # since e87cafd19 (Fold.shown) a blank assistant message shows nothing in
    # any thread; an ordinary reply with text still draws its one node
    C.Check.str("ordinary conversation hides a blank reply too", Nat.show(List.length(&2, D.Node, V.View.entries(I.En{}, [M.Msg{"ordinary-empty", "ordinary", M.Assistant{}, "", 1n}], state(), "ordinary", info(), False{}))), "0")
    C.Check.str("ordinary conversation keeps prior presentation", Nat.show(List.length(&2, D.Node, V.View.entries(I.En{}, [M.Msg{"ordinary-text", "ordinary", M.Assistant{}, "hello", 1n}], state(), "ordinary", info(), False{}))), "1")
    +changes : List<&2, M.Change> = List.append(&2, M.Change, F.changes(), empty.posts(96n, Nil{}))
    +st : M.State = M.State.replay(changes, M.State.empty())
    +i : T.Index = T.Index.of(st)
    C.Check.str("96empty plus96ride cannot starve visible32", String.join(TS.Page.ids(F.page(i, 0n)), ","), String.join(F.seq.down(49n, 18n), ","))
    C.Check.str("next page remains source ordinals17..1", String.join(TS.Page.ids(F.page(i, 18n)), ","), String.join(F.seq.down(17n, 1n), ","))
    C.Check.str("full loaded pager has no empty-wrapper slots or earlier sentinel", dom(paged(List.reverse(&2, M.Entry, M.State.mine(st, "bub-chat")))), dom(paged(List.reverse(&2, M.Entry, M.State.mine(M.State.replay(F.changes(), M.State.empty()), "bub-chat")))))
    +base : M.State = M.State.replay(F.changes(), M.State.empty())
    C.Check.str("actual thin production timeline has no wrappers from96empty rows", thin.dom(st, F.page(i, 0n)), thin.dom(base, F.page(T.Index.of(base), 0n)))
    C.Check.str("source history remains241 entries", Nat.show(List.length(&2, M.Entry, M.State.entries.all(st))), "241")
    +request : J.Json = J.Json.obj([J.KV{"target", TV.Target.json("bot", "b1790292962-8017")}, J.KV{"entryId", J.Str{"empty:96"}}])
    +cursor : J.Json = TV.Obj.put(request, "cursor", J.Str{W.Cursor.write("ep", J.Json.get_obj(request, "target"), "message:241", 1n)})
    C.Check.str("direct source memory read still accesses blank assistant", F.read.error(TV.Message.read(i, "bub-chat", request, "ep", T.Index.page(i, "bub-chat", 0n, 32n), st)), "")
    C.Check.str("direct source memory cursor still accesses blank assistant", F.read.error(TV.Message.read(i, "bub-chat", cursor, "ep", Nil{}, st)), "")
    made : Result<&1, &1, U32 & String, U32 & (String & String)> <- Process.run("mktemp", ["-d"], "", 4096, 10000)
    +dir : String <- F.dir.of(made)
    +opened : String <- DB.Db.open(dir ++ "/store.db")
    +h : String = String.drop(opened, 3n)
    schema : String <- DB.Db.exec(h, S.Sdb.schema())
    C.Check.str("temp schema", schema, "ok")
    written : String <- DB.Db.script(h, F.items(Map.to_list(&2, String, S.Mat.facts(S.Mat.replay(F.lines(changes), S.Mat.empty())))))
    C.Check.str("synthetic store written", String.take(written, 2n), "ok")
    F.parity([0n, 18n, 100n, 200n, 1n, 999n], h, i)
    +visible : TS.Page <- TS.Ts.page.chat(True{}, h, "bub-chat", 0n, 32n, T.Index.recent_only(i))
    C.Check.str("stored and memory actual expanded thin timelines match", thin.dom(st, visible), thin.dom(st, F.page(i, 0n)))
    +raw : TS.Page <- TS.Ts.page.chat(False{}, h, "bub-chat", 0n, 32n, T.Index.recent_only(i))
    C.Check.str("direct SQL source page retains blank rows", String.join(TS.Page.ids(raw), ","), String.join(F.seq.down(241n, 210n), ","))
    +whole : TS.Page <- TS.Ts.whole(h, "bub-chat", 241n, "empty:96", raw)
    C.Check.str("direct SQL source read still accesses blank assistant", F.read.error(TV.Message.read(TS.Page.index(whole), "bub-chat", request, "ep", TS.Page.ids(whole), st)), "")
    C.Check.str("direct SQL source cursor still accesses blank assistant", F.read.error(TV.Message.read(TS.Page.index(whole), "bub-chat", cursor, "ep", TS.Page.ids(whole), st)), "")
    closed : String <- DB.Db.close(h)
    removed : Result<&1, &1, U32 & String, U32 & (String & String)> <- Process.run("rm", ["-rf", dir], "", 4096, 10000)
    return Unit{}
