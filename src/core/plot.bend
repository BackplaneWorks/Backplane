import Base
import ./board.bend as B
import ./look.bend as LO
import ./eq.bend as Q
import ./json.bend as J
import ./glb.bend as G

# A board or schematic for the phones. The hub turns the viewer's items
# into chunks the phone draws as one path each: the fills, the strokes of
# one width or the dots of one diameter on one layer, in paint order, in
# integer micrometres, delta coded as varint bytes (Enc). Only visible
# layers are sent (law plot_hidden_not_sent). Each piece names its item's
# info (what the inspector shows) in its chunk's table, so a phone
# inspects a tap by itself.
#
# A group's items are spread over 16 chunks by a hash of their keys, so an
# edit to one item changes one chunk, whatever else the file holds. A new
# version of the file goes to a phone as a delta against the chunks it
# already holds (Plot.delta): a chunk it has goes as its index, so an
# unchanged board sends no geometry (law plot_unchanged_sends_nothing),
# and what the phone rebuilds is exactly the new plot (law
# plot_delta_exact). The phone fades in exactly the chunks sent whole.

# mode: 0 fill, 1 stroke (width), 2 dots (width is the diameter); sig is a
# hash of the rest, so most unequal chunks differ at the first compare;
# rev is the chunk's bytes (a char each) last first, as they were built,
# and info its info table (each entry ended by a NUL), last char first:
# a message puts both in place in one pass
type Chunk is Data:
  Chunk{sig: U32, layer: U32, color: U32, alpha: U32, mode: U32, width: U32, rev: String, info: String}

# one item's share of a chunk: where it sorts (rank, then its key's hash),
# how many points it has and their coordinates, x y x y ..., and its info
type Piece is Data:
  Piece{rank: U32, kh: U32, n: U32, ints: List<&2, U32>, info: String}

# what a phone gets for each chunk of a plot: the index of a chunk it
# already holds, or the chunk itself
type Ref is Data:
  Keep{at: Nat}
  Fresh{chunk: Chunk}

# A solid: a 3D model's triangles for the phones' 3D view (a board's body
# and parts, as kicad-cli exports them), in board axes (x, y as on the
# board, z up) and 10 micrometre units: each triangle is the XOR of its
# colour with the one before, then its 9 coordinates as zigzag deltas.
# box is x0 y0 z0 x1 y1 z1.
type Solid is Data:
  Solid{n: U32, box: List<&2, U32>, rev: String}

# Numbers
# -------
# Coordinates are micrometres as two's complement U32, so a delta is one
# wrapping subtraction. Each number goes out zigzag coded (small magnitudes
# of either sign stay small) as a varint: 7 bits a byte, low first, the
# top bit set while more follow.

def Num.um.k(neg: Bool, +x: F32) -> U32:
  match neg:
    case True{}:
      U32.sub(0, F32.to_u32(F32.round(F32.mul(F32.neg(x), 1000.0))))
    case False{}:
      F32.to_u32(F32.round(F32.mul(x, 1000.0)))

# millimetres to micrometres
def Num.um(+x: F32) -> U32:
  Num.um.k(F32.is_lt(x, 0.0), x)

def Num.zig(+d: U32) -> U32:
  U32.xor(U32.shln(d, 1n), U32.sub(0, U32.shrn(d, 31n)))

def Num.unzig(+z: U32) -> U32:
  U32.xor(U32.shrn(z, 1n), U32.sub(0, U32.and(z, 1)))

# Encoding
# --------
# The bytes are built backwards (a cons per byte, tail calls only) with a
# running FNV-1a hash of them; the info table beside them, each info given
# an index the first time a piece names it.

type Enc is Data:
  Enc{rev: String, px: U32, py: U32, h: U32, tab: Map<&2, U32>, n: U32, info: String}

def Enc.new() -> Enc:
  Enc{SNil{}, 0, 0, 2166136261, Map.new(&2, U32), 0, SNil{}}

def Enc.put(+c: U32, e: Enc) -> Enc:
  match e:
    case Enc{rev, px, py, +h, tab, n, info}:
      Enc{SCon{Chr{c}, rev}, px, py, (U32.xor(h, c) * 16777619 : U32), tab, n, info}

# one number, 7 bits a byte; `last` says nothing is left above them
def Enc.var(n: Nat, last: Bool, +z: U32, e: Enc) -> Enc:
  match n last:
    case 0n _:
      Enc.put(U32.and(z, 127), e)
    case 1n+m True{}:
      Enc.put(U32.and(z, 127), e)
    case 1n+m False{}:
      Enc.var(m, U32.is_eq(U32.shrn(z, 14n), 0), U32.shrn(z, 7n), Enc.put(U32.or(U32.and(z, 127), 128), e))

def Enc.num(+z: U32, e: Enc) -> Enc:
  Enc.var(4n, U32.is_eq(U32.shrn(z, 7n), 0), z, e)

def Enc.pt(+x: U32, +y: U32, e: Enc) -> Enc:
  match e:
    case Enc{rev, +px, +py, h, tab, n, info}:
      Enc.num(Num.zig((y - py : U32)), Enc.num(Num.zig((x - px : U32)), Enc{rev, x, y, h, tab, n, info}))

def Enc.pts(xs: List<&2, U32>, e: Enc) -> Enc:
  match xs:
    case +x <> +y <> t:
      Enc.pts(t, Enc.pt(x, y, e))
    case _:
      e

def Enc.idx.at(fresh: Bool, +i: U32, +s: String, e: Enc) -> Enc:
  match fresh e:
    case True{} Enc{rev, px, py, h, tab, +n, info}:
      Enc.num(n, Enc{rev, px, py, h, Map.set(&2, U32, tab, s, n), (n + 1 : U32), SCon{Chr{0}, String.reverse.go(s, info)}})
    case False{} _:
      Enc.num(i, e)

def Enc.idx.k(r: Map<&2, U32> & U32, +s: String, rev: String, +px: U32, +py: U32, +h: U32, +n: U32, info: String) -> Enc:
  (tab, +i) = r
  Enc.idx.at(U32.is_eq(i, 4294967295), i, s, Enc{rev, px, py, h, tab, n, info})

# the index of an info in the chunk's table (a new one is added)
def Enc.idx(+s: String, e: Enc) -> Enc:
  match e:
    case Enc{rev, px, py, h, tab, n, info}:
      Enc.idx.k(Map.get(U32, 4294967295, tab, s), s, rev, px, py, h, n, info)

# a piece is its info's index, then a path's point count and its points,
# or a dot's one point
def Enc.piece.k(dot: Bool, +n: U32, xs: List<&2, U32>, e: Enc) -> Enc:
  match dot:
    case True{}:
      Enc.pts(xs, e)
    case False{}:
      Enc.pts(xs, Enc.num(n, e))

def Enc.piece(+mode: U32, p: Piece, e: Enc) -> Enc:
  match p:
    case Piece{_, _, +n, xs, +info}:
      Enc.piece.k(U32.is_eq(mode, 2), n, xs, Enc.idx(info, e))

def Enc.pieces(ps: List<&2, Piece>, +mode: U32, e: Enc) -> Enc:
  match ps:
    case Nil{}:
      e
    case p <> t:
      Enc.pieces(t, mode, Enc.piece(mode, p, e))

def Enc.list(xs: List<&2, U32>, e: Enc) -> Enc:
  match xs:
    case Nil{}:
      e
    case +x <> t:
      Enc.list(t, Enc.num(x, e))

def Enc.rev(e: Enc) -> String:
  match e:
    case Enc{rev, _, _, _, _, _, _}:
      rev

# some numbers as bytes (a char each)
def Enc.text(xs: List<&2, U32>) -> String:
  String.reverse(Enc.rev(Enc.list(xs, Enc.new())))

# the numbers of some bytes (what the phones decode; the tests' reference)
type DecS is Data:
  DecS{acc: U32, sh: U32, out: List<&2, U32>}

def Dec.step.k(more: Bool, +acc: U32, +sh: U32, out: List<&2, U32>) -> DecS:
  match more:
    case True{}:
      DecS{acc, (sh + 7 : U32), out}
    case False{}:
      DecS{0, 0, acc <> out}

def Dec.step(+v: U32, st: DecS) -> DecS:
  match st:
    case DecS{+acc, +sh, out}:
      Dec.step.k(U32.is_ge(v, 128), U32.or(acc, U32.shln(U32.and(v, 127), U32.to_nat(sh))), sh, out)

def Dec.go(s: String, st: DecS) -> List<&2, U32>:
  match s st:
    case SNil{} DecS{_, _, out}:
      List.reverse(&2, U32, out)
    case SCon{Chr{+c}, t} _:
      Dec.go(t, Dec.step(c, st))

def Dec.nums(s: String) -> List<&2, U32>:
  Dec.go(s, DecS{0, 0, Nil{}})

# Rank
# ----
# A piece sorts by layer (paint order), mode, width, then bucket (its
# key's hash, 16 ways), packed in one U32: 5 + 2 + 21 + 4 bits.

def Rank.of(+layer: U32, +mode: U32, +width: U32, +kh: U32) -> U32:
  U32.or(U32.or(U32.shln(layer, 27n), U32.shln(mode, 25n)), U32.or(U32.shln(U32.min(width, 2097151), 4n), U32.and(kh, 15)))

def Rank.layer(+r: U32) -> U32:
  U32.shrn(r, 27n)

def Rank.mode(+r: U32) -> U32:
  U32.and(U32.shrn(r, 25n), 3)

def Rank.width(+r: U32) -> U32:
  U32.and(U32.shrn(r, 4n), 2097151)

def Piece.le.k(same: Bool, +ra: U32, +rb: U32, +ka: U32, +kb: U32) -> Bool:
  match same:
    case True{}:
      U32.is_le(ka, kb)
    case False{}:
      U32.is_lt(ra, rb)

def Piece.le(a: Piece, b: Piece) -> Bool:
  match a b:
    case Piece{+ra, +ka, _, _, _} Piece{+rb, +kb, _, _, _}:
      Piece.le.k(U32.is_eq(ra, rb), ra, rb, ka, kb)

def Piece.rank(p: Piece) -> U32:
  match p:
    case Piece{r, _, _, _, _}:
      r

# Pieces
# ------

def Key.hash(s: String, +h: U32) -> U32:
  match s:
    case SNil{}:
      h
    case SCon{Chr{+c}, t}:
      Key.hash(t, (U32.xor(h, c) * 16777619 : U32))

def Pts.put(p: B.Pt, acc: List<&2, U32>) -> List<&2, U32>:
  match p:
    case B.Pt{+x, +y}:
      Num.um(y) <> Num.um(x) <> acc

# a path's coordinates, backwards (then turned around once)
def Pts.rev(ps: List<&2, B.Pt>, acc: List<&2, U32>) -> List<&2, U32>:
  match ps:
    case Nil{}:
      acc
    case p <> t:
      Pts.rev(t, Pts.put(p, acc))

def Pts.of(ps: List<&2, B.Pt>) -> List<&2, U32>:
  List.reverse(&2, U32, Pts.rev(ps, Nil{}))

def Pts.ring.at(+i: Nat, +cx: F32, +cy: F32, +r: F32) -> B.Pt:
  +a = F32.mul(F32.div(F32.from_nat(i), 32.0), F32.mul(2.0, F32.pi()))
  B.Pt{F32.add(cx, F32.mul(r, F32.cos(a))), F32.add(cy, F32.mul(r, F32.sin(a)))}

def Pts.ring(n: Nat, +i: Nat, +cx: F32, +cy: F32, +r: F32) -> List<&2, B.Pt>:
  match n:
    case 0n:
      [Pts.ring.at(i, cx, cy, r)]
    case 1n+m:
      Pts.ring.at(i, cx, cy, r) <> Pts.ring(m, 1n+i, cx, cy, r)

def Pts.closed(+ps: List<&2, B.Pt>) -> List<&2, B.Pt>:
  List.append(&2, B.Pt, ps, List.take(&2, B.Pt, ps, 1n))

def Piece.make(+layer: U32, +mode: U32, +width: U32, +kh: U32, +n: Nat, xs: List<&2, U32>, info: String) -> Piece:
  Piece{Rank.of(layer, mode, width, kh), kh, U32.from_nat(n), xs, info}

def Piece.path(+layer: U32, +mode: U32, +width: U32, +kh: U32, +ps: List<&2, B.Pt>, info: String) -> Piece:
  Piece.make(layer, mode, width, kh, List.length(&2, B.Pt, ps), Pts.of(ps), info)

# how one primitive is drawn: its mode, width and points
def Piece.prim(p: B.Prim, +layer: U32, +kh: U32, info: String) -> Piece:
  match p:
    case B.PSeg{+a, +b, +w}:
      Piece.path(layer, 1, Num.um(w), kh, [a, b], info)
    case B.PLine{+ps, +w}:
      Piece.path(layer, 1, Num.um(w), kh, ps, info)
    case B.PCirc{+c, +r, _, True{}}:
      Piece.make(layer, 2, Num.um(F32.mul(r, 2.0)), kh, 1n, Pts.of([c]), info)
    case B.PCirc{B.Pt{+cx, +cy}, +r, +w, False{}}:
      Piece.path(layer, 1, Num.um(w), kh, Pts.ring(32n, 0n, cx, cy, r), info)
    case B.PPoly{+ps, _, True{}}:
      Piece.path(layer, 0, 0, kh, ps, info)
    case B.PPoly{+ps, +w, False{}}:
      Piece.path(layer, 1, Num.um(w), kh, Pts.closed(ps), info)
    case B.PTint{+ps, +color, +alpha}:
      Piece.path(layer, 0, B.Tint.code(color, alpha), kh, ps, info)

def Piece.shown(+alpha: U32) -> Bool:
  U32.is_ne(alpha, 0)

def Piece.alpha.k(l: U32 & U32) -> U32:
  (_, a) = l
  a

# the viewers' opacity for a layer (0 hides it)
def Piece.alpha(+kind: U32, +layer: U32) -> U32:
  Piece.alpha.k(LO.Look.of(kind, layer))

def Piece.put.k(shown: Bool, i: B.Item, acc: List<&2, Piece>) -> List<&2, Piece>:
  match shown i:
    case True{} B.Item{+key, _, +layer, p, info}:
      Piece.prim(p, layer, Key.hash(key, 2166136261), info) <> acc
    case False{} _:
      acc

# an item's piece on the list, when its layer is shown
def Piece.put(i: B.Item, +kind: U32, acc: List<&2, Piece>) -> List<&2, Piece>:
  match i:
    case B.Item{+key, sig, +layer, p, info}:
      Piece.put.k(Piece.shown(Piece.alpha(kind, layer)), B.Item{key, sig, layer, p, info}, acc)

def Piece.all(its: List<&2, B.Item>, +kind: U32, acc: List<&2, Piece>) -> List<&2, Piece>:
  match its:
    case Nil{}:
      acc
    case i <> t:
      Piece.all(t, kind, Piece.put(i, kind, acc))

# the pieces of some items (order is the sort's business)
def Piece.of(its: List<&2, B.Item>, +kind: U32) -> List<&2, Piece>:
  Piece.all(its, kind, Nil{})

# Chunks
# ------

type Group is Data:
  Group{i: Nat, rank: U32, ps: List<&2, Piece>}

type Hand is Data:
  Hand{a: List<&2, Group>, b: List<&2, Group>}

def Group.close(+i: Nat, +rank: U32, ps: List<&2, Piece>, gs: List<&2, Group>) -> List<&2, Group>:
  match ps:
    case Nil{}:
      gs
    case _:
      Group{i, rank, List.reverse(&2, Piece, ps)} <> gs

# the walk's state: the open group's number, rank and pieces (backwards),
# and the closed groups (backwards)
type GS is Data:
  GS{i: Nat, rank: U32, cur: List<&2, Piece>, gs: List<&2, Group>}

def Group.step.k(same: Bool, +p: Piece, st: GS) -> GS:
  match same st:
    case True{} GS{i, rank, cur, gs}:
      GS{i, rank, p <> cur, gs}
    case False{} GS{+i, +rank, +cur, gs}:
      GS{Bool.pick(Nat, List.is_empty(&2, Piece, cur), i, 1n+i), Piece.rank(p), [p], Group.close(i, rank, cur, gs)}

def Group.step(+p: Piece, st: GS) -> GS:
  match st:
    case GS{i, +rank, cur, gs}:
      Group.step.k(U32.is_eq(Piece.rank(p), rank), p, GS{i, rank, cur, gs})

def Group.fin(st: GS) -> List<&2, Group>:
  match st:
    case GS{+i, +rank, cur, gs}:
      List.reverse(&2, Group, Group.close(i, rank, cur, gs))

# runs of equal rank in sorted pieces, numbered in order
def Group.go(ps: List<&2, Piece>, st: GS) -> List<&2, Group>:
  match ps:
    case Nil{}:
      Group.fin(st)
    case p <> t:
      Group.go(t, Group.step(p, st))

def Chunk.fin(e: Enc, +rank: U32, +color: U32, +alpha: U32) -> Chunk:
  match e:
    case Enc{rev, _, _, +h, _, _, +info}:
      Chunk{U32.xor(Key.hash(info, h), rank), Rank.layer(rank), color, alpha, Rank.mode(rank), Rank.width(rank), rev, info}

def Chunk.look(l: U32 & U32, +rank: U32, e: Enc) -> Chunk:
  (+color, +alpha) = l
  Chunk.fin(e, rank, color, alpha)

# fills with a width are tints (B.Tint.code): their colour is their own
def Chunk.tint(own: Bool, +kind: U32, +rank: U32) -> U32 & U32:
  match own:
    case True{}:
      (B.Tint.color(Rank.width(rank)), B.Tint.alpha(Rank.width(rank)))
    case False{}:
      LO.Look.of(kind, Rank.layer(rank))

def Chunk.of(g: Group, +kind: U32) -> Chunk:
  match g:
    case Group{_, +rank, ps}:
      Chunk.look(Chunk.tint(Bool.and(U32.is_eq(Rank.mode(rank), 0), U32.is_ne(Rank.width(rank), 0)), kind, rank), rank, Enc.pieces(ps, Rank.mode(rank), Enc.new()))

# Parallel encoding
# -----------------
# Groups are dealt alternately into two hands, as board.bend deals forms,
# so halves cost about the same; each chunk keeps its group's number and
# the plot is put back in order after.

type Ix is Data:
  Ix{i: Nat, c: Chunk}

def Ix.le(a: Ix, b: Ix) -> Bool:
  match a b:
    case Ix{x, _} Ix{y, _}:
      Nat.is_le(x, y)

def Ix.chunks(xs: List<&2, Ix>) -> List<&2, Chunk>:
  match xs:
    case Nil{}:
      Nil{}
    case Ix{_, c} <> t:
      c <> Ix.chunks(t)

def Group.deal(gs: List<&2, Group>, h: Hand) -> Hand:
  match gs h:
    case Nil{} _:
      h
    case g <> t Hand{a, b}:
      Group.deal(t, Hand{b, g <> a})

def Group.seq(gs: List<&2, Group>, +kind: U32, acc: List<&2, Ix>) -> List<&2, Ix>:
  match gs:
    case Nil{}:
      acc
    case Group{+i, rank, ps} <> t:
      Group.seq(t, kind, Ix{i, Chunk.of(Group{i, rank, ps}, kind)} <> acc)

def Group.split.k(h: Hand, k: List<&2, Group> -> List<&2, Group> -> List<&2, Ix>) -> List<&2, Ix>:
  match h:
    case Hand{a, b}:
      k(a, b)

def Group.par(d: Nat, gs: List<&2, Group>, +kind: U32) -> List<&2, Ix>:
  match d:
    case 0n:
      Group.seq(gs, kind, Nil{})
    case 1n+e0:
      +e = e0
      Group.split.k(Group.deal(gs, Hand{Nil{}, Nil{}}), a => b => (
        x y = Group.par(e, a, kind) Group.par(e, b, kind)
        List.append(&2, Ix, x, y)))

# Extent
# ------
# The box around every coordinate, in micrometres (signed), for the
# phone's first view.

type Box is Data:
  Box{x0: U32, y0: U32, x1: U32, y1: U32}

# signed order of two's complement numbers
def Box.lt(+a: U32, +b: U32) -> Bool:
  U32.is_lt(U32.xor(a, 2147483648), U32.xor(b, 2147483648))

def Box.pt(+x: U32, +y: U32, b: Box) -> Box:
  match b:
    case Box{+x0, +y0, +x1, +y1}:
      Box{Bool.pick(U32, Box.lt(x, x0), x, x0), Bool.pick(U32, Box.lt(y, y0), y, y0), Bool.pick(U32, Box.lt(x1, x), x, x1), Bool.pick(U32, Box.lt(y1, y), y, y1)}

def Box.ints(xs: List<&2, U32>, b: Box) -> Box:
  match xs:
    case +x <> +y <> t:
      Box.ints(t, Box.pt(x, y, b))
    case _:
      b

def Box.pieces(ps: List<&2, Piece>, b: Box) -> Box:
  match ps:
    case Nil{}:
      b
    case Piece{_, _, _, xs, _} <> t:
      Box.pieces(t, Box.ints(xs, b))

def Box.empty() -> Box:
  Box{2147483647, 2147483647, 2147483648, 2147483648}

# The board's rectangle: the box round its outline (Edge.Cuts) points, a
# circle to its radius, stroke widths left out. The phones' 3D view turns
# about its centre, as the window's does (face.bend's Outline.box).

def Edge.pts(ps: List<&2, B.Pt>, +r: F32, b: Box) -> Box:
  match ps:
    case Nil{}:
      b
    case B.Pt{+x, +y} <> t:
      Edge.pts(t, r, Box.pt(Num.um(F32.add(x, r)), Num.um(F32.add(y, r)), Box.pt(Num.um(F32.sub(x, r)), Num.um(F32.sub(y, r)), b)))

def Edge.prim(p: B.Prim, b: Box) -> Box:
  match p:
    case B.PSeg{a, c, _}:
      Edge.pts([a, c], 0.0, b)
    case B.PLine{ps, _}:
      Edge.pts(ps, 0.0, b)
    case B.PCirc{c, +r, _, _}:
      Edge.pts([c], r, b)
    case B.PPoly{ps, _, _}:
      Edge.pts(ps, 0.0, b)
    case B.PTint{ps, _, _}:
      Edge.pts(ps, 0.0, b)

def Edge.put(on: Bool, p: B.Prim, b: Box) -> Box:
  match on:
    case True{}:
      Edge.prim(p, b)
    case False{}:
      b

# the box round the items on layer edge
def Edge.box(its: List<&2, B.Item>, +edge: U32, b: Box) -> Box:
  match its:
    case Nil{}:
      b
    case B.Item{_, _, +layer, p, _} <> t:
      Edge.box(t, edge, Edge.put(U32.is_eq(layer, edge), p, b))

# Sorting in parallel
# -------------------
# Items are dealt into 2^d hands; each hand becomes pieces and is sorted
# on its own core, and sorted hands merge pairwise on the way back.

type IHand is Data:
  IHand{a: List<&2, B.Item>, b: List<&2, B.Item>}

def Item.deal(its: List<&2, B.Item>, h: IHand) -> IHand:
  match its h:
    case Nil{} _:
      h
    case i <> t IHand{a, b}:
      Item.deal(t, IHand{b, i <> a})

def Piece.hle(xs: List<&2, Piece>, ys: List<&2, Piece>) -> Bool:
  match xs ys:
    case +x <> _ +y <> _:
      Piece.le(x, y)
    case _ _:
      True{}

# two sorted lists as one (backwards onto acc, then turned around); n
# bounds the steps, `le` is whether xs's head goes first
def Piece.merge(n: Nat, le: Bool, xs: List<&2, Piece>, ys: List<&2, Piece>, acc: List<&2, Piece>) -> List<&2, Piece>:
  match n le xs ys:
    case 1n+m True{} x <> +xt +ys2:
      Piece.merge(m, Piece.hle(xt, ys2), xt, ys2, x <> acc)
    case 1n+m False{} +xs2 y <> +yt:
      Piece.merge(m, Piece.hle(xs2, yt), xs2, yt, y <> acc)
    case _ _ Nil{} ys2:
      List.reverse.go(&2, Piece, acc, ys2)
    case _ _ xs2 Nil{}:
      List.reverse.go(&2, Piece, acc, xs2)
    case 0n _ xs2 ys2:
      List.reverse.go(&2, Piece, acc, List.append(&2, Piece, xs2, ys2))

def Piece.join(+xs: List<&2, Piece>, +ys: List<&2, Piece>) -> List<&2, Piece>:
  Piece.merge(Nat.add(List.length(&2, Piece, xs), List.length(&2, Piece, ys)), Piece.hle(xs, ys), xs, ys, Nil{})

def Piece.psort.k(h: IHand, k: List<&2, B.Item> -> List<&2, B.Item> -> List<&2, Piece>) -> List<&2, Piece>:
  match h:
    case IHand{a, b}:
      k(a, b)

def Piece.psort(d: Nat, its: List<&2, B.Item>, +kind: U32) -> List<&2, Piece>:
  match d:
    case 0n:
      List.sort(~Piece, ~Piece.le, Piece.of(its, kind))
    case 1n+e0:
      +e = e0
      Piece.psort.k(Item.deal(its, IHand{Nil{}, Nil{}}), a => b => (
        x y = Piece.psort(e, a, kind) Piece.psort(e, b, kind)
        Piece.join(x, y)))

# Solids
# ------

def Solid.none() -> Solid:
  Solid{0, Nil{}, SNil{}}

type SE is Data:
  SE{e: Enc, pz: U32, c: U32, n: U32, b: List<&2, U32>}

def SE.box(+x: U32, +y: U32, +z: U32, b: List<&2, U32>) -> List<&2, U32>:
  match b:
    case +x0 <> +y0 <> +z0 <> +x1 <> +y1 <> +z1 <> _:
      [Bool.pick(U32, Box.lt(x, x0), x, x0), Bool.pick(U32, Box.lt(y, y0), y, y0), Bool.pick(U32, Box.lt(z, z0), z, z0),
       Bool.pick(U32, Box.lt(x1, x), x, x1), Bool.pick(U32, Box.lt(y1, y), y, y1), Bool.pick(U32, Box.lt(z1, z), z, z1)]
    case _:
      [x, y, z, x, y, z]

# a glTF coordinate (metres, y up) in 10 micrometre units
def SE.u(+m: F32) -> U32:
  Num.um(F32.mul(m, 100.0))

# one vertex: x and y as board axes (glTF x and z), z up (glTF y)
def SE.v(+gx: F32, +gy: F32, +gz: F32, st: SE) -> SE:
  match st:
    case SE{e, +pz, c, n, b}:
      SE{Enc.num(Num.zig((SE.u(gy) - pz : U32)), Enc.pt(SE.u(gx), SE.u(gz), e)), SE.u(gy), c, n, SE.box(SE.u(gx), SE.u(gz), SE.u(gy), b)}

def SE.tri.k(t: G.Tri3, st: SE) -> SE:
  match t:
    case G.Tri3{+ax, +ay, +az, +bx, +by, +bz, +cx, +cy, +cz, _}:
      SE.v(cx, cy, cz, SE.v(bx, by, bz, SE.v(ax, ay, az, st)))

def SE.tri(t: G.Tri3, st: SE) -> SE:
  match t st:
    case G.Tri3{ax, ay, az, bx, by, bz, cx, cy, cz, +col} SE{e, pz, +c, +n, b}:
      SE.tri.k(G.Tri3{ax, ay, az, bx, by, bz, cx, cy, cz, col}, SE{Enc.num(U32.xor(col, c), e), pz, col, (n + 1 : U32), b})

def SE.go(ts: List<&2, G.Tri3>, st: SE) -> SE:
  match ts:
    case Nil{}:
      st
    case t <> rest:
      SE.go(rest, SE.tri(t, st))

def SE.fin(st: SE) -> Solid:
  match st:
    case SE{e, _, _, n, b}:
      Solid{n, b, Enc.rev(e)}

# Thinning for the phones and the web, by vertex clustering: every corner
# moves across to the centre of its cell in a grid a thousandth of the
# model's size, and a triangle two of whose corners land in one cell
# is dropped. The ones kept meet at the moved corners, so surfaces stay
# whole: a part's small triangles fold into fewer larger ones rather than
# leaving it in fragments (a board's parts once kept 18% of their
# triangles as scattered slivers). A case assembly with its board went from
# 880k triangles to 134k, 12 MB to 2. The window draws the whole model.
type Grid is Data:
  Grid{x0: F32, y0: F32, z0: F32, x1: F32, y1: F32, z1: F32}

def Grid.put(g: Grid, +x: F32, +y: F32, +z: F32) -> Grid:
  match g:
    case Grid{x0, y0, z0, x1, y1, z1}:
      Grid{F32.min(x0, x), F32.min(y0, y), F32.min(z0, z), F32.max(x1, x), F32.max(y1, y), F32.max(z1, z)}

def Grid.go(ts: List<&2, G.Tri3>, g: Grid) -> Grid:
  match ts:
    case Nil{}:
      g
    case G.Tri3{ax, ay, az, bx, by, bz, cx, cy, cz, _} <> t:
      Grid.go(t, Grid.put(Grid.put(Grid.put(g, ax, ay, az), bx, by, bz), cx, cy, cz))

# the cell size: a thousandth of the box's diagonal (0 for an empty model)
def Grid.cell(g: Grid) -> F32:
  match g:
    case Grid{+x0, +y0, +z0, +x1, +y1, +z1}:
      +dx = F32.max(F32.sub(x1, x0), 0.0)
      +dy = F32.max(F32.sub(y1, y0), 0.0)
      +dz = F32.max(F32.sub(z1, z0), 0.0)
      F32.div(F32.sqrt(F32.add(F32.mul(dx, dx), F32.add(F32.mul(dy, dy), F32.mul(dz, dz)))), 1000.0)

def Grid.at(+v: F32, +o: F32, +s: F32) -> U32:
  F32.to_u32(F32.floor(F32.div(F32.sub(v, o), s)))

# two corners in one cell
def Grid.same(+ax: F32, +ay: F32, +az: F32, +bx: F32, +by: F32, +bz: F32, +x0: F32, +y0: F32, +z0: F32, +s: F32) -> Bool:
  Bool.and(U32.is_eq(Grid.at(ax, x0, s), Grid.at(bx, x0, s)), Bool.and(U32.is_eq(Grid.at(ay, y0, s), Grid.at(by, y0, s)), U32.is_eq(Grid.at(az, z0, s), Grid.at(bz, z0, s))))

def Grid.keep(+t: G.Tri3, +x0: F32, +y0: F32, +z0: F32, +s: F32) -> Bool:
  match t:
    case G.Tri3{+ax, +ay, +az, +bx, +by, +bz, +cx, +cy, +cz, _}:
      Bool.not(Bool.or(Grid.same(ax, ay, az, bx, by, bz, x0, y0, z0, s), Bool.or(Grid.same(bx, by, bz, cx, cy, cz, x0, y0, z0, s), Grid.same(ax, ay, az, cx, cy, cz, x0, y0, z0, s))))

# a coordinate at its cell's centre
def Grid.snap(+v: F32, +o: F32, +s: F32) -> F32:
  F32.add(o, F32.mul(F32.add(F32.floor(F32.div(F32.sub(v, o), s)), 0.5), s))

# (across only: y, the model's up, keeps its value, so a board's thin
# layers (body, copper, mask, silk, hundredths of a millimetre apart) stay
# apart instead of merging into one height and fighting)
def Grid.snap.tri(t: G.Tri3, +x0: F32, +y0: F32, +z0: F32, +s: F32) -> G.Tri3:
  match t:
    case G.Tri3{ax, ay, az, bx, by, bz, cx, cy, cz, c}:
      G.Tri3{Grid.snap(ax, x0, s), ay, Grid.snap(az, z0, s), Grid.snap(bx, x0, s), by, Grid.snap(bz, z0, s),
        Grid.snap(cx, x0, s), cy, Grid.snap(cz, z0, s), c}

def Grid.put.k(keep: Bool, t: G.Tri3, acc: List<&2, G.Tri3>) -> List<&2, G.Tri3>:
  match keep:
    case True{}:
      t <> acc
    case False{}:
      acc

def Grid.thin(ts: List<&2, G.Tri3>, +x0: F32, +y0: F32, +z0: F32, +s: F32, acc: List<&2, G.Tri3>) -> List<&2, G.Tri3>:
  match ts:
    case Nil{}:
      List.reverse(&2, G.Tri3, acc)
    case +t <> r:
      Grid.thin(r, x0, y0, z0, s, Grid.put.k(Grid.keep(t, x0, y0, z0, s), Grid.snap.tri(t, x0, y0, z0, s), acc))

def Solid.thin.go(+s: F32, +x0: F32, +y0: F32, +z0: F32, +ts: List<&2, G.Tri3>) -> List<&2, G.Tri3>:
  Bool.pick(List<&2, G.Tri3>, F32.is_gt(s, 0.0), Grid.thin(ts, x0, y0, z0, s, Nil{}), ts)

def Solid.thin.k(g: Grid, +ts: List<&2, G.Tri3>) -> List<&2, G.Tri3>:
  match g:
    case Grid{+x0, +y0, +z0, x1, y1, z1}:
      Solid.thin.go(Grid.cell(Grid{x0, y0, z0, x1, y1, z1}), x0, y0, z0, ts)

def Solid.thin(+ts: List<&2, G.Tri3>) -> List<&2, G.Tri3>:
  +big = {1000000000.0 : F32}
  Solid.thin.k(Grid.go(ts, Grid{big, big, big, F32.neg(big), F32.neg(big), F32.neg(big)}), ts)

# a model's triangles as a solid
def Solid.of(ts: List<&2, G.Tri3>) -> Solid:
  SE.fin(SE.go(ts, SE{Enc.new(), 0, 0, 0, Nil{}}))

# A plot
# ------

# thick is the board's thickness in micrometres (the phones' 3D view);
# solid is set when the plot is a 3D model's; edge is the board's
# rectangle (Edge.box), empty for a schematic
type Plot is Data:
  Plot{cs: List<&2, Chunk>, box: Box, thick: U32, solid: Solid, edge: Box}

def Plot.of.k(+ps: List<&2, Piece>, +kind: U32, edge: Box) -> Plot:
  Plot{Ix.chunks(List.sort(~Ix, ~Ix.le, Group.par(6n, Group.go(ps, GS{0n, 0, Nil{}, Nil{}}), kind))), Box.pieces(ps, Box.empty()), 1600, Solid.none(), edge}

# the chunks of some items, in paint order, their extent and the board's
# rectangle
def Plot.of(+its: List<&2, B.Item>, +kind: U32) -> Plot:
  Plot.of.k(Piece.psort(5n, its, kind), kind, Edge.box(its, Bool.pick(U32, U32.is_eq(LO.Look.base(kind), 0), B.Layer.id("Edge.Cuts"), 4294967295), Box.empty()))

def Plot.of.solid(so: Solid) -> Plot:
  Plot{Nil{}, Box.empty(), 0, so, Box.empty()}

def Plot.chunks(p: Plot) -> List<&2, Chunk>:
  match p:
    case Plot{cs, _, _, _, _}:
      cs

def Plot.box(p: Plot) -> Box:
  match p:
    case Plot{_, b, _, _, _}:
      b

def Plot.thick(p: Plot) -> U32:
  match p:
    case Plot{_, _, t, _, _}:
      t

def Plot.solid(p: Plot) -> Solid:
  match p:
    case Plot{_, _, _, so, _}:
      so

def Plot.edge(p: Plot) -> Box:
  match p:
    case Plot{_, _, _, _, e}:
      e

def Plot.with_thick(p: Plot, +t: U32) -> Plot:
  match p:
    case Plot{cs, b, _, so, e}:
      Plot{cs, b, t, so, e}

# Deltas
# ------

# exact equality of chunks, the cheap fields first (unequal chunks almost
# always differ in sig and stop there)
def Eq.chunk.k(same: Bool, a: Chunk, b: Chunk) -> Bool:
  match same a b:
    case False{} _ _:
      False{}
    case True{} Chunk{_, l1, c1, a1, m1, w1, d1, i1} Chunk{_, l2, c2, a2, m2, w2, d2, i2}:
      Bool.and(U32.is_eq(l1, l2), Bool.and(U32.is_eq(c1, c2), Bool.and(U32.is_eq(a1, a2), Bool.and(U32.is_eq(m1, m2), Bool.and(U32.is_eq(w1, w2), Bool.and(Q.Eq.str(d1, d2), Q.Eq.str(i1, i2)))))))

def Eq.sig(a: Chunk, b: Chunk) -> Bool:
  match a b:
    case Chunk{s1, _, _, _, _, _, _, _} Chunk{s2, _, _, _, _, _, _, _}:
      U32.is_eq(s1, s2)

def Eq.chunk(+a: Chunk, +b: Chunk) -> Bool:
  match a b:
    case Chunk{+s1, l1, c1, a1, m1, w1, d1, i1} Chunk{+s2, l2, c2, a2, m2, w2, d2, i2}:
      Eq.chunk.k(U32.is_eq(s1, s2), Chunk{s1, l1, c1, a1, m1, w1, d1, i1}, Chunk{s2, l2, c2, a2, m2, w2, d2, i2})

# whether the first held chunk is c
def Plot.hit(os: List<&2, Chunk>, +c: Chunk) -> Bool:
  match os:
    case Nil{}:
      False{}
    case +o <> _:
      Eq.chunk(c, o)

def Ref.next(r: Ref) -> Ref:
  match r:
    case Keep{i}:
      Keep{1n+i}
    case Fresh{c}:
      Fresh{c}

# c as the phone gets it: the index of an equal chunk it holds, or itself
def Plot.find(os: List<&2, Chunk>, hit: Bool, +c: Chunk) -> Ref:
  match os hit:
    case Nil{} _:
      Fresh{c}
    case _ <> _ True{}:
      Keep{0n}
    case _ <> +ot False{}:
      Ref.next(Plot.find(ot, Plot.hit(ot, c), c))

# the plot `ns` for a phone holding `old`
def Plot.delta(ns: List<&2, Chunk>, +old: List<&2, Chunk>) -> List<&2, Ref>:
  match ns:
    case Nil{}:
      Nil{}
    case +n <> t:
      Plot.find(old, Plot.hit(old, n), n) <> Plot.delta(t, old)

def Chunk.none() -> Chunk:
  Chunk{0, 0, 0, 0, 0, 0, "", ""}

def Plot.at(i: Nat, os: List<&2, Chunk>) -> Chunk:
  match i os:
    case 0n o <> _:
      o
    case 1n+j _ <> ot:
      Plot.at(j, ot)
    case _ Nil{}:
      Chunk.none()

def Ref.get(r: Ref, +os: List<&2, Chunk>) -> Chunk:
  match r:
    case Keep{i}:
      Plot.at(i, os)
    case Fresh{c}:
      c

# what the phone does with a delta: the chunks it names, in order
def Plot.apply(rs: List<&2, Ref>, +old: List<&2, Chunk>) -> List<&2, Chunk>:
  match rs:
    case Nil{}:
      Nil{}
    case r <> t:
      Ref.get(r, old) <> Plot.apply(t, old)

def Ref.fresh(r: Ref) -> Nat:
  match r:
    case Keep{_}:
      0n
    case Fresh{_}:
      1n

# how many chunks go out whole (the only geometry on the air)
def Plot.fresh(rs: List<&2, Ref>) -> Nat:
  match rs:
    case Nil{}:
      0n
    case r <> t:
      Nat.add(Ref.fresh(r), Plot.fresh(t))

# whether a delta changes nothing for a phone holding n chunks (each kept
# where it was): then nothing is sent
def Plot.quiet(rs: List<&2, Ref>, +i: Nat, +n: Nat) -> Bool:
  match rs:
    case Nil{}:
      Nat.is_eq(i, n)
    case Keep{k} <> t:
      Bool.and(Nat.is_eq(k, i), Plot.quiet(t, 1n+i, n))
    case Fresh{_} <> _:
      False{}

# The message
# -----------
# One CBOR map in the hub's dialect (core/cbor.bend: "t" and "key" go as
# their dictionary indexes 1 and 31): {"t": "plot", "key", "box":
# [x0, y0, x1, y1], "thick", "edge": [x0, y0, x1, y1], "cs": [...]},
# each of cs an unsigned number (keep that chunk of the last plot) or {"l", "c", "a", "m", "w": numbers,
# "d": the geometry bytes}. A chunk's info table stays on the hub: a phone
# asks for one piece's info when it inspects a tap (Plot.info). Written as bytes
# straight onto the frame, back to front with tail calls only, so a big
# board is neither parsed again nor deepens the stack.

# a CBOR head (major type already shifted, as cbor.bend's) before acc
def Cb.head.k(a: Bool, b: Bool, c: Bool, +mb: U32, +n: U32, acc: List<&2, U32>) -> List<&2, U32>:
  match a b c:
    case True{} _ _:
      U32.or(mb, n) <> acc
    case False{} True{} _:
      U32.or(mb, 24) <> n <> acc
    case False{} False{} True{}:
      U32.or(mb, 25) <> U32.shrn(n, 8n) <> U32.and(n, 255) <> acc
    case False{} False{} False{}:
      U32.or(mb, 26) <> U32.shrn(n, 24n) <> U32.and(U32.shrn(n, 16n), 255) <> U32.and(U32.shrn(n, 8n), 255) <> U32.and(n, 255) <> acc

def Cb.head(+mb: U32, +n: U32, acc: List<&2, U32>) -> List<&2, U32>:
  Cb.head.k(U32.is_lt(n, 24), U32.is_lt(n, 256), U32.is_lt(n, 65536), mb, n, acc)

def Cb.uint(+n: U32, acc: List<&2, U32>) -> List<&2, U32>:
  Cb.head(0, n, acc)

# a signed number (two's complement) as major 0 or 1
def Cb.int(+v: U32, +acc: List<&2, U32>) -> List<&2, U32>:
  Bool.pick(List<&2, U32>, U32.is_ge(v, 2147483648), Cb.head(32, U32.not(v), acc), Cb.head(0, v, acc))

# the UTF-8 bytes of a code point, in order, before acc
def Cb.cp.k(a: Bool, b: Bool, c: Bool, +cp: U32, acc: List<&2, U32>) -> List<&2, U32>:
  match a b c:
    case True{} _ _:
      cp <> acc
    case False{} True{} _:
      U32.or(192, U32.shrn(cp, 6n)) <> U32.or(128, U32.and(cp, 63)) <> acc
    case False{} False{} True{}:
      U32.or(224, U32.shrn(cp, 12n)) <> U32.or(128, U32.and(U32.shrn(cp, 6n), 63)) <> U32.or(128, U32.and(cp, 63)) <> acc
    case False{} False{} False{}:
      U32.or(240, U32.shrn(cp, 18n)) <> U32.or(128, U32.and(U32.shrn(cp, 12n), 63)) <> U32.or(128, U32.and(U32.shrn(cp, 6n), 63)) <> U32.or(128, U32.and(cp, 63)) <> acc

def Cb.cp(+cp: U32, acc: List<&2, U32>) -> List<&2, U32>:
  Cb.cp.k(U32.is_lt(cp, 128), U32.is_lt(cp, 2048), U32.is_lt(cp, 65536), cp, acc)

# a text given backwards, as UTF-8 in order before acc
def Cb.utf8(rs: String, acc: List<&2, U32>) -> List<&2, U32>:
  match rs:
    case SNil{}:
      acc
    case SCon{Chr{+c}, t}:
      Cb.utf8(t, Cb.cp(c, acc))

# bytes given backwards (a char each), in order before acc
def Cb.raw(rs: String, acc: List<&2, U32>) -> List<&2, U32>:
  match rs:
    case SNil{}:
      acc
    case SCon{Chr{c}, t}:
      Cb.raw(t, c <> acc)

def Cb.blen(s: String, +n: U32) -> U32:
  match s:
    case SNil{}:
      n
    case SCon{Chr{+c}, t}:
      Cb.blen(t, (n + Bool.pick(U32, U32.is_lt(c, 128), 1, Bool.pick(U32, U32.is_lt(c, 2048), 2, Bool.pick(U32, U32.is_lt(c, 65536), 3, 4))) : U32))

def Cb.count(s: String, +n: U32) -> U32:
  match s:
    case SNil{}:
      n
    case SCon{_, t}:
      Cb.count(t, (n + 1 : U32))

# a text item (given backwards) before acc
def Cb.text(+rs: String, acc: List<&2, U32>) -> List<&2, U32>:
  Cb.head(96, Cb.blen(rs, 0), Cb.utf8(rs, acc))

# an ASCII key before acc
def Cb.key(+k: String, acc: List<&2, U32>) -> List<&2, U32>:
  Cb.text(String.reverse(k), acc)

def Out.chunk(c: Chunk, acc: List<&2, U32>) -> List<&2, U32>:
  match c:
    case Chunk{_, +l, +co, +a, +m, +w, +d, _}:
      Cb.head(160, 6, Cb.key("l", Cb.uint(l, Cb.key("c", Cb.uint(co, Cb.key("a", Cb.uint(a, Cb.key("m", Cb.uint(m, Cb.key("w", Cb.uint(w,
        Cb.key("d", Cb.head(64, Cb.count(d, 0), Cb.raw(d, acc))))))))))))))

def Out.ref(r: Ref, acc: List<&2, U32>) -> List<&2, U32>:
  match r:
    case Keep{i}:
      Cb.uint(U32.from_nat(i), acc)
    case Fresh{c}:
      Out.chunk(c, acc)

# the refs, given last first, before acc
def Out.refs(rs: List<&2, Ref>, acc: List<&2, U32>) -> List<&2, U32>:
  match rs:
    case Nil{}:
      acc
    case r <> t:
      Out.refs(t, Out.ref(r, acc))

def Out.box(b: Box, acc: List<&2, U32>) -> List<&2, U32>:
  match b:
    case Box{+x0, +y0, +x1, +y1}:
      Cb.head(128, 4, Cb.int(x0, Cb.int(y0, Cb.int(x1, Cb.int(y1, acc)))))

# a plot for one phone: the CBOR bytes of one frame
def Plot.msg(+key: String, b: Box, +thick: U32, e: Box, +rs: List<&2, Ref>) -> List<&2, U32>:
  Cb.head(160, 6, Cb.uint(1, Cb.key("plot", Cb.uint(31, Cb.text(String.reverse(key), Cb.key("box", Out.box(b,
    Cb.key("thick", Cb.uint(thick, Cb.key("edge", Out.box(e, Cb.key("cs", Cb.head(128, U32.from_nat(List.length(&2, Ref, rs)), Out.refs(List.reverse(&2, Ref, rs), Nil{}))))))))))))))

# the same with "fam": the file the source is a version of (core/hist.bend's
# Hist.key.family): a viewer moving between versions of one file keeps its
# view instead of fitting the new plot; and "base": the source whose plot
# the kept indexes are of ("" none; a web page shows several at once)
def Plot.msg.fam(+key: String, +fam: String, +base: String, b: Box, +thick: U32, e: Box, +rs: List<&2, Ref>) -> List<&2, U32>:
  Cb.head(160, 8, Cb.uint(1, Cb.key("plot", Cb.uint(31, Cb.text(String.reverse(key), Cb.key("fam", Cb.text(String.reverse(fam), Cb.key("base", Cb.text(String.reverse(base), Cb.key("box", Out.box(b,
    Cb.key("thick", Cb.uint(thick, Cb.key("edge", Out.box(e, Cb.key("cs", Cb.head(128, U32.from_nat(List.length(&2, Ref, rs)), Out.refs(List.reverse(&2, Ref, rs), Nil{}))))))))))))))))))

# s in front of acc, given s backwards
def Out.pre(rs: String, acc: String) -> String:
  String.reverse.go(rs, acc)

def Out.put(s: String, acc: String) -> String:
  Out.pre(String.reverse(s), acc)

def Cb.ints(xs: List<&2, U32>, acc: List<&2, U32>) -> List<&2, U32>:
  match xs:
    case Nil{}:
      acc
    case +x <> t:
      Cb.int(x, Cb.ints(t, acc))

# a 3D model for one phone: {"t": "plot", "key", "box3", "n", "mesh": the
# triangles' bytes}
def Plot.msg.solid(+key: String, so: Solid) -> List<&2, U32>:
  match so:
    case Solid{+n, +b, +d}:
      Cb.head(160, 5, Cb.uint(1, Cb.key("plot", Cb.uint(31, Cb.text(String.reverse(key), Cb.key("box3",
        Cb.head(128, U32.from_nat(List.length(&2, U32, b)), Cb.ints(b, Cb.key("n", Cb.uint(n, Cb.key("mesh", Cb.head(64, Cb.count(d, 0), Cb.raw(d, Nil{})))))))))))))

def Info.nth(idx: Nat, ss: List<&2, String>) -> String:
  match idx ss:
    case 0n s <> _:
      s
    case 1n+j _ <> t:
      Info.nth(j, t)
    case _ Nil{}:
      ""

def Info.of(c: Chunk, idx: Nat) -> String:
  match c:
    case Chunk{_, _, _, _, _, _, _, i}:
      Info.nth(idx, String.split(String.reverse(i), Chr{0}))

# the info of piece idx of the chunk at `at` (what a phone that holds these
# chunks asks when it inspects a tap; "" when there is none)
def Plot.info(cs: List<&2, Chunk>, at: Nat, idx: Nat) -> String:
  Info.of(Plot.at(at, cs), idx)

def Chunk.sig(c: Chunk) -> U32:
  match c:
    case Chunk{s, _, _, _, _, _, _, _}:
      s

# Info files
# ----------
# The hub keeps no info tables (on a big board they would be most of its
# memory): the child that reads a file writes them beside its plot, one
# line per chunk, "<sig> <chars> <table>", and a tap is looked up there by
# the tapped chunk's sig. A chunk's sig covers its info, and what a phone
# holds is always chunks of the newest plot, so the newest file answers.

def Infos.lines(cs: List<&2, Chunk>, acc: String) -> String:
  match cs:
    case Nil{}:
      acc
    case Chunk{s, _, _, _, _, _, _, +i} <> t:
      Infos.lines(t, Out.put(U32.show(s) ++ " " ++ U32.show(Cb.count(i, 0)) ++ " ", Out.pre(i, Out.put("\n", acc))))

# a plot's info file
def Infos.file(cs: List<&2, Chunk>) -> String:
  Infos.lines(List.reverse(&2, Chunk, cs), "")

def Infos.strip(cs: List<&2, Chunk>) -> List<&2, Chunk>:
  match cs:
    case Nil{}:
      Nil{}
    case Chunk{s, l, c, a, m, w, d, _} <> t:
      Chunk{s, l, c, a, m, w, d, ""} <> Infos.strip(t)

# a plot without its info tables (what the hub keeps; sigs unchanged)
def Plot.bare(p: Plot) -> Plot:
  match p:
    case Plot{cs, b, t, so, e}:
      Plot{Infos.strip(cs), b, t, so, e}

# looking up: the state is which field (0 sig, 1 count, 2 table), the
# number read, the sig, the chars left, the table so far (backwards), and
# the table found
type IS is Data:
  IS{f: U32, n: U32, sig: U32, left: U32, cur: String, hit: String}

# within a table, or at the newline after it
def IS.text(tab: Bool, last: Bool, mine: Bool, +c: U32, st: IS) -> IS:
  match tab last mine st:
    case True{} True{} True{} IS{_, _, s, _, cur, _}:
      IS{3, 0, s, 0, SNil{}, String.reverse(SCon{Chr{c}, cur})}
    case True{} True{} False{} IS{_, _, s, _, _, hit}:
      IS{3, 0, s, 0, SNil{}, hit}
    case True{} False{} _ IS{f, n, s, +l, cur, hit}:
      IS{f, n, s, (l - 1 : U32), SCon{Chr{c}, cur}, hit}
    case False{} _ _ IS{_, _, _, _, _, hit}:
      IS{0, 0, 0, 0, SNil{}, hit}

def IS.step.k(f0: Bool, f1: Bool, sp: Bool, +want: U32, +c: U32, +st: IS) -> IS:
  match f0 f1 sp st:
    case True{} _ True{} IS{_, +n, _, l, cur, hit}:
      IS{1, 0, n, l, cur, hit}
    case True{} _ False{} IS{f, +n, s, l, cur, hit}:
      Bool.pick(IS, U32.is_eq(c, 10), IS{f, n, s, l, cur, hit}, IS{f, ((n * 10 : U32) + (c - 48 : U32) : U32), s, l, cur, hit})
    case False{} True{} True{} IS{_, +n, s, _, _, hit}:
      Bool.pick(IS, U32.is_eq(n, 0), IS{3, 0, s, 0, SNil{}, hit}, IS{2, 0, s, n, SNil{}, hit})
    case False{} True{} False{} IS{f, +n, s, l, cur, hit}:
      IS{f, ((n * 10 : U32) + (c - 48 : U32) : U32), s, l, cur, hit}
    case False{} False{} _ IS{+f, n, +s, +l, cur, hit}:
      IS.text(U32.is_eq(f, 2), U32.is_eq(l, 1), U32.is_eq(s, want), c, IS{f, n, s, l, cur, hit})

def IS.step(+want: U32, +c: U32, +st: IS) -> IS:
  match st:
    case IS{+f, n, s, l, cur, hit}:
      IS.step.k(U32.is_eq(f, 0), U32.is_eq(f, 1), U32.is_eq(c, 32), want, c, IS{f, n, s, l, cur, hit})

def IS.go(t: String, +want: U32, st: IS) -> IS:
  match t:
    case SNil{}:
      st
    case SCon{Chr{+c}, rest}:
      IS.go(rest, want, IS.step(want, c, st))

def IS.hit(st: IS) -> String:
  match st:
    case IS{_, _, _, _, _, h}:
      h

# the info of piece idx of the chunk with this sig, from an info file
def Infos.find(t: String, +sig: U32, idx: Nat) -> String:
  Info.nth(idx, String.split(IS.hit(IS.go(t, sig, IS{0, 0, 0, 0, SNil{}, SNil{}})), Chr{0}))

# Plots between processes
# ----------------------
# The hub reads each file in a child process (src/server/plot.bend: a
# fresh heap every time) that prints the plot as lines: "b" and the box,
# thickness and the board's rectangle, then "c" and each chunk's sig, layer, colour, alpha, mode,
# width and the lengths of its bytes and info, a space, then exactly those
# bytes and info chars (any char may be among them) and a newline. Numbers
# are U32 decimals, the first right after its letter. Tail calls only.

def Line.chunks(cs: List<&2, Chunk>, acc: String) -> String:
  match cs:
    case Nil{}:
      acc
    case Chunk{s, l, c, a, m, w, +d, +i} <> t:
      Line.chunks(t, Out.put("c" ++ U32.show(s) ++ " " ++ U32.show(l) ++ " " ++ U32.show(c) ++ " " ++ U32.show(a) ++ " " ++ U32.show(m) ++ " " ++ U32.show(w)
        ++ " " ++ U32.show(Cb.count(d, 0)) ++ " " ++ U32.show(Cb.count(i, 0)) ++ " ", Out.pre(d, Out.pre(i, Out.put("\n", acc)))))

def Line.box(b: Box, +thick: U32, e: Box) -> String:
  match b e:
    case Box{x0, y0, x1, y1} Box{ex0, ey0, ex1, ey1}:
      "b" ++ U32.show(x0) ++ " " ++ U32.show(y0) ++ " " ++ U32.show(x1) ++ " " ++ U32.show(y1) ++ " " ++ U32.show(thick)
        ++ " " ++ U32.show(ex0) ++ " " ++ U32.show(ey0) ++ " " ++ U32.show(ex1) ++ " " ++ U32.show(ey1) ++ "\n"

def Line.nums(xs: List<&2, U32>) -> String:
  match xs:
    case Nil{}:
      ""
    case x <> t:
      " " ++ U32.show(x) ++ Line.nums(t)

# a solid's line ("" for none)
def Line.solid(so: Solid) -> String:
  match so:
    case Solid{+n, b, +d}:
      Bool.pick(String, U32.is_eq(n, 0), "", Out.put("m" ++ U32.show(n) ++ Line.nums(b) ++ " " ++ U32.show(Cb.count(d, 0)) ++ " ", Out.pre(d, "\n")))

def Line.plot(p: Plot) -> String:
  match p:
    case Plot{cs, b, +t, so, e}:
      Out.put(Line.box(b, t, e), Line.chunks(List.reverse(&2, Chunk, cs), Line.solid(so)))

# the scan's state: the line's letter (0 at a line's start), its numbers
# so far (backwards), the number being read, which field it is, how many
# raw chars were read, the chunk's bytes and info (built backwards, as a
# chunk keeps them), and what is done
type Sc is Data:
  Sc{tag: U32, nums: List<&2, U32>, n: U32, f: U32, k: U32, rev: String, info: String, cs: List<&2, Chunk>, box: Box, thick: U32, solid: Solid, edge: Box}

# a box line: the box, thickness and board rectangle
type BL is Data:
  BL{box: Box, thick: U32, edge: Box}

def Scan.box(ns: List<&2, U32>, b: Box, +t: U32, e: Box) -> BL:
  match ns:
    case ey1 <> ex1 <> ey0 <> ex0 <> th <> y1 <> x1 <> y0 <> x0 <> _:
      BL{Box{x0, y0, x1, y1}, th, Box{ex0, ey0, ex1, ey1}}
    case _:
      BL{b, t, e}

def Scan.chunk(ns: List<&2, U32>, rev: String, info: String, cs: List<&2, Chunk>) -> List<&2, Chunk>:
  match ns:
    case _ <> _ <> w <> m <> a <> c <> l <> s <> _:
      Chunk{s, l, c, a, m, w, rev, info} <> cs
    case _:
      cs

def Scan.boxed(r: BL, cs: List<&2, Chunk>, so: Solid) -> Sc:
  match r:
    case BL{b, t, e}:
      Sc{0, Nil{}, 0, 0, 0, SNil{}, SNil{}, cs, b, t, so, e}

# a solid's line: n, the box's six numbers, the byte count
def Scan.solid(ns: List<&2, U32>, rev: String, so: Solid) -> Solid:
  match ns:
    case _ <> z1 <> y1 <> x1 <> z0 <> y0 <> x0 <> n <> _:
      Solid{n, [x0, y0, z0, x1, y1, z1], rev}
    case _:
      so

def Scan.line.k(box: Bool, mesh: Bool, s: Sc) -> Sc:
  match box mesh s:
    case True{} _ Sc{_, ns, +n, _, _, _, _, cs, b, t, so, e}:
      Scan.boxed(Scan.box(n <> ns, b, t, e), cs, so)
    case False{} True{} Sc{_, ns, _, _, _, rev, _, cs, b, t, so, e}:
      Sc{0, Nil{}, 0, 0, 0, SNil{}, SNil{}, cs, b, t, Scan.solid(ns, rev, so), e}
    case False{} False{} Sc{_, ns, _, _, _, rev, info, cs, b, t, so, e}:
      Sc{0, Nil{}, 0, 0, 0, SNil{}, SNil{}, Scan.chunk(ns, rev, info, cs), b, t, so, e}

def Sc.tag(s: Sc) -> U32:
  match s:
    case Sc{t, _, _, _, _, _, _, _, _, _, _, _}:
      t

def Sc.f(s: Sc) -> U32:
  match s:
    case Sc{_, _, _, f, _, _, _, _, _, _, _, _}:
      f

# the chunk's byte count, and bytes plus info count, once all 8 numbers
# are read
def Sc.lens(mesh: Bool, ns: List<&2, U32>) -> U32 & U32:
  match mesh ns:
    case True{} +dl <> _:
      (dl, dl)
    case False{} +il <> +dl <> _:
      (dl, (dl + il : U32))
    case _ _:
      (0, 0)

def Sc.raw.k(r: U32 & U32, +k: U32) -> Bool & Bool:
  (+dl, +all) = r
  (U32.is_lt(k, all), U32.is_lt(k, dl))

# whether the next char is raw, and whether it is one of the bytes
def Sc.raw(+s: Sc) -> Bool & Bool:
  match s:
    case Sc{+t, +ns, _, +f, +k, _, _, _, _, _, _, _}:
      Bool.pick(Bool & Bool, Bool.and(Bool.or(U32.is_eq(t, 99), U32.is_eq(t, 109)), U32.is_ge(f, 8)), Sc.raw.k(Sc.lens(U32.is_eq(t, 109), ns), k), (False{}, False{}))

def Scan.step.k(raw: Bool & Bool, nl: Bool, start: Bool, sp: Bool, +c: U32, +s: Sc) -> Sc:
  (r, byte) = raw
  match r byte nl start sp s:
    case True{} True{} _ _ _ Sc{t, ns, n, f, +k, rev, info, cs, b, th, so, e}:
      Sc{t, ns, n, f, (k + 1 : U32), SCon{Chr{c}, rev}, info, cs, b, th, so, e}
    case True{} False{} _ _ _ Sc{t, ns, n, f, +k, rev, info, cs, b, th, so, e}:
      Sc{t, ns, n, f, (k + 1 : U32), rev, SCon{Chr{c}, info}, cs, b, th, so, e}
    case False{} _ True{} _ _ _:
      Scan.line.k(U32.is_eq(Sc.tag(s), 98), U32.is_eq(Sc.tag(s), 109), s)
    case False{} _ False{} True{} _ Sc{_, ns, n, f, k, rev, info, cs, b, th, so, e}:
      Sc{c, ns, n, f, k, rev, info, cs, b, th, so, e}
    case False{} _ False{} False{} True{} Sc{t, ns, +n, +f, k, rev, info, cs, b, th, so, e}:
      Sc{t, n <> ns, 0, (f + 1 : U32), k, rev, info, cs, b, th, so, e}
    case False{} _ False{} False{} False{} Sc{t, ns, +n, f, k, rev, info, cs, b, th, so, e}:
      Sc{t, ns, ((n * 10 : U32) + (c - 48 : U32) : U32), f, k, rev, info, cs, b, th, so, e}

def Scan.step(+c: U32, +s: Sc) -> Sc:
  Scan.step.k(Sc.raw(s), U32.is_eq(c, 10), U32.is_eq(Sc.tag(s), 0), U32.is_eq(c, 32), c, s)

def Scan.go(t: String, s: Sc) -> Sc:
  match t:
    case SNil{}:
      s
    case SCon{Chr{+c}, rest}:
      Scan.go(rest, Scan.step(c, s))

def Scan.fin(s: Sc) -> Plot:
  match s:
    case Sc{_, _, _, _, _, _, _, cs, b, t, so, e}:
      Plot{List.reverse(&2, Chunk, cs), b, t, so, e}

# the plot a child printed (Line.plot)
def Scan.plot(t: String) -> Plot:
  Scan.fin(Scan.go(t, Sc{0, Nil{}, 0, 0, 0, SNil{}, SNil{}, Nil{}, Box.empty(), 1600, Solid.none(), Box.empty()}))

# no plot for the source (yet): why, for the phone to show
def Plot.none(+key: String, +why: String) -> String:
  J.Json.show(J.Json.obj([J.KV{"t", J.Str{"plot"}}, J.KV{"key", J.Str{key}}, J.KV{"none", J.Str{why}}]))
