import Base
import ../check.bend as C
import ../glyph_table_test.bend as G
import ../../src/gfx/text.bend as X
import ../../src/gfx/raster.bend as R
import ../../src/gfx/draw.bend as D
import ../../src/gfx/tiles.bend as T
import ../../src/server/perf.bend as P

# Native font-fill, exact-pixel and tile-plan measurements. Each sample
# is printed in microseconds; the driver reports p50/p99/max from all of
# them. "masks" keeps the pre-table ops and compares their full masks;
# "keys" includes the full Eq.tab guard, without per-frame conversion.
def sample() -> String:
  "Message 001: the regulator enable is pulled up through R12 to VBAT; check U3 footprint, then rerun DRC."

def lines(+n: Nat, +c: Map<&2, X.TG>, +f: X.Face) -> List<&2, R.Op>:
  match n:
    case 0n:
      Nil{}
    case 1n+k:
      List.append(&2, R.Op, X.Text.draw(c, f, sample(), D.Clip.all(), 300, U32.from_nat(Nat.add(24n, Nat.mul(k, 19n))), 15790320), lines(k, c, f))

def nodes(t: R.Tab) -> Nat:
  match t:
    case R.TNil{}:
      0n
    case R.TBr{_, _, _, a, b}:
      1n+Nat.add(nodes(a), nodes(b))

def tiled(+os: List<&2, R.Op>, +tab: R.Tab) -> Image:
  T.Tiles.image(T.Frame.make(80, 40, R.Key.ops(os), Pix{0}, tab))

def small(+c: Map<&2, X.TG>, +f: X.Face) -> List<&2, R.Op>:
  X.Text.draw(c, f, "Aé日α→", D.Clip.all(), 2, 22, 16777215)

def exact(+c: Map<&2, X.TG>, +f: X.Face, +tab: R.Tab) -> IO(Unit):
  +os = small(c, f)
  do IO<Unit>:
    C.Check.str("native nonASCII fill drains wants", Nat.show(List.length(&2, X.Want, X.Text.wants(os, Nil{}))), "0")
    C.Check.str("native layout emits keys directly", Bool.show(T.Eq.ops(X.Text.keydraw(c, f, "Aé日α→", D.Clip.all(), 2, 22, 16777215), R.Key.ops(os))), "True")
    C.Check.str("native table resolves original masks", Bool.show(T.Eq.ops(R.Res.ops(R.Key.ops(os), tab), os)), "True")
    C.Check.str("native keyed pixels equal original RGlyph bytes", G.same(tiled(os, tab), R.Frame.render(80, 40, os, 0)), "True")

def report(+name: String, +i: Nat, +a: U32, +b: U32, k: Nat) -> IO(Unit):
  IO.print("sample," ++ name ++ "," ++ Nat.show(i) ++ "," ++ U32.show((b - a : U32)) ++ "," ++ Nat.show(k))

def plans(+n: Nat, +os: List<&2, R.Op>, +keyed: List<&2, R.Op>, +tab: R.Tab, +grown: R.Tab, +keys: T.Tiles, +masks: T.Tiles) -> IO(Unit):
  match n:
    case 0n:
      IO.pure(Unit, Unit{})
    case 1n+k:
      do IO<Unit>:
        a : U32 <- P.Perf.us()
        km : Nat = T.Plan.count(T.Frame.plan.k(True{}, 1400, 900, os, Pix{0}, masks, 0))
        +b : U32 <- P.Perf.us()
        kk : Nat = T.Plan.count(T.Frame.plan(1400, 900, keyed, Pix{0}, tab, T.FSome{1400, 900, tab, keys}, 0))
        +d : U32 <- P.Perf.us()
        kp : Nat = T.Plan.count(T.Frame.plan.k(True{}, 1400, 900, keyed, Pix{0}, keys, 0))
        +e : U32 <- P.Perf.us()
        guard : Nat = Bool.pick(Nat, T.Frame.same(1400, 900, tab, 1400, 900, tab, keys), 1n, 0n)
        +g : U32 <- P.Perf.us()
        convert : Nat = Bool.pick(Nat, T.Eq.ops(R.Key.ops(os), keyed), 1n, 0n)
        +h : U32 <- P.Perf.us()
        eq : Nat = Bool.pick(Nat, T.Eq.ops(keyed, keyed), 1n, 0n)
        +j : U32 <- P.Perf.us()
        ex : Nat = T.Plan.count(T.Frame.plan(1400, 900, keyed, Pix{0}, grown, T.FSome{1400, 900, tab, keys}, 0))
        z : U32 <- P.Perf.us()
        report("masks", k, a, b, km)
        report("keys", k, b, d, kk)
        report("prekeyed", k, d, e, kp)
        report("table-guard", k, e, g, guard)
        report("convert-and-eq", k, g, h, convert)
        report("key-eq", k, h, j, eq)
        report("extension-plan", k, j, z, ex)
        plans(k, os, keyed, tab, grown, keys, masks)

def main() -> IO(Unit):
  do IO<Unit>:
    a : U32 <- P.Perf.us()
    +f : X.Face <- X.Text.face(["monospace"], 14)
    b : U32 <- P.Perf.us()
    +empty : Map<&2, X.TG> = Map.new(&2, X.TG)
    +cold : List<&2, R.Op> = small(empty, f)
    +nw : Nat = List.length(&2, X.Want, X.Text.wants(cold, Nil{}))
    report("font-open", 0n, a, b, nw)
    C.Check.str("native font loaded", Bool.show(X.Face.ft(f)), "True")
    C.Check.str("native cold stream has wants", Bool.show(Nat.is_gt(nw, 0n)), "True")
    t0 : U32 <- P.Perf.us()
    +c0 : Map<&2, X.TG> <- X.Text.warm(empty, f)
    +tab0 : R.Tab = X.Text.table(c0)
    +k0 : Nat = nodes(tab0)
    t1 : U32 <- P.Perf.us()
    report("ascii-fill-table", 0n, t0, t1, k0)
    +os0 : List<&2, R.Op> = small(c0, f)
    +before : T.Frames = T.FSome{80, 40, tab0, T.Frame.make(80, 40, R.Key.ops(os0), Pix{0}, tab0)}
    t2 : U32 <- P.Perf.us()
    +c : Map<&2, X.TG> <- X.Text.fill(X.Text.wants(os0, Nil{}), c0)
    +tab : R.Tab = X.Text.table(c)
    +kc : Nat = nodes(tab)
    +t3 : U32 <- P.Perf.us()
    report("nonASCII-fill-table", 0n, t2, t3, kc)
    +smallos : List<&2, R.Op> = small(c, f)
    +after : T.Tiles = T.Frame.par(80, 40, R.Key.ops(smallos), Pix{0}, tab, before, 0)
    +ok : String = G.same(T.Tiles.image(after), R.Frame.render(80, 40, smallos, 0))
    t4 : U32 <- P.Perf.us()
    report("fill-redraw-and-byte-check", 0n, t3, t4, kc)
    C.Check.str("native cold to warm frame is fresh", ok, "True")
    exact(c, f, tab)
    +os : List<&2, R.Op> = lines(44n, c, f)
    t5 : U32 <- P.Perf.us()
    +keys : T.Tiles = T.Frame.make(1400, 900, R.Key.ops(os), Pix{0}, tab)
    +img : Image = T.Tiles.image(keys)
    +px : U32 = R.Img.at(11n, img, 300, 24)
    t6 : U32 <- P.Perf.us()
    report("cold-frame", 0n, t5, t6, U32.to_nat(px))
    +masks : T.Tiles = T.Tiles.make(T.Frame.levels(1400, 900), T.Frame.edge(1400, 900), R.TNil{}, os, Pix{0}, 0, 0, 2048)
    +grown : R.Tab = X.Text.table(Map.set(&2, X.TG, c, X.Text.key(X.Face.id(f), 26412), X.Text.get(c, f, 65)))
    plans(100n, os, R.Key.ops(os), tab, grown, keys, masks)
