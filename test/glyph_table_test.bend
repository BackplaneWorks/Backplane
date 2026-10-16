import Base
import ./check.bend as C
import ../src/gfx/raster.bend as R
import ../src/gfx/draw.bend as D
import ../src/gfx/text.bend as X
import ../src/gfx/tiles.bend as T
import ../src/core/utf8.bend as U

# Table changes must invalidate even identical keyed ops; resolving keys
# must preserve the original mask-bearing layout's exact rendered bytes.
def bytes(xs: List<&2, U32>, ys: List<&2, U32>, ok: Bool) -> Bool:
  match xs ys ok:
    case _ _ False{}:
      False{}
    case Nil{} Nil{} True{}:
      True{}
    case x <> xt y <> yt True{}:
      bytes(xt, yt, U32.is_eq(x, y))
    case _ _ True{}:
      False{}

def same(+a: Image, +b: Image) -> String:
  Bool.show(bytes(D.Snap.ppm(a, 80, 40), D.Snap.ppm(b, 80, 40), True{}))

def glyph(+cov: U32) -> X.TG:
  X.TG{256, 32768, 32770, 2, 2, 0n, R.MWord{cov}}

def face() -> X.Face:
  X.Face{1, True{}, 14, 11, 3, 18, 256, "test", X.GTab.none(7n)}

def cache(+cov: U32) -> Map<&2, X.TG>:
  Map.set(&2, X.TG, Map.set(&2, X.TG, Map.new(&2, X.TG), X.Text.key(1, 65), glyph(cov)), X.Text.key(1, 26085), glyph(cov))

def ops(+c: Map<&2, X.TG>) -> List<&2, R.Op>:
  X.Text.draw(c, face(), "A日", D.Clip.all(), 62, 8, 16777215)

def image(os: List<&2, R.Op>) -> Image:
  R.Frame.render(80, 40, os, 0)

def fresh(+os: List<&2, R.Op>, +tab: R.Tab) -> Image:
  T.Tiles.image(T.Frame.make(80, 40, R.Key.ops(os), Pix{0}, tab))

def changed(+os: List<&2, R.Op>, +old: R.Tab, +new: R.Tab) -> Image:
  T.Tiles.image(T.Frame.par(80, 40, R.Key.ops(os), Pix{0}, new, T.FSome{80, 40, old, T.Frame.make(80, 40, R.Key.ops(os), Pix{0}, old)}, 64))

def main() -> IO(Unit):
  do IO<Unit>:
    +c : Map<&2, X.TG> = cache(4294967295)
    +tab : R.Tab = X.Text.table(c)
    +os : List<&2, R.Op> = ops(c)
    +keys : List<&2, R.Op> = R.Key.ops(os)
    +new : R.Tab = X.Text.table(cache(16744448))
    +missing : R.Tab = R.TNil{}
    C.Check.str("layout emits keys directly", Bool.show(T.Eq.ops(X.Text.keydraw(c, face(), "A日", D.Clip.all(), 62, 8, 16777215), keys)), "True")
    C.Check.str("keyed resolution recovers original ops", Bool.show(T.Eq.ops(R.Res.ops(keys, tab), os)), "True")
    C.Check.str("keyed tiles preserve original RGlyph bytes", same(fresh(os, tab), image(os)), "True")
    C.Check.str("same ops and ids, replacement masks draw fresh", same(changed(os, tab, new), image(R.Res.ops(keys, new))), "True")
    C.Check.str("replacement actually changes pixels", same(changed(os, tab, new), fresh(os, tab)), "False")
    C.Check.str("same ops and ids, missing entry draws fresh", same(changed(os, tab, missing), image(R.Res.ops(keys, missing))), "True")
    C.Check.str("missing entries remove old pixels", same(changed(os, tab, missing), fresh(os, tab)), "False")
    +cold : Map<&2, X.TG> = Map.new(&2, X.TG)
    +ascii : Map<&2, X.TG> = Map.set(&2, X.TG, cold, X.Text.key(1, 65), glyph(4294967295))
    C.Check.str("cold stream requests ASCII and nonASCII", Nat.show(List.length(&2, X.Want, X.Text.wants(ops(cold), Nil{}))), "2")
    C.Check.str("partial stream requests only missing nonASCII", Nat.show(List.length(&2, X.Want, X.Text.wants(ops(ascii), Nil{}))), "1")
    C.Check.str("warm stream has no wants", Nat.show(List.length(&2, X.Want, X.Text.wants(os, Nil{}))), "0")
    C.Check.str("warm nonASCII table preserves bytes", same(fresh(os, tab), image(os)), "True")
    C.Check.str("native maximum face id fits packed id", U32.show(U32.shrn(X.Text.gid(255, 1114111), 21n)), "255")
    C.Check.str("maximum scalar survives packed id", U32.show(U32.and(X.Text.gid(255, 1114111), 2097151)), "1114111")
    C.Check.str("invalid UTF8 does not introduce oversized cp", U.Utf8.decode([244, 144, 128, 128]), "�")
