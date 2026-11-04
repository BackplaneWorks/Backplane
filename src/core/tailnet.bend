import Base
import ./json.bend as J
import ./cbor.bend as CB
import ./utf8.bend as U

# Machines on the tailnet. Every machine the same person owns may run a
# Backplane hub; each hub finds the others (`tailscale status --json`,
# then a GET /hello on each online peer). Every client presents the
# pairing token or a paired device's credential (core/auth.bend); the
# owner's machines (Tailscale's `whois`) may only ask for an invitation
# (Auth.owner.pair).

# the login that owns a node, from `tailscale whois --json <ip>`
def Tailnet.login(+whois: String) -> String:
  J.Json.get_str(J.Json.get_obj(Maybe.default(&2, J.Json, J.Json.parse(whois), J.Null{}), "UserProfile"), "LoginName", "")

# a peer is let in when it has an owner and that owner is ours (tagged
# devices have no person behind them)
def Tailnet.trusts(+owner: String, +login: String) -> Bool:
  Bool.and(Bool.and(Bool.not(String.is_empty(owner)), String.eq(owner, login)), Bool.not(String.eq(login, "tagged-devices")))

# The address a hub names itself by when it asks another of the owner's
# machines to pair (server.bend's Pair.go): the one its machines list found
# for it only while it is on the tailnet. A hub started with --no-tailscale
# pairs only with the hubs BACKPLANE_PEERS names, which reach it where it
# listens; its machines list may still carry the tailnet's address of the
# machine it runs on, which those hubs never found, so they refused it (403
# "not one of the machines this hub found"). "" lets the caller use the
# listen address (Pair.here).
def Tailnet.announce(on: Bool, +url: String) -> String:
  match on:
    case True{}:
      url
    case False{}:
      ""

# the pairing link a phone or browser pastes (the Settings screen shows
# it): the hub's address with the token after "#", so it never reaches a
# server's logs
def Tailnet.link(+base: String, +token: String) -> String:
  base ++ "/#token=" ++ token

# What the hub knows of its tailnet side, one string: the state ("" not yet
# known, "unavailable" tailscale gave no address or the bind failed,
# "reachable" the hub answers on its tailnet address, through its own
# listener or because its main one already covers it), a newline, then the
# pairing link ("" while there is none). The two are separate facts: a hub
# started with --host 0.0.0.0 is reachable and has no pairing link.
def Tailnet.report(+state: String, +link: String) -> String:
  state ++ "\n" ++ link

def Tailnet.report.state(+r: String) -> String:
  Maybe.default(&2, String, List.head(&2, String, String.split(r, '\n')), "")

def Tailnet.report.link(+r: String) -> String:
  Maybe.default(&2, String, List.head(&2, String, List.drop(&2, String, String.split(r, '\n'), 1n)), "")

# A machine a client can switch to
type Machine is Data:
  Machine{name: String, ip: String, online: Bool}

def Tailnet.first_ip(ips: List<&2, J.Json>) -> String:
  match ips:
    case J.Str{s} <> _:
      s
    case _:
      ""

def Tailnet.host(+p: J.Json) -> String:
  J.Json.get_str(p, "HostName", Maybe.default(&2, String, List.head(&2, String, String.split(J.Json.get_str(p, "DNSName", ""), '.')), ""))

def Tailnet.peer.k(mine: Bool, +p: J.Json, rest: List<&2, Machine>) -> List<&2, Machine>:
  match mine:
    case True{}:
      Machine{Tailnet.host(p), Tailnet.first_ip(J.Json.elems(J.Json.get_obj(p, "TailscaleIPs"))), J.Json.get_bool(p, "Online", False{})} <> rest
    case False{}:
      rest

def Tailnet.peers.go(fs: J.Json, +uid: String, acc: List<&2, Machine>) -> List<&2, Machine>:
  match fs:
    case J.Field{_, +p, t}:
      Tailnet.peers.go(t, uid, Tailnet.peer.k(Bool.and(Bool.not(String.is_empty(uid)), String.eq(J.Json.as_raw(J.Json.get(p, "UserID"), ""), uid)), p, acc))
    case _:
      acc

def Tailnet.peers.of(o: J.Json, +uid: String) -> List<&2, Machine>:
  match o:
    case J.Obj{fs}:
      List.reverse(&2, Machine, Tailnet.peers.go(fs, uid, Nil{}))
    case _:
      Nil{}

# the owner's other machines, from `tailscale status --json`
def Tailnet.peers(+status: String) -> List<&2, Machine>:
  Tailnet.peers.of(J.Json.get_obj(Maybe.default(&2, J.Json, J.Json.parse(status), J.Null{}), "Peer"),
    J.Json.as_raw(J.Json.get(J.Json.get_obj(Maybe.default(&2, J.Json, J.Json.parse(status), J.Null{}), "Self"), "UserID"), ""))

def Tailnet.online.k(on: Bool, m: Machine, rest: List<&2, Machine>) -> List<&2, Machine>:
  match on:
    case True{}:
      m <> rest
    case False{}:
      rest

# only machines that are up (and have an address) are worth asking
def Tailnet.online(ms: List<&2, Machine>) -> List<&2, Machine>:
  match ms:
    case Nil{}:
      Nil{}
    case Machine{n, +ip, +on} <> t:
      Tailnet.online.k(Bool.and(on, Bool.not(String.is_empty(ip))), Machine{n, ip, on}, Tailnet.online(t))

# What the machines list (server.bend's Machines.loop) is built from: the
# hub's `tailscale status --json`, or nothing for a hub started with
# --no-tailscale (on False). Such a hub neither runs Tailscale nor asks the
# owner's tailnet machines GET /hello: it asks only the hubs BACKPLANE_PEERS
# names, and names itself by its loopback address (Machines.me), so a
# throwaway hub never reaches the owner's real machines and never waits on
# one that does not answer.
def Tailnet.seen(on: Bool, +status: String) -> String:
  match on:
    case True{}:
      status
    case False{}:
      ""

# the tailnet machines the machines list asks GET /hello
def Tailnet.asked(on: Bool, +status: String) -> List<&2, Machine>:
  Tailnet.online(Tailnet.peers(Tailnet.seen(on, status)))

# the tailnet's MagicDNS domain ("box.tail1.ts.net." → "tail1.ts.net"):
# every machine on it is named <host>.<domain>
def Tailnet.domain.of(+dns: String) -> String:
  String.join(List.drop(&2, String, String.split(Bool.pick(String, String.ends_with(dns, "."), String.take(dns, Nat.sub(String.length(dns), 1n)), dns), '.'), 1n), ".")

def Tailnet.domain(+status: String) -> String:
  Tailnet.domain.of(J.Json.get_str(J.Json.get_obj(Maybe.default(&2, J.Json, J.Json.parse(status), J.Null{}), "Self"), "DNSName", ""))

def Tailnet.self.of(+p: J.Json) -> Machine:
  Machine{Tailnet.host(p), Tailnet.first_ip(J.Json.elems(J.Json.get_obj(p, "TailscaleIPs"))), True{}}

# this node itself (its name and first address)
def Tailnet.self(+status: String) -> Machine:
  Tailnet.self.of(J.Json.get_obj(Maybe.default(&2, J.Json, J.Json.parse(status), J.Null{}), "Self"))

# Another hub's GET /hello
# ------------------------

# what this hub answers: its version, as CBOR
def Hello.answer(v: String) -> List<&2, U32>:
  CB.Cbor.encode(J.Json.obj([J.KV{"backplane", J.Str{v}}]))

# the same with how many threads run or wait on work ("busy"; an install
# waits for them: scripts/dev.sh install)
def Hello.answer.b(v: String, +busy: Nat) -> List<&2, U32>:
  CB.Cbor.encode(J.Json.obj([J.KV{"backplane", J.Str{v}}, J.KV{"busy", J.Json.nat(busy)}]))

# the same with the commit the build was made from (core/version.bend's
# Version.commit: a main-track build reports the release version beside it)
def Hello.answer.c(v: String, +busy: Nat, c: String) -> List<&2, U32>:
  CB.Cbor.encode(J.Json.obj([J.KV{"backplane", J.Str{v}}, J.KV{"busy", J.Json.nat(busy)}, J.KV{"commit", J.Str{c}}]))

def Hello.byte(+w: String, +rest: List<&2, U32>) -> List<&2, U32>:
  Bool.pick(List<&2, U32>, String.is_empty(w), rest, U32.from_nat(Maybe.default(&2, Nat, Nat.read(w), 0n)) <> rest)

def Hello.bytes.go(ws: List<&2, String>, acc: List<&2, U32>) -> List<&2, U32>:
  match ws:
    case Nil{}:
      acc
    case +w <> t:
      Hello.bytes.go(t, Hello.byte(w, acc))

# the bytes of an answer ("163 105 ...")
def Hello.bytes(+text: String) -> List<&2, U32>:
  List.reverse(&2, U32, Hello.bytes.go(String.split(String.trim(text), ' '), Nil{}))

# the version a /hello answer names: CBOR, or JSON from a hub that has
# not been updated yet (it only reads that; it never sends JSON)
def Hello.version.k(+cbor: String, +bs: List<&2, U32>) -> String:
  Bool.pick(String, String.is_empty(cbor),
    J.Json.get_str(Maybe.default(&2, J.Json, J.Json.parse(U.Utf8.decode(bs)), J.Null{}), "backplane", ""), cbor)

def Hello.version(+bs: List<&2, U32>) -> String:
  Hello.version.k(J.Json.get_str(Maybe.default(&2, J.Json, CB.Cbor.decode(bs), J.Null{}), "backplane", ""), bs)

# Where phones reach this hub over HTTPS
# --------------------------------------
# `tailscale serve status --json` lists "Web" entries by "host:port"; the
# one whose "/" handler proxies to the hub's port is its HTTPS address
# ("https://host", with ":port" unless it is 443).

def Tailnet.serve.to(+proxy: String, +port: String) -> Bool:
  Bool.or(String.ends_with(proxy, ":" ++ port), String.ends_with(proxy, ":" ++ port ++ "/"))

def Tailnet.serve.url(+hp: String) -> String:
  Bool.pick(String, String.ends_with(hp, ":443"), "https://" ++ String.take(hp, Nat.sub(String.length(hp), 4n)), "https://" ++ hp)

def Tailnet.serve.pick(+acc: String, hit: Bool, +hp: String) -> String:
  Bool.pick(String, String.is_empty(acc), Bool.pick(String, hit, Tailnet.serve.url(hp), ""), acc)

def Tailnet.serve.go(fs: J.Json, +port: String, +acc: String) -> String:
  match fs:
    case J.Field{+hp, +w, t}:
      Tailnet.serve.go(t, port, Tailnet.serve.pick(acc, Tailnet.serve.to(J.Json.get_str(J.Json.get_obj(J.Json.get_obj(w, "Handlers"), "/"), "Proxy", ""), port), hp))
    case _:
      acc

def Tailnet.serve.of(o: J.Json, +port: String) -> String:
  match o:
    case J.Obj{fs}:
      Tailnet.serve.go(fs, port, "")
    case _:
      ""

# the hub's HTTPS address, "" when no entry proxies to its port
def Tailnet.serve(+status: String, +port: String) -> String:
  Tailnet.serve.of(J.Json.get_obj(Maybe.default(&2, J.Json, J.Json.parse(status), J.Null{}), "Web"), port)

# Pairing's schedule
# ------------------
# What server.bend's Machines.loop does each period, in order: find the
# machines (tailscale status, GET /hello) and hand the list to the hub,
# ask the hub to pair at once (PairDue: the list is already there, sent
# first on the same channel), wait half a period, ask again, wait again.
# Nothing else asks the hub to pair, so two asks are at least half a
# period (60 s) apart however long finding the machines takes (a late
# first list included), and the first ask follows the first list at once.
# An ask and the link it makes take at most 15 s each, so asks never
# overlap. Tailnet.asks runs this plan on given discovery times (laws:
# test/tailnet_seen_test.bend).
type PStep is Data:
  PFind{}
  PAsk{}
  PWait{ms: Nat}

def Pairing.half() -> Nat:
  60000n

def Pairing.plan() -> List<&2, PStep>:
  [PFind{}, PAsk{}, PWait{Pairing.half()}, PAsk{}, PWait{Pairing.half()}]

# one period of a plan from time t, its finding taking f: the times it asks
def Pairing.period.asks(ps: List<&2, PStep>, +f: Nat, +t: Nat) -> List<&2, Nat>:
  match ps:
    case Nil{}:
      Nil{}
    case PFind{} <> rest:
      Pairing.period.asks(rest, f, Nat.add(t, f))
    case PAsk{} <> rest:
      t <> Pairing.period.asks(rest, f, t)
    case PWait{ms} <> rest:
      Pairing.period.asks(rest, f, Nat.add(t, ms))

# and when it ends
def Pairing.period.end(ps: List<&2, PStep>, +f: Nat, +t: Nat) -> Nat:
  match ps:
    case Nil{}:
      t
    case PFind{} <> rest:
      Pairing.period.end(rest, f, Nat.add(t, f))
    case PAsk{} <> rest:
      Pairing.period.end(rest, f, t)
    case PWait{ms} <> rest:
      Pairing.period.end(rest, f, Nat.add(t, ms))

# the times (ms from start) Pairing.plan asks, when each period's finding
# takes the given time; one period per entry of finds
def Tailnet.asks(finds: List<&2, Nat>, +t: Nat) -> List<&2, Nat>:
  match finds:
    case Nil{}:
      Nil{}
    case +f <> rest:
      List.append(&2, Nat, Pairing.period.asks(Pairing.plan(), f, t), Tailnet.asks(rest, Pairing.period.end(Pairing.plan(), f, t)))

# every two asks at least `gap` apart (asks in time order)
def Tailnet.apart.k(+a: Nat, ts: List<&2, Nat>, +gap: Nat) -> Bool:
  match ts:
    case Nil{}:
      True{}
    case +b <> rest:
      Bool.and(Nat.is_ge(Nat.sub(b, a), gap), Tailnet.apart.k(b, rest, gap))

def Tailnet.apart(ts: List<&2, Nat>, +gap: Nat) -> Bool:
  match ts:
    case Nil{}:
      True{}
    case +a <> rest:
      Tailnet.apart.k(a, rest, gap)
