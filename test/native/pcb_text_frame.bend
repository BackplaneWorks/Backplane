import Base
import ../../src/core/board.bend as B
import ../../src/core/pcb2d.bend as P
import ../../src/app/viewer.bend as V
import ../../src/gfx/raster.bend as R
import ../../src/gfx/draw.bend as D
import ../../src/server/argv.bend as AV

def frame(xs: List<&2, B.Item>) -> Image:
  +seen = V.Scene.update(Nil{}, V.Scene.sort(xs), 0n)
  R.Frame.render(1000, 700, List.reverse(&2, R.Op, V.Ops.scene(seen,
    V.View.fit(seen, 0.0, 0.0, 1000.0, 700.0), D.Clip{0, 0, 1000, 700}, 10000n, 10000n, True{}, 0, 0)), 526603)

def written(result: File & Result<&1, &1, U32 & String, Unit>) -> IO(Unit):
  (file, status) = result
  do IO<Unit>:
    File.close(file)
    IO.pass(Unit, status)

def save(m: Maybe<&2, List<&2, B.Item>>, +path: String) -> IO(Unit):
  match m:
    case Some{xs}:
      do IO<Unit>:
        file : File <- IO.try(File, File.open(path, "w"))
        result : File & Result<&1, &1, U32 & String, Unit> <- File.write_bytes(file, D.Snap.ppm(frame(xs), 1000, 700))
        written(result)
    case None{}:
      IO.die(Unit, 2, "incomplete PCB")

def read(result: File & Result<&1, &1, U32 & String, List<&2, U32>>, +out: String) -> IO(Unit):
  (file, data) = result
  do IO<Unit>:
    File.close(file)
    bytes : List<&2, U32> <- IO.pass(List<&2, U32>, data)
    save(P.Pcb2.items(bytes), out)

def copy(args: List<String>) -> List<&2, String>:
  match args:
    case Nil{}:
      Nil{}
    case h <> t:
      h <> copy(t)

def main() -> IO(Unit):
  do IO<Unit>:
    args : List<String> <- AV.Argv.args()
    +paths : List<&2, String> = copy(args)
    +input : String = Maybe.default(&2, String, List.head(&2, String, paths), "missing.kicad_pcb")
    +output : String = Maybe.default(&2, String, List.get(&2, String, paths, 1n), "pcb-text.ppm")
    file : File <- IO.try(File, File.open(input, "r"))
    result : File & Result<&1, &1, U32 & String, List<&2, U32>> <- File.read_bytes(file, 200000000)
    read(result, output)
