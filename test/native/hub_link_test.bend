import Base
import ../check.bend as C
import ../../src/app/link.bend as L
import ../../src/server/server.bend as S
import ../../src/server/host.bend as H
import ../../src/core/json.bend as J
import ../../src/core/utf8.bend as U
import ../../src/server/argv.bend as AV

# Native network regression probe (temporary hubs only):
#   scripts/build-js.sh test/native/hub_link_test.bend build/hub_link_test
#   build/hub_link_test http://localhost:3792 [SINCE ORIGIN]
#   build/hub_link_test http://localhost:3794  (a silent HTTP peer)
#   PATH=<fake tailscale that prints an owner then hangs>:$PATH ... whois
# The link must yield a log or an explicit failure, never wait indefinitely.
# The whois probe supplies no pairing token: partial stdout from a timed-out
# command must never authenticate it, even when it names the right owner.

def Probe.object(text: String) -> J.Json:
  Maybe.default(&2, J.Json, J.Json.parse(text), J.Null{})

def Probe.done(+o: J.Json) -> Bool:
  Bool.or(String.eq(J.Json.get_str(o, "t", ""), "log"), Bool.not(String.is_empty(J.Json.get_str(o, "error", ""))))

def Probe.stop(done: Bool, text: String, k: Unit -> IO(String)) -> IO(String):
  match done:
    case True{}: IO.pure(String, text)
    case False{}: k(Unit{})

def Probe.wire(m: Maybe<&1, S.Wire>, k: Unit -> IO(String)) -> IO(String):
  match m:
    case Some{S.WText{+text}}:
      Probe.stop(Probe.done(Probe.object(text)), text, k)
    case Some{_}: k(Unit{})
    case None{}: IO.pure(String, "closed without a result")

def Probe.read(fuel: Nat, +out: Chan(S.Wire)) -> IO(String):
  match fuel:
    case 0n: IO.pure(String, "too many messages before log")
    case 1n+k:
      IO.bind(Maybe<&1, S.Wire>, String, Chan.recv(S.Wire, out), m => Probe.wire(m, u => Probe.read(k, out)))

def Probe.link(url: String, since: Nat, origin: String) -> IO(Unit):
  do IO<Unit>:
    +inbox : Chan(S.Msg) <- L.Link.start(url)
    +out : Chan(S.Wire) <- Chan.new(S.Wire, 8192)
    +t0 : Nat <- IO.now()
    ok : Result<&1, &1, S.Msg, Unit> <- Chan.send(S.Msg, inbox, S.Join{0, out, since, origin, 1, 0, False{}})
    +text : String <- Probe.read(10n, out)
    +t1 : Nat <- IO.now()
    IO.print("link elapsed ms=" ++ Nat.show(Nat.sub(t1, t0)) ++ " " ++ text)
    C.Check.str("link returns log or explicit unreachable", Bool.show(Probe.done(Probe.object(text))), "True")
    C.Check.str("connect and handshake are bounded", Bool.show(Nat.is_le(Nat.sub(t1, t0), 11000n)), "True")
    left : Result<&1, &1, S.Msg, Unit> <- Chan.send(S.Msg, inbox, S.Leave{0})
    return Unit{}

def Probe.accepted(r: Listener & Result<&1, &1, U32 & String, Socket>) -> IO(Unit):
  (listener, result) = r
  match result:
    case Done{s}:
      do IO<Unit>:
        Listener.close(listener)
        +inbox : Chan(S.Msg) <- Chan.new(S.Msg, 16)
        S.Accept.whois((s, "127.0.0.1"), "owner@test", 7, "", inbox, "pairing-token")
    case Fail{_}:
      Listener.close(listener)

def Probe.response(r: Socket & Result<&1, &1, U32 & String, List<&2, U32>>, +t0: Nat) -> IO(Unit):
  (s, response) = r
  do IO<Unit>:
    Socket.close(s)
    +t1 : Nat <- IO.now()
    +text : String = U.Utf8.decode(Result.default(&1, &1, U32 & String, List<&2, U32>, response, Nil{}))
    C.Check.str("timed out identity cannot authenticate", Bool.show(String.starts_with(text, "HTTP/1.1 401 Unauthorized")), "True")
    C.Check.str("whois timeout exercised", Bool.show(Nat.is_ge(Nat.sub(t1, t0), 4500n)), "True")
    C.Check.str("whois wait is bounded", Bool.show(Nat.is_le(Nat.sub(t1, t0), 6500n)), "True")
    IO.print("whois elapsed ms=" ++ Nat.show(Nat.sub(t1, t0)) ++ " " ++ text)

def Probe.sent(r: Socket & Result<&1, &1, U32 & String, Unit>, t0: Nat) -> IO(Unit):
  (s, _) = r
  IO.bind(Socket & Result<&1, &1, U32 & String, List<&2, U32>>, Unit, H.Sock.recv_bytes(s, 65536), r2 => Probe.response(r2, t0))

def Probe.whois() -> IO(Unit):
  do IO<Unit>:
    listener : Listener <- IO.try(Listener, TCP.listen("0.0.0.0", 3891))
    IO.spawn(Unit, IO.bind(Listener & Result<&1, &1, U32 & String, Socket>, Unit, TCP.accept(listener), Probe.accepted))
    s : Socket <- IO.try(Socket, TCP.connect("127.0.0.1", 3891))
    t0 : Nat <- IO.now()
    IO.bind(Socket & Result<&1, &1, U32 & String, Unit>, Unit, H.Sock.send_text(s, "GET /ws HTTP/1.1\r\nHost: localhost\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Version: 13\r\n\r\n"), r => Probe.sent(r, t0))

def Probe.args(xs: List<&1, String>) -> IO(Unit):
  match xs:
    case "whois" <> _: Probe.whois()
    case url <> count <> origin <> _:
      Probe.link(url, Maybe.default(&2, Nat, Nat.read(count), 0n), origin)
    case url <> _: Probe.link(url, 0n, "")
    case Nil{}: IO.print("pass a temporary hub URL or whois")

def main() -> IO(Unit):
  IO.bind(List<&1, String>, Unit, AV.Argv.args(), Probe.args)
