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

def main() -> IO(Unit):
  do IO<Unit>:
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
