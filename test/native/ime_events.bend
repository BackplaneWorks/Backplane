import Base
import ../../src/app/win.bend as W

# The window's input events, with a real X window (run under Xvfb): ime
# events queued before the app drains them each keep their own text, in
# order; a long commit arrives whole; no unknown event is a quit.
#   scripts/build-app.sh test/native/ime_events.bend build/ime_events
#   DISPLAY=:91 BACKPLANE_NO_IME=1 build/ime_events

def ok(+name: String, +got: String, +want: String) -> IO(Unit):
  IO.print(Bool.pick(String, String.eq(got, want), "ok " ++ name, "FAIL " ++ name ++ ": got " ++ got ++ " want " ++ want))

# the ime events among the decoded, as "c<id>" and "p<id>"
def ids(es: List<&2, W.WinEv>) -> List<&2, U32>:
  match es:
    case Nil{}:
      Nil{}
    case W.WCommit{id, _} <> t:
      id <> ids(t)
    case W.WPreedit{_, _, id, _, _, _, _} <> t:
      id <> ids(t)
    case _ <> t:
      ids(t)

def quits(es: List<&2, W.WinEv>) -> Nat:
  match es:
    case Nil{}:
      0n
    case W.WQuit{} <> t:
      1n+quits(t)
    case _ <> t:
      quits(t)

def firsts(xs: List<&2, U32>) -> String:
  match xs:
    case Nil{}:
      ""
    case x <> t:
      U32.show(x) ++ " " ++ firsts(t)

def fst(r: Window & String) -> Window:
  (w, _) = r
  w

def snd(r: Window & String) -> String:
  (_, t) = r
  t

def snd2(r: Window & U32) -> U32:
  (_, t) = r
  t

def fst2(r: Window & List<&2, W.WinEv>) -> Window:
  (w, _) = r
  w

def snd3(r: Window & List<&2, W.WinEv>) -> List<&2, W.WinEv>:
  (_, e) = r
  e

def nth(xs: List<&2, U32>, +k: Nat) -> U32:
  Maybe.default(&2, U32, List.get(&2, U32, xs, k), 0)

def pair(r: Window & String & String) -> IO(Window & String):
  (w, (t, f)) = r
  IO.pure(Window & String, (w, t ++ "|" ++ f ++ ";"))

def cat(r: Window & String, p: Window & String) -> IO(Window & String):
  (w, acc) = p
  IO.pure(Window & String, (w, acc))

def add(acc: String, q: Window & String) -> IO(Window & String):
  (w, t) = q
  IO.pure(Window & String, (w, acc ++ t))

def one(x: U32, r: Window & String) -> IO(Window & String):
  (w, acc) = r
  IO.bind(Window & String, Window & String, IO.bind(Window & String & String, Window & String, W.Win.payload(w, x), pair), q => add(acc, q))

# fetch the payloads newest first, so each is shown not to depend on order
def fetch(w: Window, xs: List<&2, U32>) -> IO(Window & String):
  match xs:
    case Nil{}:
      IO.pure(Window & String, (w, ""))
    case x <> t:
      IO.bind(Window & String, Window & String, fetch(w, t), r => one(x, r))

def decoded(r: Window & List<&2, U32>) -> IO(Window & List<&2, W.WinEv>):
  (w, ws) = r
  IO.pure(Window & List<&2, W.WinEv>, (w, W.Win.decode(ws)))

def drain(w: Window) -> IO(Window & List<&2, W.WinEv>):
  IO.bind(Window & List<&2, U32>, Window & List<&2, W.WinEv>, W.Win.words(w), decoded)

def text_of(r: Window & String & String) -> IO(Window & String):
  (w, (t, _)) = r
  IO.pure(Window & String, (w, t))

def go3(r: Window & String, t1: String, +big: String) -> IO(Unit):
  (w, t2) = r
  do IO<Unit>:
    ok("a long commit arrives whole", Bool.show(String.eq(t1, big)), "True")
    ok("and the next one is its own", t2, "after")
    m : Window & U32 <- W.Win.ime(w)
    IO.print("mode " ++ U32.show(snd2(m)))

def go2b(r: Window & String, +es: List<&2, W.WinEv>, +big: String) -> IO(Unit):
  (w, t1) = r
  IO.bind(Window & String, Unit, IO.bind(Window & String & String, Window & String, W.Win.payload(w, nth(ids(es), 1n)), text_of), q => go3(q, t1, big))

def go2(w: Window, +es: List<&2, W.WinEv>, +big: String) -> IO(Unit):
  IO.bind(Window & String, Unit, IO.bind(Window & String & String, Window & String, W.Win.payload(w, nth(ids(es), 0n)), text_of), q => go2b(q, es, big))

def go2c(r: Window & List<&2, W.WinEv>, +big: String) -> IO(Unit):
  (w, es) = r
  go2(w, es, big)

def go1b(r: Window & String, +es: List<&2, W.WinEv>) -> IO(Unit):
  (w, t) = r
  do IO<Unit>:
    ok("queued payloads keep their own text, in order", t, "日本語😀|0000;にほん|222;あ|0;")
    ok("no stray quit", Nat.show(quits(es)), "0")
    +big : String = String.repeat("日本語ABC😀", 60000n)
    w8 : Window <- W.Win.ime_inject(w, 0, big)
    w9 : Window <- W.Win.ime_inject(w8, 0, "after")
    d2 : Window & List<&2, W.WinEv> <- drain(w9)
    go2c(d2, big)

def go1(w: Window, +es: List<&2, W.WinEv>) -> IO(Unit):
  IO.bind(Window & String, Unit, fetch(w, ids(es)), r => go1b(r, es))

def go1c(r: Window & List<&2, W.WinEv>) -> IO(Unit):
  (w, es) = r
  go1(w, es)

def go(w: Window) -> IO(Unit):
  do IO<Unit>:
    w1 : Window <- W.Win.ime_inject(w, 0, "あ")
    w2 : Window <- W.Win.ime_inject(w1, 1, "にほん")
    w3 : Window <- W.Win.ime_inject(w2, 0, "日本語😀")
    w4 : Window <- W.Win.ime_inject(w3, 2, "")
    w5 : Window <- W.Win.ime_inject(w4, 0, "")
    d : Window & List<&2, W.WinEv> <- drain(w5)
    go1c(d)

def go0(r: Result<&1, &1, U32 & String, Window>) -> IO(Unit):
  match r:
    case Done{w}:
      go(w)
    case Fail{e}:
      IO.print("FAIL open")

def main() -> IO(Unit):
  do IO<Unit>:
    r : Result<&1, &1, U32 & String, Window> <- W.Win.open("ime", 400, 300)
    go0(r)
