import Base
import ../../src/core/client.bend as U
import ../../src/core/model.bend as M
import ../../src/app/nui.bend as N
import ../../src/app/layout.bend as L
import ../../src/app/bots.bend as BV
import ../../src/app/cats.bend as CA
import ../../src/app/theme.bend as T
import ../../src/gfx/raster.bend as R
import ../../src/gfx/draw.bend as D
import ../../src/server/argv.bend as AV

import ../trb_seed.bend as SEED
def ops(o: L.Out) -> List<&2, R.Op>:
  match o:
    case L.Out{os, _}:
      os

def write.k(r: File & Result<&1, &1, U32 & String, Unit>) -> IO(Unit):
  (f, _) = r
  File.close(f)

def write(+path: String, +w: U32, +h: U32, +img: Image) -> IO(Unit):
  do IO<Unit>:
    g : File <- IO.try(File, File.open(path, "w"))
    r : File & Result<&1, &1, U32 & String, Unit> <- File.write_bytes(g, D.Snap.ppm(img, w, h))
    write.k(r)

def frame(+ty: L.Ty, +n: N.Nui) -> L.Out:
  BV.Bv.frame.w(ty, L.Lay.memo(ty, n, L.TL.memo.none()), BV.Bv.warm(N.Nui.ui(n), CA.Cats.none(), 1234n), BV.Bv.wraps(ty, n, BV.Bv.wraps.none()), n)

def shot(+ty: L.Ty, +dir: String, +name: String, +u: U.Ui, +w: U32, +h: U32) -> IO(Unit):
  +n = N.Nui.init(u, w, h)
  +os = ops(frame(ty, n))
  do IO<Unit>:
    write(dir ++ "/win-" ++ name ++ ".ppm", w, h, R.Frame.render(w, h, L.Theme.ops(N.Nui.light(n), os), L.Theme.ground(N.Nui.look(n))))
    IO.print(dir ++ "/win-" ++ name ++ ".ppm")

def arg(xs: List<&2, String>, +i: Nat, d: String) -> String:
  match xs i:
    case Nil{} _:
      d
    case h <> _ 0n:
      h
    case _ <> t 1n+k:
      arg(t, k, d)

def copy(xs: List<&1, String>) -> List<&2, String>:
  match xs:
    case Nil{}:
      Nil{}
    case h <> t:
      String.append(h, "") <> copy(t)

def scroll(+ty: L.Ty, +u: U.Ui) -> U.Ui:
  +n = N.Nui.init(u, 1400, 1000)
  +st = U.Ui.state(u)
  +x = {472 : U32}
  +w = {720 : U32}
  +off = {(128 + L.Set.notify.h(st, ty, w) + 1010 + 26 + 248 + L.Set.voice.h(ty, n, x, w) + L.Set.mates.h(n) + L.Set.keys.h(n) - 76 : U32) : U32}
  U.Ui.scratch_set(u, "@set.sc", U32.show(off))

def main() -> IO(Unit):
  do IO<Unit>:
    args : List<String> <- AV.Argv.args()
    +dir : String = arg(copy(args), 0n, "build/trb/shots")
    +ty : L.Ty <- L.Ty.load()
    +u : U.Ui = SEED.ui("dark")
    shot(ty, dir, "rules-closed", scroll(ty, SEED.settings(u, False{})), 1400, 1000)
    shot(ty, dir, "rules-form", scroll(ty, SEED.settings(u, True{})), 1400, 1000)
    shot(ty, dir, "board", SEED.board(u), 1400, 1000)
    shot(ty, dir, "board-detail", SEED.act(SEED.board(u), "tb-row", "i0"), 1400, 1000)
    shot(ty, dir, "status", SEED.status(u), 1400, 1000)
