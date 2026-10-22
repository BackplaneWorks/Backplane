import Base
import ./locks.bend as LK
import ./json.bend as J
import ./model.bend as M

# Changes as JSON: the event log on disk and the change stream to clients
# share this codec, so the browser folds exactly what the server stored.
# An encoded Change is {"$": "<Constructor>", ...fields}. Decoding is total:
# unknown tags answer None (a newer server's log is skipped, not misread).

def Codec.nat(n: Nat) -> J.Json:
  J.Num{Nat.show(n)}

def Codec.str(s: String) -> J.Json:
  J.Str{s}

def Codec.bool(b: Bool) -> J.Json:
  J.Flag{b}

def Codec.index.hit(names: List<&2, String>, +s: String) -> Bool:
  match names:
    case n <> _:
      String.eq(n, s)
    case Nil{}:
      False{}

def Codec.index.go(names: List<&2, String>, hit: Bool, +i: Nat, +s: String) -> Nat:
  match names hit:
    case _ <> _ True{}:
      i
    case _ <> +t False{}:
      Codec.index.go(t, Codec.index.hit(t, s), 1n+i, s)
    case Nil{} _:
      i

def Codec.pos(+names: List<&2, String>, +s: String) -> Nat:
  Codec.index.go(names, Codec.index.hit(names, s), 0n, s)

# Enums
# -----

def Role.name(r: M.Role) -> String:
  match r:
    case M.User{}:
      "user"
    case M.Assistant{}:
      "assistant"
    case M.System{}:
      "system"

def Role.of.k(i: Nat) -> M.Role:
  match i:
    case 0n:
      M.User{}
    case 1n:
      M.Assistant{}
    case _:
      M.System{}

def Role.of(+s: String) -> M.Role:
  Role.of.k(Codec.pos(["user", "assistant", "system"], s))

def Turn.name(t: M.TurnState) -> String:
  match t:
    case M.Idle{}:
      "idle"
    case M.Running{}:
      "running"
    case M.Completed{}:
      "completed"
    case M.Interrupted{}:
      "interrupted"
    case M.Failed{}:
      "failed"

def Turn.of.k(i: Nat) -> M.TurnState:
  match i:
    case 1n:
      M.Running{}
    case 2n:
      M.Completed{}
    case 3n:
      M.Interrupted{}
    case 4n:
      M.Failed{}
    case _:
      M.Idle{}

def Turn.of(+s: String) -> M.TurnState:
  Turn.of.k(Codec.pos(["idle", "running", "completed", "interrupted", "failed"], s))

def Env.name(e: M.EnvMode) -> String:
  match e:
    case M.Local{}:
      "local"
    case M.Worktree{}:
      "worktree"

def Env.of(+s: String) -> M.EnvMode:
  Bool.pick(M.EnvMode, String.eq(s, "worktree"), M.Worktree{}, M.Local{})

def Runtime.name(r: M.RuntimeMode) -> String:
  match r:
    case M.Supervised{}:
      "approval-required"
    case M.AcceptEdits{}:
      "auto-accept-edits"
    case M.AutoMode{}:
      "auto"
    case M.FullAccess{}:
      "full-access"

def Runtime.of.k(i: Nat) -> M.RuntimeMode:
  match i:
    case 0n:
      M.Supervised{}
    case 1n:
      M.AcceptEdits{}
    case 2n:
      M.AutoMode{}
    case _:
      M.FullAccess{}

def Runtime.of(+s: String) -> M.RuntimeMode:
  Runtime.of.k(Codec.pos(["approval-required", "auto-accept-edits", "auto", "full-access"], s))

def Interaction.name(i: M.InteractionMode) -> String:
  match i:
    case M.DefaultMode{}:
      "default"
    case M.PlanMode{}:
      "plan"

def Interaction.of(+s: String) -> M.InteractionMode:
  Bool.pick(M.InteractionMode, String.eq(s, "plan"), M.PlanMode{}, M.DefaultMode{})

def TaskSt.name(t: M.TaskState) -> String:
  match t:
    case M.TaskQueued{}:
      "queued"
    case M.TaskRunning{}:
      "running"
    case M.TaskDone{}:
      "done"
    case M.TaskFailed{}:
      "failed"
    case M.TaskCancelled{}:
      "cancelled"

def TaskSt.of.k(i: Nat) -> M.TaskState:
  match i:
    case 0n:
      M.TaskQueued{}
    case 1n:
      M.TaskRunning{}
    case 2n:
      M.TaskDone{}
    case 3n:
      M.TaskFailed{}
    case _:
      M.TaskCancelled{}

def TaskSt.of(+s: String) -> M.TaskState:
  TaskSt.of.k(Codec.pos(["queued", "running", "done", "failed", "cancelled"], s))

# Changes
# -------

def Change.obj(tag: String, kvs: List<&2, J.KV>) -> J.Json:
  J.Json.obj(J.KV{"$", J.Str{tag}} <> kvs)

def Change.encode(c: M.Change) -> J.Json:
  match c:
    case M.ProjectCreated{id, title, root, at}:
      Change.obj("ProjectCreated", [J.KV{"id", J.Str{id}}, J.KV{"title", J.Str{title}}, J.KV{"root", J.Str{root}}, J.KV{"at", Codec.nat(at)}])
    case M.ProjectRemoved{id, at}:
      Change.obj("ProjectRemoved", [J.KV{"id", J.Str{id}}, J.KV{"at", Codec.nat(at)}])
    case M.ThreadCreated{id, project, title, env, branch, worktree, provider, model, at}:
      Change.obj("ThreadCreated", [J.KV{"id", J.Str{id}}, J.KV{"project", J.Str{project}}, J.KV{"title", J.Str{title}}, J.KV{"env", J.Str{Env.name(env)}}, J.KV{"branch", J.Str{branch}}, J.KV{"worktree", J.Str{worktree}}, J.KV{"provider", J.Str{provider}}, J.KV{"model", J.Str{model}}, J.KV{"at", Codec.nat(at)}])
    case M.ThreadRenamed{id, title, at}:
      Change.obj("ThreadRenamed", [J.KV{"id", J.Str{id}}, J.KV{"title", J.Str{title}}, J.KV{"at", Codec.nat(at)}])
    case M.MessagePosted{thread, msg, role, text, at}:
      Change.obj("MessagePosted", [J.KV{"thread", J.Str{thread}}, J.KV{"msg", J.Str{msg}}, J.KV{"role", J.Str{Role.name(role)}}, J.KV{"text", J.Str{text}}, J.KV{"at", Codec.nat(at)}])
    case M.TurnChanged{thread, state, at}:
      Change.obj("TurnChanged", [J.KV{"thread", J.Str{thread}}, J.KV{"state", J.Str{Turn.name(state)}}, J.KV{"at", Codec.nat(at)}])
    case M.PendingChanged{thread, count, at}:
      Change.obj("PendingChanged", [J.KV{"thread", J.Str{thread}}, J.KV{"count", Codec.nat(count)}, J.KV{"at", Codec.nat(at)}])
    case M.ActivityLogged{thread, act, tone, kind, summary, at}:
      Change.obj("ActivityLogged", [J.KV{"thread", J.Str{thread}}, J.KV{"act", J.Str{act}}, J.KV{"tone", J.Str{tone}}, J.KV{"kind", J.Str{kind}}, J.KV{"summary", J.Str{summary}}, J.KV{"at", Codec.nat(at)}])
    case M.ThreadSettled{id, manual, at}:
      Change.obj("ThreadSettled", [J.KV{"id", J.Str{id}}, J.KV{"manual", J.Flag{manual}}, J.KV{"at", Codec.nat(at)}])
    case M.ThreadUnsettled{id, at}:
      Change.obj("ThreadUnsettled", [J.KV{"id", J.Str{id}}, J.KV{"at", Codec.nat(at)}])
    case M.ThreadArchived{id, archived, at}:
      Change.obj("ThreadArchived", [J.KV{"id", J.Str{id}}, J.KV{"archived", J.Flag{archived}}, J.KV{"at", Codec.nat(at)}])
    case M.ThreadPinned{id, pinned, at}:
      Change.obj("ThreadPinned", [J.KV{"id", J.Str{id}}, J.KV{"pinned", J.Flag{pinned}}, J.KV{"at", Codec.nat(at)}])
    case M.ThreadDeleted{id, at}:
      Change.obj("ThreadDeleted", [J.KV{"id", J.Str{id}}, J.KV{"at", Codec.nat(at)}])
    case M.WorktreeSet{id, branch, worktree, at}:
      Change.obj("WorktreeSet", [J.KV{"id", J.Str{id}}, J.KV{"branch", J.Str{branch}}, J.KV{"worktree", J.Str{worktree}}, J.KV{"at", Codec.nat(at)}])
    case M.SessionSet{id, session, at}:
      Change.obj("SessionSet", [J.KV{"id", J.Str{id}}, J.KV{"session", J.Str{session}}, J.KV{"at", Codec.nat(at)}])
    case M.ModesSet{id, runtime, interaction, model, at}:
      Change.obj("ModesSet", [J.KV{"id", J.Str{id}}, J.KV{"runtime", J.Str{Runtime.name(runtime)}}, J.KV{"interaction", J.Str{Interaction.name(interaction)}}, J.KV{"model", J.Str{model}}, J.KV{"at", Codec.nat(at)}])
    case M.TaskDelegated{task, parent, child, depth, provider, model, at}:
      Change.obj("TaskDelegated", [J.KV{"task", J.Str{task}}, J.KV{"parent", J.Str{parent}}, J.KV{"child", J.Str{child}}, J.KV{"depth", Codec.nat(depth)}, J.KV{"provider", J.Str{provider}}, J.KV{"model", J.Str{model}}, J.KV{"at", Codec.nat(at)}])
    case M.TaskStateSet{task, state, summary, at}:
      Change.obj("TaskStateSet", [J.KV{"task", J.Str{task}}, J.KV{"state", J.Str{TaskSt.name(state)}}, J.KV{"summary", J.Str{summary}}, J.KV{"at", Codec.nat(at)}])
    case M.TaskDelivered{task, at}:
      Change.obj("TaskDelivered", [J.KV{"task", J.Str{task}}, J.KV{"at", Codec.nat(at)}])
    case M.SettingSet{key, value, at}:
      Change.obj("SettingSet", [J.KV{"key", J.Str{key}}, J.KV{"value", J.Str{value}}, J.KV{"at", Codec.nat(at)}])
    case M.TurnQueued{thread, msg, text, at}:
      Change.obj("TurnQueued", [J.KV{"thread", J.Str{thread}}, J.KV{"msg", J.Str{msg}}, J.KV{"text", J.Str{text}}, J.KV{"at", Codec.nat(at)}])
    case M.QueueTaken{thread, at}:
      Change.obj("QueueTaken", [J.KV{"thread", J.Str{thread}}, J.KV{"at", Codec.nat(at)}])
    case M.AskOpened{id, thread, kind, tool, detail, options, at}:
      Change.obj("AskOpened", [J.KV{"id", J.Str{id}}, J.KV{"thread", J.Str{thread}}, J.KV{"kind", J.Str{kind}}, J.KV{"tool", J.Str{tool}}, J.KV{"detail", J.Str{detail}}, J.KV{"options", J.Str{options}}, J.KV{"at", Codec.nat(at)}])
    case M.AskClosed{id, answer, at}:
      Change.obj("AskClosed", [J.KV{"id", J.Str{id}}, J.KV{"answer", J.Str{answer}}, J.KV{"at", Codec.nat(at)}])
    case M.DeviceRegistered{token, platform, kind, thread, env, bundle, at}:
      Change.obj("DeviceRegistered", [J.KV{"token", J.Str{token}}, J.KV{"platform", J.Str{platform}}, J.KV{"kind", J.Str{kind}}, J.KV{"thread", J.Str{thread}}, J.KV{"env", J.Str{env}}, J.KV{"bundle", J.Str{bundle}}, J.KV{"at", Codec.nat(at)}])
    case M.DeviceDropped{token, at}:
      Change.obj("DeviceDropped", [J.KV{"token", J.Str{token}}, J.KV{"at", Codec.nat(at)}])
    case M.ThreadSnoozed{id, until, at}:
      Change.obj("ThreadSnoozed", [J.KV{"id", J.Str{id}}, J.KV{"until", Codec.nat(until)}, J.KV{"at", Codec.nat(at)}])
    case M.ThreadOrdered{id, section, key, at}:
      Change.obj("ThreadOrdered", [J.KV{"id", J.Str{id}}, J.KV{"section", J.Str{section}}, J.KV{"key", J.Str{key}}, J.KV{"at", Codec.nat(at)}])
    case M.WorktreeSkipped{id, at}:
      Change.obj("WorktreeSkipped", [J.KV{"id", J.Str{id}}, J.KV{"at", Codec.nat(at)}])
    case M.ThreadMerged{id, url, at}:
      Change.obj("ThreadMerged", [J.KV{"id", J.Str{id}}, J.KV{"url", J.Str{url}}, J.KV{"at", Codec.nat(at)}])
    case M.WorktreeRemoved{id, at}:
      Change.obj("WorktreeRemoved", [J.KV{"id", J.Str{id}}, J.KV{"at", Codec.nat(at)}])
    case M.ProjectOrdered{id, key, at}:
      Change.obj("ProjectOrdered", [J.KV{"id", J.Str{id}}, J.KV{"key", J.Str{key}}, J.KV{"at", Codec.nat(at)}])
    case M.QueueEdited{thread, msg, text, at}:
      Change.obj("QueueEdited", [J.KV{"thread", J.Str{thread}}, J.KV{"msg", J.Str{msg}}, J.KV{"text", J.Str{text}}, J.KV{"at", Codec.nat(at)}])
    case M.QueueRemoved{thread, msg, at}:
      Change.obj("QueueRemoved", [J.KV{"thread", J.Str{thread}}, J.KV{"msg", J.Str{msg}}, J.KV{"at", Codec.nat(at)}])
    case M.QueueMoved{thread, msg, to, at}:
      Change.obj("QueueMoved", [J.KV{"thread", J.Str{thread}}, J.KV{"msg", J.Str{msg}}, J.KV{"to", Codec.nat(to)}, J.KV{"at", Codec.nat(at)}])
    case M.QueueTakenOne{thread, msg, at}:
      Change.obj("QueueTakenOne", [J.KV{"thread", J.Str{thread}}, J.KV{"msg", J.Str{msg}}, J.KV{"at", Codec.nat(at)}])
    case M.ThreadForked{id, source, at}:
      Change.obj("ThreadForked", [J.KV{"id", J.Str{id}}, J.KV{"source", J.Str{source}}, J.KV{"at", Codec.nat(at)}])
    case M.ProviderSet{id, provider, model, at}:
      Change.obj("ProviderSet", [J.KV{"id", J.Str{id}}, J.KV{"provider", J.Str{provider}}, J.KV{"model", J.Str{model}}, J.KV{"at", Codec.nat(at)}])
    case M.TodosSet{thread, todos, at}:
      Change.obj("TodosSet", [J.KV{"thread", J.Str{thread}}, J.KV{"todos", J.Str{todos}}, J.KV{"at", Codec.nat(at)}])
    case M.UsageSet{thread, used, window, at}:
      Change.obj("UsageSet", [J.KV{"thread", J.Str{thread}}, J.KV{"used", Codec.nat(used)}, J.KV{"window", Codec.nat(window)}, J.KV{"at", Codec.nat(at)}])
    case M.BotSet{id, name, thread, look, persona, at}:
      Change.obj("BotSet", [J.KV{"id", J.Str{id}}, J.KV{"name", J.Str{name}}, J.KV{"thread", J.Str{thread}}, J.KV{"look", Codec.nat(look)}, J.KV{"persona", J.Str{persona}}, J.KV{"at", Codec.nat(at)}])
    case M.BotDeleted{id, at}:
      Change.obj("BotDeleted", [J.KV{"id", J.Str{id}}, J.KV{"at", Codec.nat(at)}])
    case M.RoomSet{id, name, members, at}:
      Change.obj("RoomSet", [J.KV{"id", J.Str{id}}, J.KV{"name", J.Str{name}}, J.KV{"members", J.Str{members}}, J.KV{"at", Codec.nat(at)}])
    case M.RoomDeleted{id, at}:
      Change.obj("RoomDeleted", [J.KV{"id", J.Str{id}}, J.KV{"at", Codec.nat(at)}])
    case M.RoomPosted{id, room, from, text, hop, at}:
      Change.obj("RoomPosted", [J.KV{"id", J.Str{id}}, J.KV{"room", J.Str{room}}, J.KV{"from", J.Str{from}}, J.KV{"text", J.Str{text}}, J.KV{"hop", Codec.nat(hop)}, J.KV{"at", Codec.nat(at)}])
    case M.HookSet{id, bot, name, revoked, at}:
      Change.obj("HookSet", [J.KV{"id", J.Str{id}}, J.KV{"bot", J.Str{bot}}, J.KV{"name", J.Str{name}}, J.KV{"revoked", J.Flag{revoked}}, J.KV{"at", Codec.nat(at)}])
    case M.HookHit{id, at}:
      Change.obj("HookHit", [J.KV{"id", J.Str{id}}, J.KV{"at", Codec.nat(at)}])
    case M.RoutineSet{id, bot, name, cron, prompt, on, at}:
      Change.obj("RoutineSet", [J.KV{"id", J.Str{id}}, J.KV{"bot", J.Str{bot}}, J.KV{"name", J.Str{name}}, J.KV{"cron", J.Str{cron}}, J.KV{"prompt", J.Str{prompt}}, J.KV{"on", J.Flag{on}}, J.KV{"at", Codec.nat(at)}])
    case M.RoutineDeleted{id, at}:
      Change.obj("RoutineDeleted", [J.KV{"id", J.Str{id}}, J.KV{"at", Codec.nat(at)}])
    case M.RoutineFired{id, at}:
      Change.obj("RoutineFired", [J.KV{"id", J.Str{id}}, J.KV{"at", Codec.nat(at)}])
    case M.MemorySaved{bot, key, kind, text, tags, at}:
      Change.obj("MemorySaved", [J.KV{"bot", J.Str{bot}}, J.KV{"key", J.Str{key}}, J.KV{"kind", J.Str{kind}}, J.KV{"text", J.Str{text}}, J.KV{"tags", J.Str{tags}}, J.KV{"at", Codec.nat(at)}])
    case M.MemoryForgot{bot, key, at}:
      Change.obj("MemoryForgot", [J.KV{"bot", J.Str{bot}}, J.KV{"key", J.Str{key}}, J.KV{"at", Codec.nat(at)}])
    case M.SpaceSet{bot, spec, at}:
      Change.obj("SpaceSet", [J.KV{"bot", J.Str{bot}}, J.KV{"spec", J.Str{spec}}, J.KV{"at", Codec.nat(at)}])
    case M.PeerSet{id, name, url, revoked, at}:
      Change.obj("PeerSet", [J.KV{"id", J.Str{id}}, J.KV{"name", J.Str{name}}, J.KV{"url", J.Str{url}}, J.KV{"revoked", J.Flag{revoked}}, J.KV{"at", Codec.nat(at)}])
    case M.BotWoke{bot, hop, at}:
      Change.obj("BotWoke", [J.KV{"bot", J.Str{bot}}, J.KV{"hop", Codec.nat(hop)}, J.KV{"at", Codec.nat(at)}])
    case M.TaskSet{bot, slug, json, at}:
      Change.obj("TaskSet", [J.KV{"bot", J.Str{bot}}, J.KV{"slug", J.Str{slug}}, J.KV{"json", J.Str{json}}, J.KV{"at", Codec.nat(at)}])
    case M.TaskDeleted{bot, slug, at}:
      Change.obj("TaskDeleted", [J.KV{"bot", J.Str{bot}}, J.KV{"slug", J.Str{slug}}, J.KV{"at", Codec.nat(at)}])
    case M.SpacePageSet{bot, html, at}:
      Change.obj("SpacePageSet", [J.KV{"bot", J.Str{bot}}, J.KV{"html", J.Str{html}}, J.KV{"at", Codec.nat(at)}])
    case M.WorkSet{thread, kind, rows, at}:
      Change.obj("WorkSet", [J.KV{"thread", J.Str{thread}}, J.KV{"kind", J.Str{kind}}, J.KV{"rows", J.Str{rows}}, J.KV{"at", Codec.nat(at)}])
    case M.UberSet{bot, uid, at}:
      Change.obj("UberSet", [J.KV{"bot", J.Str{bot}}, J.KV{"uid", J.Str{uid}}, J.KV{"at", Codec.nat(at)}])
    case M.UberOp{uid, origin, n, stamp, op, at}:
      Change.obj("UberOp", [J.KV{"uid", J.Str{uid}}, J.KV{"origin", J.Str{origin}}, J.KV{"n", Codec.nat(n)}, J.KV{"stamp", J.Str{stamp}}, J.KV{"op", J.Str{op}}, J.KV{"at", Codec.nat(at)}])
    case M.WakeSet{id, by, kind, target, events, once, note, at}:
      Change.obj("WakeSet", [J.KV{"id", J.Str{id}}, J.KV{"by", J.Str{by}}, J.KV{"kind", J.Str{kind}}, J.KV{"target", J.Str{target}}, J.KV{"events", J.Json.arr(M.Wake.strs(events))}, J.KV{"once", J.Flag{once}}, J.KV{"note", J.Str{note}}, J.KV{"at", Codec.nat(at)}])
    case M.LockChanged{name, l, at}:
      Change.obj("LockChanged", [J.KV{"name", J.Str{name}}, J.KV{"lock", LK.Lock.json(l)}, J.KV{"at", Codec.nat(at)}])
    case M.WakeDropped{id, at}:
      Change.obj("WakeDropped", [J.KV{"id", J.Str{id}}, J.KV{"at", Codec.nat(at)}])
    case M.ThreadLaunched{id, by, report, at}:
      Change.obj("ThreadLaunched", [J.KV{"id", J.Str{id}}, J.KV{"by", J.Str{by}}, J.KV{"report", J.Flag{report}}, J.KV{"at", Codec.nat(at)}])
    case M.RoutinePlan{id, once, tz, kind, at}:
      Change.obj("RoutinePlan", [J.KV{"id", J.Str{id}}, J.KV{"once", Codec.nat(once)}, J.KV{"tz", J.Str{tz}}, J.KV{"kind", J.Str{kind}}, J.KV{"at", Codec.nat(at)}])
    case M.RoutineSlot{id, slot, at}:
      Change.obj("RoutineSlot", [J.KV{"id", J.Str{id}}, J.KV{"slot", Codec.nat(slot)}, J.KV{"at", Codec.nat(at)}])
    case M.RoutineDone{id, at}:
      Change.obj("RoutineDone", [J.KV{"id", J.Str{id}}, J.KV{"at", Codec.nat(at)}])
    case M.Reminded{id, bot, thread, text, at}:
      Change.obj("Reminded", [J.KV{"id", J.Str{id}}, J.KV{"bot", J.Str{bot}}, J.KV{"thread", J.Str{thread}}, J.KV{"text", J.Str{text}}, J.KV{"at", Codec.nat(at)}])
    case M.MessageFrom{thread, msg, from, kind, ref, at}:
      Change.obj("MessageFrom", [J.KV{"thread", J.Str{thread}}, J.KV{"msg", J.Str{msg}}, J.KV{"from", J.Str{from}}, J.KV{"kind", J.Str{kind}}, J.KV{"ref", J.Str{ref}}, J.KV{"at", Codec.nat(at)}])
    case M.ConvSet{id, subject, kind, parts, project, tags, at}:
      Change.obj("ConvSet", [J.KV{"id", J.Str{id}}, J.KV{"subject", J.Str{subject}}, J.KV{"kind", J.Str{kind}}, J.KV{"parts", J.Str{parts}}, J.KV{"project", J.Str{project}}, J.KV{"tags", J.Str{tags}}, J.KV{"at", Codec.nat(at)}])
    case M.MailPosted{id, conv, from, to, kind, ref, want, head, size, at}:
      Change.obj("MailPosted", [J.KV{"id", J.Str{id}}, J.KV{"conv", J.Str{conv}}, J.KV{"from", J.Str{from}}, J.KV{"to", J.Str{to}}, J.KV{"kind", J.Str{kind}}, J.KV{"ref", J.Str{ref}}, J.KV{"want", J.Str{want}}, J.KV{"head", J.Str{head}}, J.KV{"size", Codec.nat(size)}, J.KV{"at", Codec.nat(at)}])
    case M.MailMarked{bot, conv, status, seen, until, at}:
      Change.obj("MailMarked", [J.KV{"bot", J.Str{bot}}, J.KV{"conv", J.Str{conv}}, J.KV{"status", J.Str{status}}, J.KV{"seen", Codec.nat(seen)}, J.KV{"until", Codec.nat(until)}, J.KV{"at", Codec.nat(at)}])
    case M.MailTagged{bot, conv, tags, at}:
      Change.obj("MailTagged", [J.KV{"bot", J.Str{bot}}, J.KV{"conv", J.Str{conv}}, J.KV{"tags", J.Str{tags}}, J.KV{"at", Codec.nat(at)}])
    case M.RunSet{thread, bot, conv, mail, at}:
      Change.obj("RunSet", [J.KV{"thread", J.Str{thread}}, J.KV{"bot", J.Str{bot}}, J.KV{"conv", J.Str{conv}}, J.KV{"mail", J.Str{mail}}, J.KV{"at", Codec.nat(at)}])
    case M.CharterSet{bot, text, by, at}:
      Change.obj("CharterSet", [J.KV{"bot", J.Str{bot}}, J.KV{"text", J.Str{text}}, J.KV{"by", J.Str{by}}, J.KV{"at", Codec.nat(at)}])
    case M.DecisionLogged{id, bot, conv, run, text, reason, at}:
      Change.obj("DecisionLogged", [J.KV{"id", J.Str{id}}, J.KV{"bot", J.Str{bot}}, J.KV{"conv", J.Str{conv}}, J.KV{"run", J.Str{run}}, J.KV{"text", J.Str{text}}, J.KV{"reason", J.Str{reason}}, J.KV{"at", Codec.nat(at)}])
    case M.TurnUsage{thread, provider, model, key, input, cread, cwrite, output, reasoning, cost, known, basis, at}:
      Change.obj("TurnUsage", [J.KV{"thread", J.Str{thread}}, J.KV{"provider", J.Str{provider}}, J.KV{"model", J.Str{model}}, J.KV{"key", J.Str{key}},
        J.KV{"input", Codec.nat(input)}, J.KV{"cread", Codec.nat(cread)}, J.KV{"cwrite", Codec.nat(cwrite)}, J.KV{"output", Codec.nat(output)},
        J.KV{"reasoning", Codec.nat(reasoning)}, J.KV{"cost", Codec.nat(cost)}, J.KV{"known", J.Str{known}}, J.KV{"basis", J.Str{basis}}, J.KV{"at", Codec.nat(at)}])
    case M.DeployAsked{by, note, commit, cover, at}:
      Change.obj("DeployAsked", [J.KV{"by", J.Str{by}}, J.KV{"note", J.Str{note}}, J.KV{"commit", J.Str{commit}}, J.KV{"cover", J.Str{cover}}, J.KV{"at", Codec.nat(at)}])
    case M.DeployBuilt{commit, state, version, detail, at}:
      Change.obj("DeployBuilt", [J.KV{"commit", J.Str{commit}}, J.KV{"state", J.Str{state}}, J.KV{"version", J.Str{version}}, J.KV{"detail", J.Str{detail}}, J.KV{"at", Codec.nat(at)}])
    case M.CtxNoted{thread, l, i, text, at}:
      Change.obj("CtxNoted", [J.KV{"thread", J.Str{thread}}, J.KV{"l", Codec.nat(l)}, J.KV{"i", Codec.nat(i)}, J.KV{"text", J.Str{text}}, J.KV{"at", Codec.nat(at)}])

def Change.tags() -> List<&2, String>:
  ["ProjectCreated", "ProjectRemoved", "ThreadCreated", "ThreadRenamed", "MessagePosted", "TurnChanged", "PendingChanged", "ActivityLogged", "ThreadSettled", "ThreadUnsettled", "ThreadArchived", "ThreadPinned", "ThreadDeleted", "WorktreeSet", "SessionSet", "ModesSet", "TaskDelegated", "TaskStateSet", "TaskDelivered", "SettingSet", "TurnQueued", "QueueTaken", "AskOpened", "AskClosed", "DeviceRegistered", "DeviceDropped", "ThreadSnoozed", "ThreadOrdered", "WorktreeSkipped", "ThreadMerged", "WorktreeRemoved", "ProjectOrdered", "QueueEdited", "QueueRemoved", "QueueMoved", "QueueTakenOne", "ThreadForked", "ProviderSet", "TodosSet", "UsageSet", "BotSet", "BotDeleted", "RoomSet", "RoomDeleted", "RoomPosted", "HookSet", "HookHit", "RoutineSet", "RoutineDeleted", "RoutineFired", "MemorySaved", "MemoryForgot", "SpaceSet", "PeerSet", "BotWoke", "SpacePageSet", "WorkSet", "UberSet", "UberOp", "ThreadLaunched", "WakeSet", "WakeDropped", "LockChanged", "RoutinePlan", "RoutineSlot", "RoutineDone", "Reminded", "TaskSet", "TaskDeleted", "MessageFrom", "ConvSet", "MailPosted", "MailMarked", "MailTagged", "RunSet", "CharterSet", "DecisionLogged", "TurnUsage", "DeployAsked", "DeployBuilt", "CtxNoted"]

def Codec.wake.events(xs: List<&2, J.Json>) -> List<&2, String>:
  match xs:
    case Nil{}:
      Nil{}
    case x <> t:
      J.Json.as_str(Some{x}, "") <> Codec.wake.events(t)

def Change.decode.k(i: Nat, +o: J.Json) -> Maybe<&2, M.Change>:
  match i:
    case 0n:
      Some{M.ProjectCreated{J.Json.get_str(o, "id", ""), J.Json.get_str(o, "title", ""), J.Json.get_str(o, "root", ""), J.Json.get_nat(o, "at", 0n)}}
    case 1n:
      Some{M.ProjectRemoved{J.Json.get_str(o, "id", ""), J.Json.get_nat(o, "at", 0n)}}
    case 2n:
      Some{M.ThreadCreated{J.Json.get_str(o, "id", ""), J.Json.get_str(o, "project", ""), J.Json.get_str(o, "title", ""), Env.of(J.Json.get_str(o, "env", "")), J.Json.get_str(o, "branch", ""), J.Json.get_str(o, "worktree", ""), J.Json.get_str(o, "provider", ""), J.Json.get_str(o, "model", ""), J.Json.get_nat(o, "at", 0n)}}
    case 3n:
      Some{M.ThreadRenamed{J.Json.get_str(o, "id", ""), J.Json.get_str(o, "title", ""), J.Json.get_nat(o, "at", 0n)}}
    case 4n:
      Some{M.MessagePosted{J.Json.get_str(o, "thread", ""), J.Json.get_str(o, "msg", ""), Role.of(J.Json.get_str(o, "role", "")), J.Json.get_str(o, "text", ""), J.Json.get_nat(o, "at", 0n)}}
    case 5n:
      Some{M.TurnChanged{J.Json.get_str(o, "thread", ""), Turn.of(J.Json.get_str(o, "state", "")), J.Json.get_nat(o, "at", 0n)}}
    case 6n:
      Some{M.PendingChanged{J.Json.get_str(o, "thread", ""), J.Json.get_nat(o, "count", 0n), J.Json.get_nat(o, "at", 0n)}}
    case 7n:
      Some{M.ActivityLogged{J.Json.get_str(o, "thread", ""), J.Json.get_str(o, "act", ""), J.Json.get_str(o, "tone", ""), J.Json.get_str(o, "kind", ""), J.Json.get_str(o, "summary", ""), J.Json.get_nat(o, "at", 0n)}}
    case 8n:
      Some{M.ThreadSettled{J.Json.get_str(o, "id", ""), J.Json.get_bool(o, "manual", False{}), J.Json.get_nat(o, "at", 0n)}}
    case 9n:
      Some{M.ThreadUnsettled{J.Json.get_str(o, "id", ""), J.Json.get_nat(o, "at", 0n)}}
    case 10n:
      Some{M.ThreadArchived{J.Json.get_str(o, "id", ""), J.Json.get_bool(o, "archived", False{}), J.Json.get_nat(o, "at", 0n)}}
    case 11n:
      Some{M.ThreadPinned{J.Json.get_str(o, "id", ""), J.Json.get_bool(o, "pinned", False{}), J.Json.get_nat(o, "at", 0n)}}
    case 12n:
      Some{M.ThreadDeleted{J.Json.get_str(o, "id", ""), J.Json.get_nat(o, "at", 0n)}}
    case 13n:
      Some{M.WorktreeSet{J.Json.get_str(o, "id", ""), J.Json.get_str(o, "branch", ""), J.Json.get_str(o, "worktree", ""), J.Json.get_nat(o, "at", 0n)}}
    case 14n:
      Some{M.SessionSet{J.Json.get_str(o, "id", ""), J.Json.get_str(o, "session", ""), J.Json.get_nat(o, "at", 0n)}}
    case 15n:
      Some{M.ModesSet{J.Json.get_str(o, "id", ""), Runtime.of(J.Json.get_str(o, "runtime", "")), Interaction.of(J.Json.get_str(o, "interaction", "")), J.Json.get_str(o, "model", ""), J.Json.get_nat(o, "at", 0n)}}
    case 16n:
      Some{M.TaskDelegated{J.Json.get_str(o, "task", ""), J.Json.get_str(o, "parent", ""), J.Json.get_str(o, "child", ""), J.Json.get_nat(o, "depth", 0n), J.Json.get_str(o, "provider", ""), J.Json.get_str(o, "model", ""), J.Json.get_nat(o, "at", 0n)}}
    case 17n:
      Some{M.TaskStateSet{J.Json.get_str(o, "task", ""), TaskSt.of(J.Json.get_str(o, "state", "")), J.Json.get_str(o, "summary", ""), J.Json.get_nat(o, "at", 0n)}}
    case 18n:
      Some{M.TaskDelivered{J.Json.get_str(o, "task", ""), J.Json.get_nat(o, "at", 0n)}}
    case 19n:
      Some{M.SettingSet{J.Json.get_str(o, "key", ""), J.Json.get_str(o, "value", ""), J.Json.get_nat(o, "at", 0n)}}
    case 20n:
      Some{M.TurnQueued{J.Json.get_str(o, "thread", ""), J.Json.get_str(o, "msg", ""), J.Json.get_str(o, "text", ""), J.Json.get_nat(o, "at", 0n)}}
    case 21n:
      Some{M.QueueTaken{J.Json.get_str(o, "thread", ""), J.Json.get_nat(o, "at", 0n)}}
    case 22n:
      Some{M.AskOpened{J.Json.get_str(o, "id", ""), J.Json.get_str(o, "thread", ""), J.Json.get_str(o, "kind", ""), J.Json.get_str(o, "tool", ""), J.Json.get_str(o, "detail", ""), J.Json.get_str(o, "options", ""), J.Json.get_nat(o, "at", 0n)}}
    case 23n:
      Some{M.AskClosed{J.Json.get_str(o, "id", ""), J.Json.get_str(o, "answer", ""), J.Json.get_nat(o, "at", 0n)}}
    case 24n:
      Some{M.DeviceRegistered{J.Json.get_str(o, "token", ""), J.Json.get_str(o, "platform", ""), J.Json.get_str(o, "kind", ""), J.Json.get_str(o, "thread", ""), J.Json.get_str(o, "env", ""), J.Json.get_str(o, "bundle", ""), J.Json.get_nat(o, "at", 0n)}}
    case 25n:
      Some{M.DeviceDropped{J.Json.get_str(o, "token", ""), J.Json.get_nat(o, "at", 0n)}}
    case 26n:
      Some{M.ThreadSnoozed{J.Json.get_str(o, "id", ""), J.Json.get_nat(o, "until", 0n), J.Json.get_nat(o, "at", 0n)}}
    case 27n:
      Some{M.ThreadOrdered{J.Json.get_str(o, "id", ""), J.Json.get_str(o, "section", ""), J.Json.get_str(o, "key", ""), J.Json.get_nat(o, "at", 0n)}}
    case 28n:
      Some{M.WorktreeSkipped{J.Json.get_str(o, "id", ""), J.Json.get_nat(o, "at", 0n)}}
    case 29n:
      Some{M.ThreadMerged{J.Json.get_str(o, "id", ""), J.Json.get_str(o, "url", ""), J.Json.get_nat(o, "at", 0n)}}
    case 30n:
      Some{M.WorktreeRemoved{J.Json.get_str(o, "id", ""), J.Json.get_nat(o, "at", 0n)}}
    case 31n:
      Some{M.ProjectOrdered{J.Json.get_str(o, "id", ""), J.Json.get_str(o, "key", ""), J.Json.get_nat(o, "at", 0n)}}
    case 32n:
      Some{M.QueueEdited{J.Json.get_str(o, "thread", ""), J.Json.get_str(o, "msg", ""), J.Json.get_str(o, "text", ""), J.Json.get_nat(o, "at", 0n)}}
    case 33n:
      Some{M.QueueRemoved{J.Json.get_str(o, "thread", ""), J.Json.get_str(o, "msg", ""), J.Json.get_nat(o, "at", 0n)}}
    case 34n:
      Some{M.QueueMoved{J.Json.get_str(o, "thread", ""), J.Json.get_str(o, "msg", ""), J.Json.get_nat(o, "to", 0n), J.Json.get_nat(o, "at", 0n)}}
    case 35n:
      Some{M.QueueTakenOne{J.Json.get_str(o, "thread", ""), J.Json.get_str(o, "msg", ""), J.Json.get_nat(o, "at", 0n)}}
    case 36n:
      Some{M.ThreadForked{J.Json.get_str(o, "id", ""), J.Json.get_str(o, "source", ""), J.Json.get_nat(o, "at", 0n)}}
    case 37n:
      Some{M.ProviderSet{J.Json.get_str(o, "id", ""), J.Json.get_str(o, "provider", ""), J.Json.get_str(o, "model", ""), J.Json.get_nat(o, "at", 0n)}}
    case 38n:
      Some{M.TodosSet{J.Json.get_str(o, "thread", ""), J.Json.get_str(o, "todos", ""), J.Json.get_nat(o, "at", 0n)}}
    case 39n:
      Some{M.UsageSet{J.Json.get_str(o, "thread", ""), J.Json.get_nat(o, "used", 0n), J.Json.get_nat(o, "window", 0n), J.Json.get_nat(o, "at", 0n)}}
    case 40n:
      Some{M.BotSet{J.Json.get_str(o, "id", ""), J.Json.get_str(o, "name", ""), J.Json.get_str(o, "thread", ""), J.Json.get_nat(o, "look", 0n), J.Json.get_str(o, "persona", ""), J.Json.get_nat(o, "at", 0n)}}
    case 41n:
      Some{M.BotDeleted{J.Json.get_str(o, "id", ""), J.Json.get_nat(o, "at", 0n)}}
    case 42n:
      Some{M.RoomSet{J.Json.get_str(o, "id", ""), J.Json.get_str(o, "name", ""), J.Json.get_str(o, "members", ""), J.Json.get_nat(o, "at", 0n)}}
    case 43n:
      Some{M.RoomDeleted{J.Json.get_str(o, "id", ""), J.Json.get_nat(o, "at", 0n)}}
    case 44n:
      Some{M.RoomPosted{J.Json.get_str(o, "id", ""), J.Json.get_str(o, "room", ""), J.Json.get_str(o, "from", ""), J.Json.get_str(o, "text", ""), J.Json.get_nat(o, "hop", 0n), J.Json.get_nat(o, "at", 0n)}}
    case 45n:
      Some{M.HookSet{J.Json.get_str(o, "id", ""), J.Json.get_str(o, "bot", ""), J.Json.get_str(o, "name", ""), J.Json.get_bool(o, "revoked", False{}), J.Json.get_nat(o, "at", 0n)}}
    case 46n:
      Some{M.HookHit{J.Json.get_str(o, "id", ""), J.Json.get_nat(o, "at", 0n)}}
    case 47n:
      Some{M.RoutineSet{J.Json.get_str(o, "id", ""), J.Json.get_str(o, "bot", ""), J.Json.get_str(o, "name", ""), J.Json.get_str(o, "cron", ""), J.Json.get_str(o, "prompt", ""), J.Json.get_bool(o, "on", False{}), J.Json.get_nat(o, "at", 0n)}}
    case 48n:
      Some{M.RoutineDeleted{J.Json.get_str(o, "id", ""), J.Json.get_nat(o, "at", 0n)}}
    case 49n:
      Some{M.RoutineFired{J.Json.get_str(o, "id", ""), J.Json.get_nat(o, "at", 0n)}}
    case 50n:
      Some{M.MemorySaved{J.Json.get_str(o, "bot", ""), J.Json.get_str(o, "key", ""), J.Json.get_str(o, "kind", ""), J.Json.get_str(o, "text", ""), J.Json.get_str(o, "tags", ""), J.Json.get_nat(o, "at", 0n)}}
    case 51n:
      Some{M.MemoryForgot{J.Json.get_str(o, "bot", ""), J.Json.get_str(o, "key", ""), J.Json.get_nat(o, "at", 0n)}}
    case 52n:
      Some{M.SpaceSet{J.Json.get_str(o, "bot", ""), J.Json.get_str(o, "spec", ""), J.Json.get_nat(o, "at", 0n)}}
    case 53n:
      Some{M.PeerSet{J.Json.get_str(o, "id", ""), J.Json.get_str(o, "name", ""), J.Json.get_str(o, "url", ""), J.Json.get_bool(o, "revoked", False{}), J.Json.get_nat(o, "at", 0n)}}
    case 54n:
      Some{M.BotWoke{J.Json.get_str(o, "bot", ""), J.Json.get_nat(o, "hop", 0n), J.Json.get_nat(o, "at", 0n)}}
    case 55n:
      Some{M.SpacePageSet{J.Json.get_str(o, "bot", ""), J.Json.get_str(o, "html", ""), J.Json.get_nat(o, "at", 0n)}}
    case 56n:
      Some{M.WorkSet{J.Json.get_str(o, "thread", ""), J.Json.get_str(o, "kind", ""), J.Json.get_str(o, "rows", ""), J.Json.get_nat(o, "at", 0n)}}
    case 57n:
      Some{M.UberSet{J.Json.get_str(o, "bot", ""), J.Json.get_str(o, "uid", ""), J.Json.get_nat(o, "at", 0n)}}
    case 58n:
      Some{M.UberOp{J.Json.get_str(o, "uid", ""), J.Json.get_str(o, "origin", ""), J.Json.get_nat(o, "n", 0n), J.Json.get_str(o, "stamp", ""), J.Json.get_str(o, "op", ""), J.Json.get_nat(o, "at", 0n)}}
    case 60n:
      Some{M.WakeSet{J.Json.get_str(o, "id", ""), J.Json.get_str(o, "by", ""), J.Json.get_str(o, "kind", ""), J.Json.get_str(o, "target", ""), Codec.wake.events(J.Json.elems(J.Json.get_obj(o, "events"))), J.Json.get_bool(o, "once", True{}), J.Json.get_str(o, "note", ""), J.Json.get_nat(o, "at", 0n)}}
    case 62n:
      Some{M.LockChanged{J.Json.get_str(o, "name", ""), LK.Lock.read(J.Json.get_obj(o, "lock")), J.Json.get_nat(o, "at", 0n)}}
    case 61n:
      Some{M.WakeDropped{J.Json.get_str(o, "id", ""), J.Json.get_nat(o, "at", 0n)}}
    case 59n:
      Some{M.ThreadLaunched{J.Json.get_str(o, "id", ""), J.Json.get_str(o, "by", ""), J.Json.get_bool(o, "report", True{}), J.Json.get_nat(o, "at", 0n)}}
    case 63n:
      Some{M.RoutinePlan{J.Json.get_str(o, "id", ""), J.Json.get_nat(o, "once", 0n), J.Json.get_str(o, "tz", ""), J.Json.get_str(o, "kind", ""), J.Json.get_nat(o, "at", 0n)}}
    case 64n:
      Some{M.RoutineSlot{J.Json.get_str(o, "id", ""), J.Json.get_nat(o, "slot", 0n), J.Json.get_nat(o, "at", 0n)}}
    case 65n:
      Some{M.RoutineDone{J.Json.get_str(o, "id", ""), J.Json.get_nat(o, "at", 0n)}}
    case 66n:
      Some{M.Reminded{J.Json.get_str(o, "id", ""), J.Json.get_str(o, "bot", ""), J.Json.get_str(o, "thread", ""), J.Json.get_str(o, "text", ""), J.Json.get_nat(o, "at", 0n)}}
    case 67n:
      Some{M.TaskSet{J.Json.get_str(o, "bot", ""), J.Json.get_str(o, "slug", ""), J.Json.get_str(o, "json", ""), J.Json.get_nat(o, "at", 0n)}}
    case 68n:
      Some{M.TaskDeleted{J.Json.get_str(o, "bot", ""), J.Json.get_str(o, "slug", ""), J.Json.get_nat(o, "at", 0n)}}
    case 69n:
      Some{M.MessageFrom{J.Json.get_str(o, "thread", ""), J.Json.get_str(o, "msg", ""), J.Json.get_str(o, "from", ""), J.Json.get_str(o, "kind", ""), J.Json.get_str(o, "ref", ""), J.Json.get_nat(o, "at", 0n)}}
    case 70n:
      Some{M.ConvSet{J.Json.get_str(o, "id", ""), J.Json.get_str(o, "subject", ""), J.Json.get_str(o, "kind", ""), J.Json.get_str(o, "parts", ""), J.Json.get_str(o, "project", ""), J.Json.get_str(o, "tags", ""), J.Json.get_nat(o, "at", 0n)}}
    case 71n:
      Some{M.MailPosted{J.Json.get_str(o, "id", ""), J.Json.get_str(o, "conv", ""), J.Json.get_str(o, "from", ""), J.Json.get_str(o, "to", ""), J.Json.get_str(o, "kind", ""), J.Json.get_str(o, "ref", ""), J.Json.get_str(o, "want", ""), J.Json.get_str(o, "head", ""), J.Json.get_nat(o, "size", 0n), J.Json.get_nat(o, "at", 0n)}}
    case 72n:
      Some{M.MailMarked{J.Json.get_str(o, "bot", ""), J.Json.get_str(o, "conv", ""), J.Json.get_str(o, "status", ""), J.Json.get_nat(o, "seen", 0n), J.Json.get_nat(o, "until", 0n), J.Json.get_nat(o, "at", 0n)}}
    case 73n:
      Some{M.MailTagged{J.Json.get_str(o, "bot", ""), J.Json.get_str(o, "conv", ""), J.Json.get_str(o, "tags", ""), J.Json.get_nat(o, "at", 0n)}}
    case 74n:
      Some{M.RunSet{J.Json.get_str(o, "thread", ""), J.Json.get_str(o, "bot", ""), J.Json.get_str(o, "conv", ""), J.Json.get_str(o, "mail", ""), J.Json.get_nat(o, "at", 0n)}}
    case 75n:
      Some{M.CharterSet{J.Json.get_str(o, "bot", ""), J.Json.get_str(o, "text", ""), J.Json.get_str(o, "by", ""), J.Json.get_nat(o, "at", 0n)}}
    case 76n:
      Some{M.DecisionLogged{J.Json.get_str(o, "id", ""), J.Json.get_str(o, "bot", ""), J.Json.get_str(o, "conv", ""), J.Json.get_str(o, "run", ""), J.Json.get_str(o, "text", ""), J.Json.get_str(o, "reason", ""), J.Json.get_nat(o, "at", 0n)}}
    case 77n:
      Some{M.TurnUsage{J.Json.get_str(o, "thread", ""), J.Json.get_str(o, "provider", ""), J.Json.get_str(o, "model", ""), J.Json.get_str(o, "key", ""),
        J.Json.get_nat(o, "input", 0n), J.Json.get_nat(o, "cread", 0n), J.Json.get_nat(o, "cwrite", 0n), J.Json.get_nat(o, "output", 0n),
        J.Json.get_nat(o, "reasoning", 0n), J.Json.get_nat(o, "cost", 0n), J.Json.get_str(o, "known", ""), J.Json.get_str(o, "basis", ""), J.Json.get_nat(o, "at", 0n)}}
    case 78n:
      Some{M.DeployAsked{J.Json.get_str(o, "by", ""), J.Json.get_str(o, "note", ""), J.Json.get_str(o, "commit", ""), J.Json.get_str(o, "cover", ""), J.Json.get_nat(o, "at", 0n)}}
    case 80n:
      Some{M.CtxNoted{J.Json.get_str(o, "thread", ""), J.Json.get_nat(o, "l", 0n), J.Json.get_nat(o, "i", 0n), J.Json.get_str(o, "text", ""), J.Json.get_nat(o, "at", 0n)}}
    case 79n:
      Some{M.DeployBuilt{J.Json.get_str(o, "commit", ""), J.Json.get_str(o, "state", ""), J.Json.get_str(o, "version", ""), J.Json.get_str(o, "detail", ""), J.Json.get_nat(o, "at", 0n)}}
    case _:
      None{}

# a Change from its JSON, or None for an unknown tag
def Change.decode(+o: J.Json) -> Maybe<&2, M.Change>:
  Change.decode.k(Codec.pos(Change.tags(), J.Json.get_str(o, "$", "")), o)

# What clients see
# ----------------
# The log on disk keeps every field; the copy sent to clients (the "log" and
# "changes" messages) carries device events with only their tag and time.
# A push token is a secret, and clients fold an empty token as no device,
# so their sequence numbers still match the server's (law
# device_token_never_sent).

def Change.wire(c: M.Change) -> J.Json:
  match c:
    case M.DeviceRegistered{_, _, _, _, _, _, at}:
      Change.obj("DeviceRegistered", [J.KV{"at", Codec.nat(at)}])
    case M.DeviceDropped{_, at}:
      Change.obj("DeviceDropped", [J.KV{"at", Codec.nat(at)}])
    case M.ProjectCreated{id, title, root, at}:
      Change.encode(M.ProjectCreated{id, title, root, at})
    case M.ProjectRemoved{id, at}:
      Change.encode(M.ProjectRemoved{id, at})
    case M.ThreadCreated{id, project, title, env, branch, worktree, provider, model, at}:
      Change.encode(M.ThreadCreated{id, project, title, env, branch, worktree, provider, model, at})
    case M.ThreadRenamed{id, title, at}:
      Change.encode(M.ThreadRenamed{id, title, at})
    case M.MessagePosted{thread, msg, role, text, at}:
      Change.encode(M.MessagePosted{thread, msg, role, text, at})
    case M.TurnChanged{thread, state, at}:
      Change.encode(M.TurnChanged{thread, state, at})
    case M.PendingChanged{thread, count, at}:
      Change.encode(M.PendingChanged{thread, count, at})
    case M.ActivityLogged{thread, act, tone, kind, summary, at}:
      Change.encode(M.ActivityLogged{thread, act, tone, kind, summary, at})
    case M.ThreadSettled{id, manual, at}:
      Change.encode(M.ThreadSettled{id, manual, at})
    case M.ThreadUnsettled{id, at}:
      Change.encode(M.ThreadUnsettled{id, at})
    case M.ThreadArchived{id, archived, at}:
      Change.encode(M.ThreadArchived{id, archived, at})
    case M.ThreadPinned{id, pinned, at}:
      Change.encode(M.ThreadPinned{id, pinned, at})
    case M.ThreadDeleted{id, at}:
      Change.encode(M.ThreadDeleted{id, at})
    case M.WorktreeSet{id, branch, worktree, at}:
      Change.encode(M.WorktreeSet{id, branch, worktree, at})
    case M.SessionSet{id, session, at}:
      Change.encode(M.SessionSet{id, session, at})
    case M.ModesSet{id, runtime, interaction, model, at}:
      Change.encode(M.ModesSet{id, runtime, interaction, model, at})
    case M.TaskDelegated{task, parent, child, depth, provider, model, at}:
      Change.encode(M.TaskDelegated{task, parent, child, depth, provider, model, at})
    case M.TaskStateSet{task, state, summary, at}:
      Change.encode(M.TaskStateSet{task, state, summary, at})
    case M.TaskDelivered{task, at}:
      Change.encode(M.TaskDelivered{task, at})
    case M.SettingSet{key, value, at}:
      Change.encode(M.SettingSet{key, value, at})
    case M.TurnQueued{thread, msg, text, at}:
      Change.encode(M.TurnQueued{thread, msg, text, at})
    case M.QueueTaken{thread, at}:
      Change.encode(M.QueueTaken{thread, at})
    case M.AskOpened{id, thread, kind, tool, detail, options, at}:
      Change.encode(M.AskOpened{id, thread, kind, tool, detail, options, at})
    case M.AskClosed{id, answer, at}:
      Change.encode(M.AskClosed{id, answer, at})
    case M.ThreadSnoozed{id, until, at}:
      Change.encode(M.ThreadSnoozed{id, until, at})
    case M.ThreadOrdered{id, section, key, at}:
      Change.encode(M.ThreadOrdered{id, section, key, at})
    case M.WorktreeSkipped{id, at}:
      Change.encode(M.WorktreeSkipped{id, at})
    case M.ThreadMerged{id, url, at}:
      Change.encode(M.ThreadMerged{id, url, at})
    case M.WorktreeRemoved{id, at}:
      Change.encode(M.WorktreeRemoved{id, at})
    case M.ProjectOrdered{id, key, at}:
      Change.encode(M.ProjectOrdered{id, key, at})
    case M.QueueEdited{thread, msg, text, at}:
      Change.encode(M.QueueEdited{thread, msg, text, at})
    case M.QueueRemoved{thread, msg, at}:
      Change.encode(M.QueueRemoved{thread, msg, at})
    case M.QueueMoved{thread, msg, to, at}:
      Change.encode(M.QueueMoved{thread, msg, to, at})
    case M.QueueTakenOne{thread, msg, at}:
      Change.encode(M.QueueTakenOne{thread, msg, at})
    case M.ThreadForked{id, source, at}:
      Change.encode(M.ThreadForked{id, source, at})
    case M.ProviderSet{id, provider, model, at}:
      Change.encode(M.ProviderSet{id, provider, model, at})
    case M.TodosSet{thread, todos, at}:
      Change.encode(M.TodosSet{thread, todos, at})
    case M.UsageSet{thread, used, window, at}:
      Change.encode(M.UsageSet{thread, used, window, at})
    case M.BotSet{id, name, thread, look, persona, at}:
      Change.encode(M.BotSet{id, name, thread, look, persona, at})
    case M.BotDeleted{id, at}:
      Change.encode(M.BotDeleted{id, at})
    case M.RoomSet{id, name, members, at}:
      Change.encode(M.RoomSet{id, name, members, at})
    case M.RoomDeleted{id, at}:
      Change.encode(M.RoomDeleted{id, at})
    case M.RoomPosted{id, room, from, text, hop, at}:
      Change.encode(M.RoomPosted{id, room, from, text, hop, at})
    case M.HookSet{id, bot, name, revoked, at}:
      Change.encode(M.HookSet{id, bot, name, revoked, at})
    case M.HookHit{id, at}:
      Change.encode(M.HookHit{id, at})
    case M.RoutineSet{id, bot, name, cron, prompt, on, at}:
      Change.encode(M.RoutineSet{id, bot, name, cron, prompt, on, at})
    case M.RoutineDeleted{id, at}:
      Change.encode(M.RoutineDeleted{id, at})
    case M.RoutineFired{id, at}:
      Change.encode(M.RoutineFired{id, at})
    case M.MemorySaved{bot, key, kind, text, tags, at}:
      Change.encode(M.MemorySaved{bot, key, kind, text, tags, at})
    case M.MemoryForgot{bot, key, at}:
      Change.encode(M.MemoryForgot{bot, key, at})
    case M.SpaceSet{bot, spec, at}:
      Change.encode(M.SpaceSet{bot, spec, at})
    case M.PeerSet{id, name, url, revoked, at}:
      Change.encode(M.PeerSet{id, name, url, revoked, at})
    case M.BotWoke{bot, hop, at}:
      Change.encode(M.BotWoke{bot, hop, at})
    case M.TaskSet{bot, slug, json, at}:
      Change.encode(M.TaskSet{bot, slug, json, at})
    case M.TaskDeleted{bot, slug, at}:
      Change.encode(M.TaskDeleted{bot, slug, at})
    case M.SpacePageSet{bot, html, at}:
      Change.encode(M.SpacePageSet{bot, html, at})
    case M.WorkSet{thread, kind, rows, at}:
      Change.encode(M.WorkSet{thread, kind, rows, at})
    case M.UberSet{bot, uid, at}:
      Change.encode(M.UberSet{bot, uid, at})
    case M.UberOp{uid, origin, n, stamp, op, at}:
      Change.encode(M.UberOp{uid, origin, n, stamp, op, at})
    case M.WakeSet{id, by, kind, target, events, once, note, at}:
      Change.encode(M.WakeSet{id, by, kind, target, events, once, note, at})
    case M.LockChanged{name, l, at}:
      Change.encode(M.LockChanged{name, l, at})
    case M.WakeDropped{id, at}:
      Change.encode(M.WakeDropped{id, at})
    case M.ThreadLaunched{id, by, report, at}:
      Change.encode(M.ThreadLaunched{id, by, report, at})
    case M.RoutinePlan{id, once, tz, kind, at}:
      Change.encode(M.RoutinePlan{id, once, tz, kind, at})
    case M.RoutineSlot{id, slot, at}:
      Change.encode(M.RoutineSlot{id, slot, at})
    case M.RoutineDone{id, at}:
      Change.encode(M.RoutineDone{id, at})
    case M.Reminded{id, bot, thread, text, at}:
      Change.encode(M.Reminded{id, bot, thread, text, at})
    case M.MessageFrom{thread, msg, from, kind, ref, at}:
      Change.encode(M.MessageFrom{thread, msg, from, kind, ref, at})
    case M.ConvSet{id, subject, kind, parts, project, tags, at}:
      Change.encode(M.ConvSet{id, subject, kind, parts, project, tags, at})
    case M.MailPosted{id, conv, from, to, kind, ref, want, head, size, at}:
      Change.encode(M.MailPosted{id, conv, from, to, kind, ref, want, head, size, at})
    case M.MailMarked{bot, conv, status, seen, until, at}:
      Change.encode(M.MailMarked{bot, conv, status, seen, until, at})
    case M.MailTagged{bot, conv, tags, at}:
      Change.encode(M.MailTagged{bot, conv, tags, at})
    case M.RunSet{thread, bot, conv, mail, at}:
      Change.encode(M.RunSet{thread, bot, conv, mail, at})
    case M.CharterSet{bot, text, by, at}:
      Change.encode(M.CharterSet{bot, text, by, at})
    case M.DecisionLogged{id, bot, conv, run, text, reason, at}:
      Change.encode(M.DecisionLogged{id, bot, conv, run, text, reason, at})
    case M.TurnUsage{thread, provider, model, key, input, cread, cwrite, output, reasoning, cost, known, basis, at}:
      Change.encode(M.TurnUsage{thread, provider, model, key, input, cread, cwrite, output, reasoning, cost, known, basis, at})
    case M.DeployAsked{by, note, commit, cover, at}:
      Change.encode(M.DeployAsked{by, note, commit, cover, at})
    case M.DeployBuilt{commit, state, version, detail, at}:
      Change.encode(M.DeployBuilt{commit, state, version, detail, at})
    case M.CtxNoted{_, _, _, _, at}:
      Change.obj("CtxNoted", [J.KV{"at", Codec.nat(at)}])

# (CtxNoted too: the uberbot's memory lines are for its agent only, and
# clients fold it to nothing)
def Change.private(+tag: String) -> Bool:
  Bool.or(Bool.or(String.eq(tag, "DeviceRegistered"), String.eq(tag, "DeviceDropped")), String.eq(tag, "CtxNoted"))

def Change.wire_line.k(private: Bool, +o: J.Json, l: String) -> String:
  match private:
    case True{}:
      J.Json.show(Change.obj(J.Json.get_str(o, "$", ""), [J.KV{"at", Codec.nat(J.Json.get_nat(o, "at", 0n))}]))
    case False{}:
      l

# a stored line (parsed as o) as clients may see it
def Change.wire_line(+o: J.Json, l: String) -> String:
  Change.wire_line.k(Change.private(J.Json.get_str(o, "$", "")), o, l)
