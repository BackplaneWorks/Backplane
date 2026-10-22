import Base
import ../../src/server/server.bend as S
import ../../src/core/model.bend as M
import ../../src/core/client.bend as U
import ../../src/core/json.bend as J
import ../../src/app/link.bend as LK
import ../../src/server/argv.bend as AV

# What switching the window to another machine's hub costs: a link
# (src/app/link.bend) joined as the window joins it, timed until the hub's
# log arrives, then the window's parse and replay of it.
#   scripts/build-js.sh test/native/link_bench.bend build/link_bench
#   build/link_bench http://127.0.0.1:3790

def seen(+n: Nat, +what: String, +a: Nat, +b: Nat) -> IO(Unit):
  IO.print(what ++ ": " ++ Nat.show(Nat.sub(b, a)) ++ " ms (" ++ Nat.show(n) ++ ")")

def count(s: M.State) -> Nat:
  match s:
    case M.State{_, _, es, _, _, _, _, _, _, _, _}:
      List.length(&2, M.Entry, es)

def url(xs: List<String>) -> String:
  match xs:
    case Nil{}:
      "http://127.0.0.1:3787"
    case p <> Nil{}:
      p
    case _ <> t:
      url(t)

def is_log(+t: String) -> Bool:
  String.starts_with(t, "{\"t\":\"log\"")

def got(m: Maybe<&1, S.Wire>, k: Maybe<&2, String> -> IO(Maybe<&2, String>), +t0: Nat) -> IO(Maybe<&2, String>):
  match m:
    case Some{S.WText{+t}}:
      do IO<Maybe<&2, String>>:
        +t1 : Nat <- IO.now()
        IO.print("  +" ++ Nat.show(Nat.sub(t1, t0)) ++ " ms: " ++ String.take(t, 60n) ++ " (" ++ Nat.show(String.length(t)) ++ " chars)")
        k(Bool.pick(Maybe<&2, String>, is_log(t), Some{t}, None{}))
    case Some{_}:
      k(None{})
    case None{}:
      IO.pure(Maybe<&2, String>, None{})

def found(m: Maybe<&2, String>, loop: Unit -> IO(Maybe<&2, String>)) -> IO(Maybe<&2, String>):
  match m:
    case Some{t}:
      IO.pure(Maybe<&2, String>, Some{t})
    case None{}:
      loop(Unit{})

# the messages until the log
def wait(fuel: Nat, +out: Chan(S.Wire), +t0: Nat) -> IO(Maybe<&2, String>):
  match fuel:
    case 0n:
      IO.pure(Maybe<&2, String>, None{})
    case 1n+f:
      IO.bind(Maybe<&1, S.Wire>, Maybe<&2, String>, Chan.recv(S.Wire, out),
        m => got(m, r => found(r, u => wait(f, out, t0)), t0))

def replay(m: Maybe<&2, String>, +t1: Nat) -> IO(Unit):
  match m:
    case None{}:
      IO.print("no log")
    case Some{+text}:
      do IO<Unit>:
        +o : J.Json = Maybe.default(&2, J.Json, J.Json.parse(text), J.Null{})
        +t2 : Nat <- IO.now()
        seen(List.length(&2, J.Json, J.Json.elems(J.Json.get_obj(o, "items"))), "window: parse log", t1, t2)
        +u : U.Ui = U.Ui.recv(U.Ui.init(1790000000n), o)
        +t3 : Nat <- IO.now()
        seen(count(U.Ui.state(u)), "window: replay", t2, t3)

def main() -> IO(Unit):
  do IO<Unit>:
    args : List<String> <- AV.Argv.args()
    +to : String = url(args)
    +t0 : Nat <- IO.now()
    +inbox : Chan(S.Msg) <- LK.Link.start(to)
    +out : Chan(S.Wire) <- Chan.new(S.Wire, 8192)
    ok : Result<&1, &1, S.Msg, Unit> <- Chan.send(S.Msg, inbox, S.Join{0, out, 0n, "", 1, 0, False{}})
    +m : Maybe<&2, String> <- wait(64n, out, t0)
    +t1 : Nat <- IO.now()
    seen(0n, "link: connect, fetch and decode the log", t0, t1)
    replay(m, t1)
    +t4 : Nat <- IO.now()
    seen(0n, "total", t0, t4)
    ok2 : Result<&1, &1, S.Msg, Unit> <- Chan.send(S.Msg, inbox, S.Leave{0})
    return Unit{}
