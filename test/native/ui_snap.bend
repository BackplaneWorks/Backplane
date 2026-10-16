import Base
import ../../src/app/bots.bend as BV
import ../../src/app/cats.bend as CA
import ../../src/core/json.bend as J
import ../../src/core/utf8.bend as U8
import ../../src/core/client.bend as U
import ../../src/app/nui.bend as S
import ../../src/app/layout.bend as L
import ../../src/gfx/raster.bend as R
import ../../src/gfx/draw.bend as D
import ../../src/app/theme.bend as T
import ../../src/server/argv.bend as AV

# A frame of the window from an event log and an info message, as a PPM,
# with no window (its timeline and its theme as the window draws them): `ui_snap EVENTS INFO THREAD FLAGS OUT` (the INFO file may hold
# more messages, a line each, received in order: voice messages, say;
# FLAGS is a
# comma-separated list of Ui flags to open, e.g. "models" or "-"; then
# optionally the window's width and height)

def items(ls: List<&2, String>) -> List<&2, J.Json>:
  match ls:
    case Nil{}:
      Nil{}
    case l <> t:
      Maybe.default(&2, J.Json, J.Json.parse(l), J.Null{}) <> items(t)

def lines(r: U8.Lines & List<&2, String>) -> List<&2, String>:
  (_, ls) = r
  ls

def act.k(a: U.Act) -> U.Ui:
  match a:
    case U.Act{u2, _}:
      u2

def flag.k(ps: List<&2, String>, u: U.Ui) -> U.Ui:
  match ps:
    case +a <> +v <> _:
      act.k(U.Ui.act(u, a, v))
    case +f <> Nil{}:
      act.k(U.Ui.act(u, "flag", f))
    case Nil{}:
      u

# "-" is none, "models" a flag, "look:p1" an action with its value
def flag(skip: Bool, u: U.Ui, +f: String) -> U.Ui:
  match skip:
    case True{}:
      u
    case False{}:
      flag.k(String.split(f, ':'), u)

def flags(fs: List<&2, String>, u: U.Ui) -> U.Ui:
  match fs:
    case Nil{}:
      u
    case +f <> t:
      flags(t, flag(String.eq(f, "-"), u, f))

def recv_all(ls: List<&2, String>, +u: U.Ui) -> U.Ui:
  match ls:
    case Nil{}:
      u
    case l <> t:
      recv_all(t, U.Ui.recv(u, Maybe.default(&2, J.Json, J.Json.parse(l), J.Null{})))

def ui_of(bs: List<&2, U32>, +info: String, +sel: String, +fl: String) -> U.Ui:
  flags(String.split(fl, ','), act.k(U.Ui.act(recv_all(String.lines(info), U.Ui.recv(U.Ui.init(U32.to_nat(1790180000)), J.Json.obj([J.KV{"t", J.Str{"log"}}, J.KV{"version", J.Str{"0.1.0"}},
    J.KV{"items", J.Json.arr(items(lines(U8.Lines.feed(U8.Lines.new(), bs))))}]))), "select", sel)))

def ops(o: L.Out) -> List<&2, R.Op>:
  match o:
    case L.Out{os, _}:
      os

def arg(+xs: List<&2, String>, +i: Nat) -> String:
  Maybe.default(&2, String, List.head(&2, String, List.drop(&2, String, xs, i)), "")

def copy(xs: List<&1, String>) -> List<&2, String>:
  match xs:
    case Nil{}:
      Nil{}
    case x <> t:
      x <> copy(t)

def read.k(r: File & Result<&1, &1, U32 & String, List<&2, U32>>) -> IO(List<&2, U32>):
  (f, res) = r
  do IO<List<&2, U32>>:
    File.close(f)
    bs : List<&2, U32> <- IO.pass(List<&2, U32>, res)
    return bs

def read(+path: String) -> IO(List<&2, U32>):
  do IO<List<&2, U32>>:
    f : File <- IO.try(File, File.open(path, "r"))
    r : File & Result<&1, &1, U32 & String, List<&2, U32>> <- File.read_bytes(f, 100000000)
    read.k(r)

def go(+as: List<&2, String>) -> IO(Unit):
  do IO<Unit>:
    bs : List<&2, U32> <- read(arg(as, 0n))
    ib : List<&2, U32> <- read(arg(as, 1n))
    +ty : L.Ty <- L.Ty.load()
    +ww : U32 = Maybe.default(&2, U32, U32.read(arg(as, 5n)), 1400)
    +hh : U32 = Maybe.default(&2, U32, U32.read(arg(as, 6n)), 860)
    +n : S.Nui = S.Nui.init(ui_of(bs, U8.Utf8.decode(ib), arg(as, 2n), arg(as, 3n)), ww, hh)
    +ws : Map<&2, BV.PW> = BV.Bv.wraps(ty, n, BV.Bv.wraps.none())
    +os : List<&2, R.Op> = L.Theme.ops(S.Nui.light(n), ops(BV.Bv.with_cats(BV.Bv.frame.w(ty, L.TL.memos.now(L.Lay.memos(ty, n, Nil{})), CA.Cats.none(), ws, n), CA.Cats.none(), S.Nui.ms_of(n))))
    +img : Image = R.Frame.render(ww, hh, R.Res.ops(os, L.Ty.tab(ty)), L.Theme.ground(S.Nui.look(n)))
    g : File <- IO.try(File, File.open(arg(as, 4n), "w"))
    w : File & Result<&1, &1, U32 & String, Unit> <- File.write_bytes(g, D.Snap.ppm(img, ww, hh))
    IO.print("wrote " ++ arg(as, 4n))

def main() -> IO(Unit):
  do IO<Unit>:
    args : List<String> <- AV.Argv.args()
    go(copy(args))
