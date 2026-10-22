import Base
import ../../src/core/client.bend as U
import ../../src/core/model.bend as M
import ../../src/core/codec.bend as C
import ../../src/core/json.bend as J
import ../../src/app/nui.bend as N
import ../../src/app/layout.bend as L
import ../../src/app/theme.bend as T
import ../../src/gfx/raster.bend as R
import ../../src/gfx/tiles.bend as TI
import ../../src/app/bots.bend as BV
import ../../src/app/cats.bend as CA
import ../../src/server/argv.bend as AV

# Many agents streaming, as the window sees it: 50 running threads, and a
# stream of hub messages dealt round robin over them (text deltas; every
# 20th message per thread a finished message and a tool call), each heard
# the way the app hears it (N.Nui.hear), and after each whether it asks for
# a frame, and the frame (memo, layout, tiles) timed. Once with a quiet
# thread open, once with one of the streaming threads open.
#   scripts/build-js.sh test/native/agents_bench.bend build/agents_bench
#   build/agents_bench [MESSAGES [ENTRIES PER THREAD]]

def para(+i: Nat) -> String:
  "Message " ++ Nat.show(i) ++ ": the regulator's enable pin is pulled up through R12 to VBAT, so the output comes up before the MCU "
    ++ "drives it low. Moving R12 to the 3V3 rail fixes the sequencing, and C7 can stay where it is.\n\n- item one\n- item two with `code`\n"

def msg(+k: Nat, +th: String, +at: Nat) -> M.Change:
  M.MessagePosted{th, th ++ "m" ++ Nat.show(k), Bool.pick(M.Role, Nat.is_eq(Nat.mod(k, 2n), 0n), M.User{}, M.Assistant{}), para(k), Nat.add(at, k)}

def msgs(+n: Nat, +th: String, +at: Nat) -> List<&2, M.Change>:
  match n:
    case 0n:
      Nil{}
    case 1n+k:
      msg(k, th, at) <> msgs(k, th, at)

def tid(+k: Nat) -> String:
  "t" ++ Nat.show(1n+k)

def agent(+k: Nat, +per: Nat, +at: Nat) -> List<&2, M.Change>:
  M.ThreadCreated{tid(k), "p1", "Agent " ++ Nat.show(k), M.Local{}, "", "", "claude", "", at} <>
    List.append(&2, M.Change, msgs(per, tid(k), at), [M.TurnChanged{tid(k), M.Running{}, Nat.add(at, 30n)}])

def agents(+n: Nat, +per: Nat, +at: Nat) -> List<&2, M.Change>:
  match n:
    case 0n:
      Nil{}
    case 1n+k:
      List.append(&2, M.Change, agent(k, per, at), agents(k, per, at))

# the quiet thread first, so its entries are the oldest
def log(+per: Nat, +at: Nat) -> List<&2, M.Change>:
  List.concat(&2, M.Change, [[M.ProjectCreated{"p1", "board", "/tmp/board", at}, M.ThreadCreated{"t0", "p1", "Quiet thread", M.Local{}, "", "", "claude", "", at}],
    msgs(60n, "t0", at), agents(50n, per, at)])

def ui_sel(a: U.Act) -> U.Ui:
  match a:
    case U.Act{u, _}:
      u

def delta(+th: String, +k: Nat) -> J.Json:
  J.Json.obj([J.KV{"t", J.Str{"delta"}}, J.KV{"thread", J.Str{th}}, J.KV{"text", J.Str{"word" ++ Nat.show(k) ++ " of an answer "}}])

def done(+th: String, +k: Nat, +at: Nat) -> J.Json:
  J.Json.obj([J.KV{"t", J.Str{"changes"}}, J.KV{"items", J.Json.arr([
    C.Change.wire(M.MessagePosted{th, "a" ++ Nat.show(k), M.Assistant{}, para(k), at}),
    C.Change.wire(M.ActivityLogged{th, "u" ++ Nat.show(k), "tool", "Bash", "ls -la src/core", at})])}])

# the k-th message of the stream: thread k mod 50, a finished message every
# 20th round
def one(+k: Nat, +at: Nat) -> J.Json:
  Bool.pick(J.Json, Nat.is_eq(Nat.mod(Nat.div(k, 50n), 20n), 19n), done(tid(Nat.mod(k, 50n)), k, at), delta(tid(Nat.mod(k, 50n)), k))

def stream(+n: Nat, +at: Nat, acc: List<&2, J.Json>) -> List<&2, J.Json>:
  match n:
    case 0n:
      acc
    case 1n+k:
      stream(k, at, one(k, at) <> acc)

type Kept is Data:
  Kept{n: N.Nui, memo: L.TLMemo, frames: TI.Frames}

# hear ms, frame ms, frames asked for, the worst frame
type Times is Data:
  Times{hear: Nat, frame: Nat, asked: Nat, worst: Nat}

def Times.add(t: Times, +a: Nat, +b: Nat, +d: Bool) -> Times:
  match t:
    case Times{h, f, n, +w}:
      Times{Nat.add(h, a), Nat.add(f, b), Nat.add(n, Bool.pick(Nat, d, 1n, 0n)), Bool.pick(Nat, Nat.is_gt(b, w), b, w)}

def dec(+x: Nat, +n: Nat) -> String:
  Nat.show(Nat.div(x, n)) ++ "." ++ Nat.show(Nat.div(Nat.mod(Nat.mul(x, 100n), Nat.mul(n, 100n)), n))

def Times.show(t: Times, +n: Nat, +secs: Nat) -> String:
  match t:
    case Times{+h, +f, +a, w}:
      "hear " ++ dec(Nat.mul(h, 100n), Nat.mul(n, 100n)) ++ " ms/msg, frames asked " ++ Nat.show(a) ++ " of " ++ Nat.show(n)
        ++ ", frame " ++ dec(f, Bool.pick(Nat, Nat.is_eq(a, 0n), 1n, a)) ++ " ms mean, worst " ++ Nat.show(w) ++ " ms; all " ++ Nat.show(Nat.add(h, f)) ++ " ms"

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

def warm(+ty: L.Ty, +cats: Map<&2, CA.CatF>, +n0: N.Nui) -> Kept:
  +n = prepared(n0, 0n)
  +memo = L.Lay.memo(ty, n, L.TL.memo.none())
  +o = BV.Bv.frame(ty, memo, cats, n)
  Kept{with_hits(n, hits(o), 0n), memo, TI.FSome{N.Nui.width(n), N.Nui.height(n), TI.Frame.par(N.Nui.width(n), N.Nui.height(n), ops(o), Pix{T.Th.bg()}, TI.FNone{}, 0)}}

# a frame, drawn as the app draws it
def draw(+ty: L.Ty, +cats: Map<&2, CA.CatF>, k: Kept) -> IO(Kept & Nat):
  match k:
    case Kept{+n0, memo0, fr0}:
      do IO<Kept & Nat>:
        +t0 : Nat <- IO.now()
        +n1 : N.Nui = prepared(n0, t0)
        +memo : L.TLMemo = L.Lay.memo(ty, n1, memo0)
        +o : L.Out = BV.Bv.frame(ty, memo, cats, n1)
        +tiles : TI.Tiles = TI.Frame.par(N.Nui.width(n1), N.Nui.height(n1), ops(o), Pix{T.Th.bg()}, fr0, 0)
        +img : Image = TI.Tiles.image(tiles)
        +t1 : Nat <- IO.now()
        return (Kept{with_hits(n1, hits(o), t1), memo, TI.FSome{N.Nui.width(n1), N.Nui.height(n1), tiles}}, Nat.sub(t1, t0))

def step_of(s: N.Step) -> N.Nui & Bool:
  match s:
    case N.Step{n, _, d}:
      (n, d)

def after(d: Bool, +ty: L.Ty, +cats: Map<&2, CA.CatF>, k: Kept) -> IO(Kept & Nat):
  match d:
    case True{}:
      draw(ty, cats, k)
    case False{}:
      IO.pure(Kept & Nat, (k, 0n))

def heard.k(p: Kept & Nat, +dt: Nat, +d: Bool, t: Times, k: Kept -> Times -> IO(Times)) -> IO(Times):
  (kk, +ft) = p
  k(kk, Times.add(t, dt, ft, d))

def heard(r: N.Nui & Bool, +memo: L.TLMemo, fr: TI.Frames, +ty: L.Ty, +cats: Map<&2, CA.CatF>, +dt: Nat, t: Times, k: Kept -> Times -> IO(Times)) -> IO(Times):
  (n, +d) = r
  IO.bind(Kept & Nat, Times, after(d, ty, cats, Kept{n, memo, fr}), p => heard.k(p, dt, d, t, k))

# each message heard, then its frame when it asks for one
def run(ms: List<&2, J.Json>, +ty: L.Ty, +cats: Map<&2, CA.CatF>, k: Kept, t: Times) -> IO(Times):
  match ms k:
    case Nil{} _:
      IO.pure(Times, t)
    case o <> rest Kept{+n0, memo, fr}:
      do IO<Times>:
        +t0 : Nat <- IO.now()
        r : N.Nui & Bool = step_of(N.Nui.hear(n0, o))
        +t1 : Nat <- IO.now()
        heard(r, memo, fr, ty, cats, Nat.sub(t1, t0), t, k2 => t2 => run(rest, ty, cats, k2, t2))

def copy(xs: List<&1, String>) -> List<&2, String>:
  match xs:
    case Nil{}:
      Nil{}
    case x <> t:
      x <> copy(t)

def arg(+xs: List<&2, String>, +i: Nat) -> String:
  Maybe.default(&2, String, List.head(&2, String, List.drop(&2, String, xs, i)), "")

def pass(+name: String, +sel: String, +n: Nat, +ty: L.Ty, +base: U.Ui) -> IO(Unit):
  +u = ui_sel(U.Ui.act(base, "select", sel))
  +cats = BV.Bv.warm(u, CA.Cats.none(), 0n)
  +k = warm(ty, cats, N.Nui.init(u, 1400, 900))
  do IO<Unit>:
    +a : Nat <- IO.now()
    t : Times <- run(stream(n, 1790000200n, Nil{}), ty, cats, k, Times{0n, 0n, 0n, 0n})
    +b : Nat <- IO.now()
    IO.print(name ++ ": " ++ Times.show(t, n, Nat.sub(b, a)))

def main() -> IO(Unit):
  do IO<Unit>:
    args : List<String> <- AV.Argv.args()
    +as : List<&2, String> = copy(args)
    +n : Nat = Maybe.default(&2, Nat, Nat.read(arg(as, 0n)), 1000n)
    +ty : L.Ty <- L.Ty.load()
    +per : Nat = Maybe.default(&2, Nat, Nat.read(arg(as, 1n)), 20n)
    +u : U.Ui = U.Ui.fold(log(per, 1790000000n), U.Ui.init(1790000100n))
    IO.print(Nat.show(n) ++ " hub messages over 50 streaming threads with " ++ Nat.show(per) ++ " entries each, a frame after each that asks")
    pass("quiet thread open    ", "t0", n, ty, u)
    pass("streaming thread open", "t1", n, ty, u)
