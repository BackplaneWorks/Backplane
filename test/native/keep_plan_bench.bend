import Base
import ../check.bend as C
import ../keep_step_test.bend as K
import ../../src/gfx/text.bend as X
import ../../src/gfx/raster.bend as R
import ../../src/gfx/draw.bend as D
import ../../src/gfx/tiles.bend as T
import ../../src/server/perf.bend as P

# The previous production comparator, kept independent of Keep.step.
def Ref.keep(os: List<&2, R.Op>, +px: U32, +py: U32, +rx: U32, +ry: U32, +n: U32, +ys: List<&2, R.Op>, ok: Bool) -> Bool:
  match os ok:
    case _ False{}: False{}
    case Nil{} True{}: T.Eq.ops.go(Nil{}, ys, True{})
    case +o <> t True{}:
      Ref.keep(t, px, py, rx, ry, n, T.Keep.ys(o, px, py, rx, ry, n, ys), T.Keep.ok(o, px, py, rx, ry, n, ys))

def Ref.eq(c: T.Cut, +ys: List<&2, R.Op>) -> Bool:
  match c:
    case T.CAll{o}: T.Eq.ops(o, ys)
    case T.CIn{o, px, py, rx, ry, n}: Ref.keep(o, px, py, rx, ry, n, ys, True{})

def Ref.same(c: T.Cut, +base: Image, t: T.Tiles) -> Bool:
  match t:
    case T.TLeaf{_}: False{}
    case T.TNode{o, b, _, _, _, _}: T.Eq.tile.k(Ref.eq(c, o), base, b)

def Ref.leaf(+e: Nat, +c: T.Cut, +base: Image, t: T.Tile, p: T.Past, +rx: U32, +ry: U32, +n: U32) -> T.Plan:
  match t:
    case T.Tile{+o, l, +b, i}:
      T.Tile.plan2.k(T.Eq.tile.k(Ref.eq(c, o), base, b), e, c, base, T.Tile{o, l, b, i}, p, rx, ry, n)

def Ref.plan(+k: Nat, +e: Nat, c: T.Cut, +base: Image, prev: T.Tiles, +p: T.Past, +rx: U32, +ry: U32, +n: U32, same: Bool) -> T.Plan:
  match k prev same:
    case 0n T.TLeaf{t} _: Ref.leaf(e, c, base, t, p, rx, ry, n)
    case 1n+j0 T.TNode{o, b, pa, pb, pc, pd} True{}: T.PKeepN{T.TNode{o, b, pa, pb, pc, pd}}
    case 1n+j0 T.TNode{_, _, +pa, +pb, +pc, +pd} False{}:
      +j = j0
      +h = U32.shr(n)
      +os = T.Cut.get(c)
      T.Plan.node(os, base,
        Ref.plan(j, e, T.CIn{os, rx, ry, rx, ry, h}, T.Img.q(base, 0n), pa, p, rx, ry, h, Ref.same(T.CIn{os, rx, ry, rx, ry, h}, T.Img.q(base, 0n), pa)),
        Ref.plan(j, e, T.CIn{os, rx, ry, (rx + h : U32), ry, h}, T.Img.q(base, 1n), pb, p, (rx + h : U32), ry, h, Ref.same(T.CIn{os, rx, ry, (rx + h : U32), ry, h}, T.Img.q(base, 1n), pb)),
        Ref.plan(j, e, T.CIn{os, rx, ry, rx, (ry + h : U32), h}, T.Img.q(base, 2n), pc, p, rx, (ry + h : U32), h, Ref.same(T.CIn{os, rx, ry, rx, (ry + h : U32), h}, T.Img.q(base, 2n), pc)),
        Ref.plan(j, e, T.CIn{os, rx, ry, (rx + h : U32), (ry + h : U32), h}, T.Img.q(base, 3n), pd, p, (rx + h : U32), (ry + h : U32), h, Ref.same(T.CIn{os, rx, ry, (rx + h : U32), (ry + h : U32), h}, T.Img.q(base, 3n), pd)))
    case _ _ _: T.Tiles.plan.make(k, e, T.Cut.get(c), base, rx, ry, n)

def Ref.frame(+os: List<&2, R.Op>, +prev: T.Tiles) -> T.Plan:
  Ref.plan(5n, 6n, T.CAll{os}, Pix{0}, prev, T.Past{5n, prev, 2048, 0}, 0, 0, 2048, T.Tiles.same(os, Pix{0}, prev))

def rows(+n: Nat, +c: Map<&2, X.TG>, +f: X.Face) -> List<&2, R.Op>:
  match n:
    case 0n: Nil{}
    case 1n+k:
      List.append(&2, R.Op, X.Text.draw(c, f, "The regulator enable pin is pulled up through R12 to VBAT; check U3 footprint, then rerun DRC.", D.Clip.all(), 300, U32.from_nat(Nat.add(24n, Nat.mul(k, 19n))), 15790320), rows(k, c, f))

def sample(+n: Nat, +os: List<&2, R.Op>, +prev: T.Tiles) -> IO(Unit):
  match n:
    case 0n: IO.pure(Unit, Unit{})
    case 1n+k:
      do IO<Unit>:
        a : U32 <- P.Perf.us()
        old : Nat = T.Plan.count(Ref.frame(os, prev))
        +b : U32 <- P.Perf.us()
        new : Nat = T.Plan.count(T.Frame.plan.k(True{}, 1400, 900, os, Pix{0}, prev, 0))
        z : U32 <- P.Perf.us()
        IO.print("sample," ++ Nat.show(k) ++ "," ++ U32.show((b - a : U32)) ++ "," ++ U32.show((z - b : U32)) ++ "," ++ Nat.show(old) ++ "," ++ Nat.show(new))
        sample(k, os, prev)

def main() -> IO(Unit):
  do IO<Unit>:
    K.main()
    +f : X.Face <- X.Text.face(["monospace"], 14)
    +c : Map<&2, X.TG> <- X.Text.warm(Map.new(&2, X.TG), f)
    +os : List<&2, R.Op> = rows(44n, c, f)
    +old : T.Tiles = T.Frame.make(1400, 900, os, Pix{0})
    +changed : List<&2, R.Op> = List.append(&2, R.Op, X.Text.draw(c, f, "Backplane typed", D.Clip.all(), 300, 840, 16777215), os)
    +a : Image = T.Tiles.image(T.Plan.run(Ref.frame(changed, old), 6n))
    +b : Image = T.Tiles.image(T.Plan.run(T.Frame.plan.k(True{}, 1400, 900, changed, Pix{0}, old, 0), 6n))
    C.Check.str("whole plan exact image", Bool.show(T.Eq.image(a, b)), "True")
    sample(100n, changed, old)
