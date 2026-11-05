import Base
import ../../src/core/json.bend as J
import ../../src/core/client.bend as U
import ../../src/mobile/hubs.bend as H
import ../../src/mobile/session.bend as MS
import ../../src/core/native-sidebar.bend as NS
import ../../src/mobile/thin.bend as MT
import ../../src/mobile/folded-actions.bend as FA

# bp-policy: the slim backend's policy process, in machine code. It folds the
# hub's wire frames incrementally into Backplane's own client state (the same
# U.Ui.recv every client runs) and draws the screen a native client shows
# (the same MS.Session call Render.mobile makes in src/server/thin-view.bend).
# Base IO only: one loopback TCP connection, one request per line, one reply
# per line. No custom effects, no JS.
#
#   F <frame json>      fold one wire frame (log, changes, delta, info, ...)   -> ok
#   V <request json>    view.open/update/action params; optional p.now (ms since epoch: native IO.now()
#                       is not the wall clock) and p.window (the host's window object, as Render.native_screen takes it)  -> view json
#   C <ms>              set the wall clock (ms since epoch) the folded state reads              -> ok
#   P                   ping                                                    -> pong
#   Q                   close
#
# A request is ASCII (the host escapes non-ASCII as \uXXXX); a reply is one
# line of JSON. The state never crosses the socket: frames in, one picture out.

# ---- the picture -----------------------------------------------------------

def Pol.target.k(+empty: Bool, +sel: String) -> J.Json:
  match empty:
    case True{}: J.Json.obj([J.KV{"kind", J.Str{"recent"}}, J.KV{"id", J.Str{""}}])
    case False{}: J.Json.obj([J.KV{"kind", J.Str{"thread"}}, J.KV{"id", J.Str{sel}}])

def Pol.target(+session: J.Json) -> J.Json:
  +sel = J.Json.get_str(session, "sel", "")
  Pol.target.k(String.is_empty(sel), sel)

def Pol.same(+a: J.Json, +b: J.Json) -> Bool:
  Bool.and(String.eq(J.Json.get_str(a, "kind", ""), J.Json.get_str(b, "kind", "")), String.eq(J.Json.get_str(a, "id", ""), J.Json.get_str(b, "id", "")))

def Pol.is_obj(+j: J.Json) -> Bool:
  match j:
    case J.Obj{_}: True{}
    case _: False{}

# Render.native_screen of thin-view.bend: partial rows, folded actions, thread.earlier = window.remaining
def Pol.earlier(+th: J.Json, +window: J.Json) -> J.Json:
  match th:
    case J.Null{}: J.Null{}
    case +t: NS.Obj.put(t, "earlier", J.Json.nat(J.Json.get_nat(window, "remaining", 0n)))

def Pol.native(+screen: J.Json, +window: J.Json) -> J.Json:
  +partial = FA.Folded.screen(MT.Partial.screen(screen, window))
  NS.Obj.put(partial, "thread", Pol.earlier(J.Json.get_obj(partial, "thread"), window))

# with the host's window the screen is the hub's native screen; without it, the raw screen
def Pol.screen(+has: Bool, +screen: J.Json, +window: J.Json) -> J.Json:
  match has:
    case True{}: Pol.native(screen, window)
    case False{}: screen

# result.overlay: the {key, name, j} object with j drawn the same way
def Pol.overlay(+has: Bool, +shown: J.Json, +window: J.Json) -> J.Json:
  NS.Obj.put(shown, "j", Pol.screen(has, J.Json.get_obj(shown, "j"), window))

def Pol.out.k(+same: Bool, +has: Bool, +r: J.Json, +next: J.Json, +window: J.Json) -> J.Json:
  match same:
    case True{}:
      J.Json.obj([J.KV{"remoteReady", J.Flag{True{}}}, J.KV{"shown", Pol.screen(has, J.Json.get_obj(r, "screen"), window)}, J.KV{"overlay", Pol.overlay(has, J.Json.get_obj(r, "shown"), window)}, J.KV{"session", J.Json.get_obj(r, "session")}, J.KV{"cmds", J.Json.get_obj(r, "cmds")}])
    case False{}:
      J.Json.obj([J.KV{"remoteReady", J.Flag{False{}}}, J.KV{"nextTarget", next}, J.KV{"session", J.Json.get_obj(r, "session")}, J.KV{"cmds", J.Json.get_obj(r, "cmds")}])

def Pol.out(+same: Bool, +r: J.Json, +next: J.Json, +window: J.Json) -> J.Json:
  Pol.out.k(same, Pol.is_obj(window), r, next, window)

def Pol.view.r(+r: J.Json, +want: J.Json, +window: J.Json) -> J.Json:
  +next = Pol.target(J.Json.get_obj(r, "session"))
  Pol.out(Pol.same(next, want), r, next, window)

# the session the client sent adopted onto the shared state, the action run,
# the screen drawn
def Pol.view(+u0: U.Ui, +now0: Nat, +p: J.Json) -> J.Json:
  +now = J.Json.get_nat(p, "now", now0)
  +key = J.Json.get_str(p, "hub", "")
  +cid = J.Json.get_str(p, "cid", "")
  +u1 = U.Ui.scratch_set(U.Ui.online(U.Ui.set_cid(MS.Session.adopt(U.Ui.tick(u0, now), J.Json.get_obj(p, "session")), cid), True{}), "@sync", "1")
  +u = NS.Ns.prepare(u1, False{}, p, J.Json.get_str(p, "epoch", ""), J.Json.get_str(p, "viewId", ""), J.Json.get_nat(p, "revision", 0n), J.Json.get_str(p, "nativeKey", ""))
  Pol.view.r(MS.Session.result(MS.Session.act(u, key, cid, now, J.Json.get_str(p, "action", ""), J.Json.get_str(p, "value", "")), key), J.Json.get_obj(p, "target"), J.Json.get_obj(p, "window"))

# ---- the fold --------------------------------------------------------------

def Pol.fold.k(m: Maybe<&2, J.Json>, +u: U.Ui) -> U.Ui:
  match m:
    case Some{j}: U.Ui.recv(u, j)
    case None{}: u

def Pol.fold(+u: U.Ui, +text: String) -> U.Ui:
  Pol.fold.k(J.Json.parse(text), u)

# ---- one request -----------------------------------------------------------

def Pol.tail(+line: String) -> String:
  String.drop(line, 2n)

def Pol.op(+line: String) -> String:
  String.take(line, 1n)

type Step is Data:
  Step{u: U.Ui, reply: String, stop: Bool}

def Pol.step.k(+op: String, +u: U.Ui, +line: String, +now: Nat) -> Step:
  match op:
    case "F": Step{Pol.fold(u, Pol.tail(line)), "ok", False{}}
    case "V": Step{u, J.Json.show(Pol.view(u, now, Maybe.default(&2, J.Json, J.Json.parse(Pol.tail(line)), J.Null{}))), False{}}
    case "C": Step{U.Ui.tick(u, Maybe.default(&2, Nat, Nat.read(Pol.tail(line)), 0n)), "ok", False{}}
    case "P": Step{u, "pong", False{}}
    case "Q": Step{u, "bye", True{}}
    case _: Step{u, "err", False{}}

def Pol.step(+u: U.Ui, +line: String, +now: Nat) -> Step:
  Pol.step.k(Pol.op(line), u, line, now)

# ---- the loop --------------------------------------------------------------

def Pol.fuel() -> Nat:
  U32.to_nat(4294967295)

def Pol.copy(xs: List<&1, String>) -> List<&2, String>:
  match xs:
    case Nil{}: Nil{}
    case x <> t: x <> Pol.copy(t)

def Pol.arg(+xs: List<&2, String>, +i: Nat) -> String:
  Maybe.default(&2, String, List.head(&2, String, List.drop(&2, String, xs, i)), "")

def Pol.next(+stop: Bool, s: Socket, +u: U.Ui, k: Socket -> U.Ui -> String -> IO(Unit)) -> IO(Unit):
  match stop:
    case True{}: Socket.close(s)
    case False{}: k(s, u, "")

def Pol.sent(r: Socket & Result<&1, &1, (U32 & String) & String, Unit>, +stop: Bool, +u: U.Ui, k: Socket -> U.Ui -> String -> IO(Unit)) -> IO(Unit):
  (s, _) = r
  Pol.next(stop, s, u, k)

def Pol.answer(+st: Step, s: Socket, k: Socket -> U.Ui -> String -> IO(Unit)) -> IO(Unit):
  match st:
    case Step{+u, +reply, +stop}:
      IO.bind(Socket & Result<&1, &1, (U32 & String) & String, Unit>, Unit, TCP.send(s, reply ++ "\n"),
        r => Pol.sent(r, stop, u, k))

# a complete request: answer it, then go on with the state it left
def Pol.chunk(+whole: Bool, s: Socket, +u: U.Ui, +buf: String, k: Socket -> U.Ui -> String -> IO(Unit)) -> IO(Unit):
  match whole:
    case False{}: k(s, u, buf)
    case True{}:
      do IO<Unit>:
        +now : Nat <- IO.now()
        Pol.answer(Pol.step(u, String.trim(buf), now), s, k)

def Pol.got.k(res: Result<&1, &1, U32 & String, Maybe<&1, String>>, s: Socket, +u: U.Ui, +buf: String, k: Socket -> U.Ui -> String -> IO(Unit)) -> IO(Unit):
  match res:
    case Fail{_}: Socket.close(s)
    case Done{None{}}: Socket.close(s)
    case Done{Some{+chunk}}: Pol.chunk(String.ends_with(chunk, "\n"), s, u, buf ++ chunk, k)

def Pol.got(r: Socket & Result<&1, &1, U32 & String, Maybe<&1, String>>, +u: U.Ui, +buf: String, k: Socket -> U.Ui -> String -> IO(Unit)) -> IO(Unit):
  (s, res) = r
  Pol.got.k(res, s, u, buf, k)

def Pol.loop(fuel: Nat, s: Socket, +u: U.Ui, +buf: String) -> IO(Unit):
  match fuel:
    case 0n: Socket.close(s)
    case 1n+f:
      IO.bind(Socket & Result<&1, &1, U32 & String, Maybe<&1, String>>, Unit, TCP.recv(s, 1048576),
        r => Pol.got(r, u, buf, s2 => u2 => b2 => Pol.loop(f, s2, u2, b2)))

def Pol.accepted(r: Listener & Result<&1, &1, U32 & String, Socket>, +now: Nat) -> IO(Unit):
  (l, res) = r
  match res:
    case Done{s}:
      do IO<Unit>:
        Listener.close(l)
        Pol.loop(Pol.fuel(), s, U.Ui.init(now), "")
    case Fail{(code, msg)}:
      IO.die(Unit, code, msg)

def Pol.main(+xs: List<&2, String>, +now: Nat) -> IO(Unit):
  do IO<Unit>:
    l : Listener <- IO.try(Listener, TCP.listen("127.0.0.1", U32.from_nat(Maybe.default(&2, Nat, Nat.read(Pol.arg(xs, 1n)), 0n))))
    IO.print("bp-policy listening")
    IO.bind(Listener & Result<&1, &1, U32 & String, Socket>, Unit, TCP.accept(l), r => Pol.accepted(r, now))

def main() -> IO(Unit):
  do IO<Unit>:
    a : List<&1, String> <- IO.args()
    +xs : List<&2, String> = Pol.copy(a)
    +now : Nat <- IO.now()
    Pol.main(xs, now)
