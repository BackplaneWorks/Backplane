import Base
import ./check.bend as C
import ./maildisposition_test.bend as F
import ../src/core/bothub.bend as BH
import ../src/core/model.bend as M
import ../src/core/hub.bend as H
import ../src/core/json.bend as J
import ../src/core/mcp.bend as MC
import ../src/core/maildisposition.bend as MD
import ../src/core/ledger.bend as LG
import ../src/core/order.bend as OR

def extra() -> List<&2, J.Json>:
  [MC.Tool.def("public_fixture_extra", "Public routing sentinel", [], [])]

def request(+method: String, +name: String, a: J.Json) -> J.Json:
  J.Json.obj([J.KV{"jsonrpc", J.Str{"2.0"}}, J.KV{"id", J.Num{"7"}}, J.KV{"method", J.Str{method}},
    J.KV{"params", J.Json.obj([J.KV{"name", J.Str{name}}, J.KV{"arguments", a}])}])

# This is the production outer router used by Hub.mcp.hub.normal, not
# Mcp.handle (the previous adapter bypassed both bot listing branches).
def routed(+s: M.State, +th: String, o: J.Json) -> MC.McpOut:
  BH.Bots.mcp.o(Some{o}, s, 1792519190n, th, J.Null{}, "fixture", J.Json.arr([]), extra())

def reply(o: MC.McpOut) -> J.Json:
  match o:
    case MC.McpOut{text, _, _}: Maybe.default(&2, J.Json, J.Json.parse(text), J.Null{})

def tools(+s: M.State, +th: String) -> List<&2, J.Json>:
  J.Json.elems(J.Json.get_obj(J.Json.get_obj(reply(routed(s, th, request("tools/list", "", J.Null{}))), "result"), "tools"))

def count(xs: List<&2, J.Json>, +name: String, acc: Nat) -> Nat:
  match xs:
    case Nil{}: acc
    case h <> t: count(t, name, Nat.add(acc, Bool.pick(Nat, String.eq(J.Json.get_str(h, "name", ""), name), 1n, 0n)))

def tool_count(+s: M.State, +th: String, +name: String) -> Nat:
  count(tools(s, th), name, 0n)

def n(+label: String, got: Nat, expected: Nat) -> IO(Unit):
  C.Check.str(label, Nat.show(got), Nat.show(expected))

def b(+label: String, got: Bool, expected: Bool) -> IO(Unit):
  C.Check.str(label, Bool.show(got), Bool.show(expected))

# Independent full-payload assembly keeps every prior tool, schema and
# position exact, not merely a substring or the new tool's presence.
def base_tools() -> List<&2, J.Json>:
  J.Json.elems(J.Json.get_obj(MC.Mcp.tools(), "tools"))

def with_mail() -> List<&2, J.Json>:
  List.concat(&2, J.Json, [base_tools(), MC.Mcp.mail.tools(), MC.Mcp.run.tools()])

# a plain listing ends with the order tools: Ordering is on by default
# (order_review only, outside a buy thread)
def on_order() -> List<&2, J.Json>:
  BH.Bots.order.tools(True{}, False{})

def bot_tail() -> List<&2, J.Json>:
  List.concat(&2, J.Json, [LG.Ledger.tools(), BH.Bots.tools(), BH.Bots.task.tools(), [BH.Bots.newproj.def()], extra()])

def exact(+label: String, +s: M.State, +th: String, expected: List<&2, J.Json>) -> IO(Unit):
  C.Check.str(label, J.Json.show(J.Json.arr(tools(s, th))), J.Json.show(J.Json.arr(expected)))

def no_grant(+label: String, +s: M.State, +th: String) -> IO(Unit):
  do IO<Unit>:
    n(label ++ " no advert", tool_count(s, th, "mail_dispose"), 0n)
    C.Check.str(label ++ " no Codex pregrant", String.join(MD.Disposition.codex(MD.Disposition.launch(s, th)), "|"), "")

def called(+s: M.State, +th: String) -> MC.McpOut:
  routed(s, th, request("tools/call", "mail_dispose", F.args(F.input(), "done")))

def changes(o: MC.McpOut) -> List<&2, M.Change>:
  match o:
    case MC.McpOut{_, st, _}: H.Queue.changes(st)

def route_name(r: BH.Route) -> String:
  match r:
    case BH.RList{}: "bot"
    case BH.RPlain{}: "plain"
    case BH.RLedger{}: "ledger"
    case BH.RCall{}: "bot-call"
    case BH.RPass{}: "pass"

def main() -> IO(Unit):
  do IO<Unit>:
    +s : M.State = F.base()
    +plain : M.State = M.State.apply(s, M.BotDeleted{"root", 9n})
    C.Check.str("real live run uses bot tools/list route", route_name(BH.Bots.route(s, F.run(), request("tools/list", "", J.Null{}))), "bot")
    C.Check.str("retained live run without owner uses plain route", route_name(BH.Bots.route(plain, F.run(), request("tools/list", "", J.Null{}))), "plain")
    n("actual bot router advertises live disposition exactly once", tool_count(s, F.run(), "mail_dispose"), 1n)
    n("actual plain router advertises retained live disposition exactly once", tool_count(plain, F.run(), "mail_dispose"), 1n)
    exact("live bot full tool payload and order retained", s, F.run(), List.concat(&2, J.Json, [with_mail(), MC.Mcp.dispose.tools(), MC.Mcp.context.tools(), bot_tail()]))
    exact("live plain full tool payload and order retained", plain, F.run(), List.concat(&2, J.Json, [with_mail(), MC.Mcp.dispose.tools(), MC.Mcp.context.tools(), LG.Ledger.tools(), on_order()]))
    no_grant("ordinary worker", s, F.worker())
    no_grant("ordinary FOH", s, "foh")
    exact("ordinary bot extras payload unchanged", s, "foh", List.concat(&2, J.Json, [with_mail(), bot_tail()]))
    exact("ordinary plain payload unchanged", s, F.worker(), List.concat(&2, J.Json, [with_mail(), LG.Ledger.tools(), on_order()]))
    +off : M.State = M.State.apply(s, M.SettingSet{"mail.on", "off", 9n})
    no_grant("mail off live run", off, F.run())
    exact("mail off plain payload unchanged", off, F.run(), List.concat(&2, J.Json, [base_tools(), LG.Ledger.tools(), on_order()]))
    exact("mail off FOH bot extras unchanged", off, "foh", List.append(&2, J.Json, base_tools(), bot_tail()))
    no_grant("completed run", M.State.apply(s, M.TurnChanged{F.run(), M.Completed{}, 9n}), F.run())
    no_grant("failed run", M.State.apply(s, M.TurnChanged{F.run(), M.Failed{}, 9n}), F.run())
    no_grant("interrupted run", M.State.apply(s, M.TurnChanged{F.run(), M.Interrupted{}, 9n}), F.run())
    +ended : M.State = M.State.apply(M.State.apply(s, M.TurnChanged{F.run(), M.Completed{}, 9n}), M.TurnChanged{F.run(), M.Running{}, 10n})
    no_grant("ended run ordinary resumed turn", ended, F.run())
    +buy : M.State = M.State.replay([M.SettingSet{"order.enabled", "on", 9n}, M.SettingSet{OR.Order.buy.key(F.worker()), "public-buy", 9n}], s)
    exact("buy thread order and ledger payload unchanged", buy, F.worker(), List.concat(&2, J.Json, [with_mail(), LG.Ledger.tools(), OR.Order.tools(), [OR.Order.review.tool()]]))
    n("ordinary order review stays exactly once", tool_count(M.State.apply(s, M.SettingSet{"order.enabled", "on", 9n}), F.worker(), "order_review"), 1n)
    n("ordinary non-buy gets no order quote", tool_count(M.State.apply(s, M.SettingSet{"order.enabled", "on", 9n}), F.worker(), "order_quote"), 0n)
    C.Check.str("ledger tool retains first route priority", route_name(BH.Bots.route(s, F.run(), request("tools/call", "task_board_list", J.Json.obj([])))), "ledger")
    b("actual routed ledger call succeeds", J.Json.get_bool(J.Json.get_obj(reply(routed(s, F.run(), request("tools/call", "task_board_list", J.Json.obj([])))), "result"), "isError", True{}), False{})
    b("actual routed live disposition call succeeds", J.Json.get_bool(J.Json.get_obj(reply(called(s, F.run())), "result"), "isError", True{}), False{})
    +handled : M.State = M.State.replay(changes(called(s, F.run())), s)
    b("actual outer router commits exact current notice receipt", String.is_empty(M.Setting.get(handled, MD.Disposition.key(F.run(), F.input()), "")), False{})
    n("routed handled notice prevents duplicate fallback", F.owes(handled), 0n)
    b("ordinary routed disposition rejected", J.Json.get_bool(J.Json.get_obj(reply(called(s, F.worker())), "result"), "isError", False{}), True{})
    n("ordinary routed rejection changes nothing", List.length(&2, M.Change, changes(called(s, F.worker()))), 0n)
    n("ordinary router list is read only", List.length(&2, M.Change, changes(routed(s, F.worker(), request("tools/list", "", J.Null{})))), 0n)
