import Base
import ../../src/app/win.bend as W
import ../../src/core/json.bend as J

# Real Win.open/words pump for desktop drag acceptance, without a hub.
def has(xs: List<&2, U32>, +kind: U32) -> Bool:
  match xs:
    case k <> _ <> _ <> _ <> _ <> t:
      Bool.or(U32.is_eq(k, kind), has(t, kind))
    case _:
      False{}

def dropped(r: Window & String) -> IO(Window):
  (w, text) = r
  do IO<Window>:
    IO.print(J.Json.show(J.Json.obj([J.KV{"drop", J.Str{text}}])))
    IO.pure(Window, w)

def take_drop(on: Bool, w: Window) -> IO(Window):
  match on:
    case True{}: IO.bind(Window & String, Window, W.Win.dropped(w), dropped)
    case False{}: IO.pure(Window, w)

def keep(quit: Bool, w: Window, cont: Window -> IO(Unit)) -> IO(Unit):
  match quit:
    case True{}: IO.print("{\"closed\":true}")
    case False{}: cont(w)

def words(r: Window & List<&2, U32>, cont: Window -> IO(Unit)) -> IO(Unit):
  (w, +xs) = r
  do IO<Unit>:
    w1 : Window <- take_drop(has(xs, 10), w)
    keep(has(xs, 6), w1, cont)

def loop(n: Nat, w: Window) -> IO(Unit):
  match n:
    case 0n: IO.print("{\"error\":\"event budget\"}")
    case 1n+k:
      IO.bind(Window & List<&2, U32>, Unit, W.Win.words(w), r => words(r, w1 => loop(k, w1)))

def ready(r: Window & U32) -> IO(Unit):
  (w, xid) = r
  do IO<Unit>:
    IO.print("{\"parent\":" ++ U32.show(xid) ++ "}")
    loop(100000n, w)

def opened(r: Result<&1, &1, U32 & String, Window>) -> IO(Unit):
  match r:
    case Fail{_}: IO.print("{\"error\":\"window open\"}")
    case Done{w}: IO.bind(Window & U32, Unit, W.Win.xid(w), ready)

def main() -> IO(Unit):
  IO.bind(Result<&1, &1, U32 & String, Window>, Unit, W.Win.open("CEF native input acceptance", 1400, 900), opened)
