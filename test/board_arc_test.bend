import Base
import ./check.bend as C
import ./sch_test.bend as T
import ../src/core/board.bend as B
import ../src/core/pcb2d.bend as P
import ../src/core/utf8.bend as U

# A length-tuning meander's corner: a quarter of a 0.08 mm circle around
# (152.4, 97.3), far from the origin as tracks are on a real board.

def board() -> String:
  "(kicad_pcb (arc (start 152.32 97.3) (mid 152.3434315 97.2434315) (end 152.4 97.22) (width 0.1) (layer \"F.Cu\") (net 1) (uuid a1)))"

def near(+a: F32, +b: F32, +e: F32) -> Bool:
  F32.is_lt(F32.abs(F32.sub(a, b)), e)

# every point at the radius from the true centre (within 1% of it)
def round(ps: List<&2, B.Pt>) -> Bool:
  match ps:
    case Nil{}:
      True{}
    case B.Pt{x, y} <> t:
      Bool.and(near(F32.hypot(F32.sub(x, 152.4), F32.sub(y, 97.3)), 0.08, 0.0008), round(t))

def first(ps: List<&2, B.Pt>) -> String:
  match ps:
    case B.Pt{x, y} <> _:
      Bool.show(Bool.and(near(x, 152.32, 0.0001), near(y, 97.3, 0.0001)))
    case Nil{}:
      "empty"

def end(m: Maybe<&2, B.Pt>) -> String:
  match m:
    case Some{B.Pt{x, y}}:
      Bool.show(Bool.and(near(x, 152.4, 0.0001), near(y, 97.22, 0.0001)))
    case None{}:
      "empty"

def last(ps: List<&2, B.Pt>) -> String:
  end(List.last(&2, B.Pt, ps))

def line(xs: List<&2, B.Item>) -> List<&2, B.Pt>:
  match xs:
    case B.Item{_, _, _, B.PLine{ps, _}, _} <> _:
      ps
    case _:
      Nil{}

def main() -> IO(Unit):
  +ps = B.Arc.points(B.Pt{152.32, 97.3}, B.Pt{152.3434315, 97.2434315}, B.Pt{152.4, 97.22})
  +drawn = line(T.items(P.Pcb2.items(U.Utf8.encode(board()))))
  do IO<Unit>:
    C.Check.str("a small arc far from the origin stays on its circle", Bool.show(round(ps)), "True")
    C.Check.str("the arc starts at its start", first(ps), "True")
    C.Check.str("the arc ends at its end", last(ps), "True")
    C.Check.str("a board's track arc is drawn on its circle", Bool.show(round(drawn)), "True")
    C.Check.str("the drawn arc has its points", Nat.show(List.length(&2, B.Pt, drawn)), "25")
