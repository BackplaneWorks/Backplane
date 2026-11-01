import Base
import ./check.bend as C
import ../src/core/hub.bend as H
import ../src/core/model.bend as M
import ../src/core/json.bend as J
import ../src/core/client.bend as U
import ../src/core/modelpick.bend as MP
import ../src/core/accounts.bend as A
import ../src/core/mcp.bend as MC
import ../src/core/codec.bend as CD
import ../src/core/fmcp.bend as FM

# Exercise the same request, event replay and turn output used by the hub.
# No real provider processes or credentials are used.
def prompt() -> String:
  "  日本語 é 🛠️\n\nKeep `literal` text and trailing spaces.  \n"

def fresh() -> M.State:
  H.Demo.state("codex", M.Idle{})

def pick(+s: M.State, +to: String, +model: String) -> H.Step:
  H.Hub.request(s, 10n, 0, J.Json.obj([J.KV{"id", J.Str{"pick"}}, J.KV{"m", J.Str{"thread.provider"}},
    J.KV{"p", J.Json.obj([J.KV{"thread", J.Str{"t1"}}, J.KV{"provider", J.Str{to}}, J.KV{"model", J.Str{model}}, J.KV{"effort", J.Str{"high"}}])}]))

def switched(+s: M.State, to: String, model: String) -> M.State:
  M.State.replay(H.Queue.changes(pick(s, to, model)), s)

def once() -> M.State:
  switched(fresh(), "claude", "opus")

def repeated() -> M.State:
  switched(switched(once(), "grok", "grok-model"), "codex", "gpt-6.1-sol")

def send(+s: M.State) -> H.Step:
  H.Hub.post(s, 20n, "t1", "first", M.User{}, prompt())

def outs(st: H.Step) -> List<&2, H.Out>:
  match st:
    case H.Step{_, os}: os

def input(os: List<&2, H.Out>) -> String:
  match os:
    case Nil{}: "NO TURN"
    case H.CodexTurn{_, _, _, _, _, text} <> _: text
    case H.GrokTurn{_, _, _, _, _, text} <> _: text
    case H.AgentSend{_, line} <> _: J.Json.get_str(Maybe.default(&2, J.Json, List.head(&2, J.Json, J.Json.elems(J.Json.get_obj(J.Json.get_obj(Maybe.default(&2, J.Json, J.Json.parse(line), J.Null{}), "message"), "content"))), J.Null{}), "text", "NO CONTENT")
    case _ <> rest: input(rest)

def posted(cs: List<&2, M.Change>) -> String:
  match cs:
    case Nil{}: "NO MESSAGE"
    case M.MessagePosted{_, _, M.User{}, text, _} <> _: text
    case _ <> rest: posted(rest)

def ui(a: U.Act) -> U.Ui:
  match a:
    case U.Act{u, _}: u

def draft() -> String:
  +u = ui(U.Ui.act(ui(U.Ui.act(U.Ui.adopt.raw(U.Ui.init(10n), fresh()), "select", "t1")), "draft", prompt()))
  +picked = ui(U.Mpk.apply(u, MP.MpCur{0n, "claude", "opus", "high"}))
  U.Ui.draft_text(U.Ui.adopt.raw(picked, once()))

def named() -> M.State:
  M.State.replay([M.SettingSet{A.Accounts.binding.key("t1", "codex"), A.Accounts.binding.show("owner-A", 2n), 3n},
    M.SettingSet{A.Accounts.binding.key("t1", "claude"), A.Accounts.binding.show("owner-B", 4n), 3n},
    M.SessionSet{"t1", "session-A", 3n}], fresh())

def established() -> M.State:
  switched(H.Demo.switched(), "grok", "grok-model")

def configure(+s: M.State, +args: String) -> MC.Res:
  MC.Mcp.call.k(CD.Codec.pos(MC.Mcp.names(), "thread_update"), s, 10n, "t1", J.Null{}, Maybe.default(&2, J.Json, J.Json.parse(args), J.Null{}))

def configured(+s: M.State, args: String) -> M.State:
  M.State.replay(MC.Res.changes(configure(s, args)), s)

def response(r: MC.Res) -> String:
  match r:
    case MC.Res{_, body, _}: J.Json.show(body)

def effects(r: MC.Res) -> Nat:
  match r:
    case MC.Res{_, _, H.Step{_, os}}: List.length(&2, H.Out, os)

def config.once() -> M.State:
  configured(fresh(), "{\"provider\":\"claude\",\"model\":\"opus\"}")

def config.repeated() -> M.State:
  configured(configured(config.once(), "{\"provider\":\"grok\",\"model\":\"grok-model\"}"), "{\"provider\":\"codex\",\"model\":\"gpt-6.1-sol\"}")

def foreign() -> M.State:
  M.State.replay([M.ThreadCreated{"t2", "p2", "other", M.Local{}, "", "", "codex", "", 1n}], fresh())

def config.named() -> M.State:
  configured(configured(named(), "{\"provider\":\"claude\",\"model\":\"opus\"}"), "{\"provider\":\"codex\",\"model\":\"gpt-6.1-sol\"}")

def tool.schema(ts: List<&2, J.Json>) -> J.Json:
  match ts:
    case Nil{}: J.Null{}
    case +t <> rest: Bool.pick(J.Json, String.eq(J.Json.get_str(t, "name", ""), "thread_update"), J.Json.get_obj(t, "inputSchema"), tool.schema(rest))

def remote(+s: M.State) -> MC.Res:
  FM.Fm.call(s, 10n, "thread_update", "L~caller", J.Json.obj([J.KV{"threadId", J.Str{"t1"}}, J.KV{"provider", J.Str{"claude"}}, J.KV{"model", J.Str{"opus"}}]))

def protocol(+s: M.State) -> MC.McpOut:
  MC.Mcp.handle(s, 10n, "t1", J.Null{}, "test", J.Json.obj([J.KV{"jsonrpc", J.Str{"2.0"}}, J.KV{"id", J.Num{"7"}}, J.KV{"method", J.Str{"tools/call"}},
    J.KV{"params", J.Json.obj([J.KV{"name", J.Str{"thread_update"}}, J.KV{"arguments", J.Json.obj([J.KV{"provider", J.Str{"claude"}}, J.KV{"model", J.Str{"opus"}}])}])}]))

def protocol.step(r: MC.McpOut) -> H.Step:
  match r:
    case MC.McpOut{_, st, _}: st

def main() -> IO(Unit):
  +one = once()
  +many = repeated()
  +namedback = switched(switched(named(), "claude", "opus"), "codex", "gpt-6.1-sol")
  +est = established()
  do IO<Unit>:
    C.Check.str("fresh choice has no visible narrative", Nat.show(M.Switch.count(one, "t1")), "0")
    C.Check.str("fresh choice has no queued context", M.Queue.of(one, "t1"), "")
    C.Check.str("fresh choice selects provider and model", H.Hub.provider(H.Hub.thread(one, "t1")) ++ "/" ++ H.Turn.model(H.Hub.thread(one, "t1")), "claude/opus")
    C.Check.str("fresh choice selects effort", M.Setting.get(one, "effort.t1", ""), "high")
    C.Check.str("opposite default provider also starts quietly", Nat.show(M.Switch.count(switched(H.Demo.state("claude", M.Idle{}), "codex", "gpt-6.1-sol"), "t1")), "0")
    C.Check.str("fresh first provider input is verbatim", input(outs(send(one))), prompt())
    C.Check.str("fresh first posted message is verbatim", posted(H.Queue.changes(send(one))), prompt())
    C.Check.str("repeated choices and back have no narrative", Nat.show(M.Switch.count(many, "t1")), "0")
    C.Check.str("repeated choices and back have no queued context", M.Queue.of(many, "t1"), "")
    C.Check.str("back selects final provider and model", H.Hub.provider(H.Hub.thread(many, "t1")) ++ "/" ++ H.Turn.model(H.Hub.thread(many, "t1")), "codex/gpt-6.1-sol")
    C.Check.str("repeated choices first provider input is verbatim", input(outs(send(many))), prompt())
    C.Check.str("repeated choices first posted message is verbatim", posted(H.Queue.changes(send(many))), prompt())
    C.Check.str("picker and event adoption preserve unicode draft", draft(), prompt())
    C.Check.str("switching back preserves existing session", M.Switch.sess(H.Hub.thread(namedback, "t1")), "session-A")
    C.Check.str("codex account ownership unchanged", A.Accounts.id(namedback, "t1", "codex"), "owner-A")
    C.Check.str("claude account ownership unchanged", A.Accounts.id(namedback, "t1", "claude"), "owner-B")
    C.Check.str("unavailable named account never becomes legacy", Bool.show(A.Accounts.allowed(namedback, "t1", "codex")), "False")
    C.Check.str("established switch still has visible narrative", Nat.show(M.Switch.count(est, "t1")), "3")
    C.Check.str("established switch queues real history", Bool.show(String.contains(M.Queue.of(est, "t1"), "User: hi\n\nAssistant: hello")), "True")
    C.Check.str("established switch keeps original messages", Nat.show(List.length(&2, M.Entry, H.Hub.entries(est))), "3")
    C.Check.str("established real prompt receives history", Bool.show(String.contains(input(outs(send(est))), "Assistant: hello")), "True")
    C.Check.str("established real prompt retains user text verbatim", Bool.show(String.ends_with(input(outs(send(est))), prompt())), "True")
    C.Check.str("return to existing provider resumes its session", M.Switch.sess(H.Hub.thread(switched(H.Demo.switched(), "claude", "opus"), "t1")), "S1")
    C.Check.str("running conversation rejects switch", Nat.show(List.length(&2, M.Change, H.Queue.changes(pick(H.Demo.state("codex", M.Running{}), "claude", "opus")))), "0")
    C.Check.str("MCP configuration selects provider/model without a message", H.Hub.provider(H.Hub.thread(config.once(), "t1")) ++ "/" ++ H.Turn.model(H.Hub.thread(config.once(), "t1")), "claude/opus")
    C.Check.str("MCP configuration emits only provider selection", Nat.show(List.length(&2, M.Change, MC.Res.changes(configure(fresh(), "{\"provider\":\"claude\",\"model\":\"opus\"}")))), "1")
    C.Check.str("MCP configuration dispatches no effects", Nat.show(effects(configure(fresh(), "{\"provider\":\"claude\",\"model\":\"opus\"}"))), "0")
    C.Check.str("MCP readback reports actual provider/model", Bool.show(String.contains(response(configure(fresh(), "{\"provider\":\"claude\",\"model\":\"opus\"}")), "\"provider\":\"claude\",\"model\":\"opus\"")), "True")
    C.Check.str("MCP repeated choices have no narrative", Nat.show(M.Switch.count(config.repeated(), "t1")), "0")
    C.Check.str("MCP repeated choices have no queued context", M.Queue.of(config.repeated(), "t1"), "")
    C.Check.str("MCP repeated choices first provider input is verbatim", input(outs(send(config.repeated()))), prompt())
    C.Check.str("MCP model-only config applies", H.Turn.model(H.Hub.thread(configured(fresh(), "{\"model\":\"gpt-6.1-sol\"}"), "t1")), "gpt-6.1-sol")
    C.Check.str("MCP running turn rejects configuration", Bool.show(MC.Res.good(configure(H.Demo.state("codex", M.Running{}), "{\"provider\":\"claude\"}"))), "False")
    C.Check.str("MCP running model-only change rejects configuration", Bool.show(MC.Res.good(configure(H.Demo.state("codex", M.Running{}), "{\"model\":\"gpt-6.1-sol\"}"))), "False")
    C.Check.str("MCP running rejection dispatches nothing", Nat.show(effects(configure(H.Demo.state("codex", M.Running{}), "{\"provider\":\"claude\"}"))), "0")
    C.Check.str("MCP invalid provider fails closed", Bool.show(MC.Res.good(configure(fresh(), "{\"provider\":\"typo\"}"))), "False")
    C.Check.str("MCP missing thread fails closed", Bool.show(MC.Res.good(configure(fresh(), "{\"threadId\":\"missing\",\"provider\":\"claude\"}"))), "False")
    C.Check.str("MCP established config queues history", Bool.show(String.contains(M.Queue.of(configured(H.Demo.switched(), "{\"provider\":\"grok\"}"), "t1"), "Assistant: hello")), "True")
    C.Check.str("MCP established config dispatches nothing", Nat.show(effects(configure(H.Demo.switched(), "{\"provider\":\"grok\"}"))), "0")
    C.Check.str("MCP established config adds no visible narrative", Nat.show(M.Switch.count(configured(H.Demo.switched(), "{\"provider\":\"grok\"}"), "t1")), "2")
    C.Check.str("MCP running rejection changes no state", Nat.show(List.length(&2, M.Change, MC.Res.changes(configure(H.Demo.state("codex", M.Running{}), "{\"provider\":\"claude\"}")))), "0")
    C.Check.str("MCP repeat same choice is idempotent", Nat.show(List.length(&2, M.Change, MC.Res.changes(configure(config.once(), "{\"provider\":\"claude\",\"model\":\"opus\"}")))), "0")
    C.Check.str("MCP model-only preserves provider", H.Hub.provider(H.Hub.thread(configured(config.once(), "{\"model\":\"sonnet\"}"), "t1")), "claude")
    C.Check.str("MCP explicit default resets model", H.Turn.model(H.Hub.thread(configured(config.once(), "{\"model\":\"\"}"), "t1")), "")
    C.Check.str("MCP another provider defaults its model", H.Turn.model(H.Hub.thread(configured(config.once(), "{\"provider\":\"grok\"}"), "t1")), "")
    C.Check.str("MCP invalid type fails closed", Bool.show(MC.Res.good(configure(fresh(), "{\"model\":null}"))), "False")
    C.Check.str("MCP config cannot accidentally rename", Bool.show(MC.Res.good(configure(fresh(), "{\"provider\":\"claude\",\"title\":\"unrelated\"}"))), "False")
    C.Check.str("MCP config cannot alter reports", Bool.show(MC.Res.good(configure(fresh(), "{\"provider\":\"claude\",\"report\":false}"))), "False")
    C.Check.str("MCP cross-project config refused", Bool.show(MC.Res.good(configure(foreign(), "{\"threadId\":\"t2\",\"provider\":\"claude\"}"))), "False")
    C.Check.str("MCP codex account remains bound", A.Accounts.id(config.named(), "t1", "codex"), "owner-A")
    C.Check.str("MCP claude account remains bound", A.Accounts.id(config.named(), "t1", "claude"), "owner-B")
    C.Check.str("MCP session survives switching back", M.Switch.sess(H.Hub.thread(config.named(), "t1")), "session-A")
    C.Check.str("MCP model-only retains active session", M.Switch.sess(H.Hub.thread(configured(named(), "{\"model\":\"gpt-6.1-sol\"}"), "t1")), "session-A")
    C.Check.str("MCP picker adoption retains draft", U.Ui.draft_text(U.Ui.adopt.raw(ui(U.Ui.act(ui(U.Ui.act(U.Ui.adopt.raw(U.Ui.init(10n), fresh()), "select", "t1")), "draft", prompt())), config.repeated())), prompt())
    C.Check.str("MCP tool schema exposes configuration", Bool.show(Maybe.is_some(&2, J.Json, J.Json.get(J.Json.get_obj(tool.schema(J.Json.elems(J.Json.get_obj(MC.Mcp.tools(), "tools"))), "properties"), "provider"))), "True")
    C.Check.str("linked-machine harness route selects provider", H.Hub.provider(H.Hub.thread(M.State.replay(MC.Res.changes(remote(fresh())), fresh()), "t1")), "claude")
    C.Check.str("linked-machine configuration dispatches no effects", Nat.show(effects(remote(fresh()))), "0")
    C.Check.str("linked-machine running turn is rejected", Bool.show(MC.Res.good(remote(H.Demo.state("codex", M.Running{})))), "False")
    C.Check.str("JSON-RPC tools/call changes only selection", Nat.show(List.length(&2, M.Change, H.Queue.changes(protocol.step(protocol(fresh()))))), "1")
    C.Check.str("JSON-RPC tools/call dispatches no effects", Nat.show(List.length(&2, H.Out, outs(protocol.step(protocol(fresh()))))), "0")
    C.Check.str("JSON-RPC reply contains saved model", Bool.show(String.contains(MC.McpOut.reply(protocol(fresh())), "opus")), "True")
    C.Check.str("JSON-RPC running rejection has no changes", Nat.show(List.length(&2, M.Change, H.Queue.changes(protocol.step(protocol(H.Demo.state("codex", M.Running{})))))), "0")
    C.Check.str("MCP retains existing queued conversation context", M.Queue.of(configured(M.State.replay([M.TurnQueued{"t1", "handoff:fork", "real fork history", 3n}], fresh()), "{\"provider\":\"claude\"}"), "t1"), "real fork history")
