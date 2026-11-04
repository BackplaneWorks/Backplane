import Base
import ./board.bend as B
import ./schematic.bend as SC
import ./look.bend as LO

# The layer list and the focus (pure)
# ===================================
# Every client lists a board's layers in KiCad's order (its Appearance
# panel): copper from front to back, then the pads on every copper layer,
# the vias and the holes, the technical layers in front/back pairs, the
# user layers, the edge and margin, the courtyards and the fab layers; each
# named as KiCad 7+ names it. A schematic keeps its own order. A
# comparison lists its marks after.
#
# The focus is KiCad's high contrast (ctrl+H): one active layer, and the
# others shown as they are (mode 0), dimmed (1) or not at all (2). What
# stays with the active layer: the board's outline (for bearing), and with
# a copper layer the pads, vias and holes on it; a comparison's marks.

# board.bend's Layer.names indices in KiCad's order
def Lyr.board.order() -> List<&2, U32>:
  [12, 11, 10, 9, 8, 7, 6, 5, 13, 23, 24, 15, 2, 16, 3, 14, 4, 19, 20, 22, 21, 17, 1, 18, 0]

def Lyr.upto.go(n: Nat, +i: U32, acc: List<&2, U32>) -> List<&2, U32>:
  match n:
    case 0n:
      List.reverse(&2, U32, acc)
    case 1n+k:
      Lyr.upto.go(k, (i + 1 : U32), i <> acc)

# 0, 1, ... n-1
def Lyr.upto(+n: Nat) -> List<&2, U32>:
  Lyr.upto.go(n, 0, Nil{})

def Lyr.sch(+kind: U32) -> Bool:
  U32.is_eq(LO.Look.base(kind), 1)

def Lyr.own(sch: Bool) -> List<&2, U32>:
  match sch:
    case True{}:
      Lyr.upto(24n)
    case False{}:
      Lyr.board.order()

def Lyr.marks(diff: Bool) -> List<&2, U32>:
  match diff:
    case True{}:
      [26, 27, 28]
    case False{}:
      Nil{}

# the order a client lists a kind's layers in (kind 0 a board, 1 a
# schematic, with LO.Look.diff() a comparison)
def Lyr.order(+kind: U32) -> List<&2, U32>:
  List.append(&2, U32, Lyr.own(Lyr.sch(kind)), Lyr.marks(LO.Look.is_diff(kind)))

# KiCad's names where they differ from the file's
def Lyr.board.label(+i: Nat, +raw: String) -> String:
  match i:
    case 1n:
      "B.Courtyard"
    case 3n:
      "B.Silkscreen"
    case 13n:
      "Pads"
    case 16n:
      "F.Silkscreen"
    case 17n:
      "F.Courtyard"
    case 19n:
      "User.Drawings"
    case 20n:
      "User.Comments"
    case 23n:
      "Vias"
    case 24n:
      "Holes"
    case _:
      raw

def Lyr.label.k(sch: Bool, +i: U32, +raw: String) -> String:
  match sch:
    case True{}:
      raw
    case False{}:
      Lyr.board.label(U32.to_nat(i), raw)

def Lyr.raw(+sch: Bool) -> List<&2, String>:
  Bool.pick(List<&2, String>, sch, SC.Sch.layer_names(), B.Layer.names())

# a layer's name in the list
def Lyr.name(+kind: U32, +i: U32) -> String:
  Lyr.label.k(Lyr.sch(kind), i, Maybe.default(&2, String, List.get(&2, String, LO.Look.names(kind, Lyr.raw(Lyr.sch(kind))), U32.to_nat(i)), ""))

# The focus
# ---------

type LayerFocus is Data:
  LayerFocus{active: U32, mode: U32}

# no active layer
def Lyr.none() -> U32:
  255

def Lyr.plain() -> LayerFocus:
  LayerFocus{Lyr.none(), 0}

def Lyr.active(f: LayerFocus) -> U32:
  match f:
    case LayerFocus{a, _}:
      a

def Lyr.mode(f: LayerFocus) -> U32:
  match f:
    case LayerFocus{_, m}:
      m

# whether the focus changes anything
def Lyr.on(+f: LayerFocus) -> Bool:
  Bool.and(U32.is_gt(Lyr.mode(f), 0), Bool.not(U32.is_eq(Lyr.active(f), Lyr.none())))

# kept as text in each client's state: "<active>|<mode>"
def Lyr.show(+f: LayerFocus) -> String:
  U32.show(Lyr.active(f)) ++ "|" ++ U32.show(Lyr.mode(f))

def Lyr.num(+s: String, +d: U32) -> U32:
  Maybe.default(&2, U32, U32.read(s), d)

def Lyr.read.k(ps: List<&2, String>) -> LayerFocus:
  match ps:
    case +a <> +m <> _:
      LayerFocus{Lyr.num(a, Lyr.none()), U32.min(Lyr.num(m, 0), 2)}
    case _:
      Lyr.plain()

def Lyr.read(+s: String) -> LayerFocus:
  Lyr.read.k(String.split(s, '|'))

# the copper layers (pads, vias and holes stay with them)
def Lyr.copper(+kind: U32, +l: U32) -> Bool:
  Bool.and(Bool.not(Lyr.sch(kind)), Bool.and(U32.is_ge(l, 5), U32.is_le(l, 12)))

def Lyr.bit(+l: U32) -> U32:
  U32.shln(1, U32.to_nat(U32.min(l, 31)))

# the layers that stay as they are beside the active one, as bits
def Lyr.kept(+kind: U32, +f: LayerFocus) -> U32:
  U32.or(U32.or(Lyr.bit(Lyr.active(f)),
      Bool.pick(U32, Lyr.copper(kind, Lyr.active(f)), U32.or(U32.or(Lyr.bit(13), Lyr.bit(23)), Lyr.bit(24)), 0)),
    U32.or(Bool.pick(U32, Lyr.sch(kind), 0, Lyr.bit(22)), U32.or(U32.or(Lyr.bit(26), Lyr.bit(27)), Lyr.bit(28))))

def Lyr.has(+bits: U32, +l: U32) -> Bool:
  Bool.not(U32.is_eq(U32.and(bits, Lyr.bit(l)), 0))

# every layer a board, a schematic or a comparison has (bits 0-28)
def Lyr.all() -> U32:
  536870911

def Lyr.shade.k(on: Bool, active: Bool, kept: Bool, +mode: U32, look: U32 & U32) -> U32 & U32:
  match on active kept:
    case False{} _ _:
      look
    case True{} True{} _:
      (c, +a) = look
      (c, Bool.pick(U32, U32.is_eq(a, 0), 200, a))
    case True{} False{} True{}:
      look
    case True{} False{} False{}:
      (c, +a) = look
      (c, Bool.pick(U32, U32.is_eq(mode, 1), (a / 4 : U32), 0))

# a layer's colour and opacity under the focus: the active layer always
# shows, those kept with it as they are, the rest dimmed to a quarter or
# hidden
def Lyr.shade(+kind: U32, +f: LayerFocus, +l: U32, look: U32 & U32) -> U32 & U32:
  Lyr.shade.k(Lyr.on(f), U32.is_eq(l, Lyr.active(f)), Lyr.has(Lyr.kept(kind, f), l), Lyr.mode(f), look)

type Masks is Data:
  Masks{off: U32, dim: U32}

# for the clients that draw from bits (the web, the phones): the layers
# not drawn (the person's own, those the focus hides, never the active
# one) and those drawn dimmed
def Lyr.masks.k(on: Bool, +kind: U32, +off: U32, +f: LayerFocus) -> Masks:
  match on:
    case False{}:
      Masks{off, 0}
    case True{}:
      +rest = {U32.and(Lyr.all(), U32.not(Lyr.kept(kind, f))) : U32}
      +act = {U32.not(Lyr.bit(Lyr.active(f))) : U32}
      Bool.pick(Masks, U32.is_eq(Lyr.mode(f), 1),
        Masks{U32.and(off, act), U32.and(rest, U32.not(off))},
        Masks{U32.and(U32.or(off, rest), act), 0})

def Lyr.masks(+kind: U32, +off: U32, +f: LayerFocus) -> Masks:
  Lyr.masks.k(Lyr.on(f), kind, off, f)

def Masks.off(m: Masks) -> U32:
  match m:
    case Masks{o, _}:
      o

def Masks.dim(m: Masks) -> U32:
  match m:
    case Masks{_, d}:
      d

# The focus changes
# -----------------

# the layer a focus starts from when none is active: front copper on a
# board, the first layer of a schematic
def Lyr.first(+kind: U32) -> U32:
  Bool.pick(U32, Lyr.sch(kind), 0, 12)

def Lyr.or.first(+kind: U32, +a: U32) -> U32:
  Bool.pick(U32, U32.is_eq(a, Lyr.none()), Lyr.first(kind), a)

# a click on a layer's name: it becomes the active one and the others dim
# (or stay hidden, in that mode); a click on the active one again puts
# every layer back as it was
def Lyr.pick.k(again: Bool, +f: LayerFocus, +l: U32) -> LayerFocus:
  match again:
    case True{}:
      LayerFocus{l, 0}
    case False{}:
      LayerFocus{l, Bool.pick(U32, U32.is_eq(Lyr.mode(f), 0), 1, Lyr.mode(f))}

def Lyr.pick(+f: LayerFocus, +l: U32) -> LayerFocus:
  Lyr.pick.k(Bool.and(Lyr.on(f), U32.is_eq(Lyr.active(f), l)), f, l)

# the list's mode switch (All, Dim, Only)
def Lyr.set(+kind: U32, +f: LayerFocus, +mode: U32) -> LayerFocus:
  LayerFocus{Lyr.or.first(kind, Lyr.active(f)), U32.min(mode, 2)}

# ctrl+H, as in KiCad: as they are, dimmed, hidden, and around again
def Lyr.cycle(+kind: U32, +f: LayerFocus) -> LayerFocus:
  Lyr.set(kind, f, Bool.pick(U32, U32.is_eq(Lyr.mode(f), 2), 0, (Lyr.mode(f) + 1 : U32)))

# The window draws from a kind word: its kind and comparison flag in the
# low byte, the focus above (active in bits 8-15, mode in 16-17)
def Lyr.kind.with(+kind: U32, +f: LayerFocus) -> U32:
  U32.or(U32.and(kind, 255), U32.or(U32.shln(U32.and(Lyr.active(f), 255), 8n), U32.shln(U32.and(Lyr.mode(f), 3), 16n)))

def Lyr.kind.base(+kind: U32) -> U32:
  U32.and(kind, 255)

def Lyr.kind.focus(+kind: U32) -> LayerFocus:
  LayerFocus{U32.and(U32.shrn(kind, 8n), 255), U32.and(U32.shrn(kind, 16n), 3)}

# The active layer drawn on top
# ------------------------------
# A layer drawn apart from the rest of the scene: every layer but it
# first (under), then it alone (over); none draws all in one pass. As a
# number: 255 none, below that every layer but it, 256 and up it alone.

def Lyr.lift.none() -> U32:
  255

def Lyr.lift.under(+f: LayerFocus) -> U32:
  U32.min(Lyr.active(f), 254)

def Lyr.lift.over(+f: LayerFocus) -> U32:
  (U32.min(Lyr.active(f), 254) + 256 : U32)

def Lyr.lift.ok(+lift: U32, +l: U32) -> Bool:
  Bool.pick(Bool, U32.is_eq(lift, 255), True{}, Bool.pick(Bool, U32.is_lt(lift, 255), Bool.not(U32.is_eq(l, lift)), U32.is_eq(l, (lift - 256 : U32))))

# the layer the web and phones draw last (255 none)
def Lyr.top(+f: LayerFocus) -> U32:
  Bool.pick(U32, Lyr.on(f), Lyr.active(f), Lyr.none())
