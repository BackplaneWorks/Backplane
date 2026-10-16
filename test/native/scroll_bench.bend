import Base
import ../../src/core/client.bend as U
import ../../src/core/model.bend as M
import ../../src/app/nui.bend as N
import ../../src/app/win.bend as W
import ../../src/app/layout.bend as L
import ../../src/app/theme.bend as T
import ../../src/gfx/raster.bend as R
import ../../src/gfx/tiles.bend as TI
import ../../src/server/server.bend as S
import ../../src/core/store.bend as ST
import ../../src/core/json.bend as J
import ../../src/server/argv.bend as AV

# Scrolling the timeline, timed: each wheel notch goes through the same
# steps as in the app (the event, the timeline memo, layout, the tiles
# slid or redrawn) with no window, 30 notches up then 30 down, then the
# same in the sidebar, each frame
# checked against the frame drawn from scratch. The log is type_bench's.
#   scripts/build-app.sh test/native/scroll_bench.bend build/scroll_bench
#   build/scroll_bench
#   build/scroll_bench EVENTS.jsonl   (the sidebar of a real log; only reads it)

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

def log(+at: Nat) -> List<&2, M.Change>:
  List.concat(&2, M.Change, [[M.ProjectCreated{"p1", "board", "/tmp/board", at}, M.ThreadCreated{"t0", "p1", "Power sequencing", M.Local{}, "", "", "claude", "", at}],
    msgs(200n, "t0", at), threads(30n, at)])

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

def Times.show(t: Times, +keys: Nat) -> String:
  match t:
    case Times{+e, +m, +l, +ti, w, ea}:
      "per step (ms, mean of " ++ Nat.show(keys) ++ "): event " ++ Nat.show(Nat.div(e, keys)) ++ "." ++ Nat.show(Nat.div(Nat.mod(Nat.mul(e, 10n), Nat.mul(keys, 10n)), keys))
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

def same(+ty: L.Ty, +w: U32, +h: U32, +os: List<&2, R.Op>, +tiles: TI.Tiles) -> Bool:
  TI.Eq.image(TI.Tiles.image(tiles), TI.Tiles.image(TI.Frame.make(w, h, os, Pix{T.Th.bg()}, L.Ty.tab(ty))))

# one wheel notch (dir 0 up, 1 down) and the frame after it, timed; ok
# stays true while every frame is exactly the one drawn from scratch
def notch(n: Nat, +x: U32, +dir: U32, +ty: L.Ty, k: Kept, t: Times, ok: Bool, sh: String) -> IO(Times & Kept):
  match n k:
    case 0n _:
      do IO<Times & Kept>:
        IO.print("exact: " ++ Bool.show(ok) ++ "  shifts:" ++ sh)
        return (t, k)
    case 1n+rest Kept{+n0, memo0, fr0}:
      do IO<Times & Kept>:
        +t0 : Nat <- IO.now()
        +n1 : N.Nui = prepared(step_nui(N.Nui.event(n0, W.WWheel{x, 400, dir})), t0)
        +t1 : Nat <- IO.now()
        +memo : L.TLMemo = L.Lay.memo(ty, n1, memo0)
        +t2 : Nat <- IO.now()
        +o : L.Out = L.Lay.frame(ty, memo, n1)
        +k2 : Nat = List.length(&2, R.Op, ops(o))
        +t3 : Nat <- IO.now()
        +dy : U32 = N.Hits.slide(hits(o), N.Nui.hits(n0))
        +tiles : TI.Tiles = TI.Frame.par(N.Nui.width(n1), N.Nui.height(n1), ops(o), Pix{T.Th.bg()}, L.Ty.tab(ty), fr0, dy)
        +img : Image = TI.Tiles.image(tiles)
        +t4 : Nat <- IO.now()
        notch(rest, x, dir, ty, Kept{with_hits(n1, hits(o), t4), memo, TI.FSome{N.Nui.width(n1), N.Nui.height(n1), L.Ty.tab(ty), tiles}},
          Times.add(t, Nat.sub(t1, t0), Nat.sub(t2, t1), Nat.sub(t3, t2), Nat.sub(t4, t3)),
          Bool.and(ok, same(ty, N.Nui.width(n1), N.Nui.height(n1), ops(o), tiles)), sh ++ " " ++ U32.show(dy))

# the first frame (everything drawn), then the keys
def warm(+ty: L.Ty, +n0: N.Nui) -> Kept:
  +n = prepared(n0, 0n)
  +memo = L.Lay.memo(ty, n, L.TL.memo.none())
  +o = L.Lay.frame(ty, memo, n)
  Kept{with_hits(n, hits(o), 0n), memo, TI.FSome{N.Nui.width(n), N.Nui.height(n), L.Ty.tab(ty), TI.Frame.par(N.Nui.width(n), N.Nui.height(n), ops(o), Pix{T.Th.bg()}, L.Ty.tab(ty), TI.FNone{}, 0)}}

def report(r: Times & Kept, +label: String) -> IO(Kept):
  (t, k) = r
  do IO<Kept>:
    IO.print(label ++ Times.show(t, 30n))
    return k

# the last argument, when it names a .jsonl log
def path(xs: List<String>) -> String:
  match xs:
    case Nil{}:
      ""
    case +p <> Nil{}:
      Bool.pick(String, String.ends_with(p, ".jsonl"), p, "")
    case _ <> t:
      path(t)

# a real log as the window gets it (boot_bench's steps)
def real(+file: String) -> IO(U.Ui):
  do IO<U.Ui>:
    +bs : List<&2, U32> <- S.Files.read(file)
    +ld : ST.Loaded = ST.Store.load(S.Store.lines(bs), ST.Loaded{M.State.empty(), Nil{}})
    +msg : String = "{\"t\":\"log\",\"since\":0,\"origin\":\"x\",\"version\":\"0\",\"info\":{},\"items\":[" ++ String.join(List.reverse(&2, String, ST.Loaded.wire(ld)), ",") ++ "]}"
    +now : Nat <- IO.now()
    return U.Ui.recv(U.Ui.init(Nat.div(now, 1000n)), Maybe.default(&2, J.Json, J.Json.parse(msg), J.Null{}))

def sidebar(+ty: L.Ty, +u: U.Ui) -> IO(Unit):
  do IO<Unit>:
    +k : Kept = warm(ty, N.Nui.init(u, 1400, 900))
    r3 : Times & Kept <- notch(30n, 100, 1, ty, k, Times{0n, 0n, 0n, 0n, 0n, ""}, True{}, "")
    k3 : Kept <- report(r3, "side down ")
    r4 : Times & Kept <- notch(30n, 100, 0, ty, k3, Times{0n, 0n, 0n, 0n, 0n, ""}, True{}, "")
    k4 : Kept <- report(r4, "side up   ")
    return Unit{}

def synthetic(+ty: L.Ty) -> IO(Unit):
  do IO<Unit>:
    +u : U.Ui = ui_sel(U.Ui.act(U.Ui.fold(log(1790000000n), U.Ui.init(1790000100n)), "select", "t0"))
    +k : Kept = warm(ty, N.Nui.init(u, 1400, 900))
    r1 : Times & Kept <- notch(30n, 800, 0, ty, k, Times{0n, 0n, 0n, 0n, 0n, ""}, True{}, "")
    k1 : Kept <- report(r1, "up   ")
    r2 : Times & Kept <- notch(30n, 800, 1, ty, k1, Times{0n, 0n, 0n, 0n, 0n, ""}, True{}, "")
    k2 : Kept <- report(r2, "down ")
    r3 : Times & Kept <- notch(30n, 100, 1, ty, k2, Times{0n, 0n, 0n, 0n, 0n, ""}, True{}, "")
    k3 : Kept <- report(r3, "side down ")
    r4 : Times & Kept <- notch(30n, 100, 0, ty, k3, Times{0n, 0n, 0n, 0n, 0n, ""}, True{}, "")
    k4 : Kept <- report(r4, "side up   ")
    return Unit{}

def main.k(none: Bool, +file: String, +ty: L.Ty) -> IO(Unit):
  match none:
    case True{}:
      synthetic(ty)
    case False{}:
      IO.bind(U.Ui, Unit, real(file), u => sidebar(ty, u))

def main() -> IO(Unit):
  do IO<Unit>:
    args : List<String> <- AV.Argv.args()
    +file : String = path(args)
    +ty : L.Ty <- L.Ty.load()
    main.k(String.is_empty(file), file, ty)
