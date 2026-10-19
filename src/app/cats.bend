import Base
import ../core/cat.bend as C
import ../gfx/cat.bend as G
import ../gfx/raster.bend as R
import ../gfx/draw.bend as D

# The bots' cats in the window (docs/bots.md, core/cat.bend). Before each
# frame the window warms a cache (main.bend's Kept, bots.bend's Bv.warm):
# each cat on show is flattened once per (look, mood) and posed, as draw
# ops, once per tick of the cat clock (and per size), away from the
# window's corner. Layout then only moves those ops to where the cat sits
# (Cats.draw). The clock ticks at ~30 frames a second while a shown cat
# moves (main.bend's Shell.cats); frames between ticks, a key typed say,
# reuse the ops as they are, and tiles.bend redraws nothing there.

# a cat flattened for a look and mood, and posed at `clock` as ops for a
# px square at (Cats.at(), Cats.at()); ok is False for "not in the cache"
type CatF is Data:
  CatF{ok: Bool, parts: List<&2, C.FPart>, clock: Nat, ops: List<&2, R.Op>}
  CatAt{parts: List<&2, C.FPart>, clock: Nat, ops: List<&2, R.Op>, x: U32, y: U32}

def Cats.none() -> Map<&2, CatF>:
  Map.new(&2, CatF)

def Cats.blank() -> CatF:
  CatF{False{}, Nil{}, 0n, Nil{}}

# where cached cats are posed: off the frame's top-left corner, so moving
# a polygon never changes which side of it that corner is on
def Cats.at() -> U32:
  1000

# whether a mood moves (a sleeping, sick or absent cat holds still)
def Cats.moving(+m: String) -> Bool:
  C.Cat.moving(m)

def Cats.key(+look: Nat, +mood: String, +px: U32) -> String:
  Nat.show(look) ++ ":" ++ mood ++ ":" ++ U32.show(px)

def Cats.found(r: Map<&2, CatF> & CatF) -> CatF:
  (_, f) = r
  f

def Cats.find(+c: Map<&2, CatF>, +look: Nat, +mood: String, +px: U32) -> CatF:
  Cats.found(Map.get(CatF, Cats.blank(), c, Cats.key(look, mood, px)))

# a flattened cat posed for time ms, as ops in a px square at (x, y),
# topmost first, as a frame's ops are when the window draws them: the
# cats go into the frame after the layout is done (bots.bend's Spot,
# Fronted.of), so they are not turned around by Lay.rev with the rest.
# Cat.ops gives them in that order. (When the layout drew the cats among
# its own ops, before Lay.rev, they were reversed here to come out right;
# reversed now, each cat painted its back over its face.)
def Cats.pose(+ps: List<&2, C.FPart>, +ms: Nat, +c: D.Clip, +x: F32, +y: F32, +px: F32) -> List<&2, R.Op>:
  G.Cat.ops(C.Cat.pose.at(ps, ms), c, x, y, px)

def Cats.parts.k(f: CatF, +look: Nat, +mood: String) -> List<&2, C.FPart>:
  match f:
    case CatF{True{}, ps, _, _}:
      ps
    case CatF{False{}, _, _, _}:
      C.Cat.flat(look, mood)
    case CatAt{ps, _, _, _, _}:
      ps

def Cats.posed(+ps: List<&2, C.FPart>, +clock: Nat, +px: U32) -> CatF:
  CatF{True{}, ps, clock, Cats.pose(ps, clock, D.Clip.all(), U32.to_f32(Cats.at()), U32.to_f32(Cats.at()), U32.to_f32(px))}

def Cats.fresh(f: CatF, +clock: Nat) -> Bool:
  match f:
    case CatF{+ok, _, +k, _}:
      Bool.and(ok, Nat.is_eq(k, clock))
    case CatAt{_, k, _, _, _}:
      Nat.is_eq(k, clock)

def Cats.warm.k(fresh: Bool, c: Map<&2, CatF>, f: CatF, +look: Nat, +mood: String, +px: U32, +clock: Nat) -> Map<&2, CatF>:
  match fresh:
    case True{}:
      c
    case False{}:
      Map.set(&2, CatF, c, Cats.key(look, mood, px), Cats.posed(Cats.parts.k(f, look, mood), clock, px))

# the cache with this cat posed at the clock (flattened only when new)
def Cats.warm(+c: Map<&2, CatF>, +look: Nat, +mood: String, +px: U32, +clock: Nat) -> Map<&2, CatF>:
  Cats.warm.k(Cats.fresh(Cats.find(c, look, mood, px), clock), c, Cats.find(c, look, mood, px), look, mood, px, clock)

# A newly requested size is posed at its actual coordinates: posing at
# 1000 then translating can round small coverage masks differently. There
# is still only one entry per look/mood/size, even when the cat moves.
def Cats.placed(+ps: List<&2, C.FPart>, +clock: Nat, +x: U32, +y: U32, +px: U32) -> CatF:
  CatAt{ps, clock, Cats.pose(ps, clock, D.Clip.all(), U32.to_f32(x), U32.to_f32(y), U32.to_f32(px)), x, y}

def Cats.visible.fresh(f: CatF, +clock: Nat, +x: U32, +y: U32) -> Bool:
  match f:
    case CatF{+ok, _, k, _}:
      Bool.and(ok, Nat.is_eq(k, clock))
    case CatAt{_, k, _, ox, oy}:
      Bool.and(Nat.is_eq(k, clock), Bool.and(U32.is_eq(x, ox), U32.is_eq(y, oy)))

def Cats.visible.keep(same: Bool, c: Map<&2, CatF>, f: CatF, +look: Nat, +mood: String, +px: U32, +clock: Nat, +x: U32, +y: U32) -> Map<&2, CatF>:
  match same:
    case True{}: c
    case False{}:
      Map.set(&2, CatF, c, Cats.key(look, mood, px), Cats.placed(Cats.parts.k(f, look, mood), clock, x, y, px))

def Cats.visible(+c: Map<&2, CatF>, +look: Nat, +mood: String, +px: U32, +clock: Nat, +x: U32, +y: U32) -> Map<&2, CatF>:
  Cats.visible.keep(Cats.visible.fresh(Cats.find(c, look, mood, px), clock, x, y), c, Cats.find(c, look, mood, px), look, mood, px, clock, x, y)

# Moving posed ops
# ----------------

def Cats.edges(es: List<&2, R.Edge>, +dx: F32, +dy: F32) -> List<&2, R.Edge>:
  match es:
    case Nil{}:
      Nil{}
    case R.Edge{ax, ay, bx, by} <> t:
      R.Edge{F32.add(ax, dx), F32.add(ay, dy), F32.add(bx, dx), F32.add(by, dy)} <> Cats.edges(t, dx, dy)

def Cats.shape(s: R.Shape, +ix: U32, +iy: U32, +dx: F32, +dy: F32) -> R.Shape:
  match s:
    case R.SRect{}:
      R.SRect{}
    case R.SPoly{i, es}:
      R.SPoly{i, Cats.edges(es, dx, dy)}
    case R.SGlyph{gx, gy, g}:
      R.SGlyph{(gx + ix : U32), (gy + iy : U32), g}
    case R.SSeg{ax, ay, bx, by, hw}:
      R.SSeg{F32.add(ax, dx), F32.add(ay, dy), F32.add(bx, dx), F32.add(by, dy), hw}
    case R.SDisc{cx, cy, r}:
      R.SDisc{F32.add(cx, dx), F32.add(cy, dy), r}
    case R.STri{ax, ay, bx, by, cx, cy}:
      R.STri{F32.add(ax, dx), F32.add(ay, dy), F32.add(bx, dx), F32.add(by, dy), F32.add(cx, dx), F32.add(cy, dy)}
    case R.SMask{gx, gy, w, d, m}:
      R.SMask{(gx + ix : U32), (gy + iy : U32), w, d, m}
    case R.SRun{ox, oy, r}:
      R.SRun{(ox + ix : U32), (oy + iy : U32), r}

# ops posed at (Cats.at(), Cats.at()) moved to (x, y) and cut to c
def Cats.move(os: List<&2, R.Op>, +c: D.Clip, +ix: U32, +iy: U32, +dx: F32, +dy: F32) -> List<&2, R.Op>:
  match os:
    case Nil{}:
      Nil{}
    case R.Op{x0, y0, x1, y1, +col, +a, s} <> t:
      D.Clip.cut(c, (x0 + ix : U32), (y0 + iy : U32), (x1 + ix : U32), (y1 + iy : U32), col, a, Cats.shape(s, ix, iy, dx, dy)) <> Cats.move(t, c, ix, iy, dx, dy)

def Cats.draw.at(same: Bool, os: List<&2, R.Op>, ps: List<&2, C.FPart>, +clock: Nat, +c: D.Clip, +x: U32, +y: U32, +px: U32) -> List<&2, R.Op>:
  match same:
    case True{}: Cats.move(os, c, 0, 0, 0.0, 0.0)
    case False{}: Cats.pose(ps, clock, c, U32.to_f32(x), U32.to_f32(y), U32.to_f32(px))

def Cats.draw.k(f: CatF, +look: Nat, +mood: String, +ms: Nat, +c: D.Clip, +x: U32, +y: U32, +px: U32) -> List<&2, R.Op>:
  match f:
    case CatF{True{}, _, _, os}:
      Cats.move(os, c, (x - Cats.at() : U32), (y - Cats.at() : U32), F32.sub(U32.to_f32(x), U32.to_f32(Cats.at())), F32.sub(U32.to_f32(y), U32.to_f32(Cats.at())))
    case CatF{False{}, _, _, _}:
      Cats.pose(C.Cat.flat(look, mood), ms, c, U32.to_f32(x), U32.to_f32(y), U32.to_f32(px))
    case CatAt{ps, clock, os, ox, oy}:
      Cats.draw.at(Bool.and(U32.is_eq(x, ox), U32.is_eq(y, oy)), os, ps, clock, c, x, y, px)

# a cat drawn from the cache (flattened and posed now, at ms, when the
# cache lacks it)
def Cats.draw(+cache: Map<&2, CatF>, +look: Nat, +mood: String, +ms: Nat, +c: D.Clip, +x: U32, +y: U32, +px: U32) -> List<&2, R.Op>:
  Cats.draw.k(Cats.find(cache, look, mood, px), look, mood, ms, c, x, y, px)
