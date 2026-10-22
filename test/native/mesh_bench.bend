import Base
import ../../src/core/glb.bend as G
import ../../src/gfx/mesh.bend as M
import ../../src/gfx/draw.bend as D
import ../../src/server/argv.bend as AV

# The 3D viewer's frame time at a window's size, over a few orbits.
#   scripts/build-js.sh test/native/mesh_bench.bend build/mesh_bench
#   build/mesh_bench MODEL.glb [W H]   # frames to /tmp/mesh-N.ppm

def many(xs: List<&1, String>) -> List<&2, String>:
  match xs:
    case Nil{}:
      Nil{}
    case x <> t:
      x <> many(t)

def u32(m: Maybe<&2, String>, d: U32) -> U32:
  match m:
    case Some{s}:
      Maybe.default(&2, U32, U32.read(s), d)
    case None{}:
      d

# a checksum that reads every pixel, so the frame is surely drawn
def isum(img: Image) -> U32:
  match img:
    case Pix{c}:
      c
    case Qua{a, b, c, d}:
      (isum(a) + isum(b) + isum(c) + isum(d) : U32)

def ms(+a: Nat, +b: Nat) -> String:
  Nat.show(Nat.sub(b, a)) ++ " ms"

def frame(+n: Nat, +m: M.Model, +c: M.Cam, +w: U32, +h: U32) -> IO(Unit):
  match n:
    case 0n:
      IO.pure(Unit, Unit{})
    case 1n+k:
      do IO<Unit>:
        +t0 : Nat <- IO.now()
        +img : Image = M.Mesh.render(w, h, m, c, 2105384)
        +px : U32 = isum(img)
        +t1 : Nat <- IO.now()
        IO.print("frame " ++ Nat.show(k) ++ ": " ++ ms(t0, t1) ++ " (" ++ U32.show(px) ++ ")")
        g : File <- IO.try(File, File.open("/tmp/mesh-" ++ Nat.show(k) ++ ".ppm", "w"))
        wr : File & Result<&1, &1, U32 & String, Unit> <- File.write_bytes(g, D.Snap.ppm(img, w, h))
        frame(k, m, M.Cam.orbit(M.Cam.zoom(c, 0.8), 0.9, F32.neg(0.25)), w, h)

def go(r: File & Result<&1, &1, U32 & String, List<&2, U32>>, +w: U32, +h: U32) -> IO(Unit):
  (f, res) = r
  do IO<Unit>:
    File.close(f)
    bs : List<&2, U32> <- IO.pass(List<&2, U32>, res)
    +t0 : Nat <- IO.now()
    +ts : List<&2, G.Tri3> = G.Mesh.tris(G.Glb.parse(bs))
    +n : Nat = List.length(&2, G.Tri3, ts)
    +t1 : Nat <- IO.now()
    +m : M.Model = M.Model.of(ts)
    +c : M.Cam = M.Model.fit(m)
    +t2 : Nat <- IO.now()
    IO.print("parse " ++ ms(t0, t1) ++ " (" ++ Nat.show(n) ++ " tris); model " ++ ms(t1, t2))
    frame(4n, m, c, w, h)

def main() -> IO(Unit):
  do IO<Unit>:
    as : List<&1, String> <- AV.Argv.args()
    +args : List<&2, String> = many(as)
    +path : String = Maybe.default(&2, String, List.head(&2, String, args), "/tmp/glb.glb")
    f : File <- IO.try(File, File.open(path, "r"))
    r : File & Result<&1, &1, U32 & String, List<&2, U32>> <- File.read_bytes(f, 400000000)
    go(r, u32(List.get(&2, String, args, 1n), 1600), u32(List.get(&2, String, args, 2n), 1000))
