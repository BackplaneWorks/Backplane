import Base
import ../../src/core/board.bend as B
import ../../src/core/schematic.bend as SC
import ../../src/core/client.bend as U
import ../../src/core/model.bend as M
import ../../src/app/nui.bend as N
import ../../src/app/win.bend as W
import ../../src/app/layout.bend as L
import ../../src/app/viewer.bend as V
import ../../src/app/theme.bend as T
import ../../src/app/bots.bend as BV
import ../../src/app/cats.bend as CA
import ../../src/app/main.bend as MA
import ../../src/server/server.bend as SV
import ../../src/gfx/raster.bend as R
import ../../src/gfx/tiles.bend as TI
import ../../src/gfx/bitmap.bend as BM
import ../../src/gfx/draw.bend as D
import ../../src/gfx/fresh.bend as FR
import ../../src/server/argv.bend as AV

# The board or schematic viewer, timed: each step goes through the same
# work as a frame in the app (the event, the viewer's base, the memo,
# layout, the tiles planned, drawn in parallel and filled) with no window.
# Drags that pan, wheel notches that zoom, and keys typed with the viewer
# open; every frame is checked against the one drawn from scratch. Also
# times the parts of a base drawn from scratch (FR.Fresh.frame), and
# writes the base a pan left and one drawn from scratch at the same view
# (/tmp/vb-panned.ppm, /tmp/vb-fresh.ppm) to compare.
#   scripts/build-js.sh test/native/viewer_bench.bend build/viewer_bench
#   build/viewer_bench FILE.kicad_pcb|FILE.kicad_sch [W H]

def para(+i: Nat) -> String:
  "Message " ++ Nat.show(i) ++ ": the regulator's enable pin is pulled up through R12 to VBAT, so the output comes up before the MCU "
    ++ "drives it low. Moving R12 to the 3V3 rail fixes the sequencing. **Next:** rerun DRC.\n"

def msgs(+n: Nat, +th: String, +at: Nat) -> List<&2, M.Change>:
  match n:
    case 0n:
      Nil{}
    case 1n+k:
      M.MessagePosted{th, th ++ "m" ++ Nat.show(k), Bool.pick(M.Role, Nat.is_eq(Nat.mod(k, 2n), 0n), M.User{}, M.Assistant{}), para(k), Nat.add(at, k)} <> msgs(k, th, at)

def log(+at: Nat) -> List<&2, M.Change>:
  M.ProjectCreated{"p1", "board", "/tmp/board", at} <> M.ThreadCreated{"t0", "p1", "Power sequencing", M.Local{}, "", "", "claude", "", at} <> msgs(40n, "t0", at)

def ui_of(a: U.Act) -> U.Ui:
  match a:
    case U.Act{u, _}:
      u

def items(m: Maybe<&2, List<&2, B.Item>>) -> List<&2, B.Item>:
  match m:
    case Some{xs}:
      xs
    case None{}:
      Nil{}

def ends_sch(+p: String) -> Bool:
  String.ends_with(p, ".kicad_sch")

def parse(sch: Bool, bs: List<&2, U32>) -> List<&2, B.Item>:
  match sch:
    case True{}:
      items(SC.Sch.items(bs))
    case False{}:
      items(B.Board.items(bs))

def hits(o: L.Out) -> List<&2, N.Hit>:
  match o:
    case L.Out{_, hs}:
      hs

def ops(o: L.Out) -> List<&2, R.Op>:
  match o:
    case L.Out{os, _}:
      os

def step_nui(s: N.Step) -> N.Nui:
  match s:
    case N.Step{n, _, _}:
      n

def with_hits(n: N.Nui, hs: List<&2, N.Hit>, +ms: Nat) -> N.Nui:
  match n:
    case N.Nui{u, w, h, mx, my, sd, sc, ca, fo, pa, _, _, vw}:
      N.Nui{u, w, h, mx, my, sd, sc, ca, fo, pa, hs, ms, vw}

# the frame's base brought up to date, as Shell.paint.k does
def based(+n: N.Nui, +ms: Nat) -> N.Nui:
  N.Nui.with_vw(n, MA.Shell.vw.if(N.Nui.panel(n), N.Nui.vw(n), N.Nui.width(n), N.Nui.height(n), N.Nui.vx0(n), ms, N.Nui.flip(n), N.Nui.lfocus.bits(n)))

def base_of(vw: V.Vw) -> Image:
  match vw:
    case V.Vw{_, _, _, _, _, _, _, b, _, _, _}:
      b

# the base's pixel count (evaluates it)
def leaves(i: Image) -> Nat:
  match i:
    case Pix{_}:
      1n
    case Qua{a, b, c, d}:
      Nat.add(Nat.add(leaves(a), leaves(b)), Nat.add(leaves(c), leaves(d)))

type Kept is Data:
  Kept{n: N.Nui, memo: L.TLMemo, frames: TI.Frames}

type Times is Data:
  Times{ev: Nat, base: Nat, lay: Nat, plan: Nat, draw: Nat, fill: Nat, worst: Nat, drawn: Nat}

def Times.zero() -> Times:
  Times{0n, 0n, 0n, 0n, 0n, 0n, 0n, 0n}

def Times.add(t: Times, +a: Nat, +b: Nat, +c: Nat, +d: Nat, +e: Nat, +f: Nat, +k: Nat) -> Times:
  match t:
    case Times{ea, eb, ec, ed, ee, ef, +w, dk}:
      +all = Nat.add(Nat.add(Nat.add(a, b), Nat.add(c, d)), Nat.add(e, f))
      Times{Nat.add(ea, a), Nat.add(eb, b), Nat.add(ec, c), Nat.add(ed, d), Nat.add(ee, e), Nat.add(ef, f), Bool.pick(Nat, Nat.is_gt(all, w), all, w), Nat.add(dk, k)}

def tenths(+x: Nat, +n: Nat) -> String:
  Nat.show(Nat.div(x, n)) ++ "." ++ Nat.show(Nat.mod(Nat.div(Nat.mul(x, 10n), n), 10n))

def Times.show(t: Times, +n: Nat) -> String:
  match t:
    case Times{+a, +b, +c, +d, +e, +f, w, k}:
      "event " ++ tenths(a, n) ++ "  base " ++ tenths(b, n) ++ "  layout " ++ tenths(c, n) ++ "  plan " ++ tenths(d, n) ++ "  draw " ++ tenths(e, n)
        ++ "  fill " ++ tenths(f, n) ++ "  | total " ++ tenths(Nat.add(Nat.add(Nat.add(a, b), Nat.add(c, d)), Nat.add(e, f)), n) ++ " ms  worst " ++ Nat.show(w)
        ++ "  tiles drawn " ++ tenths(k, n)

def same(+w: U32, +h: U32, +os: List<&2, R.Op>, +vw: V.Vw, +x0: U32, +ms: Nat, +flip: U32, +tiles: TI.Tiles) -> Bool:
  TI.Eq.image(TI.Tiles.image(tiles), TI.Tiles.image(TI.Plan.run(MA.Shell.image(True{}, os, vw, w, h, x0, ms, flip, 0, Nil{}, Map.new(&2, BM.Bitmap), TI.FNone{}, 0), TI.Frame.edge(w, h))))

# a frame's steps, timed
type Laid is Data:
  Laid{n: N.Nui, memo: L.TLMemo, o: L.Out, ev: Nat, base: Nat, lay: Nat}

def f.lay(+ty: L.Ty, +cats: Map<&2, CA.CatF>, +n1: N.Nui, memo0: L.TLMemo, +ms: Nat, +ev: Nat) -> IO(Laid):
  do IO<Laid>:
    +t1 : Nat <- IO.now()
    +n2 : N.Nui = based(n1, ms)
    +lv : Nat = leaves(base_of(N.Nui.vw(n2)))
    +t2 : Nat <- IO.now()
    +memo : L.TLMemo = L.Lay.memo(ty, n2, memo0)
    +o : L.Out = BV.Bv.frame(ty, memo, cats, n2)
    +k2 : Nat = List.length(&2, R.Op, ops(o))
    +t3 : Nat <- IO.now()
    return Laid{n2, memo, o, ev, Nat.sub(t2, t1), Nat.sub(t3, t2)}

def f.ev(e: W.WinEv, +ty: L.Ty, +cats: Map<&2, CA.CatF>, +n0: N.Nui, memo0: L.TLMemo, +ms: Nat) -> IO(Laid):
  do IO<Laid>:
    +t0 : Nat <- IO.now()
    +n1 : N.Nui = MA.Shell.stamp(step_nui(N.Nui.event(n0, e)), ms)
    +t1 : Nat <- IO.now()
    f.lay(ty, cats, n1, memo0, ms, Nat.sub(t1, t0))

type Fin is Data:
  Fin{k: Kept, t: Times, ok: Bool}

def f.tiles.k(+n2: N.Nui, +o: L.Out, +memo: L.TLMemo, +oh: List<&2, N.Hit>, fr0: TI.Frames, +ms: Nat, +a: Nat, +b: Nat, +c: Nat, t: Times, ok: Bool, +check: Bool) -> IO(Fin):
  do IO<Fin>:
    +t3 : Nat <- IO.now()
    +pl : TI.Plan = MA.Shell.image(N.Nui.panel(n2), ops(o), N.Nui.vw(n2), N.Nui.width(n2), N.Nui.height(n2), N.Nui.vx0(n2), ms, N.Nui.flip(n2), N.Nui.lfocus.bits(n2), hits(o), Map.new(&2, BM.Bitmap), fr0, N.Hits.slide(hits(o), oh))
    +kd : Nat = TI.Plan.count(pl)
    +t4 : Nat <- IO.now()
    +d : TI.Drawn = TI.Plan.drawn(pl, TI.Frame.edge(N.Nui.width(n2), N.Nui.height(n2)))
    +t5 : Nat <- IO.now()
    +tiles : TI.Tiles = TI.Plan.fill(pl, TI.Frame.edge(N.Nui.width(n2), N.Nui.height(n2)), 0n, d)
    +img : Image = TI.Tiles.image(tiles)
    +t6 : Nat <- IO.now()
    return Fin{Kept{with_hits(n2, hits(o), ms), memo, TI.FSome{N.Nui.width(n2), N.Nui.height(n2), tiles}},
      Times.add(t, a, b, c, Nat.sub(t4, t3), Nat.sub(t5, t4), Nat.sub(t6, t5), kd),
      Bool.and(ok, Bool.or(Bool.not(check), same(N.Nui.width(n2), N.Nui.height(n2), ops(o), N.Nui.vw(n2), N.Nui.vx0(n2), ms, N.Nui.flip(n2), tiles)))}

def f.tiles(l: Laid, +oh: List<&2, N.Hit>, fr0: TI.Frames, +ms: Nat, t: Times, ok: Bool, +check: Bool) -> IO(Fin):
  match l:
    case Laid{n2, memo, o, a, b, c}:
      f.tiles.k(n2, o, memo, oh, fr0, ms, a, b, c, t, ok, check)

# one event and the frame after it
def frame(e: W.WinEv, +ty: L.Ty, +cats: Map<&2, CA.CatF>, k: Kept, +ms: Nat, t: Times, ok: Bool, +check: Bool) -> IO(Fin):
  match k:
    case Kept{+n0, memo0, fr0}:
      IO.bind(Laid, Fin, f.ev(e, ty, cats, n0, memo0, ms), l => f.tiles(l, N.Nui.hits(n0), fr0, ms, t, ok, check))

def run.k(d: Fin, k: Kept -> Times -> Bool -> IO(Kept & Times & Bool)) -> IO(Kept & Times & Bool):
  match d:
    case Fin{a, b, c}:
      k(a, b, c)

# each event and the frame after it, timed step by step
def run(es: List<&2, W.WinEv>, +ty: L.Ty, +cats: Map<&2, CA.CatF>, k: Kept, +ms: Nat, t: Times, ok: Bool, +check: Bool) -> IO(Kept & Times & Bool):
  match es:
    case Nil{}:
      IO.pure(Kept & Times & Bool, (k, (t, ok)))
    case e <> rest:
      IO.bind(Fin, Kept & Times & Bool, frame(e, ty, cats, k, ms, t, ok, check),
        d => run.k(d, k2 => t2 => ok2 => run(rest, ty, cats, k2, Nat.add(ms, 20n), t2, ok2, check)))

def report(r: Kept & Times & Bool, +label: String, +n: Nat) -> IO(Kept):
  (k, rest) = r
  (t, ok) = rest
  do IO<Kept>:
    IO.print(label ++ Times.show(t, n) ++ "  exact " ++ Bool.show(ok))
    return k

# n moves of a drag from (x, y), 9 and 5 px a step
def drags(n: Nat, +x: U32, +y: U32) -> List<&2, W.WinEv>:
  match n:
    case 0n:
      Nil{}
    case 1n+m:
      W.WMove{x, y, 0} <> drags(m, (x + 9 : U32), (y + 5 : U32))

# n notches in, then n out
def wheels(n: Nat, +dir: U32, +x: U32, +y: U32) -> List<&2, W.WinEv>:
  match n:
    case 0n:
      Nil{}
    case 1n+m:
      W.WWheel{x, y, dir} <> wheels(m, dir, x, y)

def keys(s: String) -> List<&2, W.WinEv>:
  match s:
    case SNil{}:
      Nil{}
    case SCon{Chr{+c}, t}:
      W.WKey{c, c, 0, True{}, 0} <> keys(t)

def warm(+ty: L.Ty, +cats: Map<&2, CA.CatF>, +n0: N.Nui, +ms: Nat) -> Kept:
  +n = based(MA.Shell.stamp(n0, ms), ms)
  +memo = L.Lay.memo(ty, n, L.TL.memo.none())
  +o = BV.Bv.frame(ty, memo, cats, n)
  Kept{with_hits(n, hits(o), ms), memo, TI.FSome{N.Nui.width(n), N.Nui.height(n),
    TI.Plan.run(MA.Shell.image(N.Nui.panel(n), ops(o), N.Nui.vw(n), N.Nui.width(n), N.Nui.height(n), N.Nui.vx0(n), ms, N.Nui.flip(n), N.Nui.lfocus.bits(n), hits(o), Map.new(&2, BM.Bitmap), TI.FNone{}, 0), TI.Frame.edge(N.Nui.width(n), N.Nui.height(n)))}}

def arg.u(+xs: List<&2, String>, +i: Nat, +d: U32) -> U32:
  F32.to_u32(Maybe.default(&2, F32, F32.read(Maybe.default(&2, String, List.get(&2, String, xs, i), "")), U32.to_f32(d)))

def copy(xs: List<&1, String>) -> List<&2, String>:
  match xs:
    case Nil{}:
      Nil{}
    case x <> t:
      x <> copy(t)

def kept_n(k: Kept) -> N.Nui:
  match k:
    case Kept{n, _, _}:
      n

# the base drawn from scratch at the viewer's view
def fresh.k(vw: V.Vw, +w: U32, +h: U32, +x0: U32, +ms: Nat, +flip: U32) -> Image:
  match vw:
    case V.Vw{+ss, +v, _, _, _, _, _, _, _, _, +vx}:
      MA.Shell.base(w, h, x0, ss, v, ms, flip, 0, False{}, vx)

def fresh(+n: N.Nui, +ms: Nat) -> Image:
  fresh.k(N.Nui.vw(n), N.Nui.width(n), N.Nui.height(n), N.Nui.vx0(n), ms, N.Nui.flip(n))

# the base the pan left and one drawn from scratch, as PPMs
def snaps(+k: Kept, +ms: Nat) -> IO(Unit):
  do IO<Unit>:
    g : File <- IO.try(File, File.open("/tmp/vb-panned.ppm", "w"))
    w : File & Result<&1, &1, U32 & String, Unit> <- File.write_bytes(g, D.Snap.ppm(base_of(N.Nui.vw(kept_n(k))), N.Nui.width(kept_n(k)), N.Nui.height(kept_n(k))))
    g2 : File <- IO.try(File, File.open("/tmp/vb-fresh.ppm", "w"))
    w2 : File & Result<&1, &1, U32 & String, Unit> <- File.write_bytes(g2, D.Snap.ppm(fresh(kept_n(k), ms), N.Nui.width(kept_n(k)), N.Nui.height(kept_n(k))))
    return Unit{}

def ss_of(vw: V.Vw) -> List<&2, V.Seen>:
  match vw:
    case V.Vw{ss, _, _, _, _, _, _, _, _, _, _}:
      ss

def view_of(vw: V.Vw) -> V.View:
  match vw:
    case V.Vw{_, v, _, _, _, _, _, _, _, _, _}:
      v

def kind_of(vw: V.Vw) -> U32:
  match vw:
    case V.Vw{_, _, _, _, _, _, _, _, _, _, V.Vx{k, _, _, _, _, _, _, _, _, _, _}}:
      k

# the parts of a base drawn from scratch, timed
def parts(+n: N.Nui) -> IO(Unit):
  do IO<Unit>:
    +w : U32 = N.Nui.width(n)
    +h : U32 = N.Nui.height(n)
    +x0 : U32 = N.Nui.vx0(n)
    +t0 : Nat <- IO.now()
    +ops : List<&2, R.Op> = List.reverse(&2, R.Op, D.Draw.rect(D.Clip.all(), x0, 52, (w - x0 : U32), h, 526603, 255) <> V.Ops.scene(ss_of(N.Nui.vw(n)), view_of(N.Nui.vw(n)), D.Clip{x0, 52, w, h}, 200000n, 200000n, True{}, kind_of(N.Nui.vw(n)), 0))
    +k : Nat = List.length(&2, R.Op, ops)
    +t1 : Nat <- IO.now()
    +pl : TI.Plan = TI.Tiles.plan.make(TI.Frame.levels(w, h), TI.Frame.edge(w, h), ops, Pix{T.Th.bg()}, 0, 0, U32.shln(1, R.Frame.depth(w, h)))
    +kd : Nat = TI.Plan.count(pl)
    +t2 : Nat <- IO.now()
    +d : TI.Drawn = TI.Plan.drawn(pl, TI.Frame.edge(w, h))
    +t3 : Nat <- IO.now()
    +img : Image = TI.Tiles.image(TI.Plan.fill(pl, TI.Frame.edge(w, h), 0n, d))
    +lv : Nat = leaves(img)
    +t4 : Nat <- IO.now()
    +fp : FR.FPlan = FR.FPlan.of(Nat.sub(R.Frame.depth(w, h), FR.Fresh.e()), False{}, ops, 0, 0, U32.shln(1, R.Frame.depth(w, h)))
    +nb : Nat = FR.FPlan.count(fp)
    +ta : Nat <- IO.now()
    +fj : FR.FJobs = FR.FJobs.deal(24n, FR.FHalves.of(FR.FPlan.jobs(fp, Nil{})))
    +fd : FR.FDrawn = FR.FJobs.run(fj, T.Th.bg())
    +tb : Nat <- IO.now()
    +sh : Image = FR.Fresh.fill(fp, fd, T.Th.bg())
    +lv2 : Nat = leaves(sh)
    +t5 : Nat <- IO.now()
    IO.print("Fresh: split " ++ Nat.show(Nat.sub(ta, t4)) ++ " ms (" ++ Nat.show(nb) ++ " blocks), deal+draw " ++ Nat.show(Nat.sub(tb, ta)) ++ ", fill+count " ++ Nat.show(Nat.sub(t5, tb)))
    IO.print("fresh base: ops " ++ Nat.show(Nat.sub(t1, t0)) ++ " ms (" ++ Nat.show(k) ++ "), plan " ++ Nat.show(Nat.sub(t2, t1)) ++ " (" ++ Nat.show(kd) ++ " tiles), draw " ++ Nat.show(Nat.sub(t3, t2))
      ++ ", fill " ++ Nat.show(Nat.sub(t4, t3)) ++ " (" ++ Nat.show(lv) ++ " leaves); Fresh.frame " ++ Nat.show(Nat.sub(t5, t4)) ++ " ms")

def go(+path: String, +w: U32, +h: U32) -> IO(Unit):
  do IO<Unit>:
    bs : List<&2, U32> <- SV.Files.read(path)
    +ty : L.Ty <- L.Ty.load()
    +u : U.Ui = ui_of(U.Ui.act(ui_of(U.Ui.act(U.Ui.fold(log(1790000000n), U.Ui.init(1790000100n)), "select", "t0")), "panel", ""))
    +cats : Map<&2, CA.CatF> = BV.Bv.warm(u, CA.Cats.none(), 0n)
    +n0 : N.Nui = MA.Shell.stamp(N.Nui.init(u, w, h), 0n)
    +its : List<&2, B.Item> = V.Scene.sort(parse(ends_sch(path), bs))
    +n1 : N.Nui = N.Nui.board(N.Nui.asked(n0, N.Nui.source(n0)), N.Nui.source(n0), its, 0n)
    +n2 : N.Nui = N.Nui.with_vw(n1, N.Vw.fit(N.Nui.vw(n1), N.Nui.vx0(n1), h, w))
    +a : Nat <- IO.now()
    +k0 : Kept = warm(ty, cats, n2, 100000n)
    +b : Nat <- IO.now()
    IO.print(Nat.show(List.length(&2, B.Item, its)) ++ " items, " ++ U32.show(w) ++ "x" ++ U32.show(h) ++ ", panel from x " ++ U32.show(N.Nui.vx0(n2)) ++ "; first frame " ++ Nat.show(Nat.sub(b, a)) ++ " ms")
    +cx : U32 = (N.Nui.vx0(n2) + U32.shr((w - N.Nui.vx0(n2) : U32)) : U32)
    +cy : U32 = U32.shr(h)
    parts(kept_n(k0))
    parts(kept_n(k0))
    r1 : Kept & Times & Bool <- run(W.WButton{cx, cy, 1, True{}} <> drags(20n, cx, cy), ty, cats, k0, 101000n, Times.zero(), True{}, True{})
    +k1 : Kept <- report(r1, "pan   (21) ", 21n)
    snaps(k1, 101000n)
    r2 : Kept & Times & Bool <- run(W.WButton{(cx + 180 : U32), (cy + 100 : U32), 1, False{}} <> wheels(6n, 0, cx, cy), ty, cats, k1, 102000n, Times.zero(), True{}, True{})
    +k2 : Kept <- report(r2, "zoom  (7)  ", 7n)
    r3 : Kept & Times & Bool <- run(keys("typing with the board open "), ty, cats, k2, 103000n, Times.zero(), True{}, False{})
    k3 : Kept <- report(r3, "keys  (27) ", 27n)
    r4 : Kept & Times & Bool <- run(W.WButton{cx, cy, 1, True{}} <> drags(20n, cx, cy), ty, cats, k3, 104000n, Times.zero(), True{}, False{})
    k4 : Kept <- report(r4, "pan zoomed (21) ", 21n)
    IO.print("done")

def main() -> IO(Unit):
  do IO<Unit>:
    args : List<String> <- AV.Argv.args()
    +xs : List<&2, String> = copy(args)
    go(Maybe.default(&2, String, List.head(&2, String, xs), "x"), arg.u(xs, 1n, 2560), arg.u(xs, 2n, 1400))
