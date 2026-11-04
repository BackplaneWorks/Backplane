import Base
import ./json.bend as J
import ./acp.bend as AC
import ./dialog.bend as DL
import ./pi.bend as PI

# Oh My Pi (`omp --mode rpc-ui`, can1357/oh-my-pi 18.12)
# -------------------------------------------------------
# omp is a fork of Pi with its own RPC dialect (docs/rpc.md of
# can1357/oh-my-pi 18.12.6): JSON commands on stdin, a `ready` frame, then
# responses and session events on stdout, one a line, the events and the
# extension dialogs Pi's (pi.bend), so most lines are read the way Pi's
# are. One process per turn, on Grok's runner (msp.bend's POmp):
#   0 set_event_filter: message updates as deltas only (no snapshot of the
#     whole reply in every line);
#   1 get_state: the session omp opened (the newest in the thread's own
#     session folder, `--session-dir` with `--continue`, a new one when the
#     folder is empty);
#   2 prompt: the person's text; then its events until prompt_result;
#   3 get_session_stats: the session's running totals, reported with
#     turn.completed (hub.bend's Turn.codex takes the turn's share) and its
#     context fill (context.usage).
# What it says is reported in the Codex event shapes (acp.bend).
#
# Approvals are omp's own (`--approval-mode`, from the thread's mode): a
# tool its mode asks for comes as an extension_ui_request "select" titled
# "Allow tool: <name>" with the details under it and the choices Approve,
# Approve <name> for this session and Deny; it goes to the person as an
# approval and the choice made goes back (dialog.bend). Other selects
# (omp's `ask` questions), confirms, inputs and editors go as questions.
# Backplane's own tools come from an extension Backplane writes (Omp.ext,
# loaded with `-e`): it lists the thread's Backplane MCP tools and
# registers each under the name Claude sees (mcp__backplane__<tool>),
# calling the MCP server for it; they ask for no approval (the hub has its
# own rules for them). Instructions go in with `--append-system-prompt` at
# every start as a private file path, never into argv or the conversation.

# the turn: Grok's (cwd, MCP endpoint, text, session, whether tools ask)
# with the thread's model ("provider/id", "" for omp's default) and
# thinking level ("" for omp's default); the model and level go as flags
type OmpTurn is Data:
  OmpTurn{acp: AC.AcpTurn, model: String, effort: String}

def Omp.acp(t: OmpTurn) -> AC.AcpTurn:
  match t:
    case OmpTurn{a, _, _}:
      a

def Omp.text(t: AC.AcpTurn) -> String:
  match t:
    case AC.AcpTurn{_, _, _, x, _, _}:
      x

# the approval mode (`--approval-mode`): supervised (0) asks before writes
# and commands (always-ask: only reading runs unasked), accept edits (1)
# lets writes and edits run (write), auto (2) and full access (3) ask
# nothing (yolo)
def Omp.mode.k(i: Nat) -> String:
  match i:
    case 0n:
      "always-ask"
    case 1n:
      "write"
    case _:
      "yolo"

# the first lines: deltas only, then the session
def Omp.init() -> String:
  PI.Pi.cmd("0", "set_event_filter", [J.KV{"events", J.Null{}}, J.KV{"messageUpdates", J.Str{"delta"}}]) ++ "\n" ++ PI.Pi.init()

def Omp.prompt(+t: AC.AcpTurn) -> String:
  PI.Pi.cmd("2", "prompt", [J.KV{"message", J.Str{Omp.text(t)}}])

def Omp.stats() -> String:
  PI.Pi.cmd("3", "get_session_stats", Nil{})

# a thread's session folder (under Backplane's home), from its id
def Omp.dir(+home: String, +thread: String) -> String:
  home ++ "/omp/sessions/" ++ PI.Pi.safe(thread)

# The extension, as omp loads it (plain ES module JavaScript): the thread's
# Backplane MCP tools (BACKPLANE_OMP_MCP), registered when omp starts
def Omp.ext() -> String:
  String.join([
    "// Written by Backplane for its Oh My Pi threads; rewritten before every turn.",
    "// Backplane's MCP tools for this thread (BACKPLANE_OMP_MCP), as omp tools named",
    "// as Claude sees them (mcp__backplane__<tool>), each a call to the MCP server.",
    "export default async function (pi) {",
    "  const url = process.env.BACKPLANE_OMP_MCP || '';",
    "  if (!url) return;",
    "  let session = '', n = 0;",
    "  const call = async (method, params, signal) => {",
    "    const headers = { 'content-type': 'application/json', accept: 'application/json, text/event-stream' };",
    "    if (session) headers['mcp-session-id'] = session;",
    "    const id = ++n;",
    "    const r = await fetch(url, { method: 'POST', headers, signal, body: JSON.stringify({ jsonrpc: '2.0', id, method, params }) });",
    "    session = r.headers.get('mcp-session-id') || session;",
    "    const text = (await r.text()).trim();",
    "    const body = text.startsWith('{') ? text : (text.split('\\n').filter(l => l.startsWith('data:')).map(l => l.slice(5)).pop() || '{}');",
    "    const o = JSON.parse(body);",
    "    if (o.error) throw new Error(o.error.message || 'Backplane MCP error');",
    "    return o.result || {};",
    "  };",
    "  let tools = [];",
    "  try {",
    "    await call('initialize', { protocolVersion: '2025-03-26', capabilities: {}, clientInfo: { name: 'backplane-omp', version: '1' } });",
    "    tools = (await call('tools/list', {})).tools || [];",
    "  } catch (e) {",
    "    pi.on('session_start', (_e, ctx) => ctx.ui.notify('Backplane tools are not available: ' + String(e && e.message || e), 'warning'));",
    "    return;",
    "  }",
    "  for (const t of tools) {",
    "    pi.registerTool({",
    "      name: 'mcp__backplane__' + t.name, label: 'backplane/' + t.name,",
    "      description: t.description || ('Backplane: ' + t.name),",
    "      parameters: t.inputSchema || { type: 'object', properties: {} },",
    "      approval: 'read', loadMode: 'essential', strict: false,",
    "      async execute(_id, params, signal) {",
    "        const r = await call('tools/call', { name: t.name, arguments: params || {} }, signal);",
    "        return { content: Array.isArray(r.content) ? r.content : [{ type: 'text', text: JSON.stringify(r) }], isError: !!r.isError };",
    "      },",
    "    });",
    "  }",
    "}",
    ""], "\n")

# Failures
# --------

def Omp.signin(+low: String) -> Bool:
  Bool.or(Bool.or(String.contains(low, "no api key"), String.contains(low, "no model selected")), Bool.or(String.contains(low, "/login"), String.contains(low, "unauthorized")))

def Omp.hint.k(+low: String, +msg: String) -> String:
  Bool.pick(String, String.contains(low, "model not found"), msg ++ " (pick another Oh My Pi model in the composer)",
    Bool.pick(String, Omp.signin(low), msg ++ " (sign Oh My Pi in once: run `omp` and use /login, or set the provider's API key)", msg))

def Omp.hint(+msg: String) -> String:
  Omp.hint.k(String.to_lower(String.take(msg, 400n)), msg)

def Omp.fail(a: AC.Acp, +msg: String) -> AC.AcpOut:
  AC.AcpOut{PI.Pi.close(a), Nil{}, [AC.Acp.failed(Omp.hint(msg))], Nil{}}

def Omp.notice(+msg: String) -> String:
  AC.Acp.event("notice", [J.KV{"message", J.Str{msg}}])

# Replies to our commands (by id)
# -------------------------------

# a resumed thread whose folder holds no conversation (it was removed, or
# the thread's earlier turns ran on omp's ACP) starts afresh, said once
def Omp.lost(resumed: Bool, +count: String) -> List<&2, String>:
  Bool.pick(List<&2, String>, Bool.and(resumed, String.eq(count, "0")),
    [Omp.notice("Oh My Pi had no saved conversation for this thread, so this turn starts without the earlier context. This thread's history in Backplane is kept.")], Nil{})

def Omp.opened.k(a: AC.Acp, +t: AC.AcpTurn, +sid: String, +count: String) -> AC.AcpOut:
  AC.AcpOut{PI.Pi.with_sid(a, sid), [Omp.prompt(t)],
    AC.Acp.event("thread.started", [J.KV{"thread_id", J.Str{sid}}]) <> Omp.lost(Bool.not(String.is_empty(AC.Acp.session(t))), count), Nil{}}

def Omp.opened(a: AC.Acp, +t: AC.AcpTurn, +d: J.Json) -> AC.AcpOut:
  Omp.opened.k(a, t, J.Json.get_str(d, "sessionId", AC.Acp.session(t)), J.Json.as_raw(J.Json.get(d, "messageCount"), "0"))

# a prompt omp finished itself (a builtin slash command) starts no run
def Omp.local(invoked: Bool, a: AC.Acp) -> AC.AcpOut:
  match invoked:
    case True{}:
      PI.Pi.none(a)
    case False{}:
      AC.Acp.finished(a)

# the session's running totals as Codex's (input counts what was read from
# the cache, which Turn.codex takes out again), its cost, and the context
# it holds now
def Omp.usage(+d: J.Json) -> J.Json:
  +tk = J.Json.get_obj(d, "tokens")
  J.Json.obj([J.KV{"input_tokens", J.Json.nat(Nat.add(J.Json.get_nat(tk, "input", 0n), J.Json.get_nat(tk, "cacheRead", 0n)))},
    J.KV{"cached_input_tokens", J.Json.nat(J.Json.get_nat(tk, "cacheRead", 0n))}, J.KV{"cache_write_input_tokens", J.Json.nat(J.Json.get_nat(tk, "cacheWrite", 0n))},
    J.KV{"output_tokens", J.Json.nat(J.Json.get_nat(tk, "output", 0n))}, J.KV{"reasoning_output_tokens", J.Json.nat(J.Json.get_nat(tk, "reasoning", 0n))},
    J.KV{"cost_usd", J.Num{J.Json.as_raw(J.Json.get(d, "cost"), "0")}}])

def Omp.context(+d: J.Json) -> String:
  AC.Acp.event("context.usage", [J.KV{"used", J.Json.nat(J.Json.get_nat(J.Json.get_obj(d, "contextUsage"), "tokens", 0n))},
    J.KV{"window", J.Json.nat(J.Json.get_nat(J.Json.get_obj(d, "contextUsage"), "contextWindow", 0n))}])

# the reply, whole (it streamed as deltas), when there is one
def Omp.reply(+b: String) -> List<&2, String>:
  Bool.pick(List<&2, String>, String.is_empty(b), Nil{}, [AC.Acp.event("item.completed", [J.KV{"item", J.Json.obj([J.KV{"type", J.Str{"agent_message"}}, J.KV{"text", J.Str{b}}])}])])

# the turn is over: the reply, its usage, then the end
def Omp.counted(+a: AC.Acp, +d: J.Json) -> AC.AcpOut:
  AC.AcpOut{PI.Pi.close(a), Nil{}, List.append(&2, String, Omp.reply(PI.Pi.buf(a)), [Omp.context(d), AC.Acp.event("turn.completed", [J.KV{"usage", Omp.usage(d)}])]), Nil{}}

def Omp.reply.k(i: Nat, ok: Bool, a: AC.Acp, +t: AC.AcpTurn, +o: J.Json) -> AC.AcpOut:
  match i ok:
    case 1n True{}:
      Omp.opened(a, t, J.Json.get_obj(o, "data"))
    case 2n True{}:
      Omp.local(J.Json.get_bool(J.Json.get_obj(o, "data"), "agentInvoked", True{}), a)
    case 3n True{}:
      Omp.counted(a, J.Json.get_obj(o, "data"))
    case 3n False{}:
      AC.Acp.finished(a)
    case 0n False{}:
      PI.Pi.none(a)
    case _ False{}:
      Omp.fail(a, J.Json.get_str(o, "error", "omp refused the command"))
    case _ True{}:
      PI.Pi.none(a)

# the prompt's run is over: counted when it completed, else failed
def Omp.result.k(+status: String, +o: J.Json, +a: AC.Acp) -> AC.AcpOut:
  Bool.pick(AC.AcpOut, String.eq(status, "completed"), AC.AcpOut{a, [Omp.stats()], Nil{}, Nil{}},
    Omp.fail(a, Bool.pick(String, String.eq(status, "aborted"), "Oh My Pi aborted the turn",
      AC.Acp.or(J.Json.get_str(J.Json.get_obj(o, "error"), "message", ""), AC.Acp.or(PI.Pi.err(a), "Oh My Pi's turn failed")))))

def Omp.result(+o: J.Json, +a: AC.Acp) -> AC.AcpOut:
  Omp.result.k(J.Json.get_str(o, "status", "completed"), o, a)

# Tool calls
# ----------

# a call's line, as Pi's; a Backplane tool omp reached through its xd://
# device (a write to xd://mcp__backplane__<tool>) by its own name
def Omp.label(+name: String, +args: J.Json) -> String:
  Bool.pick(String, String.starts_with(J.Json.get_str(args, "path", ""), "xd://mcp__backplane__"),
    "backplane " ++ String.drop(J.Json.get_str(args, "path", ""), 21n), PI.Pi.label(name, args))

def Omp.log(+o: J.Json) -> String:
  AC.Acp.event("item.completed", [J.KV{"item", J.Json.obj([J.KV{"type", J.Str{"command_execution"}}, J.KV{"id", J.Str{J.Json.get_str(o, "toolCallId", "")}},
    J.KV{"command", J.Str{Omp.label(J.Json.get_str(o, "toolName", "tool"), J.Json.get_obj(o, "args"))}}])}])

# Dialogs
# -------

def Omp.approval.title() -> String:
  "Allow tool: "

def Omp.first(xs: List<&2, String>) -> String:
  match xs:
    case Nil{}:
      ""
    case +x <> _:
      x

def Omp.rest(xs: List<&2, String>) -> List<&2, String>:
  match xs:
    case Nil{}:
      Nil{}
    case _ <> t:
      t

# the choice whose label holds `part` ("" when none does)
def Omp.label.of(xs: List<&2, String>, +part: String) -> String:
  match xs:
    case Nil{}:
      ""
    case +x <> t:
      Bool.pick(String, String.contains(x, part), x, Omp.label.of(t, part))

# an approval: the tool (the title's first line), what it would do (the
# rest), and the labels each answer picks
def Omp.ask(+o: J.Json, +opts: List<&2, String>) -> String:
  +lines = String.split(String.drop(J.Json.get_str(o, "title", ""), String.length(Omp.approval.title())), '\n')
  DL.Dlg.line(DL.Dlg.id(o), "approval", "pi", [J.KV{"tool", J.Str{Omp.first(lines)}}, J.KV{"detail", J.Str{String.join(Omp.rest(lines), "\n")}},
    J.KV{"once", J.Str{Omp.first(opts)}}, J.KV{"session", J.Str{Omp.label.of(opts, "for this session")}}, J.KV{"deny", J.Str{Omp.label.of(opts, "Deny")}}])

# asked in a mode that asks nothing (omp should not): allowed once
def Omp.opts(+o: J.Json) -> List<&2, String>:
  PI.Pi.strs(J.Json.elems(J.Json.get_obj(o, "options")))

def Omp.approval(ask: Bool, +a: AC.Acp, +o: J.Json) -> AC.AcpOut:
  match ask:
    case True{}:
      AC.AcpOut{a, Nil{}, Nil{}, [Omp.ask(o, Omp.opts(o))]}
    case False{}:
      AC.AcpOut{a, [DL.Dlg.reply(Omp.ask(o, Omp.opts(o)), "accept")], Nil{}, Nil{}}

def Omp.ui.pos(+m: String, +title: String) -> Nat:
  Bool.pick(Nat, Bool.and(String.eq(m, "select"), String.starts_with(title, Omp.approval.title())), 0n,
    Bool.pick(Nat, String.eq(m, "notify"), 2n,
      Bool.pick(Nat, Bool.or(Bool.or(String.eq(m, "select"), String.eq(m, "confirm")), Bool.or(String.eq(m, "input"), String.eq(m, "editor"))), 1n, 3n)))

def Omp.ui.k(i: Nat, ask: Bool, +a: AC.Acp, +o: J.Json) -> AC.AcpOut:
  match i:
    case 0n:
      Omp.approval(ask, a, o)
    case 1n:
      AC.AcpOut{a, Nil{}, Nil{}, [DL.Dlg.line(DL.Dlg.id(o), "question", "pi", [J.KV{"tool", J.Str{"Oh My Pi"}}, J.KV{"detail", J.Str{PI.Pi.question.text(o)}},
        J.KV{"options", J.Str{PI.Pi.question.opts(o)}}, J.KV{"bool", J.Flag{String.eq(J.Json.get_str(o, "method", ""), "confirm")}}])]}
    case 2n:
      AC.AcpOut{a, Nil{}, Bool.pick(List<&2, String>, String.eq(J.Json.get_str(o, "notifyType", "info"), "info"), Nil{}, [Omp.notice("Oh My Pi: " ++ J.Json.get_str(o, "message", ""))]), Nil{}}
    case _:
      PI.Pi.none(a)

# The lines
# ---------

def Omp.kind(+ty: String) -> Nat:
  AC.Acp.pos(["response", "message_update", "tool_execution_start", "tool_execution_end", "message_end", "prompt_result", "extension_ui_request",
    "auto_retry_start", "compaction_start", "extension_error"], ty, 0n)

def Omp.line.k(i: Nat, +a: AC.Acp, +t: AC.AcpTurn, +o: J.Json) -> AC.AcpOut:
  match i:
    case 0n:
      Omp.reply.k(AC.Acp.pos(["0", "1", "2", "3"], J.Json.get_str(o, "id", ""), 0n), J.Json.get_bool(o, "success", False{}), a, t, o)
    case 1n:
      PI.Pi.update.k(AC.Acp.pos(["text_start", "text_delta", "thinking_end"], J.Json.get_str(J.Json.get_obj(o, "assistantMessageEvent"), "type", ""), 0n), a, J.Json.get_obj(o, "assistantMessageEvent"))
    case 2n:
      AC.AcpOut{a, Nil{}, [Omp.log(o)], Nil{}}
    case 3n:
      AC.AcpOut{a, Nil{}, [PI.Pi.done(o)], Nil{}}
    case 4n:
      PI.Pi.none(PI.Pi.msg_end(J.Json.get_obj(o, "message"), a))
    case 5n:
      Omp.result(o, a)
    case 6n:
      Omp.ui.k(Omp.ui.pos(J.Json.get_str(o, "method", ""), J.Json.get_str(o, "title", "")), AC.Acp.ask_of(t), a, o)
    case 7n:
      AC.AcpOut{a, Nil{}, [Omp.notice("Oh My Pi is retrying (attempt " ++ J.Json.as_raw(J.Json.get(o, "attempt"), "?") ++ " of " ++ J.Json.as_raw(J.Json.get(o, "maxAttempts"), "?") ++ "): " ++ J.Json.get_str(o, "errorMessage", ""))], Nil{}}
    case 8n:
      AC.AcpOut{a, Nil{}, [Omp.notice("Oh My Pi is compacting the conversation")], Nil{}}
    case 9n:
      AC.AcpOut{a, Nil{}, [Omp.notice("An Oh My Pi extension failed (" ++ J.Json.get_str(o, "event", "") ++ "): " ++ J.Json.get_str(o, "error", ""))], Nil{}}
    case _:
      PI.Pi.none(a)

def Omp.line.parsed(m: Maybe<&2, J.Json>, +a: AC.Acp, +t: AC.AcpTurn) -> AC.AcpOut:
  match m:
    case Some{+o}:
      Omp.line.k(Omp.kind(J.Json.get_str(o, "type", "")), a, t, o)
    case None{}:
      PI.Pi.none(a)

# lines this turn reads nothing from, told by their start (omp writes
# "type" first): the whole conversation, the system prompt and the
# command list come in some of them, so they are not parsed. get_state's
# reply (it holds the system prompt) is read: its id is first.
def Omp.skip(+line: String) -> Bool:
  Bool.or(Bool.or(String.starts_with(line, "{\"type\":\"message_start\""), String.starts_with(line, "{\"type\":\"agent_end\"")),
    Bool.or(Bool.or(String.starts_with(line, "{\"type\":\"turn_end\""), String.starts_with(line, "{\"type\":\"available_commands_update\"")),
      Bool.or(String.starts_with(line, "{\"type\":\"message_end\",\"message\":{\"role\":\"user\""), String.starts_with(line, "{\"type\":\"message_end\",\"message\":{\"role\":\"toolResult\""))))

def Omp.line.s(skip: Bool, +a: AC.Acp, +t: OmpTurn, line: String) -> AC.AcpOut:
  match skip:
    case True{}:
      PI.Pi.none(a)
    case False{}:
      Omp.line.parsed(J.Json.parse(line), a, Omp.acp(t))

# one line from omp
def Omp.line(+a: AC.Acp, +t: OmpTurn, +line: String) -> AC.AcpOut:
  Omp.line.s(Omp.skip(line), a, t, line)
