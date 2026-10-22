import Base
import ../../src/app/mm.bend as MM
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

# The main area's memo checked on a real log: the log less its last K
# events gives the window's state, then each of the K comes as a hub
# message of its own. At each step the main area is laid out from scratch
# and, when the memo's key says nothing it shows changed, compared with the
# memo's: they must be the same, op for op and hit for hit.
#   scripts/build-js.sh test/native/memo_replay.bend build/memo_replay
#   build/memo_replay EVENTS THREAD [K]

def lines(r: U8.Lines & List<&2, String>) -> List<&2, String>:
  (_, ls) = r
  ls

def items(ls: List<&2, String>, acc: List<&2, J.Json>) -> List<&2, J.Json>:
  match ls:
    case Nil{}:
      List.reverse(&2, J.Json, acc)
    case l <> t:
      items(t, Maybe.default(&2, J.Json, J.Json.parse(l), J.Null{}) <> acc)

def real(xs: List<&2, J.Json>, +sel: String) -> U.Ui:
  ui_sel(U.Ui.act(U.Ui.recv(U.Ui.init(1790180000n), J.Json.obj([J.KV{"t", J.Str{"log"}}, J.KV{"version", J.Str{"0.1.0"}},
    J.KV{"items", J.Json.arr(xs)}])), "select", sel))

def msg(x: J.Json) -> J.Json:
  J.Json.obj([J.KV{"t", J.Str{"changes"}}, J.KV{"items", J.Json.arr([x])}])

# each event its own message: the state, the touches, the main area again

# The window's client folding hub changes, timed: the log less its last K
# events gives the state, then each of the K comes as its own "changes"
# message and the state is forced after each (its sequence), as a frame
# would. Prints ms for all K (profile it with callgrind).
#   scripts/build-js.sh test/native/fold_bench.bend build/fold_bench
#   build/fold_bench EVENTS THREAD [K]

def kind(+x: J.Json) -> String:
  J.Json.get_str(x, "$", "") ++ " " ++ String.take(J.Json.get_str(x, "key", ""), 30n)

def slow.k(slow: Bool, +what: String, +ms: Nat) -> IO(Unit):
  match slow:
    case True{}:
      IO.print("  slow " ++ Nat.show(ms) ++ " ms: " ++ what)
    case False{}:
      IO.pure(Unit, Unit{})

def fold(xs: List<&2, J.Json>, u: U.Ui, +acc: Nat) -> IO(Nat):
  match xs:
    case Nil{}:
      IO.pure(Nat, acc)
    case +x <> rest:
      do IO<Nat>:
        +a : Nat <- IO.now()
        +u2 : U.Ui = U.Ui.recv(u, msg(x))
        +s : Nat = M.State.seq(U.Ui.state(u2))
        +b : Nat <- IO.now()
        slow.k(False{}, kind(x), Nat.sub(b, a))
        fold(rest, u2, Nat.add(acc, s))

def batch(xs: List<&2, J.Json>) -> J.Json:
  J.Json.obj([J.KV{"t", J.Str{"changes"}}, J.KV{"items", J.Json.arr(xs)}])

def ixtime(+u: U.Ui, +o: J.Json) -> IO(Unit):
  do IO<Unit>:
    +k0 : MM.IxK = MM.Ix.full(u, 0n)
    +f0 : Nat = MM.IxK.force(k0)
    +u2 : U.Ui = U.Ui.recv(u, o)
    +s2 : Nat = M.State.seq(U.Ui.state(u2))
    +t2 : MM.TouchK = MM.TouchK.msg(o, MM.TouchK.none())
    +n2 : Nat = MM.TouchK.n(t2)
    +a : Nat <- IO.now()
    +k1 : MM.IxK = MM.Ix.up([k0], t2, u2)
    +f1 : Nat = MM.IxK.force(k1)
    +b : Nat <- IO.now()
    +k2 : MM.IxK = MM.Ix.full(u2, 0n)
    +f2 : Nat = MM.IxK.force(k2)
    +c : Nat <- IO.now()
    IO.print("index up to date after one message of those changes: " ++ Nat.show(Nat.sub(b, a)) ++ " ms; made whole: " ++ Nat.show(Nat.sub(c, b)) ++ " ms (" ++ Nat.show(Nat.add(Nat.add(f0, f1), Nat.add(f2, Nat.add(s2, n2)))) ++ ")")

def main() -> IO(Unit):
  do IO<Unit>:
    args : List<String> <- AV.Argv.args()
    +as : List<&2, String> = copy(args)
    bs : List<&2, U32> <- read(arg(as, 0n))
    +xs : List<&2, J.Json> = items(lines(U8.Lines.feed(U8.Lines.new(), bs)), Nil{})
    +k : Nat = Maybe.default(&2, Nat, Nat.read(arg(as, 2n)), 300n)
    +cut : Nat = Nat.sub(List.length(&2, J.Json, xs), k)
    +u : U.Ui = real(List.take(&2, J.Json, xs, cut), arg(as, 1n))
    +s0 : Nat = M.State.seq(U.Ui.state(u))
    +a : Nat <- IO.now()
    ixtime(u, batch(List.drop(&2, J.Json, xs, cut)))
    r : Nat <- fold(List.drop(&2, J.Json, xs, cut), u, 0n)
    +b : Nat <- IO.now()
    IO.print("folded " ++ Nat.show(k) ++ " changes one message each: " ++ Nat.show(Nat.sub(b, a)) ++ " ms (" ++ Nat.show(Nat.add(r, s0)) ++ ")")
