import Base
import ../core/i18n.bend as I
import ../core/msgs.bend as MSG
import ./dom.bend as D
import ../core/space.bend as SP

# A bot's space as DOM (classes sp-*, styled by space.css). Every decision
# about the spec (what is kept, caps, percentages) is made in core/space.bend.
#
# A button sends action "space" with value "<bot>\x1f<action>". An input
# sends action "space-input" on Enter or its Send button; with host.js's
# data-space support its value is "<bot>\x1f<action>\x1f<text>" and the
# field is cleared.

# the unit separator between the parts of a value
def WSpace.sep() -> String:
  SCon{Chr{31}, SNil{}}

def WSpace.key(+bot: String, +action: String) -> String:
  bot ++ WSpace.sep() ++ action

def WSpace.el(+tag: String, +cls: String, kids: List<&2, D.Node>) -> D.Node:
  D.El{tag, "", [D.At{"class", cls}], kids}

def WSpace.txt(+tag: String, +cls: String, +s: String) -> D.Node:
  D.El{tag, "", [D.At{"class", cls}], [D.Txt{s}]}

def WSpace.strs(+tag: String, xs: List<&2, String>) -> List<&2, D.Node>:
  match xs:
    case Nil{}:
      Nil{}
    case x <> t:
      D.El{tag, "", Nil{}, [D.Txt{x}]} <> WSpace.strs(tag, t)

def WSpace.cells(cs: List<&2, SP.Cell>) -> List<&2, D.Node>:
  match cs:
    case Nil{}:
      Nil{}
    case SP.Cell{k, v} <> t:
      WSpace.txt("div", "sp-k", k) <> WSpace.txt("div", "sp-v", v) <> WSpace.cells(t)

def WSpace.trows(rs: List<&2, List<&2, String>>) -> List<&2, D.Node>:
  match rs:
    case Nil{}:
      Nil{}
    case r <> t:
      D.El{"tr", "", Nil{}, WSpace.strs("td", r)} <> WSpace.trows(t)

def WSpace.head(h: List<&2, String>) -> List<&2, D.Node>:
  match h:
    case Nil{}:
      Nil{}
    case x <> t:
      [D.El{"thead", "", Nil{}, [D.El{"tr", "", Nil{}, WSpace.strs("th", x <> t)}]}]

def WSpace.hint(+h: String) -> List<&2, D.Node>:
  match h:
    case SNil{}:
      Nil{}
    case SCon{_, _}:
      [WSpace.txt("span", "sp-stat-hint", h)]

def WSpace.label(+l: String) -> List<&2, D.Node>:
  match l:
    case SNil{}:
      Nil{}
    case SCon{_, _}:
      [WSpace.txt("label", "sp-input-label", l)]

def WSpace.tone(+t: String) -> String:
  match t:
    case SNil{}:
      "sp-badge"
    case SCon{_, _}:
      "sp-badge " ++ t

# an input line: Enter in the field or the Send button sends it
def WSpace.input.loc(+lc: I.Loc, +id: String, +k: String, +l: String, +a: String, +p: String) -> D.Node:
  D.El{"div", "sp-in:" ++ a, [D.At{"class", "sp-input"}], D.Dom.cat(WSpace.label(l), [
    D.El{"div", "", [D.At{"class", "sp-input-line"}], [
      D.El{"input", "sp-in:" ++ a, [D.At{"id", id}, D.At{"class", "sp-field"}, D.At{"placeholder", p}, D.At{"autocomplete", "off"},
        D.At{"data-enter", "space-input"}, D.At{"data-space", k}, D.At{"data-field", id}], Nil{}},
      D.El{"button", "", [D.At{"class", "sp-send"}, D.At{"title", a}, D.At{"data-space", k}, D.At{"data-field", id}, D.On{"click", "space-input"}], [D.Txt{MSG.T.composer.send(lc)}]}]}])}

# one block; `id` names its input field, if it has one

def WSpace.input(+id: String, +k: String, +l: String, +a: String, +p: String) -> D.Node:
  WSpace.input.loc(I.En{}, id, k, l, a, p)

def WSpace.block.loc(+lc: I.Loc, b: SP.Block, +bot: String, +id: String) -> D.Node:
  match b:
    case SP.BHeading{s}:
      WSpace.txt("h3", "sp-heading", s)
    case SP.BText{s}:
      WSpace.el("div", "sp-text body", D.Md.render.loc(lc, s))
    case SP.BStat{l, v, +h}:
      WSpace.el("div", "sp-stat", D.Dom.cat([WSpace.txt("span", "sp-stat-label", l), WSpace.txt("span", "sp-stat-value", v)], WSpace.hint(h)))
    case SP.BProgress{l, +p}:
      WSpace.el("div", "sp-progress", [
        WSpace.el("div", "sp-progress-head", [WSpace.txt("span", "sp-progress-label", l), WSpace.txt("span", "sp-progress-pct", SP.Space.pct(p))]),
        WSpace.el("div", "sp-bar", [D.El{"div", "", [D.At{"class", "sp-fill"}, D.At{"style", "width:" ++ SP.Space.pct(p)}], Nil{}}])])
    case SP.BList{xs}:
      WSpace.el("ul", "sp-list", WSpace.strs("li", xs))
    case SP.BKv{rs}:
      WSpace.el("div", "sp-kv", WSpace.cells(rs))
    case SP.BTable{h, rs}:
      WSpace.el("div", "sp-table-wrap", [WSpace.el("table", "sp-table", D.Dom.cat(WSpace.head(h), [D.El{"tbody", "", Nil{}, WSpace.trows(rs)}]))])
    case SP.BButton{l, +a}:
      D.Dom.button("sp-button", "space", WSpace.key(bot, a), a, l)
    case SP.BInput{+l, +a, +p}:
      WSpace.input.loc(lc, id, WSpace.key(bot, a), l, a, p)
    case SP.BDivider{}:
      D.El{"hr", "", [D.At{"class", "sp-divider"}], Nil{}}
    case SP.BBadge{s, +t}:
      WSpace.txt("span", WSpace.tone(t), s)
    case SP.BCode{s}:
      D.El{"pre", "", [D.At{"class", "sp-code"}], [D.El{"code", "", Nil{}, [D.Txt{s}]}]}
    case SP.BRow{_}:
      WSpace.el("div", "sp-row", Nil{})

def WSpace.block(b: SP.Block, +bot: String, +id: String) -> D.Node:
  WSpace.block.loc(I.En{}, b, bot, id)

def WSpace.kids.loc(+lc: I.Loc, bs: List<&2, SP.Block>, +bot: String, +pre: String, +i: Nat) -> List<&2, D.Node>:
  match bs:
    case Nil{}:
      Nil{}
    case b <> t:
      WSpace.block.loc(lc, b, bot, pre ++ Nat.show(i)) <> WSpace.kids.loc(lc, t, bot, pre, 1n+i)

# a row: its blocks side by side (never a row: Space.parse keeps one level)

def WSpace.kids(bs: List<&2, SP.Block>, +bot: String, +pre: String, +i: Nat) -> List<&2, D.Node>:
  WSpace.kids.loc(I.En{}, bs, bot, pre, i)

def WSpace.top.loc(+lc: I.Loc, b: SP.Block, +bot: String, +id: String) -> D.Node:
  match b:
    case SP.BRow{kids}:
      WSpace.el("div", "sp-row", WSpace.kids.loc(lc, kids, bot, id ++ "-", 0n))
    case _:
      WSpace.block.loc(lc, b, bot, id)

def WSpace.top(b: SP.Block, +bot: String, +id: String) -> D.Node:
  WSpace.top.loc(I.En{}, b, bot, id)

def WSpace.blocks.loc(+lc: I.Loc, bs: List<&2, SP.Block>, +bot: String, +i: Nat) -> List<&2, D.Node>:
  match bs:
    case Nil{}:
      Nil{}
    case b <> t:
      WSpace.top.loc(lc, b, bot, "sp-in-" ++ Nat.show(i)) <> WSpace.blocks.loc(lc, t, bot, 1n+i)

def WSpace.blocks(bs: List<&2, SP.Block>, +bot: String, +i: Nat) -> List<&2, D.Node>:
  WSpace.blocks.loc(I.En{}, bs, bot, i)

def WSpace.title(+t: String) -> List<&2, D.Node>:
  match t:
    case SNil{}:
      Nil{}
    case SCon{_, _}:
      [WSpace.txt("div", "sp-title", t)]

# a bot's space
def WSpace.view.loc(+lc: I.Loc, sp: SP.Space, +bot: String) -> D.Node:
  match sp:
    case SP.Space{+t, bs}:
      D.El{"div", "space", [D.At{"class", "sp"}], D.Dom.cat(WSpace.title(t), WSpace.blocks.loc(lc, bs, bot, 0n))}

def WSpace.view(sp: SP.Space, +bot: String) -> D.Node:
  WSpace.view.loc(I.En{}, sp, bot)
