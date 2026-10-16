import Base
import ../../src/core/client.bend as U
import ../../src/core/model.bend as M
import ../../src/app/nui.bend as N
import ../../src/app/win.bend as W
import ../../src/app/layout.bend as L
import ../../src/app/theme.bend as T
import ../../src/gfx/raster.bend as R
import ../../src/gfx/tiles.bend as TI
import ../../src/app/bots.bend as BV
import ../../src/app/cats.bend as CA
import ../../src/core/json.bend as J
import ../../src/core/utf8.bend as U8
import ../../src/core/botui.bend as BU
import ../../src/core/bot.bend as BT
import ../../src/gfx/draw.bend as D
import ../../src/server/argv.bend as AV

# A bot's screens on a real log, timed as the window makes each frame
# (prepare, the timeline memo, layout, the tiles): from a thread, the
# steps open a room or a bot ("room=ID", "bot=ID", apart by ;), then that
# frame again, keys typed into the focused field, cat frames and wheel
# notches over the main area. ms per frame, split memo/layout/tiles, and
# how many frames were not the one drawn from scratch (none should be).
#   scripts/build-app.sh test/native/bots_bench.bend build/bots_bench
#   build/bots_bench EVENTS STEPS [FOCUS [THREAD [REMOTE [busy [DIR]]]]]
# FOCUS "room" is the room's line, "remote" a remote bot's, else the
# composer; REMOTE is "bots.remote" as the hub would send it ("" for
# the one here); busy: every bot's turn running, so the cats move; DIR:
# the frames after opening and after the wheel as PPMs (open, up, down).

def ui_of(a: U.Act) -> U.Ui:
  match a:
    case U.Act{u, _}:
      u

type Kept is Data:
  Kept{n: N.Nui, memo: L.TLMemo, frames: TI.Frames, cats: Map<&2, CA.CatF>, wraps: Map<&2, BV.PW>}

type Times is Data:
  Times{ev: Nat, memo: Nat, lay: Nat, tiles: Nat, worst: Nat, k: Nat, bad: Nat}

def Times.none() -> Times:
  Times{0n, 0n, 0n, 0n, 0n, 0n, 0n}

def Times.add(t: Times, +a: Nat, +b: Nat, +c: Nat, +d: Nat, +ok: Bool) -> Times:
  match t:
    case Times{e, m, l, ti, +w, k, x}:
      +s = Nat.add(a, Nat.add(b, Nat.add(c, d)))
      Times{Nat.add(e, a), Nat.add(m, b), Nat.add(l, c), Nat.add(ti, d), Bool.pick(Nat, Nat.is_gt(s, w), s, w), 1n+k, Nat.add(x, Bool.pick(Nat, ok, 0n, 1n))}

type KT is Data:
  KT{k: Kept, t: Times}

def dec(+x: Nat, +n: Nat) -> String:
  Nat.show(Nat.div(x, n)) ++ "." ++ Nat.show(Nat.mod(Nat.div(Nat.mul(x, 10n), n), 10n))

def Times.show(t: Times, +label: String) -> String:
  match t:
    case Times{+e, +m, +l, +ti, w, +k, x}:
      +d = Bool.pick(Nat, Nat.is_eq(k, 0n), 1n, k)
      label ++ " (" ++ Nat.show(k) ++ "): event " ++ dec(e, d) ++ "  memo " ++ dec(m, d) ++ "  layout " ++ dec(l, d) ++ "  tiles " ++ dec(ti, d)
        ++ "  total " ++ dec(Nat.add(e, Nat.add(m, Nat.add(l, ti))), d) ++ " ms  worst " ++ Nat.show(w) ++ "  not exact " ++ Nat.show(x)

def hits(o: L.Out) -> List<&2, N.Hit>:
  match o:
    case L.Out{_, hs}:
      hs

def ops(o: L.Out) -> List<&2, R.Op>:
  match o:
    case L.Out{os, _}:
      os

def with_hits(n: N.Nui, hs: List<&2, N.Hit>, +ms: Nat) -> N.Nui:
  match n:
    case N.Nui{u, w, h, mx, my, sd, sc, ca, fo, pa, _, _, vw}:
      N.Nui{u, w, h, mx, my, sd, sc, ca, fo, pa, hs, ms, vw}

def with_focus(n: N.Nui, fo: String) -> N.Nui:
  match n:
    case N.Nui{u, w, h, mx, my, sd, sc, ca, _, pa, hi, ms, vw}:
      N.Nui{u, w, h, mx, my, sd, sc, ca, fo, pa, hi, ms, vw}

def prepared(+n: N.Nui, +ms: Nat) -> N.Nui:
  BV.Bv.stamp(N.Lbx.stamp(N.Blink.stamp(N.Hover.stamp(N.Nui.prepare(n), ms), ms), ms), ms)

def step_nui(s: N.Step) -> N.Nui:
  match s:
    case N.Step{n, _, _}:
      n

def kn(k: Kept) -> N.Nui:
  match k:
    case Kept{n, _, _, _, _}:
      n

def with_n(k: Kept, n: N.Nui) -> Kept:
  match k:
    case Kept{_, m, f, c, w}:
      Kept{n, m, f, c, w}

def fst(r: KT) -> Kept:
  match r:
    case KT{k, _}:
      k

def snd(r: KT) -> Times:
  match r:
    case KT{_, t}:
      t

def ev(+n: N.Nui, e: W.WinEv) -> N.Nui:
  step_nui(N.Nui.event(n, e))

# the cats' next frame is due: the clock moves 33 ms on
def catdue(+n: N.Nui) -> N.Nui:
  N.Nui.with_ui(n, U.Ui.info_set(U.Ui.info_set(N.Nui.ui(n), "cat.due", "0"), "act.at", Nat.show(N.Nui.ms_of(n))))

def wheel(+up: Bool, +n: N.Nui) -> N.Nui:
  ev(n, W.WWheel{800, 400, Bool.pick(U32, up, 0, 1)})

# what comes before a frame: 0 nothing, 1 a cat frame due, 2 wheel up, 3 down
def next(f: Nat, +c: U32, +n: N.Nui) -> N.Nui:
  match f:
    case 0n:
      n
    case 1n:
      catdue(n)
    case 2n:
      wheel(True{}, n)
    case 3n:
      wheel(False{}, n)
    case _:
      ev(n, W.WKey{c, c, 0, True{}, 0})

# the frame drawn from scratch: every post wrapped afresh, every tile drawn
def fresh(+ty: L.Ty, +memo: L.TLMemo, +cats: Map<&2, CA.CatF>, +n: N.Nui) -> Image:
  TI.Tiles.image(TI.Frame.make(N.Nui.width(n), N.Nui.height(n), ops(BV.Bv.frame(ty, memo, cats, n)), Pix{T.Th.bg()}, L.Ty.tab(ty)))

# one frame as the app makes it, from what the last left (the posts'
# wraps kept, the tiles reused), checked against the one drawn from scratch
def frame(+ty: L.Ty, +f: Nat, +c: U32, k: Kept, t: Times, +slide: Bool) -> IO(KT):
  match k:
    case Kept{+np, memo0, fr0, cats0, ws0}:
      do IO<KT>:
        +te : Nat <- IO.now()
        +n0 : N.Nui = next(f, c, np)
        +t0 : Nat <- IO.now()
        +n1 : N.Nui = prepared(n0, t0)
        +memo : L.TLMemo = L.Lay.memo(ty, n1, memo0)
        +t1 : Nat <- IO.now()
        +cats : Map<&2, CA.CatF> = BV.Bv.warm(N.Nui.ui(n1), cats0, BV.Bv.clock.of(n1))
        +ws : Map<&2, BV.PW> = BV.Bv.wraps(ty, n1, ws0)
        +o : L.Out = BV.Bv.frame.w(ty, memo, cats, ws, n1)
        +k2 : Nat = List.length(&2, R.Op, ops(o))
        +t2 : Nat <- IO.now()
        +tiles : TI.Tiles = TI.Frame.par(N.Nui.width(n1), N.Nui.height(n1), ops(o), Pix{T.Th.bg()}, L.Ty.tab(ty), fr0, Bool.pick(U32, slide, N.Hits.slide(hits(o), N.Nui.hits(np)), 0))
        +img : Image = TI.Tiles.image(tiles)
        +t3 : Nat <- IO.now()
        +ok : Bool = TI.Eq.image(img, fresh(ty, memo, cats, n1))
        return KT{Kept{with_hits(n1, hits(o), t3), memo, TI.FSome{N.Nui.width(n1), N.Nui.height(n1), L.Ty.tab(ty), tiles}, cats, ws}, Times.add(t, Nat.sub(t0, te), Nat.sub(t1, t0), Nat.sub(t2, t1), Nat.sub(t3, t2), ok)}

# keys typed one by one (each a frame)
def keys(cs: String, +ty: L.Ty, +k: Kept, t: Times) -> IO(KT):
  match cs:
    case SNil{}:
      IO.pure(KT, KT{k, t})
    case SCon{Chr{+c}, rest}:
      IO.bind(KT, KT, frame(ty, 4n, c, k, t, True{}), +r => keys(rest, ty, fst(r), snd(r)))

# n frames, each after f made the window state from the last
def frames(+c: Nat, +ty: L.Ty, +f: Nat, +k: Kept, t: Times, +slide: Bool) -> IO(KT):
  match c:
    case 0n:
      IO.pure(KT, KT{k, t})
    case 1n+j:
      IO.bind(KT, KT, frame(ty, f, 0, k, t, slide), +r => frames(j, ty, f, fst(r), snd(r), slide))

# a real log (an events.jsonl), as a client receives it
def items(ls: List<&2, String>) -> List<&2, J.Json>:
  match ls:
    case Nil{}:
      Nil{}
    case l <> t:
      Maybe.default(&2, J.Json, J.Json.parse(l), J.Null{}) <> items(t)

def lines(r: U8.Lines & List<&2, String>) -> List<&2, String>:
  (_, ls) = r
  ls

def real(bs: List<&2, U32>, +sel: String, +remote: String) -> U.Ui:
  U.Ui.info_set(ui_of(U.Ui.act(U.Ui.recv(U.Ui.init(1790180000n), J.Json.obj([J.KV{"t", J.Str{"log"}}, J.KV{"version", J.Str{"0.1.0"}},
    J.KV{"items", J.Json.arr(items(lines(U8.Lines.feed(U8.Lines.new(), bs))))}])), "select", sel)), "bots.remote", remote)

def read.k(r: File & Result<&1, &1, U32 & String, List<&2, U32>>) -> IO(List<&2, U32>):
  (f, res) = r
  do IO<List<&2, U32>>:
    File.close(f)
    bs : List<&2, U32> <- IO.pass(List<&2, U32>, res)
    return bs

def read(+path: String) -> IO(List<&2, U32>):
  do IO<List<&2, U32>>:
    f : File <- IO.try(File, File.open(path, "r"))
    r : File & Result<&1, &1, U32 & String, List<&2, U32>> <- File.read_bytes(f, 200000000)
    read.k(r)

def copy(xs: List<&1, String>) -> List<&2, String>:
  match xs:
    case Nil{}:
      Nil{}
    case x <> t:
      x <> copy(t)

def arg(+xs: List<&2, String>, +i: Nat, +d: String) -> String:
  Maybe.default(&2, String, List.head(&2, String, List.drop(&2, String, xs, i)), d)

def step.k(+p: String, +u: U.Ui) -> U.Ui:
  +k = String.join(List.take(&2, String, String.split(p, '='), 1n), "")
  +v = String.join(List.drop(&2, String, String.split(p, '='), 1n), "=")
  ui_of(U.Ui.act(u, k, v))

def acts(ps: List<&2, String>, u: U.Ui) -> U.Ui:
  match ps:
    case Nil{}:
      u
    case +p <> t:
      acts(t, step.k(p, u))

def focus(+f: String) -> String:
  Bool.pick(String, String.eq(f, "room"), N.Fld.id("rpost", "room-post", ""), Bool.pick(String, String.eq(f, "remote"), N.Fld.id("rtext", "remote-send", ""), "composer"))

def remote() -> String:
  "[{\"name\":\"Pebble\",\"peer\":\"p97a9070fbff5a66b\",\"peerName\":\"backplane\",\"look\":925348863,\"state\":\"ready\"}]"

# every bot's turn running (its cat works, so it moves)
def working(bs: List<&2, BT.Bot>, +at: Nat) -> List<&2, M.Change>:
  match bs:
    case Nil{}:
      Nil{}
    case b <> t:
      M.TurnChanged{BT.Bot.thread(b), M.Running{}, at} <> working(t, at)

def busy(on: Bool, +u: U.Ui) -> U.Ui:
  match on:
    case True{}:
      U.Ui.fold(working(BT.World.bots(M.State.bots(U.Ui.state(u))), U.Side.now(u)), u)
    case False{}:
      u

def with_steps(+n: N.Nui, +steps: String, +fo: String) -> N.Nui:
  with_focus(N.Nui.with_ui(n, acts(String.split(steps, ';'), N.Nui.ui(n))), focus(fo))

def show(r: KT, +label: String) -> IO(Kept):
  match r:
    case KT{k, t}:
      do IO<Kept>:
        IO.print(Times.show(t, label))
        return k

def nops(o: L.Out) -> Nat:
  List.length(&2, R.Op, ops(o))

def mops(m: Maybe<&2, L.Out>) -> Nat:
  match m:
    case Some{o}:
      nops(o)
    case None{}:
      0n

def km(k: Kept) -> L.TLMemo:
  match k:
    case Kept{_, m, _, _, _}:
      m

def kw(k: Kept) -> Map<&2, BV.PW>:
  match k:
    case Kept{_, _, _, _, w}:
      w

def kc(k: Kept) -> Map<&2, CA.CatF>:
  match k:
    case Kept{_, _, _, c, _}:
      c

def part(w: Nat, +q: Nat, +ty: L.Ty, +k: Kept) -> Nat:
  match w:
    case 0n:
      nops(L.Lay.sidebar.x(ty, kn(k), 900, c => y => BV.Bv.side(ty, kc(k), kn(k), c, y)))
    case 1n:
      mops(BV.Bv.main.w(ty, km(k), kc(k), kw(k), kn(k)))
    case 2n:
      nops(BV.Bv.frame.w(ty, km(k), kc(k), kw(k), kn(k)))
    case _:
      Map.size(&2, CA.CatF, BV.Bv.warm(N.Nui.ui(kn(k)), kc(k), Nat.mul(1n+q, 33n)))

# a part of the frame laid out r times (its op count, so each is made):
# 0 the sidebar, 1 the main area, 2 the whole frame, 3 the cats posed
def rep(+r: Nat, +what: Nat, +ty: L.Ty, +k: Kept) -> Nat:
  match r:
    case 0n:
      0n
    case 1n+q:
      Nat.add(part(what, q, ty, k), rep(q, what, ty, k))

def parts(+ty: L.Ty, +k: Kept) -> IO(Unit):
  do IO<Unit>:
    +a : Nat <- IO.now()
    +x0 : Nat = rep(20n, 0n, ty, k)
    +b : Nat <- IO.now()
    +x1 : Nat = rep(20n, 1n, ty, k)
    +c : Nat <- IO.now()
    +x2 : Nat = rep(20n, 2n, ty, k)
    +d : Nat <- IO.now()
    +x3 : Nat = rep(20n, 3n, ty, k)
    +e : Nat <- IO.now()
    IO.print("each part, ms per frame (x20): sidebar " ++ dec(Nat.sub(b, a), 20n) ++ "  main " ++ dec(Nat.sub(c, b), 20n) ++ "  whole layout " ++ dec(Nat.sub(d, c), 20n)
      ++ "  cats posed " ++ dec(Nat.sub(e, d), 20n) ++ "  (ops " ++ Nat.show(Nat.div(x0, 20n)) ++ " " ++ Nat.show(Nat.div(x1, 20n)) ++ " " ++ Nat.show(Nat.div(x2, 20n)) ++ ")")

def kimg(k: Kept) -> Image:
  match k:
    case Kept{_, _, TI.FSome{_, _, _, t}, _, _}:
      TI.Tiles.image(t)
    case Kept{_, _, TI.FNone{}, _, _}:
      Pix{0}

def write.k(r: File & Result<&1, &1, U32 & String, Unit>) -> IO(Unit):
  (f, _) = r
  File.close(f)

# the frame last shown as a PPM in dir ("" writes none)
def snap(none: Bool, +dir: String, +name: String, +k: Kept) -> IO(Unit):
  match none:
    case True{}:
      IO.pure(Unit, Unit{})
    case False{}:
      do IO<Unit>:
        g : File <- IO.try(File, File.open(dir ++ "/" ++ name ++ ".ppm", "w"))
        w : File & Result<&1, &1, U32 & String, Unit> <- File.write_bytes(g, D.Snap.ppm(kimg(k), 1400, 900))
        write.k(w)

def main() -> IO(Unit):
  do IO<Unit>:
    args : List<String> <- AV.Argv.args()
    +as : List<&2, String> = copy(args)
    +ty : L.Ty <- L.Ty.load()
    +a : Nat <- IO.now()
    bs : List<&2, U32> <- read(arg(as, 0n, "/tmp/bbench/events.jsonl"))
    +u : U.Ui = real(bs, arg(as, 3n, ""), Bool.pick(String, String.is_empty(arg(as, 4n, "")), remote(), arg(as, 4n, "")))
    +n0 : N.Nui = N.Nui.init(busy(String.eq(arg(as, 5n, ""), "busy"), u), 1400, 900)
    r0 : KT <- frame(ty, 0n, 0, Kept{n0, L.TL.memo.none(), TI.FNone{}, CA.Cats.none(), BV.Bv.wraps.none()}, Times.none(), False{})
    +k0 : Kept <- show(r0, "first frame, a thread")
    +b : Nat <- IO.now()
    IO.print("load and first frame: " ++ Nat.show(Nat.sub(b, a)) ++ " ms")
    +c0 : Nat <- IO.now()
    +ko : Kept = with_n(k0, with_steps(kn(k0), arg(as, 1n, "room=r1790403491-15272"), arg(as, 2n, "room")))
    +c1 : Nat <- IO.now()
    IO.print("the open action: " ++ Nat.show(Nat.sub(c1, c0)) ++ " ms; cats moving: " ++ Bool.show(BU.BU.moving(N.Nui.ui(kn(ko)))))
    r1 : KT <- frame(ty, 0n, 0, ko, Times.none(), False{})
    +k1 : Kept <- show(r1, "open")
    +dir : String = arg(as, 6n, "")
    snap(String.is_empty(dir), dir, "open", k1)
    r2 : KT <- frames(10n, ty, 0n, k1, Times.none(), False{})
    +k2 : Kept <- show(r2, "same frame again")
    r3 : KT <- keys("hello there, a few keys", ty, k2, Times.none())
    +k3 : Kept <- show(r3, "keys")
    r4 : KT <- frames(30n, ty, 1n, k3, Times.none(), False{})
    +k4 : Kept <- show(r4, "cat frames")
    r5 : KT <- frames(10n, ty, 2n, k4, Times.none(), True{})
    +k5 : Kept <- show(r5, "wheel up")
    snap(String.is_empty(dir), dir, "up", k5)
    r6 : KT <- frames(10n, ty, 3n, k5, Times.none(), True{})
    +k6 : Kept <- show(r6, "wheel down")
    snap(String.is_empty(dir), dir, "down", k6)
    parts(ty, k6)
    IO.print("done")
