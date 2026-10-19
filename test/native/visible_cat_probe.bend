import Base
import ../../src/app/bots.bend as B
import ../../src/app/cats.bend as C
import ../../src/app/nui.bend as N
import ../../src/gfx/draw.bend as D
import ../../src/gfx/raster.bend as R
import ../../src/gfx/text.bend as X
import ../../src/server/perf.bend as P

# Same-binary ABBA: original cold sleep24 requests versus retained visible cache.
# Synthetic component attribution, not a Mailroom or application benchmark.
# No hub, provider, window, private log, or file writes. Cached and uncached
# front preparation are timed separately over identical visible geometry.

def clip() -> D.Clip:
  D.Clip{0, 0, 1400, 900}

def y(+i: Nat) -> U32:
  (70 + U32.from_nat(Nat.mul(i, 36n)) : U32)

def spots(left: Nat, +i: Nat) -> List<&2, B.Spot>:
  match left:
    case 0n: Nil{}
    case 1n+k:
      B.Spot{Nat.add(12345n, i), "sleep", 24, 20, y(i), clip()} <> spots(k, 1n+i)

def anchors(ss: List<&2, B.Spot>) -> List<&2, R.Op>:
  match ss:
    case Nil{}: Nil{}
    case B.Spot{_, _, px, x, y, c} <> t:
      B.Spot.anchor(c, x, y, px) <> anchors(t)

def hits(ss: List<&2, B.Spot>) -> List<&2, N.Hit>:
  match ss:
    case Nil{}: Nil{}
    case B.Spot{lk, md, px, x, y, c} <> t:
      B.Spot.hit(lk, md, c, x, y, px) <> hits(t)

def rectangles(left: Nat, +i: Nat) -> List<&2, R.Op>:
  match left:
    case 0n: Nil{}
    case 1n+k:
      D.Draw.rect(clip(), (400 + U32.from_nat(Nat.mod(i, 500n)) : U32), (100 + U32.from_nat(Nat.mod(i, 600n)) : U32), 2, 2, 100, 255) <> rectangles(k, 1n+i)

def warm(ss: List<&2, B.Spot>, c: Map<&2, C.CatF>) -> Map<&2, C.CatF>:
  B.Spot.warm(ss, c, 0n)

type Fixture is Data:
  Fixture{ops: List<&2, R.Op>, hits: List<&2, N.Hit>, spots: List<&2, B.Spot>, caches: List<&2, Map<&2, C.CatF>>}

def cache(xs: List<&2, Map<&2, C.CatF>>) -> Map<&2, C.CatF>:
  match xs:
    case c <> _: c
    case Nil{}: C.Cats.none()

def cover(on: Bool, ops: List<&2, R.Op>) -> List<&2, R.Op>:
  match on:
    case False{}: ops
    case True{}: D.Draw.rect(clip(), 0, 0, 80, 900, 200, 255) <> ops

def fixture(+n: Nat, +covered: Bool) -> Fixture:
  +ss : List<&2, B.Spot> = spots(n, 0n)
  Fixture{cover(covered, List.append(&2, R.Op, anchors(ss), rectangles(400n, 0n))), hits(ss), ss, [warm(ss, C.Cats.none())]}

def force_edges(es: List<&2, R.Edge>, acc: U32) -> Nat:
  match es:
    case Nil{}: U32.to_nat(acc)
    case R.Edge{ax, ay, bx, by} <> t:
      force_edges(t, (acc + F32.bits(ax) + F32.bits(ay) + F32.bits(bx) + F32.bits(by) : U32))

# Force the complete coverage bitmap as well as vector coordinates.
def force_mask(m: R.Mask) -> U32:
  match m:
    case R.MWord{w}: w
    case R.MPair{a, b}: (force_mask(a) + force_mask(b) : U32)
    case R.M16{a, b, c, d, e, f, g, h, i, j, k, l, m, n, o, p}:
      (a + b + c + d + e + f + g + h + i + j + k + l + m + n + o + p : U32)

def force_shape(s: R.Shape) -> Nat:
  match s:
    case R.SPoly{_, es}: force_edges(es, 0)
    case R.SMask{_, _, _, _, m}: U32.to_nat(force_mask(m))
    case R.SSeg{ax, ay, bx, by, hw}: U32.to_nat((F32.bits(ax) + F32.bits(ay) + F32.bits(bx) + F32.bits(by) + F32.bits(hw) : U32))
    case R.SDisc{cx, cy, r}: U32.to_nat((F32.bits(cx) + F32.bits(cy) + F32.bits(r) : U32))
    case R.STri{ax, ay, bx, by, cx, cy}: U32.to_nat((F32.bits(ax) + F32.bits(ay) + F32.bits(bx) + F32.bits(by) + F32.bits(cx) + F32.bits(cy) : U32))
    case _: 1n

def force(ops: List<&2, R.Op>, +acc: Nat) -> Nat:
  match ops:
    case Nil{}: acc
    case R.Op{x0, y0, x1, y1, _, _, s} <> t:
      force(t, Nat.add(acc, Nat.add(force_shape(s), U32.to_nat((x0 + y0 + x1 + y1 : U32)))))

def force_boxes(bs: List<&2, B.Box>, +acc: Nat) -> Nat:
  match bs:
    case Nil{}: acc
    case B.Box{k, x0, y0, x1, y1} <> t:
      force_boxes(t, Nat.add(acc, U32.to_nat((k + x0 + y0 + x1 + y1 : U32))))

def force_front(fr: B.Fronted) -> Nat:
  match fr:
    case B.Fronted{os, _, _}: force(os, 0n)

# Each result, including polygon coordinates, is consumed inside the timed
# region. Fixture/cache preparation stays outside it. No UI records cross IO.
def evaluate(kind: Nat, f: Fixture) -> Nat:
  match kind f:
    case 0n Fixture{os, _, _, _}:
      List.length(&2, X.Want, X.Text.wants(os, Nil{}))
    case 1n Fixture{_, _, ss, cs}:
      force_boxes(B.Box.all(ss, cache(cs), 0n), 0n)
    case 2n Fixture{os, _, ss, cs}:
      Bool.pick(Nat, B.Spot.covered(os, B.Box.all(ss, cache(cs), 0n)), 1n, 0n)
    case 3n Fixture{_, _, ss, cs}:
      force(B.Spot.all.ops(ss, cache(cs), 0n), 0n)
    case 4n Fixture{os, _, ss, cs}:
      force(B.Spot.inline(os, ss, cache(cs), 0n), 0n)
    case 5n Fixture{os, +hs, _, cs}:
      force_front(B.Fronted.of(os, hs, B.Spot.warm(B.Spot.all(hs), cache(cs), 0n), 0n))
    case _ Fixture{os, hs, _, _}:
      force_front(B.Fronted.of(os, hs, C.Cats.none(), 0n))

def name(kind: Nat) -> String:
  match kind:
    case 0n: "wants"
    case 1n: "boxes.warm"
    case 2n: "covered.warm"
    case 3n: "cats.ops.warm"
    case 4n: "inline.warm"
    case 5n: "front.warm"
    case _: "front.cold"

def one(+kind: Nat, +fs: List<&2, Fixture>) -> IO(Nat):
  match fs:
    case Nil{}: IO.pure(Nat, 0n)
    case +f <> _:
      do IO<Nat>:
        +t0 : U32 <- P.Perf.us()
        P.Perf.add("probe.checksum", U32.from_nat(Nat.mod(evaluate(kind, f), 1000000n)))
        +t1 : U32 <- P.Perf.us()
        return U32.to_nat((t1 - t0 : U32))

def total(xs: List<&2, Nat>, acc: Nat) -> Nat:
  match xs:
    case Nil{}: acc
    case x <> t: total(t, Nat.add(acc, x))

def at(xs: List<&2, Nat>, i: Nat) -> Nat:
  Maybe.default(&2, Nat, List.get(&2, Nat, xs, i), 0n)

def report(kind: Nat, +xs: List<&2, Nat>) -> IO(Unit):
  +sorted : List<&2, Nat> = List.sort(~Nat, ~Nat.is_lt, xs)
  +n : Nat = List.length(&2, Nat, xs)
  IO.print(name(kind) ++ " n=" ++ Nat.show(n) ++ " total_us=" ++ Nat.show(total(xs, 0n)) ++ " p50_us=" ++ Nat.show(at(sorted, Nat.div(n, 2n))) ++ " max_us=" ++ Nat.show(at(sorted, Nat.sub(n, 1n))))

def reps(left: Nat, +kind: Nat, +fs: List<&2, Fixture>, times: List<&2, Nat>) -> IO(Unit):
  match left:
    case 0n: report(kind, times)
    case 1n+k: IO.bind(Nat, Unit, one(kind, fs), us => reps(k, kind, fs, us <> times))

def stages(ks: List<&2, Nat>, +fs: List<&2, Fixture>) -> IO(Unit):
  match ks:
    case Nil{}: IO.pure(Unit, Unit{})
    case +k <> t:
      do IO<Unit>:
        reps(12n, k, fs, Nil{})
        stages(t, fs)

def front_count(fr: B.Fronted) -> Nat:
  match fr:
    case B.Fronted{_, ss, _}: List.length(&2, B.Spot, ss)

def ready(+n: Nat, covered: Bool, +f: Fixture) -> Bool:
  match covered f:
    case +on Fixture{+os, +hs, _, +cs}:
      Bool.and(Nat.is_eq(front_count(B.Fronted.of(os, hs, cache(cs), 0n)), Bool.pick(Nat, on, 0n, n)), Nat.is_gt(List.length(&2, R.Op, os), 400n))

def scenario_run(ok: Bool, +fs: List<&2, Fixture>) -> IO(Unit):
  match ok:
    case False{}: IO.print("FAIL scenario geometry/coverage")
    case True{}:
      do IO<Unit>:
        IO.print("PASS scenario geometry/coverage")
        stages([6n, 5n, 5n, 6n], fs)

def scenario(+n: Nat, +covered: Bool) -> IO(Unit):
  +f : Fixture = fixture(n, covered)
  do IO<Unit>:
    IO.print("cats=" ++ Nat.show(n) ++ " covered=" ++ Bool.show(covered))
    scenario_run(ready(n, covered, f), [f])

def main() -> IO(Unit):
  do IO<Unit>:
    scenario(1n, False{})
    scenario(11n, False{})
    scenario(11n, True{})
