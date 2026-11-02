import Base
import ../src/core/board.bend as B
import ../src/core/sexpr.bend as S

# A polygon's points read by the production Board.pts, in the emitted
# JavaScript (test/tools/board_pts.mjs runs each on a small stack): a routed
# board's zone fills run to tens of thousands of points, and a frame per
# point faulted the --plot child's 64 MiB stack on macOS, so the board
# viewer showed "bend: memory fault" instead of the board.

def atom(+s: String) -> S.Sx:
  S.XAtom{s}

# (xy X Y)
def xy(+x: String, +y: String) -> S.Sx:
  S.XList{S.XCons{atom("xy"), S.XCons{atom(x), S.XCons{atom(y), S.XEnd{}}}}}

# (arc ...), not a point
def other() -> S.Sx:
  S.XList{S.XCons{atom("arc"), S.XEnd{}}}

# n points (xy i 0) in front of acc, last first, one tail call each
def poly(n: Nat, acc: List<&2, S.Sx>) -> List<&2, S.Sx>:
  match n:
    case 0n:
      acc
    case 1n+k:
      +i = k
      poly(i, xy(Nat.show(i), "0") <> acc)

# a length in tail calls (the test's own, so only Board.pts is measured)
def len(ps: List<&2, B.Pt>, n: Nat) -> Nat:
  match ps:
    case Nil{}:
      n
    case _ <> t:
      len(t, Nat.add(n, 1n))

# how many points Board.pts reads from an n-point polygon
def count(n: Nat) -> Nat:
  len(B.Board.pts(poly(n, Nil{}), B.Board.home()), 0n)

def show(ps: List<&2, B.Pt>, acc: String) -> String:
  match ps:
    case Nil{}:
      acc
    case B.Pt{x, y} <> t:
      show(t, acc ++ F32.show(x) ++ ":" ++ F32.show(y) ++ ",")

# three points around an arc, in the order the file gives them
def order() -> String:
  show(B.Board.pts([xy("1", "2"), other(), xy("3", "4"), xy("5", "6")], B.Board.home()), "")

# the last point's x, so the walk keeps the file's order
def last.go(ps: List<&2, B.Pt>) -> String:
  match ps:
    case B.Pt{x, _} <> Nil{}:
      F32.show(x)
    case _ <> t:
      last.go(t)
    case Nil{}:
      ""

# the last point of an n-point polygon
def last(n: Nat) -> String:
  last.go(B.Board.pts(poly(n, Nil{}), B.Board.home()))
