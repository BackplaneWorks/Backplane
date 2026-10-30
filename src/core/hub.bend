import Base
import ./mailprivate.bend as PM
import ./accounts.bend as ACCT
import ./subthreads.bend as ST
import ./mailtriage.bend as TRI
import ./runlife.bend as RL
import ./uber.bend as UB
import ./mailrun.bend as MR
import ./mailcontinuity.bend as MCN
import ./maildisposition.bend as MD
import ./mail.bend as ML
import ./mailbody.bend as MB
import ./ref.bend as REF
import ./ride.bend as RD
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
import ./bot.bend as BT
import ./tz.bend as TZ
import ./sched.bend as SC
import ./notice.bend as NO
import ./wake.bend as WK
import ./told.bend as TLD
import ./deploy.bend as DP
import ./ship.bend as SH
import ./version.bend as VER
import ./locks.bend as LK
import ./adopt.bend as AD
import ./cmds.bend as CM
import ./order.bend as OR
import ./fallback.bend as FB
import ./secret.bend as SEC
import ./stamp.bend as STP
import ./wnote.bend as WN

# The decider. Requests from clients and lines from agents come in; changes
# (persisted, folded into the read model) and effects (run by the server)
# come out. Nothing here does IO, so the server's behaviour is these defs.

# what the server must do after a step
type Out is Data:
  RunStart{thread: String, conv: String, head: String}
  RunNext{bot: String, conv: String}
  CharterWrite{path: String, text: String}
  MailAppend{line: String, conv: String, changes: List<&2, M.Change>}
  MailRead{client: U32, id: String, conv: String, before: String, n: Nat}
  LockGuard{name: String, old: String, holder: String}
  # to one client
  Send{client: U32, text: String}
  # to every client
  Broadcast{text: String}
  # ensure an agent process runs for a thread, then write a line to it
  AgentStart{thread: String, cwd: String, provider: String, model: String, session: String, runtime: M.RuntimeMode, interaction: M.InteractionMode}
  AgentSend{thread: String, line: String}
  AgentStop{thread: String}
  # show a client a project's board or schematic (kind "board",
  # "schematic"; anything else stops), or the file at path when set; tree:
  # the root sheet the schematic's hierarchy starts from ("" the project's
  # own; a project with several boards names the board's); sheet: the
  # combined view's schematic (kind "split:" and its panes)
  Watch{client: U32, kind: String, root: String, path: String, tree: String, sheet: String}
  # install the latest release and restart
  Upgrade{}
  # the same, asked on purpose (update.now) or after the wait ran out: its
  # restart does not wait for work
  UpgradeForce{}
  # the install is ready: end the hub's process into it now
  Restart{}
  KiCadInstall{}
  KiCadRetry{}
  # opencode's models read again for every client (server.bend's OcJob)
  ModelsRefresh{}
  # the person allowed a project's own checks (.backplane/checks.json) as
  # they are: their sha256 joins <home>/checks.trusted (server.bend)
  ChecksTrust{hash: String}
  # run one Codex turn (a process per turn; `session` resumes)
  CodexTurn{thread: String, cwd: String, model: String, session: String, runtime: M.RuntimeMode, text: String}
  GrokTurn{thread: String, cwd: String, model: String, session: String, runtime: M.RuntimeMode, text: String}
  MuseTurn{thread: String, cwd: String, model: String, session: String, runtime: M.RuntimeMode, text: String}
  # Google's Antigravity agent over ACP (core/agy.bend)
  AgyTurn{thread: String, cwd: String, model: String, session: String, runtime: M.RuntimeMode, text: String}
  # opencode over ACP (`opencode acp`, Grok's runner)
  OcTurn{thread: String, cwd: String, model: String, session: String, runtime: M.RuntimeMode, text: String}
  # Pi over its RPC (`pi --mode rpc`, core/pi.bend) and Oh My Pi over its
  # own RPC (`omp --mode rpc-ui`, core/omp.bend), both on Grok's runner
  PiTurn{thread: String, cwd: String, model: String, session: String, runtime: M.RuntimeMode, text: String}
  OmpTurn{thread: String, cwd: String, model: String, session: String, runtime: M.RuntimeMode, text: String}
  # a turn ended: "completed", "failed", or "stopped" (Stop, archive,
  # delete: no alert, but the phones' island still hears of it). alert:
  # the kind of the record this end wrote (notice.bend's Policy.end), ""
  # when it wrote none or a quiet one; never an older record's. error and
  # reason: the ending turn's own logged error and why it ended
  # (wnote.bend's Wn.cap, Wn.end.reason), "" when none, read as it ends;
  # text: that turn's own newest reply, "" when it gave none
  TurnEnded{thread: String, state: String, alert: String, error: String, reason: String, text: String}
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
  # an alert record of the thread was written (Alert.outs): the server
  # reads it and pushes the alert, shows it on the desk used last and tells
  # the owner's other machines (not persisted)
  Alerted{thread: String}
  WakePoll{prs: List<&2, String>, files: List<&2, String>}
  # a deploy waits and no tool call runs: look at the build locks (the server
  # answers with Msg DeployLocks, Hub.deploy_locks)
  DeployProbe{}
  # a tool call of a thread started / ended, or every call of a thread ended
  # (its turn, its agent): the server keeps the open ones in memory for the
  # deploy check (core/deploy.bend's Open.*); never written to the log
  ToolOpen{thread: String, call: String}
  ToolShut{thread: String, call: String}
  ToolsGone{thread: String}
  # the deploy goes: swap the staged package in (when stage is set), then
  # restart into it
  DeployGo{stage: String}
  # deploy_main (core/ship.bend): build commit in a job of its own, out of
  # the hub's process group and scope (launched unless it already runs, so
  # after a restart this watches the one that does); read origin/main again
  # for the follow-up build ("next") or deploy.follow ("follow")
  ShipRun{commit: String}
  ShipNext{why: String}
  # a request about a bot's browser and what it is taught (core/teach.bend):
  # the server keeps the driver and the draft, so it answers (m is
  # bots.browser or bots.teach, p the request's params as JSON text)
  Teach{client: U32, id: String, m: String, p: String}
  # the queue for the owner's machines that are away (the setting
  # far.queue, Fq.*) is due to be sent, in order
  FarQueued{}

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
    # a thread's background rows cleared (its agent or the hub that owned
    # them is gone: Bg.gone, Rec.lost*): what it waited on is no more, so
    # the Live Activity drops its "waiting on N" now, not at the next beat.
    # Only a clear: rows change at every subagent step, and each push costs
    case M.WorkSet{_, +kind, +v, _} <> t:
      Bool.or(Bool.and(String.eq(kind, "bg"), String.is_empty(v)), Island.touch(t))
    case _ <> t:
      Island.touch(t)

def Island.told(os: List<&2, Out>) -> Bool:
  match os:
    case Nil{}:
      False{}
    case TurnEnded{_, _, _, _, _, _} <> _:
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

# Alerts
# ------
# Every step passes through here before it is stored (the server's
# Hub.step.ok): an ask opened for the person gets its record in the same
# step (NO.Policy.ask), and every record written with a kind tells the
# server (Alerted), so alerts come from one place whatever path made them
# (laws policy_ask_alerts, policy_alerted_*).

def Alerts.has(cs: List<&2, M.Change>) -> Bool:
  match cs:
    case Nil{}:
      False{}
    case M.AskOpened{_, _, _, _, _, _, _} <> _:
      True{}
    case _ <> t:
      Alerts.has(t)

def Alerts.asks(+post: M.State, cs: List<&2, M.Change>) -> List<&2, M.Change>:
  match cs:
    case Nil{}:
      Nil{}
    case M.AskOpened{_, +th, _, +tool, +d, _, +at} <> t:
      List.append(&2, M.Change, NO.Policy.ask(post, at, th, tool, d), Alerts.asks(post, t))
    case _ <> t:
      Alerts.asks(post, t)

def Alerts.changes.k(has: Bool, +st: M.State, +cs: List<&2, M.Change>) -> List<&2, M.Change>:
  match has:
    case True{}:
      List.append(&2, M.Change, cs, Alerts.asks(M.State.replay(cs, st), cs))
    case False{}:
      cs

# the step's changes with the records of the asks it opens
# with what the person was told (told.bend's Told.after: every record that
# alerts is marked, under its key)
def Alerts.changes(+st: M.State, +cs: List<&2, M.Change>) -> List<&2, M.Change>:
  TLD.Told.after(st, Alerts.changes.k(Alerts.has(cs), st, cs))

# a record that alerts: its setting key starts "alert." and its kind (the
# value's first field) is not empty
def Alerts.is(+k: String, +v: String) -> Bool:
  Bool.and(String.starts_with(k, "alert."), Bool.not(String.starts_with(v, "\t")))

def Alerts.add(hit: Bool, +k: String, os: List<&2, Out>) -> List<&2, Out>:
  match hit:
    case True{}:
      List.append(&2, Out, os, [Alerted{String.drop(k, 6n)}])
    case False{}:
      os

def Alerts.outs(cs: List<&2, M.Change>, os: List<&2, Out>) -> List<&2, Out>:
  match cs:
    case Nil{}:
      os
    case M.SettingSet{+k, +v, _} <> t:
      Alerts.outs(t, Alerts.add(Alerts.is(k, v), k, os))
    case _ <> t:
      Alerts.outs(t, os)

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

# the answer to a request that returns a value
def Reply.data(+client: U32, +id: String, +v: J.Json) -> Out:
  Send{client, J.Json.show(J.Json.obj([J.KV{"t", J.Str{"reply"}}, J.KV{"id", J.Num{id}}, J.KV{"ok", J.Flag{True{}}}, J.KV{"result", v}]))}

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
    case 5n:
      [OcTurn{thread, cwd, model, session, runtime, text}]
    case 6n:
      [PiTurn{thread, cwd, model, session, runtime, text}]
    case 7n:
      [OmpTurn{thread, cwd, model, session, runtime, text}]
    case _:
      [AgentStart{thread, cwd, provider, model, session, runtime, interaction}, Claude.send(thread, cwd, text, uuid)]

# (uuid: the Claude command's, Hub.uuid)
def Hub.run(t: M.Thread, +cwd: String, +text: String, +uuid: String) -> List<&2, Out>:
  match t:
    case M.Thread{+id, _, _, _, _, _, _, _, _, _, _, _, _, _, _, +provider, model, runtime, interaction, session, _}:
      Hub.run.k(C.Codec.pos(["claude", "codex", "grok", "muse", "antigravity", "opencode", "pi", "omp"], provider), id, cwd, provider, model, session, runtime, interaction, text, uuid)

def Hub.post.go(wait: Bool, +t: M.Thread, +cwd: String, +queued: String, +now: Nat, +thread: String, +msg: String, role: M.Role, +text: String) -> Step:
  match wait:
    case True{}:
      Step{[M.MessagePosted{thread, msg, role, text, now}, M.TurnQueued{thread, msg, text, now}], Nil{}}
    case False{}:
      Step{List.concat(&2, M.Change, [[M.MessagePosted{thread, msg, role, text, now}],
          Bool.pick(List<&2, M.Change>, String.is_empty(queued), Nil{}, [M.QueueTaken{thread, now}]),
          [M.TurnChanged{thread, M.Running{}, now}]]),
        Git{0, "", "checkpoint0", thread, cwd, "", ""} <> Git{0, "", "step", thread, cwd, msg, "start"} <> Hub.run(t, cwd, M.Queue.join(queued, text), Hub.uuid(now, thread ++ "/" ++ msg))}

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

def Hub.thread_create.base(+s: M.State, +now: Nat, +client: U32, +id: String, +p: J.Json) -> Step:
  +project = J.Json.get_str(p, "project", "")
  +title = J.Json.get_str(p, "title", "New thread")
  +env = J.Json.get_str(p, "env", M.Setting.get(s, "thread.env", "auto"))
  +th = Hub.id("t", now, Hub.seq(s))
  Step{[M.ThreadCreated{th, project, title,
    Hub.new.env(M.Scratch.is(project), env), "", Hub.new.wt(M.Scratch.is(project), Hub.project.root(Hub.project(s, project)), th), J.Json.get_str(p, "provider", M.Setting.get(s, "agent.provider", "claude")),
    J.Json.get_str(p, "model", M.Setting.get(s, "agent.model", "")), now}],
    Reply.made(client, id, th) <> Hub.new.outs(M.Scratch.is(project), th, Hub.project.root(Hub.project(s, project)), env)}

# A new visible child is a normal thread. Parent links are never inferred
# from worker launches, delegated tasks or forks of older conversations.
def Hub.thread_create.link(+parent: String, +th: String, +now: Nat, step: Step) -> Step:
  match step:
    case Step{cs, os}:
      Step{List.append(&2, M.Change, cs, [M.ThreadParentSet{th, parent, now}, M.SettingSet{ST.Tree.open_key(parent), "1", now}]), os}

def Hub.thread_create.child(ok: Bool, +s: M.State, +now: Nat, +client: U32, +id: String, +p: J.Json, +parent: String) -> Step:
  match ok:
    case False{}: Step{Nil{}, [Reply.err(client, id, "Subthread parent must be an existing conversation in the same project (maximum depth 64).") ]}
    case True{}:
      Hub.thread_create.link(parent, Hub.id("t", now, Hub.seq(s)), now, Hub.thread_create.base(s, now, client, id, p))

def Hub.thread_create.kind(child: Bool, +s: M.State, +now: Nat, +client: U32, +id: String, +p: J.Json, +parent: String) -> Step:
  match child:
    case False{}: Hub.thread_create.base(s, now, client, id, p)
    case True{}:
      Hub.thread_create.child(Bool.and(ST.Tree.alive(Hub.thread(s, parent)), Bool.and(
        String.eq(ST.Tree.project(Hub.thread(s, parent)), J.Json.get_str(p, "project", "")),
        ST.Tree.chain(63n, s, ST.Tree.raw(s, parent), parent))), s, now, client, id, p, parent)

def Hub.thread_create(+s: M.State, +now: Nat, +client: U32, +id: String, +p: J.Json) -> Step:
  Hub.thread_create.kind(Bool.not(String.is_empty(J.Json.get_str(p, "parent", ""))), s, now, client, id, p, J.Json.get_str(p, "parent", ""))

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

# a request's byte array, in tail calls only (a 64 KB paste is 64K
# elements: a frame each overflowed the JS stack)
def Term.bytes.go(xs: List<&2, J.Json>, acc: List<&2, U32>) -> List<&2, U32>:
  match xs:
    case Nil{}:
      List.reverse(&2, U32, acc)
    case +x <> t:
      Term.bytes.go(t, U32.from_nat(Maybe.default(&2, Nat, Nat.read(J.Json.as_raw(Some{x}, "0")), 0n)) <> acc)

def Term.bytes(xs: List<&2, J.Json>) -> List<&2, U32>:
  Term.bytes.go(xs, Nil{})

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
  ["thread.snooze", "thread.reorder", "files.list", "kicad.open", "project.reorder", "mech.list", "mech.render", "file.open", "files.dir", "pdf.page", "kicad.export10", "boards.list"]

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
        Git{0, "", "checkpoint0", thread, cwd, "", ""} <> Git{0, "", "step", thread, cwd, "", "start"} <> List.append(&2, Out, Wt.name(first, t, queued), Hub.run(t, cwd, queued, Hub.uuid(now, thread ++ "/w/" ++ Nat.show(seq))))}
    case False{}:
      Step.none()

# gh saw the thread's branch merged: it settles, once (a recorded merge is
# not due again)
def Merge.found(due: Bool, +s: M.State, +now: Nat, +client: U32, +id: String, +thread: String, +url: String) -> Step:
  match due:
    case True{}:
      Step.reply(client, id, List.append(&2, M.Change, [M.ThreadMerged{thread, url, now},
        M.ActivityLogged{thread, Hub.id("g", now, Hub.seq(s)), "info", "git", "merged" ++ Bool.pick(String, String.is_empty(url), "", ": " ++ url), now}],
        NO.Policy.merged(s, now, thread, url)))
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
  ["history.resolve", "review.bom", "review.checks", "review.tick", "review.comment", "review.release", "zone.report", "zone.set"]

# The person's time zone (core/sched.bend's Zone.*)
# zone.report {zone}: a phone says where it is; kept as person.tz.device when
# it differs (the server looks the zone's rules up on its tick).
# zone.set {zone}: Settings sets the person's own ("" or "auto" follows the
# phone); a zone the hub has no rules for yet is looked up first
# (BotFx "zone"), and set only if that works.

def Zone.report.k(ok: Bool, same: Bool, +client: U32, +id: String, +zone: String, +now: Nat) -> Step:
  match ok same:
    case False{} _:
      Step.outs([Reply.err(client, id, "not a time zone name")])
    case True{} True{}:
      Step.outs([Reply.ok(client, id)])
    case True{} False{}:
      Step.reply(client, id, [M.SettingSet{"person.tz.device", zone, now}])

def Zone.report(+s: M.State, +now: Nat, +client: U32, +id: String, +zone: String) -> Step:
  Zone.report.k(SC.Zone.valid(zone), String.eq(M.Setting.get(s, "person.tz.device", ""), zone), client, id, zone, now)

def Zone.set.k(auto: Bool, ok: Bool, known: Bool, +client: U32, +id: String, +zone: String, +now: Nat) -> Step:
  match auto ok known:
    case True{} _ _:
      Step.reply(client, id, [M.SettingSet{"person.tz", "", now}])
    case False{} False{} _:
      Step.outs([Reply.err(client, id, "that is not a time zone name (like Europe/Helsinki)")])
    case False{} True{} True{}:
      Step.reply(client, id, [M.SettingSet{"person.tz", zone, now}])
    case False{} True{} False{}:
      Step.outs([BotFx{"zone", zone, U32.show(client), id}])

def Zone.set(+s: M.State, +now: Nat, +client: U32, +id: String, +zone: String) -> Step:
  Zone.set.k(Bool.or(String.is_empty(zone), String.eq(zone, "auto")), SC.Zone.valid(zone), BT.Maybe.is(TZ.TzRule, SC.Zone.rule(s, zone)), client, id, zone, now)

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
    case 6n:
      Zone.report(s, now, client, id, String.trim(J.Json.get_str(p, "zone", "")))
    case 7n:
      Zone.set(s, now, client, id, String.trim(J.Json.get_str(p, "zone", "")))
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
   "queue.edit", "queue.remove", "queue.move", "queue.steer", "thread.fork", "thread.back", "thread.provider", "update.now", "work.cancel", "task.cancel", "wake.cancel", "deploy.restart", "deploy.cancel", "deploy.status", "lock.release", "lock.cancel", "kicad.retry", "mail.list", "mail.read", "mail.find", "mail.mark", "mail.tag", "mail.send",
   "fab.look", "fab.intent", "order.buy", "models.refresh", "thread.detach"]

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

# a short line describing a tool call, from its input (a question, as a buy
# thread's order_ask asks, when there is nothing else; law
# tool_summary_question)
def Claude.tool_summary(+name: String, +input: J.Json) -> String:
  +cmd = J.Json.get_str(input, "command", "")
  +path = J.Json.get_str(input, "file_path", J.Json.get_str(input, "path", ""))
  +pat = J.Json.get_str(input, "pattern", "")
  Bool.pick(String, Bool.not(String.is_empty(cmd)), cmd,
    Bool.pick(String, Bool.not(String.is_empty(path)), path,
      Bool.pick(String, Bool.not(String.is_empty(pat)), pat, J.Json.get_str(input, "description", J.Json.get_str(input, "question", name)))))

# The CLI writes one block per line, each numbered from 0, so two text
# blocks of one message (text, thinking, text) share "msg:0". A line that
# has a uuid adds its first 8 characters to the id; one without keeps the
# old id (and the persisted ids are untouched: law claude_text_ids_distinct).
def Claude.uid(+uuid: String) -> String:
  Bool.pick(String, String.is_empty(uuid), "", ":" ++ String.take(uuid, 8n))

# Thinking (model.bend's Think): a finished thought is an act of tone
# "thinking", logged only while the setting is on; an empty one (Claude
# started without --thinking-display, or the setting just turned on) is not
def Think.act(think: Bool, +thread: String, +id: String, +text: String, +now: Nat) -> List<&2, M.Change>:
  Bool.pick(List<&2, M.Change>, Bool.and(think, Bool.not(String.is_empty(String.trim(text)))), [M.ActivityLogged{thread, id, M.Think.tone(), "thinking", text, now}], Nil{})

# the item kinds a Codex-shaped item.completed is read for (a reasoning
# item, Codex's summary or Pi's thought, is logged only while thinking is on)
def Think.items() -> List<&2, String>:
  ["agent_message", "command_execution", "file_change", "todo_list", "reasoning"]

def Claude.block.k(i: Nat, +thread: String, +msg: String, +n: Nat, +now: Nat, +seq: Nat, +uuid: String, +b: J.Json, +think: Bool) -> List<&2, M.Change>:
  match i:
    case 0n:
      [M.MessagePosted{thread, msg ++ ":" ++ Nat.show(n) ++ Claude.uid(uuid), M.Assistant{}, J.Json.get_str(b, "text", ""), now}]
    case 1n:
      [M.ActivityLogged{thread, J.Json.get_str(b, "id", Hub.id("a", now, seq)), "tool", J.Json.get_str(b, "name", "tool"),
        Claude.tool_summary(J.Json.get_str(b, "name", "tool"), J.Json.get_obj(b, "input")), now}]
    case 2n:
      Think.act(think, thread, "think:" ++ msg ++ ":" ++ Nat.show(n) ++ Claude.uid(uuid), J.Json.get_str(b, "thinking", ""), now)
    case _:
      Nil{}

def Claude.blocks(bs: List<&2, J.Json>, +thread: String, +msg: String, +n: Nat, +now: Nat, +seq: Nat, +uuid: String, +think: Bool) -> List<&2, M.Change>:
  match bs:
    case Nil{}:
      Nil{}
    case +b <> t:
      List.append(&2, M.Change,
        Claude.block.k(C.Codec.pos(["text", "tool_use", "thinking"], J.Json.get_str(b, "type", "")), thread, msg, n, now, seq, uuid, b, think),
        Claude.blocks(t, thread, msg, 1n+n, now, seq, uuid, think))

def Claude.assistant(+thread: String, +now: Nat, +seq: Nat, +m: J.Json, +uuid: String, +think: Bool) -> Step:
  Step{Claude.blocks(J.Json.elems(J.Json.get_obj(m, "content")), thread, J.Json.get_str(m, "id", "msg"), 0n, now, seq, uuid, think),
    [Broadcast{J.Json.show(J.Json.obj([J.KV{"t", J.Str{"delta_end"}}, J.KV{"thread", J.Str{thread}}]))}]}

# A model or API error Claude Code reports for the running turn (a bad
# model name, a server or billing error, a rate limit) comes as an
# assistant line whose "error" names it, its text the message. It is logged
# as the turn's own error (tone "error", kind "api"), as a failed result
# is, so a turn that then ends without a failed result still says it
# failed by its own error (wnote.bend's Wn.err). Lines for a turn no
# longer running log nothing.
def Claude.api.text(bs: List<&2, J.Json>) -> String:
  match bs:
    case Nil{}:
      ""
    case +b <> t:
      Bool.pick(String, String.eq(J.Json.get_str(b, "type", ""), "text"), J.Json.get_str(b, "text", ""), Claude.api.text(t))

def Claude.api.err.k(said: Bool, +thread: String, +now: Nat, +seq: Nat, +code: String, +text: String) -> List<&2, M.Change>:
  match said:
    case True{}:
      [M.ActivityLogged{thread, Hub.id("api", now, seq), "error", "api", Bool.pick(String, String.is_empty(String.trim(text)), code, text), now}]
    case False{}:
      Nil{}

def Claude.api.err(run: Bool, +thread: String, +now: Nat, +seq: Nat, +o: J.Json) -> List<&2, M.Change>:
  Claude.api.err.k(Bool.and(run, Bool.not(String.is_empty(J.Json.get_str(o, "error", "")))), thread, now, seq, J.Json.get_str(o, "error", ""),
    Claude.api.text(J.Json.elems(J.Json.get_obj(J.Json.get_obj(o, "message"), "content"))))

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

# Turn usage (model.bend's Tokens): each turn's tokens and cost as the
# provider reported them, logged once (TurnUsage). Claude's result line
# gives the turn's own usage; its cost is the process's running total, so a
# turn's cost is that total less the last one (result_index 0 starts a
# process: from 0). Codex's turn.completed gives the session's running
# totals, kept across resume, so a turn is the rise since the session's
# last (a new session, or any total going down, starts again). A line seen
# again (same key) is not counted twice.

def Turn.raw(+o: J.Json, +k: String) -> String:
  J.Json.as_raw(J.Json.get(o, k), "")

def Turn.has(+o: J.Json, +k: String) -> Bool:
  Bool.not(String.is_empty(Turn.raw(o, k)))

def Turn.nth(ps: List<&2, String>, +i: Nat) -> String:
  Maybe.default(&2, String, List.head(&2, String, List.drop(&2, String, ps, i)), "")

def Turn.shift(n: Nat, +v: Nat) -> Nat:
  match n:
    case 0n:
      v
    case 1n+k:
      Turn.shift(k, Nat.div(v, 10n))

def Turn.mant(+m: String) -> Nat:
  Nat.add(Nat.mul(M.Switch.num(Turn.nth(String.split(m, '.'), 0n), 0n), 1000000000n),
    M.Switch.num(String.take(Turn.nth(String.split(m, '.'), 1n) ++ "000000000", 9n), 0n))

# a JSON number of dollars ("0.00070325", "7.5e-7") in nano-dollars
def Turn.nusd(+raw: String) -> Nat:
  Bool.pick(Nat, String.starts_with(Turn.nth(String.split(raw, 'e'), 1n), "-"),
    Turn.shift(M.Switch.num(String.drop(Turn.nth(String.split(raw, 'e'), 1n), 1n), 0n), Turn.mant(Turn.nth(String.split(raw, 'e'), 0n))),
    Turn.mant(Turn.nth(String.split(raw, 'e'), 0n)))

def Turn.model(t: M.Thread) -> String:
  match t:
    case M.Thread{_, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, mo, _, _, _, _}:
      mo

def Turn.session(t: M.Thread) -> String:
  match t:
    case M.Thread{_, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, se, _}:
      se

# the session's main model: the one with the widest window (helpers such
# as a small model for titles are listed too, and may come first)
def Turn.main(fs: J.Json, +best: String, +w: Nat) -> String:
  match fs:
    case J.Field{+k, +v, t}:
      Turn.main(t, Bool.pick(String, Bool.or(String.is_empty(best), Nat.is_gt(J.Json.get_nat(v, "contextWindow", 0n), w)), k, best),
        Bool.pick(Nat, Bool.or(String.is_empty(best), Nat.is_gt(J.Json.get_nat(v, "contextWindow", 0n), w)), J.Json.get_nat(v, "contextWindow", 0n), w))
    case _:
      best

def Turn.or(+a: String, +b: String) -> String:
  Bool.pick(String, String.is_empty(a), b, a)

def Turn.last(+s: M.State, +th: String) -> String:
  M.Setting.get(s, "tok.key." ++ th, "")

def Turn.basis(+s: M.State, +th: String) -> String:
  M.Setting.get(s, "tok.basis." ++ th, "")

# Claude: the session's running cost before this turn, as the baseline
# "c:<nano-dollars>" ("" when unknown). A session the hub saw start has 0
# (Turn.claude.start); a session resumed in a new process keeps its total
# going (result_index 0 there does not mean 0, measured 2026-10-16), so
# only a session start resets it
def Turn.claude.basis(+s: M.State, +th: String) -> String:
  Bool.pick(String, String.starts_with(Turn.basis(s, th), "c:"), Turn.basis(s, th), "")

def Turn.claude.base(+s: M.State, +th: String) -> Nat:
  M.Switch.num(String.drop(Turn.claude.basis(s, th), 2n), 0n)

# a Claude session the hub sees start (a new session id): its cost starts at 0
def Turn.claude.start(same: Bool, +th: String, +sid: String, +now: Nat) -> List<&2, M.Change>:
  match same:
    case True{}:
      Nil{}
    case False{}:
      Bool.pick(List<&2, M.Change>, String.is_empty(sid), Nil{}, [M.SettingSet{"tok.basis." ++ th, "c:0", now}])

# a count started again once more (the setting tok.recount, part of the
# totals' prefix): a count's totals from before 2026-10-16's fix for
# resumed Claude sessions charged one turn with the session's past cost
def Turn.recount(+s: M.State, +now: Nat) -> List<&2, M.Change>:
  Bool.pick(List<&2, M.Change>, String.is_empty(M.Setting.get(s, "tok.recount", "")), [M.SettingSet{"tok.recount", "b", now}], Nil{})

def Turn.rise(+now: Nat, +before: Nat) -> Nat:
  Bool.pick(Nat, Nat.is_ge(now, before), Nat.sub(now, before), now)

# counting starts (again) at the first turn of a build that keeps baselines
# (the setting tok.epoch; model.bend's Tok.pre): turns counted before it
# could take a process's or session's whole past as one turn's
def Turn.epoch(+s: M.State, +now: Nat) -> List<&2, M.Change>:
  Bool.pick(List<&2, M.Change>, String.is_empty(M.Setting.get(s, "tok.epoch", "")), [M.SettingSet{"tok.epoch", "1", now}], Nil{})

def Turn.restart(+s: M.State, +now: Nat) -> List<&2, M.Change>:
  List.append(&2, M.Change, Turn.epoch(s, now), Turn.recount(s, now))

# whether a Claude turn's cost can be told: Claude gave a running total, and
# either it starts from 0 (first) or the total before it is known. Claude
# turns pass first = False: a session's start is told by its id
# (Turn.claude.start sets the baseline to 0), not by result_index
def Turn.cost.known(has: Bool, first: Bool, +basis: String) -> Bool:
  Bool.and(has, Bool.or(first, Bool.not(String.is_empty(basis))))

# Speed (usage.bend's Ts): Claude's result gives the process's running API
# time (duration_api_ms, every model call's own time added up) and, per
# model, its running output tokens (modelUsage); a turn's share is the rise
# since the last result (measured 2026-10-24: both run on across a
# session's turns). The setting turn.speed.<thread> is
# "<api ms>/<out>/<turn api ms>/<turn out>/<turn wall ms>/<timed api ms>/<timed out>/<steps>":
# the running totals, the last turn's share and its wall time
# (duration_ms), the thread's turns whose share was told, and the turn's
# model calls (num_turns). A session the hub saw start starts at 0
# (Speed.start); one already running is not told until its next result.
def Speed.key(+th: String) -> String:
  "turn.speed." ++ th

def Speed.outs(fs: J.Json, +acc: Nat) -> Nat:
  match fs:
    case J.Field{_, +v, t}:
      Speed.outs(t, Nat.add(acc, J.Json.get_nat(v, "outputTokens", 0n)))
    case _:
      acc

def Speed.share(known: Bool, +now: Nat, +before: Nat) -> Nat:
  Bool.pick(Nat, known, Turn.rise(now, before), 0n)

def Speed.told(told: Bool, +n: Nat) -> Nat:
  Bool.pick(Nat, told, n, 0n)

def Speed.value.k(+old: String, +api: Nat, +out: Nat, +wall: Nat, +steps: Nat, +a: Nat, +o: Nat, +told: Bool) -> String:
  String.join([Nat.show(api), Nat.show(out), Nat.show(Speed.told(told, a)), Nat.show(Speed.told(told, o)), Nat.show(wall),
    Nat.show(Nat.add(M.Tok.part(old, 5n), Speed.told(told, a))), Nat.show(Nat.add(M.Tok.part(old, 6n), Speed.told(told, o))), Nat.show(steps)], "/")

def Speed.value(+old: String, +api: Nat, +out: Nat, +wall: Nat, +steps: Nat) -> String:
  +a = Speed.share(Bool.not(String.is_empty(old)), api, M.Tok.part(old, 0n))
  +o = Speed.share(Bool.not(String.is_empty(old)), out, M.Tok.part(old, 1n))
  Speed.value.k(old, api, out, wall, steps, a, o, Bool.and(Nat.is_gt(a, 0n), Nat.is_gt(o, 0n)))

def Speed.claude(+s: M.State, +th: String, +now: Nat, +o: J.Json) -> List<&2, M.Change>:
  Bool.pick(List<&2, M.Change>, Turn.has(o, "duration_api_ms"),
    [M.SettingSet{Speed.key(th), Speed.value(M.Setting.get(s, Speed.key(th), ""), J.Json.get_nat(o, "duration_api_ms", 0n),
      Speed.outs(Usage.fields(J.Json.get_obj(o, "modelUsage")), 0n), J.Json.get_nat(o, "duration_ms", 0n), J.Json.get_nat(o, "num_turns", 0n)), now}], Nil{})

# a new session's running totals start at 0; the last turn's and the
# thread's stay
def Speed.restart(+old: String) -> String:
  String.join(["0", "0", Nat.show(M.Tok.part(old, 2n)), Nat.show(M.Tok.part(old, 3n)), Nat.show(M.Tok.part(old, 4n)),
    Nat.show(M.Tok.part(old, 5n)), Nat.show(M.Tok.part(old, 6n)), Nat.show(M.Tok.part(old, 7n))], "/")

def Speed.start(+s: M.State, same: Bool, +th: String, +sid: String, +now: Nat) -> List<&2, M.Change>:
  Bool.pick(List<&2, M.Change>, Bool.or(same, String.is_empty(sid)), Nil{},
    [M.SettingSet{Speed.key(th), Speed.restart(M.Setting.get(s, Speed.key(th), "")), now}])

# Claude's session totals per model (modelUsage: the main model and any
# helper or subagent model), as the setting turn.models.<thread>, a line
# per model "<model>|<in>|<out>|<cache read>|<cache write>|<nano-dollars>|<web searches>|<window>",
# logged when it changes
def Mu.key(+th: String) -> String:
  "turn.models." ++ th

def Mu.line(+k: String, +v: J.Json) -> String:
  k ++ "|" ++ String.join([Nat.show(J.Json.get_nat(v, "inputTokens", 0n)), Nat.show(J.Json.get_nat(v, "outputTokens", 0n)),
    Nat.show(J.Json.get_nat(v, "cacheReadInputTokens", 0n)), Nat.show(J.Json.get_nat(v, "cacheCreationInputTokens", 0n)),
    Nat.show(Turn.nusd(Turn.raw(v, "costUSD"))), Nat.show(J.Json.get_nat(v, "webSearchRequests", 0n)), Nat.show(J.Json.get_nat(v, "contextWindow", 0n))], "|")

def Mu.lines(fs: J.Json, +acc: String) -> String:
  match fs:
    case J.Field{+k, +v, t}:
      Mu.lines(t, Bool.pick(String, String.is_empty(acc), Mu.line(k, v), acc ++ "\n" ++ Mu.line(k, v)))
    case _:
      acc

def Mu.set(+s: M.State, +th: String, +now: Nat, +v: String) -> List<&2, M.Change>:
  Bool.pick(List<&2, M.Change>, Bool.or(String.is_empty(v), String.eq(v, M.Setting.get(s, Mu.key(th), ""))), Nil{}, [M.SettingSet{Mu.key(th), v, now}])

def Mu.claude(+s: M.State, +th: String, +now: Nat, +o: J.Json) -> List<&2, M.Change>:
  Mu.set(s, th, now, Mu.lines(Usage.fields(J.Json.get_obj(o, "modelUsage")), ""))

def Turn.claude.k(fresh: Bool, +s: M.State, +th: String, +now: Nat, +o: J.Json) -> List<&2, M.Change>:
  match fresh:
    case False{}:
      Nil{}
    case True{}:
      +u = J.Json.get_obj(o, "usage")
      +total = Turn.nusd(Turn.raw(o, "total_cost_usd"))
      +hasc = Bool.and(Turn.cost.known(Turn.has(o, "total_cost_usd"), False{}, Turn.claude.basis(s, th)), Nat.is_ge(total, Turn.claude.base(s, th)))
      +think = Turn.has(J.Json.get_obj(u, "output_tokens_details"), "thinking_tokens")
      List.concat(&2, M.Change, [Turn.restart(s, now), [M.TurnUsage{th, "claude", Turn.or(Turn.model(Hub.thread(s, th)), Turn.main(Usage.fields(J.Json.get_obj(o, "modelUsage")), "", 0n)), J.Json.get_str(o, "uuid", ""),
        J.Json.get_nat(u, "input_tokens", 0n), J.Json.get_nat(u, "cache_read_input_tokens", 0n), J.Json.get_nat(u, "cache_creation_input_tokens", 0n),
        J.Json.get_nat(u, "output_tokens", 0n), J.Json.get_nat(J.Json.get_obj(u, "output_tokens_details"), "thinking_tokens", 0n),
        Bool.pick(Nat, hasc, Nat.sub(total, Turn.claude.base(s, th)), 0n),
        "irwo" ++ Bool.pick(String, think, "t", "") ++ Bool.pick(String, hasc, "c", ""),
        Bool.pick(String, Turn.has(o, "total_cost_usd"), "c:" ++ Nat.show(total), Turn.basis(s, th)), now}],
        Speed.claude(s, th, now, o), Mu.claude(s, th, now, o)])

# a result line's turn, once
def Turn.claude(+s: M.State, +th: String, +now: Nat, +o: J.Json) -> List<&2, M.Change>:
  Turn.claude.k(Bool.and(Turn.has(J.Json.get_obj(o, "usage"), "output_tokens"),
    Bool.and(Bool.not(String.is_empty(J.Json.get_str(o, "uuid", ""))), Bool.not(String.eq(J.Json.get_str(o, "uuid", ""), Turn.last(s, th))))), s, th, now, o)

# Codex: the session's totals before this turn (all 0 for a new session)
def Turn.codex.was(+s: M.State, +th: String, +se: String) -> List<&2, String>:
  Bool.pick(List<&2, String>, String.starts_with(Turn.basis(s, th), "x:" ++ se ++ ":"),
    String.split(String.drop(Turn.basis(s, th), Nat.add(String.length(se), 3n)), '/'), Nil{})

def Turn.num(ps: List<&2, String>, +i: Nat) -> Nat:
  M.Switch.num(Turn.nth(ps, i), 0n)

# whether every total rose or held (else the session started again)
def Turn.codex.up(+was: List<&2, String>, +ci: Nat, +cc: Nat, +cw: Nat, +co: Nat, +ct: Nat) -> Bool:
  Bool.and(Bool.and(Nat.is_ge(ci, Turn.num(was, 0n)), Nat.is_ge(cc, Turn.num(was, 1n))),
    Bool.and(Bool.and(Nat.is_ge(cw, Turn.num(was, 2n)), Nat.is_ge(co, Turn.num(was, 3n))), Nat.is_ge(ct, Turn.num(was, 4n))))

def Turn.codex.d(up: Bool, +was: List<&2, String>, +i: Nat, +v: Nat) -> Nat:
  match up:
    case True{}:
      Nat.sub(v, Turn.num(was, i))
    case False{}:
      v

# the provider a Codex-shaped turn is counted for: the thread's (Codex,
# or Oh My Pi, whose session totals come the same way, omp.bend)
def Turn.codex.prov(+s: M.State, +th: String) -> String:
  Turn.or(M.Thread.provider(Hub.thread(s, th)), "codex")

# the session's running cost (nano-dollars), when the provider gives one
# (Oh My Pi's cost_usd; Codex gives none)
def Turn.codex.cost(+u: J.Json) -> Nat:
  Turn.nusd(Turn.raw(u, "cost_usd"))

def Turn.codex.m(+s: M.State, +th: String, +now: Nat, +se: String, +u: J.Json, +was: List<&2, String>, +ci: Nat, +cc: Nat, +cw: Nat, +co: Nat, +ct: Nat, +up: Bool) -> List<&2, M.Change>:
  [M.TurnUsage{th, Turn.codex.prov(s, th), Turn.or(Turn.model(Hub.thread(s, th)), "default"), "x:" ++ se ++ ":" ++ Nat.show(ci) ++ "/" ++ Nat.show(co),
    Nat.sub(Turn.codex.d(up, was, 0n, ci), Turn.codex.d(up, was, 1n, cc)), Turn.codex.d(up, was, 1n, cc), Turn.codex.d(up, was, 2n, cw),
    Turn.codex.d(up, was, 3n, co), Turn.codex.d(up, was, 4n, ct), Bool.pick(Nat, Turn.has(u, "cost_usd"), Turn.codex.d(up, was, 5n, Turn.codex.cost(u)), 0n),
    "irwo" ++ Bool.pick(String, Turn.has(u, "reasoning_output_tokens"), "t", "") ++ Bool.pick(String, Turn.has(u, "cost_usd"), "c", ""),
    "x:" ++ se ++ ":" ++ String.join([Nat.show(ci), Nat.show(cc), Nat.show(cw), Nat.show(co), Nat.show(ct), Nat.show(Turn.codex.cost(u))], "/"), now}]

# a session whose start the hub saw (Turn.codex.start set its totals to 0)
# counts the rise; one already running when it was first seen counts the
# turn with its tokens unknown, and its totals become the baseline
def Turn.codex.seen(seen: Bool, +s: M.State, +th: String, +now: Nat, +se: String, +u: J.Json, +was: List<&2, String>, +ci: Nat, +cc: Nat, +cw: Nat, +co: Nat, +ct: Nat) -> List<&2, M.Change>:
  match seen:
    case True{}:
      Turn.codex.m(s, th, now, se, u, was, ci, cc, cw, co, ct, Turn.codex.up(was, ci, cc, cw, co, ct))
    case False{}:
      [M.TurnUsage{th, Turn.codex.prov(s, th), Turn.or(Turn.model(Hub.thread(s, th)), "default"), "x:" ++ se ++ ":" ++ Nat.show(ci) ++ "/" ++ Nat.show(co),
        0n, 0n, 0n, 0n, 0n, 0n, "", "x:" ++ se ++ ":" ++ String.join([Nat.show(ci), Nat.show(cc), Nat.show(cw), Nat.show(co), Nat.show(ct), Nat.show(Turn.codex.cost(u))], "/"), now}]

# a Codex session the hub sees start: its totals start at 0
def Turn.codex.start(same: Bool, +th: String, +se: String, +now: Nat) -> List<&2, M.Change>:
  match same:
    case True{}:
      Nil{}
    case False{}:
      [M.SettingSet{"tok.basis." ++ th, "x:" ++ se ++ ":0/0/0/0/0", now}]

def Turn.codex.k(fresh: Bool, +s: M.State, +th: String, +now: Nat, +se: String, +u: J.Json) -> List<&2, M.Change>:
  match fresh:
    case False{}:
      Nil{}
    case True{}:
      +was = Turn.codex.was(s, th, se)
      +ci = J.Json.get_nat(u, "input_tokens", 0n)
      +cc = J.Json.get_nat(u, "cached_input_tokens", 0n)
      +cw = J.Json.get_nat(u, "cache_write_input_tokens", 0n)
      +co = J.Json.get_nat(u, "output_tokens", 0n)
      +ct = J.Json.get_nat(u, "reasoning_output_tokens", 0n)
      List.append(&2, M.Change, Turn.restart(s, now),
        Turn.codex.seen(String.starts_with(Turn.basis(s, th), "x:" ++ se ++ ":"), s, th, now, se, u, was, ci, cc, cw, co, ct))

# a turn.completed line's turn, once
def Turn.codex(+s: M.State, +th: String, +now: Nat, +o: J.Json) -> List<&2, M.Change>:
  Turn.codex.k(Bool.and(Turn.has(J.Json.get_obj(o, "usage"), "output_tokens"),
    Bool.not(String.eq("x:" ++ Turn.session(Hub.thread(s, th)) ++ ":" ++ Nat.show(J.Json.get_nat(J.Json.get_obj(o, "usage"), "input_tokens", 0n)) ++ "/" ++
      Nat.show(J.Json.get_nat(J.Json.get_obj(o, "usage"), "output_tokens", 0n)), Turn.last(s, th)))),
    s, th, now, Turn.session(Hub.thread(s, th)), J.Json.get_obj(o, "usage"))

# a result line: the context window, and the turn's usage
def Usage.result(+s: M.State, +th: String, +now: Nat, +o: J.Json) -> List<&2, M.Change>:
  List.append(&2, M.Change, Usage.window(s, th, now, o), Turn.claude(s, th, now, o))

# Claude's plan limits (rate_limit_event): each window's share used, when it
# resets and the status, as the setting limit.claude.<window> =
# "<per mille>/<resets at>/<status>" (the share kept to whole percent), logged
# only when it changes
def Lim.permille(+raw: String) -> Nat:
  Nat.mul(Nat.div(Nat.div(Turn.nusd(raw), 1000000n), 10n), 10n)

def Lim.value(+w: J.Json, +status: String) -> String:
  Nat.show(Lim.permille(Turn.raw(w, "utilization"))) ++ "/" ++ Turn.raw(w, "resetsAt") ++ "/" ++ status

def Lim.one(+s: M.State, +now: Nat, +name: String, +w: J.Json, +status: String) -> List<&2, M.Change>:
  Bool.pick(List<&2, M.Change>, Bool.or(Bool.not(Turn.has(w, "utilization")), String.eq(Lim.value(w, status), M.Setting.get(s, "limit.claude." ++ name, ""))), Nil{},
    [M.SettingSet{"limit.claude." ++ name, Lim.value(w, status), now}])

def Lim.claude(+s: M.State, +now: Nat, +o: J.Json) -> List<&2, M.Change>:
  List.append(&2, M.Change,
    Lim.one(s, now, "five_hour", J.Json.get_obj(J.Json.get_obj(J.Json.get_obj(o, "rate_limit_info"), "unifiedWindows"), "five_hour"), J.Json.get_str(J.Json.get_obj(o, "rate_limit_info"), "status", "")),
    Lim.one(s, now, "seven_day", J.Json.get_obj(J.Json.get_obj(J.Json.get_obj(o, "rate_limit_info"), "unifiedWindows"), "seven_day"), J.Json.get_str(J.Json.get_obj(o, "rate_limit_info"), "status", "")))

# an assistant message and what its todo tools do to the thread's list
def Claude.assistant.t(+s: M.State, +thread: String, +now: Nat, +m: J.Json, +uuid: String) -> Step:
  Step.after(Step.after(Claude.assistant(thread, now, Hub.seq(s), m, uuid, M.Think.on(s)), Claude.todos(s, thread, now, J.Json.elems(J.Json.get_obj(m, "content")))), Usage.claude(s, thread, now, m))

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

# the session id arrives in the init line; record it once (and a new
# session's cost starts at 0, Turn.claude.start)
def Claude.init(+s: M.State, +thread: String, +now: Nat, +o: J.Json) -> Step:
  +sid = J.Json.get_str(o, "session_id", "")
  Step.after(Claude.init.k(Claude.init.same(Hub.thread(s, thread), sid), thread, sid, now), List.append(&2, M.Change, Turn.claude.start(Claude.init.same(Hub.thread(s, thread), sid), thread, sid, now),
    Speed.start(s, Claude.init.same(Hub.thread(s, thread), sid), thread, sid, now)))

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

# A missing timeout uses the hub default; explicit zero is the opt-out.
# All deadlines report only. They never signal a task or its process.
def WorkTimeout.number(s: String, valid: Bool, +acc: Nat) -> Maybe<&2, Nat>:
  match s valid:
    case _ False{}: None{}
    case SNil{} True{}: Some{acc}
    case SCon{+c, t} True{}:
      WorkTimeout.number(t, Char.is_digit(c), Nat.add(Nat.mul(acc, 10n), U32.to_nat((Char.to_u32(c) - 48 : U32))))

def WorkTimeout.parse(+raw: String, fallback: Nat) -> Nat:
  Maybe.default(&2, Nat, WorkTimeout.number(raw, Bool.and(Bool.not(String.is_empty(raw)), Nat.is_le(String.length(raw), 10n)), 0n), fallback)

def WorkTimeout.seconds(+s: M.State, +a: J.Json) -> Nat:
  WorkTimeout.parse(J.Json.as_raw(J.Json.get(a, "timeoutSec"), ""), WorkTimeout.parse(M.Setting.get(s, "work.timeout.seconds", "1200"), 1200n))

def WorkTimeout.key(+th: String, +id: String) -> String:
  "work.deadline." ++ th ++ "." ++ id

def WorkTimeout.start.k(fresh: Bool, +s: M.State, +now: Nat, +th: String, +id: String, +secs: Nat) -> List<&2, M.Change>:
  match fresh:
    case False{}: Nil{}
    case True{}:
      [M.SettingSet{WorkTimeout.key(th, id), Nat.show(Bool.pick(Nat, Nat.is_eq(secs, 0n), 0n, Nat.add(now, secs))), now},
        M.SettingSet{WorkTimeout.key(th, id) ++ ".noted", "", now}]

def WorkTimeout.start(+s: M.State, +now: Nat, +th: String, +id: String, +a: J.Json) -> List<&2, M.Change>:
  WorkTimeout.start.k(String.is_empty(M.Setting.get(s, WorkTimeout.key(th, id), "")), s, now, th, id, WorkTimeout.seconds(s, a))

def Bg.started(agent: Bool, +s: M.State, +th: String, +now: Nat, +o: J.Json) -> List<&2, M.Change>:
  match agent:
    case True{}:
      +call = J.Json.get_str(o, "tool_use_id", "")
      +what = J.Json.get_str(o, "description", "Subagent")
      +kind = M.Bg.kind(J.Json.get_str(o, "task_type", ""))
      List.append(&2, M.Change, Bg.put(s, th, now, M.Bg.start(M.Bg.of(s, th), J.Json.get_str(o, "task_id", ""), what, call, what, kind)),
        List.append(&2, M.Change, Bg.mark(th, now, call, "start:" ++ kind, what), WorkTimeout.start(s, now, th, J.Json.get_str(o, "task_id", ""), o)))
    case False{}:
      Nil{}

# a foreground shell Claude moved to the background when it ran past its
# timeout (task_updated sets is_backgrounded; the patch has no description)
def Bg.moved(bg: Bool, +s: M.State, +th: String, +now: Nat, +id: String) -> List<&2, M.Change>:
  match bg:
    case True{}:
      List.append(&2, M.Change, Bg.put(s, th, now, M.Bg.start(M.Bg.of(s, th), id, "Background shell", "", "Background shell", "shell")), WorkTimeout.start(s, now, th, id, J.Null{}))
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

# Open tool calls
# ---------------
# Where a tool call starts and ends, the hub tells the server (ToolOpen,
# ToolShut) and the server keeps the open ones in memory, for the safe
# deploy (core/deploy.bend): Claude's tool_use block opens, the tool_result
# block of the same id shuts; Codex's item.started opens and item.completed
# shuts; Grok's, Muse's and Antigravity's logged call opens and its
# "tool.done" shuts. A turn's end, an archive or a delete shuts every call of
# the thread (ToolsGone). The requester's own deploy_* call is never opened.
# Nothing here is written to the log (laws tool_*).

def Tools.open.k(go: Bool, +thread: String, +id: String, rest: List<&2, Out>) -> List<&2, Out>:
  match go:
    case True{}:
      ToolOpen{thread, id} <> rest
    case False{}:
      rest

# the tool calls the changes log for a thread's own timeline
def Tools.opens(cs: List<&2, M.Change>, +thread: String) -> List<&2, Out>:
  match cs:
    case Nil{}:
      Nil{}
    case M.ActivityLogged{+th, +id, +tone, +kind, _, _} <> t:
      Tools.open.k(Bool.and(Bool.and(String.eq(th, thread), String.eq(tone, "tool")), Bool.not(String.contains(kind, "deploy_"))), thread, id, Tools.opens(t, thread))
    case _ <> t:
      Tools.opens(t, thread)

def Tools.step(st: Step, +thread: String) -> Step:
  match st:
    case Step{+cs, os}:
      Step{cs, List.append(&2, Out, os, Tools.opens(cs, thread))}

def Tools.shut.k(go: Bool, +thread: String, +id: String, rest: List<&2, Out>) -> List<&2, Out>:
  match go:
    case True{}:
      ToolShut{thread, id} <> rest
    case False{}:
      rest

# Claude: every tool result of a line ends its call
def Tools.shuts(bs: List<&2, J.Json>, +thread: String) -> List<&2, Out>:
  match bs:
    case Nil{}:
      Nil{}
    case +b <> t:
      Tools.shut.k(String.eq(J.Json.get_str(b, "type", ""), "tool_result"), thread, J.Json.get_str(b, "tool_use_id", ""), Tools.shuts(t, thread))

def Tools.kind(+t: String) -> Bool:
  Bool.or(String.eq(t, "command_execution"), String.eq(t, "file_change"))

# Codex-like lines (acp: Grok, Muse, Antigravity log the call when it starts
# and end it at "tool.done"; Codex logs it when it completes)
def Tools.line.k(i: Nat, acp: Bool, +thread: String, +id: String, +kind: Bool) -> List<&2, Out>:
  match i:
    case 1n:
      Bool.pick(List<&2, Out>, Bool.and(kind, Bool.not(String.is_empty(id))), Bool.pick(List<&2, Out>, acp, [ToolOpen{thread, id}], [ToolShut{thread, id}]), Nil{})
    case 5n:
      Bool.pick(List<&2, Out>, Bool.and(kind, Bool.and(Bool.not(acp), Bool.not(String.is_empty(id)))), [ToolOpen{thread, id}], Nil{})
    case 6n:
      Bool.pick(List<&2, Out>, String.is_empty(id), Nil{}, [ToolShut{thread, id}])
    case _:
      Nil{}

def Tools.line.id(i: Nat, +o: J.Json) -> String:
  Bool.pick(String, Nat.is_eq(i, 6n), J.Json.get_str(o, "id", ""), J.Json.get_str(J.Json.get_obj(o, "item"), "id", ""))

def Tools.line(+i: Nat, acp: Bool, +thread: String, +o: J.Json) -> List<&2, Out>:
  Tools.line.k(i, acp, thread, Tools.line.id(i, o), Tools.kind(J.Json.get_str(J.Json.get_obj(o, "item"), "type", "")))

def Tools.gone.k(go: Bool, +th: String, rest: List<&2, Out>) -> List<&2, Out>:
  match go:
    case True{}:
      ToolsGone{th} <> rest
    case False{}:
      rest

def Tools.ended(st: M.TurnState) -> Bool:
  match st:
    case M.Running{}:
      False{}
    case _:
      True{}

# the changes that end a thread's calls: a turn that is not running, an
# archive, a delete
def Tools.gone(cs: List<&2, M.Change>) -> List<&2, Out>:
  match cs:
    case Nil{}:
      Nil{}
    case M.TurnChanged{+th, st, _} <> t:
      Tools.gone.k(Tools.ended(st), th, Tools.gone(t))
    case M.ThreadArchived{+th, +b, _} <> t:
      Tools.gone.k(b, th, Tools.gone(t))
    case M.ThreadDeleted{+th, _} <> t:
      ToolsGone{th} <> Tools.gone(t)
    case _ <> t:
      Tools.gone(t)

# the step's outputs, with the calls of the threads it ended shut after them
def Tools.outs(+cs: List<&2, M.Change>, +os: List<&2, Out>) -> List<&2, Out>:
  List.append(&2, Out, os, Tools.gone(cs))

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
      Step.after(Tools.step(Claude.assistant.t(s, thread, now, J.Json.get_obj(o, "message"), J.Json.get_str(o, "uuid", "")), thread),
        Claude.api.err(Hub.running(Hub.thread(s, thread)), thread, now, Hub.seq(s), o))
    case 3n:
      Step.after(Claude.result.if(Hub.running(Hub.thread(s, thread)), thread, now, o), Usage.result(s, thread, now, o))
    case 5n:
      Step{Lim.claude(s, now, o), Nil{}}
    case 4n:
      Step.also(Step.also(Claude.named(s, thread, now, o), Steps.claude(J.Json.elems(J.Json.get_obj(J.Json.get_obj(o, "message"), "content")), thread, Hub.cwd(s, Hub.thread(s, thread)))),
        Tools.shuts(J.Json.elems(J.Json.get_obj(J.Json.get_obj(o, "message"), "content")), thread))
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
      Claude.line.who(String.is_empty(call), C.Codec.pos(["system", "stream_event", "assistant", "result", "user", "rate_limit_event"], J.Json.get_str(o, "type", "")), s, thread, now, call, o)
    case None{}:
      Step.none()

# first: the thread's first worktree (not one made again), whose waiting
# message names its branch
def Wt.start(+s: M.State, +now: Nat, +thread: String, +first: Bool) -> Step:
  Wt.start.k(Bool.and(Bool.not(Hub.running(Hub.thread(s, thread))), M.Queue.waiting(s, thread)), first,
    Hub.thread(s, thread), Hub.cwd(s, Hub.thread(s, thread)), M.Queue.of(s, thread), now, Hub.seq(s), thread)

# the record that the hub asked for it, so a message while it is being made
# waits for it rather than asking twice (model.bend's Worktree.waits)
def Wt.asked(+th: String, +now: Nat) -> List<&2, M.Change>:
  [M.SettingSet{M.Worktree.again.key(th), Nat.show(now), now}]

# the record cleared once the worktree came, could not be made, or went
def Wt.answered(+s: M.State, +th: String, +now: Nat) -> List<&2, M.Change>:
  Bool.pick(List<&2, M.Change>, Nat.is_eq(M.Worktree.again.at(s, th), 0n), Nil{}, [M.SettingSet{M.Worktree.again.key(th), "", now}])

# the laws' example: a worktree thread whose turn completed at 5 and whose
# worktree was removed at 6 (its branch b1 kept), with more of its log after
def Wt.demo(cs: List<&2, M.Change>) -> M.State:
  M.State.replay(List.append(&2, M.Change, [M.ProjectCreated{"p1", "p", "/p", 1n}, M.ThreadCreated{"t1", "p1", "x", M.Worktree{}, "", "", "claude", "", 1n},
    M.WorktreeSet{"t1", "b1", "/p/.backplane/worktrees/b1", 2n}, M.MessagePosted{"t1", "m1", M.User{}, "go", 3n}, M.TurnChanged{"t1", M.Running{}, 3n},
    M.TurnChanged{"t1", M.Completed{}, 5n}, M.WorktreeRemoved{"t1", 6n}], cs), M.State.empty())

def Wt.ready(+s: M.State, +now: Nat, +client: U32, +id: String, +thread: String, +c: M.Change) -> Step:
  Step.before(c <> Wt.answered(s, thread, now), [Reply.ok(client, id)], Wt.start(M.State.replay([c], s), now, thread, String.is_empty(M.Merge.branch(Hub.thread(s, thread)))))

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
      Step.reply(client, id, M.WorktreeRemoved{J.Json.get_str(p, "thread", ""), now} <> Wt.answered(s, J.Json.get_str(p, "thread", ""), now))
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
    case 10n:
      Step.outs([Git{client, id, "kexport", J.Json.get_str(p, "thread", ""), Hub.cwd(s, Hub.thread(s, J.Json.get_str(p, "thread", ""))), J.Json.show(p), ""}])
    # the board picker opened: the project's boards read again, {thread}
    # (core/order.bend's Boards.list; server.bend's BoardsJob)
    case 11n:
      Step.outs([Git{client, id, "boards", J.Json.get_str(p, "thread", ""), Hub.cwd(s, Hub.thread(s, J.Json.get_str(p, "thread", ""))), "", ""}])
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

def Steer.stop(+provider: String, +thread: String) -> List<&2, Out>:
  Bool.pick(List<&2, Out>, String.eq(provider, "claude"), Nil{}, [AgentStop{thread}])

def Steer.note(+provider: String, +thread: String, +now: Nat, +seq: Nat) -> List<&2, M.Change>:
  Bool.pick(List<&2, M.Change>, String.eq(provider, "claude"), Nil{},
    [M.ActivityLogged{thread, Hub.id("s", now, seq), "info", "steer", "Steered: the turn starts again with this message", now}])

# the running turn takes `text` now (the message itself is already posted)
def Steer.go(+t: M.Thread, +cwd: String, +now: Nat, +seq: Nat, +thread: String, +text: String) -> Step:
  Step{List.append(&2, M.Change, Steer.note(Hub.provider(t), thread, now, seq), [M.TurnChanged{thread, M.Running{}, now}]),
    List.append(&2, Out, Steer.stop(Hub.provider(t), thread), Hub.run(t, cwd, text, Hub.uuid(now, thread ++ "/s/" ++ Nat.show(seq))))}

# Riding mail
# -----------
# (ride.bend) A running Codex, Grok, Muse or Antigravity turn takes a
# message on its next Backplane tool result (Ride.taken, bothub.bend's
# Ride.mcp); a turn that ends with mail waiting starts the next at once
# with it (Ride.after); a turn that starts for any other reason takes it
# too (Ride.post). Laws ride_*.

def Ride.of(+s: M.State, +th: String) -> String:
  M.Setting.get(s, RD.Ride.key(th), "")

def Ride.last(+v: String) -> String:
  Maybe.default(&2, String, List.last(&2, String, RD.Ride.ids(RD.Ride.read(v))), "")

def Ride.count(+v: String) -> String:
  Nat.show(List.length(&2, RD.RItem, RD.Ride.read(v)))

# a message waits for the running turn: posted now, added to the ride
def Ride.wait(+s: M.State, +now: Nat, +thread: String, +msg: String, role: M.Role, +text: String) -> Step:
  Step{[M.MessagePosted{thread, msg, role, text, now}, M.SettingSet{RD.Ride.key(thread), RD.Ride.add(Ride.of(s, thread), msg, text), now}], Nil{}}

# the ride emptied, and the log told how much went where
def Ride.done(+v: String, +now: Nat, +thread: String, +into: String) -> List<&2, M.Change>:
  [M.SettingSet{RD.Ride.key(thread), "", now}, M.ActivityLogged{thread, "ride:" ++ Ride.last(v), "info", "ride", "Delivered " ++ into ++ ": " ++ Ride.count(v) ++ " waiting message(s)", now}]

# what a tool result that carries the ride writes
def Ride.taken(+s: M.State, +now: Nat, +thread: String) -> List<&2, M.Change>:
  Ride.done(Ride.of(s, thread), now, thread, "into the running turn")

# a turn started with the ride (after the legacy queue, if any)
def Ride.run(+t: M.Thread, +cwd: String, +queued: String, +v: String, +now: Nat, +thread: String) -> Step:
  Step{List.concat(&2, M.Change, [Ride.done(v, now, thread, "with a new turn"), Bool.pick(List<&2, M.Change>, String.is_empty(queued), Nil{}, [M.QueueTaken{thread, now}]), [M.TurnChanged{thread, M.Running{}, now}]]),
    Git{0, "", "checkpoint0", thread, cwd, "", ""} <> Git{0, "", "step", thread, cwd, Ride.last(v), "start"} <> Hub.run(t, cwd, M.Queue.join(queued, RD.Ride.input(v)), Hub.uuid(now, thread ++ "/r/" ++ Ride.last(v)))}

def Ride.start.k(empty: Bool, +s: M.State, +now: Nat, +th: String) -> Step:
  match empty:
    case True{}:
      Step.none()
    case False{}:
      Ride.run(Hub.thread(s, th), Hub.cwd(s, Hub.thread(s, th)), M.Queue.of(s, th), Ride.of(s, th), now, th)

# a thread that is not running and has mail waiting starts a turn with it
def Ride.start(+s: M.State, +now: Nat, +th: String) -> Step:
  Ride.start.k(Bool.or(Hub.running(Hub.thread(s, th)), String.is_empty(Ride.of(s, th))), s, now, th)

# Delivery
# --------
# A message for a thread, from the person, a bot, another thread or
# Backplane itself, is delivered at once; it never waits behind a running
# turn (law deliver_never_waits). An idle thread starts a turn with it. A
# running turn takes it: Claude on its input, mid-turn; a Codex, Grok,
# Muse or Antigravity turn on its next Backplane tool result (Riding mail,
# above), or, when the person steers, its process stops and the turn starts
# again on the same session with it (Steer.go, law steer_codex_restarts).
# Only a thread whose worktree is still being made holds it, and starts
# with it as soon as the worktree is there (Wt.start). Whatever the thread
# held (handed-over context, or messages from logs older than this rule)
# goes with it.

# a running Codex, Grok, Muse or Antigravity turn stopped and started
# again with the message (and whatever waited: the legacy queue, the ride)
def Hub.deliver.restart(+s: M.State, +now: Nat, +thread: String, +msg: String, role: M.Role, +text: String) -> Step:
  Step.before(M.MessagePosted{thread, msg, role, text, now} <> List.append(&2, M.Change, Bool.pick(List<&2, M.Change>, String.is_empty(M.Queue.of(s, thread)), Nil{}, [M.QueueTaken{thread, now}]),
      Bool.pick(List<&2, M.Change>, String.is_empty(Ride.of(s, thread)), Nil{}, Ride.done(Ride.of(s, thread), now, thread, "with the steer"))), Nil{},
    Steer.go(Hub.thread(s, thread), Hub.cwd(s, Hub.thread(s, thread)), now, Hub.seq(s), thread, M.Queue.join(M.Queue.join(M.Queue.of(s, thread), RD.Ride.input(Ride.of(s, thread))), text)))

# Automatic delivery never restarts a running turn. An older log's queue
# remains owned and joins the ride at the next natural turn boundary.
def Hub.deliver.ride(clear: Bool, +s: M.State, +now: Nat, +thread: String, +msg: String, role: M.Role, +text: String) -> Step:
  match clear:
    case True{}:
      Ride.wait(s, now, thread, msg, role, text)
    case False{}:
      Ride.wait(s, now, thread, msg, role, text)

def Hub.deliver.steer(claude: Bool, +s: M.State, +now: Nat, +thread: String, +msg: String, role: M.Role, +text: String) -> Step:
  match claude:
    case True{}:
      Hub.post.go(False{}, Hub.thread(s, thread), Hub.cwd(s, Hub.thread(s, thread)), M.Queue.of(s, thread), now, thread, msg, role, text)
    case False{}:
      Hub.deliver.ride(String.is_empty(M.Queue.of(s, thread)), s, now, thread, msg, role, text)

# an idle thread starts a turn with the message, and with any mail still
# waiting (a turn stopped by the person, or one a restart cut short)
def Ride.post(empty: Bool, +s: M.State, +now: Nat, +thread: String, +msg: String, role: M.Role, +text: String) -> Step:
  match empty:
    case True{}:
      Hub.post.go(False{}, Hub.thread(s, thread), Hub.cwd(s, Hub.thread(s, thread)), M.Queue.of(s, thread), now, thread, msg, role, text)
    case False{}:
      Step.before(Ride.done(Ride.of(s, thread), now, thread, "with a new turn"), Nil{},
        Hub.post.go(False{}, Hub.thread(s, thread), Hub.cwd(s, Hub.thread(s, thread)), M.Queue.join(M.Queue.of(s, thread), RD.Ride.input(Ride.of(s, thread))), now, thread, msg, role, text))

def Hub.deliver.r(run: Bool, +s: M.State, +now: Nat, +thread: String, +msg: String, role: M.Role, +text: String) -> Step:
  match run:
    case True{}:
      Hub.deliver.steer(String.eq(Hub.provider(Hub.thread(s, thread)), "claude"), s, now, thread, msg, role, text)
    case False{}:
      Ride.post(String.is_empty(Ride.of(s, thread)), s, now, thread, msg, role, text)

# running: the thread's turn runs; pending: its worktree is being made
def Hub.deliver(run: Bool, pending: Bool, +s: M.State, +now: Nat, +thread: String, +msg: String, role: M.Role, +text: String) -> Step:
  match pending:
    case True{}:
      Hub.post.go(True{}, Hub.thread(s, thread), Hub.cwd(s, Hub.thread(s, thread)), M.Queue.of(s, thread), now, thread, msg, role, text)
    case False{}:
      Hub.deliver.r(run, s, now, thread, msg, role, text)

# whether changes hold a message back for later (a TurnQueued that is not
# handed-over context); law deliver_never_waits
def Deliver.held.one(c: M.Change) -> Bool:
  match c:
    case M.TurnQueued{_, +m, _, _}:
      Bool.not(String.starts_with(m, "handoff:"))
    case _:
      False{}

def Deliver.held(cs: List<&2, M.Change>) -> Bool:
  match cs:
    case Nil{}:
      False{}
    case c <> t:
      Bool.or(Deliver.held.one(c), Deliver.held(t))

# a message for a thread's agent, delivered (Hub.deliver); a thread whose
# worktree was removed has it made again first
def Hub.thread_project(t: M.Thread) -> String:
  match t:
    case M.Thread{_, p, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _}:
      p

# the worktree made again from a thread's kept branch
def Wt.revive(+s: M.State, +th: String) -> Out:
  Git{0, "", "worktree", th, Hub.project.root(Hub.project(s, Hub.thread_project(Hub.thread(s, th)))), "again", M.Merge.branch(Hub.thread(s, th))}

# whether a message makes a thread's removed worktree again first: the
# thread is idle, its worktree is gone, and it is not being made already
# (pending: model.bend's Worktree.waits, a fresh ask of the hub's, never the
# thread's last activity)
def Hub.revive(+running: Bool, gone: Bool, +pending: Bool) -> Bool:
  Bool.and(Bool.not(running), Bool.and(gone, Bool.not(pending)))

def Hub.post.v(revive: Bool, +running: Bool, +pending: Bool, +s: M.State, +now: Nat, +thread: String, +msg: String, role: M.Role, +text: String) -> Step:
  match revive:
    case True{}:
      Step.before(Wt.asked(thread, now), [Wt.revive(s, thread)], Hub.post.go(True{}, Hub.thread(s, thread), Hub.cwd(s, Hub.thread(s, thread)), M.Queue.of(s, thread), now, thread, msg, role, text))
    case False{}:
      Hub.deliver(running, pending, s, now, thread, msg, role, text)

def Hub.post.w(+running: Bool, gone: Bool, +pending: Bool, +s: M.State, +now: Nat, +thread: String, +msg: String, role: M.Role, +text: String) -> Step:
  Hub.post.v(Hub.revive(running, gone, pending), running, pending, s, now, thread, msg, role, text)

def Hub.post(+s: M.State, +now: Nat, +thread: String, +msg: String, role: M.Role, +text: String) -> Step:
  Hub.post.w(Hub.running(Hub.thread(s, thread)), M.Worktree.gone(Hub.thread(s, thread)), M.Worktree.waits(s, Hub.thread(s, thread), now), s, now, thread, msg, role, text)

# a message from another thread (thread_send, a delegated task's result,
# a launched thread's report, a wake): delivered like any other
# (Hub.deliver); its id names the sender, which is what the link visuals
# read
def Hub.inbox.v(revive: Bool, +running: Bool, +pending: Bool, +s: M.State, +now: Nat, +from: String, +to: String, role: M.Role, +text: String, +msg: String) -> Step:
  match revive:
    case True{}:
      Step.before(Wt.asked(to, now), [Wt.revive(s, to)], Hub.post.go(True{}, Hub.thread(s, to), Hub.cwd(s, Hub.thread(s, to)), M.Queue.of(s, to), now, to,
        msg, role, text))
    case False{}:
      Hub.deliver(running, pending, s, now, to, msg, role, text)

def Hub.inbox.w(+running: Bool, gone: Bool, +pending: Bool, +s: M.State, +now: Nat, +from: String, +to: String, role: M.Role, +text: String, +msg: String) -> Step:
  Hub.inbox.v(Hub.revive(running, gone, pending), running, pending, s, now, from, to, role, text, msg)

def Origin.one(yes: Bool, thread: String, msg: String, o: ML.Origin, at: Nat) -> List<&2, M.Change>:
  match yes o:
    case False{} _: Nil{}
    case True{} ML.Origin{from, kind, ref}: [M.MessageFrom{thread, msg, from, kind, ref, at}]

# the same under a message id of the caller's (a delegated task's result:
# Task.finish.go, so a delivery torn by a crash is found again, Task.repair)
# Add provenance directly after the existing MessagePosted without moving
# any delivery changes or effects (including the running-turn steer path).
def Origin.changes(cs: List<&2, M.Change>, +thread: String, +msg: String, +o: ML.Origin, +at: Nat) -> List<&2, M.Change>:
  match cs:
    case Nil{}: Nil{}
    case M.MessagePosted{+th, +id, role, text, when} <> rest:
      M.MessagePosted{th, id, role, text, when} <> List.append(&2, M.Change,
        Origin.one(Bool.and(String.eq(th, thread), String.eq(id, msg)), thread, msg, o, at), Origin.changes(rest, thread, msg, o, at))
    case h <> rest: h <> Origin.changes(rest, thread, msg, o, at)

def Origin.step(st: Step, thread: String, msg: String, o: ML.Origin, at: Nat) -> Step:
  match st:
    case Step{cs, os}: Step{Origin.changes(cs, thread, msg, o, at), os}

def Origin.system(role: M.Role) -> Bool:
  match role:
    case M.System{}: True{}
    case _: False{}

# ---- mail 2b ----
# Body IO is deliberately an Out: no mail event is committed before it.
def Mail.addr.got(b: Maybe<&2, BT.Bot>, +thread: String) -> String:
  match b:
    case Some{+bot}: REF.Ref.addr(REF.Ref.bot(BT.Bot.id(bot)))
    case None{}: REF.Ref.addr(REF.Ref.thread(thread))

def Mail.addr(+s: M.State, +thread: String) -> String:
  Mail.addr.got(BT.World.of_thread(M.State.bots(s), thread), thread)

def Mail.name.got(r: Maybe<&2, REF.Ref>, +s: M.State, +addr: String) -> String:
  match r:
    case Some{REF.Ref{"bot", id <> _, _}}: BT.Bot.name(BT.World.bot(M.State.bots(s), id))
    case Some{REF.Ref{"thread", id <> _, _}}: Hub.thread_title(Hub.thread(s, id))
    case Some{REF.Ref{"person", _, _}}: "Person"
    case _: addr

def Mail.name(+s: M.State, +addr: String) -> String:
  Mail.name.got(REF.Ref.of_addr(addr), s, addr)

def Mail.subject(+launch: Bool, +s: M.State, +o: ML.Origin, +to: String) -> String:
  match launch:
    case True{}: "Launch: " ++ Mail.name(s, ML.Origin.from(o))
    case False{}: Mail.name(s, ML.Origin.from(o)) ++ " ↔ " ++ Mail.name(s, to)

def Mail.project.got(r: Maybe<&2, REF.Ref>, +s: M.State) -> String:
  match r:
    case Some{REF.Ref{"thread", id <> _, _}}: Hub.thread_project(Hub.thread(s, id))
    case Some{REF.Ref{"bot", id <> _, _}}: Hub.thread_project(Hub.thread(s, BT.Bot.thread(BT.World.bot(M.State.bots(s), id))))
    case _: ""

def Mail.flags(+o: ML.Origin, +text: String) -> ML.Flags:
  ML.Flags{Bool.pick(String, String.contains(String.take(text, 240n), "completed"), "completed", ""),
    ML.Parts.has(["task", "wake", "deploy", "lock"], ML.Origin.kind(o), False{}),
    ML.Parts.has(["thread", "bot", "person", "peer"], ML.Origin.kind(o), False{}), False{}}

# UTF-8 bytes, not Unicode scalar count.
def Mail.bytes(+text: String, +acc: Nat) -> Nat:
  match text:
    case SNil{}: acc
    case SCon{Chr{+u}, +t}: Mail.bytes(t, Nat.add(acc, Bool.pick(Nat, U32.is_lt(u, 128), 1n, Bool.pick(Nat, U32.is_lt(u, 2048), 2n, Bool.pick(Nat, U32.is_lt(u, 65536), 3n, 4n)))))

def Mail.metadata.new(+missing: Bool, +c: ML.Conv) -> List<&2, M.Change>:
  match missing:
    case True{}: [MB.Body.metadata(c)]
    case False{}: Nil{}

def Mail.pending(+missing: Bool, +body: MB.Body) -> List<&2, M.Change>:
  List.append(&2, M.Change, Mail.metadata.new(missing, MB.Body.conv(body)), [MB.Body.header(MB.Body.mail(body))])

def Mail.file.body(+s: M.State, +body: MB.Body) -> Step:
  Step{Nil{}, [MailAppend{MB.Body.line(body), ML.Mail.conv(MB.Body.mail(body)), Mail.pending(String.is_empty(ML.Conv.id(ML.Box.conv(M.State.mail(s), ML.Mail.conv(MB.Body.mail(body))))), body)}]}

def Mail.sender.empty(+person: Bool, +s: M.State, +from: String) -> String:
  match person:
    case True{}: REF.Ref.addr(REF.Ref.person())
    case False{}: Mail.addr(s, from)

def Mail.sender(+s: M.State, +from: String) -> String:
  Mail.sender.empty(String.is_empty(from), s, from)

def Mail.strings(+xs: List<&2, String>) -> List<&2, J.Json>:
  match xs:
    case Nil{}: Nil{}
    case +h <> +t: J.Str{h} <> Mail.strings(t)

def Mail.row(+box: ML.Box, +bot: String, +c: ML.Conv) -> J.Json:
  +mark = ML.Box.mark_of(box, bot, ML.Conv.id(c))
  +last = ML.Box.newest(box, ML.Conv.id(c))
  J.Json.obj([J.KV{"id", J.Str{ML.Conv.id(c)}}, J.KV{"subject", J.Str{ML.Conv.subject(c)}}, J.KV{"kind", J.Str{ML.Conv.kind(c)}},
    J.KV{"parts", J.Json.arr(Mail.strings(ML.Conv.parts(c)))}, J.KV{"project", J.Str{ML.Conv.project(c)}}, J.KV{"tags", J.Json.arr(Mail.strings(List.append(&2, String, ML.Conv.tags(c), ML.Mark.tags(mark))))},
    J.KV{"last", J.Json.nat(ML.Conv.last(c))}, J.KV{"n", J.Json.nat(ML.Conv.n(c))}, J.KV{"status", J.Str{ML.Mark.status(mark)}},
    J.KV{"unread", J.Flag{String.eq(ML.Mark.status(mark), "unread")}}, J.KV{"head", J.Str{ML.Mail.header(last)}}, J.KV{"from", J.Str{ML.Mail.from(last)}}])

def Mail.rows(+cs: List<&2, ML.Conv>, +box: ML.Box, +bot: String) -> List<&2, J.Json>:
  match cs:
    case Nil{}: Nil{}
    case +c <> +t: Mail.row(box, bot, c) <> Mail.rows(t, box, bot)

def Mail.limit(+n: Nat) -> Nat:
  Bool.pick(Nat, Nat.is_gt(n, 200n), 200n, n)

def Mail.list(+s: M.State, +p: J.Json) -> J.Json:
  +box = M.State.mail(s)
  +bot = J.Json.get_str(p, "bot", "*")
  +status = J.Json.get_str(p, "status", "")
  +project = J.Json.get_str(p, "project", "")
  J.Json.obj([J.KV{"convs", J.Json.arr(Mail.rows(ML.Box.list(box, bot, status, project, J.Json.get_nat(p, "skip", 0n), Mail.limit(J.Json.get_nat(p, "n", 50n))), box, bot))},
    J.KV{"total", J.Json.nat(ML.Box.count(box, bot, status, project))}])

def Mail.find(+s: M.State, +p: J.Json) -> J.Json:
  +box = M.State.mail(s)
  +bot = J.Json.get_str(p, "bot", "*")
  +cs = ML.Box.find(box, bot, J.Json.get_str(p, "q", ""), Mail.limit(J.Json.get_nat(p, "n", 50n)))
  J.Json.obj([J.KV{"convs", J.Json.arr(Mail.rows(cs, box, bot))}, J.KV{"total", J.Json.nat(List.length(&2, ML.Conv, cs))}])

def Mail.mark.valid(+status: String) -> Bool:
  ML.Parts.has(["unread", "open", "snoozed", "done"], status, False{})

def Mail.mark(+s: M.State, +now: Nat, +p: J.Json) -> List<&2, M.Change>:
  +bot = J.Json.get_str(p, "bot", "")
  +conv = J.Json.get_str(p, "conv", "")
  [M.MailMarked{bot, conv, J.Json.get_str(p, "status", "open"), ML.Conv.last(ML.Box.conv(M.State.mail(s), conv)), J.Json.get_nat(p, "until", 0n), now}]

def Mail.array.strings(+xs: List<&2, J.Json>) -> List<&2, String>:
  match xs:
    case Nil{}: Nil{}
    case J.Str{+h} <> +t: h <> Mail.array.strings(t)
    case _ <> +t: Mail.array.strings(t)

def Mail.tag(+now: Nat, +p: J.Json) -> List<&2, M.Change>:
  [M.MailTagged{J.Json.get_str(p, "bot", ""), J.Json.get_str(p, "conv", ""), String.join(Mail.array.strings(J.Json.elems(J.Json.get_obj(p, "tags"))), "\n"), now}]

def Mail.due.changes(+ms: List<&2, ML.Mark>, +now: Nat) -> List<&2, M.Change>:
  match ms:
    case Nil{}: Nil{}
    case +m <> +t: M.MailMarked{ML.Mark.bot(m), ML.Mark.conv(m), "unread", ML.Mark.seen(m), 0n, now} <> Mail.due.changes(t, now)

def Mail.due.on(+on: Bool, +s: M.State, +now: Nat) -> List<&2, M.Change>:
  match on:
    case False{}: Nil{}
    case True{}: Mail.due.changes(ML.Box.due(M.State.mail(s), now), now)

def Mail.due(+s: M.State, +now: Nat) -> List<&2, M.Change>:
  Mail.due.on(String.eq(M.Setting.get(s, "mail.on", "off"), "on"), s, now)

def Mail.seen(+given: Bool, +s: M.State, +now: Nat, +p: J.Json) -> List<&2, M.Change>:
  match given:
    case False{}: Nil{}
    case True{}: Mail.mark(s, now, J.Json.obj([J.KV{"bot", J.Str{J.Json.get_str(p, "bot", "")}}, J.KV{"conv", J.Str{J.Json.get_str(p, "conv", "")}}, J.KV{"status", J.Str{"open"}}]))

def Mail.read(+s: M.State, +now: Nat, +client: U32, +id: String, +p: J.Json) -> Step:
  Step{Mail.seen(Bool.not(String.is_empty(J.Json.get_str(p, "bot", ""))), s, now, p),
    [MailRead{client, id, J.Json.get_str(p, "conv", ""), J.Json.get_str(p, "before", ""), Mail.limit(J.Json.get_nat(p, "n", 50n))}]}

def Mail.body.row(+body: MB.Body) -> J.Json:
  +m = MB.Body.mail(body)
  J.Json.obj([J.KV{"id", J.Str{ML.Mail.id(m)}}, J.KV{"from", J.Str{ML.Mail.from(m)}}, J.KV{"to", J.Str{ML.Mail.to(m)}},
    J.KV{"kind", J.Str{ML.Mail.kind(m)}}, J.KV{"want", J.Str{ML.Mail.wanted(m)}}, J.KV{"at", J.Json.nat(ML.Mail.at(m))}, J.KV{"text", J.Str{MB.Body.text(body)}}])

def Mail.body.rows(+bs: List<&2, MB.Body>) -> List<&2, J.Json>:
  match bs:
    case Nil{}: Nil{}
    case +h <> +t: Mail.body.row(h) <> Mail.body.rows(t)

def Mail.read.json(+s: M.State, +conv: String, +bot: String, +bs: List<&2, MB.Body>, +more: Bool) -> J.Json:
  J.Json.obj([J.KV{"conv", Mail.row(M.State.mail(s), bot, ML.Box.conv(M.State.mail(s), conv))}, J.KV{"mails", J.Json.arr(Mail.body.rows(bs))}, J.KV{"more", J.Flag{more}}])

# Used by both RPCs and the MCP primitive. Person messages are ordinary
# User messages; agent messages retain provenance. Only filing is gated.
def Mail.target.got(r: Maybe<&2, REF.Ref>, +s: M.State) -> String:
  match r:
    case Some{REF.Ref{"thread", th <> _, _}}: th
    case Some{REF.Ref{"bot", id <> _, _}}: BT.Bot.thread(BT.World.bot(M.State.bots(s), id))
    case _: ""

def Mail.target(+s: M.State, +to: String) -> String:
  Mail.target.got(REF.Ref.of_addr(to), s)

def Mail.or(+s: String, +fallback: String) -> String:
  Bool.pick(String, String.is_empty(s), fallback, s)

def Mail.project(+s: M.State, +o: ML.Origin, +to: String) -> String:
  Mail.or(Mail.project.got(REF.Ref.of_addr(ML.Origin.from(o)), s), Mail.project.got(REF.Ref.of_addr(to), s))

def Mail.conv.new(+missing: Bool, +old: ML.Conv, +s: M.State, +now: Nat, +o: ML.Origin, +to: String, +conv: String, +subject: String) -> ML.Conv:
  match missing:
    case False{}: old
    case True{}: ML.Conv{conv, subject, ML.Conv.mail_kind(conv, ML.Origin.kind(o)), [ML.Origin.from(o), to], Mail.project(s, o, to), Nil{}, now, now, 0n}

def Mail.conv(+s: M.State, +now: Nat, +o: ML.Origin, +to: String, +conv: String, +subject: String) -> ML.Conv:
  +old = ML.Box.conv(M.State.mail(s), conv)
  Mail.conv.new(String.is_empty(ML.Conv.id(old)), old, s, now, o, to, conv, subject)

def Mail.body(+s: M.State, +now: Nat, +o: ML.Origin, +to: String, +text: String, +msg: String, +conv: String, +subject: String, +given: String) -> MB.Body:
  MB.Body{Mail.conv(s, now, o, to, conv, subject), ML.Mail{msg, conv, ML.Origin.from(o), to, ML.Origin.kind(o), ML.Origin.ref(o),
    ML.Mail.want(ML.Origin.kind(o), given, Mail.flags(o, text)), ML.Mail.head(text), Mail.bytes(text, 0n), now}, text}

def Mail.file(+s: M.State, +now: Nat, +o: ML.Origin, +to: String, +text: String, +msg: String) -> Step:
  Mail.file.body(s, Mail.body(s, now, o, to, text, msg, ML.Mail.route(o, to), Mail.subject(String.eq(ML.Origin.kind(o), "launch"), s, o, to), ""))

# Operational reports to the uberbot
# ----------------------------------
# A launched thread's report (Lr.send.here), a delegated task's result,
# watched work's outcome, a wake addressed to the uberbot's own thread and
# another thread's thread_send to it (mcp.bend's Mcp.send.m) are filed in
# its Mailroom, never posted in its Chat: that is the person's, their
# messages and the uberbot's answers. The
# producer chooses this path by what it is, never by the text. Filing is
# the durable MailAppend: the body is written, then its header and the
# producer's own acknowledgements (TaskDelivered...) commit together, so a
# report is never marked delivered before it is filed; with the header
# already there (recovered after a crash) only the acknowledgements commit.
# Filing commits before routing. The promoted owner's FYI and act mail
# both enter private triage, including with generic automation off. Other
# threads retain their existing delivery policy.
def Operational.owner(+s: M.State, +to: String) -> String:
  Bool.pick(String, UBS.Ub.chat.is(s, to), Mail.or(UBS.Ub.chat(s), to), to)

def Operational.is(+s: M.State, +to: String) -> Bool:
  Bool.and(UBS.Ub.chat.is(s, to), Bool.not(FR.Fr.is(Operational.owner(s, to))))

# the producer's acknowledgements, committed with the header
def Operational.after(st: Step, +after: List<&2, M.Change>) -> Step:
  match st:
    case Step{cs, MailAppend{l, c, ch} <> Nil{}}: Step{cs, [MailAppend{l, c, List.append(&2, M.Change, ch, after)}]}
    case Step{cs, os}: Step{List.append(&2, M.Change, cs, after), os}

def Operational.file.k(has: Bool, +s: M.State, +body: MB.Body, +after: List<&2, M.Change>) -> Step:
  match has:
    case True{}: Step{after, Nil{}}
    case False{}: Operational.after(Mail.file.body(s, body), after)

# want: "fyi", or "" for the kind's own (a thread's direct message asks)
def Operational.mail(+s: M.State, +now: Nat, +o: ML.Origin, +to: String, +text: String, +msg: String, +want: String, +after: List<&2, M.Change>) -> Step:
  +addr = Mail.addr(s, Operational.owner(s, to))
  Operational.file.k(ML.Box.has(M.State.mail(s), msg), s,
    Mail.body(s, now, o, addr, text, msg, ML.Mail.route(o, addr), Mail.subject(String.eq(ML.Origin.kind(o), "launch"), s, o, addr), want), after)

def Operational.file(+s: M.State, +now: Nat, +o: ML.Origin, +to: String, +text: String, +msg: String, +after: List<&2, M.Change>) -> Step:
  Operational.mail(s, now, o, to, text, msg, "fyi", after)

def Mail.compose(+s: M.State, +now: Nat, +from: String, +to: String, +text: String, +msg: String, +conv: String, +subject: String, +want: String) -> Step:
  +o = {ML.Origin{from, "thread", ""} : ML.Origin}
  +cid = Mail.or(conv, ML.Mail.route(o, to))
  Mail.file.body(s, Mail.body(s, now, o, to, text, msg, cid, Mail.or(subject, Mail.subject(False{}, s, o, to)), want))

def Mail.request.mark(+valid: Bool, +s: M.State, +now: Nat, +client: U32, +id: String, +p: J.Json) -> Step:
  match valid:
    case True{}: Step.reply(client, id, Mail.mark(s, now, p))
    case False{}: Step.outs([Reply.err(client, id, "invalid mail status")])

# The delivery kernel keeps the exact Step promised by
# inbox_running_delivers. Production inbound funnels add metadata around it.
def Hub.inbox.id(+s: M.State, +now: Nat, +from: String, +to: String, role: M.Role, +text: String, +msg: String) -> Step:
  Hub.inbox.w(Hub.running(Hub.thread(s, to)), M.Worktree.gone(Hub.thread(s, to)), M.Worktree.waits(s, Hub.thread(s, to), now), s, now, from, to, role, text, msg)

def Hub.answer.plan(impl: Bool, +s: M.State, +now: Nat, +client: U32, +id: String, +ask: String, +answer: String, +th: String) -> Step:
  match impl:
    case True{}:
      Step.before(M.AskClosed{ask, answer, now} <> Hub.implement.k(Hub.thread(s, th), now), [Reply.ok(client, id)],
        Hub.post(Hub.impl.state(s, th, now), now, th, Hub.id("m", now, Hub.seq(s)), M.User{}, "Implement the proposed plan."))
    case False{}:
      Step{[M.AskClosed{ask, answer, now}], [Reply.ok(client, id), AskAnswered{ask, answer}]}

# an order's question (order_ask, core/order.bend) parks nothing: the
# person's answer is posted in its thread as their message, which starts
# (or queues) the agent's turn however long they took; a dismissal only
# closes it (laws order_ask_*)
# a buy thread's question dismissed: said in the thread as the person's
# message, so its agent hears it (silence left the thread waiting)
def Order.skipped() -> String:
  "I skipped this question."

def Hub.answer.order(said: Bool, +s: M.State, +now: Nat, +client: U32, +id: String, +ask: String, +answer: String, +th: String) -> Step:
  match said:
    case True{}:
      Step.before([M.AskClosed{ask, answer, now}], [Reply.ok(client, id)], Hub.post(s, now, th, Hub.id("m", now, Hub.seq(s)), M.User{}, String.trim(answer)))
    case False{}:
      Step.before([M.AskClosed{ask, answer, now}], [Reply.ok(client, id)], Hub.post(s, now, th, Hub.id("m", now, Hub.seq(s)), M.User{}, Order.skipped()))

def Hub.answer.open(order: Bool, +s: M.State, +now: Nat, +client: U32, +id: String, +ask: String, +answer: String, +th: String, +k: String) -> Step:
  match order:
    case True{}:
      Hub.answer.order(Bool.not(String.is_empty(String.trim(answer))), s, now, client, id, ask, answer, th)
    case False{}:
      Hub.answer.plan(Bool.and(String.eq(k, "plan"), String.eq(answer, "implement")), s, now, client, id, ask, answer, th)

# the user's answer: the ask closes (once; law ask_closes_once), a parked
# agent request gets its reply, implementing a plan leaves plan mode and
# asks the agent to carry it out, and an order's question is answered in
# its thread (Hub.answer.order)
# A project's own checks wait for the person's OK (server/review.bend's
# Rvc.trusted): the hub asks in the thread (an "input" ask, tool
# "checks_ok", id "checks:<sha256>:<n>", server.bend's Hub.checks.ask);
# "Run them" trusts that checks.json, anything else only closes the ask.
def Checks.yes() -> String:
  "Run them"

def Checks.hash(+ask: String) -> String:
  String.take(String.drop(ask, 7n), 64n)

def Checks.answer.k(yes: Bool, +now: Nat, +client: U32, +id: String, +ask: String, +answer: String) -> Step:
  match yes:
    case True{}:
      Step{[M.AskClosed{ask, answer, now}], [Reply.ok(client, id), ChecksTrust{Checks.hash(ask)}]}
    case False{}:
      Step{[M.AskClosed{ask, answer, now}], [Reply.ok(client, id)]}

def Checks.asked.go(as: List<&2, M.Ask>, +pre: String) -> Bool:
  match as:
    case Nil{}:
      False{}
    case M.Ask{+aid, _, _, _, _, _, +op, _} <> t:
      Bool.or(Bool.and(op, String.starts_with(aid, pre)), Checks.asked.go(t, pre))

# a question for these checks is open already
def Checks.asked(+s: M.State, +hash: String) -> Bool:
  Checks.asked.go(M.Asks.all(s), "checks:" ++ hash ++ ":")

def Hub.answer.cx.t(cx: Bool, order: Bool, +s: M.State, +now: Nat, +client: U32, +id: String, +ask: String, +answer: String, +th: String, +k: String) -> Step:
  match cx:
    case True{}:
      Checks.answer.k(String.eq(String.trim(answer), Checks.yes()), now, client, id, ask, answer)
    case False{}:
      Hub.answer.open(order, s, now, client, id, ask, answer, th, k)

def Hub.answer.k(a: M.Ask, +s: M.State, +now: Nat, +client: U32, +id: String, +ask: String, +answer: String) -> Step:
  match a:
    case M.Ask{_, +th, +k, +tool, _, _, True{}, _}:
      Hub.answer.cx.t(String.eq(tool, "checks_ok"), String.eq(tool, "order_ask"), s, now, client, id, ask, answer, th, k)
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

# Follow-up modes
# ---------------
# Clients and tools name a mode ("queue", "steer", "auto"). Every mode
# delivers the message at once (Hub.deliver): there is no queue. "steer"
# (what the person's send is) restarts a running non-Claude turn with it.

def Hub.post.steer.k(go: Bool, +s: M.State, +now: Nat, +thread: String, +msg: String, role: M.Role, +text: String) -> Step:
  match go:
    case True{}:
      Hub.deliver.restart(s, now, thread, msg, role, text)
    case False{}:
      Hub.post(s, now, thread, msg, role, text)

# the person's explicit steer: a running Codex, Grok, Muse or Antigravity
# turn stops and starts again with it (law steer_codex_restarts); anything
# else is delivered like any message
def Hub.post.steer(+s: M.State, +now: Nat, +thread: String, +msg: String, role: M.Role, +text: String) -> Step:
  Hub.post.steer.k(Bool.and(Hub.running(Hub.thread(s, thread)), Bool.and(Bool.not(String.eq(Hub.provider(Hub.thread(s, thread)), "claude")),
    Bool.and(Bool.not(M.Worktree.waits(s, Hub.thread(s, thread), now)), Bool.not(M.Worktree.gone(Hub.thread(s, thread)))))), s, now, thread, msg, role, text)

def Hub.post.mode.k(steer: Bool, +s: M.State, +now: Nat, +thread: String, +msg: String, role: M.Role, +text: String) -> Step:
  match steer:
    case True{}:
      Hub.post.steer(s, now, thread, msg, role, text)
    case False{}:
      Hub.post(s, now, thread, msg, role, text)

# a message with a follow-up mode: "steer" restarts a running non-Claude
# turn with it; every other mode delivers it like any message
def Hub.post.mode(+mode: String, +s: M.State, +now: Nat, +thread: String, +msg: String, role: M.Role, +text: String) -> Step:
  Hub.post.mode.k(String.eq(mode, "steer"), s, now, thread, msg, role, text)

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
        Reply.ok(client, id) <> Git{0, "", "checkpoint0", thread, Hub.cwd(s, Hub.thread(s, thread)), "", ""} <> Git{0, "", "step", thread, Hub.cwd(s, Hub.thread(s, thread)), "", "start"} <> Hub.run(Hub.thread(s, thread), Hub.cwd(s, Hub.thread(s, thread)), text, Hub.uuid(now, thread ++ "/" ++ msg))}
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

def Hub.fork.if(known: Bool, +s: M.State, +now: Nat, +client: U32, +id: String, +src: String) -> Step:
  match known:
    case True{}:
      Hub.fork.k(Hub.thread(s, src), s, now, client, id, src)
    case False{}:
      Step.outs([Reply.err(client, id, "no such thread")])

# a new thread that goes on from this one (only from a thread the log
# knows: a name it never made forked a blank one, "Fork: " in no project
# with no agent, listed nowhere)
def Hub.fork(+s: M.State, +now: Nat, +client: U32, +id: String, +src: String) -> Step:
  Hub.fork.if(Hub.owns(s, src), s, now, client, id, src)

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

# the effort a switch was asked for, kept as a level the new provider
# offers ("" its default); none when the request names no effort
def Switch.effort(es: List<&2, String>, +th: String, +to: String, +now: Nat) -> List<&2, M.Change>:
  match es:
    case Nil{}:
      Nil{}
    case +e <> t:
      M.SettingSet{AG.Effort.key(th), AG.Effort.of(to, e), now} <> Switch.effort(t, th, to, now)

def Switch.effort.str(j: J.Json) -> List<&2, String>:
  match j:
    case J.Str{e}:
      [e]
    case _:
      Nil{}

def Switch.effort.of(m: Maybe<&2, J.Json>) -> List<&2, String>:
  match m:
    case Some{j}:
      Switch.effort.str(j)
    case None{}:
      Nil{}

def Switch.go(+s: M.State, +now: Nat, +client: U32, +id: String, +th: String, +from: String, +to: String, +model: String, es: List<&2, String>) -> Step:
  +seen = M.Switch.seen(s, th, to)
  +missed = Nat.sub(M.Switch.count(s, th), seen)
  Step{List.concat(&2, M.Change, [Switch.old(M.Queue.items(s, th), th, now), [M.ProviderSet{th, to, model, now}], Switch.effort(es, th, to, now),
      Switch.ctx(Nat.is_eq(missed, 0n), th, HD.Hand.prefix() ++ "switch:" ++ Nat.show(Hub.seq(s)), HD.Hand.switch(Hub.entries(s), th, from, to, seen), now),
      [M.ActivityLogged{th, Hub.id("p", now, Hub.seq(s)), "info", "provider", "Now on " ++ to ++ Bool.pick(String, Nat.is_eq(missed, 0n), "", "; it gets what it missed with the next message"), now}]]),
    [Reply.ok(client, id), AgentStop{th}]}

def Switch.k(run: Bool, same: Bool, +s: M.State, +now: Nat, +client: U32, +id: String, +th: String, +from: String, +to: String, +model: String, es: List<&2, String>) -> Step:
  match run same:
    case True{} _:
      Step.outs([Reply.err(client, id, "a provider changes between turns: stop this one first")])
    case False{} True{}:
      Step.outs([Reply.ok(client, id)])
    case False{} False{}:
      Switch.go(s, now, client, id, th, from, to, model, es)

# the thread goes on with provider `to` from its next turn, on model
# `model` ("" its default) and, when es names one, that effort: all of it
# between turns or none of it (the model picker's one request, modelpick.bend)
def Hub.switch.as(+s: M.State, +now: Nat, +client: U32, +id: String, +th: String, +to: String, +model: String, es: List<&2, String>) -> Step:
  Switch.k(Hub.running(Hub.thread(s, th)), String.eq(Hub.provider(Hub.thread(s, th)), to), s, now, client, id, th, Hub.provider(Hub.thread(s, th)), to, model, es)

# the thread goes on with provider `to` (its default model) from its next turn
def Hub.switch(+s: M.State, +now: Nat, +client: U32, +id: String, +th: String, +to: String) -> Step:
  Hub.switch.as(s, now, client, id, th, to, "", Nil{})

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

# Fold decimal digits modulo 2^32, the part Git.hex8 uses. Nat.read rejects
# the concatenated timestamp and six-digit sequence (past 2^48-1), which
# used to turn every such id into seed zero. Keep the accumulator bounded
# at every digit, including for ids with still longer sequences.
def Wt.seed.step(keep: Bool, c: U32, acc: U32) -> U32:
  match keep:
    case True{}:
      U32.add(U32.mul(acc, 10), U32.sub(c, 48))
    case False{}:
      acc

def Wt.seed.go(s: String, acc: U32) -> U32:
  match s:
    case SNil{}:
      acc
    case SCon{Chr{+c}, t}:
      Wt.seed.go(t, Wt.seed.step(Bool.and(U32.is_ge(c, 48), U32.is_le(c, 57)), c, acc))

# A future thread's temporary branch seed: its id's digits (time and
# sequence). Existing branches and paths are kept by their saved names.
def Wt.seed(id: String) -> Nat:
  U32.to_nat(Wt.seed.go(id, 0))

# Codex (codex exec --json)
# -------------------------

def Codex.item.k(i: Nat, +thread: String, +now: Nat, +seq: Nat, +it: J.Json, think: Bool) -> Step:
  match i:
    case 0n:
      Step{[M.MessagePosted{thread, J.Json.get_str(it, "id", Hub.id("c", now, seq)) ++ "-" ++ Nat.show(seq), M.Assistant{}, J.Json.get_str(it, "text", ""), now}], Nil{}}
    case 1n:
      Step{[M.ActivityLogged{thread, J.Json.get_str(it, "id", Hub.id("x", now, seq)) ++ "-" ++ Nat.show(seq), "tool", "shell", J.Json.get_str(it, "command", ""), now}], Nil{}}
    case 2n:
      Step{[M.ActivityLogged{thread, Hub.id("f", now, seq), "tool", "edit", J.Json.show(J.Json.get_obj(it, "changes")), now}], Nil{}}
    case 3n:
      Step{[M.TodosSet{thread, TD.Todo.show(TD.Todo.codex(it)), now}], Nil{}}
    case 4n:
      Step{Think.act(think, thread, "think:" ++ J.Json.get_str(it, "id", Hub.id("r", now, seq)) ++ "-" ++ Nat.show(seq), J.Json.get_str(it, "text", ""), now), Nil{}}
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

# item.started and item.updated share a case: only todo lists are read
# there; "notice" is a provider's word to the person, neither the agent's
# reply nor a failure (opencode's renewed session, opencode.bend)
def Codex.kind(+t: String) -> Nat:
  Bool.pick(Nat, String.eq(t, "error"), 9n, Bool.pick(Nat, String.eq(t, "context.usage"), 8n, Bool.pick(Nat, String.eq(t, "notice"), 7n, Bool.pick(Nat, String.eq(t, "tool.done"), 6n, Bool.pick(Nat, Bool.or(String.eq(t, "item.started"), String.eq(t, "item.updated")), 5n,
    C.Codec.pos(["thread.started", "item.completed", "turn.completed", "turn.failed", "item.delta"], t))))))

# Codex emits recoverable transport/rate-limit errors while it retries.
# Keep the running turn and input intact, but do not hide the wait. Use a
# bounded public notice rather than raw transport headers or request text.
def Codex.error.rate(+message: String) -> Bool:
  Bool.or(String.contains(message, "too many requests"), Bool.or(String.contains(message, "http 429"), String.contains(message, "status: 429")))

def Codex.error.text(+o: J.Json) -> String:
  +message : String = String.to_lower(J.Json.get_str(o, "message", J.Json.get_str(J.Json.get_obj(o, "error"), "message", "")))
  Bool.pick(String, Codex.error.rate(message), "Provider is rate limited (HTTP 429). Waiting for the provider to retry; your message is kept.",
    Bool.pick(String, String.contains(message, "reconnecting"), "Provider connection interrupted. The provider is retrying; your message is kept.", "Provider reported an error. Your message is kept; waiting for the provider's next result."))

def Codex.error.step(+s: M.State, +thread: String, +now: Nat, o: J.Json) -> Step:
  Step{[M.ActivityLogged{thread, Hub.id("n", now, Hub.seq(s)), "info", "provider", Codex.error.text(o), now}], Nil{}}

def Codex.line.k(i: Nat, +s: M.State, +thread: String, +now: Nat, +o: J.Json) -> Step:
  match i:
    case 0n:
      Step.after(Claude.init.k(Claude.init.same(Hub.thread(s, thread), J.Json.get_str(o, "thread_id", "")), thread, J.Json.get_str(o, "thread_id", ""), now),
        Turn.codex.start(Claude.init.same(Hub.thread(s, thread), J.Json.get_str(o, "thread_id", "")), thread, J.Json.get_str(o, "thread_id", ""), now))
    case 1n:
      Codex.item.k(C.Codec.pos(Think.items(), J.Json.get_str(J.Json.get_obj(o, "item"), "type", "")), thread, now, Hub.seq(s), J.Json.get_obj(o, "item"), M.Think.on(s))
    case 2n:
      Step{M.TurnChanged{thread, M.Completed{}, now} <> Turn.codex(s, thread, now, o), Nil{}}
    case 4n:
      Step.outs([Delta.push(thread, J.Json.get_str(o, "text", ""))])
    case 5n:
      Codex.todo(String.eq(J.Json.get_str(J.Json.get_obj(o, "item"), "type", ""), "todo_list"), thread, now, J.Json.get_obj(o, "item"))
    case 6n:
      Step.outs([Steps.out(thread, Hub.cwd(s, Hub.thread(s, thread)), "")])
    case 7n:
      Step{[M.ActivityLogged{thread, Hub.id("n", now, Hub.seq(s)), "info", "session", J.Json.get_str(o, "message", ""), now}], Nil{}}
    case 9n:
      Codex.error.step(s, thread, now, o)
    case 8n:
      Step{Usage.set(Nat.is_gt(J.Json.get_nat(o, "used", 0n), 0n), thread, J.Json.get_nat(o, "used", 0n), J.Json.get_nat(o, "window", 0n), now), Nil{}}
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
  Bool.or(Bool.or(Bool.or(String.eq(p, "grok"), String.eq(p, "muse")), Bool.or(String.eq(p, "antigravity"), String.eq(p, "opencode"))), Bool.or(String.eq(p, "pi"), String.eq(p, "omp")))

# the open-call outs of a Codex-like line, after its step
def Tools.codex(acp: Bool, m: Maybe<&2, J.Json>, st: Step, +thread: String) -> Step:
  match m:
    case Some{+o}:
      Step.also(st, Tools.line(Codex.kind(J.Json.get_str(o, "type", "")), acp, thread, o))
    case None{}:
      st

# a Codex, Grok or Muse line counts only while its thread's turn runs: what a
# stopped turn's process still says is old news (law stopped_turn_quiet)
def Hub.agent_line.c(run: Bool, +s: M.State, +now: Nat, +thread: String, +line: String) -> Step:
  match run:
    case True{}:
      Tools.codex(Hub.acp_like(Hub.provider(Hub.thread(s, thread))), J.Json.parse(line), Steps.after.if(Bool.not(Hub.acp_like(Hub.provider(Hub.thread(s, thread)))), Codex.line.parsed(J.Json.parse(line), s, thread, now), thread, Hub.cwd(s, Hub.thread(s, thread))), thread)
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
# how the changes end the thread's turn: "completed", "failed", or "" (no
# end), and where a running turn the agent cut off (Claude cancelling its
# own command, Claude.line.v.k's 5) ends "interrupted", so the server
# hears of it as it does of a failure. Only a turn still running counts:
# a Stop already ended its turn "stopped" (Retire.turn.f), and the
# cancel that follows it is not a second end.
def Ended.state(cs: List<&2, M.Change>, +thread: String, run: Bool) -> String:
  match cs run:
    case Nil{} _:
      ""
    case M.TurnChanged{+th, M.Completed{}, _} <> t _:
      Bool.pick(String, String.eq(th, thread), "completed", Ended.state(t, thread, run))
    case M.TurnChanged{+th, M.Failed{}, _} <> t _:
      Bool.pick(String, String.eq(th, thread), "failed", Ended.state(t, thread, run))
    case M.TurnChanged{+th, M.Interrupted{}, _} <> t True{}:
      Bool.pick(String, String.eq(th, thread), "interrupted", Ended.state(t, thread, True{}))
    case _ <> t _:
      Ended.state(t, thread, run)

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

def Run.settle(+s: M.State, +now: Nat, +th: String) -> List<&2, M.Change>:
  Bool.pick(List<&2, M.Change>, MR.Run.is(s, th), [M.ThreadSettled{th, False{}, now}], Nil{})

# Turn starts
# ----------
# A turn is known by where it started, not by a message from the user (a
# wake starts one with none; a message queued while a turn runs is logged
# inside it). Every step stored passes through Turn.marks (the server's
# Hub.commit.w): each TurnChanged to Running is followed by
# turn.from.<thread>, the thread's newest entry at that point (wnote.bend's
# Wn.mark), which the turn's end scans back to (Wn.cap).

def Turn.mark(+s: M.State, c: M.Change) -> List<&2, M.Change>:
  match c:
    case M.TurnChanged{+th, M.Running{}, +at}:
      [M.SettingSet{WN.Wn.from.key(th), WN.Wn.mark(M.State.mine(s, th)), at}]
    case _:
      Nil{}

def Turn.starts(cs: List<&2, M.Change>) -> Bool:
  match cs:
    case Nil{}:
      False{}
    case M.TurnChanged{_, M.Running{}, _} <> _:
      True{}
    case _ <> t:
      Turn.starts(t)

# s: the state as of c (the step's earlier changes folded in)
def Turn.marks.go(cs: List<&2, M.Change>, +s: M.State, acc: List<&2, M.Change>) -> List<&2, M.Change>:
  match cs:
    case Nil{}:
      List.reverse(&2, M.Change, acc)
    case +c <> t:
      Turn.marks.go(t, M.State.apply(s, c), List.append(&2, M.Change, Turn.mark(s, c), c <> acc))

def Turn.marks.k(starts: Bool, +s: M.State, cs: List<&2, M.Change>) -> List<&2, M.Change>:
  match starts:
    case True{}:
      Turn.marks.go(cs, s, Nil{})
    case False{}:
      cs

# s: the state before the step
def Turn.marks(+s: M.State, +cs: List<&2, M.Change>) -> List<&2, M.Change>:
  Turn.marks.k(Turn.starts(cs), s, cs)

# what a turn end tells the server: the end's own alert kind, and the
# turn's error, why it ended and its own reply, found by one scan from
# the turn's start (wnote.bend's Wn.cap; none when no webhook is set)
def Ended.turn(+thread: String, +state: String, +alert: String, g: WN.Got) -> Out:
  match g:
    case WN.Got{+err, +reply}:
      TurnEnded{thread, state, alert, err, WN.Wn.end.reason(state, err, reply), reply}

# post: the state after the step; the policy's record (core/notice.bend)
# is written with it
def Ended.out.p(+post: M.State, +s: M.State, +now: Nat, +state: String, +thread: String, +cwd: String, +cs: List<&2, M.Change>, os: List<&2, Out>) -> Step:
  Step{List.append(&2, M.Change, List.append(&2, M.Change, List.append(&2, M.Change, cs, Run.settle(s, now, thread)), Sum.at(RF.Sum.due(s, thread, now), thread, now)), NO.Policy.end(post, now, thread, state)),
    List.append(&2, Out, os, Ended.turn(thread, state, NO.Policy.kind(NO.Policy.end(post, now, thread, state)), WN.Wn.cap(post, thread)) <> Git{0, "", "step", thread, cwd, "", ""} <> Git{0, "", "checkpoint", thread, cwd, "", ""}
      <> List.append(&2, Out, Sum.out(RF.Sum.due(s, thread, now), s, thread, "thread"), Answer.out(String.eq(state, "completed"), post, thread)))}

def Ended.out.k(none: Bool, +s: M.State, +now: Nat, +state: String, +thread: String, +cwd: String, st: Step) -> Step:
  match none st:
    case True{} _:
      st
    case False{} Step{+cs, os}:
      Ended.out.p(M.State.replay(cs, s), s, now, state, thread, cwd, cs, os)

def Ended.out.s(+s: M.State, +now: Nat, +state: String, +thread: String, +cwd: String, st: Step) -> Step:
  Ended.out.k(String.is_empty(state), s, now, state, thread, cwd, st)

# a turn that ends tells the server (phone notifications hang off this),
# and has the thread summarized when its summary is due
def Ended.out(+s: M.State, +now: Nat, +thread: String, +st: Step) -> Step:
  Ended.out.s(s, now, Ended.state(Queue.changes(st), thread, Hub.running(Hub.thread(s, thread))), thread, Hub.cwd(s, Hub.thread(s, thread)), st)

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

# Legacy only: how much of the last reply a report kept in logs written
# before reports were sent whole. No report is cut now; Lr.whole reads this
# to give such an old stored row back whole.
def Lr.old.cut() -> Nat:
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

def Lr.head(+id: String, +title: String) -> String:
  "[Backplane] Launched thread \"" ++ title ++ "\" (" ++ id ++ ")"

# the note for a turn's end: the state and the last reply, whole. Nothing
# cuts a report: the row is delivered and shown in full on every client.
def Lr.note(+id: String, +title: String, +word: String, +reply: String) -> String:
  Lr.head(id, title) ++ " " ++ word ++ Bool.pick(String, String.is_empty(String.trim(reply)), ".", ":\n\n" ++ reply)

# Legacy only: the tail old stored reports end in (the reply's first
# Lr.old.cut() characters, then this). Nothing writes it now; Lr.whole
# recognises it to show such a row whole (thin-view's Message.whole).
def Lr.old.tail(+th: String) -> String:
  "…\n\n(cut short: thread_read " ++ th ++ " for the rest)"

def Lr.upto.k(keep: Bool, +e: M.Entry, +rest: List<&2, M.Entry>) -> List<&2, M.Entry>:
  match keep:
    case True{}: e <> rest
    case False{}: rest

def Lr.entry.at(+e: M.Entry) -> Nat:
  match e:
    case M.Msg{_, _, _, _, +at}: at
    case M.Act{_, _, _, _, _, +at}: at

# the thread's entries up to a time, in their order
def Lr.upto(es: List<&2, M.Entry>, +at: Nat) -> List<&2, M.Entry>:
  match es:
    case Nil{}: Nil{}
    case +e <> +t: Lr.upto.k(Nat.is_le(Lr.entry.at(e), at), e, Lr.upto(t, at))

def Lr.whole.k(ok: Bool, +text: String, +head: String, +reply: String) -> String:
  match ok:
    case True{}: head ++ reply
    case False{}: text

# the head before the reply is a report's own: its opening, the source
# thread, then one of Lr.word's words and the colon (the title between may
# be anything the thread was called then)
def Lr.whole.word(+head: String, +th: String, +word: String) -> Bool:
  String.ends_with(head, " (" ++ th ++ ") " ++ word ++ ":\n\n")

def Lr.whole.head(+head: String, +th: String) -> Bool:
  Bool.and(String.starts_with(head, "[Backplane] Launched thread \""),
    Bool.or(Bool.or(Lr.whole.word(head, th, "completed"), Lr.whole.word(head, th, "failed")),
      Bool.or(Lr.whole.word(head, th, "was interrupted"), Lr.whole.word(head, th, "finished its turn and waits for an answer"))))

# body: the row without the tail; reply: what the report was made from
def Lr.whole.got(+text: String, +th: String, +body: String, +reply: String) -> String:
  +cut = String.take(reply, Lr.old.cut())
  +head = String.take(body, Nat.sub(String.length(body), String.length(cut)))
  Lr.whole.k(Bool.and(Nat.is_gt(String.length(reply), Lr.old.cut()), Bool.and(String.ends_with(body, cut), Lr.whole.head(head, th))), text, head, reply)

def Lr.whole.cut(cut: Bool, +st: M.State, +th: String, +text: String, +at: Nat) -> String:
  match cut:
    case True{}:
      Lr.whole.got(text, th, String.take(text, Nat.sub(String.length(text), String.length(Lr.old.tail(th)))), Lr.reply(Answer.of(Lr.upto(M.State.mine(st, th), at), th)))
    case False{}: text

# A report row (id, text, at) stored cut short, whole: its launched thread
# is the id's link (Lr.msg: never the hint in the text), its reply is read
# as Lr.end.c read it then (Answer.of over the thread's entries up to the
# report), and it is used only when the row is exactly that reply's old
# cut form under a report's own head (Lr.whole.head); any other row, or one
# that does not match, is left as it is.
def Lr.whole(+st: M.State, +id: String, +text: String, +at: Nat) -> String:
  Lr.whole.cut(Bool.and(Lr.is(id), Bool.and(Bool.not(String.is_empty(M.Link.from(id))), String.ends_with(text, Lr.old.tail(M.Link.from(id))))), st, M.Link.from(id), text, at)

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

# a turn's end: the counter of launcher-started turns moves on, and at
# the bound the reports stop
# the counter, written only when it moves
def Lr.n.set(same: Bool, +thread: String, +n: Nat, +now: Nat) -> List<&2, M.Change>:
  match same:
    case True{}:
      Nil{}
    case False{}:
      [M.SettingSet{Lr.n.key(thread), Nat.show(n), now}]

def Task.by.id.k(r: Map<&2, M.Task> & M.Task) -> M.Task:
  (_, t) = r
  t

def Task.by.id(+s: M.State, +id: String) -> M.Task:
  match s:
    case M.State{_, _, _, _, tk, _, _, _, _, _, _}:
      Task.by.id.k(Map.get(M.Task, M.Task.blank(id), tk, id))

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

# (an order's question parks nothing either: it stays, and an answer
# later starts the agent again, Hub.answer.order)
def Retire.hit(a: M.Ask, +th: String) -> Bool:
  match a:
    case M.Ask{_, t, +kind, +tool, _, _, op, _}:
      Bool.and(op, Bool.and(String.eq(t, th), Bool.not(Bool.or(String.eq(kind, "plan"), String.eq(tool, "order_ask")))))

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

def Mail.send.worker(+person: Bool, +s: M.State, +now: Nat, +from: String, +thread: String, +p: J.Json, +msg: String) -> Step:
  match person:
    case True{}:
      Step.cat(Hub.post(s, now, thread, msg, M.User{}, J.Json.get_str(p, "text", "")), Mail.compose(s, now, "person", Mail.addr(s, thread), J.Json.get_str(p, "text", ""), msg,
        J.Json.get_str(p, "conv", ""), J.Json.get_str(p, "subject", ""), J.Json.get_str(p, "want", "")))
    case False{}:
      Step.cat(Origin.step(Hub.inbox.id(s, now, from, thread, M.User{}, J.Json.get_str(p, "text", ""), msg), thread, msg, ML.Origin{Mail.addr(s, from), "thread", ""}, now),
        Mail.compose(s, now, Mail.addr(s, from), Mail.addr(s, thread), J.Json.get_str(p, "text", ""), msg, J.Json.get_str(p, "conv", ""), J.Json.get_str(p, "subject", ""), J.Json.get_str(p, "want", "")))

def Mail.send.bot(bot: Bool, +person: Bool, +s: M.State, +now: Nat, +from: String, +thread: String, +p: J.Json, +msg: String) -> Step:
  match bot:
    case False{}: Mail.send.worker(person, s, now, from, thread, p, msg)
    case True{}: Mail.compose(s, now, Mail.sender(s, Bool.pick(String, person, "", from)), Mail.addr(s, thread), J.Json.get_str(p, "text", ""), msg, J.Json.get_str(p, "conv", ""), J.Json.get_str(p, "subject", ""), J.Json.get_str(p, "want", ""))

def Mail.send.to(+person: Bool, +s: M.State, +now: Nat, +from: String, +thread: String, +p: J.Json, +msg: String) -> Step:
  Mail.send.bot(Bool.and(String.eq(M.Setting.get(s, "mail.on", "off"), "on"), BT.World.is_bot_thread(M.State.bots(s), thread)), person, s, now, from, thread, p, msg)

def Mail.send.valid(+ok: Bool, +s: M.State, +now: Nat, +from: String, +thread: String, +p: J.Json, +msg: String) -> Step:
  match ok:
    case True{}: Mail.send.to(String.eq(from, "person"), s, now, from, thread, p, msg)
    case False{}: Step.none()

def Mail.send(+s: M.State, +now: Nat, +from: String, +p: J.Json) -> Step:
  +thread = Mail.target(s, J.Json.get_str(p, "to", ""))
  Mail.send.valid(Hub.owns(s, thread), s, now, from, thread, p, Hub.id("mail", now, Hub.seq(s)))

# Human socket requests are already authenticated by Conn.allowed. Mailbox
# usability is independent of the hidden-agent routing switch. Bot targets
# always use the durable mailbox path, including while automation is off.
def Mail.ui.send.valid(ok: Bool, +s: M.State, +now: Nat, +thread: String, +p: J.Json, +msg: String) -> Step:
  match ok:
    case False{}: Step.none()
    case True{}: Mail.send.bot(BT.World.is_bot_thread(M.State.bots(s), thread), True{}, s, now, "person", thread, p, msg)

def Mail.ui.send(+s: M.State, +now: Nat, +p: J.Json) -> Step:
  +thread = Mail.target(s, J.Json.get_str(p, "to", ""))
  Mail.ui.send.valid(Hub.owns(s, thread), s, now, thread, p, Hub.id("mail", now, Hub.seq(s)))

# Authentication is already checked by the socket. Never ACK an invalid
# human target as though a durable append had been scheduled.
def Mail.ui.request.valid(ok: Bool, +s: M.State, +now: Nat, +client: U32, +id: String, +p: J.Json) -> Step:
  match ok:
    case False{}: Step.outs([Reply.err(client, id, "invalid mail target")])
    case True{}: Step.before(Nil{}, [Reply.ok(client, id)], Mail.ui.send(s, now, p))

def Mail.ui.request(+s: M.State, +now: Nat, +client: U32, +id: String, +p: J.Json) -> Step:
  Mail.ui.request.valid(Hub.owns(s, Mail.target(s, J.Json.get_str(p, "to", ""))), s, now, client, id, p)

def Mail.request.on(+on: Bool, +i: Nat, +s: M.State, +now: Nat, +client: U32, +id: String, +p: J.Json) -> Step:
  match on i:
    case False{} _: Step.outs([Reply.err(client, id, "mail is off")])
    case True{} 0n: Step.outs([Reply.data(client, id, Mail.list(s, p))])
    case True{} 1n: Mail.read(s, now, client, id, p)
    case True{} 2n: Step.outs([Reply.data(client, id, Mail.find(s, p))])
    case True{} 3n: Mail.request.mark(Mail.mark.valid(J.Json.get_str(p, "status", "")), s, now, client, id, p)
    case True{} 4n: Step.reply(client, id, Mail.tag(now, p))
    case True{} _: Mail.ui.request(s, now, client, id, p)

# This entrypoint is for authenticated client RPCs only. MCP continues to
# use Mcp.mail.call's existing mail.on + caller-scope checks.
def Mail.request.ui(+i: Nat, +s: M.State, +now: Nat, +client: U32, +id: String, +p: J.Json) -> Step:
  Mail.request.on(True{}, i, s, now, client, id, p)

def Mail.tee.on(+on: Bool, +st: Step, +s: M.State, +now: Nat, +o: ML.Origin, +to: String, +text: String, +msg: String) -> Step:
  match on:
    case False{}: st
    case True{}: Step.cat(st, Mail.file(s, now, o, to, text, msg))

def Mail.tee(+st: Step, +s: M.State, +now: Nat, +o: ML.Origin, +to: String, +text: String, +msg: String) -> Step:
  Mail.tee.on(Bool.and(String.eq(M.Setting.get(s, "mail.on", "off"), "on"), Bool.not(String.eq(ML.Origin.kind(o), "person"))), st, s, now, o, to, text, msg)

# The one room-level tee is independent of the member wake count.
def Mail.room.file(+s: M.State, +now: Nat, +from: String, +room: String, +text: String, +msg: String, +parts: List<&2, String>, +subject: String) -> Step:
  +o = {ML.Origin{from, "room", room} : ML.Origin}
  +conv = ML.Mail.route(o, REF.Ref.addr(REF.Ref.room(room)))
  Mail.file.body(s, MB.Body{ML.Conv{conv, subject, ML.Conv.mail_kind(conv, "room"), parts, "", Nil{}, now, now, 0n},
    ML.Mail{msg, conv, from, REF.Ref.addr(REF.Ref.room(room)), "room", room, "fyi", ML.Mail.head(text), Mail.bytes(text, 0n), now}, text})

def Mail.room.on(+on: Bool, +st: Step, +s: M.State, +now: Nat, +from: String, +room: String, +text: String, +msg: String, +parts: List<&2, String>, +subject: String) -> Step:
  match on:
    case False{}: st
    case True{}: Step.cat(st, Mail.room.file(s, now, from, room, text, msg, parts, subject))

def Mail.room(+st: Step, +s: M.State, +now: Nat, +from: String, +room: String, +text: String, +msg: String, +parts: List<&2, String>, +subject: String) -> Step:
  Mail.room.on(Bool.or(String.eq(M.Setting.get(s, "mail.on", "off"), "on"), ML.Parts.has(parts, "bot:" ++ UB.Uber.bot(s), False{})), st, s, now, from, room, text, msg, parts, subject)

# Discard the old FOH-delivery step while retaining independent room and
# wake bookkeeping. Calls that wrap the funnel retain their own changes.
def Mail.foh.cs(+cs: List<&2, M.Change>) -> List<&2, M.Change>:
  match cs:
    case Nil{}: Nil{}
    case M.RoomPosted{+id, +r, +f, +tx, +hop, +at} <> +t: M.RoomPosted{id, r, f, tx, hop, at} <> Mail.foh.cs(t)
    case M.BotWoke{+b, +hop, +at} <> +t: M.BotWoke{b, hop, at} <> Mail.foh.cs(t)
    case _ <> +t: Mail.foh.cs(t)

def Mail.foh(st: Step) -> Step:
  match st:
    case Step{cs, _}: Step{Mail.foh.cs(cs), Nil{}}

def Mail.funnel.bot(bot: Bool, +st: Step, +s: M.State, +now: Nat, +o: ML.Origin, +to: String, +text: String, +msg: String) -> Step:
  match bot:
    case True{}: Mail.tee(Mail.foh(st), s, now, o, to, text, msg)
    case False{}: Mail.tee(st, s, now, o, to, text, msg)

# Keep Mail.tee's existing delivery-preservation law. All production
# inbound sources pass this switch, before the tee writes their body.
def Mail.funnel.owner(owned: Bool, +st: Step, +s: M.State, +now: Nat, +o: ML.Origin, +to: String, +text: String, +msg: String) -> Step:
  match owned:
    case True{}: Step.cat(Mail.foh(st), Mail.file(s, now, o, to, text, msg))
    case False{}: Mail.funnel.bot(Bool.and(String.eq(M.Setting.get(s, "mail.on", "off"), "on"), Bool.and(String.starts_with(to, "bot:"), Bool.not(String.eq(ML.Origin.kind(o), "person")))), st, s, now, o, to, text, msg)

def Mail.funnel(+st: Step, +s: M.State, +now: Nat, +o: ML.Origin, +to: String, +text: String, +msg: String) -> Step:
  Mail.funnel.owner(Bool.and(String.eq(to, "bot:" ++ UB.Uber.bot(s)), Bool.not(String.eq(ML.Origin.kind(o), "person"))), st, s, now, o, to, text, msg)

def Mail.inbound(+st: Step, +s: M.State, +now: Nat, +o: ML.Origin, +to: String, +text: String, +msg: String) -> Step:
  Mail.funnel(Origin.step(st, to, msg, o, now), s, now, o, Mail.addr(s, to), text, msg)

# A linked inbound message, with durable provenance following its post.
def Hub.inbox.from(+s: M.State, +now: Nat, +from: String, +to: String, +role: M.Role, +text: String, +msg: String) -> Step:
  Mail.inbound(Hub.inbox.id(s, now, from, to, role, text, msg), s, now, ML.Mail.classify(M.Link.from(msg), msg, Origin.system(role), text), to, text, msg)

# a report from a typed producer (kind "launch" or "task"): to the
# uberbot's thread it is filed; to any other thread it is posted as before,
# with the producer's acknowledgements after it
def Operational.inbox.k(owned: Bool, +s: M.State, +now: Nat, +kind: String, +from: String, +to: String, role: M.Role, +text: String, +msg: String, +after: List<&2, M.Change>) -> Step:
  match owned:
    case True{}: Operational.file(s, now, ML.Origin{Mail.addr(s, from), kind, ""}, to, text, msg, after)
    case False{}: Step.after(Hub.inbox.from(s, now, from, to, role, text, msg), after)

def Operational.inbox(+s: M.State, +now: Nat, +kind: String, +from: String, +to: String, role: M.Role, +text: String, +msg: String, +after: List<&2, M.Change>) -> Step:
  Operational.inbox.k(Operational.is(s, to), s, now, kind, from, to, role, text, msg, after)

def Hub.inbox(+s: M.State, +now: Nat, +from: String, +to: String, role: M.Role, +text: String) -> Step:
  Hub.inbox.from(s, now, from, to, role, text, M.Link.id(from, Hub.id("m", now, Hub.seq(s))))

# (the result reaches the parent before TaskDelivered: a step torn by a
# crash after the message leaves the task undelivered, and Task.repair
# finishes it without a second message; law task_finish_delivered_last)
def Task.finish.go(deliver: Bool, +s: M.State, +now: Nat, +id: String, +thread: String, +parent: String, +provider: String, +model: String, +word: String, st: M.TaskState, +summary: String, cs: List<&2, M.Change>, os: List<&2, Out>) -> Step:
  match deliver:
    case True{}:
      Step.before(List.append(&2, M.Change, cs, [M.TaskStateSet{id, st, summary, now}]), os,
        Operational.inbox(s, now, "task", thread, parent, M.System{}, Task.note(id, provider, model, word, summary), Task.msg(thread, id), [M.TaskDelivered{id, now}]))
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

def Task.finish.w.e(e: Ended, +s: M.State, +now: Nat, +thread: String, +cs: List<&2, M.Change>, os: List<&2, Out>) -> Step:
  match e:
    case NotEnded{}:
      Step{cs, os}
    case EndedOk{}:
      Task.finish.e(Task.ok.if(M.Work.ready(M.State.replay(cs, s), thread)), s, now, thread, cs, os)
    case EndedBad{}:
      Task.finish.e(EndedBad{}, s, now, thread, cs, os)

# a child's task ends when its thread is done: a completed turn that left
# work out (its own children, background or watched work) or a queued
# message keeps the task open, and the turn that finishes that work
# delivers; a failure or a stop ends it at once (laws task_waits_for_child_work,
# task_failed_ends_even_busy)
def Task.finish.w(+s: M.State, +now: Nat, +thread: String, st: Step) -> Step:
  match st:
    case Step{+cs, os}:
      Task.finish.w.e(Task.ended.last(cs, thread), s, now, thread, cs, os)

# what a note through Lr.send is, for the Mailroom: a deploy's report, a
# lock's grant, else a launched thread's report
def Lr.kind(+thread: String) -> String:
  Bool.pick(String, String.eq(thread, "deploy"), "deploy", Bool.pick(String, String.starts_with(thread, "lock:"), "lock", "launch"))

def Lr.send.here(ok: Bool, +s: M.State, +now: Nat, +thread: String, +to: String, +text: String) -> Step:
  match ok:
    case True{}:
      Operational.inbox(s, now, Lr.kind(thread), thread, to, M.User{}, text, Lr.msg(thread, Hub.id("r", now, Hub.seq(s))), Nil{})
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

# The queue for the owner's machines that are away
# --------------------------------------------------
# A thread_send or thread_launch for one of the owner's machines that is
# away (fmcp.bend's Fm.plan.i) is not refused: it waits in the setting
# far.queue, a line each, oldest first: "key\tlink\tthread\ttool\tat\tbody"
# (body: the request as the machine takes it, Fm.request; JSON, so no tab).
# The server sends the lines in order (FarQueued: when one is queued and on
# each minute while any waits), each machine's after its earlier ones: a
# machine that does not answer keeps all of its lines back. A line its
# machine answered (taken or refused) leaves the queue, and in the same
# step the thread that queued it is told what came of it (Fq.done, laws
# fq_*); so is one older than Fq.ttl(), which is never sent. At most once:
# a line is sent again only when its machine gave no answer at all.
def Fq.key() -> String:
  "far.queue"

def Fq.ttl() -> Nat:
  86400n

# the tools that wait for a machine that is away (others are refused)
def Fq.tool(+tool: String) -> Bool:
  Bool.or(String.eq(tool, "thread_send"), String.eq(tool, "thread_launch"))

def Fq.line(+key: String, +link: String, +thread: String, +tool: String, +at: Nat, +body: String) -> String:
  key ++ "\t" ++ link ++ "\t" ++ thread ++ "\t" ++ tool ++ "\t" ++ Nat.show(at) ++ "\t" ++ body

def Fq.lines(+s: M.State) -> List<&2, String>:
  M.Launch.lines(M.Setting.get(s, Fq.key(), ""))

def Fq.add(+s: M.State, +line: String, +now: Nat) -> M.Change:
  M.SettingSet{Fq.key(), Bool.pick(String, String.is_empty(M.Setting.get(s, Fq.key(), "")), line, M.Setting.get(s, Fq.key(), "") ++ "\n" ++ line), now}

# a call queued under its key (the request carries it too: fmcp.bend's
# Fm.keyed, so its machine answers a call sent again from its receipt):
# kept, and the queue sent at once (the machine may be back)
def Fq.queue(+s: M.State, +now: Nat, +key: String, +link: String, +thread: String, +tool: String, +body: String) -> Step:
  Step{[Fq.add(s, Fq.line(key, link, thread, tool, now, body), now)], [FarQueued{}]}

# The queue is bounded: at most Fq.max() calls, each at most Fq.bytes()
# UTF-8 bytes of the call as stored (keyed: Fm.keyed), checked before it is
# queued (one over is refused, and the agent told). Its machine keeps more
# receipts per link than this (Fm.seen.max(), law fq_window), so a call the
# sender can still send again always finds its answer there.
def Fq.max() -> Nat:
  100n

def Fq.bytes() -> Nat:
  65536n

def Fq.room.n(+n: Nat, +size: Nat) -> Bool:
  Bool.and(Nat.is_lt(n, Fq.max()), Nat.is_le(size, Fq.bytes()))

# a text's size as stored and sent: its UTF-8 bytes (not its characters)
def Fq.utf8.k(+c: U32) -> Nat:
  Bool.pick(Nat, U32.is_lt(c, 128), 1n, Bool.pick(Nat, U32.is_lt(c, 2048), 2n, Bool.pick(Nat, U32.is_lt(c, 65536), 3n, 4n)))

def Fq.utf8(s: String, +acc: Nat) -> Nat:
  match s:
    case SNil{}:
      acc
    case SCon{Chr{+c}, t}:
      Fq.utf8(t, Nat.add(acc, Fq.utf8.k(c)))

# room for one more call, its body the exact keyed request that is stored
# and sent (fmcp.bend's Fm.later.res measures it after Fm.keyed)
def Fq.room(+s: M.State, +body: String) -> Bool:
  Fq.room.n(List.length(&2, String, Fq.lines(s)), Fq.utf8(body, 0n))

# the fields of a line
def Fq.f.key(+l: String) -> String:
  M.Bg.field(l, 0n)

def Fq.f.link(+l: String) -> String:
  M.Bg.field(l, 1n)

def Fq.f.thread(+l: String) -> String:
  M.Bg.field(l, 2n)

def Fq.f.tool(+l: String) -> String:
  M.Bg.field(l, 3n)

def Fq.f.body(+l: String) -> String:
  M.Bg.field(l, 5n)

def Fq.num.k(skip: Bool, +c: U32, +acc: Nat) -> Nat:
  match skip:
    case True{}:
      acc
    case False{}:
      Nat.add(Nat.mul(acc, 10n), U32.to_nat((c - 48 : U32)))

def Fq.num(s: String, +acc: Nat) -> Nat:
  match s:
    case SNil{}:
      acc
    case SCon{Chr{+c}, t}:
      Fq.num(t, Fq.num.k(Bool.or(U32.is_lt(c, 48), U32.is_gt(c, 57)), c, acc))

def Fq.f.at(+l: String) -> Nat:
  Fq.num(M.Bg.field(l, 4n), 0n)

# a line too old to send
def Fq.old(+l: String, +now: Nat) -> Bool:
  Nat.is_gt(now, Nat.add(Fq.f.at(l), Fq.ttl()))

def Fq.keep.k(drop: Bool, +l: String, rest: List<&2, String>) -> List<&2, String>:
  match drop:
    case True{}:
      rest
    case False{}:
      l <> rest

def Fq.keep(ls: List<&2, String>, +key: String) -> List<&2, String>:
  match ls:
    case Nil{}:
      Nil{}
    case +l <> t:
      Fq.keep.k(String.starts_with(l, key ++ "\t"), l, Fq.keep(t, key))

def Fq.has(ls: List<&2, String>, +key: String) -> Bool:
  match ls:
    case Nil{}:
      False{}
    case +l <> t:
      Bool.or(String.starts_with(l, key ++ "\t"), Fq.has(t, key))

# what the thread is told
def Fq.said(+tool: String, +machine: String, ok: Bool, +detail: String) -> String:
  "Your queued " ++ tool ++ " for " ++ machine ++ Bool.pick(String, ok, " reached it: ", " did not go through: ") ++ detail

def Fq.expired(+tool: String, +machine: String) -> String:
  "Your queued " ++ tool ++ " for " ++ machine ++ " expired: that machine stayed away for a day, so it was not sent."

def Fq.told(go: Bool, +s: M.State, +now: Nat, +key: String, +thread: String, +text: String) -> Step:
  match go:
    case True{}:
      Hub.inbox.id(s, now, "far", thread, M.User{}, text, "fq-" ++ key)
    case False{}:
      Step.none()

def Fq.done.k(had: Bool, +s: M.State, +now: Nat, +key: String, +thread: String, +text: String) -> Step:
  match had:
    case True{}:
      Step.before([M.SettingSet{Fq.key(), String.join(Fq.keep(Fq.lines(s), key), "\n"), now}], Nil{}, Fq.told(Lr.exists(s, thread), s, now, key, thread, text))
    case False{}:
      Step.none()

# a line its machine answered, or that expired: it leaves the queue and its
# thread is told, once (a key no longer queued does nothing: law fq_done_once)
def Fq.done(+s: M.State, +now: Nat, +key: String, +thread: String, +text: String) -> Step:
  Fq.done.k(Fq.has(Fq.lines(s), key), s, now, key, thread, text)

# for laws: a queue of two lines (machines p1 and p2), and what a step
# writes to the queue
def Fq.demo.l1() -> String:
  Fq.line("q1", "p1", "t1", "thread_send", 100n, "{}")

def Fq.demo.l2() -> String:
  Fq.line("q2", "p2", "t1", "thread_launch", 200n, "{}")

def Fq.demo.s() -> M.State:
  M.State.replay([M.ThreadCreated{"t1", "", "Bub", M.Local{}, "", "", "claude", "", 1n}, M.SettingSet{Fq.key(), Fq.demo.l1() ++ "\n" ++ Fq.demo.l2(), 2n}], M.State.empty())

def Fq.sets.k(mine: Bool, +v: String, rest: List<&2, String>) -> List<&2, String>:
  match mine:
    case True{}:
      v <> rest
    case False{}:
      rest

def Fq.sets(cs: List<&2, M.Change>) -> List<&2, String>:
  match cs:
    case Nil{}:
      Nil{}
    case M.SettingSet{+k, +v, _} <> t:
      Fq.sets.k(String.eq(k, Fq.key()), v, Fq.sets(t))
    case _ <> t:
      Fq.sets(t)

def Fq.step.sets(st: Step) -> List<&2, String>:
  match st:
    case Step{cs, _}:
      Fq.sets(cs)

def Fq.step.n(st: Step) -> Nat:
  match st:
    case Step{cs, _}:
      List.length(&2, M.Change, cs)

# each minute: sent again while anything waits
def Fq.tick.k(none: Bool) -> Step:
  match none:
    case True{}:
      Step.none()
    case False{}:
      Step.outs([FarQueued{}])

def Fq.tick(+s: M.State) -> Step:
  Fq.tick.k(String.is_empty(M.Setting.get(s, Fq.key(), "")))

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

def Lr.ask.send(coordinated: Bool, +s: M.State, +now: Nat, +thread: String, +to: String, +ask: String) -> Step:
  match coordinated:
    case True{}: Step.none()
    case False{}: Lr.send(s, now, thread, to, Lr.ask.note(thread, Lr.title(Hub.thread(s, thread)), ask))

def Lr.what(asked: Bool, +ask: String, +e: Nat, +s: M.State, +now: Nat, +thread: String, +by: String, +to: String) -> Step:
  match asked:
    case True{}:
      Lr.ask.send(Operational.is(s, to), s, now, thread, to, ask)
    case False{}:
      Lr.what.e(Nat.is_eq(e, 0n), e, s, now, thread, by, to)

def Lr.tell.q(quiet: Bool, +to: String, +ask: String, +e: Nat, +s1: M.State, +now: Nat, +thread: String) -> Step:
  match quiet:
    case True{}:
      Step.none()
    case False{}:
      Lr.what(Bool.not(String.is_empty(ask)), ask, e, s1, now, thread, M.Launch.by(s1, thread), to)

# the person cancels a delegated task from its row: its child stops, and
# the parent is told (once: TaskDelivered last); a finished task is left
def Task.cancel.user(t: M.Task, +s: M.State, +now: Nat) -> Step:
  match t:
    case M.Task{+id, +parent, +child, _, +st, _, _, +provider, +model}:
      Bool.pick(Step, M.TaskState.terminal(st), Step.none(),
        Step.before([M.TaskStateSet{id, M.TaskCancelled{}, "cancelled by the user", now}, M.TurnChanged{child, M.Interrupted{}, now}], [AgentStop{child}],
          Operational.inbox(s, now, "task", child, parent, M.System{}, Task.note(id, provider, model, "cancelled by the user", ""), Task.msg(child, id), [M.TaskDelivered{id, now}])))

def Ride.after.one(+s: M.State, +now: Nat, +th: String, st: Step) -> Step:
  match st:
    case Step{+cs, os}:
      Step.cat(Step{cs, os}, Ride.start(M.State.replay(cs, s), now, th))

def Ride.after.go(cs: List<&2, M.Change>, +s: M.State, +now: Nat, st: Step) -> Step:
  match cs:
    case Nil{}:
      st
    case M.TurnChanged{_, M.Running{}, _} <> t:
      Ride.after.go(t, s, now, st)
    case M.TurnChanged{+th, _, _} <> t:
      Ride.after.go(t, s, now, Ride.after.one(s, now, th, st))
    case _ <> t:
      Ride.after.go(t, s, now, st)

# a step that ends a turn with mail waiting starts the next turn with it
# (law ride_turn_end_starts); s: the state before the step
def Ride.after(+s: M.State, +now: Nat, +st: Step) -> Step:
  Ride.after.go(Queue.changes(st), s, now, st)

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
# harness-coord: wakes. A matching step sends one message through Lr.send.
# A wake the uberbot asked for (wake_on) is filed in its Mailroom, and,
# enters the same private triage pipeline as launched reports. Only a
# triaged escalation uses the ride; other threads keep Lr.send.
def Wake.ride(+s: M.State, +now: Nat, +by: String, +msg: String, +text: String) -> Step:
  +cs = {[M.SettingSet{RD.Ride.key(by), RD.Ride.add(Ride.of(s, by), msg, text), now}] : List<&2, M.Change>}
  Step.cat(Step{cs, Nil{}}, Ride.start(M.State.replay(cs, s), now, by))

def Wake.send(owned: Bool, +s: M.State, +now: Nat, +from: String, +by: String, +ref: String, +text: String) -> Step:
  match owned:
    case False{}: Lr.send(s, now, from, by, text)
    case True{}:
      +msg = "wake:" ++ ref
      Operational.file(s, now, ML.Origin{Mail.addr(s, from), "wake", ref}, by, text, msg, Nil{})

def Wake.fire.k(fresh: Bool, +s: M.State, +now: Nat, +id: String, +key: String, +events: List<&2, String>, +text: String) -> Step:
  match fresh:
    case False{}:
      Step.none()
    case True{}:
      +o = WK.Wake.get(s, id)
      Step.before(WK.Wake.mark(id, key, J.Json.get_bool(o, "once", True{}), now), Nil{},
        Wake.send(Operational.is(s, J.Json.get_str(o, "by", "")), s, now, Bool.pick(String, String.eq(J.Json.get_str(o, "kind", ""), "thread"), J.Json.get_str(o, "target", ""), J.Json.get_str(o, "by", "")), J.Json.get_str(o, "by", ""), J.Json.get_str(o, "kind", "") ++ ":" ++ J.Json.get_str(o, "target", "") ++ ":" ++ key, WK.Wake.note(o, text, events)))

def Wake.thread.end.key(cs: List<&2, M.Change>, +th: String) -> String:
  match cs:
    case Nil{}:
      ""
    case M.TurnChanged{+t, +state, +at} <> rest:
      Bool.pick(String, Bool.and(String.eq(t, th), Nat.is_gt(Lr.one(M.TurnChanged{t, state, at}, th, 0n), 0n)), t ++ ":" ++ Nat.show(at) ++ ":" ++ Nat.show(Lr.one(M.TurnChanged{t, state, at}, th, 0n)), Wake.thread.end.key(rest, th))
    case _ <> rest:
      Wake.thread.end.key(rest, th)

def Wake.thread.ask.key(cs: List<&2, M.Change>, +th: String) -> String:
  match cs:
    case Nil{}:
      ""
    case M.AskOpened{+id, +t, _, _, _, _, at} <> rest:
      Bool.pick(String, String.eq(t, th), t ++ ":ask:" ++ id ++ ":" ++ Nat.show(at), Wake.thread.ask.key(rest, th))
    case _ <> rest:
      Wake.thread.ask.key(rest, th)

def Wake.ended(cs: List<&2, M.Change>, +th: String) -> Nat:
  match cs:
    case Nil{}:
      0n
    case +c <> t:
      Bool.pick(Nat, Nat.is_gt(Lr.one(c, th, 0n), 0n), Lr.one(c, th, 0n), Wake.ended(t, th))

def Wake.thread.events(+s: M.State, +th: String, +cs: List<&2, M.Change>) -> List<&2, String>:
  +end = Wake.ended(cs, th)
  List.concat(&2, String, [Bool.pick(List<&2, String>, Nat.is_gt(end, 0n), ["end"], Nil{}),
    Bool.pick(List<&2, String>, Nat.is_eq(end, 1n), ["done"], Nil{}),
    Bool.pick(List<&2, String>, Nat.is_eq(end, 2n), ["fail"], Nil{}),
    Bool.pick(List<&2, String>, Bool.not(String.is_empty(Wake.thread.ask.key(cs, th))), ["ask"], Nil{}),
    Bool.pick(List<&2, String>, Bool.and(Nat.is_gt(end, 0n), Bool.and(Nat.is_eq(M.Work.busy(s, th), 0n), Bool.and(Bool.not(M.Queue.waiting(s, th)), Bool.not(Hub.running(Hub.thread(s, th)))))), ["idle"], Nil{})])

def Wake.thread.text(+s: M.State, +th: String, +cs: List<&2, M.Change>) -> String:
  "thread \"" ++ Lr.title(Hub.thread(s, th)) ++ "\" (" ++ th ++ ") " ++
    Bool.pick(String, Nat.is_gt(Wake.ended(cs, th), 0n), "ended its turn (" ++ Lr.word(Wake.ended(cs, th), "") ++ "). Its last reply: " ++ Lr.reply(Answer.of(M.State.mine(s, th), th)), "opened an ask: " ++ Lr.ask(cs, th))

def Wake.poll.k(empty: Bool, prs: List<&2, String>, files: List<&2, String>) -> Step:
  match empty:
    case True{}:
      Step.none()
    case False{}:
      Step.outs([WakePoll{prs, files}])

def Wake.poll(+s: M.State) -> Step:
  +prs = WK.Wake.targets(M.Wake.ids(s, "wakes.poll"), s, "pr", Nil{})
  +files = WK.Wake.targets(M.Wake.ids(s, "wakes.poll"), s, "file", Nil{})
  Wake.poll.k(Bool.and(Nat.is_eq(List.length(&2, String, prs), 0n), Nat.is_eq(List.length(&2, String, files), 0n)), prs, files)

# The first stat is the subscription's baseline; exists may fire immediately.
def Wake.file.events(+base: String, +stamp: String) -> List<&2, String>:
  List.concat(&2, String, [Bool.pick(List<&2, String>, String.eq(stamp, "missing"), Nil{}, ["exists"]),
    Bool.pick(List<&2, String>, Bool.and(Bool.not(String.is_empty(base)), Bool.not(String.eq(base, stamp))), ["changed"], Nil{})])

# Drops accompany every deletion, including scratch expiry and bot deletion.
def Wake.deleted.go(cs: List<&2, M.Change>, +s: M.State, +now: Nat) -> List<&2, M.Change>:
  match cs:
    case Nil{}:
      Nil{}
    case M.ThreadDeleted{th, at} <> t:
      List.append(&2, M.Change, WK.Wake.drop(WK.Wake.by(s, th), at), Wake.deleted.go(t, s, now))
    case _ <> t:
      Wake.deleted.go(t, s, now)

def Wake.deleted(+s: M.State, st: Step) -> Step:
  match st:
    case Step{+cs, os}:
      Step{List.append(&2, M.Change, cs, Wake.deleted.go(cs, s, 0n)), os}

# an archived or deleted thread is told nothing
def Lost.live(t: M.Thread) -> Bool:
  match t:
    case M.Thread{_, _, _, _, _, _, _, ar, _, _, _, _, _, _, _, _, _, _, _, _, de}:
      Bool.not(Bool.or(ar, de))

def Wake.fire(+s: M.State, +now: Nat, +id: String, +key: String, +actual: List<&2, String>, +text: String) -> Step:
  +events = WK.Wake.select(WK.Wake.events(WK.Wake.get(s, id)), actual)
  Wake.fire.k(Bool.and(WK.Wake.fresh(s, id, key, events), Bool.and(Hub.owns(s, J.Json.get_str(WK.Wake.get(s, id), "by", "")), Lost.live(Hub.thread(s, J.Json.get_str(WK.Wake.get(s, id), "by", ""))))), s, now, id, key, events, text)

# State moves between subscriptions so several waking the same thread queue safely.
def Wake.deliver(ids: List<&2, String>, +s: M.State, +now: Nat, +key: String, +events: List<&2, String>, +text: String, +st: Step) -> Step:
  match ids:
    case Nil{}:
      st
    case id <> t:
      Wake.deliver(t, s, now, key, events, text, Step.cat(st, Wake.fire(M.State.replay(Queue.changes(st), s), now, id, key, events, text)))

def Wake.thread.fire.normal(+s: M.State, +now: Nat, +id: String, +signals: List<&2, WK.WakeSignal>, +text: String) -> Step:
  +matches = WK.Wake.signal.unseen(signals, s, id, WK.Wake.events(WK.Wake.get(s, id)))
  +events = WK.Wake.signal.events(matches)
  +st = Wake.fire.k(Bool.and(Nat.is_gt(List.length(&2, String, events), 0n), Bool.and(Hub.owns(s, J.Json.get_str(WK.Wake.get(s, id), "by", "")), Lost.live(Hub.thread(s, J.Json.get_str(WK.Wake.get(s, id), "by", ""))))), s, now, id, String.join(WK.Wake.signal.keys(matches), "|"), events, text)
  Step.before(Bool.pick(List<&2, M.Change>, Nat.is_gt(List.length(&2, M.Change, Queue.changes(st)), 0n), WK.Wake.signal.mark(matches, id, now), Nil{}), Nil{}, st)

def Wake.thread.covered.mark(hit: Bool, +matches: List<&2, WK.WakeSignal>, +id: String, +o: J.Json, +now: Nat) -> Step:
  match hit:
    case False{}: Step.none()
    case True{}: Step{List.append(&2, M.Change, WK.Wake.signal.mark(matches, id, now), WK.Wake.mark(id, String.join(WK.Wake.signal.keys(matches), "|"), J.Json.get_bool(o, "once", True{}), now)), Nil{}}

def Wake.thread.fire.covered(covered: Bool, +s: M.State, +now: Nat, +id: String, +signals: List<&2, WK.WakeSignal>, +text: String) -> Step:
  match covered:
    case False{}: Wake.thread.fire.normal(s, now, id, signals, text)
    case True{}:
      +o = WK.Wake.get(s, id)
      +matches = WK.Wake.signal.unseen(signals, s, id, WK.Wake.events(o))
      Wake.thread.covered.mark(Bool.not(List.is_empty(&2, WK.WakeSignal, matches)), matches, id, o, now)

def Wake.thread.fire(+s: M.State, +now: Nat, +id: String, +signals: List<&2, WK.WakeSignal>, +text: String) -> Step:
  +o = WK.Wake.get(s, id)
  +by = J.Json.get_str(o, "by", "")
  +target = J.Json.get_str(o, "target", "")
  Wake.thread.fire.covered(Bool.and(Operational.is(s, by), Bool.and(M.Launch.reports(s, target), String.eq(M.Launch.by(s, target), by))), s, now, id, signals, text)

def Wake.thread.deliver(ids: List<&2, String>, +s: M.State, +now: Nat, +signals: List<&2, WK.WakeSignal>, +text: String, +st: Step) -> Step:
  match ids:
    case Nil{}:
      st
    case id <> t:
      Wake.thread.deliver(t, s, now, signals, text, Step.cat(st, Wake.thread.fire(M.State.replay(Queue.changes(st), s), now, id, signals, text)))

def Wake.thread.after.changed(hit: Bool, +s: M.State, +now: Nat, +th: String, +cs: List<&2, M.Change>, os: List<&2, Out>) -> Step:
  match hit:
    case False{}:
      Step{cs, os}
    case True{}:
      +s1 = M.State.replay(cs, s)
      Wake.thread.deliver(WK.Wake.on(s1, "thread", th), s, now, WK.Wake.signals(Wake.thread.events(s1, th, cs), Wake.thread.end.key(cs, th), Wake.thread.ask.key(cs, th)), Wake.thread.text(s1, th, cs), Step{cs, os})

def Wake.thread.after.st(+s: M.State, +now: Nat, +th: String, st: Step) -> Step:
  match st:
    case Step{+cs, os}:
      Wake.thread.after.changed(Bool.or(Nat.is_gt(Wake.ended(cs, th), 0n), Bool.not(String.is_empty(Wake.thread.ask.key(cs, th)))), s, now, th, cs, os)

def Wake.thread.after.k(ids: List<&2, String>, +s: M.State, +now: Nat, +th: String, st: Step) -> Step:
  match ids:
    case Nil{}:
      st
    case _ <> _:
      Wake.thread.after.st(s, now, th, st)

def Wake.thread.after(+s: M.State, +now: Nat, +th: String, st: Step) -> Step:
  Wake.thread.after.k(WK.Wake.on(s, "thread", th), s, now, th, st)

def Wake.pr.fire(+s: M.State, +now: Nat, +id: String, +target: String, +p: WK.PrSeen) -> Step:
  +events = WK.Wake.pr.unseen(WK.Wake.select(WK.Wake.events(WK.Wake.get(s, id)), WK.Wake.pr.events(p)), s, id, p)
  +st = Wake.fire.k(Bool.and(Nat.is_gt(List.length(&2, String, events), 0n), Bool.and(Hub.owns(s, J.Json.get_str(WK.Wake.get(s, id), "by", "")), Lost.live(Hub.thread(s, J.Json.get_str(WK.Wake.get(s, id), "by", ""))))), s, now, id, target ++ ":" ++ WK.Wake.pr.key(p), events, WK.Wake.pr.text(target, p))
  Step.before(Bool.pick(List<&2, M.Change>, Nat.is_gt(List.length(&2, M.Change, Queue.changes(st)), 0n), WK.Wake.pr.mark(events, id, p, now), Nil{}), Nil{}, st)

def Wake.pr.go(ids: List<&2, String>, +s: M.State, +now: Nat, +target: String, +p: WK.PrSeen, +st: Step) -> Step:
  match ids:
    case Nil{}:
      st
    case id <> t:
      Wake.pr.go(t, s, now, target, p, Step.cat(st, Wake.pr.fire(M.State.replay(Queue.changes(st), s), now, id, target, p)))

def Wake.pr(+s: M.State, +now: Nat, +target: String, +p: WK.PrSeen) -> Step:
  Wake.pr.go(WK.Wake.on(s, "pr", target), s, now, target, p, Step.none())

def Wake.file.one(+s: M.State, +now: Nat, +id: String, +target: String, +stamp: String) -> Step:
  +base = M.Setting.get(s, "wake.base." ++ id, "")
  Step.before(Bool.pick(List<&2, M.Change>, String.is_empty(base), [M.SettingSet{"wake.base." ++ id, stamp, now}], Nil{}), Nil{}, Wake.fire(s, now, id, target ++ ":" ++ stamp, Wake.file.events(base, stamp), "file " ++ target ++ " (" ++ stamp ++ ")"))

def Wake.file.go(ids: List<&2, String>, +s: M.State, +now: Nat, +target: String, +stamp: String, +st: Step) -> Step:
  match ids:
    case Nil{}:
      st
    case id <> t:
      Wake.file.go(t, s, now, target, stamp, Step.cat(st, Wake.file.one(M.State.replay(Queue.changes(st), s), now, id, target, stamp)))

def Wake.file(+s: M.State, +now: Nat, +target: String, +stamp: String) -> Step:
  Wake.file.go(WK.Wake.on(s, "file", target), s, now, target, stamp, Step.none())

def Wake.after.go(cs: List<&2, M.Change>, +s: M.State, +st: Step) -> Step:
  match cs:
    case Nil{}:
      st
    case M.TurnChanged{th, _, at} <> t:
      Wake.after.go(t, s, Wake.thread.after(s, at, th, st))
    case M.AskOpened{_, th, _, _, _, _, at} <> t:
      Wake.after.go(t, s, Wake.thread.after(s, at, th, st))
    case _ <> t:
      Wake.after.go(t, s, st)

def Wake.after(+s: M.State, +st: Step) -> Step:
  Wake.deleted(s, Wake.after.go(Queue.changes(st), s, st))

def Launch.tell.from(+s: M.State, +now: Nat, +thread: String, +from: String, st: Step) -> Step:
  match st:
    case Step{+cs, os}:
      Wake.thread.after(s, now, thread, Lr.tell.k(M.Launch.reports(s, thread), s, now, thread, from, cs, os))

def Launch.tell(+s: M.State, +now: Nat, +thread: String, st: Step) -> Step:
  Launch.tell.from(s, now, thread, "", st)

# a turn still running ends as interrupted, and its task with it; the
# server is told, so a phone's Live Activity drops it (law
# retire_stop_tells); the turn's message is marked stopped, so its last
# reply keeps saying so (core/stamp.bend)
# (a launched thread's launcher hears of it, unless it stopped it itself:
# from, the thread whose call stopped it)
def Retire.turn.f(run: Bool, +s: M.State, +now: Nat, +th: String, +from: String, st: Step) -> Step:
  match run:
    case True{}:
      Launch.tell.from(s, now, th, from, Task.finish(s, now, th, Step.before(M.TurnChanged{th, M.Interrupted{}, now} <> STP.Stamp.stop.mark(s, th, now), [TurnEnded{th, "stopped", "", "", "", ""}], st)))
    case False{}:
      st

def Retire.turn(run: Bool, +s: M.State, +now: Nat, +th: String, st: Step) -> Step:
  Retire.turn.f(run, s, now, th, "", st)

# Explicit stop paths (client and MCP) durably pause the conversation's
# mail FIFO. An internal restart/fallback Interrupted event is not consent
# to clear this register. Keep an undisposed seed at the front for review.
def Run.queue.hold.got(r: Maybe<&2, ML.Run>, +s: M.State, +now: Nat) -> List<&2, M.Change>:
  match r:
    case None{}: Nil{}
    case Some{+r}:
      +input = MD.Disposition.current(s, r)
      +ids = TRI.Triage.queue(s, ML.Run.bot(r), ML.Run.conv(r))
      +next = Bool.pick(List<&2, String>, Bool.or(String.is_empty(input), ML.Parts.has(ids, input, False{})), ids, input <> ids)
      Bool.pick(List<&2, M.Change>, TRI.Triage.owner(s, ML.Run.bot(r)),
        [M.SettingSet{TRI.Triage.hold.key(ML.Run.bot(r), ML.Run.conv(r)), ML.Run.thread(r), now}, TRI.Triage.queue.set(ML.Run.bot(r), ML.Run.conv(r), next, now)], Nil{})

def Run.queue.hold(+s: M.State, +now: Nat, +th: String) -> List<&2, M.Change>:
  Run.queue.hold.got(ML.Box.run_of(M.State.mail(s), th), s, now)

# Stop: the running turn is interrupted (Stop.outs), its asks close
def Retire.stop.f(+s: M.State, +now: Nat, +th: String, +from: String, os: List<&2, Out>) -> Step:
  Retire.turn.f(True{}, s, now, th, from, Step{M.SettingSet{"restart.retry." ++ th, "", now} <> M.SettingSet{"provider.resume." ++ th, "", now} <> List.append(&2, M.Change, Run.queue.hold(s, now, th), Retire.asks(M.Asks.all(s), th, now)),
    List.append(&2, Out, os, List.append(&2, Out, Stop.outs(Hub.provider(Hub.thread(s, th)), th, Hub.id("i", now, Hub.seq(s))), Retire.answers(M.Asks.all(s), th)))})

def Retire.stop(+s: M.State, +now: Nat, +th: String, os: List<&2, Out>) -> Step:
  Retire.stop.f(s, now, th, "", os)

def Hub.interrupt.k(run: Bool, +s: M.State, +now: Nat, +client: U32, +id: String, +thread: String) -> Step:
  match run:
    case True{}:
      Retire.stop(s, now, thread, [Reply.ok(client, id)])
    case False{}:
      Step{[], [Reply.ok(client, id)]}

# archive and delete kill the process whatever it was doing: its asks and
# background rows go, and a running turn ends
def Retire.archive(+s: M.State, +now: Nat, +th: String, base: Step) -> Step:
  Retire.turn(Hub.running(Hub.thread(s, th)), s, now, th,
    Step.also(Step.after(base, List.append(&2, M.Change, Retire.asks(M.Asks.all(s), th, now), Bg.clear(s, th, now))), Retire.answers(M.Asks.all(s), th)))

# the note itself, delivered like any message (Hub.deliver)
def Lost.note(+s: M.State, +now: Nat, +th: String, +text: String) -> Step:
  Hub.inbox.from(s, now, "backplane", th, M.System{}, text, M.Link.id("backplane", Hub.id("l", now, Hub.seq(s))))

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

# watched work of the uberbot's own thread is filed (Operational.file);
# any other thread is told in its chat as before
def Watch.tell.to(owned: Bool, +s: M.State, +now: Nat, +th: String, +text: String) -> Step:
  match owned:
    case True{}: Operational.file(s, now, ML.Origin{"backplane", "system", "work"}, th, text, M.Link.id("backplane", Hub.id("l", now, Hub.seq(s))), Nil{})
    case False{}: Lost.note(s, now, th, text)

def Watch.tell.k(live: Bool, +s: M.State, +now: Nat, +th: String, +text: String) -> Step:
  match live:
    case True{}:
      Watch.tell.to(Operational.is(s, th), s, now, th, text)
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
# A timeout reports uncertainty, but cannot free a lock over live work.
def Locks.work.has.go(names: List<&2, String>, +s: M.State, +id: String) -> Bool:
  match names:
    case Nil{}:
      False{}
    case name <> t:
      Bool.or(LK.Lock.has(M.Locks.get(s, name), id), Locks.work.has.go(t, s, id))

def Locks.work.has(+s: M.State, +id: String) -> Bool:
  Locks.work.has.go(M.Locks.names(s), s, id)

def Watch.timeout.rows(ls: List<&2, String>, +id: String) -> List<&2, String>:
  match ls:
    case Nil{}: Nil{}
    case +l <> rest:
      Bool.pick(String, M.Bg.is(l, id), Watch.line(M.Bg.field(l, 0n), "watching (timed out, still running)", M.Bg.title(l), Watch.kind(l), Watch.target(l), Watch.ident(l), 0n), l) <> Watch.timeout.rows(rest, id)

def Watch.timeout.locked(fresh: Bool, +s: M.State, +now: Nat, +th: String, +id: String, +detail: String) -> Step:
  match fresh:
    case False{}: Step.none()
    case True{}:
      Step.cat(Step{[Watch.put(th, now, Watch.timeout.rows(M.Work.watch(s, th), id))], Nil{}}, Watch.tell(s, now, th, Watch.text(M.Bg.title(Watch.find(M.Work.watch(s, th), id)), "timed out, still running, lock retained", detail)))

def Watch.end.locked(timeout: Bool, +s: M.State, +now: Nat, +th: String, +id: String, +status: String, +detail: String) -> Step:
  match timeout:
    case True{}:
      Watch.timeout.locked(Bool.and(Bool.not(String.is_empty(Watch.find(M.Work.watch(s, th), id))), Bool.not(String.eq(Watch.state(Watch.find(M.Work.watch(s, th), id)), "watching (timed out, still running)"))), s, now, th, id, detail)
    case False{}:
      Watch.end.k(Bool.and(Bool.not(String.is_empty(Watch.find(M.Work.watch(s, th), id))), Bool.or(Bool.not(M.Bg.ended(Watch.find(M.Work.watch(s, th), id))), Bool.and(String.eq(M.Bg.status(Watch.find(M.Work.watch(s, th), id)), "unknown"), Bool.not(String.eq(status, "timeout"))))), String.eq(status, "timeout"), Watch.find(M.Work.watch(s, th), id), s, now, th, id, status, detail)

def Watch.end.raw(+s: M.State, +now: Nat, +th: String, +id: String, +status: String, +detail: String) -> Step:
  Watch.end.locked(Bool.and(String.eq(status, "timeout"), Locks.work.has(s, id)), s, now, th, id, status, detail)

def Watch.end(+s: M.State, +now: Nat, +th: String, +id: String, +status: String, +detail: String) -> Step:
  +st = Watch.end.raw(s, now, th, id, status, detail)
  Wake.deliver(WK.Wake.on(s, "work", id), s, now, id ++ ":" ++ Nat.show(now) ++ ":" ++ status, Bool.pick(List<&2, String>, Nat.is_gt(List.length(&2, M.Change, Queue.changes(st)), 0n), ["end"], Nil{}), "work " ++ id ++ " ended: " ++ status ++ ". " ++ detail, st)

def Watch.cancel.k(live: Bool, +l: String, +s: M.State, +now: Nat, +th: String, +id: String) -> Step:
  match live:
    case True{}:
      Step{[Watch.put(th, now, Watch.set(M.Work.watch(s, th), id, "cancelling"))], [WatchStop{th, id, Watch.kind(l), Watch.target(l), Watch.ident(l)}]}
    case False{}:
      Step.none()

# owned work stopped: the row says "cancelling" until its watcher
# confirms the end (then "cancelled") or that it cannot know (law
# watch_cancel_keeps_until_confirmed)
def Watch.cancel.raw(+s: M.State, +now: Nat, +th: String, +id: String) -> Step:
  Watch.cancel.k(Bool.and(Bool.not(String.is_empty(Watch.find(M.Work.watch(s, th), id))), Bool.not(M.Bg.ended(Watch.find(M.Work.watch(s, th), id)))),
    Watch.find(M.Work.watch(s, th), id), s, now, th, id)

def Watch.cancel.queued(waiting: Bool, +s: M.State, +now: Nat, +th: String, +id: String) -> Step:
  match waiting:
    case True{}:
      Step.cat(Step{[Watch.put(th, now, M.Bg.drop(M.Work.watch(s, th), id))], Nil{}}, Watch.tell(s, now, th, Watch.text(M.Bg.title(Watch.find(M.Work.watch(s, th), id)), "cancelled", "")))
    case False{}:
      Watch.cancel.raw(s, now, th, id)

def Watch.cancel(+s: M.State, +now: Nat, +th: String, +id: String) -> Step:
  Watch.cancel.queued(String.starts_with(Watch.state(Watch.find(M.Work.watch(s, th), id)), "waiting for lock"), s, now, th, id)

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
      Await{th, M.Bg.field(l, 0n), Watch.kind(l), Watch.target(l), Watch.ident(l), Bool.pick(Nat, String.eq(M.Bg.status(l), "unknown"), 0n, Watch.deadline(l)), cwd} <> rest

def Watch.rearm.rows(ls: List<&2, String>, +th: String, +cwd: String) -> List<&2, Out>:
  match ls:
    case Nil{}:
      Nil{}
    case +l <> t:
      Watch.rearm.one(Bool.or(Bool.and(M.Bg.ended(l), Bool.not(String.eq(M.Bg.status(l), "unknown"))), String.starts_with(Watch.state(l), "waiting for lock")), l, th, cwd, Watch.rearm.rows(t, th, cwd))

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

# Deploying
# ---------
# A deploy (core/deploy.bend) waits for no tool call in flight and the
# build locks free, then the hub swaps the staged package in and restarts
# into it; the turns it cuts resume (Carry). deploy.wait is the waiting
# request, deploy.done what the next start reads once (laws deploy_*).

def Deploy.probe() -> List<&2, Out>:
  [DeployProbe{}]

# deploy_main's state (core/ship.bend), and whether a build is on its way.
# A deploy does not wait for one: the build runs in a scope of its own,
# holds no build lock, and the next hub re-adopts it (Ship.recover)
def Ship.get(+s: M.State) -> SH.Ship:
  SH.Ship.read(M.Setting.get(s, "deploy.ship", ""))

def Ship.busy.s(+s: M.State) -> Bool:
  SH.Ship.busy(Ship.get(s))

# a note to the requester thread (none for the person: by is empty)
def Deploy.tell(+s: M.State, +now: Nat, +by: String, +text: String) -> Step:
  Bool.pick(Step, String.is_empty(by), Step.none(), Lr.send(s, now, "deploy", by, text))

def Deploy.stage.ok(+stage: String) -> Bool:
  Bool.or(String.is_empty(stage), String.starts_with(stage, "/"))

def Deploy.ask(+s: M.State, +now: Nat, +by: String, +note: String, +stage: String) -> Step:
  Step{[M.SettingSet{"deploy.wait", DP.Deploy.line(by, note, stage, VER.Version.current(), now, False{}, False{}, ""), now}], Deploy.probe()}

def Deploy.restart(ok: Bool, +s: M.State, +now: Nat, +client: U32, +id: String, +stage: String, +note: String) -> Step:
  match ok:
    case True{}:
      Step.before(Nil{}, [Reply.ok(client, id)], Deploy.ask(s, now, "", note, stage))
    case False{}:
      Step.outs([Reply.err(client, id, "stage must be an absolute path")])

def Deploy.cancel(+s: M.State, +now: Nat) -> Step:
  Step{[M.SettingSet{"deploy.wait", "", now}], Nil{}}

# the go: deploy.done is written (with the threads this cuts) before the
# Out, and the wait cleared, in one step
def Deploy.go(+line: String, +s: M.State, +now: Nat) -> Step:
  Step{[M.SettingSet{"deploy.done", DP.Deploy.line(DP.Deploy.by(line), DP.Deploy.note(line), DP.Deploy.stage(line), DP.Deploy.from(line),
      DP.Deploy.since(line), DP.Deploy.noted(line), DP.Deploy.locked(line), DP.Deploy.running(s)), now}, M.SettingSet{"deploy.wait", "", now}],
    [DeployGo{DP.Deploy.stage(line)}]}

def Deploy.lock.set(same: Bool, free: Bool, +line: String, +open: String, +now: Nat) -> Step:
  match same:
    case True{}:
      Step.none()
    case False{}:
      Step{[M.SettingSet{"deploy.wait", DP.Deploy.with(line, DP.Deploy.noted(line), Bool.not(free), open), now}], Nil{}}

def Deploy.check.k(go: Bool, +free: Bool, +line: String, +open: String, +s: M.State, +now: Nat) -> Step:
  match go:
    case True{}:
      Deploy.go(line, s, now)
    case False{}:
      Deploy.lock.set(Bool.and(Bool.pick(Bool, free, Bool.not(DP.Deploy.locked(line)), DP.Deploy.locked(line)), String.eq(DP.Deploy.open(line), open)), free, line, open, now)

def Deploy.check.w(waiting: Bool, +clear: Bool, +free: Bool, +open: String, +s: M.State, +now: Nat) -> Step:
  match waiting:
    case True{}:
      Deploy.check.k(Bool.and(clear, free), free, DP.Deploy.wait(s), open, s, now)
    case False{}:
      Step.none()

# the build locks were looked at (free: both are): goes when no tool call is
# open (the hub's in-memory list) and they are free, else remembers whether
# a lock holds it and what is open, for the plane and deploy_status
def Deploy.check(+s: M.State, +now: Nat, free: Bool, +open: List<&2, String>) -> Step:
  Deploy.check.w(DP.Deploy.waiting(s), Nat.is_eq(DP.Open.count(open), 0n), free, DP.Open.threads(open), s, now)

def Deploy.slow.k(due: Bool, +line: String, +s: M.State, +now: Nat) -> Step:
  match due:
    case True{}:
      Step.cat(Step{[M.SettingSet{"deploy.wait", DP.Deploy.with(line, True{}, DP.Deploy.locked(line), DP.Deploy.open(line)), now}], Nil{}},
        Deploy.tell(s, now, DP.Deploy.by(line), DP.Deploy.slow.text(DP.Deploy.count(line), DP.Deploy.locked(line))))
    case False{}:
      Step.none()

def Deploy.tick.w(waiting: Bool, +s: M.State, +now: Nat) -> Step:
  match waiting:
    case True{}:
      Step.cat(Deploy.slow.k(Bool.and(Bool.not(DP.Deploy.noted(DP.Deploy.wait(s))), Nat.is_ge(now, Nat.add(DP.Deploy.since(DP.Deploy.wait(s)), DP.Deploy.slow()))), DP.Deploy.wait(s), s, now),
        Step.outs(Deploy.probe()))
    case False{}:
      Step.none()

# the tick's look at a waiting deploy
def Deploy.tick(+s: M.State, +now: Nat) -> Step:
  Deploy.tick.w(DP.Deploy.waiting(s), s, now)

def Deploy.after.w(go: Bool, +line: String, +s: M.State, +now: Nat) -> Step:
  match go:
    case True{}:
      Step.cat(Step{[M.SettingSet{"deploy.done", "", now}], Nil{}},
        Bool.pick(Step, DP.Deploy.in(line, DP.Deploy.by(line)), Step.none(), Deploy.tell(s, now, DP.Deploy.by(line), DP.Deploy.after.text(line, VER.Version.current()))))
    case False{}:
      Step.none()

# at start: a deploy that restarted the hub is told once to its requester
# (the threads it cut got the words with their continue message), and
# deploy.done is cleared
def Deploy.after(+s: M.State, +now: Nat) -> Step:
  Deploy.after.w(Bool.not(String.is_empty(DP.Deploy.done(s))), DP.Deploy.done(s), s, now)

# the swap failed: nothing restarted
def Deploy.failed(+s: M.State, +now: Nat, +err: String) -> Step:
  Step.cat(Step{[M.SettingSet{"deploy.done", "", now}], Nil{}},
    Deploy.tell(s, now, DP.Deploy.by(DP.Deploy.done(s)), "[Backplane] Your deploy could not be installed (" ++ err ++ "). Nothing was restarted."))

# demos for the laws and tests (t0 asks; t1 runs, inside a tool call or not)
def Deploy.demo.line(+by: String) -> String:
  DP.Deploy.line(by, "dev.sh", "", "0.9.0", 5n, False{}, False{}, "t1")

def Deploy.demo.changes(tool: Bool, +key: String, +line: String) -> List<&2, M.Change>:
  List.concat(&2, M.Change, [[M.ProjectCreated{"p1", "p", "/p", 1n}, M.ThreadCreated{"t0", "p1", "boss", M.Local{}, "", "", "claude", "", 1n},
    M.ThreadCreated{"t1", "p1", "work", M.Local{}, "", "", "claude", "", 1n}, M.TurnChanged{"t1", M.Running{}, 3n}],
    Bool.pick(List<&2, M.Change>, tool, [M.ActivityLogged{"t1", "a1", "tool", "Bash", "make", 4n}], Nil{}),
    Bool.pick(List<&2, M.Change>, String.is_empty(line), Nil{}, [M.SettingSet{key, line, 5n}])])

# tool: t1's newest entry is a tool call (the log shows nothing of whether
# it is open: that is the hub's list, Deploy.demo.open); the line is deploy.wait
def Deploy.demo(tool: Bool, +line: String) -> M.State:
  M.State.replay(Deploy.demo.changes(tool, "deploy.wait", line), M.State.empty())

# the hub's open calls: t1 is inside one, or between two
def Deploy.demo.open(open: Bool) -> List<&2, String>:
  Bool.pick(List<&2, String>, open, DP.Open.add(Nil{}, "t1", "a1"), Nil{})

# the state at a start after a deploy by `by` (deploy.done set)
def Deploy.demo.done(+by: String) -> M.State:
  M.State.replay(Deploy.demo.changes(False{}, "deploy.done", Deploy.demo.line(by)), M.State.empty())

def Deploy.demo.gos(os: List<&2, Out>) -> Nat:
  match os:
    case Nil{}:
      0n
    case DeployGo{_} <> t:
      1n+Deploy.demo.gos(t)
    case _ <> t:
      Deploy.demo.gos(t)

def Deploy.demo.go(st: Step) -> Nat:
  match st:
    case Step{_, os}:
      Deploy.demo.gos(os)

# the settings a step sets to nothing (key), and to something
def Deploy.demo.sets(cs: List<&2, M.Change>, +key: String, +empty: Bool) -> Nat:
  match cs:
    case Nil{}:
      0n
    case M.SettingSet{+k, +v, _} <> t:
      Nat.add(Bool.pick(Nat, Bool.and(String.eq(k, key), Bool.pick(Bool, empty, String.is_empty(v), Bool.not(String.is_empty(v)))), 1n, 0n), Deploy.demo.sets(t, key, empty))
    case _ <> t:
      Deploy.demo.sets(t, key, empty)

def Deploy.demo.cs(st: Step) -> List<&2, M.Change>:
  match st:
    case Step{cs, _}:
      cs

# messages a step posts
def Deploy.demo.posts(cs: List<&2, M.Change>) -> Nat:
  match cs:
    case Nil{}:
      0n
    case M.MessagePosted{_, _, _, _, _} <> t:
      1n+Deploy.demo.posts(t)
    case _ <> t:
      Deploy.demo.posts(t)

def Deploy.demo.notes(st: Step) -> Nat:
  Deploy.demo.posts(Deploy.demo.cs(st))

# the step again over the state it made
def Deploy.demo.again(+s: M.State, st: Step) -> M.State:
  M.State.replay(Deploy.demo.cs(st), s)

# a bot (its thread "tb", no project) asked
def Deploy.demo.bot() -> M.State:
  M.State.replay([M.ProjectCreated{"p1", "p", "/p", 1n}, M.ThreadCreated{"tb", "", "Bub", M.Local{}, "", "", "claude", "", 1n}, M.BotSet{"b1", "Bub", "tb", 0n, "", 1n},
    M.SettingSet{"deploy.done", Deploy.demo.line("tb"), 5n}], M.State.empty())

# Deploying origin/main
# ---------------------
# deploy_main (core/ship.bend, docs/deploy.md): the hub fetches origin/main
# of deploy.repo, builds it in a job of its own (ShipRun) and stages it
# through the safe restart above (Deploy.ask). One build at a time; a
# request the build, the staged or the live commit covers starts none; the
# requests a build does not cover make one follow-up (ShipNext). Everyone
# who asked is told once when the build ends: a thread like watched work
# (Watch.tell), the person ("-") with an alert.

def Ship.repo(+s: M.State) -> String:
  M.Setting.get(s, "deploy.repo", "")

def Ship.wrap(+s: M.State) -> String:
  M.Setting.get(s, "deploy.wrap", "")

def Ship.follow(+s: M.State) -> Bool:
  SH.Ship.follow.on(M.Setting.get(s, "deploy.follow", ""))

# who asked: the MCP route's "deploy" is no thread (a script, the person)
def Ship.by(+thread: String) -> String:
  Bool.pick(String, Bool.or(String.eq(thread, "deploy"), String.is_empty(thread)), SH.Ship.person(), thread)

def Ship.eff.k(waiting: Bool, sh: SH.Ship) -> SH.Ship:
  match waiting:
    case True{}:
      sh
    case False{}:
      SH.Ship.unstage(sh)

# the state as it covers requests: a staged commit only while its deploy
# still waits
def Ship.eff(+s: M.State) -> SH.Ship:
  Ship.eff.k(DP.Deploy.waiting(s), Ship.get(s))

# the commits the server asks git about, comma-joined
def Ship.cands(+s: M.State) -> String:
  String.join(SH.Ship.cands(Ship.eff(s)), ",")

# the person's alert (no thread: its record is alert.deploy)
def Ship.alert(+s: M.State, +now: Nat, +text: String) -> Step:
  Step{[NO.Alert.set(s, now, "deploy", "note", "deploy-" ++ Hub.id("n", now, Hub.seq(s)), NO.Alert.memory(s, "deploy"), NO.Alert.text(text, 500n))], Nil{}}

def Ship.tell.k(person: Bool, +s: M.State, +now: Nat, +who: String, +text: String) -> Step:
  match person:
    case True{}:
      Ship.alert(s, now, text)
    case False{}:
      Watch.tell(s, now, who, text)

# each asker told once, the state moving between them
def Ship.tells(ws: List<&2, String>, +s: M.State, +now: Nat, +text: String, +st: Step) -> Step:
  match ws:
    case Nil{}:
      st
    case +w <> t:
      Ship.tells(t, s, now, text, Step.cat(st, Ship.tell.k(String.eq(w, SH.Ship.person()), M.State.replay(Queue.changes(st), s), now, w, text)))

def Ship.more.outs(more: Bool) -> List<&2, Out>:
  match more:
    case True{}:
      [ShipNext{"next"}]
    case False{}:
      Nil{}

# a build ends (staged, failed, skipped): written, its askers told, and the
# follow-up asked for when one is wanted
def Ship.end(+s: M.State, +now: Nat, +commit: String, +state: String, +version: String, +detail: String) -> Step:
  Ship.tells(SH.Ship.told(Ship.get(s)), s, now, SH.Ship.text(state, commit, version, detail),
    Step{[M.DeployBuilt{commit, state, version, detail, now}], Ship.more.outs(SH.Ship.more(Ship.get(s)))})

def Ship.ask.k(start: Bool, +now: Nat, +by: String, +note: String, +head: String, +cover: String) -> Step:
  match start:
    case True{}:
      Step{[M.DeployAsked{by, note, head, cover, now}, M.DeployBuilt{head, "building", "", "", now}], [ShipRun{head}]}
    case False{}:
      Step{[M.DeployAsked{by, note, head, cover, now}], Nil{}}

# a request for head (within: the candidate commits git says contain it)
def Ship.ask(+s: M.State, +now: Nat, +by: String, +note: String, +head: String, +within: List<&2, String>) -> Step:
  Ship.ask.k(SH.Ship.start(Ship.eff(s), head, within), now, by, note, head, SH.Ship.cover(Ship.eff(s), head, within))

def Ship.next.c(+cover: String, +s: M.State, +now: Nat, +head: String) -> Step:
  match cover:
    case SNil{}:
      Step{[M.DeployBuilt{head, "building", "", "", now}], [ShipRun{head}]}
    case SCon{_, _}:
      Ship.end(s, now, SH.Ship.by(Ship.eff(s), cover), "skipped", "", cover)

def Ship.next.e(+err: String, +s: M.State, +now: Nat, +out: String) -> Step:
  match err:
    case SNil{}:
      Ship.next.c(SH.Ship.cover(Ship.eff(s), SH.Look.head(out), SH.Look.within(out)), s, now, SH.Look.head(out))
    case SCon{_, _}:
      Ship.end(s, now, SH.Ship.commit(Ship.get(s)), "failed", "", "origin/main could not be read: " ++ err)

def Ship.next.k(asked: Bool, +s: M.State, +now: Nat, +out: String) -> Step:
  match asked:
    case True{}:
      Ship.next.e(SH.Look.err(out), s, now, out)
    case False{}:
      Step.none()

def Ship.follow.k(go: Bool, +s: M.State, +now: Nat, +out: String) -> Step:
  match go:
    case True{}:
      Ship.ask(s, now, SH.Ship.person(), "deploy.follow", SH.Look.head(out), SH.Look.within(out))
    case False{}:
      Step.none()

def Ship.newest.k(follow: Bool, +s: M.State, +now: Nat, +out: String) -> Step:
  match follow:
    case True{}:
      Ship.follow.k(Bool.and(String.is_empty(SH.Look.err(out)), SH.Ship.start(Ship.eff(s), SH.Look.head(out), SH.Look.within(out))), s, now, out)
    case False{}:
      Ship.next.k(String.eq(SH.Ship.phase(Ship.get(s)), "asked"), s, now, out)

# origin/main read again (why: "next" for the follow-up, "follow" for
# deploy.follow's look, which only builds what nothing covers and asks no
# one)
def Ship.newest(+s: M.State, +now: Nat, +why: String, +out: String) -> Step:
  Ship.newest.k(String.eq(why, "follow"), s, now, out)

def Ship.mine(+s: M.State, +commit: String) -> Bool:
  Bool.and(String.eq(SH.Ship.phase(Ship.get(s)), "building"), String.eq(SH.Ship.commit(Ship.get(s)), commit))

def Ship.pid.k(mine: Bool, +now: Nat, +commit: String, +detail: String) -> Step:
  match mine:
    case True{}:
      Step{[M.DeployBuilt{commit, "pid", "", detail, now}], Nil{}}
    case False{}:
      Step.none()

# the build's job started: its pid and what proves it is the same process
def Ship.pid(+s: M.State, +now: Nat, +commit: String, +detail: String) -> Step:
  Ship.pid.k(Ship.mine(s, commit), now, commit, detail)

def Ship.done.ok(ok: Bool, +s: M.State, +now: Nat, +commit: String, +out: String) -> Step:
  match ok:
    case True{}:
      Step.cat(Ship.end(s, now, commit, "staged", SH.Ran.version(out), ""), Deploy.ask(s, now, "", SH.Ship.note(commit), SH.Ran.stage(out)))
    case False{}:
      Ship.end(s, now, commit, "failed", SH.Ran.version(out),
        Bool.pick(String, SH.Ran.gone(out), SH.Ship.lost() ++ "\n", "") ++ SH.Ran.tail(out))

def Ship.done.k(mine: Bool, +s: M.State, +now: Nat, +commit: String, +out: String) -> Step:
  match mine:
    case True{}:
      Ship.done.ok(SH.Ran.ok(out), s, now, commit, out)
    case False{}:
      Step.none()

# the build's job ended (out: its STATUS, VERSION, STAGE and log tail): a
# staged package goes to the safe restart; a job gone without an exit
# status is lost, never success
def Ship.done(+s: M.State, +now: Nat, +commit: String, +out: String) -> Step:
  Ship.done.k(Ship.mine(s, commit), s, now, commit, out)

def Ship.live.k(go: Bool, +now: Nat, +staged: String) -> Step:
  match go:
    case True{}:
      Step{[M.DeployBuilt{staged, "live", VER.Version.current(), "", now}], Nil{}}
    case False{}:
      Step.none()

def Ship.recover.k(i: Nat, +s: M.State, +now: Nat) -> Step:
  match i:
    case 0n:
      Step.outs([ShipRun{SH.Ship.commit(Ship.get(s))}])
    case 1n:
      Step.outs([ShipNext{"next"}])
    case _:
      Ship.live.k(Bool.and(Bool.not(String.is_empty(SH.Ship.staged(Ship.get(s)))), String.eq(DP.Deploy.note(DP.Deploy.done(s)), SH.Ship.note(SH.Ship.staged(Ship.get(s))))),
        now, SH.Ship.staged(Ship.get(s)))

# at start: a build that was under way is watched again (its job outlives
# the hub: it reports its exit, or is lost only when it is gone without
# one); a follow-up still wanted is asked for; a staged build this restart
# was for is live
def Ship.recover(+s: M.State, +now: Nat) -> Step:
  Ship.recover.k(Bool.pick(Nat, String.eq(SH.Ship.phase(Ship.get(s)), "building"), 0n, Bool.pick(Nat, String.eq(SH.Ship.phase(Ship.get(s)), "asked"), 1n, 2n)), s, now)

def Ship.tick.k(go: Bool) -> Step:
  match go:
    case True{}:
      Step.outs([ShipNext{"follow"}])
    case False{}:
      Step.none()

# the minute's tick: with deploy.follow on (off by default) and nothing
# under way, origin/main is read on every minute's tick (SH.Ship.every)
def Ship.tick(+s: M.State, +now: Nat) -> Step:
  Ship.tick.k(Bool.and(Ship.follow(s), Bool.and(Bool.not(String.is_empty(Ship.repo(s))), Bool.and(Bool.not(Ship.busy.s(s)), SH.Ship.due(now)))))

def Ship.status(+s: M.State) -> J.Json:
  SH.Ship.status(Ship.get(s), Ship.follow(s), Ship.repo(s))

# demos for the laws and tests: a project with threads t1, t2
def Ship.demo(+line: String) -> M.State:
  M.State.replay([M.ProjectCreated{"p1", "p", "/p", 1n}, M.ThreadCreated{"t1", "p1", "a", M.Local{}, "", "", "claude", "", 1n},
    M.ThreadCreated{"t2", "p1", "b", M.Local{}, "", "", "claude", "", 1n}, M.SettingSet{"deploy.repo", "/src", 1n}, M.SettingSet{"deploy.ship", line, 2n}], M.State.empty())

def Ship.demo.runs(os: List<&2, Out>) -> Nat:
  match os:
    case Nil{}:
      0n
    case ShipRun{_} <> t:
      1n+Ship.demo.runs(t)
    case _ <> t:
      Ship.demo.runs(t)

def Ship.demo.nexts(os: List<&2, Out>) -> Nat:
  match os:
    case Nil{}:
      0n
    case ShipNext{_} <> t:
      1n+Ship.demo.nexts(t)
    case _ <> t:
      Ship.demo.nexts(t)

def Ship.demo.outs(st: Step) -> List<&2, Out>:
  match st:
    case Step{_, os}:
      os

# builds a step starts
def Ship.demo.builds(st: Step) -> Nat:
  Ship.demo.runs(Ship.demo.outs(st))

def Ship.demo.posts.go(cs: List<&2, M.Change>, +th: String) -> Nat:
  match cs:
    case Nil{}:
      0n
    case M.MessagePosted{+t, _, _, _, _} <> r:
      Nat.add(Bool.pick(Nat, String.eq(t, th), 1n, 0n), Ship.demo.posts.go(r, th))
    case _ <> r:
      Ship.demo.posts.go(r, th)

# messages a step posts into thread th
def Ship.demo.posts(st: Step, +th: String) -> Nat:
  Ship.demo.posts.go(Queue.changes(st), th)

# Open tool calls, for the laws: the outs of a Claude line on a running thread
def Tools.demo.os(st: Step) -> List<&2, Out>:
  match st:
    case Step{_, os}:
      os

def Tools.demo.outs(+line: String) -> List<&2, Out>:
  Tools.demo.os(Claude.line.parsed(J.Json.parse(line), Deploy.demo(False{}, ""), "t1", 9n))

def Tools.demo.use() -> String:
  "{\"type\":\"assistant\",\"message\":{\"id\":\"m1\",\"content\":[{\"type\":\"tool_use\",\"id\":\"toolu_1\",\"name\":\"Bash\",\"input\":{\"command\":\"make\"}}]}}"

def Tools.demo.result() -> String:
  "{\"type\":\"user\",\"message\":{\"content\":[{\"type\":\"tool_result\",\"tool_use_id\":\"toolu_1\",\"content\":\"ok\"}]}}"

# how many opens (shut: False) or shuts (True) of a call
def Tools.demo.count(os: List<&2, Out>, +shut: Bool, +call: String) -> Nat:
  match os:
    case Nil{}:
      0n
    case ToolOpen{_, +c} <> t:
      Nat.add(Bool.pick(Nat, Bool.and(Bool.not(shut), String.eq(c, call)), 1n, 0n), Tools.demo.count(t, shut, call))
    case ToolShut{_, +c} <> t:
      Nat.add(Bool.pick(Nat, Bool.and(shut, String.eq(c, call)), 1n, 0n), Tools.demo.count(t, shut, call))
    case _ <> t:
      Tools.demo.count(t, shut, call)

# the tool_use line opens its call once and shuts nothing; its tool_result
# line shuts the same call once and opens nothing: 0 + 0 + 10 + 1 = 11
def Tools.demo.pair() -> Nat:
  Nat.add(Nat.add(Nat.mul(1000n, Tools.demo.count(Tools.demo.outs(Tools.demo.use()), True{}, "toolu_1")), Nat.mul(100n, Tools.demo.count(Tools.demo.outs(Tools.demo.result()), False{}, "toolu_1"))),
    Nat.add(Nat.mul(10n, Tools.demo.count(Tools.demo.outs(Tools.demo.use()), False{}, "toolu_1")), Tools.demo.count(Tools.demo.outs(Tools.demo.result()), True{}, "toolu_1")))

# a turn's end: the ToolsGone outs of one thread
def Tools.demo.ends() -> Nat:
  List.length(&2, Out, Tools.outs([M.TurnChanged{"t1", M.Completed{}, 5n}], Nil{}))

# harness-coord: locks. Grants and work launches share the log write boundary.
def Locks.answer(+l: LK.NamedLock, +id: String) -> J.Json:
  J.Json.obj([J.KV{"held", J.Flag{Bool.and(String.eq(LK.Lock.owner(l), id), LK.Lock.ready(l))}}, J.KV{"position", J.Num{Nat.show(LK.Lock.position(l, id))}}, J.KV{"holder", J.Str{LK.Lock.owner(l)}}])

# A client the record had lost is queued again, and its answer says so.
def Locks.answer.re(+l: LK.NamedLock, +id: String, requeued: Bool) -> J.Json:
  Bool.pick(J.Json, requeued, J.Json.obj([J.KV{"held", J.Flag{Bool.and(String.eq(LK.Lock.owner(l), id), LK.Lock.ready(l))}}, J.KV{"position", J.Num{Nat.show(LK.Lock.position(l, id))}}, J.KV{"holder", J.Str{LK.Lock.owner(l)}}, J.KV{"requeued", J.Flag{True{}}}]), Locks.answer(l, id))

def Locks.list.go(names: List<&2, String>, +s: M.State) -> List<&2, J.Json>:
  match names:
    case Nil{}:
      Nil{}
    case +name <> t:
      J.Json.obj(List.append(&2, J.KV, [J.KV{"name", J.Str{name}}, J.KV{"holder", LK.Lock.holder.json(LK.Lock.holder(M.Locks.get(s, name)))}, J.KV{"waiters", J.Json.arr(LK.Lock.parties.json(LK.Lock.waiters(M.Locks.get(s, name))))}, J.KV{"ready", J.Flag{LK.Lock.ready(M.Locks.get(s, name))}}], LK.Lock.outside.kv(LK.Lock.outside(M.Locks.get(s, name))))) <> Locks.list.go(t, s)

def Locks.put(+s: M.State, +now: Nat, +name: String, +l: LK.NamedLock) -> Step:
  Step{[M.LockChanged{name, l, now}], Nil{}}

def Locks.acquire(+s: M.State, +now: Nat, +name: String, +party: LK.LockParty, +ready: Bool) -> Step:
  Bool.pick(Step, LK.Lock.has(M.Locks.get(s, name), LK.Lock.id(party)), Step.none(), Locks.put(s, now, name, LK.Lock.acquire(M.Locks.get(s, name), party, ready)))

def Locks.release.k(own: Bool, +s: M.State, +now: Nat, +name: String, +id: String) -> Step:
  match own:
    case True{}:
      Locks.put(s, now, name, LK.Lock.release(M.Locks.get(s, name), id))
    case False{}:
      Step.none()

def Locks.release(+s: M.State, +now: Nat, +name: String, +id: String) -> Step:
  Locks.release.k(String.eq(LK.Lock.owner(M.Locks.get(s, name)), id), s, now, name, id)

def Locks.cancel(+s: M.State, +now: Nat, +name: String, +id: String) -> Step:
  Locks.put(s, now, name, LK.Lock.cancel(M.Locks.get(s, name), id))

# The shell helper's three calls. A renew or status from a client the record no
# longer holds queues it again (like acquire), never an invisible held:false.
def Locks.http.party(+holder: String, +note: String, +now: Nat) -> LK.LockParty:
  LK.LockParty{holder, "client", "", note, now, Nat.add(now, 90n)}

def Locks.http.lost(op: String, +l: LK.NamedLock, +holder: String) -> Bool:
  Bool.and(M.Launch.has(["renew", "status"], op), Bool.not(LK.Lock.has(l, holder)))

def Locks.http.step(op: String, +s: M.State, +now: Nat, +name: String, +holder: String, +note: String, +ready: Bool) -> Step:
  match op:
    case "acquire":
      Locks.put(s, now, name, LK.Lock.renew(LK.Lock.acquire(M.Locks.get(s, name), Locks.http.party(holder, note, now), ready), holder, Nat.add(now, 90n)))
    case "renew":
      Locks.put(s, now, name, LK.Lock.renew(LK.Lock.acquire(M.Locks.get(s, name), Locks.http.party(holder, note, now), ready), holder, Nat.add(now, 90n)))
    case "status":
      Locks.acquire(s, now, name, Locks.http.party(holder, note, now), ready)
    case "release":
      Locks.release(s, now, name, holder)
    case "cancel":
      Locks.cancel(s, now, name, holder)
    case _:
      Step.none()

def Locks.work.start(+s: M.State, +now: Nat, +th: String, +a: J.Json, +ready: Bool) -> Step:
  +name = J.Json.get_str(a, "lock", "")
  +id = Watch.id(s, th)
  +party = {LK.LockParty{id, "work", th, J.Json.get_str(a, "note", J.Json.get_str(a, "command", "")), now, 0n} : LK.LockParty}
  +l = LK.Lock.acquire(M.Locks.get(s, name), party, ready)
  +held = Bool.and(String.eq(LK.Lock.owner(l), id), LK.Lock.ready(l))
  Step{[M.LockChanged{name, l, now}, Watch.put(th, now, List.append(&2, String, M.Work.watch(s, th), [Watch.line(id, Bool.pick(String, held, "launching", LK.Lock.wait.word(name, LK.Lock.position(l, id))), "" ++ LK.Lock.note(party), "run", J.Json.get_str(a, "command", ""), "-", Watch.deadline.of(now, WorkTimeout.seconds(s, a)))]))],
    Bool.pick(List<&2, Out>, held, [Await{th, id, "run", J.Json.get_str(a, "command", ""), "-", Watch.deadline.of(now, WorkTimeout.seconds(s, a)), Hub.cwd(s, Hub.thread(s, th))}], Nil{})}

def Locks.work.grant(+s: M.State, +now: Nat, +p: LK.LockParty) -> Step:
  +th = LK.Lock.thread(p)
  +id = LK.Lock.id(p)
  +l = Watch.find(M.Work.watch(s, th), id)
  Step{[Watch.put(th, now, Watch.set(M.Work.watch(s, th), id, "launching"))], [Await{th, id, Watch.kind(l), Watch.target(l), Watch.ident(l), Watch.deadline(l), Hub.cwd(s, Hub.thread(s, th))}]}

def Locks.grant.kind(kind: String, +s: M.State, +now: Nat, +name: String, +p: LK.LockParty) -> Step:
  match kind:
    case "thread":
      Lr.send(s, now, "lock:" ++ name, LK.Lock.thread(p), "Lock " ++ name ++ " is yours (waited " ++ M.Time.ago(now, LK.Lock.since(p)) ++ "). Release it with lock_release when done.")
    case "work":
      Locks.work.grant(s, now, p)
    case _:
      Step.none()

def Locks.grant(needed: Bool, h: Maybe<&2, LK.LockParty>, +s: M.State, +now: Nat, +name: String) -> Step:
  match needed h:
    case True{} Some{+p}:
      Locks.grant.kind(LK.Lock.kind(p), s, now, name, p)
    case _ _:
      Step.none()

def Locks.ready.k(ok: Bool, +s: M.State, +now: Nat, +name: String) -> Step:
  match ok:
    case True{}:
      Locks.put(s, now, name, LK.Lock.mark(M.Locks.get(s, name)))
    case False{}:
      Step.none()

def Locks.ready(+s: M.State, +now: Nat, +name: String, +id: String) -> Step:
  Locks.ready.k(Bool.and(String.eq(LK.Lock.owner(M.Locks.get(s, name)), id), Bool.not(LK.Lock.ready(M.Locks.get(s, name)))), s, now, name)

def Locks.outside.same(old: Maybe<&2, LK.LockParty>, +pid: Nat, +cmd: String) -> Bool:
  match old:
    case Some{LK.LockParty{+id, _, _, +note, _, _}}:
      Bool.and(String.eq(id, Nat.show(pid)), String.eq(note, cmd))
    case None{}:
      False{}

# The hub could not take the guard: who holds the flock outside it (pid 0 = unknown).
def Locks.outside.k(ok: Bool, +l: LK.NamedLock, +old: Maybe<&2, LK.LockParty>, +s: M.State, +now: Nat, +name: String, +pid: Nat, +cmd: String) -> Step:
  match ok:
    case False{}:
      Step.none()
    case True{}:
      +same = Bool.pick(Bool, Nat.is_eq(pid, 0n), False{}, Locks.outside.same(old, pid, cmd))
      +o = {Bool.pick(Maybe<&2, LK.LockParty>, Nat.is_eq(pid, 0n), None{}, Bool.pick(Maybe<&2, LK.LockParty>, same, old, Some{LK.LockParty{Nat.show(pid), "outside", "", cmd, now, 0n}})) : Maybe<&2, LK.LockParty>}
      Bool.pick(Step, String.eq(J.Json.show(LK.Lock.json(LK.Lock.outside.set(l, o))), J.Json.show(LK.Lock.json(l))), Step.none(), Locks.put(s, now, name, LK.Lock.outside.set(l, o)))

def Locks.outside(+s: M.State, +now: Nat, +name: String, +id: String, +pid: Nat, +cmd: String) -> Step:
  +l = M.Locks.get(s, name)
  Locks.outside.k(Bool.and(String.eq(LK.Lock.owner(l), id), Bool.not(LK.Lock.ready(l))), l, LK.Lock.outside(l), s, now, name, pid, cmd)

def Locks.live(+s: M.State, +now: Nat, +p: LK.LockParty) -> Bool:
  Bool.pick(Bool, String.eq(LK.Lock.kind(p), "client"), Nat.is_gt(LK.Lock.lease(p), now),
    Bool.and(Hub.owns(s, LK.Lock.thread(p)), Bool.and(Lost.live(Hub.thread(s, LK.Lock.thread(p))),
      Bool.pick(Bool, String.eq(LK.Lock.kind(p), "work"), Bool.and(Bool.not(String.is_empty(Watch.find(M.Work.watch(s, LK.Lock.thread(p)), LK.Lock.id(p)))), Bool.not(M.Bg.ended(Watch.find(M.Work.watch(s, LK.Lock.thread(p)), LK.Lock.id(p))))), True{}))))

def Locks.prune.q(q: List<&2, LK.LockParty>, +s: M.State, +now: Nat) -> List<&2, LK.LockParty>:
  match q:
    case Nil{}:
      Nil{}
    case +p <> t:
      List.append(&2, LK.LockParty, Bool.pick(List<&2, LK.LockParty>, Locks.live(s, now, p), [p], Nil{}), Locks.prune.q(t, s, now))

def Locks.prune.h(h: Maybe<&2, LK.LockParty>, +q: List<&2, LK.LockParty>, +r: Bool, o: Maybe<&2, LK.LockParty>, +s: M.State, +now: Nat) -> LK.NamedLock:
  match h:
    case None{}:
      LK.Lock.promote(q)
    case Some{+p}:
      +k = {LK.NamedLock{Some{p}, q, r, o} : LK.NamedLock}
      Bool.pick(LK.NamedLock, Locks.live(s, now, p), k, LK.Lock.expire(k, LK.Lock.id(p)))

def Locks.prune(l: LK.NamedLock, +s: M.State, +now: Nat) -> LK.NamedLock:
  match l:
    case LK.NamedLock{h, q, r, o}:
      Locks.prune.h(h, Locks.prune.q(q, s, now), r, o, s, now)

def Locks.wait.work(same: Bool, +s: M.State, +now: Nat, +th: String, +id: String, +word: String) -> Step:
  match same:
    case True{}:
      Step.none()
    case False{}:
      Step{[Watch.put(th, now, Watch.set(M.Work.watch(s, th), id, word))], Nil{}}

def Locks.wait.party(work: Bool, +p: LK.LockParty, +s: M.State, +now: Nat, +name: String, +n: Nat) -> Step:
  match work:
    case True{}:
      +word = LK.Lock.wait.word(name, n)
      Locks.wait.work(String.eq(Watch.state(Watch.find(M.Work.watch(s, LK.Lock.thread(p)), LK.Lock.id(p))), word), s, now, LK.Lock.thread(p), LK.Lock.id(p), word)
    case False{}:
      Step.none()

def Locks.wait.go(q: List<&2, LK.LockParty>, +s: M.State, +now: Nat, +name: String, +n: Nat, +st: Step) -> Step:
  match q:
    case Nil{}:
      st
    case +p <> t:
      Locks.wait.go(t, s, now, name, 1n+n, Step.cat(st, Locks.wait.party(String.eq(LK.Lock.kind(p), "work"), p, M.State.replay(Queue.changes(st), s), now, name, n)))

# Lock heartbeats come only from real progress, never from the watchdog's
# own nudge, a waiting poll or the hub's periodic heartbeat.
def Locks.lease.number(s: String, valid: Bool, +acc: Nat) -> Nat:
  match s valid:
    case _ False{}: 0n
    case SNil{} True{}: acc
    case SCon{+c, t} True{}:
      Locks.lease.number(t, Char.is_digit(c), Nat.add(Nat.mul(acc, 10n), U32.to_nat((Char.to_u32(c) - 48 : U32))))

def Locks.lease.seconds.k(+minutes: Nat) -> Nat:
  Nat.mul(Bool.pick(Nat, Nat.is_eq(minutes, 0n), 20n, minutes), 60n)

def Locks.lease.seconds(+s: M.State) -> Nat:
  Locks.lease.seconds.k(Locks.lease.number(M.Setting.get(s, "locks.lease.minutes", "20"), True{}, 0n))

def Locks.warning.key(+name: String) -> String:
  "lock.warning." ++ name

def Locks.warning(+s: M.State, +name: String) -> J.Json:
  Maybe.default(&2, J.Json, J.Json.parse(M.Setting.get(s, Locks.warning.key(name), "")), J.Null{})

def Locks.warning.same(+s: M.State, +name: String, +p: LK.LockParty) -> Bool:
  +o = Locks.warning(s, name)
  Bool.and(String.eq(J.Json.get_str(o, "owner", ""), LK.Lock.id(p)), Nat.is_eq(J.Json.get_nat(o, "lease", 0n), LK.Lock.lease(p)))

def Locks.warning.put(+name: String, +p: LK.LockParty, +now: Nat) -> M.Change:
  M.SettingSet{Locks.warning.key(name), J.Json.show(J.Json.obj([J.KV{"owner", J.Str{LK.Lock.id(p)}}, J.KV{"lease", J.Num{Nat.show(LK.Lock.lease(p))}}, J.KV{"at", J.Num{Nat.show(now)}}])), now}

def Locks.heartbeat.one(+s: M.State, +now: Nat, +thread: String, +name: String) -> Step:
  +l = M.Locks.get(s, name)
  +l2 = LK.Lock.heartbeat(l, thread, Nat.add(now, Locks.lease.seconds(s)))
  +same = String.eq(J.Json.show(LK.Lock.json(l)), J.Json.show(LK.Lock.json(l2)))
  Step{Bool.pick(List<&2, M.Change>, same, Nil{}, [M.LockChanged{name, l2, now}]), Nil{}}

def Locks.heartbeat.go(names: List<&2, String>, +s: M.State, +now: Nat, +thread: String) -> Step:
  match names:
    case Nil{}: Step.none()
    case name <> t: Step.cat(Locks.heartbeat.one(s, now, thread, name), Locks.heartbeat.go(t, s, now, thread))

def Locks.heartbeat(+s: M.State, +now: Nat, +thread: String) -> Step:
  Locks.heartbeat.go(M.Locks.names(s), s, now, thread)

def Locks.lease.busy(+s: M.State, +p: LK.LockParty) -> Bool:
  Bool.or(M.Work.turn.runs(M.Thread.turn(Hub.thread(s, LK.Lock.thread(p)))), Nat.is_gt(M.Work.busy(s, LK.Lock.thread(p)), 0n))

def Locks.lease.notice.go(q: List<&2, LK.LockParty>, +s: M.State, +now: Nat, +name: String, +st: Step) -> Step:
  match q:
    case Nil{}: st
    case +p <> t:
      Locks.lease.notice.go(t, s, now, name, Step.cat(st, Lr.send(M.State.replay(Queue.changes(st), s), now, "lock:" ++ name, LK.Lock.thread(p), "Lock " ++ name ++ " had a stale idle holder; its lease was released. Your FIFO position is preserved.")))

def Locks.lease.release.go(release: Bool, +s: M.State, +now: Nat, +name: String, +p: LK.LockParty) -> Step:
  match release:
    case False{}: Step.none()
    case True{}:
      +st = Locks.release(s, now, name, LK.Lock.id(p))
      Locks.lease.notice.go(LK.Lock.waiters(M.Locks.get(s, name)), s, now, name,
        Step.cat(st, Lr.send(M.State.replay(Queue.changes(st), s), now, "lock:" ++ name, LK.Lock.thread(p), "Lock " ++ name ++ " lease expired after the warning grace period while you had no running turn or owned work; the reservation was released.")))

def Locks.lease.warn(warned: Bool, +s: M.State, +now: Nat, +name: String, +p: LK.LockParty) -> Step:
  match warned:
    case False{}:
      Step.cat(Step{[Locks.warning.put(name, p, now)], Nil{}}, Lr.send(s, now, "lock:" ++ name, LK.Lock.thread(p), "Lock " ++ name ++ " has no recent progress heartbeat. Report progress or release it. Busy work keeps its lock; an idle reservation is released after a 60-second warning grace period."))
    case True{}:
      Locks.lease.release.go(LK.Lock.lease.release(True{}, True{}, Nat.is_ge(now, Nat.add(J.Json.get_nat(Locks.warning(s, name), "at", now), 60n)), Locks.lease.busy(s, p)), s, now, name, p)

def Locks.lease.expired(late: Bool, +s: M.State, +now: Nat, +name: String, +p: LK.LockParty) -> Step:
  match late:
    case False{}: Step.none()
    case True{}: Locks.lease.warn(Locks.warning.same(s, name, p), s, now, name, p)

def Locks.lease.init(fresh: Bool, +s: M.State, +now: Nat, +name: String, +p: LK.LockParty) -> Step:
  match fresh:
    case True{}:
      Locks.put(s, now, name, LK.Lock.renew(M.Locks.get(s, name), LK.Lock.id(p), Nat.add(now, Locks.lease.seconds(s))))
    case False{}:
      Locks.lease.expired(LK.Lock.lease.expired(LK.Lock.lease(p), now), s, now, name, p)

def Locks.lease.holder(h: Maybe<&2, LK.LockParty>, +changed: Bool, +s: M.State, +now: Nat, +name: String) -> Step:
  match h:
    case None{}: Step.none()
    case Some{LK.LockParty{_, "client", _, _, _, _}}: Step.none()
    case Some{+p}: Locks.lease.init(Bool.or(changed, Nat.is_eq(LK.Lock.lease(p), 0n)), s, now, name, p)

def Locks.lease.ready(ready: Bool, +before: M.State, +s: M.State, +now: Nat, +name: String) -> Step:
  match ready:
    case False{}: Step.none()
    case True{}:
      Locks.lease.holder(LK.Lock.holder(M.Locks.get(s, name)), Bool.or(Bool.not(LK.Lock.ready(M.Locks.get(before, name))), Bool.not(String.eq(LK.Lock.owner(M.Locks.get(before, name)), LK.Lock.owner(M.Locks.get(s, name))))), s, now, name)

def Locks.lease(+before: M.State, +s: M.State, +now: Nat, +name: String) -> Step:
  Locks.lease.ready(LK.Lock.ready(M.Locks.get(s, name)), before, s, now, name)

def Locks.after.pruned(+before: M.State, +s: M.State, +now: Nat, +name: String) -> Step:
  +old = M.Locks.get(before, name)
  +current = M.Locks.get(s, name)
  +l = Locks.prune(current, s, now)
  +changed = Bool.not(String.eq(LK.Lock.owner(old), LK.Lock.owner(l)))
  +cs = Bool.pick(List<&2, M.Change>, String.eq(J.Json.show(LK.Lock.json(current)), J.Json.show(LK.Lock.json(l))), Nil{}, [M.LockChanged{name, l, now}])
  +s1 = M.State.replay(cs, s)
  Step.cat(Step.cat(Step{cs, Bool.pick(List<&2, Out>, changed, [LockGuard{name, LK.Lock.owner(old), LK.Lock.owner(l)}], Nil{})}, Locks.wait.go(LK.Lock.waiters(l), s1, now, name, 1n, Step.none())),
    Locks.grant(Bool.and(LK.Lock.ready(l), Bool.and(Bool.not(changed), Bool.not(LK.Lock.ready(old)))), LK.Lock.holder(l), s1, now, name))

def Locks.after.leased(+before: M.State, +s: M.State, +now: Nat, +name: String, +st: Step) -> Step:
  Step.cat(st, Locks.after.pruned(before, M.State.replay(Queue.changes(st), s), now, name))

def Locks.after.one(+before: M.State, +s: M.State, +now: Nat, +name: String) -> Step:
  Locks.after.leased(before, s, now, name, Locks.lease(before, s, now, name))

# a step's changes hold a LockChanged of this lock (the only change that
# writes a lock's setting, M.Locks.fold)
def Locks.touched(cs: List<&2, M.Change>, +name: String, hit: Bool) -> Bool:
  match cs hit:
    case _ True{}:
      True{}
    case Nil{} False{}:
      False{}
    case M.LockChanged{n, _, _} <> t False{}:
      Locks.touched(t, name, String.eq(n, name))
    case _ <> t False{}:
      Locks.touched(t, name, False{})

# a lock idle before the step and not changed by it can neither expire,
# promote nor grant: Locks.after.one answers Step.none() for it (law
# locks_after_idle_none), so it is not read
def Locks.after.skip(+idle: Bool, +before: M.State, +now: Nat, +name: String, +st: Step) -> Step:
  match idle:
    case True{}:
      st
    case False{}:
      Step.cat(st, Locks.after.one(before, M.State.replay(Queue.changes(st), before), now, name))

def Locks.after.go(names: List<&2, String>, +before: M.State, +now: Nat, +st: Step) -> Step:
  match names:
    case Nil{}:
      st
    case +name <> t:
      Locks.after.go(t, before, now, Locks.after.skip(Bool.and(M.Locks.idle(before, name), Bool.not(Locks.touched(Queue.changes(st), name, False{}))), before, now, name, st))

def Locks.after(+s: M.State, +now: Nat, +st: Step) -> Step:
  Locks.after.go(M.Locks.names(M.State.replay(Queue.changes(st), s)), s, now, st)

# Ordering (core/order.bend)
# --------------------------
# The project card (fab.look: the intent's goal and the "from" price) and
# an intent the person wrote (fab.intent) are the server's jobs "fab" and
# "intent", in the thread's folder; the setting order.estimates ("off"
# keeps every part's features on this machine) and order.server ride along.
# (an emptied setting is the default again)
def Buy.server.k(+v: String) -> String:
  Bool.pick(String, String.is_empty(String.trim(v)), OR.Order.server(), String.trim(v))

def Buy.server(+s: M.State) -> String:
  Buy.server.k(M.Setting.get(s, "order.server", ""))

# (estimates are asked only with ordering on: off, the hub never contacts
# the order server)
def Buy.est(+s: M.State) -> String:
  Bool.pick(String, OR.Order.enabled.of(s), M.Setting.get(s, "order.estimates", "on"), "off")

# the project's buy threads (their ids, comma-separated), so the card can
# say what of it was ordered
def Buy.ids.k(keep: Bool, +id: String, +rest: String) -> String:
  match keep:
    case True{}:
      id ++ "," ++ rest
    case False{}:
      rest

def Buy.ids(ts: List<&2, M.Thread>, +s: M.State, +project: String) -> String:
  match ts:
    case Nil{}:
      ""
    case M.Thread{+id, +p, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _} <> r:
      Buy.ids.k(Bool.and(String.eq(p, project), Bool.not(String.is_empty(M.Setting.get(s, OR.Order.buy.key(id), "")))), id, Buy.ids(r, s, project))

def Buy.proj(t: M.Thread) -> String:
  match t:
    case M.Thread{_, +p, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _}:
      p

def Buy.all(s: M.State) -> List<&2, M.Thread>:
  match s:
    case M.State{_, ts, _, _, _, _, _, _, _, _, _}:
      Map.values(&2, M.Thread, ts)

def Buy.of(+s: M.State, +th: String) -> String:
  Buy.ids(Buy.all(s), s, Buy.proj(Hub.thread(s, th)))

def Buy.look(+s: M.State, +client: U32, +id: String, +th: String, +job: String, +text: String) -> Step:
  Step.outs([Git{client, id, job, th, Hub.cwd(s, Hub.thread(s, th)), Buy.est(s) ++ "\n" ++ Buy.server(s) ++ "\n" ++ Buy.of(s, th), text}])

def Buy.title(p: M.Project) -> String:
  match p:
    case M.Project{_, +title, _, _, _}:
      OR.Order.buy.title(title)

def Buy.threads(s: M.State) -> List<&2, M.Thread>:
  match s:
    case M.State{_, ts, _, _, _, _, _, _, _, _, _}:
      Map.values(&2, M.Thread, ts)

# how many of the project's threads are already buy threads by their title
def Buy.n(ts: List<&2, M.Thread>, +pre: String) -> Nat:
  match ts:
    case Nil{}:
      0n
    case M.Thread{_, _, +title, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _} <> r:
      Nat.add(Bool.pick(Nat, String.starts_with(title, pre), 1n, 0n), Buy.n(r, pre))

# a second buy thread of a project is "Buy: lamp · 2" (laws buy_title_*)
def Buy.title.k(+t: String, +n: Nat) -> String:
  Bool.pick(String, Nat.is_eq(n, 0n), t, t ++ " · " ++ Nat.show(1n+n))

def Buy.title.of(+s: M.State, +project: String) -> String:
  Buy.title.k(Buy.title(Hub.project(s, project)), Buy.n(M.Thread.filter_project(Buy.threads(s), project), Buy.title(Hub.project(s, project))))

# Buy it: a thread of its own in the same folder (the source's worktree,
# like a delegated task), marked buy.<thread> (its source), started by the
# person's "Buy it"; the client that asked goes to it
def Buy.go(t: M.Thread, +s: M.State, +now: Nat, +client: U32, +id: String, +src: String) -> Step:
  match t:
    case M.Thread{_, +project, _, _, _, _, _, _, _, _, _, _, +env, +branch, +wt, +provider, +model, rm, im, _, _}:
      +th = Hub.id("t", now, Hub.seq(s))
      +cs = {[M.ThreadCreated{th, project, Buy.title.of(s, project), env, branch, wt, provider, model, now}, M.ModesSet{th, rm, im, model, now},
        M.SettingSet{OR.Order.buy.key(th), src, now}] : List<&2, M.Change>}
      +s1 = {M.State.replay(cs, s) : M.State}
      Step.before(cs, [Reply.made(client, id, th)], Hub.post(s1, now, th, Hub.id("m", now, Hub.seq(s1)), M.User{}, OR.Order.buy.text()))

def Buy.start.k(on: Bool, +s: M.State, +now: Nat, +client: U32, +id: String, +src: String) -> Step:
  match on:
    case True{}:
      Buy.go(Hub.thread(s, src), s, now, client, id, src)
    case False{}:
      Step.outs([Reply.err(client, id, "Ordering is off: turn it on in Settings to buy")])

def Buy.start(known: Bool, +s: M.State, +now: Nat, +client: U32, +id: String, +src: String) -> Step:
  match known:
    case True{}:
      Buy.start.k(OR.Order.enabled.of(s), s, now, client, id, src)
    case False{}:
      Step.outs([Reply.err(client, id, "no such thread")])

# one line of an agent's stdout
def Hub.agent_line.raw(+s: M.State, +now: Nat, +thread: String, line: String) -> Step:
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
      Step{Usage.result(s, thread, now, o), Nil{}}
    case _:
      Claude.line.k(i, s, thread, now, o)

def Claude.line.who.r(main: Bool, i: Nat, run: Bool, +s: M.State, +thread: String, +now: Nat, +call: String, +o: J.Json) -> Step:
  match main:
    case True{}:
      Claude.line.r(i, run, s, thread, now, o)
    case False{}:
      Sub.line(i, thread, now, call, o)

def Claude.line.o(run: Bool, +s: M.State, +thread: String, +now: Nat, +o: J.Json) -> Step:
  Claude.line.who.r(String.is_empty(J.Json.get_str(o, "parent_tool_use_id", "")), C.Codec.pos(["system", "stream_event", "assistant", "result", "user", "rate_limit_event"], J.Json.get_str(o, "type", "")),
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

# how the work that woke it ended, when not by finishing: a subagent
# killed by a Stop or an interrupt woke the agent too, and "Woken: <task>"
# alone read as that task done (law wake_says_killed)
def Wake.how(+status: String) -> String:
  Bool.pick(String, Bool.or(String.is_empty(status), String.eq(status, "completed")), "", " (" ++ status ++ ")")

def Wake.text(lc: Bool, +s: M.State, +thread: String, +o: J.Json) -> String:
  match lc:
    case True{}:
      "Resumed after Stop"
    case False{}:
      "Woken: " ++ Wake.title(s, thread, o) ++ Wake.how(J.Json.get_str(o, "status", ""))

# a turn Claude started by itself runs where the person sees it
def Wake.start(+s: M.State, +thread: String, +now: Nat, +text: String) -> List<&2, M.Change>:
  [M.TurnChanged{thread, M.Running{}, now}, M.ActivityLogged{thread, Hub.id("k", now, Hub.seq(s)), "info", "wake", text, now}]

def Claude.line.v.k(v: Nat, +s: M.State, +thread: String, +now: Nat, +o: J.Json, +text: String) -> Step:
  match v:
    case 2n:
      Claude.line.o(True{}, s, thread, now, o)
    case 3n:
      Step.after(Step.after(Claude.result.k(False{}, thread, now, text), Usage.result(s, thread, now, o)), Bg.settle(s, thread, now))
    case 4n:
      Step.after(Step.after(Claude.result.k(True{}, thread, now, text), Usage.result(s, thread, now, o)), Bg.settle(s, thread, now))
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
def Hub.agent_line.vo.raw(v: Nat, +s: M.State, +now: Nat, +thread: String, line: String, +o: J.Json, +text: String) -> Step:
  match v:
    case 0n:
      Hub.agent_line.raw(s, now, thread, line)
    case 1n+p:
      Hub.agent_line.v.k(1n+p, s, now, thread, o, text)

# Lost work
# ---------
# When the process that owned a thread's background work goes (a restart,
# the agent's exit, its replacement for a new model or mode), the work is
# no longer anyone's: the agent is told once, by a message naming each
# task, and its rows go in the same step (laws recover_lost_*, gone_*,
# replaced_*). With "restart.continue" off nothing is started: an Act
# says the same, and the rows stay, ended with status "unknown", until a
# turn ends them. The note is delivered like any message (Lost.note).

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

# Work lost at a restart is put aside, out of the thread's rows, until the
# agent is told (laws lostwork_*): the phones' island counts a thread's rows
# (M.Work.busy), and work whose owner died is nobody's to wait on. The
# stash holds the rows as Bg.show writes them.
def Lost.key(+th: String) -> String:
  "lostbg." ++ th

def Lost.stash(+s: M.State, +th: String) -> List<&2, String>:
  M.Bg.lines(M.Setting.get(s, Lost.key(th), ""))

# the work to tell: what was put aside, and any rows filed since
def Lost.of(+s: M.State, +th: String) -> List<&2, String>:
  List.append(&2, String, Lost.stash(s, th), M.Bg.of(s, th))

# the rows put aside, and the stash emptied (nothing for a thread with none)
def Lost.aside(+s: M.State, +now: Nat, +th: String) -> List<&2, M.Change>:
  Bool.pick(List<&2, M.Change>, List.is_empty(&2, String, Lost.of(s, th)), Nil{},
    [M.SettingSet{Lost.key(th), M.Bg.show(Lost.of(s, th)), now}, M.WorkSet{th, "bg", "", now}])

def Lost.unstash(+s: M.State, +now: Nat, +th: String) -> List<&2, M.Change>:
  Bool.pick(List<&2, M.Change>, List.is_empty(&2, String, Lost.stash(s, th)), Nil{}, [M.SettingSet{Lost.key(th), "", now}])

def Bg.lost.k(has: Bool, on: Bool, +s: M.State, +now: Nat, +th: String, +why: String, +ls: List<&2, String>) -> Step:
  match has on:
    case False{} _:
      Step.none()
    case True{} True{}:
      Step.cat(Lost.note(s, now, th, Lost.text(ls, why)), Step{List.append(&2, M.Change, [M.WorkSet{th, "bg", "", now}], Lost.unstash(s, now, th)), Nil{}})
    case True{} False{}:
      Step{List.append(&2, M.Change, [M.ActivityLogged{th, Hub.id("l", now, Hub.seq(s)), "info", "lost", Lost.text(ls, why), now}, M.WorkSet{th, "bg", M.Bg.show(Lost.unknown(M.Bg.of(s, th))), now}], Lost.unstash(s, now, th)), Nil{}}

# a thread's background work lost (why: "restart", "exit", "replaced"):
# told once; nothing for a thread with none new, or archived or deleted
def Bg.lost(+s: M.State, +now: Nat, +th: String, +why: String) -> Step:
  Bg.lost.k(Bool.and(Lost.new(Lost.of(s, th)), Lost.live(Hub.thread(s, th))), M.Carry.on(s), s, now, th, why, Lost.of(s, th))

# the agent's process is gone, and with it every row it filed: told (a live
# thread), then none left, whatever restart.continue says (law
# lostwork_exit_clears)
def Bg.gone.k(live: Bool, +s: M.State, +now: Nat, +th: String) -> Step:
  match live:
    case True{}:
      Bg.lost(s, now, th, "exit")
    case False{}:
      Step.none()

def Bg.gone(live: Bool, +s: M.State, +now: Nat, +th: String) -> Step:
  Step.cat(Bg.gone.k(live, s, now, th), Step{Bg.clear(s, th, now), Nil{}})

# an agent process ended; a turn still running ends with it, and the
# background work it owned is told lost (behind the failed turn's queue
# when one ran: law gone_idle_told)
def Wake.exit.k(needed: Bool, +s: M.State, +now: Nat, +thread: String, +code: U32, +st: Step) -> Step:
  match needed:
    case True{}:
      Wake.deliver(WK.Wake.on(s, "thread", thread), s, now, thread ++ ":exit:" ++ Nat.show(now) ++ ":" ++ U32.show(code), ["fail"], "thread \"" ++ Lr.title(Hub.thread(s, thread)) ++ "\" (" ++ thread ++ ") agent exited with code " ++ U32.show(code), st)
    case False{}:
      st

def Wake.exit(+s: M.State, +now: Nat, +thread: String, +code: U32, +st: Step) -> Step:
  Wake.exit.k(Bool.and(Bool.not(U32.is_eq(code, 0)), Bool.and(Bool.not(String.eq(C.Turn.name(M.Thread.turn(Hub.thread(s, thread))), "failed")), Nat.is_eq(Wake.ended(Queue.changes(st), thread), 0n))), s, now, thread, code, st)

# (the lost work is told against the state after the turn ended, so the
# note starts a turn of its own: law gone_running_told)
def Hub.agent_gone.m(+main: Step, +s: M.State, +now: Nat, +thread: String, +code: U32) -> Step:
  Wake.exit(s, now, thread, code, Step.before(Nil{}, Retire.answers(M.Asks.all(s), thread),
    Step.cat(Step.after(main, Retire.asks(M.Asks.all(s), thread, now)), Bg.gone(Lost.live(Hub.thread(s, thread)), M.State.replay(Queue.changes(main), s), now, thread))))

def Hub.agent_gone.active(+s: M.State, +now: Nat, +thread: String, +code: U32, +why: String) -> Step:
  Hub.agent_gone.m(Launch.tell(s, now, thread, Task.finish(s, now, thread, Queue.drain(s, now, thread, Ended.out(s, now, thread, Hub.agent_gone.k(Hub.running(Hub.thread(s, thread)), thread, now, code, why))))),
    s, now, thread, code)

# A completed private SDK disappearing is resource retirement, not failed
# work. Source approvals, receipts and pending conversation FIFO stay put.
# A killed unfinished run still takes normal failure/recovery handling.
def Hub.agent_gone.quiet(quiet: Bool, +s: M.State, +now: Nat, +thread: String, +code: U32, +why: String) -> Step:
  match quiet:
    case True{}: Step.none()
    case False{}: Hub.agent_gone.active(s, now, thread, code, why)

# The server supplies the departing SDK's actual ledger before dropping
# its actor. A terminal-state race must not hide accepted unfinished input.
def Hub.agent_gone.ledger(+s: M.State, +now: Nat, +thread: String, +code: U32, +why: String, cmds: CM.Cmds, held: Bool) -> Step:
  Hub.agent_gone.quiet(RL.Life.retire(s, thread, cmds, held), s, now, thread, code, why)

def Hub.agent_gone.why(+s: M.State, +now: Nat, +thread: String, +code: U32, +why: String) -> Step:
  Hub.agent_gone.quiet(RL.Life.finished(s, thread), s, now, thread, code, why)

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

# Settling by work
# ----------------
# Every step passes through here before it is stored (the server's
# Hub.step.go, after the pending counts): a thread the step leaves done
# settles (with the setting settle.done on, Shelve.done), and one it sets
# working again wakes (model.bend's Lifecycle.*, laws life_*). Only the
# threads a change can move are asked, each once, stamped with that
# change's time; a step that names none costs no replay.
# The tick (Shelve.tick) asks every thread, for state no step corrected.

type SvAt is Data:
  SvAt{thread: String, at: Nat}

def Shelve.watch(hit: Bool, +k: String, +at: Nat) -> List<&2, SvAt>:
  match hit:
    case True{}:
      [SvAt{String.drop(k, 6n), at}]
    case False{}:
      Nil{}

def Shelve.user(r: M.Role, +th: String, +at: Nat) -> List<&2, SvAt>:
  match r:
    case M.User{}:
      [SvAt{th, at}]
    case _:
      Nil{}

# the threads whose work a change can move (pre: the state before it, for
# a task's parent)
def Shelve.names(+pre: M.State, c: M.Change) -> List<&2, SvAt>:
  match c:
    case M.TurnChanged{+th, _, +at}:
      [SvAt{th, at}]
    case M.MessagePosted{+th, _, r, _, +at}:
      Shelve.user(r, th, at)
    case M.PendingChanged{+th, _, +at}:
      [SvAt{th, at}]
    case M.WorkSet{+th, _, _, +at}:
      [SvAt{th, at}]
    case M.TurnQueued{+th, _, _, +at}:
      [SvAt{th, at}]
    case M.QueueTaken{+th, +at}:
      [SvAt{th, at}]
    case M.QueueTakenOne{+th, _, +at}:
      [SvAt{th, at}]
    case M.QueueRemoved{+th, _, +at}:
      [SvAt{th, at}]
    case M.TaskDelegated{_, +p, +ch, _, _, _, +at}:
      [SvAt{p, at}, SvAt{ch, at}]
    case M.TaskStateSet{+k, _, _, +at}:
      [SvAt{M.Lifecycle.task.parent(pre, k), at}]
    case M.SettingSet{+k, _, +at}:
      Shelve.watch(String.starts_with(k, "watch."), k, at)
    case _:
      Nil{}

def Shelve.has(xs: List<&2, SvAt>, +th: String) -> Bool:
  match xs:
    case Nil{}:
      False{}
    case SvAt{+t, _} <> r:
      Bool.or(String.eq(t, th), Shelve.has(r, th))

def Shelve.add.k(skip: Bool, +t: String, +at: Nat, +acc: List<&2, SvAt>) -> List<&2, SvAt>:
  match skip:
    case True{}:
      acc
    case False{}:
      SvAt{t, at} <> acc

def Shelve.add(ys: List<&2, SvAt>, +acc: List<&2, SvAt>) -> List<&2, SvAt>:
  match ys:
    case Nil{}:
      acc
    case SvAt{+t, +at} <> r:
      Shelve.add(r, Shelve.add.k(Bool.or(String.is_empty(t), Shelve.has(acc, t)), t, at, acc))

# every thread the changes name, once (the first change's time)
def Shelve.ats(cs: List<&2, M.Change>, +pre: M.State, +acc: List<&2, SvAt>) -> List<&2, SvAt>:
  match cs:
    case Nil{}:
      acc
    case +c <> r:
      Shelve.ats(r, pre, Shelve.add(Shelve.names(pre, c), acc))

def Shelve.fresh.u(r: M.Role, +t: String, +th: String) -> Bool:
  match r:
    case M.User{}:
      String.eq(t, th)
    case _:
      False{}

# These local identity scans return at the first match. The tail calls
# stay bounded in stack even when a manually settled thread has a long log.
def Shelve.rows.has.go(xs: List<&2, String>, +id: String, found: Bool) -> Bool:
  match xs:
    case Nil{}: found
    case +l <> rest:
      match found:
        case True{}: True{}
        case False{}: Shelve.rows.has.go(rest, id, M.Bg.is(l, id))

def Shelve.rows.has(xs: List<&2, String>, +id: String) -> Bool:
  Shelve.rows.has.go(xs, id, False{})

# Snapshot updates count as fresh work only when they introduce a live
# identity. Progress, launch acknowledgments and completions keep the same
# identity and do not undo a manual settle, even if the busy count changes.
def Shelve.rows.fresh.go(xs: List<&2, String>, +old: List<&2, String>, found: Bool) -> Bool:
  match xs:
    case Nil{}: found
    case +l <> rest:
      match found:
        case True{}: True{}
        case False{}:
          Shelve.rows.fresh.go(rest, old, Bool.and(Bool.not(M.Bg.ended(l)), Bool.and(Bool.not(String.is_empty(M.Bg.field(l, 0n))), Bool.not(Shelve.rows.has(old, M.Bg.field(l, 0n))))))

def Shelve.rows.fresh(xs: List<&2, String>, +old: List<&2, String>) -> Bool:
  Shelve.rows.fresh.go(xs, old, False{})

def Shelve.queue.has.go(qs: List<&2, M.QItem>, +msg: String, found: Bool) -> Bool:
  match qs:
    case Nil{}: found
    case M.QItem{+id, _} <> rest:
      match found:
        case True{}: True{}
        case False{}: Shelve.queue.has.go(rest, msg, String.eq(id, msg))

def Shelve.queue.has(qs: List<&2, M.QItem>, +msg: String) -> Bool:
  Shelve.queue.has.go(qs, msg, False{})

def Shelve.input.seen.go(es: List<&2, M.Entry>, +msg: String, found: Bool) -> Bool:
  match es:
    case Nil{}: found
    case M.Msg{+id, _, _, _, _} <> rest:
      match found:
        case True{}: True{}
        case False{}: Shelve.input.seen.go(rest, msg, String.eq(id, msg))
    case _ <> rest:
      match found:
        case True{}: True{}
        case False{}: Shelve.input.seen.go(rest, msg, False{})

def Shelve.input.seen(es: List<&2, M.Entry>, +msg: String) -> Bool:
  Shelve.input.seen.go(es, msg, False{})

# Check the small queue before consulting the thread's history.
def Shelve.input.new.k(queued: Bool, +s: M.State, +th: String, +msg: String) -> Bool:
  match queued:
    case True{}: False{}
    case False{}: Bool.not(Shelve.input.seen(M.State.mine(s, th), msg))

def Shelve.input.new(+s: M.State, +th: String, +msg: String) -> Bool:
  Shelve.input.new.k(Shelve.queue.has(M.Queue.items(s, th), msg), s, th, msg)

# Real activity is accepted at its event boundary, against the state just
# before it. Older callbacks cannot supersede a later manual settlement.
def Shelve.recent.k(o: M.Override, +sa: Nat, +at: Nat) -> Bool:
  match o:
    case M.ForcedSettled{}: Nat.is_ge(at, sa)
    case _: True{}

def Shelve.recent(+s: M.State, +th: String, +at: Nat) -> Bool:
  Shelve.recent.k(M.Lifecycle.override(M.State.thread.get(s, th)), M.Lifecycle.settled_at(M.State.thread.get(s, th)), at)

# Only a recent user message for this thread may read its history.
def Shelve.input.recent(recent: Bool, +s: M.State, +th: String, +msg: String) -> Bool:
  match recent:
    case False{}: False{}
    case True{}: Shelve.input.new(s, th, msg)

def Shelve.input.for(hit: Bool, +s: M.State, +th: String, +msg: String, +at: Nat) -> Bool:
  match hit:
    case False{}: False{}
    case True{}: Shelve.input.recent(Shelve.recent(s, th, at), s, th, msg)

def Shelve.input.role(r: M.Role, +s: M.State, +t: String, +th: String, +msg: String, +at: Nat) -> Bool:
  match r:
    case M.User{}: Shelve.input.for(String.eq(t, th), s, th, msg, at)
    case _: False{}

def Shelve.fresh.event(+s: M.State, c: M.Change, +th: String) -> Bool:
  match c:
    case M.MessagePosted{+t, +msg, r, _, +at}:
      Shelve.input.role(r, s, t, th, msg, at)
    case M.TurnQueued{+t, +msg, _, +at}:
      Bool.and(String.eq(t, th), Bool.and(Shelve.recent(s, th, at), Bool.not(Shelve.queue.has(M.Queue.items(s, th), msg))))
    case M.WorkSet{+t, +kind, rows, +at}:
      Bool.and(String.eq(t, th), Bool.and(Shelve.recent(s, th, at), Bool.and(Bool.or(String.eq(kind, "bg"), String.eq(kind, "watch")),
        Shelve.rows.fresh(M.Bg.lines(rows), M.Bg.lines(M.Setting.get(s, kind ++ "." ++ th, ""))))))
    case M.TaskDelegated{+id, +parent, _, _, _, _, +at}:
      Bool.and(String.eq(parent, th), Bool.and(Shelve.recent(s, th, at), String.is_empty(M.Lifecycle.task.parent(s, id))))
    case M.TurnChanged{+t, M.Running{}, +at}:
      Bool.and(String.eq(t, th), Bool.and(Shelve.recent(s, th, at), Bool.not(M.State.running.k(M.State.thread.get(s, th)))))
    case _: False{}

# A settle later in the same transaction wins over the work before it;
# work accepted after that settle can wake it, including in the same second.
def Shelve.hold(c: M.Change, +th: String) -> Bool:
  match c:
    case M.ThreadSettled{+t, _, _}: String.eq(t, th)
    case M.ThreadMerged{+t, _, _}: String.eq(t, th)
    case _: False{}

def Shelve.held.go(cs: List<&2, M.Change>, +th: String, found: Bool) -> Bool:
  match cs:
    case Nil{}: found
    case +c <> rest:
      match found:
        case True{}: True{}
        case False{}: Shelve.held.go(rest, th, Shelve.hold(c, th))

def Shelve.held(cs: List<&2, M.Change>, +th: String) -> Bool:
  Shelve.held.go(cs, th, False{})

def Shelve.fresh.next(hold: Bool, fresh: Bool, +s: M.State, c: M.Change, +th: String) -> Bool:
  match hold:
    case True{}: False{}
    case False{}:
      match fresh:
        case True{}: True{}
        case False{}: Shelve.fresh.event(s, c, th)

def Shelve.fresh.go(cs: List<&2, M.Change>, +s: M.State, +th: String, fresh: Bool) -> Bool:
  match cs:
    case Nil{}: fresh
    case +c <> rest:
      Shelve.fresh.go(rest, M.State.apply.go(s, c), th, Shelve.fresh.next(Shelve.hold(c, th), fresh, s, c, th))

# Active streaming threads skip the event scan entirely.
def Shelve.fresh.on(sh: M.Shelf, cs: List<&2, M.Change>, +pre: M.State, +th: String) -> Bool:
  match sh:
    case M.Settled{}: Shelve.fresh.go(cs, pre, th, False{})
    case _: False{}

# whether a done thread settles now: only with the setting settle.done on
# (model.bend's Lifecycle.on); off, it stays active, done, until the person
# settles it or the inactivity window does (Hub.settle.go; laws life_off_*)
def Shelve.done(on: Bool, +kids: Map<&2, Nat>, +s: M.State, +t: M.Thread, +navs: Map<&2, M.Nav>, +th: String, +at: Nat) -> Bool:
  match on:
    case True{}:
      M.Lifecycle.done(kids, s, t, M.Snooze.on(at, M.Nav.until(M.Nav.get(navs, th))))
    case False{}:
      False{}

def Shelve.one.k(wake: Bool, done: Bool, +th: String, +at: Nat, +t: M.Thread, rest: List<&2, M.Change>) -> List<&2, M.Change>:
  match wake done:
    case True{} _:
      M.ThreadWoke{th, at} <> rest
    case False{} True{}:
      M.ThreadSettled{th, False{}, M.Lifecycle.at(t, at)} <> rest
    case False{} False{}:
      rest

def Shelve.wake.k(+was: Bool, +now: Bool, fresh: Bool, +post: M.State, +th: String) -> Bool:
  Bool.or(M.Lifecycle.wake.k(was, now, M.Shelf.of(M.State.thread.get(post, th)), M.Lifecycle.override(M.State.thread.get(post, th))),
    M.Lifecycle.wake.fresh(fresh, now, M.Shelf.of(M.State.thread.get(post, th))))

# woken: it started working, or new input came while it works (fresh)
def Shelve.wake(+pre: M.State, +post: M.State, +k0: Map<&2, Nat>, +k1: Map<&2, Nat>, +th: String, fresh: Bool) -> Bool:
  Shelve.wake.k(M.Lifecycle.working(k0, pre, M.State.thread.get(pre, th)), M.Lifecycle.working(k1, post, M.State.thread.get(post, th)), fresh, post, th)

def Shelve.one(+cs: List<&2, M.Change>, +pre: M.State, +post: M.State, +k0: Map<&2, Nat>, +k1: Map<&2, Nat>, +navs: Map<&2, M.Nav>, +th: String, +at: Nat, rest: List<&2, M.Change>) -> List<&2, M.Change>:
  Shelve.one.k(Bool.or(Bool.and(Bool.and(Bool.not(Shelve.held(cs, th)), Shelve.recent(pre, th, at)), Shelve.wake(pre, post, k0, k1, th, False{})),
      M.Lifecycle.wake.fresh(Shelve.fresh.on(M.Shelf.of(M.State.thread.get(post, th)), cs, pre, th), M.Lifecycle.working(k1, post, M.State.thread.get(post, th)), M.Shelf.of(M.State.thread.get(post, th)))), Shelve.done(M.Lifecycle.on(post), k1, post, M.State.thread.get(post, th), navs, th, at),
    th, at, M.State.thread.get(post, th), rest)

def Shelve.go(xs: List<&2, SvAt>, +cs: List<&2, M.Change>, +pre: M.State, +post: M.State, +k0: Map<&2, Nat>, +k1: Map<&2, Nat>, +navs: Map<&2, M.Nav>) -> List<&2, M.Change>:
  match xs:
    case Nil{}:
      Nil{}
    case SvAt{+th, +at} <> r:
      Shelve.one(cs, pre, post, k0, k1, navs, th, at, Shelve.go(r, cs, pre, post, k0, k1, navs))

def Shelve.step.post(xs: List<&2, SvAt>, +cs: List<&2, M.Change>, +pre: M.State, +post: M.State) -> List<&2, M.Change>:
  Shelve.go(xs, cs, pre, post, M.Work.kids.map(pre), M.Work.kids.map(post), M.State.navs(post))

def Shelve.step.k(xs: List<&2, SvAt>, +pre: M.State, +cs: List<&2, M.Change>) -> List<&2, M.Change>:
  match xs:
    case Nil{}:
      Nil{}
    case +x <> +r:
      Shelve.step.post(x <> r, cs, pre, M.State.replay(cs, pre))

# the settles and wakes a step's changes call for (pre: the state before it)
def Shelve.step(+pre: M.State, +cs: List<&2, M.Change>) -> List<&2, M.Change>:
  Shelve.step.k(Shelve.ats(cs, pre, Nil{}), pre, cs)

def Shelve.fix.k(+s: M.State, +cs: List<&2, M.Change>) -> List<&2, M.Change>:
  List.append(&2, M.Change, cs, Shelve.step(s, cs))

# a step's changes with the pending counts they move (Pend.fix), then the
# settles and wakes they call for
def Shelve.fix(+s: M.State, +cs: List<&2, M.Change>) -> List<&2, M.Change>:
  Shelve.fix.k(s, Pend.fix(s, cs))

# the laws' example: a project with one thread whose turn runs (started by
# a message at 2), with more of its log after
def Shelve.demo(cs: List<&2, M.Change>) -> M.State:
  M.State.replay(List.append(&2, M.Change, [M.ProjectCreated{"p1", "p", "/p", 1n}, M.ThreadCreated{"t1", "p1", "x", M.Local{}, "", "", "claude", "", 1n},
    M.MessagePosted{"t1", "m1", M.User{}, "go", 2n}, M.TurnChanged{"t1", M.Running{}, 2n}], cs), M.State.empty())

# the same with the setting settle.done on: a done thread settles at once
def Shelve.demo.on(cs: List<&2, M.Change>) -> M.State:
  Shelve.demo(M.SettingSet{M.Lifecycle.key(), "on", 1n} <> cs)

def Shelve.tick.k(wake: Bool, done: Bool, +t: M.Thread, +now: Nat, rest: List<&2, M.Change>) -> List<&2, M.Change>:
  match wake done:
    case True{} _:
      M.ThreadWoke{M.Thread.id(t), now} <> rest
    case False{} True{}:
      M.ThreadSettled{M.Thread.id(t), False{}, M.Lifecycle.at(t, now)} <> rest
    case False{} False{}:
      rest

def Shelve.tick.go(+on: Bool, ts: List<&2, M.Thread>, +navs: Map<&2, M.Nav>, +kids: Map<&2, Nat>, +s: M.State, +now: Nat) -> List<&2, M.Change>:
  match ts:
    case Nil{}:
      Nil{}
    case +t <> rest:
      Shelve.tick.k(M.Lifecycle.stale(kids, s, t), Shelve.done(on, kids, s, t, navs, M.Thread.id(t), now), t, now,
        Shelve.tick.go(on, rest, navs, kids, s, now))

# every thread's settle or wake that no step made: a settled thread that
# works (Lifecycle.stale) wakes, a done one settles (Lifecycle.done, with
# settle.done on). This is what corrects a log written before the rule,
# or after a restart
def Shelve.tick(ts: List<&2, M.Thread>, +navs: Map<&2, M.Nav>, +kids: Map<&2, Nat>, +s: M.State, +now: Nat) -> List<&2, M.Change>:
  Shelve.tick.go(M.Lifecycle.on(s), ts, navs, kids, s, now)

def Shelve.threads(+s: M.State) -> List<&2, M.Thread>:
  match s:
    case M.State{_, +ts, _, _, _, _, _, _, _, _, _}:
      Map.values(&2, M.Thread, ts)

def Shelve.tick.post(+post: M.State, +now: Nat, +kids: Map<&2, Nat>) -> List<&2, M.Change>:
  Shelve.tick(Shelve.threads(post), M.State.navs(post), kids, post, now)

# the tick's settles (Hub.settle.go, by inactivity) and then, on the state
# after them, the settles and wakes by work, so no thread settles twice
def Shelve.after(+cs: List<&2, M.Change>, +s: M.State, +now: Nat, +kids: Map<&2, Nat>) -> List<&2, M.Change>:
  List.append(&2, M.Change, cs, Shelve.tick.post(M.State.replay(cs, s), now, kids))

# the tick's settles and wakes of a state (Hub.tick.k's first part), for
# the laws and tests
def Shelve.ticked(+s: M.State, +now: Nat) -> List<&2, M.Change>:
  Shelve.after(Hub.settle.go(Shelve.threads(s), M.State.navs(s), M.Work.kids.map(s), s, now, Hub.settle.window(s)), s, now, M.Work.kids.map(s))

# every thread that should settle now, settled (a snoozed one never is);
# gh asked about due merges; worktrees put away long enough removed;
# stranded worktree waits run. Legacy conversations never expire.
# (nothing settles, expires or loses its worktree while it waits on work:
# M.Work.busy, the children counted once for all threads)
def Hub.tick.k(+s: M.State, +now: Nat, +kids: Map<&2, Nat>) -> Step:
  match s:
    case M.State{ps, +ts, _, _, _, _, _, _, _, _, _}:
      Step.before(Shelve.after(Hub.settle.go(Map.values(&2, M.Thread, ts), M.State.navs(s), kids, s, now, Hub.settle.window(s)), s, now, kids),
        List.append(&2, Out, Merge.asks(Map.values(&2, M.Project, ps), Map.values(&2, M.Thread, ts), s), Drop.asks(Map.values(&2, M.Thread, ts), kids, s, now, Hub.wt.window(s))),
        Wt.stale(Map.values(&2, M.Thread, ts), s, now))

def Hub.tick(+s: M.State, +now: Nat) -> Step:
  Step.join(Step.join(Step.join(Step.join(Hub.tick.k(s, now, M.Work.kids.map(s)), Update.due(s, now)), Update.held(s, now)), Step.cat(Step.cat(Lr.far.tick(s), Fq.tick(s)), Wake.poll(s))), Step.cat(Deploy.tick(s, now), Ship.tick(s, now)))

# A commit's lines
# ----------------
# A step's changes are written to disk as C.Change.encode and told to
# clients as C.Change.wire, the same text for every kind but the device
# ones (wire leaves their tokens out): the disk line is reused, so a change
# is shown once per commit, not three times (law commit_line_is_wire).
def Commit.line(+c: M.Change, +disk: String) -> String:
  match c:
    case M.DeviceRegistered{_, _, _, _, _, _, _}:
      J.Json.show(C.Change.wire(c))
    case M.DeviceDropped{_, _}:
      J.Json.show(C.Change.wire(c))
    case _:
      disk

def Commit.line.ok(+c: M.Change) -> Bool:
  String.eq(Commit.line(c, J.Json.show(C.Change.encode(c))), J.Json.show(C.Change.wire(c)))

# The busy count
# --------------
# /hello's busy count (M.Work.threads) walks every thread, its tasks and
# its rows: 35 ms on a real log, and the server once took it after every
# step, an agent's streamed line included. It is taken again only after a
# step whose changes can move it. What M.Work.threads reads: each
# thread's turn, archive and deletion, the tasks, and the settings bg.<th>
# and watch.<th>. A change that touches none of those leaves it as it was
# (laws busy_*); anything not listed here counts as moving it.
def Busy.inert(c: M.Change) -> Bool:
  match c:
    case M.ThreadParentSet{_, _, _}:
      True{}
    case M.MessagePosted{_, _, _, _, _}:
      True{}
    case M.ActivityLogged{_, _, _, _, _, _}:
      True{}
    case M.UsageSet{_, _, _, _}:
      True{}
    case M.TurnUsage{_, _, _, _, _, _, _, _, _, _, _, _, _}:
      True{}
    case M.DeployAsked{_, _, _, _, _}:
      True{}
    case M.DeployBuilt{_, _, _, _, _}:
      True{}
    case M.TodosSet{_, _, _}:
      True{}
    case M.SessionSet{_, _, _}:
      True{}
    case M.ModesSet{_, _, _, _, _}:
      True{}
    case M.ThreadRenamed{_, _, _}:
      True{}
    case M.PendingChanged{_, _, _}:
      True{}
    case M.LockChanged{_, _, _}:
      True{}
    case M.SettingSet{+k, _, _}:
      Bool.not(Bool.or(String.starts_with(k, "bg."), String.starts_with(k, "watch.")))
    case _:
      False{}

def Busy.moved.go(cs: List<&2, M.Change>, acc: Bool) -> Bool:
  match cs:
    case Nil{}:
      acc
    case c <> t:
      Busy.moved.go(t, Bool.or(acc, Bool.not(Busy.inert(c))))

# whether a step's changes can move the busy count
def Busy.moved(cs: List<&2, M.Change>) -> Bool:
  Busy.moved.go(cs, False{})

# for the laws: a thread running, one waiting on a background row, one done
def Busy.demo() -> M.State:
  M.State.replay([M.ProjectCreated{"p1", "p", "/p", 1n}, M.ThreadCreated{"t1", "p1", "x", M.Local{}, "", "", "claude", "", 1n},
    M.ThreadCreated{"t2", "p1", "y", M.Local{}, "", "", "claude", "", 1n}, M.ThreadCreated{"t3", "p1", "z", M.Local{}, "", "", "claude", "", 1n},
    M.TurnChanged{"t1", M.Running{}, 2n}, M.WorkSet{"t2", "bg", "a\tx\tc1\tsleep 8\tshell", 2n}, M.TurnChanged{"t3", M.Completed{}, 2n}], M.State.empty())

# what a streaming turn writes: text, a tool row, usage, todos, a session
def Busy.demo.stream() -> List<&2, M.Change>:
  [M.MessagePosted{"t1", "m1", M.Assistant{}, "hello", 3n}, M.ActivityLogged{"t1", "u1", "tool", "Bash", "ls", 3n}, M.UsageSet{"t1", 10n, 100n, 3n},
    M.TodosSet{"t1", "[]", 3n}, M.SessionSet{"t1", "S1", 3n}, M.PendingChanged{"t1", 0n, 3n}, M.SettingSet{"agent.model", "opus", 3n}]

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
# the continue message, with a deploy's words after it or not
def Carry.is(+text: String) -> Bool:
  String.eq(String.take(text, String.length(M.Carry.text())), M.Carry.text())

def Carry.fresh.go(es: List<&2, M.Entry>, skip: Bool) -> Bool:
  match es skip:
    case Nil{} _:
      True{}
    case _ <> +t True{}:
      Carry.fresh.go(t, Carry.restart(t))
    case M.Msg{_, _, _, +text, _} <> _ False{}:
      Bool.not(Carry.is(text))
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
  Bool.and(Bool.not(MR.Run.is(before, th)), Bool.and(M.Carry.on(before), Bool.and(Bool.and(Hub.running(Hub.thread(before, th)), Carry.live(Hub.thread(before, th))), Carry.fresh(M.State.mine(before, th)))))

# A repeated continuation is delayed by restartretry.bend. Keep a delegated
# task's ownership while it waits; Carry.will and immediate starts stay exact.
def Carry.prompt(es: List<&2, M.Entry>) -> Bool:
  match es:
    case Nil{}: False{}
    case M.Act{_, _, _, "restart", _, _} <> rest: Carry.prompt(rest)
    case M.Msg{_, _, M.User{}, text, _} <> _: Carry.is(text)
    case _: False{}

def Carry.deferred(+before: M.State, +th: String) -> Bool:
  Bool.and(Bool.not(MR.Run.is(before, th)), Bool.and(M.Carry.on(before),
    Bool.and(Hub.running(Hub.thread(before, th)), Bool.and(Carry.live(Hub.thread(before, th)),
      Bool.and(Bool.not(Carry.will(before, th)), Carry.prompt(M.State.mine(before, th)))))))

# Recovery
# --------
# After a restart nothing the old process held is left: no agent runs, no
# request waits parked for an answer. Every turn still marked running ends
# as interrupted (a delegated child's task fails, and its parent hears of
# it) and every open ask closes unanswered (laws recover_*). Anything a
# thread held from an older log goes with its next turn (Carry, below, or
# the next message).

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
      Launch.tell(s, now, th, Task.finish(s, now, th, Step{Rec.note(th, now, Hub.seq(s), "Backplane restarted before this turn finished") <> M.TurnChanged{th, M.Interrupted{}, now} <> STP.Stamp.stop.mark(s, th, now), Nil{}}))
    case True{} True{}:
      Step{[Rec.note(th, now, Hub.seq(s), "Backplane restarted before this turn finished"), M.TurnChanged{th, M.Interrupted{}, now}], Nil{}}
    case False{} _:
      Step.none()

def Rec.turn(r: Rec, +before: M.State, +now: Nat, +th: String) -> Rec:
  match r:
    case Rec{+s, cs, os}:
      Rec.add(Rec{s, cs, os}, Rec.turn.k(Hub.running(Hub.thread(s, th)), Bool.or(Carry.will(before, th), Carry.deferred(before, th)), s, now, th))

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
        Git{0, "", "checkpoint0", parent, Hub.cwd(s, Hub.thread(s, parent)), "", ""} <> Git{0, "", "step", parent, Hub.cwd(s, Hub.thread(s, parent)), "", "start"}
          <> Hub.run(Hub.thread(s, parent), Hub.cwd(s, Hub.thread(s, parent)), text, Hub.uuid(now, parent ++ "/r/" ++ id))}
    case True{} False{}:
      Step{[M.TaskDelivered{id, now}], Nil{}}
    case False{} _:
      Step.after(Hub.inbox.from(s, now, child, parent, M.System{}, text, Task.msg(child, id)), [M.TaskDelivered{id, now}])

# a report owed to the uberbot's thread is filed again (idempotent: a
# filed one commits only its TaskDelivered); any other is posted as before
def Task.repair.to(owned: Bool, +s: M.State, +now: Nat, +id: String, +parent: String, +child: String, +text: String) -> Step:
  match owned:
    case True{}: Operational.file(s, now, ML.Origin{Mail.addr(s, child), "task", ""}, parent, text, Task.msg(child, id), [M.TaskDelivered{id, now}])
    case False{}:
      Task.repair.has(Task.holds(M.State.mine(s, parent), Task.msg(child, id)),
        Bool.and(Bool.not(Hub.running(Hub.thread(s, parent))), Bool.and(Task.newest(M.State.mine(s, parent), Task.msg(child, id)), Bool.not(M.Queue.waiting(s, parent)))),
        s, now, id, parent, child, text)

def Task.repair.k(due: Bool, +s: M.State, +now: Nat, +id: String, +parent: String, +child: String, +text: String) -> Step:
  match due:
    case True{}:
      Task.repair.to(Operational.is(s, parent), s, now, id, parent, child, text)
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
      Step{List.append(&2, M.Change, [M.ActivityLogged{th, Hub.id("l", now, Hub.seq(s)), "info", "lost", Lost.text(Lost.of(s, th), "restart"), now},
        M.WorkSet{th, "bg", "", now}, M.SettingSet{Resume.n.key(th), "0", now}], List.append(&2, M.Change, Lost.unstash(s, now, th), Resume.clear(s, now, th))), Nil{}}
    case False{}:
      Step{List.append(&2, M.Change, [M.SettingSet{Resume.n.key(th), Nat.show(1n+Resume.n(s, th)), now}], Lost.aside(s, now, th)), Nil{}}

# a restart found a thread's work lost: with the setting off it is logged
# (Bg.lost); else it is put aside (Lost.aside) and the thread marked to be
# told (a marker already there counts a restart it outlived, and on the
# second it is only logged)
def Rec.lost.on(again: Bool, +s: M.State, +now: Nat, +th: String) -> Step:
  match again:
    case False{}:
      Step{List.append(&2, M.Change, Resume.set(s, now, th, "restart"), Lost.aside(s, now, th)), Nil{}}
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

# rows left ended with an unknown outcome by an earlier restart (told
# already) wait for nothing: their owner is gone, and they go at the next
# (laws lostwork_recover_old_rows_go)
def Rec.lost.old(old: Bool, +s: M.State, +now: Nat, +th: String) -> Step:
  match old:
    case True{}:
      Step{Bg.clear(s, th, now), Nil{}}
    case False{}:
      Step.none()

def Rec.lost.one(r: Rec, +now: Nat, +th: String) -> Rec:
  match r:
    case Rec{+s, cs, os}:
      Rec.add(Rec.add(Rec{s, cs, os}, Rec.lost.k(Lost.new(Lost.of(s, th)), Lost.live(Hub.thread(s, th)), M.Carry.on(s), s, now, th)),
        Rec.lost.old(Bool.and(Bool.not(Lost.new(Lost.of(s, th))), Bool.not(List.is_empty(&2, String, M.Bg.of(s, th)))), s, now, th))

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
def Locks.recover.party(p: LK.LockParty, +s: M.State, +now: Nat) -> Step:
  Step.none()

def Locks.recover.parties(ps: List<&2, LK.LockParty>, +s: M.State, +now: Nat, +st: Step) -> Step:
  match ps:
    case Nil{}:
      st
    case p <> t:
      Locks.recover.parties(t, s, now, Step.cat(st, Locks.recover.party(p, M.State.replay(Queue.changes(st), s), now)))

def Locks.recover.go(names: List<&2, String>, +s: M.State, +now: Nat, +st: Step) -> Step:
  match names:
    case Nil{}:
      st
    case +name <> t:
      +l = M.Locks.get(s, name)
      +ps = List.append(&2, LK.LockParty, LK.Lock.holder.list(LK.Lock.holder(l)), LK.Lock.waiters(l))
      +l2 = LK.Lock.extend(l, Nat.add(now, 90n))
      +same = String.eq(J.Json.show(LK.Lock.json(l)), J.Json.show(LK.Lock.json(l2)))
      Locks.recover.go(t, s, now, Step.cat(Locks.recover.parties(ps, s, now, st), Step{Bool.pick(List<&2, M.Change>, same, Nil{}, [M.LockChanged{name, l2, now}]), Bool.pick(List<&2, Out>, String.is_empty(LK.Lock.owner(l)), Nil{}, [LockGuard{name, "", LK.Lock.owner(l)}])}))

def Locks.recover(+s: M.State, +now: Nat) -> Step:
  Locks.recover.go(M.Locks.names(s), s, now, Step.none())

# the threads but those whose agent the restart kept (adopt.bend): their
# turns still run, their work is still theirs
def Rec.but(ids: List<&2, String>, +kept: List<&2, String>) -> List<&2, String>:
  match ids:
    case Nil{}:
      Nil{}
    case +x <> t:
      List.append(&2, String, Bool.pick(List<&2, String>, AD.Adopt.in(kept, x), Nil{}, [x]), Rec.but(t, kept))

def Hub.recover.k(+s: M.State, +now: Nat, +kept: List<&2, String>) -> Step:
  Rec.step(Rec.lost(Rec.but(Rec.ids(Rec.threads(s)), kept), Rec.repair(Rec.turns(Rec.but(Rec.ids(Rec.threads(s)), kept), Rec.asks(M.Asks.all(s), Rec{s, Nil{}, Nil{}}, now), s, now), s, now), now))

def Run.row(+s: M.State, +now: Nat, +bot: String, +conv: String, +text: String) -> M.Change:
  M.ActivityLogged{BT.Bot.thread(BT.World.bot(M.State.bots(s), bot)), Hub.id("esc", now, Hub.seq(s)), "info", "escalation", text ++ " · " ++ REF.Ref.uri(REF.Ref.conv(conv)), now}

def Run.recent(+s: M.State, +now: Nat, +key: String) -> Bool:
  +j = Maybe.default(&2, J.Json, J.Json.parse(M.Setting.get(s, "told." ++ key, "")), J.Null{})
  Bool.and(Nat.is_gt(J.Json.get_nat(j, "at", 0n), 0n), Nat.is_lt(Nat.sub(now, J.Json.get_nat(j, "at", 0n)), 3600n))

def Run.told(+now: Nat, +key: String, +bot: String, +text: String) -> List<&2, M.Change>:
  TLD.Told.mark.set(key, now, key, text, "alert", bot)

def Run.person(on: Bool, +s: M.State, +now: Nat, +bot: String, +conv: String, +text: String) -> Step:
  match on:
    case False{}: Step.none()
    case True{}:
      +th = BT.Bot.thread(BT.World.bot(M.State.bots(s), bot))
      Step{List.append(&2, M.Change, NO.Policy.ask(s, now, th, "Mailroom escalation", text ++ " · " ++ REF.Ref.uri(REF.Ref.conv(conv))), Run.told(now, "mail-person-" ++ bot ++ "-" ++ conv, bot, text)), [Alerted{th}]}

def Run.person.ready(+s: M.State, +now: Nat, +bot: String, +conv: String) -> Bool:
  Bool.and(NO.Policy.on(s, "needs"), Bool.not(Run.recent(s, now, "mail-person-" ++ bot ++ "-" ++ conv)))

def Run.escalate.to(person: Bool, +s: M.State, +now: Nat, +bot: String, +conv: String, +text: String) -> Step:
  match person:
    case True{}: Run.person(Run.person.ready(s, now, bot, conv), s, now, bot, conv, text)
    case False{}:
      +th = BT.Bot.thread(BT.World.bot(M.State.bots(s), bot))
      +msg = Hub.id("esc", now, Hub.seq(s))
      Origin.step(Hub.deliver(Hub.running(Hub.thread(s, th)), M.Worktree.waits(s, Hub.thread(s, th), now), s, now, th, msg, M.System{}, "[mailroom escalation from " ++ conv ++ "] " ++ text), th, msg, ML.Origin{"mailroom:" ++ conv, "escalation", conv}, now)

def Run.input(+s: M.State, +r: ML.Run) -> String:
  MD.Disposition.current(s, r)

def Run.escalated(+s: M.State, +r: ML.Run, +now: Nat) -> M.Change:
  M.SettingSet{"run.escalated." ++ ML.Run.thread(r), Run.input(s, r), now}

def Run.escalation.mark.got(r: Maybe<&2, ML.Run>, +s: M.State, +now: Nat, delivered: Bool) -> List<&2, M.Change>:
  match r delivered:
    case Some{+r} True{}: [Run.escalated(s, r, now)]
    case _ _: Nil{}

def Run.escalation.mark(+s: M.State, +now: Nat, +bot: String, +conv: String, +to: String) -> List<&2, M.Change>:
  Run.escalation.mark.got(ML.Box.run_of(M.State.mail(s), ML.Box.live(M.State.mail(s), bot, conv)), s, now,
    Bool.or(Bool.not(String.eq(to, "person")), Run.person.ready(s, now, bot, conv)))

# Private delivery gets a separate thread AND provider session. A ride in
# the original thread hid the input, but still published every model reply.
# A fresh lane inherits existing owner permissions/account bindings, never
# resumes a stopped lane, and is hidden by the shared Run.is projection.
def Private.modes(parent: M.Thread, +thread: String, +now: Nat) -> M.Change:
  match parent:
    case M.Thread{_, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, +model, runtime, interaction, _, _}:
      M.ModesSet{thread, runtime, interaction, model, now}

def Private.spawn(+s: M.State, +now: Nat, +bot: String, +input: String, +note: String) -> Step:
  +owner = BT.Bot.thread(BT.World.bot(M.State.bots(s), bot))
  +parent = Hub.thread(s, owner)
  +thread = PM.Private.thread(bot) ++ "-" ++ Hub.id("turn", now, Hub.seq(s))
  +provider = Hub.provider(parent)
  +cs = {[M.ThreadCreated{thread, "", BT.Bot.name(BT.World.bot(M.State.bots(s), bot)) ++ ": private coordination", M.Local{}, "", Hub.cwd(s, parent), provider, Turn.model(parent), now},
    M.WorktreeSet{thread, "", Hub.cwd(s, parent), now}, Private.modes(parent, thread, now),
    M.SettingSet{PM.Private.key(thread), bot, now}, M.SettingSet{"mail.private.input." ++ thread, input, now},
    M.RunSet{thread, bot, ML.Mail.pair("bot:" ++ bot, "thread:" ++ thread), input, now},
    M.SettingSet{"account.thread." ++ thread ++ "." ++ provider, M.Setting.get(s, "account.thread." ++ owner ++ "." ++ provider, ""), now},
    M.SettingSet{AG.Effort.key(thread), M.Setting.get(s, AG.Effort.key(owner), ""), now}] : List<&2, M.Change>}
  +post = M.State.replay(cs, s)
  Step.before(cs, Nil{}, Wake.ride(post, now, thread, "triage:" ++ input, PM.Private.brief() ++ note))

def Private.tell.fresh(fresh: Bool, +s: M.State, +now: Nat, +thread: String, +text: String, +key: String) -> Step:
  match fresh:
    case False{}: Step.none()
    case True{}:
      +bot = PM.Private.owner(s, thread)
      +target = PM.Private.target(s, thread)
      +msg = Hub.id("tell", now, Hub.seq(s))
      Origin.step(Step{List.append(&2, M.Change,
        [M.MessagePosted{target, msg, M.Assistant{}, text, now}, M.SettingSet{PM.Private.told.key(bot, key), msg, now}],
        TLD.Told.mark.set(msg, now, key, text, "chat", bot)), Nil{}}, target, msg, ML.Origin{"thread:" ++ thread, "tell-person", key}, now)

def Private.tell(allowed: Bool, +s: M.State, +now: Nat, +thread: String, +text: String, +key: String) -> Step:
  match allowed:
    case False{}: Step.none()
    case True{}: Private.tell.fresh(String.is_empty(M.Setting.get(s, PM.Private.told.key(PM.Private.owner(s, thread), key), "")), s, now, thread, text, key)

def Run.triage.send(fresh: Bool, +s: M.State, +now: Nat, +bot: String, +conv: String, +text: String, +input: String) -> Step:
  match fresh:
    case False{}: Step.none()
    case True{}:
      +note = "[Private mailroom escalation; assess in this separate private lane. Resolve routine coordination, duplicate reports and evidence limitations silently within existing authority. Only for a verified material new outcome, real decision, approval or coordination blocker, explicitly call tell_person with your OWN contextual update and source links. Empty final output is successful; no visible response or acknowledgement is owed. Never forward raw worker output, internal tool denials or charter chatter; a triage limitation alone does not require user approval. Genuine source approvals remain pending.]\n" ++
        TRI.Triage.source(ML.Box.conv(M.State.mail(s), conv)) ++ "\nInput: " ++ input ++ "\nTriage assessment (untrusted report, not authorization):\n" ++ text
      +cs = List.append(&2, M.Change, [M.SettingSet{TRI.Triage.key(bot, input), conv, now}, M.SettingSet{"triage.audit." ++ bot ++ "." ++ Nat.show(now) ++ ":" ++ input, note, now}], Run.escalation.mark(s, now, bot, conv, "bot"))
      Step.before(cs, Nil{}, Private.spawn(M.State.replay(cs, s), now, bot, input, note))

def Run.triage.got(r: Maybe<&2, ML.Run>, +s: M.State, +now: Nat, +bot: String, +conv: String, +text: String) -> Step:
  match r:
    case None{}: Step.none()
    case Some{+r}:
      +input = Run.input(s, r)
      Run.triage.send(Bool.and(Bool.not(String.is_empty(input)), String.is_empty(M.Setting.get(s, TRI.Triage.key(bot, input), ""))), s, now, bot, conv, text, input)

def Run.escalate.k(triage: Bool, +s: M.State, +now: Nat, +bot: String, +conv: String, +to: String, +text: String) -> Step:
  match triage:
    case True{}: Run.triage.got(ML.Box.run_of(M.State.mail(s), ML.Box.live(M.State.mail(s), bot, conv)), s, now, bot, conv, text)
    case False{}: Step.before(Run.row(s, now, bot, conv, text) <> Run.escalation.mark(s, now, bot, conv, to), Nil{}, Run.escalate.to(String.eq(to, "person"), s, now, bot, conv, text))

def Run.escalate.topic.fresh(fresh: Bool, +s: M.State, +now: Nat, +bot: String, +conv: String, +to: String, +text: String, +input: String, +topic: String) -> Step:
  match fresh:
    case True{}: Step.before([M.SettingSet{topic, input, now}], Nil{}, Run.escalate.k(True{}, s, now, bot, conv, to, text))
    case False{}: Step{List.append(&2, M.Change, [M.SettingSet{TRI.Triage.key(bot, input), "duplicate:" ++ topic, now}], Run.escalation.mark(s, now, bot, conv, "bot")), Nil{}}

# A model can correlate the same exact event/version reported by a room,
# direct worker report or launch result. Scope the key to the owner; it
# records queued delivery, never that the person was already told.
def Run.escalate.topic.got(r: Maybe<&2, ML.Run>, +s: M.State, +now: Nat, +bot: String, +conv: String, +to: String, +text: String, +key: String) -> Step:
  match r:
    case None{}: Step.none()
    case Some{+r}:
      +input = Run.input(s, r)
      +topic = "triage.event." ++ J.Json.show(J.Json.arr([J.Str{bot}, J.Str{String.take(key, 200n)}]))
      Run.escalate.topic.fresh(String.is_empty(M.Setting.get(s, topic, "")), s, now, bot, conv, to, text, input, topic)

def Run.escalate.topic(keyed: Bool, +s: M.State, +now: Nat, +bot: String, +conv: String, +to: String, +text: String, +key: String) -> Step:
  match keyed:
    case False{}: Run.escalate.k(TRI.Triage.owner(s, bot), s, now, bot, conv, to, text)
    case True{}: Run.escalate.topic.got(ML.Box.run_of(M.State.mail(s), ML.Box.live(M.State.mail(s), bot, conv)), s, now, bot, conv, to, text, key)

def Run.escalate(+s: M.State, +now: Nat, +bot: String, +conv: String, +to: String, +text: String) -> Step:
  Run.escalate.k(TRI.Triage.owner(s, bot), s, now, bot, conv, to, text)

# Independent of agent-side tool approval: an actionable mail run cannot
# finish silently when its reply/escalation tools were denied or failed.
# This is a report to its own bot, not permission to perform the action.
def Run.unresolved.error(es: List<&2, M.Entry>) -> String:
  match es:
    case Nil{}: ""
    case M.Act{_, _, "error", _, text, _} <> _: text
    case _ <> t: Run.unresolved.error(t)

def Run.unresolved.triage(+s: M.State, +r: ML.Run) -> String:
  "Private triage could not finish assessing this report; it remains unresolved. This is an evidence limitation, not evidence of completion, a new worker outcome, or a request for user authorization. Review the source using permitted status/history tools and the coordinator's existing context/told records. Resolve already-covered reports and known holds quietly. Only a verified material new outcome, decision, genuine source approval or coordination blocker warrants a user update. Internal tool arguments and errors remain in the run timeline.\nRun: " ++ REF.Ref.uri(REF.Ref.thread(ML.Run.thread(r))) ++ "\nActionable input: " ++ Run.input(s, r)

def Run.unresolved.legacy(+s: M.State, +r: ML.Run) -> String:
  "Mailroom run " ++ ML.Run.thread(r) ++ " ended without a committed reply, deferral, completion or escalation. The request remains unresolved; review it and respond in the conversation. Do not repeat completed actions.\n" ++
  "Run: " ++ REF.Ref.uri(REF.Ref.thread(ML.Run.thread(r))) ++ "\nFailure detail: " ++ String.take(Run.unresolved.error(M.State.mine(s, ML.Run.thread(r))), 2000n) ++ "\nConversation: " ++ ML.Run.conv(r) ++ "\nActionable input: " ++ Run.input(s, r) ++ "\nRun's last response (not evidence of completion):\n" ++ String.take(Task.last(s, ML.Run.thread(r)), 4000n)

def Run.unresolved.note(+s: M.State, +r: ML.Run) -> String:
  Bool.pick(String, TRI.Triage.owner(s, ML.Run.bot(r)), Run.unresolved.triage(s, r), Run.unresolved.legacy(s, r))

# The journal could not accept a report. Surface an explicit delivery
# failure through the private durable ride, without acknowledging a body
# or producer result that was never committed. Source history stays put.
def Mail.failure.owner(owned: Bool, +s: M.State, +now: Nat, +body: MB.Body, +bot: String, +reason: String) -> Step:
  match owned:
    case False{}: Step.none()
    case True{}:
      +input = "delivery-failed:" ++ ML.Mail.id(MB.Body.mail(body))
      Run.triage.send(String.is_empty(M.Setting.get(s, TRI.Triage.key(bot, input), "")), s, now, bot, ML.Mail.conv(MB.Body.mail(body)),
        "Internal mail delivery FAILED; do not claim this report was filed or handled. Review the original source; its work/approval is unchanged. Do not repeat source actions.\n" ++ TRI.Triage.source(MB.Body.conv(body)) ++ "\nFailure: " ++ reason, input)

def Mail.failure.got(to: Maybe<&2, REF.Ref>, +s: M.State, +now: Nat, +body: MB.Body, +reason: String) -> Step:
  match to:
    case Some{REF.Ref{"bot", +bot <> _, _}}: Mail.failure.owner(TRI.Triage.owner(s, bot), s, now, body, bot, reason)
    case _: Step.none()

def Mail.failure(+s: M.State, +now: Nat, +body: MB.Body, +reason: String) -> Step:
  Mail.failure.got(REF.Ref.of_addr(ML.Mail.to(MB.Body.mail(body))), s, now, body, reason)

def Run.unresolved.forward(triage: Bool, +s: M.State, +now: Nat, +r: ML.Run) -> Step:
  match triage:
    case True{}:
      +input = Run.input(s, r)
      Step.before([M.SettingSet{"run.forwarded." ++ ML.Run.thread(r), input, now}], Nil{},
        Run.triage.send(String.is_empty(M.Setting.get(s, TRI.Triage.key(ML.Run.bot(r), input), "")), s, now, ML.Run.bot(r), ML.Run.conv(r), Run.unresolved.note(s, r), input))
    case False{}:
      Step.before([M.SettingSet{"run.forwarded." ++ ML.Run.thread(r), Run.input(s, r), now}], Nil{},
        Run.escalate(s, now, ML.Run.bot(r), ML.Run.conv(r), "bot", Run.unresolved.note(s, r)))

def Run.unresolved.send(send: Bool, +s: M.State, +now: Nat, +r: ML.Run) -> Step:
  match send:
    case False{}: Step.none()
    case True{}:
      Run.unresolved.forward(TRI.Triage.owner(s, ML.Run.bot(r)), s, now, r)

def Run.unresolved.status(+s: M.State, +r: ML.Run) -> String:
  Bool.pick(String, TRI.Triage.ask.mail(ML.Box.headers(M.State.mail(s), ML.Run.conv(r)), s, Run.input(s, r)), "", ML.Mark.status(ML.Box.mark_of(M.State.mail(s), ML.Run.bot(r), ML.Run.conv(r))))

def Run.unresolved(+s: M.State, +now: Nat, +r: ML.Run) -> Step:
  Run.unresolved.send(Bool.and(Bool.not(MD.Disposition.handled(s, r)), MCN.Continuity.needs(Run.input(s, r), M.Setting.get(s, "run.escalated." ++ ML.Run.thread(r), ""),
    M.Setting.get(s, "run.forwarded." ++ ML.Run.thread(r), ""), Run.unresolved.status(s, r),
    Bool.and(Bool.not(TRI.Triage.owner(s, ML.Run.bot(r))), MCN.Continuity.replied(ML.Box.headers(M.State.mail(s), ML.Run.conv(r)), ML.Run.bot(r), ML.Run.thread(r))))), s, now, r)

# Failed/unknown triage forwards EVERY pending input, oldest first. Each
# receipt advances current; replay sees the durable per-input markers.
def Run.unresolved.all.go(headers: List<&2, ML.Mail>, +s: M.State, +now: Nat, +r: ML.Run, +st: Step) -> Step:
  match headers:
    case Nil{}: st
    case _ <> t: Run.unresolved.all.go(t, s, now, r, Step.cat(st, Run.unresolved(M.State.replay(Queue.changes(st), s), now, r)))

def Run.quiet.finished(completed: M.TurnState, +s: M.State, +now: Nat, +r: ML.Run) -> List<&2, M.Change>:
  match completed:
    case M.Completed{}: Bool.pick(List<&2, M.Change>, Bool.and(List.is_empty(&2, String, TRI.Triage.queue(s, ML.Run.bot(r), ML.Run.conv(r))), Bool.and(String.is_empty(MD.Disposition.current(s, r)), Bool.not(String.is_empty(M.Setting.get(s, MD.Disposition.key(ML.Run.thread(r), ML.Run.mail(r)), ""))))), [M.MailMarked{ML.Run.bot(r), ML.Run.conv(r), "done", ML.Conv.last(ML.Box.conv(M.State.mail(s), ML.Run.conv(r))), 0n, now}], Nil{})
    case _: Nil{}

def Run.queue.interrupted(t: M.TurnState) -> Bool:
  match t:
    case M.Interrupted{}: True{}
    case _: False{}

def Run.ended.quiet(+s: M.State, +now: Nat, +r: ML.Run, +st: Step) -> Step:
  +post = M.State.replay(Queue.changes(st), s)
  +done = Step.after(st, Run.quiet.finished(M.Thread.turn(Hub.thread(post, ML.Run.thread(r))), post, now, r))
  Step.cat(done, Step.outs(Bool.pick(List<&2, Out>, Bool.and(TRI.Triage.bound(s, r), Bool.not(Run.queue.interrupted(M.Thread.turn(Hub.thread(post, ML.Run.thread(r)))))), [RunNext{ML.Run.bot(r), ML.Run.conv(r)}], Nil{})))

def Run.ended.pending(triage: Bool, +r: ML.Run, +before: M.State, +now: Nat, +st: Step) -> Step:
  match triage:
    case False{}: Step.cat(st, Run.unresolved(M.State.replay(Queue.changes(st), before), now, r))
    case True{}: Run.ended.quiet(before, now, r, Run.unresolved.all.go(ML.Box.headers(M.State.mail(before), ML.Run.conv(r)), before, now, r, st))

def Run.ended.live(live: Bool, +r: ML.Run, +before: M.State, +now: Nat, +st: Step) -> Step:
  match live:
    case False{}: st
    case True{}: Run.ended.pending(TRI.Triage.owner(before, ML.Run.bot(r)), r, before, now, st)

def Run.ended.got(r: Maybe<&2, ML.Run>, +before: M.State, +now: Nat, +st: Step) -> Step:
  match r:
    case None{}: st
    case Some{+r}:
      Run.ended.live(ML.Run.live(r), r, before, now, st)

# An actual approval/question remains open at its source. No ask is
# copied, answered or converted into a permission in the coordinator.
def Triage.ask.got(run: Maybe<&2, ML.Run>, +s: M.State, +now: Nat, +th: String, +ask: String, +tool: String, +detail: String) -> Step:
  match run:
    case Some{+r}: Run.escalate(s, now, ML.Run.bot(r), ML.Run.conv(r), "bot", "Private triage is waiting on its own tool permission or question; evidence gathering is incomplete. This is not a source worker approval or evidence that the person must act. Review privately and resolve coordination within existing authority; keep internal tool arguments out of user updates.\nRun: " ++ REF.Ref.uri(REF.Ref.thread(th)) ++ "\nPending run ask: " ++ ask ++ ". It remains pending; no authorization was granted.")
    case None{}: Operational.mail(s, now, ML.Origin{Mail.addr(s, th), "ask", ask}, TRI.Triage.thread(s), "Actual pending ask " ++ ask ++ ": " ++ tool ++ " " ++ detail ++ "\nSource: " ++ REF.Ref.uri(REF.Ref.thread(th)) ++ "\nThe real approval remains at its source and must be answered by the person.", "triage-ask:" ++ ask, "act", Nil{})

def Triage.ask(managed: Bool, +s: M.State, +now: Nat, +th: String, +ask: String, +tool: String, +detail: String) -> Step:
  match managed:
    case False{}: Step.none()
    case True{}: Triage.ask.got(MR.Run.of(s, th), s, now, th, ask, tool, detail)

# Other bots' final reports also enter the one pipeline. Launched workers
# already report through Lr, and private runs never report themselves.
# A successful empty bot turn carries no report to assess. Do not create
# an actionable envelope from its wrapper alone. Failed turns and source
# asks still follow their existing paths; reminders are not bot reports.
def Triage.bot.report(turn: M.TurnState, text: String) -> Bool:
  match turn:
    case M.Completed{}: Bool.not(String.is_empty(String.trim(text)))
    case _: True{}

def Triage.bot.file(report: Bool, +s: M.State, +now: Nat, +th: String, text: String) -> Step:
  match report:
    case False{}: Step.none()
    case True{}: Operational.file(s, now, ML.Origin{Mail.addr(s, th), "bot", ""}, TRI.Triage.thread(s), "Bot report from " ++ REF.Ref.uri(REF.Ref.thread(th)) ++ ":\n" ++ text, "triage-bot:" ++ th ++ ":" ++ Nat.show(now), Nil{})

def Triage.bot.end(report: Bool, +s: M.State, +now: Nat, +th: String) -> Step:
  match report:
    case False{}: Step.none()
    case True{}: Triage.bot.file(Triage.bot.report(M.Thread.turn(Hub.thread(s, th)), Task.last(s, th)), s, now, th, Task.last(s, th))

def Triage.ended(+before: M.State, +now: Nat, +th: String, +st: Step) -> Step:
  +s = M.State.replay(Queue.changes(st), before)
  Step.cat(Run.ended.got(MD.Runs.gate(before, ML.Box.run_of(M.State.mail(before), th)), before, now, st), Triage.bot.end(Bool.and(TRI.Triage.managed(before, th), Bool.and(BT.World.is_bot_thread(M.State.bots(before), th), Bool.not(MR.Run.is(before, th)))), s, now, th))

def Run.after.go(cs: List<&2, M.Change>, +before: M.State, +now: Nat, +st: Step) -> Step:
  match cs:
    case Nil{}: st
    case M.TurnChanged{+th, M.Completed{}, _} <> rest:
      Run.after.go(rest, before, now, Triage.ended(before, now, th, st))
    case M.TurnChanged{+th, M.Failed{}, _} <> rest:
      Run.after.go(rest, before, now, Triage.ended(before, now, th, st))
    case M.AskOpened{+ask, +th, _, +tool, +detail, _, _} <> rest:
      Run.after.go(rest, before, now, Step.cat(st, Triage.ask(TRI.Triage.managed(before, th), M.State.replay(Queue.changes(st), before), now, th, ask, tool, detail)))
    case M.ActivityLogged{+th, _, _, "fallback-blocked", _, _} <> rest:
      Run.after.go(rest, before, now, Run.ended.got(MD.Runs.gate(before, ML.Box.run_of(M.State.mail(before), th)), before, now, st))
    case _ <> rest: Run.after.go(rest, before, now, st)

def Run.fallback.go(cs: List<&2, M.Change>, +before: M.State, +now: Nat, +st: Step) -> Step:
  match cs:
    case Nil{}: st
    case M.ActivityLogged{+th, _, _, "fallback-blocked", _, _} <> t:
      Run.fallback.go(t, before, now, Run.ended.got(MD.Runs.gate(before, ML.Box.run_of(M.State.mail(before), th)), before, now, st))
    case _ <> t: Run.fallback.go(t, before, now, st)

def Run.fallback(+before: M.State, +now: Nat, +st: Step) -> Step:
  Run.fallback.go(Queue.changes(st), before, now, st)

def Run.after.on(on: Bool, +before: M.State, +now: Nat, +st: Step) -> Step:
  match on:
    case False{}: st
    case True{}: Run.after.go(Queue.changes(st), before, now, st)

def Run.after(+before: M.State, now: Nat, st: Step) -> Step:
  Run.after.on(Bool.or(String.eq(M.Setting.get(before, "mail.on", "off"), "on"), Bool.not(String.is_empty(UB.Uber.bot(before)))), before, now, st)

def Run.recover.report(on: Bool, +s: M.State, +now: Nat, +r: ML.Run, +st: Step) -> Step:
  match on:
    case False{}: st
    case True{}: Step.cat(st, Run.unresolved(M.State.replay(Queue.changes(st), s), now, r))

def Run.recover.one(go: Bool, +s: M.State, +now: Nat, +r: ML.Run) -> Step:
  match go:
    case False{}: Step.none()
    case True{}:
      Run.recover.report(M.Carry.on(s), s, now, r,
        Step.before([M.TurnChanged{ML.Run.thread(r), M.Interrupted{}, now}, M.ThreadSettled{ML.Run.thread(r), False{}, now}], Nil{},
          Mail.file(s, now, ML.Origin{"backplane", "system", "system:" ++ ML.Run.bot(r)}, "bot:" ++ ML.Run.bot(r), "run " ++ ML.Run.thread(r) ++ " was cut off by a restart", Hub.id("run-restart-" ++ ML.Run.thread(r), now, Hub.seq(s)))))

def Run.recover.all(+rs: List<&2, ML.Run>, +s: M.State, +now: Nat, +kept: List<&2, String>, +st: Step) -> Step:
  match rs:
    case Nil{}: st
    case +r <> +t: Run.recover.all(t, s, now, kept, Step.cat(st, Run.recover.one(Bool.and(Bool.not(PM.Private.is(s, ML.Run.thread(r))), Bool.and(MD.Runs.on(s, ML.Run.bot(r)), Bool.and(ML.Run.live(r), Bool.not(AD.Adopt.in(kept, ML.Run.thread(r)))))),  M.State.replay(Queue.changes(st), s), now, r)))

def Run.recover.on(on: Bool, +s: M.State, +now: Nat, +kept: List<&2, String>, +st: Step) -> Step:
  match on:
    case False{}: st
    case True{}: Run.recover.all(Map.values(&2, ML.Run, ML.Box.runs(M.State.mail(s))), s, now, kept, st)

def Run.queue.recover.one(parts: List<&2, String>, +s: M.State) -> List<&2, Out>:
  match parts:
    case +bot <> +conv <> Nil{}:
      Bool.pick(List<&2, Out>, Bool.and(Bool.not(TRI.Triage.held(s, bot, conv)), Bool.and(TRI.Triage.owner(s, bot), Bool.and(String.is_empty(ML.Box.live(M.State.mail(s), bot, conv)), Bool.not(List.is_empty(&2, String, TRI.Triage.queue(s, bot, conv)))))), [RunNext{bot, conv}], Nil{})
    case _: Nil{}

def Run.queue.recover.keys(ks: List<&2, String>, +s: M.State) -> List<&2, Out>:
  match ks:
    case Nil{}: Nil{}
    case +k <> rest:
      List.append(&2, Out, Bool.pick(List<&2, Out>, String.starts_with(k, "triage.queue."), Run.queue.recover.one(TRI.Triage.queue.strings(J.Json.elems(Maybe.default(&2, J.Json, J.Json.parse(String.drop(k, 13n)), J.Null{}))), s), Nil{}), Run.queue.recover.keys(rest, s))

def Run.recover(+s: M.State, +now: Nat, +kept: List<&2, String>, +st: Step) -> Step:
  +done = Run.recover.on(Bool.or(String.eq(M.Setting.get(s, "mail.on", "off"), "on"), Bool.not(String.is_empty(UB.Uber.bot(s)))), s, now, kept, st)
  +post = M.State.replay(Queue.changes(done), s)
  Step.cat(done, Step.outs(Bool.pick(List<&2, Out>, M.Carry.on(s), Run.queue.recover.keys(TRI.Triage.settings(post), post), Nil{})))

# (and every watched work still open is watched again: Watch.rearm)
# kept: the threads whose agent process the restart kept (adopted): their
# turns are not ended, their work not lost, nothing carries them on
def Hub.recover.a(+s: M.State, +now: Nat, +kept: List<&2, String>) -> Step:
  +locks = Locks.recover(s, now)
  +s1 = M.State.replay(Queue.changes(locks), s)
  Run.recover(s1, now, kept, Step.cat(locks, Step.cat(Step{Nil{}, Watch.rearm(s1)}, Hub.recover.k(s1, now, kept))))

def Hub.recover(+s: M.State, +now: Nat) -> Step:
  Hub.recover.a(s, now, Nil{})

# Carrying on
# -----------
# With the "restart.continue" setting on (M.Carry.on), every turn the
# restart interrupted goes on: the thread is told to continue (with any
# message held in its queue), and its agent resumes its session. A thread
# whose newest entry is already that message made no progress since the
# last restart and is left stopped, so a turn that brings the hub down
# never loops (laws carry_*). Runs after Hub.recover, over the state from
# before it.

# what a cut turn is told besides: an agent process that was not kept
# through the restart lost the result of a tool call in flight, and what
# the call started (a command) may still run or have partly run
def Carry.cut() -> String:
  "If a tool call was running when it stopped, its result was lost, and what it started may still be running or have partly run: check its effect before running it again."

def Carry.k(go: Bool, +s: M.State, +now: Nat, +th: String) -> Step:
  match go:
    case True{}:
      Step.after(Mail.inbound(Hub.post(s, now, th, Hub.id("c", now, Hub.seq(s)), M.User{}, M.Carry.text() ++ " " ++ Carry.cut() ++ DP.Deploy.suffix(s, VER.Version.current())), s, now, ML.Origin{"backplane", "carry", ""}, th, M.Carry.text() ++ " " ++ Carry.cut() ++ DP.Deploy.suffix(s, VER.Version.current()), Hub.id("c", now, Hub.seq(s))),
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

def Carry.go(on: Bool, +before: M.State, +after: M.State, +now: Nat, +kept: List<&2, String>) -> Step:
  match on:
    case True{}:
      Cr.step(Carry.all(Rec.but(Rec.ids(Rec.threads(before)), kept), Cr{Rec{after, Nil{}, Nil{}}, Rec.burst()}, before, now))
    case False{}:
      Step.none()

# before: the state Hub.recover started from; after: once its step applied
# (turns carried on and lost work told, Rec.burst() threads at most)
def Hub.carry.a(+before: M.State, +after: M.State, +now: Nat, +kept: List<&2, String>) -> Step:
  Step.cat(Carry.go(M.Carry.on(after), before, after, now, kept), Step.cat(Deploy.after(after, now), Ship.recover(after, now)))

def Hub.carry(+before: M.State, +after: M.State, +now: Nat) -> Step:
  Hub.carry.a(before, after, now, Nil{})

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

# a thread whose background shell an earlier restart left ended, outcome unknown
def Lost.stale(+on: Bool) -> M.State:
  M.State.replay([M.WorkSet{"t1", "bg", M.Bg.line("b1", M.Bg.ended.mark() ++ "unknown"), 5n}], Lost.demo(on, False{}))

# the agent of thread th exited
def Lost.exited(+s: M.State, +th: String) -> M.State:
  M.State.replay(Queue.changes(Hub.agent_gone(s, 9n, th, 0)), s)

# an idle thread t0 with a background shell and a running child task
def Lost.parent() -> M.State:
  M.State.replay([M.WorkSet{"t0", "bg", "b1\tsleep 60\tc1\tsleep 60\tshell", 3n}], Task.demo(False{}))

def Lost.open.go(ids: List<&2, String>, +s: M.State) -> Nat:
  match ids:
    case Nil{}:
      0n
    case +i <> t:
      Nat.add(M.Bg.running(M.Bg.of(s, i)), Lost.open.go(t, s))

# how many background rows are at work, over every thread
def Lost.open(+s: M.State) -> Nat:
  Lost.open.go(Rec.ids(Rec.threads(s)), s)

def Lost.held.go(ids: List<&2, String>, +s: M.State) -> Nat:
  match ids:
    case Nil{}:
      0n
    case +i <> t:
      Nat.add(M.Bg.live(M.Bg.of(s, i)), Lost.held.go(t, s))

# how many background rows are filed (ended ones too: they count as work)
def Lost.held(+s: M.State) -> Nat:
  Lost.held.go(Rec.ids(Rec.threads(s)), s)

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
  Nat.is_lt(C.Codec.pos(["orchestrator_capabilities", "task_status", "thread_list", "thread_read", "thread_wait", "thread_wait_now", "work_list", "project_list", "machine_list", "plane_read", "told_check", "task_board_list", "deploy_status"], tool), 13n)

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

# A turn left running though its agent's ledger shows nothing at work
# (CM.Cmds.idle: its last lifecycle line was lost, say in a restart's
# handoff) ends after Stuck.after() seconds of silence with no work out, as
# its ending would have ended it, with a note (laws stuck_*).
def Stuck.after() -> Nat:
  120n

def Stuck.code.k(run: Bool, +c: CM.Cmds, +heard: Nat, +now: Nat, +busy: Nat, +wait: Nat) -> Nat:
  match run:
    case True{}:
      Bool.pick(Nat, Bool.and(CM.Cmds.idle(c), Bool.and(Nat.is_eq(busy, 0n), Nat.is_ge(now, Nat.add(heard, wait)))), CM.Cmds.idle.end(c), 0n)
    case False{}:
      0n

# 0: nothing to do; else the verdict that ends the turn (3 ok, 4 failed,
# 5 stopped)
def Stuck.code(run: Bool, +c: CM.Cmds, +heard: Nat, +now: Nat, +busy: Nat) -> Nat:
  Stuck.code.k(run, c, heard, now, busy, Stuck.after())

# the same after a silence of wait seconds (BACKPLANE_STUCK_S, for tests)
def Stuck.code.w(run: Bool, +c: CM.Cmds, +heard: Nat, +now: Nat, +busy: Nat, +wait: Nat) -> Nat:
  Stuck.code.k(run, c, heard, now, busy, wait)

def Stuck.step(+s: M.State, +now: Nat, +thread: String, +code: Nat) -> Step:
  Step.before([M.ActivityLogged{thread, Hub.id("x", now, Hub.seq(s)), "info", "stuck", "Its agent finished its last command but its turn never ended: ended now", now}], Nil{},
    Hub.agent_line.v.k(code, s, now, thread, J.Null{}, ""))

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

# a restart that kept the agents of `kept` (adopt.bend): recovery and
# carrying on, as one step (for the laws)
def Adopt.recovered(+s: M.State, +now: Nat, +kept: List<&2, String>) -> Step:
  +r = Hub.recover.a(s, now, kept)
  Step.cat(r, Hub.carry.a(s, M.State.replay(Queue.changes(r), s), now, kept))

# whether changes end, start or post into thread th's turn
def Adopt.touches(cs: List<&2, M.Change>, +th: String) -> Bool:
  match cs:
    case Nil{}:
      False{}
    case M.TurnChanged{+t, _, _} <> rest:
      Bool.or(String.eq(t, th), Adopt.touches(rest, th))
    case M.MessagePosted{+t, _, _, _, _} <> rest:
      Bool.or(String.eq(t, th), Adopt.touches(rest, th))
    case _ <> rest:
      Adopt.touches(rest, th)

# the running scenario with more changes on top (for the carry laws)
def Carry.demo(cs: List<&2, M.Change>) -> M.State:
  M.State.replay(cs, Rec.demo(M.Running{}))

# a thread "t1" on `provider` whose turn is in state st (for laws)
def Demo.state(+provider: String, +st: M.TurnState) -> M.State:
  M.State.replay([M.ProjectCreated{"p1", "p", "/p", 1n}, M.ThreadCreated{"t1", "p1", "x", M.Local{}, "", "", provider, "", 1n}, M.TurnChanged{"t1", st, 2n}], M.State.empty())

# a Codex thread "t1" whose turn runs with one message waiting, "hi" (m1)
# (for the ride laws)
def Ride.demo() -> M.State:
  M.State.replay([M.SettingSet{RD.Ride.key("t1"), RD.Ride.add("", "m1", "hi"), 3n}], Demo.state("codex", M.Running{}))

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
    case TurnEnded{+t, +state, _, _, _, _} <> r:
      Bool.or(Bool.and(String.eq(t, th), String.eq(state, "stopped")), Ended.stopped(r, th))
    case _ <> r:
      Ended.stopped(r, th)

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

# t1's turn ends twice, t0 busy: both reports reach t0 at once, in order
def Lr.demo.twice.k(+s1: M.State) -> Nat:
  Lr.count(Queue.changes(Launch.tell(s1, 11n, "t1", Step{[M.TurnChanged{"t1", M.Completed{}, 11n}], Nil{}})), "t0")

def Lr.demo.twice() -> Nat:
  +s = Lr.demo.k(True{}, False{}, True{}, Nil{})
  +st = Launch.tell(s, 9n, "t1", Step{Lr.demo.end(), Nil{}})
  +s1 = M.State.replay(List.append(&2, M.Change, Queue.changes(st), [M.MessagePosted{"t1", "m2", M.User{}, "again", 10n}, M.TurnChanged{"t1", M.Running{}, 10n}]), s)
  Nat.add(Lr.count(Queue.changes(st), "t0"), Lr.demo.twice.k(s1))

# the loop's bound: t0 started t1's turn Lr.loops() times in a row
def Lr.demo.loop() -> M.State:
  Lr.demo.k(True{}, True{}, False{}, [M.SettingSet{Lr.n.key("t1"), Nat.show(Nat.sub(Lr.loops(), 1n)), 1n}])

def Lr.demo.loop.on() -> Bool:
  M.Launch.reports(M.State.replay(Queue.changes(Launch.tell(Lr.demo.loop(), 9n, "t1", Step{Lr.demo.end(), Nil{}})), Lr.demo.loop()), "t1")

# Alerts: the laws' examples
# --------------------------

# the asks a step opens (needs on, or off): the Alerted outs it gets
def Alerts.demo.asks(off: Bool) -> Nat:
  +s = M.State.replay(Bool.pick(List<&2, M.Change>, off, [M.SettingSet{"notify.needs", "off", 1n}], Nil{}), NO.Policy.demo.s(NO.Policy.demo.ask()))
  List.length(&2, Out, Alerts.outs(Alerts.changes(s, [M.AskOpened{"k1", "t1", "approval", "Bash", "ls", "", 20n}]), Nil{}))

def Alerts.demo.merge.k(st: Step) -> String:
  match st:
    case Step{+cs, _}:
      NO.Policy.said(cs) ++ " " ++ Nat.show(List.length(&2, Out, Alerts.outs(cs, Nil{})))

# a merge found: its record's kind and the Alerted outs of the step
def Alerts.demo.merge() -> String:
  +s = NO.Policy.demo.s(NO.Policy.demo.ask())
  Alerts.demo.merge.k(Merge.found(True{}, s, 30n, 0, "i", "t1", "http://x/pull/1"))

def Alerts.demo.end.k(st: Step) -> String:
  match st:
    case Step{+cs, _}:
      NO.Policy.said(cs) ++ " " ++ Nat.show(List.length(&2, Out, Alerts.outs(cs, Nil{})))

# a turn that ends (the person's message, a completed ready turn): the
# step's record and its Alerted out
def Alerts.demo.end() -> String:
  +s = NO.Policy.demo.s(NO.Policy.demo.cat([NO.Policy.demo.base(), NO.Policy.demo.run("t1", "m1", "do it", 10n)]))
  Alerts.demo.end.k(Ended.out(s, 13n, "t1", Step{NO.Policy.demo.done("t1", 12n), Nil{}}))

# Small replayed states used by the wake laws and tests.
def Wake.demo(+once: Bool, +events: List<&2, String>) -> M.State:
  M.State.replay([M.ThreadCreated{"tb", "", "Bot", M.Local{}, "", "", "claude", "", 1n}, M.BotSet{"bot", "Bot", "tb", 0n, "", 1n},
    M.ThreadCreated{"t1", "", "Fix DRC", M.Local{}, "", "", "claude", "", 1n}, M.TurnChanged{"t1", M.Running{}, 2n}, M.MessagePosted{"t1", "a1", M.Assistant{}, "DRC fixed", 3n},
    M.WakeSet{"w1", "tb", "thread", "t1", events, once, "review it", 4n}], M.State.empty())

def Wake.demo.end() -> List<&2, M.Change>:
  [M.TurnChanged{"t1", M.Completed{}, 9n}]

def Wake.demo.step(+s: M.State) -> Step:
  Launch.tell(s, 9n, "t1", Step{Wake.demo.end(), Nil{}})

def Wake.demo.count(+s: M.State) -> Nat:
  Lr.count(Queue.changes(Wake.demo.step(s)), "tb")

def Wake.demo.twice() -> Nat:
  +s = Wake.demo(False{}, ["end"])
  Wake.demo.count(M.State.replay(Queue.changes(Wake.demo.step(s)), s))

def Wake.demo.dropped() -> Nat:
  +s = Wake.demo(True{}, ["end"])
  List.length(&2, String, WK.Wake.by(M.State.replay(Queue.changes(Wake.demo.step(s)), s), "tb"))

def Wake.demo.pr() -> M.State:
  M.State.replay([M.WakeSet{"wp", "tb", "pr", "owner/repo#7", ["merged", "ci"], False{}, "", 4n}], Wake.demo(True{}, ["end"]))

def Wake.demo.merged() -> WK.PrSeen:
  WK.PrSeen{"MERGED", "now", "abc", "passed", ["pcb.kicad_pcb"], "7", "Route board", "checks-1", ""}

def Wake.demo.pr.twice() -> Nat:
  +s = Wake.demo.pr()
  +st = Wake.pr(s, 9n, "owner/repo#7", Wake.demo.merged())
  Lr.count(Queue.changes(Wake.pr(M.State.replay(Queue.changes(st), s), 10n, "owner/repo#7", Wake.demo.merged())), "tb")

def Wake.demo.partial() -> Nat:
  +s = Wake.demo(False{}, ["ask", "end", "done"])
  +cs = {M.AskOpened{"q1", "t1", "question", "AskUserQuestion", "review?", "", 9n} <> Wake.demo.end() : List<&2, M.Change>}
  +after = M.State.replay(Queue.changes(Launch.tell(s, 9n, "t1", Step{cs, Nil{}})), s)
  Lr.count(Queue.changes(Launch.tell(after, 9n, "t1", Step{Wake.demo.end(), Nil{}})), "tb")

# Compact fixtures for the lock laws: real state fold, grant path and work end.
def Locks.demo() -> M.State:
  M.State.replay([M.ProjectCreated{"p1", "P", "/p", 1n}, M.ThreadCreated{"ta", "p1", "A", M.Local{}, "", "", "claude", "", 1n}, M.ThreadCreated{"tb", "p1", "B", M.Local{}, "", "", "claude", "", 1n}], M.State.empty())

def Locks.demo.party(+id: String) -> LK.LockParty:
  LK.LockParty{id, "thread", id, "build", 2n, 0n}

# a client waiting behind a held lock, its lease about to lapse
def Locks.demo.client() -> M.State:
  M.State.replay([M.LockChanged{"bend", LK.Lock.acquire(LK.Lock.acquire(LK.Lock.empty(), Locks.demo.party("ta"), True{}), LK.LockParty{"pid:1", "client", "", "bp-lock.sh", 2n, 5n}, False{}), 3n}], Locks.demo())

def Locks.demo.recovered() -> M.State:
  +s = Locks.demo.client()
  M.State.replay(Queue.changes(Locks.recover(s, 6n)), s)

def Locks.demo.queue() -> M.State:
  M.State.replay([M.LockChanged{"bend", LK.Lock.acquire(LK.Lock.acquire(LK.Lock.empty(), Locks.demo.party("ta"), True{}), Locks.demo.party("tb"), False{}), 3n}], Locks.demo())

def Locks.demo.release() -> M.State:
  +s = Locks.demo.queue()
  M.State.replay(Queue.changes(Locks.after(s, 4n, Locks.release(s, 4n, "bend", "ta"))), s)

def Locks.demo.grant() -> Step:
  +s = Locks.demo.release()
  Locks.after(s, 5n, Locks.ready(s, 5n, "bend", "tb"))

def Locks.demo.twice() -> Nat:
  +s = M.State.replay(Queue.changes(Locks.demo.grant()), Locks.demo.release())
  Lr.count(Queue.changes(Locks.after(s, 6n, Locks.ready(s, 6n, "bend", "tb"))), "tb")

# a client the record never heard of calls status/renew: queued after the others
def Locks.demo.requeue(+op: String, +want: String) -> String:
  +s = Locks.demo.queue()
  +l = M.Locks.get(s, "bend")
  +s2 = M.State.replay(Queue.changes(Locks.http.step(op, s, 10n, "bend", want, "bp-lock.sh", False{})), s)
  +l2 = M.Locks.get(s2, "bend")
  Nat.show(LK.Lock.position(l2, want)) ++ ":" ++ Nat.show(LK.Lock.position(l2, "tb")) ++ ":" ++ Bool.pick(String, Locks.http.lost(op, l, want), "lost", "kept")

# the lost party's own calls keep it queued once (no second entry)
def Locks.demo.requeue.twice() -> Nat:
  +s = M.State.replay(Queue.changes(Locks.http.step("status", Locks.demo.queue(), 10n, "bend", "pid:9", "x", False{})), Locks.demo.queue())
  +s2 = M.State.replay(Queue.changes(Locks.http.step("status", s, 11n, "bend", "pid:9", "x", False{})), s)
  List.length(&2, LK.LockParty, LK.Lock.waiters(M.Locks.get(s2, "bend")))

def Locks.demo.lease() -> String:
  +s = Locks.demo.recovered()
  +s2 = M.State.replay(Queue.changes(Locks.after(s, 6n, Step.none())), s)
  +l = M.Locks.get(s2, "bend")
  Nat.show(LK.Lock.lease(Maybe.default(&2, LK.LockParty, List.head(&2, LK.LockParty, LK.Lock.waiters(l)), Locks.demo.party("none")))) ++ ":" ++ Nat.show(LK.Lock.position(l, "pid:1"))

def Locks.demo.deleted() -> String:
  +s = Locks.demo.queue()
  LK.Lock.owner(M.Locks.get(M.State.replay(Queue.changes(Locks.after(s, 4n, Step{[M.ThreadDeleted{"ta", 4n}], Nil{}})), s), "bend"))

def Locks.demo.work() -> M.State:
  +s = Locks.demo()
  +a = J.Json.obj([J.KV{"lock", J.Str{"bend"}}, J.KV{"command", J.Str{"true"}}])
  M.State.replay(Queue.changes(Locks.work.start(s, 3n, "ta", a, True{})), s)

def Locks.demo.work.end() -> String:
  +s = Locks.demo.work()
  LK.Lock.owner(M.Locks.get(M.State.replay(Queue.changes(Locks.after(s, 4n, Watch.end(s, 4n, "ta", Watch.id(Locks.demo(), "ta"), "exit 0", ""))), s), "bend"))

# ---- mail 2b invariant projections ----
def Mail.pending.count(+cs: List<&2, M.Change>) -> Nat:
  match cs:
    case Nil{}: 0n
    case M.MailPosted{_, _, _, _, _, _, _, _, _, _} <> +t: 1n+Mail.pending.count(t)
    case _ <> +t: Mail.pending.count(t)

def Mail.file.count(+st: Step) -> Nat:
  match st:
    case Step{_, MailAppend{_, _, +cs} <> Nil{}}: Mail.pending.count(cs)
    case _: 0n

def Mail.pending.conv(cs: List<&2, M.Change>) -> String:
  match cs:
    case Nil{}: ""
    case M.MailPosted{_, conv, _, _, _, _, _, _, _, _} <> _: conv
    case _ <> t: Mail.pending.conv(t)

def Mail.file.route(+st: Step) -> String:
  match st:
    case Step{_, MailAppend{_, _, cs} <> Nil{}}: Mail.pending.conv(cs)
    case _: ""

def Mail.file.waits(+st: Step) -> Bool:
  match st:
    case Step{Nil{}, MailAppend{_, _, _} <> Nil{}}: False{}
    case _: True{}

# Optional conversation/subject/want on the primitive apply to the room's
# single body/header, without changing its existing member deliveries.
def Mail.options.body(r: Maybe<&2, MB.Body>, +s: M.State, +p: J.Json) -> Step:
  match r:
    case None{}: Step.none()
    case Some{MB.Body{+c, ML.Mail{+id, _, +from, +to, +kind, +ref, +want, +head, +size, +at}, +text}}:
      +conv = Mail.or(J.Json.get_str(p, "conv", ""), ML.Conv.id(c))
      +old = ML.Box.conv(M.State.mail(s), conv)
      Mail.file.body(s, MB.Body{ML.Conv{conv, Mail.or(ML.Conv.subject(old), Mail.or(J.Json.get_str(p, "subject", ""), ML.Conv.subject(c))), ML.Conv.mail_kind(conv, kind),
        ML.Conv.parts(c), ML.Conv.project(c), ML.Conv.tags(c), ML.Conv.made(c), ML.Conv.last(c), ML.Conv.n(c)},
        ML.Mail{id, conv, from, to, kind, ref, ML.Mail.want(kind, Mail.or(J.Json.get_str(p, "want", ""), want), Mail.flags(ML.Origin{from, kind, ref}, text)), head, size, at}, text})

def Mail.options.outs(os: List<&2, Out>, +s: M.State, +p: J.Json) -> List<&2, Out>:
  match os:
    case Nil{}: Nil{}
    case MailAppend{line, _, _} <> rest:
      List.append(&2, Out, Step.outs_of(Mail.options.body(MB.Body.parse(line), s, p)), Mail.options.outs(rest, s, p))
    case o <> rest: o <> Mail.options.outs(rest, s, p)

def Mail.options(st: Step, s: M.State, p: J.Json) -> Step:
  match st:
    case Step{cs, os}: Step{cs, Mail.options.outs(os, s, p)}

# ---- mail 2c ----
# These operations never call the inbound funnel: escalation is the only
# mail path allowed to post into a bot's front of house.
def Run.alarm.on(on: Bool, +s: M.State, +now: Nat, +bot: String, +conv: String) -> List<&2, M.Change>:
  match on:
    case False{}: Nil{}
    case True{}:
      +text = Nat.show(ML.Box.live_n(M.State.mail(s), bot)) ++ " mailroom runs live"
      Run.row(s, now, bot, conv, text) <> Run.told(now, "mail-runs-" ++ bot, bot, text)

def Run.alarm(+s: M.State, +now: Nat, +bot: String, +conv: String) -> List<&2, M.Change>:
  Run.alarm.on(Bool.and(Nat.is_gt(ML.Box.live_n(M.State.mail(s), bot), 6n), Bool.not(Run.recent(s, now, "mail-runs-" ++ bot))), s, now, bot, conv)

# Short runs keep the bot's provider. Claude's role defaults must never
# escape into another provider after the bot switches. Reuse the same
# known-family compatibility policy as text-model settings; custom model
# names remain opaque. An empty model delegates to that provider's CLI.
def Run.model.keep(keep: Bool, model: String, fallback: String) -> String:
  match keep:
    case True{}: model
    case False{}: fallback

def Run.model.accept(+provider: String, +model: String) -> Bool:
  Bool.and(Bool.not(String.is_empty(model)), AG.Text.fits(provider, model))

def Run.model.default(claude: Bool, +provider: String, +parent: String, uber: Bool) -> String:
  match claude:
    case True{}: Bool.pick(String, uber, "opus", "sonnet")
    case False{}: Run.model.keep(Run.model.accept(provider, parent), parent, "")

def Run.model.pick(+provider: String, +parent: String, +override: String, uber: Bool) -> String:
  Run.model.keep(Run.model.accept(provider, override), override,
    Run.model.default(String.eq(provider, "claude"), provider, parent, uber))

def Run.provider(+s: M.State, +bot: String) -> String:
  Run.model.keep(TRI.Triage.owner(s, bot), String.trim(M.Setting.get(s, "run.provider." ++ bot, AG.Text.provider(s))), Hub.provider(Hub.thread(s, BT.Bot.thread(BT.World.bot(M.State.bots(s), bot)))))

def Run.triage.model(on: Bool, +s: M.State, +bot: String, old: String) -> String:
  match on:
    case False{}: old
    case True{}:
      +p = Run.provider(s, bot)
      +configured = String.trim(M.Setting.get(s, "run.model." ++ bot, ""))
      Run.model.keep(Run.model.accept(p, configured), configured, Run.model.keep(String.eq(p, AG.Text.provider(s)), AG.Text.model(s), ""))

def Run.model(+s: M.State, +bot: String) -> String:
  +parent = Hub.thread(s, BT.Bot.thread(BT.World.bot(M.State.bots(s), bot)))
  Run.triage.model(TRI.Triage.owner(s, bot), s, bot, Run.model.pick(Hub.provider(parent), String.trim(Turn.model(parent)), String.trim(M.Setting.get(s, "run.model." ++ bot, "")), String.eq(M.Setting.get(s, "uber.bot", ""), bot)))

def Run.spawn.new(+s: M.State, +now: Nat, +bot: String, +conv: String, +body: MB.Body, +th: String) -> Step:
  +b = BT.World.bot(M.State.bots(s), bot)
  +parent = Hub.thread(s, BT.Bot.thread(b))
  +model = Run.model(s, bot)
  +cs = {[M.ThreadCreated{th, ML.Conv.project(MB.Body.conv(body)), BT.Bot.name(b) ++ ": " ++ ML.Conv.subject(MB.Body.conv(body)), M.Local{}, "", Hub.cwd(s, parent), Run.provider(s, bot), model, now},
    M.WorktreeSet{th, "", Hub.cwd(s, parent), now}, M.ModesSet{th, M.Supervised{}, M.DefaultMode{}, model, now}, M.RunSet{th, bot, conv, ML.Mail.id(MB.Body.mail(body)), now}, M.SettingSet{"run.pending." ++ th, "on", now}] : List<&2, M.Change>}
  +bound = List.append(&2, M.Change, cs, Bool.pick(List<&2, M.Change>, TRI.Triage.owner(s, bot), [M.SettingSet{"run.bound." ++ th, ML.Mail.id(MB.Body.mail(body)), now}], Nil{}))
  +s1 = M.State.replay(bound, s)
  Step{List.append(&2, M.Change, bound, Run.alarm(s1, now, bot, conv)), [RunStart{th, conv, MR.Run.brief(s1, bot, conv)}]}

# Only a fresh run receives queued mail. Starting it and removing the ID
# are one commit; the existing RunStart effect reads its exact journal body.
def Run.queue.got(m: Maybe<&2, ML.Mail>, +s: M.State, +now: Nat, +bot: String, +conv: String, +rest: List<&2, String>) -> Step:
  match m:
    case None{}: Run.triage.send(String.is_empty(M.Setting.get(s, TRI.Triage.key(bot, "queue-missing:" ++ conv), "")), s, now, bot, conv, "Private mail queue has a missing header; evidence is unavailable. Review the conversation privately; no work or approval was changed.", "queue-missing:" ++ conv)
    case Some{m}:
      +cs = {[TRI.Triage.queue.set(bot, conv, rest, now)] : List<&2, M.Change>}
      +next = M.State.replay(cs, s)
      Step.before(cs, Nil{}, Run.spawn.new(next, now, bot, conv, MB.Body{ML.Box.conv(M.State.mail(next), conv), m, ""}, Hub.id("run", now, Hub.seq(next))))

def Run.queue.ids(ids: List<&2, String>, +s: M.State, +now: Nat, +bot: String, +conv: String) -> Step:
  match ids:
    case Nil{}: Step.none()
    case id <> rest: Run.queue.got(TRI.Triage.queue.mail(ML.Box.headers(M.State.mail(s), conv), id), s, now, bot, conv, rest)

def Run.queue.ready(ready: Bool, +s: M.State, +now: Nat, +bot: String, +conv: String) -> Step:
  match ready:
    case False{}: Step.none()
    case True{}: Run.queue.ids(TRI.Triage.queue(s, bot, conv), s, now, bot, conv)

def Run.queue.start(+s: M.State, +now: Nat, +bot: String, +conv: String) -> Step:
  Run.queue.ready(Bool.and(Bool.not(TRI.Triage.held(s, bot, conv)), Bool.and(TRI.Triage.owner(s, bot), String.is_empty(ML.Box.live(M.State.mail(s), bot, conv)))), s, now, bot, conv)

def Run.queue.enqueue(+s: M.State, +now: Nat, +body: MB.Body, +bot: String) -> Step:
  +conv = ML.Mail.conv(MB.Body.mail(body))
  +id = ML.Mail.id(MB.Body.mail(body))
  +ids = TRI.Triage.queue(s, bot, conv)
  +cs = {[TRI.Triage.queue.set(bot, conv, Bool.pick(List<&2, String>, ML.Parts.has(ids, id, False{}), ids, List.append(&2, String, ids, [id])), now)] : List<&2, M.Change>}
  Step.before(cs, Nil{}, Run.queue.start(M.State.replay(cs, s), now, bot, conv))

def Run.queue.admit.ready(direct: Bool, +s: M.State, +now: Nat, +body: MB.Body, +bot: String) -> Step:
  match direct:
    case True{}: Run.spawn.new(s, now, bot, ML.Mail.conv(MB.Body.mail(body)), body, Hub.id("run", now, Hub.seq(s)))
    case False{}: Run.queue.enqueue(s, now, body, bot)

def Run.queue.admit(+s: M.State, +now: Nat, +body: MB.Body, +bot: String) -> Step:
  +conv = ML.Mail.conv(MB.Body.mail(body))
  +live = ML.Box.live(M.State.mail(s), bot, conv)
  Bool.pick(Step, String.eq(M.Setting.get(s, "run.bound." ++ live, ""), ML.Mail.id(MB.Body.mail(body))), Step.none(),
    Run.queue.admit.ready(Bool.and(Bool.not(TRI.Triage.held(s, bot, conv)), Bool.and(String.is_empty(live), List.is_empty(&2, String, TRI.Triage.queue(s, bot, conv)))), s, now, body, bot))

def Run.deliver.current(r: Maybe<&2, ML.Run>, +s: M.State, fallback: String) -> String:
  match r:
    case None{}: fallback
    case Some{r}: MD.Disposition.current(s, r)

def Run.deliver.ready(+s: M.State, +now: Nat, +th: String, +body: MB.Body) -> Step:
  +m = MB.Body.mail(body)
  Origin.step(Hub.deliver(Hub.running(Hub.thread(s, th)), M.Worktree.waits(s, Hub.thread(s, th), now), s, now, th, ML.Mail.id(m), M.User{},
    "[mail " ++ ML.Mail.conv(m) ++ " · " ++ ML.Mail.kind(m) ++ " from " ++ Mail.name(s, ML.Mail.from(m)) ++ "]\nCurrent actionable input: " ++ ML.Mail.id(m) ++ "\n" ++ MB.Body.text(body)), th, ML.Mail.id(m), ML.Origin{ML.Mail.from(m), ML.Mail.kind(m), ML.Mail.conv(m)}, now)

# Server has loaded the last 20 bodies, newest first. Keep the most
# recent text first so a character cap discards the oldest mail first.
def Run.bodies(+bs: List<&2, MB.Body>) -> String:
  match bs:
    case Nil{}: ""
    case +b <> +t: "\n[mail " ++ ML.Mail.id(MB.Body.mail(b)) ++ " from " ++ ML.Mail.from(MB.Body.mail(b)) ++ "]\n" ++ String.take(MB.Body.text(b), 4000n) ++ Run.bodies(t)

def Run.deliver.pending(pending: Bool, +s: M.State, +now: Nat, +th: String, +body: MB.Body) -> Step:
  match pending:
    case False{}: Run.deliver.ready(s, now, th, body)
    case True{}: Step{[M.MessagePosted{th, ML.Mail.id(MB.Body.mail(body)), M.User{}, "Received input: " ++ ML.Mail.id(MB.Body.mail(body)) ++ "\nCurrent actionable input: " ++ Run.deliver.current(MR.Run.of(s, th), s, ML.Mail.id(MB.Body.mail(body))) ++ "\n" ++ MB.Body.text(body), now}, M.MessageFrom{th, ML.Mail.id(MB.Body.mail(body)), ML.Mail.from(MB.Body.mail(body)), ML.Mail.kind(MB.Body.mail(body)), ML.Mail.conv(MB.Body.mail(body)), now}], Nil{}}

def Run.deliver(+s: M.State, +now: Nat, +th: String, +body: MB.Body) -> Step:
  Run.deliver.pending(String.eq(M.Setting.get(s, "run.pending." ++ th, ""), "on"), s, now, th, body)

def Run.spawn.live(empty: Bool, +s: M.State, +now: Nat, +bot: String, +conv: String, +body: MB.Body, +live: String) -> Step:
  match empty:
    case True{}: Run.spawn.new(s, now, bot, conv, body, Hub.id("run", now, Hub.seq(s)))
    case False{}: Run.deliver(s, now, live, body)

def Run.spawn(+s: M.State, +now: Nat, +bot: String, +conv: String, +body: MB.Body) -> Step:
  +live = ML.Box.live(M.State.mail(s), bot, conv)
  Run.spawn.live(String.is_empty(live), s, now, bot, conv, body, live)

def Mail.route_bot.spawn(owned: Bool, +s: M.State, +now: Nat, +body: MB.Body, +bot: String) -> Step:
  match owned:
    case True{}: Run.queue.admit(s, now, body, bot)
    case False{}: Run.spawn(s, now, bot, ML.Mail.conv(MB.Body.mail(body)), body)

def Mail.route_bot.want(act: Bool, +s: M.State, +now: Nat, +body: MB.Body, +bot: String) -> Step:
  match act:
    case False{}: Step.none()
    case True{}: Mail.route_bot.spawn(TRI.Triage.owner(s, bot), s, now, body, bot)

def Mail.route_bot.plan(p: MR.MailRunPlan, +s: M.State, +now: Nat, +body: MB.Body, +bot: String) -> Step:
  match p:
    case MR.FileOnly{}: Step.none()
    case MR.NewRun{}: Run.spawn(s, now, bot, ML.Mail.conv(MB.Body.mail(body)), body)
    case MR.IntoRun{live}: Run.deliver(s, now, live, body)

def Mail.route_bot.owned(owned: Bool, +s: M.State, +now: Nat, +body: MB.Body, +bot: String) -> Step:
  match owned:
    case True{}: Mail.route_bot.want(TRI.Triage.inbound(MB.Body.mail(body), bot), s, now, body, bot)
    case False{}: Mail.route_bot.plan(MR.Run.plan(ML.Mail.wanted(MB.Body.mail(body)), ML.Box.live(M.State.mail(s), bot, ML.Mail.conv(MB.Body.mail(body)))), s, now, body, bot)

def Mail.route_bot(+s: M.State, +now: Nat, +body: MB.Body, +bot: String) -> Step:
  Mail.route_bot.owned(TRI.Triage.owner(s, bot), s, now, body, bot)

def Mail.route.part(r: Maybe<&2, REF.Ref>, +s: M.State, +now: Nat, +body: MB.Body) -> Step:
  match r:
    case Some{REF.Ref{"bot", +bot <> _, _}}:
      Mail.route_bot.want(Bool.and(MD.Runs.on(s, bot), Bool.and(Bool.not(String.eq(ML.Mail.from(MB.Body.mail(body)), "bot:" ++ bot)), Bool.or(TRI.Triage.owner(s, bot), Bool.or(String.eq(ML.Mail.wanted(MB.Body.mail(body)), "act"), String.contains(MB.Body.text(body), "@" ++ BT.Bot.name(BT.World.bot(M.State.bots(s), bot))))))), s, now, body, bot)
    case _: Step.none()

# The journal commits first. This pure router is called once, only after
# a fresh header was durably written; replays and retries do not run it.
def Mail.route.parts(+parts: List<&2, String>, +s: M.State, +now: Nat, +body: MB.Body, +st: Step) -> Step:
  match parts:
    case Nil{}: st
    case +p <> +t: Mail.route.parts(t, s, now, body, Step.cat(st, Mail.route.part(REF.Ref.of_addr(p), M.State.replay(Queue.changes(st), s), now, body)))

def Mail.route.got(r: Maybe<&2, REF.Ref>, +s: M.State, +now: Nat, +body: MB.Body) -> Step:
  match r:
    case Some{REF.Ref{"bot", bot <> _, _}}: Mail.route_bot(s, now, body, bot)
    case Some{REF.Ref{"room", _, _}}: Mail.route.parts(ML.Conv.parts(MB.Body.conv(body)), s, now, body, Step.none())
    case _: Step.none()

def Mail.route.enabled(+s: M.State, +now: Nat, +body: MB.Body) -> Step:
  Mail.route.got(REF.Ref.of_addr(ML.Mail.to(MB.Body.mail(body))), s, now, body)

def Run.pending.text(+es: List<&2, M.Entry>) -> String:
  match es:
    case Nil{}: ""
    case M.Msg{_, _, _, +text, _} <> +t: String.take(text, 2000n) ++ "\n" ++ Run.pending.text(t)
    case _ <> +t: Run.pending.text(t)

# A first turn never resumes a provider session, even if a torn or old
# state happened to attach one between RunSet and its body read.
def Run.fresh.out(o: Out) -> Out:
  match o:
    case AgentStart{t, cwd, p, m, _, r, i}: AgentStart{t, cwd, p, m, "", r, i}
    case CodexTurn{t, cwd, m, _, r, text}: CodexTurn{t, cwd, m, "", r, text}
    case GrokTurn{t, cwd, m, _, r, text}: GrokTurn{t, cwd, m, "", r, text}
    case MuseTurn{t, cwd, m, _, r, text}: MuseTurn{t, cwd, m, "", r, text}
    case AgyTurn{t, cwd, m, _, r, text}: AgyTurn{t, cwd, m, "", r, text}
    case OcTurn{t, cwd, m, _, r, text}: OcTurn{t, cwd, m, "", r, text}
    case PiTurn{t, cwd, m, _, r, text}: PiTurn{t, cwd, m, "", r, text}
    case OmpTurn{t, cwd, m, _, r, text}: OmpTurn{t, cwd, m, "", r, text}
    case other: other

def Run.fresh.outs(+os: List<&2, Out>) -> List<&2, Out>:
  match os:
    case Nil{}: Nil{}
    case +o <> +t: Run.fresh.out(o) <> Run.fresh.outs(t)

def Run.fresh(st: Step) -> Step:
  match st:
    case Step{cs, os}: Step{cs, Run.fresh.outs(os)}

def Run.start(+s: M.State, +now: Nat, +th: String, +head: String, +bs: List<&2, MB.Body>) -> Step:
  Run.fresh(Step.before([M.SettingSet{"run.pending." ++ th, "", now}], Nil{}, Hub.post(s, now, th, Hub.id("brief", now, Hub.seq(s)), M.User{}, head ++ String.take(Run.pending.text(M.State.mine(s, th)), 2000n) ++ "\n<mail_history newest-first>\n" ++ String.take(Run.bodies(bs), 8000n) ++ "\n</mail_history>")))

def Mail.route.triage(r: Maybe<&2, REF.Ref>, +s: M.State, +now: Nat, +body: MB.Body) -> Step:
  match r:
    case Some{REF.Ref{"bot", +bot <> _, _}}: Mail.route_bot.want(Bool.and(TRI.Triage.owner(s, bot), TRI.Triage.inbound(MB.Body.mail(body), bot)), s, now, body, bot)
    case Some{REF.Ref{"room", _, _}}: Mail.route.parts(ML.Conv.parts(MB.Body.conv(body)), s, now, body, Step.none())
    case _: Step.none()

def Mail.route.on(on: Bool, +s: M.State, +now: Nat, +body: MB.Body) -> Step:
  match on:
    case False{}: Mail.route.triage(REF.Ref.of_addr(ML.Mail.to(MB.Body.mail(body))), s, now, body)
    case True{}: Mail.route.enabled(s, now, body)

def Mail.route(+s: M.State, +now: Nat, +body: MB.Body) -> Step:
  Mail.route.on(String.eq(M.Setting.get(s, "mail.on", "off"), "on"), s, now, body)

def Run.ready.got(r: Maybe<&2, ML.Run>, pending: Bool) -> Bool:
  match r:
    case None{}: False{}
    case Some{+r}: Bool.and(ML.Run.live(r), pending)

def Run.ready(+s: M.State, +th: String) -> Bool:
  Run.ready.got(MR.Run.of(s, th), String.eq(M.Setting.get(s, "run.pending." ++ th, ""), "on"))

def Run.start.on(ready: Bool, +s: M.State, +now: Nat, +th: String, +head: String, +bs: List<&2, MB.Body>) -> Step:
  match ready:
    case False{}: Step.none()
    case True{}: Run.start(s, now, th, head, bs)

# ---- global provider fallback (single owner) ----
# All start effects pass Fallback.after in the server, after bot/mail/wake
# routing. There is no separate bot policy and no process signalling here.

def Fallback.target(t: M.Thread) -> FB.Target:
  match t:
    case M.Thread{_, _, _, _, _, _, _, _, _, _, _, _, _, _, _, p, m, _, _, _, _}: FB.Target{p, m}

def Fallback.pending(+s: M.State, +th: String) -> J.Json:
  Maybe.default(&2, J.Json, J.Json.parse(M.Setting.get(s, "fallback.pending." ++ th, "")), J.Null{})

def Fallback.continue(+s: M.State, +th: String) -> String:
  "[Backplane] The provider reached its usage limit. Continue the interrupted work from its current state. Read the handed-over tool results before acting. Do not repeat completed tool actions or restart finished work. If the request is already satisfied, report that.\n\nOriginal request (context, not a new task):\n" ++ M.Setting.get(s, "fallback.input." ++ th, "")

def Fallback.limited(+s: M.State, +now: Nat, +th: String, +lim: FB.Limit) -> Step:
  +cs = {List.append(&2, M.Change,
    Bool.pick(List<&2, M.Change>, FB.Limit.active(s, FB.Limit.scope(lim), now), Nil{}, [M.SettingSet{FB.Limit.key(FB.Limit.scope(lim)), FB.Limit.json(lim, now), now}]),
    [M.SettingSet{"fallback.pending." ++ th, "", now}, M.SettingSet{"fallback.continuing." ++ th, "yes", now}, M.TurnChanged{th, M.Interrupted{}, now}, M.QueueTaken{th, now}, M.TurnChanged{th, M.Running{}, now}]) : List<&2, M.Change>}
  Step{cs, AgentStop{th} <> Hub.run(Hub.thread(s, th), Hub.cwd(s, Hub.thread(s, th)), M.Queue.join(Fallback.continue(s, th), M.Queue.of(s, th)), Hub.uuid(now, th ++ "/fallback/" ++ Nat.show(Hub.seq(s))))}

# A reset retry belongs to the interrupted thread, independently of whether
# a fallback chain was configured. Unknown or expired reset evidence never
# invents a retry time. Persist the marker before scheduling any continuation.
def ProviderResume.mark(+s: M.State, +now: Nat, +th: String, +lim: FB.Limit) -> List<&2, M.Change>:
  [M.SettingSet{"provider.resume." ++ th,
    J.Json.show(J.Json.obj([J.KV{"target", J.Str{FB.Target.key(Fallback.target(Hub.thread(s, th)))}},
      J.KV{"reset", J.Json.nat(Bool.pick(Nat, Nat.is_gt(FB.Limit.reset(lim), now), FB.Limit.reset(lim), 0n))},
      J.KV{"reason", J.Str{String.take(FB.Limit.reason(lim), 400n)}}])), now}]

def ProviderResume.hold(+s: M.State, +now: Nat, +th: String, +lim: FB.Limit, normal: Step) -> Step:
  Step{List.append(&2, M.Change, ProviderResume.mark(s, now, th, lim),
    [M.TurnChanged{th, M.Interrupted{}, now}, M.SettingSet{"fallback.pending." ++ th, "", now},
      M.ActivityLogged{th, Hub.id("quota", now, Hub.seq(s)), "warn", "provider-wait", "Provider limit reached; " ++
        Bool.pick(String, Nat.is_gt(FB.Limit.reset(lim), now), "will continue when the reported reset arrives.", "no future reset supplied; waiting for updated provider evidence or your action."), now}]), [ToolsGone{th}]}

def ProviderResume.pending(has: Bool, +s: M.State, +now: Nat, +th: String, +o: J.Json, normal: Step) -> Step:
  match has:
    case False{}: normal
    case True{}: ProviderResume.hold(s, now, th, FB.Limit{J.Json.get_str(o, "scope", ""), J.Json.get_str(o, "reason", "usage limit reached"), J.Json.get_nat(o, "reset", 0n)}, normal)

def ProviderResume.line.m(lim: Maybe<&2, FB.Limit>, terminal: Bool, +s: M.State, +now: Nat, +th: String, normal: Step) -> Step:
  match lim terminal:
    case Some{+lim} True{}: ProviderResume.hold(s, now, th, lim, normal)
    case Some{+lim} False{}: Step.after(normal, [M.SettingSet{"fallback.pending." ++ th, FB.Limit.json(lim, now), now}])
    case None{} True{}:
      +o = Fallback.pending(s, th)
      +scope = J.Json.get_str(o, "scope", "")
      ProviderResume.pending(Bool.not(String.is_empty(scope)), s, now, th, o, normal)
    case None{} False{}: normal

def Fallback.line.limit(terminal: Bool, +s: M.State, +now: Nat, +th: String, +lim: FB.Limit) -> Step:
  match terminal:
    case True{}: Fallback.limited(s, now, th, lim)
    case False{}: Step{[M.SettingSet{"fallback.pending." ++ th, FB.Limit.json(lim, now), now}], Nil{}}

def Fallback.line.m(lim: Maybe<&2, FB.Limit>, terminal: Bool, s: M.State, now: Nat, th: String, normal: Step) -> Step:
  match lim:
    case Some{l}: Fallback.line.limit(terminal, s, now, th, l)
    case None{}: normal

def Fallback.line.pending(has: Bool, +s: M.State, +now: Nat, +th: String, +o: J.Json, normal: Step) -> Step:
  match has:
    case True{}:
      Fallback.limited(s, now, th, FB.Limit{J.Json.get_str(o, "scope", ""), J.Json.get_str(o, "reason", "usage limit reached"), J.Json.get_nat(o, "reset", 0n)})
    case False{}: normal

def Fallback.line.on(on: Bool, +terminal: Bool, +s: M.State, +now: Nat, +th: String, +o: J.Json, normal: Step) -> Step:
  match on:
    case True{}:
      Fallback.line.m(FB.Error.classify(Hub.provider(Hub.thread(s, th)), FB.Target.model(Fallback.target(Hub.thread(s, th))), o), terminal, s, now, th,
        Fallback.line.pending(Bool.and(terminal, Bool.not(String.is_empty(J.Json.get_str(Fallback.pending(s, th), "scope", "")))), s, now, th, Fallback.pending(s, th), normal))
    case False{}: ProviderResume.line.m(FB.Error.classify(Hub.provider(Hub.thread(s, th)), FB.Target.model(Fallback.target(Hub.thread(s, th))), o), terminal, s, now, th, normal)

def Fallback.line.live(live: Bool, +s: M.State, +now: Nat, +th: String, +o: J.Json, +terminal: Bool, normal: Step) -> Step:
  match live:
    case False{}: normal
    case True{}: Fallback.line.on(Bool.and(Bool.not(ACCT.Accounts.named(s, th)), FB.Chain.on(s)), terminal, s, now, th, o, normal)

def Fallback.line(+s: M.State, +now: Nat, +th: String, +o: J.Json, terminal: Bool, normal: Step) -> Step:
  Fallback.line.live(Bool.and(Hub.running(Hub.thread(s, th)), String.is_empty(J.Json.get_str(o, "parent_tool_use_id", ""))), s, now, th, o, terminal, normal)

# One durable alert per outage, plus an audit row in every affected thread.
# The count includes active threads on the same unavailable scope at discovery.
def Fallback.count(ts: List<&2, M.Thread>, +scope: String, +n: Nat) -> Nat:
  match ts:
    case Nil{}: n
    case +t <> rest:
      Fallback.count(rest, scope, Bool.pick(Nat, Bool.and(Hub.running(t), Bool.or(String.eq(FB.Target.provider(Fallback.target(t)), scope), String.eq(FB.Target.key(Fallback.target(t)), scope))), 1n+n, n))

def Fallback.scope(+s: M.State, +t: FB.Target, +now: Nat) -> String:
  Bool.pick(String, FB.Limit.active(s, FB.Target.provider(t), now), FB.Target.provider(t), FB.Target.key(t))

def Fallback.reason(+s: M.State, +scope: String) -> String:
  J.Json.get_str(FB.Limit.record(s, scope), "reason", "usage limit reached")

def Fallback.reset(+s: M.State, +scope: String) -> String:
  +n = J.Json.get_nat(FB.Limit.record(s, scope), "reset", 0n)
  Bool.pick(String, Nat.is_eq(n, 0n), "; reset not supplied", "; reset " ++ TZ.Tz.iso(TZ.Tz.utc0(), n))

def Fallback.alert.k(once: Bool, +s: M.State, +now: Nat, +th: String, +scope: String, +text: String, +blocked: Bool) -> List<&2, M.Change>:
  match once:
    case True{}:
      [M.SettingSet{"fallback.alerted." ++ scope ++ Bool.pick(String, blocked, ".blocked", ""), M.Setting.get(s, FB.Limit.key(scope), ""), now},
        NO.Alert.set(s, now, th, Bool.pick(String, blocked, "fallback-blocked", "fallback"), "fallback-" ++ scope ++ Bool.pick(String, blocked, "-blocked", ""), NO.Alert.memory(s, th), text)]
    case False{}: Nil{}

def Fallback.alert.scope(+s: M.State, +now: Nat, +th: String, +scope: String, text: String, +blocked: Bool) -> List<&2, M.Change>:
  Fallback.alert.k(Bool.not(String.eq(M.Setting.get(s, "fallback.alerted." ++ scope ++ Bool.pick(String, blocked, ".blocked", ""), ""), M.Setting.get(s, FB.Limit.key(scope), ""))), s, now, th, scope, text, blocked)

def Fallback.say(+s: M.State, +now: Nat, +th: String, +from: FB.Target, +to: String, +blocked: Bool) -> String:
  +sc = Fallback.scope(s, from, now)
  Bool.pick(String, blocked, "Action needed: all configured fallbacks are unavailable", "Automatic fallback: " ++ FB.Target.key(from) ++ " → " ++ to)
    ++ ". " ++ Lr.title(Hub.thread(s, th)) ++ " (" ++ th ++ "): " ++ Fallback.reason(s, sc) ++ Fallback.reset(s, sc)
    ++ ". " ++ Nat.show(Fallback.count(Rec.threads(s), sc, 0n)) ++ " active threads affected."

# Process starts sequentially against the state including earlier switches,
# so many bots on one outage produce one alert, even in the same hub step.
def Fallback.outs.next(st: Step, rest: Step) -> Step:
  Step.cat(st, rest)

def Fallback.claude.uuid(o: Out) -> String:
  match o:
    case AgentSend{_, line}: J.Json.get_str(Maybe.default(&2, J.Json, J.Json.parse(line), J.Null{}), "uuid", "")
    case Git{_, _, "attach.claude", _, _, _, uuid}: uuid
    case _: ""

# ---- local credential recovery ----
# A local locked keyring is not a provider outage. Keep the exact unsent
# prompt queued, notify once for the episode, and only resume after an actual
# unlocked probe. Unknown state never counts as proof of unlock.
def Auth.waiting(+s: M.State, +th: String) -> Bool:
  Bool.not(String.is_empty(M.Setting.get(s, "auth.wait." ++ th, "")))

def Auth.any(ts: List<&2, M.Thread>, +s: M.State) -> Bool:
  match ts:
    case Nil{}: False{}
    case t <> rest: Bool.or(Auth.waiting(s, M.Thread.id(t)), Auth.any(rest, s))

def Auth.hold(+s: M.State, +now: Nat, +th: String, +text: String, +why: String) -> Step:
  Step{List.append(&2, M.Change,
    [M.SettingSet{"auth.wait." ++ th, FB.Target.key(Fallback.target(Hub.thread(s, th))), now},
      M.TurnChanged{th, M.Interrupted{}, now}, M.TurnQueued{th, "auth:held:" ++ Nat.show(Hub.seq(s)), text, now}, M.QueueMoved{th, "auth:held:" ++ Nat.show(Hub.seq(s)), 0n, now},
      M.ActivityLogged{th, Hub.id("auth", now, Hub.seq(s)), "error", "auth-blocked", why, now}],
    Bool.pick(List<&2, M.Change>, String.eq(M.Setting.get(s, "auth.alerted.keyring", ""), "held"), Nil{},
      [M.SettingSet{"auth.alerted.keyring", "held", now}, NO.Alert.set(s, now, th, "auth-blocked", "auth-keyring", NO.Alert.memory(s, th), "Action needed: unlock the desktop login keyring. Credential-dependent threads are paused with their prompts saved; they resume once after unlock. " ++ why)])), Nil{}}

def Auth.recover.one(go: Bool, +s: M.State, +now: Nat, +th: String) -> Step:
  match go:
    case False{}: Step.none()
    case True{}:
      +cs = {[M.SettingSet{"auth.wait." ++ th, "", now}, M.ActivityLogged{th, Hub.id("auth", now, Hub.seq(s)), "info", "auth-resumed", "Login keyring unlocked; resuming the saved prompt once", now}] : List<&2, M.Change>}
      Step.before(cs, Nil{}, Wt.start(M.State.replay(cs, s), now, th, False{}))

def Auth.next(ts: List<&2, M.Thread>, +s: M.State, +found: String, done: Bool) -> String:
  match ts done:
    case _ True{}: found
    case Nil{} False{}: ""
    case +t <> rest False{}:
      +th = M.Thread.id(t)
      +go = Bool.and(Auth.waiting(s, th), Bool.and(Lost.live(t), Bool.not(Hub.running(t))))
      Auth.next(rest, s, Bool.pick(String, go, th, ""), go)

def Auth.recover.clear(+s: M.State, +now: Nat, +st: Step) -> Step:
  Step.after(st, Bool.pick(List<&2, M.Change>, Bool.and(Auth.any(Rec.threads(s), s), Bool.not(Auth.any(Rec.threads(s), M.State.replay(Queue.changes(st), s)))), [M.SettingSet{"auth.alerted.keyring", "", now}], Nil{}))

# Called only after the single in-flight probe of a five-second Beat.
# One paused turn is released; clearing its marker commits before start.
def Auth.recover(open: Bool, +s: M.State, +now: Nat) -> Step:
  match open:
    case False{}: Step.none()
    case True{}:
      +th = Auth.next(Rec.threads(s), s, "", False{})
      Auth.recover.clear(s, now, Auth.recover.one(Bool.not(String.is_empty(th)), s, now, th))

# Validate public Settings mutations against this hub's installed capabilities.
def Fallback.settings.ok(cs: List<&2, M.Change>, +s: M.State, +caps: J.Json) -> Bool:
  match cs:
    case Nil{}: True{}
    case M.SettingSet{"fallback.chain", +value, _} <> rest:
      Bool.and(Bool.and(Nat.is_le(String.length(value), 2000n), Nat.is_le(List.length(&2, FB.Target, FB.Chain.parse(value)), 16n)), Bool.and(FB.Chain.valid(FB.Chain.parse(value), caps), Fallback.settings.ok(rest, s, caps)))
    case M.SettingSet{"fallback.on", "on", _} <> rest:
      Bool.and(Bool.not(List.is_empty(&2, FB.Target, FB.Chain.of(s))), Bool.and(FB.Chain.valid(FB.Chain.of(s), caps), Fallback.settings.ok(rest, s, caps)))
    case _ <> rest: Fallback.settings.ok(rest, s, caps)

def Fallback.settings.errors(os: List<&2, Out>) -> List<&2, Out>:
  match os:
    case Nil{}: Nil{}
    case Send{client, +text} <> rest:
      Reply.err(client, J.Json.as_raw(J.Json.get(Maybe.default(&2, J.Json, J.Json.parse(text), J.Null{}), "id"), "0"), "Fallback hierarchy must contain at most 16 distinct installed provider/model entries. Choose a model from the installed list before enabling.") <> Fallback.settings.errors(rest)
    case _ <> rest: Fallback.settings.errors(rest)

def Auth.cancel(+s: M.State, +now: Nat, +th: String) -> List<&2, M.Change>:
  List.append(&2, M.Change, Bool.pick(List<&2, M.Change>, Auth.waiting(s, th), [M.SettingSet{"auth.wait." ++ th, "", now}], Nil{}),
    Bool.pick(List<&2, M.Change>, String.is_empty(M.Setting.get(s, "fallback.pending." ++ th, "")), Nil{}, [M.SettingSet{"fallback.pending." ++ th, "", now}]))

def AutoResume.cancel(+s: M.State, +now: Nat, +thread: String) -> List<&2, M.Change>:
  List.append(&2, M.Change,
    Bool.pick(List<&2, M.Change>, String.is_empty(M.Setting.get(s, "restart.retry." ++ thread, "")), Nil{}, [M.SettingSet{"restart.retry." ++ thread, "", now}]),
    Bool.pick(List<&2, M.Change>, String.is_empty(M.Setting.get(s, "provider.resume." ++ thread, "")), Nil{}, [M.SettingSet{"provider.resume." ++ thread, "", now}]))

def Hub.interrupt(+s: M.State, +now: Nat, +client: U32, +id: String, +thread: String) -> Step:
  Step.after(Hub.interrupt.k(Hub.running(Hub.thread(s, thread)), s, now, client, id, thread),
    List.append(&2, M.Change, Auth.cancel(s, now, thread), AutoResume.cancel(s, now, thread)))

# an archived thread waits on no clock: archiving a snoozed one wakes it, so
# its row and header no longer say it is snoozed (law archive_wakes)
def Archive.wake.k(snoozed: Bool, +now: Nat, +th: String) -> List<&2, M.Change>:
  match snoozed:
    case True{}:
      [M.ThreadSnoozed{th, 0n, now}]
    case False{}:
      Nil{}

def Archive.wake(+until: Nat, +now: Nat, +th: String) -> List<&2, M.Change>:
  Archive.wake.k(M.Snooze.on(now, until), now, th)

def Archive.changes(+s: M.State, +now: Nat, +th: String) -> List<&2, M.Change>:
  M.ThreadArchived{th, True{}, now} <> Archive.wake(M.Nav.until(M.Nav.get(M.State.navs(s), th)), now, th)

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
      Step.cat(Retire.archive(s, now, J.Json.get_str(p, "thread", ""), Step{Archive.changes(s, now, J.Json.get_str(p, "thread", "")), [Reply.ok(client, id), AgentStop{J.Json.get_str(p, "thread", "")}]}), Watch.retire(s, now, J.Json.get_str(p, "thread", "")))
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
      Step.outs([Reply.ok(client, id), Watch{client, J.Json.get_str(p, "kind", ""), J.Json.get_str(p, "root", ""), J.Json.get_str(p, "path", ""), J.Json.get_str(p, "tree", ""), J.Json.get_str(p, "sheet", "")}])
    case 14n:
      Hub.modes(s, now, client, id, p, Hub.thread(s, J.Json.get_str(p, "thread", "")))
    case 15n:
      Update.ask(Nat.is_gt(M.Work.threads(s), 0n), now, client, id)
    case 16n:
      Step{[M.SettingSet{J.Json.get_str(p, "key", ""), J.Json.get_str(p, "value", ""), now}], Bool.pick(List<&2, Out>, String.eq(J.Json.get_str(p, "key", ""), "kicad.path"), [Reply.ok(client, id), KiCadRetry{}], [Reply.ok(client, id)])}
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
      Hub.switch.as(s, now, client, id, J.Json.get_str(p, "thread", ""), J.Json.get_str(p, "provider", ""), J.Json.get_str(p, "model", ""), Switch.effort.of(J.Json.get(p, "effort")))
    case 30n:
      Step{[M.SettingSet{"update.wait", "", now}], [Reply.ok(client, id), UpgradeForce{}]}
    # the person stops watched work or a delegated task from its row
    case 31n:
      Step.before(Nil{}, [Reply.ok(client, id)], Watch.cancel(s, now, J.Json.get_str(p, "thread", ""), J.Json.get_str(p, "id", "")))
    case 37n:
      Step.before(Nil{}, [Reply.ok(client, id)], Locks.release(s, now, J.Json.get_str(p, "name", ""), J.Json.get_str(p, "thread", "")))
    case 39n:
      Step.outs([Reply.ok(client, id), KiCadRetry{}])
    case 38n:
      Step.before(Nil{}, [Reply.ok(client, id)], Locks.cancel(s, now, J.Json.get_str(p, "name", ""), J.Json.get_str(p, "thread", "")))
    case 33n:
      Step.reply(client, id, Bool.pick(List<&2, M.Change>, String.eq(J.Json.get_str(WK.Wake.get(s, J.Json.get_str(p, "id", "")), "by", ""), J.Json.get_str(p, "thread", "")), [M.WakeDropped{J.Json.get_str(p, "id", ""), now}], Nil{}))
    case 32n:
      Step.before(Nil{}, [Reply.ok(client, id)], Task.cancel.user(Maybe.default(&2, M.Task, Task.of_child(s, J.Json.get_str(p, "child", "")), M.Task.blank("")), s, now))
    case 34n:
      Deploy.restart(Deploy.stage.ok(J.Json.get_str(p, "stage", "")), s, now, client, id, J.Json.get_str(p, "stage", ""), J.Json.get_str(p, "note", ""))
    case 35n:
      Step.before(Nil{}, [Reply.ok(client, id)], Deploy.cancel(s, now))
    case 36n:
      Step.outs([Reply.data(client, id, DP.Deploy.status(s))])
    case 40n: Mail.request.ui(0n, s, now, client, id, p)
    case 41n: Mail.request.ui(1n, s, now, client, id, p)
    case 42n: Mail.request.ui(2n, s, now, client, id, p)
    case 43n: Mail.request.ui(3n, s, now, client, id, p)
    case 44n: Mail.request.ui(4n, s, now, client, id, p)
    case 45n: Mail.request.ui(5n, s, now, client, id, p)
    case 46n:
      Buy.look(s, client, id, J.Json.get_str(p, "thread", ""), "fab", "")
    case 47n:
      Buy.look(s, client, id, J.Json.get_str(p, "thread", ""), "intent", J.Json.get_str(p, "text", ""))
    case 48n:
      Buy.start(Hub.owns(s, J.Json.get_str(p, "thread", "")), s, now, client, id, J.Json.get_str(p, "thread", ""))
    # the model picker opened on opencode: its models read again (about a second)
    case 49n:
      Step.outs([Reply.ok(client, id), ModelsRefresh{}])
    case 50n:
      Step.reply(client, id, [M.ThreadParentSet{J.Json.get_str(p, "thread", ""), "", now}])
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

def Retire.request(i: Nat, +s: M.State) -> Step:
  Hub.request.k(i, s, 9n, 0, "r", J.Json.obj([J.KV{"thread", J.Str{"t1"}}]), "")

# A refusal before spawn may replay the exact unsent input. A process that
# already started gets a continuation with its completed actions as context.
def Auth.prompt(+s: M.State, +th: String, +o: J.Json) -> String:
  Bool.pick(String, Bool.or(J.Json.get_bool(o, "not_started", False{}), String.contains(FB.Error.reason(o), "was not started:")),
    M.Setting.get(s, "launch.input." ++ th, M.Setting.get(s, "fallback.input." ++ th, "")),
    HD.Hand.switch(Hub.entries(s), th, Hub.provider(Hub.thread(s, th)), Hub.provider(Hub.thread(s, th)), 0n)
      ++ "\n[Backplane] Credentials became unavailable. Continue the interrupted work from the recorded state. Do not repeat completed tool actions or restart finished work.\nOriginal request (context, not a new task):\n" ++ M.Setting.get(s, "fallback.input." ++ th, ""))

def Auth.line.k(locked: Bool, +s: M.State, +now: Nat, +th: String, +o: J.Json, normal: Step) -> Step:
  match locked:
    case True{}: Auth.hold(s, now, th, Auth.prompt(s, th, o), FB.Error.reason(o))
    case False{}: normal

def Auth.line(+s: M.State, +now: Nat, +th: String, +o: J.Json, normal: Step) -> Step:
  Auth.line.k(Bool.and(Hub.running(Hub.thread(s, th)), Bool.and(FB.Error.envelope(o), String.contains(String.to_lower(FB.Error.reason(o)), "login keyring is locked"))), s, now, th, o, normal)

# Quota failures never reach normal task completion/report-back. A late line
# from an interrupted/finished turn cannot reopen it.
def Hub.agent_line(+s: M.State, +now: Nat, +thread: String, +line: String) -> Step:
  +o = Maybe.default(&2, J.Json, J.Json.parse(line), J.Null{})
  Auth.line(s, now, thread, o, Fallback.line(s, now, thread, o, Bool.and(FB.Error.envelope(o), Bool.not(String.eq(J.Json.get_str(o, "type", ""), "error"))), Hub.agent_line.raw(s, now, thread, line)))

def Hub.agent_line.vo.active(+v: Nat, +s: M.State, +now: Nat, +thread: String, +line: String, +o: J.Json, +text: String) -> Step:
  Auth.line(s, now, thread, o, Fallback.line(s, now, thread, o, Bool.or(Bool.or(Nat.is_eq(v, 3n), Nat.is_eq(v, 4n)), Bool.and(Nat.is_eq(v, 0n), FB.Error.envelope(o))), Hub.agent_line.vo.raw(v, s, now, thread, line, o, text)))

def Fallback.blocks(xs: List<&2, J.Json>) -> List<&2, String>:
  match xs:
    case Nil{}: Nil{}
    case h <> t: J.Json.get_str(h, "text", "") <> Fallback.blocks(t)

def Fallback.content(c: J.Json) -> String:
  match c:
    case J.Str{s}: s
    case other: String.join(Fallback.blocks(J.Json.elems(other)), "\n")

def Fallback.claude.text(+o: Out) -> String:
  match o:
    case AgentSend{_, +line}: Fallback.content(J.Json.get_obj(J.Json.get_obj(Maybe.default(&2, J.Json, J.Json.parse(line), J.Null{}), "message"), "content"))
    case Git{_, _, "attach.claude", _, _, text, _}: text
    case _: ""

# Exhaustion is one action-needed incident for the configured chain, even
# when different threads first encounter different members of the outage.
def Fallback.chain.token(xs: List<&2, FB.Target>, +s: M.State) -> String:
  match xs:
    case Nil{}: ""
    case +t <> rest:
      M.Setting.get(s, FB.Limit.key(FB.Target.provider(t)), "") ++ M.Setting.get(s, FB.Limit.key(FB.Target.key(t)), "") ++ Fallback.chain.token(rest, s)

def Fallback.alert.blocked(new: Bool, +s: M.State, +now: Nat, +th: String, +token: String, text: String) -> List<&2, M.Change>:
  match new:
    case False{}: Nil{}
    case True{}: [M.SettingSet{"fallback.alerted.chain", token, now}, NO.Alert.set(s, now, th, "fallback-blocked", "fallback-chain-blocked", NO.Alert.memory(s, th), text)]

def Fallback.alert(+s: M.State, +now: Nat, +th: String, scope: String, text: String, blocked: Bool) -> List<&2, M.Change>:
  match blocked:
    case False{}: Fallback.alert.scope(s, now, th, scope, text, False{})
    case True{}:
      +token = M.Setting.get(s, "fallback.chain", "") ++ Fallback.chain.token(FB.Chain.of(s), s)
      Fallback.alert.blocked(Bool.not(String.eq(M.Setting.get(s, "fallback.alerted.chain", ""), token)), s, now, th, token, text)

def Fallback.block(+s: M.State, +now: Nat, +th: String, +text: String) -> Step:
  +from = Fallback.target(Hub.thread(s, th))
  +say = Fallback.say(s, now, th, from, "", True{})
  Step{List.append(&2, M.Change, ProviderResume.mark(s, now, th, FB.Limit{Fallback.scope(s, from, now), Fallback.reason(s, Fallback.scope(s, from, now)), J.Json.get_nat(FB.Limit.record(s, Fallback.scope(s, from, now)), "reset", 0n)}), List.append(&2, M.Change, [M.TurnChanged{th, M.Interrupted{}, now}, M.TurnQueued{th, "fallback:held:" ++ Nat.show(Hub.seq(s)), text, now},
    M.ActivityLogged{th, Hub.id("fallback", now, Hub.seq(s)), "error", "fallback-blocked", say, now}], Fallback.alert(s, now, th, Fallback.scope(s, from, now), say, True{}))), Nil{}}

# The legacy ledger verdict shares exactly the ordinary line path. This
# preserves cmds_legacy_same for every input, including provider errors.
def Hub.agent_line.vo(v: Nat, +s: M.State, +now: Nat, +thread: String, +line: String, o: J.Json, text: String) -> Step:
  match v:
    case 0n: Hub.agent_line(s, now, thread, line)
    case 1n+p: Hub.agent_line.vo.active(1n+p, s, now, thread, line, o, text)

def Hub.agent_line.v(v: Nat, +s: M.State, +now: Nat, +thread: String, +line: String, +text: String) -> Step:
  Hub.agent_line.vo(v, s, now, thread, line, Maybe.default(&2, J.Json, J.Json.parse(line), J.Null{}), text)

# Settings/model changes in the incoming step are explicit choices. Automatic
# switches are appended later by preflight, so they keep the original intent.
def Fallback.manual(cs: List<&2, M.Change>, +now: Nat) -> List<&2, M.Change>:
  match cs:
    case Nil{}: Nil{}
    case M.ProviderSet{+th, _, _, _} <> rest:
      M.SettingSet{"restart.retry." ++ th, "", now} <> M.SettingSet{"provider.resume." ++ th, "", now} <> M.SettingSet{"fallback.intent." ++ th, "", now} <> Fallback.manual(rest, now)
    case M.TurnChanged{+th, M.Running{}, _} <> rest:
      M.SettingSet{"restart.retry." ++ th, "", now} <> M.SettingSet{"provider.resume." ++ th, "", now} <> Fallback.manual(rest, now)
    case M.ModesSet{+th, _, _, _, _} <> rest:
      M.SettingSet{"fallback.intent." ++ th, "", now} <> Fallback.manual(rest, now)
    case _ <> rest: Fallback.manual(rest, now)

def Fallback.intent(+s: M.State, +th: String, +from: FB.Target) -> String:
  +saved = M.Setting.get(s, "fallback.intent." ++ th, "")
  Bool.pick(String, String.is_empty(saved), FB.Target.key(from), saved)

def Fallback.change(+s: M.State, +now: Nat, +th: String, +to: FB.Target, +text: String) -> List<&2, M.Change>:
  +from = Fallback.target(Hub.thread(s, th))
  +p = FB.Target.provider(to)
  +say = Fallback.say(s, now, th, from, FB.Target.key(to), False{})
  List.concat(&2, M.Change, [[M.SettingSet{"fallback.intent." ++ th, Fallback.intent(s, th, from), now},
    M.SettingSet{"fallback.session." ++ th ++ "." ++ FB.Target.key(from), M.Switch.sess(Hub.thread(s, th)), now},
    M.ProviderSet{th, p, FB.Target.model(to), now}, M.SessionSet{th, M.Setting.get(s, "fallback.session." ++ th ++ "." ++ FB.Target.key(to), ""), now}, M.SettingSet{"fallback.input." ++ th, Bool.pick(String, String.eq(M.Setting.get(s, "fallback.continuing." ++ th, ""), "yes"), M.Setting.get(s, "fallback.input." ++ th, text), text), now}, M.SettingSet{"fallback.continuing." ++ th, "", now},
    M.ActivityLogged{th, Hub.id("fallback", now, Hub.seq(s)), "info", "fallback", say, now}],
    Fallback.alert(s, now, th, Fallback.scope(s, from, now), say, False{})])

def Fallback.use(choice: Maybe<&2, FB.Target>, +s: M.State, +now: Nat, +th: String, +cwd: String, +text: String, +uuid: String) -> Step:
  match choice:
    case None{}: Fallback.block(s, now, th, text)
    case Some{+to}:
      +cs = Fallback.change(s, now, th, to, text)
      +ctx = HD.Hand.switch(Hub.entries(s), th, Hub.provider(Hub.thread(s, th)), FB.Target.provider(to), 0n)
      Step{List.append(&2, M.Change, cs, [M.SettingSet{"launch.input." ++ th, M.Queue.join(ctx, text), now}]), AgentStop{th} <> Hub.run(Hub.thread(M.State.replay(cs, s), th), cwd, M.Queue.join(ctx, text), uuid)}

def Fallback.attempt.k(held: Bool, +s: M.State, +caps: J.Json, +now: Nat, +th: String, +cwd: String, +text: String, +uuid: String, original: List<&2, Out>) -> Step:
  match held:
    case False{}: Step{[M.SettingSet{"fallback.input." ++ th, text, now}, M.SettingSet{"launch.input." ++ th, text, now}], original}
    case True{}: Fallback.use(FB.Chain.pick(FB.Chain.suffix(FB.Chain.of(s), FB.Target.key(Fallback.target(Hub.thread(s, th)))), s, caps, now), s, now, th, cwd, text, uuid)

def Fallback.attempt.quota(+s: M.State, caps: J.Json, +now: Nat, +th: String, cwd: String, text: String, uuid: String, original: List<&2, Out>) -> Step:
  Fallback.attempt.k(Bool.and(Bool.not(ACCT.Accounts.named(s, th)), Bool.and(FB.Chain.on(s), FB.Target.held(s, Fallback.target(Hub.thread(s, th)), now))), s, caps, now, th, cwd, text, uuid, original)

def Auth.attempt(wait: Bool, +s: M.State, caps: J.Json, +now: Nat, +th: String, cwd: String, text: String, uuid: String, original: List<&2, Out>) -> Step:
  match wait:
    case True{}: Step{[M.TurnChanged{th, M.Interrupted{}, now}, M.TurnQueued{th, "auth:held:" ++ Nat.show(Hub.seq(s)), text, now}], Nil{}}
    case False{}: Fallback.attempt.quota(s, caps, now, th, cwd, text, uuid, original)

def Fallback.attempt(+s: M.State, caps: J.Json, +now: Nat, +th: String, cwd: String, text: String, uuid: String, original: List<&2, Out>) -> Step:
  Auth.attempt(Auth.waiting(s, th), s, caps, now, th, cwd, text, uuid, original)

def Fallback.outs.one(+o: Out, +s: M.State, +caps: J.Json, +now: Nat) -> Step:
  match o:
    case CodexTurn{+th, +cwd, _, _, _, +text}:
      Fallback.attempt(s, caps, now, th, cwd, text, Hub.uuid(now, th), [o])
    case GrokTurn{+th, +cwd, _, _, _, +text}:
      Fallback.attempt(s, caps, now, th, cwd, text, Hub.uuid(now, th), [o])
    case MuseTurn{+th, +cwd, _, _, _, +text}:
      Fallback.attempt(s, caps, now, th, cwd, text, Hub.uuid(now, th), [o])
    case AgyTurn{+th, +cwd, _, _, _, +text}:
      Fallback.attempt(s, caps, now, th, cwd, text, Hub.uuid(now, th), [o])
    case OcTurn{+th, +cwd, _, _, _, +text}:
      Fallback.attempt(s, caps, now, th, cwd, text, Hub.uuid(now, th), [o])
    case PiTurn{+th, +cwd, _, _, _, +text}:
      Fallback.attempt(s, caps, now, th, cwd, text, Hub.uuid(now, th), [o])
    case OmpTurn{+th, +cwd, _, _, _, +text}:
      Fallback.attempt(s, caps, now, th, cwd, text, Hub.uuid(now, th), [o])
    case _: Step.outs([o])

def Fallback.outs(os: List<&2, Out>, +s: M.State, +caps: J.Json, +now: Nat) -> Step:
  match os:
    case Nil{}: Step.none()
    case AgentStart{+th, +cwd, p, m, se, r, i} <> +send <> tail:
      +first = Fallback.attempt(s, caps, now, th, cwd, Fallback.claude.text(send), Fallback.claude.uuid(send), [AgentStart{th, cwd, p, m, se, r, i}, send])
      Step.cat(first, Fallback.outs(tail, M.State.replay(Queue.changes(first), s), caps, now))
    case +o <> rest:
      +first = Fallback.outs.one(o, s, caps, now)
      Step.cat(first, Fallback.outs(rest, M.State.replay(Queue.changes(first), s), caps, now))

def Fallback.after(+s: M.State, caps: J.Json, +now: Nat, st: Step) -> Step:
  match st:
    case Step{+cs, os}:
      +all = List.append(&2, M.Change, cs, Fallback.manual(cs, now))
      Step.before(all, Nil{}, Fallback.outs(os, M.State.replay(all, s), caps, now))

def Fallback.validated(ok: Bool, +s: M.State, caps: J.Json, now: Nat, step: Step) -> Step:
  match ok:
    case True{}: Fallback.after(s, caps, now, step)
    case False{}: Step{Nil{}, Fallback.settings.errors(Step.outs_of(step))}

def Fallback.validate(+s: M.State, +caps: J.Json, now: Nat, +step: Step) -> Step:
  Fallback.validated(Fallback.settings.ok(Queue.changes(step), s, caps), s, caps, now, step)

# Operational reports, for the laws (oprep_*; test/oprep_test.bend): a task
# of t1 for t0, t0 the bot "bu"'s thread, made the uberbot when `on`
def Operational.demo(+on: Bool) -> M.State:
  +s = {M.State.apply(Task.demo(False{}), M.BotSet{"bu", "Bub", "t0", 0n, "", 1n}) : M.State}
  Bool.pick(M.State, on, M.State.apply(s, M.SettingSet{"uber.bot", "bu", 1n}), s)

def Operational.demo.finish(+on: Bool) -> Step:
  Task.finish(Operational.demo(on), 9n, "t1", Step{[M.TurnChanged{"t1", M.Completed{}, 9n}], Nil{}})

# messages posted in th, and TaskDelivered among the changes
def Operational.posts(+cs: List<&2, M.Change>, +th: String) -> Nat:
  match cs:
    case Nil{}: 0n
    case M.MessagePosted{+t, _, _, _, _} <> +rest: Nat.add(Bool.pick(Nat, String.eq(t, th), 1n, 0n), Operational.posts(rest, th))
    case _ <> +rest: Operational.posts(rest, th)

def Operational.delivered(+cs: List<&2, M.Change>) -> Nat:
  match cs:
    case Nil{}: 0n
    case M.TaskDelivered{_, _} <> +t: 1n+Operational.delivered(t)
    case _ <> +t: Operational.delivered(t)

# the changes a filing commits after its body is written
def Operational.pending(+os: List<&2, Out>) -> List<&2, M.Change>:
  match os:
    case Nil{}: Nil{}
    case MailAppend{_, _, +cs} <> +t: List.append(&2, M.Change, cs, Operational.pending(t))
    case _ <> +t: Operational.pending(t)

# Secrets (core/secret.bend, docs/secrets.md)
# -------------------------------------------
# A step's changes with every stored value masked before they are written
# and folded in: each change's JSON through SEC.Sec.json (law
# sec_json_total), decoded again only when it held a value. With nothing
# stored the step's changes go as they are, unencoded (law
# sec_changes_empty).
def Secret.decoded(m: Maybe<&2, M.Change>) -> List<&2, M.Change>:
  match m:
    case Some{c}:
      [c]
    case None{}:
      Nil{}

def Secret.change.k(dirty: Bool, +v: SEC.Vault, +j: J.Json, c: M.Change) -> List<&2, M.Change>:
  match dirty:
    case True{}:
      Secret.decoded(C.Change.decode(SEC.Sec.json(v, j)))
    case False{}:
      [c]

def Secret.change.j(+j: J.Json, +v: SEC.Vault, c: M.Change) -> List<&2, M.Change>:
  Secret.change.k(SEC.Sec.json.dirty(v, j), v, j, c)

def Secret.change(+v: SEC.Vault, +c: M.Change) -> List<&2, M.Change>:
  Secret.change.j(C.Change.encode(c), v, c)

def Secret.changes.go(cs: List<&2, M.Change>, +v: SEC.Vault, acc: List<&2, M.Change>) -> List<&2, M.Change>:
  match cs:
    case Nil{}:
      List.reverse(&2, M.Change, acc)
    case +c <> t:
      Secret.changes.go(t, v, List.append(&2, M.Change, List.reverse(&2, M.Change, Secret.change(v, c)), acc))

def Secret.changes.k(empty: Bool, +v: SEC.Vault, cs: List<&2, M.Change>) -> List<&2, M.Change>:
  match empty:
    case True{}:
      cs
    case False{}:
      Secret.changes.go(cs, v, Nil{})

def Secret.changes(+v: SEC.Vault, cs: List<&2, M.Change>) -> List<&2, M.Change>:
  Secret.changes.k(SEC.Sec.empty(v), v, cs)

# A step's outs with the same values masked: a turn's end carries text
# read from the thread (its error and reply) to the server, which sends it
# on (the webhook), so it is covered here as the stored change it was
# read from is. No other out carries stored text out of the hub.
def Secret.outs.go(os: List<&2, Out>, +v: SEC.Vault, acc: List<&2, Out>) -> List<&2, Out>:
  match os:
    case Nil{}:
      List.reverse(&2, Out, acc)
    case TurnEnded{+th, +state, +alert, +err, +reason, +text} <> t:
      Secret.outs.go(t, v, TurnEnded{th, state, alert, SEC.Sec.mask(v, err), reason, SEC.Sec.mask(v, text)} <> acc)
    case +o <> t:
      Secret.outs.go(t, v, o <> acc)

def Secret.outs.k(empty: Bool, +v: SEC.Vault, os: List<&2, Out>) -> List<&2, Out>:
  match empty:
    case True{}:
      os
    case False{}:
      Secret.outs.go(os, v, Nil{})

def Secret.outs(+v: SEC.Vault, os: List<&2, Out>) -> List<&2, Out>:
  Secret.outs.k(SEC.Sec.empty(v), v, os)

# the thread that asked for a secret (secret_request), or ""
def Secret.ask.thread(+s: M.State, +name: String) -> String:
  J.Json.get_str(Maybe.default(&2, J.Json, J.Json.parse(M.Setting.get(s, SEC.Sec.ask.key(name), "")), J.Null{}), "thread", "")

# an agent asks for a secret: the request the clients show (its name and
# purpose, the thread; never a value)
def Secret.asked(+name: String, +purpose: String, +thread: String, +by: String, +now: Nat) -> Step:
  Step{[M.SettingSet{SEC.Sec.ask.key(name), SEC.Sec.ask.body(purpose, thread, by, now), now}], Nil{}}

def Secret.clear.k(none: Bool, +name: String, +now: Nat) -> List<&2, M.Change>:
  match none:
    case True{}:
      Nil{}
    case False{}:
      [M.SettingSet{SEC.Sec.ask.key(name), "", now}]

# the person dismissed a request
def Secret.dismiss(+s: M.State, +name: String, +now: Nat) -> List<&2, M.Change>:
  Secret.clear.k(String.is_empty(M.Setting.get(s, SEC.Sec.ask.key(name), "")), name, now)

# what the thread is told: the handle, never the value
def Secret.note(+name: String) -> String:
  J.Json.show(SEC.Sec.handle(name, True{})) ++ " The person saved this secret in Backplane's vault. Use it by name with secret_run; you will never see its value."

def Secret.tell.k(none: Bool, +s: M.State, +now: Nat, +to: String, +name: String) -> Step:
  match none:
    case True{}:
      Step.none()
    case False{}:
      Hub.inbox.id(s, now, "secret", to, M.User{}, Secret.note(name), "t:" ++ to ++ ":sec-" ++ name ++ "-" ++ Nat.show(now))

def Secret.tell(+s: M.State, +now: Nat, +to: String, +name: String) -> Step:
  Secret.tell.k(Bool.or(String.is_empty(to), Bool.not(Lr.exists(s, to))), s, now, to, name)

# a secret saved: its request closed, and the thread that asked for it and
# the one it was saved from (/secret in a chat) told its handle
def Secret.saved.k(+asker: String, +s: M.State, +now: Nat, +name: String, +from: String) -> Step:
  Step.join(Step{Secret.dismiss(s, name, now), Nil{}},
    Step.join(Secret.tell(s, now, asker, name), Secret.tell(s, now, Bool.pick(String, String.eq(from, asker), "", from), name)))

def Secret.saved(+s: M.State, +now: Nat, +name: String, +from: String) -> Step:
  Secret.saved.k(Secret.ask.thread(s, name), s, now, name, from)

# Report background task deadlines once while retaining ownership and any lock.
def WorkTimeout.row(go: Bool, +s: M.State, +now: Nat, +th: String, +l: String) -> Step:
  match go:
    case False{}: Step.none()
    case True{}:
      Step.before([M.SettingSet{WorkTimeout.key(th, M.Bg.field(l, 0n)) ++ ".noted", "yes", now}], Nil{},
        Watch.tell(s, now, th, "[Backplane] Background work \"" ++ M.Bg.title(l) ++ "\" timed out. It may still be running; no process was interrupted. Report status or continue; check the task before repeating it."))

def WorkTimeout.rows(ls: List<&2, String>, +s: M.State, +now: Nat, +th: String) -> Step:
  match ls:
    case Nil{}: Step.none()
    case +l <> rest:
      +ensure = WorkTimeout.start(s, now, th, M.Bg.field(l, 0n), J.Null{})
      +before = M.State.replay(ensure, s)
      +deadline = M.Switch.num(M.Setting.get(before, WorkTimeout.key(th, M.Bg.field(l, 0n)), "0"), 0n)
      +st = Step.before(ensure, Nil{}, WorkTimeout.row(Bool.and(Bool.not(M.Bg.ended(l)), Bool.and(Nat.is_gt(deadline, 0n), Bool.and(Nat.is_ge(now, deadline), String.is_empty(M.Setting.get(before, WorkTimeout.key(th, M.Bg.field(l, 0n)) ++ ".noted", ""))))), before, now, th, l))
      Step.cat(st, WorkTimeout.rows(rest, M.State.replay(Queue.changes(st), s), now, th))

def WorkTimeout.threads(ts: List<&2, M.Thread>, +s: M.State, +now: Nat) -> Step:
  match ts:
    case Nil{}: Step.none()
    case +t <> rest:
      +st = WorkTimeout.rows(Bool.pick(List<&2, String>, Lost.live(t), M.Bg.of(s, M.Thread.id(t)), Nil{}), s, now, M.Thread.id(t))
      Step.cat(st, WorkTimeout.threads(rest, M.State.replay(Queue.changes(st), s), now))

def WorkTimeout.tick(+s: M.State, now: Nat) -> Step:
  WorkTimeout.threads(Rec.threads(s), s, now)

def ProviderResume.record(s: M.State, +th: String) -> J.Json:
  Maybe.default(&2, J.Json, J.Json.parse(M.Setting.get(s, "provider.resume." ++ th, "")), J.Null{})

def ProviderResume.interrupted(t: M.TurnState) -> Bool:
  match t:
    case M.Interrupted{}: True{}
    case _: False{}

def ProviderResume.ready(+s: M.State, +now: Nat, +th: String) -> Bool:
  +r = ProviderResume.record(s, th)
  +reset = J.Json.get_nat(r, "reset", 0n)
  Bool.and(Bool.and(Lost.live(Hub.thread(s, th)), ProviderResume.interrupted(M.Thread.turn(Hub.thread(s, th)))),
    Bool.and(Nat.is_gt(reset, 0n), Bool.and(Nat.is_ge(now, reset), Bool.and(Nat.is_eq(Hub.pending(Hub.thread(s, th)), 0n),
      String.eq(J.Json.get_str(r, "target", ""), FB.Target.key(Fallback.target(Hub.thread(s, th))))))))

def ProviderResume.one(go: Bool, +s: M.State, +now: Nat, +th: String) -> Step:
  match go:
    case False{}: Step.none()
    case True{}:
      +cs = {[M.SettingSet{"provider.resume." ++ th, "", now}, M.ActivityLogged{th, Hub.id("quota", now, Hub.seq(s)), "info", "provider-resume", "Provider limit reset; continuing from the last step", now}] : List<&2, M.Change>}
      Step.before(cs, Nil{}, Hub.post(M.State.replay(cs, s), now, th, Hub.id("resume", now, Hub.seq(s)), M.User{},
        "[Backplane] The provider's reported limit reset has arrived. Continue from your last step. Check completed tool results and any work still running before repeating an action."))

def ProviderResume.threads(ts: List<&2, M.Thread>, +s: M.State, +now: Nat) -> Step:
  match ts:
    case Nil{}: Step.none()
    case +t <> rest:
      +st = ProviderResume.one(ProviderResume.ready(s, now, M.Thread.id(t)), s, now, M.Thread.id(t))
      Step.cat(st, ProviderResume.threads(rest, M.State.replay(Queue.changes(st), s), now))

def ProviderResume.tick(+s: M.State, now: Nat) -> Step:
  ProviderResume.threads(Rec.threads(s), s, now)
