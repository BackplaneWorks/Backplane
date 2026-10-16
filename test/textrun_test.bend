import Base
import ./check.bend as C
import ../src/gfx/raster.bend as R
import ../src/gfx/draw.bend as D
import ../src/gfx/text.bend as X
import ../src/gfx/tiles.bend as T

# A line of text as one op (text.bend's Text.draw: an R.SRun) draws the
# same pixels as one op per glyph (Text.run), over what lies below, cut to
# a clip, with glyphs that overlap, glyphs above the frame's top and left
# of its edge, and glyphs not in the cache yet (wants); and the same again
# when the frame is drawn as tiles.

# a made-up glyph for cp: its advance, bearings and coverage vary with it
def cov(n: Nat, +cp: U32, acc: List<&2, U32>) -> List<&2, U32>:
  match n:
    case 0n:
      acc
    case 1n+i0:
      +k = i0
      +i = U32.from_nat(k)
      cov(k, cp, U32.or(U32.and(((i * 37 : U32) + cp : U32), 255), U32.shln(U32.and(((i * 91 : U32) + (cp * 3 : U32) : U32), 255), 8n)) <> acc)

def glyph(+cp: U32) -> X.TG:
  +w = (4 + U32.and(cp, 7) : U32)
  +h = (9 + U32.and(cp, 3) : U32)
  X.TG.decode(List.append(&2, U32, [0, (448 + (U32.and(cp, 15) * 16 : U32) : U32), (32766 + U32.and(cp, 3) : U32), (32768 + 11 : U32), w, h], cov(U32.to_nat((((w * h : U32) + 3 : U32) / 4 : U32)), cp, Nil{})), 448)

def fill(cs: List<&2, U32>, cache: Map<&2, X.TG>) -> Map<&2, X.TG>:
  match cs:
    case Nil{}:
      cache
    case +cp <> t:
      fill(t, Map.set(&2, X.TG, cache, X.Text.key(1, cp), glyph(cp)))

def cps(s: String) -> List<&2, U32>:
  match s:
    case SNil{}:
      Nil{}
    case SCon{Chr{c}, t}:
      c <> cps(t)

def face() -> X.Face:
  X.Face{1, True{}, 14, 11, 3, 18, 448, "made up", X.GTab.none(7n)}

# every glyph but "q" is in the cache
def cache() -> Map<&2, X.TG>:
  fill(cps("Hello, wörld! The quick brown fx jumps 0123456789 →"), Map.new(&2, X.TG))

def under() -> List<&2, R.Op>:
  [D.Draw.rect(D.Clip.all(), 20, 4, 90, 30, 3381759, 128), D.Draw.rect(D.Clip.all(), 60, 20, 200, 40, 16711680, 255)]

type Line is Data:
  Line{s: String, c: D.Clip, x: U32, base: U32, color: U32}

def lines() -> List<&2, Line>:
  [Line{"Hello, wörld! The quick brown fx", D.Clip.all(), 3, 20, 15790320},
   Line{"jumps 0123456789 → q", D.Clip{30, 25, 150, 60}, 1, 38, 1118481},
   Line{"Hello above the top", D.Clip.all(), 10, 5, 8421504},
   Line{"left edge", D.Clip.all(), 0, 52, 65280},
   Line{"   ", D.Clip.all(), 40, 52, 255},
   Line{"", D.Clip.all(), 40, 52, 255}]

def ops.old(ls: List<&2, Line>, +cache: Map<&2, X.TG>) -> List<&2, R.Op>:
  match ls:
    case Nil{}:
      under()
    case Line{s, +c, +x, +b, +col} <> t:
      List.append(&2, R.Op, X.Text.run(s, cache, face(), c, x, 0, b, col), ops.old(t, cache))

def ops.new(ls: List<&2, Line>, +cache: Map<&2, X.TG>) -> List<&2, R.Op>:
  match ls:
    case Nil{}:
      under()
    case Line{s, +c, +x, +b, +col} <> t:
      List.append(&2, R.Op, X.Text.draw(cache, face(), s, c, x, b, col), ops.new(t, cache))

# pixels of two images (each 2^d wide) that differ in the w x h frame
def diff(n: Nat, +a: Image, +b: Image, +d: Nat, +w: U32, +acc: Nat) -> Nat:
  match n:
    case 0n:
      acc
    case 1n+i0:
      +i = i0
      +x = U32.from_nat(Nat.mod(i, U32.to_nat(w)))
      +y = U32.from_nat(Nat.div(i, U32.to_nat(w)))
      diff(i, a, b, d, w, Bool.pick(Nat, U32.is_eq(R.Img.at(d, a, x, y), R.Img.at(d, b, x, y)), acc, 1n+acc))

def wsum(ws: List<&2, X.Want>, acc: U32) -> U32:
  match ws:
    case Nil{}:
      acc
    case X.Want{_, +cp, _} <> t:
      wsum(t, (acc + cp : U32))

# how many glyphs are wanted, and their code points summed
def wants(+os: List<&2, R.Op>) -> String:
  Nat.show(List.length(&2, X.Want, X.Text.wants(os, Nil{}))) ++ "/" ++ U32.show(wsum(X.Text.wants(os, Nil{}), 0))

def W() -> U32:
  200

def H() -> U32:
  60

def main() -> IO(Unit):
  do IO<Unit>:
    +c : Map<&2, X.TG> = cache()
    +old : List<&2, R.Op> = ops.old(lines(), c)
    +new : List<&2, R.Op> = ops.new(lines(), c)
    +d : Nat = R.Frame.depth(W(), H())
    +bg : U32 = 4279383126
    +a : Image = R.Frame.render(W(), H(), old, bg)
    +b : Image = R.Frame.render(W(), H(), new, bg)
    +tb : Image = T.Tiles.image(T.Frame.make(W(), H(), new, Pix{bg}, X.Text.table(c)))
    +ta : Image = T.Tiles.image(T.Frame.make(W(), H(), old, Pix{bg}, X.Text.table(c)))
    C.Check.str("a run draws as its glyphs", Nat.show(diff(U32.to_nat((W() * H() : U32)), a, b, d, W(), 0n)), "0")
    C.Check.str("a run draws as its glyphs in tiles", Nat.show(diff(U32.to_nat((W() * H() : U32)), ta, tb, d, W(), 0n)), "0")
    C.Check.str("tiles draw a run as the frame does", Nat.show(diff(U32.to_nat((W() * H() : U32)), a, tb, d, W(), 0n)), "0")
    C.Check.str("the frame draws something", Bool.show(Nat.is_gt(diff(U32.to_nat((W() * H() : U32)), a, R.Frame.render(W(), H(), under(), bg), d, W(), 0n), 500n)), "True")
    C.Check.str("one op a line, wants after", Nat.show(Nat.sub(List.length(&2, R.Op, new), List.length(&2, X.Want, X.Text.wants(new, Nil{})))), "6")
    C.Check.str("glyph ops before", Bool.show(Nat.is_gt(List.length(&2, R.Op, old), 60n)), "True")
    C.Check.str("the same glyphs wanted", wants(new), wants(old))
