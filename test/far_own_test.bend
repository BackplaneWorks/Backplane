import Base
import ./check.bend as C
import ../src/core/model.bend as M
import ../src/core/json.bend as J
import ../src/core/client.bend as U
import ../src/core/bot.bend as BT
import ../src/core/hub.bend as B
import ../src/core/bothub.bend as BH

# Which links are the owner's machines (client.bend's Far.mine, server.bend's
# Far.own): only what the hub recorded makes a link one ("peer.own.<link>":
# paired through /bots/pair, vouched for by Tailscale, or said by the owner),
# never the name and address the link gave of itself.
#   bend test/far_own_test.bend

# the owner's machines as this hub found them: "desk" at 100.2.0.1
def machines() -> J.Json:
  U.Mate.demo("http://100.1.0.1:3787")

# a link that says it is desk (its address and name): what an invite's
# holder may claim of itself on POST /bots/link
def claims() -> BT.Peer:
  BT.Peer{"p1", "desk", "http://100.2.0.1:3787", 5n, False{}}

def with(+how: String) -> M.State:
  M.State.setting(M.State.empty(), "peer.own.p1", how)

def linked(+how: String) -> M.State:
  M.State.apply(with(how), M.PeerSet{"p1", "desk", "http://100.2.0.1:3787", False{}, 5n})

def mine(+s: M.State) -> String:
  Bool.show(U.Far.mine(s, machines(), claims()))

def changes(st: B.Step) -> String:
  match st:
    case B.Step{cs, _}:
      Nat.show(List.length(&2, M.Change, cs))

def setting(st: B.Step, +k: String) -> String:
  match st:
    case B.Step{cs, _}:
      M.Setting.get(M.State.replay(cs, M.State.empty()), k, "")

def rows(rs: List<&2, U.MateRow>) -> String:
  match rs:
    case Nil{}:
      ""
    case U.MateRow{+n, _, +on, _, +ask} <> t:
      n ++ "|" ++ on ++ "|" ++ ask ++ ";" ++ rows(t)

def with_st(u: U.Ui, s: M.State) -> U.Ui:
  match u:
    case U.Ui{_, sel, dr, live, op, on, now, ad, pa, v, rid, w, er, note, nf, ob, tr}:
      U.Ui{s, sel, dr, live, op, on, now, ad, pa, v, rid, w, er, note, nf, ob, tr}

def ui(s: M.State) -> U.Ui:
  U.Ui.info_set(with_st(U.Ui.init(1n), s), "machines", J.Json.show(machines()))

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("a link that only says it is desk is not one of the owner's machines", mine(M.State.empty()), "False")
    C.Check.str("paired through /bots/pair", mine(with("pair")), "True")
    C.Check.str("vouched for by Tailscale", mine(with("tailnet")), "True")
    C.Check.str("the owner said it is theirs", mine(with("owner")), "True")
    C.Check.str("the owner said it is not theirs", mine(with("no")), "False")
    C.Check.str("verified, but none of the machines this hub found", Bool.show(U.Far.mine(with("tailnet"), machines(), BT.Peer{"p1", "box", "http://10.0.0.9:3787", 5n, False{}})), "False")
    C.Check.str("verified, but sync with it is off", Bool.show(U.Far.mine(M.State.setting(with("pair"), "sync.desk", "off"), machines(), claims())), "False")
    C.Check.str("verified, but revoked", Bool.show(U.Far.mine(with("pair"), machines(), BT.Peer{"p1", "desk", "http://100.2.0.1:3787", 5n, True{}})), "False")
    # a link the owner said is not theirs does not stand in for the machine:
    # the machine is still asked to pair, and still invited when it asks
    C.Check.str("a refused link does not keep desk from pairing", Nat.show(U.Mate.answer(U.Mate.demo("http://100.3.0.1:3787"), [claims()], with("no"), "desk", "http://100.2.0.1:3787")), "0")
    C.Check.str("an unanswered link still counts as linked (no second link meanwhile)", Nat.show(U.Mate.answer(U.Mate.demo("http://100.3.0.1:3787"), [claims()], M.State.empty(), "desk", "http://100.2.0.1:3787")), "2")
    # Settings asks about a link nothing verified, once: not after an answer
    C.Check.str("Settings asks about an unverified link", rows(U.Mate.rows(ui(linked("")))), "desk|on|p1;")
    C.Check.str("Settings does not ask about a verified one", rows(U.Mate.rows(ui(linked("tailnet")))), "desk|on|;")
    C.Check.str("Settings does not ask again once answered", rows(U.Mate.rows(ui(linked("no")))), "desk|on|;")
    # the answer is the hub's bots.peer_own (only for a live link), never
    # a client's setting.set
    +ask : J.Json = J.Json.obj([J.KV{"m", J.Str{"bots.peer_own"}}, J.KV{"id", J.Num{"1"}}, J.KV{"p", J.Json.obj([J.KV{"peer", J.Str{"p1"}}, J.KV{"own", J.Flag{True{}}}])}])
    C.Check.str("the owner's Mine is recorded", setting(BH.Bots.request(linked(""), 9n, 1, ask), "peer.own.p1"), "owner")
    +no : J.Json = J.Json.obj([J.KV{"m", J.Str{"bots.peer_own"}}, J.KV{"id", J.Num{"1"}}, J.KV{"p", J.Json.obj([J.KV{"peer", J.Str{"p1"}}, J.KV{"own", J.Flag{False{}}}])}])
    C.Check.str("the owner's Not mine is recorded", setting(BH.Bots.request(linked(""), 9n, 1, no), "peer.own.p1"), "no")
    C.Check.str("no answer for a link that is not there", changes(BH.Bots.request(M.State.empty(), 9n, 1, ask)), "0")
    C.Check.str("a client's setting.set cannot make a link the owner's",
      changes(B.Hub.setting(M.State.empty(), 9n, 1, "1", J.Json.obj([J.KV{"key", J.Str{"peer.own.p1"}}, J.KV{"value", J.Str{"owner"}}]))), "0")
    C.Check.str("other settings still set", changes(B.Hub.setting(M.State.empty(), 9n, 1, "1", J.Json.obj([J.KV{"key", J.Str{"sync.desk"}}, J.KV{"value", J.Str{"off"}}]))), "1")
    # the client's Mine / Not mine buttons ask the hub (and show at once)
    C.Check.str("Mine asks bots.peer_own", U.Far.own.how(U.Ui.state(U.Act.ui(U.Ui.act(ui(linked("")), "peer-own", "p1=1"))), "p1"), "owner")
    C.Check.str("Not mine asks bots.peer_own", U.Far.own.how(U.Ui.state(U.Act.ui(U.Ui.act(ui(linked("")), "peer-own", "p1=0"))), "p1"), "no")
    return Unit{}
