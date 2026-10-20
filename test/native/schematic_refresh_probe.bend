import Base
import ../../src/server/host.bend as H
import ../../src/app/main.bend as M
import ../../src/app/viewer.bend as V
import ../../src/app/win.bend as W
import ../../src/core/board.bend as B
import ../../src/core/schematic.bend as SC
import ../../src/core/hier.bend as HI
import ../../src/server/plot.bend as PV
import ../../src/server/git.bend as GT
import ../../src/server/argv.bend as AV
import ../../src/gfx/draw.bend as D
import ../../src/gfx/raster.bend as R

# Real native watcher -> channel -> scene -> X11 presentation. Synthetic
# fixtures only. The driver timestamps writes and inspects captured pixels.
# A separate stage pass measures read, parse, sort and hierarchy loading.

def items(m: Maybe<&2, List<&2, B.Item>>) -> List<&2, B.Item>:
  match m:
    case Some{xs}: xs
    case None{}: Nil{}

def wire.end(ps: List<&2, B.Pt>) -> String:
  match ps:
    case _ <> B.Pt{x, _} <> _: U32.show(U32.sub(F32.to_u32(x), 20))
    case _: "unknown"

def revision.hit(xs: List<&2, B.Item>) -> Bool:
  match xs:
    case B.Item{k, _, _, _, _} <> _: String.eq(k, "w0")
    case _: False{}

def revision.go(xs: List<&2, B.Item>, hit: Bool) -> String:
  match xs hit:
    case B.Item{_, _, _, B.PLine{ps, _}, _} <> _ True{}: wire.end(ps)
    case _ <> +t _: revision.go(t, revision.hit(t))
    case Nil{} _: "unknown"

def revision(+xs: List<&2, B.Item>) -> String:
  revision.go(xs, revision.hit(xs))

def stages(+path: String) -> IO(Unit):
  do IO<Unit>:
    +t0 : Nat <- IO.now()
    +bs : List<&2, U32> <- GT.Git.version(path)
    +len : Nat = List.length(&2, U32, bs)
    +t1 : Nat <- IO.now()
    +xs : List<&2, B.Item> = items(SC.Sch.items(bs))
    +n : Nat = List.length(&2, B.Item, xs)
    +t2 : Nat <- IO.now()
    +sorted : List<&2, B.Item> = V.Scene.sort(xs)
    +ns : Nat = List.length(&2, B.Item, sorted)
    +t3 : Nat <- IO.now()
    +tab : List<&2, HI.Kids> <- PV.Tree.load(64n, [path], Nil{})
    +rows : String = HI.Hier.show(HI.Hier.rows(tab, path))
    +nr : Nat = String.length(rows)
    +t4 : Nat <- IO.now()
    IO.print("stages bytes=" ++ Nat.show(len) ++ " items=" ++ Nat.show(n) ++ " sorted=" ++ Nat.show(ns) ++ " treechars=" ++ Nat.show(nr)
      ++ " read=" ++ Nat.show(Nat.sub(t1, t0)) ++ " parse=" ++ Nat.show(Nat.sub(t2, t1)) ++ " sort=" ++ Nat.show(Nat.sub(t3, t2)) ++ " tree=" ++ Nat.show(Nat.sub(t4, t3)))

def first(r: Window & Image) -> Window:
  (w, _) = r
  w

# A fixed camera makes pixel changes comparable across revisions.
def draw(xs: List<&2, B.Item>, +old: List<&2, V.Seen>, win: Window) -> IO(Window & List<&2, V.Seen>):
  do IO<Window & List<&2, V.Seen>>:
    +t0 : Nat <- IO.now()
    +ss : List<&2, V.Seen> = V.Scene.update(old, xs, 0n)
    +n : Nat = List.length(&2, V.Seen, ss)
    +t1 : Nat <- IO.now()
    +ops : List<&2, R.Op> = List.reverse(&2, R.Op, V.Ops.scene(ss, V.View{10.0, 300.0, 240.0}, D.Clip{0, 0, 800, 600}, 10000n, 10000n, True{}, 1, 0))
    +no : Nat = List.length(&2, R.Op, ops)
    +t2 : Nat <- IO.now()
    +img : Image = R.Frame.render(800, 600, ops, 855826)
    # Win.present evaluates the raster before sending XPutImage/XFlush.
    pair : Window & Image <- W.Win.present(win, img)
    +t3 : Nat <- IO.now()
    IO.print("display ms=" ++ Nat.show(t3) ++ " items=" ++ Nat.show(n) ++ " ops=" ++ Nat.show(no) ++ " scene=" ++ Nat.show(Nat.sub(t1, t0)) ++ " prepare=" ++ Nat.show(Nat.sub(t2, t1)) ++ " present=" ++ Nat.show(Nat.sub(t3, t2)))
    return (first(pair), ss)

def drawn(r: Window & List<&2, V.Seen>, k: Window -> List<&2, V.Seen> -> IO(Unit)) -> IO(Unit):
  (w, ss) = r
  k(w, ss)

def got(m: Maybe<&1, M.ShellMsg>, win: Window, +ss: List<&2, V.Seen>, k: Window -> List<&2, V.Seen> -> IO(Unit)) -> IO(Unit):
  match m:
    case Some{M.Board{_, +xs}}:
      do IO<Unit>:
        +now : Nat <- IO.now()
        IO.print("received ms=" ++ Nat.show(now) ++ " revision=" ++ revision(xs))
        IO.bind(Window & List<&2, V.Seen>, Unit, draw(xs, ss, win), r => drawn(r, k))
    case Some{M.FromHub{_, _}}:
      IO.pure(Unit, Unit{})
    case _:
      k(win, ss)

def consume(fuel: Nat, +q: Chan(M.ShellMsg), win: Window, +ss: List<&2, V.Seen>) -> IO(Unit):
  match fuel:
    case 0n: IO.pure(Unit, Unit{})
    case 1n+f:
      IO.bind(Maybe<&1, M.ShellMsg>, Unit, Chan.recv(M.ShellMsg, q), m => got(m, win, ss, w => s => consume(f, q, w, s)))

def watch(+path: String, +q: Chan(M.ShellMsg)) -> IO(Unit):
  do IO<Unit>:
    M.Watch.loop(80n, "1|" ++ HI.Hier.dir(path) ++ "|" ++ path, path, 0, "", 0n, q)
    ok : Result<&1, &1, M.ShellMsg, Unit> <- Chan.send(M.ShellMsg, q, M.FromHub{"done", ""})
    return Unit{}

def main() -> IO(Unit):
  do IO<Unit>:
    args : List<String> <- AV.Argv.args()
    +path : String = Maybe.default(&1, String, List.head(&1, String, args), "x")
    # host.c's unconditional Ji helpers need their CBOR support linked in
    # component builds too; the full app already uses this effect.
    framed : List<&2, U32> <- H.Json.items_frames("{", "", "", Nil{}, "}", 1, 100, 100, "", "")
    stages(path)
    +q : Chan(M.ShellMsg) <- Chan.new(M.ShellMsg, 32)
    win : Window <- IO.try(Window, W.Win.open("Schematic refresh probe", 800, 600))
    IO.spawn(Unit, watch(path, q))
    consume(200n, q, win, Nil{})
