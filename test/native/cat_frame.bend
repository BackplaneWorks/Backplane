import Base
import ../../src/core/cat.bend as C
import ../../src/gfx/cat.bend as G
import ../../src/gfx/draw.bend as D
import ../../src/gfx/raster.bend as R
import ../../src/server/argv.bend as AV

# Every mood for a few looks, at 96 and 32 pixels, posed at a time, into a
# PPM (default /tmp/cat_frame.ppm). Also times one cat's frame work.
#   scripts/build-js.sh test/native/cat_frame.bend build/cat_frame
#   build/cat_frame [ms] [out.ppm]
#   scripts/ppm-to-png.py /tmp/cat_frame.ppm /tmp/cat_frame.png

def row(ms: List<&2, String>, +seed: Nat, +t: Nat, +x: U32, +y: U32, +px: U32, +step: U32) -> List<&2, R.Op>:
  match ms:
    case Nil{}:
      Nil{}
    case +m <> rest:
      List.append(&2, R.Op, G.Cat.ops(C.Cat.pose(seed, m, t), D.Clip.all(), U32.to_f32(x), U32.to_f32(y), U32.to_f32(px)),
        row(rest, seed, t, (x + step : U32), y, px, step))

def rows(ss: List<&2, Nat>, +t: Nat, +y: U32, +px: U32, +step: U32) -> List<&2, R.Op>:
  match ss:
    case Nil{}:
      Nil{}
    case +s <> rest:
      List.append(&2, R.Op, row(C.Cat.moods(), s, t, 8, y, px, step), rows(rest, t, (y + step : U32), px, step))

def seeds() -> List<&2, Nat>:
  [0n, 13n, 6n, 19n, 32n, 45n]

def ops(+t: Nat) -> List<&2, R.Op>:
  List.append(&2, R.Op, rows(seeds(), t, 8, 96, 104), rows(seeds(), t, 640, 32, 40))

def width() -> U32:
  840

def height() -> U32:
  888

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

# one small cat's frame, n times: pose, ops, render
def bench(+n: Nat, +fs: List<&2, C.FPart>, +px: U32, +acc: U32) -> U32:
  match n:
    case 0n:
      acc
    case 1n+k:
      bench(k, fs, px, U32.add(acc, R.Four.top(R.Frame.render(px, px, G.Cat.ops(C.Cat.pose.at(fs, Nat.mul(k, 33n)), D.Clip.all(), 0.0, 0.0, U32.to_f32(px)), 1118481))))

def main() -> IO(Unit):
  do IO<Unit>:
    a : List<String> <- AV.Argv.args()
    +args : List<&2, String> = copy(a)
    +t : Nat = Maybe.default(&2, Nat, Nat.read(arg(args, 0n, "0")), 0n)
    t0 : Nat <- IO.now()
    +img : Image = R.Frame.render(width(), height(), ops(t), 1118481)
    t1 : Nat <- IO.now()
    f : File <- IO.try(File, File.open(arg(args, 1n, "/tmp/cat_frame.ppm"), "w"))
    r : File & Result<&1, &1, U32 & String, Unit> <- File.write_bytes(f, D.Snap.ppm(img, width(), height()))
    IO.print("96 cats rendered in " ++ Nat.show(Nat.sub(t1, t0)) ++ " ms")
    +fs : List<&2, C.FPart> = C.Cat.flat(0n, "idle")
    t2 : Nat <- IO.now()
    +b : U32 = bench(100n, fs, 32, 0)
    t3 : Nat <- IO.now()
    IO.print("one 32 px idle cat: " ++ Nat.show(Nat.div(Nat.mul(Nat.sub(t3, t2), 10n), 100n)) ++ " tenths of a ms per frame (" ++ U32.show(U32.and(b, 1)) ++ ")")
    +fs2 : List<&2, C.FPart> = C.Cat.flat(0n, "work")
    t4 : Nat <- IO.now()
    +b2 : U32 = bench(100n, fs2, 96, 0)
    t5 : Nat <- IO.now()
    IO.print("one 96 px work cat: " ++ Nat.show(Nat.div(Nat.mul(Nat.sub(t5, t4), 10n), 100n)) ++ " tenths of a ms per frame (" ++ U32.show(U32.and(b2, 1)) ++ ")")
