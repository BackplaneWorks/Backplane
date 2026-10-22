import Base
import ./check.bend as C
import ../src/core/model.bend as M
import ../src/core/hub.bend as H
import ../src/core/handoff.bend as HD
import ../src/core/mcp.bend as MC
import ../src/core/refs.bend as RF
import ../src/core/emb.bend as EMB
import ../src/core/json.bend as J

# The hub's bounded fold (model.bend's Win.*) against the whole one: every
# reader the hub has gives the same answer (laws win_*).

def full() -> M.State:
  M.Win.demo(0n)

def win() -> M.State:
  M.Win.demo(1n)

def n(+x: Nat) -> String:
  Nat.show(x)

def ids(es: List<&2, M.Entry>) -> String:
  match es:
    case Nil{}:
      ""
    case +e <> t:
      M.Idx.id(e) ++ "," ++ ids(t)

def ans(a: H.Answer) -> String:
  match a:
    case H.Answer{x, t}:
      String.take(x, 10n) ++ "|" ++ t

def res(r: MC.Res) -> String:
  match r:
    case MC.Res{ok, j, _}:
      Bool.show(ok) ++ J.Json.show(j)

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("no log in a hub's state", n(List.length(&2, M.Entry, M.State.entries.all(win()))), "1")
    C.Check.str("the window was trimmed", ids(M.State.mine(win(), "t1")), "x4,x3,m3,a6,k1,x2,a5,a4,a3,a2,")
    C.Check.str("whole fold keeps all", n(List.length(&2, M.Entry, M.State.mine(full(), "t1"))), "15")
    C.Check.str("count", n(M.Switch.count(win(), "t1")), n(M.Switch.count(full(), "t1")))
    C.Check.str("count t2", n(M.Switch.count(win(), "t2")), n(M.Switch.count(full(), "t2")))
    C.Check.str("seen at the switch", M.Setting.get(win(), "seen.t1.claude", "?"), M.Setting.get(full(), "seen.t1.claude", "?"))
    C.Check.str("answer", ans(H.Answer.of(M.State.mine(win(), "t1"), "t1")), ans(H.Answer.of(M.State.mine(full(), "t1"), "t1")))
    C.Check.str("trigger", H.Lr.trig(M.State.mine(win(), "t1"), "t1"), H.Lr.trig(M.State.mine(full(), "t1"), "t1"))
    C.Check.str("last reply", MC.Mcp.last(M.State.mine(win(), "t1"), "t1"), MC.Mcp.last(M.State.mine(full(), "t1"), "t1"))
    C.Check.str("sum user", RF.Sum.last(win(), "t1", True{}), RF.Sum.last(full(), "t1", True{}))
    C.Check.str("sum reply", RF.Sum.last(win(), "t1", False{}), RF.Sum.last(full(), "t1", False{}))
    C.Check.str("first", MC.Mcp.first(M.State.own(win(), "t1"), "t1", ""), MC.Mcp.first(M.State.own(full(), "t1"), "t1", ""))
    C.Check.str("first is the first", MC.Mcp.first(M.State.own(win(), "t1"), "t1", ""), "first ask")
    C.Check.str("fork", HD.Hand.fork(M.State.own(win(), "t1"), "t1", "A"), HD.Hand.fork(M.State.own(full(), "t1"), "t1", "A"))
    C.Check.str("fork from the log", HD.Hand.fork(M.State.own(win(), "t1"), "t1", "A"), HD.Hand.fork(M.State.entries.all(full()), "t1", "A"))
    C.Check.str("btw", HD.Hand.btw(H.Hub.entries(win()), "t1", "A", "q"), HD.Hand.btw(H.Hub.entries(full()), "t1", "A", "q"))
    C.Check.str("switch", HD.Hand.switch(M.State.own(win(), "t1"), "t1", "codex", "claude", 11n), HD.Hand.switch(M.State.own(full(), "t1"), "t1", "codex", "claude", 11n))
    C.Check.str("switch all", HD.Hand.switch(M.State.own(win(), "t1"), "t1", "codex", "claude", 0n), HD.Hand.switch(M.State.own(full(), "t1"), "t1", "codex", "claude", 0n))
    C.Check.str("dup recent", Bool.show(H.Hub.dup(True{}, win(), "m3")), "True")
    C.Check.str("dup none", Bool.show(H.Hub.dup(True{}, win(), "zz")), "False")
    C.Check.str("term", EMB.Emb.sum(M.State.terms(win(), "t1"), "k1"), EMB.Emb.sum(M.State.mine(full(), "t1"), "k1"))
    C.Check.str("thread_read", res(MC.Mcp.read.k(True{}, win(), "t1", 5n)), res(MC.Mcp.read.k(True{}, full(), "t1", 5n)))
    C.Check.str("thread_read needs no store", Bool.show(MC.Mcp.read.need(win(), "t1", 5n)), "False")
    C.Check.str("thread_read needs the store", Bool.show(MC.Mcp.read.need(win(), "t1", 40n)), "True")
    C.Check.str("a client never does", Bool.show(MC.Mcp.read.need(full(), "t1", 40n)), "False")
    C.Check.str("thread_read from rows", res(MC.Mcp.read.rows(M.State.mine(full(), "t1"), "t1", 40n)), res(MC.Mcp.read.k(True{}, full(), "t1", 40n)))
    C.Check.str("page enough", Bool.show(M.Win.enough(M.State.mine(full(), "t1"), "t1", 1n)), "True")
    C.Check.str("loaded window", HD.Hand.fork(M.State.own(M.State.win.load(M.State.bounded(1n), "t1", M.State.mine(full(), "t1"), M.Switch.count(full(), "t1"), [M.Msg{"m1", "t1", M.User{}, "first ask", 2n}]), "t1"), "t1", "A"), HD.Hand.fork(M.State.own(full(), "t1"), "t1", "A"))
    C.Check.str("cost bound", Bool.show(Nat.is_le(M.Win.cost(M.Act{"x", "t", "error", "shell", "make", 1n}), 1n+String.length(HD.Hand.line(M.Act{"x", "t", "error", "shell", "make", 1n})))), "True")
