import Base
import ../../src/server/server.bend as S
import ../../src/core/model.bend as M
import ../../src/core/client.bend as U
import ../../src/core/json.bend as J
import ../../src/core/store.bend as ST
import ../../src/app/nui.bend as N
import ../../src/app/layout.bend as L
import ../../src/app/theme.bend as T
import ../../src/gfx/raster.bend as R
import ../../src/gfx/tiles.bend as TI
import ../../src/app/win.bend as W
import ../../src/core/far.bend as FR
import ../../src/core/bot.bend as BT
import ../../src/app/bots.bend as BV
import ../../src/app/cats.bend as CA
import ../../src/server/argv.bend as AV

# Choosing a bot that lives on another machine, timed on a real event log:
# the log's own bots and their threads are made a linked machine's mirror
# (far.bend: its feed, as the owning hub would serve it, folded in under
# link p1), then the row the linked machine's directory reports is chosen
# ("remote", "name@box") and the frame after it drawn. Before, the window
# switched to that machine's hub and fetched its whole log
# (test/native/link_bench.bend: 11-20 s on a 13 MB log).
#   scripts/build-js.sh test/native/far_bench.bend build/far_bench
#   build/far_bench ~/.backplane-bend/events.jsonl   (only reads it)

def ops(o: L.Out) -> List<&2, R.Op>:
  match o:
    case L.Out{os, _}:
      os

def prepared(+n: N.Nui, +ms: Nat) -> N.Nui:
  N.Lbx.stamp(N.Blink.stamp(N.Hover.stamp(N.Nui.prepare(n), ms), ms), ms)

def seen(+n: Nat, +what: String, +a: Nat, +b: Nat) -> IO(Unit):
  IO.print(what ++ ": " ++ Nat.show(Nat.sub(b, a)) ++ " ms (" ++ Nat.show(n) ++ ")")

def count(s: M.State) -> Nat:
  match s:
    case M.State{_, _, es, _, _, _, _, _, _, _, _}:
      List.length(&2, M.Entry, es)

def some(f: TI.Tiles) -> Nat:
  match f:
    case _:
      1n

def path(xs: List<String>) -> String:
  match xs:
    case Nil{}:
      "/dev/null"
    case p <> Nil{}:
      p
    case _ <> t:
      path(t)

def ent_thread(e: M.Entry) -> String:
  match e:
    case M.Msg{_, t, _, _, _}:
      t
    case M.Act{_, t, _, _, _, _}:
      t

def entries(s: M.State) -> List<&2, M.Entry>:
  match s:
    case M.State{_, _, es, _, _, _, _, _, _, _, _}:
      es

# the thread of the first entry not in `not`
def other(es: List<&2, M.Entry>, +not: String) -> String:
  match es:
    case Nil{}:
      ""
    case +e <> t:
      Bool.pick(String, String.eq(ent_thread(e), not), other(t, not), ent_thread(e))

def act_ui(a: U.Act) -> U.Ui:
  match a:
    case U.Act{u, _}:
      u

def hits(o: L.Out) -> List<&2, N.Hit>:
  match o:
    case L.Out{_, hs}:
      hs

def with_hits(n: N.Nui, hs: List<&2, N.Hit>, +ms: Nat) -> N.Nui:
  match n:
    case N.Nui{u, w, h, mx, my, sd, sc, ca, fo, pa, _, _, vw}:
      N.Nui{u, w, h, mx, my, sd, sc, ca, fo, pa, hs, ms, vw}

def step_nui(s: N.Step) -> N.Nui:
  match s:
    case N.Step{n, _, _}:
      n

# what one frame leaves for the next
type Kept is Data:
  Kept{n: N.Nui, memos: List<&2, L.TLMemo>, frames: TI.Frames}

def kept_n(k: Kept) -> N.Nui:
  match k:
    case Kept{n, _, _}:
      n

# Use the real bot window and skip wrapping for a hidden timeline.
def memos(visible: Bool, +ty: L.Ty, +n: N.Nui, old: List<&2, L.TLMemo>) -> List<&2, L.TLMemo>:
  match visible:
    case True{}:
      L.Lay.memos(ty, n, old)
    case False{}:
      old

def frame(+ty: L.Ty, +mm: List<&2, L.TLMemo>, +n: N.Nui) -> L.Out:
  BV.Bv.frame(ty, L.TL.memos.now(mm), BV.Bv.warm(N.Nui.ui(n), CA.Cats.none(), 0n), n)

def warm(+ty: L.Ty, +n0: N.Nui) -> Kept:
  +n = prepared(n0, 0n)
  +mm = memos(U.Ui.timeline(N.Nui.ui(n)), ty, n, Nil{})
  +o = frame(ty, mm, n)
  Kept{with_hits(n, hits(o), 0n), mm, TI.FSome{N.Nui.width(n), N.Nui.height(n), TI.Frame.par(N.Nui.width(n), N.Nui.height(n), ops(o), Pix{T.Th.bg()}, TI.FNone{}, 0)}}

# one event and the frame after it, timed: event, memo, layout, tiles (ms)
def step(+what: String, e: W.WinEv, +ty: L.Ty, k: Kept) -> IO(Kept):
  match k:
    case Kept{n0, memo0, fr0}:
      do IO<Kept>:
        +t0 : Nat <- IO.now()
        +n1 : N.Nui = prepared(step_nui(N.Nui.event(n0, e)), 7n)
        +t1 : Nat <- IO.now()
        +mm : List<&2, L.TLMemo> = memos(U.Ui.timeline(N.Nui.ui(n1)), ty, n1, memo0)
        +t2 : Nat <- IO.now()
        +o : L.Out = frame(ty, mm, n1)
        +k2 : Nat = List.length(&2, R.Op, ops(o))
        +t3 : Nat <- IO.now()
        +tiles : TI.Tiles = TI.Frame.par(N.Nui.width(n1), N.Nui.height(n1), ops(o), Pix{T.Th.bg()}, fr0, 0)
        +img : Image = TI.Tiles.image(tiles)
        +t4 : Nat <- IO.now()
        IO.print(what ++ ": event " ++ Nat.show(Nat.sub(t1, t0)) ++ "  memo " ++ Nat.show(Nat.sub(t2, t1)) ++ "  layout " ++ Nat.show(Nat.sub(t3, t2))
          ++ "  tiles " ++ Nat.show(Nat.sub(t4, t3)) ++ "  = " ++ Nat.show(Nat.sub(t4, t0)) ++ " ms (" ++ Nat.show(k2) ++ " ops)")
        return Kept{with_hits(n1, hits(o), t4), mm, TI.FSome{N.Nui.width(n1), N.Nui.height(n1), tiles}}


def first_far(bs: List<&2, BT.Bot>) -> String:
  match bs:
    case Nil{}:
      ""
    case +b <> t:
      Bool.pick(String, FR.Fr.is(BT.Bot.id(b)), BT.Bot.name(b), first_far(t))

def main() -> IO(Unit):
  do IO<Unit>:
    args : List<String> <- AV.Argv.args()
    +file : String = path(args)
    +bs : List<&2, U32> <- S.Files.read(file)
    +ld : ST.Loaded = ST.Store.load(S.Store.lines(bs), ST.Loaded{M.State.empty(), Nil{}})
    +st : M.State = ST.Loaded.st(ld)
    +ls : List<&2, String> = ST.Loaded.wire(ld)
    +u0 : U.Ui = U.Ui.adopt(U.Ui.recv(U.Ui.init(1790000000n), Maybe.default(&2, J.Json, J.Json.parse("{\"t\":\"log\",\"since\":0,\"items\":[]}"), J.Null{})), M.State.private(st))
    +t0 : Nat <- IO.now()
    +feed : List<&2, FR.Line> = FR.Fr.feed(ls, st)
    +t1 : Nat <- IO.now()
    seen(List.length(&2, FR.Line, feed), "owning hub: its feed from the log (shared lines)", t0, t1)
    +msg : String = FR.Fr.msg("p1", "box", False{}, 0n, List.length(&2, String, ls), FR.Fr.lines.items(feed, Nil{}))
    +u1 : U.Ui = U.Ui.recv(U.Ui.recv(u0, Maybe.default(&2, J.Json, J.Json.parse(msg), J.Null{})),
      Maybe.default(&2, J.Json, J.Json.parse("{\"t\":\"log\",\"since\":1,\"items\":[]}"), J.Null{}))
    +t2 : Nat <- IO.now()
    seen(String.length(msg), "client: the mirror folded in (chars)", t1, t2)
    +name : String = first_far(BT.World.bots(M.State.bots(U.Ui.state(u1))))
    +info : String = "{\"t\":\"info\",\"info\":{\"bots.remote\":" ++ J.Json.show(J.Str{"[{\"name\":\"" ++ name ++ "\",\"peer\":\"p1\",\"peerName\":\"box\",\"look\":1,\"state\":\"ready\"},{\"name\":\"visitor\",\"peer\":\"p1\",\"peerName\":\"box\",\"look\":1,\"state\":\"ready\"}]"}) ++ "}}"
    +u2 : U.Ui = U.Ui.recv(u1, Maybe.default(&2, J.Json, J.Json.parse(info), J.Null{}))
    +ty : L.Ty <- L.Ty.load()
    +k : Kept = warm(ty, N.Nui.init(u2, 1400, 900))
    IO.print("choosing " ++ name ++ "@box:")
    +n0 : N.Nui = kept_n(k)
    +t3 : Nat <- IO.now()
    +u3 : U.Ui = act_ui(U.Ui.act(N.Nui.ui(n0), "remote", name ++ "@box"))
    +t4 : Nat <- IO.now()
    seen(0n, "  the choice (state)", t3, t4)
    IO.print("  selected " ++ U.Ui.scratch(u3, "@bot") ++ " thread " ++ U.Ui.sel(u3) ++ ", entries " ++ Nat.show(List.length(&2, M.Entry, M.State.mine(U.Ui.state(u3), U.Ui.sel(u3)))))
    k2 : Kept <- step("  its first frame", W.WExpose{}, ty, Kept{N.Nui.with_ui(n0, u3), Nil{}, TI.FNone{}})
    k3 : Kept <- step("  the next frame", W.WExpose{}, ty, k2)
    +t5 : Nat <- IO.now()
    seen(0n, "total, choice to second frame", t3, t5)
    +local : String = BT.Bot.id(Maybe.default(&2, BT.Bot, List.head(&2, BT.Bot, BT.World.bots(M.State.bots(st))), BT.Bot.blank("")))
    k4 : Kept <- step("local bot first frame", W.WExpose{}, ty, Kept{N.Nui.with_ui(n0, act_ui(U.Ui.act(u2, "bot", local))), Nil{}, TI.FNone{}})
    k5 : Kept <- step("reported-only bot first frame", W.WExpose{}, ty, Kept{N.Nui.with_ui(n0, act_ui(U.Ui.act(u2, "remote", "visitor@box"))), Nil{}, TI.FNone{}})
    return Unit{}
