import Base
import ../../src/app/win.bend as W

# A real platform-window smoke test. The external Windows test resizes the
# window, sends Unicode, mouse and wheel input, then closes it. No X server.
def words(xs: List<&2, U32>) -> String:
  match xs:
    case Nil{}:
      ""
    case x <> t:
      U32.show(x) ++ " " ++ words(t)

def quitting(xs: List<&2, U32>) -> Bool:
  match xs:
    case k <> _ <> _ <> _ <> _ <> t:
      Bool.or(U32.is_eq(k, 6), quitting(t))
    case _:
      False{}

def after_paste(r: Window & String) -> IO(Window):
  (w, text) = r
  do IO<Window>:
    IO.print("CLIPBOARD " ++ text)
    IO.pure(Window, w)

def take(r: Window & List<&2, U32>) -> IO(Window & Bool):
  (w, +xs) = r
  do IO<Window & Bool>:
    IO.print("EVENTS " ++ words(xs))
    p : Window & String <- W.Win.pasted(w)
    w1 : Window <- after_paste(p)
    IO.pure(Window & Bool, (w1, quitting(xs)))

def keep_going(quit: Bool, w: Window, cont: Window -> IO(Unit)) -> IO(Unit):
  match quit:
    case True{}:
      IO.print("WINDOW_CLOSED")
    case False{}:
      cont(w)

def continue_with(r: Window & Bool, cont: Window -> IO(Unit)) -> IO(Unit):
  (w, quit) = r
  keep_going(quit, w, cont)

def loop(n: Nat, w: Window) -> IO(Unit):
  match n:
    case 0n:
      IO.print("FAIL event budget")
    case 1n+k:
      do IO<Unit>:
        r : Window & List<&2, U32> <- W.Win.words(w)
        q : Window & Bool <- take(r)
        continue_with(q, w1 => loop(k, w1))

def presented(r: Window & Image) -> IO(Unit):
  (w, _) = r
  do IO<Unit>:
    w1 : Window <- W.Win.copy(w, "Windows clipboard 日本😀")
    w2 : Window <- W.Win.paste(w1)
    IO.print("WINDOW_READY")
    loop(10000n, w2)

def opened(r: Result<&1, &1, U32 & String, Window>) -> IO(Unit):
  match r:
    case Fail{_}:
      IO.print("FAIL window open")
    case Done{w}:
      IO.bind(Window & Image, Unit, W.Win.present(w, Qua{Pix{16711680}, Pix{65280}, Pix{255}, Pix{16777215}}), presented)

def main() -> IO(Unit):
  IO.bind(Result<&1, &1, U32 & String, Window>, Unit, W.Win.open("Backplane Windows smoke", 400, 300), opened)
