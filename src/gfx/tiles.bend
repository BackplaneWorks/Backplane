import Base
import ./raster.bend as R
import ./font.bend as F
import ../core/eq.bend as Q

# Frames drawn again only where they changed. A frame is a tree of tiles;
# each tile keeps the ops that reached it, the base image under it, and
# the image they made. The next frame walks the same tree: a tile whose
# ops and base are exactly the same keeps its image, any other is drawn
# again. Laws tiles_again_exact and frame_again_exact (the result is the
# frame drawn from scratch) and tile_keep (an unchanged tile is not drawn
# again) hold this to account.

# Exact equality
# --------------
# Structural, bit for bit (floats by their bits): true only for equal
# values (laws eq_*_sound), so a reused tile is never a stale one.

def Eq.f32(a: F32, b: F32) -> Bool:
  match a b:
    case F32{x} F32{y}:
      U32.is_eq(U32{x}, U32{y})

def Eq.edge(a: R.Edge, b: R.Edge) -> Bool:
  match a b:
    case R.Edge{a0, a1, a2, a3} R.Edge{b0, b1, b2, b3}:
      Bool.and(Eq.f32(a0, b0), Bool.and(Eq.f32(a1, b1), Bool.and(Eq.f32(a2, b2), Eq.f32(a3, b3))))

def Eq.edges(xs: List<&2, R.Edge>, ys: List<&2, R.Edge>) -> Bool:
  match xs ys:
    case Nil{} Nil{}:
      True{}
    case x <> xt y <> yt:
      Bool.and(Eq.edge(x, y), Eq.edges(xt, yt))
    case _ _:
      False{}

def Eq.mask(a: R.Mask, b: R.Mask) -> Bool:
  match a b:
    case R.MWord{x} R.MWord{y}:
      U32.is_eq(x, y)
    case R.MPair{al, ah} R.MPair{bl, bh}:
      Bool.and(Eq.mask(al, bl), Eq.mask(ah, bh))
    case R.M16{a0, a1, a2, a3, a4, a5, a6, a7, a8, a9, a10, a11, a12, a13, a14, a15} R.M16{b0, b1, b2, b3, b4, b5, b6, b7, b8, b9, b10, b11, b12, b13, b14, b15}:
      Bool.and(U32.is_eq(a0, b0), Bool.and(U32.is_eq(a1, b1), Bool.and(U32.is_eq(a2, b2), Bool.and(U32.is_eq(a3, b3), Bool.and(U32.is_eq(a4, b4), Bool.and(U32.is_eq(a5, b5), Bool.and(U32.is_eq(a6, b6), Bool.and(U32.is_eq(a7, b7), Bool.and(U32.is_eq(a8, b8), Bool.and(U32.is_eq(a9, b9), Bool.and(U32.is_eq(a10, b10), Bool.and(U32.is_eq(a11, b11), Bool.and(U32.is_eq(a12, b12), Bool.and(U32.is_eq(a13, b13), Bool.and(U32.is_eq(a14, b14), U32.is_eq(a15, b15))))))))))))))))
    case _ _:
      False{}

def Eq.run(a: R.Run, b: R.Run) -> Bool:
  match a b:
    case R.RNone{} R.RNone{}:
      True{}
    case R.RGlyph{a0, a1, a2, a3, a4, a5, a6} R.RGlyph{b0, b1, b2, b3, b4, b5, b6}:
      Bool.and(U32.is_eq(a0, b0), Bool.and(U32.is_eq(a1, b1), Bool.and(U32.is_eq(a2, b2), Bool.and(U32.is_eq(a3, b3), Bool.and(U32.is_eq(a4, b4), Bool.and(Nat.is_eq(a5, b5), Eq.mask(a6, b6)))))))
    case R.RKey{a0, a1, a2, a3, a4} R.RKey{b0, b1, b2, b3, b4}:
      Bool.and(U32.is_eq(a0, b0), Bool.and(U32.is_eq(a1, b1), Bool.and(U32.is_eq(a2, b2), Bool.and(U32.is_eq(a3, b3), U32.is_eq(a4, b4)))))
    case R.RPair{a0, a1, a2, a3, a4, a5} R.RPair{b0, b1, b2, b3, b4, b5}:
      Bool.and(U32.is_eq(a0, b0), Bool.and(U32.is_eq(a1, b1), Bool.and(U32.is_eq(a2, b2), Bool.and(U32.is_eq(a3, b3), Bool.and(Eq.run(a4, b4), Eq.run(a5, b5))))))
    case _ _:
      False{}

# two glyph tables alike (a frame's tiles are kept only under the table
# they were drawn with: law frame_again_exact)
def Eq.tab(a: R.Tab, b: R.Tab) -> Bool:
  match a b:
    case R.TNil{} R.TNil{}:
      True{}
    case R.TBr{a0, a1, a2, a3, a4} R.TBr{b0, b1, b2, b3, b4}:
      Bool.and(U32.is_eq(a0, b0), Bool.and(Nat.is_eq(a1, b1), Bool.and(Eq.mask(a2, b2), Bool.and(Eq.tab(a3, b3), Eq.tab(a4, b4)))))
    case _ _:
      False{}

def Eq.glyph(a: F.Glyph, b: F.Glyph) -> Bool:
  match a b:
    case F.Glyph{a0, a1, a2, a3} F.Glyph{b0, b1, b2, b3}:
      Bool.and(U32.is_eq(a0, b0), Bool.and(U32.is_eq(a1, b1), Bool.and(U32.is_eq(a2, b2), U32.is_eq(a3, b3))))

def Eq.shape(a: R.Shape, b: R.Shape) -> Bool:
  match a b:
    case R.SRect{} R.SRect{}:
      True{}
    case R.SPoly{ai, ae} R.SPoly{bi, be}:
      Bool.and(Q.Eq.bool(ai, bi), Eq.edges(ae, be))
    case R.SGlyph{ax, ay, ag} R.SGlyph{bx, by, bg}:
      Bool.and(U32.is_eq(ax, bx), Bool.and(U32.is_eq(ay, by), Eq.glyph(ag, bg)))
    case R.SSeg{a0, a1, a2, a3, a4} R.SSeg{b0, b1, b2, b3, b4}:
      Bool.and(Eq.f32(a0, b0), Bool.and(Eq.f32(a1, b1), Bool.and(Eq.f32(a2, b2), Bool.and(Eq.f32(a3, b3), Eq.f32(a4, b4)))))
    case R.SDisc{a0, a1, a2} R.SDisc{b0, b1, b2}:
      Bool.and(Eq.f32(a0, b0), Bool.and(Eq.f32(a1, b1), Eq.f32(a2, b2)))
    case R.STri{a0, a1, a2, a3, a4, a5} R.STri{b0, b1, b2, b3, b4, b5}:
      Bool.and(Eq.f32(a0, b0), Bool.and(Eq.f32(a1, b1), Bool.and(Eq.f32(a2, b2), Bool.and(Eq.f32(a3, b3), Bool.and(Eq.f32(a4, b4), Eq.f32(a5, b5))))))
    case R.SMask{ax, ay, aw, ad, am} R.SMask{bx, by, bw, bd, bm}:
      Bool.and(U32.is_eq(ax, bx), Bool.and(U32.is_eq(ay, by), Bool.and(U32.is_eq(aw, bw), Bool.and(Nat.is_eq(ad, bd), Eq.mask(am, bm)))))
    case R.SRun{ax, ay, ar} R.SRun{bx, by, br}:
      Bool.and(U32.is_eq(ax, bx), Bool.and(U32.is_eq(ay, by), Eq.run(ar, br)))
    case _ _:
      False{}

def Eq.op(a: R.Op, b: R.Op) -> Bool:
  match a b:
    case R.Op{a0, a1, a2, a3, ac, aa, as} R.Op{b0, b1, b2, b3, bc, ba, bs}:
      Bool.and(U32.is_eq(a0, b0), Bool.and(U32.is_eq(a1, b1), Bool.and(U32.is_eq(a2, b2), Bool.and(U32.is_eq(a3, b3),
        Bool.and(U32.is_eq(ac, bc), Bool.and(U32.is_eq(aa, ba), Eq.shape(as, bs)))))))

# ok: the ops so far were equal; the walk stops at the first that is not
# (a frame's changed regions differ early, and are compared that far)
def Eq.ops.go(xs: List<&2, R.Op>, ys: List<&2, R.Op>, ok: Bool) -> Bool:
  match xs ys ok:
    case _ _ False{}:
      False{}
    case Nil{} Nil{} True{}:
      True{}
    case x <> xt y <> yt True{}:
      Eq.ops.go(xt, yt, Eq.op(x, y))
    case _ _ True{}:
      False{}

def Eq.ops(xs: List<&2, R.Op>, ys: List<&2, R.Op>) -> Bool:
  Eq.ops.go(xs, ys, True{})

# ok: the quadrants so far were equal; once one is not, the rest are not
# walked (a viewer's base that moved differs in its first pixels)
def Eq.image.go(a: Image, b: Image, ok: Bool) -> Bool:
  match a b ok:
    case Pix{x} Pix{y} True{}:
      U32.is_eq(x, y)
    case Qua{a0, a1, a2, a3} Qua{b0, b1, b2, b3} True{}:
      Eq.image.go(a3, b3, Eq.image.go(a2, b2, Eq.image.go(a1, b1, Eq.image.go(a0, b0, True{}))))
    case _ _ _:
      False{}

def Eq.image(a: Image, b: Image) -> Bool:
  Eq.image.go(a, b, True{})

def Eq.tile.k(same: Bool, a: Image, b: Image) -> Bool:
  match same:
    case True{}:
      Eq.image(a, b)
    case False{}:
      False{}

# the same ops over the same base; the bases (big, for a viewer's) are
# compared only when the ops are the same
def Eq.tile(xs: List<&2, R.Op>, ys: List<&2, R.Op>, a: Image, b: Image) -> Bool:
  Eq.tile.k(Eq.ops(xs, ys), a, b)

# Tiles
# -----
# A tile is drawn in its own coordinates: its ops moved so its corner is
# (0, 0). Two tiles with the same local ops over the same base are the
# same image wherever they sit, so when the view scrolls by whole tiles a
# tile takes the image the last frame drew one scroll step away.

# Local ops
# ---------

def Loc.f(+v: F32, +o: F32) -> F32:
  F32.sub(v, o)

def Loc.edge(e: R.Edge, +fx: F32, +fy: F32) -> R.Edge:
  match e:
    case R.Edge{ax, ay, bx, by}:
      R.Edge{Loc.f(ax, fx), Loc.f(ay, fy), Loc.f(bx, fx), Loc.f(by, fy)}

def Loc.edges(es: List<&2, R.Edge>, +fx: F32, +fy: F32) -> List<&2, R.Edge>:
  match es:
    case Nil{}:
      Nil{}
    case e <> t:
      Loc.edge(e, fx, fy) <> Loc.edges(t, fx, fy)

# glyph corners wrap below 0: pixel lookups subtract them again, modulo
# 2^32, so they land on the same bits
def Loc.shape(s: R.Shape, +ox: U32, +oy: U32) -> R.Shape:
  match s:
    case R.SRect{}:
      R.SRect{}
    case R.SPoly{i, es}:
      R.SPoly{i, Loc.edges(es, U32.to_f32(ox), U32.to_f32(oy))}
    case R.SGlyph{gx, gy, g}:
      R.SGlyph{(gx - ox : U32), (gy - oy : U32), g}
    case R.SSeg{ax, ay, bx, by, hw}:
      R.SSeg{Loc.f(ax, U32.to_f32(ox)), Loc.f(ay, U32.to_f32(oy)), Loc.f(bx, U32.to_f32(ox)), Loc.f(by, U32.to_f32(oy)), hw}
    case R.SDisc{cx, cy, r}:
      R.SDisc{Loc.f(cx, U32.to_f32(ox)), Loc.f(cy, U32.to_f32(oy)), r}
    case R.STri{ax, ay, bx, by, cx, cy}:
      R.STri{Loc.f(ax, U32.to_f32(ox)), Loc.f(ay, U32.to_f32(oy)), Loc.f(bx, U32.to_f32(ox)), Loc.f(by, U32.to_f32(oy)), Loc.f(cx, U32.to_f32(ox)), Loc.f(cy, U32.to_f32(oy))}
    case R.SMask{gx, gy, w, d, m}:
      R.SMask{(gx - ox : U32), (gy - oy : U32), w, d, m}
    case R.SRun{rx, ry, r}:
      R.SRun{(rx - ox : U32), (ry - oy : U32), r}

# a box edge cut to [o, o + n], from o
def Loc.lo(+v: U32, +o: U32, +n: U32) -> U32:
  (U32.min(U32.max(v, o), (o + n : U32)) - o : U32)

def Loc.op(o: R.Op, +ox: U32, +oy: U32, +n: U32) -> R.Op:
  match o:
    case R.Op{x0, y0, x1, y1, c, a, s}:
      R.Op{Loc.lo(x0, ox, n), Loc.lo(y0, oy, n), Loc.lo(x1, ox, n), Loc.lo(y1, oy, n), c, a, Loc.shape(s, ox, oy)}

# the ops of the n x n tile at (ox, oy), in its own coordinates
def Loc.ops(ops: List<&2, R.Op>, +ox: U32, +oy: U32, +n: U32) -> List<&2, R.Op>:
  match ops:
    case Nil{}:
      Nil{}
    case o <> t:
      Loc.op(o, ox, oy, n) <> Loc.ops(t, ox, oy, n)

# Tiles
# -----

# ops as they reach the tile, the same in its own coordinates, the base
# under it and the image
type Tile is Data:
  Tile{ops: List<&2, R.Op>, loc: List<&2, R.Op>, base: Image, img: Image}

# a node keeps the ops that reached it and the base under it: a node whose
# ops and base are unchanged is kept whole, its tiles not looked at
type Tiles is Data:
  TLeaf{t: Tile}
  TNode{ops: List<&2, R.Op>, base: Image, a: Tiles, b: Tiles, c: Tiles, d: Tiles}

# the last frame's tiles (k levels over tiles, n wide from (0, 0)) and how
# far its content moved down since, modulo 2^32 (0: not at all)
type Past is Data:
  Past{k: Nat, t: Tiles, n: U32, dy: U32}

# quadrant i (0 top-left, 1 top-right, 2 bottom-left, 3 bottom-right) of
# an image (a solid one is solid in each)
def Img.q(img: Image, i: Nat) -> Image:
  match img i:
    case Pix{c} _:
      Pix{c}
    case Qua{a, _, _, _} 0n:
      a
    case Qua{_, b, _, _} 1n:
      b
    case Qua{_, _, c, _} 2n:
      c
    case Qua{_, _, _, d} _:
      d

def Tile.size(+e: Nat) -> U32:
  U32.shln(1, e)

def Tile.draw.r(+e: Nat, +l: List<&2, R.Op>, +base: Image) -> Image:
  R.Region.node(e, R.Region.leaf(l, 0, 0, Tile.size(e)), l, 0, 0, Tile.size(e), base)

# a 2^e tile of local ops over base, its text drawn from the glyph table
def Tile.draw(+e: Nat, +tab: R.Tab, +l: List<&2, R.Op>, +base: Image) -> Image:
  Tile.draw.r(e, R.Res.ops(l, tab), base)

def Tile.of(+e: Nat, +tab: R.Tab, +ops: List<&2, R.Op>, +l: List<&2, R.Op>, +base: Image) -> Tile:
  Tile{ops, l, base, Tile.draw(e, tab, l, base)}

# the tile at (rx, ry), 2^e wide, drawn from scratch
def Tile.make(+e: Nat, +tab: R.Tab, +ops: List<&2, R.Op>, +base: Image, +rx: U32, +ry: U32, +n: U32) -> Tile:
  Tile.of(e, tab, ops, Loc.ops(ops, rx, ry, Tile.size(e)), base)

def Tiles.pick(right: Bool, down: Bool, a: Tiles, b: Tiles, c: Tiles, d: Tiles) -> Tiles:
  match right down:
    case False{} False{}:
      a
    case True{} False{}:
      b
    case False{} True{}:
      c
    case True{} True{}:
      d

def Tiles.seek.k(same: Bool, img: Image, +e: Nat, +tab: R.Tab, +l: List<&2, R.Op>, +base: Image) -> Image:
  match same:
    case True{}:
      img
    case False{}:
      Tile.draw(e, tab, l, base)

def Tiles.seek.leaf(t: Tile, +e: Nat, +tab: R.Tab, +l: List<&2, R.Op>, +base: Image) -> Image:
  match t:
    case Tile{_, +o, +b, i}:
      Tiles.seek.k(Eq.tile(o, l, b, base), i, e, tab, l, base)

# the image of a tile of local ops l over base: the one the old tile at
# (x, y) holds when it drew the same, else drawn (law seek_made)
def Tiles.seek(k: Nat, t: Tiles, +x: U32, +y: U32, +rx: U32, +ry: U32, +n: U32, +e: Nat, +tab: R.Tab, +l: List<&2, R.Op>, +base: Image) -> Image:
  match k t:
    case 0n TLeaf{tl}:
      Tiles.seek.leaf(tl, e, tab, l, base)
    case 1n+j0 TNode{_, _, a, b, c, d}:
      +j = j0
      Tiles.seek(j, Tiles.pick(U32.is_ge(x, (rx + U32.shr(n) : U32)), U32.is_ge(y, (ry + U32.shr(n) : U32)), a, b, c, d), x, y,
        Bool.pick(U32, U32.is_ge(x, (rx + U32.shr(n) : U32)), (rx + U32.shr(n) : U32), rx), Bool.pick(U32, U32.is_ge(y, (ry + U32.shr(n) : U32)), (ry + U32.shr(n) : U32), ry), U32.shr(n), e, tab, l, base)
    case _ _:
      Tile.draw(e, tab, l, base)

def Tile.fresh.k(look: Bool, +e: Nat, +tab: R.Tab, +ops: List<&2, R.Op>, +l: List<&2, R.Op>, +base: Image, p: Past, +rx: U32, +ry: U32) -> Tile:
  match look p:
    case True{} Past{pk, pt, pn, dy}:
      Tile{ops, l, base, Tiles.seek(pk, pt, rx, (ry - dy : U32), 0, 0, pn, e, tab, l, base)}
    case False{} _:
      Tile{ops, l, base, Tile.draw(e, tab, l, base)}

# a changed tile: when the view moved, the old tile its content came from
# may hold it already
def Tile.fresh(+e: Nat, +tab: R.Tab, +ops: List<&2, R.Op>, +l: List<&2, R.Op>, +base: Image, p: Past, +rx: U32, +ry: U32) -> Tile:
  match p:
    case Past{pk, pt, pn, +dy}:
      Tile.fresh.k(U32.is_ne(dy, 0), e, tab, ops, l, base, Past{pk, pt, pn, dy}, rx, ry)

def Tile.again.k(same: Bool, +e: Nat, +tab: R.Tab, +ops: List<&2, R.Op>, +base: Image, t: Tile, p: Past, +rx: U32, +ry: U32, +n: U32) -> Tile:
  match same:
    case True{}:
      t
    case False{}:
      Tile.fresh(e, tab, ops, Loc.ops(ops, rx, ry, Tile.size(e)), base, p, rx, ry)

# the same ops over the same base keep the tile; else it is drawn again
# (or found where the view moved it from)
def Tile.again(+e: Nat, +tab: R.Tab, +ops: List<&2, R.Op>, +base: Image, t: Tile, p: Past, +rx: U32, +ry: U32, +n: U32) -> Tile:
  match t:
    case Tile{+o, l, +b, i}:
      Tile.again.k(Eq.tile(ops, o, base, b), e, tab, ops, base, Tile{o, l, b, i}, p, rx, ry, n)

# the n x n region at (rx, ry) as tiles k levels down; each tile is 2^e
# wide. ops reach each child as Region.keep cuts them (the parent's
# corner is the region's own)
def Tiles.make(k: Nat, +e: Nat, +tab: R.Tab, +ops: List<&2, R.Op>, +base: Image, +rx: U32, +ry: U32, +n: U32) -> Tiles:
  match k:
    case 0n:
      TLeaf{Tile.make(e, tab, ops, base, rx, ry, n)}
    case 1n+j0:
      +j = j0
      # the four quadrants at once (a first frame, or one after a resize)
      a b c d = Tiles.make(j, e, tab, R.Region.keep(ops, rx, ry, rx, ry, U32.shr(n)), Img.q(base, 0n), rx, ry, U32.shr(n)) Tiles.make(j, e, tab, R.Region.keep(ops, rx, ry, (rx + U32.shr(n) : U32), ry, U32.shr(n)), Img.q(base, 1n), (rx + U32.shr(n) : U32), ry, U32.shr(n)) Tiles.make(j, e, tab, R.Region.keep(ops, rx, ry, rx, (ry + U32.shr(n) : U32), U32.shr(n)), Img.q(base, 2n), rx, (ry + U32.shr(n) : U32), U32.shr(n)) Tiles.make(j, e, tab, R.Region.keep(ops, rx, ry, (rx + U32.shr(n) : U32), (ry + U32.shr(n) : U32), U32.shr(n)), Img.q(base, 3n), (rx + U32.shr(n) : U32), (ry + U32.shr(n) : U32), U32.shr(n))
      TNode{ops, base, a, b, c, d}

# the last frame's node holds the same ops over the same base
def Tiles.same(+ops: List<&2, R.Op>, +base: Image, t: Tiles) -> Bool:
  match t:
    case TLeaf{_}:
      False{}
    case TNode{o, b, _, _, _, _}:
      Eq.tile(ops, o, base, b)

# the next frame over the last one's tiles: an unchanged node is kept
# whole (same: Tiles.same of it), an unchanged tile is kept, the rest is
# drawn again or taken from the past; the four quadrants run at once
def Tiles.again.go(+k: Nat, +e: Nat, +tab: R.Tab, +ops: List<&2, R.Op>, +base: Image, prev: Tiles, +p: Past, +rx: U32, +ry: U32, +n: U32, same: Bool) -> Tiles:
  match k prev same:
    case 0n TLeaf{t} _:
      TLeaf{Tile.again(e, tab, ops, base, t, p, rx, ry, n)}
    case 1n+j0 TNode{o, b, pa, pb, pc, pd} True{}:
      TNode{o, b, pa, pb, pc, pd}
    case 1n+j0 TNode{_, _, +pa, +pb, +pc, +pd} False{}:
      +j = j0
      +h = U32.shr(n)
      +oa = R.Region.keep(ops, rx, ry, rx, ry, h)
      +ob = R.Region.keep(ops, rx, ry, (rx + h : U32), ry, h)
      +oc = R.Region.keep(ops, rx, ry, rx, (ry + h : U32), h)
      +od = R.Region.keep(ops, rx, ry, (rx + h : U32), (ry + h : U32), h)
      a b c d = Tiles.again.go(j, e, tab, oa, Img.q(base, 0n), pa, p, rx, ry, h, Tiles.same(oa, Img.q(base, 0n), pa)) Tiles.again.go(j, e, tab, ob, Img.q(base, 1n), pb, p, (rx + h : U32), ry, h, Tiles.same(ob, Img.q(base, 1n), pb)) Tiles.again.go(j, e, tab, oc, Img.q(base, 2n), pc, p, rx, (ry + h : U32), h, Tiles.same(oc, Img.q(base, 2n), pc)) Tiles.again.go(j, e, tab, od, Img.q(base, 3n), pd, p, (rx + h : U32), (ry + h : U32), h, Tiles.same(od, Img.q(base, 3n), pd))
      TNode{ops, base, a, b, c, d}
    case _ _ _:
      Tiles.make(k, e, tab, ops, base, rx, ry, n)

def Tiles.again(+k: Nat, +e: Nat, +tab: R.Tab, +ops: List<&2, R.Op>, +base: Image, +prev: Tiles, +p: Past, +rx: U32, +ry: U32, +n: U32) -> Tiles:
  Tiles.again.go(k, e, tab, ops, base, prev, p, rx, ry, n, Tiles.same(ops, base, prev))

# the frame the tiles make
def Tiles.image(t: Tiles) -> Image:
  match t:
    case TLeaf{Tile{_, _, _, i}}:
      i
    case TNode{_, _, a, b, c, d}:
      Qua{Tiles.image(a), Tiles.image(b), Tiles.image(c), Tiles.image(d)}

# Frames
# ------

# tiles are 2^6 = 64 pixels wide (32 cut a 1400 x 900 frame into so many
# that comparing them cost more than the redraws they saved). A wheel notch
# (nui.bend's Nui.notch) is one tile, so a scroll finds its tiles where
# they were: change both together
def Frame.tile() -> Nat:
  6n

# a frame's tiles, the size they cover and the glyph table they were drawn
# with (none yet: an empty cache)
type Frames is Data:
  FNone{}
  FSome{w: U32, h: U32, tab: R.Tab, tiles: Tiles}

# levels from the root to the tiles, for a w x h frame
def Frame.levels(+w: U32, +h: U32) -> Nat:
  Nat.sub(R.Frame.depth(w, h), Frame.tile())

def Frame.edge(+w: U32, +h: U32) -> Nat:
  Nat.sub(R.Frame.depth(w, h), Frame.levels(w, h))

# A frame's tiles from scratch. Layout already keeps text by glyph ids;
# mask-bearing ops remain valid too and resolve unchanged.
def Frame.make(+w: U32, +h: U32, +ops: List<&2, R.Op>, +base: Image, +tab: R.Tab) -> Tiles:
  Tiles.make(Frame.levels(w, h), Frame.edge(w, h), tab, ops, base, 0, 0, U32.shln(1, R.Frame.depth(w, h)))

# Layout supplies glyph keys directly.
def Frame.again.k(same: Bool, +w: U32, +h: U32, +ops: List<&2, R.Op>, +base: Image, +tab: R.Tab, +prev: Tiles, +dy: U32) -> Tiles:
  match same:
    case True{}:
      Tiles.again(Frame.levels(w, h), Frame.edge(w, h), tab, ops, base, prev, Past{Frame.levels(w, h), prev, U32.shln(1, R.Frame.depth(w, h)), dy}, 0, 0, U32.shln(1, R.Frame.depth(w, h)))
    case False{}:
      Tiles.make(Frame.levels(w, h), Frame.edge(w, h), tab, ops, base, 0, 0, U32.shln(1, R.Frame.depth(w, h)))

# the same size, drawn with the same glyph table
def Frame.same(+w: U32, +h: U32, +tab: R.Tab, +pw: U32, +ph: U32, +ptab: R.Tab) -> Bool:
  Bool.and(Bool.and(U32.is_eq(pw, w), U32.is_eq(ph, h)), Eq.tab(tab, ptab))

# a w x h frame of ops (topmost first) over base, reusing the last frame's
# tiles when it had the same size and glyph table; dy is how far the
# content moved down since (a hint: tiles are only taken when they drew
# exactly the same)
def Frame.again(+w: U32, +h: U32, +ops: List<&2, R.Op>, +base: Image, +tab: R.Tab, prev: Frames, +dy: U32) -> Tiles:
  match prev:
    case FNone{}:
      Frame.make(w, h, ops, base, tab)
    case FSome{+pw, +ph, +ptab, t}:
      Frame.again.k(Frame.same(w, h, tab, pw, ph, ptab), w, h, ops, base, tab, t, dy)

# Drawing in parallel
# -------------------
# A fork's work stays on the core it was dealt to, and Tiles.again forks by
# quadrant: the tiles that change (a thread's text, in a narrow column) sit
# in few quadrants, so their drawing ran on one or two cores. Frame.par
# makes the same decisions as Frame.again without drawing (a plan), deals
# the tiles to draw into a balanced tree (alternate ones to each side, so
# neighbours land apart), draws them in parallel, and fills the plan from
# what they made. Each tile checks that what it takes was drawn from its
# own local ops and base (else it draws them itself), so the frame is
# exactly Frame.again's (law frame_par_exact) whatever the dealing did.

# a tile: kept as it is, or to draw (its ops, local ops and base); a node
# counts the tiles to draw under it
type Plan is Data:
  PKeep{t: Tile}
  PDraw{ops: List<&2, R.Op>, l: List<&2, R.Op>, base: Image}
  PNode{n: Nat, ops: List<&2, R.Op>, base: Image, a: Plan, b: Plan, c: Plan, d: Plan}
  PKeepN{t: Tiles}

def Plan.count(p: Plan) -> Nat:
  match p:
    case PKeep{_}:
      0n
    case PDraw{_, _, _}:
      1n
    case PNode{n, _, _, _, _, _, _}:
      n
    case PKeepN{_}:
      0n

def Plan.node(+ops: List<&2, R.Op>, +base: Image, +a: Plan, +b: Plan, +c: Plan, +d: Plan) -> Plan:
  PNode{Nat.add(Nat.add(Plan.count(a), Plan.count(b)), Nat.add(Plan.count(c), Plan.count(d))), ops, base, a, b, c, d}

# Tiles.seek without the drawing: the old image when it drew the same
def Tiles.find.k(same: Bool, img: Image) -> Maybe<&2, Image>:
  match same:
    case True{}:
      Some{img}
    case False{}:
      None{}

def Tiles.find.leaf(t: Tile, +l: List<&2, R.Op>, +base: Image) -> Maybe<&2, Image>:
  match t:
    case Tile{_, +o, +b, i}:
      Tiles.find.k(Eq.tile(o, l, b, base), i)

def Tiles.find(k: Nat, t: Tiles, +x: U32, +y: U32, +rx: U32, +ry: U32, +n: U32, +l: List<&2, R.Op>, +base: Image) -> Maybe<&2, Image>:
  match k t:
    case 0n TLeaf{tl}:
      Tiles.find.leaf(tl, l, base)
    case 1n+j0 TNode{_, _, a, b, c, d}:
      +j = j0
      Tiles.find(j, Tiles.pick(U32.is_ge(x, (rx + U32.shr(n) : U32)), U32.is_ge(y, (ry + U32.shr(n) : U32)), a, b, c, d), x, y,
        Bool.pick(U32, U32.is_ge(x, (rx + U32.shr(n) : U32)), (rx + U32.shr(n) : U32), rx), Bool.pick(U32, U32.is_ge(y, (ry + U32.shr(n) : U32)), (ry + U32.shr(n) : U32), ry), U32.shr(n), l, base)
    case _ _:
      None{}

# a tile found where the view moved it from is kept; one not found is drawn
def Tile.plan.found(m: Maybe<&2, Image>, +ops: List<&2, R.Op>, +l: List<&2, R.Op>, +base: Image) -> Plan:
  match m:
    case Some{i}:
      PKeep{Tile{ops, l, base, i}}
    case None{}:
      PDraw{ops, l, base}

def Tile.plan.fresh.k(look: Bool, +e: Nat, +ops: List<&2, R.Op>, +l: List<&2, R.Op>, +base: Image, p: Past, +rx: U32, +ry: U32) -> Plan:
  match look p:
    case True{} Past{pk, pt, pn, dy}:
      Tile.plan.found(Tiles.find(pk, pt, rx, (ry - dy : U32), 0, 0, pn, l, base), ops, l, base)
    case False{} _:
      PDraw{ops, l, base}

def Tile.plan.fresh(+e: Nat, +ops: List<&2, R.Op>, +l: List<&2, R.Op>, +base: Image, p: Past, +rx: U32, +ry: U32) -> Plan:
  match p:
    case Past{pk, pt, pn, +dy}:
      Tile.plan.fresh.k(U32.is_ne(dy, 0), e, ops, l, base, Past{pk, pt, pn, dy}, rx, ry)

def Tile.plan.k(same: Bool, +e: Nat, +ops: List<&2, R.Op>, +base: Image, t: Tile, p: Past, +rx: U32, +ry: U32, +n: U32) -> Plan:
  match same:
    case True{}:
      PKeep{t}
    case False{}:
      Tile.plan.fresh(e, ops, Loc.ops(ops, rx, ry, Tile.size(e)), base, p, rx, ry)

# Tile.again's decision, the drawing left for later
def Tile.plan(+e: Nat, +ops: List<&2, R.Op>, +base: Image, t: Tile, p: Past, +rx: U32, +ry: U32, +n: U32) -> Plan:
  match t:
    case Tile{+o, l, +b, i}:
      Tile.plan.k(Eq.tile(ops, o, base, b), e, ops, base, Tile{o, l, b, i}, p, rx, ry, n)

# Tiles.make's plan: every tile to draw
def Tiles.plan.make(k: Nat, +e: Nat, +ops: List<&2, R.Op>, +base: Image, +rx: U32, +ry: U32, +n: U32) -> Plan:
  match k:
    case 0n:
      PDraw{ops, Loc.ops(ops, rx, ry, Tile.size(e)), base}
    case 1n+j0:
      +j = j0
      Plan.node(ops, base, Tiles.plan.make(j, e, R.Region.keep(ops, rx, ry, rx, ry, U32.shr(n)), Img.q(base, 0n), rx, ry, U32.shr(n)),
        Tiles.plan.make(j, e, R.Region.keep(ops, rx, ry, (rx + U32.shr(n) : U32), ry, U32.shr(n)), Img.q(base, 1n), (rx + U32.shr(n) : U32), ry, U32.shr(n)),
        Tiles.plan.make(j, e, R.Region.keep(ops, rx, ry, rx, (ry + U32.shr(n) : U32), U32.shr(n)), Img.q(base, 2n), rx, (ry + U32.shr(n) : U32), U32.shr(n)),
        Tiles.plan.make(j, e, R.Region.keep(ops, rx, ry, (rx + U32.shr(n) : U32), (ry + U32.shr(n) : U32), U32.shr(n)), Img.q(base, 3n), (rx + U32.shr(n) : U32), (ry + U32.shr(n) : U32), U32.shr(n)))

# Tiles.again's plan
def Tiles.plan.go(+k: Nat, +e: Nat, +ops: List<&2, R.Op>, +base: Image, prev: Tiles, +p: Past, +rx: U32, +ry: U32, +n: U32, same: Bool) -> Plan:
  match k prev same:
    case 0n TLeaf{t} _:
      Tile.plan(e, ops, base, t, p, rx, ry, n)
    case 1n+j0 TNode{o, b, pa, pb, pc, pd} True{}:
      PKeepN{TNode{o, b, pa, pb, pc, pd}}
    case 1n+j0 TNode{_, _, +pa, +pb, +pc, +pd} False{}:
      +j = j0
      +h = U32.shr(n)
      +oa = R.Region.keep(ops, rx, ry, rx, ry, h)
      +ob = R.Region.keep(ops, rx, ry, (rx + h : U32), ry, h)
      +oc = R.Region.keep(ops, rx, ry, rx, (ry + h : U32), h)
      +od = R.Region.keep(ops, rx, ry, (rx + h : U32), (ry + h : U32), h)
      Plan.node(ops, base, Tiles.plan.go(j, e, oa, Img.q(base, 0n), pa, p, rx, ry, h, Tiles.same(oa, Img.q(base, 0n), pa)),
        Tiles.plan.go(j, e, ob, Img.q(base, 1n), pb, p, (rx + h : U32), ry, h, Tiles.same(ob, Img.q(base, 1n), pb)),
        Tiles.plan.go(j, e, oc, Img.q(base, 2n), pc, p, rx, (ry + h : U32), h, Tiles.same(oc, Img.q(base, 2n), pc)),
        Tiles.plan.go(j, e, od, Img.q(base, 3n), pd, p, (rx + h : U32), (ry + h : U32), h, Tiles.same(od, Img.q(base, 3n), pd)))
    case _ _ _:
      Tiles.plan.make(k, e, ops, base, rx, ry, n)

def Tiles.plan(+k: Nat, +e: Nat, +ops: List<&2, R.Op>, +base: Image, +prev: Tiles, +p: Past, +rx: U32, +ry: U32, +n: U32) -> Plan:
  Tiles.plan.go(k, e, ops, base, prev, p, rx, ry, n, Tiles.same(ops, base, prev))

# Plans that build only the changed nodes' ops
# --------------------------------------------
# Tiles.plan built every child's ops (Region.keep, four lists a node) before
# comparing them with the last frame's, so an unchanged child cost a list
# made only to be dropped. Here a child's ops are compared as they reach it,
# straight from its parent's (Keep.eq, law keep_eq: exactly Eq.ops of the
# list Region.keep would build), and only a changed child's list is built.
# The plan is Tiles.plan's (law plan_cut_same), so frame_par_exact holds.

# a node's ops: these, or those of these that reach the region at (rx, ry)
# whose parent's corner is (px, py) (Region.keep's, not built yet)
type Cut is Data:
  CAll{ops: List<&2, R.Op>}
  CIn{ops: List<&2, R.Op>, px: U32, py: U32, rx: U32, ry: U32, n: U32}

def Cut.get(c: Cut) -> List<&2, R.Op>:
  match c:
    case CAll{o}:
      o
    case CIn{o, +px, +py, +rx, +ry, +n}:
      R.Region.keep(o, px, py, rx, ry, n)

# Eq.op of an op given by its fields
def Eq.op.f(+a0: U32, +a1: U32, +a2: U32, +a3: U32, +ac: U32, +aa: U32, as: R.Shape, b: R.Op) -> Bool:
  match b:
    case R.Op{b0, b1, b2, b3, bc, ba, bs}:
      Bool.and(U32.is_eq(a0, b0), Bool.and(U32.is_eq(a1, b1), Bool.and(U32.is_eq(a2, b2), Bool.and(U32.is_eq(a3, b3),
        Bool.and(U32.is_eq(ac, bc), Bool.and(U32.is_eq(aa, ba), Eq.shape(as, bs)))))))

def Keep.drop1(ys: List<&2, R.Op>) -> List<&2, R.Op>:
  match ys:
    case Nil{}:
      Nil{}
    case _ <> t:
      t

# kept: the op reaches the region and draws there (Region.keep.k's flag)
def Keep.ys.k(kept: Bool, ys: List<&2, R.Op>) -> List<&2, R.Op>:
  match kept:
    case True{}:
      Keep.drop1(ys)
    case False{}:
      ys

def Keep.ys.h(hit: Bool, +s: R.Shape, +px: U32, +py: U32, +rx: U32, +ry: U32, +n: U32, ys: List<&2, R.Op>) -> List<&2, R.Op>:
  match hit:
    case True{}:
      Keep.ys.k(Bool.not(R.Region.empty(R.Region.move(s, px, py, rx, ry, n))), ys)
    case False{}:
      ys

# the last frame's ops left to compare after op o
def Keep.ys(o: R.Op, +px: U32, +py: U32, +rx: U32, +ry: U32, +n: U32, ys: List<&2, R.Op>) -> List<&2, R.Op>:
  match o:
    case R.Op{+x0, +y0, +x1, +y1, _, _, +s}:
      Keep.ys.h(R.Region.hits(x0, y0, x1, y1, rx, ry, n), s, px, py, rx, ry, n, ys)

def Keep.ok.c(+x0: U32, +y0: U32, +x1: U32, +y1: U32, +c: U32, +a: U32, s2: R.Shape, ys: List<&2, R.Op>) -> Bool:
  match ys:
    case Nil{}:
      False{}
    case y <> _:
      Eq.op.f(x0, y0, x1, y1, c, a, s2, y)

def Keep.ok.k(kept: Bool, +x0: U32, +y0: U32, +x1: U32, +y1: U32, +c: U32, +a: U32, s2: R.Shape, ys: List<&2, R.Op>) -> Bool:
  match kept:
    case True{}:
      Keep.ok.c(x0, y0, x1, y1, c, a, s2, ys)
    case False{}:
      True{}

def Keep.ok.h(hit: Bool, +x0: U32, +y0: U32, +x1: U32, +y1: U32, +c: U32, +a: U32, +s: R.Shape, +px: U32, +py: U32, +rx: U32, +ry: U32, +n: U32, ys: List<&2, R.Op>) -> Bool:
  match hit:
    case True{}:
      +s2 = R.Region.move(s, px, py, rx, ry, n)
      Keep.ok.k(Bool.not(R.Region.empty(s2)), x0, y0, x1, y1, c, a, s2, ys)
    case False{}:
      True{}

# op o, as it reaches the region, is the next of the last frame's (or does
# not reach it)
def Keep.ok(o: R.Op, +px: U32, +py: U32, +rx: U32, +ry: U32, +n: U32, ys: List<&2, R.Op>) -> Bool:
  match o:
    case R.Op{+x0, +y0, +x1, +y1, +c, +a, +s}:
      Keep.ok.h(R.Region.hits(x0, y0, x1, y1, rx, ry, n), x0, y0, x1, y1, c, a, s, px, py, rx, ry, n, ys)

# Eq.ops.go(Region.keep(ops, px, py, rx, ry, n), ys, ok) without building
# the list (law keep_eq); it stops at the first op that differs
def Keep.eq(ops: List<&2, R.Op>, +px: U32, +py: U32, +rx: U32, +ry: U32, +n: U32, +ys: List<&2, R.Op>, ok: Bool) -> Bool:
  match ops ok:
    case _ False{}:
      False{}
    case Nil{} True{}:
      Eq.ops.go(Nil{}, ys, True{})
    case +o <> t True{}:
      Keep.eq(t, px, py, rx, ry, n, Keep.ys(o, px, py, rx, ry, n, ys), Keep.ok(o, px, py, rx, ry, n, ys))

# Eq.ops(Cut.get(c), ys) (law cut_eq)
def Cut.eq(c: Cut, +ys: List<&2, R.Op>) -> Bool:
  match c:
    case CAll{o}:
      Eq.ops(o, ys)
    case CIn{o, +px, +py, +rx, +ry, +n}:
      Keep.eq(o, px, py, rx, ry, n, ys, True{})

# Eq.tile(Cut.get(c), ys, a, b)
def Cut.tile(c: Cut, +ys: List<&2, R.Op>, a: Image, b: Image) -> Bool:
  Eq.tile.k(Cut.eq(c, ys), a, b)

# Tiles.same(Cut.get(c), base, t)
def Cut.same(c: Cut, +base: Image, t: Tiles) -> Bool:
  match t:
    case TLeaf{_}:
      False{}
    case TNode{o, b, _, _, _, _}:
      Cut.tile(c, o, base, b)

def Tile.plan2.k(same: Bool, +e: Nat, c: Cut, +base: Image, t: Tile, p: Past, +rx: U32, +ry: U32, +n: U32) -> Plan:
  match same:
    case True{}:
      PKeep{t}
    case False{}:
      +ops = Cut.get(c)
      Tile.plan.fresh(e, ops, Loc.ops(ops, rx, ry, Tile.size(e)), base, p, rx, ry)

# Tile.plan of Cut.get(c)
def Tile.plan2(+e: Nat, +c: Cut, +base: Image, t: Tile, p: Past, +rx: U32, +ry: U32, +n: U32) -> Plan:
  match t:
    case Tile{+o, l, +b, i}:
      Tile.plan2.k(Cut.tile(c, o, base, b), e, c, base, Tile{o, l, b, i}, p, rx, ry, n)

# Tiles.plan.go of Cut.get(c) (law plan2_go): a changed node builds its ops
# and hands its children theirs uncut
def Tiles.plan2.go(+k: Nat, +e: Nat, c: Cut, +base: Image, prev: Tiles, +p: Past, +rx: U32, +ry: U32, +n: U32, same: Bool) -> Plan:
  match k prev same:
    case 0n TLeaf{t} _:
      Tile.plan2(e, c, base, t, p, rx, ry, n)
    case 1n+j0 TNode{o, b, pa, pb, pc, pd} True{}:
      PKeepN{TNode{o, b, pa, pb, pc, pd}}
    case 1n+j0 TNode{_, _, +pa, +pb, +pc, +pd} False{}:
      +j = j0
      +h = U32.shr(n)
      +ops = Cut.get(c)
      Plan.node(ops, base, Tiles.plan2.go(j, e, CIn{ops, rx, ry, rx, ry, h}, Img.q(base, 0n), pa, p, rx, ry, h, Cut.same(CIn{ops, rx, ry, rx, ry, h}, Img.q(base, 0n), pa)),
        Tiles.plan2.go(j, e, CIn{ops, rx, ry, (rx + h : U32), ry, h}, Img.q(base, 1n), pb, p, (rx + h : U32), ry, h, Cut.same(CIn{ops, rx, ry, (rx + h : U32), ry, h}, Img.q(base, 1n), pb)),
        Tiles.plan2.go(j, e, CIn{ops, rx, ry, rx, (ry + h : U32), h}, Img.q(base, 2n), pc, p, rx, (ry + h : U32), h, Cut.same(CIn{ops, rx, ry, rx, (ry + h : U32), h}, Img.q(base, 2n), pc)),
        Tiles.plan2.go(j, e, CIn{ops, rx, ry, (rx + h : U32), (ry + h : U32), h}, Img.q(base, 3n), pd, p, (rx + h : U32), (ry + h : U32), h, Cut.same(CIn{ops, rx, ry, (rx + h : U32), (ry + h : U32), h}, Img.q(base, 3n), pd)))
    case _ _ _:
      Tiles.plan.make(k, e, Cut.get(c), base, rx, ry, n)

# Tiles.plan (law plan_cut_same)
def Tiles.plan2(+k: Nat, +e: Nat, +ops: List<&2, R.Op>, +base: Image, +prev: Tiles, +p: Past, +rx: U32, +ry: U32, +n: U32) -> Plan:
  Tiles.plan2.go(k, e, CAll{ops}, base, prev, p, rx, ry, n, Cut.same(CAll{ops}, base, prev))

# the tiles to draw, in the plan's order (a, b, c, d under each node)
type Job is Data:
  Job{l: List<&2, R.Op>, base: Image}

def Plan.jobs(p: Plan, acc: List<&2, Job>) -> List<&2, Job>:
  match p:
    case PKeep{_}:
      acc
    case PDraw{_, l, b}:
      Job{l, b} <> acc
    case PNode{_, _, _, a, b, c, d}:
      Plan.jobs(a, Plan.jobs(b, Plan.jobs(c, Plan.jobs(d, acc))))
    case PKeepN{_}:
      acc

# jobs dealt into a balanced tree: the i-th of a list is found by i's bits,
# lowest first (0: the even places, 1: the odd ones)
type Jobs is Data:
  JNone{}
  JOne{j: Job}
  JTwo{a: Jobs, b: Jobs}

type Halves is Data:
  Halves{even: List<&2, Job>, odd: List<&2, Job>}

def Halves.cons(x: Job, h: Halves) -> Halves:
  match h:
    case Halves{a, b}:
      Halves{x <> b, a}

def Halves.of(xs: List<&2, Job>) -> Halves:
  match xs:
    case Nil{}:
      Halves{Nil{}, Nil{}}
    case x <> t:
      Halves.cons(x, Halves.of(t))

def Jobs.deal.k(+d: Nat, h: Halves) -> Jobs:
  match d h:
    case _ Halves{Nil{}, Nil{}}:
      JNone{}
    case _ Halves{x <> Nil{}, Nil{}}:
      JOne{x}
    case 0n _:
      JNone{}
    case 1n+k Halves{a, b}:
      JTwo{Jobs.deal.k(k, Halves.of(a)), Jobs.deal.k(k, Halves.of(b))}

def Jobs.deal(xs: List<&2, Job>) -> Jobs:
  Jobs.deal.k(24n, Halves.of(xs))

# what the jobs drew, in the same tree
type Drawn is Data:
  DNone{}
  DOne{l: List<&2, R.Op>, base: Image, img: Image}
  DTwo{a: Drawn, b: Drawn}

def Jobs.run(t: Jobs, +e: Nat, +tab: R.Tab) -> Drawn:
  match t:
    case JNone{}:
      DNone{}
    case JOne{Job{+l, +b}}:
      DOne{l, b, Tile.draw(e, tab, l, b)}
    case JTwo{a, b}:
      x y = Jobs.run(a, e, tab) Jobs.run(b, e, tab)
      DTwo{x, y}

# the bits of i, lowest first (as many as a list of 2^24 needs)
def Jobs.path(+d: Nat, +i: Nat) -> List<&2, Bool>:
  match d:
    case 0n:
      Nil{}
    case 1n+k:
      Nat.is_eq(Nat.mod(i, 2n), 1n) <> Jobs.path(k, Nat.div(i, 2n))

def Drawn.take(same: Bool, img: Image, +e: Nat, +tab: R.Tab, +l: List<&2, R.Op>, +base: Image) -> Image:
  match same:
    case True{}:
      img
    case False{}:
      Tile.draw(e, tab, l, base)

# the image of local ops l over base: the one drawn where the path leads
# when it drew exactly these, else drawn here (law drawn_get_run)
def Drawn.get(t: Drawn, path: List<&2, Bool>, +e: Nat, +tab: R.Tab, +l: List<&2, R.Op>, +base: Image) -> Image:
  match t path:
    case DOne{+l2, +b2, img} _:
      Drawn.take(Eq.tile(l, l2, base, b2), img, e, tab, l, base)
    case DTwo{a, _} False{} <> r:
      Drawn.get(a, r, e, tab, l, base)
    case DTwo{_, b} True{} <> r:
      Drawn.get(b, r, e, tab, l, base)
    case _ _:
      Tile.draw(e, tab, l, base)

# the plan's tiles, the ones to draw taken from what was drawn (the first
# to draw under p is the off-th job)
def Plan.fill(p: Plan, +e: Nat, +tab: R.Tab, +off: Nat, +d: Drawn) -> Tiles:
  match p:
    case PKeep{t}:
      TLeaf{t}
    case PDraw{o, +l, +b}:
      TLeaf{Tile{o, l, b, Drawn.get(d, Jobs.path(24n, off), e, tab, l, b)}}
    case PKeepN{t}:
      t
    case PNode{_, o, bs, +a, +b, +c, +dd}:
      TNode{o, bs, Plan.fill(a, e, tab, off, d), Plan.fill(b, e, tab, Nat.add(off, Plan.count(a)), d),
        Plan.fill(c, e, tab, Nat.add(Nat.add(off, Plan.count(a)), Plan.count(b)), d),
        Plan.fill(dd, e, tab, Nat.add(Nat.add(Nat.add(off, Plan.count(a)), Plan.count(b)), Plan.count(c)), d)}

# a plan's tiles to draw, drawn in parallel
def Plan.drawn(+p: Plan, +e: Nat, +tab: R.Tab) -> Drawn:
  Jobs.run(Jobs.deal(Plan.jobs(p, Nil{})), e, tab)

# a plan carried out: its tiles to draw drawn in parallel, then filled in
# (the window takes these steps one by one: the drawing is then evaluated
# on its own, so its forks are the first dealt to the cores)
def Plan.run(+p: Plan, +e: Nat, +tab: R.Tab) -> Tiles:
  Plan.fill(p, e, tab, 0n, Plan.drawn(p, e, tab))

def Frame.plan.k(same: Bool, +w: U32, +h: U32, +ops: List<&2, R.Op>, +base: Image, +prev: Tiles, +dy: U32) -> Plan:
  match same:
    case True{}:
      Tiles.plan2(Frame.levels(w, h), Frame.edge(w, h), ops, base, prev, Past{Frame.levels(w, h), prev, U32.shln(1, R.Frame.depth(w, h)), dy}, 0, 0, U32.shln(1, R.Frame.depth(w, h)))
    case False{}:
      Tiles.plan.make(Frame.levels(w, h), Frame.edge(w, h), ops, base, 0, 0, U32.shln(1, R.Frame.depth(w, h)))

# Frame.again's plan
def Frame.plan(+w: U32, +h: U32, +ops: List<&2, R.Op>, +base: Image, +tab: R.Tab, prev: Frames, +dy: U32) -> Plan:
  match prev:
    case FNone{}:
      Tiles.plan.make(Frame.levels(w, h), Frame.edge(w, h), ops, base, 0, 0, U32.shln(1, R.Frame.depth(w, h)))
    case FSome{+pw, +ph, +ptab, t}:
      Frame.plan.k(Frame.same(w, h, tab, pw, ph, ptab), w, h, ops, base, t, dy)

# Frame.again, its drawing spread over the cores (law frame_par_exact)
def Frame.par(+w: U32, +h: U32, +ops: List<&2, R.Op>, +base: Image, +tab: R.Tab, prev: Frames, +dy: U32) -> Tiles:
  Plan.run(Frame.plan(w, h, ops, base, tab, prev, dy), Frame.edge(w, h), tab)


# Caret blinks
# ------------
# A caret op (draw.bend's Draw.caret) carries its shown alpha in its
# colour's top byte, and is drawn at that alpha or at 0. A blink changes
# nothing else, so the last frame's tiles are blinked in place: a node or
# tile none of whose ops is a caret in the other state is kept as it is,
# and the tiles under the caret are drawn again. Caret.set commutes with
# how ops reach a node and a tile (laws caret_keep, caret_loc), so each
# node and tile ends up holding what a frame drawn from the blinked ops
# would hold.

def Caret.is(+c: U32) -> Bool:
  U32.is_ge(U32.shrn(c, 24n), 128)

def Caret.alpha.k(tag: Bool, on: Bool, +c: U32, +a: U32) -> U32:
  match tag on:
    case True{} True{}:
      U32.shrn(c, 24n)
    case True{} False{}:
      0
    case False{} _:
      a

# the alpha an op has with the carets shown (on) or hidden
def Caret.alpha(+c: U32, +a: U32, +on: Bool) -> U32:
  Caret.alpha.k(Caret.is(c), on, c, a)

def Caret.op(o: R.Op, +on: Bool) -> R.Op:
  match o:
    case R.Op{x0, y0, x1, y1, +c, +a, s}:
      R.Op{x0, y0, x1, y1, c, Caret.alpha(c, a, on), s}

# the ops with every caret shown (on) or hidden
def Caret.set(ops: List<&2, R.Op>, +on: Bool) -> List<&2, R.Op>:
  match ops:
    case Nil{}:
      Nil{}
    case o <> t:
      Caret.op(o, on) <> Caret.set(t, on)

# the op is already as on says
def Caret.same(o: R.Op, +on: Bool) -> Bool:
  match o:
    case R.Op{_, _, _, _, +c, +a, _}:
      U32.is_eq(Caret.alpha(c, a, on), a)

# ok: the ops so far were as on says; the walk stops at the first that is
# not (law caret_fresh: then Caret.set changes nothing)
def Caret.fresh.go(ops: List<&2, R.Op>, +on: Bool, ok: Bool) -> Bool:
  match ops ok:
    case _ False{}:
      False{}
    case Nil{} True{}:
      True{}
    case o <> t True{}:
      Caret.fresh.go(t, on, Caret.same(o, on))

def Caret.fresh(ops: List<&2, R.Op>, +on: Bool) -> Bool:
  Caret.fresh.go(ops, on, True{})

def Tile.blink.k(fresh: Bool, +e: Nat, +tab: R.Tab, t: Tile, +on: Bool) -> Tile:
  match fresh t:
    case True{} _:
      t
    case False{} Tile{o, l, +b, _}:
      +l2 = Caret.set(l, on)
      Tile{Caret.set(o, on), l2, b, Tile.draw(e, tab, l2, b)}

def Tile.blink(t: Tile, +e: Nat, +tab: R.Tab, +on: Bool) -> Tile:
  match t:
    case Tile{+o, l, b, i}:
      Tile.blink.k(Caret.fresh(o, on), e, tab, Tile{o, l, b, i}, on)

# a node's carets are all as on says already (a leaf: Tile.blink looks)
def Tiles.fresh(t: Tiles, +on: Bool) -> Bool:
  match t:
    case TLeaf{_}:
      False{}
    case TNode{o, _, _, _, _, _}:
      Caret.fresh(o, on)

# the tiles with every caret shown (on) or hidden: a node already so is
# kept whole, and only the tiles under a caret that changed are drawn
# again (fresh: Tiles.fresh of t; law tiles_blink_exact)
def Tiles.blink.go(t: Tiles, +e: Nat, +tab: R.Tab, +on: Bool, fresh: Bool) -> Tiles:
  match t fresh:
    case TLeaf{tl} _:
      TLeaf{Tile.blink(tl, e, tab, on)}
    case TNode{o, b, ta, tb, tc, td} True{}:
      TNode{o, b, ta, tb, tc, td}
    case TNode{o, b, +ta, +tb, +tc, +td} False{}:
      a2 b2 c2 d2 = Tiles.blink.go(ta, e, tab, on, Tiles.fresh(ta, on)) Tiles.blink.go(tb, e, tab, on, Tiles.fresh(tb, on)) Tiles.blink.go(tc, e, tab, on, Tiles.fresh(tc, on)) Tiles.blink.go(td, e, tab, on, Tiles.fresh(td, on))
      TNode{Caret.set(o, on), b, a2, b2, c2, d2}

def Tiles.blink(+t: Tiles, +e: Nat, +tab: R.Tab, +on: Bool) -> Tiles:
  Tiles.blink.go(t, e, tab, on, Tiles.fresh(t, on))

# the last frame blinked (none: none)
def Frames.blink(f: Frames, +on: Bool) -> Frames:
  match f:
    case FNone{}:
      FNone{}
    case FSome{+w, +h, +tab, t}:
      FSome{w, h, tab, Tiles.blink(t, Frame.edge(w, h), tab, on)}

# Cat frames
# ----------
# The cats move at ~30 frames a second while nothing else changes. A frame
# with cats keeps their ops in front of the rest (main.bend's Catv, only
# when nothing is drawn over a cat, so the pixels are the same): its ops
# are cs then the rest. A cat frame changes only cs, so each node and tile
# is cs's part there then the rest it held: nodes whose cat ops are the
# same are kept whole, and only tiles the cats reach are drawn again (law
# tiles_recat_exact).

# a then b
def Ops.cat(a: List<&2, R.Op>, b: List<&2, R.Op>) -> List<&2, R.Op>:
  match a:
    case Nil{}:
      b
    case o <> t:
      o <> Ops.cat(t, b)

# the ops after as many as cs has (law ops_after_cat)
def Ops.after(cs: List<&2, R.Op>, o: List<&2, R.Op>) -> List<&2, R.Op>:
  match cs o:
    case Nil{} _:
      o
    case c <> ct x <> ot:
      Ops.after(ct, ot)
    case c <> ct Nil{}:
      Nil{}

def Tile.recat(+e: Nat, +tab: R.Tab, +cs: List<&2, R.Op>, +cs2: List<&2, R.Op>, t: Tile, +rx: U32, +ry: U32) -> Tile:
  match t:
    case Tile{o, _, +b, _}:
      +o2 = Ops.cat(cs2, Ops.after(cs, o))
      Tile.of(e, tab, o2, Loc.ops(o2, rx, ry, Tile.size(e)), b)

# the tiles k levels over the n x n region at (rx, ry) whose ops are cs
# then the rest, with cs2 in place of cs (same: Eq.ops(cs, cs2))
def Tiles.recat.go(k: Nat, +e: Nat, +tab: R.Tab, +cs: List<&2, R.Op>, +cs2: List<&2, R.Op>, t: Tiles, +rx: U32, +ry: U32, +n: U32, same: Bool) -> Tiles:
  match k t same:
    case _ _ True{}:
      t
    case 0n TLeaf{tl} False{}:
      TLeaf{Tile.recat(e, tab, cs, cs2, tl, rx, ry)}
    case 1n+j0 TNode{o, b, +pa, +pb, +pc, +pd} False{}:
      +j = j0
      +h = U32.shr(n)
      +ca = R.Region.keep(cs, rx, ry, rx, ry, h)
      +da = R.Region.keep(cs2, rx, ry, rx, ry, h)
      +cb = R.Region.keep(cs, rx, ry, (rx + h : U32), ry, h)
      +db = R.Region.keep(cs2, rx, ry, (rx + h : U32), ry, h)
      +cc = R.Region.keep(cs, rx, ry, rx, (ry + h : U32), h)
      +dc = R.Region.keep(cs2, rx, ry, rx, (ry + h : U32), h)
      +cd = R.Region.keep(cs, rx, ry, (rx + h : U32), (ry + h : U32), h)
      +dd = R.Region.keep(cs2, rx, ry, (rx + h : U32), (ry + h : U32), h)
      a2 b2 c2 d2 = Tiles.recat.go(j, e, tab, ca, da, pa, rx, ry, h, Eq.ops(ca, da)) Tiles.recat.go(j, e, tab, cb, db, pb, (rx + h : U32), ry, h, Eq.ops(cb, db)) Tiles.recat.go(j, e, tab, cc, dc, pc, rx, (ry + h : U32), h, Eq.ops(cc, dc)) Tiles.recat.go(j, e, tab, cd, dd, pd, (rx + h : U32), (ry + h : U32), h, Eq.ops(cd, dd))
      TNode{Ops.cat(cs2, Ops.after(cs, o)), b, a2, b2, c2, d2}
    case _ _ _:
      t

def Tiles.recat(+k: Nat, +e: Nat, +tab: R.Tab, +cs: List<&2, R.Op>, +cs2: List<&2, R.Op>, t: Tiles, +rx: U32, +ry: U32, +n: U32) -> Tiles:
  Tiles.recat.go(k, e, tab, cs, cs2, t, rx, ry, n, Eq.ops(cs, cs2))

# the last frame (ops cs then the rest) with cs2 in place of cs
def Frames.recat(f: Frames, +cs: List<&2, R.Op>, +cs2: List<&2, R.Op>) -> Frames:
  match f:
    case FNone{}:
      FNone{}
    case FSome{+w, +h, +tab, t}:
      FSome{w, h, tab, Tiles.recat(Frame.levels(w, h), Frame.edge(w, h), tab, cs, cs2, t, 0, 0, U32.shln(1, R.Frame.depth(w, h)))}
