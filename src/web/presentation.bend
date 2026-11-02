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

# A local send is display-only until the authenticated action replies. It
# never enters retained history, a cached picture, or a server session.
def Node.pending.kids(timeline: Bool, kids: List<&2, D.Node>, rows: List<&2, D.Node>) -> List<&2, D.Node>:
  match timeline:
    case True{}: List.append(&2, D.Node, kids, rows)
    case False{}: kids

def Node.timeline(attrs: List<&2, D.Attr>) -> Bool:
  match attrs:
    case Nil{}: False{}
    case D.At{"id", "timeline"} <> _: True{}
    case _ <> tail: Node.timeline(tail)

def Node.pending.go(nodes: List<&2, D.Node>, +rows: List<&2, D.Node>) -> List<&2, D.Node>:
  match nodes:
    case Nil{}: Nil{}
    case D.Txt{t} <> tail: D.Txt{t} <> Node.pending.go(tail, rows)
    case D.El{tag, key, +attrs, kids} <> tail:
      D.El{tag, key, attrs, Node.pending.kids(Node.timeline(attrs), Node.pending.go(kids, rows), rows)} <> Node.pending.go(tail, rows)

def Node.pending(node: D.Node, rows: List<&2, D.Node>) -> D.Node:
  Node.read.first(Node.pending.go([node], rows))

# An optimistic menu is a picture of server-supplied bounded choices only.
# This never changes a session, sends a command or selects a model.
def Node.models.attrs(attrs: List<&2, D.Attr>, +open: Bool) -> List<&2, D.Attr>:
  match attrs:
    case Nil{}: Nil{}
    case D.At{"style", _} <> tail: D.At{"style", Bool.pick(String, open, "display:contents", "display:none")} <> Node.models.attrs(tail, open)
    case D.At{"aria-hidden", _} <> tail: D.At{"aria-hidden", Bool.pick(String, open, "false", "true")} <> Node.models.attrs(tail, open)
    case head <> tail: head <> Node.models.attrs(tail, open)

# The Subagents rows shown or shut at the click (subs-feedback.js), ahead of
# the hub's view: "1" open, "0" shut, "" as the hub drew them. The hub
# always sends the rows (view.bend's View.subs.dock), hidden while shut, so
# the head's state and the section's " shut" are all that change here.
def Node.subs.word(+open: Bool) -> String:
  Bool.pick(String, open, "true", "false")

def Node.subs.act(+open: Bool) -> String:
  Bool.pick(String, open, "subs-shut", "subs-open")

def Node.subs.head(attrs: List<&2, D.Attr>, +open: Bool) -> List<&2, D.Attr>:
  match attrs:
    case Nil{}: Nil{}
    case D.At{"aria-expanded", _} <> tail: D.At{"aria-expanded", Node.subs.word(open)} <> Node.subs.head(tail, open)
    case D.On{"click", _} <> tail: D.On{"click", Node.subs.act(open)} <> Node.subs.head(tail, open)
    case head <> tail: head <> Node.subs.head(tail, open)

def Node.subs.bare(+c: String) -> String:
  Bool.pick(String, String.ends_with(c, " shut"), String.take(c, Nat.sub(String.length(c), 5n)), c)

def Node.subs.section(attrs: List<&2, D.Attr>, +open: Bool) -> List<&2, D.Attr>:
  match attrs:
    case Nil{}: Nil{}
    case D.At{"class", c} <> tail: D.At{"class", Node.subs.bare(c) ++ Bool.pick(String, open, "", " shut")} <> Node.subs.section(tail, open)
    case head <> tail: head <> Node.subs.section(tail, open)

def Node.subs.attrs(none: Bool, head: Bool, attrs: List<&2, D.Attr>, open: Bool) -> List<&2, D.Attr>:
  match none head:
    case True{} _: attrs
    case False{} True{}: Node.subs.head(attrs, open)
    case False{} False{}: Node.subs.section(attrs, open)

# A subagent's row opened or shut at the click (subs-feedback.js's rows:
# a line "1<key>" or "0<key>" each), ahead of the hub's view: the row's
# " open" and its button's aria-expanded and next fold value (fold.bend's
# Fold.ask: its first character). Its steps are drawn either way
# (view.bend's View.sub.detail).
def Node.rows.go(ls: List<&2, String>, +key: String) -> String:
  match ls:
    case Nil{}: ""
    case +l <> t: Bool.pick(String, String.eq(String.drop(l, 1n), key), String.take(l, 1n), Node.rows.go(t, key))

def Node.rows.get(none: Bool, +rows: String, +key: String) -> String:
  match none:
    case True{}: ""
    case False{}: Node.rows.go(String.split(rows, '\n'), key)

def Node.rows.bare(+c: String) -> String:
  Bool.pick(String, String.ends_with(c, " open"), String.take(c, Nat.sub(String.length(c), 5n)), c)

def Node.rows.attrs(attrs: List<&2, D.Attr>, +open: Bool) -> List<&2, D.Attr>:
  match attrs:
    case Nil{}: Nil{}
    case D.At{"class", c} <> tail: D.At{"class", Node.rows.bare(c) ++ Bool.pick(String, open, " open", "")} <> Node.rows.attrs(tail, open)
    case head <> tail: head <> Node.rows.attrs(tail, open)

def Node.rows.button(attrs: List<&2, D.Attr>, +open: Bool) -> List<&2, D.Attr>:
  match attrs:
    case Nil{}: Nil{}
    case D.At{"aria-expanded", _} <> tail: D.At{"aria-expanded", Node.subs.word(open)} <> Node.rows.button(tail, open)
    case D.At{"data-value", v} <> tail: D.At{"data-value", Bool.pick(String, open, "0", "1") ++ String.drop(v, 1n)} <> Node.rows.button(tail, open)
    case head <> tail: head <> Node.rows.button(tail, open)

# the row's own button is its first child
def Node.rows.kids(kids: List<&2, D.Node>, +open: Bool) -> List<&2, D.Node>:
  match kids:
    case D.El{"button", k, as, ks} <> tail: D.El{"button", k, Node.rows.button(as, open), ks} <> tail
    case _: kids

# only a Subagents row (class "sub ..."): a timeline row may carry the
# same id (the Agent call that started it)
def Node.rows.sub(attrs: List<&2, D.Attr>) -> Bool:
  match attrs:
    case Nil{}: False{}
    case D.At{"class", c} <> _: String.starts_with(c, "sub ")
    case _ <> tail: Node.rows.sub(tail)

def Node.rows.el.k(hit: Bool, +open: Bool, +tag: String, +key: String, attrs: List<&2, D.Attr>, kids: List<&2, D.Node>) -> D.Node:
  match hit:
    case False{}: D.El{tag, key, attrs, kids}
    case True{}: D.El{tag, key, Node.rows.attrs(attrs, open), Node.rows.kids(kids, open)}

def Node.rows.el.h(none: Bool, +hit: String, +tag: String, +key: String, +attrs: List<&2, D.Attr>, kids: List<&2, D.Node>) -> D.Node:
  match none:
    case True{}: D.El{tag, key, attrs, kids}
    case False{}: Node.rows.el.k(Node.rows.sub(attrs), String.eq(hit, "1"), tag, key, attrs, kids)

def Node.rows.el(+hit: String, +tag: String, +key: String, +attrs: List<&2, D.Attr>, kids: List<&2, D.Node>) -> D.Node:
  Node.rows.el.h(String.is_empty(hit), hit, tag, key, attrs, kids)

def Node.overlay.attrs(key: String, +attrs: List<&2, D.Attr>, text: String, open: Bool, +subs: String) -> List<&2, D.Attr>:
  match key:
    case "model-layer": Node.models.attrs(attrs, open)
    case "subs-head": Node.subs.attrs(String.is_empty(subs), True{}, attrs, String.eq(subs, "1"))
    case "subs": Node.subs.attrs(String.is_empty(subs), False{}, attrs, String.eq(subs, "1"))
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

def Node.overlay.go(nodes: List<&2, D.Node>, +text: String, +open: Bool, +subs: String, +rows: String, acc: List<&2, D.Node>) -> List<&2, D.Node>:
  match nodes:
    case Nil{}: acc
    case D.Txt{value} <> tail: Node.overlay.go(tail, text, open, subs, rows, D.Txt{value} <> acc)
    case D.El{+tag, +key, attrs, kids} <> tail:
      Node.overlay.go(tail, text, open, subs, rows, Node.rows.el(Node.rows.get(String.is_empty(rows), rows, key), tag, key, Node.overlay.attrs(key, attrs, text, open, subs),
        Node.overlay.kids(key, open, List.reverse(&2, D.Node, Node.overlay.go(kids, text, open, subs, rows, Nil{})))) <> acc)

def Node.overlay.rows(node: D.Node, text: String, open: Bool, subs: String, rows: String) -> D.Node:
  Node.read.first(Node.overlay.go([node], text, open, subs, rows, Nil{}))

def Node.overlay.subs(node: D.Node, text: String, open: Bool, subs: String) -> D.Node:
  Node.overlay.rows(node, text, open, subs, "")

def Node.overlay(node: D.Node, text: String, open: Bool) -> D.Node:
  Node.overlay.subs(node, text, open, "")
