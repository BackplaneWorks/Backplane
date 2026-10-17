import Base
import ./check.bend as C
import ../src/core/acp.bend as A
import ../src/core/json.bend as J

# The Grok ACP translator: a scripted exchange, checked line by line.

def turn(+sid: String) -> A.AcpTurn:
  A.AcpTurn{"/tmp/p", "http://127.0.0.1:1/mcp/t?token=x", "RULES", "hello", sid, False{}}

# a supervised turn: tool requests go to the user
def turn.ask(+sid: String) -> A.AcpTurn:
  A.AcpTurn{"/tmp/p", "http://127.0.0.1:1/mcp/t?token=x", "RULES", "hello", sid, True{}}

def outs(o: A.AcpOut) -> String:
  match o:
    case A.AcpOut{_, send, emit, _}:
      String.join(List.append(&2, String, send, emit), " | ")

def asks(o: A.AcpOut) -> String:
  match o:
    case A.AcpOut{_, _, _, ks}:
      String.join(ks, " | ")

def st(o: A.AcpOut) -> A.Acp:
  match o:
    case A.AcpOut{a, _, _, _}:
      a

def method(+line: String) -> String:
  J.Json.get_str(Maybe.default(&2, J.Json, J.Json.parse(line), J.Null{}), "method", "")

def done(a: A.Acp) -> String:
  match a:
    case A.Acp{_, _, True{}}:
      "done"
    case A.Acp{_, _, False{}}:
      "open"

def sends(o: A.AcpOut) -> String:
  match o:
    case A.AcpOut{_, send, _, _}:
      String.join(send, " | ")

def elicit(+id: String, +schema: String) -> String:
  "{\"jsonrpc\":\"2.0\",\"id\":" ++ id ++ ",\"method\":\"elicitation/create\",\"params\":{\"mode\":\"form\",\"sessionId\":\"S1\",\"message\":\"Approve the tool?\",\"requestedSchema\":" ++ schema ++ "}}"

def approve_schema() -> String:
  "{\"type\":\"object\",\"properties\":{\"value\":{\"type\":\"string\",\"enum\":[\"Approve\",\"Deny\"]}},\"required\":[\"value\"]}"

def yes_no_schema() -> String:
  "{\"type\":\"object\",\"properties\":{\"value\":{\"type\":\"string\",\"enum\":[\"Yes\",\"No\"]}},\"required\":[\"value\"]}"

def main() -> IO(Unit):
  +t = turn("")
  +a0 = A.Acp.new()
  +o1 = A.Acp.line(a0, t, "{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{\"protocolVersion\":1}}")
  +o2 = A.Acp.line(st(o1), t, "{\"jsonrpc\":\"2.0\",\"id\":2,\"result\":{\"sessionId\":\"S1\"}}")
  +o3 = A.Acp.line(st(o2), t, "{\"jsonrpc\":\"2.0\",\"method\":\"session/update\",\"params\":{\"sessionId\":\"S1\",\"update\":{\"sessionUpdate\":\"agent_message_chunk\",\"content\":{\"type\":\"text\",\"text\":\"Hi \"}}}}")
  +o4 = A.Acp.line(st(o3), t, "{\"jsonrpc\":\"2.0\",\"method\":\"session/update\",\"params\":{\"sessionId\":\"S1\",\"update\":{\"sessionUpdate\":\"agent_message_chunk\",\"content\":{\"type\":\"text\",\"text\":\"there\"}}}}")
  +o5 = A.Acp.line(st(o4), t, "{\"jsonrpc\":\"2.0\",\"id\":3,\"result\":{\"stopReason\":\"end_turn\"}}")
  +ob = A.Acp.line(a0, turn("OLD"), "{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{}}")
  +orp = A.Acp.line(a0, turn("OLD"), "{\"jsonrpc\":\"2.0\",\"method\":\"session/update\",\"params\":{\"sessionId\":\"OLD\",\"update\":{\"sessionUpdate\":\"agent_message_chunk\",\"content\":{\"type\":\"text\",\"text\":\"old\"}}}}")
  +oe = A.Acp.line(a0, t, "{\"jsonrpc\":\"2.0\",\"id\":2,\"error\":{\"code\":-32000,\"message\":\"Authentication required\"}}")
  +op = A.Acp.line(a0, t, "{\"jsonrpc\":\"2.0\",\"id\":\"p9\",\"method\":\"session/request_permission\",\"params\":{\"options\":[{\"optionId\":\"no\",\"kind\":\"reject_once\"},{\"optionId\":\"yes\",\"kind\":\"allow_once\"}]}}")
  +oq = A.Acp.line(a0, turn.ask(""), "{\"jsonrpc\":\"2.0\",\"id\":\"p9\",\"method\":\"session/request_permission\",\"params\":{\"toolCall\":{\"toolCallId\":\"c1\",\"title\":\"List files\",\"kind\":\"execute\",\"rawInput\":{\"command\":\"ls -la\"}},\"options\":[{\"optionId\":\"no\",\"kind\":\"reject_once\"},{\"optionId\":\"always\",\"kind\":\"allow_always\"},{\"optionId\":\"yes\",\"kind\":\"allow_once\"}]}}")
  +body = {"{\"jsonrpc\":\"2.0\",\"id\":\"p9\",\"method\":\"session/request_permission\",\"params\":{\"toolCall\":{\"toolCallId\":\"c1\",\"title\":\"List files\",\"kind\":\"execute\",\"rawInput\":{\"command\":\"ls -la\"}},\"options\":[{\"optionId\":\"no\",\"kind\":\"reject_once\"},{\"optionId\":\"always\",\"kind\":\"allow_always\"},{\"optionId\":\"yes\",\"kind\":\"allow_once\"}]}}" : String}
  +ea = elicit("\"e1\"", approve_schema())
  +eo = elicit("\"e1\"", yes_no_schema())
  +ae = A.Acp.line(a0, turn.ask(""), ea)
  +af = A.Acp.line(a0, turn(""), ea)
  +ano = A.Acp.line(a0, turn(""), eo)
  +ano2 = A.Acp.line(a0, turn.ask(""), eo)
  do IO<Unit>:
    C.Check.str("acp init method", method(A.Acp.init()), "initialize")
    C.Check.str("acp new session", method(outs(o1)), "session/new")
    C.Check.str("acp resume", method(outs(ob)), "session/load")
    C.Check.str("acp replay ignored", outs(orp), "")
    C.Check.str("acp opened", Bool.pick(String, String.contains(outs(o2), "session/prompt"), "yes", "no"), "yes")
    C.Check.str("acp rules first", Bool.pick(String, String.contains(outs(o2), "<backplane_instructions>\\nRULES"), "yes", "no"), "yes")
    C.Check.str("acp started", Bool.pick(String, String.contains(outs(o2), "{\"type\":\"thread.started\",\"thread_id\":\"S1\"}"), "yes", "no"), "yes")
    C.Check.str("acp delta", outs(o3), "{\"type\":\"item.delta\",\"text\":\"Hi \"}")
    C.Check.str("acp finished", outs(o5), "{\"type\":\"item.completed\",\"item\":{\"type\":\"agent_message\",\"text\":\"Hi there\"}} | {\"type\":\"turn.completed\"}")
    C.Check.str("acp finished state", done(st(o5)), "done")
    C.Check.str("acp auth", outs(oe), "{\"type\":\"turn.failed\",\"message\":\"Authentication required (sign in once with `grok login`)\"}")
    C.Check.str("acp permit", outs(op), "{\"jsonrpc\":\"2.0\",\"id\":\"p9\",\"result\":{\"outcome\":{\"outcome\":\"selected\",\"optionId\":\"yes\"}}}")
    C.Check.str("acp ask routes", outs(oq), "")
    C.Check.str("acp ask parks", method(asks(oq)), "session/request_permission")
    C.Check.str("acp full never parks", asks(op), "")
    C.Check.str("acp answer accept", A.Acp.permit.answer(body, "accept"), "{\"jsonrpc\":\"2.0\",\"id\":\"p9\",\"result\":{\"outcome\":{\"outcome\":\"selected\",\"optionId\":\"yes\"}}}")
    C.Check.str("acp answer session", A.Acp.permit.answer(body, "session"), "{\"jsonrpc\":\"2.0\",\"id\":\"p9\",\"result\":{\"outcome\":{\"outcome\":\"selected\",\"optionId\":\"always\"}}}")
    C.Check.str("acp answer decline", A.Acp.permit.answer(body, "decline"), "{\"jsonrpc\":\"2.0\",\"id\":\"p9\",\"result\":{\"outcome\":{\"outcome\":\"selected\",\"optionId\":\"no\"}}}")
    C.Check.str("acp answer no reject", A.Acp.permit.answer("{\"id\":4,\"params\":{\"options\":[{\"optionId\":\"y\",\"kind\":\"allow_once\"}]}}", "decline"), "{\"jsonrpc\":\"2.0\",\"id\":4,\"result\":{\"outcome\":{\"outcome\":\"cancelled\"}}}")
    C.Check.str("acp tool", A.Acp.tool(J.Json.get_obj(Maybe.default(&2, J.Json, J.Json.parse(body), J.Null{}), "params")), "execute")
    C.Check.str("acp detail", A.Acp.detail(J.Json.get_obj(Maybe.default(&2, J.Json, J.Json.parse(body), J.Null{}), "params")), "ls -la")
    C.Check.str("acp init unchanged for grok", A.Acp.init(), "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{\"protocolVersion\":1,\"clientCapabilities\":{\"fs\":{\"readTextFile\":false,\"writeTextFile\":false},\"terminal\":false}}}")
    C.Check.str("acp init form advertises", Bool.pick(String, String.contains(A.Acp.init.form(), "\"elicitation\":{\"form\":{}}"), "yes", "no"), "yes")
    C.Check.str("acp elicit parks in supervised", asks(ae), ea)
    C.Check.str("acp elicit full access never parks", asks(af), "")
    C.Check.str("acp elicit approve at once", sends(af), "{\"jsonrpc\":\"2.0\",\"id\":\"e1\",\"result\":{\"action\":\"accept\",\"content\":{\"value\":\"Approve\"}}}")
    C.Check.str("acp elicit other form declines", sends(ano), "{\"jsonrpc\":\"2.0\",\"id\":\"e1\",\"result\":{\"action\":\"decline\"}}")
    C.Check.str("acp elicit other form never asks", asks(ano2), "")
    C.Check.str("acp elicit other form declines supervised", sends(ano2), "{\"jsonrpc\":\"2.0\",\"id\":\"e1\",\"result\":{\"action\":\"decline\"}}")
    C.Check.str("acp elicit answer accept", A.Acp.elicit.answer(ea, "accept"), "{\"jsonrpc\":\"2.0\",\"id\":\"e1\",\"result\":{\"action\":\"accept\",\"content\":{\"value\":\"Approve\"}}}")
    C.Check.str("acp elicit answer deny", A.Acp.elicit.answer(ea, "decline"), "{\"jsonrpc\":\"2.0\",\"id\":\"e1\",\"result\":{\"action\":\"decline\"}}")
    C.Check.str("acp elicit answer session approves once", A.Acp.elicit.answer(ea, "session"), "{\"jsonrpc\":\"2.0\",\"id\":\"e1\",\"result\":{\"action\":\"accept\",\"content\":{\"value\":\"Approve\"}}}")
    C.Check.str("acp hint grok unchanged", A.Acp.hint("grok login", "Authentication required"), "Authentication required (sign in once with `grok login`)")
    C.Check.str("acp error default grok", A.Acp.error(J.Json.obj([J.KV{"id", J.Num{"2"}}])), "grok failed")
    C.Check.str("acp error named", A.Acp.error.named(J.Json.obj([J.KV{"id", J.Num{"2"}}]), "omp"), "omp failed")
    C.Check.str("acp error message kept", A.Acp.error.named(J.Json.obj([J.KV{"error", J.Json.obj([J.KV{"message", J.Str{"nope"}}])}]), "omp"), "nope")
    C.Check.str("acp elicit tool from omp's live form", A.Acp.elicit.tool("Allow tool: write\nPath: form-check.txt\nContent:\nhello"), "write")
    C.Check.str("acp elicit tool from another form", A.Acp.elicit.tool("Approve the tool?"), "Approve the tool?")
    C.Check.str("acp elicit tool without a body", A.Acp.elicit.tool("Allow tool: bash"), "bash")
