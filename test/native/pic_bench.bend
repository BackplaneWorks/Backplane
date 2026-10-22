import Base
import ../../src/app/imgfx.bend as IF
import ../../src/gfx/bitmap.bend as BM
import ../../src/gfx/raster.bend as R
import ../../src/server/argv.bend as AV

# A browser frame into the window, timed: the old way (the file's pixels
# as a list of bytes, Bitmap.image walking it) and the new (Imgfx.tree:
# decoded off the loop, the quadtree built in C), each REPS times, then
# the bitmap placed in a 1400x900 frame. Prints ms per frame and checks
# both trees draw the same frame.
#   scripts/build-js.sh test/native/pic_bench.bend build/pic_bench
#   build/pic_bench FRAME.jpg [REPS]

def arg(xs: List<&2, String>, +i: Nat, d: String) -> String:
  match xs i:
    case Nil{} _:
      d
    case x <> _ 0n:
      x
    case _ <> t 1n+k:
      arg(t, k, d)

def num(s: String, n: Nat) -> Nat:
  match s:
    case SNil{}:
      n
    case SCon{Chr{c}, t}:
      num(t, Nat.add(Nat.mul(n, 10n), Nat.sub(U32.to_nat(c), 48n)))

def size(b: BM.Bitmap) -> String:
  match b:
    case BM.Bitmap{w, h, _, _, _, _, _, _}:
      U32.show(w) ++ "x" ++ U32.show(h)

def old.b(got: U32 & U32 & List<&2, U32>) -> BM.Bitmap:
  (+w, rest) = got
  (+h, rgb) = rest
  BM.Bitmap.image(w, h, rgb, 0, 0, w, h)

def old.k(r: Result<&1, &1, U32 & String, U32 & U32 & List<&2, U32>>) -> BM.Bitmap:
  match r:
    case Done{got}:
      old.b(got)
    case Fail{_}:
      BM.Bitmap{0, 0, 0n, Pix{0}, 0, 0, 0, 0}

def new.b(got: U32 & U32 & Image) -> BM.Bitmap:
  (+w, rest) = got
  (+h, t) = rest
  BM.Bitmap{w, h, R.Frame.depth(w, h), t, 0, 0, w, h}

def new.k(r: Result<&1, &1, U32 & String, U32 & U32 & Image>) -> BM.Bitmap:
  match r:
    case Done{got}:
      new.b(got)
    case Fail{_}:
      BM.Bitmap{0, 0, 0n, Pix{0}, 0, 0, 0, 0}

def leaves(i: Image) -> Nat:
  match i:
    case Pix{_}:
      1n
    case Qua{a, b, c, d}:
      Nat.add(Nat.add(leaves(a), leaves(b)), Nat.add(leaves(c), leaves(d)))

def same(a: Image, b: Image) -> Bool:
  match a b:
    case Pix{x} Pix{y}:
      U32.is_eq(x, y)
    case Qua{a1, b1, c1, d1} Qua{a2, b2, c2, d2}:
      Bool.and(Bool.and(same(a1, a2), same(b1, b2)), Bool.and(same(c1, c2), same(d1, d2)))
    case _ _:
      False{}

def tree(b: BM.Bitmap) -> Image:
  match b:
    case BM.Bitmap{_, _, _, t, _, _, _, _}:
      t

def olds(+n: Nat, +file: String) -> IO(BM.Bitmap):
  match n:
    case 0n:
      IO.pure(BM.Bitmap, BM.Bitmap{0, 0, 0n, Pix{0}, 0, 0, 0, 0})
    case 1n+k:
      do IO<BM.Bitmap>:
        r : Result<&1, &1, U32 & String, U32 & U32 & List<&2, U32>> <- IF.Imgfx.decode(file, 1600, 1000)
        +b : BM.Bitmap = old.k(r)
        +l : Nat = leaves(tree(b))
        IO.bind(BM.Bitmap, BM.Bitmap, olds(k, file), x => IO.pure(BM.Bitmap, Bool.pick(BM.Bitmap, Nat.is_eq(k, 0n), b, x)))

def news(+n: Nat, +file: String) -> IO(BM.Bitmap):
  match n:
    case 0n:
      IO.pure(BM.Bitmap, BM.Bitmap{0, 0, 0n, Pix{0}, 0, 0, 0, 0})
    case 1n+k:
      do IO<BM.Bitmap>:
        r : Result<&1, &1, U32 & String, U32 & U32 & Image> <- IF.Imgfx.tree(file, 1600, 1000)
        +b : BM.Bitmap = new.k(r)
        +l : Nat = leaves(tree(b))
        IO.bind(BM.Bitmap, BM.Bitmap, news(k, file), x => IO.pure(BM.Bitmap, Bool.pick(BM.Bitmap, Nat.is_eq(k, 0n), b, x)))

def places(+n: Nat, +b: BM.Bitmap) -> Nat:
  match n:
    case 0n:
      0n
    case 1n+k:
      Nat.add(leaves(BM.Bitmap.over(1400, 900, BM.Bitmap.move(b, 100, 50, 1200, 750), Pix{0})), places(k, b))

def field(s: String, +i: Nat) -> String:
  Maybe.default(&2, String, List.get(&2, String, String.split(s, '\t'), i), "")

def run(+all: String) -> IO(Unit):
  do IO<Unit>:
    +file : String = field(all, 0n)
    +reps : Nat = num(field(all, 1n), 0n)
    +t0 : Nat <- IO.now()
    +a : BM.Bitmap <- olds(reps, file)
    +t1 : Nat <- IO.now()
    +b : BM.Bitmap <- news(reps, file)
    +t2 : Nat <- IO.now()
    +x : Nat = places(reps, b)
    +t3 : Nat <- IO.now()
    IO.print(file ++ " " ++ size(b) ++ ", " ++ Nat.show(leaves(tree(b))) ++ " leaves")
    IO.print("old (bytes as a list, Bitmap.image): " ++ Nat.show(Nat.div(Nat.sub(t1, t0), reps)) ++ " ms a frame")
    IO.print("new (Imgfx.tree): " ++ Nat.show(Nat.div(Nat.sub(t2, t1), reps)) ++ " ms a frame")
    IO.print("placed in a 1400x900 frame: " ++ Nat.show(Nat.div(Nat.sub(t3, t2), reps)) ++ " ms a frame (" ++ Nat.show(x) ++ ")")
    IO.print("same tree: " ++ Bool.pick(String, same(tree(a), tree(b)), "yes", "NO"))

def copy(xs: List<String>) -> List<&2, String>:
  match xs:
    case Nil{}:
      Nil{}
    case x <> t:
      x <> copy(t)

def main() -> IO(Unit):
  do IO<Unit>:
    args0 : List<String> <- AV.Argv.args()
    +args : List<&2, String> = copy(args0)
    +all : String = arg(args, 0n, "") ++ "\t" ++ arg(args, 1n, "5")
    run(all)
