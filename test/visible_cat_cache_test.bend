import Base
import ./check.bend as K
import ../src/app/bots.bend as B
import ../src/app/cats.bend as C
import ../src/gfx/draw.bend as D
import ../src/gfx/raster.bend as R

def request(+look: Nat, mood: String, px: U32) -> B.Spot:
  B.Spot{look, mood, px, 6, 6, D.Clip.all()}

def found(f: C.CatF) -> Bool:
  match f:
    case C.CatF{ok, _, _, _}: ok
    case C.CatAt{_, _, _, _, _}: True{}

def clock(f: C.CatF) -> Nat:
  match f:
    case C.CatF{_, _, c, _}: c
    case C.CatAt{_, c, _, _, _}: c

def pixels(+n: Nat, +a: Image, +b: Image, acc: Nat) -> Nat:
  match n:
    case 0n: acc
    case 1n+i:
      pixels(i, a, b, Nat.add(acc, Bool.pick(Nat, U32.is_eq(R.Img.at(6n, a, U32.from_nat(Nat.mod(i, 40n)), U32.from_nat(Nat.div(i, 40n))), R.Img.at(6n, b, U32.from_nat(Nat.mod(i, 40n)), U32.from_nat(Nat.div(i, 40n)))), 0n, 1n)))

def exact(+look: Nat, +ms: Nat) -> Nat:
  +sp = request(look, "sleep", 24)
  +cs = C.Cats.visible(C.Cats.none(), look, "sleep", 24, ms, 6, 6)
  pixels(1600n, R.Frame.render(40, 40, B.Spot.ops(sp, C.Cats.none(), ms), 1118481), R.Frame.render(40, 40, B.Spot.ops(sp, cs, ms), 1118481), 0n)

def stationary(+look: Nat) -> Nat:
  +sp = request(look, "sleep", 24)
  +cs = B.Spot.warm([sp], C.Cats.none(), 900n)
  pixels(1600n, R.Frame.render(40, 40, B.Spot.ops(sp, C.Cats.none(), 0n), 1118481), R.Frame.render(40, 40, B.Spot.ops(sp, cs, 900n), 1118481), 0n)

def moved(+look: Nat) -> Nat:
  +sp : B.Spot = B.Spot{look, "sleep", 24, 7, 8, D.Clip{9, 10, 29, 30}}
  +cs = B.Spot.warm([sp], C.Cats.none(), 900n)
  pixels(1600n, R.Frame.render(40, 40, B.Spot.ops(sp, C.Cats.none(), 0n), 1118481), R.Frame.render(40, 40, B.Spot.ops(sp, cs, 1200n), 1118481), 0n)

def main() -> IO(Unit):
  +before = C.Cats.warm(C.Cats.warm(C.Cats.none(), 13n, "work", 28, 330n), 13n, "work", 64, 330n)
  +cs = B.Spot.warm([request(13n, "sleep", 24), request(17n, "work", 24)], before, 660n)
  +again = B.Spot.warm([request(13n, "sleep", 24), request(17n, "work", 24)], cs, 990n)
  +relocated = B.Spot.warm([B.Spot{13n, "sleep", 24, 7, 8, D.Clip.all()}], again, 1200n)
  do IO<Unit>:
    K.Check.str("sidebar warmup alone misses Mailroom chip", Bool.show(found(C.Cats.find(before, 13n, "sleep", 24))), "False")
    K.Check.str("actual visible mood and size warmed", Bool.show(found(C.Cats.find(cs, 13n, "sleep", 24))), "True")
    K.Check.str("existing sidebar pose preserved", Nat.show(clock(C.Cats.find(cs, 13n, "work", 28))), "330")
    K.Check.str("existing selected pose preserved", Nat.show(clock(C.Cats.find(cs, 13n, "work", 64))), "330")
    K.Check.str("stationary chip posed at zero", Nat.show(clock(C.Cats.find(again, 13n, "sleep", 24))), "0")
    K.Check.str("moving visible size follows clock", Nat.show(clock(C.Cats.find(again, 17n, "work", 24))), "990")
    K.Check.str("hidden size is not warmed", Bool.show(found(C.Cats.find(cs, 13n, "sleep", 48))), "False")
    K.Check.str("empty visible requests preserve cache", Bool.show(found(C.Cats.find(B.Spot.warm(Nil{}, cs, 1000n), 13n, "sleep", 24))), "True")
    K.Check.str("sleep chip 0 cached same-clock pixels", Nat.show(exact(0n, 900n)), "0")
    K.Check.str("sleep chip 13 cached same-clock pixels", Nat.show(exact(13n, 900n)), "0")
    K.Check.str("sleep chip 12345 cached same-clock pixels", Nat.show(exact(12345n, 900n)), "0")
    K.Check.str("sleep chip 0 follows native stationary pose", Nat.show(stationary(0n)), "0")
    K.Check.str("sleep chip 13 follows native stationary pose", Nat.show(stationary(13n)), "0")
    K.Check.str("sleep chip 12345 follows native stationary pose", Nat.show(stationary(12345n)), "0")
    K.Check.str("moving a request does not grow the cache", Nat.show(List.length(&2, String, Map.keys(&2, C.CatF, relocated))), Nat.show(List.length(&2, String, Map.keys(&2, C.CatF, again))))
    K.Check.str("moved clipped chip pixels stay exact", Nat.show(moved(13n)), "0")
