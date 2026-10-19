import Base
import ./check.bend as C
import ./sch_test.bend as T
import ../src/core/board.bend as B
import ../src/core/pcb3d.bend as P
import ../src/core/glb.bend as G
import ../src/core/sexpr.bend as S
import ../src/core/utf8.bend as U
import ../src/app/viewer.bend as V
import ../src/gfx/raster.bend as R
import ../src/gfx/draw.bend as D

def frame(+xs: List<&2, B.Item>) -> Image:
  +ss = V.Scene.update(Nil{}, V.Scene.sort(xs), 0n)
  R.Frame.render(600, 600, List.reverse(&2, R.Op, V.Ops.scene(ss, V.View{8.0, 0.0, 0.0}, D.Clip{0, 0, 600, 600}, 10000n, 10000n, True{}, 0, 0)), 0)

def same(a: Image, b: Image) -> Bool:
  match a b:
    case Pix{x} Pix{y}:
      U32.is_eq(x, y)
    case Qua{a, b, c, d} Qua{e, f, g, h}:
      Bool.and(Bool.and(same(a, e), same(b, f)), Bool.and(same(c, g), same(d, h)))
    case _ _:
      False{}

def signed(+f: F32) -> String:
  Bool.pick(String, F32.is_lt(f, 0.0), "-", "") ++ T.rnd(F32.abs(f))

def point(p: G.V3) -> String:
  match p:
    case G.V3{x, y, z}:
      signed(x) ++ "," ++ signed(y) ++ "," ++ signed(z)

def models(m: Maybe<&2, P.Pcb3>) -> List<&2, P.Model3>:
  match m:
    case Some{P.Pcb3{_, _, _, ms}}:
      ms
    case None{}:
      Nil{}

def model.at(m: Maybe<&2, P.Model3>, +x: F32, +y: F32, +z: F32) -> String:
  match m:
    case Some{P.Model3{_, mat}}:
      point(G.M4.app(mat, x, y, z))
    case None{}:
      "missing"

def model(ms: List<&2, P.Model3>, +i: Nat, +x: F32, +y: F32, +z: F32) -> String:
  model.at(List.get(&2, P.Model3, ms, i), x, y, z)

def check(+fork: List<&2, U32>, +stock: List<&2, U32>) -> IO(Unit):
  +xs = T.items(B.Board.items(fork))
  +img = frame(xs)
  +ms = models(P.Pcb3.of(fork))
  do IO<Unit>:
    C.Check.str("fork front graphic translated/rotated", T.start(T.find(xs, "front/line")), "20,30")
    C.Check.str("fork stock board renders identical", Bool.show(same(img, frame(T.items(B.Board.items(stock))))), "True")
    C.Check.str("fork pad visible at placed origin", Bool.show(Bool.not(U32.is_eq(R.Img.at(10n, img, 160, 224), 0))), "True")
    C.Check.str("fork origin has no collapsed footprint", U32.show(R.Img.at(10n, img, 0, 0)), "0")
    C.Check.str("fork model count", Nat.show(List.length(&2, P.Model3, ms)), "2")
    C.Check.str("fork front model offset", model(ms, 1n, 0.0, 0.0, 0.0), "18,5,29")
    C.Check.str("fork front model scale and rotation", model(ms, 1n, 1.0, 1.0, 1.0), "15,9,27")
    C.Check.str("fork back model offset and flip", model(ms, 0n, 0.0, 0.0, 0.0), "48,-3,41")
    C.Check.str("fork stock model placement", model(models(P.Pcb3.of(stock)), 0n, 1.0, 1.0, 1.0), model(ms, 0n, 1.0, 1.0, 1.0))
    C.Check.str("zero transform takes precedence over stale at", S.Sx.arg(B.Fp.at(S.Sx.parse(U.Utf8.encode("(footprint x (at 99 99 99) (transform (translate 0 0) (rotate 0) (scale 1 1)))"))), 0n), "0")

def close(r: File & Result<&1, &1, U32 & String, List<&2, U32>>) -> IO(List<&2, U32>):
  (f, res) = r
  do IO<List<&2, U32>>:
    File.close(f)
    IO.pass(List<&2, U32>, res)

def read(+p: String) -> IO(List<&2, U32>):
  do IO<List<&2, U32>>:
    f : File <- IO.try(File, File.open(p, "r"))
    r : File & Result<&1, &1, U32 & String, List<&2, U32>> <- File.read_bytes(f, 1000000)
    close(r)

def main() -> IO(Unit):
  do IO<Unit>:
    fork : List<&2, U32> <- read("test/fixtures/fork-viewers/transforms.kicad_pcb")
    stock : List<&2, U32> <- read("test/fixtures/fork-viewers/legacy.kicad_pcb")
    check(fork, stock)
