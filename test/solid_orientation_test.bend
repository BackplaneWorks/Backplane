import Base
import ./check.bend as C
import ../src/core/plot.bend as P
import ../src/core/glb.bend as G

# Decode the actual mesh sent to the web and phone viewers. The source
# triangle faces glTF +Y; rotating it to Z-up must leave it facing +Z.
type Pt is Data:
  Pt{x: U32, y: U32, z: U32}

def signed(+n: U32) -> String:
  Bool.pick(String, P.Box.lt(n, 0), "-" ++ U32.show(U32.sub(0, n)), U32.show(n))

def point(p: Pt) -> String:
  match p:
    case Pt{x, y, z}:
      signed(x) ++ "," ++ signed(y) ++ "," ++ signed(z)

def points(ps: List<&2, Pt>) -> List<&2, String>:
  match ps:
    case Nil{}:
      Nil{}
    case p <> rest:
      point(p) <> points(rest)

def decode(ns: List<&2, U32>, +x: U32, +y: U32, +z: U32) -> List<&2, Pt>:
  match ns:
    case dx <> dy <> dz <> rest:
      +nx = U32.add(x, P.Num.unzig(dx))
      +ny = U32.add(y, P.Num.unzig(dy))
      +nz = U32.add(z, P.Num.unzig(dz))
      Pt{nx, ny, nz} <> decode(rest, nx, ny, nz)
    case _:
      Nil{}

def vertices(s: P.Solid) -> List<&2, Pt>:
  match s:
    case P.Solid{_, _, data}:
      decode(List.drop(&2, U32, P.Dec.nums(String.reverse(data)), 1n), 0, 0, 0)

def facing_up(ps: List<&2, Pt>) -> Bool:
  match ps:
    case Pt{+ax, +ay, _} <> Pt{bx, by, _} <> Pt{cx, cy, _} <> _:
      P.Box.lt(0, U32.sub(U32.mul(U32.sub(bx, ax), U32.sub(cy, ay)), U32.mul(U32.sub(by, ay), U32.sub(cx, ax))))
    case _:
      False{}

def numbers(xs: List<&2, U32>) -> List<&2, String>:
  match xs:
    case Nil{}:
      Nil{}
    case x <> rest:
      signed(x) <> numbers(rest)

def bounds(s: P.Solid) -> String:
  match s:
    case P.Solid{_, b, _}:
      String.join(numbers(b), ",")

def main() -> IO(Unit):
  do IO<Unit>:
    +s : P.Solid = P.Solid.of([G.Tri3{0.002, 0.0016, F32.neg(0.003), 0.007, 0.0016, F32.neg(0.003), 0.002, 0.0016, F32.neg(0.009), 255}])
    C.Check.str("Z-up mesh preserves asymmetric landmark positions", String.join(points(vertices(s)), ";"), "200,300,160;700,300,160;200,900,160")
    C.Check.str("front-face winding survives Y-up to Z-up rotation", Bool.show(facing_up(vertices(s))), "True")
    C.Check.str("mesh bounds follow the rotated vertices", bounds(s), "200,300,160,700,900,160")
    +negative : P.Solid = P.Solid.of([G.Tri3{0.001, F32.neg(0.0016), 0.002, 0.003, F32.neg(0.0016), 0.002, 0.001, F32.neg(0.0016), 0.004, 255}])
    C.Check.str("rotation retains signed bounds and underside height", bounds(negative), "100,-400,-160,300,-200,-160")
