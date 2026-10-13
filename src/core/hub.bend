import Base
import ./json.bend as J
import ./model.bend as M
import ./review.bend as RVW
import ./codec.bend as C
import ./attach.bend as AT
import ./agent.bend as AG
import ./handoff.bend as HD
import ./todo.bend as TD
import ./fm.bend as FM
import ./refs.bend as RF
import ./emb.bend as EM
import ./far.bend as FR
import ./ubersync.bend as UBS

# The decider. Requests from clients and lines from agents come in; changes
# (persisted, folded into the read model) and effects (run by the server)
# come out. Nothing here does IO, so the server's behaviour is these defs.

# what the server must do after a step
type Out is Data:
  # to one client
  Send{client: U32, text: String}
  # to every client
  Broadcast{text: String}
  # ensure an agent process runs for a thread, then write a line to it
  AgentStart{thread: String, cwd: String, provider: String, model: String, session: String, runtime: M.RuntimeMode, interaction: M.InteractionMode}
  AgentSend{thread: String, line: String}
  AgentStop{thread: String}
  # show a client a project's board or schematic (kind "board",
  # "schematic"; anything else stops), or the file at path when set
  Watch{client: U32, kind: String, root: String, path: String}
  # install the latest release and restart
  Upgrade{}
  # the same, asked on purpose (update.now) or after the wait ran out: its
  # restart does not wait for work
  UpgradeForce{}
  # the install is ready: end the hub's process into it now
  Restart{}
  KiCadInstall{}
  # run one Codex turn (a process per turn; `session` resumes)
  CodexTurn{thread: String, cwd: String, model: String, session: String, runtime: M.RuntimeMode, text: String}
  GrokTurn{thread: String, cwd: String, model: String, session: String, runtime: M.RuntimeMode, text: String}
  MuseTurn{thread: String, cwd: String, model: String, session: String, runtime: M.RuntimeMode, text: String}
  # Google's Antigravity agent over ACP (core/agy.bend)
  AgyTurn{thread: String, cwd: String, model: String, session: String, runtime: M.RuntimeMode, text: String}
  # a turn ended: "completed", "failed", or "stopped" (Stop, archive,
  # delete: no alert, but the phones' island still hears of it)
  TurnEnded{thread: String, state: String}
  # the phones' island (Live Activity) is pushed again, no alert: a turn
  # started or changed, a thread went (Island.outs)
  IslandSync{}
  AskAnswered{id: String, answer: String}
  Git{client: U32, id: String, job: String, thread: String, cwd: String, a: String, b: String}
  TermOp{op: String, thread: String, cwd: String, data: List<&2, U32>, cols: U32, rows: U32}
  # a terminal in the chat (core/emb.bend): its command run (again: a
  # restart), a client's snapshot of it, hung up
  TermRun{key: String, title: String, cwd: String, cmd: String, cols: U32, rows: U32, again: Bool}
  TermSnap{client: U32, key: String}
  TermKill{key: String}
  Fs{client: U32, id: String, op: String, path: String}
  # what a piece a phone tapped is: the info of piece idx of the chunk at
  # `at` of the plot key it holds (the plot actor answers)
  PlotInfo{client: U32, id: String, key: String, at: Nat, idx: Nat}
  # a bot effect the server runs with IO or secrets (bothub.bend): kind
  # home, gone, hook, unhook, send, invite, join, unpeer, google
  BotFx{kind: String, a: String, b: String, c: String}
  # a desk client (the window, the web page) shows thread, with or
  # without the focus (core/desk.bend; the server keeps it per client)
  DeskLook{client: U32, thread: String, focused: Bool}
  # a bot's answer to a person on a linked machine (`to`), captured when
  # the turn that finished the thread's work ended (Answer.out); done names
  # that completion, so the server sends it once
  BotReply{thread: String, to: String, text: String, done: String}
  # watch work for a thread (Watch.*): kind run (launch target, a shell
  # command, in cwd, unless it already ran), unit, pid or file; ident
  # what proves it is the same work ("-" not known yet); deadline 0 for
  # none
  Await{thread: String, id: String, kind: String, target: String, ident: String, deadline: Nat, cwd: String}
  # stop owned work (work_cancel): its watcher reports the end
  WatchStop{thread: String, id: String, kind: String, target: String, ident: String}
  # a thread's streamed text: kept by the server (core/live.bend) until its
  # message is posted or its turn ends, and sent as a delta
  Live{thread: String, text: String}
  # the outbox of reports to launchers on other machines (the setting
  # launch.far, Lr.far.*) is due to be sent, in order
  LaunchFar{}
  # a request about a bot's browser and what it is taught (core/teach.bend):
  # the server keeps the driver and the draft, so it answers (m is
  # bots.browser or bots.teach, p the request's params as JSON text)
  Teach{client: U32, id: String, m: String, p: String}

type Step is Data:
  Step{changes: List<&2, M.Change>, outs: List<&2, Out>}

def Step.none() -> Step:
  Step{Nil{}, Nil{}}

def Step.outs(os: List<&2, Out>) -> Step:
  Step{Nil{}, os}

# The phones' island
# ------------------
# A phone in the background hears of its island only by push. Every step
# that moves a turn (a start, an end, a stop, recovery at start) or takes a
# thread away (archive, delete) ends with an IslandSync, unless it already
# tells the server a turn ended (TurnEnded pushes the island with its
# alert), so the Live Activity never shows a thread working that is not
# (laws island_sync_*).

def Island.touch(cs: List<&2, M.Change>) -> Bool:
  match cs:
    case Nil{}:
      False{}
    case M.TurnChanged{_, _, _} <> _:
      True{}
    case M.ThreadArchived{_, _, _} <> _:
      True{}
    case M.ThreadDeleted{_, _} <> _:
      True{}
    case _ <> t:
      Island.touch(t)

def Island.told(os: List<&2, Out>) -> Bool:
  match os:
    case Nil{}:
      False{}
    case TurnEnded{_, _} <> _:
      True{}
    case _ <> t:
      Island.told(t)

def Island.outs.k(due: Bool, os: List<&2, Out>) -> List<&2, Out>:
  match due:
    case True{}:
      List.append(&2, Out, os, [IslandSync{}])
    case False{}:
      os

# a step's outputs, with the island pushed after them when the step moved it
def Island.outs(+cs: List<&2, M.Change>, +os: List<&2, Out>) -> List<&2, Out>:
  Island.outs.k(Bool.and(Island.touch(cs), Bool.not(Island.told(os))), os)

# Replies
# -------

def Reply.ok(+client: U32, +id: String) -> Out:
  Send{client, J.Json.show(J.Json.obj([J.KV{"t", J.Str{"reply"}}, J.KV{"id", J.Num{id}}, J.KV{"ok", J.Flag{True{}}}]))}

# the answer to a request that made a thread: the thread, so the client
# that asked (and only it) selects it (client.bend's Made.reply)
def Reply.made(+client: U32, +id: String, +thread: String) -> Out:
  Send{client, J.Json.show(J.Json.obj([J.KV{"t", J.Str{"reply"}}, J.KV{"id", J.Num{id}}, J.KV{"ok", J.Flag{True{}}}, J.KV{"thread", J.Str{thread}}]))}

def Reply.err(+client: U32, +id: String, msg: String) -> Out:
  Send{client, J.Json.show(J.Json.obj([J.KV{"t", J.Str{"reply"}}, J.KV{"id", J.Num{id}}, J.KV{"ok", J.Flag{False{}}}, J.KV{"error", J.Str{msg}}]))}

def Step.reply(+client: U32, +id: String, cs: List<&2, M.Change>) -> Step:
  Step{cs, [Reply.ok(client, id)]}

# Ids
# ---

# unique and replay-stable: the event sequence never repeats
def Hub.id(prefix: String, +now: Nat, +seq: Nat) -> String:
  prefix ++ Nat.show(now) ++ "-" ++ Nat.show(seq)

def Hub.seq(s: M.State) -> Nat:
  match s:
    case M.State{_, _, _, n, _, _, _, _, _, _, _}:
      n

def Hub.thread.k(r: Map<&2, M.Thread> & M.Thread) -> M.Thread:
  (_, t) = r
  t

# Lookups
# -------

def Hub.thread(s: M.State, +id: String) -> M.Thread:
  match s:
    case M.State{_, ts, _, _, _, _, _, _, _, _, _}:
      Hub.thread.k(Map.get(M.Thread, M.Thread.blank(id), ts, id))

# whether the log knows a thread (a scope left by an earlier run is this
# hub's to stop only if its thread is)
def Hub.owns.k(r: Map<&2, M.Thread> & Bool) -> Bool:
  (_, b) = r
  b

def Hub.owns(s: M.State, +id: String) -> Bool:
  match s:
    case M.State{_, ts, _, _, _, _, _, _, _, _, _}:
      Hub.owns.k(Map.has(&2, M.Thread, ts, id))

def Hub.project.k(r: Map<&2, M.Project> & M.Project) -> M.Project:
  (_, p) = r
  p

def Hub.project(s: M.State, +id: String) -> M.Project:
  match s:
    case M.State{ps, _, _, _, _, _, _, _, _, _, _}:
      Hub.project.k(Map.get(M.Project, M.Project.blank(id), ps, id))

# the base name of a path, for a default project title
def Hub.base.k(parts: List<&2, String>) -> String:
  Maybe.default(&2, String, List.last(&2, String, List.filter(~String, ~(x => Bool.not(String.is_empty(x))), parts)), "project")

def Hub.base(p: String) -> String:
  Hub.base.k(String.split(p, '/'))

def Hub.project.root(p: M.Project) -> String:
  match p:
    case M.Project{_, _, root, _, _}:
      root

# the directory a thread's agent works in
def Hub.cwd(+s: M.State, t: M.Thread) -> String:
  match t:
    case M.Thread{_, +project, _, _, _, _, _, _, _, _, _, _, _, _, +worktree, _, _, _, _, _, _}:
      Bool.pick(String, String.is_empty(worktree), Hub.project.root(Hub.project(s, project)), worktree)

# Claude's stream-json input line for one user message
def Claude.user(text: String) -> String:
  J.Json.show(J.Json.obj([
    J.KV{"type", J.Str{"user"}},
    J.KV{"message", J.Json.obj([
      J.KV{"role", J.Str{"user"}},
      J.KV{"content", J.Json.arr([J.Json.obj([J.KV{"type", J.Str{"text"}}, J.KV{"text", J.Str{text}}])])}])}]))

# the same with the command's uuid, by which Claude reports its lifecycle
# (core/cmds.bend): minted when the hub takes the message, kept by every
# rewrite of the line (Claude.uuid.of)
def Claude.tag(+line: String, +uuid: String) -> String:
  Bool.pick(String, String.is_empty(uuid), line, "{\"uuid\":" ++ J.Json.show(J.Str{uuid}) ++ "," ++ String.drop(line, 1n))

def Claude.user.u(text: String, +uuid: String) -> String:
  Claude.tag(Claude.user(text), uuid)

def Claude.uuid.of(+line: String) -> String:
  J.Json.get_str(Maybe.default(&2, J.Json, J.Json.parse(line), J.Null{}), "uuid", "")

# hex digits of n, the last w of them
def Hub.hex(w: Nat, +n: Nat) -> String:
  match w:
    case 0n:
      ""
    case 1n+k:
      Hub.hex(k, Nat.div(n, 16n)) ++ SCon{J.Json.hex(U32.from_nat(Nat.mod(n, 16n))), SNil{}}

def Hub.hash(s: String, +m: Nat, +acc: Nat) -> Nat:
  match s:
    case SNil{}:
      acc
    case SCon{Chr{+c}, t}:
      Hub.hash(t, m, Nat.mod(Nat.add(Nat.mul(acc, m), U32.to_nat(c)), Nat.mul(65536n, 65536n)))

def Hub.uuid.k(+now: Nat, +a: Nat, +b: Nat) -> String:
  Hub.hex(8n, now) ++ "-" ++ Hub.hex(4n, a) ++ "-4" ++ Hub.hex(3n, Nat.div(a, 65536n)) ++ "-8" ++ Hub.hex(3n, b) ++ "-" ++ Hub.hex(4n, Nat.div(b, 4096n)) ++ Hub.hex(8n, Nat.add(a, b))

# a command's uuid (v4-shaped, as Claude wants): the time and a hash of
# what makes it unique (the thread and the message or sequence)
def Hub.uuid(+now: Nat, +key: String) -> String:
  Hub.uuid.k(now, Hub.hash(key, 31n, 7n), Hub.hash(key, 131n, 3n))

def Claude.interrupt(+id: String) -> String:
  J.Json.show(J.Json.obj([
    J.KV{"type", J.Str{"control_request"}},
    J.KV{"request_id", J.Str{id}},
    J.KV{"request", J.Json.obj([J.KV{"subtype", J.Str{"interrupt"}}])}]))

# a user message for Claude: straight in, or, when it attaches images, by
# the server, which reads them into image content blocks first
def Claude.send(+thread: String, +cwd: String, +text: String, +uuid: String) -> Out:
  Bool.pick(Out, List.is_empty(&2, String, AT.Att.images(text)), AgentSend{thread, Claude.user.u(text, uuid)},
    Git{0, "", "attach.claude", thread, cwd, text, uuid})

def Hub.title.cut(long: Bool, +line: String) -> String:
  match long:
    case True{}:
      String.take(line, 57n) ++ "..."
    case False{}:
      line

def Hub.title.of(+text: String) -> String:
  +line = String.trim(Maybe.default(&2, String, List.head(&2, String, String.lines(text)), ""))
  Hub.title.cut(Nat.is_gt(String.length(line), 60n), line)

# Requests
# --------

# a thread still called "New thread" takes its first message as its title
def Hub.title.k(fresh: Bool, +thread: String, +text: String, +now: Nat, rest: List<&2, M.Change>) -> List<&2, M.Change>:
  match fresh:
    case True{}:
      M.ThreadRenamed{thread, Hub.title.of(text), now} <> rest
    case False{}:
      rest

# the effects that run one turn of a thread's agent, by provider: Claude
# keeps one process per thread fed with stream-json; Codex, Grok, Muse and
# Antigravity run a process per turn, resuming their session (Grok and
# Antigravity over ACP, Muse over MSP, all reported in Codex's event shapes)
def Hub.run.k(i: Nat, +thread: String, +cwd: String, +provider: String, +model: String, +session: String, runtime: M.RuntimeMode, interaction: M.InteractionMode, +text: String, +uuid: String) -> List<&2, Out>:
  match i:
    case 1n:
      [CodexTurn{thread, cwd, model, session, runtime, text}]
    case 2n:
      [GrokTurn{thread, cwd, model, session, runtime, text}]
    case 3n:
      [MuseTurn{thread, cwd, model, session, runtime, text}]
    case 4n:
      [AgyTurn{thread, cwd, model, session, runtime, text}]
    case _:
      [AgentStart{thread, cwd, provider, model, session, runtime, interaction}, Claude.send(thread, cwd, text, uuid)]

# (uuid: the Claude command's, Hub.uuid)
def Hub.run(t: M.Thread, +cwd: String, +text: String, +uuid: String) -> List<&2, Out>:
  match t:
    case M.Thread{+id, _, _, _, _, _, _, _, _, _, _, _, _, _, _, +provider, model, runtime, interaction, session, _}:
      Hub.run.k(C.Codec.pos(["claude", "codex", "grok", "muse", "antigravity"], provider), id, cwd, provider, model, session, runtime, interaction, text, uuid)

def Hub.post.go(wait: Bool, +t: M.Thread, +cwd: String, +queued: String, +now: Nat, +thread: String, +msg: String, role: M.Role, +text: String) -> Step:
  match wait:
    case True{}:
      Step{[M.MessagePosted{thread, msg, role, text, now}, M.TurnQueued{thread, msg, text, now}], Nil{}}
    case False{}:
      Step{List.concat(&2, M.Change, [[M.MessagePosted{thread, msg, role, text, now}],
          Bool.pick(List<&2, M.Change>, String.is_empty(queued), Nil{}, [M.QueueTaken{thread, now}]),
          [M.TurnChanged{thread, M.Running{}, now}]]),
        Git{0, "", "checkpoint0", thread, cwd, "", ""} <> Git{0, "", "step", thread, cwd, msg, ""} <> Hub.run(t, cwd, M.Queue.join(queued, text), Hub.uuid(now, thread ++ "/" ++ msg))}

def Step.after(st: Step, more: List<&2, M.Change>) -> Step:
  match st:
    case Step{cs, os}:
      Step{List.append(&2, M.Change, cs, more), os}

def Step.before(cs: List<&2, M.Change>, os: List<&2, Out>, st: Step) -> Step:
  match st:
    case Step{c2, o2}:
      Step{List.append(&2, M.Change, cs, c2), List.append(&2, Out, os, o2)}

# whether a message with this id is in the entries: a tail walk that stops
# at the first hit
def Hub.has_msg.go(es: List<&2, M.Entry>, +id: String, found: Bool) -> Bool:
  match es found:
    case _ True{}:
      True{}
    case Nil{} False{}:
      False{}
    case M.Msg{i, _, _, _, _} <> t False{}:
      Hub.has_msg.go(t, id, String.eq(i, id))
    case M.Act{_, _, _, _, _, _} <> t False{}:
      Hub.has_msg.go(t, id, False{})

def Hub.has_msg(es: List<&2, M.Entry>, +id: String) -> Bool:
  Hub.has_msg.go(es, id, False{})

def Hub.entries(s: M.State) -> List<&2, M.Entry>:
  match s:
    case M.State{_, _, es, _, _, _, _, _, _, _, _}:
      es

# a message the client named and the hub already stored: only a named one
# looks anything up
def Hub.dup(named: Bool, +s: M.State, +msg: String) -> Bool:
  match named:
    case True{}:
      Hub.has_msg(Hub.entries(s), msg)
    case False{}:
      False{}

# a thread works in its own worktree unless asked for the project folder
# ("local"): "auto" (the default) and "worktree" both ask the server for
# one, which falls back to the folder outside a repository
def Hub.env.of(+mode: String) -> M.EnvMode:
  Bool.pick(M.EnvMode, String.eq(mode, "local"), M.Local{}, M.Worktree{})

def Hub.env.outs(e: M.EnvMode, +th: String, +root: String, +mode: String) -> List<&2, Out>:
  match e:
    case M.Worktree{}:
      [Git{0, "", "worktree", th, root, mode, ""}]
    case M.Local{}:
      Nil{}

# a scratch thread (M.Scratch) works in a folder of its own that the
# server makes (<home>/scratch/<thread>, a git repository like a bot's
# home); any other thread as its project and mode say
def Hub.new.env(scratch: Bool, +mode: String) -> M.EnvMode:
  match scratch:
    case True{}:
      M.Local{}
    case False{}:
      Hub.env.of(mode)

# a scratch thread's folder, known from the start so its first turn runs
# there (the server makes it before anything else runs)
def Hub.new.wt(scratch: Bool, +root: String, +th: String) -> String:
  match scratch:
    case True{}:
      root ++ "/" ++ th
    case False{}:
      ""

def Hub.new.outs(scratch: Bool, +th: String, +root: String, +mode: String) -> List<&2, Out>:
  match scratch:
    case True{}:
      [BotFx{"scratch", th, "", ""}]
    case False{}:
      Hub.env.outs(Hub.env.of(mode), th, root, mode)

def Hub.thread_create(+s: M.State, +now: Nat, +client: U32, +id: String, +p: J.Json) -> Step:
  +project = J.Json.get_str(p, "project", "")
  +title = J.Json.get_str(p, "title", "New thread")
  +env = J.Json.get_str(p, "env", M.Setting.get(s, "thread.env", "auto"))
  +th = Hub.id("t", now, Hub.seq(s))
  Step{[M.ThreadCreated{th, project, title,
    Hub.new.env(M.Scratch.is(project), env), "", Hub.new.wt(M.Scratch.is(project), Hub.project.root(Hub.project(s, project)), th), J.Json.get_str(p, "provider", M.Setting.get(s, "agent.provider", "claude")),
    J.Json.get_str(p, "model", M.Setting.get(s, "agent.model", "")), now}],
    Reply.made(client, id, th) <> Hub.new.outs(M.Scratch.is(project), th, Hub.project.root(Hub.project(s, project)), env)}

# the project's first thread, made as thread.create would, after it
def Hub.project_add.thread(+s: M.State, +now: Nat, +client: U32, +id: String, +pid: String, +made: M.Change) -> Step:
  Step.before([made], Nil{}, Hub.thread_create(s, now, client, id, J.Json.obj([J.KV{"project", J.Str{pid}}, J.KV{"title", J.Str{"New thread"}}])))

def Hub.root.norm.fin(+r: String) -> String:
  Bool.pick(String, String.is_empty(r), "/", r)

def Hub.root.slashes.k(slash: Bool, rest: Nat) -> Nat:
  match slash:
    case True{}:
      1n+rest
    case False{}:
      0n

def Hub.root.slashes(s: String) -> Nat:
  match s:
    case SNil{}:
      0n
    case SCon{Chr{+c}, t}:
      Hub.root.slashes.k(U32.is_eq(c, 47), Hub.root.slashes(t))

def Hub.root.norm.go(+rev: String) -> String:
  Hub.root.norm.fin(String.reverse(String.drop(rev, Hub.root.slashes(rev))))

# a root without trailing slashes ("/" stays)
def Hub.root.norm(+r: String) -> String:
  Bool.pick(String, Bool.and(String.ends_with(r, "/"), Nat.is_gt(String.length(r), 1n)),
    Hub.root.norm.go(String.reverse(r)), r)

# whether a live project has this root (trailing slashes aside)
def Hub.has_root(ps: List<&2, M.Project>, +root: String) -> Bool:
  match ps:
    case Nil{}:
      False{}
    case M.Project{_, _, r, _, rm} <> t:
      Bool.or(Bool.and(String.eq(Hub.root.norm(r), Hub.root.norm(root)), Bool.not(rm)), Hub.has_root(t, root))

# the id of a removed project with this root ("" when none): adding its
# folder again brings it back, threads and all
def Hub.removed_id(ps: List<&2, M.Project>, +root: String) -> String:
  match ps:
    case Nil{}:
      ""
    case M.Project{+i, _, r, _, rm} <> t:
      Bool.pick(String, Bool.and(rm, String.eq(Hub.root.norm(r), Hub.root.norm(root))), i, Hub.removed_id(t, root))

def Hub.projects(s: M.State) -> List<&2, M.Project>:
  match s:
    case M.State{ps, _, _, _, _, _, _, _, _, _, _}:
      Map.values(&2, M.Project, ps)

# a new project's id, or the removed one's it brings back
def Hub.project_id(+old: String, +s: M.State, +now: Nat) -> String:
  Bool.pick(String, String.is_empty(old), Hub.id("p", now, Hub.seq(s)), old)

# a project made afresh starts with a thread (the client that asked selects
# it); one brought back has its own
def Hub.project_add.new(old: Bool, +s: M.State, +now: Nat, +client: U32, +id: String, +pid: String, +made: M.Change) -> Step:
  match old:
    case True{}:
      Step.reply(client, id, [made])
    case False{}:
      Hub.project_add.thread(M.State.apply(s, made), now, client, id, pid, made)

def Hub.project_add.k(dup: Bool, +s: M.State, +now: Nat, +client: U32, +id: String, +p: J.Json, +root: String) -> Step:
  match dup:
    case True{}:
      Step.outs([Reply.err(client, id, "already a project: " ++ root)])
    case False{}:
      Hub.project_add.new(Bool.not(String.is_empty(Hub.removed_id(Hub.projects(s), root))), s, now, client, id, Hub.project_id(Hub.removed_id(Hub.projects(s), root), s, now),
        M.ProjectCreated{Hub.project_id(Hub.removed_id(Hub.projects(s), root), s, now), J.Json.get_str(p, "title", Hub.base(root)), Hub.root.norm(root), now})

# a folder becomes a project once (the same root twice is refused; a
# removed project's folder brings that project back). The
# server resolves project.add's path first (~, relative, symlinks; it must
# be a folder it can open) and sends it back here as project.create
def Hub.project_add.p(+s: M.State, +now: Nat, +client: U32, +id: String, +p: J.Json, +root: String) -> Step:
  Bool.pick(Step, String.is_empty(String.trim(root)), Step.outs([Reply.err(client, id, "no folder given")]),
    Hub.project_add.k(Hub.has_root(Hub.projects(s), root), s, now, client, id, p, root))

def Hub.project_add(+s: M.State, +now: Nat, +client: U32, +id: String, +p: J.Json) -> Step:
  Hub.project_add.p(s, now, client, id, p, J.Json.get_str(p, "path", ""))

# change a thread's modes; what the request leaves out stays as it is
def Hub.modes(+s: M.State, +now: Nat, +client: U32, +id: String, +p: J.Json, t: M.Thread) -> Step:
  match t:
    case M.Thread{+th, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, mo, rt, im, _, _}:
      Step.before([M.ModesSet{th, C.Runtime.of(J.Json.get_str(p, "runtime", C.Runtime.name(rt))),
        C.Interaction.of(J.Json.get_str(p, "interaction", C.Interaction.name(im))), J.Json.get_str(p, "model", mo), now}], [Reply.ok(client, id)], Step.none())

# Answering asks
# --------------

def Ask.find(as: List<&2, M.Ask>, +id: String) -> M.Ask:
  match as:
    case Nil{}:
      M.Ask.blank(id)
    case M.Ask{+i, th, k, t, d, o, op, an} <> rest:
      Bool.pick(M.Ask, String.eq(i, id), M.Ask{i, th, k, t, d, o, op, an}, Ask.find(rest, id))

def Hub.implement.k(t: M.Thread, +now: Nat) -> List<&2, M.Change>:
  match t:
    case M.Thread{+th, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, mo, rt, _, _, _}:
      [M.ModesSet{th, rt, M.DefaultMode{}, mo, now}]

# the state the implementing turn starts from: the plan is out of the way
def Hub.impl.state(+s: M.State, +th: String, +now: Nat) -> M.State:
  M.State.replay(Hub.implement.k(Hub.thread(s, th), now), s)

# Git: worktrees, checkpoints, diffs, stacked actions
# ---------------------------------------------------
# The hub decides; the server runs git (src/server/git.bend) and reports
# back: thread.worktree and thread.note come from the server itself.

# a commit made from a thread says what the thread was about
def Hub.thread_title(t: M.Thread) -> String:
  match t:
    case M.Thread{_, _, ti, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _}:
      ti

def Rename.changes(+th: String, +title: String, +now: Nat) -> List<&2, M.Change>:
  Bool.pick(List<&2, M.Change>, Bool.or(String.is_empty(title), String.is_empty(th)), Nil{}, [M.ThreadRenamed{th, title, now}])

def Hub.methods.git() -> List<&2, String>:
  ["git.diff", "git.revert", "git.act", "git.status", "thread.worktree", "thread.note", "term.open", "term.input", "term.resize", "term.snap", "term.restart"]

def Hub.git(+s: M.State, +client: U32, +id: String, +job: String, +thread: String, +a: String, +b: String) -> Step:
  Step.outs([Git{client, id, job, thread, Hub.cwd(s, Hub.thread(s, thread)), a, b}])

def Term.bytes(xs: List<&2, J.Json>) -> List<&2, U32>:
  match xs:
    case Nil{}:
      Nil{}
    case +x <> t:
      U32.from_nat(Maybe.default(&2, Nat, Nat.read(J.Json.as_raw(Some{x}, "0")), 0n)) <> Term.bytes(t)

# a thread's terminal: open (in the thread's directory), keys in, resize
def Hub.term(+s: M.State, +client: U32, +id: String, +op: String, +p: J.Json) -> Step:
  Step.outs([Reply.ok(client, id), TermOp{op, J.Json.get_str(p, "thread", ""), Hub.cwd(s, Hub.thread(s, J.Json.get_str(p, "thread", ""))),
    Term.bytes(J.Json.elems(J.Json.get_obj(p, "b"))), J.Json.get_u32(p, "cols", 80), J.Json.get_u32(p, "rows", 24)}])

# a terminal in the chat starts again: live from now (core/emb.bend)
def Emb.restart.k(+sum: String, +s: M.State, +now: Nat, +client: U32, +id: String, +thread: String, +term: String) -> Step:
  Bool.pick(Step, String.is_empty(sum), Step.outs([Reply.err(client, id, "no such terminal")]),
    Step{[M.SettingSet{EM.Emb.at.key(EM.Emb.key(thread, term)), Nat.show(now), now}],
      [Reply.ok(client, id), TermRun{EM.Emb.key(thread, term), EM.Emb.title(sum), Hub.cwd(s, Hub.thread(s, thread)), EM.Emb.cmd(sum), EM.Emb.wide(sum), EM.Emb.tall(sum), True{}}]})

def Emb.restart(+s: M.State, +now: Nat, +client: U32, +id: String, +thread: String, +term: String) -> Step:
  Emb.restart.k(EM.Emb.sum(M.State.mine(s, thread), term), s, now, client, id, thread, term)

# Phones
# ------

# a push token or bundle id: letters, digits and . _ : - only (APNs tokens
# are hex, FCM tokens base64url with a colon), so neither can bend a URL
# or a header
def Device.char(+c: U32) -> Bool:
  Bool.or(Bool.or(Bool.and(U32.is_ge(c, 48), U32.is_le(c, 57)), Bool.and(U32.is_ge(c, 65), U32.is_le(c, 90))),
    Bool.or(Bool.and(U32.is_ge(c, 97), U32.is_le(c, 122)), Bool.or(Bool.or(U32.is_eq(c, 46), U32.is_eq(c, 95)), Bool.or(U32.is_eq(c, 58), U32.is_eq(c, 45)))))

def Device.chars(s: String) -> Bool:
  match s:
    case SNil{}:
      True{}
    case SCon{Chr{+c}, t}:
      Bool.and(Device.char(c), Device.chars(t))

def Device.safe(+s: String) -> Bool:
  Bool.and(Bool.not(String.is_empty(s)), Bool.and(Nat.is_le(String.length(s), 512n), Device.chars(s)))

def Device.one_of(+names: List<&2, String>, +s: String) -> Bool:
  Nat.is_lt(C.Codec.pos(names, s), List.length(&2, String, names))

def Device.ok(+p: J.Json) -> Bool:
  Bool.and(Bool.and(Device.safe(J.Json.get_str(p, "token", "")), Device.safe(J.Json.get_str(p, "bundle", ""))),
    Bool.and(Bool.and(Device.one_of(["ios", "android"], J.Json.get_str(p, "platform", "")), Device.one_of(["alert", "activity", "start"], J.Json.get_str(p, "kind", ""))),
      Device.one_of(["sandbox", "production"], J.Json.get_str(p, "env", "production"))))

def Device.register.k(ok: Bool, +now: Nat, +client: U32, +id: String, +p: J.Json) -> Step:
  match ok:
    case True{}:
      Step.reply(client, id, [M.DeviceRegistered{J.Json.get_str(p, "token", ""), J.Json.get_str(p, "platform", ""), J.Json.get_str(p, "kind", ""),
        J.Json.get_str(p, "thread", ""), J.Json.get_str(p, "env", "production"), J.Json.get_str(p, "bundle", ""), now}])
    case False{}:
      Step.outs([Reply.err(client, id, "bad device")])

# a phone registers a push token (never echoed: clients get the event
# without it)
def Device.register(+now: Nat, +client: U32, +id: String, +p: J.Json) -> Step:
  Device.register.k(Device.ok(p), now, client, id, p)

# Method groups (each appended to only by its own feature area)
# ------------------------------------------------------------

# folders for the project picker (listing, creating, creating as a
# project, finding projects on disk); the server answers. project.create
# is the server's own, with a path it has resolved
def Hub.methods.fs() -> List<&2, String>:
  ["fs.list", "fs.mkdir", "project.new", "project.create", "fs.find"]

# sidebar and navigation
def Hub.methods.nav() -> List<&2, String>:
  ["thread.snooze", "thread.reorder", "files.list", "kicad.open", "project.reorder", "mech.list", "mech.render", "file.open", "files.dir", "pdf.page"]

# snooze a thread: {thread, for: "1h" | "1w" | "tomorrow" | "off"} or
# {thread, until}. Tomorrow 9:00 is local time, which the server knows:
# it answers with the time (thread.snooze {thread, until})
def Hub.snooze.k(i: Nat, +now: Nat, +client: U32, +id: String, +th: String, +p: J.Json) -> Step:
  match i:
    case 0n:
      Step.reply(client, id, [M.ThreadSnoozed{th, Nat.add(now, M.Snooze.hour()), now}])
    case 1n:
      Step.reply(client, id, [M.ThreadSnoozed{th, Nat.add(now, M.Snooze.week()), now}])
    case 2n:
      Step.reply(client, id, [M.ThreadSnoozed{th, 0n, now}])
    case 3n:
      Step.outs([Git{client, id, "snooze", th, "", Nat.show(now), ""}])
    case _:
      Step.reply(client, id, [M.ThreadSnoozed{th, J.Json.get_nat(p, "until", 0n), now}])

def Hub.snooze(+now: Nat, +client: U32, +id: String, +p: J.Json) -> Step:
  Hub.snooze.k(C.Codec.pos(["1h", "1w", "off", "tomorrow"], J.Json.get_str(p, "for", "")), now, client, id, J.Json.get_str(p, "thread", ""), p)

def Reorder.okeys(xs: List<&2, J.Json>, +navs: Map<&2, M.Nav>, +section: String) -> List<&2, M.OKey>:
  match xs:
    case Nil{}:
      Nil{}
    case J.Str{+i} <> t:
      M.OKey{i, M.Nav.key(M.Nav.get(navs, i), section)} <> Reorder.okeys(t, navs, section)
    case _ <> t:
      Reorder.okeys(t, navs, section)

def Reorder.changes(ks: List<&2, M.OKey>, +section: String, +now: Nat) -> List<&2, M.Change>:
  match ks:
    case Nil{}:
      Nil{}
    case M.OKey{i, k} <> t:
      M.ThreadOrdered{i, section, k, now} <> Reorder.changes(t, section, now)

def Hub.reorder.k(ok: Bool, +s: M.State, +now: Nat, +client: U32, +id: String, +section: String, +p: J.Json) -> Step:
  match ok:
    case True{}:
      Step.reply(client, id, Reorder.changes(M.Order.plan(Reorder.okeys(J.Json.elems(J.Json.get_obj(p, "ids")), M.State.navs(s), section), J.Json.get_str(p, "thread", "")), section, now))
    case False{}:
      Step.outs([Reply.err(client, id, "only pinned and active threads reorder")])

# a shelf's new order after a drag: {section: "pinned" | "active", thread:
# the one moved, ids: the shelf's threads in their new order}
def Hub.reorder(+s: M.State, +now: Nat, +client: U32, +id: String, +p: J.Json) -> Step:
  Hub.reorder.k(Bool.or(String.eq(J.Json.get_str(p, "section", ""), "pinned"), String.eq(J.Json.get_str(p, "section", ""), "active")), s, now, client, id, J.Json.get_str(p, "section", ""), p)

def Reorder.pkeys(xs: List<&2, J.Json>, +navs: Map<&2, M.Nav>) -> List<&2, M.OKey>:
  match xs:
    case Nil{}:
      Nil{}
    case J.Str{+i} <> t:
      M.OKey{i, M.Nav.project_key(navs, i)} <> Reorder.pkeys(t, navs)
    case _ <> t:
      Reorder.pkeys(t, navs)

def Reorder.projects(ks: List<&2, M.OKey>, +now: Nat) -> List<&2, M.Change>:
  match ks:
    case Nil{}:
      Nil{}
    case M.OKey{i, k} <> t:
      M.ProjectOrdered{i, k, now} <> Reorder.projects(t, now)

# the sidebar's projects in their new order after a drag: {project: the one
# moved, ids: every project in the new order}
def Hub.reorder_projects(+s: M.State, +now: Nat, +client: U32, +id: String, +p: J.Json) -> Step:
  Step.reply(client, id, Reorder.projects(M.Order.plan(Reorder.pkeys(J.Json.elems(J.Json.get_obj(p, "ids")), M.State.navs(s)), J.Json.get_str(p, "project", "")), now))

# Summaries (refs.bend's Sum.*): the text generation model writes one
# when a thread's turn ends and one is due; the time it was asked for is
# kept (summary.at.<id>) so a failed answer is not asked for again at once.
# A thread's summary makes its project's when that is due too.

def Sum.at(due: Bool, +id: String, +now: Nat) -> List<&2, M.Change>:
  match due:
    case True{}:
      [M.SettingSet{RF.Sum.at.key(id), Nat.show(now), now}]
    case False{}:
      Nil{}

def Sum.input(+kind: String, +s: M.State, +id: String) -> String:
  Bool.pick(String, String.eq(kind, "project"), RF.Sum.project.input(s, id), RF.Sum.thread.input(s, id))

def Sum.out(due: Bool, +s: M.State, +id: String, +kind: String) -> List<&2, Out>:
  match due:
    case True{}:
      [Git{0, "", "summary", id, "", kind, Sum.input(kind, s, id)}]
    case False{}:
      Nil{}

def Sum.keep(+id: String, +text: String, +now: Nat) -> List<&2, M.Change>:
  Bool.pick(List<&2, M.Change>, String.is_empty(text), Nil{}, [M.SettingSet{RF.Sum.key(id), text, now}])

# a thread's project summarized next, when it has one and it is due
def Sum.next(thread: Bool, +s: M.State, +now: Nat, +pid: String) -> Step:
  match thread:
    case True{}:
      Step{Sum.at(RF.Sum.due(s, pid, now), pid, now), Sum.out(RF.Sum.due(s, pid, now), s, pid, "project")}
    case False{}:
      Step{Nil{}, Nil{}}

# the writer's summary of a thread or project ({id, kind, text})
def Sum.got(+s: M.State, +now: Nat, +client: U32, +id: String, +p: J.Json) -> Step:
  Step.before(Sum.keep(J.Json.get_str(p, "id", ""), RF.Sum.clean(J.Json.get_str(p, "text", "")), now), [Reply.ok(client, id)],
    Sum.next(Bool.and(String.eq(J.Json.get_str(p, "kind", ""), "thread"), Bool.not(String.is_empty(RF.Refs.t.project(RF.Sum.thread.of(s, J.Json.get_str(p, "id", "")))))),
      s, now, RF.Refs.t.project(RF.Sum.thread.of(s, J.Json.get_str(p, "id", "")))))

# agent features (skills, effort, naming)
def Hub.methods.agent() -> List<&2, String>:
  ["thread.branch", "skills.list", "thread.effort", "thread.local", "thread.merged", "thread.unrooted", "thread.titled", "thread.btw", "summary.set"]

# a side question (/btw {thread, q}): the writing model answers it off
# the hub (Git job "btw": a is the question, b what it reads), only to the
# client that asked. Nothing is logged: the thread never holds it (law
# btw_never_logged)
def Hub.btw(+s: M.State, +client: U32, +id: String, +th: String, +q: String) -> Step:
  Step.outs([Git{client, id, "btw", th, "", q, HD.Hand.btw(Hub.entries(s), th, HD.Hand.title(Hub.thread(s, th)), q)}])

def Hub.provider_of(t: M.Thread) -> String:
  match t:
    case M.Thread{_, _, _, _, _, _, _, _, _, _, _, _, _, _, _, p, _, _, _, _, _}:
      p

def Hub.worktree(t: M.Thread) -> String:
  match t:
    case M.Thread{_, _, _, _, _, _, _, _, _, _, _, _, _, _, wt, _, _, _, _, _, _}:
      wt

# a thread's reasoning effort: kept in the settings map (effort.<thread>),
# only as a level its provider offers ("" is the provider's default)
def Hub.effort(+s: M.State, +now: Nat, +client: U32, +id: String, +thread: String, +e: String) -> Step:
  Step.reply(client, id, [M.SettingSet{AG.Effort.key(thread), AG.Effort.of(Hub.provider_of(Hub.thread(s, thread)), e), now}])

# a worktree thread's first message names its branch: the server asks a
# small model for a name while the turn runs, renames the temporary
# backplane/<hex> branch and reports it (thread.branch)
def Name.out.k(go: Bool, +thread: String, +wt: String, +branch: String, +text: String) -> List<&2, Out>:
  match go:
    case True{}:
      [Git{0, "", "name", thread, wt, branch, text}]
    case False{}:
      Nil{}

def Name.out(t: M.Thread, +text: String) -> List<&2, Out>:
  match t:
    case M.Thread{+id, _, +title, _, _, _, _, _, _, _, _, _, _, +br, +wt, _, _, _, _, _, _}:
      Name.out.k(Bool.and(Bool.and(String.eq(title, "New thread"), AG.Name.temp(br)), Bool.not(String.is_empty(wt))), id, wt, br, text)

# a worktree thread's first turn, run once its worktree exists
def Name.first(t: M.Thread, +text: String) -> List<&2, Out>:
  match t:
    case M.Thread{+id, _, _, _, _, _, _, _, _, _, _, _, _, +br, +wt, _, _, _, _, _, _}:
      Name.out.k(Bool.and(AG.Name.temp(br), Bool.not(String.is_empty(wt))), id, wt, br, text)

# A thread's first message titles it at once with its first line (the
# placeholder); the server asks the text generation model for a better
# title (Git job "title": a is the placeholder, b the message) and answers
# with thread.titled {thread, placeholder, title}
def Title.out(fresh: Bool, +thread: String, +text: String) -> List<&2, Out>:
  match fresh:
    case True{}:
      Bool.pick(List<&2, Out>, String.is_empty(Hub.title.of(text)), Nil{}, [Git{0, "", "title", thread, "", Hub.title.of(text), String.take(text, 4000n)}])
    case False{}:
      Nil{}

def Title.changes.ok(ok: Bool, +thread: String, +title: String, +now: Nat) -> List<&2, M.Change>:
  match ok:
    case True{}:
      [M.ThreadRenamed{thread, title, now}]
    case False{}:
      Nil{}

def Title.changes.k(same: Bool, ok: Bool, +thread: String, +title: String, +now: Nat) -> List<&2, M.Change>:
  match same:
    case True{}:
      Title.changes.ok(ok, thread, title, now)
    case False{}:
      Nil{}

# the writer's title replaces the placeholder only while the thread still
# has it (a rename by the user wins) and only when there is an answer
def Title.changes(+current: String, +placeholder: String, +answer: String, +thread: String, +now: Nat) -> List<&2, M.Change>:
  Title.changes.k(Bool.and(String.eq(current, placeholder), Bool.not(String.is_empty(placeholder))), Bool.not(String.is_empty(AG.Title.clean(answer))), thread, AG.Title.clean(answer), now)

def Title.got(+s: M.State, +now: Nat, +client: U32, +id: String, +thread: String, +p: J.Json) -> Step:
  Step.reply(client, id, Title.changes(Hub.thread_title(Hub.thread(s, thread)), J.Json.get_str(p, "placeholder", ""), J.Json.get_str(p, "title", ""), thread, now))

# Commit & push and Open PR without a message: the server writes one from
# the thread's diff (the thread's title when it cannot)
def Hub.act(+s: M.State, +client: U32, +id: String, +thread: String, +action: String, +p: J.Json) -> Step:
  Bool.pick(Step, Bool.and(String.contains(action, "commit"), String.eq(J.Json.show(Maybe.default(&2, J.Json, J.Json.get(p, "message"), J.Null{})), "null")),
    Hub.git(s, client, id, "act_gen", thread, action, Hub.thread_title(Hub.thread(s, thread))),
    Hub.git(s, client, id, "act", thread, action, J.Json.get_str(p, "message", Hub.thread_title(Hub.thread(s, thread)))))

# A thread's worktree is ready (thread.worktree) or cannot be made
# (thread.local): what waited for it runs now, in the thread's directory.
# The first message of a worktree thread names its branch here, since the
# worktree did not exist when it was sent.
def Wt.name(first: Bool, t: M.Thread, +text: String) -> List<&2, Out>:
  match first:
    case True{}:
      Name.first(t, text)
    case False{}:
      Nil{}

def Wt.start.k(go: Bool, +first: Bool, +t: M.Thread, +cwd: String, +queued: String, +now: Nat, +seq: Nat, +thread: String) -> Step:
  match go:
    case True{}:
      Step{[M.QueueTaken{thread, now}, M.TurnChanged{thread, M.Running{}, now}],
        Git{0, "", "checkpoint0", thread, cwd, "", ""} <> Git{0, "", "step", thread, cwd, "", ""} <> List.append(&2, Out, Wt.name(first, t, queued), Hub.run(t, cwd, queued, Hub.uuid(now, thread ++ "/w/" ++ Nat.show(seq))))}
    case False{}:
      Step.none()

# gh saw the thread's branch merged: it settles, once (a recorded merge is
# not due again)
def Merge.found(due: Bool, +s: M.State, +now: Nat, +client: U32, +id: String, +thread: String, +url: String) -> Step:
  match due:
    case True{}:
      Step.reply(client, id, [M.ThreadMerged{thread, url, now},
        M.ActivityLogged{thread, Hub.id("g", now, Hub.seq(s)), "info", "git", "merged" ++ Bool.pick(String, String.is_empty(url), "", ": " ++ url), now}])
    case False{}:
      Step.outs([Reply.ok(client, id)])

# attachments, files and the web client's panes
def Hub.methods.media() -> List<&2, String>:
  ["attach.put", "attach.file", "update.check", "history.refs"]

# one piece of an upload: "<index>|<1 when last>|<stored name>"
def Attach.piece(+p: J.Json) -> String:
  Nat.show(J.Json.get_nat(p, "i", 0n)) ++ "|" ++ Bool.pick(String, J.Json.get_bool(p, "last", True{}), "1", "0") ++ "|" ++
    AT.Att.stored(J.Json.get_str(p, "key", "u"), J.Json.get_str(p, "name", "file"))

# uploads are stored per thread; a thread must be named
def Attach.go(ok: Bool, +client: U32, +id: String, o: Out) -> Step:
  match ok:
    case True{}:
      Step.outs([o])
    case False{}:
      Step.outs([Reply.err(client, id, "no thread to attach to")])

# attach.put: a piece of an uploaded file (base64); attach.file: a file on
# this machine, copied in; update.check: look for a release now
# desk.look {thread, focused}: no reply (a desk says it often)
def Desk.request(look: Bool, +client: U32, +id: String, +p: J.Json) -> Step:
  match look:
    case True{}:
      Step.outs([DeskLook{client, J.Json.get_str(p, "thread", ""), J.Json.get_bool(p, "focused", False{})}])
    case False{}:
      Step.outs([Reply.err(client, id, "unknown method")])

# Design review (docs/design-review.md): requests the server answers from
# the thread's folder (server/review.bend's RvJob, jobs "rv.…").
# history.resolve {thread, side, ref}: any commit, branch or tag typed in;
# review.bom {thread, a, b}: a BOM, or the change between two versions;
# review.checks {thread, commit, trigger}: the design checks;
# review.tick {thread, index}: a checklist box ticked or cleared;
# review.comment {thread, op, ...}: a comment added, answered, resolved,
# reopened, edited or dropped (core/review.bend's Review.act), by the person;
# review.release {thread, tag, publish, force}: a release (server/review.bend)
def Rv.methods() -> List<&2, String>:
  ["history.resolve", "review.bom", "review.checks", "review.tick", "review.comment", "review.release"]

def Rv.comment.k(ok: Bool, +s: M.State, +now: Nat, +client: U32, +id: String, +p: J.Json) -> Step:
  match ok:
    case True{}:
      Step.reply(client, id, [RVW.Review.act(J.Json.get_str(p, "thread", ""), Hub.id("r", now, Hub.seq(s)), p, "person", now)])
    case False{}:
      Step.outs([Reply.err(client, id, "a comment needs its text (or the comment it is about)")])

def Rv.request(i: Nat, +s: M.State, +now: Nat, +client: U32, +id: String, +p: J.Json, +m: String) -> Step:
  match i:
    case 0n:
      Hub.git(s, client, id, "rv.resolve", J.Json.get_str(p, "thread", ""), J.Json.get_str(p, "side", "a"), J.Json.get_str(p, "ref", ""))
    case 1n:
      Hub.git(s, client, id, "rv.bom", J.Json.get_str(p, "thread", ""), J.Json.get_str(p, "a", ""), J.Json.get_str(p, "b", ""))
    case 2n:
      Hub.git(s, client, id, "rv.checks", J.Json.get_str(p, "thread", ""), J.Json.get_str(p, "commit", ""), J.Json.get_str(p, "trigger", "manual"))
    case 3n:
      Hub.git(s, client, id, "rv.tick", J.Json.get_str(p, "thread", ""), J.Json.as_raw(J.Json.get(p, "index"), "0"), "")
    case 4n:
      Rv.comment.k(Bool.and(RVW.Review.op.ok(J.Json.get_str(p, "op", ""), p), Bool.not(String.is_empty(J.Json.get_str(p, "thread", "")))), s, now, client, id, p)
    case 5n:
      Hub.git(s, client, id, "rv.release", J.Json.get_str(p, "thread", ""), J.Json.get_str(p, "tag", ""),
        Bool.pick(String, J.Json.get_bool(p, "publish", False{}), "p", "") ++ Bool.pick(String, J.Json.get_bool(p, "force", False{}), "f", ""))
    case _:
      Desk.request(String.eq(m, "desk.look"), client, id, p)

def Hub.media(i: Nat, +s: M.State, +now: Nat, +client: U32, +id: String, +p: J.Json, +m: String) -> Step:
  match i:
    case 0n:
      Attach.go(Bool.not(String.is_empty(J.Json.get_str(p, "thread", ""))), client, id,
        Git{client, id, "attach.put", J.Json.get_str(p, "thread", ""), "", Attach.piece(p), J.Json.get_str(p, "data", "")})
    case 1n:
      Attach.go(Bool.not(String.is_empty(J.Json.get_str(p, "thread", ""))), client, id,
        Git{client, id, "attach.file", J.Json.get_str(p, "thread", ""), "", AT.Att.from_uri(J.Json.get_str(p, "path", "")),
          AT.Att.stored(Hub.id("a", now, Hub.seq(s)), AT.Att.base(AT.Att.from_uri(J.Json.get_str(p, "path", ""))))})
    case 2n:
      Step.outs([Git{client, id, "update.check", "", "", M.Setting.get(s, "update.channel", "stable"), ""}])
    case 3n:
      Hub.git(s, client, id, "refs", J.Json.get_str(p, "thread", ""), "", "")
    case _:
      Rv.request(C.Codec.pos(Rv.methods(), m), s, now, client, id, p, m)

def Hub.methods() -> List<&2, String>:
  ["project.add", "project.remove", "thread.create", "turn.start", "turn.interrupt",
   "thread.settle", "thread.unsettle", "thread.archive", "thread.unarchive",
   "thread.pin", "thread.unpin", "thread.rename", "thread.delete", "kicad.watch", "thread.modes", "app.update", "setting.set", "kicad.install",
   "ask.answer", "device.register", "device.drop", "kicad.info", "queue.resume",
   "queue.edit", "queue.remove", "queue.move", "queue.steer", "thread.fork", "thread.back", "thread.provider", "update.now", "work.cancel", "task.cancel"]

# Agent output (Claude stream-json)
# ---------------------------------

def Delta.push(thread: String, text: String) -> Out:
  Live{thread, text}

# the delta a Live out is sent as
def Delta.msg(+thread: String, +text: String) -> String:
  J.Json.show(J.Json.obj([J.KV{"t", J.Str{"delta"}}, J.KV{"thread", J.Str{thread}}, J.KV{"text", J.Str{text}}]))

# Claude streams a token a line, and each was a frame to every client: a
# phone rebuilt its screen per token. The lines of one read of the agent's
# stdout (as much as has come, up to 64 KB) go to the hub together, and a
# run of the main agent's text deltas to one block among them becomes one
# delta with their text: an agent writing fast sends a few frames a read,
# one writing slowly still sends each token as it comes. Every other line
# stays as it is, in its place (test/delta_test.bend).

# the merge so far: the lines out (last first), and the run open (its
# block's key and its texts, last first)
type DL is Data:
  DL{out: List<&2, String>, key: String, texts: List<&2, String>, open: Bool}

# a run's text as the one line that stands for it
def Delta.line(+ts: List<&2, String>) -> String:
  J.Json.show(J.Json.obj([J.KV{"type", J.Str{"stream_event"}}, J.KV{"event", J.Json.obj([J.KV{"type", J.Str{"content_block_delta"}},
    J.KV{"delta", J.Json.obj([J.KV{"type", J.Str{"text_delta"}}, J.KV{"text", J.Str{String.join(List.reverse(&2, String, ts), "")}}])}])}]))

def Delta.flush(open: Bool, +ts: List<&2, String>, out: List<&2, String>) -> List<&2, String>:
  match open:
    case True{}:
      Delta.line(ts) <> out
    case False{}:
      out

def Delta.step.k(delta: Bool, same: Bool, +key: String, text: String, line: String, st: DL) -> DL:
  match delta same st:
    case True{} True{} DL{out, k, ts, o}:
      DL{out, k, text <> ts, o}
    case True{} False{} DL{out, _, +ts, o}:
      DL{Delta.flush(o, ts, out), key, [text], True{}}
    case False{} _ DL{out, _, +ts, o}:
      DL{line <> Delta.flush(o, ts, out), "", Nil{}, False{}}

def DL.open(st: DL) -> Bool:
  match st:
    case DL{_, _, _, o}:
      o

def DL.key(st: DL) -> String:
  match st:
    case DL{_, k, _, _}:
      k

# a text delta of the main agent (not a subagent's), and its block
def Delta.step.j(+o: J.Json, line: String, +st: DL) -> DL:
  +ev = J.Json.get_obj(o, "event")
  +d = J.Json.get_obj(ev, "delta")
  +key = J.Json.show(J.Json.get_obj(ev, "index"))
  Delta.step.k(Bool.and(Bool.and(String.eq(J.Json.get_str(o, "type", ""), "stream_event"), String.is_empty(J.Json.get_str(o, "parent_tool_use_id", ""))),
      Bool.and(String.eq(J.Json.get_str(ev, "type", ""), "content_block_delta"), String.eq(J.Json.get_str(d, "type", ""), "text_delta"))),
    Bool.and(DL.open(st), String.eq(key, DL.key(st))), key, J.Json.get_str(d, "text", ""), line, st)

def Delta.step.m(m: Maybe<&2, J.Json>, line: String, +st: DL) -> DL:
  match m:
    case Some{+o}:
      Delta.step.j(o, line, st)
    case None{}:
      Delta.step.k(False{}, False{}, "", "", line, st)

def Delta.step.p(stream: Bool, +line: String, +st: DL) -> DL:
  match stream:
    case True{}:
      Delta.step.m(J.Json.parse(line), line, st)
    case False{}:
      Delta.step.k(False{}, False{}, "", "", line, st)

# (only a stream event is parsed: the other lines, a tool's whole output
# among them, pass as they are)
def Delta.step(+line: String, +st: DL) -> DL:
  Delta.step.p(String.starts_with(line, "{\"type\":\"stream_event\""), line, st)

def Delta.go(ls: List<&2, String>, st: DL) -> DL:
  match ls:
    case Nil{}:
      st
    case l <> t:
      Delta.go(t, Delta.step(l, st))

def Delta.fin(st: DL) -> List<&2, String>:
  match st:
    case DL{out, _, +ts, o}:
      List.reverse(&2, String, Delta.flush(o, ts, out))

# one read's lines with each run of text deltas as one
def Delta.merge(ls: List<&2, String>) -> List<&2, String>:
  Delta.fin(Delta.go(ls, DL{Nil{}, "", Nil{}, False{}}))

def Delta.last(ls: List<&2, String>, acc: Bool) -> Bool:
  match ls:
    case Nil{}:
      acc
    case l <> t:
      Delta.last(t, String.starts_with(l, "{\"type\":\"stream_event\""))

# whether a read ended in streamed text: the reader then waits Delta.pace()
# ms before reading again, so the tokens written meanwhile come in one read
# and go out as one delta (an agent streams tens of tokens a second, each a
# frame to every client, and a screen for a phone showing the thread);
# anything else is read at once
def Delta.streaming(ls: List<&2, String>) -> Bool:
  Delta.last(ls, False{})

def Delta.pace() -> U32:
  60

def Claude.stream.k(block: Bool, text: Bool, +thread: String, s: String) -> Step:
  match block text:
    case True{} True{}:
      Step.outs([Delta.push(thread, s)])
    case _ _:
      Step.none()

def Claude.stream(+thread: String, +ev: J.Json) -> Step:
  +d = J.Json.get_obj(ev, "delta")
  Claude.stream.k(String.eq(J.Json.get_str(ev, "type", ""), "content_block_delta"),
    String.eq(J.Json.get_str(d, "type", ""), "text_delta"), thread, J.Json.get_str(d, "text", ""))

# a short line describing a tool call, from its input
def Claude.tool_summary(+name: String, +input: J.Json) -> String:
  +cmd = J.Json.get_str(input, "command", "")
  +path = J.Json.get_str(input, "file_path", J.Json.get_str(input, "path", ""))
  +pat = J.Json.get_str(input, "pattern", "")
  Bool.pick(String, Bool.not(String.is_empty(cmd)), cmd,
    Bool.pick(String, Bool.not(String.is_empty(path)), path,
      Bool.pick(String, Bool.not(String.is_empty(pat)), pat, J.Json.get_str(input, "description", name))))

# The CLI writes one block per line, each numbered from 0, so two text
# blocks of one message (text, thinking, text) share "msg:0". A line that
# has a uuid adds its first 8 characters to the id; one without keeps the
# old id (and the persisted ids are untouched: law claude_text_ids_distinct).
def Claude.uid(+uuid: String) -> String:
  Bool.pick(String, String.is_empty(uuid), "", ":" ++ String.take(uuid, 8n))

def Claude.block.k(i: Nat, +thread: String, +msg: String, +n: Nat, +now: Nat, +seq: Nat, +uuid: String, +b: J.Json) -> List<&2, M.Change>:
  match i:
    case 0n:
      [M.MessagePosted{thread, msg ++ ":" ++ Nat.show(n) ++ Claude.uid(uuid), M.Assistant{}, J.Json.get_str(b, "text", ""), now}]
    case 1n:
      [M.ActivityLogged{thread, J.Json.get_str(b, "id", Hub.id("a", now, seq)), "tool", J.Json.get_str(b, "name", "tool"),
        Claude.tool_summary(J.Json.get_str(b, "name", "tool"), J.Json.get_obj(b, "input")), now}]
    case _:
      Nil{}

def Claude.blocks(bs: List<&2, J.Json>, +thread: String, +msg: String, +n: Nat, +now: Nat, +seq: Nat, +uuid: String) -> List<&2, M.Change>:
  match bs:
    case Nil{}:
      Nil{}
    case +b <> t:
      List.append(&2, M.Change,
        Claude.block.k(C.Codec.pos(["text", "tool_use"], J.Json.get_str(b, "type", "")), thread, msg, n, now, seq, uuid, b),
        Claude.blocks(t, thread, msg, 1n+n, now, seq, uuid))

def Claude.assistant(+thread: String, +now: Nat, +seq: Nat, +m: J.Json, +uuid: String) -> Step:
  Step{Claude.blocks(J.Json.elems(J.Json.get_obj(m, "content")), thread, J.Json.get_str(m, "id", "msg"), 0n, now, seq, uuid),
    [Broadcast{J.Json.show(J.Json.obj([J.KV{"t", J.Str{"delta_end"}}, J.KV{"thread", J.Str{thread}}]))}]}

def Claude.result.k(err: Bool, +thread: String, +now: Nat, text: String) -> Step:
  match err:
    case True{}:
      Step{[M.ActivityLogged{thread, Hub.id("e", now, 0n), "error", "result", text, now}, M.TurnChanged{thread, M.Failed{}, now}], Nil{}}
    case False{}:
      Step{[M.TurnChanged{thread, M.Completed{}, now}], Nil{}}

def Claude.result(+thread: String, +now: Nat, +o: J.Json) -> Step:
  Claude.result.k(J.Json.get_bool(o, "is_error", False{}), thread, now, J.Json.get_str(o, "result", ""))

# Claude keeps its process across a Stop, and what it still says for the
# stopped turn (its partial text, its result) changes nothing: only a
# running turn completes or fails, and only a running turn's text streams
# (laws claude_result_after_stop_quiet, claude_delta_after_stop_quiet). Its
# whole messages, tool results and background tasks still land. (A result
# for the old turn that comes after a new one started is still taken: the
# lines carry no turn of their own.)
def Claude.stream.if(run: Bool, +thread: String, +ev: J.Json) -> Step:
  match run:
    case True{}:
      Claude.stream(thread, ev)
    case False{}:
      Step.none()

def Claude.result.if(run: Bool, +thread: String, +now: Nat, +o: J.Json) -> Step:
  match run:
    case True{}:
      Claude.result(thread, now, o)
    case False{}:
      Step.none()

# Claude's todo tools
# -------------------
# TodoWrite sets the whole list; TaskCreate adds a step and TaskUpdate
# changes one, so the hub folds them over the thread's list, and a
# TaskCreate's result (a "user" line) names the step it made.

def Claude.todo.k(i: Nat, +b: J.Json, ts: List<&2, TD.Todo>) -> List<&2, TD.Todo>:
  match i:
    case 0n:
      TD.Todo.write(J.Json.get_obj(b, "input"))
    case 1n:
      TD.Todo.create(ts, J.Json.get_str(b, "id", ""), J.Json.get_obj(b, "input"))
    case 2n:
      TD.Todo.update(ts, J.Json.get_obj(b, "input"))
    case _:
      ts

def Claude.todo.kind(+b: J.Json) -> Nat:
  Bool.pick(Nat, String.eq(J.Json.get_str(b, "type", ""), "tool_use"), C.Codec.pos(["TodoWrite", "TaskCreate", "TaskUpdate"], J.Json.get_str(b, "name", "")), 3n)

def Claude.todo.go(bs: List<&2, J.Json>, ts: List<&2, TD.Todo>) -> List<&2, TD.Todo>:
  match bs:
    case Nil{}:
      ts
    case +b <> t:
      Claude.todo.go(t, Claude.todo.k(Claude.todo.kind(b), b, ts))

def Claude.todo.any(bs: List<&2, J.Json>) -> Bool:
  match bs:
    case Nil{}:
      False{}
    case +b <> t:
      Bool.or(Nat.is_lt(Claude.todo.kind(b), 3n), Claude.todo.any(t))

def Claude.todos.of(+s: M.State, +th: String) -> List<&2, TD.Todo>:
  TD.Todo.read(M.Setting.get(s, "todos." ++ th, ""))

def Claude.todos.set(changed: Bool, +th: String, +now: Nat, +ts: String) -> List<&2, M.Change>:
  match changed:
    case True{}:
      [M.TodosSet{th, ts, now}]
    case False{}:
      Nil{}

# an assistant message's todo tools, folded over the thread's list
def Claude.todos(+s: M.State, +th: String, +now: Nat, +bs: List<&2, J.Json>) -> List<&2, M.Change>:
  Claude.todos.set(Claude.todo.any(bs), th, now, TD.Todo.show(Claude.todo.go(bs, Claude.todos.of(s, th))))

# Context usage (Claude)
# ----------------------
# Each model call reports its usage: what it read (input and both caches)
# plus what it wrote is what the session now holds. The result line names
# the model's window; a compaction says what is left. A reading goes to the
# log only when it changes (law usage_same_nothing).

def Usage.now(+s: M.State, +th: String) -> String:
  M.Setting.get(s, M.Usage.key(th), "")

def Usage.set(fresh: Bool, +th: String, +used: Nat, +window: Nat, +now: Nat) -> List<&2, M.Change>:
  match fresh:
    case True{}:
      [M.UsageSet{th, used, window, now}]
    case False{}:
      Nil{}

def Usage.sum(+u: J.Json) -> Nat:
  Nat.add(Nat.add(J.Json.get_nat(u, "input_tokens", 0n), J.Json.get_nat(u, "cache_creation_input_tokens", 0n)),
    Nat.add(J.Json.get_nat(u, "cache_read_input_tokens", 0n), J.Json.get_nat(u, "output_tokens", 0n)))

def Usage.used.k(+s: M.State, +th: String, +now: Nat, +n: Nat) -> List<&2, M.Change>:
  Usage.set(Bool.and(Nat.is_gt(n, 0n), Bool.not(Nat.is_eq(n, M.Usage.used(Usage.now(s, th))))), th, n, 0n, now)

# an assistant message's usage
def Usage.claude(+s: M.State, +th: String, +now: Nat, +m: J.Json) -> List<&2, M.Change>:
  Usage.used.k(s, th, now, Usage.sum(J.Json.get_obj(m, "usage")))

def Usage.widest(fs: J.Json, +acc: Nat) -> Nat:
  match fs:
    case J.Field{_, +v, t}:
      Usage.widest(t, Bool.pick(Nat, Nat.is_gt(J.Json.get_nat(v, "contextWindow", 0n), acc), J.Json.get_nat(v, "contextWindow", 0n), acc))
    case _:
      acc

def Usage.fields(o: J.Json) -> J.Json:
  match o:
    case J.Obj{fs}:
      fs
    case _:
      J.Null{}

def Usage.window.k(+s: M.State, +th: String, +now: Nat, +w: Nat) -> List<&2, M.Change>:
  Usage.set(Bool.and(Nat.is_gt(w, 0n), Bool.not(Nat.is_eq(w, M.Usage.window(Usage.now(s, th))))), th, 0n, w, now)

# the result's window: the widest model it used (the main one; helpers
# such as a small model have smaller windows)
def Usage.window(+s: M.State, +th: String, +now: Nat, +o: J.Json) -> List<&2, M.Change>:
  Usage.window.k(s, th, now, Usage.widest(Usage.fields(J.Json.get_obj(o, "modelUsage")), 0n))

# a compaction: what the session holds after it
def Usage.compact(+s: M.State, +th: String, +now: Nat, +o: J.Json) -> List<&2, M.Change>:
  Bool.pick(List<&2, M.Change>, String.eq(J.Json.get_str(o, "subtype", ""), "compact_boundary"),
    Usage.used.k(s, th, now, J.Json.get_nat(J.Json.get_obj(o, "compact_metadata"), "post_tokens", 0n)), Nil{})

# an assistant message and what its todo tools do to the thread's list
def Claude.assistant.t(+s: M.State, +thread: String, +now: Nat, +m: J.Json, +uuid: String) -> Step:
  Step.after(Step.after(Claude.assistant(thread, now, Hub.seq(s), m, uuid), Claude.todos(s, thread, now, J.Json.elems(J.Json.get_obj(m, "content")))), Usage.claude(s, thread, now, m))

def Claude.result.text(+c: J.Json) -> String:
  J.Json.get_str(Maybe.default(&2, J.Json, List.head(&2, J.Json, J.Json.elems(c)), J.Null{}), "text", J.Json.as_str(Some{c}, ""))

def Claude.named.go(bs: List<&2, J.Json>, ts: List<&2, TD.Todo>) -> List<&2, TD.Todo>:
  match bs:
    case Nil{}:
      ts
    case +b <> t:
      Claude.named.go(t, TD.Todo.named(ts, J.Json.get_str(b, "tool_use_id", ""), TD.Todo.result(Claude.result.text(J.Json.get_obj(b, "content")))))

# a "user" line carries tool results: a TaskCreate's names its step
def Claude.named(+s: M.State, +th: String, +now: Nat, +o: J.Json) -> Step:
  +before = TD.Todo.show(Claude.todos.of(s, th))
  +after = TD.Todo.show(Claude.named.go(J.Json.elems(J.Json.get_obj(J.Json.get_obj(o, "message"), "content")), Claude.todos.of(s, th)))
  Step{Claude.todos.set(Bool.not(String.eq(before, after)), th, now, after), Nil{}}

def Claude.init.same(t: M.Thread, +sid: String) -> Bool:
  match t:
    case M.Thread{_, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, session, _}:
      String.eq(session, sid)

def Claude.init.k(same: Bool, thread: String, sid: String, +now: Nat) -> Step:
  match same:
    case True{}:
      Step.none()
    case False{}:
      Step{[M.SessionSet{thread, sid, now}], Nil{}}

# the session id arrives in the init line; record it once
def Claude.init(+s: M.State, +thread: String, +now: Nat, +o: J.Json) -> Step:
  +sid = J.Json.get_str(o, "session_id", "")
  Claude.init.k(Claude.init.same(Hub.thread(s, thread), sid), thread, sid, now)

# Claude's subagents (model.bend's Bg): a local agent started, what it
# does now, and its end; the list is written only when it changes, as a
# WorkSet (shared with the owner's other machines with its thread; old
# logs wrote it as the setting bg.<thread>)
def Bg.put.k(same: Bool, +th: String, +now: Nat, +after: String) -> List<&2, M.Change>:
  match same:
    case True{}:
      Nil{}
    case False{}:
      [M.WorkSet{th, "bg", after, now}]

def Bg.put(+s: M.State, +th: String, +now: Nat, ls: List<&2, String>) -> List<&2, M.Change>:
  +after = M.Bg.show(ls)
  Bg.put.k(String.eq(M.Setting.get(s, M.Bg.key(th), ""), after), th, now, after)

# a mark in a subagent's log (model.bend's Bg.log): it started, or ended
# with a status; none for one no tool call started
def Bg.mark.k(none: Bool, +th: String, +now: Nat, +call: String, +kind: String, +text: String) -> List<&2, M.Change>:
  match none:
    case True{}:
      Nil{}
    case False{}:
      [M.ActivityLogged{M.Bg.log(th), call ++ "/" ++ kind, call, kind, text, now}]

def Bg.mark(+th: String, +now: Nat, +call: String, +kind: String, +text: String) -> List<&2, M.Change>:
  Bg.mark.k(String.is_empty(call), th, now, call, kind, text)

def Bg.started(agent: Bool, +s: M.State, +th: String, +now: Nat, +o: J.Json) -> List<&2, M.Change>:
  match agent:
    case True{}:
      +call = J.Json.get_str(o, "tool_use_id", "")
      +what = J.Json.get_str(o, "description", "Subagent")
      +kind = M.Bg.kind(J.Json.get_str(o, "task_type", ""))
      List.append(&2, M.Change, Bg.put(s, th, now, M.Bg.start(M.Bg.of(s, th), J.Json.get_str(o, "task_id", ""), what, call, what, kind)),
        Bg.mark(th, now, call, "start:" ++ kind, what))
    case False{}:
      Nil{}

# a foreground shell Claude moved to the background when it ran past its
# timeout (task_updated sets is_backgrounded; the patch has no description)
def Bg.moved(bg: Bool, +s: M.State, +th: String, +now: Nat, +id: String) -> List<&2, M.Change>:
  match bg:
    case True{}:
      Bg.put(s, th, now, M.Bg.start(M.Bg.of(s, th), id, "Background shell", "", "Background shell", "shell"))
    case False{}:
      Nil{}

# a task finished: its row stays, ended, until the agent is handed its
# outcome (its task_notification; Bg.settle at a turn's end)
def Bg.ended(done: Bool, bg: Bool, +s: M.State, +th: String, +now: Nat, +id: String, +status: String) -> List<&2, M.Change>:
  match done:
    case True{}:
      Bg.put(s, th, now, M.Bg.end(M.Bg.of(s, th), id, status))
    case False{}:
      Bg.moved(bg, s, th, now, id)

def Bg.ids(ts: List<&2, J.Json>) -> List<&2, String>:
  match ts:
    case Nil{}:
      Nil{}
    case +t <> rest:
      J.Json.get_str(t, "task_id", "") <> Bg.ids(rest)

# Claude's whole list of open tasks (background_tasks_changed): a row not
# on it has ended (M.Bg.sync; none is added)
def Bg.sync(+s: M.State, +th: String, +now: Nat, +o: J.Json) -> List<&2, M.Change>:
  Bg.put(s, th, now, M.Bg.sync(M.Bg.of(s, th), Bg.ids(J.Json.elems(J.Json.get_obj(o, "tasks")))))

# a turn ended: the outcomes of the rows that ended before it are the
# agent's now (the CLI may fold a notification into a running turn)
def Bg.settle(+s: M.State, +th: String, +now: Nat) -> List<&2, M.Change>:
  Bg.put(s, th, now, M.Bg.unended(M.Bg.of(s, th)))

# a task the agent left running: a subagent, a background shell or
# monitor, a workflow; not one a subagent runs for itself, and not a
# foreground shell (listed only if Claude moves it to the background,
# Bg.moved)
def Bg.listed(+o: J.Json) -> Bool:
  Bool.and(Nat.is_lt(C.Codec.pos(["local_agent", "local_bash", "local_workflow", "remote_agent", "monitor_mcp", "monitor_ws", "in_process_teammate"], J.Json.get_str(o, "task_type", "")), 7n),
    Bool.and(Bool.not(J.Json.get_bool(o, "owned_by_subagent", False{})), J.Json.get_bool(o, "is_backgrounded", True{})))

def Bg.system.k(i: Nat, +s: M.State, +th: String, +now: Nat, +o: J.Json) -> List<&2, M.Change>:
  match i:
    case 0n:
      Bg.started(Bg.listed(o), s, th, now, o)
    case 1n:
      Bg.put(s, th, now, M.Bg.progress(M.Bg.of(s, th), J.Json.get_str(o, "task_id", ""), J.Json.get_str(o, "description", "")))
    case 2n:
      List.append(&2, M.Change, Bg.put(s, th, now, M.Bg.drop(M.Bg.of(s, th), J.Json.get_str(o, "task_id", ""))),
        Bg.mark(th, now, J.Json.get_str(o, "tool_use_id", ""), "end", J.Json.get_str(o, "status", "completed")))
    case 3n:
      Bg.ended(Nat.is_lt(C.Codec.pos(["completed", "failed", "killed", "stopped", "cancelled"], J.Json.get_str(J.Json.get_obj(o, "patch"), "status", "")), 5n),
        J.Json.get_bool(J.Json.get_obj(o, "patch"), "is_backgrounded", False{}), s, th, now, J.Json.get_str(o, "task_id", ""), J.Json.get_str(J.Json.get_obj(o, "patch"), "status", ""))
    case 4n:
      Bg.sync(s, th, now, o)
    case _:
      Nil{}

def Bg.system(+s: M.State, +th: String, +now: Nat, +o: J.Json) -> List<&2, M.Change>:
  Bg.system.k(C.Codec.pos(["task_started", "task_progress", "task_notification", "task_updated", "background_tasks_changed"], J.Json.get_str(o, "subtype", "")), s, th, now, o)

# Progress, coalesced
# -------------------
# A task's task_progress line is only what its row says it is doing now,
# yet each distinct one was a persisted WorkSet and a broadcast. The server
# keeps a thread's progress lines (the newest of each task) and lets them
# through at most once every Prog.gap() seconds (Prog.line); a line held
# lands on the server's next beat once the gap is over (Prog.beat), so the
# newest always lands. Starts, ends and everything else are written at
# once (laws bg_progress_coalesced, bg_progress_latest_lands).
type PStep is Data:
  PStep{at: Nat, pend: List<&2, String>, flush: List<&2, String>}

def PStep.at(p: PStep) -> Nat:
  match p:
    case PStep{a, _, _}:
      a

def PStep.pend(p: PStep) -> List<&2, String>:
  match p:
    case PStep{_, d, _}:
      d

def PStep.flush(p: PStep) -> List<&2, String>:
  match p:
    case PStep{_, _, f}:
      f

def Prog.gap() -> Nat:
  5n

# a progress line of the thread's own agent (a subagent's goes to its log)
def Prog.is.k(sys: Bool, +o: J.Json) -> Bool:
  Bool.and(sys, Bool.and(String.eq(J.Json.get_str(o, "subtype", ""), "task_progress"), String.is_empty(J.Json.get_str(o, "parent_tool_use_id", ""))))

def Prog.is(+o: J.Json) -> Bool:
  Prog.is.k(String.eq(J.Json.get_str(o, "type", ""), "system"), o)

def Prog.due(+at: Nat, +now: Nat) -> Bool:
  Nat.is_ge(now, Nat.add(at, Prog.gap()))

def Prog.task(+line: String) -> String:
  J.Json.get_str(Maybe.default(&2, J.Json, J.Json.parse(line), J.Null{}), "task_id", "")

def Prog.put.k(same: Bool, +l: String, +line: String, t: List<&2, String>, rest: List<&2, String>) -> List<&2, String>:
  match same:
    case True{}:
      line <> t
    case False{}:
      l <> rest

# the held lines with a task's replaced by its newer one
def Prog.put(ls: List<&2, String>, +id: String, +line: String) -> List<&2, String>:
  match ls:
    case Nil{}:
      [line]
    case +l <> +t:
      Prog.put.k(String.eq(Prog.task(l), id), l, line, t, Prog.put(t, id, line))

def Prog.line.k(due: Bool, +at: Nat, +now: Nat, pend: List<&2, String>) -> PStep:
  match due:
    case True{}:
      PStep{now, Nil{}, pend}
    case False{}:
      PStep{at, pend, Nil{}}

# a progress line heard at now: let through with every held one when the
# gap since the last write is over, else held
def Prog.line(+at: Nat, pend: List<&2, String>, +now: Nat, +line: String, +o: J.Json) -> PStep:
  Prog.line.k(Prog.due(at, now), at, now, Prog.put(pend, J.Json.get_str(o, "task_id", ""), line))

def Prog.beat.k(go: Bool, +at: Nat, +now: Nat, pend: List<&2, String>) -> PStep:
  match go:
    case True{}:
      PStep{now, Nil{}, pend}
    case False{}:
      PStep{at, pend, Nil{}}

# the server's beat: held lines land once the gap is over
def Prog.beat(+at: Nat, +pend: List<&2, String>, +now: Nat) -> PStep:
  Prog.beat.k(Bool.and(Prog.due(at, now), Bool.not(List.is_empty(&2, String, pend))), at, now, pend)

def Prog.one(rows: List<&2, String>, +o: J.Json) -> List<&2, String>:
  M.Bg.progress(rows, J.Json.get_str(o, "task_id", ""), J.Json.get_str(o, "description", ""))

def Prog.fold(ls: List<&2, String>, rows: List<&2, String>) -> List<&2, String>:
  match ls:
    case Nil{}:
      rows
    case +l <> t:
      Prog.fold(t, Prog.one(rows, Maybe.default(&2, J.Json, J.Json.parse(l), J.Null{})))

# the rows after the lines, written once
def Prog.flush(+s: M.State, +th: String, +now: Nat, ls: List<&2, String>) -> Step:
  Step{Bg.put(s, th, now, Prog.fold(ls, M.Bg.of(s, th))), Nil{}}

# two progress lines of the laws' task "a"
def Prog.l1() -> String:
  "{\"type\":\"system\",\"subtype\":\"task_progress\",\"task_id\":\"a\",\"description\":\"one\"}"

def Prog.l2() -> String:
  "{\"type\":\"system\",\"subtype\":\"task_progress\",\"task_id\":\"a\",\"description\":\"two\"}"

def Prog.o(+l: String) -> J.Json:
  Maybe.default(&2, J.Json, J.Json.parse(l), J.Null{})

# a thread with one background shell at work (the laws' example)
def Bg.demo() -> M.State:
  M.State.replay([M.ProjectCreated{"p1", "p", "/p", 1n}, M.ThreadCreated{"t1", "p1", "x", M.Local{}, "", "", "claude", "", 1n},
    M.WorkSet{"t1", "bg", "a\tx\tc1\tsleep 8\tshell", 1n}], M.State.empty())

# none left at work (the agent's process ended)
def Bg.clear(+s: M.State, +th: String, +now: Nat) -> List<&2, M.Change>:
  Bg.put(s, th, now, Nil{})

# Design steps
# ------------
# After a tool call the server snapshots the thread's KiCad files (job
# "step", its `a` the entry the step follows; core/hist.bend). Claude's
# tool calls end with a "user" line of tool results; Codex's and Grok's
# are logged when they have run.

def Steps.out(+thread: String, +cwd: String, +entry: String) -> Out:
  Git{0, "", "step", thread, cwd, entry, ""}

def Step.also(st: Step, os: List<&2, Out>) -> Step:
  match st:
    case Step{cs, o}:
      Step{cs, List.append(&2, Out, o, os)}

def Steps.hit(bs: List<&2, J.Json>) -> Bool:
  match bs:
    case +b <> _:
      String.eq(J.Json.get_str(b, "type", ""), "tool_result")
    case Nil{}:
      False{}

# the first tool result's call
def Steps.first(bs: List<&2, J.Json>, hit: Bool) -> String:
  match bs hit:
    case Nil{} _:
      ""
    case +b <> +r True{}:
      J.Json.get_str(b, "tool_use_id", "")
    case +b <> t False{}:
      Steps.first(t, Steps.hit(t))

def Steps.claude.k(+id: String, +thread: String, +cwd: String) -> List<&2, Out>:
  Bool.pick(List<&2, Out>, String.is_empty(id), Nil{}, [Steps.out(thread, cwd, id)])

# a line of tool results: a step after the first of them
def Steps.claude(+bs: List<&2, J.Json>, +thread: String, +cwd: String) -> List<&2, Out>:
  Steps.claude.k(Steps.first(bs, Steps.hit(bs)), thread, cwd)

def Steps.acts(cs: List<&2, M.Change>, +thread: String, +cwd: String) -> List<&2, Out>:
  match cs:
    case Nil{}:
      Nil{}
    case M.ActivityLogged{_, +id, +tone, _, _, _} <> t:
      List.append(&2, Out, Bool.pick(List<&2, Out>, String.eq(tone, "tool"), [Steps.out(thread, cwd, id)], Nil{}), Steps.acts(t, thread, cwd))
    case _ <> t:
      Steps.acts(t, thread, cwd)

def Steps.after(st: Step, +thread: String, +cwd: String) -> Step:
  match st:
    case Step{+cs, os}:
      Step{cs, List.append(&2, Out, os, Steps.acts(cs, thread, cwd))}

# Grok's calls are logged before they run: its step waits for the call's
# end (acp.bend's "tool.done")
def Steps.after.if(on: Bool, st: Step, +thread: String, +cwd: String) -> Step:
  match on:
    case True{}:
      Steps.after(st, thread, cwd)
    case False{}:
      st

# A subagent's own lines (parent_tool_use_id names the Agent call that
# started it): what it writes and the tools it calls go to its log, not the
# thread's timeline, and its partial text, tool results and usage are not
# the agent's
def Sub.block.k(i: Nat, +log: String, +call: String, +msg: String, +n: Nat, +now: Nat, +b: J.Json) -> List<&2, M.Change>:
  match i:
    case 0n:
      [M.ActivityLogged{log, call ++ "/" ++ msg ++ ":" ++ Nat.show(n), call, "say", J.Json.get_str(b, "text", ""), now}]
    case 1n:
      [M.ActivityLogged{log, call ++ "/" ++ J.Json.get_str(b, "id", msg ++ ":" ++ Nat.show(n)), call, J.Json.get_str(b, "name", "tool"),
        Claude.tool_summary(J.Json.get_str(b, "name", "tool"), J.Json.get_obj(b, "input")), now}]
    case _:
      Nil{}

def Sub.blocks(bs: List<&2, J.Json>, +log: String, +call: String, +msg: String, +n: Nat, +now: Nat) -> List<&2, M.Change>:
  match bs:
    case Nil{}:
      Nil{}
    case +b <> t:
      List.append(&2, M.Change,
        Sub.block.k(C.Codec.pos(["text", "tool_use"], J.Json.get_str(b, "type", "")), log, call, msg, n, now, b),
        Sub.blocks(t, log, call, msg, 1n+n, now))

def Sub.line(i: Nat, +thread: String, +now: Nat, +call: String, +o: J.Json) -> Step:
  match i:
    case 2n:
      Step{Sub.blocks(J.Json.elems(J.Json.get_obj(J.Json.get_obj(o, "message"), "content")), M.Bg.log(thread), call,
        J.Json.get_str(J.Json.get_obj(o, "message"), "id", "msg"), 0n, now), Nil{}}
    case _:
      Step.none()

def Hub.provider(t: M.Thread) -> String:
  match t:
    case M.Thread{_, _, _, _, _, _, _, _, _, _, _, _, _, _, _, p, _, _, _, _, _}:
      p

def Hub.running(t: M.Thread) -> Bool:
  match t:
    case M.Thread{_, _, _, _, _, M.Running{}, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _}:
      True{}
    case _:
      False{}

def Claude.line.k(i: Nat, +s: M.State, +thread: String, +now: Nat, +o: J.Json) -> Step:
  match i:
    case 0n:
      Step.after(Step.after(Claude.init(s, thread, now, o), Usage.compact(s, thread, now, o)), Bg.system(s, thread, now, o))
    case 1n:
      Claude.stream.if(Hub.running(Hub.thread(s, thread)), thread, J.Json.get_obj(o, "event"))
    case 2n:
      Claude.assistant.t(s, thread, now, J.Json.get_obj(o, "message"), J.Json.get_str(o, "uuid", ""))
    case 3n:
      Step.after(Claude.result.if(Hub.running(Hub.thread(s, thread)), thread, now, o), Usage.window(s, thread, now, o))
    case 4n:
      Step.also(Claude.named(s, thread, now, o), Steps.claude(J.Json.elems(J.Json.get_obj(J.Json.get_obj(o, "message"), "content")), thread, Hub.cwd(s, Hub.thread(s, thread))))
    case _:
      Step.none()

def Claude.line.who(main: Bool, i: Nat, +s: M.State, +thread: String, +now: Nat, +call: String, +o: J.Json) -> Step:
  match main:
    case True{}:
      Claude.line.k(i, s, thread, now, o)
    case False{}:
      Sub.line(i, thread, now, call, o)

def Claude.line.parsed(m: Maybe<&2, J.Json>, +s: M.State, +thread: String, +now: Nat) -> Step:
  match m:
    case Some{+o}:
      +call = J.Json.get_str(o, "parent_tool_use_id", "")
      Claude.line.who(String.is_empty(call), C.Codec.pos(["system", "stream_event", "assistant", "result", "user"], J.Json.get_str(o, "type", "")), s, thread, now, call, o)
    case None{}:
      Step.none()

# first: the thread's first worktree (not one made again), whose waiting
# message names its branch
def Wt.start(+s: M.State, +now: Nat, +thread: String, +first: Bool) -> Step:
  Wt.start.k(Bool.and(Bool.not(Hub.running(Hub.thread(s, thread))), M.Queue.waiting(s, thread)), first,
    Hub.thread(s, thread), Hub.cwd(s, Hub.thread(s, thread)), M.Queue.of(s, thread), now, Hub.seq(s), thread)

def Wt.ready(+s: M.State, +now: Nat, +client: U32, +id: String, +thread: String, +c: M.Change) -> Step:
  Step.before([c], [Reply.ok(client, id)], Wt.start(M.State.replay([c], s), now, thread, String.is_empty(M.Merge.branch(Hub.thread(s, thread)))))

def Hub.agentm(i: Nat, +s: M.State, +now: Nat, +client: U32, +id: String, +p: J.Json, +m: String) -> Step:
  match i:
    case 0n:
      Step.reply(client, id, [M.WorktreeSet{J.Json.get_str(p, "thread", ""), J.Json.get_str(p, "branch", ""), Hub.worktree(Hub.thread(s, J.Json.get_str(p, "thread", ""))), now}])
    case 1n:
      Hub.git(s, client, id, "skills", J.Json.get_str(p, "thread", ""), "", "")
    case 2n:
      Hub.effort(s, now, client, id, J.Json.get_str(p, "thread", ""), J.Json.get_str(p, "effort", ""))
    case 3n:
      Wt.ready(s, now, client, id, J.Json.get_str(p, "thread", ""), M.WorktreeSkipped{J.Json.get_str(p, "thread", ""), now})
    case 4n:
      Merge.found(M.Merge.due(s, Hub.thread(s, J.Json.get_str(p, "thread", ""))), s, now, client, id, J.Json.get_str(p, "thread", ""), J.Json.get_str(p, "url", ""))
    case 5n:
      Step.reply(client, id, [M.WorktreeRemoved{J.Json.get_str(p, "thread", ""), now}])
    case 6n:
      Title.got(s, now, client, id, J.Json.get_str(p, "thread", ""), p)
    case 7n:
      Hub.btw(s, client, id, J.Json.get_str(p, "thread", ""), J.Json.get_str(p, "q", ""))
    case 8n:
      Sum.got(s, now, client, id, p)
    case _:
      Hub.media(C.Codec.pos(Hub.methods.media(), m), s, now, client, id, p, m)

def Hub.dir.k(ok: Bool, +client: U32, +id: String, +th: String, +cwd: String, +path: String) -> Step:
  match ok:
    case True{}:
      Step.outs([Git{client, id, "dir", th, cwd, path, ""}])
    case False{}:
      Step.outs([Reply.err(client, id, "that folder is not in this project")])

def Hub.nav(i: Nat, +s: M.State, +now: Nat, +client: U32, +id: String, +p: J.Json, +m: String) -> Step:
  match i:
    case 0n:
      Hub.snooze(now, client, id, p)
    case 1n:
      Hub.reorder(s, now, client, id, p)
    case 2n:
      Step.outs([Git{client, id, "files", J.Json.get_str(p, "thread", ""), Hub.cwd(s, Hub.thread(s, J.Json.get_str(p, "thread", ""))), "", ""}])
    case 3n:
      Step.outs([Git{client, id, "kicad", J.Json.get_str(p, "thread", ""), Hub.cwd(s, Hub.thread(s, J.Json.get_str(p, "thread", ""))), "", ""}])
    case 4n:
      Hub.reorder_projects(s, now, client, id, p)
    case 5n:
      Step.outs([Git{client, id, "mech", J.Json.get_str(p, "thread", ""), Hub.cwd(s, Hub.thread(s, J.Json.get_str(p, "thread", ""))), "", ""}])
    case 6n:
      Step.outs([Git{client, id, "render", J.Json.get_str(p, "thread", ""), Hub.cwd(s, Hub.thread(s, J.Json.get_str(p, "thread", ""))), J.Json.get_str(p, "path", ""), ""}])
    # a file link opened on this machine, or shown in its folder: {path, how}
    case 7n:
      Step.outs([Git{client, id, "open", "", "/", J.Json.get_str(p, "path", ""), J.Json.get_str(p, "how", "open")}])
    # the viewer's Files tab: a folder of the thread's project, {thread, path}
    # (from its root, never leaving it: core/fm.bend's Fm.safe)
    case 8n:
      Hub.dir.k(FM.Fm.safe(J.Json.get_str(p, "path", "")), client, id, J.Json.get_str(p, "thread", ""), Hub.cwd(s, Hub.thread(s, J.Json.get_str(p, "thread", ""))), J.Json.get_str(p, "path", ""))
    # a PDF's page as a picture, for the window, {path, page}
    case 9n:
      Step.outs([Git{client, id, "pdf", "", "/", J.Json.get_str(p, "path", ""), J.Json.as_raw(J.Json.get(p, "page"), "1")}])
    case _:
      Hub.agentm(C.Codec.pos(Hub.methods.agent(), m), s, now, client, id, p, m)

def Hub.fs(i: Nat, +s: M.State, +now: Nat, +client: U32, +id: String, +p: J.Json, +m: String) -> Step:
  match i:
    case 0n:
      Step.outs([Fs{client, id, "list", J.Json.get_str(p, "path", "")}])
    case 1n:
      Step.outs([Fs{client, id, "mkdir", J.Json.get_str(p, "path", "")}])
    case 2n:
      Step.outs([Fs{client, id, "new", J.Json.get_str(p, "path", "")}])
    case 3n:
      Hub.project_add(s, now, client, id, p)
    case 4n:
      Step.outs([Fs{client, id, "find", ""}])
    case _:
      Hub.nav(C.Codec.pos(Hub.methods.nav(), m), s, now, client, id, p, m)

def Hub.more(i: Nat, +s: M.State, +now: Nat, +client: U32, +id: String, +p: J.Json, +m: String) -> Step:
  match i:
    case 0n:
      Hub.git(s, client, id, "diff", J.Json.get_str(p, "thread", ""), J.Json.as_raw(J.Json.get(p, "from"), "0"), J.Json.as_raw(J.Json.get(p, "to"), "-1"))
    case 1n:
      Hub.git(s, client, id, "revert", J.Json.get_str(p, "thread", ""), J.Json.as_raw(J.Json.get(p, "turn"), "0"), "")
    case 2n:
      Hub.act(s, client, id, J.Json.get_str(p, "thread", ""), J.Json.get_str(p, "action", "commit"), p)
    case 3n:
      Hub.git(s, client, id, "status", J.Json.get_str(p, "thread", ""), "", "")
    case 4n:
      Wt.ready(s, now, client, id, J.Json.get_str(p, "thread", ""), M.WorktreeSet{J.Json.get_str(p, "thread", ""), J.Json.get_str(p, "branch", ""), J.Json.get_str(p, "path", ""), now})
    case 5n:
      Step.reply(client, id, [M.ActivityLogged{J.Json.get_str(p, "thread", ""), Hub.id("g", now, Hub.seq(s)), J.Json.get_str(p, "tone", "info"),
        J.Json.get_str(p, "kind", "git"), J.Json.get_str(p, "summary", ""), now}])
    case 6n:
      Hub.term(s, client, id, "open", p)
    case 7n:
      Hub.term(s, client, id, "input", p)
    case 8n:
      Hub.term(s, client, id, "resize", p)
    case 9n:
      Step.outs([Reply.ok(client, id), TermSnap{client, J.Json.get_str(p, "key", "")}])
    case 10n:
      Emb.restart(s, now, client, id, J.Json.get_str(p, "thread", ""), J.Json.get_str(p, "id", ""))
    case _:
      Hub.fs(C.Codec.pos(Hub.methods.fs(), m), s, now, client, id, p, m)

# a message for a thread's agent: it starts a turn, or, while a Codex or
# Grok turn runs (one turn at a time) or the thread's worktree is still
# being made, it is stored and waits in the thread's queue (Claude takes
# input mid-turn itself)
def Hub.thread_project(t: M.Thread) -> String:
  match t:
    case M.Thread{_, p, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _}:
      p

# the worktree made again from a thread's kept branch
def Wt.revive(+s: M.State, +th: String) -> Out:
  Git{0, "", "worktree", th, Hub.project.root(Hub.project(s, Hub.thread_project(Hub.thread(s, th)))), "again", M.Merge.branch(Hub.thread(s, th))}

# whether a message makes a thread's removed worktree again first: the
# thread is idle, its worktree is gone, and it is not being made already
def Hub.revive(+running: Bool, gone: Bool, +pending: Bool) -> Bool:
  Bool.and(Bool.not(running), Bool.and(gone, Bool.not(pending)))

def Hub.post.v(revive: Bool, +running: Bool, +pending: Bool, +s: M.State, +now: Nat, +thread: String, +msg: String, role: M.Role, +text: String) -> Step:
  match revive:
    case True{}:
      Step.before(Nil{}, [Wt.revive(s, thread)], Hub.post.go(True{}, Hub.thread(s, thread), Hub.cwd(s, Hub.thread(s, thread)), M.Queue.of(s, thread), now, thread, msg, role, text))
    case False{}:
      Hub.post.go(Bool.or(Bool.and(running, Bool.not(String.eq(Hub.provider(Hub.thread(s, thread)), "claude"))), pending),
        Hub.thread(s, thread), Hub.cwd(s, Hub.thread(s, thread)), M.Queue.of(s, thread), now, thread, msg, role, text)

def Hub.post.w(+running: Bool, gone: Bool, +pending: Bool, +s: M.State, +now: Nat, +thread: String, +msg: String, role: M.Role, +text: String) -> Step:
  Hub.post.v(Hub.revive(running, gone, pending), running, pending, s, now, thread, msg, role, text)

def Hub.post(+s: M.State, +now: Nat, +thread: String, +msg: String, role: M.Role, +text: String) -> Step:
  Hub.post.w(Hub.running(Hub.thread(s, thread)), M.Worktree.gone(Hub.thread(s, thread)), M.Worktree.pending(Hub.thread(s, thread), now), s, now, thread, msg, role, text)

# a message from another thread (thread_send, a delegated task's result):
# it lands in the target's inbox and waits while the target works, for
# every provider (it never interrupts a running turn, Claude's included);
# its id names the sender, which is what the link visuals read
def Hub.inbox.v(revive: Bool, +running: Bool, +pending: Bool, +s: M.State, +now: Nat, +from: String, +to: String, role: M.Role, +text: String, +msg: String) -> Step:
  match revive:
    case True{}:
      Step.before(Nil{}, [Wt.revive(s, to)], Hub.post.go(True{}, Hub.thread(s, to), Hub.cwd(s, Hub.thread(s, to)), M.Queue.of(s, to), now, to,
        msg, role, text))
    case False{}:
      Hub.post.go(Bool.or(running, pending), Hub.thread(s, to), Hub.cwd(s, Hub.thread(s, to)), M.Queue.of(s, to), now, to,
        msg, role, text)

def Hub.inbox.w(+running: Bool, gone: Bool, +pending: Bool, +s: M.State, +now: Nat, +from: String, +to: String, role: M.Role, +text: String, +msg: String) -> Step:
  Hub.inbox.v(Hub.revive(running, gone, pending), running, pending, s, now, from, to, role, text, msg)

# the same under a message id of the caller's (a delegated task's result:
# Task.finish.go, so a delivery torn by a crash is found again, Task.repair)
def Hub.inbox.id(+s: M.State, +now: Nat, +from: String, +to: String, role: M.Role, +text: String, +msg: String) -> Step:
  Hub.inbox.w(Hub.running(Hub.thread(s, to)), M.Worktree.gone(Hub.thread(s, to)), M.Worktree.pending(Hub.thread(s, to), now), s, now, from, to, role, text, msg)

def Hub.inbox(+s: M.State, +now: Nat, +from: String, +to: String, role: M.Role, +text: String) -> Step:
  Hub.inbox.id(s, now, from, to, role, text, M.Link.id(from, Hub.id("m", now, Hub.seq(s))))

def Hub.answer.plan(impl: Bool, +s: M.State, +now: Nat, +client: U32, +id: String, +ask: String, +answer: String, +th: String) -> Step:
  match impl:
    case True{}:
      Step.before(M.AskClosed{ask, answer, now} <> Hub.implement.k(Hub.thread(s, th), now), [Reply.ok(client, id)],
        Hub.post(Hub.impl.state(s, th, now), now, th, Hub.id("m", now, Hub.seq(s)), M.User{}, "Implement the proposed plan."))
    case False{}:
      Step{[M.AskClosed{ask, answer, now}], [Reply.ok(client, id), AskAnswered{ask, answer}]}

# the user's answer: the ask closes (once; law ask_closes_once), a parked
# agent request gets its reply, and implementing a plan leaves plan mode
# and asks the agent to carry it out
def Hub.answer.k(a: M.Ask, +s: M.State, +now: Nat, +client: U32, +id: String, +ask: String, +answer: String) -> Step:
  match a:
    case M.Ask{_, +th, +k, _, _, _, True{}, _}:
      Hub.answer.plan(Bool.and(String.eq(k, "plan"), String.eq(answer, "implement")), s, now, client, id, ask, answer, th)
    case M.Ask{_, _, _, _, _, _, False{}, _}:
      Step.outs([Reply.ok(client, id)])

def Hub.answer(+s: M.State, +now: Nat, +client: U32, +id: String, +ask: String, +answer: String) -> Step:
  Hub.answer.k(Ask.find(M.Asks.all(s), ask), s, now, client, id, ask, answer)

def Hub.turn.go(+s: M.State, +t: M.Thread, +now: Nat, +client: U32, +id: String, +thread: String, +msg: String, +text: String) -> Step:
  match t:
    case M.Thread{_, _, +title, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _}:
      Step.before(Hub.title.k(String.eq(title, "New thread"), thread, text, now, Nil{}),
        Reply.ok(client, id) <> List.append(&2, Out, Name.out(Hub.thread(s, thread), text), Title.out(String.eq(title, "New thread"), thread, text)),
        Hub.post(s, now, thread, msg, M.User{}, text))

def Hub.turn.k(dup: Bool, +s: M.State, +now: Nat, +client: U32, +id: String, +thread: String, +msg: String, +text: String) -> Step:
  match dup:
    case True{}:
      Step.outs([Reply.ok(client, id)])
    case False{}:
      Hub.turn.go(s, Hub.thread(s, thread), now, client, id, thread, msg, text)

def Hub.turn.m(+s: M.State, +now: Nat, +client: U32, +id: String, +thread: String, +text: String, +msg: String, named: Bool) -> Step:
  Hub.turn.k(Hub.dup(named, s, msg), s, now, client, id, thread, msg, text)

# a user message. The client names it (msg), so a message resent after a
# dropped connection is stored once; the hub names it otherwise
def Hub.turn(+s: M.State, +now: Nat, +client: U32, +id: String, +thread: String, +text: String, +msg: String) -> Step:
  Hub.turn.m(s, now, client, id, thread, text, Bool.pick(String, String.is_empty(msg), Hub.id("m", now, Hub.seq(s)), msg), Bool.not(String.is_empty(msg)))

# Follow-ups: queue or steer
# --------------------------
# A message sent while a turn runs either waits in the queue for a later
# turn or steers the running one. Claude takes it in place, on its input;
# a Codex or Grok turn cannot, so its process stops and the turn starts
# again on the same session with the steering message (law
# steer_codex_restarts). With no turn running, either starts at once.
# "auto" (or no mode) keeps the provider's default: Claude steers, the
# others queue.

def Steer.stop(+provider: String, +thread: String) -> List<&2, Out>:
  Bool.pick(List<&2, Out>, String.eq(provider, "claude"), Nil{}, [AgentStop{thread}])

def Steer.note(+provider: String, +thread: String, +now: Nat, +seq: Nat) -> List<&2, M.Change>:
  Bool.pick(List<&2, M.Change>, String.eq(provider, "claude"), Nil{},
    [M.ActivityLogged{thread, Hub.id("s", now, seq), "info", "steer", "Steered: the turn starts again with this message", now}])

# the running turn takes `text` now (the message itself is already posted)
def Steer.go(+t: M.Thread, +cwd: String, +now: Nat, +seq: Nat, +thread: String, +text: String) -> Step:
  Step{List.append(&2, M.Change, Steer.note(Hub.provider(t), thread, now, seq), [M.TurnChanged{thread, M.Running{}, now}]),
    List.append(&2, Out, Steer.stop(Hub.provider(t), thread), Hub.run(t, cwd, text, Hub.uuid(now, thread ++ "/s/" ++ Nat.show(seq))))}

def Hub.post.q(run: Bool, +s: M.State, +now: Nat, +thread: String, +msg: String, role: M.Role, +text: String) -> Step:
  match run:
    case True{}:
      Step{[M.MessagePosted{thread, msg, role, text, now}, M.TurnQueued{thread, msg, text, now}], Nil{}}
    case False{}:
      Hub.post(s, now, thread, msg, role, text)

def Hub.post.st(run: Bool, +s: M.State, +now: Nat, +thread: String, +msg: String, role: M.Role, +text: String) -> Step:
  match run:
    case True{}:
      Step.before([M.MessagePosted{thread, msg, role, text, now}], Nil{}, Steer.go(Hub.thread(s, thread), Hub.cwd(s, Hub.thread(s, thread)), now, Hub.seq(s), thread, text))
    case False{}:
      Hub.post(s, now, thread, msg, role, text)

def Hub.post.mode.k(i: Nat, +s: M.State, +now: Nat, +thread: String, +msg: String, role: M.Role, +text: String) -> Step:
  match i:
    case 0n:
      Hub.post.q(Hub.running(Hub.thread(s, thread)), s, now, thread, msg, role, text)
    case 1n:
      Hub.post.st(Hub.running(Hub.thread(s, thread)), s, now, thread, msg, role, text)
    case _:
      Hub.post(s, now, thread, msg, role, text)

# a message with its follow-up mode: "queue", "steer", or "auto"
def Hub.post.mode(+mode: String, +s: M.State, +now: Nat, +thread: String, +msg: String, role: M.Role, +text: String) -> Step:
  Hub.post.mode.k(C.Codec.pos(["queue", "steer"], mode), s, now, thread, msg, role, text)

def Hub.turnx.go(+mode: String, +s: M.State, +t: M.Thread, +now: Nat, +client: U32, +id: String, +thread: String, +msg: String, +text: String) -> Step:
  match t:
    case M.Thread{_, _, +title, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _}:
      Step.before(Hub.title.k(String.eq(title, "New thread"), thread, text, now, Nil{}),
        Reply.ok(client, id) <> List.append(&2, Out, Name.out(Hub.thread(s, thread), text), Title.out(String.eq(title, "New thread"), thread, text)),
        Hub.post.mode(mode, s, now, thread, msg, M.User{}, text))

def Hub.turnx.k(dup: Bool, +mode: String, +s: M.State, +now: Nat, +client: U32, +id: String, +thread: String, +msg: String, +text: String) -> Step:
  match dup:
    case True{}:
      Step.outs([Reply.ok(client, id)])
    case False{}:
      Hub.turnx.go(mode, s, Hub.thread(s, thread), now, client, id, thread, msg, text)

# a user message with an explicit follow-up mode (Hub.turn is "auto")
def Hub.turnx(+mode: String, +s: M.State, +now: Nat, +client: U32, +id: String, +thread: String, +text: String, +msg: String) -> Step:
  Hub.turnx.k(Hub.dup(Bool.not(String.is_empty(msg)), s, msg), mode, s, now, client, id, thread,
    Bool.pick(String, String.is_empty(msg), Hub.id("m", now, Hub.seq(s)), msg), text)

def Hub.turn.mode(auto: Bool, +mode: String, +s: M.State, +now: Nat, +client: U32, +id: String, +thread: String, +text: String, +msg: String) -> Step:
  match auto:
    case True{}:
      Hub.turn(s, now, client, id, thread, text, msg)
    case False{}:
      Hub.turnx(mode, s, now, client, id, thread, text, msg)

# The queue, message by message
# -----------------------------

def Queue.steer.k(m: Maybe<&2, M.QItem>, +run: Bool, +s: M.State, +now: Nat, +client: U32, +id: String, +thread: String) -> Step:
  match m run:
    case Some{M.QItem{+msg, +text}} True{}:
      Step.before([M.QueueTakenOne{thread, msg, now}], [Reply.ok(client, id)], Steer.go(Hub.thread(s, thread), Hub.cwd(s, Hub.thread(s, thread)), now, Hub.seq(s), thread, text))
    case Some{M.QItem{+msg, +text}} False{}:
      Step{[M.QueueTakenOne{thread, msg, now}, M.TurnChanged{thread, M.Running{}, now}],
        Reply.ok(client, id) <> Git{0, "", "checkpoint0", thread, Hub.cwd(s, Hub.thread(s, thread)), "", ""} <> Git{0, "", "step", thread, Hub.cwd(s, Hub.thread(s, thread)), "", ""} <> Hub.run(Hub.thread(s, thread), Hub.cwd(s, Hub.thread(s, thread)), text, Hub.uuid(now, thread ++ "/" ++ msg))}
    case None{} _:
      Step.outs([Reply.err(client, id, "no such queued message")])

# a queued message sent now: into the running turn (a steer), or as a turn
# of its own when none runs
def Queue.steer(+s: M.State, +now: Nat, +client: U32, +id: String, +thread: String, +msg: String) -> Step:
  Queue.steer.k(M.Queue.find(M.Queue.items(s, thread), msg), Hub.running(Hub.thread(s, thread)), s, now, client, id, thread)

def Queue.op(i: Nat, +s: M.State, +now: Nat, +client: U32, +id: String, +p: J.Json) -> Step:
  match i:
    case 0n:
      Step.reply(client, id, [M.QueueEdited{J.Json.get_str(p, "thread", ""), J.Json.get_str(p, "msg", ""), J.Json.get_str(p, "text", ""), now}])
    case 1n:
      Step.reply(client, id, [M.QueueRemoved{J.Json.get_str(p, "thread", ""), J.Json.get_str(p, "msg", ""), now}])
    case 2n:
      Step.reply(client, id, [M.QueueMoved{J.Json.get_str(p, "thread", ""), J.Json.get_str(p, "msg", ""), Maybe.default(&2, Nat, Nat.read(J.Json.as_raw(J.Json.get(p, "to"), "0")), 0n), now}])
    case _:
      Queue.steer(s, now, client, id, J.Json.get_str(p, "thread", ""), J.Json.get_str(p, "msg", ""))

# Forks
# -----
# A fork is a new thread that continues another from where it stands (V2's
# thread lineage). It records its source (ThreadForked), works in its own
# worktree made from the source's last checkpoint when the source has one,
# and carries the source's conversation as handed-over context, queued
# ahead of its first message (HD.Hand). Bringing a fork back queues the
# fork's conversation into its source the same way, for the source's next
# message; the fork's files stay on its own branch.

def Fork.env(+wt: String) -> M.EnvMode:
  Bool.pick(M.EnvMode, String.is_empty(wt), M.Local{}, M.Worktree{})

def Fork.outs(+wt: String, +th: String, +root: String, +src: String) -> List<&2, Out>:
  Bool.pick(List<&2, Out>, String.is_empty(wt), Nil{}, [Git{0, "", "worktree", th, root, "fork", src}])

# a scratch thread's fork is a scratch thread with a folder of its own
def Fork.env.s(scratch: Bool, +wt: String) -> M.EnvMode:
  match scratch:
    case True{}:
      M.Local{}
    case False{}:
      Fork.env(wt)

def Fork.outs.s(scratch: Bool, +wt: String, +th: String, +root: String, +src: String) -> List<&2, Out>:
  match scratch:
    case True{}:
      [BotFx{"scratch", th, "", ""}]
    case False{}:
      Fork.outs(wt, th, root, src)

def Hub.fork.k(t: M.Thread, +s: M.State, +now: Nat, +client: U32, +id: String, +src: String) -> Step:
  match t:
    case M.Thread{_, +p, +ti, _, _, _, _, _, _, _, _, _, _, _, +wt, +pr, +mo, rt, im, _, _}:
      +th = Hub.id("t", now, Hub.seq(s))
      Step{[M.ThreadCreated{th, p, "Fork: " ++ ti, Fork.env.s(M.Scratch.is(p), wt), "", Hub.new.wt(M.Scratch.is(p), Hub.project.root(Hub.project(s, p)), th), pr, mo, now}, M.ModesSet{th, rt, im, mo, now}, M.ThreadForked{th, src, now},
          M.TurnQueued{th, HD.Hand.prefix() ++ src, HD.Hand.fork(Hub.entries(s), src, ti), now},
          M.ActivityLogged{src, Hub.id("k", now, Hub.seq(s)), "info", "fork", "Forked into \"Fork: " ++ ti ++ "\"", now}],
        Reply.made(client, id, th) <> Fork.outs.s(M.Scratch.is(p), wt, th, Hub.project.root(Hub.project(s, p)), src)}

# a new thread that goes on from this one
def Hub.fork(+s: M.State, +now: Nat, +client: U32, +id: String, +src: String) -> Step:
  Hub.fork.k(Hub.thread(s, src), s, now, client, id, src)

def Hub.back.k(none: Bool, +s: M.State, +now: Nat, +client: U32, +id: String, +fork: String, +src: String) -> Step:
  match none:
    case True{}:
      Step.outs([Reply.err(client, id, "this thread is not a fork")])
    case False{}:
      Step.reply(client, id, [M.TurnQueued{src, HD.Hand.prefix() ++ "back:" ++ fork ++ ":" ++ Nat.show(Hub.seq(s)), HD.Hand.back(Hub.entries(s), fork, HD.Hand.title(Hub.thread(s, fork))), now},
        M.ActivityLogged{src, Hub.id("k", now, Hub.seq(s)), "info", "fork", "The work of \"" ++ HD.Hand.title(Hub.thread(s, fork)) ++ "\" goes with the next message", now},
        M.ActivityLogged{fork, Hub.id("k", now, 1n+Hub.seq(s)), "info", "fork", "Brought back to \"" ++ HD.Hand.title(Hub.thread(s, src)) ++ "\"", now}])

# a fork's work, queued into its source for the source's next message
def Hub.back(+s: M.State, +now: Nat, +client: U32, +id: String, +fork: String) -> Step:
  Hub.back.k(String.is_empty(M.Fork.source(s, fork)), s, now, client, id, fork, M.Fork.source(s, fork))

# Switching providers
# -------------------
# Between turns a thread may go on with another provider (and its default
# model). The provider left keeps its session and how much it saw; the one
# taken up resumes its own session if it had one, and gets what it missed
# as handed-over context with the next message (a full handoff when it
# never took part). A context from an earlier switch not sent yet is
# dropped for the new one. A running turn is never switched (law
# switch_between_turns).

def Switch.old(qs: List<&2, M.QItem>, +th: String, +now: Nat) -> List<&2, M.Change>:
  match qs:
    case Nil{}:
      Nil{}
    case M.QItem{+m, _} <> t:
      List.append(&2, M.Change, Bool.pick(List<&2, M.Change>, String.starts_with(m, "handoff:switch:"), [M.QueueRemoved{th, m, now}], Nil{}), Switch.old(t, th, now))

def Switch.ctx(none: Bool, +th: String, +msg: String, +text: String, +now: Nat) -> List<&2, M.Change>:
  match none:
    case True{}:
      Nil{}
    case False{}:
      [M.TurnQueued{th, msg, text, now}]

def Switch.go(+s: M.State, +now: Nat, +client: U32, +id: String, +th: String, +from: String, +to: String) -> Step:
  +seen = M.Switch.seen(s, th, to)
  +missed = Nat.sub(M.Switch.count(s, th), seen)
  Step{List.concat(&2, M.Change, [Switch.old(M.Queue.items(s, th), th, now), [M.ProviderSet{th, to, "", now}],
      Switch.ctx(Nat.is_eq(missed, 0n), th, HD.Hand.prefix() ++ "switch:" ++ Nat.show(Hub.seq(s)), HD.Hand.switch(Hub.entries(s), th, from, to, seen), now),
      [M.ActivityLogged{th, Hub.id("p", now, Hub.seq(s)), "info", "provider", "Now on " ++ to ++ Bool.pick(String, Nat.is_eq(missed, 0n), "", "; it gets what it missed with the next message"), now}]]),
    [Reply.ok(client, id), AgentStop{th}]}

def Switch.k(run: Bool, same: Bool, +s: M.State, +now: Nat, +client: U32, +id: String, +th: String, +from: String, +to: String) -> Step:
  match run same:
    case True{} _:
      Step.outs([Reply.err(client, id, "a provider changes between turns: stop this one first")])
    case False{} True{}:
      Step.outs([Reply.ok(client, id)])
    case False{} False{}:
      Switch.go(s, now, client, id, th, from, to)

# the thread goes on with provider `to` from its next turn
def Hub.switch(+s: M.State, +now: Nat, +client: U32, +id: String, +th: String, +to: String) -> Step:
  Switch.k(Hub.running(Hub.thread(s, th)), String.eq(Hub.provider(Hub.thread(s, th)), to), s, now, client, id, th, Hub.provider(Hub.thread(s, th)), to)

# Stopping a turn
# ---------------
# Claude takes an interrupt on its input and stays up; a Codex or Grok turn
# is a process of its own, which is stopped. Only a running turn is
# stopped (law interrupt_idle_nothing).

def Stop.outs(+provider: String, +thread: String, +rid: String) -> List<&2, Out>:
  Bool.pick(List<&2, Out>, String.eq(provider, "claude"), [AgentSend{thread, Claude.interrupt(rid)}], [AgentStop{thread}])

# the held queue of an idle thread (kept across a restart, or after a
# stop) starts now
def Hub.resume(+s: M.State, +now: Nat, +client: U32, +id: String, +thread: String) -> Step:
  Step.before(Nil{}, [Reply.ok(client, id)], Wt.start(s, now, thread, False{}))

# why an agent's process ended, in words: a SIGKILL (137) is most likely
# the kernel taking it for memory; any other end says its plain exit code
# ("" here). Backplane picks the thread back up either way.
def Exit.why(+code: U32) -> String:
  Bool.pick(String, U32.is_eq(code, 137), "agent was killed (signal 9, likely out of memory)", "")

# why: what the server learned of the exit (Exit.why); "" says the plain
# exit code
def Hub.agent_gone.k(running: Bool, +thread: String, +now: Nat, +code: U32, +why: String) -> Step:
  match running:
    case True{}:
      Step{[M.ActivityLogged{thread, Hub.id("x", now, 0n), "error", "exit", Bool.pick(String, String.is_empty(why), "agent exited with code " ++ U32.show(code), why), now},
        M.TurnChanged{thread, M.Failed{}, now}], Nil{}}
    case False{}:
      Step.none()

def Hub.settle.k(auto: Bool, sh: M.Shelf, t: M.Thread, +now: Nat, rest: List<&2, M.Change>) -> List<&2, M.Change>:
  match auto sh t:
    case True{} M.Active{} M.Thread{id, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _}:
      M.ThreadSettled{id, False{}, now} <> rest
    case _ _ _:
      rest

# Settle tick
# -----------

# whether a thread has a delegated task still at work: its parent waits
# for it, so it does not settle (law settle_waits_for_children)
def Kids.busy(ts: List<&2, M.Task>, +parent: String) -> Bool:
  match ts:
    case Nil{}:
      False{}
    case M.Task{_, p, _, _, st, _, _, _, _} <> t:
      Bool.or(Bool.and(String.eq(p, parent), Bool.not(M.TaskState.terminal(st))), Kids.busy(t, parent))

# (kids: every parent's unfinished child tasks, M.Work.kids.map; a thread
# with any work out, children, background or watched, does not settle:
# law settle_waits_for_work)
def Hub.settle.go(ts: List<&2, M.Thread>, +navs: Map<&2, M.Nav>, +kids: Map<&2, Nat>, +s: M.State, +now: Nat, +w: Nat) -> List<&2, M.Change>:
  match ts:
    case Nil{}:
      Nil{}
    case +t <> rest:
      Hub.settle.k(Bool.and(M.Settle.auto.at(now, w, t, M.Nav.until(M.Nav.get(navs, M.Thread.id(t)))), Nat.is_eq(M.Work.busy.at(kids, s, M.Thread.id(t)), 0n)), M.Shelf.of(t), t, now, Hub.settle.go(rest, navs, kids, s, now, w))

# the settle window in seconds: the "settle.days" setting (1..90), else 3 days
def Hub.settle.window(+s: M.State) -> Nat:
  Nat.mul(Nat.max(1n, Nat.min(90n, Maybe.default(&2, Nat, Nat.read(M.Setting.get(s, "settle.days", "3")), 3n))), 86400n)

# Merges
# ------
# Each tick, a project with threads whose merge is due asks gh which of
# their branches were merged (one call per project); the server answers
# thread.merged for each.

def Merge.line.k(due: Bool, +in: Bool, +t: M.Thread, rest: List<&2, String>) -> List<&2, String>:
  match due in:
    case True{} True{}:
      (M.Merge.id(t) ++ "\t" ++ M.Merge.branch(t) ++ "\t" ++ Hub.worktree(t)) <> rest
    case _ _:
      rest

# "<thread>\t<branch>\t<worktree>" for each of a project's threads whose
# merge is due
def Merge.lines(ts: List<&2, M.Thread>, +s: M.State, +pid: String) -> List<&2, String>:
  match ts:
    case Nil{}:
      Nil{}
    case +t <> rest:
      Merge.line.k(M.Merge.due(s, t), M.Thread.in_project(pid, t), t, Merge.lines(rest, s, pid))

def Merge.ask.k(ls: List<&2, String>, +root: String, rest: List<&2, Out>) -> List<&2, Out>:
  match ls:
    case Nil{}:
      rest
    case +l <> t:
      Git{0, "", "merged", "", root, String.join(l <> t, "\n"), ""} <> rest

def Merge.ask.p(removed: Bool, +ts: List<&2, M.Thread>, +s: M.State, +id: String, +root: String, rest: List<&2, Out>) -> List<&2, Out>:
  match removed:
    case True{}:
      rest
    case False{}:
      Merge.ask.k(Merge.lines(ts, s, id), root, rest)

def Merge.asks(ps: List<&2, M.Project>, +ts: List<&2, M.Thread>, +s: M.State) -> List<&2, Out>:
  match ps:
    case Nil{}:
      Nil{}
    case M.Project{+id, _, +root, _, rm} <> rest:
      Merge.ask.p(rm, ts, s, id, root, Merge.asks(rest, ts, s))

# gh's merged pull requests (gh pr list --json headRefName,headRefOid,url)
# against "<thread>\t<branch>[\t<worktree>[\t<commits>]]" lines, the
# commits (space separated) being the ones the thread's worktree has been on
# (its HEAD reflog): each thread whose branch was merged, with the pull
# request's url. A pull request is the thread's when its head branch is the
# thread's branch, or its head commit is one the worktree was on (agents
# often push from a branch of another name)
def Merge.has(cs: List<&2, String>, +c: String) -> Bool:
  match cs:
    case Nil{}:
      False{}
    case +h <> t:
      Bool.or(String.eq(h, c), Merge.has(t, c))

def Merge.ours(+o: J.Json, +branch: String, +cs: List<&2, String>) -> Bool:
  +oid = J.Json.get_str(o, "headRefOid", "")
  Bool.or(Bool.and(Bool.not(String.is_empty(branch)), String.eq(J.Json.get_str(o, "headRefName", ""), branch)),
    Bool.and(Bool.not(String.is_empty(oid)), Merge.has(cs, oid)))

def Merge.find.k(ours: Bool, +o: J.Json, rest: Maybe<&2, String>) -> Maybe<&2, String>:
  match ours:
    case True{}:
      Some{J.Json.get_str(o, "url", "")}
    case False{}:
      rest

def Merge.find(prs: List<&2, J.Json>, +branch: String, +cs: List<&2, String>) -> Maybe<&2, String>:
  match prs:
    case Nil{}:
      None{}
    case +o <> t:
      Merge.find.k(Merge.ours(o, branch, cs), o, Merge.find(t, branch, cs))

def Merge.hit(m: Maybe<&2, String>, +th: String, rest: List<&2, J.KV>) -> List<&2, J.KV>:
  match m:
    case Some{u}:
      J.KV{th, J.Str{u}} <> rest
    case None{}:
      rest

def Merge.pair(parts: List<&2, String>, +prs: List<&2, J.Json>, rest: List<&2, J.KV>) -> List<&2, J.KV>:
  match parts:
    case +th <> +br <> _ <> +cs <> Nil{}:
      Merge.hit(Merge.find(prs, br, String.split(cs, ' ')), th, rest)
    case +th <> +br <> _:
      Merge.hit(Merge.find(prs, br, Nil{}), th, rest)
    case _:
      rest

def Merge.hits.go(ls: List<&2, String>, +prs: List<&2, J.Json>) -> List<&2, J.KV>:
  match ls:
    case Nil{}:
      Nil{}
    case l <> t:
      Merge.pair(String.split(l, '\t'), prs, Merge.hits.go(t, prs))

def Merge.hits(+json: String, +lines: String) -> List<&2, J.KV>:
  Merge.hits.go(String.lines(lines), J.Json.elems(Maybe.default(&2, J.Json, J.Json.parse(json), J.Null{})))

def Wt.stale.one.k(t: M.Thread, +now: Nat, +pending: Bool) -> Bool:
  match t:
    case M.Thread{_, _, _, _, _, _, _, _, _, _, _, _, M.Worktree{}, +br, +wt, _, _, _, _, _, _}:
      Bool.and(Bool.and(String.is_empty(wt), String.is_empty(br)), Bool.not(pending))
    case _:
      False{}

def Wt.stale.one(+t: M.Thread, +now: Nat) -> Bool:
  Wt.stale.one.k(t, now, M.Worktree.pending(t, now))

# a hardware project: KiCad files in the project folder. Its new worktrees
# bring the folder's uncommitted work along (board work is often not
# committed yet, and boards do not merge by hand)
def Wt.hardware(ns: List<&2, String>) -> Bool:
  match ns:
    case Nil{}:
      False{}
    case +n <> t:
      Bool.or(Bool.or(String.ends_with(n, ".kicad_pro"), String.ends_with(n, ".kicad_pcb")), Bool.or(String.ends_with(n, ".kicad_sch"), Wt.hardware(t)))

# what the hub tells the clients when an agent starts in a KiCad project:
# show the thread's board (its schematic when there is no board) as the
# hub's own view ("auto", which a panel shut by hand ignores); "" for a
# project with neither
def Auto.view.json(+thread: String, +pcb: String, +sch: String) -> J.Json:
  J.Json.obj([J.KV{"t", J.Str{"view"}}, J.KV{"thread", J.Str{thread}}, J.KV{"kind", J.Str{Bool.pick(String, String.is_empty(pcb), "schematic", "board")}},
    J.KV{"path", J.Str{Bool.pick(String, String.is_empty(pcb), sch, pcb)}}, J.KV{"auto", J.Json.bool(True{})}])

def Auto.view.k(none: Bool, +thread: String, +pcb: String, +sch: String) -> String:
  match none:
    case True{}:
      ""
    case False{}:
      J.Json.show(Auto.view.json(thread, pcb, sch))

def Auto.view(+thread: String, +pcb: String, +sch: String) -> String:
  Auto.view.k(Bool.and(String.is_empty(pcb), String.is_empty(sch)), thread, pcb, sch)

def Wt.digit(keep: Bool, +c: U32, rest: String) -> String:
  match keep:
    case True{}:
      SCon{Chr{c}, rest}
    case False{}:
      rest

def Wt.digits(s: String) -> String:
  match s:
    case SNil{}:
      SNil{}
    case SCon{Chr{+c}, t}:
      Wt.digit(Bool.and(U32.is_ge(c, 48), U32.is_le(c, 57)), c, Wt.digits(t))

# a thread's temporary branch seed: its id's digits (time and sequence), so
# two threads made in the same second get different branches
def Wt.seed(+id: String) -> Nat:
  Maybe.default(&2, Nat, Nat.read(Wt.digits(id)), 0n)

# Codex (codex exec --json)
# -------------------------

def Codex.item.k(i: Nat, +thread: String, +now: Nat, +seq: Nat, +it: J.Json) -> Step:
  match i:
    case 0n:
      Step{[M.MessagePosted{thread, J.Json.get_str(it, "id", Hub.id("c", now, seq)) ++ "-" ++ Nat.show(seq), M.Assistant{}, J.Json.get_str(it, "text", ""), now}], Nil{}}
    case 1n:
      Step{[M.ActivityLogged{thread, J.Json.get_str(it, "id", Hub.id("x", now, seq)) ++ "-" ++ Nat.show(seq), "tool", "shell", J.Json.get_str(it, "command", ""), now}], Nil{}}
    case 2n:
      Step{[M.ActivityLogged{thread, Hub.id("f", now, seq), "tool", "edit", J.Json.show(J.Json.get_obj(it, "changes")), now}], Nil{}}
    case 3n:
      Step{[M.TodosSet{thread, TD.Todo.show(TD.Todo.codex(it)), now}], Nil{}}
    case _:
      Step.none()

# a todo list as it starts or changes (only its completion goes through
# item.completed)
def Codex.todo(todo: Bool, +thread: String, +now: Nat, +it: J.Json) -> Step:
  match todo:
    case True{}:
      Step{[M.TodosSet{thread, TD.Todo.show(TD.Todo.codex(it)), now}], Nil{}}
    case False{}:
      Step.none()

# item.started and item.updated share a case: only todo lists are read there
def Codex.kind(+t: String) -> Nat:
  Bool.pick(Nat, String.eq(t, "tool.done"), 6n, Bool.pick(Nat, Bool.or(String.eq(t, "item.started"), String.eq(t, "item.updated")), 5n,
    C.Codec.pos(["thread.started", "item.completed", "turn.completed", "turn.failed", "item.delta"], t)))

def Codex.line.k(i: Nat, +s: M.State, +thread: String, +now: Nat, +o: J.Json) -> Step:
  match i:
    case 0n:
      Claude.init.k(Claude.init.same(Hub.thread(s, thread), J.Json.get_str(o, "thread_id", "")), thread, J.Json.get_str(o, "thread_id", ""), now)
    case 1n:
      Codex.item.k(C.Codec.pos(["agent_message", "command_execution", "file_change", "todo_list"], J.Json.get_str(J.Json.get_obj(o, "item"), "type", "")), thread, now, Hub.seq(s), J.Json.get_obj(o, "item"))
    case 2n:
      Step{[M.TurnChanged{thread, M.Completed{}, now}], Nil{}}
    case 4n:
      Step.outs([Delta.push(thread, J.Json.get_str(o, "text", ""))])
    case 5n:
      Codex.todo(String.eq(J.Json.get_str(J.Json.get_obj(o, "item"), "type", ""), "todo_list"), thread, now, J.Json.get_obj(o, "item"))
    case 6n:
      Step.outs([Steps.out(thread, Hub.cwd(s, Hub.thread(s, thread)), "")])
    case 3n:
      Step{[M.ActivityLogged{thread, Hub.id("e", now, Hub.seq(s)), "error", Hub.provider(Hub.thread(s, thread)), J.Json.get_str(J.Json.get_obj(o, "error"), "message", J.Json.get_str(o, "message", "turn failed")), now}, M.TurnChanged{thread, M.Failed{}, now}], Nil{}}
    case _:
      Step.none()

def Codex.line.parsed(m: Maybe<&2, J.Json>, +s: M.State, +thread: String, +now: Nat) -> Step:
  match m:
    case Some{+o}:
      Codex.line.k(Codex.kind(J.Json.get_str(o, "type", "")), s, thread, now, o)
    case None{}:
      Step.none()

# Grok's and Muse's tool calls are logged before they run: their design
# steps come at "tool.done"
def Hub.acp_like(+p: String) -> Bool:
  Bool.or(Bool.or(String.eq(p, "grok"), String.eq(p, "muse")), String.eq(p, "antigravity"))

# a Codex, Grok or Muse line counts only while its thread's turn runs: what a
# stopped turn's process still says is old news (law stopped_turn_quiet)
def Hub.agent_line.c(run: Bool, +s: M.State, +now: Nat, +thread: String, line: String) -> Step:
  match run:
    case True{}:
      Steps.after.if(Bool.not(Hub.acp_like(Hub.provider(Hub.thread(s, thread)))), Codex.line.parsed(J.Json.parse(line), s, thread, now), thread, Hub.cwd(s, Hub.thread(s, thread)))
    case False{}:
      Step.none()

def Hub.agent_line.k(codex: Bool, +s: M.State, +now: Nat, +thread: String, line: String) -> Step:
  match codex:
    case True{}:
      Hub.agent_line.c(Hub.running(Hub.thread(s, thread)), s, now, thread, line)
    case False{}:
      Claude.line.parsed(J.Json.parse(line), s, thread, now)

# Queued turns
# ------------

def Queue.ended.one(c: M.Change, +thread: String) -> Bool:
  match c:
    case M.TurnChanged{th, M.Completed{}, _}:
      String.eq(th, thread)
    case M.TurnChanged{th, M.Failed{}, _}:
      String.eq(th, thread)
    case _:
      False{}

# whether these changes end the thread's turn (an interrupt keeps the
# queue: it goes with the next message)
def Queue.ended(cs: List<&2, M.Change>, +thread: String) -> Bool:
  match cs:
    case Nil{}:
      False{}
    case c <> t:
      Bool.or(Queue.ended.one(c, thread), Queue.ended(t, thread))

def Queue.drain.k(go: Bool, st: Step, +t: M.Thread, cwd: String, +queued: String, +now: Nat, +seq: Nat, +thread: String) -> Step:
  match go st:
    case True{} Step{cs, os}:
      Step{List.append(&2, M.Change, cs, [M.QueueTaken{thread, now}, M.TurnChanged{thread, M.Running{}, now}]), List.append(&2, Out, os, Hub.run(t, cwd, queued, Hub.uuid(now, thread ++ "/q/" ++ Nat.show(seq))))}
    case False{} _:
      st

def Queue.changes(st: Step) -> List<&2, M.Change>:
  match st:
    case Step{cs, _}:
      cs

# the turn ended with messages waiting: start the next turn with them
def Ended.state(cs: List<&2, M.Change>, +thread: String) -> String:
  match cs:
    case Nil{}:
      ""
    case M.TurnChanged{+th, M.Completed{}, _} <> t:
      Bool.pick(String, String.eq(th, thread), "completed", Ended.state(t, thread))
    case M.TurnChanged{+th, M.Failed{}, _} <> t:
      Bool.pick(String, String.eq(th, thread), "failed", Ended.state(t, thread))
    case _ <> t:
      Ended.state(t, thread)

# Answers to a person elsewhere
# ----------------------------
# A bot's thread woken by a person on a linked machine ("[message from
# person@<machine>]") answers back to them (bothub.bend's Reply.*). The
# answer is taken when the turn that leaves the thread ready ends (its
# work done: M.Work.ready over the state after the step), not read again
# later, and goes out once (law bot_reply_captured_once).

# the machine a person wrote from, read from the head of a turn's message
# ("[message from person@<machine>]"); "" for anything else: a bot, a room
def Answer.peer.k(ok: Bool, +head: String) -> String:
  match ok:
    case True{}:
      String.take(String.drop(head, 21n), Nat.sub(String.length(head), 22n))
    case False{}:
      ""

def Answer.peer.head(+head: String) -> String:
  Answer.peer.k(Bool.and(Bool.and(String.starts_with(head, "[message from person@"), String.ends_with(head, "]")),
    Bool.and(Bool.not(String.contains(head, " in #")), Nat.is_gt(String.length(head), 22n))), head)

def Answer.peer(+text: String) -> String:
  Answer.peer.head(Maybe.default(&2, String, List.head(&2, String, String.split(text, '\n')), ""))

# a thread's current turn read back from its entries (newest first): the
# message that started it and the newest answer since
type Answer is Data:
  Answer{answer: String, trigger: String}

def Answer.user(hit: Bool, text: String, acc: Answer) -> Answer:
  match hit acc:
    case True{} Answer{a, _}:
      Answer{a, text}
    case False{} _:
      acc

def Answer.answer(hit: Bool, text: String, acc: Answer) -> Answer:
  match hit acc:
    case True{} Answer{+a, t}:
      Answer{Bool.pick(String, String.is_empty(a), text, a), t}
    case False{} _:
      acc

# the walk stops at the thread's newest user message
def Answer.scan(es: List<&2, M.Entry>, +id: String, done: Bool, acc: Answer) -> Answer:
  match es done:
    case _ True{}:
      acc
    case Nil{} False{}:
      acc
    case M.Msg{_, +th, M.User{}, text, _} <> t False{}:
      Answer.scan(t, id, String.eq(th, id), Answer.user(String.eq(th, id), text, acc))
    case M.Msg{_, +th, M.Assistant{}, text, _} <> t False{}:
      Answer.scan(t, id, False{}, Answer.answer(String.eq(th, id), text, acc))
    case _ <> t False{}:
      Answer.scan(t, id, False{}, acc)

def Answer.of(es: List<&2, M.Entry>, +id: String) -> Answer:
  Answer.scan(es, id, False{}, Answer{"", ""})

def Answer.out.k(ok: Bool, +thread: String, +to: String, +text: String, +done: String) -> List<&2, Out>:
  match ok:
    case True{}:
      [BotReply{thread, to, text, done}]
    case False{}:
      Nil{}

def Answer.out.a(a: Answer, ready: Bool, +thread: String, +done: String) -> List<&2, Out>:
  match a:
    case Answer{+answer, +trigger}:
      Answer.out.k(Bool.and(ready, Bool.and(Bool.not(String.is_empty(answer)), Bool.not(String.is_empty(Answer.peer(trigger))))),
        thread, Answer.peer(trigger), answer, done)

# a turn that completed (done) with the thread ready after it (post: the
# state after the step): the answer to a person elsewhere, if it owes one
def Answer.out(done: Bool, +post: M.State, +thread: String) -> List<&2, Out>:
  match done:
    case True{}:
      Answer.out.a(Answer.of(M.State.mine(post, thread), thread), M.Work.ready(post, thread), thread, thread ++ ":" ++ Nat.show(Hub.seq(post)))
    case False{}:
      Nil{}

# a bot thread a person elsewhere wrote to, answered (busy: with a
# background shell still out); the laws' example
def Answer.demo(busy: Bool) -> M.State:
  M.State.replay(List.append(&2, M.Change, [M.ProjectCreated{"p1", "p", "/p", 1n}, M.ThreadCreated{"t1", "p1", "x", M.Local{}, "", "", "claude", "", 1n},
    M.MessagePosted{"t1", "m1", M.User{}, "[message from person@desk]\nhey", 2n}, M.MessagePosted{"t1", "m2", M.Assistant{}, "hi", 3n},
    M.TurnChanged{"t1", M.Completed{}, 3n}], Bool.pick(List<&2, M.Change>, busy, [M.WorkSet{"t1", "bg", "a\tx", 3n}], Nil{})), M.State.empty())

def Ended.out.k(none: Bool, +s: M.State, +now: Nat, +state: String, +thread: String, +cwd: String, st: Step) -> Step:
  match none st:
    case True{} _:
      st
    case False{} Step{+cs, os}:
      Step{List.append(&2, M.Change, cs, Sum.at(RF.Sum.due(s, thread, now), thread, now)),
        List.append(&2, Out, os, TurnEnded{thread, state} <> Git{0, "", "step", thread, cwd, "", ""} <> Git{0, "", "checkpoint", thread, cwd, "", ""}
          <> List.append(&2, Out, Sum.out(RF.Sum.due(s, thread, now), s, thread, "thread"), Answer.out(String.eq(state, "completed"), M.State.replay(cs, s), thread)))}

def Ended.out.s(+s: M.State, +now: Nat, +state: String, +thread: String, +cwd: String, st: Step) -> Step:
  Ended.out.k(String.is_empty(state), s, now, state, thread, cwd, st)

# a turn that ends tells the server (phone notifications hang off this),
# and has the thread summarized when its summary is due
def Ended.out(+s: M.State, +now: Nat, +thread: String, +st: Step) -> Step:
  Ended.out.s(s, now, Ended.state(Queue.changes(st), thread), thread, Hub.cwd(s, Hub.thread(s, thread)), st)

def Queue.drain(+s: M.State, +now: Nat, +thread: String, +st: Step) -> Step:
  Queue.drain.k(Bool.and(Queue.ended(Queue.changes(st), thread), M.Queue.waiting(s, thread)), st,
    Hub.thread(s, thread), Hub.cwd(s, Hub.thread(s, thread)), M.Queue.of(s, thread), now, Hub.seq(s), thread)

# Delegated tasks finishing
# -------------------------

def Task.child.hit(ts: List<&2, M.Task>, +child: String) -> Bool:
  match ts:
    case M.Task{_, _, c, _, _, _, _, _, _} <> _:
      String.eq(c, child)
    case Nil{}:
      False{}

def Task.child.go(ts: List<&2, M.Task>, hit: Bool, +child: String) -> Maybe<&2, M.Task>:
  match ts hit:
    case t <> _ True{}:
      Some{t}
    case _ <> +rest False{}:
      Task.child.go(rest, Task.child.hit(rest, child), child)
    case Nil{} _:
      None{}

def Task.of_child.k(+ts: List<&2, M.Task>, +child: String) -> Maybe<&2, M.Task>:
  Task.child.go(ts, Task.child.hit(ts, child), child)

# the task a child thread is working on, if any
def Task.of_child(s: M.State, +child: String) -> Maybe<&2, M.Task>:
  match s:
    case M.State{_, _, _, _, tk, _, _, _, _, _, _}:
      Task.of_child.k(Map.values(&2, M.Task, tk), child)

def Task.depth.k(m: Maybe<&2, M.Task>) -> Nat:
  match m:
    case Some{M.Task{_, _, _, d, _, _, _, _, _}}:
      d
    case None{}:
      0n

# the depth of a thread: 0 at the top, its task's depth for a child
def Task.depth(+s: M.State, +thread: String) -> Nat:
  Task.depth.k(Task.of_child(s, thread))

# the newest assistant message of a thread (a child's result)
def Task.last.k(same: Bool, e: M.Entry, rest: String) -> String:
  match same e:
    case True{} M.Msg{_, _, M.Assistant{}, text, _}:
      text
    case _ _:
      rest

def Task.last.hit(es: List<&2, M.Entry>, +th: String) -> Bool:
  match es:
    case M.Msg{_, t, M.Assistant{}, _, _} <> _:
      String.eq(t, th)
    case _:
      False{}

def Task.last.go(es: List<&2, M.Entry>, hit: Bool, +th: String) -> String:
  match es hit:
    case M.Msg{_, _, _, text, _} <> _ True{}:
      text
    case _ <> +t False{}:
      Task.last.go(t, Task.last.hit(t, th), th)
    case _ _:
      ""

def Task.last.of(+es: List<&2, M.Entry>, +th: String) -> String:
  Task.last.go(es, Task.last.hit(es, th), th)

# read from the thread's own entries (M.State.mine), never the whole log
def Task.last(+s: M.State, +th: String) -> String:
  Task.last.of(M.State.mine(s, th), th)

# the turn outcome a step reports for a thread, if it ended one
type Ended is Data:
  NotEnded{}
  EndedOk{}
  EndedBad{}

def Task.ended(cs: List<&2, M.Change>) -> Ended:
  match cs:
    case Nil{}:
      NotEnded{}
    case M.TurnChanged{_, M.Completed{}, _} <> _:
      EndedOk{}
    case M.TurnChanged{_, M.Failed{}, _} <> _:
      EndedBad{}
    case M.TurnChanged{_, M.Interrupted{}, _} <> _:
      EndedBad{}
    case _ <> t:
      Task.ended(t)

def Task.state_of(e: Ended) -> M.TaskState:
  match e:
    case EndedOk{}:
      M.TaskDone{}
    case _:
      M.TaskFailed{}

def Task.word(e: Ended) -> String:
  match e:
    case EndedOk{}:
      "finished"
    case _:
      "failed"

def Task.note(+id: String, +provider: String, +model: String, +word: String, +summary: String) -> String:
  "[Backplane] Delegated task " ++ id ++ " (" ++ provider ++ Bool.pick(String, String.is_empty(model), "", "/" ++ model) ++ ") " ++ word ++ ":\n\n" ++ summary

# the parent's message with a task's result: its id is the task's, so a
# restart finds it (Task.repair)
def Task.msg(+thread: String, +id: String) -> String:
  M.Link.id(thread, "td-" ++ id)

# (the result reaches the parent before TaskDelivered: a step torn by a
# crash after the message leaves the task undelivered, and Task.repair
# finishes it without a second message; law task_finish_delivered_last)
def Task.finish.go(deliver: Bool, +s: M.State, +now: Nat, +id: String, +thread: String, +parent: String, +provider: String, +model: String, +word: String, st: M.TaskState, +summary: String, cs: List<&2, M.Change>, os: List<&2, Out>) -> Step:
  match deliver:
    case True{}:
      Step.after(Step.before(List.append(&2, M.Change, cs, [M.TaskStateSet{id, st, summary, now}]), os,
        Hub.inbox.id(s, now, thread, parent, M.System{}, Task.note(id, provider, model, word, summary), Task.msg(thread, id))), [M.TaskDelivered{id, now}])
    case False{}:
      Step{List.append(&2, M.Change, cs, [M.TaskStateSet{id, st, summary, now}]), os}

def Task.finish.k(e: Ended, m: Maybe<&2, M.Task>, +s: M.State, +now: Nat, +thread: String, cs: List<&2, M.Change>, os: List<&2, Out>) -> Step:
  match e m:
    case NotEnded{} _:
      Step{cs, os}
    case _ None{}:
      Step{cs, os}
    case +ended Some{M.Task{+id, +parent, _, _, +cur, _, +dl, +provider, +model}}:
      Task.finish.go(M.Task.pending(M.Task{id, parent, thread, 0n, M.TaskState.next(cur, Task.state_of(ended)), "", dl, provider, model}),
        s, now, id, thread, parent, provider, model, Task.word(ended), Task.state_of(ended), Task.last(s, thread), cs, os)

# when a child thread's turn ends, its task ends and the result goes to the
# parent once (the pending check makes delivery exactly-once)
# (the task is looked up only once the step is known to end a turn: every
# agent line comes through here)
def Task.finish.e(e: Ended, +s: M.State, +now: Nat, +thread: String, cs: List<&2, M.Change>, os: List<&2, Out>) -> Step:
  match e:
    case NotEnded{}:
      Step{cs, os}
    case EndedOk{}:
      Task.finish.k(EndedOk{}, Task.of_child(s, thread), s, now, thread, cs, os)
    case EndedBad{}:
      Task.finish.k(EndedBad{}, Task.of_child(s, thread), s, now, thread, cs, os)

def Task.finish(+s: M.State, +now: Nat, +thread: String, st: Step) -> Step:
  match st:
    case Step{+cs, os}:
      Task.finish.e(Task.ended(cs), s, now, thread, cs, os)

def Task.ended.one(c: M.Change, +thread: String, acc: Ended) -> Ended:
  match c:
    case M.TurnChanged{+th, M.Completed{}, _}:
      Bool.pick(Ended, String.eq(th, thread), EndedOk{}, acc)
    case M.TurnChanged{+th, M.Failed{}, _}:
      Bool.pick(Ended, String.eq(th, thread), EndedBad{}, acc)
    case M.TurnChanged{+th, M.Interrupted{}, _}:
      Bool.pick(Ended, String.eq(th, thread), EndedBad{}, acc)
    case M.TurnChanged{+th, _, _}:
      Bool.pick(Ended, String.eq(th, thread), NotEnded{}, acc)
    case _:
      acc

def Task.ended.go(cs: List<&2, M.Change>, +thread: String, acc: Ended) -> Ended:
  match cs:
    case Nil{}:
      acc
    case c <> t:
      Task.ended.go(t, thread, Task.ended.one(c, thread, acc))

# where a step leaves a thread's turn: its last turn change (a completion
# a queued message starts again after is no end; law task_ended_last_change)
def Task.ended.last(cs: List<&2, M.Change>, +thread: String) -> Ended:
  Task.ended.go(cs, thread, NotEnded{})

def Task.ok.if(ready: Bool) -> Ended:
  match ready:
    case True{}:
      EndedOk{}
    case False{}:
      NotEnded{}

def Task.finish.w.e(e: Ended, +s: M.State, +now: Nat, +thread: String, +cs: List<&2, M.Change>, os: List<&2, Out>) -> Step:
  match e:
    case NotEnded{}:
      Step{cs, os}
    case EndedOk{}:
      Task.finish.e(Task.ok.if(M.Work.ready(M.State.replay(cs, s), thread)), s, now, thread, cs, os)
    case EndedBad{}:
      Task.finish.e(EndedBad{}, s, now, thread, cs, os)

# whether a step sets task id's state
def Task.marked(cs: List<&2, M.Change>, +id: String) -> Bool:
  match cs:
    case Nil{}:
      False{}
    case M.TaskStateSet{+t, _, _, _} <> rest:
      Bool.or(String.eq(t, id), Task.marked(rest, id))
    case _ <> rest:
      Task.marked(rest, id)

# a parent t0 whose child t1 works on task k1 (busy: with a background
# shell still out); the laws' example
def Task.demo(busy: Bool) -> M.State:
  M.State.replay(List.append(&2, M.Change, [M.ProjectCreated{"p1", "p", "/p", 1n}, M.ThreadCreated{"t0", "p1", "x", M.Local{}, "", "", "claude", "", 1n},
    M.ThreadCreated{"t1", "p1", "y", M.Local{}, "", "", "claude", "", 1n}, M.TaskDelegated{"k1", "t0", "t1", 1n, "claude", "", 1n},
    M.TaskStateSet{"k1", M.TaskRunning{}, "", 1n}, M.TurnChanged{"t1", M.Running{}, 1n}], Bool.pick(List<&2, M.Change>, busy, [M.WorkSet{"t1", "bg", "a\tx", 2n}], Nil{})), M.State.empty())

# a child's task ends when its thread is done: a completed turn that left
# work out (its own children, background or watched work) or a queued
# message keeps the task open, and the turn that finishes that work
# delivers; a failure or a stop ends it at once (laws task_waits_for_child_work,
# task_failed_ends_even_busy)
def Task.finish.w(+s: M.State, +now: Nat, +thread: String, st: Step) -> Step:
  match st:
    case Step{+cs, os}:
      Task.finish.w.e(Task.ended.last(cs, thread), s, now, thread, cs, os)

# Launched threads reporting back
# -------------------------------
# A thread started with thread_launch (model.bend's Launch) tells whoever
# launched it, the way a delegated task's child does: when a step ends its
# turn (completed, failed, interrupted) or opens an ask, a short note goes
# to the launcher's thread through its inbox (Hub.inbox: a turn starts
# there, or the note waits in its queue behind a running one; the queue is
# in the log, so a restart keeps the order). One note per turn end at most
# (law launch_once_per_end); none when the launcher turned reports off
# (law launch_muted_quiet), when the turn left work out of hand (the turn
# that finishes it reports; law launch_waits_for_work), or when the thread
# already wrote to its launcher in this turn (law launch_said_quiet).
# A launcher that goes back and forth with its thread Lr.loops() times
# with no word from the person gets a last note and the reports stop
# (law launch_loop_bounded). The uberbot's notes go to the machine that
# holds its conversation (Ub.chat), and a launcher on another machine is
# reached through the outbox below.

# the most turns in a row a launcher's own messages start before reports stop
def Lr.loops() -> Nat:
  20n

# how much of the last reply a note carries
def Lr.cut() -> Nat:
  1500n

# 0: nothing, 1 completed, 2 failed, 3 interrupted (the thread's last turn change)
def Lr.one(c: M.Change, +thread: String, acc: Nat) -> Nat:
  match c:
    case M.TurnChanged{+th, M.Completed{}, _}:
      Bool.pick(Nat, String.eq(th, thread), 1n, acc)
    case M.TurnChanged{+th, M.Failed{}, _}:
      Bool.pick(Nat, String.eq(th, thread), 2n, acc)
    case M.TurnChanged{+th, M.Interrupted{}, _}:
      Bool.pick(Nat, String.eq(th, thread), 3n, acc)
    case M.TurnChanged{+th, _, _}:
      Bool.pick(Nat, String.eq(th, thread), 0n, acc)
    case _:
      acc

def Lr.go(cs: List<&2, M.Change>, +thread: String, acc: Nat) -> Nat:
  match cs:
    case Nil{}:
      acc
    case c <> t:
      Lr.go(t, thread, Lr.one(c, thread, acc))

def Lr.ended(cs: List<&2, M.Change>, +thread: String) -> Nat:
  Lr.go(cs, thread, 0n)

def Lr.ask.k(hit: Bool, +kind: String, +tool: String, +detail: String, +rest: String) -> String:
  match hit:
    case True{}:
      Bool.pick(String, String.is_empty(tool), kind, kind ++ " (" ++ tool ++ ")") ++ Bool.pick(String, String.is_empty(detail), "", ": " ++ String.take(detail, 600n))
    case False{}:
      rest

# the first ask the step opened in the thread ("" for none)
def Lr.ask(cs: List<&2, M.Change>, +thread: String) -> String:
  match cs:
    case Nil{}:
      ""
    case M.AskOpened{_, +th, +k, +tool, +d, _, _} <> t:
      Lr.ask.k(String.eq(th, thread), k, tool, d, Lr.ask(t, thread))
    case _ <> t:
      Lr.ask(t, thread)

# a report's message id: "t:<thread>:lr-<key>"
def Lr.msg(+from: String, +key: String) -> String:
  M.Link.id(from, "lr-" ++ key)

def Lr.is(+id: String) -> Bool:
  String.contains(id, ":lr-")

def Lr.sent(+i: String, +t: String, +th: String, +to: String) -> Bool:
  Bool.and(String.eq(t, to), Bool.and(String.eq(M.Link.from(i), th), Bool.not(Lr.is(i))))

def Lr.said.go(es: List<&2, M.Entry>, +th: String, +to: String, done: Bool, acc: Bool) -> Bool:
  match es done:
    case _ True{}:
      acc
    case Nil{} False{}:
      acc
    case M.Msg{+i, +t, M.User{}, _, _} <> rest False{}:
      Lr.said.go(rest, th, to, Bool.or(String.eq(t, th), Lr.sent(i, t, th, to)), Lr.sent(i, t, th, to))
    case M.Msg{+i, +t, _, _, _} <> rest False{}:
      Lr.said.go(rest, th, to, Lr.sent(i, t, th, to), Lr.sent(i, t, th, to))
    case _ <> rest False{}:
      Lr.said.go(rest, th, to, False{}, acc)

# whether the thread wrote to `to` since its newest user message (its own
# entries, newest first, hold what it sent; reports are not its words)
def Lr.said(es: List<&2, M.Entry>, +th: String, +to: String) -> Bool:
  Lr.said.go(es, th, to, False{}, False{})

def Lr.trig.go(es: List<&2, M.Entry>, +th: String, done: Bool, +acc: String) -> String:
  match es done:
    case _ True{}:
      acc
    case Nil{} False{}:
      acc
    case M.Msg{+i, +t, M.User{}, _, _} <> rest False{}:
      Lr.trig.go(rest, th, String.eq(t, th), Bool.pick(String, String.eq(t, th), i, acc))
    case _ <> rest False{}:
      Lr.trig.go(rest, th, False{}, acc)

# the id of the thread's newest user message (what started its turn)
def Lr.trig(es: List<&2, M.Entry>, +th: String) -> String:
  Lr.trig.go(es, th, False{}, "")

def Lr.reply(a: Answer) -> String:
  match a:
    case Answer{r, _}:
      r

def Lr.title(t: M.Thread) -> String:
  match t:
    case M.Thread{_, _, ti, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _}:
      ti

def Lr.word(+e: Nat, +reply: String) -> String:
  match e:
    case 1n:
      Bool.pick(String, String.ends_with(String.trim(reply), "?"), "finished its turn and waits for an answer", "completed")
    case 2n:
      "failed"
    case _:
      "was interrupted"

def Lr.body(long: Bool, +reply: String, +id: String) -> String:
  match long:
    case True{}:
      String.take(reply, Lr.cut()) ++ "…\n\n(cut short: thread_read " ++ id ++ " for the rest)"
    case False{}:
      reply

def Lr.head(+id: String, +title: String) -> String:
  "[Backplane] Launched thread \"" ++ title ++ "\" (" ++ id ++ ")"

# the note for a turn's end: the state and the last reply, cut to Lr.cut()
def Lr.note(+id: String, +title: String, +word: String, +reply: String) -> String:
  Lr.head(id, title) ++ " " ++ word ++ Bool.pick(String, String.is_empty(String.trim(reply)), ".", ":\n\n" ++ Lr.body(Nat.is_gt(String.length(reply), Lr.cut()), reply, id))

# the note for an ask: the thread waits on the person
def Lr.ask.note(+id: String, +title: String, +ask: String) -> String:
  Lr.head(id, title) ++ " needs the person: " ++ ask ++ "\nIt waits until they answer in that thread."

# the last note before the reports stop
def Lr.loop.note(+id: String) -> String:
  "\n\nReports from this thread stop here: it went back and forth with you " ++ Nat.show(Lr.loops()) ++ " times with no word from the person. thread_update {threadId: " ++ id ++ ", report: true} starts them again."

def Lr.n.key(+th: String) -> String:
  "launch.n." ++ th

# Where a report goes
# -------------------

# thread_send between threads a launch ties, whatever their projects (a
# bot's thread has none): a launched thread to its launcher and back, and
# any thread to the uberbot's own thread (here or a replica's elsewhere;
# law launch_send_launcher)
def Lr.may(+s: M.State, +from: String, +to: String) -> Bool:
  Bool.and(Bool.not(String.is_empty(to)),
    Bool.or(Bool.or(String.eq(M.Launch.by(s, from), to), String.eq(M.Launch.by(s, to), from)), UBS.Ub.chat.is(s, to)))


# the uberbot's thread here or a replica's elsewhere: the thread that holds
# its conversation now; anything else is itself
def Lr.dest(+s: M.State, +by: String) -> String:
  Bool.pick(String, UBS.Ub.chat.is(s, by), Bool.pick(String, String.is_empty(UBS.Ub.chat(s)), by, UBS.Ub.chat(s)), by)

# The outbox for launchers on other machines: the setting launch.far, one
# report a line ("key\tto\tfrom\ttext as JSON"), oldest first. The server
# sends them in order (LaunchFar), each to its machine as launch_report
# (fmcp.bend), drops a line once its machine took it (Lr.far.sent), and
# tries the rest again each minute; the receiver keeps one message per key
# (law launch_far_once), so a line sent twice posts once.
def Lr.far.key() -> String:
  "launch.far"

def Lr.far.line(+key: String, +to: String, +from: String, +text: String) -> String:
  key ++ "\t" ++ to ++ "\t" ++ from ++ "\t" ++ J.Json.show(J.Str{text})

def Lr.far.add(+s: M.State, +line: String, +now: Nat) -> M.Change:
  M.SettingSet{Lr.far.key(), Bool.pick(String, String.is_empty(M.Setting.get(s, Lr.far.key(), "")), line, M.Setting.get(s, Lr.far.key(), "") ++ "\n" ++ line), now}

def Lr.far.keep.k(drop: Bool, +l: String, rest: List<&2, String>) -> List<&2, String>:
  match drop:
    case True{}:
      rest
    case False{}:
      l <> rest

def Lr.far.keep(ls: List<&2, String>, +key: String) -> List<&2, String>:
  match ls:
    case Nil{}:
      Nil{}
    case +l <> t:
      Lr.far.keep.k(String.starts_with(l, key ++ "\t"), l, Lr.far.keep(t, key))

def Lr.far.lines(+s: M.State) -> List<&2, String>:
  M.Launch.lines(M.Setting.get(s, Lr.far.key(), ""))

# the outbox once `key` was taken
def Lr.far.sent(+s: M.State, +now: Nat, +key: String) -> Step:
  Step{[M.SettingSet{Lr.far.key(), String.join(Lr.far.keep(Lr.far.lines(s), key), "\n"), now}], Nil{}}

# each minute: the outbox is sent again while anything is in it
def Lr.far.tick.k(none: Bool) -> Step:
  match none:
    case True{}:
      Step.none()
    case False{}:
      Step.outs([LaunchFar{}])

def Lr.far.tick(+s: M.State) -> Step:
  Lr.far.tick.k(String.is_empty(M.Setting.get(s, Lr.far.key(), "")))

# Delivering
# ----------

def Lr.exists(+s: M.State, +th: String) -> Bool:
  String.eq(M.Thread.id(Hub.thread(s, th)), th)

def Lr.send.here(ok: Bool, +s: M.State, +now: Nat, +thread: String, +to: String, +text: String) -> Step:
  match ok:
    case True{}:
      Hub.inbox.id(s, now, thread, to, M.User{}, text, Lr.msg(thread, Hub.id("r", now, Hub.seq(s))))
    case False{}:
      Step.none()

def Lr.send.k(far: Bool, +s: M.State, +now: Nat, +thread: String, +to: String, +text: String) -> Step:
  match far:
    case True{}:
      Step{[Lr.far.add(s, Lr.far.line(Hub.id("r", now, Hub.seq(s)), to, thread, text), now)], [LaunchFar{}]}
    case False{}:
      Lr.send.here(Lr.exists(s, to), s, now, thread, to, text)

# a note from thread to its launcher's thread `to`
def Lr.send(+s: M.State, +now: Nat, +thread: String, +to: String, +text: String) -> Step:
  Lr.send.k(FR.Fr.is(to), s, now, thread, to, text)

# a turn's end: the counter of launcher-started turns moves on, and at
# the bound the reports stop
# the counter, written only when it moves
def Lr.n.set(same: Bool, +thread: String, +n: Nat, +now: Nat) -> List<&2, M.Change>:
  match same:
    case True{}:
      Nil{}
    case False{}:
      [M.SettingSet{Lr.n.key(thread), Nat.show(n), now}]

def Lr.end.n(stop: Bool, +s: M.State, +now: Nat, +thread: String, +by: String, +to: String, +n: Nat, +text: String) -> Step:
  match stop:
    case True{}:
      Step.before([M.SettingSet{Lr.n.key(thread), "0", now}, M.ThreadLaunched{thread, by, False{}, now}], Nil{}, Lr.send(s, now, thread, to, text ++ Lr.loop.note(thread)))
    case False{}:
      Step.before(Lr.n.set(Nat.is_eq(n, M.Switch.num(M.Setting.get(s, Lr.n.key(thread), "0"), 0n)), thread, n, now), Nil{}, Lr.send(s, now, thread, to, text))

def Lr.end.c(own: Bool, +s: M.State, +now: Nat, +thread: String, +by: String, +to: String, +e: Nat) -> Step:
  +n = {Bool.pick(Nat, own, Nat.add(M.Switch.num(M.Setting.get(s, Lr.n.key(thread), "0"), 0n), 1n), 0n) : Nat}
  +reply = {Lr.reply(Answer.of(M.State.mine(s, thread), thread)) : String}
  Lr.end.n(Nat.is_ge(n, Lr.loops()), s, now, thread, by, to, n, Lr.note(thread, Lr.title(Hub.thread(s, thread)), Lr.word(e, reply), reply))

def Lr.end.k(quiet: Bool, +s: M.State, +now: Nat, +thread: String, +by: String, +to: String, +e: Nat) -> Step:
  match quiet:
    case True{}:
      Step.none()
    case False{}:
      Lr.end.c(Bool.and(String.eq(M.Link.from(Lr.trig(M.State.mine(s, thread), thread)), by), Bool.not(String.is_empty(by))), s, now, thread, by, to, e)

# what the step tells the launcher (s: the state after the step)
def Lr.what.e(none: Bool, +e: Nat, +s: M.State, +now: Nat, +thread: String, +by: String, +to: String) -> Step:
  match none:
    case True{}:
      Step.none()
    case False{}:
      Lr.end.k(Bool.or(Bool.and(Nat.is_eq(e, 1n), Nat.is_gt(M.Work.busy(s, thread), 0n)), Bool.and(Bool.not(FR.Fr.is(to)), Lr.said(M.State.mine(s, thread), thread, to))), s, now, thread, by, to, e)

def Lr.what(asked: Bool, +ask: String, +e: Nat, +s: M.State, +now: Nat, +thread: String, +by: String, +to: String) -> Step:
  match asked:
    case True{}:
      Lr.send(s, now, thread, to, Lr.ask.note(thread, Lr.title(Hub.thread(s, thread)), ask))
    case False{}:
      Lr.what.e(Nat.is_eq(e, 0n), e, s, now, thread, by, to)

def Lr.tell.q(quiet: Bool, +to: String, +ask: String, +e: Nat, +s1: M.State, +now: Nat, +thread: String) -> Step:
  match quiet:
    case True{}:
      Step.none()
    case False{}:
      Lr.what(Bool.not(String.is_empty(ask)), ask, e, s1, now, thread, M.Launch.by(s1, thread), to)

def Task.by.id.k(r: Map<&2, M.Task> & M.Task) -> M.Task:
  (_, t) = r
  t

def Task.by.id(+s: M.State, +id: String) -> M.Task:
  match s:
    case M.State{_, _, _, _, tk, _, _, _, _, _, _}:
      Task.by.id.k(Map.get(M.Task, M.Task.blank(id), tk, id))

# the person cancels a delegated task from its row: its child stops, and
# the parent is told (once: TaskDelivered last); a finished task is left
def Task.cancel.user(t: M.Task, +s: M.State, +now: Nat) -> Step:
  match t:
    case M.Task{+id, +parent, +child, _, +st, _, _, +provider, +model}:
      Bool.pick(Step, M.TaskState.terminal(st), Step.none(),
        Step.after(Step.before([M.TaskStateSet{id, M.TaskCancelled{}, "cancelled by the user", now}, M.TurnChanged{child, M.Interrupted{}, now}], [AgentStop{child}],
          Hub.inbox.id(s, now, child, parent, M.System{}, Task.note(id, provider, model, "cancelled by the user", ""), Task.msg(child, id))), [M.TaskDelivered{id, now}]))

# Retiring a run
# --------------
# Whenever a thread's process is retired (Stop, archive, delete, a crash,
# a process that could not start) nothing it held is left waiting: its
# open asks close, and each parked request gets the empty answer (deny for
# an approval, "dismissed" for a question: Perm.verdict.k in mcp.bend); a
# plan card parks nothing and stays. A turn still running ends as
# interrupted and a delegated child's task ends with it (laws
# archive_*, interrupt_closes_approval, interrupt_keeps_plan,
# interrupt_ends_child_task, gone_closes_approval).

def Retire.hit(a: M.Ask, +th: String) -> Bool:
  match a:
    case M.Ask{_, t, kind, _, _, _, op, _}:
      Bool.and(op, Bool.and(String.eq(t, th), Bool.not(String.eq(kind, "plan"))))

def Retire.id(a: M.Ask) -> String:
  match a:
    case M.Ask{id, _, _, _, _, _, _, _}:
      id

def Retire.close(hit: Bool, +id: String, +now: Nat, rest: List<&2, M.Change>) -> List<&2, M.Change>:
  match hit:
    case True{}:
      M.AskClosed{id, "", now} <> rest
    case False{}:
      rest

def Retire.answer(hit: Bool, +id: String, rest: List<&2, Out>) -> List<&2, Out>:
  match hit:
    case True{}:
      AskAnswered{id, ""} <> rest
    case False{}:
      rest

# the open asks of th that wait on its process, closed unanswered
def Retire.asks(as: List<&2, M.Ask>, +th: String, +now: Nat) -> List<&2, M.Change>:
  match as:
    case Nil{}:
      Nil{}
    case +a <> t:
      Retire.close(Retire.hit(a, th), Retire.id(a), now, Retire.asks(t, th, now))

# the same asks' parked requests, answered empty
def Retire.answers(as: List<&2, M.Ask>, +th: String) -> List<&2, Out>:
  match as:
    case Nil{}:
      Nil{}
    case +a <> t:
      Retire.answer(Retire.hit(a, th), Retire.id(a), Retire.answers(t, th))

def Step.cat(a: Step, b: Step) -> Step:
  match a b:
    case Step{cs, os} Step{c2, o2}:
      Step{List.append(&2, M.Change, cs, c2), List.append(&2, Out, os, o2)}

# (a launcher that stopped the thread itself is not told it was stopped)
def Lr.tell.to(+to: String, +ask: String, +e: Nat, +s1: M.State, +now: Nat, +thread: String, +from: String, +cs: List<&2, M.Change>, os: List<&2, Out>) -> Step:
  Step.cat(Step{cs, os}, Lr.tell.q(Bool.or(String.eq(to, thread), Bool.and(Bool.not(String.is_empty(from)), String.eq(from, M.Launch.by(s1, thread)))), to, ask, e, s1, now, thread))

def Lr.tell.go(none: Bool, +ask: String, +e: Nat, +s: M.State, +now: Nat, +thread: String, +from: String, +cs: List<&2, M.Change>, os: List<&2, Out>) -> Step:
  match none:
    case True{}:
      Step{cs, os}
    case False{}:
      Lr.tell.to(Lr.dest(s, M.Launch.by(s, thread)), ask, e, M.State.replay(cs, s), now, thread, from, cs, os)

def Lr.tell.k(on: Bool, +s: M.State, +now: Nat, +thread: String, +from: String, +cs: List<&2, M.Change>, os: List<&2, Out>) -> Step:
  match on:
    case False{}:
      Step{cs, os}
    case True{}:
      Lr.tell.go(Bool.and(String.is_empty(Lr.ask(cs, thread)), Nat.is_eq(Lr.ended(cs, thread), 0n)), Lr.ask(cs, thread), Lr.ended(cs, thread), s, now, thread, from, cs, os)

# a step about a thread, with its report to the launcher after it (from:
# the thread whose call made the step, "" for an agent's own line or the
# person)
def Launch.tell.from(+s: M.State, +now: Nat, +thread: String, +from: String, st: Step) -> Step:
  match st:
    case Step{+cs, os}:
      Lr.tell.k(M.Launch.reports(s, thread), s, now, thread, from, cs, os)

def Launch.tell(+s: M.State, +now: Nat, +thread: String, st: Step) -> Step:
  Launch.tell.from(s, now, thread, "", st)

# a turn still running ends as interrupted, and its task with it; the
# server is told, so a phone's Live Activity drops it (law
# retire_stop_tells)
# (a launched thread's launcher hears of it, unless it stopped it itself:
# from, the thread whose call stopped it)
def Retire.turn.f(run: Bool, +s: M.State, +now: Nat, +th: String, +from: String, st: Step) -> Step:
  match run:
    case True{}:
      Launch.tell.from(s, now, th, from, Task.finish(s, now, th, Step.before([M.TurnChanged{th, M.Interrupted{}, now}], [TurnEnded{th, "stopped"}], st)))
    case False{}:
      st

def Retire.turn(run: Bool, +s: M.State, +now: Nat, +th: String, st: Step) -> Step:
  Retire.turn.f(run, s, now, th, "", st)

# Stop: the running turn is interrupted (Stop.outs), its asks close
def Retire.stop.f(+s: M.State, +now: Nat, +th: String, +from: String, os: List<&2, Out>) -> Step:
  Retire.turn.f(True{}, s, now, th, from, Step{Retire.asks(M.Asks.all(s), th, now),
    List.append(&2, Out, os, List.append(&2, Out, Stop.outs(Hub.provider(Hub.thread(s, th)), th, Hub.id("i", now, Hub.seq(s))), Retire.answers(M.Asks.all(s), th)))})

def Retire.stop(+s: M.State, +now: Nat, +th: String, os: List<&2, Out>) -> Step:
  Retire.stop.f(s, now, th, "", os)

def Hub.interrupt.k(run: Bool, +s: M.State, +now: Nat, +client: U32, +id: String, +thread: String) -> Step:
  match run:
    case True{}:
      Retire.stop(s, now, thread, [Reply.ok(client, id)])
    case False{}:
      Step.outs([Reply.ok(client, id)])

def Hub.interrupt(+s: M.State, +now: Nat, +client: U32, +id: String, +thread: String) -> Step:
  Hub.interrupt.k(Hub.running(Hub.thread(s, thread)), s, now, client, id, thread)

# archive and delete kill the process whatever it was doing: its asks and
# background rows go, and a running turn ends
def Retire.archive(+s: M.State, +now: Nat, +th: String, base: Step) -> Step:
  Retire.turn(Hub.running(Hub.thread(s, th)), s, now, th,
    Step.also(Step.after(base, List.append(&2, M.Change, Retire.asks(M.Asks.all(s), th, now), Bg.clear(s, th, now))), Retire.answers(M.Asks.all(s), th)))

# an archived or deleted thread is told nothing
def Lost.live(t: M.Thread) -> Bool:
  match t:
    case M.Thread{_, _, _, _, _, _, _, ar, _, _, _, _, _, _, _, _, _, _, _, _, de}:
      Bool.not(Bool.or(ar, de))

# the note itself: to an idle thread it starts a turn; behind a running
# turn or a held queue it waits
def Lost.note(held: Bool, +s: M.State, +now: Nat, +th: String, +text: String) -> Step:
  match held:
    case True{}:
      Hub.post.go(True{}, Hub.thread(s, th), Hub.cwd(s, Hub.thread(s, th)), M.Queue.of(s, th), now, th, M.Link.id("backplane", Hub.id("l", now, Hub.seq(s))), M.System{}, text)
    case False{}:
      Hub.inbox.id(s, now, "backplane", th, M.System{}, text, M.Link.id("backplane", Hub.id("l", now, Hub.seq(s))))

# Watched work
# ------------
# Work outside an agent's own task list (a long build, a job on another
# machine, a process already running) is the hub's to watch: work_run
# launches a command, work_watch follows a pid or a file (a restart of the
# hub that cuts a run off reads as lost, and the thread picks it back up), and when it ends the thread is told the outcome
# (it wakes, or the note waits behind its turn). Rows are WorkSet kind
# "watch" (setting watch.<thread>), written before anything is launched;
# the row's removal makes the telling happen once. An outcome that cannot
# be known (a pid gone or reused, a deadline passed) is never success
# (laws watch_*).
#
# A row: id, state ("launching", "watching", "cancelling", or ended:
# M.Bg.ended), "", note, "watch", kind, target, ident, deadline.

def Watch.line(+id: String, +state: String, +note: String, +kind: String, +target: String, +ident: String, +deadline: Nat) -> String:
  M.Bg.line(id, state) ++ "\t\t" ++ M.Bg.clean(note) ++ "\twatch\t" ++ M.Bg.clean(kind) ++ "\t" ++ M.Bg.enc(target) ++ "\t" ++ M.Bg.clean(ident) ++ "\t" ++ Nat.show(deadline)

def Watch.kind(+l: String) -> String:
  M.Bg.field(l, 5n)

def Watch.target(+l: String) -> String:
  M.Bg.dec(M.Bg.field(l, 6n))

def Watch.ident(+l: String) -> String:
  M.Bg.field(l, 7n)

def Watch.deadline(+l: String) -> Nat:
  M.Switch.num(M.Bg.field(l, 8n), 0n)

def Watch.state(+l: String) -> String:
  M.Bg.doing(l)

def Watch.put(+th: String, +now: Nat, ls: List<&2, String>) -> M.Change:
  M.WorkSet{th, "watch", M.Bg.show(ls), now}

def Watch.find(ls: List<&2, String>, +id: String) -> String:
  match ls:
    case Nil{}:
      ""
    case +l <> t:
      Bool.pick(String, M.Bg.is(l, id), l, Watch.find(t, id))

def Watch.set.k(hit: Bool, +l: String, +state: String) -> String:
  match hit:
    case True{}:
      M.Bg.line(M.Bg.field(l, 0n), state) ++ M.Bg.rest(l)
    case False{}:
      l

# row id's state set
def Watch.set(ls: List<&2, String>, +id: String, +state: String) -> List<&2, String>:
  match ls:
    case Nil{}:
      Nil{}
    case +l <> t:
      Watch.set.k(M.Bg.is(l, id), l, state) <> Watch.set(t, id, state)

def Watch.ident.k(hit: Bool, +l: String, +ident: String) -> String:
  match hit:
    case True{}:
      Watch.line(M.Bg.field(l, 0n), Watch.state(l), M.Bg.title(l), Watch.kind(l), Watch.target(l), ident, Watch.deadline(l))
    case False{}:
      l

def Watch.ident.set(ls: List<&2, String>, +id: String, +ident: String) -> List<&2, String>:
  match ls:
    case Nil{}:
      Nil{}
    case +l <> t:
      Watch.ident.k(M.Bg.is(l, id), l, ident) <> Watch.ident.set(t, id, ident)

def Watch.deadline.of(+now: Nat, +secs: Nat) -> Nat:
  Bool.pick(Nat, Nat.is_eq(secs, 0n), 0n, Nat.add(now, secs))

# a new row's id: the thread's and the log's length
def Watch.id(+s: M.State, +th: String) -> String:
  "w-" ++ th ++ "-" ++ Nat.show(Hub.seq(s))

# work to watch, recorded before its watcher (and for kind run, its
# launch) starts: the Await goes out with the step that writes the row
# (law watch_intent_before_launch)
def Watch.start(+s: M.State, +now: Nat, +th: String, +kind: String, +target: String, +note: String, +secs: Nat) -> Step:
  Step{[Watch.put(th, now, List.append(&2, String, M.Work.watch(s, th), [Watch.line(Watch.id(s, th), Bool.pick(String, String.eq(kind, "run"), "launching", "watching"), note, kind, target, Bool.pick(String, String.eq(kind, "file"), Watch.id(s, th), "-"), Watch.deadline.of(now, secs))]))],
    [Await{th, Watch.id(s, th), kind, target, Bool.pick(String, String.eq(kind, "file"), Watch.id(s, th), "-"), Watch.deadline.of(now, secs), Hub.cwd(s, Hub.thread(s, th))}]}

# the watcher found what proves the work is the same (a unit's
# invocation, a pid's boot and start time)
def Watch.identified(+s: M.State, +now: Nat, +th: String, +id: String, +ident: String) -> Step:
  Step{[Watch.put(th, now, Watch.ident.set(M.Work.watch(s, th), id, ident))], Nil{}}

def Watch.launched.k(launching: Bool, +s: M.State, +now: Nat, +th: String, +id: String) -> Step:
  match launching:
    case True{}:
      Step{[Watch.put(th, now, Watch.set(M.Work.watch(s, th), id, "watching"))], Nil{}}
    case False{}:
      Step.none()

# the launched command is running (a row cancelling or ended meanwhile is
# left as it is)
def Watch.launched(+s: M.State, +now: Nat, +th: String, +id: String) -> Step:
  Watch.launched.k(String.eq(Watch.state(Watch.find(M.Work.watch(s, th), id)), "launching"), s, now, th, id)

def Watch.word(+status: String, cancelling: Bool) -> String:
  Bool.pick(String, Bool.and(String.eq(status, "gone"), cancelling), "cancelled",
    Bool.pick(String, String.eq(status, "gone"), "it ended, outcome unknown (not known to have succeeded)", status))

def Watch.text(+note: String, +word: String, +detail: String) -> String:
  "[Backplane] Watched work \"" ++ note ++ "\" ended: " ++ word ++ Bool.pick(String, String.is_empty(detail), "", "\n" ++ detail)

def Watch.tell.k(live: Bool, +s: M.State, +now: Nat, +th: String, +text: String) -> Step:
  match live:
    case True{}:
      Lost.note(Bool.or(Hub.running(Hub.thread(s, th)), M.Queue.waiting(s, th)), s, now, th, text)
    case False{}:
      Step.none()

# an archived or deleted thread is told nothing (law watch_end_retired_silent)
def Watch.tell(+s: M.State, +now: Nat, +th: String, +text: String) -> Step:
  Watch.tell.k(Lost.live(Hub.thread(s, th)), s, now, th, text)

def Watch.end.k(found: Bool, timeout: Bool, +l: String, +s: M.State, +now: Nat, +th: String, +id: String, +status: String, +detail: String) -> Step:
  match found timeout:
    case False{} _:
      Step.none()
    case True{} True{}:
      Step.cat(Step{[Watch.put(th, now, Watch.set(M.Work.watch(s, th), id, M.Bg.ended.mark() ++ "unknown"))], Nil{}},
        Watch.tell(s, now, th, Watch.text(M.Bg.title(l), "timed out, still running, outcome unknown", detail)))
    case True{} False{}:
      Step.cat(Step{[Watch.put(th, now, M.Bg.drop(M.Work.watch(s, th), id))], Nil{}},
        Watch.tell(s, now, th, Watch.text(M.Bg.title(l), Watch.word(status, String.eq(Watch.state(l), "cancelling")), detail)))

# watched work ended (status: "exit <code>", "gone", "timeout", or the
# file's "found"): told once, from the row that is still there; a timeout
# keeps it, ended with outcome unknown (laws watch_end_*, watch_timeout_*)
def Watch.end(+s: M.State, +now: Nat, +th: String, +id: String, +status: String, +detail: String) -> Step:
  Watch.end.k(Bool.and(Bool.not(String.is_empty(Watch.find(M.Work.watch(s, th), id))), Bool.not(M.Bg.ended(Watch.find(M.Work.watch(s, th), id)))),
    String.eq(status, "timeout"), Watch.find(M.Work.watch(s, th), id), s, now, th, id, status, detail)

def Watch.cancel.k(live: Bool, +l: String, +s: M.State, +now: Nat, +th: String, +id: String) -> Step:
  match live:
    case True{}:
      Step{[Watch.put(th, now, Watch.set(M.Work.watch(s, th), id, "cancelling"))], [WatchStop{th, id, Watch.kind(l), Watch.target(l), Watch.ident(l)}]}
    case False{}:
      Step.none()

# owned work stopped: the row says "cancelling" until its watcher
# confirms the end (then "cancelled") or that it cannot know (law
# watch_cancel_keeps_until_confirmed)
def Watch.cancel(+s: M.State, +now: Nat, +th: String, +id: String) -> Step:
  Watch.cancel.k(Bool.and(Bool.not(String.is_empty(Watch.find(M.Work.watch(s, th), id))), Bool.not(M.Bg.ended(Watch.find(M.Work.watch(s, th), id)))),
    Watch.find(M.Work.watch(s, th), id), s, now, th, id)

# no longer watched: it may still run, and its watcher stops (law
# watch_unwatch_says_may_run)
def Watch.unwatch.k(+l: String, +s: M.State, +now: Nat, +th: String, +id: String) -> Step:
  Step{[Watch.put(th, now, M.Bg.drop(M.Work.watch(s, th), id))], [WatchStop{th, id, "unwatch", Watch.target(l), Watch.ident(l)}]}

def Watch.unwatch(+s: M.State, +now: Nat, +th: String, +id: String) -> Step:
  Watch.unwatch.k(Watch.find(M.Work.watch(s, th), id), s, now, th, id)

def Watch.retire.outs(ls: List<&2, String>, +th: String) -> List<&2, Out>:
  match ls:
    case Nil{}:
      Nil{}
    case +l <> t:
      WatchStop{th, M.Bg.field(l, 0n), "unwatch", Watch.target(l), Watch.ident(l)} <> Watch.retire.outs(t, th)

# an archived or deleted thread's watched work is no longer watched (it may
# still run): its rows go and every watcher stops (law watch_retire_unwatches)
def Watch.retire(+s: M.State, +now: Nat, +th: String) -> Step:
  Step{[Watch.put(th, now, Nil{})], Watch.retire.outs(M.Work.watch(s, th), th)}

def Watch.unwatch.text() -> String:
  "no longer watched; it may still run"

def Watch.rearm.one(ended: Bool, +l: String, +th: String, +cwd: String, rest: List<&2, Out>) -> List<&2, Out>:
  match ended:
    case True{}:
      rest
    case False{}:
      Await{th, M.Bg.field(l, 0n), Watch.kind(l), Watch.target(l), Watch.ident(l), Watch.deadline(l), cwd} <> rest

def Watch.rearm.rows(ls: List<&2, String>, +th: String, +cwd: String) -> List<&2, Out>:
  match ls:
    case Nil{}:
      Nil{}
    case +l <> t:
      Watch.rearm.one(M.Bg.ended(l), l, th, cwd, Watch.rearm.rows(t, th, cwd))

def Watch.rearm.go(ts: List<&2, M.Thread>, +s: M.State) -> List<&2, Out>:
  match ts:
    case Nil{}:
      Nil{}
    case +t <> rest:
      List.append(&2, Out, Bool.pick(List<&2, Out>, String.contains(M.Thread.id(t), "~"), Nil{}, Watch.rearm.rows(M.Work.watch(s, M.Thread.id(t)), M.Thread.id(t), Hub.cwd(s, t))), Watch.rearm.go(rest, s))

# after a restart every watched work still open is watched again (a run
# still launching is launched only if it never ran); a mirror never
# watches (laws recover_rearms_watches, mirror_never_awaits)
def Watch.rearm(+s: M.State) -> List<&2, Out>:
  match s:
    case M.State{_, +ts, _, _, _, _, _, _, _, _, _}:
      Watch.rearm.go(Map.values(&2, M.Thread, ts), s)

# a thread t1 watching a build (run: its turn running; far: the thread a
# mirror's, "p1~t1"); the laws' example
def Watch.demo.at(+run: Bool, +th: String) -> M.State:
  M.State.replay(List.append(&2, M.Change, [M.ProjectCreated{"p1", "p", "/p", 1n}, M.ThreadCreated{th, "p1", "x", M.Local{}, "", "", "claude", "", 1n},
    M.WorkSet{th, "watch", Watch.line("w-1", "watching", "build", "run", "make", "-", 0n), 2n}], Bool.pick(List<&2, M.Change>, run, [M.TurnChanged{th, M.Running{}, 3n}], Nil{})), M.State.empty())

def Watch.demo(+run: Bool) -> M.State:
  Watch.demo.at(run, "t1")

def Watch.again(+s: M.State, st: Step) -> M.State:
  M.State.replay(Queue.changes(st), s)

def Watch.demo.archived() -> M.State:
  Watch.again(Watch.demo(False{}), Step{[M.ThreadArchived{"t1", True{}, 4n}], Nil{}})

def Watch.awaits(os: List<&2, Out>) -> Nat:
  match os:
    case Nil{}:
      0n
    case Await{_, _, _, _, _, _, _} <> t:
      1n+Watch.awaits(t)
    case _ <> t:
      Watch.awaits(t)

def Watch.told(es: List<&2, M.Entry>, +word: String) -> Bool:
  match es:
    case Nil{}:
      False{}
    case M.Msg{_, _, _, +text, _} <> t:
      Bool.or(String.contains(text, word), Watch.told(t, word))
    case _ <> t:
      Watch.told(t, word)

# Updating waits for work
# -----------------------
# An update restarts the hub, which ends every agent: asked while threads
# run turns or wait on work (M.Work.threads), it waits (the private
# setting update.wait, the time it was asked) and goes when they are done,
# after 30 minutes, or on update.now (laws update_wait_*).
def Update.limit() -> Nat:
  1800n

def Update.ask(busy: Bool, +now: Nat, +client: U32, +id: String) -> Step:
  match busy:
    case True{}:
      Step{[M.SettingSet{"update.wait", Nat.show(now), now}], [Reply.ok(client, id)]}
    case False{}:
      Step.outs([Reply.ok(client, id), Upgrade{}])

def Update.due.k(go: Bool, force: Bool, +now: Nat) -> Step:
  match go:
    case True{}:
      Step{[M.SettingSet{"update.wait", "", now}], [Bool.pick(Out, force, UpgradeForce{}, Upgrade{})]}
    case False{}:
      Step.none()

def Update.late(+since: String, +now: Nat) -> Bool:
  Nat.is_ge(now, Nat.add(M.Switch.num(since, 0n), Update.limit()))

def Update.due.w(waiting: Bool, +s: M.State, +now: Nat) -> Step:
  match waiting:
    case True{}:
      Update.due.k(Bool.or(Nat.is_eq(M.Work.threads(s), 0n), Update.late(M.Setting.get(s, "update.wait", "0"), now)),
        Update.late(M.Setting.get(s, "update.wait", "0"), now), now)
    case False{}:
      Step.none()

# the tick's check of a waiting update
def Update.due(+s: M.State, +now: Nat) -> Step:
  Update.due.w(Bool.not(String.is_empty(M.Setting.get(s, "update.wait", ""))), s, now)

# An install is ready to restart into. The busy check is made now, not
# when the update was asked: threads that started during the download hold
# the restart (the private setting update.restart, the time it was held)
# until they are done or the wait is up; a forced update goes at once.
def Update.ready.k(go: Bool, +held: String, +now: Nat) -> Step:
  match go:
    case True{}:
      Step{[M.SettingSet{"update.restart", "", now}], [Restart{}]}
    case False{}:
      Step{[M.SettingSet{"update.restart", Bool.pick(String, String.is_empty(held), Nat.show(now), held), now}], []}

def Update.ready(+s: M.State, force: Bool, +now: Nat) -> Step:
  Update.ready.k(Bool.or(force, Nat.is_eq(M.Work.threads(s), 0n)), M.Setting.get(s, "update.restart", ""), now)

def Update.held.w(waiting: Bool, +s: M.State, +now: Nat) -> Step:
  match waiting:
    case True{}:
      Update.ready.k(Bool.or(Nat.is_eq(M.Work.threads(s), 0n), Update.late(M.Setting.get(s, "update.restart", "0"), now)), M.Setting.get(s, "update.restart", ""), now)
    case False{}:
      Step.none()

# the tick's check of a held restart
def Update.held(+s: M.State, +now: Nat) -> Step:
  Update.held.w(Bool.not(String.is_empty(M.Setting.get(s, "update.restart", ""))), s, now)

def Hub.request.k(i: Nat, +s: M.State, +now: Nat, +client: U32, +id: String, +p: J.Json, +m: String) -> Step:
  match i:
    case 0n:
      Step.outs([Fs{client, id, "add", J.Json.get_str(p, "path", "")}])
    case 1n:
      Step.reply(client, id, [M.ProjectRemoved{J.Json.get_str(p, "project", ""), now}])
    case 2n:
      Hub.thread_create(s, now, client, id, p)
    case 3n:
      Hub.turn.mode(Bool.or(String.is_empty(J.Json.get_str(p, "mode", "")), String.eq(J.Json.get_str(p, "mode", ""), "auto")), J.Json.get_str(p, "mode", ""),
        s, now, client, id, J.Json.get_str(p, "thread", ""), J.Json.get_str(p, "text", ""), J.Json.get_str(p, "msg", ""))
    case 4n:
      Hub.interrupt(s, now, client, id, J.Json.get_str(p, "thread", ""))
    case 5n:
      Step.reply(client, id, [M.ThreadSettled{J.Json.get_str(p, "thread", ""), True{}, now}])
    case 6n:
      Step.reply(client, id, [M.ThreadUnsettled{J.Json.get_str(p, "thread", ""), now}])
    case 7n:
      Step.cat(Retire.archive(s, now, J.Json.get_str(p, "thread", ""), Step{[M.ThreadArchived{J.Json.get_str(p, "thread", ""), True{}, now}], [Reply.ok(client, id), AgentStop{J.Json.get_str(p, "thread", "")}]}), Watch.retire(s, now, J.Json.get_str(p, "thread", "")))
    case 8n:
      Step.reply(client, id, [M.ThreadArchived{J.Json.get_str(p, "thread", ""), False{}, now}])
    case 9n:
      Step.reply(client, id, [M.ThreadPinned{J.Json.get_str(p, "thread", ""), True{}, now}])
    case 10n:
      Step.reply(client, id, [M.ThreadPinned{J.Json.get_str(p, "thread", ""), False{}, now}])
    # a rename: the title trimmed; a blank one changes nothing
    case 11n:
      Step.reply(client, id, Rename.changes(J.Json.get_str(p, "thread", ""), String.trim(J.Json.get_str(p, "title", "")), now))
    case 12n:
      Step.cat(Retire.archive(s, now, J.Json.get_str(p, "thread", ""), Step{[M.ThreadDeleted{J.Json.get_str(p, "thread", ""), now}], [Reply.ok(client, id), AgentStop{J.Json.get_str(p, "thread", "")}, BotFx{"gone", J.Json.get_str(p, "thread", ""), "", ""}]}), Watch.retire(s, now, J.Json.get_str(p, "thread", "")))
    case 13n:
      Step.outs([Reply.ok(client, id), Watch{client, J.Json.get_str(p, "kind", ""), J.Json.get_str(p, "root", ""), J.Json.get_str(p, "path", "")}])
    case 14n:
      Hub.modes(s, now, client, id, p, Hub.thread(s, J.Json.get_str(p, "thread", "")))
    case 15n:
      Update.ask(Nat.is_gt(M.Work.threads(s), 0n), now, client, id)
    case 16n:
      Step.reply(client, id, [M.SettingSet{J.Json.get_str(p, "key", ""), J.Json.get_str(p, "value", ""), now}])
    case 17n:
      Step.outs([Reply.ok(client, id), KiCadInstall{}])
    case 18n:
      Hub.answer(s, now, client, id, J.Json.get_str(p, "id", ""), J.Json.get_str(p, "answer", ""))
    case 19n:
      Device.register(now, client, id, p)
    case 20n:
      Step.reply(client, id, [M.DeviceDropped{J.Json.get_str(p, "token", ""), now}])
    case 21n:
      Step.outs([PlotInfo{client, id, J.Json.get_str(p, "key", ""), Maybe.default(&2, Nat, Nat.read(J.Json.as_raw(J.Json.get(p, "at"), "0")), 0n),
        Maybe.default(&2, Nat, Nat.read(J.Json.as_raw(J.Json.get(p, "idx"), "0")), 0n)}])
    case 22n:
      Hub.resume(s, now, client, id, J.Json.get_str(p, "thread", ""))
    case 23n:
      Queue.op(0n, s, now, client, id, p)
    case 24n:
      Queue.op(1n, s, now, client, id, p)
    case 25n:
      Queue.op(2n, s, now, client, id, p)
    case 26n:
      Queue.op(3n, s, now, client, id, p)
    case 27n:
      Hub.fork(s, now, client, id, J.Json.get_str(p, "thread", ""))
    case 28n:
      Hub.back(s, now, client, id, J.Json.get_str(p, "thread", ""))
    case 29n:
      Hub.switch(s, now, client, id, J.Json.get_str(p, "thread", ""), J.Json.get_str(p, "provider", ""))
    case 30n:
      Step{[M.SettingSet{"update.wait", "", now}], [Reply.ok(client, id), UpgradeForce{}]}
    # the person stops watched work or a delegated task from its row
    case 31n:
      Step.before(Nil{}, [Reply.ok(client, id)], Watch.cancel(s, now, J.Json.get_str(p, "thread", ""), J.Json.get_str(p, "id", "")))
    case 32n:
      Step.before(Nil{}, [Reply.ok(client, id)], Task.cancel.user(Maybe.default(&2, M.Task, Task.of_child(s, J.Json.get_str(p, "child", "")), M.Task.blank("")), s, now))
    case _:
      Hub.more(C.Codec.pos(Hub.methods.git(), m), s, now, client, id, p, m)

# one client request {"id": n, "m": method, "p": params}
def Hub.request(+s: M.State, +now: Nat, +client: U32, +o: J.Json) -> Step:
  Hub.request.k(C.Codec.pos(Hub.methods(), J.Json.get_str(o, "m", "")), s, now, client,
    J.Json.as_raw(J.Json.get(o, "id"), "0"), J.Json.get_obj(o, "p"), J.Json.get_str(o, "m", ""))

def Hub.request_text.k(m: Maybe<&2, J.Json>, +s: M.State, +now: Nat, +client: U32) -> Step:
  match m:
    case Some{o}:
      Hub.request(s, now, client, o)
    case None{}:
      Step.outs([Reply.err(client, "0", "malformed request")])

def Hub.request_text(+s: M.State, +now: Nat, +client: U32, text: String) -> Step:
  Hub.request_text.k(J.Json.parse(text), s, now, client)

# one line of an agent's stdout
def Hub.agent_line(+s: M.State, +now: Nat, +thread: String, line: String) -> Step:
  Launch.tell(s, now, thread, Task.finish.w(s, now, thread, Queue.drain(s, now, thread, Ended.out(s, now, thread, Hub.agent_line.k(Bool.not(String.eq(Hub.provider(Hub.thread(s, thread)), "claude")), s, now, thread, line)))))

# Claude's lines by the command ledger (core/cmds.bend)
# ------------------------------------------------------
# The server keeps each Claude process's ledger and judges every line
# before it comes here (CM.Cmds.verdict): 0 is the old rule above
# (Hub.agent_line, a CLI without lifecycles); otherwise a result never
# ends the turn by itself: 1 quiet (a stopped or foreign turn's line: it
# lands, its text does not stream), 2 the work on hand (it streams), 3/4
# the turn ends ok/failed (`text` the outcome's), 5 the CLI cancelled the
# command itself (interrupted), 6 Claude started a turn by itself (a
# background task ended, a wakeup): the thread runs again, with an Act
# saying why (laws cmds_*).

# a line as the old rule takes it, except that whether its text streams is
# the ledger's call and a result changes no turn
def Claude.line.r(i: Nat, run: Bool, +s: M.State, +thread: String, +now: Nat, +o: J.Json) -> Step:
  match i:
    case 1n:
      Claude.stream.if(run, thread, J.Json.get_obj(o, "event"))
    case 3n:
      Step{Usage.window(s, thread, now, o), Nil{}}
    case _:
      Claude.line.k(i, s, thread, now, o)

def Claude.line.who.r(main: Bool, i: Nat, run: Bool, +s: M.State, +thread: String, +now: Nat, +call: String, +o: J.Json) -> Step:
  match main:
    case True{}:
      Claude.line.r(i, run, s, thread, now, o)
    case False{}:
      Sub.line(i, thread, now, call, o)

def Claude.line.o(run: Bool, +s: M.State, +thread: String, +now: Nat, +o: J.Json) -> Step:
  Claude.line.who.r(String.is_empty(J.Json.get_str(o, "parent_tool_use_id", "")), C.Codec.pos(["system", "stream_event", "assistant", "result", "user"], J.Json.get_str(o, "type", "")),
    run, s, thread, now, J.Json.get_str(o, "parent_tool_use_id", ""), o)

def Wake.row(ls: List<&2, String>, +id: String) -> String:
  match ls:
    case Nil{}:
      ""
    case +l <> t:
      Bool.pick(String, M.Bg.is(l, id), M.Bg.title(l), Wake.row(t, id))

def Wake.or(+a: String, +b: String) -> String:
  Bool.pick(String, String.is_empty(a), b, a)

# what woke the agent: the notification's summary, else its task's title,
# else background work
def Wake.title(+s: M.State, +thread: String, +o: J.Json) -> String:
  Wake.or(J.Json.get_str(o, "summary", ""), Wake.or(Wake.row(M.Bg.of(s, thread), J.Json.get_str(o, "task_id", "")), "background work"))

def Wake.text(lc: Bool, +s: M.State, +thread: String, +o: J.Json) -> String:
  match lc:
    case True{}:
      "Resumed after Stop"
    case False{}:
      "Woken: " ++ Wake.title(s, thread, o)

# a turn Claude started by itself runs where the person sees it
def Wake.start(+s: M.State, +thread: String, +now: Nat, +text: String) -> List<&2, M.Change>:
  [M.TurnChanged{thread, M.Running{}, now}, M.ActivityLogged{thread, Hub.id("k", now, Hub.seq(s)), "info", "wake", text, now}]

def Claude.line.v.k(v: Nat, +s: M.State, +thread: String, +now: Nat, +o: J.Json, +text: String) -> Step:
  match v:
    case 2n:
      Claude.line.o(True{}, s, thread, now, o)
    case 3n:
      Step.after(Step.after(Claude.result.k(False{}, thread, now, text), Usage.window(s, thread, now, o)), Bg.settle(s, thread, now))
    case 4n:
      Step.after(Step.after(Claude.result.k(True{}, thread, now, text), Usage.window(s, thread, now, o)), Bg.settle(s, thread, now))
    case 5n:
      Step{List.append(&2, M.Change, [M.TurnChanged{thread, M.Interrupted{}, now}], Bg.settle(s, thread, now)), Nil{}}
    case 6n:
      Step.before(Wake.start(s, thread, now, Wake.text(String.eq(J.Json.get_str(o, "type", ""), "command_lifecycle"), s, thread, o)), Nil{},
        Claude.line.o(True{}, s, thread, now, o))
    case _:
      Claude.line.o(False{}, s, thread, now, o)

def Hub.agent_line.v.k(v: Nat, +s: M.State, +now: Nat, +thread: String, +o: J.Json, +text: String) -> Step:
  Launch.tell(s, now, thread, Task.finish.w(s, now, thread, Queue.drain(s, now, thread, Ended.out(s, now, thread, Claude.line.v.k(v, s, thread, now, o, text)))))

# one line of an agent's stdout with its verdict (o: the line parsed;
# text: an ending's outcome, CM.Cmds.text)
def Hub.agent_line.vo(v: Nat, +s: M.State, +now: Nat, +thread: String, line: String, +o: J.Json, +text: String) -> Step:
  match v:
    case 0n:
      Hub.agent_line(s, now, thread, line)
    case 1n+p:
      Hub.agent_line.v.k(1n+p, s, now, thread, o, text)

def Hub.agent_line.v(v: Nat, +s: M.State, +now: Nat, +thread: String, +line: String, +text: String) -> Step:
  Hub.agent_line.vo(v, s, now, thread, line, Maybe.default(&2, J.Json, J.Json.parse(line), J.Null{}), text)

# Lost work
# ---------
# When the process that owned a thread's background work goes (a restart,
# the agent's exit, its replacement for a new model or mode), the work is
# no longer anyone's: the agent is told once, by a message naming each
# task, and its rows go in the same step (laws recover_lost_*, gone_*,
# replaced_*). With "restart.continue" off nothing is started: an Act
# says the same, and the rows stay, ended with status "unknown", until a
# turn ends them. A thread with a queue held (after a Stop) gets the note
# queued behind it, never drained.

def Lost.why(+why: String) -> String:
  Bool.pick(String, String.eq(why, "restart"), "A restart of Backplane",
    Bool.pick(String, String.eq(why, "exit"), "The agent process exiting",
      Bool.pick(String, String.eq(why, "replaced"), "Restarting the agent for a new model or mode", why)))

def Lost.row(+l: String) -> String:
  "- " ++ Bool.pick(String, String.is_empty(M.Bg.kind.of(l)), "task", M.Bg.kind.of(l)) ++ " " ++ M.Bg.field(l, 0n) ++ ": " ++ M.Work.row.title(l)
    ++ Bool.pick(String, M.Bg.ended(l), " (it had ended: " ++ M.Bg.status(l) ++ ", its outcome not yet handed to you)", "")

def Lost.rows(ls: List<&2, String>) -> String:
  match ls:
    case Nil{}:
      ""
    case +l <> t:
      "\n" ++ Lost.row(l) ++ Lost.rows(t)

# what the agent (or, with restart.continue off, the timeline) is told
def Lost.text(ls: List<&2, String>, +why: String) -> String:
  "[Backplane] " ++ Lost.why(why) ++ " ended your background work. Ownership was lost; the outcome is unknown." ++ Lost.rows(ls)
    ++ "\nCheck what finished before rerunning."

def Lost.new(ls: List<&2, String>) -> Bool:
  match ls:
    case Nil{}:
      False{}
    case +l <> t:
      Bool.or(Bool.not(String.eq(M.Bg.status(l), "unknown")), Lost.new(t))

# every row ended, its outcome unknown
def Lost.unknown(ls: List<&2, String>) -> List<&2, String>:
  match ls:
    case Nil{}:
      Nil{}
    case +l <> t:
      (M.Bg.line(M.Bg.field(l, 0n), M.Bg.ended.mark() ++ "unknown") ++ M.Bg.rest(l)) <> Lost.unknown(t)

def Bg.lost.k(has: Bool, on: Bool, +s: M.State, +now: Nat, +th: String, +why: String, +ls: List<&2, String>) -> Step:
  match has on:
    case False{} _:
      Step.none()
    case True{} True{}:
      Step.cat(Lost.note(M.Queue.waiting(s, th), s, now, th, Lost.text(ls, why)), Step{[M.WorkSet{th, "bg", "", now}], Nil{}})
    case True{} False{}:
      Step{[M.ActivityLogged{th, Hub.id("l", now, Hub.seq(s)), "info", "lost", Lost.text(ls, why), now}, M.WorkSet{th, "bg", M.Bg.show(Lost.unknown(ls)), now}], Nil{}}

# a thread's background work lost (why: "restart", "exit", "replaced"):
# told once; nothing for a thread with none new, or archived or deleted
def Bg.lost(+s: M.State, +now: Nat, +th: String, +why: String) -> Step:
  Bg.lost.k(Bool.and(Lost.new(M.Bg.of(s, th)), Lost.live(Hub.thread(s, th))), M.Carry.on(s), s, now, th, why, M.Bg.of(s, th))

def Bg.gone(live: Bool, +s: M.State, +now: Nat, +th: String) -> Step:
  match live:
    case True{}:
      Bg.lost(s, now, th, "exit")
    case False{}:
      Step{Bg.clear(s, th, now), Nil{}}

# an agent process ended; a turn still running ends with it, and the
# background work it owned is told lost (behind the failed turn's queue
# when one ran: law gone_idle_told)
def Hub.agent_gone.why(+s: M.State, +now: Nat, +thread: String, +code: U32, +why: String) -> Step:
  Step.before(Nil{}, Retire.answers(M.Asks.all(s), thread),
    Step.cat(Step.after(Launch.tell(s, now, thread, Task.finish(s, now, thread, Queue.drain(s, now, thread, Ended.out(s, now, thread, Hub.agent_gone.k(Hub.running(Hub.thread(s, thread)), thread, now, code, why))))),
      Retire.asks(M.Asks.all(s), thread, now)), Bg.gone(Lost.live(Hub.thread(s, thread)), s, now, thread)))

def Hub.agent_gone(+s: M.State, +now: Nat, +thread: String, +code: U32) -> Step:
  Hub.agent_gone.why(s, now, thread, code, "")

def Step.outs_of(st: Step) -> List<&2, Out>:
  match st:
    case Step{_, os}:
      os

# Worktrees on the tick
# ---------------------
# A new thread whose worktree never came (the server restarted while
# making it) runs what waited for it in the project folder once the wait
# is over. A thread whose worktree was removed never does: its next
# message makes the worktree again.

def Wt.stale.k(stale: Bool, +s: M.State, +now: Nat, +th: String, rest: Step) -> Step:
  match stale:
    case True{}:
      Step.before(Queue.changes(Wt.start(s, now, th, False{})), Step.outs_of(Wt.start(s, now, th, False{})), rest)
    case False{}:
      rest

def Wt.stale(ts: List<&2, M.Thread>, +s: M.State, +now: Nat) -> Step:
  match ts:
    case Nil{}:
      Step.none()
    case +t <> rest:
      Wt.stale.k(Bool.and(Wt.stale.one(t, now), M.Queue.waiting(s, M.Thread.id(t))), s, now, M.Thread.id(t), Wt.stale(rest, s, now))

# Removing worktrees
# ------------------
# Each tick, the worktrees of threads put away long enough (or merged) are
# removed by the server (M.Worktree.drop), which reports thread.unrooted.

# how long a settled thread keeps its worktree: the "worktree.days"
# setting (1..90), else 3 days
def Hub.wt.window(+s: M.State) -> Nat:
  Nat.mul(Nat.max(1n, Nat.min(90n, Maybe.default(&2, Nat, Nat.read(M.Setting.get(s, "worktree.days", "3")), 3n))), 86400n)

def Drop.k(go: Bool, +s: M.State, +t: M.Thread, rest: List<&2, Out>) -> List<&2, Out>:
  match go:
    case True{}:
      Git{0, "", "wt_remove", M.Thread.id(t), Hub.worktree(t), Hub.project.root(Hub.project(s, Hub.thread_project(t))), M.Merge.branch(t)} <> rest
    case False{}:
      rest

def Drop.asks(ts: List<&2, M.Thread>, +kids: Map<&2, Nat>, +s: M.State, +now: Nat, +w: Nat) -> List<&2, Out>:
  match ts:
    case Nil{}:
      Nil{}
    case +t <> rest:
      Drop.k(M.Worktree.drop.w(s, t, now, w, M.Work.busy.at(kids, s, M.Thread.id(t))), s, t, Drop.asks(rest, kids, s, now, w))

# Scratch threads left alone long enough (M.Scratch.expired) are deleted,
# like thread.delete, and the server removes their folders
def Scratch.gone.k(go: Bool, +th: String, +now: Nat, rest: Step) -> Step:
  match go:
    case True{}:
      Step.before([M.ThreadDeleted{th, now}], [AgentStop{th}, BotFx{"gone", th, "", ""}, BotFx{"scratch-gone", th, "", ""}], rest)
    case False{}:
      rest

def Scratch.sweep(ts: List<&2, M.Thread>, +kids: Map<&2, Nat>, +s: M.State, +now: Nat, +w: Nat) -> Step:
  match ts:
    case Nil{}:
      Step.none()
    case +t <> rest:
      Scratch.gone.k(M.Scratch.expired.w(now, w, t, M.Work.busy.at(kids, s, M.Thread.id(t))), M.Thread.id(t), now, Scratch.sweep(rest, kids, s, now, w))

def Step.join(a: Step, b: Step) -> Step:
  match a:
    case Step{cs, os}:
      Step.before(cs, os, b)

# every thread that should settle now, settled (a snoozed one never is);
# gh asked about due merges; worktrees put away long enough removed;
# stranded worktree waits run; expired scratch threads deleted
# (nothing settles, expires or loses its worktree while it waits on work:
# M.Work.busy, the children counted once for all threads)
def Hub.tick.k(+s: M.State, +now: Nat, +kids: Map<&2, Nat>) -> Step:
  match s:
    case M.State{ps, +ts, _, _, _, _, _, _, _, _, _}:
      Step.before(Hub.settle.go(Map.values(&2, M.Thread, ts), M.State.navs(s), kids, s, now, Hub.settle.window(s)),
        List.append(&2, Out, Merge.asks(Map.values(&2, M.Project, ps), Map.values(&2, M.Thread, ts), s), Drop.asks(Map.values(&2, M.Thread, ts), kids, s, now, Hub.wt.window(s))),
        Step.join(Wt.stale(Map.values(&2, M.Thread, ts), s, now), Scratch.sweep(Map.values(&2, M.Thread, ts), kids, s, now, M.Scratch.window(s))))

def Hub.tick(+s: M.State, +now: Nat) -> Step:
  Step.join(Step.join(Step.join(Hub.tick.k(s, now, M.Work.kids.map(s)), Update.due(s, now)), Update.held(s, now)), Lr.far.tick(s))

# Asks keep their thread busy
# ---------------------------
# A thread with an open ask (an approval, a question, a plan) waits on the
# user: every step that opens or closes one also sets the thread's pending
# count to its open asks, so it never auto-settles meanwhile (laws
# ask_makes_pending, answer_clears_pending).

def Pend.one(hit: Bool, n: Nat) -> Nat:
  match hit:
    case True{}:
      1n+n
    case False{}:
      n

def Pend.open(as: List<&2, M.Ask>, +th: String) -> Nat:
  match as:
    case Nil{}:
      0n
    case M.Ask{_, t, _, _, _, _, op, _} <> rest:
      Pend.one(Bool.and(op, String.eq(t, th)), Pend.open(rest, th))

def Pend.thread(a: M.Ask) -> String:
  match a:
    case M.Ask{_, th, _, _, _, _, _, _}:
      th

def Pend.go(cs: List<&2, M.Change>, +before: M.State, +after: M.State) -> List<&2, M.Change>:
  match cs:
    case Nil{}:
      Nil{}
    case M.AskOpened{_, +th, _, _, _, _, +at} <> t:
      M.PendingChanged{th, Pend.open(M.Asks.all(after), th), at} <> Pend.go(t, before, after)
    case M.AskClosed{+id, _, +at} <> t:
      M.PendingChanged{Pend.thread(Ask.find(M.Asks.all(before), id)), Pend.open(M.Asks.all(after), Pend.thread(Ask.find(M.Asks.all(before), id))), at} <> Pend.go(t, before, after)
    case _ <> t:
      Pend.go(t, before, after)

# a step's changes with the pending counts they move
def Pend.fix(+s: M.State, +cs: List<&2, M.Change>) -> List<&2, M.Change>:
  List.append(&2, M.Change, cs, Pend.go(cs, s, M.State.replay(cs, s)))

# whether the newest entry is the "restart" note Carry.k adds after its
# message (it is no progress)
def Carry.restart(es: List<&2, M.Entry>) -> Bool:
  match es:
    case M.Act{_, _, _, +kind, _, _} <> _:
      String.eq(kind, "restart")
    case _:
      False{}

# progress since the last carry: the newest entry, past restart notes, is
# anything but the continue message
def Carry.fresh.go(es: List<&2, M.Entry>, skip: Bool) -> Bool:
  match es skip:
    case Nil{} _:
      True{}
    case _ <> +t True{}:
      Carry.fresh.go(t, Carry.restart(t))
    case M.Msg{_, _, _, +text, _} <> _ False{}:
      Bool.not(String.eq(text, M.Carry.text()))
    case _ False{}:
      True{}

def Carry.fresh(+es: List<&2, M.Entry>) -> Bool:
  Carry.fresh.go(es, Carry.restart(es))

# an archived or deleted thread never carries on
def Carry.live(t: M.Thread) -> Bool:
  match t:
    case M.Thread{_, _, _, _, _, _, _, ar, _, _, _, _, _, _, _, _, _, _, _, _, de}:
      Bool.not(Bool.or(ar, de))

# whether a turn the restart cut short carries on: the setting is on, the
# thread was running, is live, and made progress since the last carry
def Carry.will(+before: M.State, +th: String) -> Bool:
  Bool.and(M.Carry.on(before), Bool.and(Bool.and(Hub.running(Hub.thread(before, th)), Carry.live(Hub.thread(before, th))), Carry.fresh(M.State.mine(before, th))))

# Recovery
# --------
# After a restart nothing the old process held is left: no agent runs, no
# request waits parked for an answer. Every turn still marked running ends
# as interrupted (a delegated child's task fails, and its parent hears of
# it) and every open ask closes unanswered (laws recover_*). Queued
# messages stay held until the user resumes them (queue.resume) or sends
# another, unless the turn carries on (Carry, below).

type Rec is Data:
  Rec{s: M.State, cs: List<&2, M.Change>, os: List<&2, Out>}

def Rec.add(r: Rec, st: Step) -> Rec:
  match r st:
    case Rec{s, cs, os} Step{+c2, o2}:
      Rec{M.State.replay(c2, s), List.append(&2, M.Change, cs, c2), List.append(&2, Out, os, o2)}

def Rec.note(+th: String, +now: Nat, +seq: Nat, +text: String) -> M.Change:
  M.ActivityLogged{th, Hub.id("r", now, seq), "error", "restart", text, now}

# (a turn that carries on (Carry.will) keeps a delegated child's task
# open: the child goes on and delivers it; law carry_keeps_child_task)
def Rec.turn.k(run: Bool, will: Bool, +s: M.State, +now: Nat, +th: String) -> Step:
  match run will:
    case True{} False{}:
      Launch.tell(s, now, th, Task.finish(s, now, th, Step{[Rec.note(th, now, Hub.seq(s), "Backplane restarted before this turn finished"), M.TurnChanged{th, M.Interrupted{}, now}], Nil{}}))
    case True{} True{}:
      Step{[Rec.note(th, now, Hub.seq(s), "Backplane restarted before this turn finished"), M.TurnChanged{th, M.Interrupted{}, now}], Nil{}}
    case False{} _:
      Step.none()

def Rec.turn(r: Rec, +before: M.State, +now: Nat, +th: String) -> Rec:
  match r:
    case Rec{+s, cs, os}:
      Rec.add(Rec{s, cs, os}, Rec.turn.k(Hub.running(Hub.thread(s, th)), Carry.will(before, th), s, now, th))

def Rec.turns(ts: List<&2, String>, r: Rec, +before: M.State, +now: Nat) -> Rec:
  match ts:
    case Nil{}:
      r
    case +th <> t:
      Rec.turns(t, Rec.turn(r, before, now, th), before, now)

def Rec.ask.k(a: M.Ask, +s: M.State, +now: Nat) -> Step:
  match a:
    case M.Ask{+id, +th, _, _, _, _, True{}, _}:
      Step{[M.AskClosed{id, "", now}, Rec.note(th, now, Hub.seq(s), "Backplane restarted: this request can no longer be answered")], Nil{}}
    case _:
      Step.none()

def Rec.ask(r: Rec, a: M.Ask, +now: Nat) -> Rec:
  match r:
    case Rec{+s, cs, os}:
      Rec.add(Rec{s, cs, os}, Rec.ask.k(a, s, now))

def Rec.asks(as: List<&2, M.Ask>, r: Rec, +now: Nat) -> Rec:
  match as:
    case Nil{}:
      r
    case a <> t:
      Rec.asks(t, Rec.ask(r, a, now), now)

def Rec.ids(ts: List<&2, M.Thread>) -> List<&2, String>:
  match ts:
    case Nil{}:
      Nil{}
    case t <> rest:
      M.Thread.id(t) <> Rec.ids(rest)

def Rec.threads(s: M.State) -> List<&2, M.Thread>:
  match s:
    case M.State{_, ts, _, _, _, _, _, _, _, _, _}:
      Map.values(&2, M.Thread, ts)

def Rec.step(r: Rec) -> Step:
  match r:
    case Rec{_, cs, os}:
      Step{cs, os}

# no agent survives a restart: none of their subagents are at work
def Bg.clear.all(ids: List<&2, String>, +s: M.State, +now: Nat) -> List<&2, M.Change>:
  match ids:
    case Nil{}:
      Nil{}
    case +id <> t:
      List.append(&2, M.Change, Bg.clear(s, id, now), Bg.clear.all(t, s, now))

# Repairing deliveries
# --------------------
# One write per step is still not atomic: a crash can tear a delivery
# after the parent's message, after its turn started, or before either.
# At start-up every finished task not yet delivered is finished: a parent
# that already holds its message (Task.msg) only gets TaskDelivered, and,
# idle with that message as its newest and nothing queued, its turn starts;
# otherwise the result is delivered now (laws task_repair_*). It does not
# make the agent's own work exactly-once across a crash.

def Task.holds(es: List<&2, M.Entry>, +msg: String) -> Bool:
  match es:
    case Nil{}:
      False{}
    case M.Msg{+id, _, _, _, _} <> t:
      Bool.or(String.eq(id, msg), Task.holds(t, msg))
    case _ <> t:
      Task.holds(t, msg)

# whether a thread's newest message is msg (entries newest first)
def Task.newest(es: List<&2, M.Entry>, +msg: String) -> Bool:
  match es:
    case Nil{}:
      False{}
    case M.Msg{+id, _, _, _, _} <> _:
      String.eq(id, msg)
    case _ <> t:
      Task.newest(t, msg)

def Task.word.of(st: M.TaskState) -> String:
  match st:
    case M.TaskDone{}:
      "finished"
    case _:
      "failed"

def Task.repair.has(has: Bool, wake: Bool, +s: M.State, +now: Nat, +id: String, +parent: String, +child: String, +text: String) -> Step:
  match has wake:
    case True{} True{}:
      Step{[M.TurnChanged{parent, M.Running{}, now}, M.TaskDelivered{id, now}],
        Git{0, "", "checkpoint0", parent, Hub.cwd(s, Hub.thread(s, parent)), "", ""} <> Git{0, "", "step", parent, Hub.cwd(s, Hub.thread(s, parent)), "", ""}
          <> Hub.run(Hub.thread(s, parent), Hub.cwd(s, Hub.thread(s, parent)), text, Hub.uuid(now, parent ++ "/r/" ++ id))}
    case True{} False{}:
      Step{[M.TaskDelivered{id, now}], Nil{}}
    case False{} _:
      Step.after(Hub.inbox.id(s, now, child, parent, M.System{}, text, Task.msg(child, id)), [M.TaskDelivered{id, now}])

def Task.repair.k(due: Bool, +s: M.State, +now: Nat, +id: String, +parent: String, +child: String, +text: String) -> Step:
  match due:
    case True{}:
      Task.repair.has(Task.holds(M.State.mine(s, parent), Task.msg(child, id)),
        Bool.and(Bool.not(Hub.running(Hub.thread(s, parent))), Bool.and(Task.newest(M.State.mine(s, parent), Task.msg(child, id)), Bool.not(M.Queue.waiting(s, parent)))),
        s, now, id, parent, child, text)
    case False{}:
      Step.none()

# where a child's turn left its task: 0 still its business (running, idle,
# or work out), 1 it completed, 2 it failed or was stopped
def Task.child.end(t: M.TurnState, ready: Bool) -> Nat:
  match t ready:
    case M.Completed{} True{}:
      1n
    case M.Failed{} _:
      2n
    case M.Interrupted{} _:
      2n
    case _ _:
      0n

def Task.repair.fin(e: Nat, +s: M.State, +now: Nat, +id: String, +parent: String, +child: String, +provider: String, +model: String) -> Step:
  match e:
    case 0n:
      Step.none()
    case 1n:
      Step.before([M.TaskStateSet{id, M.TaskDone{}, Task.last(s, child), now}], Nil{},
        Task.repair.k(True{}, s, now, id, parent, child, Task.note(id, provider, model, "finished", Task.last(s, child))))
    case _:
      Step.before([M.TaskStateSet{id, M.TaskFailed{}, Task.last(s, child), now}], Nil{},
        Task.repair.k(True{}, s, now, id, parent, child, Task.note(id, provider, model, "failed", Task.last(s, child))))

# (before: the state the hub started from: a child the restart cut short
# is the restart's business, Rec.turns and Carry)
def Task.repair.st(term: Bool, +before: M.State, +s: M.State, +now: Nat, +id: String, +parent: String, +child: String, +st: M.TaskState, +summary: String, +dl: Bool, +provider: String, +model: String) -> Step:
  match term:
    case True{}:
      Task.repair.k(Bool.not(dl), s, now, id, parent, child,
        Task.note(id, provider, model, Task.word.of(st), Bool.pick(String, String.is_empty(summary), Task.last(s, child), summary)))
    # torn before TaskStateSet: the child's turn ended, its task did not
    case False{}:
      Task.repair.fin(Task.child.end(M.Thread.turn(Hub.thread(before, child)), M.Work.ready(before, child)), s, now, id, parent, child, provider, model)

def Task.repair.one(t: M.Task, +before: M.State, +s: M.State, +now: Nat) -> Step:
  match t:
    case M.Task{+id, +parent, +child, _, +st, +summary, +dl, +provider, +model}:
      Task.repair.st(M.TaskState.terminal(st), before, s, now, id, parent, child, st, summary, dl, provider, model)

def Task.repair.r(r: Rec, t: M.Task, +before: M.State, +now: Nat) -> Rec:
  match r:
    case Rec{+s, cs, os}:
      Rec.add(Rec{s, cs, os}, Task.repair.one(t, before, s, now))

def Task.repair.go(ts: List<&2, M.Task>, r: Rec, +before: M.State, +now: Nat) -> Rec:
  match ts:
    case Nil{}:
      r
    case t <> rest:
      Task.repair.go(rest, Task.repair.r(r, t, before, now), before, now)

def Task.repair.tasks(s: M.State) -> List<&2, M.Task>:
  match s:
    case M.State{_, _, _, _, tk, _, _, _, _, _, _}:
      Map.values(&2, M.Task, tk)

# every finished task's delivery, finished (over the recovery so far)
def Rec.repair(r: Rec, +before: M.State, +now: Nat) -> Rec:
  match r:
    case Rec{+s, cs, os}:
      Task.repair.go(Task.repair.tasks(s), Rec{s, cs, os}, before, now)

# the same as a step of its own (for the laws)
def Task.repair(+s: M.State, +now: Nat) -> Step:
  Rec.step(Rec.repair(Rec{s, Nil{}, Nil{}}, s, now))

# a child's finished task k1 whose delivery a crash tore: stage 0 before
# the parent's message, 1 after it, 2 after the parent's turn started, 3
# delivered (the laws' example)
def Repair.demo(+stage: Nat) -> M.State:
  M.State.replay(List.concat(&2, M.Change, [[M.ProjectCreated{"p1", "p", "/p", 1n}, M.ThreadCreated{"t0", "p1", "x", M.Local{}, "", "", "codex", "", 1n},
    M.ThreadCreated{"t1", "p1", "y", M.Local{}, "", "", "codex", "", 1n}, M.TaskDelegated{"k1", "t0", "t1", 1n, "codex", "", 1n},
    M.TaskStateSet{"k1", M.TaskRunning{}, "", 1n}, M.MessagePosted{"t1", "a1", M.Assistant{}, "done it", 2n}, M.TaskStateSet{"k1", M.TaskDone{}, "done it", 3n}],
    Bool.pick(List<&2, M.Change>, Nat.is_ge(stage, 1n), [M.MessagePosted{"t0", Task.msg("t1", "k1"), M.System{}, "note", 3n}], Nil{}),
    Bool.pick(List<&2, M.Change>, Nat.is_ge(stage, 2n), [M.TurnChanged{"t0", M.Running{}, 3n}], Nil{}),
    Bool.pick(List<&2, M.Change>, Nat.is_ge(stage, 3n), [M.TaskDelivered{"k1", 3n}], Nil{})]), M.State.empty())

def Repair.again(+s: M.State, +now: Nat) -> M.State:
  M.State.replay(Queue.changes(Task.repair(s, now)), s)

def Repair.msgs(cs: List<&2, M.Change>) -> Nat:
  match cs:
    case Nil{}:
      0n
    case M.MessagePosted{_, _, _, _, _} <> t:
      1n+Repair.msgs(t)
    case _ <> t:
      Repair.msgs(t)

# Resuming, bounded
# -----------------
# A restart may leave many threads to tell or carry on. At most
# Rec.burst() start at once (Hub.carry); each of the rest keeps a private
# setting resume.<thread> ("restart": its lost work to tell, "carry": its
# turn to carry on), listed in resume.ids, and Rec.release starts up to
# Rec.burst() of them each minute and after each turn's end. A marker that
# survives another restart (resume.<thread>.n reaches 2) is only logged
# (laws rec_*).

def Rec.burst() -> Nat:
  4n

def Resume.key(+th: String) -> String:
  "resume." ++ th

def Resume.n.key(+th: String) -> String:
  "resume." ++ th ++ ".n"

def Resume.why(+s: M.State, +th: String) -> String:
  M.Setting.get(s, Resume.key(th), "")

def Resume.n(+s: M.State, +th: String) -> Nat:
  M.Switch.num(M.Setting.get(s, Resume.n.key(th), "0"), 0n)

def Resume.ids(+s: M.State) -> List<&2, String>:
  M.Bg.lines(M.Setting.get(s, "resume.ids", ""))

def Resume.del.go(ids: List<&2, String>, +th: String) -> List<&2, String>:
  match ids:
    case Nil{}:
      Nil{}
    case +x <> t:
      M.Bg.keep(String.eq(x, th), x, Resume.del.go(t, th))

# a thread's marker set (and listed once)
def Resume.set(+s: M.State, +now: Nat, +th: String, +why: String) -> List<&2, M.Change>:
  [M.SettingSet{Resume.key(th), why, now}, M.SettingSet{"resume.ids", M.Bg.show(List.append(&2, String, Resume.del.go(Resume.ids(s), th), [th])), now}]

# a thread's marker cleared
def Resume.clear(+s: M.State, +now: Nat, +th: String) -> List<&2, M.Change>:
  [M.SettingSet{Resume.key(th), "", now}, M.SettingSet{"resume.ids", M.Bg.show(Resume.del.go(Resume.ids(s), th)), now}]

def Rec.lost.again(give: Bool, +s: M.State, +now: Nat, +th: String) -> Step:
  match give:
    case True{}:
      Step{List.append(&2, M.Change, [M.ActivityLogged{th, Hub.id("l", now, Hub.seq(s)), "info", "lost", Lost.text(M.Bg.of(s, th), "restart"), now},
        M.WorkSet{th, "bg", M.Bg.show(Lost.unknown(M.Bg.of(s, th))), now}, M.SettingSet{Resume.n.key(th), "0", now}], Resume.clear(s, now, th)), Nil{}}
    case False{}:
      Step{[M.SettingSet{Resume.n.key(th), Nat.show(1n+Resume.n(s, th)), now}], Nil{}}

# a restart found a thread's work lost: with the setting off it is logged
# (Bg.lost); else it is marked to be told (a marker already there counts
# a restart it outlived, and on the second it is only logged)
def Rec.lost.on(again: Bool, +s: M.State, +now: Nat, +th: String) -> Step:
  match again:
    case False{}:
      Step{Resume.set(s, now, th, "restart"), Nil{}}
    case True{}:
      Rec.lost.again(Nat.is_ge(1n+Resume.n(s, th), 2n), s, now, th)

def Rec.lost.k(has: Bool, live: Bool, on: Bool, +s: M.State, +now: Nat, +th: String) -> Step:
  match has live on:
    case False{} _ _:
      Step.none()
    case True{} False{} _:
      Step{Bg.clear(s, th, now), Nil{}}
    case True{} True{} False{}:
      Bg.lost(s, now, th, "restart")
    case True{} True{} True{}:
      Rec.lost.on(Bool.not(String.is_empty(Resume.why(s, th))), s, now, th)

def Rec.lost.one(r: Rec, +now: Nat, +th: String) -> Rec:
  match r:
    case Rec{+s, cs, os}:
      Rec.add(Rec{s, cs, os}, Rec.lost.k(Lost.new(M.Bg.of(s, th)), Lost.live(Hub.thread(s, th)), M.Carry.on(s), s, now, th))

def Rec.lost(ts: List<&2, String>, r: Rec, +now: Nat) -> Rec:
  match ts:
    case Nil{}:
      r
    case +th <> t:
      Rec.lost(t, Rec.lost.one(r, now, th), now)

# what a hub that just started from its log does first: turns marked
# running end (a carried child's task stays open), asks close, torn task
# deliveries are finished, and lost background work is logged or marked
# to be told (Hub.carry and Rec.release tell it)
def Hub.recover.k(+s: M.State, +now: Nat) -> Step:
  Rec.step(Rec.lost(Rec.ids(Rec.threads(s)), Rec.repair(Rec.turns(Rec.ids(Rec.threads(s)), Rec.asks(M.Asks.all(s), Rec{s, Nil{}, Nil{}}, now), s, now), s, now), now))

# (and every watched work still open is watched again: Watch.rearm)
def Hub.recover(+s: M.State, +now: Nat) -> Step:
  Step.cat(Step{Nil{}, Watch.rearm(s)}, Hub.recover.k(s, now))

# Carrying on
# -----------
# With the "restart.continue" setting on (M.Carry.on), every turn the
# restart interrupted goes on: the thread is told to continue (with any
# message held in its queue), and its agent resumes its session. A thread
# whose newest entry is already that message made no progress since the
# last restart and is left stopped, so a turn that brings the hub down
# never loops (laws carry_*). Runs after Hub.recover, over the state from
# before it.

def Carry.k(go: Bool, +s: M.State, +now: Nat, +th: String) -> Step:
  match go:
    case True{}:
      Step.after(Hub.post(s, now, th, Hub.id("c", now, Hub.seq(s)), M.User{}, M.Carry.text()),
        [M.ActivityLogged{th, Hub.id("r", now, 1n+Hub.seq(s)), "info", "restart", "Continuing after the restart", now}])
    case False{}:
      Step.none()

def Rec.carry(will: Bool, r: Rec, +now: Nat, +th: String) -> Rec:
  match r:
    case Rec{+s, cs, os}:
      Rec.add(Rec{s, cs, os}, Carry.k(will, s, now, th))

# a marked thread's lost work told (after its carry, if any: behind it)
def Rec.tell(marked: Bool, r: Rec, +now: Nat, +th: String) -> Rec:
  match marked r:
    case True{} Rec{+s, cs, os}:
      Rec.add(Rec{s, cs, os}, Step.cat(Bg.lost(s, now, th, "restart"), Step{Resume.clear(s, now, th), Nil{}}))
    case False{} _:
      r

def Rec.defer(will: Bool, r: Rec, +now: Nat, +th: String) -> Rec:
  match will r:
    case True{} Rec{+s, cs, os}:
      Rec.add(Rec{s, cs, os}, Step{Resume.set(s, now, th, "carry"), Nil{}})
    case False{} _:
      r

# the restart's starts, Rec.burst() at most: the rest are marked
type Cr is Data:
  Cr{r: Rec, n: Nat}

def Carry.one.b(go: Bool, +will: Bool, +marked: Bool, r: Rec, +n: Nat, +now: Nat, +th: String) -> Cr:
  match go:
    case True{}:
      Cr{Rec.tell(marked, Rec.carry(will, r, now, th), now, th), n}
    case False{}:
      Cr{r, 1n+n}

def Carry.one.k(+will: Bool, +marked: Bool, c: Cr, +now: Nat, +th: String) -> Cr:
  match c:
    case Cr{r, 1n+n}:
      Carry.one.b(Bool.or(will, marked), will, marked, r, n, now, th)
    case Cr{r, 0n}:
      Cr{Rec.defer(will, r, now, th), 0n}

def Carry.one(c: Cr, +before: M.State, +now: Nat, +th: String) -> Cr:
  match c:
    case Cr{Rec{+s, cs, os}, n}:
      Carry.one.k(Carry.will(before, th), Bool.not(String.is_empty(Resume.why(s, th))), Cr{Rec{s, cs, os}, n}, now, th)

def Carry.all(ts: List<&2, String>, c: Cr, +before: M.State, +now: Nat) -> Cr:
  match ts:
    case Nil{}:
      c
    case +th <> t:
      Carry.all(t, Carry.one(c, before, now, th), before, now)

def Cr.step(c: Cr) -> Step:
  match c:
    case Cr{r, _}:
      Rec.step(r)

def Carry.go(on: Bool, +before: M.State, +after: M.State, +now: Nat) -> Step:
  match on:
    case True{}:
      Cr.step(Carry.all(Rec.ids(Rec.threads(before)), Cr{Rec{after, Nil{}, Nil{}}, Rec.burst()}, before, now))
    case False{}:
      Step.none()

# before: the state Hub.recover started from; after: once its step applied
# (turns carried on and lost work told, Rec.burst() threads at most)
def Hub.carry(+before: M.State, +after: M.State, +now: Nat) -> Step:
  Carry.go(M.Carry.on(after), before, after, now)

def Rel.k(run: Bool, live: Bool, r: Rec, +now: Nat, +th: String) -> Rec:
  match run live r:
    case True{} _ _:
      r
    case False{} False{} Rec{+s, cs, os}:
      Rec.add(Rec{s, cs, os}, Step{Resume.clear(s, now, th), Nil{}})
    case False{} True{} Rec{+s, cs, os}:
      Rec.tell(True{}, Rec.carry(Bool.and(String.eq(Resume.why(s, th), "carry"), M.Carry.on(s)), Rec{s, cs, os}, now, th), now, th)

def Rel.one(r: Rec, +now: Nat, +th: String) -> Rec:
  match r:
    case Rec{+s, cs, os}:
      Rel.k(Hub.running(Hub.thread(s, th)), Lost.live(Hub.thread(s, th)), Rec{s, cs, os}, now, th)

def Rel.all(ts: List<&2, String>, r: Rec, +now: Nat) -> Rec:
  match ts:
    case Nil{}:
      r
    case +th <> t:
      Rel.all(t, Rel.one(r, now, th), now)

# (a marker saved while continuing was on is read again at its release:
# with the setting off it starts nothing and is cleared)
# the next marked threads (Rec.burst() at most) carried on or told their
# lost work; a running one waits for its turn's end (the server runs this
# each minute and after each turn ends). A lost-work note never takes a
# held queue (Lost.note)
def Rec.release(+s: M.State, +now: Nat) -> Step:
  Rec.step(Rel.all(List.take(&2, String, Resume.ids(s), Rec.burst()), Rec{s, Nil{}, Nil{}}, now))

# Scenarios the lost-work laws are stated over: thread t1 (claude) with a
# background shell at work, idle or running, restart.continue on or off;
# six such threads; a thread marked to be told with a queue held
def Lost.demo.cs(+on: Bool, +run: Bool, +th: String) -> List<&2, M.Change>:
  List.concat(&2, M.Change, [[M.ThreadCreated{th, "p1", th, M.Local{}, "", "", "claude", "", 1n}, M.WorkSet{th, "bg", "b1\tsleep 60\tc1\tsleep 60\tshell", 2n}],
    Bool.pick(List<&2, M.Change>, run, [M.MessagePosted{th, "u1", M.User{}, "go", 2n}, M.TurnChanged{th, M.Running{}, 2n}], [M.TurnChanged{th, M.Completed{}, 3n}]),
    Bool.pick(List<&2, M.Change>, on, Nil{}, [M.SettingSet{"restart.continue", "off", 1n}])])

def Lost.demo(+on: Bool, +run: Bool) -> M.State:
  M.State.replay(M.ProjectCreated{"p1", "p", "/p", 1n} <> Lost.demo.cs(on, run, "t1"), M.State.empty())

def Lost.many() -> M.State:
  M.State.replay(List.concat(&2, M.Change, [[M.ProjectCreated{"p1", "p", "/p", 1n}], Lost.demo.cs(True{}, False{}, "a1"), Lost.demo.cs(True{}, False{}, "a2"),
    Lost.demo.cs(True{}, False{}, "a3"), Lost.demo.cs(True{}, False{}, "a4"), Lost.demo.cs(True{}, False{}, "a5"), Lost.demo.cs(True{}, False{}, "a6")]), M.State.empty())

# a1 marked "carry" (deferred past a restart's burst) and not running,
# continuing on or off, then released
def Lost.released.s(+on: Bool) -> M.State:
  M.State.replay(List.append(&2, M.Change, Bool.pick(List<&2, M.Change>, on, Nil{}, [M.SettingSet{"restart.continue", "off", 9n}]), Resume.set(Lost.many(), 9n, "a1", "carry")), Lost.many())

def Lost.released(+on: Bool) -> M.State:
  M.State.replay(Queue.changes(Rec.release(Lost.released.s(on), 10n)), Lost.released.s(on))

def Lost.marked() -> M.State:
  M.State.replay(List.append(&2, M.Change, Resume.set(Lost.demo(True{}, False{}), 4n, "t1", "restart"), [M.TurnQueued{"t1", "q1", "held", 4n}]), Lost.demo(True{}, False{}))

def Lost.running.go(ids: List<&2, String>, +s: M.State) -> Nat:
  match ids:
    case Nil{}:
      0n
    case +i <> t:
      Nat.add(Bool.pick(Nat, Hub.running(Hub.thread(s, i)), 1n, 0n), Lost.running.go(t, s))

# how many threads run
def Lost.running(+s: M.State) -> Nat:
  Lost.running.go(Rec.ids(Rec.threads(s)), s)

# how many of a thread's messages tell lost work
def Lost.told(es: List<&2, M.Entry>) -> Nat:
  match es:
    case Nil{}:
      0n
    case M.Msg{_, _, _, +text, _} <> t:
      Nat.add(Bool.pick(Nat, String.contains(text, "ended your background work"), 1n, 0n), Lost.told(t))
    case _ <> t:
      Lost.told(t)

def Lost.taken(cs: List<&2, M.Change>) -> Bool:
  match cs:
    case Nil{}:
      False{}
    case M.QueueTaken{_, _} <> _:
      True{}
    case _ <> t:
      Lost.taken(t)

def Lost.forced(os: List<&2, Out>) -> Nat:
  match os:
    case Nil{}:
      0n
    case UpgradeForce{} <> t:
      1n+Lost.forced(t)
    case _ <> t:
      Lost.forced(t)

def Lost.restarts(os: List<&2, Out>) -> Nat:
  match os:
    case Nil{}:
      0n
    case Restart{} <> t:
      1n+Lost.restarts(t)
    case _ <> t:
      Lost.restarts(t)

def Lost.upgrades(os: List<&2, Out>) -> Nat:
  match os:
    case Nil{}:
      0n
    case Upgrade{} <> t:
      1n+Lost.upgrades(t)
    case UpgradeForce{} <> t:
      1n+Lost.upgrades(t)
    case _ <> t:
      Lost.upgrades(t)

# Turn processes
# --------------
# A Codex or Grok turn is a process of its own. Its lines reach the hub
# tagged "<thread>\t<pid>", and only the process the hub last started for a
# thread is heard (a stopped one may still be talking). Thread ids hold no
# tab; an untagged line (Claude's) is always heard.

def Tag.of(+thread: String, +pid: U32) -> String:
  thread ++ "\t" ++ U32.show(pid)

def Tag.thread.k(ps: List<&2, String>) -> String:
  match ps:
    case th <> _:
      th
    case Nil{}:
      ""

def Tag.thread(+tag: String) -> String:
  Tag.thread.k(String.split(tag, '\t'))

def Tag.digit(+c: U32) -> Bool:
  Bool.and(U32.is_ge(c, 48), U32.is_le(c, 57))

# a pid's digits as a number (None for anything else)
def Tag.num(s: String, acc: Maybe<&2, U32>) -> Maybe<&2, U32>:
  match s acc:
    case SNil{} _:
      acc
    case SCon{Chr{+c}, t} Some{+n}:
      Tag.num(t, Bool.pick(Maybe<&2, U32>, Tag.digit(c), Some{((n * 10 : U32) + (c - 48 : U32) : U32)}, None{}))
    case SCon{Chr{+c}, t} None{}:
      None{}

def Tag.num.k(empty: Bool, p: String) -> Maybe<&2, U32>:
  match empty:
    case True{}:
      None{}
    case False{}:
      Tag.num(p, Some{0})

def Tag.pid.k(ps: List<&2, String>) -> Maybe<&2, U32>:
  match ps:
    case _ <> +p <> Nil{}:
      Tag.num.k(String.is_empty(p), p)
    case _:
      None{}

# the process a tag names (None: untagged)
def Tag.pid(+tag: String) -> Maybe<&2, U32>:
  Tag.pid.k(String.split(tag, '\t'))

# Spawn generations
# -----------------
# Every agent process the hub starts is a generation (a Claude process; a
# Codex or Grok turn's process), named when the server starts it. Until its
# pid is known the thread's agent is a placeholder keyed Gen.key(gen); the
# pid is adopted only by that generation's report, any other is a process
# no one wants and is stopped; a failure to start is heard only from the
# current generation. A message whose images were read for one generation
# is written only to it, and a tool call over MCP from a process that is
# no longer the thread's may read but not change anything (its URL carries
# "<thread>.<gen>"). A pid of 0 or 1 is never signalled: kill(0) would
# stop the hub's own process group (laws spawn_gen_*, kill_never_low_pid,
# mcp_stale_gen_refused, prep_stale_dropped).

def Gen.key(+gen: String) -> String:
  "spawn:" ++ gen

# the key of an adopted turn process
def Gen.turn(+gen: String) -> String:
  "turn:" ++ gen

# whether a generation's report is the one the thread's agent waits for
def Gen.accepts(+key: String, +gen: String) -> Bool:
  String.eq(key, Gen.key(gen))

def Gen.after(ps: List<&2, String>) -> String:
  match ps:
    case _ <> +g <> _:
      g
    case _:
      ""

# the generation a key names ("" for none)
def Gen.of(+key: String) -> String:
  Bool.pick(String, Bool.or(String.starts_with(key, "spawn:"), String.starts_with(key, "turn:")), Gen.after(String.split(key, ':')), "")

def Gen.first(ps: List<&2, String>) -> String:
  match ps:
    case +t <> _:
      t
    case Nil{}:
      ""

# an MCP path's thread and generation ("<thread>.<gen>"; no generation: "")
def Gen.thread(+tag: String) -> String:
  Gen.first(String.split(tag, '.'))

def Gen.tag(+tag: String) -> String:
  Gen.after(String.split(tag, '.'))

def Gen.mcp(+thread: String, +gen: String) -> String:
  Bool.pick(String, String.is_empty(gen), thread, thread ++ "." ++ gen)

# a call named with a generation that is not the thread's current one
def Gen.stale(+tagged: String, +current: String) -> Bool:
  Bool.and(Bool.not(String.is_empty(tagged)), Bool.not(String.eq(tagged, current)))

# the tools that only read
def Gen.reads(+tool: String) -> Bool:
  Nat.is_lt(C.Codec.pos(["orchestrator_capabilities", "task_status", "thread_list", "thread_read", "thread_wait", "thread_wait_now", "work_list", "project_list", "machine_list", "plane_read"], tool), 10n)

# an MCP request that changes something: a tools/call of any tool that does
# not only read (initialize, tools/list and the like change nothing)
def Gen.changes(+o: J.Json) -> Bool:
  Bool.and(String.eq(J.Json.get_str(o, "method", ""), "tools/call"), Bool.not(Gen.reads(J.Json.get_str(J.Json.get_obj(o, "params"), "name", ""))))

# whether a request from this MCP path is refused: a stale generation's
# change
def Gen.refused(+tagged: String, +current: String, +o: J.Json) -> Bool:
  Bool.and(Gen.stale(tagged, current), Gen.changes(o))

# whether a prepared message may be written: its generation is the agent's
# and its command is not a tomb (core/cmds.bend's st 4)
def Gen.write(+gen: String, +current: String, +st: Nat) -> Bool:
  Bool.and(String.eq(gen, current), Nat.is_lt(st, 4n))

# whether a pid may be signalled
def Proc.kill.ok(+pid: U32) -> Bool:
  U32.is_gt(pid, 1)

# Silence and exit
# ----------------
# A running turn whose agent wrote nothing for Quiet.after() seconds, with
# no work out (which it may be waiting on), is noted once in the timeline;
# nothing is killed (laws quiet_*). An agent process's exit is noticed by
# its own watcher even when a child keeps its output open; a report from a
# process that is no longer the thread's changes nothing (law
# exit_stale_gen_quiet).

def Quiet.after() -> Nat:
  600n

def Quiet.due.k(run: Bool, said: Bool, +heard: Nat, +now: Nat, +busy: Nat) -> Bool:
  match run said:
    case True{} False{}:
      Bool.and(Nat.is_eq(busy, 0n), Nat.is_ge(now, Nat.add(heard, Quiet.after())))
    case _ _:
      False{}

def Quiet.due(run: Bool, +heard: Nat, +now: Nat, said: Bool, +busy: Nat) -> Bool:
  Quiet.due.k(run, said, heard, now, busy)

def Quiet.act(+thread: String, +now: Nat, +seq: Nat) -> M.Change:
  M.ActivityLogged{thread, Hub.id("q", now, seq), "info", "quiet", "No output for 10 minutes", now}

# whether an exit report (its generation) is the thread's current agent's
def Exit.current(+gen: String, +current: String) -> Bool:
  Bool.and(Bool.not(String.is_empty(gen)), String.eq(gen, current))

# whether a line from this tag is heard, given the pid the hub runs for
# its thread (None: none)
def Tag.heard(m: Maybe<&2, U32>, run: Maybe<&2, U32>) -> Bool:
  match m run:
    case None{} _:
      True{}
    case Some{+p} Some{+q}:
      U32.is_eq(p, q)
    case Some{_} None{}:
      False{}

def Hub.pending(t: M.Thread) -> Nat:
  match t:
    case M.Thread{_, _, _, _, _, _, pe, _, _, _, _, _, _, _, _, _, _, _, _, _, _}:
      pe

# Scenarios the recovery laws are stated over: a Codex thread whose turn
# runs (or finished), with an approval open
def Rec.demo.base() -> List<&2, M.Change>:
  [M.ProjectCreated{"p1", "p", "/p", 1n}, M.ThreadCreated{"t1", "p1", "x", M.Local{}, "", "", "codex", "", 1n}]

def Rec.demo(+st: M.TurnState) -> M.State:
  M.State.replay(List.append(&2, M.Change, Rec.demo.base(), [M.TurnChanged{"t1", st, 2n}, M.AskOpened{"a1", "t1", "approval", "Bash", "ls", "", 3n}]), M.State.empty())

def Rec.after(+s: M.State, +now: Nat) -> M.State:
  M.State.replay(Queue.changes(Hub.recover(s, now)), s)

def Carry.after.k(+before: M.State, +after: M.State, +now: Nat) -> M.State:
  M.State.replay(Queue.changes(Hub.carry(before, after, now)), after)

# the recovery scenario carried on: the state after both steps
def Carry.after(+s: M.State, +now: Nat) -> M.State:
  Carry.after.k(s, Rec.after(s, now), now)

# the running scenario with more changes on top (for the carry laws)
def Carry.demo(cs: List<&2, M.Change>) -> M.State:
  M.State.replay(cs, Rec.demo(M.Running{}))

# a thread "t1" on `provider` whose turn is in state st (for laws)
def Demo.state(+provider: String, +st: M.TurnState) -> M.State:
  M.State.replay([M.ProjectCreated{"p1", "p", "/p", 1n}, M.ThreadCreated{"t1", "p1", "x", M.Local{}, "", "", provider, "", 1n}, M.TurnChanged{"t1", st, 2n}], M.State.empty())

# a Claude thread "t1" with session S1 and two messages, then switched
# to Codex, whose session became S2 (for laws)
def Demo.switched() -> M.State:
  M.State.replay([M.SessionSet{"t1", "S1", 3n}, M.MessagePosted{"t1", "m1", M.User{}, "hi", 3n}, M.MessagePosted{"t1", "m2", M.Assistant{}, "hello", 3n},
    M.ProviderSet{"t1", "codex", "", 4n}, M.SessionSet{"t1", "S2", 5n}], Demo.state("claude", M.Completed{}))

# a step applied as the server does (with the pending counts it moves)
def Retire.after(+s: M.State, st: Step) -> M.State:
  M.State.replay(Pend.fix(s, Queue.changes(st)), s)

# whether a step tells the server thread th's turn was stopped (for laws)
def Ended.stopped(os: List<&2, Out>, +th: String) -> Bool:
  match os:
    case Nil{}:
      False{}
    case TurnEnded{+t, +state} <> r:
      Bool.or(Bool.and(String.eq(t, th), String.eq(state, "stopped")), Ended.stopped(r, th))
    case _ <> r:
      Ended.stopped(r, th)

# thread.archive (or thread.delete: i 12n) of "t1" (for laws)
def Retire.request(i: Nat, +s: M.State) -> Step:
  Hub.request.k(i, s, 9n, 0, "r", J.Json.obj([J.KV{"thread", J.Str{"t1"}}]), "")

def Retire.task.k(m: Maybe<&2, M.Task>) -> Bool:
  match m:
    case Some{M.Task{_, _, _, _, st, _, _, _, _}}:
      M.TaskState.terminal(st)
    case None{}:
      False{}

# whether the task "t1" works on has ended (for laws)
def Retire.task(+s: M.State) -> Bool:
  Retire.task.k(Task.of_child(s, "t1"))

# the running scenario with an open plan card and a task t0 gave t1
def Retire.demo() -> M.State:
  M.State.replay([M.ThreadCreated{"t0", "p1", "x", M.Local{}, "", "", "codex", "", 1n}, M.TaskDelegated{"k1", "t0", "t1", 1n, "codex", "", 2n},
    M.AskOpened{"a2", "t1", "plan", "", "plan", "", 3n}], Rec.demo(M.Running{}))

# Launched threads: the laws' examples
# ------------------------------------

# messages a step sends to a thread (posted, or queued behind its turn)
def Lr.count(cs: List<&2, M.Change>, +th: String) -> Nat:
  match cs:
    case Nil{}:
      0n
    case M.MessagePosted{+t, _, _, _, _} <> rest:
      Nat.add(Bool.pick(Nat, String.eq(t, th), 1n, 0n), Lr.count(rest, th))
    case M.TurnQueued{+t, _, _, _} <> rest:
      Nat.add(Bool.pick(Nat, String.eq(t, th), 1n, 0n), Lr.count(rest, th))
    case _ <> rest:
      Lr.count(rest, th)

def Lr.queued(cs: List<&2, M.Change>, +th: String) -> Nat:
  match cs:
    case Nil{}:
      0n
    case M.TurnQueued{+t, _, _, _} <> rest:
      Nat.add(Bool.pick(Nat, String.eq(t, th), 1n, 0n), Lr.queued(rest, th))
    case _ <> rest:
      Lr.queued(rest, th)

# t0 launched t1 (report: whether it reports back) with t0's own message
# (own) or the person's; t1 runs; t0 runs too (busy); more: changes after
def Lr.demo.k(+report: Bool, +own: Bool, +busy: Bool, more: List<&2, M.Change>) -> M.State:
  M.State.replay(List.concat(&2, M.Change, [[M.ProjectCreated{"p1", "p", "/p", 1n}, M.ThreadCreated{"t0", "p1", "boss", M.Local{}, "", "", "claude", "", 1n},
    M.ThreadCreated{"t1", "p1", "work", M.Local{}, "", "", "claude", "", 1n}, M.ThreadLaunched{"t1", "t0", report, 1n},
    M.MessagePosted{"t1", Bool.pick(String, own, M.Link.id("t0", "m1"), "m1"), M.User{}, "go", 1n}, M.TurnChanged{"t1", M.Running{}, 1n},
    M.MessagePosted{"t1", "a1", M.Assistant{}, "done it", 2n}], Bool.pick(List<&2, M.Change>, busy, [M.TurnChanged{"t0", M.Running{}, 2n}], Nil{}), more]), M.State.empty())

def Lr.demo(+report: Bool) -> M.State:
  Lr.demo.k(report, False{}, False{}, Nil{})

# what t0 hears when t1's step has these changes
def Lr.demo.tell(+s: M.State, cs: List<&2, M.Change>) -> Nat:
  Lr.count(Queue.changes(Launch.tell(s, 9n, "t1", Step{cs, Nil{}})), "t0")

def Lr.demo.end() -> List<&2, M.Change>:
  [M.TurnChanged{"t1", M.Completed{}, 9n}]

# t1's turn ends twice, t0 busy: both reports wait in t0's queue, in order
def Lr.demo.twice.k(+s1: M.State) -> Nat:
  Lr.queued(Queue.changes(Launch.tell(s1, 11n, "t1", Step{[M.TurnChanged{"t1", M.Completed{}, 11n}], Nil{}})), "t0")

def Lr.demo.twice() -> Nat:
  +s = Lr.demo.k(True{}, False{}, True{}, Nil{})
  +st = Launch.tell(s, 9n, "t1", Step{Lr.demo.end(), Nil{}})
  +s1 = M.State.replay(List.append(&2, M.Change, Queue.changes(st), [M.MessagePosted{"t1", "m2", M.User{}, "again", 10n}, M.TurnChanged{"t1", M.Running{}, 10n}]), s)
  Nat.add(Lr.queued(Queue.changes(st), "t0"), Lr.demo.twice.k(s1))

# the loop's bound: t0 started t1's turn Lr.loops() times in a row
def Lr.demo.loop() -> M.State:
  Lr.demo.k(True{}, True{}, False{}, [M.SettingSet{Lr.n.key("t1"), Nat.show(Nat.sub(Lr.loops(), 1n)), 1n}])

def Lr.demo.loop.on() -> Bool:
  M.Launch.reports(M.State.replay(Queue.changes(Launch.tell(Lr.demo.loop(), 9n, "t1", Step{Lr.demo.end(), Nil{}})), Lr.demo.loop()), "t1")
