import Base
import ./check.bend as C
import ../src/gfx/raster.bend as R
import ../src/gfx/draw.bend as D
import ../src/gfx/text.bend as X
import ../src/gfx/tiles.bend as T

def Fixture.bytes(xs: List<&2, U32>, ys: List<&2, U32>, ok: Bool) -> Bool:
  match xs ys ok:
    case _ _ False{}:
      False{}
    case Nil{} Nil{} True{}:
      True{}
    case x <> xt y <> yt True{}:
      Fixture.bytes(xt, yt, U32.is_eq(x, y))
    case _ _ True{}:
      False{}

def Fixture.same(+a: Image, +b: Image) -> String:
  Bool.show(Fixture.bytes(D.Snap.ppm(a, 80, 40), D.Snap.ppm(b, 80, 40), True{}))

def Fixture.glyph(+cov: U32) -> X.TG:
  X.TG{256, 32768, 32770, 2, 2, 0n, R.MWord{cov}}

def Fixture.face() -> X.Face:
  X.Face{1, True{}, 14, 11, 3, 18, 256, "test", X.GTab.none(7n)}

def Fixture.cache(+cov: U32) -> Map<&2, X.TG>:
  Map.set(&2, X.TG, Map.set(&2, X.TG, Map.new(&2, X.TG), X.Text.key(1, 65), Fixture.glyph(cov)), X.Text.key(1, 26085), Fixture.glyph(4278190335))

# Compare with the independently materialized Region.keep list, including
# drops, empty clipped runs, missing/extra/changed ops and changed bases.
def ops() -> List<&2, R.Op>:
  List.concat(&2, R.Op, [
    X.Text.draw(Fixture.cache(4294967295), Fixture.face(), "A日A日", D.Clip.all(), 61, 8, 16777215),
    [R.Op{30, 1, 78, 35, 16711680, 145, R.SPoly{False{}, [R.Edge{30.0, 1.0, 78.0, 12.0}, R.Edge{78.0, 12.0, 38.0, 35.0}, R.Edge{38.0, 35.0, 30.0, 1.0}]}},
     R.Op{2, 2, 79, 39, 65535, 255, R.SRun{0, 0, R.RNone{}}},
     D.Draw.rect(D.Clip.all(), 76, 30, 4, 9, 255, 127),
     D.Draw.rect(D.Clip.all(), 0, 0, 80, 40, 2236962, 255)]])

def polygon(os: List<&2, R.Op>) -> List<&2, R.Op>:
  match os:
    case Nil{}: Nil{}
    case R.Op{x0, y0, x1, y1, _, a, R.SPoly{i, es}} <> t:
      R.Op{x0, y0, x1, y1, 65280, a, R.SPoly{i, es}} <> t
    case o <> t:
      o <> polygon(t)

def region(+os: List<&2, R.Op>, +x: U32, +y: U32, +n: U32, +ys: List<&2, R.Op>, +label: String) -> IO(Unit):
  do IO<Unit>:
    C.Check.str(label, Bool.show(T.Keep.eq(os, 0, 0, x, y, n, ys, True{})), Bool.show(T.Eq.ops(R.Region.keep(os, 0, 0, x, y, n), ys)))
    C.Check.str(label ++ " false flag", Bool.show(T.Keep.eq(os, 0, 0, x, y, n, ys, False{})), "False")

def regions(+os: List<&2, R.Op>, +x: U32, +y: U32, +n: U32) -> IO(Unit):
  +ys = R.Region.keep(os, 0, 0, x, y, n)
  +tag = U32.show(x) ++ "," ++ U32.show(y) ++ "," ++ U32.show(n)
  do IO<Unit>:
    region(os, x, y, n, ys, tag ++ " equal")
    region(os, x, y, n, Nil{}, tag ++ " removed")
    region(os, x, y, n, D.Draw.rect(D.Clip.all(), 0, 0, 1, 1, 99, 255) <> ys, tag ++ " inserted")
    region(os, x, y, n, List.reverse(&2, R.Op, ys), tag ++ " reordered")

def exact(+os: List<&2, R.Op>, +base: Image, +prev: T.Frames, +dy: U32, label: String) -> IO(Unit):
  +img = T.Tiles.image(T.Frame.par(80, 40, os, base, prev, dy))
  +fresh = T.Tiles.image(T.Frame.make(80, 40, os, base))
  C.Check.str(label, Fixture.same(img, fresh), "True")

def main() -> IO(Unit):
  do IO<Unit>:
    +os : List<&2, R.Op> = ops()
    regions(os, 0, 0, 128)
    regions(os, 0, 0, 64)
    regions(os, 64, 0, 64)
    regions(os, 16, 16, 16)
    regions(os, 256, 256, 32)
    +prev : T.Frames = T.FSome{80, 40, T.Frame.make(80, 40, os, Pix{0})}
    +changed : List<&2, R.Op> = X.Text.draw(Fixture.cache(4294967295), Fixture.face(), "日日AA", D.Clip.all(), 61, 8, 16777215)
    exact(os, Pix{0}, prev, 0, "unchanged exact pixels")
    exact(changed, Pix{0}, prev, 0, "changed run exact pixels")
    exact(List.reverse(&2, R.Op, os), Pix{0}, prev, 0, "reordered ops exact pixels")
    exact(os, Qua{Pix{1}, Pix{2}, Pix{3}, Pix{4}}, prev, 0, "changed base exact pixels")
    exact(changed, Pix{0}, prev, 64, "scroll hint still exact pixels")
    exact(polygon(os), Pix{0}, prev, 0, "changed polygon exact pixels")
    C.Check.str("changed polygon negative control", Fixture.same(T.Tiles.image(T.Frame.make(80, 40, polygon(os), Pix{0})), T.Tiles.image(T.Frame.make(80, 40, os, Pix{0}))), "False")
    C.Check.str("changed run negative control", Fixture.same(T.Tiles.image(T.Frame.make(80, 40, changed, Pix{0})), T.Tiles.image(T.Frame.make(80, 40, os, Pix{0}))), "False")
