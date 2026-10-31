import Base
import ./check.bend as C
import ../src/core/board.bend as B
import ../src/core/sexpr.bend as S
import ../src/core/utf8.bend as U

def many(n: Nat, +x: S.Sx, acc: List<&2, S.Sx>) -> List<&2, S.Sx>:
  match n:
    case 0n:
      acc
    case 1n+m:
      many(m, x, x <> acc)

def count(xs: List<&2, B.Pt>, n: Nat) -> Nat:
  match xs:
    case Nil{}:
      n
    case _ <> t:
      count(t, 1n+n)

def points(xs: List<&2, B.Pt>) -> String:
  match xs:
    case Nil{}:
      ""
    case B.Pt{x, y} <> t:
      F32.show(x) ++ "," ++ F32.show(y) ++ ";" ++ points(t)

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("polygon vertex order/filter/translation",
      points(B.Board.pts(S.Sx.args(S.Sx.parse(U.Utf8.encode("(pts (xy 1 2) (ignored 99) (xy 3 4))"))), B.Place{10.0, 20.0, 0.0})), "11,22;13,24;")
    C.Check.str("empty polygon", points(B.Board.pts(Nil{}, B.Board.home())), "")
    C.Check.str("100000 vertices at normal JS stack", Nat.show(count(B.Board.pts(many(100000n, S.Sx.parse(U.Utf8.encode("(xy 1 2)")), Nil{}), B.Board.home()), 0n)), "100000")
