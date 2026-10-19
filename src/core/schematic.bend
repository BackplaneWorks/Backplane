import Base
import ./sexpr.bend as S
import ./board.bend as B
import ./strokefont.bend as ST

# A .kicad_sch as drawable items, the same B.Item the board viewer draws:
# keys are KiCad uuids (a symbol's children add "/pin/<n>", "/f/<field>",
# "/g<i>"), signatures hash the source form, layers index Sch.layer_names.
# Library symbols come from the file's own lib_symbols cache; every other
# top-level form converts on its own, in parallel. Sheet coordinates are mm
# with Y down; library coordinates have Y up.

# Layers
# ------

# every schematic layer, in paint order (first is lowest)
def Sch.layer_names() -> List<&2, String>:
  ["page", "sheet fill", "body fill", "sheet", "graphic", "note", "body", "pin", "pin name", "pin number",
   "field", "field other", "bus", "bus entry", "wire", "no connect", "junction", "label", "global label",
   "hier label", "sheet name", "sheet file", "sheet pin", "dnp"]

def L.page() -> U32:
  0

def L.sheet_fill() -> U32:
  1

def L.body_fill() -> U32:
  2

def L.sheet() -> U32:
  3

def L.graphic() -> U32:
  4

def L.note() -> U32:
  5

def L.body() -> U32:
  6

def L.pin() -> U32:
  7

def L.pin_name() -> U32:
  8

def L.pin_num() -> U32:
  9

def L.field() -> U32:
  10

def L.field_other() -> U32:
  11

def L.bus() -> U32:
  12

def L.bus_entry() -> U32:
  13

def L.wire() -> U32:
  14

def L.nc() -> U32:
  15

def L.junction() -> U32:
  16

def L.label() -> U32:
  17

def L.global() -> U32:
  18

def L.hier() -> U32:
  19

def L.sheet_name() -> U32:
  20

def L.sheet_file() -> U32:
  21

def L.sheet_pin() -> U32:
  22

def L.dnp() -> U32:
  23

def Sch.look.k(i: Nat) -> U32 & U32:
  match i:
    case 0n:
      (8018790, 200)
    case 1n:
      (2758711, 90)
    case 2n:
      (4209695, 110)
    case 3n:
      (11757747, 255)
    case 4n:
      (8360959, 230)
    case 5n:
      (8360959, 255)
    case 6n:
      (13788239, 255)
    case 7n:
      (13788239, 255)
    case 8n:
      (4046261, 255)
    case 9n:
      (13131866, 255)
    case 10n:
      (4046261, 255)
    case 11n:
      (11561648, 230)
    case 12n:
      (5209599, 255)
    case 13n:
      (5209599, 255)
    case 14n:
      (4177743, 255)
    case 15n:
      (7307263, 255)
    case 16n:
      (4177743, 255)
    case 17n:
      (15132390, 255)
    case 18n:
      (14707290, 255)
    case 19n:
      (14263361, 255)
    case 20n:
      (4046261, 255)
    case 21n:
      (11045466, 255)
    case 22n:
      (14263361, 255)
    case 23n:
      (14696512, 220)
    case _:
      (10000536, 200)

# colour (0xRRGGBB) and opacity of a layer, for a #0d0f12 background:
# KiCad's schematic palette, lifted for a dark UI
def Sch.look(+layer: U32) -> U32 & U32:
  Sch.look.k(U32.to_nat(layer))

# Reading forms
# -------------

def Sch.is_atom(x: S.Sx, +a: String) -> Bool:
  match x:
    case S.XAtom{s}:
      String.eq(s, a)
    case _:
      False{}

def Sch.atom(xs: List<&2, S.Sx>, +a: String) -> Bool:
  match xs:
    case Nil{}:
      False{}
    case x <> t:
      Bool.or(Sch.is_atom(x, a), Sch.atom(t, a))

def Sch.has(x: S.Sx, +n: String) -> Bool:
  String.eq(S.Sx.name(S.Sx.get(x, n)), n)

def Sch.flag.k(has: Bool, +v: String) -> Bool:
  match has:
    case True{}:
      Bool.not(String.eq(v, "no"))
    case False{}:
      False{}

# (name yes), (name), or a bare `name` atom (older files)
def Sch.flag(+x: S.Sx, +n: String) -> Bool:
  Bool.or(Sch.flag.k(Sch.has(x, n), S.Sx.attr(x, n)), Sch.atom(S.Sx.args(x), n))

def Sch.or.f(+v: F32, +d: F32) -> F32:
  Bool.pick(F32, F32.is_gt(v, 0.0), v, d)

# the stroke width, or d when it is 0 (KiCad's "default")
def Sch.width(+x: S.Sx, +d: F32) -> F32:
  Sch.or.f(S.Sx.f(S.Sx.get(S.Sx.get(x, "stroke"), "width"), 0n), d)

def Sch.fill.k(+t: String, +colored: Bool, +faint: Bool) -> Nat:
  Bool.pick(Nat, String.eq(t, "outline"), 1n, Bool.pick(Nat, Bool.and(colored, faint), 0n,
    Bool.pick(Nat, Bool.or(String.eq(t, "background"), Bool.or(String.eq(t, "color"), colored)), 2n, 0n)))

# 0 no fill, 1 filled with the outline colour, 2 filled with the body
# colour; a fill of its own colour that is nearly clear (a tint under a
# quarter opaque, as a group's box often has) is left out, since the body
# colour would paint it solid over what it groups
def Sch.fill(+x: S.Sx) -> Nat:
  Sch.fill.k(S.Sx.attr(S.Sx.get(x, "fill"), "type"), F32.is_gt(S.Sx.f(S.Sx.get(S.Sx.get(x, "fill"), "color"), 3n), 0.0),
    F32.is_lt(S.Sx.f(S.Sx.get(S.Sx.get(x, "fill"), "color"), 3n), 0.25))

# degrees to 0..360
def Sch.deg(+d: F32) -> F32:
  F32.sub(d, F32.mul(360.0, F32.floor(F32.div(d, 360.0))))

# whether an angle is vertical (90 or 270)
def Sch.vertical(+d: F32) -> Bool:
  +n = Sch.deg(d)
  Bool.or(F32.is_lt(F32.abs(F32.sub(n, 90.0)), 1.0), F32.is_lt(F32.abs(F32.sub(n, 270.0)), 1.0))

# a readable text angle: KiCad never draws schematic text upside down
def Sch.readable(+v: Bool) -> F32:
  Bool.pick(F32, v, F32.div(F32.pi(), 2.0), 0.0)

# Text
# ----

# height, character width, pen, justification (0 left/top, 1 centre,
# 2 right/bottom) and whether it is hidden
type Tx is Data:
  Tx{h: F32, w: F32, pen: F32, hj: U32, vj: U32, hide: Bool, it: Bool}

def Tx.pen(+t: F32, +bold: Bool, +h: F32) -> F32:
  Sch.or.f(t, F32.div(h, Bool.pick(F32, bold, 5.0, 8.0)))

def Tx.hj(+js: List<&2, S.Sx>) -> U32:
  Bool.pick(U32, Sch.atom(js, "left"), 0, Bool.pick(U32, Sch.atom(js, "right"), 2, 1))

def Tx.vj(+js: List<&2, S.Sx>) -> U32:
  Bool.pick(U32, Sch.atom(js, "top"), 0, Bool.pick(U32, Sch.atom(js, "bottom"), 2, 1))

# the text style of a form with (effects (font (size h w) ...) (justify ..))
def Tx.of(+x: S.Sx) -> Tx:
  +e = S.Sx.get(x, "effects")
  +f = S.Sx.get(e, "font")
  +sz = S.Sx.get(f, "size")
  +h = Sch.or.f(S.Sx.f(sz, 0n), 1.27)
  +js = S.Sx.args(S.Sx.get(e, "justify"))
  Tx{h, Sch.or.f(S.Sx.f(sz, 1n), 1.27), Tx.pen(S.Sx.f(S.Sx.get(f, "thickness"), 0n), Sch.flag(f, "bold"), h), Tx.hj(js), Tx.vj(js),
    Bool.or(Sch.flag(e, "hide"), Sch.flag(x, "hide")), Sch.flag(f, "italic")}

def Tx.just(t: Tx, +hj: U32, +vj: U32) -> Tx:
  match t:
    case Tx{h, w, pen, _, _, hide, it}:
      Tx{h, w, pen, hj, vj, hide, it}

def Tx.h(t: Tx) -> F32:
  match t:
    case Tx{h, _, _, _, _, _, _}:
      h

def Tx.w(t: Tx) -> F32:
  match t:
    case Tx{_, w, _, _, _, _, _}:
      w

def Tx.pen.of(t: Tx) -> F32:
  match t:
    case Tx{_, _, pen, _, _, _, _}:
      pen

def Tx.hidden(t: Tx) -> Bool:
  match t:
    case Tx{_, _, _, _, _, hide, _}:
      hide

# a text's strokes at (x, y), `a` radians
def Sch.text(+s: String, +x: F32, +y: F32, +a: F32, t: Tx) -> List<&2, B.Prim>:
  match t:
    case Tx{h, w, pen, hj, vj, _, it}:
      ST.Stroke.text.full(s, x, y, h, w, pen, a, hj, vj, False{}, it)

# items for a list of prims; each key gets "/<i>"
def Sch.put(ps: List<&2, B.Prim>, +key: String, +sig: U32, +layer: U32, +info: String, +i: Nat, rest: List<&2, B.Item>) -> List<&2, B.Item>:
  match ps:
    case Nil{}:
      rest
    case p <> t:
      B.Item{key ++ "/" ++ Nat.show(i), sig, layer, p, info} <> Sch.put(t, key, sig, layer, info, 1n+i, rest)

def Sch.text.show(hide: Bool, +s: String, +x: F32, +y: F32, +a: F32, +t: Tx, +key: String, +sig: U32, +layer: U32, +info: String, rest: List<&2, B.Item>) -> List<&2, B.Item>:
  match hide:
    case True{}:
      rest
    case False{}:
      Sch.put(Sch.text(s, x, y, a, t), key, sig, layer, info, 0n, rest)

# a text's items unless it is hidden or empty
def Sch.text.items(+s: String, +x: F32, +y: F32, +a: F32, +t: Tx, +key: String, +sig: U32, +layer: U32, +info: String, rest: List<&2, B.Item>) -> List<&2, B.Item>:
  Sch.text.show(Bool.or(Tx.hidden(t), String.is_empty(s)), s, x, y, a, t, key, sig, layer, info, rest)

# Placement
# ---------

# where a symbol puts library points: flip Y (fy -1 for library points),
# mirror (mx, my), rotate by a radians, move to (x, y)
type Xf is Data:
  Xf{x: F32, y: F32, a: F32, mx: F32, my: F32, fy: F32}

def Xf.home() -> Xf:
  Xf{0.0, 0.0, 0.0, 1.0, 1.0, 1.0}

# a sheet-oriented vector mirrored and rotated
def Xf.turn(t: Xf, +dx: F32, +dy: F32) -> B.Pt:
  match t:
    case Xf{_, _, a, mx, my, _}:
      B.Geo.rot(B.Pt{F32.mul(dx, mx), F32.mul(dy, my)}, a)

# the inverse of Xf.turn
def Xf.unturn.k(p: B.Pt, +mx: F32, +my: F32) -> B.Pt:
  match p:
    case B.Pt{x, y}:
      B.Pt{F32.mul(x, mx), F32.mul(y, my)}

def Xf.unturn(t: Xf, +dx: F32, +dy: F32) -> B.Pt:
  match t:
    case Xf{_, _, a, +mx, +my, _}:
      Xf.unturn.k(B.Geo.rot(B.Pt{dx, dy}, F32.neg(a)), mx, my)

def Xf.fy(t: Xf) -> F32:
  match t:
    case Xf{_, _, _, _, _, fy}:
      fy

def Xf.at(t: Xf) -> B.Pt:
  match t:
    case Xf{x, y, _, _, _, _}:
      B.Pt{x, y}

def Pt.add(a: B.Pt, b: B.Pt) -> B.Pt:
  match a b:
    case B.Pt{ax, ay} B.Pt{bx, by}:
      B.Pt{F32.add(ax, bx), F32.add(ay, by)}

def Pt.sub(a: B.Pt, b: B.Pt) -> B.Pt:
  match a b:
    case B.Pt{ax, ay} B.Pt{bx, by}:
      B.Pt{F32.sub(ax, bx), F32.sub(ay, by)}

def Pt.scale(a: B.Pt, +k: F32) -> B.Pt:
  match a:
    case B.Pt{x, y}:
      B.Pt{F32.mul(x, k), F32.mul(y, k)}

def Pt.x(a: B.Pt) -> F32:
  match a:
    case B.Pt{x, _}:
      x

def Pt.y(a: B.Pt) -> F32:
  match a:
    case B.Pt{_, y}:
      y

# a library (or sheet) vector on the sheet
def Xf.vec(+t: Xf, +dx: F32, +dy: F32) -> B.Pt:
  Xf.turn(t, dx, F32.mul(dy, Xf.fy(t)))

def Xf.pt(+t: Xf, +px: F32, +py: F32) -> B.Pt:
  Pt.add(Xf.at(t), Xf.vec(t, px, py))

def Xf.sx(+t: Xf, +x: S.Sx) -> B.Pt:
  Xf.pt(t, S.Sx.f(x, 0n), S.Sx.f(x, 1n))

def Xf.get(+t: Xf, +x: S.Sx, +n: String) -> B.Pt:
  Xf.sx(t, S.Sx.get(x, n))

# whether the rotation turns horizontal into vertical
def Xf.quarter(t: Xf) -> Bool:
  match t:
    case Xf{_, _, +a, _, _, _}:
      F32.is_gt(F32.abs(F32.sin(a)), 0.5)

def Sch.pts.k(xy: Bool, +x: S.Sx, +t: Xf, rest: List<&2, B.Pt>) -> List<&2, B.Pt>:
  match xy:
    case True{}:
      Xf.sx(t, x) <> rest
    case False{}:
      rest

# the (xy ..) points of a (pts ..) list
def Sch.pts(xs: List<&2, S.Sx>, +t: Xf) -> List<&2, B.Pt>:
  match xs:
    case Nil{}:
      Nil{}
    case +x <> r:
      Sch.pts.k(String.eq(S.Sx.name(x), "xy"), x, t, Sch.pts(r, t))

# Graphics
# --------

def Bez.at(+f: F32, a: B.Pt, b: B.Pt, c: B.Pt, d: B.Pt) -> B.Pt:
  +g = F32.sub(1.0, f)
  Pt.add(Pt.add(Pt.scale(a, F32.mul(g, F32.mul(g, g))), Pt.scale(b, F32.mul(3.0, F32.mul(f, F32.mul(g, g))))),
    Pt.add(Pt.scale(c, F32.mul(3.0, F32.mul(f, F32.mul(f, g)))), Pt.scale(d, F32.mul(f, F32.mul(f, f)))))

def Bez.steps(n: Nat, +i: Nat, +a: B.Pt, +b: B.Pt, +c: B.Pt, +d: B.Pt) -> List<&2, B.Pt>:
  match n:
    case 0n:
      [Bez.at(F32.div(F32.from_nat(i), 16.0), a, b, c, d)]
    case 1n+m:
      Bez.at(F32.div(F32.from_nat(i), 16.0), a, b, c, d) <> Bez.steps(m, 1n+i, a, b, c, d)

def Bez.pts(ps: List<&2, B.Pt>) -> List<&2, B.Pt>:
  match ps:
    case +a <> +b <> +c <> +d <> _:
      Bez.steps(16n, 0n, a, b, c, d)
    case _:
      ps

def Sch.rect(a: B.Pt, b: B.Pt) -> List<&2, B.Pt>:
  match a b:
    case B.Pt{+ax, +ay} B.Pt{+bx, +by}:
      [B.Pt{ax, ay}, B.Pt{bx, ay}, B.Pt{bx, by}, B.Pt{ax, by}]

def Sch.rect.lib(+t: Xf, a: B.Pt, b: B.Pt) -> List<&2, B.Pt>:
  match a b:
    case B.Pt{+ax, +ay} B.Pt{+bx, +by}:
      [Xf.pt(t, ax, ay), Xf.pt(t, bx, ay), Xf.pt(t, bx, by), Xf.pt(t, ax, by)]

def Sch.pt(+x: S.Sx, +n: String) -> B.Pt:
  B.Geo.at(x, n)

# an outline and, when filled, its fill (on `lf`, or the outline's layer)
def Sch.fill.put(k: Nat, +fill: B.Prim, +line: B.Prim, +key: String, +sig: U32, +ll: U32, +lf: U32, +info: String) -> List<&2, B.Item>:
  match k:
    case 1n:
      [B.Item{key ++ "/f", sig, ll, fill, info}, B.Item{key, sig, ll, line, info}]
    case 2n:
      [B.Item{key ++ "/f", sig, lf, fill, info}, B.Item{key, sig, ll, line, info}]
    case _:
      [B.Item{key, sig, ll, line, info}]

def Sch.circ(+c: B.Pt, +r: F32, +w: F32, +k: Nat, +key: String, +sig: U32, +ll: U32, +lf: U32, +info: String) -> List<&2, B.Item>:
  Sch.fill.put(k, B.PCirc{c, r, 0.0, True{}}, B.PCirc{c, r, w, False{}}, key, sig, ll, lf, info)

def Sch.poly(+ps: List<&2, B.Pt>, +closed: Bool, +w: F32, +k: Nat, +key: String, +sig: U32, +ll: U32, +lf: U32, +info: String) -> List<&2, B.Item>:
  Sch.fill.put(k, B.PPoly{ps, 0.0, True{}}, Bool.pick(B.Prim, closed, B.PPoly{ps, w, False{}}, B.PLine{ps, w}), key, sig, ll, lf, info)

# polyline 0, rectangle 1, circle 2, arc 3, bezier 4, text 5
def Sch.shapes() -> List<&2, String>:
  ["polyline", "rectangle", "circle", "arc", "bezier", "text"]

def Sch.index.hit(ns: List<&2, String>, +s: String) -> Bool:
  match ns:
    case n <> _:
      String.eq(n, s)
    case Nil{}:
      False{}

def Sch.index.go(ns: List<&2, String>, hit: Bool, +s: String, +i: Nat) -> Nat:
  match ns hit:
    case _ <> _ True{}:
      i
    case _ <> +t False{}:
      Sch.index.go(t, Sch.index.hit(t, s), s, 1n+i)
    case Nil{} _:
      99n

# a name's position in ns (99 when absent)
def Sch.index(+ns: List<&2, String>, +s: String) -> Nat:
  Sch.index.go(ns, Sch.index.hit(ns, s), s, 0n)

# text inside a symbol turns with it but stays readable
def Sch.shape.text(+x: S.Sx, +t: Xf, +key: String, +sig: U32, +layer: U32, +info: String) -> List<&2, B.Item>:
  +at = S.Sx.get(x, "at")
  +p = Xf.sx(t, at)
  Sch.text.items(S.Sx.arg(x, 0n), Pt.x(p), Pt.y(p), Sch.readable(Bool.xor(Sch.vertical(S.Sx.f(at, 2n)), Xf.quarter(t))),
    Tx.of(x), key, sig, layer, info, Nil{})

def Sch.shape.k(i: Nat, +x: S.Sx, +t: Xf, +key: String, +sig: U32, +ll: U32, +lf: U32, +info: String, +w: F32, +fk: Nat) -> List<&2, B.Item>:
  match i:
    case 0n:
      Sch.poly(Sch.pts(S.Sx.args(S.Sx.get(x, "pts")), t), False{}, w, fk, key, sig, ll, lf, info)
    case 1n:
      Sch.poly(Sch.rect.lib(t, Sch.pt(x, "start"), Sch.pt(x, "end")), True{}, w, fk, key, sig, ll, lf, info)
    case 2n:
      Sch.circ(Xf.get(t, x, "center"), S.Sx.f(S.Sx.get(x, "radius"), 0n), w, fk, key, sig, ll, lf, info)
    case 3n:
      [B.Item{key, sig, ll, B.PLine{B.Arc.points(Xf.get(t, x, "start"), Xf.get(t, x, "mid"), Xf.get(t, x, "end")), w}, info}]
    case 4n:
      [B.Item{key, sig, ll, B.PLine{Bez.pts(Sch.pts(S.Sx.args(S.Sx.get(x, "pts")), t)), w}, info}]
    case 5n:
      Sch.shape.text(x, t, key, sig, L.note(), info)
    case _:
      Nil{}

# a graphic form placed by t: outline on ll, fill on lf
def Sch.shape(+x: S.Sx, +t: Xf, +key: String, +sig: U32, +ll: U32, +lf: U32, +info: String) -> List<&2, B.Item>:
  Sch.shape.k(Sch.index(Sch.shapes(), S.Sx.name(x)), x, t, key, sig, ll, lf, info, Sch.width(x, 0.1524), Sch.fill(x))

# Coloured boxes
# --------------
# A sheet's own shape or text box filled with a colour, (fill (type color)
# (color r g b a)), is a tint of that colour at its opacity under
# everything on the sheet (on "sheet fill"), as KiCad draws it, and its
# outline stays a graphic. No other fill of a sheet's own shapes hides
# what is under it: only "outline" fills with the line's colour.

def Tint.rgb(+c: S.Sx) -> U32:
  U32.or(U32.or(U32.shln(F32.to_u32(F32.round(S.Sx.f(c, 0n))), 16n), U32.shln(F32.to_u32(F32.round(S.Sx.f(c, 1n))), 8n)), F32.to_u32(F32.round(S.Sx.f(c, 2n))))

def Tint.a8(+c: S.Sx) -> U32:
  F32.to_u32(F32.round(F32.mul(F32.min(S.Sx.f(c, 3n), 1.0), 255.0)))

# whether a form's fill is a colour that shows
def Tint.on(+x: S.Sx) -> Bool:
  Bool.and(String.eq(S.Sx.attr(S.Sx.get(x, "fill"), "type"), "color"), F32.is_gt(S.Sx.f(S.Sx.get(S.Sx.get(x, "fill"), "color"), 3n), 0.0))

def Tint.ring.at(+i: Nat, +cx: F32, +cy: F32, +r: F32) -> B.Pt:
  +a = F32.mul(F32.div(F32.from_nat(i), 32.0), F32.mul(2.0, F32.pi()))
  B.Pt{F32.add(cx, F32.mul(r, F32.cos(a))), F32.add(cy, F32.mul(r, F32.sin(a)))}

def Tint.ring(n: Nat, +i: Nat, +cx: F32, +cy: F32, +r: F32) -> List<&2, B.Pt>:
  match n:
    case 0n:
      Nil{}
    case 1n+m:
      Tint.ring.at(i, cx, cy, r) <> Tint.ring(m, 1n+i, cx, cy, r)

def Tint.circle(c: B.Pt, +r: F32) -> List<&2, B.Pt>:
  match c:
    case B.Pt{+cx, +cy}:
      Tint.ring(32n, 0n, cx, cy, r)

# the area a sheet's shape i (Sch.shapes) fills
def Tint.pts(i: Nat, +x: S.Sx) -> List<&2, B.Pt>:
  match i:
    case 0n:
      Sch.pts(S.Sx.args(S.Sx.get(x, "pts")), Xf.home())
    case 1n:
      Sch.rect(Sch.pt(x, "start"), Sch.pt(x, "end"))
    case 2n:
      Tint.circle(Sch.pt(x, "center"), S.Sx.f(S.Sx.get(x, "radius"), 0n))
    case _:
      Nil{}

def Tint.items(on: Bool, +ps: List<&2, B.Pt>, +x: S.Sx, +key: String, +sig: U32, +info: String) -> List<&2, B.Item>:
  match on:
    case True{}:
      [B.Item{key ++ "/tint", sig, L.sheet_fill(), B.Tint.of(ps, Tint.rgb(S.Sx.get(S.Sx.get(x, "fill"), "color")), Tint.a8(S.Sx.get(S.Sx.get(x, "fill"), "color"))), info}]
    case False{}:
      Nil{}

# a sheet's own shape: its tint, then its outline (filled only by "outline")
def Sch.top(+x: S.Sx, +key: String, +sig: U32) -> List<&2, B.Item>:
  +i = Sch.index(Sch.shapes(), S.Sx.name(x))
  List.append(&2, B.Item, Tint.items(Tint.on(x), Tint.pts(i, x), x, key, sig, "graphic"),
    Sch.shape.k(i, x, Xf.home(), key, sig, L.graphic(), L.graphic(), "graphic", Sch.width(x, 0.1524),
      Bool.pick(Nat, String.eq(S.Sx.attr(S.Sx.get(x, "fill"), "type"), "outline"), 1n, 0n)))

# Library symbols
# ---------------

# a unit/body-style section ("NAME_u_s") and its drawing
type Part is Data:
  Part{u: U32, s: U32, xs: List<&2, S.Sx>}

# a library symbol: its lib_id, unit count, pin name offset, whether pin
# names and numbers show, its sections
type Lib is Data:
  Lib{name: String, units: U32, off: F32, names: Bool, nums: Bool, parts: List<&2, Part>}

def Lib.num(m: Maybe<&2, U32>) -> U32:
  Maybe.default(&2, U32, m, 0)

def Lib.part.k(r: List<&2, String>, xs: List<&2, S.Sx>) -> Part:
  match r:
    case s <> u <> _:
      Part{Lib.num(U32.read(u)), Lib.num(U32.read(s)), xs}
    case _:
      Part{0, 0, xs}

def Lib.part(+x: S.Sx) -> Part:
  Lib.part.k(List.reverse(&2, String, String.split(S.Sx.arg(x, 0n), '_')), S.Sx.args(x))

def Lib.parts(xs: List<&2, S.Sx>) -> List<&2, Part>:
  match xs:
    case Nil{}:
      Nil{}
    case x <> t:
      Lib.part(x) <> Lib.parts(t)

def Lib.units(ps: List<&2, Part>) -> U32:
  match ps:
    case Nil{}:
      1
    case Part{u, _, _} <> t:
      U32.max(u, Lib.units(t))

def Lib.off.k(has: Bool, +v: F32) -> F32:
  match has:
    case True{}:
      v
    case False{}:
      0.508

def Lib.of(+x: S.Sx) -> Lib:
  +pn = S.Sx.get(x, "pin_names")
  +parts = Lib.parts(S.Sx.all(S.Sx.args(x), "symbol"))
  Lib{S.Sx.arg(x, 0n), Lib.units(parts), Lib.off.k(Sch.has(pn, "offset"), S.Sx.f(S.Sx.get(pn, "offset"), 0n)),
    Bool.not(Sch.flag(pn, "hide")), Bool.not(Sch.flag(S.Sx.get(x, "pin_numbers"), "hide")), parts}

def Lib.all(xs: List<&2, S.Sx>) -> List<&2, Lib>:
  match xs:
    case Nil{}:
      Nil{}
    case x <> t:
      Lib.of(x) <> Lib.all(t)

# the symbols of a (lib_symbols ...) form
def Lib.table(+x: S.Sx) -> List<&2, Lib>:
  Lib.all(S.Sx.all(S.Sx.args(x), "symbol"))

def Lib.hit(ls: List<&2, Lib>, +id: String) -> Bool:
  match ls:
    case Lib{n, _, _, _, _, _} <> _:
      String.eq(n, id)
    case Nil{}:
      False{}

def Lib.find.go(ls: List<&2, Lib>, hit: Bool, +id: String) -> Maybe<&2, Lib>:
  match ls hit:
    case l <> _ True{}:
      Some{l}
    case _ <> +t False{}:
      Lib.find.go(t, Lib.hit(t, id), id)
    case Nil{} _:
      None{}

def Lib.find(+ls: List<&2, Lib>, +id: String) -> Maybe<&2, Lib>:
  Lib.find.go(ls, Lib.hit(ls, id), id)

# Pins
# ----

def Pin.deco() -> F32:
  0.635

def Pin.gap() -> F32:
  0.254

# a pin's texts: name inside the body past its end (offset > 0) or above
# the pin (offset 0, number below); the number above the pin's middle
type PinT is Data:
  PinT{name: String, num: String, tn: Tx, tm: Tx, off: F32, names: Bool, nums: Bool}

# along: the text direction's sign along the pin; up: the sheet vector
# pointing to the text's top side
def Pin.texts.k(t: PinT, +e: B.Pt, +mid: B.Pt, +d: B.Pt, +a: F32, +along: F32, +up: B.Pt, +key: String, +sig: U32, +info: String) -> List<&2, B.Item>:
  match t:
    case PinT{+name, +num, +tn, +tm, +off, names, nums}:
      +inside = F32.is_gt(off, 0.0)
      +np = Bool.pick(B.Pt, inside, Pt.add(e, Pt.scale(d, off)), Pt.add(mid, Pt.scale(up, Pin.gap())))
      +mp = Bool.pick(B.Pt, inside, Pt.add(mid, Pt.scale(up, Pin.gap())), Pt.sub(mid, Pt.scale(up, Pin.gap())))
      Sch.text.items(Bool.pick(String, Bool.or(Bool.not(names), String.eq(name, "~")), "", name), Pt.x(np), Pt.y(np), a,
        Tx.just(tn, Bool.pick(U32, inside, Bool.pick(U32, F32.is_gt(along, 0.0), 0, 2), 1), Bool.pick(U32, inside, 1, 2)),
        key ++ "/n", sig, L.pin_name(), info,
      Sch.text.items(Bool.pick(String, nums, num, ""), Pt.x(mp), Pt.y(mp), a,
        Tx.just(tm, 1, Bool.pick(U32, inside, 2, 0)), key ++ "/#", sig, L.pin_num(), info, Nil{}))

def Pin.texts(+t: PinT, +e: B.Pt, +mid: B.Pt, d: B.Pt, +key: String, +sig: U32, +info: String) -> List<&2, B.Item>:
  match d:
    case B.Pt{+dx, +dy}:
      +h = F32.is_gt(F32.abs(dx), 0.5)
      Pin.texts.k(t, e, mid, B.Pt{dx, dy}, Sch.readable(Bool.not(h)), Bool.pick(F32, h, dx, F32.neg(dy)),
        Bool.pick(B.Pt, h, B.Pt{0.0, F32.neg(1.0)}, B.Pt{F32.neg(1.0), 0.0}), key, sig, info)

def Pin.shape(+s: String) -> Nat:
  Bool.pick(Nat, String.eq(s, "inverted"), 1n, Bool.pick(Nat, String.eq(s, "clock"), 2n, Bool.pick(Nat, String.eq(s, "inverted_clock"), 3n, 0n)))

def Pin.clock(+e: B.Pt, d: B.Pt) -> List<&2, B.Pt>:
  match d:
    case B.Pt{+dx, +dy}:
      [Pt.add(e, B.Pt{F32.mul(F32.neg(dy), Pin.deco()), F32.mul(dx, Pin.deco())}), Pt.add(e, B.Pt{F32.mul(dx, Pin.deco()), F32.mul(dy, Pin.deco())}),
       Pt.add(e, B.Pt{F32.mul(dy, Pin.deco()), F32.mul(F32.neg(dx), Pin.deco())})]

def Pin.body.k(k: Nat, +line: B.Item, +short: B.Item, +bub: B.Item, +clk: B.Item) -> List<&2, B.Item>:
  match k:
    case 1n:
      [short, bub]
    case 2n:
      [line, clk]
    case 3n:
      [short, bub, clk]
    case _:
      [line]

# the pin line (shortened for a bubble) and its decoration
def Pin.body(+k: Nat, +p: B.Pt, +e: B.Pt, +d: B.Pt, +w: F32, +key: String, +sig: U32, +info: String) -> List<&2, B.Item>:
  Pin.body.k(k, B.Item{key, sig, L.pin(), B.PSeg{p, e, w}, info},
    B.Item{key, sig, L.pin(), B.PSeg{p, Pt.sub(e, Pt.scale(d, F32.mul(2.0, Pin.deco()))), w}, info},
    B.Item{key ++ "/o", sig, L.pin(), B.PCirc{Pt.sub(e, Pt.scale(d, Pin.deco())), Pin.deco(), w, False{}}, info},
    B.Item{key ++ "/c", sig, L.pin(), B.PLine{Pin.clock(e, d), w}, info})

def Pin.show(hide: Bool, +x: S.Sx, +t: Xf, +lib: Lib, +key: String, +sig: U32, +ref: String) -> List<&2, B.Item>:
  match hide lib:
    case True{} _:
      Nil{}
    case False{} Lib{_, _, +off, +names, +nums, _}:
      +at = S.Sx.get(x, "at")
      +a = B.Geo.rad(S.Sx.f(at, 2n))
      +len = S.Sx.f(S.Sx.get(x, "length"), 0n)
      +p = Xf.sx(t, at)
      +d = Xf.vec(t, F32.cos(a), F32.sin(a))
      +e = Pt.add(p, Pt.scale(d, len))
      +name = S.Sx.arg(S.Sx.get(x, "name"), 0n)
      +num = S.Sx.arg(S.Sx.get(x, "number"), 0n)
      +pk = key ++ "/pin/" ++ num
      +info = "pin " ++ num ++ B.Info.kv("name", name) ++ B.Info.kv("type", S.Sx.arg(x, 0n)) ++ B.Info.kv("symbol", ref)
      List.append(&2, B.Item, Pin.body(Pin.shape(S.Sx.arg(x, 1n)), p, e, d, 0.1524, pk, sig, info),
        Pin.texts(PinT{name, num, Tx.of(S.Sx.get(x, "name")), Tx.of(S.Sx.get(x, "number")), off, names, nums},
          e, Pt.add(p, Pt.scale(d, F32.div(len, 2.0))), d, pk, sig, info))

def Pin.of(+x: S.Sx, +t: Xf, +lib: Lib, +key: String, +sig: U32, +ref: String) -> List<&2, B.Item>:
  Pin.show(Sch.flag(x, "hide"), x, t, lib, key, sig, ref)

# Symbols
# -------

def Sym.item.k(pin: Bool, +x: S.Sx, +t: Xf, +lib: Lib, +key: String, +sig: U32, +i: Nat, +ref: String, +info: String) -> List<&2, B.Item>:
  match pin:
    case True{}:
      Pin.of(x, t, lib, key, sig, ref)
    case False{}:
      Sch.shape(x, t, key ++ "/g" ++ Nat.show(i), sig, L.body(), L.body_fill(), info)

def Sym.items(xs: List<&2, S.Sx>, +t: Xf, +lib: Lib, +key: String, +sig: U32, +i: Nat, +ref: String, +info: String) -> List<&2, B.Item>:
  match xs:
    case Nil{}:
      Nil{}
    case +x <> r:
      List.append(&2, B.Item, Sym.item.k(String.eq(S.Sx.name(x), "pin"), x, t, lib, key, sig, i, ref, info),
        Sym.items(r, t, lib, key, sig, 1n+i, ref, info))

def Sym.wants(+v: U32, +want: U32) -> Bool:
  Bool.or(U32.is_eq(v, 0), U32.is_eq(v, want))

def Sym.part.k(on: Bool, xs: List<&2, S.Sx>, +t: Xf, +lib: Lib, +key: String, +sig: U32, +i: Nat, +ref: String, +info: String) -> List<&2, B.Item>:
  match on:
    case True{}:
      Sym.items(xs, t, lib, key, sig, i, ref, info)
    case False{}:
      Nil{}

# the sections of the chosen unit and body style (and the shared ones)
def Sym.parts(ps: List<&2, Part>, +unit: U32, +style: U32, +t: Xf, +lib: Lib, +key: String, +sig: U32, +i: Nat, +ref: String, +info: String) -> List<&2, B.Item>:
  match ps:
    case Nil{}:
      Nil{}
    case Part{+u, +s, xs} <> r:
      List.append(&2, B.Item, Sym.part.k(Bool.and(Sym.wants(u, unit), Sym.wants(s, style)), xs, t, lib, key, sig, Nat.mul(i, 1000n), ref, info),
        Sym.parts(r, unit, style, t, lib, key, sig, 1n+i, ref, info))

def Sch.lib.parts(l: Lib) -> List<&2, Part>:
  match l:
    case Lib{_, _, _, _, _, ps}:
      ps

def Sym.body(m: Maybe<&2, Lib>, +unit: U32, +style: U32, +t: Xf, +key: String, +sig: U32, +ref: String, +info: String) -> List<&2, B.Item>:
  match m:
    case Some{+lib}:
      Sym.parts(Sch.lib.parts(lib), unit, style, t, lib, key, sig, 1n, ref, info)
    case None{}:
      Nil{}

def Sch.lib.units(m: Maybe<&2, Lib>) -> U32:
  match m:
    case Some{Lib{_, u, _, _, _, _}}:
      u
    case None{}:
      1

# "U1" + "A" for unit 1 of a multi-unit symbol
def Sym.suffix(+units: U32, +unit: U32) -> String:
  Bool.pick(String, U32.is_gt(units, 1), SCon{Chr{(64 + unit : U32)}, SNil{}}, "")

def Sch.tx.hj(t: Tx) -> U32:
  match t:
    case Tx{_, _, _, hj, _, _, _}:
      hj

def Sch.tx.vj(t: Tx) -> U32:
  match t:
    case Tx{_, _, _, _, vj, _, _}:
      vj

# Fields
# ------

# A symbol field's (at) is on the sheet, but KiCad lays it out in the
# symbol's own frame: the text box around its anchor there, turned with the
# symbol, and the text centred in that box, horizontal or vertical.
def Fld.centre(+t: Xf, +p: B.Pt, +at: S.Sx, +tw: F32, +tx: Tx) -> B.Pt:
  +l = Xf.unturn(t, F32.sub(S.Sx.f(at, 0n), Pt.x(p)), F32.sub(S.Sx.f(at, 1n), Pt.y(p)))
  +cu = F32.mul(tw, F32.mul(F32.sub(1.0, U32.to_f32(Sch.tx.hj(tx))), 0.5))
  +cv = F32.mul(Tx.h(tx), F32.mul(F32.sub(1.0, U32.to_f32(Sch.tx.vj(tx))), 0.5))
  +v = Sch.vertical(S.Sx.f(at, 2n))
  +c = Pt.add(l, Bool.pick(B.Pt, v, B.Pt{cv, F32.neg(cu)}, B.Pt{cu, cv}))
  Pt.add(p, Xf.turn(t, Pt.x(c), Pt.y(c)))

def Fld.show(+x: S.Sx, +t: Xf, +text: String, +key: String, +sig: U32, +layer: U32, +info: String, rest: List<&2, B.Item>) -> List<&2, B.Item>:
  +at = S.Sx.get(x, "at")
  +tx = Tx.of(x)
  +c = Fld.centre(t, Xf.at(t), at, ST.Stroke.width(text, Tx.w(tx)), tx)
  Sch.text.items(text, Pt.x(c), Pt.y(c), Sch.readable(Bool.xor(Sch.vertical(S.Sx.f(at, 2n)), Xf.quarter(t))), Tx.just(tx, 1, 1),
    key ++ "/f/" ++ S.Sx.arg(x, 0n), sig, layer, info, rest)

# "Name: value" when the field asks to show its name
def Fld.named(+x: S.Sx, +v: String) -> String:
  Bool.pick(String, Sch.flag(x, "show_name"), S.Sx.arg(x, 0n) ++ ": " ++ v, v)

def Fld.main(+n: String) -> Bool:
  Bool.or(String.eq(n, "Reference"), String.eq(n, "Value"))

def Fld.k(prop: Bool, +x: S.Sx, +t: Xf, +suffix: String, +key: String, +sig: U32, +info: String, rest: List<&2, B.Item>) -> List<&2, B.Item>:
  match prop:
    case True{}:
      +n = S.Sx.arg(x, 0n)
      Fld.show(x, t, Fld.named(x, S.Sx.arg(x, 1n) ++ Bool.pick(String, String.eq(n, "Reference"), suffix, "")), key, sig,
        Bool.pick(U32, Fld.main(n), L.field(), L.field_other()), info, rest)
    case False{}:
      rest

# a symbol's visible fields
def Fld.all(xs: List<&2, S.Sx>, +t: Xf, +suffix: String, +key: String, +sig: U32, +info: String) -> List<&2, B.Item>:
  match xs:
    case Nil{}:
      Nil{}
    case +x <> r:
      Fld.k(String.eq(S.Sx.name(x), "property"), x, t, suffix, key, sig, info, Fld.all(r, t, suffix, key, sig, info))

def Sym.mirror(+m: String) -> B.Pt:
  Bool.pick(B.Pt, String.eq(m, "x"), B.Pt{1.0, F32.neg(1.0)}, Bool.pick(B.Pt, String.eq(m, "y"), B.Pt{F32.neg(1.0), 1.0}, B.Pt{1.0, 1.0}))

def Sym.xf(+at: S.Sx, m: B.Pt) -> Xf:
  match m:
    case B.Pt{mx, my}:
      Xf{S.Sx.f(at, 0n), S.Sx.f(at, 1n), B.Geo.rad(S.Sx.f(at, 2n)), mx, my, F32.neg(1.0)}

def Sym.num(+x: S.Sx, +n: String) -> U32:
  F32.to_u32(Sch.or.f(S.Sx.f(S.Sx.get(x, n), 0n), 1.0))

# Do-not-populate marks
# ---------------------

type Bb is Data:
  Bb{x0: F32, y0: F32, x1: F32, y1: F32}

def Bb.pt(p: B.Pt, +r: F32, b: Bb) -> Bb:
  match p b:
    case B.Pt{+x, +y} Bb{+x0, +y0, +x1, +y1}:
      Bb{F32.min(x0, F32.sub(x, r)), F32.min(y0, F32.sub(y, r)), F32.max(x1, F32.add(x, r)), F32.max(y1, F32.add(y, r))}

def Bb.pts(ps: List<&2, B.Pt>, b: Bb) -> Bb:
  match ps:
    case Nil{}:
      b
    case p <> t:
      Bb.pts(t, Bb.pt(p, 0.0, b))

def Bb.prim(p: B.Prim, b: Bb) -> Bb:
  match p:
    case B.PSeg{a, e, _}:
      Bb.pt(e, 0.0, Bb.pt(a, 0.0, b))
    case B.PLine{ps, _}:
      Bb.pts(ps, b)
    case B.PCirc{c, r, _, _}:
      Bb.pt(c, r, b)
    case B.PPoly{ps, _, _}:
      Bb.pts(ps, b)
    case B.PTint{ps, _, _}:
      Bb.pts(ps, b)

# the extent of a symbol's drawing and pins (not their texts)
def Bb.items(xs: List<&2, B.Item>, +b: Bb) -> Bb:
  match xs:
    case Nil{}:
      b
    case B.Item{_, _, +l, +p, _} <> t:
      Bb.items(t, Bool.pick(Bb, Bool.or(U32.is_eq(l, L.body()), U32.is_eq(l, L.pin())), Bb.prim(p, b), b))

def Dnp.cross(b: Bb, +key: String, +sig: U32, +info: String) -> List<&2, B.Item>:
  match b:
    case Bb{+x0, +y0, +x1, +y1}:
      Bool.pick(List<&2, B.Item>, F32.is_gt(x1, x0),
        [B.Item{key ++ "/dnp", sig, L.dnp(), B.PSeg{B.Pt{x0, y0}, B.Pt{x1, y1}, 0.3}, info},
         B.Item{key ++ "/dnp2", sig, L.dnp(), B.PSeg{B.Pt{x0, y1}, B.Pt{x1, y0}, 0.3}, info}], Nil{})

# a do-not-populate symbol is crossed out
def Dnp.k(dnp: Bool, +body: List<&2, B.Item>, +key: String, +sig: U32, +info: String, rest: List<&2, B.Item>) -> List<&2, B.Item>:
  match dnp:
    case True{}:
      List.append(&2, B.Item, body, List.append(&2, B.Item,
        Dnp.cross(Bb.items(body, Bb{1000000.0, 1000000.0, F32.neg(1000000.0), F32.neg(1000000.0)}), key, sig, info), rest))
    case False{}:
      List.append(&2, B.Item, body, rest)

# KiCad 10.99 may give each cached definition its own name while lib_id
# still names the source library. An explicit lib_name is authoritative;
# old files without it use lib_id for the embedded cache as before.
def Sch.cache.name(+name: String, id: String) -> String:
  Bool.pick(String, String.is_empty(name), id, name)

def Sch.cache.id(+x: S.Sx) -> String:
  Sch.cache.name(S.Sx.attr(x, "lib_name"), S.Sx.attr(x, "lib_id"))

# a placed symbol: its unit's drawing and pins, and its fields
def Sch.symbol(+x: S.Sx, +key: String, +sig: U32, +lib: List<&2, Lib>) -> List<&2, B.Item>:
  +id = S.Sx.attr(x, "lib_id")
  +m = Lib.find(lib, Sch.cache.id(x))
  +t = Sym.xf(S.Sx.get(x, "at"), Sym.mirror(S.Sx.attr(x, "mirror")))
  +unit = Sym.num(x, "unit")
  +style = F32.to_u32(Sch.or.f(S.Sx.f(S.Sx.get(x, "body_style"), 0n), Sch.or.f(S.Sx.f(S.Sx.get(x, "convert"), 0n), 1.0)))
  +ref = B.Info.prop(x, "Reference") ++ Sym.suffix(Sch.lib.units(m), unit)
  +info = "symbol" ++ B.Info.kv("ref", ref) ++ B.Info.kv("value", B.Info.prop(x, "Value")) ++ B.Info.kv("lib", id)
    ++ B.Info.kv("footprint", B.Info.prop(x, "Footprint")) ++ Bool.pick(String, Sch.flag(x, "dnp"), ";dnp=yes", "")
  Dnp.k(Sch.flag(x, "dnp"), Sym.body(m, unit, style, t, key, sig, ref, info), key, sig, info,
    Fld.all(S.Sx.args(x), t, Sym.suffix(Sch.lib.units(m), unit), key, sig, info))

# Wires, junctions, labels
# ------------------------

def Sch.wire(+x: S.Sx, +key: String, +sig: U32, +layer: U32, +w: F32, +info: String) -> List<&2, B.Item>:
  [B.Item{key, sig, layer, B.PLine{Sch.pts(S.Sx.args(S.Sx.get(x, "pts")), Xf.home()), Sch.width(x, w)}, info}]

def Sch.entry(+x: S.Sx, +key: String, +sig: U32) -> List<&2, B.Item>:
  +a = Sch.pt(x, "at")
  [B.Item{key, sig, L.bus_entry(), B.PSeg{a, Pt.add(a, Sch.pt(x, "size")), Sch.width(x, 0.1524)}, "bus entry"}]

def Sch.junction(+x: S.Sx, +key: String, +sig: U32) -> List<&2, B.Item>:
  [B.Item{key, sig, L.junction(), B.PCirc{Sch.pt(x, "at"), F32.div(Sch.or.f(S.Sx.f(S.Sx.get(x, "diameter"), 0n), 1.016), 2.0), 0.0, True{}}, "junction"}]

def Sch.nc.k(p: B.Pt, +key: String, +sig: U32) -> List<&2, B.Item>:
  match p:
    case B.Pt{+x, +y}:
      +d = {0.635 : F32}
      [B.Item{key, sig, L.nc(), B.PSeg{B.Pt{F32.sub(x, d), F32.sub(y, d)}, B.Pt{F32.add(x, d), F32.add(y, d)}, 0.1524}, "no connect"},
       B.Item{key ++ "/2", sig, L.nc(), B.PSeg{B.Pt{F32.sub(x, d), F32.add(y, d)}, B.Pt{F32.add(x, d), F32.sub(y, d)}, 0.1524}, "no connect"}]

def Sch.nc(+x: S.Sx, +key: String, +sig: U32) -> List<&2, B.Item>:
  Sch.nc.k(Sch.pt(x, "at"), key, sig)

# a point in a label's frame: u along the label, away from its anchor, and
# v across it
def Lbl.pt(+x: F32, +y: F32, +a: F32, p: B.Pt) -> B.Pt:
  match p:
    case B.Pt{+u, +v}:
      B.Pt{F32.add(x, F32.add(F32.mul(u, F32.cos(a)), F32.mul(v, F32.sin(a)))), F32.sub(F32.add(y, F32.mul(v, F32.cos(a))), F32.mul(u, F32.sin(a)))}

def Lbl.pts(ps: List<&2, B.Pt>, +x: F32, +y: F32, +a: F32) -> List<&2, B.Pt>:
  match ps:
    case Nil{}:
      Nil{}
    case p <> t:
      Lbl.pt(x, y, a, p) <> Lbl.pts(t, x, y, a)

# input 1, output 2, bidirectional or tri-state 3, else 0
def Lbl.shape(+s: String) -> Nat:
  Bool.pick(Nat, String.eq(s, "input"), 1n, Bool.pick(Nat, String.eq(s, "output"), 2n,
    Bool.pick(Nat, Bool.or(String.eq(s, "bidirectional"), String.eq(s, "tri_state")), 3n, 0n)))

# a local label: its text just above the wire
def Sch.label(+x: S.Sx, +key: String, +sig: U32) -> List<&2, B.Item>:
  +at = S.Sx.get(x, "at")
  +tx = Tx.of(x)
  +v = Sch.vertical(S.Sx.f(at, 2n))
  +off = F32.add(F32.mul(0.15, Tx.h(tx)), Tx.pen.of(tx))
  Sch.text.items(S.Sx.arg(x, 0n), F32.sub(S.Sx.f(at, 0n), Bool.pick(F32, v, off, 0.0)), F32.sub(S.Sx.f(at, 1n), Bool.pick(F32, v, 0.0, off)),
    Sch.readable(v), tx, key, sig, L.label(), "label" ++ B.Info.kv("text", S.Sx.arg(x, 0n)), Nil{})

# a label's text, `u` along from its anchor, centred across; it reads away
# from the anchor
def Lbl.text(+s: String, +x: F32, +y: F32, +deg: F32, +u: F32, +tx: Tx, +key: String, +sig: U32, +layer: U32, +info: String, rest: List<&2, B.Item>) -> List<&2, B.Item>:
  +p = Lbl.pt(x, y, B.Geo.rad(deg), B.Pt{u, 0.0})
  Sch.text.items(s, Pt.x(p), Pt.y(p), Sch.readable(Sch.vertical(deg)), Tx.just(tx, Bool.pick(U32, F32.is_lt(Sch.deg(deg), 135.0), 0, 2), 1),
    key, sig, layer, info, rest)

def Glb.outline.k(k: Nat, +x: F32, +y: F32, +hs: F32, +ny: F32) -> List<&2, B.Pt>:
  match k:
    case 1n:
      [B.Pt{0.0, 0.0}, B.Pt{hs, ny}, B.Pt{F32.add(x, hs), ny}, B.Pt{F32.add(x, hs), 0.0}, B.Pt{F32.add(x, hs), y}, B.Pt{hs, y}]
    case 2n:
      [B.Pt{0.0, 0.0}, B.Pt{0.0, ny}, B.Pt{x, ny}, B.Pt{F32.add(x, hs), 0.0}, B.Pt{x, y}, B.Pt{0.0, y}]
    case 3n:
      [B.Pt{0.0, 0.0}, B.Pt{hs, ny}, B.Pt{F32.add(x, hs), ny}, B.Pt{F32.add(x, F32.mul(2.0, hs)), 0.0}, B.Pt{F32.add(x, hs), y}, B.Pt{hs, y}]
    case _:
      [B.Pt{0.0, 0.0}, B.Pt{0.0, ny}, B.Pt{x, ny}, B.Pt{x, 0.0}, B.Pt{x, y}, B.Pt{0.0, y}]

def Glb.outline(+k: Nat, +x: F32, +y: F32, +hs: F32) -> List<&2, B.Pt>:
  Glb.outline.k(k, x, y, hs, F32.neg(y))

def Glb.arrow(+k: Nat) -> Bool:
  Bool.or(Nat.is_eq(k, 1n), Nat.is_eq(k, 3n))

# a global label: its text in an outline shaped by its direction
def Sch.global(+x: S.Sx, +key: String, +sig: U32) -> List<&2, B.Item>:
  +at = S.Sx.get(x, "at")
  +ax = S.Sx.f(at, 0n)
  +ay = S.Sx.f(at, 1n)
  +deg = S.Sx.f(at, 2n)
  +tx = Tx.of(x)
  +s = S.Sx.arg(x, 0n)
  +k = Lbl.shape(S.Sx.attr(x, "shape"))
  +h = Tx.h(tx)
  +margin = F32.mul(0.375, h)
  +hs = F32.add(F32.div(h, 2.0), margin)
  +pen = {0.1524 : F32}
  +info = "global label" ++ B.Info.kv("text", s) ++ B.Info.kv("shape", S.Sx.attr(x, "shape"))
  B.Item{key, sig, L.global(), B.PPoly{Lbl.pts(Glb.outline(k, F32.add(ST.Stroke.width(s, Tx.w(tx)), F32.add(F32.mul(2.0, margin), pen)), F32.add(hs, pen), hs), ax, ay, B.Geo.rad(deg)), pen, False{}}, info}
  <> Lbl.text(s, ax, ay, deg, F32.add(Bool.pick(F32, Glb.arrow(k), hs, 0.0), F32.add(margin, pen)), tx, key ++ "/t", sig, L.global(), info, Nil{})

def Hier.outline.k(k: Nat, +s: F32, +ns: F32, +s2: F32) -> List<&2, B.Pt>:
  match k:
    case 1n:
      [B.Pt{0.0, 0.0}, B.Pt{s, ns}, B.Pt{s2, ns}, B.Pt{s2, s}, B.Pt{s, s}]
    case 2n:
      [B.Pt{0.0, ns}, B.Pt{s, ns}, B.Pt{s2, 0.0}, B.Pt{s, s}, B.Pt{0.0, s}]
    case 3n:
      [B.Pt{0.0, 0.0}, B.Pt{s, ns}, B.Pt{s2, 0.0}, B.Pt{s, s}]
    case _:
      [B.Pt{0.0, ns}, B.Pt{s2, ns}, B.Pt{s2, s}, B.Pt{0.0, s}]

def Hier.outline(+k: Nat, +s: F32) -> List<&2, B.Pt>:
  Hier.outline.k(k, s, F32.neg(s), F32.mul(2.0, s))

# a hierarchical label (or sheet pin): a small flag at the anchor, then
# the text; deg is the direction the text reads away in
def Hier.draw(+s: String, +ax: F32, +ay: F32, +deg: F32, +k: Nat, +tx: Tx, +key: String, +sig: U32, +layer: U32, +info: String) -> List<&2, B.Item>:
  +h = Tx.h(tx)
  B.Item{key, sig, layer, B.PPoly{Lbl.pts(Hier.outline(k, F32.div(h, 2.0)), ax, ay, B.Geo.rad(deg)), 0.1524, False{}}, info}
  <> Lbl.text(s, ax, ay, deg, F32.add(h, F32.mul(0.3, h)), tx, key ++ "/t", sig, layer, info, Nil{})

def Sch.hier(+x: S.Sx, +key: String, +sig: U32) -> List<&2, B.Item>:
  +at = S.Sx.get(x, "at")
  Hier.draw(S.Sx.arg(x, 0n), S.Sx.f(at, 0n), S.Sx.f(at, 1n), S.Sx.f(at, 2n), Lbl.shape(S.Sx.attr(x, "shape")), Tx.of(x), key, sig, L.hier(),
    "hier label" ++ B.Info.kv("text", S.Sx.arg(x, 0n)) ++ B.Info.kv("shape", S.Sx.attr(x, "shape")))

# Notes
# -----

def Sch.note(+x: S.Sx, +key: String, +sig: U32) -> List<&2, B.Item>:
  +at = S.Sx.get(x, "at")
  Sch.text.items(S.Sx.arg(x, 0n), S.Sx.f(at, 0n), S.Sx.f(at, 1n), Sch.readable(Sch.vertical(S.Sx.f(at, 2n))), Tx.of(x), key, sig, L.note(), "text", Nil{})

def Box.margin(+x: S.Sx, +i: Nat, +d: F32) -> F32:
  Bool.pick(F32, Sch.has(x, "margins"), S.Sx.f(S.Sx.get(x, "margins"), i), d)

def Box.pick(+j: U32, +lo: F32, +mid: F32, +hi: F32) -> F32:
  Bool.pick(F32, U32.is_eq(j, 0), lo, Bool.pick(F32, U32.is_eq(j, 1), mid, hi))

# a text box: its border and its text, wrapped to fit
def Sch.textbox(+x: S.Sx, +key: String, +sig: U32) -> List<&2, B.Item>:
  +a = Sch.pt(x, "at")
  +sz = Sch.pt(x, "size")
  +b = Pt.add(a, sz)
  +tx = Tx.of(x)
  +d = F32.mul(0.8, Tx.h(tx))
  +x0 = F32.add(Pt.x(a), Box.margin(x, 0n, d))
  +y0 = F32.add(Pt.y(a), Box.margin(x, 1n, d))
  +x1 = F32.sub(Pt.x(b), Box.margin(x, 2n, d))
  +y1 = F32.sub(Pt.y(b), Box.margin(x, 3n, d))
  +w = Sch.width(x, 0.1524)
  List.append(&2, B.Item, Tint.items(Tint.on(x), Sch.rect(a, b), x, key, sig, "text box"),
  List.append(&2, B.Item, Bool.pick(List<&2, B.Item>, F32.is_lt(S.Sx.f(S.Sx.get(S.Sx.get(x, "stroke"), "width"), 0n), 0.0), Nil{},
    [B.Item{key, sig, L.note(), B.PPoly{Sch.rect(a, b), w, False{}}, "text box"}]),
    Sch.text.items(ST.Stroke.wrap(S.Sx.arg(x, 0n), Tx.w(tx), F32.sub(x1, x0)), Box.pick(Sch.tx.hj(tx), x0, F32.div(F32.add(x0, x1), 2.0), x1),
      Box.pick(Sch.tx.vj(tx), y0, F32.div(F32.add(y0, y1), 2.0), y1), 0.0, tx, key ++ "/t", sig, L.note(), "text box", Nil{})))

# Sheets
# ------

def Sheet.prop.k(+x: S.Sx, is_name: Bool, is_file: Bool, +key: String, +sig: U32, +info: String, +at: S.Sx, +v: String) -> List<&2, B.Item>:
  match is_name is_file:
    case True{} _:
      Sch.text.items(v, S.Sx.f(at, 0n), S.Sx.f(at, 1n), Sch.readable(Sch.vertical(S.Sx.f(at, 2n))), Tx.of(x), key ++ "/name", sig, L.sheet_name(), info, Nil{})
    case False{} True{}:
      Sch.text.items("File: " ++ v, S.Sx.f(at, 0n), S.Sx.f(at, 1n), Sch.readable(Sch.vertical(S.Sx.f(at, 2n))), Tx.of(x), key ++ "/file", sig, L.sheet_file(), info, Nil{})
    case False{} False{}:
      Sch.text.items(v, S.Sx.f(at, 0n), S.Sx.f(at, 1n), Sch.readable(Sch.vertical(S.Sx.f(at, 2n))), Tx.of(x), key ++ "/f/" ++ S.Sx.arg(x, 0n), sig, L.field_other(), info, Nil{})

# sheet pins flag the other way round from hierarchical labels
def Sheet.pin.shape(+k: Nat) -> Nat:
  Bool.pick(Nat, Nat.is_eq(k, 1n), 2n, Bool.pick(Nat, Nat.is_eq(k, 2n), 1n, k))

def Sheet.child(+x: S.Sx, +key: String, +sig: U32, +info: String, +sname: String) -> List<&2, B.Item>:
  +n = S.Sx.name(x)
  +at = S.Sx.get(x, "at")
  +pn = S.Sx.arg(x, 0n)
  Bool.pick(List<&2, B.Item>, String.eq(n, "property"),
    Sheet.prop.k(x, Bool.or(String.eq(S.Sx.arg(x, 0n), "Sheetname"), String.eq(S.Sx.arg(x, 0n), "Sheet name")),
      Bool.or(String.eq(S.Sx.arg(x, 0n), "Sheetfile"), String.eq(S.Sx.arg(x, 0n), "Sheet file")), key, sig, info, at, S.Sx.arg(x, 1n)),
    Bool.pick(List<&2, B.Item>, String.eq(n, "pin"),
      Hier.draw(pn, S.Sx.f(at, 0n), S.Sx.f(at, 1n), F32.add(S.Sx.f(at, 2n), 180.0), Sheet.pin.shape(Lbl.shape(S.Sx.arg(x, 1n))), Tx.of(x),
        key ++ "/pin/" ++ pn, sig, L.sheet_pin(), "sheet pin" ++ B.Info.kv("name", pn) ++ B.Info.kv("shape", S.Sx.arg(x, 1n)) ++ B.Info.kv("sheet", sname)),
      Nil{}))

def Sheet.children(xs: List<&2, S.Sx>, +key: String, +sig: U32, +info: String, +sname: String) -> List<&2, B.Item>:
  match xs:
    case Nil{}:
      Nil{}
    case x <> t:
      List.append(&2, B.Item, Sheet.child(x, key, sig, info, sname), Sheet.children(t, key, sig, info, sname))

def Sheet.prop(+x: S.Sx, +a: String, +b: String) -> String:
  B.Info.kv.k(B.Info.prop(x, a), B.Info.prop(x, b))

# a hierarchical sheet: its box, name and file, and its pins
def Sch.sheet(+x: S.Sx, +key: String, +sig: U32) -> List<&2, B.Item>:
  +a = Sch.pt(x, "at")
  +ps = Sch.rect(a, Pt.add(a, Sch.pt(x, "size")))
  +sname = Sheet.prop(x, "Sheetname", "Sheet name")
  +info = "sheet" ++ B.Info.kv("name", sname) ++ B.Info.kv("file", Sheet.prop(x, "Sheetfile", "Sheet file"))
  B.Item{key ++ "/f", sig, L.sheet_fill(), B.PPoly{ps, 0.0, True{}}, info}
  <> B.Item{key, sig, L.sheet(), B.PPoly{ps, Sch.width(x, 0.1524), False{}}, info}
  <> Sheet.children(S.Sx.args(x), key, sig, info, sname)

# Directive labels
# ----------------

def Prop.k(prop: Bool, +x: S.Sx, +key: String, +sig: U32, +layer: U32, +info: String, rest: List<&2, B.Item>) -> List<&2, B.Item>:
  match prop:
    case True{}:
      +at = S.Sx.get(x, "at")
      +v = S.Sx.arg(x, 1n)
      Sch.text.items(Bool.pick(String, String.starts_with(v, "${"), "", Fld.named(x, v)), S.Sx.f(at, 0n), S.Sx.f(at, 1n), Sch.readable(Sch.vertical(S.Sx.f(at, 2n))),
        Tx.of(x), key ++ "/f/" ++ S.Sx.arg(x, 0n), sig, layer, info, rest)
    case False{}:
      rest

# a form's visible (property ..) fields, placed as written (unexpanded
# ${..} variables are left out)
def Prop.all(xs: List<&2, S.Sx>, +key: String, +sig: U32, +layer: U32, +info: String) -> List<&2, B.Item>:
  match xs:
    case Nil{}:
      Nil{}
    case +x <> t:
      Prop.k(String.eq(S.Sx.name(x), "property"), x, key, sig, layer, info, Prop.all(t, key, sig, layer, info))

def Flag.head(k: Nat, +ax: F32, +ay: F32, +a: F32, +len: F32, +r: F32) -> B.Prim:
  match k:
    case 1n:
      B.PCirc{Lbl.pt(ax, ay, a, B.Pt{F32.add(len, r), 0.0}), r, 0.0, True{}}
    case 2n:
      B.PPoly{Lbl.pts([B.Pt{len, 0.0}, B.Pt{F32.add(len, r), F32.neg(r)}, B.Pt{F32.add(len, F32.mul(2.0, r)), 0.0}, B.Pt{F32.add(len, r), r}], ax, ay, a), 0.1524, False{}}
    case 3n:
      B.PPoly{Lbl.pts([B.Pt{len, F32.neg(r)}, B.Pt{F32.add(len, F32.mul(2.0, r)), F32.neg(r)}, B.Pt{F32.add(len, F32.mul(2.0, r)), r}, B.Pt{len, r}], ax, ay, a), 0.1524, False{}}
    case _:
      B.PCirc{Lbl.pt(ax, ay, a, B.Pt{F32.add(len, r), 0.0}), r, 0.1524, False{}}

def Flag.shape(+s: String) -> Nat:
  Bool.pick(Nat, String.eq(s, "dot"), 1n, Bool.pick(Nat, String.eq(s, "diamond"), 2n, Bool.pick(Nat, String.eq(s, "rectangle"), 3n, 0n)))

# a directive (net class) flag: a stem up from its anchor (at angle 0), a
# small head, and its fields
def Sch.netflag(+x: S.Sx, +key: String, +sig: U32) -> List<&2, B.Item>:
  +at = S.Sx.get(x, "at")
  +ax = S.Sx.f(at, 0n)
  +ay = S.Sx.f(at, 1n)
  +a = B.Geo.rad(F32.add(S.Sx.f(at, 2n), 90.0))
  +len = Sch.or.f(S.Sx.f(S.Sx.get(x, "length"), 0n), 2.54)
  +info = "netclass flag" ++ B.Info.kv("netclass", B.Info.prop(x, "Netclass"))
  B.Item{key, sig, L.label(), B.PSeg{B.Pt{ax, ay}, Lbl.pt(ax, ay, a, B.Pt{len, 0.0}), 0.1524}, info}
  <> B.Item{key ++ "/h", sig, L.label(), Flag.head(Flag.shape(S.Sx.attr(x, "shape")), ax, ay, a, len, F32.mul(0.4, Tx.h(Tx.of(x)))), info}
  <> Prop.all(S.Sx.args(x), key, sig, L.field_other(), info)

# Page
# ----

# a paper size in mm (landscape unless marked portrait)
def Page.size.k(+w: F32, +h: F32, +portrait: Bool) -> B.Pt:
  Bool.pick(B.Pt, portrait, B.Pt{h, w}, B.Pt{w, h})

def Page.named(+n: String) -> B.Pt:
  Bool.pick(B.Pt, String.eq(n, "A5"), B.Pt{210.0, 148.0}, Bool.pick(B.Pt, String.eq(n, "A3"), B.Pt{420.0, 297.0},
  Bool.pick(B.Pt, String.eq(n, "A2"), B.Pt{594.0, 420.0}, Bool.pick(B.Pt, String.eq(n, "A1"), B.Pt{841.0, 594.0},
  Bool.pick(B.Pt, String.eq(n, "A0"), B.Pt{1189.0, 841.0}, Bool.pick(B.Pt, Bool.or(String.eq(n, "A"), String.eq(n, "USLetter")), B.Pt{279.4, 215.9},
  Bool.pick(B.Pt, Bool.or(String.eq(n, "B"), String.eq(n, "USLedger")), B.Pt{431.8, 279.4}, Bool.pick(B.Pt, String.eq(n, "USLegal"), B.Pt{355.6, 215.9},
  Bool.pick(B.Pt, String.eq(n, "C"), B.Pt{558.8, 431.8}, Bool.pick(B.Pt, String.eq(n, "D"), B.Pt{863.6, 558.8},
  Bool.pick(B.Pt, String.eq(n, "E"), B.Pt{1117.6, 863.6}, B.Pt{297.0, 210.0})))))))))))

def Page.size.p(p: B.Pt, +portrait: Bool) -> B.Pt:
  match p:
    case B.Pt{w, h}:
      Page.size.k(w, h, portrait)

# the page size of a (paper ..) form
def Page.size(+x: S.Sx) -> B.Pt:
  Bool.pick(B.Pt, String.eq(S.Sx.arg(x, 0n), "User"), B.Pt{S.Sx.f(x, 1n), S.Sx.f(x, 2n)},
    Page.size.p(Page.named(S.Sx.arg(x, 0n)), Sch.atom(S.Sx.args(x), "portrait")))

def Page.box(+x0: F32, +y0: F32, +x1: F32, +y1: F32) -> B.Prim:
  B.PPoly{[B.Pt{x0, y0}, B.Pt{x1, y0}, B.Pt{x1, y1}, B.Pt{x0, y1}], 0.1524, False{}}

def Page.seg(+x0: F32, +y0: F32, +x1: F32, +y1: F32) -> B.Prim:
  B.PSeg{B.Pt{x0, y0}, B.Pt{x1, y1}, 0.1524}

def Page.tx(+h: F32, +pen: F32, +hj: U32, +vj: U32) -> Tx:
  Tx{h, h, pen, hj, vj, False{}, False{}}

# grid references along the top and bottom (1, 2, ...) every 50 mm
def Page.cols(n: Nat, +i: Nat, +w: F32, +h: F32) -> List<&2, B.Prim>:
  match n:
    case 0n:
      Nil{}
    case 1n+m:
      +x = F32.add(10.0, F32.mul(50.0, F32.from_nat(1n+i)))
      +mx = F32.div(F32.add(F32.sub(x, 50.0), F32.min(x, F32.sub(w, 12.0))), 2.0)
      List.append(&2, B.Prim, Bool.pick(List<&2, B.Prim>, F32.is_lt(x, F32.sub(w, 12.0)),
        [Page.seg(x, 10.0, x, 12.0), Page.seg(x, F32.sub(h, 12.0), x, F32.sub(h, 10.0))], Nil{}),
      List.append(&2, B.Prim, Bool.pick(List<&2, B.Prim>, F32.is_lt(F32.sub(x, 50.0), F32.sub(w, 12.0)),
        List.append(&2, B.Prim, Sch.text(Nat.show(1n+i), mx, 11.0, 0.0, Page.tx(1.3, 0.1524, 1, 1)),
          Sch.text(Nat.show(1n+i), mx, F32.sub(h, 11.0), 0.0, Page.tx(1.3, 0.1524, 1, 1))), Nil{}),
        Page.cols(m, 1n+i, w, h)))

def Page.letter(+i: Nat) -> String:
  SCon{Chr{U32.from_nat(Nat.add(65n, i))}, SNil{}}

def Page.rows(n: Nat, +i: Nat, +w: F32, +h: F32) -> List<&2, B.Prim>:
  match n:
    case 0n:
      Nil{}
    case 1n+m:
      +y = F32.add(10.0, F32.mul(50.0, F32.from_nat(1n+i)))
      +my = F32.div(F32.add(F32.sub(y, 50.0), F32.min(y, F32.sub(h, 12.0))), 2.0)
      List.append(&2, B.Prim, Bool.pick(List<&2, B.Prim>, F32.is_lt(y, F32.sub(h, 12.0)),
        [Page.seg(10.0, y, 12.0, y), Page.seg(F32.sub(w, 12.0), y, F32.sub(w, 10.0), y)], Nil{}),
      List.append(&2, B.Prim, Bool.pick(List<&2, B.Prim>, F32.is_lt(F32.sub(y, 50.0), F32.sub(h, 12.0)),
        List.append(&2, B.Prim, Sch.text(Page.letter(i), 11.0, my, 0.0, Page.tx(1.3, 0.1524, 1, 1)),
          Sch.text(Page.letter(i), F32.sub(w, 11.0), my, 0.0, Page.tx(1.3, 0.1524, 1, 1))), Nil{}),
        Page.rows(m, 1n+i, w, h)))

def Page.comment.hit(+x: S.Sx, +n: String) -> Bool:
  Bool.and(String.eq(S.Sx.name(x), "comment"), String.eq(S.Sx.arg(x, 0n), n))

def Page.comment.go(xs: List<&2, S.Sx>, hit: Bool, +n: String) -> String:
  match xs hit:
    case +x <> _ True{}:
      S.Sx.arg(x, 1n)
    case _ <> +t False{}:
      Page.comment.go(t, Page.comment.hit(Maybe.default(&2, S.Sx, List.head(&2, S.Sx, t), S.XEnd{}), n), n)
    case Nil{} _:
      ""

def Page.comment(+tb: S.Sx, +n: String) -> String:
  Page.comment.go(S.Sx.args(tb), Page.comment.hit(Maybe.default(&2, S.Sx, List.head(&2, S.Sx, S.Sx.args(tb)), S.XEnd{}), n), n)

def Page.kv(+k: String, +v: String) -> String:
  Bool.pick(String, String.is_empty(v), "", k ++ v)

def Page.cat(xss: List<&2, List<&2, B.Prim>>) -> List<&2, B.Prim>:
  match xss:
    case Nil{}:
      Nil{}
    case xs <> t:
      List.append(&2, B.Prim, xs, Page.cat(t))

# the drawing sheet: frame, grid references and title block
def Page.prims(p: B.Pt, +tb: S.Sx, +name: String) -> List<&2, B.Prim>:
  match p:
    case B.Pt{+w, +h}:
      +rx = F32.sub(w, 12.0)
      +ry = F32.sub(h, 12.0)
      +bx = F32.sub(rx, 107.0)
      +small = Page.tx(1.5, 0.15, 0, 2)
      Page.cat([[Page.box(10.0, 10.0, F32.sub(w, 10.0), F32.sub(h, 10.0)), Page.box(12.0, 12.0, rx, ry),
       Page.box(F32.sub(rx, 108.0), F32.sub(ry, 32.0), rx, ry),
       Page.seg(F32.sub(rx, 108.0), F32.sub(ry, 16.5), rx, F32.sub(ry, 16.5)), Page.seg(F32.sub(rx, 108.0), F32.sub(ry, 10.5), rx, F32.sub(ry, 10.5)),
       Page.seg(F32.sub(rx, 108.0), F32.sub(ry, 6.5), rx, F32.sub(ry, 6.5)), Page.seg(F32.sub(rx, 108.0), F32.sub(ry, 3.5), rx, F32.sub(ry, 3.5)),
       Page.seg(F32.sub(rx, 24.0), F32.sub(ry, 6.5), F32.sub(rx, 24.0), ry), Page.seg(F32.sub(rx, 86.0), F32.sub(ry, 6.5), F32.sub(rx, 86.0), F32.sub(ry, 3.5))],
        Page.cols(40n, 0n, w, h),
        Page.rows(40n, 0n, w, h),
        Sch.text(Page.kv("Title: ", S.Sx.attr(tb, "title")), bx, F32.sub(ry, 7.7), 0.0, Tx{2.0, 2.0, 0.4, 0, 2, False{}, True{}}),
        Sch.text("Size: " ++ name, bx, F32.sub(ry, 4.15), 0.0, small),
        Sch.text(Page.kv("Date: ", S.Sx.attr(tb, "date")), F32.sub(rx, 85.0), F32.sub(ry, 4.15), 0.0, small),
        Sch.text(Page.kv("Rev: ", S.Sx.attr(tb, "rev")), F32.sub(rx, 22.0), F32.sub(ry, 4.15), 0.0, small),
        Sch.text(S.Sx.attr(tb, "company"), bx, F32.sub(ry, 17.25), 0.0, small),
        Sch.text(Page.comment(tb, "1"), bx, F32.sub(ry, 20.25), 0.0, small),
        Sch.text(Page.comment(tb, "2"), bx, F32.sub(ry, 23.25), 0.0, small),
        Sch.text(Page.comment(tb, "3"), bx, F32.sub(ry, 26.25), 0.0, small),
        Sch.text(Page.comment(tb, "4"), bx, F32.sub(ry, 29.25), 0.0, small)])

# Forms
# -----

def Sch.forms() -> List<&2, String>:
  ["wire", "bus", "bus_entry", "junction", "no_connect", "label", "global_label", "hierarchical_label", "text", "text_box",
   "polyline", "rectangle", "circle", "arc", "bezier", "symbol", "sheet", "netclass_flag"]

def Sch.stop(bs: List<&2, U32>) -> Bool:
  match bs:
    case +b <> _:
      Bool.or(U32.is_le(b, 32), Bool.or(U32.is_eq(b, 40), U32.is_eq(b, 41)))
    case Nil{}:
      True{}

def Sch.head.go(bs: List<&2, U32>, stop: Bool) -> String:
  match bs stop:
    case _ True{}:
      SNil{}
    case b <> +t False{}:
      SCon{Chr{b}, Sch.head.go(t, Sch.stop(t))}
    case Nil{} False{}:
      SNil{}

# a form's name, read from its bytes without parsing it
def Sch.head(bs: List<&2, U32>) -> String:
  match bs:
    case _ <> +t:
      Sch.head.go(t, Sch.stop(t))
    case Nil{}:
      ""

def Sch.form.k(i: Nat, +x: S.Sx, +key: String, +sig: U32, +lib: List<&2, Lib>) -> List<&2, B.Item>:
  match i:
    case 0n:
      Sch.wire(x, key, sig, L.wire(), 0.1524, "wire")
    case 1n:
      Sch.wire(x, key, sig, L.bus(), 0.3048, "bus")
    case 2n:
      Sch.entry(x, key, sig)
    case 3n:
      Sch.junction(x, key, sig)
    case 4n:
      Sch.nc(x, key, sig)
    case 5n:
      Sch.label(x, key, sig)
    case 6n:
      Sch.global(x, key, sig)
    case 7n:
      Sch.hier(x, key, sig)
    case 8n:
      Sch.note(x, key, sig)
    case 9n:
      Sch.textbox(x, key, sig)
    case 15n:
      Sch.symbol(x, key, sig, lib)
    case 16n:
      Sch.sheet(x, key, sig)
    case 17n:
      Sch.netflag(x, key, sig)
    case 99n:
      Nil{}
    case _:
      Sch.top(x, key, sig)

def Sch.form.go(skip: Bool, +i: Nat, +bs: List<&2, U32>, +n: Nat, +lib: List<&2, Lib>, +lsig: U32) -> List<&2, B.Item>:
  match skip:
    case True{}:
      Nil{}
    case False{}:
      +x = S.Sx.parse(bs)
      +h = B.Board.hash(bs, 2166136261)
      Sch.form.k(i, x, B.Board.key(x, "n" ++ Nat.show(n)), Bool.pick(U32, Nat.is_eq(i, 15n), U32.xor(h, lsig), h), lib)

# a form's items; forms the viewer does not draw are never parsed
def Sch.form(+i: Nat, +bs: List<&2, U32>, +n: Nat, +lib: List<&2, Lib>, +lsig: U32) -> List<&2, B.Item>:
  Sch.form.go(Nat.is_eq(i, 99n), i, bs, n, lib, lsig)

def Sch.one(+bs: List<&2, U32>, +n: Nat, +lib: List<&2, Lib>, +lsig: U32) -> List<&2, B.Item>:
  Sch.form(Sch.index(Sch.forms(), Sch.head(bs)), bs, n, lib, lsig)

def Sch.seq(fs: List<&2, B.Form>, +lib: List<&2, Lib>, +lsig: U32) -> List<&2, B.Item>:
  match fs:
    case Nil{}:
      Nil{}
    case B.Form{+i, bs} <> t:
      List.append(&2, B.Item, Sch.one(bs, i, lib, lsig), Sch.seq(t, lib, lsig))

# convert both hands at once, d levels deep (see Board.par)
def Sch.par(d: Nat, fs: List<&2, B.Form>, +lib: List<&2, Lib>, +lsig: U32) -> List<&2, B.Item>:
  match d:
    case 0n:
      Sch.seq(fs, lib, lsig)
    case 1n+e0:
      +e = e0
      B.Board.split(fs, a => b => (
        x y = Sch.par(e, a, lib, lsig) Sch.par(e, b, lib, lsig)
        List.append(&2, B.Item, x, y)))

def Sch.find.hit(fs: List<&2, List<&2, U32>>, +n: String) -> Bool:
  match fs:
    case f <> _:
      String.eq(Sch.head(f), n)
    case Nil{}:
      False{}

def Sch.find.go(fs: List<&2, List<&2, U32>>, hit: Bool, +n: String) -> List<&2, U32>:
  match fs hit:
    case f <> _ True{}:
      f
    case _ <> +t False{}:
      Sch.find.go(t, Sch.find.hit(t, n), n)
    case Nil{} _:
      Nil{}

# the bytes of the first top-level form called n (empty when none)
def Sch.find(+fs: List<&2, List<&2, U32>>, +n: String) -> List<&2, U32>:
  Sch.find.go(fs, Sch.find.hit(fs, n), n)

# the drawing sheet's items
def Sch.page(+paper: List<&2, U32>, +tb: List<&2, U32>) -> List<&2, B.Item>:
  +px = S.Sx.parse(paper)
  Sch.put(Page.prims(Page.size(px), S.Sx.parse(tb), Bool.pick(String, String.is_empty(S.Sx.arg(px, 0n)), "A4", S.Sx.arg(px, 0n))),
    "page", B.Board.hash(tb, B.Board.hash(paper, 2166136261)), L.page(), "page", 0n, Nil{})

def Sch.items.k(m: Maybe<&2, List<&2, List<&2, U32>>>) -> Maybe<&2, List<&2, B.Item>>:
  match m:
    case Some{+fs}:
      +lb = Sch.find(fs, "lib_symbols")
      Some{List.append(&2, B.Item, Sch.page(Sch.find(fs, "paper"), Sch.find(fs, "title_block")),
        Sch.par(8n, B.Board.number(fs, 0n), Lib.table(S.Sx.parse(lb)), B.Board.hash(lb, 2166136261)))}
    case None{}:
      None{}

# every item of a schematic file, or None while the file is half written
def Sch.items(bs: List<&2, U32>) -> Maybe<&2, List<&2, B.Item>>:
  Sch.items.k(S.Sx.forms(bs))
