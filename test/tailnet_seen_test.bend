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

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("on the tailnet: the owner's online machines are asked", ips(TN.Tailnet.asked(True{}, status())), "desk=100.2.2.2|box=100.3.3.3|")
    C.Check.str("--no-tailscale: no tailnet machine is asked", ips(TN.Tailnet.asked(False{}, status())), "")
    C.Check.str("--no-tailscale: the status is not used at all", TN.Tailnet.seen(False{}, status()), "")
    C.Check.str("--no-tailscale: self has no tailnet address (Machines.me uses loopback)", me(TN.Tailnet.self(TN.Tailnet.seen(False{}, status()))), "=")
    C.Check.str("--no-tailscale: no MagicDNS domain", TN.Tailnet.domain(TN.Tailnet.seen(False{}, status())), "")
    C.Check.str("on the tailnet: self and domain as before", me(TN.Tailnet.self(TN.Tailnet.seen(True{}, status()))) ++ " " ++ TN.Tailnet.domain(TN.Tailnet.seen(True{}, status())), "laptop=100.1.1.1 tail1.ts.net")
