import Base
import ./json.bend as J
import ./acp.bend as AC
import ./agy.bend as AG

# Muse Code over MSP (the Muse Session Protocol: JSON-RPC over stdio,
# `muse serve`), one process per turn: initialize, the initialized note,
# then session/start (or session/resume and session/setApprovalMode), then
# turn/start. What the agent says is reported in the same Codex event
# shapes as Grok's ACP (acp.bend), through the same AC.Acp state, so one
# runner (server.bend's Grok.*) drives both, by a Proto.

# what a turn needs: working directory, the thread's MCP endpoint, the
# Backplane instructions, the user's text, the session to resume ("" for a
# new one), the approval mode (Msp.mode), the reasoning effort ("" for the
# model's default) and a nonce unique to the turn (the hub's gen), which
# Muse's command ids are made from: Muse refuses a command id it has seen,
# in any session
type MspTurn is Data:
  MspTurn{cwd: String, mcp: String, prompt: String, text: String, session: String, mode: String, effort: String, nonce: String}

def Msp.rpc(+id: Nat, +method: String, params: J.Json) -> String:
  AC.Acp.rpc(id, method, params)

def Msp.note(+method: String, params: J.Json) -> String:
  J.Json.show(J.Json.obj([J.KV{"jsonrpc", J.Str{"2.0"}}, J.KV{"method", J.Str{method}}, J.KV{"params", params}]))

# Command ids
# -----------
# UUID-shaped: 32 hex digits from the turn's nonce (its digits, padded),
# the last but one naming the command (1 open, 2 mode, 3 turn).

def Msp.digit(+c: U32, +rest: String) -> String:
  Bool.pick(String, Bool.and(U32.is_ge(c, 48), U32.is_le(c, 57)), SCon{Chr{c}, rest}, rest)

def Msp.digits(s: String) -> String:
  match s:
    case SNil{}:
      SNil{}
    case SCon{Chr{+c}, t}:
      Msp.digit(c, Msp.digits(t))

def Msp.zeros(n: Nat, s: String) -> String:
  match n:
    case 0n:
      s
    case 1n+k:
      Msp.zeros(k, "0" ++ s)

# s as exactly n characters: zeros before it, or its last n
def Msp.pad(+s: String, +n: Nat) -> String:
  Bool.pick(String, Nat.is_ge(String.length(s), n), String.drop(s, Nat.sub(String.length(s), n)), Msp.zeros(Nat.sub(n, String.length(s)), s))

def Msp.cut(+x: String, +a: Nat, +n: Nat) -> String:
  String.take(String.drop(x, a), n)

# 32 hex digits as a UUID
def Msp.fmt(+x: String) -> String:
  Msp.cut(x, 0n, 8n) ++ "-" ++ Msp.cut(x, 8n, 4n) ++ "-" ++ Msp.cut(x, 12n, 4n) ++ "-" ++ Msp.cut(x, 16n, 4n) ++ "-" ++ Msp.cut(x, 20n, 12n)

def Msp.uuid.k(+x: String, +k: String) -> String:
  Msp.fmt(Msp.cut(x, 0n, 12n) ++ "7" ++ Msp.cut(x, 12n, 3n) ++ "8" ++ Msp.cut(x, 15n, 13n) ++ k ++ "0")

def Msp.uuid(+nonce: String, +k: String) -> String:
  Msp.uuid.k(Msp.pad(Msp.digits(nonce), 28n), k)

# Requests
# --------

# the first line of every turn; MCP servers per session need sessionMcp,
# and questions are not offered (Backplane answers no userInput/request)
def Msp.init() -> String:
  Msp.rpc(1n, "initialize", J.Json.obj([
    J.KV{"clientInfo", J.Json.obj([J.KV{"name", J.Str{"backplane"}}, J.KV{"title", J.Str{"Backplane"}}, J.KV{"version", J.Str{"1"}}])},
    J.KV{"capabilities", J.Json.obj([J.KV{"requestedCapabilities", J.Json.arr([J.Str{"sessionMcp"}])}, J.KV{"userInputDialogs", J.Flag{False{}}}])}]))

def Msp.config(+url: String) -> J.Json:
  J.Json.obj([J.KV{"mcpServers", J.Json.obj([J.KV{"backplane", J.Json.obj([J.KV{"transport", J.Str{"streamableHttp"}}, J.KV{"url", J.Str{url}}, J.KV{"mode", J.Str{"optional"}}])}])}])

def Msp.session(t: MspTurn) -> String:
  match t:
    case MspTurn{_, _, _, _, s, _, _, _}:
      s

def Msp.open.k(fresh: Bool, t: MspTurn) -> String:
  match fresh t:
    case True{} MspTurn{cwd, mcp, _, _, _, mode, _, nonce}:
      Msp.rpc(2n, "session/start", J.Json.obj([J.KV{"commandId", J.Str{Msp.uuid(nonce, "1")}}, J.KV{"workspaceRoot", J.Str{cwd}},
        J.KV{"approvalMode", J.Str{mode}}, J.KV{"config", Msp.config(mcp)}]))
    case False{} MspTurn{_, mcp, _, _, sid, _, _, nonce}:
      Msp.rpc(2n, "session/resume", J.Json.obj([J.KV{"commandId", J.Str{Msp.uuid(nonce, "1")}}, J.KV{"sessionId", J.Str{sid}},
        J.KV{"excludeItems", J.Flag{True{}}}, J.KV{"config", Msp.config(mcp)}]))

# a new session or the thread's own
def Msp.open(+t: MspTurn) -> String:
  Msp.open.k(String.is_empty(Msp.session(t)), t)

def Msp.effort(+e: String) -> List<&2, J.KV>:
  Bool.pick(List<&2, J.KV>, String.is_empty(e), Nil{}, [J.KV{"reasoningEffort", J.Str{e}}])

def Msp.say.rpc(+sid: String, +nonce: String, +effort: String, +text: String, +shown: String) -> String:
  Msp.rpc(4n, "turn/start", J.Json.obj(List.append(&2, J.KV, [J.KV{"commandId", J.Str{Msp.uuid(nonce, "3")}}, J.KV{"sessionId", J.Str{sid}},
    J.KV{"input", J.Json.arr([J.Json.obj([J.KV{"type", J.Str{"text"}}, J.KV{"text", J.Str{text}}])])}, J.KV{"displayText", J.Str{shown}}], Msp.effort(effort))))

def Msp.mode.rpc(+sid: String, +nonce: String, +mode: String) -> String:
  Msp.rpc(3n, "session/setApprovalMode", J.Json.obj([J.KV{"commandId", J.Str{Msp.uuid(nonce, "2")}}, J.KV{"sessionId", J.Str{sid}}, J.KV{"mode", J.Str{mode}}]))

# the turn: a new session gets Backplane's instructions first; a resumed
# one is set to the thread's approval mode (it may have changed) first
def Msp.say.k(fresh: Bool, t: MspTurn, +sid: String) -> List<&2, String>:
  match fresh t:
    case True{} MspTurn{_, _, pr, +text, _, _, effort, +nonce}:
      [Msp.say.rpc(sid, nonce, effort, "<backplane_instructions>\n" ++ pr ++ "\n</backplane_instructions>\n\n" ++ text, text)]
    case False{} MspTurn{_, _, _, +text, _, mode, effort, +nonce}:
      [Msp.mode.rpc(sid, nonce, mode), Msp.say.rpc(sid, nonce, effort, text, text)]

# Replies to our requests (by id)
# -------------------------------

def Msp.error(o: J.Json) -> String:
  J.Json.get_str(J.Json.get_obj(o, "error"), "message", "muse failed")

# a sign-in problem says how to fix it
def Msp.hint(+msg: String) -> String:
  Bool.pick(String, Bool.or(String.contains(String.to_lower(msg), "auth"), String.contains(String.to_lower(msg), "login")),
    msg ++ " (sign in once with `muse login`)", msg)

def Msp.fail(a: AC.Acp, +msg: String) -> AC.AcpOut:
  match a:
    case AC.Acp{s, b, _}:
      AC.AcpOut{AC.Acp{s, b, True{}}, Nil{}, [AC.Acp.failed(Msp.hint(msg))], Nil{}}

def Msp.none(a: AC.Acp) -> AC.AcpOut:
  AC.AcpOut{a, Nil{}, Nil{}, Nil{}}

def Msp.opened(a: AC.Acp, +t: MspTurn, +sid: String) -> AC.AcpOut:
  match a:
    case AC.Acp{_, b, d}:
      AC.AcpOut{AC.Acp{sid, b, d}, Msp.say.k(String.is_empty(Msp.session(t)), t, sid), [AC.Acp.event("thread.started", [J.KV{"thread_id", J.Str{sid}}])], Nil{}}

# a failed initialize, open or turn/start ends the turn; the mode's and the
# approvals' answers say nothing that matters
def Msp.reply.k(id: Nat, ok: Bool, a: AC.Acp, +t: MspTurn, +o: J.Json) -> AC.AcpOut:
  match id ok:
    case 1n True{}:
      AC.AcpOut{a, [Msp.note("initialized", J.Json.obj(Nil{})), Msp.open(t)], Nil{}, Nil{}}
    case 2n True{}:
      Msp.opened(a, t, J.Json.get_str(J.Json.get_obj(J.Json.get_obj(o, "result"), "session"), "sessionId", Msp.session(t)))
    case 1n False{}:
      Msp.fail(a, Msp.error(o))
    case 2n False{}:
      Msp.fail(a, Msp.error(o))
    case 4n False{}:
      Msp.fail(a, Msp.error(o))
    case _ _:
      Msp.none(a)

# Notifications: the transcript as it happens
# -------------------------------------------

def Msp.first(xs: List<&2, String>, +o: J.Json) -> String:
  match xs:
    case Nil{}:
      ""
    case +k <> t:
      AC.Acp.or(J.Json.get_str(o, k, ""), Msp.first(t, o))

def Msp.cmd.k(+tool: String, +args: J.Json) -> String:
  AC.Acp.or(J.Json.get_str(args, "command", ""),
    Bool.pick(String, String.is_empty(Msp.first(["path", "file_path", "pattern", "query", "url"], args)), tool, tool ++ " " ++ Msp.first(["path", "file_path", "pattern", "query", "url"], args)))

# what a tool call ran, for its row: a shell's command, else the tool and
# what it was pointed at
def Msp.cmd(+it: J.Json) -> String:
  Msp.cmd.k(J.Json.get_str(it, "tool", "tool"), Maybe.default(&2, J.Json, J.Json.parse(J.Json.get_str(it, "args", "")), J.Null{}))

def Msp.row(+id: String, +text: String) -> String:
  AC.Acp.event("item.completed", [J.KV{"item", J.Json.obj([J.KV{"type", J.Str{"command_execution"}}, J.KV{"id", J.Str{id}}, J.KV{"command", J.Str{text}}])}])

# an item opens: a tool call or a subagent is logged as it starts
def Msp.started.k(i: Nat, a: AC.Acp, +it: J.Json) -> AC.AcpOut:
  match i:
    case 0n:
      AC.AcpOut{a, Nil{}, [Msp.row(J.Json.get_str(it, "itemId", ""), Msp.cmd(it))], Nil{}}
    case 1n:
      AC.AcpOut{a, Nil{}, [Msp.row(J.Json.get_str(it, "itemId", ""), "agent: " ++ J.Json.get_str(it, "objective", J.Json.get_str(it, "role", "subagent")))], Nil{}}
    case _:
      Msp.none(a)

def Msp.message(+text: String) -> List<&2, String>:
  Bool.pick(List<&2, String>, String.is_empty(String.trim(text)), Nil{},
    [AC.Acp.event("item.completed", [J.KV{"item", J.Json.obj([J.KV{"type", J.Str{"agent_message"}}, J.KV{"text", J.Str{text}}])}])])

# an item ends: a reply is a message; a tool call that has run is
# "tool.done" (the hub snapshots the design then, hub.bend's Steps)
def Msp.completed.k(i: Nat, a: AC.Acp, +it: J.Json) -> AC.AcpOut:
  match i:
    case 0n:
      AC.AcpOut{a, Nil{}, Msp.message(J.Json.get_str(it, "text", "")), Nil{}}
    case 1n:
      AC.AcpOut{a, Nil{}, [AC.Acp.event("tool.done", [J.KV{"id", J.Str{J.Json.get_str(it, "itemId", "")}}])], Nil{}}
    case _:
      Msp.none(a)

# a reply's text streams as field "text" (absent means "text"); reasoning
# and tool output stream on other fields
def Msp.delta(text: Bool, a: AC.Acp, +p: J.Json) -> AC.AcpOut:
  match text:
    case True{}:
      AC.AcpOut{a, Nil{}, [AC.Acp.event("item.delta", [J.KV{"text", J.Str{J.Json.get_str(p, "delta", "")}}])], Nil{}}
    case False{}:
      Msp.none(a)

def Msp.status(+s: String) -> String:
  Bool.pick(String, String.eq(s, "inProgress"), "in_progress", s)

def Msp.todos(xs: List<&2, J.Json>) -> List<&2, J.Json>:
  match xs:
    case Nil{}:
      Nil{}
    case +x <> t:
      J.Json.obj([J.KV{"text", J.Str{J.Json.get_str(x, "text", "")}}, J.KV{"status", J.Str{Msp.status(J.Json.get_str(x, "status", "pending"))}}]) <> Msp.todos(t)

def Msp.done(a: AC.Acp) -> AC.Acp:
  match a:
    case AC.Acp{s, b, _}:
      AC.Acp{s, b, True{}}

def Msp.ended(ok: Bool, a: AC.Acp, +p: J.Json) -> AC.AcpOut:
  match ok:
    case True{}:
      AC.AcpOut{Msp.done(a), Nil{}, [AC.Acp.event("turn.completed", Nil{})], Nil{}}
    case False{}:
      AC.AcpOut{Msp.done(a), Nil{}, [AC.Acp.failed(Msp.hint(J.Json.get_str(J.Json.get_obj(p, "error"), "message",
        J.Json.get_str(p, "reason", "muse turn " ++ J.Json.get_str(p, "terminal", "failed")))))], Nil{}}

def Msp.note.k(i: Nat, a: AC.Acp, +p: J.Json) -> AC.AcpOut:
  match i:
    case 0n:
      Msp.delta(String.eq(J.Json.get_str(p, "field", "text"), "text"), a, p)
    case 1n:
      Msp.started.k(AC.Acp.pos(["toolCall", "subagent"], J.Json.get_str(J.Json.get_obj(p, "item"), "kind", ""), 0n), a, J.Json.get_obj(p, "item"))
    case 2n:
      Msp.completed.k(AC.Acp.pos(["agentMessage", "toolCall"], J.Json.get_str(J.Json.get_obj(p, "item"), "kind", ""), 0n), a, J.Json.get_obj(p, "item"))
    case 3n:
      Msp.ended(String.eq(J.Json.get_str(p, "terminal", ""), "completed"), a, p)
    case 4n:
      AC.AcpOut{a, Nil{}, [AC.Acp.event("item.updated", [J.KV{"item", J.Json.obj([J.KV{"type", J.Str{"todo_list"}}, J.KV{"items", J.Json.arr(Msp.todos(J.Json.elems(J.Json.get_obj(p, "items"))))}])}])], Nil{}}
    case _:
      Msp.none(a)

def Msp.sid(a: AC.Acp) -> String:
  match a:
    case AC.Acp{s, _, _}:
      s

# only this session's events, once it is open (a subagent's session and a
# resume's history are not this turn's)
def Msp.mine(mine: Bool, a: AC.Acp, +o: J.Json) -> AC.AcpOut:
  match mine:
    case True{}:
      Msp.note.k(AC.Acp.pos(["item/delta", "item/started", "item/completed", "turn/completed", "session/todoListChanged"], J.Json.get_str(o, "method", ""), 0n), a, J.Json.get_obj(o, "params"))
    case False{}:
      Msp.none(a)

# Server requests
# ---------------
# approval/request is answered with a receipt at once; the decision goes
# later, as approval/decide, once the user answers (Msp.decide). Nothing
# else is asked of a client that offered no question dialogs.

def Msp.receipt(+id: String) -> String:
  "{\"jsonrpc\":\"2.0\",\"id\":" ++ id ++ ",\"result\":{}}"

def Msp.request.k(approval: Bool, a: AC.Acp, +id: String, +o: J.Json) -> AC.AcpOut:
  match approval:
    case True{}:
      AC.AcpOut{a, [Msp.receipt(id)], Nil{}, [J.Json.show(o)]}
    case False{}:
      AC.AcpOut{a, ["{\"jsonrpc\":\"2.0\",\"id\":" ++ id ++ ",\"error\":{\"code\":-32601,\"message\":\"not supported\"}}"], Nil{}, Nil{}}

def Msp.method.k(req: Bool, +a: AC.Acp, +o: J.Json) -> AC.AcpOut:
  match req:
    case True{}:
      Msp.request.k(String.eq(J.Json.get_str(o, "method", ""), "approval/request"), a, AC.Acp.id(J.Json.get(o, "id")), o)
    case False{}:
      Msp.mine(Bool.and(Bool.not(String.is_empty(Msp.sid(a))), String.eq(J.Json.get_str(J.Json.get_obj(o, "params"), "sessionId", ""), Msp.sid(a))), a, o)

def Msp.line.k(method: Bool, a: AC.Acp, +t: MspTurn, +o: J.Json) -> AC.AcpOut:
  match method:
    case True{}:
      Msp.method.k(Bool.not(String.eq(AC.Acp.id(J.Json.get(o, "id")), "")), a, o)
    case False{}:
      Msp.reply.k(Maybe.default(&2, Nat, Nat.read(J.Json.as_raw(J.Json.get(o, "id"), "0")), 0n), String.eq(J.Json.show(J.Json.get_obj(o, "error")), "null"), a, t, o)

def Msp.line.parsed(m: Maybe<&2, J.Json>, a: AC.Acp, +t: MspTurn) -> AC.AcpOut:
  match m:
    case Some{+o}:
      Msp.line.k(Bool.not(String.is_empty(J.Json.get_str(o, "method", ""))), a, t, o)
    case None{}:
      Msp.none(a)

# one line from the agent
def Msp.line(a: AC.Acp, +t: MspTurn, line: String) -> AC.AcpOut:
  Msp.line.parsed(J.Json.parse(line), a, t)

# Approval answers
# ----------------
# "accept" picks the approve-once choice, "session" the approve-for-session
# one, anything else a denial; each falls back to its neighbour.

def Msp.choice(cs: List<&2, J.Json>, +decision: String) -> String:
  match cs:
    case Nil{}:
      ""
    case +c <> t:
      Bool.pick(String, String.eq(J.Json.get_str(c, "decision", ""), decision), J.Json.get_str(c, "choiceId", ""), Msp.choice(t, decision))

def Msp.choose(+answer: String, +cs: List<&2, J.Json>) -> String:
  Bool.pick(String, String.eq(answer, "accept"), AC.Acp.or(Msp.choice(cs, "approved"), Msp.choice(cs, "approvedForSession")),
    Bool.pick(String, String.eq(answer, "session"), AC.Acp.or(Msp.choice(cs, "approvedForSession"), Msp.choice(cs, "approved")),
      AC.Acp.or(Msp.choice(cs, "denied"), Msp.choice(cs, "abort"))))

def Msp.hex.k(+c: U32, +rest: String) -> String:
  Bool.pick(String, U32.is_eq(c, 45), rest, SCon{Chr{c}, rest})

# a UUID's hex digits
def Msp.hex(s: String) -> String:
  match s:
    case SNil{}:
      SNil{}
    case SCon{Chr{+c}, t}:
      Msp.hex.k(c, Msp.hex(t))

# a decision's command id: the approval's own id, ending in the record the
# request came from (each stage of one approval is a request of its own)
def Msp.decide.id(+approval: String, +seq: String) -> String:
  Msp.fmt(String.take(Msp.pad(Msp.hex(approval), 32n), 20n) ++ Msp.pad(Msp.digits(seq), 12n))

def Msp.decide.k(+p: J.Json, +answer: String) -> String:
  J.Json.show(J.Json.obj([J.KV{"jsonrpc", J.Str{"2.0"}}, J.KV{"id", J.Str{"decide-" ++ J.Json.get_str(p, "approvalId", "")}}, J.KV{"method", J.Str{"approval/decide"}},
    J.KV{"params", J.Json.obj([J.KV{"approvalId", J.Str{J.Json.get_str(p, "approvalId", "")}},
      J.KV{"choiceId", J.Str{Msp.choose(answer, J.Json.elems(J.Json.get_obj(p, "availableChoices")))}},
      J.KV{"commandId", J.Str{Msp.decide.id(J.Json.get_str(p, "approvalId", ""), J.Json.as_raw(J.Json.get(J.Json.get_obj(J.Json.get_obj(p, "sourceRange"), "last"), "sequence"), "0"))}},
      J.KV{"requirementId", J.Json.get_obj(p, "currentRequirementId")},
      J.KV{"sessionId", J.Str{J.Json.get_str(p, "sessionId", "")}}])}]))

# the decision for a parked approval/request line, once the user answered
def Msp.decide(+body: String, +answer: String) -> String:
  Msp.decide.k(J.Json.get_obj(Maybe.default(&2, J.Json, J.Json.parse(body), J.Null{}), "params"), answer)

# what an approval asks to run: the tool (what "Allow for session"
# remembers) and a line for the approval card
def Msp.tool(+p: J.Json) -> String:
  AC.Acp.or(J.Json.get_str(p, "toolName", ""), AC.Acp.or(J.Json.get_str(J.Json.get_obj(p, "subject"), "toolName", ""), J.Json.get_str(J.Json.get_obj(p, "subject"), "kind", "tool")))

def Msp.detail(+p: J.Json) -> String:
  AC.Acp.or(Msp.first(["command", "path", "target", "host"], J.Json.get_obj(p, "subject")), String.take(J.Json.get_str(p, "rawArgs", ""), 400n))

# Approval modes
# --------------
# Supervised asks for anything Muse's policy does not already allow;
# accept-edits and auto let Muse (and its judge) decide when to ask; full
# access allows everything.

def Msp.mode.k(i: Nat) -> String:
  match i:
    case 0n:
      "promptUnmatched"
    case 3n:
      "allowAll"
    case _:
      "onRequest"

# The protocol a runner speaks
# ----------------------------
# Grok over ACP, Muse over MSP, Google's Antigravity agent over ACP
# (agy.bend, through its runner script) or opencode over ACP (through
# `env`, its config in OPENCODE_CONFIG_CONTENT): the same process
# handling, its own lines, command and words.

type Proto is Data:
  PAcp{t: AC.AcpTurn}
  PMsp{t: MspTurn}
  PAgy{t: AG.AgyTurn}
  POc{t: AC.AcpTurn}

def Proto.line(a: AC.Acp, +p: Proto, line: String) -> AC.AcpOut:
  match p:
    case PAcp{t}:
      AC.Acp.line(a, t, line)
    case PMsp{t}:
      Msp.line(a, t, line)
    case PAgy{t}:
      AG.Agy.line(a, t, line)
    case POc{t}:
      AC.Acp.line(a, t, line)

def Proto.init(p: Proto) -> String:
  match p:
    case PAcp{_}:
      AC.Acp.init()
    case PMsp{_}:
      Msp.init()
    case PAgy{_}:
      AG.Agy.init()
    case POc{_}:
      AC.Acp.init()

# the program it runs (Antigravity's runner is a script, agy.bend)
def Proto.cmd(p: Proto) -> String:
  match p:
    case PAcp{_}:
      "grok"
    case PMsp{_}:
      "muse"
    case PAgy{_}:
      "sh"
    case POc{_}:
      "env"

# its name in what the person reads
def Proto.name(p: Proto) -> String:
  match p:
    case PAcp{_}:
      "grok"
    case PMsp{_}:
      "muse"
    case PAgy{_}:
      "antigravity"
    case POc{_}:
      "opencode"

def Proto.install(p: Proto) -> String:
  match p:
    case PAcp{_}:
      "Install Grok Build with `curl -fsSL https://x.ai/cli/install.sh | bash`, then `grok login`."
    case PMsp{_}:
      "Install Muse Code, then sign in with `muse login`."
    case PAgy{_}:
      "Install it under Settings › Antigravity."
    case POc{_}:
      "Install opencode (https://opencode.ai), then sign in with `opencode auth login`."

# why a process that ended before its turn did may have
def Proto.gone(p: Proto) -> String:
  match p:
    case PAcp{_}:
      "grok exited before the turn ended (is it signed in? run `grok login`)"
    case PMsp{_}:
      "muse exited before the turn ended (is it signed in? run `muse login`)"
    case PAgy{_}:
      "antigravity exited before the turn ended (is it installed and signed in? see Settings › Antigravity)"
    case POc{_}:
      "opencode exited before the turn ended (is it signed in? run `opencode auth login`)"
