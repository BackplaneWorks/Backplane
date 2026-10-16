import Base
import ../../src/gfx/text.bend as X
import ../../src/app/bots.bend as BV
import ../../src/app/layout.bend as L
import ../../src/core/bot.bend as BT
import ../../src/server/argv.bend as AV

# Each invocation measures one fresh process, with ASCII warmed as in the
# window. Construction/font loading are outside the timer; all lines are
# consumed inside it. Width is the investigation's 780 px.
# scripts/build-app.sh test/native/wrap_bench.bend build/wrap_bench
# build/wrap_bench [10000|90000] [token|normal|cache|current]

def word(n: Nat, acc: String) -> String:
  match n:
    case 0n:
      acc
    case 1n+k:
      word(k, SCon{Chr{120}, acc})

def normal(n: Nat, acc: String) -> String:
  match n:
    case 0n:
      acc
    case 1n+k:
      normal(k, "Check the board and its power rails before routing. " ++ acc)

def sample(token: Bool, +n: Nat) -> String:
  match token:
    case True{}:
      word(n, "")
    case False{}:
      String.take(normal(1n+Nat.div(n, 50n), ""), n)

def chars(ls: List<&2, String>, acc: Nat) -> Nat:
  match ls:
    case Nil{}:
      acc
    case l <> t:
      chars(t, Nat.add(acc, String.length(l)))

def arg(xs: List<String>) -> String & String:
  match xs:
    case a <> b <> _:
      (a, b)
    case a <> _:
      (a, "token")
    case Nil{}:
      ("10000", "token")

# "current" replays the previous one-conversation policy, with the new
# linear fitter, to isolate the benefit of retaining conversation groups.
# "cache" uses the actual bounded cache. Both run A -> B -> A in a fresh
# process and consume every wrapped character before stopping the timer.
def Cache.update(retain: Bool, +ps: List<&2, BT.Post>, +room: String, ws: Map<&2, BV.PW>, +ty: L.Ty) -> Map<&2, BV.PW>:
  match retain:
    case True{}:
      BV.Bv.wraps.conversation(ps, room, ws, ty, 780)
    case False{}:
      BV.Bv.wraps.put(ps, ws, ty, 780, BV.Bv.wraps.none())

def Cache.posts(retain: Bool, ws: Map<&2, BV.PW>, +room: String) -> Map<&2, BV.PW>:
  match retain:
    case True{}:
      BV.Bv.wraps.group(ws, room, 780)
    case False{}:
      ws

def Cache.step(+retain: Bool, +ty: L.Ty, +room: String, +text: String, ws: Map<&2, BV.PW>, name: String) -> IO(Map<&2, BV.PW>):
  do IO<Map<&2, BV.PW>>:
    +t0 : Nat <- IO.now()
    +next : Map<&2, BV.PW> = Cache.update(retain, [BT.Post{room, room, "you", text, 0n, 0n}], room, ws, ty)
    +ls : List<&2, String> = BV.Bv.wrap(Cache.posts(retain, next, room), ty, room, text, 780)
    +lines : Nat = List.length(&2, String, ls)
    +kept : Nat = chars(ls, 0n)
    +t1 : Nat <- IO.now()
    IO.print(name ++ " wrap_ms=" ++ Nat.show(Nat.sub(t1, t0)) ++ " lines=" ++ Nat.show(lines) ++ " kept=" ++ Nat.show(kept))
    return next

def Cache.run(+retain: Bool, +f: X.Face, +c: Map<&2, X.TG>, +n: Nat) -> IO(Unit):
  do IO<Unit>:
    +ty : L.Ty = L.Ty{c, X.Text.table(c), f, f, f, f, f}
    +text : String = word(n, "")
    a : Map<&2, BV.PW> <- Cache.step(retain, ty, "a", text, BV.Bv.wraps.none(), "first A")
    b : Map<&2, BV.PW> <- Cache.step(retain, ty, "b", "A short other conversation", a, "switch B")
    back : Map<&2, BV.PW> <- Cache.step(retain, ty, "a", text, b, "return A")
    IO.print("groups=" ++ Nat.show(List.length(&2, String, BV.Bv.wraps.keys(back))))

def measure(cache: Bool, +kind: String, +n: Nat, +s: String, +f: X.Face, +c: Map<&2, X.TG>) -> IO(Unit):
  match cache:
    case True{}:
      Cache.run(String.eq(kind, "cache"), f, c, n)
    case False{}:
      do IO<Unit>:
        +t0 : Nat <- IO.now()
        +ls : List<&2, String> = X.Text.wrap(c, f, s, 780)
        +lines : Nat = List.length(&2, String, ls)
        +kept : Nat = chars(ls, 0n)
        +t1 : Nat <- IO.now()
        IO.print(kind ++ " chars=" ++ Nat.show(n) ++ " wrap_ms=" ++ Nat.show(Nat.sub(t1, t0)) ++ " lines=" ++ Nat.show(lines) ++ " kept=" ++ Nat.show(kept) ++ " face=" ++ X.Face.name(f))

def run(a: String & String) -> IO(Unit):
  (+size, +kind) = a
  do IO<Unit>:
    +n : Nat = Maybe.default(&2, Nat, Nat.read(size), 10000n)
    +s : String = sample(String.eq(kind, "token"), n)
    +f : X.Face <- X.Text.face(["Berkeley Mono", "monospace"], 14)
    +c : Map<&2, X.TG> <- X.Text.warm(Map.new(&2, X.TG), f)
    +ft : X.Face = X.Face.tabled(f, c)
    measure(Bool.or(String.eq(kind, "cache"), String.eq(kind, "current")), kind, n, s, ft, c)

def main() -> IO(Unit):
  IO.bind(List<String>, Unit, AV.Argv.args(), as => run(arg(as)))
