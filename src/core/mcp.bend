import Base
import ./mailrun.bend as RUN
import ./charter.bend as CH
import ./locks.bend as LK
import ./json.bend as J
import ./model.bend as M
import ./review.bend as RVW
import ./codec.bend as C
import ./hub.bend as B
import ./deploy.bend as DP
import ./ship.bend as SH
import ./acp.bend as AC
import ./msp.bend as MS
import ./handoff.bend as HD
import ./bot.bend as BT
import ./emb.bend as EM
import ./utf8.bend as U8
import ./wake.bend as WK
import ./when.bend as WH
import ./sched.bend as SC
import ./tz.bend as TZ
import ./mail.bend as ML
import ./uber.bend as UBM

# The Backplane MCP server, as a decider. Each agent session gets its own
# endpoint (/mcp/<thread>), so every call knows which thread is asking. A
# call answers a JSON-RPC reply now, plus the changes and effects of a hub
# step. Tools follow T3Code's orchestrator toolkit: delegate work to another
# model, watch it, and read or message other threads, within the laws of
# model.bend (bounded depth, forward-only task states, one delivery).

type McpOut is Data:
  McpOut{reply: String, step: B.Step, park: String}

def McpOut.reply(o: McpOut) -> String:
  match o:
    case McpOut{r, _, _}:
      r

def Mcp.result(+id: J.Json, r: J.Json) -> String:
  J.Json.show(J.Json.obj([J.KV{"jsonrpc", J.Str{"2.0"}}, J.KV{"id", id}, J.KV{"result", r}]))

def Mcp.error(+id: J.Json, +code: String, msg: String) -> String:
  J.Json.show(J.Json.obj([J.KV{"jsonrpc", J.Str{"2.0"}}, J.KV{"id", id},
    J.KV{"error", J.Json.obj([J.KV{"code", J.Num{code}}, J.KV{"message", J.Str{msg}}])}]))

# a tool's answer: text content (JSON text), isError when it failed
def Mcp.content(+ok: Bool, body: J.Json) -> J.Json:
  J.Json.obj([J.KV{"content", J.Json.arr([J.Json.obj([J.KV{"type", J.Str{"text"}}, J.KV{"text", J.Str{J.Json.show(body)}}])])},
    J.KV{"isError", J.Flag{Bool.not(ok)}}])

def Mcp.fail(msg: String) -> J.Json:
  J.Json.obj([J.KV{"error", J.Str{msg}}])

# Tool definitions
# ----------------

def Tool.prop(+name: String, +ty: String, desc: String) -> J.KV:
  J.KV{name, J.Json.obj([J.KV{"type", J.Str{ty}}, J.KV{"description", J.Str{desc}}])}

def Tool.strs(xs: List<&2, String>) -> List<&2, J.Json>:
  match xs:
    case Nil{}:
      Nil{}
    case x <> t:
      J.Str{x} <> Tool.strs(t)

def Tool.def(name: String, desc: String, props: List<&2, J.KV>, req: List<&2, String>) -> J.Json:
  J.Json.obj([J.KV{"name", J.Str{name}}, J.KV{"description", J.Str{desc}},
    J.KV{"inputSchema", J.Json.obj([J.KV{"type", J.Str{"object"}}, J.KV{"properties", J.Json.obj(props)}, J.KV{"required", J.Json.arr(Tool.strs(req))}])}])

def Tool.target() -> List<&2, J.KV>:
  [Tool.prop("ref", "string", "A ref from preview_snapshot's outline, e.g. e5 (preferred)."),
   Tool.prop("locator", "string", "A Playwright selector, role or text based, e.g. role=button[name='Send'] or text=Continue."),
   Tool.prop("selector", "string", "A CSS selector."),
   Tool.prop("timeoutMs", "number", "Timeout in ms (default 15000, at most 60000).")]

# The browser Backplane hosts (a headless Chromium the user watches in the
# app's Browser tab); the server forwards these to it
def Mcp.preview_tools() -> List<&2, J.Json>:
  [Tool.def("preview_open", "Open the Backplane browser (the user sees it live in the app's Browser tab), optionally at a URL. Use it to check web UIs, datasheets, distributor pages and docs.",
     [Tool.prop("url", "string", "URL to load; a bare host gets https://, localhost gets http://."), Tool.prop("width", "number", "Viewport width (default 1280)."), Tool.prop("height", "number", "Viewport height (default 800).")], Nil{}),
   Tool.def("preview_navigate", "Go to a URL in the Backplane browser.", [Tool.prop("url", "string", "The URL."), Tool.prop("readiness", "string", "load (default), domContentLoaded or none.")], ["url"]),
   Tool.def("preview_snapshot", "Read the page: URL, title, visible text, interactive elements and an outline whose [ref=eN] marks are targets for click and type. Snapshot before acting.",
     [Tool.prop("screenshot", "boolean", "Also return a PNG screenshot.")], Nil{}),
   Tool.def("preview_click", "Click an element (ref, locator or selector) or a point (x, y in viewport pixels).",
     List.append(&2, J.KV, Tool.target(), [Tool.prop("x", "number", "X in viewport pixels."), Tool.prop("y", "number", "Y in viewport pixels.")]), Nil{}),
   Tool.def("preview_type", "Type text into an element (or the focused one).",
     List.append(&2, J.KV, Tool.target(), [Tool.prop("text", "string", "The text, inserted literally."), Tool.prop("clear", "boolean", "Clear the field first.")]), ["text"]),
   Tool.def("preview_press", "Press a key (Enter, Escape, Tab, ArrowDown, Backspace, a...).", [Tool.prop("key", "string", "Playwright key name.")], ["key"]),
   Tool.def("preview_scroll", "Scroll the page (dx, dy in pixels; positive is right/down), or an element with a target.",
     List.append(&2, J.KV, Tool.target(), [Tool.prop("dx", "number", "Horizontal pixels."), Tool.prop("dy", "number", "Vertical pixels.")]), Nil{}),
   Tool.def("preview_evaluate", "Evaluate a JavaScript expression in the page and return its JSON value.", [Tool.prop("expression", "string", "The expression.")], ["expression"]),
   Tool.def("preview_wait_for", "Wait until an element is visible, some text appears, or the URL contains a string.",
     List.append(&2, J.KV, Tool.target(), [Tool.prop("text", "string", "Text to wait for."), Tool.prop("url", "string", "URL substring to wait for.")]), Nil{}),
   Tool.def("preview_resize", "Resize the browser viewport.", [Tool.prop("width", "number", "Width."), Tool.prop("height", "number", "Height.")], ["width", "height"]),
   Tool.def("preview_status", "The browser's state: open, URL, title, viewport.", Nil{}, Nil{})]

# KiCad's IPC API through the headless KiCad Backplane runs for the project
# (the server starts on the first call); src/server/server.bend's KIpc.*
def Mcp.kicad_tools() -> List<&2, J.Json>:
  [Tool.def("kicad_ipc", "Send one command to the project's headless KiCad over its IPC API (Backplane starts one KiCad per project). path is the project's .kicad_pro, .kicad_pcb or .kicad_sch (absolute); type is the fully-qualified protobuf command, e.g. kiapi.board.commands.GetItems, kiapi.common.commands.BeginCommit, kiapi.backplane.SketchRoute; message is the command's fields as JSON. The answer is the response as JSON. Headless KiCad has no selection or view state: always pass explicit item IDs (the KIIDs GetItems returns), wrap edits in BeginCommit/EndCommit, then SaveDocument (the viewers follow the saved files). Unsaved work is undone with EndCommit CMA_DROP. Backplane KiCad bp.4 supports partial footprint updates and CreateItems from a symbol library id, with symbol libraries loaded on demand. On runtimes without footprint-partial-update or symbol-from-libid, the hub refuses unsafe requests and explains the workaround: send the full FootprintInstance from GetItems, or LoadAllLibraries, wait for GetLibraryStatuses, then PlaceSymbolFromLibrary. Selection, view and RunAction commands are refused at once. When IPC is unavailable the error says why and names the fallback (edit the file directly or use kicad-cli, then re-read the file).",
     [Tool.prop("path", "string", "Absolute path of the project's .kicad_pro, .kicad_pcb or .kicad_sch."), Tool.prop("type", "string", "The protobuf command, fully qualified (kiapi.board.commands.GetItems)."),
      J.KV{"message", J.Json.obj([J.KV{"type", J.Str{"object"}}, J.KV{"description", J.Str{"The command's fields as JSON (default {})."}}])}], ["path", "type"]),
   Tool.def("kicad_ipc_status", "The project's headless KiCad: its server's state (starting, ready, failed or stopped), its version, its start command line (argv), and the capabilities and commands it has (kiapi.backplane.GetCapabilities). Starts the server if it is not running. Headless KiCad has no selection or view state: always pass explicit item IDs (KIIDs from GetItems) in kicad_ipc, wrap edits in BeginCommit/EndCommit, then SaveDocument.",
     [Tool.prop("path", "string", "Absolute path of the project's .kicad_pro, .kicad_pcb or .kicad_sch.")], ["path"])]

def Mcp.thread_json(t: M.Thread) -> J.Json:
  match t:
    case M.Thread{id, project, title, _, act, turn, _, _, _, _, _, _, _, _, _, provider, model, _, _, _, _}:
      J.Json.obj([J.KV{"id", J.Str{id}}, J.KV{"project", J.Str{project}}, J.KV{"title", J.Str{title}}, J.KV{"provider", J.Str{provider}},
        J.KV{"model", J.Str{model}}, J.KV{"turn", J.Str{C.Turn.name(turn)}}, J.KV{"activity", C.Codec.nat(act)}])

# Handlers
# --------

def Mcp.caps(+s: M.State, +thread: String, +models: J.Json) -> J.Json:
  J.Json.obj([J.KV{"providers", models}, J.KV{"maxDepth", C.Codec.nat(M.Task.max_depth())},
    J.KV{"thread", Mcp.thread_json(B.Hub.thread(s, thread))}, J.KV{"depth", C.Codec.nat(B.Task.depth(s, thread))}])

def Mcp.task_json(t: M.Task) -> J.Json:
  match t:
    case M.Task{id, parent, child, depth, st, summary, _, provider, model}:
      J.Json.obj([J.KV{"taskId", J.Str{id}}, J.KV{"parentThreadId", J.Str{parent}}, J.KV{"childThreadId", J.Str{child}},
        J.KV{"depth", C.Codec.nat(depth)}, J.KV{"state", J.Str{C.TaskSt.name(st)}}, J.KV{"provider", J.Str{provider}},
        J.KV{"model", J.Str{model}}, J.KV{"summary", J.Str{summary}}])

# a tool call's outcome: success flag, JSON body, and the hub step to run
type Res is Data:
  Res{ok: Bool, body: J.Json, step: B.Step}

def Res.fail(msg: String) -> Res:
  Res{False{}, Mcp.fail(msg), B.Step.none()}

def Res.ok(body: J.Json) -> Res:
  Res{True{}, body, B.Step.none()}

def Mcp.task.hit(ts: List<&2, M.Task>, +id: String, +thread: String) -> Bool:
  match ts:
    case M.Task{i, p, _, _, _, _, _, _, _} <> _:
      Bool.and(String.eq(i, id), String.eq(p, thread))
    case Nil{}:
      False{}

def Mcp.task.go(ts: List<&2, M.Task>, hit: Bool, +id: String, +thread: String) -> Maybe<&2, M.Task>:
  match ts hit:
    case t <> _ True{}:
      Some{t}
    case _ <> +rest False{}:
      Mcp.task.go(rest, Mcp.task.hit(rest, id, thread), id, thread)
    case Nil{} _:
      None{}

def Mcp.task.k(+ts: List<&2, M.Task>, +id: String, +thread: String) -> Maybe<&2, M.Task>:
  Mcp.task.go(ts, Mcp.task.hit(ts, id, thread), id, thread)

# a task this thread delegated (other threads' tasks are not its business)
def Mcp.task(s: M.State, +id: String, +thread: String) -> Maybe<&2, M.Task>:
  match s:
    case M.State{_, _, _, _, tk, _, _, _, _, _, _}:
      Mcp.task.k(Map.values(&2, M.Task, tk), id, thread)

def Mcp.status(m: Maybe<&2, M.Task>) -> Res:
  match m:
    case Some{t}:
      Res.ok(Mcp.task_json(t))
    case None{}:
      Res.fail("no such task of this thread")

def Mcp.cancel(m: Maybe<&2, M.Task>, +now: Nat) -> Res:
  match m:
    case Some{M.Task{+id, p, +child, d, st, su, dl, pr, mo}}:
      Res{True{}, Mcp.task_json(M.Task{id, p, child, d, M.TaskState.next(st, M.TaskCancelled{}), su, dl, pr, mo}),
        B.Step{[M.TaskStateSet{id, M.TaskCancelled{}, "cancelled by the parent", now}, M.TurnChanged{child, M.Interrupted{}, now}], [B.AgentStop{child}]}}
    case None{}:
      Res.fail("no such task of this thread")

# Delegation
# ----------

def Mcp.or(+v: String, +d: String) -> String:
  Bool.pick(String, String.is_empty(v), d, v)

def Mcp.project_of(t: M.Thread) -> String:
  match t:
    case M.Thread{_, p, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _}:
      p

# Threads
# -------

# whether the thread is a bot's (bot.bend): a bot works for the person
# across all their projects, where any other thread stays in its own
def Mcp.bot(+s: M.State, +thread: String) -> Bool:
  BT.World.is_bot_thread(M.State.bots(s), thread)

def Mcp.same_project(+s: M.State, +a: String, +b: String) -> Bool:
  Bool.or(Mcp.bot(s, a), String.eq(Mcp.project_of(B.Hub.thread(s, a)), Mcp.project_of(B.Hub.thread(s, b))))

def Mcp.threads.k(keep: Bool, t: M.Thread, rest: List<&2, J.Json>) -> List<&2, J.Json>:
  match keep:
    case True{}:
      Mcp.thread_json(t) <> rest
    case False{}:
      rest

# a thread listed for a caller in `project` ("*": a bot, which sees the
# threads of every project, not other bots' own)
def Mcp.visible(t: M.Thread, +project: String) -> Bool:
  match t:
    case M.Thread{_, +p, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, de}:
      Bool.and(Bool.or(String.eq(p, project), Bool.and(String.eq(project, "*"), Bool.not(String.is_empty(p)))), Bool.not(de))

def Mcp.threads(ts: List<&2, M.Thread>, +project: String) -> List<&2, J.Json>:
  match ts:
    case Nil{}:
      Nil{}
    case +t <> rest:
      Mcp.threads.k(Mcp.visible(t, project), t, Mcp.threads(rest, project))

def Mcp.scope(+s: M.State, +thread: String) -> String:
  Bool.pick(String, Mcp.bot(s, thread), "*", Mcp.project_of(B.Hub.thread(s, thread)))

def Mcp.mail.bot(b: Maybe<&2, BT.Bot>, +thread: String) -> String:
  match b:
    case Some{+bot}: BT.Bot.id(bot)
    case None{}: "thread:" ++ thread

def Mcp.mail.scope(+s: M.State, +thread: String) -> String:
  Mcp.mail.bot(RUN.Run.bot(s, thread), thread)

def Mcp.run.visible(r: Maybe<&2, ML.Run>, +s: M.State, +caller: String) -> Bool:
  match r:
    case None{}: True{}
    case Some{+r}: Bool.and(RUN.Run.is(s, caller), Bool.and(ML.Run.live(r), String.eq(ML.Run.bot(r), Mcp.mail.scope(s, caller))))

def Mcp.run.keep(ok: Bool, +t: M.Thread, +rest: List<&2, M.Thread>) -> List<&2, M.Thread>:
  match ok:
    case True{}: t <> rest
    case False{}: rest

def Mcp.run.threads(+ts: List<&2, M.Thread>, +s: M.State, +caller: String) -> List<&2, M.Thread>:
  match ts:
    case Nil{}: Nil{}
    case +t <> +rest: Mcp.run.keep(Mcp.run.visible(RUN.Run.of(s, M.Thread.id(t)), s, caller), t, Mcp.run.threads(rest, s, caller))

def Mcp.list(+s: M.State, +thread: String) -> Res:
  match s:
    case M.State{_, ts, _, _, _, _, _, _, _, _, _}:
      Res.ok(J.Json.obj([J.KV{"threads", J.Json.arr(Mcp.threads(Mcp.run.threads(Map.values(&2, M.Thread, ts), s, thread), Mcp.scope(s, thread)))}]))

def Mcp.entry(e: M.Entry) -> J.Json:
  match e:
    case M.Msg{_, _, role, text, at}:
      J.Json.obj([J.KV{"kind", J.Str{"message"}}, J.KV{"role", J.Str{C.Role.name(role)}}, J.KV{"text", J.Str{text}}, J.KV{"at", C.Codec.nat(at)}])
    case M.Act{_, _, tone, kind, summary, at}:
      J.Json.obj([J.KV{"kind", J.Str{"activity"}}, J.KV{"tone", J.Str{tone}}, J.KV{"tool", J.Str{kind}}, J.KV{"summary", J.Str{summary}}, J.KV{"at", C.Codec.nat(at)}])

def Mcp.mine.k(same: Bool, e: M.Entry, rest: List<&2, J.Json>) -> List<&2, J.Json>:
  match same:
    case True{}:
      Mcp.entry(e) <> rest
    case False{}:
      rest

def Mcp.of(e: M.Entry) -> String:
  match e:
    case M.Msg{_, t, _, _, _}:
      t
    case M.Act{_, t, _, _, _, _}:
      t

# a thread's entries, oldest first
def Mcp.mine(es: List<&2, M.Entry>, +th: String, acc: List<&2, J.Json>) -> List<&2, J.Json>:
  match es:
    case Nil{}:
      acc
    case +e <> t:
      Mcp.mine(t, th, Mcp.mine.k(String.eq(Mcp.of(e), th), e, acc))

def Mcp.read.last(+xs: List<&2, J.Json>, +limit: Nat, +target: String) -> Res:
  Res.ok(J.Json.obj([J.KV{"threadId", J.Str{target}},
    J.KV{"entries", J.Json.arr(List.drop(&2, J.Json, xs, Nat.sub(List.length(&2, J.Json, xs), limit)))}]))

def Mcp.read.k(ok: Bool, +s: M.State, +target: String, +limit: Nat) -> Res:
  match ok s:
    case True{} M.State{_, _, es, _, _, _, _, _, _, _, _}:
      Mcp.read.last(Mcp.mine(es, target, Nil{}), limit, target)
    case False{} _:
      Res.fail("that thread is not in this project")

# the file goes to the viewer of the thread that asked
def Mcp.view(+thread: String, +path: String, +kind: String) -> Res:
  Res{True{}, J.Json.obj([J.KV{"shown", J.Str{path}}]),
    B.Step{Nil{}, [B.Broadcast{J.Json.show(J.Json.obj([J.KV{"t", J.Str{"view"}}, J.KV{"thread", J.Str{thread}}, J.KV{"path", J.Str{path}}, J.KV{"kind", J.Str{kind}}]))}]}}

# Terminals in the chat (core/emb.bend); terminal_read is the server's
# (it asks the terminal actor, off the hub)
# --------------------------------------------------------------------

def Mcp.term.title(+title: String, +cmd: String) -> String:
  Bool.pick(String, String.is_empty(String.trim(title)), String.take(String.trim(cmd), 60n), String.trim(title))

def Mcp.term.go(+id: String, +s: M.State, +now: Nat, +thread: String, +cmd: String, +title: String, +cols: U32, +rows: U32) -> Res:
  Res{True{}, J.Json.obj([J.KV{"id", J.Str{id}}, J.KV{"cols", J.Num{U32.show(cols)}}, J.KV{"rows", J.Num{U32.show(rows)}}, J.KV{"state", J.Str{"running"}}]),
    B.Step{[M.ActivityLogged{thread, id, EM.Emb.tone(), EM.Emb.kind(), EM.Emb.summary(title, cols, rows, cmd), now}],
      [B.TermRun{EM.Emb.key(thread, id), title, B.Hub.cwd(s, B.Hub.thread(s, thread)), cmd, cols, rows, False{}}]}}

def Mcp.term.open(+s: M.State, +now: Nat, +thread: String, +cmd: String, +title: String, +cols: U32, +rows: U32) -> Res:
  Bool.pick(Res, String.is_empty(String.trim(cmd)), Res.fail("command is empty"),
    Mcp.term.go(B.Hub.id("x", now, B.Hub.seq(s)), s, now, thread, cmd, Mcp.term.title(title, cmd), cols, rows))

# keys typed in (send), or the program hung up
def Mcp.term.op.k(send: Bool, +key: String, +text: String) -> Res:
  match send:
    case True{}:
      Res{True{}, J.Json.obj([J.KV{"sent", J.Num{U32.show(U32.from_nat(String.length(text)))}}]), B.Step.outs([B.TermOp{"input", key, "", U8.Utf8.encode(text), 0, 0}])}
    case False{}:
      Res{True{}, J.Json.obj([J.KV{"state", J.Str{"stopped"}}]), B.Step.outs([B.TermKill{key}])}

def Mcp.term.op(missing: Bool, send: Bool, +thread: String, +id: String, +text: String) -> Res:
  match missing:
    case True{}:
      Res.fail("no terminal " ++ id ++ " in this thread")
    case False{}:
      Mcp.term.op.k(send, EM.Emb.key(thread, id), text)

# More thread tools (V2's orchestrator toolkit)
# ---------------------------------------------

def Mcp.proj(+s: M.State, +thread: String) -> String:
  Mcp.project_of(B.Hub.thread(s, thread))

# a bot's first message names it (the link visuals read the sender)
def Mcp.from.id(+from: String, +msg: String) -> String:
  Bool.pick(String, String.is_empty(from), msg, M.Link.id(from, msg))

def Mcp.delegate.go(+s: M.State, +now: Nat, +parent: String, +project: String, env: M.EnvMode, +branch: String, +wt: String, +provider: String, +model: String,
    +rm: M.RuntimeMode, im: M.InteractionMode, +prompt: String, +title: String) -> Res:
  +seq = {B.Hub.seq(s) : Nat}
  +task = {B.Hub.id("k", now, seq) : String}
  +child = {B.Hub.id("t", now, 1n+seq) : String}
  +depth = {1n+B.Task.depth(s, parent) : Nat}
  +name = {Mcp.or(title, B.Hub.title.of(prompt)) : String}
  +cwd = {Bool.pick(String, String.is_empty(wt), B.Hub.project.root(B.Hub.project(s, project)), wt) : String}
  Res{True{}, J.Json.obj([J.KV{"taskId", J.Str{task}}, J.KV{"childThreadId", J.Str{child}}, J.KV{"state", J.Str{"running"}},
      J.KV{"provider", J.Str{provider}}, J.KV{"model", J.Str{model}}, J.KV{"depth", C.Codec.nat(depth)},
      J.KV{"note", J.Str{"The result will arrive in this thread as a message when the child finishes. End your turn instead of polling."}}]),
    B.Step{[M.ThreadCreated{child, project, name, env, branch, wt, provider, model, now},
        M.ModesSet{child, rm, im, model, now},
        M.TaskDelegated{task, parent, child, depth, provider, model, now},
        M.TaskStateSet{task, M.TaskRunning{}, "", now},
        M.MessagePosted{child, Mcp.from.id(Bool.pick(String, Mcp.bot(s, parent), parent, ""), B.Hub.id("m", now, 2n+seq)), M.User{}, prompt, now},
        M.TurnChanged{child, M.Running{}, now}],
      B.Hub.run(M.Thread{child, project, name, now, now, M.Running{}, 0n, False{}, M.NoOverride{}, 0n, 0n, False{}, M.Local{}, branch, wt, provider, model, rm, M.DefaultMode{}, "", False{}}, cwd, prompt, B.Hub.uuid(now, child ++ "/d"))}}

def Mcp.delegate.k(ok: Bool, t: M.Thread, +s: M.State, +now: Nat, +thread: String, +bot: Bool, +proj: String, +p: J.Json) -> Res:
  match ok t:
    case False{} _:
      Res.fail("delegation depth limit reached: finish this task yourself")
    case True{} M.Thread{_, +project, _, _, _, _, _, _, _, _, _, _, +env, +branch, +wt, +provider, +model, rm, im, _, _}:
      Mcp.delegate.go(s, now, thread, Bool.pick(String, bot, proj, project), Bool.pick(M.EnvMode, bot, M.Local{}, env), Bool.pick(String, bot, "", branch), Bool.pick(String, bot, "", wt),
        Mcp.or(J.Json.get_str(p, "provider", ""), provider),
        Mcp.or(J.Json.get_str(p, "model", ""), Bool.pick(String, String.eq(J.Json.get_str(p, "provider", provider), provider), model, "")),
        rm, im, J.Json.get_str(p, "prompt", ""), J.Json.get_str(p, "title", ""))

# a bot's delegated work runs in the project it names, where the person
# sees it as a thread of its own; a bot that names none is refused (law
# bot_delegate_needs_project)
def Mcp.delegate.b(bot: Bool, none: Bool, +s: M.State, +now: Nat, +thread: String, +proj: String, +p: J.Json) -> Res:
  match bot none:
    case True{} True{}:
      Res.fail("name a project (project_list shows them): a bot's delegated work runs as a thread there that the person can follow")
    case _ _:
      Mcp.delegate.k(M.Task.may_delegate(B.Task.depth(s, thread)), B.Hub.thread(s, thread), s, now, thread, bot, proj, p)

# a new top-level thread with its first message (it waits for its
# worktree like any new thread)
# (the launcher is kept with the thread, so its turn ends and asks are
# reported back: hub.bend's Launch.tell)
def Mcp.launch.cs(+c: M.Change, +th: String, +by: String, +report: Bool, +now: Nat) -> List<&2, M.Change>:
  Bool.pick(List<&2, M.Change>, String.is_empty(by), [c], [c, M.ThreadLaunched{th, by, report, now}])

def Mcp.launch.note(+by: String, +report: Bool) -> String:
  Bool.pick(String, Bool.and(report, Bool.not(String.is_empty(by))), "Its reports will come to you as messages when its turn ends or it asks the person something. End your turn instead of polling.", "")

def Mcp.launch.go(+s: M.State, +now: Nat, +th: String, +from: String, +by: String, +report: Bool, +project: String, +env: String, +provider: String, +model: String, +title: String, +prompt: String) -> Res:
  +c = {M.ThreadCreated{th, project, title, B.Hub.new.env(M.Scratch.is(project), env), "", B.Hub.new.wt(M.Scratch.is(project), B.Hub.project.root(B.Hub.project(s, project)), th), provider, model, now} : M.Change}
  +cs = {Mcp.launch.cs(c, th, by, report, now) : List<&2, M.Change>}
  +s1 = {M.State.replay(cs, s) : M.State}
  Res{True{}, J.Json.obj([J.KV{"threadId", J.Str{th}}, J.KV{"title", J.Str{title}}, J.KV{"provider", J.Str{provider}}, J.KV{"state", J.Str{"started"}}, J.KV{"note", J.Str{Mcp.launch.note(by, report)}}]),
    B.Step.before(cs, B.Hub.new.outs(M.Scratch.is(project), th, B.Hub.project.root(B.Hub.project(s, project)), env),
      B.Hub.post(s1, now, th, Mcp.from.id(from, B.Hub.id("m", now, B.Hub.seq(s1))), M.User{}, prompt))}

# a project by id or name (any case)
def Mcp.project.k(p: M.Project, +name: String) -> Bool:
  match p:
    case M.Project{i, t, _, _, rm}:
      Bool.and(Bool.not(rm), Bool.or(String.eq(i, name), String.eq(String.to_lower(t), String.to_lower(name))))

def Mcp.project.id(p: M.Project) -> String:
  match p:
    case M.Project{i, _, _, _, _}:
      i

def Mcp.project.go(ps: List<&2, M.Project>, +name: String) -> String:
  match ps:
    case Nil{}:
      ""
    case +p <> t:
      Bool.pick(String, Mcp.project.k(p, name), Mcp.project.id(p), Mcp.project.go(t, name))

def Mcp.project.find(+s: M.State, +name: String) -> String:
  match s:
    case M.State{ps, _, _, _, _, _, _, _, _, _, _}:
      Mcp.project.go(Map.values(&2, M.Project, ps), String.trim(name))

def Mcp.delegate(+s: M.State, +now: Nat, +thread: String, +p: J.Json) -> Res:
  Mcp.delegate.b(Mcp.bot(s, thread), String.is_empty(Mcp.project.find(s, J.Json.get_str(p, "project", ""))), s, now, thread,
    Mcp.project.find(s, J.Json.get_str(p, "project", "")), p)

# where a new thread goes: the caller's project, or for a bot the one it
# names (it has none of its own)
def Mcp.launch.proj(+s: M.State, +thread: String, +name: String) -> String:
  Bool.pick(String, Bool.or(Mcp.bot(s, thread), Bool.not(String.is_empty(name))), Mcp.project.find(s, name), Mcp.proj(s, thread))

def Mcp.launch.k(empty: Bool, +s: M.State, +now: Nat, +thread: String, +by: String, +p: J.Json) -> Res:
  match empty:
    case True{}:
      Res.fail("a prompt is needed")
    case False{}:
      Mcp.launch.go(s, now, B.Hub.id("t", now, B.Hub.seq(s)), Bool.pick(String, Mcp.bot(s, thread), thread, ""), by, J.Json.get_bool(p, "report", True{}), Mcp.launch.proj(s, thread, J.Json.get_str(p, "project", "")), M.Setting.get(s, "thread.env", "auto"),
        Mcp.or(J.Json.get_str(p, "provider", ""), M.Setting.get(s, "agent.provider", "claude")),
        Mcp.or(J.Json.get_str(p, "model", ""), Bool.pick(String, String.is_empty(J.Json.get_str(p, "provider", "")), M.Setting.get(s, "agent.model", ""), "")),
        Mcp.or(J.Json.get_str(p, "title", ""), B.Hub.title.of(J.Json.get_str(p, "prompt", ""))), J.Json.get_str(p, "prompt", ""))

def Mcp.launch.b(bad: Bool, +s: M.State, +now: Nat, +thread: String, +by: String, +p: J.Json) -> Res:
  match bad:
    case True{}:
      Res.fail("name an existing project (project_list shows them)")
    case False{}:
      Mcp.launch.k(String.is_empty(String.trim(J.Json.get_str(p, "prompt", ""))), s, now, thread, by, p)

# by: who hears its reports (the caller's thread; a caller on another
# machine as <link>~<thread>, fmcp.bend)
def Mcp.launch.by(+s: M.State, +now: Nat, +thread: String, +by: String, +p: J.Json) -> Res:
  Mcp.launch.b(String.is_empty(Mcp.launch.proj(s, thread, J.Json.get_str(p, "project", ""))), s, now, thread, by, p)

def Mcp.launch(+s: M.State, +now: Nat, +thread: String, +p: J.Json) -> Res:
  Mcp.launch.by(s, now, thread, thread, p)

def Mcp.first.k(e: M.Entry, +th: String, rest: String) -> String:
  match e:
    case M.Msg{_, +t, M.User{}, +text, _}:
      Bool.pick(String, String.eq(t, th), text, rest)
    case _:
      rest

# a thread's first user message (entries newest first: the last one seen)
def Mcp.first(es: List<&2, M.Entry>, +th: String, +acc: String) -> String:
  match es:
    case Nil{}:
      acc
    case e <> t:
      Mcp.first(t, th, Mcp.first.k(e, th, acc))

def Mcp.update.k(ok: Bool, regen: Bool, +s: M.State, +now: Nat, +target: String, +title: String) -> Res:
  match ok regen:
    case False{} _:
      Res.fail("that thread is not in this project")
    case True{} False{}:
      Res{True{}, J.Json.obj([J.KV{"threadId", J.Str{target}}, J.KV{"title", J.Str{title}}]), B.Step{[M.ThreadRenamed{target, title, now}], Nil{}}}
    case True{} True{}:
      Res{True{}, J.Json.obj([J.KV{"threadId", J.Str{target}}, J.KV{"title", J.Str{"being written"}}]),
        B.Step{Nil{}, [B.Git{0, "", "title", target, "", HD.Hand.title(B.Hub.thread(s, target)), String.take(Mcp.first(B.Hub.entries(s), target, ""), 4000n)}]}}

def Mcp.update(+s: M.State, +now: Nat, +thread: String, +target: String, +title: String) -> Res:
  Mcp.update.k(Mcp.same_project(s, thread, target), String.is_empty(String.trim(title)), s, now, target, String.trim(title))

# 0: not given, 1: true, 2: false
def Mcp.flag(m: Maybe<&2, J.Json>) -> Nat:
  match m:
    case Some{J.Flag{True{}}}:
      1n
    case Some{J.Flag{False{}}}:
      2n
    case Some{J.Str{+v}}:
      Bool.pick(Nat, String.eq(v, "true"), 1n, Bool.pick(Nat, String.eq(v, "false"), 2n, 0n))
    case _:
      0n

# a launched thread's reports turned off or on (its launcher stays)
def Mcp.report.k(ok: Bool, +s: M.State, +now: Nat, +target: String, +on: Bool) -> Res:
  match ok:
    case True{}:
      Res{True{}, J.Json.obj([J.KV{"threadId", J.Str{target}}, J.KV{"report", J.Flag{on}}]), B.Step{[M.ThreadLaunched{target, M.Launch.by(s, target), on, now}], Nil{}}}
    case False{}:
      Res.fail("nobody launched that thread, so it has no reports to change")

def Mcp.update.r(f: Nat, +s: M.State, +now: Nat, +thread: String, +target: String, +title: String) -> Res:
  match f:
    case 0n:
      Mcp.update(s, now, thread, target, title)
    case _:
      Bool.pick(Res, String.is_empty(String.trim(title)), Mcp.report.k(Bool.not(String.is_empty(M.Launch.by(s, target))), s, now, target, Nat.is_eq(f, 1n)),
        Mcp.update(s, now, thread, target, title))

def Mcp.pick(hit: Bool, +text: String, +acc: String) -> String:
  match hit:
    case True{}:
      text
    case False{}:
      acc

def Mcp.last.go(es: List<&2, M.Entry>, +th: String, done: Bool, +acc: String) -> String:
  match es done:
    case _ True{}:
      acc
    case Nil{} False{}:
      acc
    case M.Msg{_, +t, M.Assistant{}, +text, _} <> r False{}:
      Mcp.last.go(r, th, String.eq(t, th), Mcp.pick(String.eq(t, th), text, acc))
    case _ <> r False{}:
      Mcp.last.go(r, th, False{}, acc)

# a thread's last assistant message (entries newest first: the first seen,
# where the walk stops). Given the thread's own entries (M.State.mine),
# which also hold what it sent to others, so the thread is checked
def Mcp.last(es: List<&2, M.Entry>, +th: String) -> String:
  Mcp.last.go(es, th, False{}, "")

# whether a thread is done: no turn running or starting
def Mcp.done(+s: M.State, +th: String) -> Bool:
  Bool.and(Bool.not(B.Hub.running(B.Hub.thread(s, th))), Bool.not(M.Queue.starting(s, th)))

# done, and nothing out of hand either (model.bend's Work.ready): a turn
# that left a subagent, background shell or watched work running is not
# done until that work is (law wait_busy_not_done)
def Mcp.done.w(+s: M.State, +th: String) -> Bool:
  Bool.and(Mcp.done(s, th), Nat.is_eq(M.Work.busy(s, th), 0n))

def Mcp.strs(xs: List<&2, String>) -> List<&2, J.Json>:
  match xs:
    case Nil{}:
      Nil{}
    case x <> t:
      J.Str{x} <> Mcp.strs(t)

# where a thread stands, answered at once (the server asks again each
# second while it waits, up to the timeout)
def Mcp.wait.k(ok: Bool, +s: M.State, +target: String) -> Res:
  match ok:
    case True{}:
      Res.ok(J.Json.obj([J.KV{"threadId", J.Str{target}}, J.KV{"turn", J.Str{C.Turn.name(M.Thread.turn(B.Hub.thread(s, target)))}},
        J.KV{"done", J.Flag{Mcp.done.w(s, target)}}, J.KV{"waiting", J.Json.nat(M.Work.busy(s, target))}, J.KV{"work", J.Json.arr(Mcp.strs(M.Work.titles(s, target)))},
        J.KV{"lastReply", J.Str{String.take(Mcp.last(M.State.mine(s, target), target), 8000n)}}]))
    case False{}:
      Res.fail("that thread is not in this project")

# (from: the caller, which is not told back that it stopped its own launch)
def Mcp.interrupt.f(ok: Bool, run: Bool, +s: M.State, +now: Nat, +from: String, +target: String) -> Res:
  match ok run:
    case False{} _:
      Res.fail("that thread is not in this project")
    case True{} False{}:
      Res.ok(J.Json.obj([J.KV{"threadId", J.Str{target}}, J.KV{"state", J.Str{"not running"}}]))
    case True{} True{}:
      Res{True{}, J.Json.obj([J.KV{"threadId", J.Str{target}}, J.KV{"state", J.Str{"interrupted"}}]),
        B.Retire.stop.f(s, now, target, from, Nil{})}

def Mcp.interrupt.k(ok: Bool, run: Bool, +s: M.State, +now: Nat, +target: String) -> Res:
  Mcp.interrupt.f(ok, run, s, now, "", target)

# a message to another thread with a follow-up mode; auto is the inbox
# (it never interrupts, law inbox_never_interrupts)
def Mcp.send.m(auto: Bool, +s: M.State, +now: Nat, +thread: String, +target: String, +text: String, +mode: String) -> B.Step:
  match auto:
    case True{}:
      B.Hub.inbox(s, now, thread, target, M.User{}, text)
    case False{}:
      B.Mail.funnel(B.Hub.post.mode(mode, s, now, target, M.Link.id(thread, B.Hub.id("m", now, B.Hub.seq(s))), M.User{}, text), s, now, ML.Origin{B.Mail.addr(s, thread), "thread", ""}, B.Mail.addr(s, target), text, M.Link.id(thread, B.Hub.id("m", now, B.Hub.seq(s))))

def Mcp.send.k(ok: Bool, +s: M.State, +now: Nat, +thread: String, +target: String, +text: String, +mode: String) -> Res:
  match ok:
    case True{}:
      Res{True{}, J.Json.obj([J.KV{"threadId", J.Str{target}}, J.KV{"state", J.Str{"sent"}}]),
        Mcp.send.m(Bool.or(String.is_empty(mode), String.eq(mode, "auto")), s, now, thread, target, text, mode)}
    case False{}:
      Res.fail("that thread is not in this project")

def Mcp.names() -> List<&2, String>:
  ["orchestrator_capabilities", "delegate_task", "task_status", "task_cancel", "thread_list", "thread_read", "thread_send", "viewer_show",
   "thread_launch", "thread_update", "thread_wait", "thread_interrupt", "thread_wait_now", "terminal_open", "terminal_send", "terminal_close", "work_run", "work_watch", "work_list", "work_cancel", "work_unwatch", "review_comments", "review_comment", "wake_on", "wake_list", "wake_cancel", "deploy_restart", "deploy_status", "deploy_cancel", "lock_acquire", "lock_release", "lock_list", "lock_cancel", "deploy_main", "mail", "mail_list", "mail_read", "mail_search", "mail_tag", "mail_snooze", "mail_done", "decide", "escalate", "charter_set"]

def Mcp.lock.acquire(ok: Bool, +s: M.State, +now: Nat, +thread: String, +a: J.Json) -> Res:
  match ok:
    case False{}:
      Res.fail("lock name must contain 1..64 letters, digits, hyphens or underscores")
    case True{}:
      +name = J.Json.get_str(a, "name", "")
      +st = B.Locks.acquire(s, now, name, LK.LockParty{thread, "thread", thread, J.Json.get_str(a, "note", ""), now, 0n}, J.Json.get_bool(a, "_lockReady", False{}))
      Res{True{}, B.Locks.answer(M.Locks.get(M.State.replay(B.Queue.changes(st), s), name), thread), st}

# Watched work (hub.bend's Watch.*): work_run, work_watch, work_list,
# work_cancel, work_unwatch; each answers once its row is stored (the
# server answers after the step's write)
def Mcp.secs(+a: J.Json) -> Nat:
  M.Switch.num(J.Json.as_raw(J.Json.get(a, "timeoutSec"), "0"), 0n)

def Mcp.work.json(+l: String) -> J.Json:
  J.Json.obj([J.KV{"id", J.Str{M.Bg.field(l, 0n)}}, J.KV{"state", J.Str{Bool.pick(String, M.Bg.ended(l), M.Bg.status(l), B.Watch.state(l))}},
    J.KV{"note", J.Str{M.Bg.title(l)}}, J.KV{"kind", J.Str{B.Watch.kind(l)}}, J.KV{"target", J.Str{B.Watch.target(l)}}])

def Mcp.work.rows(ls: List<&2, String>) -> List<&2, J.Json>:
  match ls:
    case Nil{}:
      Nil{}
    case +l <> t:
      Mcp.work.json(l) <> Mcp.work.rows(t)

def Mcp.work.start.k(locked: Bool, +s: M.State, +now: Nat, +thread: String, +a: J.Json) -> B.Step:
  match locked:
    case True{}:
      B.Locks.work.start(s, now, thread, a, J.Json.get_bool(a, "_lockReady", False{}))
    case False{}:
      B.Watch.start(s, now, thread, "run", J.Json.get_str(a, "command", ""), Mcp.or(J.Json.get_str(a, "note", ""), J.Json.get_str(a, "command", "")), Mcp.secs(a))

def Mcp.work.start(+s: M.State, +now: Nat, +thread: String, +a: J.Json) -> B.Step:
  Mcp.work.start.k(Bool.not(String.is_empty(J.Json.get_str(a, "lock", ""))), s, now, thread, a)

def Mcp.work.run(empty: Bool, +s: M.State, +now: Nat, +thread: String, +a: J.Json) -> Res:
  match empty:
    case True{}:
      Res.fail("command is required")
    case False{}:
      +st = Mcp.work.start(s, now, thread, a)
      +s1 = M.State.replay(B.Queue.changes(st), s)
      Res{True{}, J.Json.obj([J.KV{"id", J.Str{B.Watch.id(s, thread)}}, J.KV{"state", J.Str{B.Watch.state(B.Watch.find(M.Work.watch(s1, thread), B.Watch.id(s, thread)))}},
          J.KV{"next", J.Str{"End your turn: you will be woken with the outcome."}}]), st}

def Mcp.work.watch(ok: Bool, +s: M.State, +now: Nat, +thread: String, +a: J.Json) -> Res:
  match ok:
    case True{}:
      Res{True{}, J.Json.obj([J.KV{"id", J.Str{B.Watch.id(s, thread)}}, J.KV{"token", J.Str{B.Watch.id(s, thread)}},
          J.KV{"next", J.Str{Bool.pick(String, String.eq(J.Json.get_str(a, "kind", ""), "file"), "Make the work write the token into the file when it is done, then end your turn.", "End your turn: you will be woken when it ends.")}}]),
        B.Watch.start(s, now, thread, J.Json.get_str(a, "kind", ""), J.Json.get_str(a, "target", ""), Mcp.or(J.Json.get_str(a, "note", ""), J.Json.get_str(a, "target", "")), Mcp.secs(a))}
    case False{}:
      Res.fail("kind must be pid or file")

def Mcp.work.cancel(+l: String, +s: M.State, +now: Nat, +thread: String, +id: String) -> Res:
  Bool.pick(Res, Bool.or(String.is_empty(l), M.Bg.ended(l)), Res.fail("no such work running"),
    Res{True{}, J.Json.obj([J.KV{"id", J.Str{id}}, J.KV{"state", J.Str{"cancelling"}}]), B.Watch.cancel(s, now, thread, id)})

def Mcp.work.unwatch(+l: String, +s: M.State, +now: Nat, +thread: String, +id: String) -> Res:
  Bool.pick(Res, Bool.or(String.is_empty(l), B.Locks.work.has(s, id)), Res.fail("no such work, or it owns a lock; cancel locked work instead"),
    Res{True{}, J.Json.obj([J.KV{"id", J.Str{id}}, J.KV{"state", J.Str{B.Watch.unwatch.text()}}]), B.Watch.unwatch(s, now, thread, id)})

# Design review comments (core/review.bend): the agent reads them and
# writes its own, the same Acts a person's make
def Mcp.review.list(+cs: List<&2, RVW.Comment>) -> Res:
  Res.ok(J.Json.obj([J.KV{"open", J.Num{Nat.show(RVW.Review.count.open(cs))}}, J.KV{"resolved", J.Num{Nat.show(List.length(&2, RVW.Comment, RVW.Review.resolved(cs)))}},
    J.KV{"text", J.Str{Bool.pick(String, String.is_empty(RVW.Review.prompt(cs)), "No open comments.", RVW.Review.prompt(cs))}}]))

def Mcp.review.do(ok: Bool, +s: M.State, +now: Nat, +thread: String, +a: J.Json) -> Res:
  match ok:
    case True{}:
      +aid = B.Hub.id("r", now, B.Hub.seq(s))
      Res{True{}, J.Json.obj([J.KV{"id", J.Str{Bool.pick(String, String.eq(J.Json.get_str(a, "op", ""), "add"), aid, J.Json.get_str(a, "id", ""))}}]),
        B.Step{[RVW.Review.act(thread, aid, a, B.Hub.provider(B.Hub.thread(s, thread)), now)], Nil{}}}
    case False{}:
      Res.fail("review_comment needs op (add, resolve, reopen, edit, drop), text for add and edit, id for the rest")

def Mcp.wake.events(+a: J.Json, +kind: String) -> List<&2, String>:
  Bool.pick(List<&2, String>, Maybe.is_some(&2, J.Json, J.Json.get(a, "events")), WK.Wake.events(a), WK.Wake.defaults(kind))

def Mcp.wake.on.k(+error: String, +s: M.State, +now: Nat, +by: String, +a: J.Json, +events: List<&2, String>) -> Res:
  match error:
    case SNil{}:
      +id = B.Hub.id("w", now, B.Hub.seq(s))
      +kind = J.Json.get_str(a, "kind", "")
      +target = J.Json.get_str(a, "target", "")
      +once = J.Json.get_bool(a, "once", True{})
      +note = J.Json.get_str(a, "note", "")
      Res{True{}, M.Wake.record(id, by, kind, target, events, once, note, now), B.Step{List.append(&2, M.Change, [M.WakeSet{id, by, kind, target, events, once, note, now}], Bool.pick(List<&2, M.Change>, Bool.and(String.eq(kind, "file"), Bool.not(String.is_empty(J.Json.get_str(a, "_baseline", "")))), [M.SettingSet{"wake.base." ++ id, J.Json.get_str(a, "_baseline", ""), now}], Nil{})),
        Bool.pick(List<&2, B.Out>, String.eq(kind, "file"), [B.WakePoll{Nil{}, [target]}], Nil{})}}
    case SCon{_, _}:
      Res.fail(error)

def Mcp.wake.error(+s: M.State, +by: String, +kind: String, +target: String, +events: List<&2, String>) -> String:
  +error = WK.Wake.error(by, kind, target, events)
  Bool.pick(String, Bool.not(String.is_empty(error)), error,
    Bool.pick(String, Bool.and(String.eq(kind, "thread"), Bool.not(B.Hub.owns(s, target))), "no such local target thread", ""))

def Mcp.wake.on(+s: M.State, +now: Nat, +by: String, +a: J.Json) -> Res:
  +kind = J.Json.get_str(a, "kind", "")
  +target = J.Json.get_str(a, "target", "")
  +events = Mcp.wake.events(a, kind)
  Mcp.wake.on.k(Mcp.wake.error(s, by, kind, target, events), s, now, by, a, events)

def Mcp.wake.cancel.k(ok: Bool, +now: Nat, +id: String) -> Res:
  match ok:
    case True{}:
      Res{True{}, J.Json.obj([J.KV{"id", J.Str{id}}, J.KV{"cancelled", J.Flag{True{}}}]), B.Step{[M.WakeDropped{id, now}], Nil{}}}
    case False{}:
      Res.fail("no such wake subscription owned by this thread")

def Mcp.wake.cancel(+s: M.State, +now: Nat, +by: String, +id: String) -> Res:
  Mcp.wake.cancel.k(Bool.and(Bool.not(String.is_empty(id)), String.eq(J.Json.get_str(WK.Wake.get(s, id), "by", ""), by)), now, id)

# deploy_restart: scheduled at once; the hub restarts when no tool call runs
# and the build locks are free, and wakes the requester after
def Mcp.deploy.restart(ok: Bool, +s: M.State, +now: Nat, +thread: String, +a: J.Json) -> Res:
  match ok:
    case True{}:
      Res{True{}, J.Json.obj([J.KV{"state", J.Str{"scheduled"}}, J.KV{"next", J.Str{"Scheduled: the hub restarts when no tool call is running and the build lock is free. You will be woken after it, with the new version. End your turn."}}]),
        B.Deploy.ask(s, now, Bool.pick(String, String.eq(thread, "deploy"), "", thread), J.Json.get_str(a, "note", ""), J.Json.get_str(a, "stage", ""))}
    case False{}:
      Res.fail("stage must be an absolute path")

# ---- mail 2b ----
def Mcp.mail.tools() -> List<&2, J.Json>:
  [Tool.def("mail", "Send mail to an address, delivered at once and filed in its conversation.",
    [Tool.prop("to", "string", "thread:<id>, bot:<id> or room:<id>."), Tool.prop("text", "string", "The message."), Tool.prop("conv", "string", "Continue this conversation."), Tool.prop("subject", "string", "Subject for a new conversation."), Tool.prop("want", "string", "fyi or act.")], ["text"]),
   Tool.def("mail_list", "List your mail conversations, newest first, optionally by status or project.", [Tool.prop("bot", "string", "Bot id; * for all mail (bots only)."), Tool.prop("status", "string", "unread, open, snoozed or done."), Tool.prop("project", "string", "Project id."), Tool.prop("n", "number", "Page size."), Tool.prop("skip", "number", "Rows to skip.")], Nil{}),
   Tool.def("mail_read", "Read a conversation's message bodies and mark it seen.", [Tool.prop("conv", "string", "Conversation id."), Tool.prop("before", "string", "Mail id cursor."), Tool.prop("n", "number", "Page size (default 50).")], ["conv"]),
   Tool.def("mail_search", "Search mail subjects, participants, tags and newest headers.", [Tool.prop("q", "string", "Query."), Tool.prop("bot", "string", "Bot id or *."), Tool.prop("n", "number", "Maximum results.")], ["q"]),
   Tool.def("mail_tag", "Set your conversation's tags.", [Tool.prop("conv", "string", "Conversation id."), J.KV{"tags", J.Json.obj([J.KV{"type", J.Str{"array"}}, J.KV{"items", J.Json.obj([J.KV{"type", J.Str{"string"}}])}])}], ["conv", "tags"]),
   Tool.def("mail_snooze", "Snooze a conversation until a time; the minute tick reopens it unread.", [Tool.prop("conv", "string", "Conversation id."), Tool.prop("until", "number", "Unix time in seconds."), Tool.prop("in", "string", "Relative time, e.g. 20m or 2h.")], ["conv"]),
   Tool.def("mail_done", "Mark a conversation done.", [Tool.prop("conv", "string", "Conversation id.")], ["conv"])]

def Mcp.tools() -> J.Json:
  J.Json.obj([J.KV{"tools", J.Json.arr(List.append(&2, J.Json, [
    Tool.def("orchestrator_capabilities",
      "List the providers and models this Backplane can run, the delegation depth limit, and this thread's own provider, model and depth.",
      Nil{}, Nil{}),
    Tool.def("delegate_task",
      "Delegate one task to a child agent of THIS thread, on any provider/model (see orchestrator_capabilities). The child sees only `prompt` (no conversation history) and works in this thread's directory. Its result arrives in this thread as a message when it finishes, so end your turn instead of polling; use task_status only when you need it mid-turn. Delegation is limited in depth; children of children are allowed up to the limit.",
      [Tool.prop("prompt", "string", "The complete task for the child, with every detail it needs."),
       Tool.prop("provider", "string", "claude, codex, grok, muse, antigravity or omp (default: this thread's)."),
       Tool.prop("model", "string", "Model for the child (default: this thread's)."),
       Tool.prop("title", "string", "Short title for the child thread."),
       Tool.prop("project", "string", "Bots only, and required for them: the project's name or id (project_list); the child is a thread of its own there that the person can follow.")], ["prompt"]),
    Tool.def("task_status", "Read a delegated task: state (queued, running, done, failed, cancelled), the child's thread, and its result summary once finished.",
      [Tool.prop("taskId", "string", "The task id delegate_task returned.")], ["taskId"]),
    Tool.def("task_cancel", "Cancel a running delegated task (stops its child agent). Finished tasks keep their result.",
      [Tool.prop("taskId", "string", "The task id.")], ["taskId"]),
    Tool.def("thread_list", "List the threads of this thread's project (id, title, provider, model, turn state), newest activity first. With `machine` (machine_list shows them), the threads of that other machine of the person's, whose ids work in the thread tools here; `project` narrows it to one project there.",
      [Tool.prop("machine", "string", "Another machine's name (or link id): list its threads."), Tool.prop("project", "string", "With machine: a project there, by name or id.")], Nil{}),
    Tool.def("machine_list", "List the person's machines linked to this one (this one first), whether each is here or away, and each one's projects with ids you can give thread_launch. A thread elsewhere has an id like <link>~<id>; thread_send, thread_read, thread_wait, thread_interrupt and thread_update take it, so you can work with a thread on any of their machines.", Nil{}, Nil{}),
    Tool.def("thread_read", "Read a thread's timeline in this project (or on another machine, by its <link>~<id>): messages and tool activity, oldest first (the last `limit` entries).",
      [Tool.prop("threadId", "string", "The thread to read."), Tool.prop("limit", "number", "How many entries (default 40).")], ["threadId"]),
    Tool.def("thread_send", "Send a message to another thread in this project, or on another machine by its <link>~<id>. It is delivered at once: an idle thread starts a turn with it, a running turn takes it (Claude at once; Codex, Grok and Muse with their next Backplane tool result, or as a new turn if theirs ends first).",
      [Tool.prop("threadId", "string", "The thread."), Tool.prop("text", "string", "The message.")], ["threadId", "text"]),
    Tool.def("thread_launch", "Start a new top-level thread in this project, or in `project` (not a child of this one: the user sees it in the sidebar like their own) with a first message, on any provider/model. Bots must name the project. Answers its threadId. Its reports come to you by themselves, as a message that wakes you: each time its turn ends (completed, failed or interrupted, with its last reply), and at once when it asks the person something; so end your turn instead of polling, and answer it with thread_send. report false launches it without reports; thread_update turns them off or on later. With `machine`, it starts on that other machine of the person's, in its project named by `project` (machine_list).",
      [Tool.prop("prompt", "string", "The first message, complete."), Tool.prop("title", "string", "A short title (default: from the prompt)."), Tool.prop("report", "boolean", "Whether its turn ends and asks are reported to you (default true)."),
       Tool.prop("provider", "string", "claude, codex, grok, muse, antigravity or omp (default: the user's default)."),
       Tool.prop("project", "string", "The project's name or id (default: this thread's; bots must give one); on another machine its <link>~<id> from machine_list."),
       Tool.prop("machine", "string", "Start it on another machine of the person's, in its project of that name (machine_list).")], ["prompt"]),
    Tool.def("thread_update", "Rename a thread in this project (this one by default), or with no title have its title written again from its first message. With report (and no title), turn the reports of a thread you launched off (false) or on again (true).",
      [Tool.prop("threadId", "string", "The thread (default: this one)."), Tool.prop("title", "string", "The new title; empty to regenerate it."), Tool.prop("report", "boolean", "For a thread you launched: whether its reports come to you.")], Nil{}),
    Tool.def("thread_wait", "Wait until a thread in this project has finished its turn, or until timeoutMs passes. Answers its state, whether it is done, and its last reply. For anything longer than a few minutes use wake_on instead and end your turn.",
      [Tool.prop("threadId", "string", "The thread."), Tool.prop("timeoutMs", "number", "How long to wait at most (default 30000, at most 600000).")], ["threadId"]),
    Tool.def("thread_interrupt", "Stop the running turn of a thread in this project.",
      [Tool.prop("threadId", "string", "The thread.")], ["threadId"]),
    Tool.def("deploy_restart", "Restart the Backplane hub into a new build safely. Answers at once: the restart waits until no thread is inside a tool call (a build, an edit) and the build lock (/tmp/bp-bend.lock) is free, then goes; every turn it cuts resumes by itself. With stage, a directory holding the built package (backplane, backplane-serve, web/ ...) is swapped into the install first. You are woken after the restart with the new version: end your turn, do not wait.",
      [Tool.prop("stage", "string", "Absolute path of a staged package to install first (default: the files are already in place)."), Tool.prop("note", "string", "Why, shown to the person.")], Nil{}),
    Tool.def("deploy_status", "Whether a deploy_restart waits, who asked, and what it still waits for (tool calls in flight, the build lock); under main, deploy_main's state (the commit building, staged, live).", Nil{}, Nil{}),
    Tool.def("deploy_main", "Deploy origin/main to this machine's hub: the hub fetches it in its source checkout (setting deploy.repo), builds it in a job of its own and restarts into it safely (as deploy_restart). Answers at once: queued, skipped (origin/main is already in the commit being built, staged or live: the answer names it) or not configured. One build runs at a time; requests during it make one more build of the newest origin/main. When the build ends you are told (commit, version, result): end your turn, do not wait or poll.",
      [Tool.prop("note", "string", "Why, shown with the request.")], Nil{}),
    Tool.def("deploy_cancel", "Cancel a waiting deploy_restart.", Nil{}, Nil{}),
    Tool.def("viewer_show",
      "Show a file in the user's Backplane viewer panel: a .kicad_pcb on the Board tab (kind 3d: the board in 3D), a .kicad_sch on the Schematic tab, and any .step/.stp or .glb on the Mech tab, the 3D design tab (the model turned in 3D, the project's parts in mech/ beside it, their renders a tap away), wherever the file is. Use it after changing a design so the user sees the result; the viewer also follows the file live.",
      [Tool.prop("path", "string", "Absolute path of the file."), Tool.prop("kind", "string", "board, schematic, 3d (a board in 3D) or mech (a STEP or GLB); default from the extension.")], ["path"]),
    Tool.def("kicad_export10", "Export an outside-project KiCad 10 copy. First returns a compatibility report; force accepts the reported changes. Never opens a GUI.", [Tool.prop("path", "string", "Absolute .kicad_pro path."), Tool.prop("force", "boolean", "Accept changes, including dropping blocked approximations (default false).")], ["path"]),
    Tool.def("terminal_open",
      "Open a terminal in this chat that runs a command (through the user's shell, in this thread's folder) and that the user sees and types into right in the conversation. Use it to hand the user a small tool: a TUI, a picker, an interactive script, a live monitor, a REPL. It answers the terminal's id at once; when the command ends the terminal stays open at the user's shell. Read its screen with terminal_read; what it shows when the user next writes to you comes with their message. It stops (and greys out, a click restarts it) when the user sends another message, unless they pin it.",
      [Tool.prop("command", "string", "The command line, e.g. python3 tools/pick.py or htop."), Tool.prop("title", "string", "A short title shown above it."),
       Tool.prop("cols", "number", "Columns (default 80, 20 to 200)."), Tool.prop("rows", "number", "Rows (default 24, 3 to 60).")], ["command"]),
    Tool.def("terminal_read", "Read a terminal of this chat: its state (running: the command runs; done N: it ended with status N and the shell is open; stopped or exited N: the terminal is closed) and the text on its screen. waitMs waits until the command ends, up to that long, for a script you want the result of.",
      [Tool.prop("id", "string", "The id terminal_open answered."), Tool.prop("waitMs", "number", "Wait for the command to end, at most this long (default 0, at most 120000).")], ["id"]),
    Tool.def("terminal_send", "Type into a terminal of this chat, as the user would: text is sent as is (\\r is Enter, \\u0003 is ctrl+c, \\u001b[A is Up).",
      [Tool.prop("id", "string", "The terminal's id."), Tool.prop("text", "string", "The bytes to send, as text.")], ["id", "text"]),
    Tool.def("terminal_close", "Stop a terminal of this chat (its program is hung up; the user can start it again).",
      [Tool.prop("id", "string", "The terminal's id.")], ["id"]),
    Tool.def("wake_on", "Subscribe to a local thread, work id, PR or absolute file path. A matching event posts a message in your thread and wakes you; end your turn instead of polling. Thread targets elsewhere are not supported yet.",
      [Tool.prop("kind", "string", "thread, work, pr or file."), Tool.prop("target", "string", "Thread/work id, owner/repo#N (or #N in a project), or absolute path."),
       J.KV{"events", J.Json.obj([J.KV{"type", J.Str{"array"}}, J.KV{"items", J.Json.obj([J.KV{"type", J.Str{"string"}}])}])}, Tool.prop("once", "boolean", "Drop after delivery (default true)."), Tool.prop("note", "string", "Include this reminder in the wake message.")], ["kind", "target"]),
    Tool.def("wake_list", "List your wake subscriptions.", Nil{}, Nil{}),
    Tool.def("wake_cancel", "Cancel one of your wake subscriptions.", [Tool.prop("id", "string", "The subscription id.")], ["id"]),
    Tool.def("work_run", "Start long or detached work (a build, a test run, a job on another machine) so Backplane owns it: when it ends you are woken with its exit code and the end of its output; if a restart of Backplane cuts it off you are told it was lost and can start it again. Use this instead of `&`, nohup or disown, then end your turn; never poll or sleep-wait.",
      [Tool.prop("command", "string", "The shell command, run in this thread's folder."), Tool.prop("lock", "string", "Optional named FIFO lock, held until work ends."), Tool.prop("note", "string", "What it is, in a few words."), Tool.prop("timeoutSec", "number", "Give up watching after this long (the work keeps running; the outcome is then unknown).")], ["command"]),
    Tool.def("work_watch", "Have Backplane watch work that is already running and wake you when it ends: kind pid (a process) or file (a file that will contain the token this call returns). Then end your turn.",
      [Tool.prop("kind", "string", "pid or file."), Tool.prop("target", "string", "The pid, or the file's path."), Tool.prop("note", "string", "What it is."), Tool.prop("timeoutSec", "number", "Give up watching after this long.")], ["kind", "target"]),
    Tool.def("lock_acquire", "Acquire a named FIFO lock without blocking; if queued, end your turn and wait for the grant message.", [Tool.prop("name", "string", "Lock name"), Tool.prop("note", "string", "Purpose")], ["name"]),
    Tool.def("lock_release", "Release your named lock, granting the next waiter.", [Tool.prop("name", "string", "Lock name")], ["name"]),
    Tool.def("lock_cancel", "Leave a named lock queue.", [Tool.prop("name", "string", "Lock name")], ["name"]),
    Tool.def("lock_list", "List named lock holders, timestamps, notes and FIFO waiters.", Nil{}, Nil{}),
    Tool.def("work_list", "List the work Backplane watches for this thread (work_run and work_watch), with each one's state.", Nil{}, Nil{}),
    Tool.def("work_cancel", "Stop work started with work_run (or a watched unit or pid). It shows as cancelling until its end is confirmed.",
      [Tool.prop("id", "string", "The work's id.")], ["id"]),
    Tool.def("work_unwatch", "Stop watching work without stopping it: it may still run, and you will not be told when it ends.",
      [Tool.prop("id", "string", "The work's id.")], ["id"]),
    Tool.def("review_comments", "The design review comments on this thread's design (a person's or an agent's, pinned to a place on a board layer or a schematic sheet): the open ones with their replies, and how many are resolved.",
      Nil{}, Nil{}),
    Tool.def("review_comment", "Comment on the design, pinned to a place (op add: file, x, y in mm in the file's own coordinates, layer for a board, text), or answer (op add with parent), resolve, reopen, edit or drop a comment by its id. The person sees it pinned in the viewer.",
      [Tool.prop("op", "string", "add, resolve, reopen, edit or drop."), Tool.prop("id", "string", "The comment (resolve, reopen, edit, drop)."),
       Tool.prop("parent", "string", "The comment answered (an add that is a reply)."), Tool.prop("file", "string", "The board or schematic, from this thread's folder."),
       Tool.prop("x", "number", "X in mm."), Tool.prop("y", "number", "Y in mm."), Tool.prop("layer", "string", "A board's layer (F.Cu, B.Silkscreen...)."),
       Tool.prop("text", "string", "What the comment says.")], ["op"])]
    , List.append(&2, J.Json, Mcp.preview_tools(), List.append(&2, J.Json, Mcp.kicad_tools(), [Tool.def("permission_prompt", "Internal to Backplane: asks the user before a tool runs. Do not call it yourself.",
      [Tool.prop("tool_name", "string", "The tool."), Tool.prop("input", "object", "Its input."), Tool.prop("tool_use_id", "string", "The call.")], ["tool_name", "input"])]))))}])

def Mcp.mail.uber(+s: M.State, +thread: String) -> Bool:
  UBM.Uber.is(s, Mcp.mail.scope(s, thread))

def Mcp.mail.params(+s: M.State, +thread: String, +a: J.Json) -> J.Json:
  # only the uberbot may look at another bot's mailroom or all of them ("*")
  J.Json.obj([J.KV{"bot", J.Str{Bool.pick(String, Mcp.mail.uber(s, thread), J.Json.get_str(a, "bot", Mcp.mail.scope(s, thread)), Mcp.mail.scope(s, thread))}},
    J.KV{"status", J.Str{J.Json.get_str(a, "status", "")}}, J.KV{"project", J.Str{J.Json.get_str(a, "project", "")}}, J.KV{"q", J.Str{J.Json.get_str(a, "q", "")}},
    J.KV{"skip", J.Json.nat(J.Json.get_nat(a, "skip", 0n))}, J.KV{"n", J.Json.nat(J.Json.get_nat(a, "n", J.Json.get_nat(a, "limit", 50n)))},
    J.KV{"conv", J.Str{J.Json.get_str(a, "conv", "")}}, J.KV{"before", J.Str{J.Json.get_str(a, "before", "")}}, J.KV{"tags", J.Json.get_obj(a, "tags")}])

def Mcp.mail.allowed(+s: M.State, +thread: String, +conv: String) -> Bool:
  +scope = Mcp.mail.scope(s, thread)
  Bool.or(Mcp.mail.uber(s, thread),
    ML.Parts.has(ML.Conv.parts(ML.Box.conv(M.State.mail(s), conv)), Bool.pick(String, String.starts_with(scope, "thread:"), scope, "bot:" ++ scope), False{}))

def Mcp.mail.until(+s: M.State, +now: Nat, +a: J.Json) -> Nat:
  Bool.pick(Nat, Nat.is_gt(J.Json.get_nat(a, "until", 0n), 0n), J.Json.get_nat(a, "until", 0n),
    Maybe.default(&2, Nat, WH.When.parse("in " ++ J.Json.get_str(a, "in", ""), now, SC.Sched.person(s, 0n, False{})), 0n))

def Mcp.mail.change(+s: M.State, +now: Nat, +thread: String, +a: J.Json, +snooze: Bool) -> Res:
  +p = J.Json.obj([J.KV{"bot", J.Str{Mcp.mail.scope(s, thread)}}, J.KV{"conv", J.Str{J.Json.get_str(a, "conv", "")}},
    J.KV{"status", J.Str{Bool.pick(String, snooze, "snoozed", "done")}}, J.KV{"until", J.Json.nat(Mcp.mail.until(s, now, a))}])
  Res{True{}, J.Json.obj([J.KV{"ok", J.Flag{True{}}}]), B.Step{B.Mail.mark(s, now, p), Nil{}}}

def Mcp.run.reply.part(+ps: List<&2, String>, +bot: String, +th: String) -> String:
  match ps:
    case Nil{}: ""
    case +h <> +t: Bool.pick(String, Bool.or(String.eq(h, "bot:" ++ bot), String.eq(h, "thread:" ++ th)), Mcp.run.reply.part(t, bot, th), h)

def Mcp.run.mail.got(r: Maybe<&2, ML.Run>, +s: M.State, +a: J.Json) -> J.Json:
  match r:
    case None{}: a
    case Some{+r}: J.Json.obj([J.KV{"to", J.Str{J.Json.get_str(a, "to", Mcp.run.reply.part(ML.Conv.parts(ML.Box.conv(M.State.mail(s), ML.Run.conv(r))), ML.Run.bot(r), ML.Run.thread(r))) }},
      J.KV{"text", J.Str{J.Json.get_str(a, "text", "")}}, J.KV{"conv", J.Str{J.Json.get_str(a, "conv", ML.Run.conv(r))}}, J.KV{"subject", J.Str{J.Json.get_str(a, "subject", "")}}, J.KV{"want", J.Str{J.Json.get_str(a, "want", "")}}])

def Mcp.mail.call.on(+ok: Bool, +i: Nat, +s: M.State, +now: Nat, +thread: String, +a: J.Json) -> Res:
  match ok i:
    case False{} _: Res.fail("mail is off or the conversation is not yours")
    case True{} 34n:
      Res{True{}, J.Json.obj([J.KV{"ok", J.Flag{True{}}}]), B.Mail.send(s, now, thread, Mcp.run.mail.got(ML.Box.run_of(M.State.mail(s), thread), s, a))}
    case True{} 35n: Res.ok(B.Mail.list(s, Mcp.mail.params(s, thread, a)))
    case True{} 36n: Res{True{}, J.Null{}, B.Mail.read(s, now, 0, "0", Mcp.mail.params(s, thread, a))}
    case True{} 37n: Res.ok(B.Mail.find(s, Mcp.mail.params(s, thread, a)))
    case True{} 38n: Res{True{}, J.Json.obj([J.KV{"ok", J.Flag{True{}}}]), B.Step{B.Mail.tag(now, Mcp.mail.params(s, thread, a)), Nil{}}}
    case True{} 39n: Mcp.mail.change(s, now, thread, a, True{})
    case True{} _: Mcp.mail.change(s, now, thread, a, False{})

def Mcp.mail.call(+i: Nat, +s: M.State, +now: Nat, +thread: String, +a: J.Json) -> Res:
  Mcp.mail.call.on(Bool.and(String.eq(M.Setting.get(s, "mail.on", "off"), "on"), Bool.or(ML.Parts.has(["34", "35", "37"], Nat.show(i), False{}), Mcp.mail.allowed(s, thread, J.Json.get_str(a, "conv", "")))), i, s, now, thread, a)

# deploy_main (core/ship.bend, hub.bend's Ship.*). The server reads
# origin/main first (server.bend's Ship.look) and answers through Mcp.ship;
# reaching the decider unread means no deploy.repo is set.
def Mcp.ship.none(empty: Bool) -> Res:
  match empty:
    case True{}:
      Res.ok(J.Json.obj([J.KV{"state", J.Str{"not configured"}}, J.KV{"next", J.Str{"deploy_main is off on this hub: the setting deploy.repo (the source checkout to build) is not set. Tell the person; do not deploy by hand."}}]))
    case False{}:
      Res.fail("deploy_main: origin/main was not read; try again")

def Mcp.ship.answer(+cover: String, start: Bool, +sh: SH.Ship, +head: String) -> J.Json:
  Bool.pick(J.Json, Bool.not(String.is_empty(cover)),
    J.Json.obj([J.KV{"state", J.Str{"skipped"}}, J.KV{"head", J.Str{head}}, J.KV{"commit", J.Str{SH.Ship.by(sh, cover)}}, J.KV{"covered", J.Str{cover}},
      J.KV{"next", J.Str{Bool.pick(String, String.eq(cover, "building"), "origin/main is in the build under way; you will be told when it ends. End your turn.", "origin/main is already " ++ cover ++ " here; nothing to do.")}}]),
    Bool.pick(J.Json, start,
      J.Json.obj([J.KV{"state", J.Str{"queued"}}, J.KV{"commit", J.Str{head}}, J.KV{"next", J.Str{"Building it. You will be told when the build ends (commit, version, result). End your turn."}}]),
      J.Json.obj([J.KV{"state", J.Str{"queued"}}, J.KV{"head", J.Str{head}}, J.KV{"after", J.Str{SH.Ship.commit(sh)}},
        J.KV{"next", J.Str{"A build is under way; one more of the newest origin/main follows it. You will be told when that ends. End your turn."}}])))

def Mcp.ship.k(ok: Bool, +s: M.State, +now: Nat, +by: String, +note: String, +out: String) -> Res:
  match ok:
    case True{}:
      Res{True{}, Mcp.ship.answer(SH.Ship.cover(B.Ship.eff(s), SH.Look.head(out), SH.Look.within(out)), SH.Ship.start(B.Ship.eff(s), SH.Look.head(out), SH.Look.within(out)), B.Ship.eff(s), SH.Look.head(out)),
        B.Ship.ask(s, now, by, note, SH.Look.head(out), SH.Look.within(out))}
    case False{}:
      Res.fail("deploy_main could not read origin/main in " ++ B.Ship.repo(s) ++ ": " ++ SH.Look.err(out))

# deploy_main with origin/main read (out: the look's lines)
def Mcp.ship(+s: M.State, +now: Nat, +thread: String, +a: J.Json, +out: String) -> Res:
  Mcp.ship.k(String.is_empty(SH.Look.err(out)), s, now, B.Ship.by(thread), J.Json.get_str(a, "note", ""), out)

def Mcp.run.decide(+s: M.State, +now: Nat, +r: ML.Run, +text: String, +reason: String) -> B.Step:
  B.Step{[M.DecisionLogged{B.Hub.id("decision", now, B.Hub.seq(s)), ML.Run.bot(r), ML.Run.conv(r), ML.Run.thread(r), text, reason, now}], Nil{}}

def Mcp.run.escalate(ok: Bool, +s: M.State, +now: Nat, +r: ML.Run, +a: J.Json) -> Res:
  match ok:
    case False{}: Res.fail("to must be bot or person")
    case True{}: Res{True{}, J.Json.obj([J.KV{"ok", J.Flag{True{}}}]), B.Run.escalate(s, now, ML.Run.bot(r), ML.Run.conv(r), J.Json.get_str(a, "to", "bot"), J.Json.get_str(a, "text", ""))}

def Mcp.run.call.got(r: Maybe<&2, ML.Run>, +i: Nat, +s: M.State, +now: Nat, +a: J.Json) -> Res:
  match r i:
    case Some{+r} 41n: Res{True{}, J.Json.obj([J.KV{"ok", J.Flag{True{}}}]), Mcp.run.decide(s, now, r, J.Json.get_str(a, "text", ""), J.Json.get_str(a, "reason", ""))}
    case Some{+r} 42n: Mcp.run.escalate(ML.Parts.has(["bot", "person"], J.Json.get_str(a, "to", ""), False{}), s, now, r, a)
    case _ _: Res.fail("This tool is only available in a mailroom run")

def Mcp.charter.got(b: Maybe<&2, BT.Bot>, +s: M.State, +now: Nat, +a: J.Json) -> Res:
  match b:
    case None{}: Res.fail("charter_set is for the front of house bot only")
    case Some{+b}:
      Res{True{}, J.Json.obj([J.KV{"ok", J.Flag{True{}}}]), B.Step{[M.CharterSet{BT.Bot.id(b), J.Json.get_str(a, "text", ""), "bot", now},
        M.ActivityLogged{BT.Bot.thread(b), B.Hub.id("charter", now, B.Hub.seq(s)), "info", "charter", J.Json.get_str(a, "reason", ""), now}],
        [B.CharterWrite{B.Hub.cwd(s, B.Hub.thread(s, BT.Bot.thread(b))) ++ "/CHARTER.md", J.Json.get_str(a, "text", "")}]}}

def Mcp.run.call(on: Bool, +i: Nat, +s: M.State, +now: Nat, +thread: String, +a: J.Json) -> Res:
  match on i:
    case False{} _: Res.fail("mail is off")
    case True{} 43n: Mcp.charter.got(BT.World.of_thread(M.State.bots(s), thread), s, now, a)
    case True{} _: Mcp.run.call.got(ML.Box.run_of(M.State.mail(s), thread), i, s, now, a)

def Mcp.call.k(i: Nat, +s: M.State, +now: Nat, +thread: String, +models: J.Json, +a: J.Json) -> Res:
  match i:
    case 0n:
      Res.ok(Mcp.caps(s, thread, models))
    case 1n:
      Mcp.delegate(s, now, thread, a)
    case 2n:
      Mcp.status(Mcp.task(s, J.Json.get_str(a, "taskId", ""), thread))
    case 3n:
      Mcp.cancel(Mcp.task(s, J.Json.get_str(a, "taskId", ""), thread), now)
    case 4n:
      Mcp.list(s, thread)
    case 5n:
      Mcp.read.k(Mcp.same_project(s, thread, J.Json.get_str(a, "threadId", "")), s, J.Json.get_str(a, "threadId", ""),
        Maybe.default(&2, Nat, Nat.read(J.Json.as_raw(J.Json.get(a, "limit"), "40")), 40n))
    case 6n:
      Mcp.send.k(Bool.or(Mcp.same_project(s, thread, J.Json.get_str(a, "threadId", "")), B.Lr.may(s, thread, J.Json.get_str(a, "threadId", ""))), s, now, thread, J.Json.get_str(a, "threadId", ""), J.Json.get_str(a, "text", ""), J.Json.get_str(a, "mode", ""))
    case 7n:
      Mcp.view(thread, J.Json.get_str(a, "path", ""), J.Json.get_str(a, "kind", ""))
    case 8n:
      Mcp.launch(s, now, thread, a)
    case 9n:
      Mcp.update.r(Mcp.flag(J.Json.get(a, "report")), s, now, thread, Mcp.or(J.Json.get_str(a, "threadId", ""), thread), J.Json.get_str(a, "title", ""))
    case 10n:
      Mcp.wait.k(Mcp.same_project(s, thread, J.Json.get_str(a, "threadId", "")), s, J.Json.get_str(a, "threadId", ""))
    case 11n:
      Mcp.interrupt.f(Mcp.same_project(s, thread, J.Json.get_str(a, "threadId", "")), B.Hub.running(B.Hub.thread(s, J.Json.get_str(a, "threadId", ""))), s, now, thread, J.Json.get_str(a, "threadId", ""))
    case 12n:
      Mcp.wait.k(Mcp.same_project(s, thread, J.Json.get_str(a, "threadId", "")), s, J.Json.get_str(a, "threadId", ""))
    case 13n:
      Mcp.term.open(s, now, thread, J.Json.get_str(a, "command", ""), J.Json.get_str(a, "title", ""), EM.Emb.cols(J.Json.get_u32(a, "cols", 0)), EM.Emb.rows(J.Json.get_u32(a, "rows", 0)))
    case 14n:
      Mcp.term.op(String.is_empty(EM.Emb.sum(M.State.mine(s, thread), J.Json.get_str(a, "id", ""))), True{}, thread, J.Json.get_str(a, "id", ""), J.Json.get_str(a, "text", ""))
    case 15n:
      Mcp.term.op(String.is_empty(EM.Emb.sum(M.State.mine(s, thread), J.Json.get_str(a, "id", ""))), False{}, thread, J.Json.get_str(a, "id", ""), "")
    case 16n:
      Mcp.work.run(Bool.or(String.is_empty(String.trim(J.Json.get_str(a, "command", ""))), Bool.and(Bool.not(String.is_empty(J.Json.get_str(a, "lock", ""))), Bool.not(LK.Lock.name.valid(J.Json.get_str(a, "lock", ""))))), s, now, thread, a)
    case 17n:
      Mcp.work.watch(Nat.is_lt(C.Codec.pos(["pid", "file"], J.Json.get_str(a, "kind", "")), 2n), s, now, thread, a)
    case 18n:
      Res.ok(J.Json.obj([J.KV{"work", J.Json.arr(Mcp.work.rows(M.Work.watch(s, thread)))}]))
    case 19n:
      Mcp.work.cancel(B.Watch.find(M.Work.watch(s, thread), J.Json.get_str(a, "id", "")), s, now, thread, J.Json.get_str(a, "id", ""))
    case 20n:
      Mcp.work.unwatch(B.Watch.find(M.Work.watch(s, thread), J.Json.get_str(a, "id", "")), s, now, thread, J.Json.get_str(a, "id", ""))
    case 21n:
      Mcp.review.list(RVW.Review.of_state(s, thread))
    case 23n:
      Mcp.wake.on(s, now, thread, a)
    case 24n:
      Res.ok(J.Json.obj([J.KV{"wakes", J.Json.arr(WK.Wake.list(WK.Wake.by(s, thread), s))}]))
    case 29n:
      Mcp.lock.acquire(LK.Lock.name.valid(J.Json.get_str(a, "name", "")), s, now, thread, a)
    case 30n:
      Res{True{}, J.Json.obj([J.KV{"released", J.Flag{String.eq(LK.Lock.owner(M.Locks.get(s, J.Json.get_str(a, "name", ""))), thread)}}]), B.Locks.release(s, now, J.Json.get_str(a, "name", ""), thread)}
    case 31n:
      Res.ok(J.Json.obj([J.KV{"locks", J.Json.arr(B.Locks.list.go(M.Locks.names(s), s))}]))
    case 32n:
      Res{True{}, J.Json.obj([J.KV{"cancelled", J.Flag{True{}}}]), B.Locks.cancel(s, now, J.Json.get_str(a, "name", ""), thread)}
    case 25n:
      Mcp.wake.cancel(s, now, thread, J.Json.get_str(a, "id", ""))
    case 22n:
      Mcp.review.do(RVW.Review.op.ok(J.Json.get_str(a, "op", ""), a), s, now, thread, a)
    case 26n:
      Mcp.deploy.restart(B.Deploy.stage.ok(J.Json.get_str(a, "stage", "")), s, now, thread, a)
    case 27n:
      Res.ok(DP.Deploy.status(s))
    case 28n:
      Res{True{}, J.Json.obj([J.KV{"state", J.Str{"cancelled"}}]), B.Deploy.cancel(s, now)}
    case 33n:
      Mcp.ship.none(String.is_empty(B.Ship.repo(s)))
    case 34n: Mcp.mail.call(i, s, now, thread, a)
    case 35n: Mcp.mail.call(i, s, now, thread, a)
    case 36n: Mcp.mail.call(i, s, now, thread, a)
    case 37n: Mcp.mail.call(i, s, now, thread, a)
    case 38n: Mcp.mail.call(i, s, now, thread, a)
    case 39n: Mcp.mail.call(i, s, now, thread, a)
    case 40n: Mcp.mail.call(i, s, now, thread, a)
    case 41n: Mcp.run.call(String.eq(M.Setting.get(s, "mail.on", "off"), "on"), i, s, now, thread, a)
    case 42n: Mcp.run.call(String.eq(M.Setting.get(s, "mail.on", "off"), "on"), i, s, now, thread, a)
    case 43n: Mcp.run.call(String.eq(M.Setting.get(s, "mail.on", "off"), "on"), i, s, now, thread, a)
    case _:
      Res.fail("unknown tool")

# Permission prompts (Claude's --permission-prompt-tool)
# -----------------------------------------------------
# Claude asks before a tool runs outside full access. The request becomes
# an ask the user answers; the MCP reply waits (the server parks it) until
# then. AskUserQuestion becomes a question; ExitPlanMode's plan becomes a
# proposed plan, and Claude is told to stop and wait.

def Perm.text(+verdict: J.Json) -> J.Json:
  J.Json.obj([J.KV{"content", J.Json.arr([J.Json.obj([J.KV{"type", J.Str{"text"}}, J.KV{"text", J.Str{J.Json.show(verdict)}}])])}])

def Perm.allow(+input: J.Json) -> J.Json:
  J.Json.obj([J.KV{"behavior", J.Str{"allow"}}, J.KV{"updatedInput", input}])

def Perm.deny(+msg: String) -> J.Json:
  J.Json.obj([J.KV{"behavior", J.Str{"deny"}}, J.KV{"message", J.Str{msg}}])

def Perm.plan_note() -> String:
  "The client captured your proposed plan. Stop here and wait for the user's feedback or implementation request."

def Perm.detail.k(+short: String, +input: J.Json) -> String:
  Bool.pick(String, String.is_empty(short), String.take(J.Json.show(input), 400n), short)

# what the approval card says the tool will do
def Perm.detail(+tool: String, +input: J.Json) -> String:
  Perm.detail.k(J.Json.get_str(input, "command", J.Json.get_str(input, "file_path", J.Json.get_str(input, "url", J.Json.get_str(input, "pattern", "")))), input)

def Perm.first_q(+input: J.Json) -> J.Json:
  Maybe.default(&2, J.Json, List.head(&2, J.Json, J.Json.elems(J.Json.get_obj(input, "questions"))), J.Null{})

def Perm.labels(os: List<&2, J.Json>) -> List<&2, String>:
  match os:
    case Nil{}:
      Nil{}
    case +o <> t:
      J.Json.get_str(o, "label", J.Json.show(o)) <> Perm.labels(t)

def Perm.question(+input: J.Json) -> String:
  J.Json.get_str(Perm.first_q(input), "question", J.Json.get_str(input, "question", ""))

def Perm.options(+input: J.Json) -> String:
  String.join(Perm.labels(J.Json.elems(J.Json.get_obj(Perm.first_q(input), "options"))), "\n")

def Perm.session_ok(as: List<&2, M.Ask>, +thread: String, +tool: String) -> Bool:
  match as:
    case Nil{}:
      False{}
    case M.Ask{_, th, k, t, _, _, _, an} <> rest:
      Bool.or(Bool.and(Bool.and(String.eq(th, thread), String.eq(t, tool)), Bool.and(String.eq(k, "approval"), String.eq(an, "session"))), Perm.session_ok(rest, thread, tool))

def Perm.approve(ok: Bool, +ask: String, +s: M.State, +now: Nat, +thread: String, +id: J.Json, +tool: String, +input: J.Json) -> McpOut:
  match ok:
    case True{}:
      McpOut{Mcp.result(id, Perm.text(Perm.allow(input))), B.Step.none(), ""}
    case False{}:
      McpOut{"", B.Step{[M.AskOpened{ask, thread, "approval", tool, Perm.detail(tool, input), "", now}], Nil{}}, ask}

def Perm.k(i: Nat, +ask: String, +s: M.State, +now: Nat, +thread: String, +id: J.Json, +tool: String, +input: J.Json) -> McpOut:
  match i:
    case 0n:
      McpOut{"", B.Step{[M.AskOpened{ask, thread, "input", tool, Perm.question(input), Perm.options(input), now}], Nil{}}, ask}
    case 1n:
      McpOut{Mcp.result(id, Perm.text(Perm.deny(Perm.plan_note()))), B.Step{[M.AskOpened{ask, thread, "plan", tool, J.Json.get_str(input, "plan", ""), "", now}], Nil{}}, ""}
    case _:
      Perm.approve(Perm.session_ok(M.Asks.all(s), thread, tool), ask, s, now, thread, id, tool, input)

def Mcp.permit.old(+s: M.State, +now: Nat, +thread: String, +id: J.Json, +a: J.Json) -> McpOut:
  Perm.k(C.Codec.pos(["AskUserQuestion", "ExitPlanMode"], J.Json.get_str(a, "tool_name", "")), B.Hub.id("q", now, B.Hub.seq(s)), s, now, thread, id, J.Json.get_str(a, "tool_name", ""), J.Json.get_obj(a, "input"))

def Perm.run.verdict(ok: Bool, +s: M.State, +now: Nat, +r: ML.Run, +id: J.Json, +tool: String, +input: J.Json) -> McpOut:
  match ok:
    case False{}: McpOut{Mcp.result(id, Perm.text(Perm.deny("Outside the charter; escalated"))), B.Run.escalate(s, now, ML.Run.bot(r), ML.Run.conv(r), "bot", "Outside the charter: " ++ tool ++ " " ++ String.take(J.Json.show(input), 1000n)), ""}
    case True{}: McpOut{Mcp.result(id, Perm.text(Perm.allow(input))), Mcp.run.decide(s, now, r, tool ++ " " ++ String.take(J.Json.show(input), 1000n), "charter: " ++ CH.Charter.line(ML.Box.charter(M.State.mail(s), ML.Run.bot(r)), tool, input)), ""}

def Perm.run.risky(risky: Bool, +s: M.State, +now: Nat, +r: ML.Run, +id: J.Json, +tool: String, +input: J.Json) -> McpOut:
  match risky:
    case False{}: McpOut{Mcp.result(id, Perm.text(Perm.allow(input))), B.Step.none(), ""}
    case True{}: Perm.run.verdict(CH.Charter.allows(ML.Box.charter(M.State.mail(s), ML.Run.bot(r)), tool, input), s, now, r, id, tool, input)

def Perm.run.got(r: Maybe<&2, ML.Run>, +s: M.State, +now: Nat, +thread: String, +id: J.Json, +a: J.Json) -> McpOut:
  match r:
    case None{}: Mcp.permit.old(s, now, thread, id, a)
    case Some{+r}: Perm.run.risky(CH.Charter.risky(J.Json.get_str(a, "tool_name", ""), J.Json.get_obj(a, "input")), s, now, r, id, J.Json.get_str(a, "tool_name", ""), J.Json.get_obj(a, "input"))

def Mcp.permit(+s: M.State, +now: Nat, +thread: String, +id: J.Json, +a: J.Json) -> McpOut:
  Perm.run.got(RUN.Run.of(s, thread), s, now, thread, id, a)

def Perm.answers.k(input: J.Json, extra: J.Json) -> J.Json:
  match input extra:
    case J.Obj{fs} J.Obj{J.Field{k, v, _}}:
      J.Obj{J.Field{k, v, fs}}
    case _ _:
      input

def Perm.answers(+input: J.Json, +answer: String) -> J.Json:
  Perm.answers.k(input, J.Json.obj([J.KV{"answers", J.Json.obj([J.KV{Perm.question(input), J.Str{answer}}])}]))

def Perm.verdict.k(i: Nat, +tool: String, +input: J.Json, +answer: String) -> J.Json:
  match i:
    case 0n:
      Bool.pick(J.Json, String.is_empty(answer), Perm.deny("The user dismissed the question."), Perm.allow(Perm.answers(input, answer)))
    case _:
      Bool.pick(J.Json, Bool.or(String.eq(answer, "accept"), String.eq(answer, "session")), Perm.allow(input), Perm.deny("The user declined this tool call."))

def Mcp.permission.reply.k(+o: J.Json, +answer: String) -> String:
  Mcp.result(J.Json.get_obj(o, "id"), Perm.text(Perm.verdict.k(C.Codec.pos(["AskUserQuestion"], J.Json.get_str(J.Json.get_obj(J.Json.get_obj(o, "params"), "arguments"), "tool_name", "")),
    J.Json.get_str(J.Json.get_obj(J.Json.get_obj(o, "params"), "arguments"), "tool_name", ""), J.Json.get_obj(J.Json.get_obj(J.Json.get_obj(o, "params"), "arguments"), "input"), answer)))

def Mcp.permission.reply.acp(i: Nat, omp: Bool, +o: J.Json, +body: String, +answer: String) -> String:
  match i omp:
    case 0n True{}:
      AC.Acp.permit.answer.once(body, answer)
    case 0n False{}:
      AC.Acp.permit.answer(body, answer)
    case 1n _:
      MS.Msp.decide(body, answer)
    case 2n _:
      AC.Acp.elicit.answer(body, answer)
    case _ _:
      Mcp.permission.reply.k(o, answer)

def Mcp.permission.reply.mode.o(omp: Bool, +o: J.Json, +body: String, +answer: String) -> String:
  Mcp.permission.reply.acp(C.Codec.pos(["session/request_permission", "approval/request", "elicitation/create"], J.Json.get_str(o, "method", "")), omp, o, body, answer)

def Mcp.permission.reply.mode(omp: Bool, +body: String, +answer: String) -> String:
  Mcp.permission.reply.mode.o(omp, Maybe.default(&2, J.Json, J.Json.parse(body), J.Null{}), body, answer)

# the parked reply for a request body, once the user answered
def Mcp.permission.reply(+body: String, +answer: String) -> String:
  Mcp.permission.reply.mode(False{}, body, answer)

# OMP's permission card is always a one-time approval
def Mcp.permission.reply.once(+body: String, +answer: String) -> String:
  Mcp.permission.reply.mode(True{}, body, answer)

def Mcp.reply(+id: J.Json, r: Res) -> McpOut:
  match r:
    case Res{ok, body, st}:
      McpOut{Mcp.result(id, Mcp.content(ok, body)), st, ""}

def Mcp.call.route(perm: Bool, +s: M.State, +now: Nat, +thread: String, +models: J.Json, +id: J.Json, +p: J.Json) -> McpOut:
  match perm:
    case True{}:
      Mcp.permit(s, now, thread, id, J.Json.get_obj(p, "arguments"))
    case False{}:
      Mcp.reply(id, Mcp.call.k(C.Codec.pos(Mcp.names(), J.Json.get_str(p, "name", "")), s, now, thread, models, J.Json.get_obj(p, "arguments")))

def Mcp.method.pos.k(note: Bool, +m: String) -> Nat:
  match note:
    case True{}:
      4n
    case False{}:
      C.Codec.pos(["initialize", "tools/list", "tools/call", "ping"], m)

def Mcp.method.pos(+m: String) -> Nat:
  Mcp.method.pos.k(String.starts_with(m, "notifications/"), m)

# Grok's permission requests
# --------------------------
# The Grok runner hands an ACP session/request_permission here (Grok has
# no permission MCP tool): a tool already allowed for the session in this
# thread is allowed again at once; otherwise it becomes an approval ask
# and the request waits, parked, for the user's answer (Park, then
# Acp.permit.answer).


# the ACP permission response is shared; providers supply the card choices
def Perm.acp.permission.k(ok: Bool, +ask: String, +now: Nat, +thread: String, +o: J.Json, +tool: String, +options: String) -> McpOut:
  match ok:
    case True{}:
      McpOut{AC.Acp.permit.reply(AC.Acp.id(J.Json.get(o, "id")), J.Json.get_obj(o, "params"), "accept"), B.Step.none(), ""}
    case False{}:
      McpOut{"", B.Step{[M.AskOpened{ask, thread, "approval", tool, AC.Acp.detail(J.Json.get_obj(o, "params")), options, now}], Nil{}}, ask}

def Grok.permit.k(ok: Bool, +ask: String, +now: Nat, +thread: String, +o: J.Json, +tool: String) -> McpOut:
  Perm.acp.permission.k(ok, ask, now, thread, o, tool, "")

def Grok.permit(+s: M.State, +now: Nat, +thread: String, +o: J.Json) -> McpOut:
  Grok.permit.k(Perm.session_ok(M.Asks.all(s), thread, AC.Acp.tool(J.Json.get_obj(o, "params"))), B.Hub.id("q", now, B.Hub.seq(s)), now, thread, o,
    AC.Acp.tool(J.Json.get_obj(o, "params")))

# OMP's permission requests remain one-shot, even if an older log contains a
# session answer. The card exposes no session-grant choice.
def Mcp.omp.permission(+s: M.State, +now: Nat, +thread: String, +o: J.Json) -> McpOut:
  Perm.acp.permission.k(False{}, B.Hub.id("q", now, B.Hub.seq(s)), now, thread, o,
    AC.Acp.tool(J.Json.get_obj(o, "params")), "Allow\nDeny")

def Mcp.permission.route.k(omp: Bool, +s: M.State, +now: Nat, +thread: String, +o: J.Json) -> McpOut:
  match omp:
    case True{}:
      Mcp.omp.permission(s, now, thread, o)
    case False{}:
      Grok.permit(s, now, thread, o)

def Mcp.permission.route(+s: M.State, +now: Nat, +thread: String, +o: J.Json) -> McpOut:
  Mcp.permission.route.k(String.eq(M.Thread.provider(M.State.thread.get(s, thread)), "omp"), s, now, thread, o)

# Muse's approvals
# ----------------
# The same for an MSP approval/request (the runner already sent Muse its
# receipt): the answer is an approval/decide (MS.Msp.decide).

def Muse.permit.k(ok: Bool, +ask: String, +now: Nat, +thread: String, +o: J.Json, +tool: String) -> McpOut:
  match ok:
    case True{}:
      McpOut{MS.Msp.decide(J.Json.show(o), "accept"), B.Step.none(), ""}
    case False{}:
      McpOut{"", B.Step{[M.AskOpened{ask, thread, "approval", tool, MS.Msp.detail(J.Json.get_obj(o, "params")), "", now}], Nil{}}, ask}

def Muse.permit(+s: M.State, +now: Nat, +thread: String, +o: J.Json) -> McpOut:
  Muse.permit.k(Perm.session_ok(M.Asks.all(s), thread, MS.Msp.tool(J.Json.get_obj(o, "params"))), B.Hub.id("q", now, B.Hub.seq(s)), now, thread, o,
    MS.Msp.tool(J.Json.get_obj(o, "params")))

# omp elicitation/create approval forms: accepted forms are shown as an
# approval ask; unsupported forms are declined immediately.
def Elicit.permit.k(approve: Bool, +ask: String, +now: Nat, +thread: String, +o: J.Json, +text: String) -> McpOut:
  match approve:
    case True{}:
      McpOut{"", B.Step{[M.AskOpened{ask, thread, "approval", AC.Acp.elicit.tool(text), text, "Allow\nDeny", now}], Nil{}}, ask}
    case False{}:
      McpOut{AC.Acp.elicit.reply(AC.Acp.id(J.Json.get(o, "id")), "decline"), B.Step.none(), ""}

def Elicit.permit(+s: M.State, +now: Nat, +thread: String, +o: J.Json) -> McpOut:
  Elicit.permit.k(AC.Acp.elicit.approve(J.Json.get_obj(o, "params")), B.Hub.id("q", now, B.Hub.seq(s)), now, thread, o,
    J.Json.get_str(J.Json.get_obj(o, "params"), "message", ""))



# Waiting (thread_wait)
# --------------------
# The hub never blocks: thread_wait answers where the thread stands at
# once (as thread_wait_now), and the server asks again each second until
# the thread is done or the wait's time is up.

def Wait.is(+o: J.Json) -> Bool:
  Bool.and(String.eq(J.Json.get_str(o, "method", ""), "tools/call"), String.eq(J.Json.get_str(J.Json.get_obj(o, "params"), "name", ""), "thread_wait"))

def Wait.clamp(+ms: Nat) -> Nat:
  Bool.pick(Nat, Nat.is_lt(ms, 1000n), 1n, Bool.pick(Nat, Nat.is_gt(ms, 600000n), 600n, Nat.div(ms, 1000n)))

# how many seconds a call waits (0: it is no thread_wait)
def Wait.secs(+o: J.Json) -> Nat:
  Bool.pick(Nat, Wait.is(o), Wait.clamp(Maybe.default(&2, Nat, Nat.read(J.Json.as_raw(J.Json.get(J.Json.get_obj(J.Json.get_obj(o, "params"), "arguments"), "timeoutMs"), "30000")), 30000n)), 0n)

# the same call as the check that answers at once
def Wait.now(+o: J.Json) -> String:
  J.Json.show(J.Json.obj([J.KV{"jsonrpc", J.Str{"2.0"}}, J.KV{"id", Maybe.default(&2, J.Json, J.Json.get(o, "id"), J.Null{})}, J.KV{"method", J.Str{"tools/call"}},
    J.KV{"params", J.Json.obj([J.KV{"name", J.Str{"thread_wait_now"}}, J.KV{"arguments", J.Json.get_obj(J.Json.get_obj(o, "params"), "arguments")}])}]))

def Wait.first(xs: List<&2, J.Json>) -> J.Json:
  match xs:
    case x <> _:
      x
    case Nil{}:
      J.Null{}

# whether a check's reply says the thread is done (a failed check ends
# the wait too)
def Wait.done(+reply: String) -> Bool:
  +r = Maybe.default(&2, J.Json, J.Json.parse(reply), J.Null{})
  +inner = Maybe.default(&2, J.Json, J.Json.parse(J.Json.get_str(Wait.first(J.Json.elems(J.Json.get_obj(J.Json.get_obj(r, "result"), "content"))), "text", "")), J.Null{})
  Bool.or(J.Json.get_bool(J.Json.get_obj(r, "result"), "isError", True{}), J.Json.get_bool(inner, "done", True{}))

def Res.changes(r: Res) -> List<&2, M.Change>:
  match r:
    case Res{_, _, B.Step{cs, _}}:
      cs

def Res.good(r: Res) -> Bool:
  match r:
    case Res{ok, _, _}:
      ok

# ---- mail 2c ----
def Mcp.run.tools() -> List<&2, J.Json>:
  [Tool.def("decide", "Record a decision and its reason in this run's conversation.", [Tool.prop("text", "string", "The decision."), Tool.prop("reason", "string", "Why.")], ["text", "reason"]),
   Tool.def("escalate", "Ask the front of house bot or person about something outside the charter.", [Tool.prop("to", "string", "bot or person."), Tool.prop("text", "string", "What needs deciding.")], ["to", "text"]),
   Tool.def("charter_set", "Front of house only: replace your mailroom charter.", [Tool.prop("text", "string", "The full charter."), Tool.prop("reason", "string", "Why it changed.")], ["text", "reason"])]

# Keep the old advertised tool payload exactly while mail is off.
def Mcp.tools.on(on: Bool) -> J.Json:
  match on:
    case False{}: Mcp.tools()
    case True{}: J.Json.obj([J.KV{"tools", J.Json.arr(List.append(&2, J.Json, J.Json.elems(J.Json.get_obj(Mcp.tools(), "tools")), List.append(&2, J.Json, Mcp.mail.tools(), Mcp.run.tools())) )}])

def Mcp.method.k(i: Nat, +s: M.State, +now: Nat, +thread: String, +models: J.Json, +version: String, +id: J.Json, +p: J.Json) -> McpOut:
  match i:
    case 0n:
      McpOut{Mcp.result(id, J.Json.obj([J.KV{"protocolVersion", J.Str{Mcp.or(J.Json.get_str(p, "protocolVersion", ""), "2025-06-18")}},
        J.KV{"capabilities", J.Json.obj([J.KV{"tools", J.Json.obj(Nil{})}])},
        J.KV{"serverInfo", J.Json.obj([J.KV{"name", J.Str{"backplane"}}, J.KV{"version", J.Str{version}}])}])), B.Step.none(), ""}
    case 1n:
      McpOut{Mcp.result(id, Mcp.tools.on(String.eq(M.Setting.get(s, "mail.on", "off"), "on"))), B.Step.none(), ""}
    case 2n:
      Mcp.call.route(String.eq(J.Json.get_str(p, "name", ""), "permission_prompt"), s, now, thread, models, id, p)
    case 3n:
      McpOut{Mcp.result(id, J.Json.obj(Nil{})), B.Step.none(), ""}
    case 4n:
      McpOut{"", B.Step.none(), ""}
    case _:
      McpOut{Mcp.error(id, "-32601", "method not found"), B.Step.none(), ""}

def Mcp.handle.k(i: Nat, +s: M.State, +now: Nat, +thread: String, +models: J.Json, +version: String, +o: J.Json) -> McpOut:
  match i:
    case 0n:
      Mcp.permission.route(s, now, thread, o)
    case 1n:
      Muse.permit(s, now, thread, o)
    case 2n:
      Elicit.permit(s, now, thread, o)
    case _:
      Mcp.method.k(Mcp.method.pos(J.Json.get_str(o, "method", "")), s, now, thread, models, version, J.Json.get_obj(o, "id"), J.Json.get_obj(o, "params"))

# one JSON-RPC message from an agent of `thread` ("" reply: a notification)
def Mcp.handle(+s: M.State, +now: Nat, +thread: String, +models: J.Json, +version: String, +o: J.Json) -> McpOut:
  Mcp.handle.k(C.Codec.pos(["session/request_permission", "approval/request", "elicitation/create"], J.Json.get_str(o, "method", "")), s, now, thread, models, version, o)

def Mcp.text.k(m: Maybe<&2, J.Json>, +s: M.State, +now: Nat, +thread: String, +models: J.Json, +version: String) -> McpOut:
  match m:
    case Some{+o}:
      Mcp.handle(s, now, thread, models, version, o)
    case None{}:
      McpOut{Mcp.error(J.Null{}, "-32700", "parse error"), B.Step.none(), ""}

def Mcp.text(+s: M.State, +now: Nat, +thread: String, +models: J.Json, +version: String, body: String) -> McpOut:
  Mcp.text.k(J.Json.parse(body), s, now, thread, models, version)

# Direct hub tools have passed the charter guard. Reuse that grant when
# Google's prepared operation returns, rather than opening a second ask.
def Perm.run.google.got(r: Maybe<&2, ML.Run>, +s: M.State, +tool: String) -> Bool:
  match r:
    case None{}: False{}
    case Some{+r}: CH.Charter.gate(ML.Box.charter(M.State.mail(s), ML.Run.bot(r)), tool, J.Null{})

def Perm.run.google(+s: M.State, +thread: String, +tool: String) -> Bool:
  Perm.run.google.got(RUN.Run.of(s, thread), s, tool)
