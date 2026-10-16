import Base
import ./font.bend as F

# Draw ops to an Image quadtree. Ops are kept topmost first. A region with
# no ops is one Pix of the background; a region fully under a topmost
# opaque rectangle is one Pix of its colour; anything else splits in four,
# each quadrant keeping only the ops that reach it, and the four render at
# once. So a quiet frame costs little, and the cost of a busy one spreads
# over every core.

# Colours are 0xRRGGBB; alpha is 0..255.

# a polygon edge, in pixels
type Edge is Data:
  Edge{ax: F32, ay: F32, bx: F32, by: F32}

# a coverage bitmap (anti-aliased glyphs): 8-bit coverage, four pixels
# per word, first pixel in the low byte, held as a complete binary tree
# of 2^d words so a pixel is found in d steps. A leaf of 16 words (M16)
# stands for the last four levels: a glyph's mask is then a few nodes, so
# frames compare glyphs in a few steps, not one per word
type Mask is Data:
  MWord{w: U32}
  MPair{lo: Mask, hi: Mask}
  M16{w0: U32, w1: U32, w2: U32, w3: U32, w4: U32, w5: U32, w6: U32, w7: U32, w8: U32, w9: U32, w10: U32, w11: U32, w12: U32, w13: U32, w14: U32, w15: U32}

# a run of glyphs (one line of text in one face and colour) as a tree in
# string order: a leaf is a glyph's coverage bitmap at (gx, gy), w x h, id
# naming it in the glyph table (RKey: the glyph by its id alone, as the
# tiles keep it, Key.ops; drawn once resolved against the table, Res.ops); a
# pair's glyphs lie in [x0, x1) x [y0, y1). Coordinates are from the run's
# origin (its SRun's ox, oy, modulo 2^32), so moving the run moves no glyph.
type Run is Data:
  RNone{}
  RGlyph{gx: U32, gy: U32, w: U32, h: U32, id: U32, d: Nat, m: Mask}
  RPair{x0: U32, y0: U32, x1: U32, y1: U32, a: Run, b: Run}
  RKey{gx: U32, gy: U32, w: U32, h: U32, id: U32}

# SPoly is an even-odd polygon cut to the region at hand: `inside` says
# whether the region's top-left corner is inside, `edges` are the edges
# that reach the region. With no edges left the region is all in or out.
type Shape is Data:
  SRect{}
  SPoly{inside: Bool, edges: List<&2, Edge>}
  SGlyph{gx: U32, gy: U32, glyph: F.Glyph}
  SSeg{ax: F32, ay: F32, bx: F32, by: F32, hw: F32}
  SDisc{cx: F32, cy: F32, r: F32}
  STri{ax: F32, ay: F32, bx: F32, by: F32, cx: F32, cy: F32}
  SMask{gx: U32, gy: U32, w: U32, d: Nat, m: Mask}
  SRun{ox: U32, oy: U32, r: Run}

# an op's box [x0, x1) x [y0, y1) is its shape's extent cut to its clip
type Op is Data:
  Op{x0: U32, y0: U32, x1: U32, y1: U32, color: U32, alpha: U32, shape: Shape}

# Colour
# ------

def Rgb.ch(+c: U32, +s: Nat) -> U32:
  U32.and(U32.shrn(c, s), 255)

def Rgb.mix.ch(+a: U32, +b: U32, +t: U32) -> U32:
  (((a * t : U32) + (b * (255 - t : U32) : U32) : U32) / 255 : U32)

# fg over bg with opacity t
def Rgb.mix(+fg: U32, +bg: U32, +t: U32) -> U32:
  U32.or(U32.shln(Rgb.mix.ch(Rgb.ch(fg, 16n), Rgb.ch(bg, 16n), t), 16n),
    U32.or(U32.shln(Rgb.mix.ch(Rgb.ch(fg, 8n), Rgb.ch(bg, 8n), t), 8n), Rgb.mix.ch(Rgb.ch(fg, 0n), Rgb.ch(bg, 0n), t)))

# Coverage
# --------

def Cov.clamp(+x: F32) -> U32:
  F32.to_u32(F32.mul(F32.clamp(x, 0.0, 1.0), 255.0))

# distance from p to segment ab, as coverage of a line hw wide on each side
def Cov.seg(+ax: F32, +ay: F32, +bx: F32, +by: F32, +hw: F32, +px: F32, +py: F32) -> U32:
  +dx = F32.sub(bx, ax)
  +dy = F32.sub(by, ay)
  +ll = F32.add(F32.mul(dx, dx), F32.mul(dy, dy))
  +t = Bool.pick(F32, F32.is_le(ll, 0.0), 0.0, F32.clamp(F32.div(F32.add(F32.mul(F32.sub(px, ax), dx), F32.mul(F32.sub(py, ay), dy)), ll), 0.0, 1.0))
  Cov.clamp(F32.sub(F32.add(hw, 0.5), F32.hypot(F32.sub(px, F32.add(ax, F32.mul(t, dx))), F32.sub(py, F32.add(ay, F32.mul(t, dy))))))

def Cov.disc(+cx: F32, +cy: F32, +r: F32, +px: F32, +py: F32) -> U32:
  Cov.clamp(F32.sub(F32.add(r, 0.5), F32.hypot(F32.sub(px, cx), F32.sub(py, cy))))

def Cov.edge(+ax: F32, +ay: F32, +bx: F32, +by: F32, +px: F32, +py: F32) -> F32:
  F32.sub(F32.mul(F32.sub(bx, ax), F32.sub(py, ay)), F32.mul(F32.sub(by, ay), F32.sub(px, ax)))

def Cov.tri.k(+e0: F32, +e1: F32, +e2: F32) -> U32:
  +pos = Bool.and(F32.is_ge(e0, 0.0), Bool.and(F32.is_ge(e1, 0.0), F32.is_ge(e2, 0.0)))
  +neg = Bool.and(F32.is_le(e0, 0.0), Bool.and(F32.is_le(e1, 0.0), F32.is_le(e2, 0.0)))
  Bool.pick(U32, Bool.or(pos, neg), 255, 0)

def Cov.tri(+ax: F32, +ay: F32, +bx: F32, +by: F32, +cx: F32, +cy: F32, +px: F32, +py: F32) -> U32:
  Cov.tri.k(Cov.edge(ax, ay, bx, by, px, py), Cov.edge(bx, by, cx, cy, px, py), Cov.edge(cx, cy, ax, ay, px, py))

def Cov.glyph(g: F.Glyph, +gx: U32, +gy: U32, +x: U32, +y: U32) -> U32:
  Bool.pick(U32, F.Font.bit(g, (x - gx : U32), (y - gy : U32)), 255, 0)

def Mask.pick(hi: Bool, a: Mask, b: Mask) -> Mask:
  match hi:
    case False{}:
      a
    case True{}:
      b

def Mask.sel(hi: Bool, +a: U32, +b: U32) -> U32:
  match hi:
    case False{}:
      a
    case True{}:
      b

def Mask.bit(+i: U32, +k: Nat) -> Bool:
  U32.is_ne(U32.and(U32.shrn(i, k), 1), 0)

# word i (its low four bits) of a 16-word leaf
def Mask.leaf(m: Mask, +i: U32) -> U32:
  match m:
    case M16{+a0, +a1, +a2, +a3, +a4, +a5, +a6, +a7, +a8, +a9, +a10, +a11, +a12, +a13, +a14, +a15}:
      Mask.sel(Mask.bit(i, 3n),
        Mask.sel(Mask.bit(i, 2n), Mask.sel(Mask.bit(i, 1n), Mask.sel(Mask.bit(i, 0n), a0, a1), Mask.sel(Mask.bit(i, 0n), a2, a3)), Mask.sel(Mask.bit(i, 1n), Mask.sel(Mask.bit(i, 0n), a4, a5), Mask.sel(Mask.bit(i, 0n), a6, a7))),
        Mask.sel(Mask.bit(i, 2n), Mask.sel(Mask.bit(i, 1n), Mask.sel(Mask.bit(i, 0n), a8, a9), Mask.sel(Mask.bit(i, 0n), a10, a11)), Mask.sel(Mask.bit(i, 1n), Mask.sel(Mask.bit(i, 0n), a12, a13), Mask.sel(Mask.bit(i, 0n), a14, a15))))
    case MWord{w}:
      w
    case MPair{_, _}:
      0

# word i of a tree of 2^d words
def Mask.at(d: Nat, m: Mask, +i: U32) -> U32:
  match d m:
    case _ MWord{w}:
      w
    case _ M16{a0, a1, a2, a3, a4, a5, a6, a7, a8, a9, a10, a11, a12, a13, a14, a15}:
      Mask.leaf(M16{a0, a1, a2, a3, a4, a5, a6, a7, a8, a9, a10, a11, a12, a13, a14, a15}, i)
    case 0n MPair{_, _}:
      0
    case 1n+k0 MPair{lo, hi}:
      +k = k0
      Mask.at(k, Mask.pick(U32.is_ne(U32.and(U32.shrn(i, k), 1), 0), lo, hi), i)

# the coverage byte of pixel (x, y) of a w-wide bitmap at (gx, gy)
def Cov.mask.k(+i: U32, d: Nat, m: Mask) -> U32:
  U32.and(U32.shrn(Mask.at(d, m, U32.shrn(i, 2n)), Nat.mul(U32.to_nat(U32.and(i, 3)), 8n)), 255)

def Cov.mask(+gx: U32, +gy: U32, +w: U32, d: Nat, m: Mask, +x: U32, +y: U32) -> U32:
  Cov.mask.k((((y - gy : U32) * w : U32) + (x - gx : U32) : U32), d, m)

def Cross.within(+v: F32, +a: F32, +b: F32) -> Bool:
  Bool.and(F32.is_ge(v, a), F32.is_lt(v, b))

def Cross.h1.k(spans: Bool, +ax: F32, +ay: F32, +bx: F32, +by: F32, +y: F32, +xa: F32, +xb: F32) -> Bool:
  match spans:
    case True{}:
      Cross.within(F32.add(ax, F32.div(F32.mul(F32.sub(y, ay), F32.sub(bx, ax)), F32.sub(by, ay))), xa, xb)
    case False{}:
      False{}

# Polygons
# --------

# does the edge cross the horizontal segment at y from xa (in) to xb (out)?
def Cross.h1(e: Edge, +y: F32, +xa: F32, +xb: F32) -> Bool:
  match e:
    case Edge{+ax, +ay, +bx, +by}:
      Cross.h1.k(Bool.xor(F32.is_gt(ay, y), F32.is_gt(by, y)), ax, ay, bx, by, y, xa, xb)

def Cross.v1.k(spans: Bool, +ax: F32, +ay: F32, +bx: F32, +by: F32, +x: F32, +ya: F32, +yb: F32) -> Bool:
  match spans:
    case True{}:
      Cross.within(F32.add(ay, F32.div(F32.mul(F32.sub(x, ax), F32.sub(by, ay)), F32.sub(bx, ax))), ya, yb)
    case False{}:
      False{}

def Cross.v1(e: Edge, +x: F32, +ya: F32, +yb: F32) -> Bool:
  match e:
    case Edge{+ax, +ay, +bx, +by}:
      Cross.v1.k(Bool.xor(F32.is_gt(ax, x), F32.is_gt(bx, x)), ax, ay, bx, by, x, ya, yb)

# parity of edges crossing the horizontal / vertical segment
def Cross.h(es: List<&2, Edge>, +y: F32, +xa: F32, +xb: F32, acc: Bool) -> Bool:
  match es:
    case Nil{}:
      acc
    case e <> t:
      Cross.h(t, y, xa, xb, Bool.xor(acc, Cross.h1(e, y, xa, xb)))

def Cross.v(es: List<&2, Edge>, +x: F32, +ya: F32, +yb: F32, acc: Bool) -> Bool:
  match es:
    case Nil{}:
      acc
    case e <> t:
      Cross.v(t, x, ya, yb, Bool.xor(acc, Cross.v1(e, x, ya, yb)))

# the parity at (qx, qy) from the parity at (px, py) (px <= qx, py <= qy),
# walking down then right
def Cross.walk(+es: List<&2, Edge>, inside: Bool, +px: F32, +py: F32, +qx: F32, +qy: F32) -> Bool:
  Cross.h(es, qy, px, qx, Cross.v(es, px, py, qy, inside))

# distance from p to the edge ab
def Cov.edist(e: Edge, +px: F32, +py: F32) -> F32:
  match e:
    case Edge{+ax, +ay, +bx, +by}:
      +dx = F32.sub(bx, ax)
      +dy = F32.sub(by, ay)
      +ll = F32.add(F32.mul(dx, dx), F32.mul(dy, dy))
      +t = Bool.pick(F32, F32.is_le(ll, 0.0), 0.0, F32.clamp(F32.div(F32.add(F32.mul(F32.sub(px, ax), dx), F32.mul(F32.sub(py, ay), dy)), ll), 0.0, 1.0))
      F32.hypot(F32.sub(px, F32.add(ax, F32.mul(t, dx))), F32.sub(py, F32.add(ay, F32.mul(t, dy))))

# the distance from p to the nearest of the edges, at most d
def Cov.near(es: List<&2, Edge>, +px: F32, +py: F32, +d: F32) -> F32:
  match es:
    case Nil{}:
      d
    case e <> t:
      Cov.near(t, px, py, F32.min(d, Cov.edist(e, px, py)))

# a polygon's coverage of a pixel whose centre is inside it or not, d from
# its nearest edge: half at the edge, a pixel's ramp across it (only the
# pixels an edge reaches have any edges left, so the rest stay whole)
def Cov.poly(inside: Bool, +d: F32) -> U32:
  match inside:
    case True{}:
      Cov.clamp(F32.add(0.5, d))
    case False{}:
      Cov.clamp(F32.sub(0.5, d))

def Run.inside(+x0: U32, +y0: U32, +x1: U32, +y1: U32, +x: U32, +y: U32) -> Bool:
  Bool.and(Bool.and(U32.is_ge(x, x0), U32.is_lt(x, x1)), Bool.and(U32.is_ge(y, y0), U32.is_lt(y, y1)))

# the part of the run may reach local pixel (x, y): a glyph's box holds
# it, a pair spans its column
def Run.hit(r: Run, +x: U32, +y: U32) -> Bool:
  match r:
    case RNone{}:
      False{}
    case RGlyph{+gx, +gy, +w, +h, _, _, _}:
      Run.inside(gx, gy, (gx + w : U32), (gy + h : U32), x, y)
    case RPair{+x0, +y0, +x1, +y1, _, _}:
      Run.inside(x0, y0, x1, y1, x, y)
    case RKey{_, _, _, _, _}:
      False{}

# the most any glyph of the run covers local pixel (x, y) (in: Run.hit;
# a frame shades a run glyph by glyph, Run.px)
def Run.cov(r: Run, in: Bool, +x: U32, +y: U32) -> U32:
  match r in:
    case RGlyph{+gx, +gy, +w, _, _, d, m} True{}:
      Cov.mask(gx, gy, w, d, m, x, y)
    case RPair{_, _, _, _, +a, +b} True{}:
      U32.max(Run.cov(a, Run.hit(a, x, y), x, y), Run.cov(b, Run.hit(b, x, y), x, y))
    case _ _:
      0

def Cov.at(s: Shape, +x: U32, +y: U32, +px: F32, +py: F32) -> U32:
  match s:
    case SRect{}:
      255
    case SGlyph{gx, gy, g}:
      Cov.glyph(g, gx, gy, x, y)
    case SSeg{ax, ay, bx, by, hw}:
      Cov.seg(ax, ay, bx, by, hw, px, py)
    case SDisc{cx, cy, r}:
      Cov.disc(cx, cy, r, px, py)
    case STri{ax, ay, bx, by, cx, cy}:
      Cov.tri(ax, ay, bx, by, cx, cy, px, py)
    case SMask{gx, gy, w, d, m}:
      Cov.mask(gx, gy, w, d, m, x, y)
    case SRun{+ox, +oy, +r}:
      Run.cov(r, Run.hit(r, (x - ox : U32), (y - oy : U32)), (x - ox : U32), (y - oy : U32))
    case SPoly{inside, +es}:
      Cov.poly(Cross.walk(es, inside, U32.to_f32(x), U32.to_f32(y), px, py), Cov.near(es, px, py, 1.0))

def Cross.meets(e: Edge, +x0: F32, +y0: F32, +x1: F32, +y1: F32) -> Bool:
  match e:
    case Edge{+ax, +ay, +bx, +by}:
      Bool.and(Bool.and(F32.is_le(F32.min(ax, bx), x1), F32.is_ge(F32.max(ax, bx), x0)),
        Bool.and(F32.is_le(F32.min(ay, by), y1), F32.is_ge(F32.max(ay, by), y0)))

def Cross.keep.k(hit: Bool, e: Edge, rest: List<&2, Edge>) -> List<&2, Edge>:
  match hit:
    case True{}:
      e <> rest
    case False{}:
      rest

# the edges whose boxes meet the box
def Cross.keep(es: List<&2, Edge>, +x0: F32, +y0: F32, +x1: F32, +y1: F32) -> List<&2, Edge>:
  match es:
    case Nil{}:
      Nil{}
    case +e <> t:
      Cross.keep.k(Cross.meets(e, x0, y0, x1, y1), e, Cross.keep(t, x0, y0, x1, y1))

# how much of pixel (x, y) the shape covers, 0..255
def Cov.of(s: Shape, +x: U32, +y: U32) -> U32:
  Cov.at(s, x, y, F32.add(U32.to_f32(x), 0.5), F32.add(U32.to_f32(y), 0.5))

# Shading one pixel
# -----------------

def Shade.inside(+x0: U32, +y0: U32, +x1: U32, +y1: U32, +x: U32, +y: U32) -> Bool:
  Bool.and(Bool.and(U32.is_ge(x, x0), U32.is_lt(x, x1)), Bool.and(U32.is_ge(y, y0), U32.is_lt(y, y1)))

def Shade.k(+cov: U32, +c: U32, +a: U32, +below: U32) -> U32:
  Rgb.mix(c, below, ((cov * a : U32) / 255 : U32))

# local pixel (x, y) shaded by the run's glyphs, the first topmost, as one
# op per glyph would be (in: Run.hit): a glyph whose box misses the pixel
# leaves it as it is, as a glyph op would (a frame shades a pixel only
# with the ops whose boxes hold it: Region.node)
def Run.px(r: Run, in: Bool, +x: U32, +y: U32, +c: U32, +a: U32, below: U32) -> U32:
  match r in:
    case RGlyph{+gx, +gy, +w, _, _, d, m} True{}:
      Shade.k(Cov.mask(gx, gy, w, d, m, x, y), c, a, below)
    case RPair{_, _, _, _, +ra, +rb} True{}:
      Run.px(ra, Run.hit(ra, x, y), x, y, c, a, Run.px(rb, Run.hit(rb, x, y), x, y, c, a, below))
    case _ _:
      below

def Run.shade(in: Bool, +ox: U32, +oy: U32, +r: Run, +x: U32, +y: U32, +c: U32, +a: U32, +below: U32) -> U32:
  match in:
    case True{}:
      Run.px(r, Run.hit(r, (x - ox : U32), (y - oy : U32)), (x - ox : U32), (y - oy : U32), c, a, below)
    case False{}:
      Shade.k(0, c, a, below)

def Shade.op(s: Shape, +x0: U32, +y0: U32, +x1: U32, +y1: U32, +c: U32, +a: U32, +x: U32, +y: U32, +below: U32) -> U32:
  match s:
    case SRun{+ox, +oy, r}:
      Run.shade(Shade.inside(x0, y0, x1, y1, x, y), ox, oy, r, x, y, c, a, below)
    case _:
      Shade.k(Bool.pick(U32, Shade.inside(x0, y0, x1, y1, x, y), Cov.of(s, x, y), 0), c, a, below)

# the topmost op that covers the pixel decides; a translucent one mixes
# with what lies below it
def Shade.px(ops: List<&2, Op>, +x: U32, +y: U32, +bg: U32) -> U32:
  match ops:
    case Nil{}:
      bg
    case Op{+x0, +y0, +x1, +y1, +c, +a, s} <> t:
      Shade.op(s, x0, y0, x1, y1, c, a, x, y, Shade.px(t, x, y, bg))

# Regions
# -------

def Region.hits(+x0: U32, +y0: U32, +x1: U32, +y1: U32, +rx: U32, +ry: U32, +n: U32) -> Bool:
  Bool.and(Bool.and(U32.is_lt(x0, (rx + n : U32)), U32.is_gt(x1, rx)), Bool.and(U32.is_lt(y0, (ry + n : U32)), U32.is_gt(y1, ry)))

def Region.keep.k(hit: Bool, o: Op, rest: List<&2, Op>) -> List<&2, Op>:
  match hit:
    case True{}:
      o <> rest
    case False{}:
      rest

# a polygon moved from the parent region (corner px, py) to a child: its
# corner parity walks to the child's corner, its edges shrink to the child
def Region.poly(+inside: Bool, +es: List<&2, Edge>, +px: U32, +py: U32, +rx: U32, +ry: U32, +n: U32) -> Shape:
  SPoly{Cross.walk(es, inside, U32.to_f32(px), U32.to_f32(py), U32.to_f32(rx), U32.to_f32(ry)),
    Cross.keep(es, U32.to_f32(rx), U32.to_f32(ry), U32.to_f32((rx + n : U32)), U32.to_f32((ry + n : U32)))}

# Runs cut to a region
# --------------------
# A region keeps only the glyphs whose boxes meet it: a part of the tree
# inside the region is kept whole (shared, nothing built), one outside is
# dropped, and only the pairs across its edges are made again. A run cut
# to nothing draws nothing there (Region.empty).

def Run.x0(r: Run) -> U32:
  match r:
    case RGlyph{gx, _, _, _, _, _, _}:
      gx
    case RKey{gx, _, _, _, _}:
      gx
    case RPair{x0, _, _, _, _, _}:
      x0
    case RNone{}:
      4294967295

def Run.y0(r: Run) -> U32:
  match r:
    case RGlyph{_, gy, _, _, _, _, _}:
      gy
    case RKey{_, gy, _, _, _}:
      gy
    case RPair{_, y0, _, _, _, _}:
      y0
    case RNone{}:
      4294967295

def Run.x1(r: Run) -> U32:
  match r:
    case RGlyph{gx, _, w, _, _, _, _}:
      (gx + w : U32)
    case RKey{gx, _, w, _, _}:
      (gx + w : U32)
    case RPair{_, _, x1, _, _, _}:
      x1
    case RNone{}:
      0

def Run.y1(r: Run) -> U32:
  match r:
    case RGlyph{_, gy, _, h, _, _, _}:
      (gy + h : U32)
    case RKey{_, gy, _, h, _}:
      (gy + h : U32)
    case RPair{_, _, _, y1, _, _}:
      y1
    case RNone{}:
      0

# a then b, as one tree (a pair spans both)
def Run.pair(+a: Run, +b: Run) -> Run:
  RPair{U32.min(Run.x0(a), Run.x0(b)), U32.min(Run.y0(a), Run.y0(b)), U32.max(Run.x1(a), Run.x1(b)), U32.max(Run.y1(a), Run.y1(b)), a, b}

def Run.none(r: Run) -> Bool:
  match r:
    case RNone{}:
      True{}
    case _:
      False{}

def Run.join.k(an: Bool, bn: Bool, +a: Run, +b: Run) -> Run:
  match an bn:
    case True{} _:
      b
    case False{} True{}:
      a
    case False{} False{}:
      Run.pair(a, b)

def Run.join(+a: Run, +b: Run) -> Run:
  Run.join.k(Run.none(a), Run.none(b), a, b)

# where a part of the run lies against the region [x0, x1) x [y0, y1)
type Part is Data:
  POut{}
  PAll{}
  PSome{}

def Run.part.k(meets: Bool, within: Bool) -> Part:
  match meets within:
    case False{} _:
      POut{}
    case True{} True{}:
      PAll{}
    case True{} False{}:
      PSome{}

def Run.part(+r: Run, +x0: U32, +y0: U32, +x1: U32, +y1: U32) -> Part:
  Run.part.k(Bool.and(Bool.and(U32.is_lt(Run.x0(r), x1), U32.is_gt(Run.x1(r), x0)), Bool.and(U32.is_lt(Run.y0(r), y1), U32.is_gt(Run.y1(r), y0))),
    Bool.and(Bool.and(U32.is_ge(Run.x0(r), x0), U32.is_le(Run.x1(r), x1)), Bool.and(U32.is_ge(Run.y0(r), y0), U32.is_le(Run.y1(r), y1))))

# the glyphs of the run whose boxes meet the region (pt: Run.part of r)
def Run.cut(r: Run, pt: Part, +x0: U32, +y0: U32, +x1: U32, +y1: U32) -> Run:
  match r pt:
    case _ POut{}:
      RNone{}
    case _ PAll{}:
      r
    case RPair{_, _, _, _, +a, +b} PSome{}:
      Run.join(Run.cut(a, Run.part(a, x0, y0, x1, y1), x0, y0, x1, y1), Run.cut(b, Run.part(b, x0, y0, x1, y1), x0, y0, x1, y1))
    case _ PSome{}:
      r

# a run moved to the n x n region at (rx, ry): cut to it, in the run's
# own coordinates
def Run.at(+ox: U32, +oy: U32, +r: Run, +rx: U32, +ry: U32, +n: U32) -> Shape:
  SRun{ox, oy, Run.cut(r, Run.part(r, (rx - ox : U32), (ry - oy : U32), (rx + n - ox : U32), (ry + n - oy : U32)), (rx - ox : U32), (ry - oy : U32), (rx + n - ox : U32), (ry + n - oy : U32))}

# The glyph table
# ---------------
# The glyphs' coverage by id (text.bend's Text.table, from the glyph
# cache), a tree ordered by id. The tiles keep text as RKey leaves
# (Key.ops: no masks, so comparing frames compares ids, not coverage) and
# draw a tile from its keys resolved against the table (Res.ops), so the
# pixels are the ones the cache's masks make.

type Tab is Data:
  TNil{}
  TBr{id: U32, d: Nat, m: Mask, l: Tab, r: Tab}

type Side is Data:
  SHere{}
  SLeft{}
  SRight{}

def Tab.side.k(eq: Bool, lt: Bool) -> Side:
  match eq lt:
    case True{} _:
      SHere{}
    case False{} True{}:
      SLeft{}
    case False{} False{}:
      SRight{}

def Tab.side(t: Tab, +id: U32) -> Side:
  match t:
    case TNil{}:
      SHere{}
    case TBr{+k, _, _, _, _}:
      Tab.side.k(U32.is_eq(id, k), U32.is_lt(id, k))

# the glyph with this id (side: Tab.side of t and id)
def Tab.find(t: Tab, side: Side, +id: U32) -> Maybe<&2, Run>:
  match t side:
    case TNil{} _:
      None{}
    case TBr{_, +d, +m, _, _} SHere{}:
      Some{RGlyph{0, 0, 0, 0, id, d, m}}
    case TBr{_, _, _, +l, _} SLeft{}:
      Tab.find(l, Tab.side(l, id), id)
    case TBr{_, _, _, _, +r} SRight{}:
      Tab.find(r, Tab.side(r, id), id)

def Res.key(g: Maybe<&2, Run>, +gx: U32, +gy: U32, +w: U32, +h: U32, +id: U32) -> Run:
  match g:
    case Some{RGlyph{_, _, _, _, _, d, m}}:
      RGlyph{gx, gy, w, h, id, d, m}
    case _:
      RNone{}

# a run with its keys resolved: each RKey the glyph the table has (none:
# nothing, as a glyph not yet fetched)
def Res.run(r: Run, +tab: Tab) -> Run:
  match r:
    case RKey{+gx, +gy, +w, +h, +id}:
      Res.key(Tab.find(tab, Tab.side(tab, id), id), gx, gy, w, h, id)
    case RPair{x0, y0, x1, y1, a, b}:
      RPair{x0, y0, x1, y1, Res.run(a, tab), Res.run(b, tab)}
    case _:
      r

def Res.shape(s: Shape, +tab: Tab) -> Shape:
  match s:
    case SRun{ox, oy, r}:
      SRun{ox, oy, Res.run(r, tab)}
    case _:
      s

# ops with their text resolved against the table
def Res.ops(ops: List<&2, Op>, +tab: Tab) -> List<&2, Op>:
  match ops:
    case Nil{}:
      Nil{}
    case Op{x0, y0, x1, y1, c, a, s} <> t:
      Op{x0, y0, x1, y1, c, a, Res.shape(s, tab)} <> Res.ops(t, tab)

# a run by its glyphs' ids alone
def Key.run(r: Run) -> Run:
  match r:
    case RGlyph{gx, gy, w, h, id, _, _}:
      RKey{gx, gy, w, h, id}
    case RPair{x0, y0, x1, y1, a, b}:
      RPair{x0, y0, x1, y1, Key.run(a), Key.run(b)}
    case _:
      r

def Key.shape(s: Shape) -> Shape:
  match s:
    case SRun{ox, oy, r}:
      SRun{ox, oy, Key.run(r)}
    case _:
      s

# ops with their text kept by ids (what the tiles compare and keep)
def Key.ops(ops: List<&2, Op>) -> List<&2, Op>:
  match ops:
    case Nil{}:
      Nil{}
    case Op{x0, y0, x1, y1, c, a, s} <> t:
      Op{x0, y0, x1, y1, c, a, Key.shape(s)} <> Key.ops(t)

# a polygon with no edges and outside draws nothing here
def Region.empty(s: Shape) -> Bool:
  match s:
    case SPoly{False{}, Nil{}}:
      True{}
    case SRun{_, _, RNone{}}:
      True{}
    case _:
      False{}

def Region.move(s: Shape, +px: U32, +py: U32, +rx: U32, +ry: U32, +n: U32) -> Shape:
  match s:
    case SPoly{+inside, +es}:
      Region.poly(inside, es, px, py, rx, ry, n)
    case SRun{+ox, +oy, +r}:
      Run.at(ox, oy, r, rx, ry, n)
    case _:
      s

def Region.keep.m(+s2: Shape, +x0: U32, +y0: U32, +x1: U32, +y1: U32, +c: U32, +a: U32, rest: List<&2, Op>) -> List<&2, Op>:
  Region.keep.k(Bool.not(Region.empty(s2)), Op{x0, y0, x1, y1, c, a, s2}, rest)

def Region.keep.op(hit: Bool, o: Op, +px: U32, +py: U32, +rx: U32, +ry: U32, +n: U32, rest: List<&2, Op>) -> List<&2, Op>:
  match hit o:
    case True{} Op{+x0, +y0, +x1, +y1, +c, +a, s}:
      Region.keep.m(Region.move(s, px, py, rx, ry, n), x0, y0, x1, y1, c, a, rest)
    case False{} _:
      rest

# the ops that reach the n x n region at (rx, ry), order kept; (px, py) is
# the parent region's corner
def Region.keep(ops: List<&2, Op>, +px: U32, +py: U32, +rx: U32, +ry: U32, +n: U32) -> List<&2, Op>:
  match ops:
    case Nil{}:
      Nil{}
    case Op{+x0, +y0, +x1, +y1, c, a, +s} <> t:
      Region.keep.op(Region.hits(x0, y0, x1, y1, rx, ry, n), Op{x0, y0, x1, y1, c, a, s}, px, py, rx, ry, n, Region.keep(t, px, py, rx, ry, n))

# the ops for the four children of the region at (rx, ry), one pass
type Four is Data:
  Four{a: List<&2, Op>, b: List<&2, Op>, c: List<&2, Op>, d: List<&2, Op>}

def Four.put(+o: Op, +px: U32, +py: U32, +h: U32, f: Four) -> Four:
  match o f:
    case Op{+x0, +y0, +x1, +y1, _, _, _} Four{a, b, c, d}:
      Four{Region.keep.op(Region.hits(x0, y0, x1, y1, px, py, h), o, px, py, px, py, h, a),
        Region.keep.op(Region.hits(x0, y0, x1, y1, (px + h : U32), py, h), o, px, py, (px + h : U32), py, h, b),
        Region.keep.op(Region.hits(x0, y0, x1, y1, px, (py + h : U32), h), o, px, py, px, (py + h : U32), h, c),
        Region.keep.op(Region.hits(x0, y0, x1, y1, (px + h : U32), (py + h : U32), h), o, px, py, (px + h : U32), (py + h : U32), h, d)}

def Four.go(ops: List<&2, Op>, +px: U32, +py: U32, +h: U32) -> Four:
  match ops:
    case Nil{}:
      Four{Nil{}, Nil{}, Nil{}, Nil{}}
    case +o <> t:
      Four.put(o, px, py, h, Four.go(t, px, py, h))

def Four.split.k(f: Four, k: List<&2, Op> -> List<&2, Op> -> List<&2, Op> -> List<&2, Op> -> Image) -> Image:
  match f:
    case Four{a, b, c, d}:
      k(a, b, c, d)

# split ops over the children (halves h) and hand the four lists to k
def Four.split(+ops: List<&2, Op>, +px: U32, +py: U32, +h: U32, k: List<&2, Op> -> List<&2, Op> -> List<&2, Op> -> List<&2, Op> -> Image) -> Image:
  Four.split.k(Four.go(ops, px, py, h), k)

# a base image's four quadrants (a solid one is four of itself)
def Four.base(+img: Image, k: Image -> Image -> Image -> Image -> Image) -> Image:
  match img:
    case Pix{+c}:
      k(Pix{c}, Pix{c}, Pix{c}, Pix{c})
    case Qua{a, b, c, d}:
      k(a, b, c, d)

# the colour of a region's base (a region at a leaf is one pixel)
def Four.top(img: Image) -> U32:
  match img:
    case Pix{c}:
      c
    case Qua{a, _, _, _}:
      Four.top(a)

# the topmost op is an opaque rectangle over the whole region
def Region.covers(+x0: U32, +y0: U32, +x1: U32, +y1: U32, +a: U32, +rx: U32, +ry: U32, +n: U32) -> Bool:
  Bool.and(Bool.and(U32.is_eq(a, 255), Bool.and(U32.is_le(x0, rx), U32.is_le(y0, ry))),
    Bool.and(U32.is_ge(x1, (rx + n : U32)), U32.is_ge(y1, (ry + n : U32))))

def Region.solid(ops: List<&2, Op>, +rx: U32, +ry: U32, +n: U32) -> Bool:
  match ops:
    case Op{x0, y0, x1, y1, _, a, SRect{}} <> _:
      Region.covers(x0, y0, x1, y1, a, rx, ry, n)
    case Op{x0, y0, x1, y1, _, a, SPoly{True{}, Nil{}}} <> _:
      Region.covers(x0, y0, x1, y1, a, rx, ry, n)
    case _:
      False{}

# (a caret's colour carries a tag in its top byte: draw.bend's Draw.caret)
def Region.top(ops: List<&2, Op>) -> U32:
  match ops:
    case Op{_, _, _, _, c, _, _} <> _:
      U32.and(c, 16777215)
    case Nil{}:
      0

type Leaf is Data:
  LEmpty{}
  LSolid{}
  LPixel{}
  LSplit{}

def Region.leaf.k(empty: Bool, solid: Bool, one: Bool) -> Leaf:
  match empty solid one:
    case True{} _ _:
      LEmpty{}
    case False{} True{} _:
      LSolid{}
    case False{} False{} True{}:
      LPixel{}
    case False{} False{} False{}:
      LSplit{}

def Region.leaf(+ops: List<&2, Op>, +rx: U32, +ry: U32, +n: U32) -> Leaf:
  Region.leaf.k(List.is_empty(&2, Op, ops), Region.solid(ops, rx, ry, n), U32.is_eq(n, 1))

# the image of the n x n region at (rx, ry) over the base image's same
# region; d counts levels left (n = 2^d)
def Region.node(d: Nat, l: Leaf, +ops: List<&2, Op>, +rx: U32, +ry: U32, +n: U32, +base: Image) -> Image:
  match d l:
    case _ LEmpty{}:
      base
    case _ LSolid{}:
      Pix{Region.top(ops)}
    case _ LPixel{}:
      Pix{Shade.px(ops, rx, ry, Four.top(base))}
    case 0n LSplit{}:
      Pix{Shade.px(ops, rx, ry, Four.top(base))}
    case 1n+e0 LSplit{}:
      +e = e0
      +h = U32.shr(n)
      +x2 = (rx + h : U32)
      +y2 = (ry + h : U32)
      Four.base(base, ba => bb => bc => bd =>
        Four.split(ops, rx, ry, h, +ta => +tb => +tc => +td => (
          a b c dd = Region.node(e, Region.leaf(ta, rx, ry, h), ta, rx, ry, h, ba) Region.node(e, Region.leaf(tb, x2, ry, h), tb, x2, ry, h, bb) Region.node(e, Region.leaf(tc, rx, y2, h), tc, rx, y2, h, bc) Region.node(e, Region.leaf(td, x2, y2, h), td, x2, y2, h, bd)
          Qua{a, b, c, dd})))

# the exponent of the smallest power of two covering w and h
def Frame.depth(+w: U32, +h: U32) -> Nat:
  1n+U32.log2((U32.max(U32.max(w, h), 2) - 1 : U32))

# a w x h frame of ops (topmost first) over a base frame of the same size:
# only regions the ops reach are drawn, the rest is the base's
def Frame.over(+w: U32, +h: U32, +ops: List<&2, Op>, +base: Image) -> Image:
  +d = Frame.depth(w, h)
  Region.node(d, Region.leaf(ops, 0, 0, U32.shln(1, d)), ops, 0, 0, U32.shln(1, d), base)

# a w x h frame of ops (topmost first) over a solid background
def Frame.render(+w: U32, +h: U32, +ops: List<&2, Op>, +bg: U32) -> Image:
  Frame.over(w, h, ops, Pix{bg})

# Reading pixels back (snapshots, tests)
# --------------------------------------

def Img.pick(right: Bool, down: Bool, a: Image, b: Image, c: Image, e: Image) -> Image:
  match right down:
    case False{} False{}:
      a
    case True{} False{}:
      b
    case False{} True{}:
      c
    case True{} True{}:
      e

# the colour at (x, y) of an image whose root spans 2^d pixels
def Img.at(d: Nat, img: Image, +x: U32, +y: U32) -> U32:
  match d img:
    case _ Pix{c}:
      c
    case 0n Qua{_, _, _, _}:
      0
    case 1n+k0 Qua{a, b, c, e}:
      +k = k0
      Img.at(k, Img.pick(U32.is_ne(U32.and(U32.shrn(x, k), 1), 0), U32.is_ne(U32.and(U32.shrn(y, k), 1), 0), a, b, c, e), x, y)
