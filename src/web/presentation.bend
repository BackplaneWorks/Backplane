import Base
import ../core/json.bend as J
import ./dom.bend as D

# Renderer transport only: no client state, replay cursor, commands or grants.
# The authenticated service supplies a tree produced by the normal Bend view.
# Text stays text throughout; the browser never parses supplied HTML.
def Node.attrs.write(xs: List<&2, D.Attr>) -> List<&2, J.Json>:
  match xs:
    case Nil{}: Nil{}
    case D.At{name, value} <> tail:
      J.Json.obj([J.KV{"name", J.Str{name}}, J.KV{"value", J.Str{value}}]) <> Node.attrs.write(tail)
    case D.On{event, action} <> tail:
      J.Json.obj([J.KV{"event", J.Str{event}}, J.KV{"action", J.Str{action}}]) <> Node.attrs.write(tail)

def Node.write.go(xs: List<&2, D.Node>, acc: List<&2, J.Json>) -> List<&2, J.Json>:
  match xs:
    case Nil{}: acc
    case D.Txt{text} <> tail:
      Node.write.go(tail, J.Json.obj([J.KV{"type", J.Str{"text"}}, J.KV{"text", J.Str{text}}]) <> acc)
    case D.El{tag, key, attrs, kids} <> tail:
      Node.write.go(tail, J.Json.obj([J.KV{"type", J.Str{"element"}}, J.KV{"tag", J.Str{tag}}, J.KV{"key", J.Str{key}},
        J.KV{"attrs", J.Json.arr(Node.attrs.write(attrs))},
        J.KV{"kids", J.Json.arr(List.reverse(&2, J.Json, Node.write.go(kids, Nil{})))}]) <> acc)

def Node.write.first(xs: List<&2, J.Json>) -> J.Json:
  match xs:
    case head <> _: head
    case Nil{}: J.Null{}

def Node.write(node: D.Node) -> J.Json:
  Node.write.first(Node.write.go([node], Nil{}))

def Node.attrs.one(event: String, +head: J.Json) -> D.Attr:
  match event:
    case "": D.At{J.Json.get_str(head, "name", ""), J.Json.get_str(head, "value", "")}
    case name: D.On{name, J.Json.get_str(head, "action", "")}

def Node.attrs.read(xs: J.Json, acc: List<&2, D.Attr>) -> List<&2, D.Attr>:
  match xs:
    case J.Arr{items}: Node.attrs.read(items, acc)
    case J.Item{+head, tail}:
      Node.attrs.read(tail, Node.attrs.one(J.Json.get_str(head, "event", ""), head) <> acc)
    case _: acc

def Node.read.one(kind: String, +head: J.Json, kids: List<&2, D.Node>) -> D.Node:
  match kind:
    case "text": D.Txt{J.Json.get_str(head, "text", "")}
    case "element": D.El{J.Json.get_str(head, "tag", "div"), J.Json.get_str(head, "key", ""),
      List.reverse(&2, D.Attr, Node.attrs.read(J.Json.get_obj(head, "attrs"), Nil{})), kids}
    case _: D.Txt{""}

# Bound nesting from a malformed presentation without touching canonical Ui.
def Node.read.go(+depth: Nat, xs: J.Json, acc: List<&2, D.Node>) -> List<&2, D.Node>:
  match depth xs:
    case 0n _: acc
    case 1n+k J.Arr{items}: Node.read.go(1n+k, items, acc)
    case 1n+k J.Item{+head, tail}:
      Node.read.go(1n+k, tail, Node.read.one(J.Json.get_str(head, "type", ""), head,
        List.reverse(&2, D.Node, Node.read.go(k, J.Json.get_obj(head, "kids"), Nil{}))) <> acc)
    case _ _: acc

def Node.read.first(xs: List<&2, D.Node>) -> D.Node:
  match xs:
    case head <> _: head
    case Nil{}: D.Txt{""}

def Node.read(json: J.Json) -> D.Node:
  Node.read.first(Node.read.go(64n, J.Json.arr([json]), Nil{}))

# A newer local draft may overtake a travelling view. Keep its normal composer
# field current without changing the server-rendered conversation or actions.
def Node.composer(attrs: List<&2, D.Attr>) -> Bool:
  match attrs:
    case Nil{}: False{}
    case D.At{"id", "composer"} <> _: True{}
    case _ <> tail: Node.composer(tail)

def Node.draft.values(attrs: List<&2, D.Attr>, +text: String) -> List<&2, D.Attr>:
  match attrs:
    case Nil{}: Nil{}
    case D.At{"value", _} <> tail: D.At{"value", text} <> Node.draft.values(tail, text)
    case head <> tail: head <> Node.draft.values(tail, text)

def Node.draft.attrs(composer: Bool, attrs: List<&2, D.Attr>, text: String) -> List<&2, D.Attr>:
  match composer:
    case True{}: Node.draft.values(attrs, text)
    case False{}: attrs

def Node.draft.go(nodes: List<&2, D.Node>, +text: String, acc: List<&2, D.Node>) -> List<&2, D.Node>:
  match nodes:
    case Nil{}: acc
    case D.Txt{value} <> tail: Node.draft.go(tail, text, D.Txt{value} <> acc)
    case D.El{tag, key, +attrs, kids} <> tail:
      Node.draft.go(tail, text, D.El{tag, key, Node.draft.attrs(Node.composer(attrs), attrs, text),
        List.reverse(&2, D.Node, Node.draft.go(kids, text, Nil{}))} <> acc)

def Node.draft(node: D.Node, text: String) -> D.Node:
  Node.read.first(Node.draft.go([node], text, Nil{}))

# An optimistic menu is a picture of server-supplied bounded choices only.
# This never changes a session, sends a command or selects a model.
def Node.models.attrs(attrs: List<&2, D.Attr>, +open: Bool) -> List<&2, D.Attr>:
  match attrs:
    case Nil{}: Nil{}
    case D.At{"style", _} <> tail: D.At{"style", Bool.pick(String, open, "display:contents", "display:none")} <> Node.models.attrs(tail, open)
    case D.At{"aria-hidden", _} <> tail: D.At{"aria-hidden", Bool.pick(String, open, "false", "true")} <> Node.models.attrs(tail, open)
    case head <> tail: head <> Node.models.attrs(tail, open)

def Node.overlay.attrs(key: String, +attrs: List<&2, D.Attr>, text: String, open: Bool) -> List<&2, D.Attr>:
  match key:
    case "model-layer": Node.models.attrs(attrs, open)
    case _: Node.draft.attrs(Node.composer(attrs), attrs, text)

# The model picker shown here before the hub draws it open has the shade
# behind it too (a click off the picker shuts it), and one shut here has
# none (a closed picker has no modal backdrop; view.bend's View.mp.loc).
def Node.models.shade() -> D.Node:
  D.El{"div", "", [D.At{"class", "shade"}, D.At{"data-value", "models"}, D.On{"click", "flag"}], Nil{}}

def Node.models.shaded(attrs: List<&2, D.Attr>) -> Bool:
  match attrs:
    case Nil{}: False{}
    case D.At{"class", "shade"} <> _: True{}
    case _ <> tail: Node.models.shaded(tail)

def Node.models.kids.k(open: Bool, shaded: Bool, head: D.Node, tail: List<&2, D.Node>) -> List<&2, D.Node>:
  match open shaded:
    case True{} False{}: Node.models.shade() <> head <> tail
    case False{} True{}: tail
    case _ _: head <> tail

def Node.models.kids(+open: Bool, kids: List<&2, D.Node>) -> List<&2, D.Node>:
  match kids:
    case Nil{}: Nil{}
    case D.El{tag, key, +attrs, ks} <> tail: Node.models.kids.k(open, Node.models.shaded(attrs), D.El{tag, key, attrs, ks}, tail)
    case head <> tail: head <> tail

def Node.overlay.kids(key: String, +open: Bool, kids: List<&2, D.Node>) -> List<&2, D.Node>:
  match key:
    case "model-layer": Node.models.kids(open, kids)
    case _: kids

def Node.overlay.go(nodes: List<&2, D.Node>, +text: String, +open: Bool, acc: List<&2, D.Node>) -> List<&2, D.Node>:
  match nodes:
    case Nil{}: acc
    case D.Txt{value} <> tail: Node.overlay.go(tail, text, open, D.Txt{value} <> acc)
    case D.El{tag, +key, attrs, kids} <> tail:
      Node.overlay.go(tail, text, open, D.El{tag, key, Node.overlay.attrs(key, attrs, text, open),
        Node.overlay.kids(key, open, List.reverse(&2, D.Node, Node.overlay.go(kids, text, open, Nil{})))} <> acc)

def Node.overlay(node: D.Node, text: String, open: Bool) -> D.Node:
  Node.read.first(Node.overlay.go([node], text, open, Nil{}))
