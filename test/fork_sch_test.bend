import Base
import ./check.bend as C
import ./sch_test.bend as T
import ../src/core/board.bend as B
import ../src/core/schematic.bend as SC
import ../src/core/sexpr.bend as S
import ../src/core/utf8.bend as U
import ../src/app/viewer.bend as V
import ../src/gfx/raster.bend as R
import ../src/gfx/draw.bend as D

def frame(+xs: List<&2, B.Item>) -> Image:
  +ss = V.Scene.update(Nil{}, V.Scene.sort(xs), 0n)
  R.Frame.render(600, 600, List.reverse(&2, R.Op, V.Ops.scene(ss, V.View{10.0, 0.0, 0.0}, D.Clip{0, 0, 600, 600}, 10000n, 10000n, True{}, 1, 0)), 0)

# This is the actual 10.99 cache format: two instances share a source
# lib_id, but have distinct cached drawings selected by lib_name.
def check(bs: List<&2, U32>) -> IO(Unit):
  +xs = T.items(SC.Sch.items(bs))
  +img = frame(xs)
  do IO<Unit>:
    C.Check.str("fork rendered body", Bool.show(Bool.not(U32.is_eq(R.Img.at(10n, img, 100, 190), 0))), "True")
    C.Check.str("fork rendered pin", Bool.show(Bool.not(U32.is_eq(R.Img.at(10n, img, 75, 200), 0))), "True")
    C.Check.str("fork aliased first pin", T.start(T.find(xs, "00000000-0000-4000-8000-000000000011/pin/1")), "7,20")
    C.Check.str("fork aliased second pin", T.start(T.find(xs, "00000000-0000-4000-8000-000000000012/pin/2")), "40,45")
    C.Check.str("fork pin metadata", T.info(T.find(xs, "00000000-0000-4000-8000-000000000012/pin/2")), "pin 2;name=B;type=passive;symbol=R2")
    C.Check.str("fork cached body", T.layer(T.find(xs, "00000000-0000-4000-8000-000000000011/g1001")), U32.show(SC.L.body()))
    C.Check.str("fork source library metadata preserved", Bool.show(String.contains(T.info(T.find(xs, "00000000-0000-4000-8000-000000000011/g1001")), ";lib=Device:R_Small")), "True")
    C.Check.str("stock cache identity", SC.Sch.cache.id(S.Sx.parse(U.Utf8.encode("(symbol (lib_id \"Device:R\"))"))), "Device:R")
    C.Check.str("fork cache identity is authoritative", SC.Sch.cache.id(S.Sx.parse(U.Utf8.encode("(symbol (lib_name \"local\") (lib_id \"Device:R\"))"))), "local")

def read(r: File & Result<&1, &1, U32 & String, List<&2, U32>>) -> IO(Unit):
  (f, res) = r
  do IO<Unit>:
    File.close(f)
    bs : List<&2, U32> <- IO.pass(List<&2, U32>, res)
    check(bs)

def main() -> IO(Unit):
  do IO<Unit>:
    f : File <- IO.try(File, File.open("test/fixtures/fork-viewers/aliases.kicad_sch", "r"))
    r : File & Result<&1, &1, U32 & String, List<&2, U32>> <- File.read_bytes(f, 1000000)
    read(r)
