import Base
import ./check.bend as C
import ../src/core/tailnet.bend as TN

# A hub started with --no-tailscale never reaches the owner's tailnet
# machines for its machines list (server.bend's Machines.loop; the 0.14.0
# RC4 telemetry gate saw such a hub curl its 100.x peers): it asks none of
# them GET /hello, and names itself by loopback, not its tailnet address.

# `tailscale status --json` with two of the owner's machines online, one
# offline and one of someone else's
def status() -> String:
  "{\"Self\":{\"UserID\":1,\"HostName\":\"laptop\",\"DNSName\":\"laptop.tail1.ts.net.\",\"TailscaleIPs\":[\"100.1.1.1\"],\"Online\":true},\"Peer\":{"
    ++ "\"k1\":{\"UserID\":1,\"HostName\":\"desk\",\"DNSName\":\"desk.tail1.ts.net.\",\"TailscaleIPs\":[\"100.2.2.2\"],\"Online\":true},"
    ++ "\"k2\":{\"UserID\":1,\"HostName\":\"box\",\"DNSName\":\"box.tail1.ts.net.\",\"TailscaleIPs\":[\"100.3.3.3\"],\"Online\":true},"
    ++ "\"k3\":{\"UserID\":1,\"HostName\":\"old\",\"DNSName\":\"old.tail1.ts.net.\",\"TailscaleIPs\":[\"100.4.4.4\"],\"Online\":false},"
    ++ "\"k4\":{\"UserID\":2,\"HostName\":\"friend\",\"DNSName\":\"friend.tail1.ts.net.\",\"TailscaleIPs\":[\"100.5.5.5\"],\"Online\":true}}}"

def ips(ms: List<&2, TN.Machine>) -> String:
  match ms:
    case Nil{}:
      ""
    case TN.Machine{n, ip, _} <> t:
      n ++ "=" ++ ip ++ "|" ++ ips(t)

def me(m: TN.Machine) -> String:
  match m:
    case TN.Machine{n, ip, _}:
      n ++ "=" ++ ip

def nats(ts: List<&2, Nat>) -> String:
  match ts:
    case Nil{}:
      ""
    case t <> rest:
      Nat.show(t) ++ "|" ++ nats(rest)

def yes(b: Bool) -> String:
  Bool.pick(String, b, "yes", "no")

# A hub whose handling stalls (TN.Pairing.ask): the machines loop posts a
# round, the hub holds it 300 ms and its ask fails, then acks; the loop's
# next round is posted only after that ack, and is the retry
def say(+log: Chan(String), +x: String) -> IO(Unit):
  do IO<Unit>:
    ok : Result<&1, &1, String, Unit> <- Chan.send(String, log, x)
    return Unit{}

def post(+log: Chan(String), +inbox: Chan(Unit), +n: String) -> IO(Unit):
  do IO<Unit>:
    say(log, "post" ++ n)
    ok : Result<&1, &1, Unit, Unit> <- Chan.send(Unit, inbox, Unit{})
    return Unit{}

def loop2(+log: Chan(String), +inbox: Chan(Unit), +ack: Chan(Unit)) -> IO(Unit):
  do IO<Unit>:
    TN.Pairing.ask(u => post(log, inbox, "1"), ack)
    say(log, "back1")
    TN.Pairing.ask(u => post(log, inbox, "2"), ack)
    say(log, "back2")

# the round's one machine: 300 ms, then the ask fails
def fail1(+log: Chan(String)) -> IO(Unit):
  do IO<Unit>:
    IO.sleep(300)
    say(log, "failed1")

def hub2(+log: Chan(String), +inbox: Chan(Unit), +ack: Chan(Unit)) -> IO(Unit):
  do IO<Unit>:
    a : Maybe<&1, Unit> <- Chan.recv(Unit, inbox)
    say(log, "due1")
    TN.Pairing.round(["m"], u => fail1(log), ack)
    b : Maybe<&1, Unit> <- Chan.recv(Unit, inbox)
    say(log, "due2")
    o2 : Result<&1, &1, Unit, Unit> <- Chan.send(Unit, ack, Unit{})
    return Unit{}

def got(m: Maybe<&1, String>) -> String:
  match m:
    case Some{x}:
      x
    case None{}:
      "?"

def lines(n: Nat, +log: Chan(String)) -> IO(String):
  match n:
    case 0n:
      IO.pure(String, "")
    case 1n+m:
      IO.bind(Maybe<&1, String>, String, Chan.recv(String, log), x => IO.bind(String, String, lines(m, log), rest => IO.pure(String, got(x) ++ "|" ++ rest)))

def stalled() -> IO(String):
  do IO<String>:
    +log : Chan(String) <- Chan.new(String, 16)
    +inbox : Chan(Unit) <- Chan.new(Unit, 16)
    +ack : Chan(Unit) <- Chan.new(Unit, 1)
    IO.spawn(Unit, loop2(log, inbox, ack))
    IO.spawn(Unit, hub2(log, inbox, ack))
    lines(7n, log)

# a round with two slow machines and a healthy one (TN.Pairing.round): the
# healthy one is through at once, the round's ack waits for the slowest
def ask3(+log: Chan(String), +u: String) -> IO(Unit):
  do IO<Unit>:
    IO.sleep(Bool.pick(U32, String.eq(u, "slow1"), 400, Bool.pick(U32, String.eq(u, "slow2"), 700, 0)))
    say(log, u)

def acked(+log: Chan(String), +ack: Chan(Unit)) -> IO(Unit):
  do IO<Unit>:
    m : Maybe<&1, Unit> <- Chan.recv(Unit, ack)
    say(log, "ack")

def peers3() -> IO(String):
  do IO<String>:
    +log : Chan(String) <- Chan.new(String, 16)
    +ack : Chan(Unit) <- Chan.new(Unit, 1)
    IO.spawn(Unit, acked(log, ack))
    IO.spawn(Unit, TN.Pairing.round(["slow1", "slow2", "healthy"], u => ask3(log, u), ack))
    lines(4n, log)

def main() -> IO(Unit):
  do IO<Unit>:
    +order : String <- stalled()
    +three : String <- peers3()
    C.Check.str("slow machines never hold up a healthy one; the round ends after the slowest", three, "healthy|slow1|slow2|ack|")
    C.Check.str("a stalled round: the next is posted only after it ends (failed), then retried", order, "post1|due1|failed1|back1|post2|due2|back2|")
    C.Check.str("on the tailnet: the owner's online machines are asked", ips(TN.Tailnet.asked(True{}, status())), "desk=100.2.2.2|box=100.3.3.3|")
    C.Check.str("--no-tailscale: no tailnet machine is asked", ips(TN.Tailnet.asked(False{}, status())), "")
    C.Check.str("--no-tailscale: the status is not used at all", TN.Tailnet.seen(False{}, status()), "")
    C.Check.str("--no-tailscale: self has no tailnet address (Machines.me uses loopback)", me(TN.Tailnet.self(TN.Tailnet.seen(False{}, status()))), "=")
    C.Check.str("--no-tailscale: no MagicDNS domain", TN.Tailnet.domain(TN.Tailnet.seen(False{}, status())), "")
    C.Check.str("on the tailnet: self and domain as before", me(TN.Tailnet.self(TN.Tailnet.seen(True{}, status()))) ++ " " ++ TN.Tailnet.domain(TN.Tailnet.seen(True{}, status())), "laptop=100.1.1.1 tail1.ts.net")
    # pairing's schedule (TN.Pairing.plan, run by server.bend's
    # Machines.loop): the first ask right after the first list, asks a
    # minute apart however late finding the machines is
    C.Check.str("the first ask follows the first list at once", nats(TN.Tailnet.asks([580n], 0n)), "580|60580|")
    C.Check.str("a slow first list (70 s) moves every ask", nats(TN.Tailnet.asks([70000n, 3000n], 0n)), "70000|130000|193000|253000|")
    C.Check.str("a list found just before the old minute tick never doubles an ask", nats(TN.Tailnet.asks([59999n, 0n], 0n)), "59999|119999|179999|239999|")
    C.Check.str("asks stay a minute apart, any delays", yes(TN.Tailnet.apart(TN.Tailnet.asks([59999n, 13000n, 0n, 125000n, 1n, 3003n], 0n), 60000n)), "yes")
    C.Check.str("and the check sees a pair closer than that", yes(TN.Tailnet.apart([0n, 60000n, 119999n], 60000n)), "no")
    C.Check.str("only the plan asks: two asks, two waits of half a period each, per find", nats(TN.Tailnet.asks([0n], 0n)), "0|60000|")
