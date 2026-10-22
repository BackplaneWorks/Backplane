import Base
import ../../src/gfx/draw.bend as D
import ../../src/gfx/raster.bend as R
import ../../src/gfx/tiles.bend as TI

# How a thread switch's redraw spreads over cores: a 3440 x 1400 frame of
# text lines in a centred column, drawn from scratch as tiles.
#   scripts/build-js.sh test/native/par_bench.bend build/par_bench
#   taskset -c 4 build/par_bench; taskset -c 4-19 build/par_bench

def line(+i: Nat) -> List<&2, R.Op>:
  D.Draw.text("Message " ++ Nat.show(i) ++ ": the regulator's enable pin is pulled up through R12 to VBAT, so the output", D.Clip.all(), 1330, U32.from_nat(Nat.add(40n, Nat.mul(i, 20n))), 14277092)

def lines(+n: Nat) -> List<&2, R.Op>:
  match n:
    case 0n:
      Nil{}
    case 1n+k:
      List.append(&2, R.Op, line(k), lines(k))

def boxes(+i: Nat, +j: Nat) -> List<&2, R.Op>:
  match j:
    case 0n:
      Nil{}
    case 1n+k:
      D.Draw.rect(D.Clip.all(), U32.from_nat(Nat.add(1330n, Nat.mul(k, 8n))), U32.from_nat(Nat.add(42n, Nat.mul(i, 20n))), 6, 12, 14277092, 200) <> boxes(i, k)

def rows(+n: Nat) -> List<&2, R.Op>:
  match n:
    case 0n:
      Nil{}
    case 1n+k:
      List.append(&2, R.Op, boxes(k, 100n), rows(k))

def ops() -> List<&2, R.Op>:
  List.append(&2, R.Op, lines(66n), [D.Draw.rect(D.Clip.all(), 0, 0, 264, 1400, 1250331, 255)])

def some(t: TI.Tiles) -> Nat:
  match t:
    case TI.TLeaf{_}:
      1n
    case TI.TNode{_, _, _, _, _, _}:
      4n

def hash(img: Image) -> U32:
  match img:
    case Pix{c}:
      c
    case Qua{a, b, c, d}:
      ((hash(a) + hash(b) : U32) + (hash(c) * 3 + hash(d) : U32) : U32)

# a hash that forks at each node
def phash(img: Image) -> U32:
  match img:
    case Pix{c}:
      c
    case Qua{a, b, c, d}:
      w x y z = phash(a) phash(b) phash(c) phash(d)
      ((w + x : U32) + (y * 3 + z : U32) : U32)

# the tile quadtree as Tiles.make forks it, each tile drawn and hashed in
# its own call (forced where it is made)
def qh(k: Nat, +ops: List<&2, R.Op>, +rx: U32, +ry: U32, +n: U32) -> U32:
  match k:
    case 0n:
      hash(TI.Tile.draw(6n, TI.Loc.ops(ops, rx, ry, 64), Pix{855826}))
    case 1n+j0:
      +j = j0
      +h = U32.shr(n)
      a b c d = qh(j, R.Region.keep(ops, rx, ry, rx, ry, h), rx, ry, h) qh(j, R.Region.keep(ops, rx, ry, (rx + h : U32), ry, h), (rx + h : U32), ry, h) qh(j, R.Region.keep(ops, rx, ry, rx, (ry + h : U32), h), rx, (ry + h : U32), h) qh(j, R.Region.keep(ops, rx, ry, (rx + h : U32), (ry + h : U32), h), (rx + h : U32), (ry + h : U32), h)
      ((a + b : U32) + (c * 3 + d : U32) : U32)

# a tile drawn with no forks inside (Region.node's recursion, one call
# after another)
def snode(d: Nat, l: R.Leaf, +ops: List<&2, R.Op>, +rx: U32, +ry: U32, +n: U32, +base: Image) -> Image:
  match d l:
    case _ R.LEmpty{}:
      base
    case _ R.LSolid{}:
      Pix{R.Region.top(ops)}
    case _ R.LPixel{}:
      Pix{R.Shade.px(ops, rx, ry, R.Four.top(base))}
    case 0n R.LSplit{}:
      Pix{R.Shade.px(ops, rx, ry, R.Four.top(base))}
    case 1n+e0 R.LSplit{}:
      +e = e0
      +h = U32.shr(n)
      +x2 = (rx + h : U32)
      +y2 = (ry + h : U32)
      R.Four.base(base, ba => bb => bc => bd =>
        R.Four.split(ops, rx, ry, h, +ta => +tb => +tc => +td =>
          Qua{snode(e, R.Region.leaf(ta, rx, ry, h), ta, rx, ry, h, ba), snode(e, R.Region.leaf(tb, x2, ry, h), tb, x2, ry, h, bb), snode(e, R.Region.leaf(tc, rx, y2, h), tc, rx, y2, h, bc), snode(e, R.Region.leaf(td, x2, y2, h), td, x2, y2, h, bd)}))

def sdraw(+l: List<&2, R.Op>) -> Image:
  snode(6n, R.Region.leaf(l, 0, 0, 64), l, 0, 0, 64, Pix{855826})

def qz(k: Nat, +pops: List<&2, R.Op>, +px: U32, +py: U32, +rx: U32, +ry: U32, +n: U32) -> U32:
  match k:
    case 0n:
      hash(sdraw(TI.Loc.ops(R.Region.keep(pops, px, py, rx, ry, n), rx, ry, 64)))
    case 1n+j0:
      +j = j0
      +h = U32.shr(n)
      +ops = R.Region.keep(pops, px, py, rx, ry, n)
      a b c d = qz(j, ops, rx, ry, rx, ry, h) qz(j, ops, rx, ry, (rx + h : U32), ry, h) qz(j, ops, rx, ry, rx, (ry + h : U32), h) qz(j, ops, rx, ry, (rx + h : U32), (ry + h : U32), h)
      ((a + b : U32) + (c * 3 + d : U32) : U32)

# the quadtree's forks, each tile cut from the whole list (as flat)
def qr(k: Nat, +os: List<&2, R.Op>, +rx: U32, +ry: U32, +n: U32) -> U32:
  match k:
    case 0n:
      Bool.pick(U32, Bool.and(U32.is_lt(rx, 3440), U32.is_lt(ry, 1400)), hash(TI.Tile.draw(6n, TI.Loc.ops(R.Region.keep(os, 0, 0, rx, ry, 64), rx, ry, 64), Pix{855826})), 0)
    case 1n+j0:
      +j = j0
      +h = U32.shr(n)
      a b c d = qr(j, os, rx, ry, h) qr(j, os, (rx + h : U32), ry, h) qr(j, os, rx, (ry + h : U32), h) qr(j, os, (rx + h : U32), (ry + h : U32), h)
      ((a + b : U32) + (c * 3 + d : U32) : U32)

# the tiles to draw as a list (serial cut), dealt into a balanced tree
# (alternate items to each side, so neighbours spread), drawn in parallel
type Job is Data:
  Job{x: U32, y: U32, l: List<&2, R.Op>}

type JT is Data:
  JNone{}
  JOne{j: Job}
  JTwo{a: JT, b: JT}

def jobs.k(empty: Bool, +ops: List<&2, R.Op>, +rx: U32, +ry: U32, acc: List<&2, Job>) -> List<&2, Job>:
  match empty:
    case True{}:
      acc
    case False{}:
      Job{rx, ry, TI.Loc.ops(ops, rx, ry, 64)} <> acc

def jobs(k: Nat, +ops: List<&2, R.Op>, +rx: U32, +ry: U32, +n: U32, acc: List<&2, Job>) -> List<&2, Job>:
  match k:
    case 0n:
      jobs.k(List.is_empty(&2, R.Op, ops), ops, rx, ry, acc)
    case 1n+j0:
      +j = j0
      +h = U32.shr(n)
      jobs(j, R.Region.keep(ops, rx, ry, rx, ry, h), rx, ry, h,
        jobs(j, R.Region.keep(ops, rx, ry, (rx + h : U32), ry, h), (rx + h : U32), ry, h,
          jobs(j, R.Region.keep(ops, rx, ry, rx, (ry + h : U32), h), rx, (ry + h : U32), h,
            jobs(j, R.Region.keep(ops, rx, ry, (rx + h : U32), (ry + h : U32), h), (rx + h : U32), (ry + h : U32), h, acc))))

type Two is Data:
  Two{a: List<&2, Job>, b: List<&2, Job>}

def halves(xs: List<&2, Job>, t: Two) -> Two:
  match xs t:
    case Nil{} _:
      t
    case x <> r Two{a, b}:
      halves(r, Two{b, x <> a})

def deal.k(+d: Nat, t: Two) -> JT:
  match d t:
    case _ Two{Nil{}, Nil{}}:
      JNone{}
    case _ Two{Nil{}, x <> Nil{}}:
      JOne{x}
    case _ Two{x <> Nil{}, Nil{}}:
      JOne{x}
    case 0n _:
      JNone{}
    case 1n+e Two{a, b}:
      JTwo{deal.k(e, halves(a, Two{Nil{}, Nil{}})), deal.k(e, halves(b, Two{Nil{}, Nil{}}))}

def run(t: JT) -> U32:
  match t:
    case JNone{}:
      0
    case JOne{Job{_, _, l}}:
      hash(TI.Tile.draw(6n, l, Pix{855826}))
    case JTwo{a, b}:
      x y = run(a) run(b)
      (x + y : U32)

# glyph masks made at run time and shared, as the glyph cache's are
def mk(+d: Nat, +seed: U32) -> R.Mask:
  match d:
    case 0n:
      R.MWord{(seed * 2654435761 : U32)}
    case 1n+k:
      R.MPair{mk(k, (seed * 2 : U32)), mk(k, (seed * 2 + 1 : U32))}

def masks(+n: Nat) -> List<&2, R.Mask>:
  match n:
    case 0n:
      Nil{}
    case 1n+k:
      mk(5n, U32.from_nat(Nat.add(k, 7n))) <> masks(k)

def nth(+xs: List<&2, R.Mask>, +i: Nat) -> R.Mask:
  Maybe.default(&2, R.Mask, List.head(&2, R.Mask, List.drop(&2, R.Mask, xs, i)), R.MWord{0})

def gl(+ms: List<&2, R.Mask>, +i: Nat, +j: Nat) -> List<&2, R.Op>:
  match j:
    case 0n:
      Nil{}
    case 1n+k:
      R.Op{U32.from_nat(Nat.add(1330n, Nat.mul(k, 8n))), U32.from_nat(Nat.add(42n, Nat.mul(i, 20n))), U32.from_nat(Nat.add(1338n, Nat.mul(k, 8n))), U32.from_nat(Nat.add(58n, Nat.mul(i, 20n))), 14277092, 255,
        R.SMask{U32.from_nat(Nat.add(1330n, Nat.mul(k, 8n))), U32.from_nat(Nat.add(42n, Nat.mul(i, 20n))), 8, 5n, nth(ms, Nat.mod(Nat.add(k, i), 64n))}} <> gl(ms, i, k)

def glines(+ms: List<&2, R.Mask>, +n: Nat) -> List<&2, R.Op>:
  match n:
    case 0n:
      Nil{}
    case 1n+k:
      List.append(&2, R.Op, gl(ms, k, 100n), glines(ms, k))

# a private copy (nothing shared with other lanes)
def Mask.copy(m: R.Mask) -> R.Mask:
  match m:
    case R.MWord{w}:
      R.MWord{w}
    case R.MPair{a, b}:
      R.MPair{Mask.copy(a), Mask.copy(b)}
    case R.M16{a0, a1, a2, a3, a4, a5, a6, a7, a8, a9, a10, a11, a12, a13, a14, a15}:
      R.M16{a0, a1, a2, a3, a4, a5, a6, a7, a8, a9, a10, a11, a12, a13, a14, a15}

def Shape.copy(s: R.Shape) -> R.Shape:
  match s:
    case R.SMask{gx, gy, w, d, m}:
      R.SMask{gx, gy, w, d, Mask.copy(m)}
    case _:
      s

def Ops.copy(os: List<&2, R.Op>) -> List<&2, R.Op>:
  match os:
    case Nil{}:
      Nil{}
    case R.Op{x0, y0, x1, y1, c, a, sh} <> t:
      R.Op{x0, y0, x1, y1, c, a, Shape.copy(sh)} <> Ops.copy(t)

def run.copy(t: JT) -> U32:
  match t:
    case JNone{}:
      0
    case JOne{Job{_, _, l}}:
      hash(TI.Tile.draw(6n, Ops.copy(l), Pix{855826}))
    case JTwo{a, b}:
      x y = run.copy(a) run.copy(b)
      (x + y : U32)

# the same, each call cutting its own ops from its parent's (the fork's
# arguments stay cheap)
def qc(k: Nat, +pops: List<&2, R.Op>, +px: U32, +py: U32, +rx: U32, +ry: U32, +n: U32) -> U32:
  match k:
    case 0n:
      hash(TI.Tile.draw(6n, TI.Loc.ops(R.Region.keep(pops, px, py, rx, ry, n), rx, ry, 64), Pix{855826}))
    case 1n+j0:
      +j = j0
      +h = U32.shr(n)
      +ops = R.Region.keep(pops, px, py, rx, ry, n)
      a b c d = qc(j, ops, rx, ry, rx, ry, h) qc(j, ops, rx, ry, (rx + h : U32), ry, h) qc(j, ops, rx, ry, rx, (ry + h : U32), h) qc(j, ops, rx, ry, (rx + h : U32), (ry + h : U32), h)
      ((a + b : U32) + (c * 3 + d : U32) : U32)

# a flat fork: 2^d tile indices split in halves; each leaf draws its 64 px
# tile of the frame (a 64 x 64 grid of them over a 4096 square)
def flat.tile(+i: U32, +os: List<&2, R.Op>) -> U32:
  +tx = (U32.mod(i, 64) * 64 : U32)
  +ty = ((i / 64 : U32) * 64 : U32)
  Bool.pick(U32, Bool.and(U32.is_lt(tx, 3440), U32.is_lt(ty, 1400)),
    hash(TI.Tile.draw(6n, TI.Loc.ops(R.Region.keep(os, 0, 0, tx, ty, 64), tx, ty, 64), Pix{855826})), 0)

def flat(+d: Nat, +i: U32, +os: List<&2, R.Op>) -> U32:
  match d:
    case 0n:
      flat.tile(i, os)
    case 1n+k:
      a b = flat(k, (i * 2 : U32), os) flat(k, (i * 2 + 1 : U32), os)
      (a + b : U32)

def main() -> IO(Unit):
  do IO<Unit>:
    +os : List<&2, R.Op> = ops()
    +k : Nat = List.length(&2, R.Op, os)
    +t0 : Nat <- IO.now()
    +a : Image = TI.Tiles.image(TI.Frame.make(3440, 1400, os, Pix{855826}))
    +ha : U32 = hash(a)
    +t1 : Nat <- IO.now()
    +b : Image = R.Frame.render(3440, 1400, os, 855826)
    +hb : U32 = hash(b)
    +t2 : Nat <- IO.now()
    +rs : List<&2, R.Op> = rows(66n)
    +t6 : Nat <- IO.now()
    +h6 : U32 = phash(TI.Tiles.image(TI.Frame.make(3440, 1400, rs, Pix{855826})))
    +t7 : Nat <- IO.now()
    IO.print("rects " ++ Nat.show(List.length(&2, R.Op, rs)) ++ ": tiles " ++ Nat.show(Nat.sub(t7, t6)) ++ " ms " ++ U32.show(h6))
    +h4 : U32 = phash(TI.Tiles.image(TI.Frame.make(3440, 1400, os, Pix{855826})))
    +t4 : Nat <- IO.now()
    +h5 : U32 = phash(R.Frame.render(3440, 1400, os, 855826))
    +t5 : Nat <- IO.now()
    IO.print("tiles+phash " ++ Nat.show(Nat.sub(t4, t2)) ++ " ms  whole+phash " ++ Nat.show(Nat.sub(t5, t4)) ++ " ms " ++ U32.show(h4) ++ U32.show(h5))
    +h8 : U32 = qh(6n, os, 0, 0, 4096)
    +t8 : Nat <- IO.now()
    IO.print("quadtree hashed in place " ++ Nat.show(Nat.sub(t8, t5)) ++ " ms " ++ U32.show(h8))
    +h9 : U32 = qc(6n, os, 0, 0, 0, 0, 4096)
    +t9 : Nat <- IO.now()
    IO.print("quadtree cut in the callee " ++ Nat.show(Nat.sub(t9, t8)) ++ " ms " ++ U32.show(h9))
    +h10 : U32 = qz(6n, os, 0, 0, 0, 0, 4096)
    +t10 : Nat <- IO.now()
    IO.print("quadtree, tiles drawn serially " ++ Nat.show(Nat.sub(t10, t9)) ++ " ms " ++ U32.show(h10))
    +h11 : U32 = qr(6n, os, 0, 0, 4096)
    +t11 : Nat <- IO.now()
    IO.print("quadtree, cut from the whole list " ++ Nat.show(Nat.sub(t11, t10)) ++ " ms " ++ U32.show(h11))
    +js : List<&2, Job> = jobs(6n, os, 0, 0, 4096, Nil{})
    +t12 : Nat <- IO.now()
    +jt : JT = deal.k(40n, halves(js, Two{Nil{}, Nil{}}))
    +t13 : Nat <- IO.now()
    +h13 : U32 = run(jt)
    +t14 : Nat <- IO.now()
    IO.print("jobs " ++ Nat.show(List.length(&2, Job, js)) ++ ": cut " ++ Nat.show(Nat.sub(t12, t11)) ++ " ms  deal " ++ Nat.show(Nat.sub(t13, t12)) ++ " ms  draw " ++ Nat.show(Nat.sub(t14, t13)) ++ " ms " ++ U32.show(h13))
    +ms : List<&2, R.Mask> = masks(64n)
    +gs : List<&2, R.Op> = glines(ms, 66n)
    +t15 : Nat <- IO.now()
    +gj : List<&2, Job> = jobs(6n, gs, 0, 0, 4096, Nil{})
    +t16 : Nat <- IO.now()
    +h16 : U32 = run(deal.k(40n, halves(gj, Two{Nil{}, Nil{}})))
    +t17 : Nat <- IO.now()
    +h17 : U32 = run.copy(deal.k(40n, halves(gj, Two{Nil{}, Nil{}})))
    +t18 : Nat <- IO.now()
    IO.print("heap masks, jobs " ++ Nat.show(List.length(&2, Job, gj)) ++ ": cut " ++ Nat.show(Nat.sub(t16, t15)) ++ " ms  draw shared " ++ Nat.show(Nat.sub(t17, t16)) ++ " ms  draw copied " ++ Nat.show(Nat.sub(t18, t17)) ++ " ms " ++ U32.show(h16) ++ " " ++ U32.show(h17))
    +h3 : U32 = flat(12n, 0, os)
    +t3 : Nat <- IO.now()
    IO.print("flat " ++ Nat.show(Nat.sub(t3, t18)) ++ " ms " ++ U32.show(h3))
    IO.print(Nat.show(k) ++ " ops  tiles " ++ Nat.show(Nat.sub(t1, t0)) ++ " ms  whole " ++ Nat.show(Nat.sub(t2, t1)) ++ " ms  " ++ U32.show(ha) ++ " " ++ U32.show(hb))
