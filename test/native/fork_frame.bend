import Base
import ../../src/core/board.bend as B
import ../../src/core/schematic.bend as SC
import ../../src/app/viewer.bend as V
import ../../src/gfx/raster.bend as R
import ../../src/gfx/draw.bend as D
import ../../src/server/argv.bend as AV

def items(m: Maybe<&2, List<&2, B.Item>>) -> List<&2, B.Item>:
  match m:
    case Some{xs}:
      xs
    case None{}:
      Nil{}

def leaves(i: Image) -> Nat:
  match i:
    case Pix{_}:
      1n
    case Qua{a, b, c, d}:
      Nat.add(Nat.add(leaves(a), leaves(b)), Nat.add(leaves(c), leaves(d)))

def ms(+a: Nat, +b: Nat) -> String:
  Nat.show(Nat.sub(b, a)) ++ " ms"

def snap(img: Image, +path: String) -> IO(Unit):
  do IO<Unit>:
    g : File <- IO.try(File, File.open(path, "w"))
    w : File & Result<&1, &1, U32 & String, Unit> <- File.write_bytes(g, D.Snap.ppm(img, 1400, 900))
    return Unit{}

# one base redraw at view v: ops (settled scene, as the window draws its
# base), then raster
def base(+ss: List<&2, V.Seen>, +v: V.View, +flip: U32, +name: String, +path: String) -> IO(Unit):
  do IO<Unit>:
    +t0 : Nat <- IO.now()
    +ops : List<&2, R.Op> = List.reverse(&2, R.Op, V.Ops.scene(ss, v, D.Clip{0, 0, 1400, 900}, 10000n, 10000n, True{}, 0, flip))
    +k : Nat = List.length(&2, R.Op, ops)
    +t1 : Nat <- IO.now()
    +img : Image = R.Frame.render(1400, 900, ops, 526603)
    +lv : Nat = leaves(img)
    +t2 : Nat <- IO.now()
    IO.print(name ++ ": ops " ++ ms(t0, t1) ++ " (" ++ Nat.show(k) ++ "), raster " ++ ms(t1, t2) ++ " (" ++ Nat.show(lv) ++ " leaves)")
    snap(img, path)

def reps(n: Nat, +ss: List<&2, V.Seen>, +v: V.View, +acc: Nat) -> Nat:
  match n:
    case 0n:
      acc
    case 1n+m:
      reps(m, ss, v, Nat.add(acc, List.length(&2, R.Op, V.Ops.scene(ss, v, D.Clip{0, 0, 1400, 900}, 10000n, 10000n, False{}, 0, 0))))

def parse(sch: Bool, bs: List<&2, U32>) -> Maybe<&2, List<&2, B.Item>>:
  match sch:
    case True{}:
      SC.Sch.items(bs)
    case False{}:
      B.Board.items(bs)

def go(+sch: Bool, r: File & Result<&1, &1, U32 & String, List<&2, U32>>) -> IO(Unit):
  (f, res) = r
  do IO<Unit>:
    File.close(f)
    bs : List<&2, U32> <- IO.pass(List<&2, U32>, res)
    +t0 : Nat <- IO.now()
    +ss : List<&2, V.Seen> = V.Scene.update(Nil{}, V.Scene.sort(items(parse(sch, bs))), 0n)
    +n : Nat = List.length(&2, V.Seen, ss)
    +t1 : Nat <- IO.now()
    IO.print("parse+sort+merge " ++ ms(t0, t1) ++ " (" ++ Nat.show(n) ++ " items)")
    +v : V.View = V.View.fit(ss, 0.0, 0.0, 1400.0, 900.0)
    base(ss, v, 0, "fit", "/tmp/board.ppm")
    base(ss, V.View.zoom(v, 8.0, 700.0, 450.0), 0, "zoom 8x", "/tmp/board-zoom.ppm")
    base(ss, v, V.Look.toggle(V.Look.toggle(0, 12), 16), "fit, F.Cu and F.SilkS off", "/tmp/board-hide.ppm")
    +t2 : Nat <- IO.now()
    +ov : Nat = List.length(&2, R.Op, V.Ops.scene(ss, v, D.Clip{0, 0, 1400, 900}, 10000n, 10000n, False{}, 0, 0))
    +t3 : Nat <- IO.now()
    IO.print("overlay of a settled scene (every frame): " ++ ms(t2, t3) ++ " (" ++ Nat.show(ov) ++ " ops)")
    +t4 : Nat <- IO.now()
    +bz : Bool = V.Fade.busy(ss, 10000n)
    +t5 : Nat <- IO.now()
    +ln : Nat = List.length(&2, V.Seen, ss)
    +t6 : Nat <- IO.now()
    +ov2 : Nat = List.length(&2, R.Op, V.Ops.scene(ss, v, D.Clip{0, 0, 1400, 900}, 10000n, 10000n, False{}, 0, 0))
    +t7 : Nat <- IO.now()
    IO.print("Fade.busy walk " ++ ms(t4, t5) ++ " (" ++ Bool.show(bz) ++ "), length walk " ++ ms(t5, t6) ++ " (" ++ Nat.show(ln) ++ "), overlay again " ++ ms(t6, t7) ++ " (" ++ Nat.show(ov2) ++ ")")
    +t8 : Nat <- IO.now()
    +rr : Nat = reps(20n, ss, v, 0n)
    +t9 : Nat <- IO.now()
    IO.print("20 settled overlays " ++ ms(t8, t9) ++ " (" ++ Nat.show(rr) ++ " ops)")
    return Unit{}

def main() -> IO(Unit):
  do IO<Unit>:
    args : List<String> <- AV.Argv.args()
    +path : String = Maybe.default(&1, String, List.head(&1, String, args), "x")
    f : File <- IO.try(File, File.open(path, "r"))
    r : File & Result<&1, &1, U32 & String, List<&2, U32>> <- File.read_bytes(f, 200000000)
    go(String.ends_with(path, ".kicad_sch"), r)
