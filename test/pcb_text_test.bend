import Base
import ./check.bend as C
import ./sch_test.bend as T
import ../src/core/board.bend as B
import ../src/core/pcb2d.bend as P
import ../src/core/utf8.bend as U
import ../src/core/strokefont.bend as ST
import ../src/app/viewer.bend as V
import ../src/gfx/draw.bend as D
import ../src/gfx/raster.bend as R

def parse(s: String) -> List<&2, B.Item>:
  T.items(P.Pcb2.items(U.Utf8.encode(s)))

def text(+s: String) -> String:
  "(kicad_pcb (gr_text \"" ++ s ++ "\" (at 10 20 0) (layer \"F.SilkS\") (uuid label) (effects (font (size 1 1) (thickness 0.125)))))"

def prims(xs: List<&2, B.Item>) -> List<&2, B.Prim>:
  match xs:
    case Nil{}:
      Nil{}
    case B.Item{_, _, _, p, _} <> t:
      p <> prims(t)

def near(+a: F32, +b: F32) -> Bool:
  F32.is_lt(F32.abs(F32.sub(a, b)), 0.0001)

def points(a: List<&2, B.Pt>, b: List<&2, B.Pt>) -> Bool:
  match a b:
    case Nil{} Nil{}:
      True{}
    case B.Pt{x, y} <> t B.Pt{u, v} <> r:
      Bool.and(Bool.and(near(x, u), near(y, v)), points(t, r))
    case _ _:
      False{}

def strokes(a: List<&2, B.Prim>, b: List<&2, B.Prim>) -> Bool:
  match a b:
    case Nil{} Nil{}:
      True{}
    case B.PLine{p, w} <> t B.PLine{q, v} <> r:
      Bool.and(Bool.and(points(p, q), near(w, v)), strokes(t, r))
    case _ _:
      False{}

def keys(xs: List<&2, B.Item>) -> String:
  match xs:
    case Nil{}:
      ""
    case B.Item{k, _, _, _, _} <> t:
      k

def signatures(xs: List<&2, B.Item>) -> String:
  match xs:
    case Nil{}:
      ""
    case B.Item{_, s, _, _, _} <> t:
      U32.show(s) ++ ";" ++ signatures(t)

def frame(xs: List<&2, B.Item>, +flip: U32) -> Image:
  +seen = V.Scene.update(Nil{}, V.Scene.sort(xs), 0n)
  R.Frame.render(128, 128, List.reverse(&2, R.Op, V.Ops.scene(seen, V.View{4.0, 0.0, 0.0}, D.Clip{0, 0, 128, 128}, 10000n, 10000n, True{}, 0, flip)), 0)

def black(i: Image) -> Bool:
  match i:
    case Pix{x}:
      U32.is_eq(x, 0)
    case Qua{a, b, c, d}:
      Bool.and(Bool.and(black(a), black(b)), Bool.and(black(c), black(d)))

def main() -> IO(Unit):
  +a = parse(text("A"))
  +b = parse(text("B"))
  +legacy = parse("(kicad_pcb (footprint x (at 10 20 90) (uuid fp) (fp_text reference \"R1\" (at 2 3 90) (layer \"F.SilkS\") (effects (font (size 1 1))))))")
  +modern = parse("(kicad_pcb (footprint x (transform (translate 10 20) (rotate 90) (scale 1 1)) (uuid fp) (property \"Reference\" \"R1\" (at 2 3 90) (layer \"F.SilkS\") (effects (font (size 1 1))))))")
  do IO<Unit>:
    C.Check.str("gr_text becomes stroke primitives", Bool.show(strokes(prims(a), ST.Stroke.text("A", 10.0, 20.0, 1.0, 0.0, 1, 1, False{}))), "True")
    C.Check.str("board text really rasterizes", Bool.show(black(frame(a, 0))), "False")
    C.Check.str("hidden silk layer removes text", Bool.show(black(frame(a, V.Look.toggle(0, 16)))), "True")
    C.Check.str("legacy and fork footprint text placement agree", Bool.show(strokes(prims(legacy), prims(modern))), "True")
    C.Check.str("rotated footprint text is placed in board coordinates", Bool.show(strokes(prims(legacy), ST.Stroke.text("R1", 13.0, 18.0, 1.0, B.Geo.rad(90.0), 1, 1, False{}))), "True")
    C.Check.str("bare hidden legacy text absent", Nat.show(List.length(&2, B.Item, parse("(kicad_pcb (footprint x (fp_text reference \"R1\" (at 0 0) (layer \"F.SilkS\") hide (effects (font (size 1 1))))))"))), "0")
    C.Check.str("modern hidden property absent", Nat.show(List.length(&2, B.Item, parse("(kicad_pcb (footprint x (property \"Reference\" \"R1\" (at 0 0) (layer \"F.SilkS\") (effects (font (size 1 1)) (hide yes)))))"))), "0")
    C.Check.str("text keys survive a saved text change", keys(a), keys(b))
    C.Check.str("saved text change updates source signature", Bool.show(String.eq(signatures(a), signatures(b))), "False")
    C.Check.str("incomplete save is not a new scene", T.some(P.Pcb2.items(U.Utf8.encode("(kicad_pcb (gr_text \"A\""))), "none")
