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
import ../../src/server/argv.bend as AV
import ../../src/core/subs.bend as SB
import ../../src/app/mm.bend as MM
import ../../src/core/emb.bend as EMB

# Typing in the window, timed: each key goes through the same steps as in
# the app (the event, the timeline memo, layout, the tiles redrawn) with
# no window. A made-up log: one long thread and 30 short ones, and four
# bots (their cats move) in two rooms; or a real event log with one of its
# threads open. Also times a caret blink: the same frame, the clock moved
# half a blink on.
#   scripts/build-app.sh test/native/type_bench.bend build/type_bench
#   build/type_bench [EVENTS THREAD [W H [PASSES]]]   ("" "" W H: the made-up log at W x H;
#   the parts' x50 lines stay at 1400 x 900)

def para(+i: Nat) -> String:
  "Message " ++ Nat.show(i) ++ ": the regulator's enable pin is pulled up through R12 to VBAT, so the output comes up before the MCU "
    ++ "drives it low. Moving R12 to the 3V3 rail fixes the sequencing, and C7 can stay where it is. **Next:** check the footprint "
    ++ "of U3 against the datasheet, then rerun DRC.\n\n- item one\n- item two with `code`\n"

def msg(+k: Nat, +th: String, +at: Nat) -> M.Change:
  M.MessagePosted{th, th ++ "m" ++ Nat.show(k), Bool.pick(M.Role, Nat.is_eq(Nat.mod(k, 2n), 0n), M.User{}, M.Assistant{}), para(k), Nat.add(at, k)}

def msgs(+n: Nat, +th: String, +at: Nat) -> List<&2, M.Change>:
  match n:
    case 0n:
      Nil{}
    case 1n+k:
      msg(k, th, at) <> msgs(k, th, at)

def short(+k: Nat, +at: Nat) -> List<&2, M.Change>:
  M.ThreadCreated{"t" ++ Nat.show(1n+k), "p1", "Thread " ++ Nat.show(k), M.Local{}, "", "", "claude", "", at} <> msgs(20n, "t" ++ Nat.show(1n+k), at)

def threads(+n: Nat, +at: Nat) -> List<&2, M.Change>:
  match n:
    case 0n:
      Nil{}
    case 1n+k:
      List.append(&2, M.Change, short(k, at), threads(k, at))

def bot(+i: Nat, +name: String, +at: Nat) -> List<&2, M.Change>:
  [M.ThreadCreated{"bt" ++ Nat.show(i), "", name, M.Local{}, "", "", "claude", "", at}, M.BotSet{"b" ++ Nat.show(i), name, "bt" ++ Nat.show(i), Nat.mul(i, 7n), "A cat", at}]

def bots(+at: Nat) -> List<&2, M.Change>:
  List.concat(&2, M.Change, [bot(0n, "Mochi", at), bot(1n, "Biscuit", at), bot(2n, "Tofu", at), bot(3n, "Pixel", at),
    [M.RoomSet{"r0", "general", "b0\nb1\nb2", at}, M.RoomSet{"r1", "boards", "b1\nb3", at}]])

def log(+at: Nat) -> List<&2, M.Change>:
  List.concat(&2, M.Change, [[M.ProjectCreated{"p1", "board", "/tmp/board", at}, M.ThreadCreated{"t0", "p1", "Power sequencing", M.Local{}, "", "", "claude", "", at}],
    msgs(200n, "t0", at), threads(30n, at), bots(at)])

def ui_sel(a: U.Act) -> U.Ui:
  match a:
    case U.Act{u, _}:
      u

# what one frame leaves: the window state (with its hit regions), the memo
# and the tiles
type Kept is Data:
  Kept{n: N.Nui, memo: L.TLMemo, frames: TI.Frames}

type Times is Data:
  Times{ev: Nat, memo: Nat, lay: Nat, tiles: Nat, worst: Nat, each: String}

def Times.add(t: Times, +a: Nat, +b: Nat, +c: Nat, +d: Nat) -> Times:
  match t:
    case Times{e, m, l, ti, +w, ea}:
      Times{Nat.add(e, a), Nat.add(m, b), Nat.add(l, c), Nat.add(ti, d), Bool.pick(Nat, Nat.is_gt(Nat.add(Nat.add(a, b), Nat.add(c, d)), w), Nat.add(Nat.add(a, b), Nat.add(c, d)), w),
        ea ++ " " ++ Nat.show(Nat.add(Nat.add(a, b), Nat.add(c, d)))}

def Times.total(t: Times, +keys: Nat) -> String:
  match t:
    case Times{+e, +m, +l, +ti, _, _}:
      Nat.show(Nat.div(Nat.add(Nat.add(e, m), Nat.add(l, ti)), keys))

def Times.show(t: Times, +keys: Nat) -> String:
  match t:
    case Times{+e, +m, +l, +ti, w, ea}:
      "per key (ms, mean of " ++ Nat.show(keys) ++ "): event " ++ Nat.show(Nat.div(e, keys)) ++ "." ++ Nat.show(Nat.div(Nat.mod(Nat.mul(e, 10n), Nat.mul(keys, 10n)), keys))
        ++ "  memo " ++ Nat.show(Nat.div(m, keys)) ++ "  layout " ++ Nat.show(Nat.div(l, keys)) ++ "  tiles " ++ Nat.show(Nat.div(ti, keys))
        ++ "  total " ++ Nat.show(Nat.div(Nat.add(Nat.add(e, m), Nat.add(l, ti)), keys)) ++ "  worst " ++ Nat.show(w) ++ "\n each:" ++ ea

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

def prepared(+n: N.Nui, +ms: Nat) -> N.Nui:
  N.Lbx.stamp(N.Blink.stamp(N.Hover.stamp(N.Nui.prepare(n), ms), ms), ms)

def step_nui(s: N.Step) -> N.Nui:
  match s:
    case N.Step{n, _, _}:
      n

def kept_n(k: Kept) -> N.Nui:
  match k:
    case Kept{n, _, _}:
      n

# one key and the frame after it, timed
def key(cs: String, +ty: L.Ty, +cats: Map<&2, CA.CatF>, k: Kept, t: Times) -> IO(Times):
  match cs k:
    case SNil{} _:
      IO.pure(Times, t)
    case SCon{Chr{+c}, rest} Kept{+n0, memo0, fr0}:
      do IO<Times>:
        +t0 : Nat <- IO.now()
        +n1 : N.Nui = prepared(step_nui(N.Nui.event(n0, W.WKey{c, c, 0, True{}, 0})), t0)
        +t1 : Nat <- IO.now()
        +memo : L.TLMemo = L.Lay.memo(ty, n1, memo0)
        +t2 : Nat <- IO.now()
        +o : L.Out = BV.Bv.frame(ty, memo, cats, n1)
        +k2 : Nat = List.length(&2, R.Op, ops(o))
        +t3 : Nat <- IO.now()
        +tiles : TI.Tiles = TI.Frame.par(N.Nui.width(n1), N.Nui.height(n1), ops(o), Pix{T.Th.bg()}, fr0, N.Hits.slide(hits(o), N.Nui.hits(n0)))
        +img : Image = TI.Tiles.image(tiles)
        +t4 : Nat <- IO.now()
        key(rest, ty, cats, Kept{with_hits(n1, hits(o), t4), memo, TI.FSome{N.Nui.width(n1), N.Nui.height(n1), tiles}},
          Times.add(t, Nat.sub(t1, t0), Nat.sub(t2, t1), Nat.sub(t3, t2), Nat.sub(t4, t3)))

# cat frames, timed: the cats' clock moves on, they are posed again, the
# frame is laid out and its tiles drawn again (only the cats' changed)
def tick(+n: Nat, +ty: L.Ty, cats: Map<&2, CA.CatF>, k: Kept, +acc: Nat) -> IO(Nat):
  match n k:
    case 0n _:
      IO.pure(Nat, acc)
    case 1n+j Kept{+n0, +memo, fr0}:
      do IO<Nat>:
        +t0 : Nat <- IO.now()
        +c2 : Map<&2, CA.CatF> = BV.Bv.warm(N.Nui.ui(n0), cats, Nat.mul(1n+j, 33n))
        +o : L.Out = BV.Bv.frame(ty, memo, c2, n0)
        +k2 : Nat = List.length(&2, R.Op, ops(o))
        +tiles : TI.Tiles = TI.Frame.par(N.Nui.width(n0), N.Nui.height(n0), ops(o), Pix{T.Th.bg()}, fr0, 0)
        +img : Image = TI.Tiles.image(tiles)
        +t1 : Nat <- IO.now()
        tick(j, ty, c2, Kept{n0, memo, TI.FSome{N.Nui.width(n0), N.Nui.height(n0), tiles}}, Nat.add(acc, Nat.sub(t1, t0)))

# the first frame (everything drawn), then the keys
def warm(+ty: L.Ty, +cats: Map<&2, CA.CatF>, +n0: N.Nui) -> Kept:
  +n = prepared(n0, 0n)
  +memo = L.Lay.memo(ty, n, L.TL.memo.none())
  +o = BV.Bv.frame(ty, memo, cats, n)
  Kept{with_hits(n, hits(o), 0n), memo, TI.FSome{N.Nui.width(n), N.Nui.height(n), TI.Frame.par(N.Nui.width(n), N.Nui.height(n), ops(o), Pix{T.Th.bg()}, TI.FNone{}, 0)}}

def text() -> String:
  "hello world, this is a test of typing speed in the composer "

def some(f: TI.Frames) -> Bool:
  match f:
    case TI.FNone{}:
      False{}
    case TI.FSome{_, _, _}:
      True{}

def nops(o: L.Out) -> Nat:
  List.length(&2, R.Op, ops(o))

# a part laid out r times (its op count, so each is made)
def rep.side(+r: Nat, +ty: L.Ty, +cats: Map<&2, CA.CatF>, +n: N.Nui) -> Nat:
  match r:
    case 0n:
      0n
    case 1n+q:
      Nat.add(nops(L.Lay.sidebar.x(ty, n, 900, c => y => BV.Bv.side(ty, cats, n, c, y))), rep.side(q, ty, cats, n))

def rep.thread(+r: Nat, +ty: L.Ty, +memo: L.TLMemo, +n: N.Nui) -> Nat:
  match r:
    case 0n:
      0n
    case 1n+q:
      Nat.add(nops(L.Lay.thread(ty, memo, n, 264, 1400)), rep.thread(q, ty, memo, n))

def rep.frame(+r: Nat, +ty: L.Ty, +cats: Map<&2, CA.CatF>, +memo: L.TLMemo, +n: N.Nui) -> Nat:
  match r:
    case 0n:
      0n
    case 1n+q:
      Nat.add(nops(BV.Bv.frame(ty, memo, cats, n)), rep.frame(q, ty, cats, memo, n))

def rep.tiles(+r: Nat, +os: List<&2, R.Op>, +prev: TI.Frames) -> Nat:
  match r:
    case 0n:
      0n
    case 1n+q:
      Nat.add(Bool.pick(Nat, some(TI.FSome{1400, 900, TI.Frame.par(1400, 900, os, Pix{T.Th.bg()}, prev, 0)}), 1n, 0n), rep.tiles(q, os, prev))

def rep.make(+r: Nat, +os: List<&2, R.Op>) -> Nat:
  match r:
    case 0n:
      0n
    case 1n+q:
      Nat.add(Bool.pick(Nat, some(TI.FSome{1400, 900, TI.Frame.make(1400, 900, os, Pix{T.Th.bg()})}), 1n, 0n), rep.make(q, os))

def kept_memo(k: Kept) -> L.TLMemo:
  match k:
    case Kept{_, m, _}:
      m

def kept_frames(k: Kept) -> TI.Frames:
  match k:
    case Kept{_, _, f}:
      f

def frames_image(f: TI.Frames) -> Image:
  match f:
    case TI.FNone{}:
      Pix{0}
    case TI.FSome{_, _, t}:
      TI.Tiles.image(t)

# each part of one frame, 50 times over (ms for all 50)
def parts(+ty: L.Ty, +cats: Map<&2, CA.CatF>, +k: Kept) -> IO(Unit):
  +n = prepared(kept_n(k), 5n)
  +memo = kept_memo(k)
  +os = ops(BV.Bv.frame(ty, memo, cats, n))
  do IO<Unit>:
    IO.print("ops per frame: " ++ Nat.show(List.length(&2, R.Op, os)) ++ " (sidebar " ++ Nat.show(nops(L.Lay.sidebar.x(ty, n, 900, c => y => BV.Bv.side(ty, cats, n, c, y)))) ++ ", thread " ++ Nat.show(nops(L.Lay.thread(ty, memo, n, 264, 1400))) ++ ")")
    +a : Nat <- IO.now()
    +x1 : Nat = rep.side(50n, ty, cats, n)
    +b : Nat <- IO.now()
    +x2 : Nat = rep.thread(50n, ty, memo, n)
    +c : Nat <- IO.now()
    +x3 : Nat = rep.frame(50n, ty, cats, memo, n)
    +d : Nat <- IO.now()
    +x4 : Nat = rep.tiles(50n, os, kept_frames(k))
    +e : Nat <- IO.now()
    +x5 : Nat = rep.make(10n, os)
    +f : Nat <- IO.now()
    IO.print("x50 ms: sidebar " ++ Nat.show(Nat.sub(b, a)) ++ "  thread " ++ Nat.show(Nat.sub(c, b)) ++ "  whole layout " ++ Nat.show(Nat.sub(d, c))
      ++ "  tiles (same frame again) " ++ Nat.show(Nat.sub(e, d)) ++ "   x10 tiles from scratch " ++ Nat.show(Nat.sub(f, e)) ++ "  " ++ Nat.show(Nat.add(Nat.add(x1, x2), Nat.add(x3, Nat.add(x4, x5)))))

# the thread view's parts, each 20 times over (ms for all 20), to find
# what a long thread's layout spends its time on
def nf_of(u: U.Ui) -> Map<&2, String>:
  match u:
    case U.Ui{_, _, _, _, _, _, _, _, _, _, _, _, _, _, nf, _, _}:
      nf

def rep.subs(+r: Nat, +st: M.State, +nf: Map<&2, String>, +sel: String, +now: Nat) -> Nat:
  match r:
    case 0n:
      0n
    case 1n+q:
      Nat.add(List.length(&2, SB.SubRow, SB.Subs.rows(st, nf, sel, now)), rep.subs(q, st, nf, sel, now))

def rep.ec(+r: Nat, +u: U.Ui, +st: M.State, +sel: String) -> Nat:
  match r:
    case 0n:
      0n
    case 1n+q:
      Nat.add(List.length(&2, EMB.EV, U.Ec.pins(U.Ec.of(u, sel), st, sel)), rep.ec(q, u, st, sel))

def rep.tl(+r: Nat, +ty: L.Ty, +memo: L.TLMemo, +st: M.State, +sel: String, +nf: Map<&2, String>) -> Nat:
  match r:
    case 0n:
      0n
    case 1n+q:
      Nat.add(List.length(&2, L.TLine, L.TL.memo.shown.loc(ty, memo, st, sel, 780, nf)), rep.tl(q, ty, memo, st, sel, nf))

def rep.info(+r: Nat, +n: N.Nui) -> Nat:
  match r:
    case 0n:
      0n
    case 1n+q:
      Nat.add(String.length(MM.Mm.info.of(n)), rep.info(q, n))

def rep.key(+r: Nat, +n: N.Nui) -> Nat:
  match r:
    case 0n:
      0n
    case 1n+q:
      Nat.add(String.length(MM.Mm.key(n, MM.TouchK.none(), MM.Mm.rels.of(n))), rep.key(q, n))

def rep.misc(+r: Nat, +st: M.State, +sel: String) -> Nat:
  match r:
    case 0n:
      0n
    case 1n+q:
      Nat.add(Nat.add(List.length(&2, M.Ask, U.Asks.open(st, sel)), String.length(U.Tasks.up(st, sel))), rep.misc(q, st, sel))

def rep.tasks(+r: Nat, +st: M.State, +sel: String) -> Nat:
  match r:
    case 0n:
      0n
    case 1n+q:
      Nat.add(List.length(&2, M.Task, U.Tasks.of(st, sel)), rep.tasks(q, st, sel))

def rep.aggs(+r: Nat, +st: M.State, +sel: String) -> Nat:
  match r:
    case 0n:
      0n
    case 1n+q:
      Nat.add(List.length(&2, SB.SubAgg, SB.Subs.aggs(st, sel)), rep.aggs(q, st, sel))

def rep.agents(+r: Nat, +st: M.State, +nf: Map<&2, String>, +sel: String, +now: Nat) -> Nat:
  match r:
    case 0n:
      0n
    case 1n+q:
      Nat.add(List.length(&2, SB.SubRow, SB.Subs.agents(st, nf, sel, now)), rep.agents(q, st, nf, sel, now))

def rep.wl(+r: Nat, +st: M.State, +sel: String) -> Nat:
  match r:
    case 0n:
      0n
    case 1n+q:
      Nat.add(Nat.add(List.length(&2, SB.SubRow, SB.Subs.wakes(st, sel)), List.length(&2, SB.SubRow, SB.Subs.locks.go(M.Locks.names(st), st, sel))), rep.wl(q, st, sel))

def pieces2(+st: M.State, +nf: Map<&2, String>, +sel: String, +now: Nat) -> IO(Unit):
  do IO<Unit>:
    +a : Nat <- IO.now()
    +x1 : Nat = rep.tasks(20n, st, sel)
    +b : Nat <- IO.now()
    +x2 : Nat = rep.aggs(20n, st, sel)
    +c : Nat <- IO.now()
    +x3 : Nat = rep.agents(20n, st, nf, sel, now)
    +d : Nat <- IO.now()
    +x4 : Nat = rep.wl(20n, st, sel)
    +e : Nat <- IO.now()
    IO.print("x20 ms, subs' parts: tasks " ++ Nat.show(Nat.sub(b, a)) ++ " (" ++ Nat.show(Nat.div(x1, 20n)) ++ ")  aggs " ++ Nat.show(Nat.sub(c, b)) ++ " (" ++ Nat.show(Nat.div(x2, 20n)) ++ ")  agents " ++ Nat.show(Nat.sub(d, c)) ++ "  wakes+locks " ++ Nat.show(Nat.sub(e, d)) ++ " " ++ Nat.show(Nat.add(x3, x4)))

def pieces(+ty: L.Ty, +k: Kept) -> IO(Unit):
  +n = prepared(kept_n(k), 5n)
  +u = N.Nui.ui(n)
  +st = U.Ui.state(u)
  +sel = U.Ui.sel(u)
  +nf = nf_of(u)
  +memo = kept_memo(k)
  do IO<Unit>:
    +a : Nat <- IO.now()
    +x1 : Nat = rep.subs(20n, st, nf, sel, U.Side.now(u))
    +b : Nat <- IO.now()
    +x2 : Nat = rep.ec(20n, u, st, sel)
    +c : Nat <- IO.now()
    +x3 : Nat = rep.tl(20n, ty, memo, st, sel, nf)
    +d : Nat <- IO.now()
    +x4 : Nat = rep.info(20n, n)
    +e : Nat <- IO.now()
    +x5 : Nat = rep.key(20n, n)
    +f : Nat <- IO.now()
    +x6 : Nat = rep.misc(20n, st, sel)
    +g : Nat <- IO.now()
    +x7 : Nat = rep.thread(20n, ty, memo, n)
    +h : Nat <- IO.now()
    IO.print("x20 ms: subs " ++ Nat.show(Nat.sub(b, a)) ++ "  terminals " ++ Nat.show(Nat.sub(c, b)) ++ "  timeline lines " ++ Nat.show(Nat.sub(d, c))
      ++ "  info key part " ++ Nat.show(Nat.sub(e, d)) ++ " (" ++ Nat.show(Nat.div(x4, 20n)) ++ " chars)  main key " ++ Nat.show(Nat.sub(f, e)) ++ "  asks+parent " ++ Nat.show(Nat.sub(g, f))
      ++ "  whole thread " ++ Nat.show(Nat.sub(h, g)) ++ "  " ++ Nat.show(Nat.add(Nat.add(x1, x2), Nat.add(x3, Nat.add(x5, Nat.add(x6, x7))))))
    pieces2(st, nf, sel, U.Side.now(u))

# the first timeline line region of the last frame (its line and y)
def first_line(hs: List<&2, N.Hit>) -> U32:
  match hs:
    case Nil{}:
      0
    case N.Hit{_, +y0, _, _, +a, _} <> t:
      Bool.pick(U32, String.eq(a, "tl-line"), y0, first_line(t))

# a frame for n, its hits kept
def again(+ty: L.Ty, +cats: Map<&2, CA.CatF>, +n0: N.Nui, +memo: L.TLMemo) -> N.Nui:
  +n = prepared(n0, 0n)
  with_hits(n, hits(BV.Bv.frame(ty, memo, cats, n)), 0n)

def ev(+n: N.Nui, e: W.WinEv) -> N.Nui:
  step_nui(N.Nui.event(n, e))

# press on a line, drag three lines down, release: what ctrl+c copies
def select(+ty: L.Ty, +cats: Map<&2, CA.CatF>, +k: Kept) -> String:
  +n = kept_n(k)
  +y = first_line(N.Nui.hits(n))
  +n1 = again(ty, cats, ev(n, W.WButton{460, 300, 1, True{}}), kept_memo(k))
  +n2 = again(ty, cats, ev(n1, W.WMove{800, 380, 0}), kept_memo(k))
  +n3 = again(ty, cats, ev(n2, W.WButton{800, 380, 1, False{}}), kept_memo(k))
  N.Sel.text(n3)

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

def real(bs: List<&2, U32>, +sel: String) -> U.Ui:
  ui_sel(U.Ui.act(U.Ui.recv(U.Ui.init(1790180000n), J.Json.obj([J.KV{"t", J.Str{"log"}}, J.KV{"version", J.Str{"0.1.0"}},
    J.KV{"items", J.Json.arr(items(lines(U8.Lines.feed(U8.Lines.new(), bs))))}])), "select", sel))

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

def arg(+xs: List<&2, String>, +i: Nat) -> String:
  Maybe.default(&2, String, List.head(&2, String, List.drop(&2, String, xs, i)), "")

# the made-up log, or the real one named on the command line
def state(none: Bool, +as: List<&2, String>) -> IO(U.Ui):
  match none:
    case True{}:
      IO.pure(U.Ui, ui_sel(U.Ui.act(U.Ui.fold(log(1790000000n), U.Ui.init(1790000100n)), "select", "t0")))
    case False{}:
      IO.bind(List<&2, U32>, U.Ui, read(arg(as, 0n)), bs => IO.pure(U.Ui, real(bs, arg(as, 1n))))

# caret blinks, timed: the clock moves half a blink on, and the frame is
# made as in the app (prepare, memo, layout, tiles); nothing else changed
def blink(+n: Nat, +ty: L.Ty, +cats: Map<&2, CA.CatF>, k: Kept, +acc: Nat) -> IO(Nat):
  match n k:
    case 0n _:
      IO.pure(Nat, acc)
    case 1n+j Kept{+n0, memo0, fr0}:
      do IO<Nat>:
        +t0 : Nat <- IO.now()
        +n1 : N.Nui = prepared(n0, Nat.mul(1n+j, N.Blink.ms()))
        +memo : L.TLMemo = L.Lay.memo(ty, n1, memo0)
        +o : L.Out = BV.Bv.frame(ty, memo, cats, n1)
        +tiles : TI.Tiles = TI.Frame.par(N.Nui.width(n1), N.Nui.height(n1), ops(o), Pix{T.Th.bg()}, fr0, 0)
        +img : Image = TI.Tiles.image(tiles)
        +t1 : Nat <- IO.now()
        blink(j, ty, cats, Kept{with_hits(n1, hits(o), t1), memo, TI.FSome{N.Nui.width(n1), N.Nui.height(n1), tiles}}, Nat.add(acc, Nat.sub(t1, t0)))

# caret blinks as the window makes them now: the last frame's tiles
# blinked in place (TI.Frames.blink), no layout
def blink.fast(+n: Nat, f: TI.Frames, +acc: Nat) -> IO(Nat):
  match n:
    case 0n:
      IO.pure(Nat, acc)
    case 1n+j:
      do IO<Nat>:
        +t0 : Nat <- IO.now()
        +f2 : TI.Frames = TI.Frames.blink(f, Nat.is_eq(Nat.mod(j, 2n), 1n))
        +img : Image = frames_image(f2)
        +t1 : Nat <- IO.now()
        blink.fast(j, f2, Nat.add(acc, Nat.sub(t1, t0)))

# a blinked frame is the frame laid out and drawn from scratch with the
# caret hidden; and the caret was there to hide
def blink.exact(+ty: L.Ty, +cats: Map<&2, CA.CatF>, +k: Kept) -> String:
  +n = prepared(kept_n(k), N.Blink.ms())
  +o = BV.Bv.frame(ty, L.Lay.memo(ty, n, kept_memo(k)), cats, n)
  +off = TI.Tiles.image(TI.Frame.make(N.Nui.width(n), N.Nui.height(n), ops(o), Pix{T.Th.bg()}))
  +fast = frames_image(TI.Frames.blink(kept_frames(k), False{}))
  +back = frames_image(TI.Frames.blink(TI.Frames.blink(kept_frames(k), False{}), True{}))
  "caret hidden " ++ Bool.pick(String, N.Blink.on(n), "no", "yes") ++ ", blinked = drawn from scratch: " ++ Bool.pick(String, TI.Eq.image(fast, off), "yes", "NO")
    ++ ", differs from shown: " ++ Bool.pick(String, TI.Eq.image(fast, frames_image(kept_frames(k))), "no (no caret on screen)", "yes")
    ++ ", blinked back = shown: " ++ Bool.pick(String, TI.Eq.image(back, frames_image(kept_frames(k))), "yes", "NO")

# the same keys typed again and again in one process, from the same first
# frame: a long-lived window gets slower at the same work as it allocates
# (bendlang/bend#1007), and this shows it (ms per key, every pass)
def passes(+r: Nat, +i: Nat, +ty: L.Ty, +cats: Map<&2, CA.CatF>, +k: Kept, +acc: String) -> IO(String):
  match r:
    case 0n:
      IO.pure(String, acc)
    case 1n+q:
      IO.bind(Times, String, key(text(), ty, cats, k, Times{0n, 0n, 0n, 0n, 0n, ""}),
        t => passes(q, 1n+i, ty, cats, k, acc ++ " " ++ Times.total(t, String.length(text()))))

def main() -> IO(Unit):
  do IO<Unit>:
    args : List<String> <- AV.Argv.args()
    +as : List<&2, String> = copy(args)
    +ty : L.Ty <- L.Ty.load()
    +a : Nat <- IO.now()
    +u : U.Ui <- state(String.is_empty(arg(as, 0n)), as)
    +cats : Map<&2, CA.CatF> = BV.Bv.warm(u, CA.Cats.none(), 0n)
    +k : Kept = warm(ty, cats, N.Nui.init(u, Maybe.default(&2, U32, U32.read(arg(as, 2n)), 1400), Maybe.default(&2, U32, U32.read(arg(as, 3n)), 900)))
    +b : Nat <- IO.now()
    IO.print("state and first frame: " ++ Nat.show(Nat.sub(b, a)) ++ " ms")
    parts(ty, cats, k)
    pieces(ty, k)
    IO.print("selected: [" ++ select(ty, cats, k) ++ "]")
    t : Times <- key(text(), ty, cats, k, Times{0n, 0n, 0n, 0n, 0n, ""})
    IO.print(Times.show(t, String.length(text())))
    +ct : Nat <- tick(30n, ty, cats, k, 0n)
    +reps : Nat = Maybe.default(&2, Nat, Nat.read(arg(as, 4n)), 0n)
    ps : String <- passes(reps, 0n, ty, cats, k, "")
    IO.print("ms per key, each pass of " ++ Nat.show(String.length(text())) ++ " keys:" ++ ps)
    +bt : Nat <- blink(20n, ty, cats, k, 0n)
    IO.print("caret blink frame: " ++ Nat.show(Nat.div(bt, 20n)) ++ "." ++ Nat.show(Nat.mod(Nat.div(Nat.mul(bt, 10n), 20n), 10n)) ++ " ms")
    +bf : Nat <- blink.fast(200n, kept_frames(k), 0n)
    IO.print("caret blink, tiles blinked in place (the window's way): " ++ Nat.show(Nat.div(bf, 200n)) ++ "." ++ Nat.show(Nat.mod(Nat.div(Nat.mul(bf, 100n), 200n), 100n)) ++ " ms")
    IO.print(blink.exact(ty, cats, k))
    IO.print("cat frame (4 cats move; pose, layout, tiles): " ++ Nat.show(Nat.div(ct, 30n)) ++ "." ++ Nat.show(Nat.mod(Nat.div(Nat.mul(ct, 10n), 30n), 10n)) ++ " ms")
