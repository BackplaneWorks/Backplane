import Base
import ./check.bend as C
import ../src/core/json.bend as J
import ../src/core/model.bend as M
import ../src/core/signal.bend as SG
import ../src/core/order.bend as OR

# A buy thread's order card (core/order.bend's Order.card) and the quote on
# its way: from the person's Send until the quote is posted the thread
# waits (the setting Order.wait.key: Work.rows, the signal's waiting, never
# Finished) and the card says the quote is coming; once quoted it has the
# total, what it covers about customs and Buy now; order_status says the
# files are sent instead of guessing at problems.

def log(+more: List<&2, M.Change>) -> List<&2, M.Change>:
  List.append(&2, M.Change, [M.ProjectCreated{"p1", "lamp", "/tmp/lamp", 1n},
   M.ThreadCreated{"t1", "p1", "Buy: lamp", M.Local{}, "", "", "claude", "", 1n},
   M.SettingSet{"buy.t1", "t0", 1n},
   M.TurnChanged{"t1", M.Running{}, 2n},
   M.TurnChanged{"t1", M.Completed{}, 3n}], more)

def st(+more: List<&2, M.Change>) -> M.State:
  M.State.replay(log(more), M.State.empty())

def sig(+s: M.State) -> String:
  SG.Signal.name(SG.Signal.at(SG.Signal.index(s), "t1"))

def approval() -> M.Change:
  M.AskOpened{"q1", "t1", "approval", "order_quote", "Send 3 files", "", 4n}

def sum(+state: String, ddp: Bool, payable: Bool) -> String:
  OR.Order.sum(J.Json.obj([J.KV{"id", J.Str{"q_1"}}, J.KV{"status", J.Str{state}},
    J.KV{"quote", J.Json.obj([J.KV{"total", J.Num{"112.4"}}, J.KV{"pay_label", J.Str{"Buy now · $112.40"}},
      J.KV{"shipping", J.Json.obj([J.KV{"ddp", J.Flag{ddp}}])}, J.KV{"problems", J.Json.arr(Bool.pick(List<&2, J.Json>, payable, Nil{}, [J.Str{"No shipping"}]))}])}]), "https://x/q/q_1?t=tok")

def card(+s: M.State, running: Bool) -> OR.OrderCard:
  OR.Order.card(False{}, OR.Order.asks.files(OR.Order.asks.open.go(M.Asks.all(s), "t1")), Bool.not(String.is_empty(M.Setting.get(s, OR.Order.wait.key("t1"), ""))), running,
    M.Setting.get(s, OR.Order.quote.key("t1"), ""))

def main() -> IO(Unit):
  do IO<Unit>:
    # the thread's state
    C.Check.str("a buy thread with its turn over is finished", sig(st([])), "finished")
    C.Check.str("waiting on the quote it is not", sig(st([OR.Order.wait.set("t1", 5n)])), "waiting")
    C.Check.str("so it has work out", Nat.show(M.Work.busy(st([OR.Order.wait.set("t1", 5n)]), "t1")), "1")
    C.Check.str("and its page says Waiting since", Bool.show(SG.Signal.waiting(Map.new(&2, String), st([OR.Order.wait.set("t1", 5n)]), "t1", False{})), "True")
    C.Check.str("the rows map counts it", Nat.show(M.Work.kids.at.k(Map.get(Nat, 0n, M.Work.rows.map(st([OR.Order.wait.set("t1", 5n)])), "t1"))), "1")
    C.Check.str("as the walk over every setting does", Nat.show(M.Work.kids.at.k(Map.get(Nat, 0n, M.Work.rows.map.all(st([OR.Order.wait.set("t1", 5n)])), "t1"))), "1")
    C.Check.str("once told, finished again", sig(M.State.replay(OR.Order.unwait(st([OR.Order.wait.set("t1", 5n)]), "t1", 9n), st([OR.Order.wait.set("t1", 5n)]))), "finished")
    C.Check.str("nothing to undo without a wait", Nat.show(List.length(&2, M.Change, OR.Order.unwait(st([]), "t1", 9n))), "0")
    # the card
    C.Check.str("details first", Nat.show(OR.Order.card.step(card(st([]), True{}))), "0")
    C.Check.str("while the agent works", OR.Order.card.text(card(st([]), True{})), "Getting the order ready…")
    C.Check.str("the files' approval open", Nat.show(OR.Order.card.step(card(st([approval()]), False{}))), "1")
    C.Check.str("says to check them", OR.Order.card.text(card(st([approval()]), False{})), "Check the files below and send them for a quote.")
    C.Check.str("sent: the quote is on its way", Nat.show(OR.Order.card.step(card(st([approval(), M.AskClosed{"q1", "accept", 5n}, OR.Order.wait.set("t1", 5n)]), False{}))), "2")
    C.Check.str("and the card says so", OR.Order.card.text(card(st([approval(), M.AskClosed{"q1", "accept", 5n}, OR.Order.wait.set("t1", 5n)]), False{})), "Getting your quote… usually 1-2 min")
    C.Check.str("quoted: pay", Nat.show(OR.Order.card.step(card(st([M.SettingSet{OR.Order.quote.key("t1"), sum("ready", True{}, True{}), 6n}]), False{}))), "3")
    C.Check.str("with its total", OR.Order.card.total(card(st([M.SettingSet{OR.Order.quote.key("t1"), sum("ready", True{}, True{}), 6n}]), False{})), "$112.40")
    C.Check.str("duty paid", OR.Order.card.customs(card(st([M.SettingSet{OR.Order.quote.key("t1"), sum("ready", True{}, True{}), 6n}]), False{})), "Includes import VAT and duties: nothing to pay on delivery.")
    C.Check.str("or not", OR.Order.card.customs(card(st([M.SettingSet{OR.Order.quote.key("t1"), sum("ready", False{}, True{}), 6n}]), False{})), "Excludes customs: the courier will contact you to pay any duties and taxes before delivery.")
    C.Check.str("Buy now", OR.Order.card.pay(card(st([M.SettingSet{OR.Order.quote.key("t1"), sum("ready", True{}, True{}), 6n}]), False{})), "https://x/q/q_1?t=tok")
    C.Check.str("no Buy now for a quote with problems", OR.Order.card.pay(card(st([M.SettingSet{OR.Order.quote.key("t1"), sum("ready", True{}, False{}), 6n}]), False{})), "")
    C.Check.str("which says so", OR.Order.card.text(card(st([M.SettingSet{OR.Order.quote.key("t1"), sum("ready", True{}, False{}), 6n}]), False{})), "The quote has something to fix first: see it below.")
    C.Check.str("a new quote on its way wins over the old", Nat.show(OR.Order.card.step(card(st([M.SettingSet{OR.Order.quote.key("t1"), sum("ready", True{}, True{}), 6n}, OR.Order.wait.set("t1", 7n)]), False{}))), "2")
    C.Check.str("an open question first", OR.Order.card.text(OR.Order.card(True{}, False{}, False{}, False{}, "")), "Backplane needs your answer below.")
    C.Check.str("paid: made", Nat.show(OR.Order.card.step(OR.Order.card(False{}, False{}, False{}, False{}, "{\"state\":\"paid\",\"payable\":true,\"total\":\"$5\"}"))), "4")
    C.Check.str("and no Buy now", OR.Order.card.pay(OR.Order.card(False{}, False{}, False{}, False{}, "{\"state\":\"paid\",\"payable\":true,\"total\":\"$5\",\"pay\":\"u\"}")), "")
    C.Check.str("shipped", OR.Order.card.text(OR.Order.card(False{}, False{}, False{}, False{}, "{\"state\":\"shipped\",\"tracking\":\"LX1\"}")), "Shipped. Tracking: LX1")
    C.Check.str("delivered: every step done", OR.Order.card.mark(OR.Order.card.step(OR.Order.card(False{}, False{}, False{}, False{}, "{\"state\":\"delivered\"}")), 5n), "done")
    C.Check.str("the step at work is lit", OR.Order.card.mark(2n, 2n), "on")
    # a newer quote takes the card; an older quote's progress does not
    C.Check.str("a fresh quote is kept", Bool.show(OR.Order.sum.keeps(True{}, "{\"id\":\"q_1\"}", "q_2")), "True")
    C.Check.str("an older quote's progress is not", Bool.show(OR.Order.sum.keeps(False{}, "{\"id\":\"q_2\"}", "q_1")), "False")
    C.Check.str("its own is", Bool.show(OR.Order.sum.keeps(False{}, "{\"id\":\"q_2\"}", "q_2")), "True")
    # the posted quote says it too
    C.Check.str("the quote's customs line", OR.Order.quote.customs("**Your quote: $5.00**", False{}), "**Your quote: $5.00**\n\nExcludes customs: the courier will contact you to pay any duties and taxes before delivery.")
    # what the agent hears
    C.Check.str("order_status before Send", OR.Order.status.of(st([approval()]), "t1"), OR.Order.status.unsent())
    C.Check.str("order_status while it is made", OR.Order.status.of(st([approval(), M.AskClosed{"q1", "accept", 5n}, OR.Order.wait.set("t1", 5n)]), "t1"), OR.Order.status.sent())
    C.Check.str("else the server is asked", OR.Order.status.of(st([]), "t1"), "")
    C.Check.str("a pending quote is no problem", J.Json.get_str(Maybe.default(&2, J.Json, J.Json.parse(OR.Order.said(J.Json.obj([J.KV{"status", J.Str{"analysing"}}]))), J.Null{}), "note", ""), OR.Order.status.sent())
    # a typed question
    C.Check.str("typed is its own tool", OR.Order.ask.tool(True{}), "order_ask.typed")
    C.Check.str("still an order question", Bool.show(OR.Order.ask.is("order_ask.typed")), "True")
    C.Check.str("the earlier answer first", OR.Order.ask.choices("Greece", ["", "Greece", "Cyprus"]), "Greece · last time\nCyprus")
    C.Check.str("none without one", OR.Order.ask.choices(" ", ["5", " ", "10"]), "5\n10")
    C.Check.str("sent without the mark", OR.Order.ask.value("Greece · last time"), "Greece")
    C.Check.str("a choice as it is", OR.Order.ask.value("10"), "10")
