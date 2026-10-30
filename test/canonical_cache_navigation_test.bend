import Base
import ./check.bend as C
import ./signal_text_actor_test.bend as F
import ../src/server/thin-actor.bend as A
import ../src/server/thin-signals.bend as K
import ../src/server/thin-signal-text.bend as X
import ../src/server/thin-view.bend as V
import ../src/core/model.bend as M
import ../src/core/client.bend as U
import ../src/core/json.bend as J

def yes(b: Bool) -> String:
  Bool.pick(String, b, "yes", "no")

def count(+a: A.Actor) -> String:
  Nat.show(Nat.add(List.length(&2, A.Memo, A.Signals.keep(A.Sidebar.memos(a))),
    Nat.add(List.length(&2, A.Memo, A.SideIndex.keep(A.Sidebar.memos(a))), List.length(&2, A.Memo, A.SignalText.keep(A.Sidebar.memos(a))))))

def request(target: String) -> J.Json:
  J.Json.obj([J.KV{"target", V.Target.json("thread", target)}, J.KV{"session", V.Route.pack(F.ui(F.state(), 11n))}])

def refresh(a: A.Actor, target: String, view: String) -> A.Actor:
  A.Actor.refresh([a], True{}, F.state(), J.Json.obj(Nil{}), J.Json.obj(Nil{}), 11n, request(target), view)

def many(+n: Nat, m: Map<&2, Nat>) -> Map<&2, Nat>:
  match n:
    case 0n: m
    case 1n+k: many(k, Map.set(&2, Nat, m, Nat.show(k), 1n))

def tainted(a: A.Actor) -> A.Actor:
  match a:
    case A.Actor{u, view, target, pending, reads, dirty, memo}:
      A.Actor{u, view, target, pending, reads, dirty,
        A.Memo{"PRIVATE picture", 10n, False{}, J.Str{"private body"}} <>
        A.Sidebar{"PRIVATE key", J.Str{"private sidebar"}} <>
        A.SidebarImage{"PRIVATE image", J.Str{"private rendered node"}} <>
        A.SidebarWire{"PRIVATE wire", "private authority"} <>
        A.SidebarSent{"PRIVATE scope", "private token", J.Str{"private sent node"}} <>
        A.BodyIssued{[J.Str{"private issued body"}]} <> memo}

def total(a: A.Actor) -> String:
  Nat.show(List.length(&2, A.Memo, A.Sidebar.memos(a)))

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("switch retains exactly three canonical derivations", count(refresh(F.seed(), "other", "view")), "3")
    C.Check.str("view replacement retains exactly three canonical derivations", count(refresh(F.seed(), "self", "new-view")), "3")
    C.Check.str("view move cannot carry pictures, wire receipts or issued bodies", total(refresh(tainted(F.seed()), "other", "new-view")), "3")
    C.Check.str("new connection cannot acquire another actor cache", count(A.Actor.refresh(Nil{}, True{}, F.state(), J.Json.obj(Nil{}), J.Json.obj(Nil{}), 11n, request("other"), "new-connection")), "0")
    C.Check.str("5750-thread scalar catalog has a reusable bounded derivation", yes(K.SignalCache.bounded(K.SignalCache.build("epoch", 7n, many(5750n, Map.new(&2, Nat))))), "yes")
    C.Check.str("5750-thread text catalog has a reusable bounded derivation", yes(X.SignalTextCache.bounded(X.SignalTextCache.build("epoch", 7n, True{}, many(5750n, Map.new(&2, Nat))))), "yes")
    C.Check.str("carried scalar cannot hit a different epoch", yes(K.SignalCache.hit(K.SignalCache.obtain("epoch", F.state(), None{}), "other-epoch", M.State.seq(F.state()))), "no")
    C.Check.str("carried text cannot hit different online visibility", yes(X.SignalTextCache.hit(F.text(), "epoch", M.State.seq(F.state()), False{})), "no")
