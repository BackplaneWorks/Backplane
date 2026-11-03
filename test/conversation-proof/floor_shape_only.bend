import Base
import ../../src/core/json.bend as J
import ../../src/core/thin.bend as T
import ../../src/core/thinq.bend as Q
import ../../src/core/client.bend as U
import ../../src/web/dom.bend as D
import ../../src/web/presentation.bend as W
import ../../src/server/thin-view.bend as V
import ../../src/server/conversation-frame.bend as F
import ./sparse_templates.bend as C

# The exact production floor's constant DOM, with its one text hole and the
# two bounded controls explicit. The shape equality below checks this model.
def tree(+u: U.Ui, +body: String, +older: Bool, +more: Bool) -> D.Node:
  D.El{"div", "workspace-root", [D.At{"class", "workspace-root"}], [D.El{"div", "app", [D.At{"class", "app picked"}], [
    D.El{"aside", "sidebar", [D.At{"class", "sidebar"}], [
      D.Dom.button("cmd", "ws-palette-open", "", "Jump or run a command", "Jump or run a command"),
      D.Dom.button("", "select", "", "Projects", "Projects")]},
    D.El{"main", "thread", [D.At{"class", "thread-view"}, D.At{"data-thread", "@compact.thread"}], [
      D.El{"div", "timeline", [D.At{"id", "timeline"}, D.At{"class", "timeline"}], List.append(&2, D.Node, V.Render.older(older, u), List.append(&2, D.Node,
        [D.El{"div", "@compact.entry", [D.At{"class", "msg assistant"}, D.At{"data-id", "@compact.entry"}],
          [D.El{"pre", "", [D.At{"class", "body"}, D.At{"style", "white-space:pre-wrap;overflow-wrap:anywhere"}], [D.Txt{body}]}]}], F.Frame.more(more)))},
      D.El{"div", "composer", [D.At{"class", "composer"}], [
        D.El{"textarea", "composer-input", [D.At{"id", "composer"}, D.At{"value", ""}, D.At{"placeholder", "Message"}, D.On{"input", "draft"}], Nil{}},
        D.Dom.button("send", "send", "", "Send", "Send")]}]}]}]}

def source_text(+index: T.Index, +id: String) -> String:
  "Message\n" ++ F.Frame.text(T.Entry.text(T.Row.entry(T.Index.row(index, Q.Q.num(id)))), 3072n)

def floor_shape(+u: U.Ui, +index: T.Index, +id: String, +window: J.Json) ->
  {F.Frame.floor(u, index, [id], window) == W.Node.write(tree(u, source_text(index, id), J.Json.get_bool(window, "hasMore", False{}), Bool.not(List.is_empty(&2, J.Json, J.Json.elems(J.Json.get_obj(window, "partial")))))) : J.Json}:
  {==}

def packet(+rid: J.Json, shown: J.Json) -> J.Json:
  F.Control.envelope(rid, J.Json.obj([J.KV{"shown", shown}]))

def template_shape(+u: U.Ui, +body: String, +rid: J.Json, older: Bool, more: Bool) ->
  {packet(rid, W.Node.write(tree(u, body, older, more))) ==
    Bool.pick(J.Json, older, Bool.pick(J.Json, more, C.model11(body, rid), C.model10(body, rid)), Bool.pick(J.Json, more, C.model01(body, rid), C.model00(body, rid))) : J.Json}:
  match older more:
    case False{} False{}: {==}
    case False{} True{}: {==}
    case True{} False{}: {==}
    case True{} True{}: {==}

def nth(xs: List<&2, J.Json>, n: Nat) -> J.Json:
  match xs n:
    case Nil{} _: J.Null{}
    case x <> _ 0n: x
    case _ <> rest 1n+k: nth(rest, k)

def child(node: J.Json, n: Nat) -> J.Json:
  nth(J.Json.elems(J.Json.get_obj(node, "kids")), n)

# The timeline is app/main/timeline, with a constant optional older marker.
def source_key(+shown: J.Json, older: Bool) -> String:
  J.Json.get_str(child(child(child(child(shown, 0n), 1n), 0n), Bool.pick(Nat, older, 1n, 0n)), "key", "")

def tree_source(+u: U.Ui, +body: String, older: Bool, more: Bool) ->
  {source_key(W.Node.write(tree(u, body, older, more)), older) == "@compact.entry" : String}:
  match older more:
    case False{} False{}: {==}
    case False{} True{}: {==}
    case True{} False{}: {==}
    case True{} True{}: {==}

def floor_source(+u: U.Ui, +index: T.Index, +id: String, +window: J.Json) ->
  {source_key(F.Frame.floor(u, index, [id], window), J.Json.get_bool(window, "hasMore", False{})) == "@compact.entry" : String}:
  +older = J.Json.get_bool(window, "hasMore", False{})
  +more = Bool.not(List.is_empty(&2, J.Json, J.Json.elems(J.Json.get_obj(window, "partial"))))
  +shown = F.Frame.floor(u, index, [id], window)
  +modeled = W.Node.write(tree(u, source_text(index, id), older, more))
  Equal.trans(String, source_key(shown, older), source_key(modeled, older), "@compact.entry",
    Equal.cong(J.Json, String, j => source_key(j, older), shown, modeled, floor_shape(u, index, id, window)), tree_source(u, source_text(index, id), older, more))

def source_visible_text(+index: T.Index, +id: String) ->
  {String.is_empty(source_text(index, id)) == False{} : Bool}:
  {==}
