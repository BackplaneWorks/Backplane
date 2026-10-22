import Base
import ../../src/core/board.bend as B
import ../../src/core/pcb3d.bend as P
import ../../src/gfx/mesh.bend as M
import ../../src/gfx/face.bend as F
import ../../src/gfx/draw.bend as D
import ../../src/gfx/raster.bend as R
import ../../src/core/glb.bend as G
import ../../src/server/server.bend as S
import ../../src/app/parts3.bend as PT
import ../../src/server/host.bend as H
import ../../src/server/argv.bend as AV

# A board drawn in 3D straight from its .kicad_pcb, its models loaded the
# way the viewer loads them (one after another here), timed.
#   scripts/build-js.sh test/native/pcb3d_frame.bend build/pcb3d_frame
#   build/pcb3d_frame BOARD.kicad_pcb [W H]   # frames to /tmp/pcb3d-N.ppm
#                                             # (and four samples a pixel to /tmp/pcb3d-N-fine.ppm)

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

def isum(img: Image) -> U32:
  match img:
    case Pix{c}:
      c
    case Qua{a, b, c, d}:
      (isum(a) + isum(b) + isum(c) + isum(d) : U32)

def ms(+a: Nat, +b: Nat) -> String:
  Nat.show(Nat.sub(b, a)) ++ " ms"

def cam(+c: M.Cam, +i: Nat) -> M.Cam:
  Bool.pick(M.Cam, Nat.is_eq(i, 1n), M.Cam.orbit(M.Cam.zoom(c, 0.35), 0.6, F32.neg(0.45)),
    Bool.pick(M.Cam, Nat.is_eq(i, 2n), M.Cam.orbit(c, 2.4, F32.neg(1.3)), c))

def fa(f: M.FaceOps) -> List<&2, R.Op>:
  match f:
    case M.FaceOps{a, _}:
      a

def parts(+s: F.Scene3, +v: M.View) -> List<&2, R.Op>:
  match s:
    case F.Scene3{_, [F.Face3{+thick, +lk, +o, t, b}], _, _}:
      fa(F.Face.side(True{}, o, t, b, lk, v, thick, D.Clip.all()))
    case _:
      Nil{}

def frame(+n: Nat, +s: F.Scene3, +c: M.Cam, +w: U32, +h: U32) -> IO(Unit):
  match n:
    case 0n:
      IO.pure(Unit, Unit{})
    case 1n+k:
      do IO<Unit>:
        +t0 : Nat <- IO.now()
        +v : M.View = M.Mesh.view(w, h, 0, 0, w, h, cam(c, k))
        +ops : List<&2, R.Op> = parts(s, v)
        +no : Nat = List.length(&2, R.Op, ops)
        +ta : Nat <- IO.now()
        +fi : Image = R.Frame.render(w, h, ops, 1)
        +fs : U32 = isum(fi)
        +tb : Nat <- IO.now()
        +img : Image = F.Scene3.render(w, h, 0, 0, w, h, s, cam(c, k), 1n, 2105384)
        +px : U32 = isum(img)
        +t1 : Nat <- IO.now()
        +fine : Image = F.Scene3.render.q(True{}, w, h, 0, 0, w, h, s, cam(c, k), 1n, 2105384)
        +fx : U32 = isum(fine)
        +t2 : Nat <- IO.now()
        IO.print("frame " ++ Nat.show(k) ++ ": " ++ ms(t0, t1) ++ " (ops " ++ ms(t0, ta) ++ " n=" ++ Nat.show(no) ++ ", face raster " ++ ms(ta, tb) ++ ", all again " ++ ms(tb, t1) ++ "; fine " ++ ms(t1, t2) ++ ")" ++ U32.show((px + fs + fx : U32)))
        g : File <- IO.try(File, File.open("/tmp/pcb3d-" ++ Nat.show(k) ++ ".ppm", "w"))
        wr : File & Result<&1, &1, U32 & String, Unit> <- File.write_bytes(g, D.Snap.ppm(img, w, h))
        g2 : File <- IO.try(File, File.open("/tmp/pcb3d-" ++ Nat.show(k) ++ "-fine.ppm", "w"))
        wr2 : File & Result<&1, &1, U32 & String, Unit> <- File.write_bytes(g2, D.Snap.ppm(fine, w, h))
        frame(k, s, c, w, h)

def scene(+items: Maybe<&2, List<&2, B.Item>>, +p: Maybe<&2, P.Pcb3>) -> F.Scene3:
  match items p:
    case Some{+its} Some{+pc}:
      F.Scene3.board(F.Face3.of(its, pc), 1n)
    case _ _:
      F.Scene3.none()

def nmodels(+p: Maybe<&2, P.Pcb3>) -> Nat:
  match p:
    case Some{P.Pcb3{_, _, tx, ms}}:
      Nat.add(Nat.mul(List.length(&2, B.Item, tx), 1000n), List.length(&2, P.Model3, ms))
    case None{}:
      0n

def groups(+p: Maybe<&2, P.Pcb3>) -> List<&2, P.Group3>:
  match p:
    case Some{pc}:
      P.Pcb3.groups(pc)
    case None{}:
      Nil{}

# every model file's parts, placed, into the scene
def load(gs: List<&2, P.Group3>, +board: String, +vars: List<&2, P.KV>, +cache: String, +step: String, s: F.Scene3) -> IO(F.Scene3):
  match gs:
    case Nil{}:
      IO.pure(F.Scene3, s)
    case P.Group3{+raw, places} <> t:
      do IO<F.Scene3>:
        +t0 : Nat <- IO.now()
        +src : String <- PT.Parts.file(raw, board, PT.Parts.dir(board), vars, cache)
        +t1 : Nat <- IO.now()
        +glb : String <- PT.Parts.glb(src, cache, step)
        +t2 : Nat <- IO.now()
        bs : List<&2, U32> <- S.Files.read(glb)
        +ts : List<&2, G.Tri3> = P.Place.all(places, G.Mesh.tris(G.Glb.parse(bs)), Nil{})
        +n : Nat = List.length(&2, G.Tri3, ts)
        +t3 : Nat <- IO.now()
        IO.print("  " ++ raw ++ ": find " ++ ms(t0, t1) ++ ", glb " ++ ms(t1, t2) ++ ", read+parse+place " ++ ms(t2, t3) ++ " (" ++ Nat.show(n) ++ ")")
        load(t, board, vars, cache, step, F.Scene3.add(s, ts, 1n))

def go(r: File & Result<&1, &1, U32 & String, List<&2, U32>>, +w: U32, +h: U32, +board: String) -> IO(Unit):
  (f, res) = r
  do IO<Unit>:
    File.close(f)
    +bs : List<&2, U32> <- IO.pass(List<&2, U32>, res)
    +t0 : Nat <- IO.now()
    +items : Maybe<&2, List<&2, B.Item>> = B.Board.items(bs)
    +t1 : Nat <- IO.now()
    +p : Maybe<&2, P.Pcb3> = P.Pcb3.of(bs)
    +t2 : Nat <- IO.now()
    +s0 : F.Scene3 = scene(items, p)
    +c : M.Cam = F.Scene3.fit(s0)
    +t3 : Nat <- IO.now()
    +cache : String <- PT.Parts.cache()
    mk : Result<&1, &1, U32 & String, Unit> <- H.FS.mkdirs(cache)
    +vars : List<&2, P.KV> <- PT.Parts.vars(PT.Parts.dir(board))
    +step : String <- PT.Parts.step2glb()
    +s : F.Scene3 <- load(groups(p), board, vars, cache, step, s0)
    +t4 : Nat <- IO.now()
    IO.print("models " ++ ms(t3, t4) ++ " (" ++ Nat.show(M.Model.size(F.Scene3.model(s))) ++ " tris)")
    IO.print("items " ++ ms(t0, t1) ++ "; pcb3 " ++ ms(t1, t2) ++ " (text*1000+models " ++ Nat.show(nmodels(p)) ++ "); scene " ++ ms(t2, t3)
      ++ " (" ++ Nat.show(M.Model.size(F.Scene3.model(s))) ++ " wall tris)")
    frame(4n, s, c, w, h)

def main() -> IO(Unit):
  do IO<Unit>:
    as : List<&1, String> <- AV.Argv.args()
    +args : List<&2, String> = many(as)
    +path : String = Maybe.default(&2, String, List.head(&2, String, args), "board.kicad_pcb")
    f : File <- IO.try(File, File.open(path, "r"))
    r : File & Result<&1, &1, U32 & String, List<&2, U32>> <- File.read_bytes(f, 400000000)
    go(r, u32(List.get(&2, String, args, 1n), 1600), u32(List.get(&2, String, args, 2n), 1000), path)
