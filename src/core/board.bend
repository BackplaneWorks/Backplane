import Base
import ./sexpr.bend as S

# A .kicad_pcb as drawable items. Each Item has a stable key (KiCad's
# uuid, or a footprint's uuid and its child's) and a signature: the hash of
# the source form it came from. The viewer always draws the newest items;
# the signature only decides what fades in, so an unchanged board never
# flickers and a hash collision could at worst skip one fade.

# Geometry
# --------

type Pt is Data:
  Pt{x: F32, y: F32}

type Prim is Data:
  PSeg{a: Pt, b: Pt, w: F32}
  PLine{pts: List<&2, Pt>, w: F32}
  PCirc{c: Pt, r: F32, w: F32, fill: Bool}
  PPoly{pts: List<&2, Pt>, w: F32, fill: Bool}
  # a filled polygon in its own colour (0xRRGGBB) and opacity (0-255): a
  # schematic's coloured box, drawn under the sheet at its layer's place
  PTint{pts: List<&2, Pt>, color: U32, alpha: U32}

# layer is an index into Layer.names
type Item is Data:
  Item{key: String, sig: U32, layer: U32, prim: Prim, info: String}

# Tints
# -----
# A coloured box's colour and opacity packed in 21 bits (5 bits a channel,
# 6 of opacity), never 0 (the opacity is at least one step): what a plot
# chunk of fills carries as its width, so boxes of one colour share a
# chunk and every viewer draws the same colour. A tint is kept quantized
# (Tint.of), so packing it again gives it back (law tint_code_again).

# a channel (0-255) to 5 bits and back, rounded both ways
def Tint.q5(+c: U32) -> U32:
  (((U32.and(c, 255) * 31 : U32) + 127 : U32) / 255 : U32)

def Tint.e5(+q: U32) -> U32:
  (((q * 255 : U32) + 15 : U32) / 31 : U32)

# an opacity (0-255) to 6 bits (at least 1) and back
def Tint.q6(+a: U32) -> U32:
  U32.max(U32.min((((a * 63 : U32) + 127 : U32) / 255 : U32), 63), 1)

def Tint.e6(+q: U32) -> U32:
  (((q * 255 : U32) + 31 : U32) / 63 : U32)

def Tint.code(+color: U32, +alpha: U32) -> U32:
  U32.or(U32.or(U32.shln(Tint.q5(U32.shrn(color, 16n)), 16n), U32.shln(Tint.q5(U32.shrn(color, 8n)), 11n)),
    U32.or(U32.shln(Tint.q5(color), 6n), Tint.q6(U32.min(alpha, 255))))

def Tint.color(+k: U32) -> U32:
  U32.or(U32.or(U32.shln(Tint.e5(U32.and(U32.shrn(k, 16n), 31)), 16n), U32.shln(Tint.e5(U32.and(U32.shrn(k, 11n), 31)), 8n)),
    Tint.e5(U32.and(U32.shrn(k, 6n), 31)))

def Tint.alpha(+k: U32) -> U32:
  Tint.e6(U32.and(k, 63))

# a filled polygon in a colour as viewers draw it
def Tint.of(ps: List<&2, Pt>, +color: U32, +alpha: U32) -> Prim:
  +k = Tint.code(color, alpha)
  PTint{ps, Tint.color(k), Tint.alpha(k)}

# a footprint's placement: origin and rotation (radians)
type Place is Data:
  Place{x: F32, y: F32, a: F32}

def Geo.rad(+deg: F32) -> F32:
  F32.mul(deg, F32.div(F32.pi(), 180.0))

def Geo.rot(p: Pt, +a: F32) -> Pt:
  match p:
    case Pt{+x, +y}:
      Pt{F32.add(F32.mul(x, F32.cos(a)), F32.mul(y, F32.sin(a))), F32.sub(F32.mul(y, F32.cos(a)), F32.mul(x, F32.sin(a)))}

def Geo.place.k(p: Pt, +px: F32, +py: F32) -> Pt:
  match p:
    case Pt{x, y}:
      Pt{F32.add(x, px), F32.add(y, py)}

# a footprint-local point in board coordinates
def Geo.place(+pl: Place, p: Pt) -> Pt:
  match pl:
    case Place{+px, +py, +a}:
      Geo.place.k(Geo.rot(p, a), px, py)

def Geo.pt(+x: S.Sx) -> Pt:
  Pt{S.Sx.f(x, 0n), S.Sx.f(x, 1n)}

def Geo.at(+x: S.Sx, +name: String) -> Pt:
  Geo.pt(S.Sx.get(x, name))

# Arcs
# ----

def Arc.norm(+x: F32) -> F32:
  +tau = F32.mul(2.0, F32.pi())
  F32.sub(x, F32.mul(tau, F32.floor(F32.div(x, tau))))

def Arc.sq(+x: F32) -> F32:
  F32.mul(x, x)

def Arc.at(+i: Nat, +ux: F32, +uy: F32, +r: F32, +a0: F32, +sw: F32) -> Pt:
  +a = F32.add(a0, F32.mul(sw, F32.div(F32.from_nat(i), 24.0)))
  Pt{F32.add(ux, F32.mul(r, F32.cos(a))), F32.add(uy, F32.mul(r, F32.sin(a)))}

def Arc.steps(n: Nat, +i: Nat, +ux: F32, +uy: F32, +r: F32, +a0: F32, +sw: F32) -> List<&2, Pt>:
  match n:
    case 0n:
      [Arc.at(i, ux, uy, r, a0, sw)]
    case 1n+m:
      Arc.at(i, ux, uy, r, a0, sw) <> Arc.steps(m, 1n+i, ux, uy, r, a0, sw)

def Arc.sweep(+ux: F32, +uy: F32, +ax: F32, +ay: F32, +bx: F32, +by: F32, +cx: F32, +cy: F32) -> List<&2, Pt>:
  +a0 = F32.atan2(F32.sub(ay, uy), F32.sub(ax, ux))
  +s1 = Arc.norm(F32.sub(F32.atan2(F32.sub(cy, uy), F32.sub(cx, ux)), a0))
  +sm = Arc.norm(F32.sub(F32.atan2(F32.sub(by, uy), F32.sub(bx, ux)), a0))
  Arc.steps(24n, 0n, ux, uy, F32.hypot(F32.sub(ax, ux), F32.sub(ay, uy)), a0,
    Bool.pick(F32, F32.is_lt(sm, s1), s1, F32.sub(s1, F32.mul(2.0, F32.pi()))))

def Arc.points.k(flat: Bool, +d: F32, +ax: F32, +ay: F32, +bx: F32, +by: F32, +cx: F32, +cy: F32) -> List<&2, Pt>:
  match flat:
    case True{}:
      [Pt{ax, ay}, Pt{cx, cy}]
    case False{}:
      Arc.sweep(
        F32.div(F32.add(F32.mul(F32.add(Arc.sq(ax), Arc.sq(ay)), F32.sub(by, cy)), F32.add(F32.mul(F32.add(Arc.sq(bx), Arc.sq(by)), F32.sub(cy, ay)), F32.mul(F32.add(Arc.sq(cx), Arc.sq(cy)), F32.sub(ay, by)))), d),
        F32.div(F32.add(F32.mul(F32.add(Arc.sq(ax), Arc.sq(ay)), F32.sub(cx, bx)), F32.add(F32.mul(F32.add(Arc.sq(bx), Arc.sq(by)), F32.sub(ax, cx)), F32.mul(F32.add(Arc.sq(cx), Arc.sq(cy)), F32.sub(bx, ax)))), d),
        ax, ay, bx, by, cx, cy)

def Arc.points.d(+d: F32, +ax: F32, +ay: F32, +bx: F32, +by: F32, +cx: F32, +cy: F32) -> List<&2, Pt>:
  Arc.points.k(F32.is_lt(F32.abs(d), 0.000001), d, ax, ay, bx, by, cx, cy)

# points along the circle through a, b, c from a to c (b decides the way)
def Arc.points(+a: Pt, +b: Pt, +c: Pt) -> List<&2, Pt>:
  match a b c:
    case Pt{+ax, +ay} Pt{+bx, +by} Pt{+cx, +cy}:
      Arc.points.d(F32.mul(2.0, F32.add(F32.mul(ax, F32.sub(by, cy)), F32.add(F32.mul(bx, F32.sub(cy, ay)), F32.mul(cx, F32.sub(ay, by))))), ax, ay, bx, by, cx, cy)

# Layers
# ------

# every layer the viewer knows, in its paint order (first is lowest)
def Layer.names() -> List<&2, String>:
  ["B.Fab", "B.CrtYd", "B.Paste", "B.SilkS", "B.Mask", "B.Cu", "In6.Cu", "In5.Cu", "In4.Cu", "In3.Cu",
   "In2.Cu", "In1.Cu", "F.Cu", "*.Cu", "F.Mask", "F.Paste", "F.SilkS", "F.CrtYd", "F.Fab",
   "Dwgs.User", "Cmts.User", "Margin", "Edge.Cuts", "via", "hole"]

def Layer.pos.hit(ns: List<&2, String>, +s: String) -> Bool:
  match ns:
    case n <> _:
      String.eq(n, s)
    case Nil{}:
      False{}

def Layer.pos.go(ns: List<&2, String>, hit: Bool, +s: String, +i: U32) -> U32:
  match ns hit:
    case _ <> _ True{}:
      i
    case _ <> +t False{}:
      Layer.pos.go(t, Layer.pos.hit(t, s), s, (i + 1 : U32))
    case Nil{} _:
      19

# a layer's index (unknown layers draw with Dwgs.User)
def Layer.id(+s: String) -> U32:
  Layer.pos.go(Layer.names(), Layer.pos.hit(Layer.names(), s), s, 0)

# Board-level forms
# -----------------

def Board.width.k(+w: F32, +sw: F32) -> F32:
  Bool.pick(F32, F32.is_gt(w, 0.0), w, sw)

def Board.width(+x: S.Sx) -> F32:
  Board.width.k(S.Sx.f(S.Sx.get(x, "width"), 0n), S.Sx.f(S.Sx.get(S.Sx.get(x, "stroke"), "width"), 0n))

def Board.filled(+x: S.Sx) -> Bool:
  +f = S.Sx.arg(S.Sx.get(x, "fill"), 0n)
  Bool.or(String.eq(f, "yes"), String.eq(f, "solid"))

def Board.key.k(+u: String, +t: String, +fallback: String) -> String:
  Bool.pick(String, Bool.not(String.is_empty(u)), u, Bool.pick(String, Bool.not(String.is_empty(t)), t, fallback))

def Board.key(+x: S.Sx, +fallback: String) -> String:
  Board.key.k(S.Sx.attr(x, "uuid"), S.Sx.attr(x, "tstamp"), fallback)

def Board.pts.k(xy: Bool, +x: S.Sx, +pl: Place, rest: List<&2, Pt>) -> List<&2, Pt>:
  match xy:
    case True{}:
      Geo.place(pl, Geo.pt(x)) <> rest
    case False{}:
      rest

def Board.pts(xs: List<&2, S.Sx>, +pl: Place) -> List<&2, Pt>:
  match xs:
    case Nil{}:
      Nil{}
    case +x <> t:
      Board.pts.k(String.eq(S.Sx.name(x), "xy"), x, pl, Board.pts(t, pl))

def Board.home() -> Place:
  Place{0.0, 0.0, 0.0}

def Board.dist(a: Pt, b: Pt) -> F32:
  match a b:
    case Pt{ax, ay} Pt{bx, by}:
      F32.hypot(F32.sub(bx, ax), F32.sub(by, ay))

def Board.rect(a: Pt, b: Pt, +pl: Place) -> List<&2, Pt>:
  match a b:
    case Pt{+ax, +ay} Pt{+bx, +by}:
      [Geo.place(pl, Pt{ax, ay}), Geo.place(pl, Pt{bx, ay}), Geo.place(pl, Pt{bx, by}), Geo.place(pl, Pt{ax, by})]

def Info.net.k(+name: String, +num: String) -> String:
  Bool.pick(String, String.is_empty(name), num, name)

# What an item is, for the viewer's inspector: "kind;key=value;..."

def Info.net(+x: S.Sx) -> String:
  Info.net.k(S.Sx.arg(S.Sx.get(x, "net"), 1n), S.Sx.arg(S.Sx.get(x, "net"), 0n))

def Info.kv(+k: String, +v: String) -> String:
  Bool.pick(String, String.is_empty(v), "", ";" ++ k ++ "=" ++ v)

def Info.graphic(+name: String, +x: S.Sx, +ctx: String) -> String:
  Bool.pick(String, Bool.or(String.eq(name, "segment"), String.eq(name, "arc")), "track", name)
    ++ Info.kv("net", Info.net(x)) ++ Info.kv("layer", S.Sx.attr(x, "layer")) ++ Info.kv("width", F32.show(Board.width(x))) ++ ctx

# a footprint property ("Reference", "Value"), newest format first
def Info.prop.hit(xs: List<&2, S.Sx>, +k: String) -> Bool:
  match xs:
    case +x <> _:
      Bool.and(String.eq(S.Sx.name(x), "property"), String.eq(S.Sx.arg(x, 0n), k))
    case Nil{}:
      False{}

def Info.prop.go(xs: List<&2, S.Sx>, hit: Bool, +k: String) -> String:
  match xs hit:
    case +x <> _ True{}:
      S.Sx.arg(x, 1n)
    case _ <> +t False{}:
      Info.prop.go(t, Info.prop.hit(t, k), k)
    case Nil{} _:
      ""

def Info.prop(+x: S.Sx, +k: String) -> String:
  Info.prop.go(S.Sx.args(x), Info.prop.hit(S.Sx.args(x), k), k)

def Info.footprint(+x: S.Sx) -> String:
  Info.kv("ref", Info.prop(x, "Reference")) ++ Info.kv("value", Info.prop(x, "Value")) ++ Info.kv("footprint", S.Sx.arg(x, 0n))

def Board.graphic.k(i: Nat, +x: S.Sx, +key: String, +sig: U32, +layer: U32, +pl: Place, +info: String) -> List<&2, Item>:
  match i:
    case 0n:
      [Item{key, sig, layer, PSeg{Geo.place(pl, Geo.at(x, "start")), Geo.place(pl, Geo.at(x, "end")), Board.width(x)}, info}]
    case 1n:
      [Item{key, sig, layer, PLine{Arc.points(Geo.place(pl, Geo.at(x, "start")), Geo.place(pl, Geo.at(x, "mid")), Geo.place(pl, Geo.at(x, "end"))), Board.width(x)}, info}]
    case 2n:
      [Item{key, sig, layer, PCirc{Geo.place(pl, Geo.at(x, "center")), Board.dist(Geo.at(x, "center"), Geo.at(x, "end")), Board.width(x), Board.filled(x)}, info}]
    case 3n:
      [Item{key, sig, layer, PPoly{Board.rect(Geo.at(x, "start"), Geo.at(x, "end"), pl), Board.width(x), Board.filled(x)}, info}]
    case 4n:
      [Item{key, sig, layer, PPoly{Board.pts(S.Sx.args(S.Sx.get(x, "pts")), pl), Board.width(x), Board.filled(x)}, info}]
    case _:
      Nil{}

def Board.kinds() -> List<&2, String>:
  ["line", "arc", "circle", "rect", "poly"]

def Board.pos.hit(ns: List<&2, String>, +s: String) -> Bool:
  match ns:
    case n <> _:
      String.eq(n, s)
    case Nil{}:
      False{}

def Board.pos.go(ns: List<&2, String>, hit: Bool, +s: String, +i: Nat) -> Nat:
  match ns hit:
    case _ <> _ True{}:
      i
    case _ <> +t False{}:
      Board.pos.go(t, Board.pos.hit(t, s), s, 1n+i)
    case Nil{} _:
      9n

def Board.index(+ns: List<&2, String>, +s: String) -> Nat:
  Board.pos.go(ns, Board.pos.hit(ns, s), s, 0n)

def Board.kind.k(gr: Bool, fp: Bool, seg: Bool, arc: Bool, +name: String) -> Nat:
  match gr fp seg arc:
    case True{} _ _ _:
      Board.index(Board.kinds(), String.drop(name, 3n))
    case _ True{} _ _:
      Board.index(Board.kinds(), String.drop(name, 3n))
    case _ _ True{} _:
      0n
    case _ _ _ True{}:
      1n
    case _ _ _ _:
      9n

# "gr_line" -> line, "fp_arc" -> arc, "segment" -> line, "arc" -> arc
def Board.kind(+name: String) -> Nat:
  Board.kind.k(String.starts_with(name, "gr_"), String.starts_with(name, "fp_"), String.eq(name, "segment"), String.eq(name, "arc"), name)

def Board.graphic(+x: S.Sx, +key: String, +sig: U32, +pl: Place, +ctx: String) -> List<&2, Item>:
  Board.graphic.k(Board.kind(S.Sx.name(x)), x, key, sig, Layer.id(S.Sx.attr(x, "layer")), pl, Info.graphic(S.Sx.name(x), x, ctx))

# Vias and pads
# -------------

def Board.via(+x: S.Sx, +key: String, +sig: U32) -> List<&2, Item>:
  +c = Geo.at(x, "at")
  +info = "via" ++ Info.kv("net", Info.net(x)) ++ Info.kv("size", S.Sx.arg(S.Sx.get(x, "size"), 0n)) ++ Info.kv("drill", S.Sx.arg(S.Sx.get(x, "drill"), 0n))
  [Item{key, sig, 23, PCirc{c, F32.div(S.Sx.f(S.Sx.get(x, "size"), 0n), 2.0), 0.0, True{}}, info},
   Item{key ++ "/h", sig, 24, PCirc{c, F32.div(S.Sx.f(S.Sx.get(x, "drill"), 0n), 2.0), 0.0, True{}}, info}]

def Pad.frame(c: Pt, +a: F32) -> Place:
  match c:
    case Pt{x, y}:
      Place{x, y, a}

# the pad's outline around its centre, rotated by its absolute angle
def Pad.corners(+c: Pt, +w: F32, +h: F32, +a: F32) -> List<&2, Pt>:
  +hw = F32.div(w, 2.0)
  +hh = F32.div(h, 2.0)
  [Geo.place(Pad.frame(c, a), Pt{F32.neg(hw), F32.neg(hh)}), Geo.place(Pad.frame(c, a), Pt{hw, F32.neg(hh)}),
   Geo.place(Pad.frame(c, a), Pt{hw, hh}), Geo.place(Pad.frame(c, a), Pt{F32.neg(hw), hh})]

def Pad.oval(+c: Pt, +w: F32, +h: F32, +a: F32) -> Prim:
  +horiz = F32.is_ge(w, h)
  +half = F32.div(F32.sub(Bool.pick(F32, horiz, w, h), Bool.pick(F32, horiz, h, w)), 2.0)
  PSeg{Geo.place(Pad.frame(c, a), Bool.pick(Pt, horiz, Pt{F32.neg(half), 0.0}, Pt{0.0, F32.neg(half)})),
    Geo.place(Pad.frame(c, a), Bool.pick(Pt, horiz, Pt{half, 0.0}, Pt{0.0, half})), Bool.pick(F32, horiz, h, w)}

def Pad.shape.k(circle: Bool, oval: Bool, +c: Pt, +w: F32, +h: F32, +a: F32) -> Prim:
  match circle oval:
    case True{} _:
      PCirc{c, F32.div(w, 2.0), 0.0, True{}}
    case False{} True{}:
      Pad.oval(c, w, h, a)
    case False{} False{}:
      PPoly{Pad.corners(c, w, h, a), 0.0, True{}}

def Pad.hole.k(has: Bool, +key: String, +sig: U32, +c: Pt, +d: F32, +info: String) -> List<&2, Item>:
  match has:
    case True{}:
      [Item{key ++ "/h", sig, 24, PCirc{c, F32.div(d, 2.0), 0.0, True{}}, info}]
    case False{}:
      Nil{}

# a pad on its first listed layer ("*.Cu" draws on both sides), plus its hole
def Board.pad(+x: S.Sx, +key: String, +sig: U32, +pl: Place, +ctx: String) -> List<&2, Item>:
  +info = "pad " ++ S.Sx.arg(x, 0n) ++ Info.kv("net", Info.net(x)) ++ Info.kv("type", S.Sx.arg(x, 1n)) ++ Info.kv("shape", S.Sx.arg(x, 2n)) ++ ctx
  +at = S.Sx.get(x, "at")
  +c = Geo.place(pl, Geo.pt(at))
  +size = S.Sx.get(x, "size")
  +shape = S.Sx.arg(x, 2n)
  +drill = S.Sx.f(S.Sx.get(x, "drill"), 0n)
  Item{key, sig, Layer.id(S.Sx.arg(S.Sx.get(x, "layers"), 0n)), Pad.shape.k(String.eq(shape, "circle"), String.eq(shape, "oval"), c,
    S.Sx.f(size, 0n), S.Sx.f(size, 1n), Geo.rad(F32.neg(S.Sx.f(at, 2n)))), info}
  <> Pad.hole.k(F32.is_gt(drill, 0.0), key, sig, c, drill, info)

# Footprints
# ----------

def Fp.child.k(pad: Bool, +x: S.Sx, +key: String, +sig: U32, +pl: Place, +ctx: String) -> List<&2, Item>:
  match pad:
    case True{}:
      Board.pad(x, key, sig, pl, ctx)
    case False{}:
      Board.graphic(x, key, sig, pl, ctx)

def Fp.children(xs: List<&2, S.Sx>, +fp: String, +sig: U32, +pl: Place, +i: Nat, +ctx: String) -> List<&2, Item>:
  match xs:
    case Nil{}:
      Nil{}
    case +x <> t:
      List.append(&2, Item,
        Fp.child.k(String.eq(S.Sx.name(x), "pad"), x, fp ++ "/" ++ Board.key(x, Nat.show(i)), sig, pl, ctx),
        Fp.children(t, fp, sig, pl, 1n+i, ctx))

# 10.99 writes the footprint origin/angle as a TRS transform. Keep the
# legacy at form for stock KiCad files. The presence of transform is
# authoritative, including an explicitly zero translation or rotation.
def Fp.at.k(legacy: Bool, +x: S.Sx) -> S.Sx:
  match legacy:
    case True{}:
      S.Sx.get(x, "at")
    case False{}:
      +tr = S.Sx.get(x, "transform")
      S.XList{S.Sx.chain([S.XAtom{S.Sx.attr(tr, "rotate")}, S.XAtom{S.Sx.arg(S.Sx.get(tr, "translate"), 1n)},
        S.XAtom{S.Sx.arg(S.Sx.get(tr, "translate"), 0n)}, S.XAtom{"at"}], S.XEnd{})}

def Fp.at(+x: S.Sx) -> S.Sx:
  Fp.at.k(String.is_empty(S.Sx.name(S.Sx.get(x, "transform"))), x)

def Fp.place(+x: S.Sx) -> Place:
  +at = Fp.at(x)
  Place{S.Sx.f(at, 0n), S.Sx.f(at, 1n), Geo.rad(S.Sx.f(at, 2n))}

def Board.footprint(+x: S.Sx, +key: String, +sig: U32) -> List<&2, Item>:
  Fp.children(S.Sx.args(x), key, sig, Fp.place(x), 0n, Info.footprint(x))

# Zones
# -----

def Zone.fill.k(hit: Bool, +x: S.Sx, +key: String, +sig: U32, +i: Nat, +info: String, rest: List<&2, Item>) -> List<&2, Item>:
  match hit:
    case True{}:
      Item{key ++ "/f" ++ Nat.show(i), sig, Layer.id(S.Sx.attr(x, "layer")), PPoly{Board.pts(S.Sx.args(S.Sx.get(x, "pts")), Board.home()), 0.0, True{}}, info ++ Info.kv("layer", S.Sx.attr(x, "layer"))} <> rest
    case False{}:
      rest

def Zone.fills(xs: List<&2, S.Sx>, +key: String, +sig: U32, +i: Nat, +info: String) -> List<&2, Item>:
  match xs:
    case Nil{}:
      Nil{}
    case +x <> t:
      Zone.fill.k(String.eq(S.Sx.name(x), "filled_polygon"), x, key, sig, i, info, Zone.fills(t, key, sig, 1n+i, info))

# Forms
# -----

def Board.forms() -> List<&2, String>:
  ["segment", "arc", "via", "footprint", "zone", "gr_line", "gr_arc", "gr_circle", "gr_rect", "gr_poly"]

# FNV-1a over the form's bytes: its signature
def Board.hash(bs: List<&2, U32>, +h: U32) -> U32:
  match bs:
    case Nil{}:
      h
    case b <> t:
      Board.hash(t, (U32.xor(h, b) * 16777619 : U32))

# Parallel conversion
# -------------------
# The runtime hands each half of a parallel call to a core once and never
# moves work afterwards, so halves must cost about the same. A board lists
# its heavy footprints before its light segments, so halves by position
# would not; dealing forms alternately gives each half a fair mix.

type Form is Data:
  Form{i: Nat, bytes: List<&2, U32>}

type Hand is Data:
  Hand{a: List<&2, Form>, b: List<&2, Form>}

def Board.number(fs: List<&2, List<&2, U32>>, +i: Nat) -> List<&2, Form>:
  match fs:
    case Nil{}:
      Nil{}
    case f <> t:
      Form{i, f} <> Board.number(t, 1n+i)

# alternate forms into two hands
def Board.deal(fs: List<&2, Form>, h: Hand) -> Hand:
  match fs h:
    case Nil{} _:
      h
    case f <> t Hand{a, b}:
      Board.deal(t, Hand{b, f <> a})

# deal fs into two hands and hand them to k
def Board.split.k(h: Hand, k: List<&2, Form> -> List<&2, Form> -> List<&2, Item>) -> List<&2, Item>:
  match h:
    case Hand{a, b}:
      k(a, b)

def Board.split(fs: List<&2, Form>, k: List<&2, Form> -> List<&2, Form> -> List<&2, Item>) -> List<&2, Item>:
  Board.split.k(Board.deal(fs, Hand{Nil{}, Nil{}}), k)

def Info.kv.k(+a: String, +b: String) -> String:
  Bool.pick(String, String.is_empty(a), b, a)

def Board.form.k(i: Nat, +x: S.Sx, +key: String, +sig: U32) -> List<&2, Item>:
  match i:
    case 2n:
      Board.via(x, key, sig)
    case 3n:
      Board.footprint(x, key, sig)
    case 4n:
      Zone.fills(S.Sx.args(x), key, sig, 0n, "zone" ++ Info.kv("net", Info.kv.k(S.Sx.attr(x, "net_name"), Info.net(x))))
    case 9n:
      Nil{}
    case _:
      Board.graphic(x, key, sig, Board.home(), "")

# the items one top-level form draws; unknown forms draw nothing
def Board.form(+x: S.Sx, +i: Nat, +sig: U32) -> List<&2, Item>:
  Board.form.k(Board.index(Board.forms(), S.Sx.name(x)), x, Board.key(x, "n" ++ Nat.show(i)), sig)

def Board.one(+bs: List<&2, U32>, +i: Nat) -> List<&2, Item>:
  Board.form(S.Sx.parse(bs), i, Board.hash(bs, 2166136261))

def Board.seq(fs: List<&2, Form>) -> List<&2, Item>:
  match fs:
    case Nil{}:
      Nil{}
    case Form{+i, bs} <> t:
      List.append(&2, Item, Board.one(bs, i), Board.seq(t))

# convert both hands at once, d levels deep
def Board.par(d: Nat, fs: List<&2, Form>) -> List<&2, Item>:
  match d:
    case 0n:
      Board.seq(fs)
    case 1n+e0:
      +e = e0
      Board.split(fs, a => b => (
        x y = Board.par(e, a) Board.par(e, b)
        List.append(&2, Item, x, y)))

def Board.items.k(m: Maybe<&2, List<&2, List<&2, U32>>>) -> Maybe<&2, List<&2, Item>>:
  match m:
    case Some{fs}:
      Some{Board.par(8n, Board.number(fs, 0n))}
    case None{}:
      None{}

# every item of a board file, or None while the file is half written
def Board.items(bs: List<&2, U32>) -> Maybe<&2, List<&2, Item>>:
  Board.items.k(S.Sx.forms(bs))
