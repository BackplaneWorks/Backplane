import Base
import ./dom.bend as D

# A timeline row's height before it is measured, from what the row holds
# (the planner places a row not measured yet at this; it is measured once
# on screen). Fitted on real threads (re-render harness, 2026-11-01): a
# folded run of tool calls is 26 px, an activity line 22.5, a message 24
# plus, per block (paragraph, list item, heading, table row, quote), 12
# and 22 a line of about width / 6.4 characters, a code block 16 a line.
# The thread's mean height, used before, was 3x too tall for a tool-heavy
# stretch (laws row_estimate_*).

# (Nat.max recurses once per unit)
def Est.max(+a: Nat, +b: Nat) -> Nat:
  Bool.pick(Nat, Nat.is_ge(a, b), a, b)

def Est.ceil(+a: Nat, +b: Nat) -> Nat:
  Nat.div(Nat.add(a, Nat.sub(b, 1n)), b)

# the characters a node list holds
def Est.text(ns: List<&2, D.Node>, acc: Nat) -> Nat:
  match ns:
    case Nil{}: acc
    case D.Txt{s} <> t: Est.text(t, Nat.add(acc, String.length(s)))
    case D.El{_, _, _, k} <> t: Est.text(t, Est.text(k, acc))

# the line breaks a text holds
def Est.breaks(s: String, +acc: Nat) -> Nat:
  match s:
    case SNil{}: acc
    case SCon{Chr{+c}, t}: Est.breaks(t, Bool.pick(Nat, U32.is_eq(c, 10), Nat.add(acc, 1n), acc))

def Est.breaks.of(ns: List<&2, D.Node>, acc: Nat) -> Nat:
  match ns:
    case Nil{}: acc
    case D.Txt{s} <> t: Est.breaks.of(t, Est.breaks(s, acc))
    case D.El{_, _, _, k} <> t: Est.breaks.of(t, Est.breaks.of(k, acc))

# what a block element is: 0 not a block (look inside), 1 prose, 2 code
def Est.kind(+tag: String) -> Nat:
  match tag:
    case "p": 1n
    case "li": 1n
    case "h1": 1n
    case "h2": 1n
    case "h3": 1n
    case "h4": 1n
    case "tr": 1n
    case "blockquote": 1n
    case "pre": 2n
    case _: 0n

def Est.block(k: Nat, +kids: List<&2, D.Node>, +cpl: Nat, +inner: Nat) -> Nat:
  match k:
    case 0n: inner
    case 1n: Nat.add(12n, Nat.mul(22n, Est.max(1n, Est.ceil(Est.text(kids, 0n), cpl))))
    case _: Nat.add(12n, Nat.mul(16n, Nat.add(1n, Est.breaks.of(kids, 0n))))

# the blocks' heights, summed (a block is not looked inside)
def Est.blocks(ns: List<&2, D.Node>, +cpl: Nat, acc: Nat) -> Nat:
  match ns:
    case Nil{}: acc
    case D.Txt{_} <> t: Est.blocks(t, cpl, acc)
    case D.El{+tag, _, _, +k} <> t: Est.blocks(t, cpl, Nat.add(acc, Est.block(Est.kind(tag), k, cpl, Est.blocks(k, cpl, 0n))))

def Est.class(as: List<&2, D.Attr>) -> String:
  match as:
    case Nil{}: ""
    case D.At{"class", v} <> _: v
    case _ <> t: Est.class(t)

def Est.first(+cls: String) -> String:
  Maybe.default(&2, String, List.head(&2, String, String.split(cls, ' ')), "")

# a message: its blocks, or with none its text as one block
def Est.message(+blocks: Nat, +text: Nat, +cpl: Nat) -> Nat:
  Nat.add(24n, Bool.pick(Nat, Nat.is_eq(blocks, 0n), Nat.add(12n, Nat.mul(22n, Est.max(1n, Est.ceil(text, cpl)))), blocks))

def Est.by(+kind: String, +kids: List<&2, D.Node>, +cpl: Nat) -> Nat:
  match kind:
    case "fold": 26n
    case "act": 23n
    case "parent": 20n
    case "work": 0n
    case "hold-pill": 0n
    case "earlier": 1n
    case _: Est.message(Est.blocks(kids, cpl, 0n), Est.text(kids, 0n), cpl)

# the row's estimated height in px at the column's width
def Row.estimate(node: D.Node, +width: Nat) -> Nat:
  match node:
    case D.El{_, _, +as, +kids}: Est.by(Est.first(Est.class(as)), kids, Est.max(20n, Nat.div(Nat.mul(width, 5n), 32n)))
    case D.Txt{_}: 0n
