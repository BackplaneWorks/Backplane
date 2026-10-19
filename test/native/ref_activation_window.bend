import Base
import ../ref_activation/fixture.bend as F
import ../../src/app/layout.bend as L
import ../../src/app/nui.bend as N
import ../../src/app/win.bend as W
import ../../src/core/client.bend as U
import ../../src/core/links.bend as LK
import ../../src/gfx/draw.bend as D
import ../../src/gfx/raster.bend as R

# Real X11 input acceptance for explicit refs; no hub or agent processes.
# The driver clicks eight rendered rows in order, and retains EVENTS/CLICK.
def rows() -> List<&2, String>:
  ["backplane://thread/t1792085069-132986",
   "backplane://open?thread=t1792085069-132986",
   "[Raul](backplane://thread/t1792085069-132986)",
   "[Raul](backplane://open?thread=t1792085069-132986)",
   "[Other machine](backplane://thread/peer%7Et1792085069-132986)",
   "backplane://open?thread=peer~t1792085069-132986",
   "backplane://thread/missing",
   "t1792085069-132986"]

def expected(i: Nat) -> String:
  match i:
    case 4n: "peer~t1792085069-132986"
    case 5n: "peer~t1792085069-132986"
    case 6n: "t1-1"
    case 7n: "t1-1"
    case _: "t1792085069-132986"

def expected_hit(i: Nat) -> String:
  match i:
    case 4n: "backplane://thread/peer~t1792085069-132986"
    case 5n: "backplane://thread/peer~t1792085069-132986"
    case 6n: "backplane://thread/missing"
    case 7n: ""
    case _: "backplane://thread/t1792085069-132986"

def draw(xs: List<&2, String>, +ty: L.Ty, +i: Nat) -> L.Out:
  match xs:
    case Nil{}: L.Out.none()
    case s <> rest:
      L.Out.cat(L.TL.link.segs(LK.Link.pieces(s), ty, D.Clip{0, 0, 1400, 500}, 300, U32.from_nat(Nat.add(100n, Nat.mul(32n, i))), 16777215), draw(rest, ty, 1n+i))

def ops(o: L.Out) -> List<&2, R.Op>:
  match o:
    case L.Out{os, _}: os

def hits(o: L.Out) -> List<&2, N.Hit>:
  match o:
    case L.Out{_, hs}: hs

def with_hits(n: N.Nui, hs: List<&2, N.Hit>) -> N.Nui:
  match n:
    case N.Nui{u, w, h, x, y, sd, sc, ca, fo, pa, _, ms, vw}:
      N.Nui{u, w, h, x, y, sd, sc, ca, fo, pa, hs, ms, vw}

def words(xs: List<&2, U32>) -> String:
  match xs:
    case Nil{}: ""
    case x <> rest: U32.show(x) ++ " " ++ words(rest)

def presses(es: List<&2, W.WinEv>, n: Nat) -> Nat:
  match es:
    case Nil{}: n
    case W.WButton{_, _, 1, True{}} <> rest: presses(rest, 1n+n)
    case _ <> rest: presses(rest, n)

def clicked_hit(es: List<&2, W.WinEv>, hs: List<&2, N.Hit>) -> String:
  match es:
    case Nil{}: ""
    case W.WButton{x, y, 1, True{}} <> _: N.Hit.value_at(hs, "ref-go", x, y)
    case _ <> rest: clicked_hit(rest, hs)

def checked(ok: Bool, done: Bool, w: Window, n: N.Nui, count: Nat, go: Window -> N.Nui -> Nat -> IO(Unit)) -> IO(Unit):
  match ok done:
    case False{} _: IO.print("FAIL native rendered selection")
    case True{} True{}: IO.print("PASS native window eight rendered clicks")
    case True{} False{}: go(w, n, count)

def pressed(yes: Bool, w: Window, +n: N.Nui, +count: Nat, +uri: String, go: Window -> N.Nui -> Nat -> IO(Unit)) -> IO(Unit):
  match yes:
    case False{}: go(w, n, count)
    case True{}:
      do IO<Unit>:
        IO.print("CLICK " ++ Nat.show(count) ++ " hit=" ++ uri ++ " selected=" ++ U.Ui.sel(N.Nui.ui(n)) ++ " expected=" ++ expected(Nat.sub(count, 1n)))
        checked(Bool.and(String.eq(uri, expected_hit(Nat.sub(count, 1n))), String.eq(U.Ui.sel(N.Nui.ui(n)), expected(Nat.sub(count, 1n)))), Nat.is_eq(count, 8n), w, n, count, go)

def received(r: Window & List<&2, U32>, +n: N.Nui, +count: Nat, go: Window -> N.Nui -> Nat -> IO(Unit)) -> IO(Unit):
  (w, +xs) = r
  do IO<Unit>:
    IO.print("EVENTS " ++ words(xs))
    +events : List<&2, W.WinEv> = W.Win.decode(xs)
    +down : Nat = presses(events, 0n)
    +uri : String = clicked_hit(events, N.Nui.hits(n))
    +next : N.Nui = N.Step.nui(N.Nui.events(events, N.Step{N.Nui.with_ui(n, F.fresh()), Nil{}, False{}}))
    pressed(Nat.is_gt(down, 0n), w, next, Nat.add(count, down), uri, go)

def loop(fuel: Nat, w: Window, +n: N.Nui, +count: Nat) -> IO(Unit):
  match fuel:
    case 0n: IO.print("FAIL event budget")
    case 1n+k:
      IO.bind(Window & List<&2, U32>, Unit, W.Win.words(w), r => received(r, n, count, w2 => n2 => c2 => loop(k, w2, n2, c2)))

def presented(r: Window & Image, n: N.Nui) -> IO(Unit):
  (w, _) = r
  do IO<Unit>:
    IO.print("READY native reference rows x301 y101+32i")
    loop(200n, w, n, 0n)

def opened(result: Result<&1, &1, U32 & String, Window>) -> IO(Unit):
  match result:
    case Fail{_}: IO.print("FAIL window open")
    case Done{w}:
      do IO<Unit>:
        +ty : L.Ty <- L.Ty.load()
        +out : L.Out = draw(rows(), ty, 0n)
        +n : N.Nui = with_hits(N.Nui.init(F.fresh(), 1400, 500), hits(out))
        shown : Window & Image <- W.Win.present(w, R.Frame.render(1400, 500, ops(out), 1119251))
        presented(shown, n)

def main() -> IO(Unit):
  IO.bind(Result<&1, &1, U32 & String, Window>, Unit, W.Win.open("Backplane", 1400, 500), opened)
