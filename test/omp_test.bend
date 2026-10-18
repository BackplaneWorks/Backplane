import Base
import ./check.bend as C
import ./replay.bend as R
import ../src/core/model.bend as M
import ../src/core/omp.bend as OMP
import ../src/core/acp.bend as AC
import ../src/core/msp.bend as MS
import ../src/core/json.bend as J
import ../src/core/hub.bend as B
import ../src/core/mcp.bend as MC
import ../src/server/park.bend as PK

# oh-my-pi (core/omp.bend): the mapping, the install line, the models, the
# runner's Proto, and the turn replayed from test/fixtures/omp/turn.jsonl
# and synthetic lines: the config steps in order (new, renewed and
# rejected), a turn with none, a lost session, a sign-in failure, the
# missing-binary failure, and the Codex item for a backplane note.

def turn(+session: String, +model: String, +effort: String) -> OMP.OmpTurn:
  OMP.OmpTurn{AC.AcpTurn{"/tmp/p", "http://127.0.0.1:1/mcp/t?token=x", "RULES", "hello", session, True{}}, model, effort}

def feed(r: R.Run, +t: OMP.OmpTurn, +l: String) -> R.Run:
  match r:
    case R.Run{a, +ss, +es}:
      R.Run.add(OMP.Omp.line(a, t, l), ss, es)

def replay(ls: List<&2, String>, r: R.Run, +t: OMP.OmpTurn) -> R.Run:
  match ls:
    case Nil{}:
      r
    case +l <> rest:
      replay(rest, feed(r, t, l), t)

def ids(xs: List<&2, String>) -> List<&2, String>:
  match xs:
    case Nil{}:
      Nil{}
    case +x <> t:
      J.Json.as_raw(J.Json.get(R.obj(x), "id"), "") <> ids(t)

# what a Codex item becomes: its first change, as text
def change.of(c: M.Change) -> String:
  match c:
    case M.MessagePosted{_, _, _, text, _}:
      "msg " ++ text
    case M.ActivityLogged{_, _, tone, kind, summary, _}:
      "act " ++ tone ++ " " ++ kind ++ " " ++ summary
    case _:
      "other"

def first.of(cs: List<&2, M.Change>) -> String:
  match cs:
    case Nil{}:
      "none"
    case +c <> _:
      change.of(c)

def item.of(s: B.Step) -> String:
  match s:
    case B.Step{cs, _}:
      first.of(cs)

def spawn.omp() -> MS.Proto:
  MS.POmp{AC.AcpTurn{"/tmp/p", "u", "P", "t", "", True{}}, "", ""}

def spawn.grok() -> MS.Proto:
  MS.PAcp{AC.AcpTurn{"/tmp/p", "u", "P", "t", "", True{}}}

# omp's approval forms through the MCP route: an approval form is asked of
# the person and its reply waits; any other form is declined at once
def ap.form(+msg: String, +values: String) -> String:
  "{\"jsonrpc\":\"2.0\",\"id\":\"e1\",\"method\":\"elicitation/create\",\"params\":{\"mode\":\"form\",\"sessionId\":\"S1\",\"message\":\"" ++ msg ++ "\",\"requestedSchema\":{\"type\":\"object\",\"properties\":{\"value\":{\"type\":\"string\",\"enum\":" ++ values ++ "}},\"required\":[\"value\"]}}}"

def ap.handle(+line: String) -> MC.McpOut:
  MC.Mcp.handle(M.State.empty(), 10n, "t1", J.Json.arr(Nil{}), "1", Maybe.default(&2, J.Json, J.Json.parse(line), J.Null{}))

def ap.handle.provider(+provider: String, +line: String) -> MC.McpOut:
  +s = M.State.replay([M.ThreadCreated{"t1", "p", "Title", M.Local{}, "", "", provider, "model", 1n}], M.State.empty())
  MC.Mcp.handle(s, 10n, "t1", J.Json.arr(Nil{}), "1", Maybe.default(&2, J.Json, J.Json.parse(line), J.Null{}))

def ap.handle.provider.session(+provider: String, +line: String) -> MC.McpOut:
  +s = M.State.replay([
    M.ThreadCreated{"t1", "p", "Title", M.Local{}, "", "", provider, "model", 1n},
    M.AskOpened{"old", "t1", "approval", "execute", "previous permission", "", 2n},
    M.AskClosed{"old", "session", 3n}], M.State.empty())
  MC.Mcp.handle(s, 10n, "t1", J.Json.arr(Nil{}), "1", Maybe.default(&2, J.Json, J.Json.parse(line), J.Null{}))


def ap.permission.request() -> String:
  "{\"jsonrpc\":\"2.0\",\"id\":\"p1\",\"method\":\"session/request_permission\",\"params\":{\"toolCall\":{\"toolCallId\":\"c1\",\"title\":\"Run command\",\"kind\":\"execute\",\"rawInput\":{\"command\":\"echo hi\"}},\"options\":[{\"optionId\":\"no\",\"kind\":\"reject_once\"},{\"optionId\":\"yes\",\"kind\":\"allow_once\"},{\"optionId\":\"always\",\"kind\":\"allow_always\"}]}}"
def ap.permission.allow.always.only() -> String:
  "{\"jsonrpc\":\"2.0\",\"id\":\"p2\",\"method\":\"session/request_permission\",\"params\":{\"options\":[{\"optionId\":\"always\",\"kind\":\"allow_always\"}]}}"

def ap.permission.reject.always.only() -> String:
  "{\"jsonrpc\":\"2.0\",\"id\":\"p3\",\"method\":\"session/request_permission\",\"params\":{\"options\":[{\"optionId\":\"never\",\"kind\":\"reject_always\"}]}}"



def ap.cs(o: MC.McpOut) -> List<&2, M.Change>:
  match o:
    case MC.McpOut{_, st, _}:
      B.Queue.changes(st)

def ap.kind(cs: List<&2, M.Change>) -> String:
  match cs:
    case Nil{}:
      ""
    case M.AskOpened{_, th, k, tool, d, _, _} <> _:
      k ++ "|" ++ th ++ "|" ++ tool ++ "|" ++ d
    case _ <> t:
      ap.kind(t)

def ap.options(cs: List<&2, M.Change>) -> String:
  match cs:
    case Nil{}:
      ""
    case M.AskOpened{_, _, _, _, _, +options, _} <> _:
      options
    case _ <> rest:
      ap.options(rest)

def ap.id(cs: List<&2, M.Change>) -> String:
  match cs:
    case Nil{}:
      ""
    case M.AskOpened{i, _, _, _, _, _, _} <> _:
      i
    case _ <> t:
      ap.id(t)

def ap.park(o: MC.McpOut) -> String:
  match o:
    case MC.McpOut{_, _, p}:
      p

def ap.reply(o: MC.McpOut) -> String:
  match o:
    case MC.McpOut{r, _, _}:
      r


def ask.state(+provider: String) -> M.State:
  M.State.replay([M.ThreadCreated{"t", "p", "Title", M.Local{}, "", "", provider, "model", 1n}, M.AskOpened{"q", "t", "approval", "write", "message", "", 2n}], M.State.empty())

def ask.closed(cs: List<&2, M.Change>) -> String:
  match cs:
    case M.AskClosed{_, +answer, _} <> _:
      answer
    case _ <> rest:
      ask.closed(rest)
    case Nil{}:
      ""

def ask.answered(os: List<&2, B.Out>) -> String:
  match os:
    case B.AskAnswered{_, +answer} <> _:
      answer
    case _ <> rest:
      ask.answered(rest)
    case Nil{}:
      ""

def ask.result(step: B.Step) -> String:
  match step:
    case B.Step{cs, os}:
      ask.closed(cs) ++ "|" ++ ask.answered(os)

def ask.applied(s: M.State, step: B.Step) -> M.State:
  match step:
    case B.Step{cs, _}:
      M.State.replay(cs, s)

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("omp approval supervised", OMP.Omp.approval(M.Supervised{}), "always-ask")
    C.Check.str("omp approval accept edits", OMP.Omp.approval(M.AcceptEdits{}), "write")
    C.Check.str("omp approval auto", OMP.Omp.approval(M.AutoMode{}), "write")
    C.Check.str("omp approval full access", OMP.Omp.approval(M.FullAccess{}), "yolo")
    C.Check.str("omp args", String.join(OMP.Omp.args(M.Supervised{}), " "), "acp --approval-mode always-ask")
    C.Check.str("omp levels", String.join(OMP.Omp.levels(), ","), "off,auto,minimal,low,medium,high,xhigh,max")
    C.Check.str("omp models keeps chat only", String.join(OMP.Omp.models("{\"models\":[{\"kind\":\"chat\",\"selector\":\"a/b\"},{\"kind\":\"embedding\",\"selector\":\"c/d\"}]}"), ","), "a/b")
    C.Check.str("omp models bad json", String.join(OMP.Omp.models("nope"), ","), "")
    C.Check.str("omp models when omp fails", String.join(OMP.Omp.models(""), ","), "")
    C.Check.str("omp efforts when omp fails", J.Json.show(J.Json.obj(OMP.Omp.efforts(""))), "{\"default\":[]}")
    C.Check.str("omp efforts per model", J.Json.show(J.Json.obj(OMP.Omp.efforts("{\"models\":[{\"kind\":\"chat\",\"selector\":\"m-think\",\"reasoning\":true,\"thinking\":[\"minimal\",\"low\"]},{\"kind\":\"chat\",\"selector\":\"m-plain\",\"reasoning\":false,\"thinking\":[]}]}"))),
      "{\"default\":[],\"m-think\":[\"off\",\"auto\",\"minimal\",\"low\"],\"m-plain\":[]}")
    C.Check.str("omp install", OMP.Omp.install(), "Install oh-my-pi with `curl -fsSL https://omp.sh/install | sh` or `bun install -g @oh-my-pi/pi-coding-agent`, then sign in with `omp login`.")
    # the runner's Proto: the command, its name, its install line, its handshake
    C.Check.str("omp proto cmd", MS.Proto.cmd(spawn.omp()), "omp")
    C.Check.str("omp proto name", MS.Proto.name(spawn.omp()), "omp")
    C.Check.str("omp proto install", MS.Proto.install(spawn.omp()), OMP.Omp.install())
    C.Check.str("omp proto gone", MS.Proto.gone(spawn.omp()), "omp exited before the turn ended (is it signed in? run `omp login`)")
    C.Check.str("omp proto init", MS.Proto.init(spawn.omp()), AC.Acp.init.form())
    # a missing omp: the turn fails at once with the install line
    C.Check.str("omp missing turn", MS.Proto.spawn_failed(spawn.omp(), "spawn ENOENT"),
      "{\"type\":\"turn.failed\",\"message\":\"could not start omp: spawn ENOENT. " ++ OMP.Omp.install() ++ "\"}")
    # Grok's text is unchanged by the helper
    C.Check.str("grok missing turn", MS.Proto.spawn_failed(spawn.grok(), "boom"),
      "{\"type\":\"turn.failed\",\"message\":\"could not start grok: boom. Install Grok Build with `curl -fsSL https://x.ai/cli/install.sh | bash`, then `grok login`.\"}")
    +fx : List<&2, String> <- R.file("test/fixtures/omp/turn.jsonl")
    # a fresh session with a model and a level: model, then thinking, then the prompt
    +cfg4 : String = "{\"jsonrpc\":\"2.0\",\"id\":4,\"result\":{}}"
    +cfg5 : String = "{\"jsonrpc\":\"2.0\",\"id\":5,\"result\":{}}"
    +spliced : List<&2, String> = List.append(&2, String, [R.nth(fx, 0n), R.nth(fx, 1n), cfg4, cfg5], List.drop(&2, String, fx, 2n))
    +r1 : R.Run = replay(spliced, R.Run.new(), turn("", "anthropic/claude-haiku-5-5", "high"))
    C.Check.str("omp turn order", String.join(R.methods(R.sends(r1)), ","), "session/new,session/set_config_option,session/set_config_option,session/prompt")
    C.Check.str("omp turn model", J.Json.get_str(R.params(R.nth(R.sends(r1), 1n)), "configId", "") ++ "=" ++ J.Json.get_str(R.params(R.nth(R.sends(r1), 1n)), "value", ""), "model=anthropic/claude-haiku-5-5")
    C.Check.str("omp turn thinking", J.Json.get_str(R.params(R.nth(R.sends(r1), 2n)), "configId", "") ++ "=" ++ J.Json.get_str(R.params(R.nth(R.sends(r1), 2n)), "value", ""), "thinking=high")
    C.Check.str("omp turn session", J.Json.get_str(R.params(R.nth(R.sends(r1), 1n)), "sessionId", ""), "01a14112-89a7-7544-b120-9a13b2948ace")
    C.Check.str("omp turn rules first", R.yes(String.contains(R.nth(R.sends(r1), 3n), "<backplane_instructions>\\nRULES")), "yes")
    C.Check.str("omp turn started", R.yes(String.contains(R.emits(r1), "{\"type\":\"thread.started\",\"thread_id\":\"01a14112-89a7-7544-b120-9a13b2948ace\"}")), "yes")
    C.Check.str("omp turn finished", R.done(r1), "done")
    # no model and no thinking: the prompt straight after session/new
    +r2 : R.Run = replay(List.append(&2, String, [R.nth(fx, 0n), R.nth(fx, 1n)], Nil{}), R.Run.new(), turn("", "", ""))
    C.Check.str("omp plain order", String.join(R.methods(R.sends(r2)), ","), "session/new,session/prompt")
    C.Check.str("omp plain prompt", R.yes(String.contains(R.nth(R.sends(r2), 1n), "<backplane_instructions>")), "yes")
    # a resumed session the agent cannot load: session/new (id 7), the note, then the config steps renewed (ids 14 and 15), then the prompt (id 3)
    +renewed : R.Run = replay(["{\"jsonrpc\":\"2.0\",\"id\":2,\"error\":{\"code\":-32603,\"message\":\"session not found\"}}",
        "{\"jsonrpc\":\"2.0\",\"id\":7,\"result\":{\"sessionId\":\"NEW\"}}",
        "{\"jsonrpc\":\"2.0\",\"id\":14,\"result\":{}}",
        "{\"jsonrpc\":\"2.0\",\"id\":15,\"result\":{}}"],
      replay([R.nth(fx, 0n)], R.Run.new(), turn("OLD", "anthropic/claude-haiku-5-5", "high")), turn("OLD", "anthropic/claude-haiku-5-5", "high"))
    C.Check.str("omp renewed order", String.join(R.methods(R.sends(renewed)), ","), "session/load,session/new,session/set_config_option,session/set_config_option,session/prompt")
    C.Check.str("omp renewed ids", String.join(ids(R.sends(renewed)), ","), "2,7,14,15,3")
    C.Check.str("omp renewed model", J.Json.get_str(R.params(R.nth(R.sends(renewed), 2n)), "configId", "") ++ "=" ++ J.Json.get_str(R.params(R.nth(R.sends(renewed), 2n)), "value", ""), "model=anthropic/claude-haiku-5-5")
    C.Check.str("omp renewed thinking", J.Json.get_str(R.params(R.nth(R.sends(renewed), 3n)), "configId", "") ++ "=" ++ J.Json.get_str(R.params(R.nth(R.sends(renewed), 3n)), "value", ""), "thinking=high")
    C.Check.str("omp renewed session", J.Json.get_str(R.params(R.nth(R.sends(renewed), 2n)), "sessionId", ""), "NEW")
    C.Check.str("omp renewed rules", R.yes(String.contains(R.nth(R.sends(renewed), 4n), "<backplane_instructions>\\nRULES")), "yes")
    C.Check.str("omp renewed note", R.yes(String.contains(R.emits(renewed), "{\"type\":\"item.completed\",\"item\":{\"type\":\"backplane_note\",\"text\":\"The earlier session could not be resumed, so this turn starts a new one.\"}}")), "yes")
    # a config step the agent rejects fails the turn with its message, and no prompt is sent
    +badlevel : R.Run = replay(["{\"jsonrpc\":\"2.0\",\"id\":5,\"error\":{\"code\":-32602,\"message\":\"bad level\"}}"],
      replay(List.append(&2, String, [R.nth(fx, 0n), R.nth(fx, 1n), cfg4], Nil{}), R.Run.new(), turn("", "anthropic/claude-haiku-5-5", "high")), turn("", "anthropic/claude-haiku-5-5", "high"))
    C.Check.str("omp rejected level order", String.join(R.methods(R.sends(badlevel)), ","), "session/new,session/set_config_option,session/set_config_option")
    C.Check.str("omp rejected level fails", R.emits(badlevel), "{\"type\":\"thread.started\",\"thread_id\":\"01a14112-89a7-7544-b120-9a13b2948ace\"} | {\"type\":\"turn.failed\",\"message\":\"bad level\"}")
    C.Check.str("omp rejected level done", R.done(badlevel), "done")
    +badmodel : R.Run = replay(["{\"jsonrpc\":\"2.0\",\"id\":4,\"error\":{\"code\":-32602,\"message\":\"bad model\"}}"],
      replay([R.nth(fx, 0n), R.nth(fx, 1n)], R.Run.new(), turn("", "anthropic/claude-haiku-5-5", "high")), turn("", "anthropic/claude-haiku-5-5", "high"))
    C.Check.str("omp rejected model order", String.join(R.methods(R.sends(badmodel)), ","), "session/new,session/set_config_option")
    C.Check.str("omp rejected model fails", R.emits(badmodel), "{\"type\":\"thread.started\",\"thread_id\":\"01a14112-89a7-7544-b120-9a13b2948ace\"} | {\"type\":\"turn.failed\",\"message\":\"bad model\"}")
    # a session/new that names auth gets the omp login hint
    C.Check.str("omp auth hint", R.emits(replay(["{\"jsonrpc\":\"2.0\",\"id\":2,\"error\":{\"code\":-32000,\"message\":\"Not authenticated\"}}"], replay([R.nth(fx, 0n)], R.Run.new(), turn("", "", "")), turn("", "", ""))),
      "{\"type\":\"turn.failed\",\"message\":\"Not authenticated (sign in once with `omp login`)\"}")
    # the Codex item the runner reports: a backplane note is an info activity, not a message; an agent message is still a message
    C.Check.str("omp note item", item.of(B.Codex.item.k(4n, "T", 5n, 7n, R.obj("{\"type\":\"backplane_note\",\"text\":\"The earlier session could not be resumed, so this turn starts a new one.\"}"))),
      "act info resume The earlier session could not be resumed, so this turn starts a new one.")
    C.Check.str("omp message item", item.of(B.Codex.item.k(0n, "T", 5n, 7n, R.obj("{\"id\":\"m1\",\"type\":\"agent_message\",\"text\":\"hi\"}"))), "msg hi")
    # an omp error with no message names omp, not grok
    C.Check.str("omp error default", R.emits(replay(["{\"jsonrpc\":\"2.0\",\"id\":2,\"error\":{\"code\":-32000}}"], replay([R.nth(fx, 0n)], R.Run.new(), turn("", "", "")), turn("", "", ""))),
      "{\"type\":\"turn.failed\",\"message\":\"omp failed\"}")
    # an error reply to the prompt fails the turn with its message, and nothing is sent after it
    +badprompt : R.Run = replay(["{\"jsonrpc\":\"2.0\",\"id\":3,\"error\":{\"code\":-32000,\"message\":\"prompt rejected\"}}"],
      replay([R.nth(fx, 0n), R.nth(fx, 1n)], R.Run.new(), turn("", "", "")), turn("", "", ""))
    C.Check.str("omp prompt error sends", String.join(R.methods(R.sends(badprompt)), ","), "session/new,session/prompt")
    C.Check.str("omp prompt error fails", R.emits(badprompt), "{\"type\":\"thread.started\",\"thread_id\":\"01a14112-89a7-7544-b120-9a13b2948ace\"} | {\"type\":\"turn.failed\",\"message\":\"prompt rejected\"}")
    # omp's approval forms: an approval form is asked, its answer goes back as Approve or decline
    C.Check.str("omp form asks", ap.kind(ap.cs(ap.handle(ap.form("Approve the tool?", "[\"Approve\",\"Deny\"]")))), "approval|t1|Approve the tool?|Approve the tool?")
    C.Check.str("omp form parks its ask", Bool.show(Bool.and(Bool.not(String.is_empty(ap.id(ap.cs(ap.handle(ap.form("Approve the tool?", "[\"Approve\",\"Deny\"]")))))), String.eq(ap.id(ap.cs(ap.handle(ap.form("Approve the tool?", "[\"Approve\",\"Deny\"]")))), ap.park(ap.handle(ap.form("Approve the tool?", "[\"Approve\",\"Deny\"]")))))), "True")
    C.Check.str("omp form reply waits", ap.reply(ap.handle(ap.form("Approve the tool?", "[\"Approve\",\"Deny\"]"))), "")
    C.Check.str("omp form accept", MC.Mcp.permission.reply(ap.form("Approve the tool?", "[\"Approve\",\"Deny\"]"), "accept"), "{\"jsonrpc\":\"2.0\",\"id\":\"e1\",\"result\":{\"action\":\"accept\",\"content\":{\"value\":\"Approve\"}}}")
    C.Check.str("omp form decline", MC.Mcp.permission.reply(ap.form("Approve the tool?", "[\"Approve\",\"Deny\"]"), "decline"), "{\"jsonrpc\":\"2.0\",\"id\":\"e1\",\"result\":{\"action\":\"decline\"}}")
    C.Check.str("omp other form declines", ap.reply(ap.handle(ap.form("Pick one", "[\"Yes\",\"No\"]"))), "{\"jsonrpc\":\"2.0\",\"id\":\"e1\",\"result\":{\"action\":\"decline\"}}")
    C.Check.str("omp other form never asks", ap.kind(ap.cs(ap.handle(ap.form("Pick one", "[\"Yes\",\"No\"]")))), "")
    # the live omp form (test/fixtures/omp/form.jsonl line 64): the card's tool is the tool, its detail the whole message
    C.Check.str("omp live form card", ap.kind(ap.cs(ap.handle(ap.form("Allow tool: write\\nPath: form-check.txt\\nContent:\\nhello", "[\"Approve\",\"Deny\"]")))), "approval|t1|write|Allow tool: write\nPath: form-check.txt\nContent:\nhello")
    C.Check.str("omp form offers Allow and Deny only", ap.options(ap.cs(ap.handle(ap.form("Approve the tool?", "[\"Approve\",\"Deny\"]")))), "Allow\nDeny")
    C.Check.str("omp session request permission offers Allow and Deny", ap.options(ap.cs(ap.handle.provider("omp", ap.permission.request()))), "Allow\nDeny")
    C.Check.str("grok session request permission keeps default card", ap.options(ap.cs(ap.handle.provider("grok", ap.permission.request()))), "")
    C.Check.str("omp legacy session grant does not suppress permission ask", ap.options(ap.cs(ap.handle.provider.session("omp", ap.permission.request()))), "Allow\nDeny")
    C.Check.str("parked OMP allow cannot choose allow_always", PK.Park.reply(True{}, ap.permission.allow.always.only(), "accept"), "{\"jsonrpc\":\"2.0\",\"id\":\"p2\",\"result\":{\"outcome\":{\"outcome\":\"cancelled\"}}}")
    C.Check.str("parked OMP deny cannot choose reject_always", PK.Park.reply(True{}, ap.permission.reject.always.only(), "decline"), "{\"jsonrpc\":\"2.0\",\"id\":\"p3\",\"result\":{\"outcome\":{\"outcome\":\"cancelled\"}}}")
    C.Check.str("parked OMP allow chooses allow_once when offered", PK.Park.reply(True{}, ap.permission.request(), "accept"), "{\"jsonrpc\":\"2.0\",\"id\":\"p1\",\"result\":{\"outcome\":{\"outcome\":\"selected\",\"optionId\":\"yes\"}}}")
    C.Check.str("parked Grok session grant still selects allow_always", PK.Park.reply(False{}, ap.permission.request(), "session"), "{\"jsonrpc\":\"2.0\",\"id\":\"p1\",\"result\":{\"outcome\":{\"outcome\":\"selected\",\"optionId\":\"always\"}}}")
    C.Check.str("generic ACP session grant still selects allow_always", MC.Mcp.permission.reply(ap.permission.request(), "session"), "{\"jsonrpc\":\"2.0\",\"id\":\"p1\",\"result\":{\"outcome\":{\"outcome\":\"selected\",\"optionId\":\"always\"}}}")
    C.Check.str("parked OMP session answer cannot choose allow_always", PK.Park.reply(True{}, ap.permission.request(), "session"), "{\"jsonrpc\":\"2.0\",\"id\":\"p1\",\"result\":{\"outcome\":{\"outcome\":\"cancelled\"}}}")
    # Legacy clients may still send "session"; only Grok/Muse store that as a
    # persistent permission.
    +ompAsk : M.State = ask.state("omp")
    +ompAnswer : B.Step = B.Hub.answer(ompAsk, 3n, 0, "rpc", "q", "session")
    C.Check.str("omp legacy session answer is one-time", ask.result(ompAnswer), "accept|accept")
    +ompAfter : M.State = ask.applied(ompAsk, ompAnswer)
    C.Check.str("omp legacy session answer grants no future tools", Bool.show(MC.Perm.session_ok(M.Asks.all(ompAfter), "t", "write")), "False")
    +ompAllow : B.Step = B.Hub.answer(ompAsk, 3n, 0, "rpc", "q", "Allow")
    C.Check.str("omp Allow answers Approve", ask.result(ompAllow), "accept|accept")
    +ompAllowAfter : M.State = ask.applied(ompAsk, ompAllow)
    C.Check.str("omp Allow is one-time", Bool.show(MC.Perm.session_ok(M.Asks.all(ompAllowAfter), "t", "write")), "False")
    +ompDeny : B.Step = B.Hub.answer(ompAsk, 3n, 0, "rpc", "q", "Deny")
    C.Check.str("omp Deny declines", ask.result(ompDeny), "decline|decline")
    +grokAsk : M.State = ask.state("grok")
    +grokAnswer : B.Step = B.Hub.answer(grokAsk, 3n, 0, "rpc", "q", "session")
    C.Check.str("grok allow for session remains session-wide", ask.result(grokAnswer), "session|session")
    +grokAfter : M.State = ask.applied(grokAsk, grokAnswer)
    C.Check.str("grok session approval still grants future tools", Bool.show(MC.Perm.session_ok(M.Asks.all(grokAfter), "t", "write")), "True")
