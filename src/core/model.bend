import Base
import ./bot.bend as BT
import ./json.bend as J
import ./ubop.bend as UO
import ./locks.bend as LK
import ./mail.bend as ML
import ./ship.bend as SH
import ./todo.bend as TD

# The product's state, event-sourced. Every change is a Change appended to
# the log; the read model is `replay(log)`. Events are persisted forever:
# add variants, never change what an existing one means.
#
# Times are Nat seconds since the Unix epoch.

# Enums
# -----

type Role is Data:
  User{}
  Assistant{}
  System{}

type TurnState is Data:
  Idle{}
  Running{}
  Completed{}
  Interrupted{}
  Failed{}

# A manual settle/unsettle holds the shelf until fresh real work resumes.
type Override is Data:
  NoOverride{}
  ForcedSettled{}
  ForcedActive{}

type EnvMode is Data:
  Local{}
  Worktree{}

type RuntimeMode is Data:
  Supervised{}
  AcceptEdits{}
  AutoMode{}
  FullAccess{}

type InteractionMode is Data:
  DefaultMode{}
  PlanMode{}

# Entities
# --------

type Project is Data:
  Project{id: String, title: String, root: String, created: Nat, removed: Bool}

type Thread is Data:
  Thread{
    id: String,
    project: String,
    title: String,
    created: Nat,
    activity: Nat,
    turn: TurnState,
    pending: Nat,
    archived: Bool,
    override: Override,
    settled_at: Nat,
    unsettled_at: Nat,
    pinned: Bool,
    env: EnvMode,
    branch: String,
    worktree: String,
    provider: String,
    model: String,
    runtime: RuntimeMode,
    interaction: InteractionMode,
    session: String,
    deleted: Bool
  }

# one item of a thread's timeline: a message, or a tool call / approval /
# error. Entries keep log order, so a timeline never needs re-sorting.
type Entry is Data:
  Msg{id: String, thread: String, role: Role, text: String, at: Nat}
  Act{id: String, thread: String, tone: String, kind: String, summary: String, at: Nat}

# Events
# ------

# Delegated tasks
# ---------------
# A thread (the parent) may delegate a task to a child thread that runs on
# any provider and model. A task's state only moves forward: once done,
# failed or cancelled it never changes again (law task_terminal_absorbing),
# its result reaches the parent exactly once (law task_delivered_once), and
# delegation stops at a fixed depth (law delegation_bounded), so models
# cannot spawn each other without end.

type TaskState is Data:
  TaskQueued{}
  TaskRunning{}
  TaskDone{}
  TaskFailed{}
  TaskCancelled{}

type Task is Data:
  Task{id: String, parent: String, child: String, depth: Nat, state: TaskState, summary: String, delivered: Bool, provider: String, model: String}

def TaskState.terminal(s: TaskState) -> Bool:
  match s:
    case TaskDone{}:
      True{}
    case TaskFailed{}:
      True{}
    case TaskCancelled{}:
      True{}
    case _:
      False{}

# the state after asking for `want`: terminal states stay; a running task
# never goes back to queued
def TaskState.next(cur: TaskState, want: TaskState) -> TaskState:
  match cur want:
    case TaskDone{} _:
      TaskDone{}
    case TaskFailed{} _:
      TaskFailed{}
    case TaskCancelled{} _:
      TaskCancelled{}
    case TaskRunning{} TaskQueued{}:
      TaskRunning{}
    case _ _:
      want

def Task.max_depth() -> Nat:
  3n

# whether a thread at this depth (0 for a top-level thread) may delegate
def Task.may_delegate(+depth: Nat) -> Bool:
  Nat.is_lt(depth, Task.max_depth())

def Task.step(t: Task, want: TaskState, summary: String) -> Task:
  match t:
    case Task{i, p, c, d, cur, _, dl, pr, mo}:
      Task{i, p, c, d, TaskState.next(cur, want), summary, dl, pr, mo}

def Task.delivered_set(t: Task) -> Task:
  match t:
    case Task{i, p, c, d, st, su, _, pr, mo}:
      Task{i, p, c, d, st, su, True{}, pr, mo}

# a finished task whose result the parent has not received yet
def Task.pending(t: Task) -> Bool:
  match t:
    case Task{_, _, _, _, st, _, dl, _, _}:
      Bool.and(TaskState.terminal(st), Bool.not(dl))

def Task.blank(id: String) -> Task:
  Task{id, "", "", 0n, TaskCancelled{}, "", True{}, "", ""}

type Change is Data:
  ProjectCreated{id: String, title: String, root: String, at: Nat}
  ProjectRemoved{id: String, at: Nat}
  ThreadCreated{id: String, project: String, title: String, env: EnvMode, branch: String, worktree: String, provider: String, model: String, at: Nat}
  ThreadRenamed{id: String, title: String, at: Nat}
  MessagePosted{thread: String, msg: String, role: Role, text: String, at: Nat}
  TurnChanged{thread: String, state: TurnState, at: Nat}
  PendingChanged{thread: String, count: Nat, at: Nat}
  ActivityLogged{thread: String, act: String, tone: String, kind: String, summary: String, at: Nat}
  ThreadSettled{id: String, manual: Bool, at: Nat}
  ThreadUnsettled{id: String, at: Nat}
  ThreadArchived{id: String, archived: Bool, at: Nat}
  ThreadPinned{id: String, pinned: Bool, at: Nat}
  ThreadDeleted{id: String, at: Nat}
  WorktreeSet{id: String, branch: String, worktree: String, at: Nat}
  SessionSet{id: String, session: String, at: Nat}
  ModesSet{id: String, runtime: RuntimeMode, interaction: InteractionMode, model: String, at: Nat}
  TaskDelegated{task: String, parent: String, child: String, depth: Nat, provider: String, model: String, at: Nat}
  TaskStateSet{task: String, state: TaskState, summary: String, at: Nat}
  TaskDelivered{task: String, at: Nat}
  SettingSet{key: String, value: String, at: Nat}
  TurnQueued{thread: String, msg: String, text: String, at: Nat}
  QueueTaken{thread: String, at: Nat}
  AskOpened{id: String, thread: String, kind: String, tool: String, detail: String, options: String, at: Nat}
  AskClosed{id: String, answer: String, at: Nat}
  # a phone asks for pushes (APNs/FCM); keyed by token, never sent to clients
  DeviceRegistered{token: String, platform: String, kind: String, thread: String, env: String, bundle: String, at: Nat}
  DeviceDropped{token: String, at: Nat}
  # snoozed until a time (0: awake again); see Snooze below
  ThreadSnoozed{id: String, until: Nat, at: Nat}
  # a manual place on the "pinned" or "active" shelf: an order key (Key
  # below, shown as hex); "" drops it
  ThreadOrdered{id: String, section: String, key: String, at: Nat}
  # a new thread's worktree could not be made (not a git repository, no
  # commit yet): it works in the project folder
  WorktreeSkipped{id: String, at: Nat}
  # the thread's branch was merged (a GitHub pull request, seen by gh): it
  # settles, and the merge is kept as the setting merged.<id>
  ThreadMerged{id: String, url: String, at: Nat}
  # the thread's worktree was removed (settled long enough, or merged); its
  # branch stays, and its last state is a checkpoint
  WorktreeRemoved{id: String, at: Nat}
  # a project's manual place in the sidebar: an order key (Key below, as
  # hex); "" drops it
  ProjectOrdered{id: String, key: String, at: Nat}
  # a queued message's text changed before it was sent
  QueueEdited{thread: String, msg: String, text: String, at: Nat}
  # a queued message dropped before it was sent
  QueueRemoved{thread: String, msg: String, at: Nat}
  # a queued message moved to place `to` (0 is next) in its thread's queue
  QueueMoved{thread: String, msg: String, to: Nat, at: Nat}
  # one queued message taken out to be sent now (a steer)
  QueueTakenOne{thread: String, msg: String, at: Nat}
  # a thread made as a fork of `source`: it starts with the source's
  # conversation as handed-over context, and in a worktree from the
  # source's last checkpoint
  ThreadForked{id: String, source: String, at: Nat}
  # the thread goes on with another provider (and model) from its next turn:
  # the provider left keeps its session and how much of the thread it saw;
  # the one taken up gets back its own session, if it had one
  ProviderSet{id: String, provider: String, model: String, at: Nat}
  # an agent's todo list for its thread, as JSON [{text, status}] (todo.bend)
  TodosSet{thread: String, todos: String, at: Nat}
  TodosChanged{thread: String, who: String, data: String, at: Nat}
  # how much of its model's context a thread's session holds (tokens), and\nthe model's window; 0 keeps what was known
  UsageSet{thread: String, used: Nat, window: Nat, at: Nat}
  # Bots (bot.bend, docs/bots.md). Secrets (webhooks, peers) never enter
  # the log. A BotSet with an empty thread keeps the one the bot has;
  # room members are one per line; BotWoke records the conversation depth
  # of the message that started a bot's turn
  BotSet{id: String, name: String, thread: String, look: Nat, persona: String, at: Nat}
  BotDeleted{id: String, at: Nat}
  RoomSet{id: String, name: String, members: String, at: Nat}
  RoomDeleted{id: String, at: Nat}
  RoomPosted{id: String, room: String, from: String, text: String, hop: Nat, at: Nat}
  HookSet{id: String, bot: String, name: String, revoked: Bool, at: Nat}
  HookHit{id: String, at: Nat}
  RoutineSet{id: String, bot: String, name: String, cron: String, prompt: String, on: Bool, at: Nat}
  RoutineDeleted{id: String, at: Nat}
  RoutineFired{id: String, at: Nat}
  MemorySaved{bot: String, key: String, kind: String, text: String, tags: String, at: Nat}
  MemoryForgot{bot: String, key: String, at: Nat}
  SpaceSet{bot: String, spec: String, at: Nat}
  PeerSet{id: String, name: String, url: String, revoked: Bool, at: Nat}
  BotWoke{bot: String, hop: Nat, at: Nat}
  # a bot's space as a web page ("" takes it away)
  SpacePageSet{bot: String, html: String, at: Nat}
  # the work a thread has out of hand (kind "bg": an agent's subagents,
  # background shells and monitors, Bg below; "watch": work Backplane
  # watches for it), its rows as text; folded as the setting
  # "<kind>.<thread>", the same state a SettingSet of it gives (old logs
  # say bg.<thread> that way). Unlike a setting, it goes to the owner's
  # other machines with its thread (far.bend's Fr.shared)
  WorkSet{thread: String, kind: String, rows: String, at: Nat}
  # the person made `bot` the uberbot (uber.bot), `uid` its id across
  # machines (uber.uid); bot "" means no uberbot (uber.bot)
  UberSet{bot: String, uid: String, at: Nat}
  # one op of the uberbot `uid`'s log, made by machine `origin` as its
  # n-th, stamped `stamp` (a hybrid logical clock, ubop.bend), `op` its
  # body as compact JSON; folded into settings under the uid (State.uberop)
  UberOp{uid: String, origin: String, n: Nat, stamp: String, op: String, at: Nat}
  # thread `id` was started by `by` (thread_launch: the thread or bot thread
  # that called it; a caller elsewhere as `<link>~<thread>`); `report`
  # whether its turn ends, asks and failures go back to `by` (Launch below).
  # Again later with the same `by`: the report choice changes (thread_update)
  ThreadLaunched{id: String, by: String, report: Bool, at: Nat}
  # Visible conversation hierarchy, independent of delegated work.
  ThreadParentSet{id: String, parent: String, at: Nat}
  WakeSet{id: String, by: String, kind: String, target: String, events: List<&2, String>, once: Bool, note: String, at: Nat}
  WakeDropped{id: String, at: Nat}
  LockChanged{name: String, lock: LK.NamedLock, at: Nat}
  # routines-cal (core/sched.bend). A routine's plan, logged right after a
  # RoutineSet: `once` the UTC instant of a one-time fire (0: repeats), `tz`
  # "" (the machine's offset, as before), "person" or an IANA zone, `kind`
  # "" (a prompt to the bot) or "remind" (an alert to the person). It
  # clears done and slot. RoutineSlot: the zone's wall clock at the last
  # fire of a zoned routine. RoutineDone: a one-time routine fired.
  # Reminded: a reminder was delivered into `thread` (an Act of tone
  # "remind"); it wakes no agent.
  RoutinePlan{id: String, once: Nat, tz: String, kind: String, at: Nat}
  RoutineSlot{id: String, slot: Nat, at: Nat}
  RoutineDone{id: String, at: Nat}
  Reminded{id: String, bot: String, thread: String, text: String, at: Nat}
  # a bot's browser task skill (task.bend), its JSON as text; TaskDeleted
  # takes it away. Folded into the bot world like memory
  TaskSet{bot: String, slug: String, json: String, at: Nat}
  TaskDeleted{bot: String, slug: String, at: Nat}
  MessageFrom{thread: String, msg: String, from: String, kind: String, ref: String, at: Nat}
  ConvSet{id: String, subject: String, kind: String, parts: String, project: String, tags: String, at: Nat}
  MailPosted{id: String, conv: String, from: String, to: String, kind: String, ref: String, want: String, head: String, size: Nat, at: Nat}
  MailMarked{bot: String, conv: String, status: String, seen: Nat, until: Nat, at: Nat}
  MailTagged{bot: String, conv: String, tags: String, at: Nat}
  RunSet{thread: String, bot: String, conv: String, mail: String, at: Nat}
  CharterSet{bot: String, text: String, by: String, at: Nat}
  DecisionLogged{id: String, bot: String, conv: String, run: String, text: String, reason: String, at: Nat}
  # a turn's usage as its provider reported it (Tokens, below)
  TurnUsage{thread: String, provider: String, model: String, key: String, input: Nat, cread: Nat, cwrite: Nat, output: Nat, reasoning: Nat, cost: Nat, known: String, basis: String, at: Nat}
  # deploy_main (core/ship.bend): `by` (a thread, or "-" for the person)
  # asked for origin/main at `commit`; `cover` what already held it
  # ("building", "staged", "live") or "" (a build was wanted). DeployBuilt:
  # a build's step, `state` building, pid (detail "<pid> <ident>"), staged,
  # failed, skipped or live. Both fold into the setting deploy.ship
  DeployAsked{by: String, note: String, commit: String, cover: String, at: Nat}
  DeployBuilt{commit: String, state: String, version: String, detail: String, at: Nat}
  # the hub saw real work start on a thread that was settled (or that the
  # person brought back): it is back on the active shelf with no override,
  # so it settles once that work is done (at once with settle.done on, else
  # by the person or the inactivity window; Lifecycle.*, laws life_*)
  ThreadWoke{id: String, at: Nat}

# the later of two times. Base's Nat.max recurses once per unit and
# overflows at epoch scale; Nat.is_ge compiles to a machine comparison.
def Time.max(+a: Nat, +b: Nat) -> Nat:
  Bool.pick(Nat, Nat.is_ge(a, b), a, b)

def Time.ago.k(m: Bool, h: Bool, d: Bool, +s: Nat) -> String:
  match m h d:
    case True{} _ _:
      "now"
    case False{} True{} _:
      Nat.show(Nat.div(s, 60n)) ++ "m"
    case False{} False{} True{}:
      Nat.show(Nat.div(s, 3600n)) ++ "h"
    case False{} False{} False{}:
      Nat.show(Nat.div(s, 86400n)) ++ "d"

# "now", "5m", "3h", "2d"
def Time.ago(+now: Nat, +at: Nat) -> String:
  +s = Nat.sub(now, at)
  Time.ago.k(Nat.is_lt(s, 60n), Nat.is_lt(s, 3600n), Nat.is_lt(s, 86400n), s)

def Time.took.k(m: Bool, h: Bool, +s: Nat) -> String:
  match m h:
    case True{} _:
      Nat.show(s) ++ "s"
    case False{} True{}:
      Nat.show(Nat.div(s, 60n)) ++ "m " ++ Nat.show(Nat.mod(s, 60n)) ++ "s"
    case False{} False{}:
      Nat.show(Nat.div(s, 3600n)) ++ "h " ++ Nat.show(Nat.div(Nat.mod(s, 3600n), 60n)) ++ "m"

# how long something took: "45s", "2m 13s", "1h 5m"
def Time.took(+s: Nat) -> String:
  Time.took.k(Nat.is_lt(s, 60n), Nat.is_lt(s, 3600n), s)

# Settle rule
# -----------

# three days, the default of `sidebarAutoSettleAfterDays`
def Settle.window() -> Nat:
  259200n

# a thread is busy while a turn runs or something waits on the user
def Thread.busy(t: TurnState, +pending: Nat) -> Bool:
  match t:
    case Running{}:
      True{}
    case _:
      Nat.is_gt(pending, 0n)

# whether inactivity alone settles a thread whose last activity was at `last`
def Settle.stale(+now: Nat, +last: Nat, +window: Nat) -> Bool:
  Nat.is_ge(now, Nat.add(last, window))

def Settle.auto.k(busy: Bool, archived: Bool, o: Override, stale: Bool) -> Bool:
  match busy archived o:
    case False{} False{} NoOverride{}:
      stale
    case _ _ _:
      False{}

# whether the server's settle tick settles this thread now
def Settle.auto(+now: Nat, +window: Nat, t: Thread) -> Bool:
  Thread{_, _, _, _, +activity, turn, +pending, archived, override, _, _, _, _, _, _, _, _, _, _, _, _} = t
  Settle.auto.k(Thread.busy(turn, pending), archived, override, Settle.stale(now, activity, window))

# Snooze
# ------
# A thread snoozed until a time sits on its own shelf until then and never
# auto-settles meanwhile (law settle_never_snoozed). It wakes by the clock
# alone: no event is needed (law snooze_wakes), and its settle window
# starts over from the wake time.

def Snooze.on(+now: Nat, +until: Nat) -> Bool:
  Bool.not(Nat.is_ge(now, until))

def Settle.auto.s(snoozed: Bool, +now: Nat, +window: Nat, t: Thread, +until: Nat) -> Bool:
  match snoozed:
    case True{}:
      False{}
    case False{}:
      Bool.and(Settle.stale(now, until, window), Settle.auto(now, window, t))

# the settle tick's rule with the thread's snooze (until 0: never snoozed)
def Settle.auto.at(+now: Nat, +window: Nat, t: Thread, +until: Nat) -> Bool:
  Settle.auto.s(Snooze.on(now, until), now, window, t, until)

def Snooze.hour() -> Nat:
  3600n

def Snooze.week() -> Nat:
  604800n

# "+0200" / "-0530" (date +%z) as whether local time is east of UTC and
# by how many seconds
def Snooze.tz.secs(+s: String) -> Nat:
  +n = Maybe.default(&2, Nat, Nat.read(String.drop(s, 1n)), 0n)
  Nat.add(Nat.mul(Nat.div(n, 100n), 3600n), Nat.mul(Nat.mod(n, 100n), 60n))

def Snooze.tz.east(+s: String) -> Bool:
  Bool.not(String.starts_with(s, "-"))

def Snooze.local(east: Bool, +now: Nat, +off: Nat) -> Nat:
  match east:
    case True{}:
      Nat.add(now, off)
    case False{}:
      Nat.sub(now, off)

def Snooze.utc(east: Bool, +t: Nat, +off: Nat) -> Nat:
  match east:
    case True{}:
      Nat.sub(t, off)
    case False{}:
      Nat.add(t, off)

# 9:00 local time tomorrow, as epoch seconds
def Snooze.tomorrow(+now: Nat, +tz: String) -> Nat:
  +off = Snooze.tz.secs(tz)
  +day = Nat.div(Snooze.local(Snooze.tz.east(tz), now, off), 86400n)
  Snooze.utc(Snooze.tz.east(tz), Nat.add(Nat.mul(1n+day, 86400n), 32400n), off)

# the shelf a thread sits on right now
type Shelf is Data:
  Pinned{}
  Active{}
  Settled{}
  Hidden{}
  Snoozed{}

def Shelf.of.k(deleted: Bool, archived: Bool, pinned: Bool, o: Override, settled: Bool) -> Shelf:
  match deleted archived pinned o settled:
    case True{} _ _ _ _:
      Hidden{}
    case False{} True{} _ _ _:
      Hidden{}
    case False{} False{} _ ForcedSettled{} _:
      Settled{}
    case False{} False{} True{} _ _:
      Pinned{}
    case False{} False{} False{} ForcedActive{} _:
      Active{}
    case False{} False{} False{} NoOverride{} True{}:
      Settled{}
    case False{} False{} False{} NoOverride{} False{}:
      Active{}

# settled_at > 0 marks an auto-settle recorded by the tick
def Shelf.of(t: Thread) -> Shelf:
  Thread{_, _, _, _, _, _, _, archived, override, +settled_at, +unsettled_at, pinned, _, _, _, _, _, _, _, _, deleted} = t
  Shelf.of.k(deleted, archived, pinned, override, Nat.is_gt(settled_at, unsettled_at))

def Thread.blank(id: String) -> Thread:
  Thread{id, "", "", 0n, 0n, Idle{}, 0n, True{}, NoOverride{}, 0n, 0n, False{}, Local{}, "", "", "", "", FullAccess{}, DefaultMode{}, "", True{}}

def Project.blank(id: String) -> Project:
  Project{id, "", "", 0n, True{}}

def Thread.renamed(+title: String, t: Thread) -> Thread:
  Thread{id, p, _, c, a, tu, pe, ar, o, sa, ua, pi, en, br, wt, pr, mo, rm, im, se, de} = t
  Thread{id, p, title, c, a, tu, pe, ar, o, sa, ua, pi, en, br, wt, pr, mo, rm, im, se, de}

# a user message is activity, and it wakes an auto-settled thread
def Thread.touched(+at: Nat, t: Thread) -> Thread:
  Thread{id, p, ti, c, _, tu, pe, ar, o, sa, _, pi, en, br, wt, pr, mo, rm, im, se, de} = t
  Thread{id, p, ti, c, at, tu, pe, ar, o, sa, at, pi, en, br, wt, pr, mo, rm, im, se, de}

def Thread.turned(st: TurnState, +at: Nat, t: Thread) -> Thread:
  Thread{id, p, ti, c, a, _, pe, ar, o, sa, ua, pi, en, br, wt, pr, mo, rm, im, se, de} = t
  Thread{id, p, ti, c, Time.max(a, at), st, pe, ar, o, sa, ua, pi, en, br, wt, pr, mo, rm, im, se, de}

def Thread.pending_set(n: Nat, t: Thread) -> Thread:
  Thread{id, p, ti, c, a, tu, _, ar, o, sa, ua, pi, en, br, wt, pr, mo, rm, im, se, de} = t
  Thread{id, p, ti, c, a, tu, n, ar, o, sa, ua, pi, en, br, wt, pr, mo, rm, im, se, de}

def Thread.settle.o(manual: Bool, o: Override) -> Override:
  match manual:
    case True{}:
      ForcedSettled{}
    case False{}:
      o

# settling clears the pin
def Thread.settled(manual: Bool, +at: Nat, t: Thread) -> Thread:
  Thread{id, p, ti, c, a, tu, pe, ar, o, _, ua, _, en, br, wt, pr, mo, rm, im, se, de} = t
  Thread{id, p, ti, c, a, tu, pe, ar, Thread.settle.o(manual, o), at, ua, False{}, en, br, wt, pr, mo, rm, im, se, de}

# unsettling puts the thread back on top of the active shelf
def Thread.unsettled(+at: Nat, t: Thread) -> Thread:
  Thread{id, p, ti, c, a, tu, pe, ar, _, +sa, _, pi, en, br, wt, pr, mo, rm, im, se, de} = t
  Thread{id, p, ti, c, Time.max(a, at), tu, pe, ar, ForcedActive{}, sa, Time.max(sa, at), pi, en, br, wt, pr, mo, rm, im, se, de}

# when a woken thread was brought back: unchanged when it already was
# (so a thread on the active shelf keeps its place), else now, so it comes
# back on top of the active shelf like any unsettle
def Thread.woke.ua(+sa: Nat, +ua: Nat, +at: Nat) -> Nat:
  Bool.pick(Nat, Nat.is_ge(ua, sa), ua, Time.max(sa, at))

# woken by work: back on the active shelf, its override cleared (a manual
# settle or bring-back held until real work started again). Its activity
# is left alone: the work that woke it is what moves it (law
# life_woke_clears_override)
def Thread.woke(+at: Nat, t: Thread) -> Thread:
  Thread{id, p, ti, c, a, tu, pe, ar, _, +sa, +ua, pi, en, br, wt, pr, mo, rm, im, se, de} = t
  Thread{id, p, ti, c, a, tu, pe, ar, NoOverride{}, sa, Thread.woke.ua(sa, ua, at), pi, en, br, wt, pr, mo, rm, im, se, de}

def Thread.archived_set(b: Bool, t: Thread) -> Thread:
  Thread{id, p, ti, c, a, tu, pe, _, o, sa, ua, pi, en, br, wt, pr, mo, rm, im, se, de} = t
  Thread{id, p, ti, c, a, tu, pe, b, o, sa, ua, pi, en, br, wt, pr, mo, rm, im, se, de}

def Thread.pinned_set(b: Bool, t: Thread) -> Thread:
  Thread{id, p, ti, c, a, tu, pe, ar, o, sa, ua, _, en, br, wt, pr, mo, rm, im, se, de} = t
  Thread{id, p, ti, c, a, tu, pe, ar, o, sa, ua, b, en, br, wt, pr, mo, rm, im, se, de}

def Thread.deleted_set(t: Thread) -> Thread:
  Thread{id, p, ti, c, a, tu, pe, ar, o, sa, ua, pi, en, br, wt, pr, mo, rm, im, se, _} = t
  Thread{id, p, ti, c, a, tu, pe, ar, o, sa, ua, pi, en, br, wt, pr, mo, rm, im, se, True{}}

def Thread.worktree_set(b: String, w: String, t: Thread) -> Thread:
  Thread{id, p, ti, c, a, tu, pe, ar, o, sa, ua, pi, en, _, _, pr, mo, rm, im, se, de} = t
  Thread{id, p, ti, c, a, tu, pe, ar, o, sa, ua, pi, Worktree{}, b, w, pr, mo, rm, im, se, de}

def Thread.local(t: Thread) -> Thread:
  Thread{id, p, ti, c, a, tu, pe, ar, o, sa, ua, pi, _, _, _, pr, mo, rm, im, se, de} = t
  Thread{id, p, ti, c, a, tu, pe, ar, o, sa, ua, pi, Local{}, "", "", pr, mo, rm, im, se, de}

def Thread.unrooted(t: Thread) -> Thread:
  Thread{id, p, ti, c, a, tu, pe, ar, o, sa, ua, pi, en, br, _, pr, mo, rm, im, se, de} = t
  Thread{id, p, ti, c, a, tu, pe, ar, o, sa, ua, pi, en, br, "", pr, mo, rm, im, se, de}

def Thread.session_set(s: String, t: Thread) -> Thread:
  Thread{id, p, ti, c, a, tu, pe, ar, o, sa, ua, pi, en, br, wt, pr, mo, rm, im, _, de} = t
  Thread{id, p, ti, c, a, tu, pe, ar, o, sa, ua, pi, en, br, wt, pr, mo, rm, im, s, de}

def Thread.modes_set(r: RuntimeMode, i: InteractionMode, m: String, t: Thread) -> Thread:
  Thread{id, p, ti, c, a, tu, pe, ar, o, sa, ua, pi, en, br, wt, pr, _, _, _, se, de} = t
  Thread{id, p, ti, c, a, tu, pe, ar, o, sa, ua, pi, en, br, wt, pr, m, r, i, se, de}

# Asks
# ----
# What an agent waits on the user for: a tool approval ("approval"), a
# question ("input", options one per line) or a proposed plan ("plan").
# Opened by the agent, closed once by the user's answer.

# a phone's push registration. kind is "alert" (notifications), "activity"
# (a Live Activity's update token; thread "" is the all-agents island) or
# "start" (push-to-start). The token is a secret: it stays on the server.
type Device is Data:
  Device{token: String, platform: String, kind: String, thread: String, env: String, bundle: String, at: Nat}

type Ask is Data:
  Ask{id: String, thread: String, kind: String, tool: String, detail: String, options: String, open: Bool, answer: String}

# Sidebar extensions
# ------------------
# What the sidebar knows of a thread beyond the Thread itself: when a snooze
# ends (0: not snoozed) and its manual order keys on the pinned and active
# shelves ("": none).

type Nav is Data:
  Nav{until: Nat, pin: String, act: String}

def Nav.blank() -> Nav:
  Nav{0n, "", ""}

def Nav.get.k(r: Map<&2, Nav> & Nav) -> Nav:
  (_, v) = r
  v

def Nav.get(+m: Map<&2, Nav>, +id: String) -> Nav:
  Nav.get.k(Map.get(Nav, Nav.blank(), m, id))

def Nav.until(n: Nav) -> Nat:
  match n:
    case Nav{u, _, _}:
      u

def Nav.snoozed(n: Nav, +until: Nat) -> Nav:
  match n:
    case Nav{_, p, a}:
      Nav{until, p, a}

# the key of a section ("pinned" or "active")
def Nav.key(n: Nav, +section: String) -> String:
  match n:
    case Nav{_, p, a}:
      Bool.pick(String, String.eq(section, "pinned"), p, a)

def Nav.keyed(n: Nav, +section: String, +key: String) -> Nav:
  match n:
    case Nav{+u, +p, +a}:
      Bool.pick(Nav, String.eq(section, "pinned"), Nav{u, key, a}, Nav{u, p, key})

# A project's order key lives in the same map under "project:<id>" (its
# "active" key), a name no thread id takes
def Nav.project(+id: String) -> String:
  "project:" ++ id

# a project's order key ("": none)
def Nav.project_key(+m: Map<&2, Nav>, +id: String) -> String:
  Nav.key(Nav.get(m, Nav.project(id)), "active")

# Worktrees
# ---------
# A new thread gets its own worktree of the project's repository (made by
# the server under <repo>/.backplane/worktrees). Until it exists the
# thread's messages wait in its queue, for at most Worktree.wait() seconds
# after the thread was made; past that (or once skipped: no repository) the
# thread works in the project folder (laws worktree_pending_*).
#
# A worktree is removed once its thread is idle and put away: settled or
# hidden for Worktree days (the "worktree.days" setting), or settled by a
# merge, at once. Only worktrees Backplane made (under .backplane/worktrees)
# are ever removed, and the branch stays (laws worktree_kept_*,
# worktree_dropped_*). A message to a thread whose worktree is gone makes
# it again from the branch, and waits for it like a new thread's first
# message (law worktree_revives).

def Worktree.wait() -> Nat:
  120n

def Worktree.pending.k(env: EnvMode, +wt: String, +since: Nat, +now: Nat) -> Bool:
  match env:
    case Worktree{}:
      Bool.and(Nat.is_lt(now, Nat.add(since, Worktree.wait())), String.is_empty(wt))
    case Local{}:
      False{}

# when the wait started: a new thread's creation, or, for a thread whose
# worktree is being made again, the message that asked for it
def Worktree.since(+branch: String, +created: Nat, +activity: Nat) -> Nat:
  Bool.pick(Nat, String.is_empty(branch), created, activity)

# whether a thread still waits for its worktree
def Worktree.pending(t: Thread, +now: Nat) -> Bool:
  Thread{_, _, _, +c, +a, _, _, _, _, _, _, _, en, +br, +wt, _, _, _, _, _, _} = t
  Worktree.pending.k(en, wt, Worktree.since(br, c, a), now)

def Worktree.gone.k(env: EnvMode, +wt: String, +branch: String) -> Bool:
  match env:
    case Worktree{}:
      Bool.and(String.is_empty(wt), Bool.not(String.is_empty(branch)))
    case Local{}:
      False{}

def Worktree.unmade.k(env: EnvMode, +wt: String) -> Bool:
  match env:
    case Worktree{}:
      String.is_empty(wt)
    case Local{}:
      False{}

# a worktree thread with no worktree yet (being made, or made again)
def Worktree.unmade(t: Thread) -> Bool:
  Thread{_, _, _, _, _, _, _, _, _, _, _, _, en, _, +wt, _, _, _, _, _, _} = t
  Worktree.unmade.k(en, wt)

# a worktree thread whose worktree was removed (its branch is kept)
def Worktree.gone(t: Thread) -> Bool:
  Thread{_, _, _, _, _, _, _, _, _, _, _, _, en, +br, +wt, _, _, _, _, _, _} = t
  Worktree.gone.k(en, wt, br)

# a directory Backplane made for a thread, the only kind it removes
def Worktree.ours(+wt: String) -> Bool:
  String.contains(wt, "/.backplane/worktrees/")

# the shelves a thread is put away on
def Worktree.parked(sh: Shelf) -> Bool:
  match sh:
    case Settled{}:
      True{}
    case Hidden{}:
      True{}
    case _:
      False{}

def Worktree.drop.s(sh: Shelf, +ours: Bool, merged: Bool, aged: Bool) -> Bool:
  match sh:
    case Settled{}:
      Bool.and(ours, Bool.or(merged, aged))
    case Hidden{}:
      Bool.and(ours, aged)
    case _:
      False{}

# whether a worktree goes now: never while its thread is busy
def Worktree.drop.k(busy: Bool, sh: Shelf, +ours: Bool, merged: Bool, aged: Bool) -> Bool:
  match busy:
    case True{}:
      False{}
    case False{}:
      Worktree.drop.s(sh, ours, merged, aged)

# Merges
# ------
# A thread whose branch is merged settles by itself, once: the merge is
# recorded (merged.<id>), and a recorded merge is never due again, so a
# thread the user brings back stays back (laws merge_*).

def Merge.key(+id: String) -> String:
  "merged." ++ id

def Merge.due.k(sh: Shelf, +branch: String, +merged: String) -> Bool:
  match sh:
    case Pinned{}:
      Bool.and(Bool.not(String.is_empty(branch)), String.is_empty(merged))
    case Active{}:
      Bool.and(Bool.not(String.is_empty(branch)), String.is_empty(merged))
    case _:
      False{}

# Links between threads
# ---------------------
# A message one thread sends another carries the sender in its id:
# "t:<from>:<n>". Thread ids hold no ':'.

def Link.id(+from: String, +n: String) -> String:
  SCon{Chr{116}, SCon{Chr{58}, String.append(from, SCon{Chr{58}, n})}}

# Link.from is read for every entry of every frame, so it looks at an id
# once, from the front, and gives up at the first character of any id
# that is not a link (laws link_from_id, link_from_plain). It answers what
# the ':'-fields do: the second when the first is "t" and a third follows.

def Link.colon() -> Char:
  Chr{58}

def Link.field.k(colon: Bool, +c: Char, rest: String) -> String:
  match colon:
    case True{}:
      SNil{}
    case False{}:
      SCon{c, rest}

# the text before the first ':'
def Link.field(s: String) -> String:
  match s:
    case SNil{}:
      SNil{}
    case SCon{+c, t}:
      Link.field.k(Char.is_eq(c, Link.colon()), c, Link.field(t))

def Link.more.k(colon: Bool, rest: Bool) -> Bool:
  match colon:
    case True{}:
      True{}
    case False{}:
      rest

# whether a ':' is in s
def Link.more(s: String) -> Bool:
  match s:
    case SNil{}:
      False{}
    case SCon{+c, t}:
      Link.more.k(Char.is_eq(c, Link.colon()), Link.more(t))

def Link.from.e(third: Bool, +r: String) -> String:
  match third:
    case True{}:
      Link.field(r)
    case False{}:
      ""

def Link.from.d(colon: Bool, +r: String) -> String:
  match colon:
    case True{}:
      Link.from.e(Link.more(r), r)
    case False{}:
      ""

def Link.from.c(s: String) -> String:
  match s:
    case SNil{}:
      ""
    case SCon{+c, +r}:
      Link.from.d(Char.is_eq(c, Link.colon()), r)

def Link.from.b(tee: Bool, s: String) -> String:
  match tee:
    case True{}:
      Link.from.c(s)
    case False{}:
      ""

# the thread a message came from ("" for the user's own)
def Link.from(id: String) -> String:
  match id:
    case SNil{}:
      ""
    case SCon{+c, t}:
      Link.from.b(Char.is_eq(c, Chr{116}), t)

# a thread id as a link carries it (with no ':')
def Link.bare.k(colon: Bool, +c: Char, rest: String) -> String:
  match colon:
    case True{}:
      rest
    case False{}:
      SCon{c, rest}

def Link.bare(s: String) -> String:
  match s:
    case SNil{}:
      SNil{}
    case SCon{+c, t}:
      Link.bare.k(Char.is_eq(c, Link.colon()), c, Link.bare(t))

# Entries by thread
# -----------------
# The log keeps every thread's entries in one list, newest first. A view
# of one thread needs only its newest page, and the sidebar only the
# messages between threads, but walking the whole log for them each frame
# grew with every agent at work. So each entry is also filed as it is
# posted: under its thread, under the thread that sent it (a message
# between threads), and a message between threads under Idx.links(); each
# list newest first, like the log (laws idx_*).

# the key messages between threads are filed under (no thread id has '@')
def Idx.links() -> String:
  "@links"

def Idx.get.k(r: Map<&2, List<&2, Entry>> & List<&2, Entry>) -> List<&2, Entry>:
  (_, v) = r
  v

# what is filed under k, newest first
def Idx.get(ix: Map<&2, List<&2, Entry>>, +k: String) -> List<&2, Entry>:
  Idx.get.k(Map.get(List<&2, Entry>, Nil{}, ix, k))

def Idx.put.k(r: Map<&2, List<&2, Entry>> & List<&2, Entry>, +k: String, e: Entry) -> Map<&2, List<&2, Entry>>:
  (m, cur) = r
  Map.set(&2, List<&2, Entry>, m, k, e <> cur)

def Idx.put(ix: Map<&2, List<&2, Entry>>, +k: String, e: Entry) -> Map<&2, List<&2, Entry>>:
  Idx.put.k(Map.get(List<&2, Entry>, Nil{}, ix, k), k, e)

def Idx.thread(e: Entry) -> String:
  match e:
    case Msg{_, t, _, _, _}:
      t
    case Act{_, t, _, _, _, _}:
      t

def Idx.id(e: Entry) -> String:
  match e:
    case Msg{i, _, _, _, _}:
      i
    case Act{i, _, _, _, _, _}:
      i

def Idx.is_msg(e: Entry) -> Bool:
  match e:
    case Msg{_, _, _, _, _}:
      True{}
    case Act{_, _, _, _, _, _}:
      False{}

def Idx.link(msg: Bool, ix: Map<&2, List<&2, Entry>>, +e: Entry) -> Map<&2, List<&2, Entry>>:
  match msg:
    case True{}:
      Idx.put(ix, Idx.links(), e)
    case False{}:
      ix

def Idx.sent(sent: Bool, ix: Map<&2, List<&2, Entry>>, +from: String, +e: Entry) -> Map<&2, List<&2, Entry>>:
  match sent:
    case True{}:
      Idx.link(Idx.is_msg(e), Idx.put(ix, from, e), e)
    case False{}:
      ix

# an entry filed under its thread, and one another thread sent under the
# sender too (and a message under the links)
def Idx.post.k(ix: Map<&2, List<&2, Entry>>, +th: String, +from: String, +e: Entry) -> Map<&2, List<&2, Entry>>:
  Idx.sent(Bool.and(Bool.not(String.is_empty(from)), Bool.not(String.eq(from, th))), Idx.put(ix, th, e), from, e)

# a design step (an Act of tone "step", core/hist.bend) is filed under
# "@steps:<thread>" as well, so the viewer's history reads a thread's
# steps without walking its entries
def Idx.steps.key(+th: String) -> String:
  "@steps:" ++ th

def Idx.is_step(e: Entry) -> Bool:
  match e:
    case Msg{_, _, _, _, _}:
      False{}
    case Act{_, _, +tone, _, _, _}:
      String.eq(tone, "step")

def Idx.step(step: Bool, ix: Map<&2, List<&2, Entry>>, +e: Entry) -> Map<&2, List<&2, Entry>>:
  match step:
    case True{}:
      Idx.put(ix, Idx.steps.key(Idx.thread(e)), e)
    case False{}:
      ix

# a design review comment op (an Act of tone "review", core/review.bend) is
# filed under "@review:<thread>" as well
def Idx.review.key(+th: String) -> String:
  "@review:" ++ th

def Idx.is_review(e: Entry) -> Bool:
  match e:
    case Msg{_, _, _, _, _}:
      False{}
    case Act{_, _, +tone, _, _, _}:
      String.eq(tone, "review")

def Idx.review(rv: Bool, ix: Map<&2, List<&2, Entry>>, +e: Entry) -> Map<&2, List<&2, Entry>>:
  match rv:
    case True{}:
      Idx.put(ix, Idx.review.key(Idx.thread(e)), e)
    case False{}:
      ix

# a terminal (an Act of tone "term", core/emb.bend) is filed under
# "@terms:<thread>" as well, so its pinned ones are found without walking
# the thread (Emb.pins walked every entry each layout: 8 ms in a long one)
def Idx.terms.key(+th: String) -> String:
  "@terms:" ++ th

def Idx.is_term(e: Entry) -> Bool:
  match e:
    case Msg{_, _, _, _, _}:
      False{}
    case Act{_, _, +tone, _, _, _}:
      String.eq(tone, "term")

def Idx.term(term: Bool, ix: Map<&2, List<&2, Entry>>, +e: Entry) -> Map<&2, List<&2, Entry>>:
  match term:
    case True{}:
      Idx.put(ix, Idx.terms.key(Idx.thread(e)), e)
    case False{}:
      ix

def Idx.post(ix: Map<&2, List<&2, Entry>>, +e: Entry) -> Map<&2, List<&2, Entry>>:
  Idx.term(Idx.is_term(e), Idx.review(Idx.is_review(e), Idx.step(Idx.is_step(e), Idx.post.k(ix, Idx.thread(e), Link.from(Idx.id(e)), e), e), e), e)

# Projection
# ----------

# a message waiting in a thread's queue: its id and text
type QItem is Data:
  QItem{msg: String, text: String}

# what the read model keeps behind its last field: the bots' world, and
# the entries filed by thread (Idx)
type Ext is Data:
  Ext{world: BT.World, by: Map<&2, List<&2, Entry>>, mail: List<&2, ML.Box>}

def Ext.of(bw: List<&2, Ext>) -> Ext:
  Maybe.default(&2, Ext, List.head(&2, Ext, bw), Ext{BT.World.empty(), Map.new(&2, List<&2, Entry>), Nil{}})

def Ext.world(x: Ext) -> BT.World:
  match x:
    case Ext{w, _, _}:
      w

def Ext.by(x: Ext) -> Map<&2, List<&2, Entry>>:
  match x:
    case Ext{_, by, _}:
      by

def Ext.with_world(f: BT.World -> BT.World, x: Ext) -> Ext:
  match x:
    case Ext{w, by, mail}:
      Ext{f(w), by, mail}

def Ext.post(x: Ext, +e: Entry) -> Ext:
  match x:
    case Ext{w, by, mail}:
      Ext{w, Idx.post(by, e), mail}

def Ext.mail(x: Ext) -> List<&2, ML.Box>:
  match x:
    case Ext{_, _, mail}: mail

def Ext.mail_box(x: Ext) -> ML.Box:
  Maybe.default(&2, ML.Box, List.head(&2, ML.Box, Ext.mail(x)), ML.Box.empty())

def Ext.with_mail(f: ML.Box -> ML.Box, x: Ext) -> Ext:
  match x:
    case Ext{w, by, mail}: Ext{w, by, [f(Maybe.default(&2, ML.Box, List.head(&2, ML.Box, mail), ML.Box.empty()))]}

type State is Data:
  State{
    projects: Map<&2, Project>,
    threads: Map<&2, Thread>,
    entries: List<&2, Entry>,
    seq: Nat,
    tasks: Map<&2, Task>,
    settings: Map<&2, String>,
    queued: Map<&2, List<&2, QItem>>,
    asks: Map<&2, Ask>,
    devices: Map<&2, Device>,
    nav: Map<&2, Nav>,
    # behind a List: the native window holds the State in its own, and a
    # record flattened into it passes the C emitter's 255 arguments (one
    # more field of any kind does too): the bots' world and each thread's
    # entries filed by thread (Ext, Idx.post)
    bots: List<&2, Ext>
  }

def State.empty() -> State:
  State{Map.new(&2, Project), Map.new(&2, Thread), Nil{}, 0n, Map.new(&2, Task), Map.new(&2, String), Map.new(&2, List<&2, QItem>), Map.new(&2, Ask), Map.new(&2, Device), Map.new(&2, Nav), [Ext{BT.World.empty(), Map.new(&2, List<&2, Entry>), Nil{}}]}

# apply f to the thread at `id` (a blank, hidden thread when absent)
def State.thread.fin(f: Thread -> Thread, ps: Map<&2, Project>, es: List<&2, Entry>, n: Nat, tk: Map<&2, Task>, sg: Map<&2, String>, qu: Map<&2, List<&2, QItem>>, ak: Map<&2, Ask>, dv: Map<&2, Device>, nv: Map<&2, Nav>, bw: List<&2, Ext>, id: String, r: Map<&2, Thread> & Thread) -> State:
  (ts, t) = r
  State{ps, Map.set(&2, Thread, ts, id, f(t)), es, n, tk, sg, qu, ak, dv, nv, bw}

def State.thread(f: Thread -> Thread, s: State, +id: String) -> State:
  State{ps, ts, es, n, tk, sg, qu, ak, dv, nv, bw} = s
  State.thread.fin(f, ps, es, n, tk, sg, qu, ak, dv, nv, bw, id, Map.get(Thread, Thread.blank(id), ts, id))

def State.put_thread(s: State, t: Thread) -> State:
  State{ps, ts, es, n, tk, sg, qu, ak, dv, nv, bw} = s
  Thread{+id, p, ti, c, a, tu, pe, ar, o, sa, ua, pi, en, br, wt, pr, mo, rm, im, se, de} = t
  State{ps, Map.set(&2, Thread, ts, id, Thread{id, p, ti, c, a, tu, pe, ar, o, sa, ua, pi, en, br, wt, pr, mo, rm, im, se, de}), es, n, tk, sg, qu, ak, dv, nv, bw}

def State.put_project(s: State, p: Project) -> State:
  State{ps, ts, es, n, tk, sg, qu, ak, dv, nv, bw} = s
  Project{+id, ti, r, c, rm} = p
  State{Map.set(&2, Project, ps, id, Project{id, ti, r, c, rm}), ts, es, n, tk, sg, qu, ak, dv, nv, bw}

def State.remove_project.fin(ts: Map<&2, Thread>, es: List<&2, Entry>, n: Nat, tk: Map<&2, Task>, sg: Map<&2, String>, qu: Map<&2, List<&2, QItem>>, ak: Map<&2, Ask>, dv: Map<&2, Device>, nv: Map<&2, Nav>, bw: List<&2, Ext>, id: String, r: Map<&2, Project> & Project) -> State:
  (ps, p) = r
  Project{i, ti, rt, c, _} = p
  State{Map.set(&2, Project, ps, id, Project{i, ti, rt, c, True{}}), ts, es, n, tk, sg, qu, ak, dv, nv, bw}

def State.remove_project(s: State, +id: String) -> State:
  State{ps, ts, es, n, tk, sg, qu, ak, dv, nv, bw} = s
  State.remove_project.fin(ts, es, n, tk, sg, qu, ak, dv, nv, bw, id, Map.get(Project, Project.blank(id), ps, id))

def State.post(s: State, +e: Entry) -> State:
  State{ps, ts, es, n, tk, sg, qu, ak, dv, nv, bw} = s
  State{ps, ts, e <> es, n, tk, sg, qu, ak, dv, nv, [Ext.post(Ext.of(bw), e)]}

# a thread's entries and the messages it sent, newest first: what walking
# the whole log for that thread finds, without the walk (Idx)
def State.mine(s: State, +th: String) -> List<&2, Entry>:
  match s:
    case State{_, _, _, _, _, _, _, _, _, _, bw}:
      Idx.get(Ext.by(Ext.of(bw)), th)

# a thread's design steps (Acts of tone "step"), newest first
def State.steps(s: State, +th: String) -> List<&2, Entry>:
  match s:
    case State{_, _, _, _, _, _, _, _, _, _, bw}:
      Idx.get(Ext.by(Ext.of(bw)), Idx.steps.key(th))

# a thread's terminals (Acts of tone "term"), newest first
def State.terms(s: State, +th: String) -> List<&2, Entry>:
  match s:
    case State{_, _, _, _, _, _, _, _, _, _, bw}:
      Idx.get(Ext.by(Ext.of(bw)), Idx.terms.key(th))

# a thread's design review ops (Acts of tone "review"), newest first
def State.reviews(s: State, +th: String) -> List<&2, Entry>:
  match s:
    case State{_, _, _, _, _, _, _, _, _, _, bw}:
      Idx.get(Ext.by(Ext.of(bw)), Idx.review.key(th))

# every message between threads, newest first
def State.links(s: State) -> List<&2, Entry>:
  match s:
    case State{_, _, _, _, _, _, _, _, _, _, bw}:
      Idx.get(Ext.by(Ext.of(bw)), Idx.links())

def State.tick(s: State) -> State:
  State{ps, ts, es, n, tk, sg, qu, ak, dv, nv, bw} = s
  State{ps, ts, es, 1n+n, tk, sg, qu, ak, dv, nv, bw}

def State.message(s: State, +thread: String, msg: String, role: Role, text: String, +at: Nat) -> State:
  match role:
    case User{}:
      State.post(State.thread((t => Thread.touched(at, t)), s, thread), Msg{msg, thread, User{}, text, at})
    case _:
      State.post(s, Msg{msg, thread, role, text, at})

def State.task.fin(f: Task -> Task, ps: Map<&2, Project>, ts: Map<&2, Thread>, es: List<&2, Entry>, n: Nat, sg: Map<&2, String>, qu: Map<&2, List<&2, QItem>>, ak: Map<&2, Ask>, dv: Map<&2, Device>, nv: Map<&2, Nav>, bw: List<&2, Ext>, id: String, r: Map<&2, Task> & Task) -> State:
  (tk, t) = r
  State{ps, ts, es, n, Map.set(&2, Task, tk, id, f(t)), sg, qu, ak, dv, nv, bw}

# apply f to the task at `id` (a blank, settled task when absent)
def State.task(f: Task -> Task, s: State, +id: String) -> State:
  State{ps, ts, es, n, tk, sg, qu, ak, dv, nv, bw} = s
  State.task.fin(f, ps, ts, es, n, sg, qu, ak, dv, nv, bw, id, Map.get(Task, Task.blank(id), tk, id))

def State.put_task(s: State, t: Task) -> State:
  State{ps, ts, es, n, tk, sg, qu, ak, dv, nv, bw} = s
  Task{+id, p, c, d, st, su, dl, pr, mo} = t
  State{ps, ts, es, n, Map.set(&2, Task, tk, id, Task{id, p, c, d, st, su, dl, pr, mo}), sg, qu, ak, dv, nv, bw}

# keys of the ledgers (told.*, rule.*, board.*): an empty value removes the
# key, so dropped records leave nothing in Setting.keys (law
# told_setting_empty_removes)
def Setting.ledger(+k: String) -> Bool:
  Bool.or(Bool.or(String.starts_with(k, "told."), String.starts_with(k, "rule.")), String.starts_with(k, "board."))

def State.setting.put(del: Bool, sg: Map<&2, String>, k: String, v: String) -> Map<&2, String>:
  match del:
    case True{}:
      Map.del(&2, String, sg, k)
    case False{}:
      Map.set(&2, String, sg, k, v)

def State.setting(s: State, +k: String, +v: String) -> State:
  State{ps, ts, es, n, tk, sg, qu, ak, dv, nv, bw} = s
  State{ps, ts, es, n, tk, State.setting.put(Bool.and(String.is_empty(v), Setting.ledger(k)), sg, k, v), qu, ak, dv, nv, bw}

def Ask.blank(id: String) -> Ask:
  Ask{id, "", "", "", "", "", False{}, ""}

def State.ask(s: State, a: Ask) -> State:
  State{ps, ts, es, n, tk, sg, qu, ak, dv, nv, bw} = s
  Ask{+id, th, k, tool, d, o, op, an} = a
  State{ps, ts, es, n, tk, sg, qu, Map.set(&2, Ask, ak, id, Ask{id, th, k, tool, d, o, op, an}), dv, nv, bw}

def Ask.close.set(m: Map<&2, Ask>, +id: String, x: Ask, a: String) -> Map<&2, Ask>:
  match x:
    case Ask{i, th, k, tool, d, o, True{}, _}:
      Map.set(&2, Ask, m, id, Ask{i, th, k, tool, d, o, False{}, a})
    case Ask{_, _, _, _, _, _, False{}, _}:
      m

def Ask.close.k(r: Map<&2, Ask> & Ask, +id: String, a: String) -> Map<&2, Ask>:
  (m, x) = r
  Ask.close.set(m, id, x, a)

# the answer closes an ask for good (a second answer changes nothing)
def State.ask.close(s: State, +id: String, a: String) -> State:
  State{ps, ts, es, n, tk, sg, qu, ak, dv, nv, bw} = s
  State{ps, ts, es, n, tk, sg, qu, Ask.close.k(Map.get(Ask, Ask.blank(id), ak, id), id, a), dv, nv, bw}

def Asks.all(s: State) -> List<&2, Ask>:
  match s:
    case State{_, _, _, _, _, _, _, ak, _, _, _}:
      Map.values(&2, Ask, ak)

# Queued turns
# ------------
# A message sent while its thread's turn is running is stored at once (the
# client never holds it) and waits here, per thread, until the turn ends.

def Queue.join(+a: String, +b: String) -> String:
  Bool.pick(String, String.is_empty(a), b, a ++ "\n\n" ++ b)

def Queue.text(q: QItem) -> String:
  match q:
    case QItem{_, t}:
      t

def Queue.msg(q: QItem) -> String:
  match q:
    case QItem{m, _}:
      m

# the waiting messages as one text, oldest first, a blank line between
def Queue.texts.go(qs: List<&2, QItem>, +acc: String) -> String:
  match qs:
    case Nil{}:
      acc
    case QItem{_, t} <> rest:
      Queue.texts.go(rest, Queue.join(acc, t))

def Queue.texts(qs: List<&2, QItem>) -> String:
  Queue.texts.go(qs, "")

def Queue.edit.k(same: Bool, q: QItem, +text: String) -> QItem:
  match same q:
    case True{} QItem{m, _}:
      QItem{m, text}
    case False{} _:
      q

# the message `msg` with a new text; the rest as they were
def Queue.edit(qs: List<&2, QItem>, +msg: String, +text: String) -> List<&2, QItem>:
  match qs:
    case Nil{}:
      Nil{}
    case QItem{+m, t} <> rest:
      Queue.edit.k(String.eq(m, msg), QItem{m, t}, text) <> Queue.edit(rest, msg, text)

def Queue.drop.k(same: Bool, q: QItem, rest: List<&2, QItem>) -> List<&2, QItem>:
  match same:
    case True{}:
      rest
    case False{}:
      q <> rest

# without the message `msg`
def Queue.drop(qs: List<&2, QItem>, +msg: String) -> List<&2, QItem>:
  match qs:
    case Nil{}:
      Nil{}
    case QItem{+m, t} <> rest:
      Queue.drop.k(String.eq(m, msg), QItem{m, t}, Queue.drop(rest, msg))

def Queue.find.k(same: Bool, q: QItem, rest: Maybe<&2, QItem>) -> Maybe<&2, QItem>:
  match same:
    case True{}:
      Some{q}
    case False{}:
      rest

def Queue.find(qs: List<&2, QItem>, +msg: String) -> Maybe<&2, QItem>:
  match qs:
    case Nil{}:
      None{}
    case QItem{+m, t} <> rest:
      Queue.find.k(String.eq(m, msg), QItem{m, t}, Queue.find(rest, msg))

# q put at place i (past the end: last)
def Queue.insert(i: Nat, qs: List<&2, QItem>, q: QItem) -> List<&2, QItem>:
  match i qs:
    case 0n _:
      q <> qs
    case _ Nil{}:
      [q]
    case 1n+j x <> rest:
      x <> Queue.insert(j, rest, q)

def Queue.move.k(m: Maybe<&2, QItem>, +qs: List<&2, QItem>, +msg: String, to: Nat) -> List<&2, QItem>:
  match m:
    case Some{q}:
      Queue.insert(to, Queue.drop(qs, msg), q)
    case None{}:
      qs

# the message `msg` moved to place `to`; an unknown message moves nothing
def Queue.move(+qs: List<&2, QItem>, +msg: String, to: Nat) -> List<&2, QItem>:
  Queue.move.k(Queue.find(qs, msg), qs, msg, to)

def State.queue.k(f: List<&2, QItem> -> List<&2, QItem>, r: Map<&2, List<&2, QItem>> & List<&2, QItem>, +th: String) -> Map<&2, List<&2, QItem>>:
  (m, cur) = r
  Map.set(&2, List<&2, QItem>, m, th, f(cur))

# apply f to a thread's queue
def State.queue(f: List<&2, QItem> -> List<&2, QItem>, s: State, +th: String) -> State:
  State{ps, ts, es, n, tk, sg, qu, ak, dv, nv, bw} = s
  State{ps, ts, es, n, tk, sg, State.queue.k(f, Map.get(List<&2, QItem>, Nil{}, qu, th), th), ak, dv, nv, bw}

def State.enqueue(s: State, +th: String, +msg: String, +text: String) -> State:
  State.queue((qs => List.append(&2, QItem, qs, [QItem{msg, text}])), s, th)

def State.take(s: State, +th: String) -> State:
  State.queue((qs => Nil{}), s, th)

def Queue.items.k(r: Map<&2, List<&2, QItem>> & List<&2, QItem>) -> List<&2, QItem>:
  (_, v) = r
  v

# a thread's waiting messages, next first
def Queue.items(s: State, +th: String) -> List<&2, QItem>:
  match s:
    case State{_, _, _, _, _, _, qu, _, _, _, _}:
      Queue.items.k(Map.get(List<&2, QItem>, Nil{}, qu, th))

# a setting's value, or d when unset
def State.get.k(r: Map<&2, String> & String) -> String:
  (_, v) = r
  v

# what waits to be sent in a thread ("" for nothing)
def Queue.real(qs: List<&2, QItem>) -> Bool:
  match qs:
    case Nil{}:
      False{}
    case QItem{+m, _} <> t:
      Bool.or(Bool.not(String.starts_with(m, "handoff:")), Queue.real(t))

# whether a thread has a message waiting (handed-over context alone waits
# for a message to go with: it never starts a turn by itself, law
# handoff_never_runs_alone)
def Queue.waiting(s: State, +th: String) -> Bool:
  Queue.real(Queue.items(s, th))

def Queue.of(s: State, +th: String) -> String:
  Queue.texts(Queue.items(s, th))

def Setting.get(s: State, +k: String, d: String) -> String:
  match s:
    case State{_, _, _, _, _, sg, qu, ak, _, _, _}:
      State.get.k(Map.get(String, d, sg, k))

# the threads with something queued (even handed-over context alone), and the
# key of every setting (the status plane counts only the threads these can touch)
def Queue.threads(s: State) -> List<&2, String>:
  match s:
    case State{_, _, _, _, _, _, qu, _, _, _, _}:
      Map.keys(&2, List<&2, QItem>, qu)

def Setting.keys(s: State) -> List<&2, String>:
  match s:
    case State{_, _, _, _, _, sg, _, _, _, _, _}:
      Map.keys(&2, String, sg)

# The keys of a String map that start with pre, without walking the others.
# Base's Map is a crit-bit trie (Map.bit: char ci of a key takes bits
# 33·ci .. 33·ci+32), so every key under a node at bit p agrees on its first
# p/33 chars: once p ≥ 33·|pre|, any one of them says whether all start
# with pre. Order: as Map.keys.
def Pfx.first(m: Map<&2, String>) -> String:
  match m:
    case MTip{}:
      ""
    case MLeaf{k, _}:
      k
    case MNode{_, MTip{}, hi}:
      Pfx.first(hi)
    case MNode{_, lo, _}:
      Pfx.first(lo)

# whether some key under m may start with pre (bits: 33·|pre|)
def Pfx.may(+m: Map<&2, String>, +pre: String, +bits: Nat) -> Bool:
  match m:
    case MTip{}:
      False{}
    case MLeaf{k, _}:
      String.starts_with(k, pre)
    case MNode{p, _, _}:
      Bool.or(Nat.is_lt(p, bits), String.starts_with(Pfx.first(m), pre))

def Pfx.go(m: Map<&2, String>, may: Bool, +pre: String, +bits: Nat, acc: List<&2, String>) -> List<&2, String>:
  match m may:
    case _ False{}:
      acc
    case MTip{} True{}:
      acc
    case MLeaf{k, _} True{}:
      k <> acc
    case MNode{_, +lo, +hi} True{}:
      Pfx.go(lo, Pfx.may(lo, pre, bits), pre, bits, Pfx.go(hi, Pfx.may(hi, pre, bits), pre, bits, acc))

def Pfx.keys(+m: Map<&2, String>, +pre: String) -> List<&2, String>:
  Pfx.go(m, Pfx.may(m, pre, Nat.mul(33n, String.length(pre))), pre, Nat.mul(33n, String.length(pre)), Nil{})

# the settings' keys that start with pre
def Setting.keys.pre(s: State, +pre: String) -> List<&2, String>:
  match s:
    case State{_, _, _, _, _, sg, _, _, _, _, _}:
      Pfx.keys(sg, pre)

# Thinking (opt-in, the setting agent.thinking, off unless "on"): the
# agents are asked for their thinking (Claude's --thinking-display
# summarized, Codex's reasoning summaries) and the hub logs each finished
# thought as an act of tone "thinking" (hub.bend's Think); the clients show
# them, folded, only while it is on
def Think.key() -> String:
  "agent.thinking"

def Think.on(+s: State) -> Bool:
  String.eq(Setting.get(s, Think.key(), "off"), "on")

def Think.tone() -> String:
  "thinking"

# The "restart.continue" setting: after a restart, a turn the restart cut
# short goes on by itself (on unless "off"; hub.bend's Carry)
def Carry.on(+s: State) -> Bool:
  Bool.not(String.eq(Setting.get(s, "restart.continue", "on"), "off"))

# what such a thread is told
def Carry.text() -> String:
  "Backplane restarted while you were working. Continue where you left off."

# Providers in turn
# -----------------
# A thread may go on with another provider between turns (V2's provider
# switch). Each provider keeps its own session of the thread (the setting
# session.<thread>.<provider>) and how many of the thread's entries it had
# seen when it was left (seen.<thread>.<provider>), so coming back resumes
# its own conversation and it is handed only what it missed (laws
# switch_*).

def Switch.session.key(+th: String, +p: String) -> String:
  "session." ++ th ++ "." ++ p

def Switch.seen.key(+th: String, +p: String) -> String:
  "seen." ++ th ++ "." ++ p

def Switch.count.k(hit: Bool, n: Nat) -> Nat:
  match hit:
    case True{}:
      1n+n
    case False{}:
      n

def Switch.of(e: Entry) -> String:
  match e:
    case Msg{_, t, _, _, _}:
      t
    case Act{_, t, _, _, _, _}:
      t

# how many of these entries are the thread's own, in a tail call: the
# phones' JavaScript has a small stack, and a walk that added outside the
# call overflowed it on a real log (one ProviderSet blanked the app)
def Switch.count.go(es: List<&2, Entry>, +th: String, n: Nat) -> Nat:
  match es:
    case Nil{}:
      n
    case +e <> t:
      Switch.count.go(t, th, Switch.count.k(String.eq(Switch.of(e), th), n))

# how many entries a thread has: its own among those filed under it (Idx;
# the messages it sent elsewhere are filed there too), never the whole log
def Switch.count(s: State, +th: String) -> Nat:
  Switch.count.go(State.mine(s, th), th, 0n)

def Switch.prov(t: Thread) -> String:
  match t:
    case Thread{_, _, _, _, _, _, _, _, _, _, _, _, _, _, _, pr, _, _, _, _, _}:
      pr

def Switch.sess(t: Thread) -> String:
  match t:
    case Thread{_, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, se, _}:
      se

def Switch.set(p: String, m: String, se: String, t: Thread) -> Thread:
  Thread{id, pj, ti, c, a, tu, pe, ar, o, sa, ua, pi, en, br, wt, _, _, rm, im, _, de} = t
  Thread{id, pj, ti, c, a, tu, pe, ar, o, sa, ua, pi, en, br, wt, p, m, rm, im, se, de}

def State.entries.all(s: State) -> List<&2, Entry>:
  match s:
    case State{_, _, es, _, _, _, _, _, _, _, _}:
      es

def State.thread.get.k(r: Map<&2, Thread> & Thread) -> Thread:
  (_, t) = r
  t

def State.thread.get(s: State, +id: String) -> Thread:
  match s:
    case State{_, ts, _, _, _, _, _, _, _, _, _}:
      State.thread.get.k(Map.get(Thread, Thread.blank(id), ts, id))

# No message ever waits behind a running turn (hub.bend's Hub.deliver, law
# deliver_never_waits). What a thread holds is handed-over context, a
# message for a worktree still being made (it starts the turn as soon as
# the worktree is there: the thread shows as working), or, from logs older
# than that rule, messages that go with the thread's next turn.
def Queue.starting(+s: State, +th: String) -> Bool:
  Bool.and(Queue.waiting(s, th), Worktree.unmade(State.thread.get(s, th)))

def Queue.context.k(keep: Bool, q: QItem, rest: List<&2, QItem>) -> List<&2, QItem>:
  match keep:
    case True{}:
      q <> rest
    case False{}:
      rest

# the rows shown above a composer: handed-over context only
def Queue.context(qs: List<&2, QItem>) -> List<&2, QItem>:
  match qs:
    case Nil{}:
      Nil{}
    case QItem{+m, t} <> rest:
      Queue.context.k(String.starts_with(m, "handoff:"), QItem{m, t}, Queue.context(rest))

# the switch given how many entries the thread has (Switch.count, or a
# store's counter that equals it: the indexed store's build keeps no entries)
def State.switch.n(+s: State, +th: String, +old: String, +oldse: String, +p: String, m: String, +n: Nat) -> State:
  +s1 = State.setting(State.setting(s, Switch.session.key(th, old), oldse), Switch.seen.key(th, old), Nat.show(n))
  State.thread((t => Switch.set(p, m, Setting.get(s1, Switch.session.key(th, p), ""), t)), s1, th)

def State.switch.k(+s: State, +th: String, +old: String, +oldse: String, +p: String, m: String) -> State:
  State.switch.n(s, th, old, oldse, p, m, Switch.count(s, th))

# a ProviderSet with the thread's entry count given
def State.switch.c(+s: State, +th: String, +p: String, m: String, +n: Nat) -> State:
  State.switch.n(s, th, Switch.prov(State.thread.get(s, th)), Switch.sess(State.thread.get(s, th)), p, m, n)

def State.switch(+s: State, +th: String, +p: String, m: String) -> State:
  State.switch.k(s, th, Switch.prov(State.thread.get(s, th)), Switch.sess(State.thread.get(s, th)), p, m)

# how many of a thread's entries a provider had seen when it was left
# (0: it never took part)
# a count written in decimal (Nat.read is slow to check in laws)
def Switch.num(s: String, +acc: Nat) -> Nat:
  match s:
    case SNil{}:
      acc
    case SCon{Chr{+c}, t}:
      Switch.num(t, Nat.add(Nat.mul(acc, 10n), U32.to_nat((c - 48 : U32))))

# Worktrees made again
# ---------------------
# A removed worktree (its branch kept) is made again by the thread's next
# message (hub.bend's Hub.revive). The hub records when it asked
# (Worktree.again.key, cleared when the worktree came or could not be
# made), and the thread waits for it only while that ask is fresh. Its last
# activity says nothing of a rebuild: a turn that just ended and whose
# worktree was then removed is not being made again, so a message then
# asks for it instead of waiting for a worktree nobody makes (laws
# wt_again_*). A new thread still waits for its first worktree from its
# creation (Worktree.pending).

def Worktree.again.key(+th: String) -> String:
  "wt.again." ++ th

# when the hub last asked for the thread's worktree again (0: not asked)
def Worktree.again.at(+s: State, +th: String) -> Nat:
  Switch.num(Setting.get(s, Worktree.again.key(th), ""), 0n)

def Worktree.kept(t: Thread) -> Bool:
  match t:
    case Thread{_, _, _, _, _, _, _, _, _, _, _, _, _, br, _, _, _, _, _, _, _}:
      Bool.not(String.is_empty(br))

def Worktree.waits.kept(+s: State, t: Thread, +now: Nat) -> Bool:
  Thread{+id, _, _, _, _, _, _, _, _, _, _, _, en, _, +wt, _, _, _, _, _, _} = t
  +ask = {Worktree.again.at(s, id) : Nat}
  Bool.and(Nat.is_gt(ask, 0n), Worktree.pending.k(en, wt, ask, now))

def Worktree.waits.k(kept: Bool, +s: State, +t: Thread, +now: Nat) -> Bool:
  match kept:
    case True{}:
      Worktree.waits.kept(s, t, now)
    case False{}:
      Worktree.pending(t, now)

# whether a thread's worktree is being made now: a new thread's first one
# (from its creation), or a removed one the hub asked for again
def Worktree.waits(+s: State, +t: Thread, +now: Nat) -> Bool:
  Worktree.waits.k(Worktree.kept(t), s, t, now)

def Switch.seen(s: State, +th: String, +p: String) -> Nat:
  Switch.num(Setting.get(s, Switch.seen.key(th, p), "0"), 0n)

# Scratch threads
# ---------------
# Conversations that belong to no project live in the built-in project
# Scratch.id(), which the hub makes at start (rooted at <home>/scratch;
# once removed it stays removed), each thread in a folder of its own. They
# are ephemeral: one left alone for the "scratch.days" setting (1..90,
# else 7) is deleted with its folder, unless it is pinned (kept) or busy
# (laws scratch_*). "/give" hands one's conversation to threads, projects
# and bots (bothub.bend's Give).

def Scratch.id() -> String:
  "scratch"

def Scratch.is(+pid: String) -> Bool:
  String.eq(pid, Scratch.id())

def Scratch.days.k(+n: Nat) -> Nat:
  Bool.pick(Nat, Nat.is_eq(n, 0n), 7n, Bool.pick(Nat, Nat.is_gt(n, 90n), 90n, n))

# how long a scratch thread lives after its last activity, in seconds
def Scratch.window(s: State) -> Nat:
  Nat.mul(Scratch.days.k(Switch.num(Setting.get(s, "scratch.days", "7"), 0n)), 86400n)

def Scratch.old.k(scratch: Bool, keep: Bool, +now: Nat, +at: Nat, +w: Nat) -> Bool:
  match scratch:
    case True{}:
      Bool.and(Bool.not(keep), Nat.is_ge(now, Nat.add(at, w)))
    case False{}:
      False{}

# a scratch thread that has had its time: not pinned, not deleted, not busy
def Scratch.expired(+now: Nat, +w: Nat, t: Thread) -> Bool:
  match t:
    case Thread{_, +p, _, _, +a, turn, +pend, _, _, _, _, pin, _, _, _, _, _, _, _, _, del}:
      Scratch.old.k(Scratch.is(p), Bool.or(pin, Bool.or(del, Thread.busy(turn, pend))), now, a, w)

def Scratch.ensure.k(r: Map<&2, Project> & Bool, +root: String, +now: Nat) -> List<&2, Change>:
  (_, has) = r
  match has:
    case True{}:
      Nil{}
    case False{}:
      [ProjectCreated{Scratch.id(), "Scratch", root, now}]

# the Scratch project, made once (a removed one is left removed)
def Scratch.ensure(s: State, +root: String, +now: Nat) -> List<&2, Change>:
  match s:
    case State{ps, _, _, _, _, _, _, _, _, _, _}:
      Scratch.ensure.k(Map.has(&2, Project, ps, Scratch.id()), root, now)

# Context usage
# -------------
# How much of its model's context window a thread's session holds, as
# "<used>/<window>" in the setting usage.<thread> (laws usage_*).

def Usage.key(+th: String) -> String:
  "usage." ++ th

def Usage.part(ps: List<&2, String>, +i: Nat) -> Nat:
  Switch.num(Maybe.default(&2, String, List.head(&2, String, List.drop(&2, String, ps, i)), "0"), 0n)

def Usage.used(+v: String) -> Nat:
  Usage.part(String.split(v, '/'), 0n)

def Usage.window(+v: String) -> Nat:
  Usage.part(String.split(v, '/'), 1n)

def Usage.keep(+new: Nat, +old: Nat) -> Nat:
  Bool.pick(Nat, Nat.is_eq(new, 0n), old, new)

# a new reading over the old one (0 keeps the old part)
def Usage.merge(+old: String, +used: Nat, +window: Nat) -> String:
  Nat.show(Usage.keep(used, Usage.used(old))) ++ "/" ++ Nat.show(Usage.keep(window, Usage.window(old)))

def State.usage(+s: State, +th: String, +used: Nat, +window: Nat) -> State:
  State.setting(s, Usage.key(th), Usage.merge(Setting.get(s, Usage.key(th), ""), used, window))

# Tokens
# ------
# Each turn's usage as its provider reported it (TurnUsage, laws tok_*):
# uncached input, cache read, cache write, output and the part of it that
# was reasoning, and the cost in nano-dollars. `known` names the parts the
# provider gave (i r w o t c): a part it did not give is unknown, never 0.
# Totals are settings "n/i/r/w/o/t/c/cn" (turns, the parts, cost, turns that
# reported a cost): per thread tok.t.<thread>, per UTC day tok.d.<YYYY-MM-DD>,
# per month tok.m.<YYYY-MM>, per month and model tok.mm.<YYYY-MM>|<provider>|<model>
# (that month's models, a line each, in tok.ml.<YYYY-MM>), per project and
# month tok.p.<project>|<YYYY-MM>. Daily dimensions: dv.<day>|<provider>
# and dp.<day>|<project>; daily.since marks their first accepted turn. tok.since is the first turn counted
# (nothing before it was recorded); tok.key.<thread> and tok.basis.<thread>
# are the hub's: the last turn's key (a line seen again is not counted
# twice) and the provider's running totals it took the turn's share from.

def Tok.pad2(+n: Nat) -> String:
  Bool.pick(String, Nat.is_lt(n, 10n), "0" ++ Nat.show(n), Nat.show(n))

def Tok.civil.m(+y: Nat, +mp: Nat, +doy: Nat) -> String:
  Nat.show(y) ++ "-" ++ Tok.pad2(Bool.pick(Nat, Nat.is_lt(mp, 10n), Nat.add(mp, 3n), Nat.sub(mp, 9n))) ++ "-" ++
    Tok.pad2(Nat.add(Nat.sub(doy, Nat.div(Nat.add(Nat.mul(153n, mp), 2n), 5n)), 1n))

# a day number (days since 1970-01-01) as a civil date (H. Hinnant's
# days_from_civil, inverted), year-month-day
def Tok.civil.k(+y: Nat, +doy: Nat) -> String:
  Tok.civil.m(Nat.add(y, Bool.pick(Nat, Nat.is_ge(Nat.div(Nat.add(Nat.mul(5n, doy), 2n), 153n), 10n), 1n, 0n)),
    Nat.div(Nat.add(Nat.mul(5n, doy), 2n), 153n), doy)

def Tok.civil.e(+era: Nat, +doe: Nat, +yoe: Nat) -> String:
  Tok.civil.k(Nat.add(yoe, Nat.mul(era, 400n)), Nat.sub(doe, Nat.sub(Nat.add(Nat.mul(365n, yoe), Nat.div(yoe, 4n)), Nat.div(yoe, 100n))))

def Tok.civil.d(+era: Nat, +doe: Nat) -> String:
  Tok.civil.e(era, doe, Nat.div(Nat.sub(Nat.add(Nat.sub(doe, Nat.div(doe, 1460n)), Nat.div(doe, 36524n)), Nat.div(doe, 146096n)), 365n))

def Tok.civil.z(+z: Nat) -> String:
  Tok.civil.d(Nat.div(z, 146097n), Nat.mod(z, 146097n))

# the UTC day of a moment ("2026-10-15") and its month ("2026-10")
def Tok.day(+at: Nat) -> String:
  Tok.civil.z(Nat.add(Nat.div(at, 86400n), 719468n))

def Tok.month(+at: Nat) -> String:
  String.take(Tok.day(at), 7n)

def Tok.part(+v: String, +i: Nat) -> Nat:
  Usage.part(String.split(v, '/'), i)

def Tok.add(+old: String, +i: Nat, +r: Nat, +w: Nat, +o: Nat, +t: Nat, +c: Nat, ck: Bool) -> String:
  String.join([Nat.show(Nat.add(Tok.part(old, 0n), 1n)), Nat.show(Nat.add(Tok.part(old, 1n), i)), Nat.show(Nat.add(Tok.part(old, 2n), r)),
    Nat.show(Nat.add(Tok.part(old, 3n), w)), Nat.show(Nat.add(Tok.part(old, 4n), o)), Nat.show(Nat.add(Tok.part(old, 5n), t)),
    Nat.show(Nat.add(Tok.part(old, 6n), c)), Nat.show(Nat.add(Tok.part(old, 7n), Bool.pick(Nat, ck, 1n, 0n)))], "/")

def Tok.has(xs: List<&2, String>, +l: String) -> Bool:
  match xs:
    case Nil{}:
      False{}
    case x <> t:
      Bool.or(String.eq(x, l), Tok.has(t, l))

# a line added to a list of lines, once
def Tok.once(+ls: String, +l: String) -> String:
  Bool.pick(String, Tok.has(String.split(ls, '\n'), l), ls, Bool.pick(String, String.is_empty(ls), l, ls ++ "\n" ++ l))

# a total with its ninth part: the turns whose tokens the provider gave (a
# turn whose share could not be told, its session's start unseen, counts
# as a turn with its tokens unknown)
def Tok.add2(+old: String, +i: Nat, +r: Nat, +w: Nat, +o: Nat, +t: Nat, +c: Nat, ck: Bool, kk: Bool) -> String:
  Tok.add(old, i, r, w, o, t, c, ck) ++ "/" ++ Nat.show(Nat.add(Tok.part(old, 8n), Bool.pick(Nat, kk, 1n, 0n)))

def Tok.sum(+s: State, +k: String, +i: Nat, +r: Nat, +w: Nat, +o: Nat, +t: Nat, +c: Nat, +ck: Bool, +kk: Bool) -> State:
  State.setting(s, k, Tok.add2(Setting.get(s, k, ""), i, r, w, o, t, c, ck, kk))

# Totals are kept under the count's epoch (the setting tok.epoch, set once
# by the hub when it starts counting again): "tok." then "<epoch>." when one
# is set. Totals of an earlier count stay in the log and are never shown.
def Tok.pre.k(+e: String) -> String:
  Bool.pick(String, String.is_empty(e), "tok.", "tok." ++ e ++ ".")

def Tok.pre(+s: State) -> String:
  Tok.pre.k(Setting.get(s, "tok.epoch", "") ++ Setting.get(s, "tok.recount", ""))

def Tok.project(t: Thread) -> String:
  match t:
    case Thread{_, p, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _}:
      p

# a thread's last turn as told (usage.bend's Ts), the setting
# turn.last.<thread> = "<in>/<cache read>/<cache write>/<out>/<reasoning>/<nano-dollars>/<at>/<known>/<provider>|<model>"
# (the model last: its name may hold a "/")
def Tok.last.key(+th: String) -> String:
  "turn.last." ++ th

def Tok.last(+i: Nat, +r: Nat, +w: Nat, +o: Nat, +t: Nat, +c: Nat, +at: Nat, +known: String, +pm: String) -> String:
  String.join([Nat.show(i), Nat.show(r), Nat.show(w), Nat.show(o), Nat.show(t), Nat.show(c), Nat.show(at), known, pm], "/")

def State.tokens.k(+s: State, +x: String, +th: String, +pj: String, +pm: String, +i: Nat, +r: Nat, +w: Nat, +o: Nat, +t: Nat, +c: Nat, +ck: Bool, +kk: Bool, +key: String, +basis: String, +at: Nat) -> State:
  +s1 = Tok.sum(Tok.sum(Tok.sum(s, x ++ "t." ++ th, i, r, w, o, t, c, ck, kk), x ++ "d." ++ Tok.day(at), i, r, w, o, t, c, ck, kk), x ++ "m." ++ Tok.month(at), i, r, w, o, t, c, ck, kk)
  +s2 = Tok.sum(Tok.sum(s1, x ++ "mm." ++ Tok.month(at) ++ "|" ++ pm, i, r, w, o, t, c, ck, kk), x ++ "p." ++ pj ++ "|" ++ Tok.month(at), i, r, w, o, t, c, ck, kk)
  # Additive daily dimensions use the same accepted turn and epoch as totals.
  # Existing persisted totals are never split or estimated retrospectively.
  +sd = Tok.sum(Tok.sum(s2, x ++ "dv." ++ Tok.day(at) ++ "|" ++ Maybe.default(&2, String, List.head(&2, String, String.split(pm, '|')), ""), i, r, w, o, t, c, ck, kk), x ++ "dp." ++ Tok.day(at) ++ "|" ++ pj, i, r, w, o, t, c, ck, kk)
  +ss = State.setting(sd, x ++ "daily.since", Bool.pick(String, String.is_empty(Setting.get(sd, x ++ "daily.since", "")), Nat.show(at), Setting.get(sd, x ++ "daily.since", "")))
  +s3 = State.setting(ss, x ++ "ml." ++ Tok.month(at), Tok.once(Setting.get(s2, x ++ "ml." ++ Tok.month(at), ""), pm))
  +s4 = State.setting(s3, x ++ "since", Bool.pick(String, String.is_empty(Setting.get(s3, x ++ "since", "")), Nat.show(at), Setting.get(s3, x ++ "since", "")))
  State.setting(State.setting(s4, "tok.key." ++ th, key), "tok.basis." ++ th, basis)

def State.tokens(+s: State, +th: String, +pv: String, +mo: String, +i: Nat, +r: Nat, +w: Nat, +o: Nat, +t: Nat, +c: Nat, +known: String, +key: String, +basis: String, +at: Nat) -> State:
  State.setting(State.tokens.k(s, Tok.pre(s), th, Tok.project(State.thread.get(s, th)), pv ++ "|" ++ Bool.pick(String, String.is_empty(mo), "default", mo), i, r, w, o, t, c,
    String.contains(known, "c"), String.contains(known, "o"), key, basis, at),
    Tok.last.key(th), Tok.last(i, r, w, o, t, c, at, known, pv ++ "|" ++ Bool.pick(String, String.is_empty(mo), "default", mo)))


# Launched threads
# ----------------
# A thread started by another (thread_launch) remembers who started it, as
# the setting launch.<id>: "1" or "0" (whether it reports back) and then
# the launcher. The launcher's list is the setting launched.<by>: its
# launched threads one per line, oldest first, each once (law
# launch_list_once).

def Launch.key(+id: String) -> String:
  "launch." ++ id

def Launch.list.key(+by: String) -> String:
  "launched." ++ by

def Launch.val(+by: String, report: Bool) -> String:
  Bool.pick(String, report, "1", "0") ++ by

# who launched a thread ("" for one nobody launched)
def Launch.by(s: State, +id: String) -> String:
  String.drop(Setting.get(s, Launch.key(id), ""), 1n)

# whether a launched thread reports back to its launcher
def Launch.reports(s: State, +id: String) -> Bool:
  +v = Setting.get(s, Launch.key(id), "")
  Bool.and(String.starts_with(v, "1"), Nat.is_gt(String.length(v), 1n))

def Launch.has(xs: List<&2, String>, +id: String) -> Bool:
  match xs:
    case Nil{}:
      False{}
    case +x <> t:
      Bool.or(String.eq(x, id), Launch.has(t, id))

def Launch.lines(+v: String) -> List<&2, String>:
  Bool.pick(List<&2, String>, String.is_empty(v), Nil{}, String.split(v, '\n'))

def Launch.add.k(had: Bool, +v: String, +id: String) -> String:
  match had:
    case True{}:
      v
    case False{}:
      Bool.pick(String, String.is_empty(v), id, v ++ "\n" ++ id)

# a launcher's list with one more thread (kept once)
def Launch.add(+v: String, +id: String) -> String:
  Launch.add.k(Launch.has(Launch.lines(v), id), v, id)

# the threads `by` launched, oldest first
def Launch.of(s: State, +by: String) -> List<&2, String>:
  Launch.lines(Setting.get(s, Launch.list.key(by), ""))

def Launch.fold(+s: State, +id: String, +by: String, report: Bool) -> State:
  State.setting(State.setting(s, Launch.key(id), Launch.val(by, report)), Launch.list.key(by), Launch.add(Setting.get(s, Launch.list.key(by), ""), id))

# Lock records and the name index are projections, never timeline scans.
def Locks.names(s: State) -> List<&2, String>:
  Launch.lines(Setting.get(s, "locks.names", ""))

def Locks.get(s: State, +name: String) -> LK.NamedLock:
  LK.Lock.read(Maybe.default(&2, J.Json, J.Json.parse(Setting.get(s, "lock." ++ name, "")), J.Null{}))

# the lock is free with no one waiting (never held, or released): what
# Locks.get reads as LK.Lock.empty(), told from its setting's text without
# parsing it. A real log keeps every lock name ever used (72), nearly all
# idle, and every step and every sidebar row read each one (laws
# locks_idle_*).
def Locks.idle.text() -> String:
  "{\"holder\":null,\"waiters\":[],\"ready\":false}"

def Locks.idle.k(+t: String) -> Bool:
  Bool.or(String.is_empty(t), String.eq(t, Locks.idle.text()))

def Locks.idle(s: State, +name: String) -> Bool:
  Locks.idle.k(Setting.get(s, "lock." ++ name, ""))

# deploy_main's state, one JSON line (core/ship.bend)
def Ship.fold.asked(+s: State, +by: String, +cover: String) -> State:
  State.setting(s, "deploy.ship", SH.Ship.show(SH.Ship.asked(SH.Ship.read(Setting.get(s, "deploy.ship", "")), by, cover)))

def Ship.fold.built(+s: State, +commit: String, +state: String, +detail: String) -> State:
  State.setting(s, "deploy.ship", SH.Ship.show(SH.Ship.built(SH.Ship.read(Setting.get(s, "deploy.ship", "")), commit, state, detail)))

def Locks.fold(+s: State, +name: String, l: LK.NamedLock) -> State:
  State.setting(State.setting(s, "lock." ++ name, J.Json.show(LK.Lock.json(l))), "locks.names", Launch.add(Setting.get(s, "locks.names", ""), name))

# Wake subscriptions: indexes contain ids, records live under wake.<id>.
def Wake.strs(xs: List<&2, String>) -> List<&2, J.Json>:
  match xs:
    case Nil{}:
      Nil{}
    case x <> t:
      J.Str{x} <> Wake.strs(t)

def Wake.record(+id: String, +by: String, +kind: String, +target: String, events: List<&2, String>, once: Bool, note: String, at: Nat) -> J.Json:
  J.Json.obj([J.KV{"id", J.Str{id}}, J.KV{"by", J.Str{by}}, J.KV{"kind", J.Str{kind}}, J.KV{"target", J.Str{target}}, J.KV{"events", J.Json.arr(Wake.strs(events))}, J.KV{"once", J.Flag{once}}, J.KV{"note", J.Str{note}}, J.KV{"created", J.Num{Nat.show(at)}}])

def Wake.get(s: State, +id: String) -> J.Json:
  Maybe.default(&2, J.Json, J.Json.parse(Setting.get(s, "wake." ++ id, "")), J.Null{})

def Wake.ids(s: State, +key: String) -> List<&2, String>:
  Launch.lines(Setting.get(s, key, ""))

def Wake.index(+s: State, +key: String, +id: String) -> State:
  State.setting(s, key, Launch.add(Setting.get(s, key, ""), id))

def Wake.poll.add(poll: Bool, +s: State, +id: String) -> State:
  match poll:
    case True{}:
      Wake.index(s, "wakes.poll", id)
    case False{}:
      s

def Wake.fold(+s: State, +id: String, +by: String, +kind: String, +target: String, events: List<&2, String>, once: Bool, note: String, at: Nat) -> State:
  Wake.poll.add(Bool.or(String.eq(kind, "pr"), String.eq(kind, "file")), Wake.index(Wake.index(State.setting(s, "wake." ++ id, J.Json.show(Wake.record(id, by, kind, target, events, once, note, at))), "wakes.by." ++ by, id), "wakes.on." ++ kind ++ ":" ++ target, id), id)

def Wake.keep.k(hit: Bool, +id: String, rest: List<&2, String>) -> List<&2, String>:
  match hit:
    case True{}:
      rest
    case False{}:
      id <> rest

def Wake.keep(xs: List<&2, String>, +id: String) -> List<&2, String>:
  match xs:
    case Nil{}:
      Nil{}
    case +h <> t:
      Wake.keep.k(String.eq(h, id), h, Wake.keep(t, id))

def Wake.unindex(+s: State, +key: String, +id: String) -> State:
  State.setting(s, key, String.join(Wake.keep(Wake.ids(s, key), id), "\n"))

def Wake.drop.record(+o: J.Json, +s: State, +id: String) -> State:
  State.setting(Wake.unindex(Wake.unindex(Wake.unindex(s, "wakes.poll", id), "wakes.by." ++ J.Json.get_str(o, "by", ""), id), "wakes.on." ++ J.Json.get_str(o, "kind", "") ++ ":" ++ J.Json.get_str(o, "target", ""), id), "wake." ++ id, "")

def Wake.drop(+s: State, +id: String) -> State:
  Wake.drop.record(Wake.get(s, id), s, id)

# Forks
# -----
# A fork remembers its source thread (the setting fork.<id>).

def Fork.key(+id: String) -> String:
  "fork." ++ id

# the thread a fork was made from ("" for a thread that is no fork)
def Fork.source(s: State, +id: String) -> String:
  Setting.get(s, Fork.key(id), "")

def Merge.branch(t: Thread) -> String:
  match t:
    case Thread{_, _, _, _, _, _, _, _, _, _, _, _, _, br, _, _, _, _, _, _, _}:
      br

def Merge.id(t: Thread) -> String:
  match t:
    case Thread{i, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _}:
      i

# whether a merge of this thread's branch should still settle it: it has a
# branch, sits on the pinned or active shelf, and no merge is recorded
def Merge.due(+s: State, +t: Thread) -> Bool:
  Merge.due.k(Shelf.of(t), Merge.branch(t), Setting.get(s, Merge.key(Merge.id(t)), ""))

def Worktree.clock(sh: Shelf, +settled_at: Nat, +activity: Nat) -> Nat:
  match sh:
    case Settled{}:
      settled_at
    case _:
      activity

def Worktree.drop.t(t: Thread, +now: Nat, +window: Nat, +merged: String, +sh: Shelf) -> Bool:
  match t:
    case Thread{_, _, _, _, +a, tu, +pe, _, _, +sa, _, _, _, _, +wt, _, _, _, _, _, _}:
      Worktree.drop.k(Thread.busy(tu, pe), sh, Worktree.ours(wt), Bool.not(String.is_empty(merged)), Settle.stale(now, Worktree.clock(sh, sa, a), window))

# whether this thread's worktree is removed now: put away for the window
# (settled: since it settled; hidden: since its last activity), or merged
def Worktree.drop(+s: State, +t: Thread, +now: Nat, +window: Nat) -> Bool:
  Worktree.drop.t(t, now, window, Setting.get(s, Merge.key(Merge.id(t)), ""), Shelf.of(t))

# Devices
# -------
# Push registrations, keyed by token: registering a token again replaces
# it (law device_register_once). An empty token is not a device: that is
# what clients fold, since the server sends them device events without it.

def State.register.k(empty: Bool, s: State, +tok: String, p: String, k: String, th: String, e: String, b: String, at: Nat) -> State:
  match empty:
    case True{}:
      s
    case False{}:
      State{ps, ts, es, n, tk, sg, qu, ak, dv, nv, bw} = s
      State{ps, ts, es, n, tk, sg, qu, ak, Map.set(&2, Device, dv, tok, Device{tok, p, k, th, e, b, at}), nv, bw}

def State.register(s: State, +tok: String, p: String, k: String, th: String, e: String, b: String, at: Nat) -> State:
  State.register.k(String.is_empty(tok), s, tok, p, k, th, e, b, at)

def State.unregister(s: State, tok: String) -> State:
  State{ps, ts, es, n, tk, sg, qu, ak, dv, nv, bw} = s
  State{ps, ts, es, n, tk, sg, qu, ak, Map.del(&2, Device, dv, tok), nv, bw}

# every registered device
# the read model as the window may hold it: no devices (their tokens stay
# in the hub, as the wire leaves them out; law window_state_private)
def State.private(s: State) -> State:
  State{ps, ts, es, n, tk, sg, qu, ak, _, nv, bw} = s
  State{ps, ts, es, n, tk, sg, qu, ak, Map.new(&2, Device), nv, bw}

def State.devices(s: State) -> Map<&2, Device>:
  match s:
    case State{_, _, _, _, _, _, _, _, dv, _, _}:
      dv

def State.nav.k(f: Nav -> Nav, r: Map<&2, Nav> & Nav, +id: String) -> Map<&2, Nav>:
  (m, n) = r
  Map.set(&2, Nav, m, id, f(n))

# apply f to a thread's sidebar extension
def State.nav(f: Nav -> Nav, s: State, +id: String) -> State:
  State{ps, ts, es, n, tk, sg, qu, ak, dv, nv, bw} = s
  State{ps, ts, es, n, tk, sg, qu, ak, dv, State.nav.k(f, Map.get(Nav, Nav.blank(), nv, id), id), bw}

# every thread's sidebar extension
def State.navs(s: State) -> Map<&2, Nav>:
  match s:
    case State{_, _, _, _, _, _, _, _, _, nv, bw}:
      nv

def State.bots.k(bw: List<&2, Ext>) -> BT.World:
  Ext.world(Ext.of(bw))

def State.world(f: BT.World -> BT.World, s: State) -> State:
  match s:
    case State{ps, ts, es, n, tk, sg, qu, ak, dv, nv, bw}:
      State{ps, ts, es, n, tk, sg, qu, ak, dv, nv, [Ext.with_world(f, Ext.of(bw))]}

def State.running.k(t: Thread) -> Bool:
  match t:
    case Thread{_, _, _, _, _, Running{}, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _}:
      True{}
    case _:
      False{}

def State.world.of(s: State) -> BT.World:
  match s:
    case State{_, _, _, _, _, _, _, _, _, _, bw}:
      State.bots.k(bw)

# a message to a bot waits for its next turn while its thread runs one (as
# hub.bend's inbox queues it)
def State.busy_bot(+s: State, +bot: String) -> Bool:
  State.running.k(State.thread.get(s, BT.Bot.thread(BT.World.bot(State.world.of(s), bot))))

def State.woke.k(busy: Bool, s: State, +bot: String, +hop: Nat) -> State:
  State.world((w => BT.World.woke(w, bot, busy, hop)), s)

def State.woke(+s: State, +bot: String, +hop: Nat) -> State:
  State.woke.k(State.busy_bot(s, bot), s, bot, hop)

# The uberbot's op log (ubop.bend, ubersync.bend, docs/uber.md)
# --------------------------------------------------------------
# An op is taken only as its origin's next (n = highest seen + 1), so what
# a machine has from an origin is always 1..n. Taken, it is kept as a line
# (uber.op.<uid>.<origin>.<n>), moves that origin's count (uber.vv...) and
# the clock (uber.hlc...), and writes its register (uber.r.<uid>.<key>,
# its stamp in uber.rs...) only if it is later than the one there: the
# same ops in any order leave the same registers. A "fired" op keeps the
# highest slot a routine ran for (uber.fired...) and who ran it.

# the op that held the register before is now a nop line
def State.uber.nop.k(ps: List<&2, String>, +s: State, +uid: String, +ostamp: String) -> State:
  match ps:
    case +o <> +c <> Nil{}:
      State.setting(s, UO.Ub.key.op(uid, o, Switch.num(c, 0n)), UO.Ub.line.nop(uid, o, Switch.num(c, 0n), ostamp))
    case _:
      s

def State.uber.nop(+s: State, +uid: String, +rn: String, +ostamp: String) -> State:
  State.uber.nop.k(String.split(rn, '.'), s, uid, ostamp)

def State.uber.reg.k(win: Bool, +s: State, +uid: String, +key: String, +stamp: String, +op: String, +origin: String, +n: Nat) -> State:
  match win:
    case True{}:
      State.setting(State.setting(State.setting(State.uber.nop(s, uid, Setting.get(s, UO.Ub.key.rn(uid, key), ""), Setting.get(s, UO.Ub.key.stamp(uid, key), "")),
        UO.Ub.key.reg(uid, key), op), UO.Ub.key.stamp(uid, key), stamp), UO.Ub.key.rn(uid, key), origin ++ "." ++ Nat.show(n))
    case False{}:
      s

def State.uber.reg(+s: State, +uid: String, +key: String, +stamp: String, +op: String, +origin: String, +n: Nat) -> State:
  State.uber.reg.k(UO.Ub.newer(key, op, stamp, Setting.get(s, UO.Ub.key.reg(uid, key), ""), Setting.get(s, UO.Ub.key.stamp(uid, key), "")),
    s, uid, key, stamp, op, origin, n)

def State.uber.fired.k(win: Bool, +s: State, +uid: String, +origin: String, +gid: String, +slot: Nat) -> State:
  match win:
    case True{}:
      State.setting(State.setting(s, UO.Ub.key.fired(uid, gid), Nat.show(slot)), UO.Ub.key.by(uid, gid), origin)
    case False{}:
      s

def State.uber.fired(+s: State, +uid: String, +origin: String, +o: J.Json) -> State:
  State.uber.fired.k(Nat.is_gt(UO.Ub.nat(o, "slot"), Switch.num(Setting.get(s, UO.Ub.key.fired(uid, UO.Ub.field(o, "routine")), "0"), 0n)),
    s, uid, origin, UO.Ub.field(o, "routine"), UO.Ub.nat(o, "slot"))

def State.uber.body.k(fired: Bool, +s: State, +uid: String, +origin: String, +n: Nat, +stamp: String, +op: String, +key: String) -> State:
  match fired:
    case True{}:
      State.uber.fired(s, uid, origin, UO.Ub.json(op))
    case False{}:
      Bool.pick(State, String.is_empty(key), s, State.uber.reg(s, uid, key, stamp, op, origin, n))

def State.uber.body(+s: State, +uid: String, +origin: String, +n: Nat, +stamp: String, +op: String) -> State:
  State.uber.body.k(String.eq(UO.Ub.op.kind(op), "fired"), s, uid, origin, n, stamp, op, UO.Ub.op.key(op))

def State.uber.origins(+s: State, +uid: String, +origin: String, +n: Nat) -> State:
  Bool.pick(State, Nat.is_eq(n, 1n), State.setting(s, UO.Ub.key.origins(uid), UO.Ub.origins.add(Setting.get(s, UO.Ub.key.origins(uid), ""), origin)), s)

def State.uber.hlc(+s: State, +uid: String, +stamp: String) -> State:
  Bool.pick(State, UO.Ub.stamp.gt(stamp, Setting.get(s, UO.Ub.key.hlc(uid), "")), State.setting(s, UO.Ub.key.hlc(uid), stamp), s)

# an op that lost its register is kept as a nop at once
def State.uber.line(keep: Bool, +uid: String, +origin: String, +n: Nat, +stamp: String, +op: String, +at: Nat) -> String:
  match keep:
    case True{}:
      UO.Ub.line(uid, origin, n, stamp, op, at)
    case False{}:
      UO.Ub.line.nop(uid, origin, n, stamp)

def State.uber.keep.k(free: Bool, +s: State, +uid: String, +stamp: String, +op: String, +key: String) -> Bool:
  Bool.or(free, UO.Ub.newer(key, op, stamp, Setting.get(s, UO.Ub.key.reg(uid, key), ""), Setting.get(s, UO.Ub.key.stamp(uid, key), "")))

def State.uber.keep(+s: State, +uid: String, +stamp: String, +op: String) -> Bool:
  State.uber.keep.k(Bool.or(String.eq(UO.Ub.op.kind(op), "fired"), String.is_empty(UO.Ub.op.key(op))), s, uid, stamp, op, UO.Ub.op.key(op))

def State.uberop.k(ok: Bool, +s: State, +uid: String, +origin: String, +n: Nat, +stamp: String, +op: String, +at: Nat) -> State:
  match ok:
    case True{}:
      State.uber.body(State.uber.hlc(State.uber.origins(State.setting(State.setting(s, UO.Ub.key.vv(uid, origin), Nat.show(n)),
        UO.Ub.key.op(uid, origin, n), State.uber.line(State.uber.keep(s, uid, stamp, op), uid, origin, n, stamp, op, at)), uid, origin, n), uid, stamp), uid, origin, n, stamp, op)
    case False{}:
      s

def State.uberop(+s: State, +uid: String, +origin: String, +n: Nat, +stamp: String, +op: String, +at: Nat) -> State:
  State.uberop.k(Bool.and(Nat.is_eq(n, Nat.add(Switch.num(Setting.get(s, UO.Ub.key.vv(uid, origin), "0"), 0n), 1n)), Bool.not(String.is_empty(origin))),
    s, uid, origin, n, stamp, op, at)

# Mail is held behind Ext's list so the native state stays below arity 255.
def State.mail(s: State) -> ML.Box:
  match s:
    case State{_, _, _, _, _, _, _, _, _, _, bw}: Ext.mail_box(Ext.of(bw))

def State.with_mail(f: ML.Box -> ML.Box, s: State) -> State:
  match s:
    case State{ps, ts, es, n, tk, sg, qu, ak, dv, nv, bw}:
      State{ps, ts, es, n, tk, sg, qu, ak, dv, nv, [Ext.with_mail(f, Ext.of(bw))]}

def State.mail_end(state: TurnState, +s: State, +thread: String) -> State:
  match state:
    case Completed{}: State.with_mail((b => ML.Box.run_end(b, thread)), s)
    case Failed{}: State.with_mail((b => ML.Box.run_end(b, thread)), s)
    case Interrupted{}: State.with_mail((b => ML.Box.run_end(b, thread)), s)
    case _: s

def MailRoom.parts(xs: List<&2, String>) -> List<&2, String>:
  match xs:
    case Nil{}: ["person"]
    case h <> t: ("bot:" ++ h) <> MailRoom.parts(t)

def State.mail_room(+s: State, +id: String, +name: String, +members: String, +at: Nat) -> State:
  State.world((w => BT.World.room_set(w, id, name, members, at)),
    State.with_mail((b => ML.Box.conv_set(b, "room:" ++ id, name, "room", MailRoom.parts(BT.Room.members.of(members)), "", Nil{}, at)), s))

def State.bots(s: State) -> BT.World:
  match s:
    case State{_, _, _, _, _, _, _, _, _, _, bw}:
      State.bots.k(bw)

# Direct legacy rooms have no RoomSet; derive both participants once.
def MailRoom.dm_parts(xs: List<&2, String>, +b: ML.Box) -> List<&2, String>:
  match xs:
    case a <> c <> Nil{}: [ML.Box.sender(b, a), ML.Box.sender(b, c)]
    case _: Nil{}

def MailRoom.dm_seed(seed: Bool, +b: ML.Box, +room: String, +at: Nat) -> ML.Box:
  match seed:
    case False{}: b
    case True{}: ML.Box.conv_set(b, "room:" ++ room, room, "room", MailRoom.dm_parts(String.split(String.drop(room, 3n), ':'), b), "", Nil{}, at)

def MailRoom.post(+b: ML.Box, +id: String, +room: String, +from: String, +text: String, +at: Nat) -> ML.Box:
  ML.Box.mail(MailRoom.dm_seed(Bool.and(String.starts_with(room, "dm:"), String.is_empty(ML.Conv.id(ML.Box.conv(b, "room:" ++ room)))), b, room, at),
    ML.Mail{id, "room:" ++ room, ML.Box.sender(b, from), "room:" ++ room, "room", room, "fyi", ML.Mail.head(text), ML.Mail.text_size(text), at})

def State.mail_post.box(+on: Bool, +s: State, +id: String, +room: String, +from: String, +text: String, +at: Nat) -> State:
  match on:
    case True{}: s
    case False{}: State.with_mail((b => MailRoom.post(b, id, room, from, text, at)), s)

def State.mail_post(+s: State, +id: String, +room: String, +from: String, +text: String, +hop: Nat, +at: Nat) -> State:
  State.world((w => BT.World.posted(w, BT.Post{id, room, from, text, hop, at})),
    State.mail_post.box(String.eq(Setting.get(s, "mail.on", "off"), "on"), s, id, room, from, text, at))

# To-do ownership and merge state are scoped by the exact thread id.
def Todos.of(+s: State, +th: String) -> List<&2, TD.Todo>:
  TD.Todo.read(Setting.get(s, "todos." ++ th, ""))

def Todos.base(+s: State, +th: String) -> List<&2, TD.Todo>:
  TD.Todo.read(Setting.get(s, "todos.base." ++ th, Setting.get(s, "todos." ++ th, "")))

def Todos.visible(+s: State, +th: String) -> Bool:
  Bool.not(String.eq(Setting.get(s, "todos.visible." ++ th, "1"), "0"))

def Todos.put(+s: State, +th: String, +current: String, base: String, visible: String) -> State:
  State.setting(State.setting(State.setting(s, "todos." ++ th, current), "todos.base." ++ th, base), "todos.visible." ++ th, visible)

def Todos.agent(+s: State, +th: String, +raw: String) -> State:
  Todos.put(s, th, TD.Todo.show(TD.Todo.merge(Todos.base(s, th), Todos.of(s, th), TD.Todo.read(raw))), raw, "1")

def Todos.snapshot(+s: State, +th: String) -> String:
  J.Json.show(J.Json.obj([J.KV{"current", J.Str{Setting.get(s, "todos." ++ th, "")}},
    J.KV{"base", J.Str{Setting.get(s, "todos.base." ++ th, Setting.get(s, "todos." ++ th, ""))}},
    J.KV{"visible", J.Str{Setting.get(s, "todos.visible." ++ th, "1")}}]))

def Todos.plan(p: TD.TPlan, s: State, th: String) -> State:
  match p:
    case TD.TPlan{base, current, fresh}: Todos.put(s, th, TD.Todo.show(current), TD.Todo.show(base), Bool.pick(String, fresh, "1", "0"))

def Todos.changed(+s: State, +th: String, who: String, +data: String) -> State:
  match who:
    case "tools":
      Todos.plan(TD.TPlan.tools(J.Json.elems(Maybe.default(&2, J.Json, J.Json.parse(data), J.Null{})), TD.TPlan{Todos.base(s, th), Todos.of(s, th), Todos.visible(s, th)}), s, th)
    case "person":
      Todos.put(s, th, TD.Todo.show(TD.Todo.edit(Todos.of(s, th), Maybe.default(&2, J.Json, J.Json.parse(data), J.Null{}))), TD.Todo.show(Todos.base(s, th)), "1")
    case "snapshot":
      +p = Maybe.default(&2, J.Json, J.Json.parse(data), J.Null{})
      Todos.put(s, th, J.Json.get_str(p, "current", ""), J.Json.get_str(p, "base", ""), J.Json.get_str(p, "visible", "0"))
    case _: Todos.agent(s, th, data)

def Todos.turn.end(done: Bool, +s: State, +th: String) -> State:
  match done:
    case True{}: Todos.put(s, th, "", "", "0")
    case False{}: s

def Todos.turn.start(was: Bool, s: State, th: String) -> State:
  match was:
    case True{}: s
    case False{}: State.setting(s, "todos.visible." ++ th, "0")

def Todos.turn(st: TurnState, +s: State, +th: String) -> State:
  match st:
    case Running{}: Todos.turn.start(State.running.k(State.thread.get(s, th)), s, th)
    case _: Todos.turn.end(TD.Todo.complete(Todos.of(s, th)), s, th)

def State.apply.go(s: State, e: Change) -> State:
  match e:
    case ProjectCreated{id, title, root, at}:
      State.put_project(s, Project{id, title, root, at, False{}})
    case ProjectRemoved{+id, _}:
      State.remove_project(s, id)
    case ThreadCreated{id, project, title, env, branch, worktree, provider, model, +at}:
      State.put_thread(s, Thread{id, project, title, at, at, Idle{}, 0n, False{}, NoOverride{}, 0n, 0n, False{}, env, branch, worktree, provider, model, FullAccess{}, DefaultMode{}, "", False{}})
    case ThreadRenamed{+id, +title, _}:
      State.thread((t => Thread.renamed(title, t)), s, id)
    case MessagePosted{+thread, msg, role, text, +at}:
      State.message(s, thread, msg, role, text, at)
    case TurnChanged{+thread, +state, +at}:
      State.mail_end(state, State.thread((t => Thread.turned(state, at, t)), Todos.turn(state, s, thread), thread), thread)
    case PendingChanged{+thread, count, _}:
      State.thread((t => Thread.pending_set(count, t)), s, thread)
    case ActivityLogged{thread, act, tone, kind, summary, at}:
      State.post(s, Act{act, thread, tone, kind, summary, at})
    case ThreadSettled{+id, manual, +at}:
      State.thread((t => Thread.settled(manual, at, t)), s, id)
    case ThreadUnsettled{+id, +at}:
      State.thread((t => Thread.unsettled(at, t)), s, id)
    case ThreadWoke{+id, +at}:
      State.thread((t => Thread.woke(at, t)), s, id)
    case ThreadArchived{+id, b, _}:
      State.thread((t => Thread.archived_set(b, t)), s, id)
    case ThreadPinned{+id, b, _}:
      State.thread((t => Thread.pinned_set(b, t)), s, id)
    case ThreadDeleted{+id, _}:
      State.thread((t => Thread.deleted_set(t)), s, id)
    case WorktreeSet{+id, b, w, _}:
      State.thread((t => Thread.worktree_set(b, w, t)), s, id)
    case SessionSet{+id, se, _}:
      State.thread((t => Thread.session_set(se, t)), s, id)
    case ModesSet{+id, r, i, m, _}:
      State.thread((t => Thread.modes_set(r, i, m, t)), s, id)
    case TaskDelegated{id, parent, child, depth, provider, model, _}:
      State.put_task(s, Task{id, parent, child, depth, TaskQueued{}, "", False{}, provider, model})
    case TaskStateSet{+id, st, summary, _}:
      State.task((t => Task.step(t, st, summary)), s, id)
    case TaskDelivered{+id, _}:
      State.task((t => Task.delivered_set(t)), s, id)
    case SettingSet{k, v, _}:
      State.setting(s, k, v)
    case TurnQueued{th, msg, text, _}:
      State.enqueue(s, th, msg, text)
    case QueueTaken{+th, _}:
      State.world((w => BT.World.taken(w, th)), State.take(s, th))
    case AskOpened{id, th, k, tool, d, o, _}:
      State.ask(s, Ask{id, th, k, tool, d, o, True{}, ""})
    case AskClosed{+id, a, _}:
      State.ask.close(s, id, a)
    case DeviceRegistered{+tok, p, k, th, e, b, at}:
      State.register(s, tok, p, k, th, e, b, at)
    case DeviceDropped{tok, _}:
      State.unregister(s, tok)
    case ThreadSnoozed{+id, +until, _}:
      State.nav((x => Nav.snoozed(x, until)), s, id)
    case ThreadOrdered{+id, +section, +key, _}:
      State.nav((x => Nav.keyed(x, section, key)), s, id)
    case WorktreeSkipped{+id, _}:
      State.thread((t => Thread.local(t)), s, id)
    case ThreadMerged{+id, url, +at}:
      State.setting(State.thread((t => Thread.settled(True{}, at, t)), s, id), Merge.key(id), url)
    case WorktreeRemoved{+id, _}:
      State.thread((t => Thread.unrooted(t)), s, id)
    case UsageSet{+th, used, window, _}:
      State.usage(s, th, used, window)
    case TurnUsage{+th, pv, mo, key, i, r, w, o, t, c, known, basis, at}:
      State.tokens(s, th, pv, mo, i, r, w, o, t, c, known, key, basis, at)
    case DeployAsked{+by, _, _, +cover, _}:
      Ship.fold.asked(s, by, cover)
    case DeployBuilt{+commit, +state, _, +detail, _}:
      Ship.fold.built(s, commit, state, detail)
    case BotSet{+id, +name, thread, look, persona, at}:
      State.world((w => BT.World.bot_set(w, id, name, thread, look, persona, at)), State.with_mail((b => ML.Box.bot_named(b, id, name)), s))
    case BotDeleted{+id, _}:
      State.world((w => BT.World.bot_deleted(w, id)), State.with_mail((b => ML.Box.bot_deleted(b, id)), s))
    case RoomSet{id, name, members, at}:
      State.mail_room(s, id, name, members, at)
    case RoomDeleted{id, _}:
      State.world((w => BT.World.room_deleted(w, id)), s)
    case RoomPosted{id, room, from, text, hop, at}:
      State.mail_post(s, id, room, from, text, hop, at)
    case HookSet{id, bot, name, revoked, at}:
      State.world((w => BT.World.hook_set(w, id, bot, name, revoked, at)), s)
    case HookHit{id, at}:
      State.world((w => BT.World.hook_hit(w, id, at)), s)
    case RoutineSet{id, bot, name, cron, prompt, on, at}:
      State.world((w => BT.World.routine_set(w, id, bot, name, cron, prompt, on, at)), s)
    case RoutineDeleted{id, _}:
      State.world((w => BT.World.routine_deleted(w, id)), s)
    case RoutineFired{id, at}:
      State.world((w => BT.World.routine_fired(w, id, at)), s)
    case MemorySaved{bot, key, kind, text, tags, at}:
      State.world((w => BT.World.mem_saved(w, bot, key, kind, text, tags, at)), s)
    case MemoryForgot{bot, key, _}:
      State.world((w => BT.World.mem_forgot(w, bot, key)), s)
    case TaskSet{bot, slug, json, _}:
      State.world((w => BT.World.task_set(w, bot, slug, json)), s)
    case TaskDeleted{bot, slug, _}:
      State.world((w => BT.World.task_del(w, bot, slug)), s)
    case MessageFrom{thread, msg, from, kind, ref, _}:
      State.with_mail((b => ML.Box.origin(b, thread, msg, ML.Origin{from, kind, ref})), s)
    case ConvSet{id, subject, kind, parts, project, tags, at}:
      State.with_mail((b => ML.Box.conv_set(b, id, subject, kind, BT.Room.members.of(parts), project, BT.Room.members.of(tags), at)), s)
    case MailPosted{id, conv, from, to, kind, ref, want, head, size, at}:
      State.with_mail((b => ML.Box.mail(b, ML.Mail{id, conv, from, to, kind, ref, want, head, size, at})), s)
    case MailMarked{bot, conv, status, seen, until, at}:
      State.with_mail((b => ML.Box.marked(b, bot, conv, status, seen, until)), s)
    case MailTagged{bot, conv, tags, _}:
      State.with_mail((b => ML.Box.tagged(b, bot, conv, BT.Room.members.of(tags))), s)
    case RunSet{thread, bot, conv, mail, at}:
      State.with_mail((b => ML.Box.run_set(b, thread, bot, conv, mail, at)), s)
    case CharterSet{bot, text, _, _}:
      State.with_mail((b => ML.Box.charter_set(b, bot, text)), s)
    case DecisionLogged{id, bot, conv, run, text, reason, at}:
      State.with_mail((b => ML.Box.decision(b, ML.Decision{id, bot, conv, run, text, reason, at})), s)
    case SpaceSet{bot, spec, _}:
      State.world((w => BT.World.space_set(w, bot, spec)), s)
    case PeerSet{id, name, url, revoked, at}:
      State.world((w => BT.World.peer_set(w, id, name, url, revoked, at)), s)
    case BotWoke{bot, hop, _}:
      State.woke(s, bot, hop)
    case SpacePageSet{bot, html, at}:
      State.world((w => BT.World.spage_set(w, bot, html, at)), s)
    case WorkSet{+th, +kind, rows, _}:
      State.setting(s, kind ++ "." ++ th, rows)
    case UberSet{bot, uid, _}:
      State.setting(State.setting(s, "uber.bot", bot), "uber.uid", uid)
    case UberOp{+uid, +origin, +n, +stamp, +op, at}:
      State.uberop(s, uid, origin, n, stamp, op, at)
    case ThreadLaunched{+id, +by, report, _}:
      Launch.fold(s, id, by, report)
    case ThreadParentSet{id, parent, _}:
      State.setting(s, "thread.parent." ++ id, parent)
    case WakeSet{id, by, kind, target, events, once, note, at}:
      Wake.fold(s, id, by, kind, target, events, once, note, at)
    case WakeDropped{id, _}:
      Wake.drop(s, id)
    case LockChanged{name, l, _}:
      Locks.fold(s, name, l)
    case RoutinePlan{id, once, tz, kind, _}:
      State.world((w => BT.World.routine_plan(w, id, once, tz, kind)), s)
    case RoutineSlot{id, slot, _}:
      State.world((w => BT.World.routine_slot(w, id, slot)), s)
    case RoutineDone{id, _}:
      State.world((w => BT.World.routine_done(w, id)), s)
    case Reminded{+id, _, thread, text, +at}:
      State.post(s, Act{"remind:" ++ id ++ ":" ++ Nat.show(at), thread, "remind", "reminder", text, at})
    case TodosSet{+th, +todos, _}:
      Todos.put(s, th, todos, todos, "1")
    case TodosChanged{th, who, data, _}:
      Todos.changed(s, th, who, data)
    case ProviderSet{+id, +p, m, _}:
      State.switch(s, id, p, m)
    case ThreadForked{+id, +src, _}:
      State.setting(s, Fork.key(id), src)
    case QueueEdited{+th, +msg, +text, _}:
      State.queue((qs => Queue.edit(qs, msg, text)), s, th)
    case QueueRemoved{+th, +msg, _}:
      State.queue((qs => Queue.drop(qs, msg)), s, th)
    case QueueMoved{+th, +msg, to, _}:
      State.queue((qs => Queue.move(qs, msg, to)), s, th)
    case QueueTakenOne{+th, +msg, _}:
      State.queue((qs => Queue.drop(qs, msg)), s, th)
    case ProjectOrdered{+id, +key, _}:
      State.nav((x => Nav.keyed(x, "active", key)), s, Nav.project(id))

# one event folded into the read model; every event bumps the sequence
def State.apply(s: State, e: Change) -> State:
  State.tick(State.apply.go(s, e))

# the read model after a log, oldest event first
def State.replay(es: List<&2, Change>, s: State) -> State:
  match es:
    case Nil{}:
      s
    case e <> t:
      State.replay(t, State.apply(s, e))

# two threads, one writing to the other (the laws idx_* read it)
def Idx.demo() -> State:
  State.replay([ThreadCreated{"t1", "p1", "A", Local{}, "", "", "claude", "", 1n}, ThreadCreated{"t2", "p1", "B", Local{}, "", "", "claude", "", 1n},
    MessagePosted{"t1", "m1", User{}, "a", 2n}, ActivityLogged{"t2", "u1", "tool", "Bash", "ls", 3n}, MessagePosted{"t2", Link.id("t1", "7"), User{}, "hi", 4n},
    MessagePosted{"t2", "m2", Assistant{}, "b", 5n}, MessagePosted{"t1", "m3", Assistant{}, "c", 6n}], State.empty())

# Sidebar
# -------

# a project's threads split by shelf, each list in display order
type Shelves is Data:
  Shelves{pinned: List<&2, Thread>, active: List<&2, Thread>, settled: List<&2, Thread>, hidden: List<&2, Thread>, snoozed: List<&2, Thread>}

def Shelves.put(sh: Shelf, t: Thread, s: Shelves) -> Shelves:
  match sh s:
    case Pinned{} Shelves{p, a, st, h, sn}:
      Shelves{t <> p, a, st, h, sn}
    case Active{} Shelves{p, a, st, h, sn}:
      Shelves{p, t <> a, st, h, sn}
    case Settled{} Shelves{p, a, st, h, sn}:
      Shelves{p, a, t <> st, h, sn}
    case Hidden{} Shelves{p, a, st, h, sn}:
      Shelves{p, a, st, t <> h, sn}
    case Snoozed{} Shelves{p, a, st, h, sn}:
      Shelves{p, a, st, h, t <> sn}

# every thread lands on exactly one shelf (law Sidebar.split_count)
def Sidebar.split(ts: List<&2, Thread>) -> Shelves:
  match ts:
    case Nil{}:
      Shelves{Nil{}, Nil{}, Nil{}, Nil{}, Nil{}}
    case +t <> rest:
      Shelves.put(Shelf.of(t), t, Sidebar.split(rest))

def Shelves.count(s: Shelves) -> Nat:
  Shelves{p, a, st, h, sn} = s
  Nat.add(List.length(&2, Thread, p), Nat.add(List.length(&2, Thread, a), Nat.add(List.length(&2, Thread, st), Nat.add(List.length(&2, Thread, h), List.length(&2, Thread, sn)))))

def Thread.turn(t: Thread) -> TurnState:
  match t:
    case Thread{_, _, _, _, _, tu, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _}:
      tu

def Thread.provider(t: Thread) -> String:
  match t:
    case Thread{_, _, _, _, _, _, _, _, _, _, _, _, _, _, _, pr, _, _, _, _, _}:
      pr

def Thread.id(t: Thread) -> String:
  match t:
    case Thread{i, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _}:
      i

def Thread.hidden(t: Thread) -> Bool:
  match t:
    case Thread{_, _, _, _, _, _, _, ar, _, _, _, _, _, _, _, _, _, _, _, _, de}:
      Bool.or(ar, de)

def Shelf.at.k(hidden: Bool, snoozed: Bool, sh: Shelf) -> Shelf:
  match hidden snoozed:
    case False{} True{}:
      Snoozed{}
    case _ _:
      sh

# the shelf with the snooze: a thread snoozed past now sits on Snoozed
# (above settled, whatever its pin or settle state); it goes back where it
# was when the snooze ends
def Shelf.at(+t: Thread, +until: Nat, +now: Nat) -> Shelf:
  Shelf.at.k(Thread.hidden(t), Snooze.on(now, until), Shelf.of(t))

# every thread lands on exactly one shelf, snoozed ones included (law
# sidebar_split_at_count)
def Sidebar.split.at(ts: List<&2, Thread>, +navs: Map<&2, Nav>, +now: Nat) -> Shelves:
  match ts:
    case Nil{}:
      Shelves{Nil{}, Nil{}, Nil{}, Nil{}, Nil{}}
    case +t <> rest:
      Shelves.put(Shelf.at(t, Nav.until(Nav.get(navs, Thread.id(t))), now), t, Sidebar.split.at(rest, navs, now))

# where a thread sorts on the active shelf: created, or brought back
def Thread.rank(t: Thread) -> Nat:
  Thread{_, _, _, +c, _, _, _, _, _, _, +ua, _, _, _, _, _, _, _, _, _, _} = t
  Time.max(c, ua)

def Thread.settled_rank(t: Thread) -> Nat:
  Thread{_, _, _, _, _, _, _, _, _, +sa, _, _, _, _, _, _, _, _, _, _, _} = t
  sa

# newest first; activity does not reorder a shelf
def Thread.before(a: Thread, b: Thread) -> Bool:
  Nat.is_ge(Thread.rank(a), Thread.rank(b))

def Thread.settled_before(a: Thread, b: Thread) -> Bool:
  Nat.is_ge(Thread.settled_rank(a), Thread.settled_rank(b))

def Sidebar.order(s: Shelves) -> Shelves:
  Shelves{p, a, st, h, sn} = s
  Shelves{List.sort(~Thread, ~Thread.before, p), List.sort(~Thread, ~Thread.before, a), List.sort(~Thread, ~Thread.settled_before, st), h, sn}

def Thread.in_project(+pid: String, t: Thread) -> Bool:
  Thread{_, +p, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _} = t
  String.eq(p, pid)

def Thread.filter_project.pick(keep: Bool, t: Thread, rest: List<&2, Thread>) -> List<&2, Thread>:
  match keep:
    case True{}:
      t <> rest
    case False{}:
      rest

def Thread.filter_project(ts: List<&2, Thread>, +pid: String) -> List<&2, Thread>:
  match ts:
    case Nil{}:
      Nil{}
    case +t <> rest:
      Thread.filter_project.pick(Thread.in_project(pid, t), t, Thread.filter_project(rest, pid))

# how many changes have been folded in
def State.seq(s: State) -> Nat:
  match s:
    case State{_, _, _, n, _, _, _, ak, _, _, _}:
      n

# Order keys
# ----------
# A manual place on a shelf is a key: a binary fraction 0.b1b2b3... read
# with zeros after its last digit (so "1" and "10" are the same place).
# Between any two different places there is another (laws key_mid_above,
# key_mid_below), and past any place there is a later one (law key_after),
# so dragging a thread never renumbers its neighbours. Keys travel as hex,
# four digits a character.

def Key.cmpz(t: List<&2, Bool>) -> Cmp:
  match t:
    case Nil{}:
      EQ{}
    case False{} <> r:
      Key.cmpz(r)
    case True{} <> _:
      LT{}

def Key.cmpz_r(s: List<&2, Bool>) -> Cmp:
  match s:
    case Nil{}:
      EQ{}
    case False{} <> r:
      Key.cmpz_r(r)
    case True{} <> _:
      GT{}

# two places in order
def Key.cmp(a: List<&2, Bool>, b: List<&2, Bool>) -> Cmp:
  match a:
    case Nil{}:
      Key.cmpz(b)
    case x <> s:
      match x b:
        case False{} Nil{}:
          Key.cmpz_r(s)
        case True{} Nil{}:
          GT{}
        case False{} False{} <> t:
          Key.cmp(s, t)
        case False{} True{} <> _:
          LT{}
        case True{} False{} <> _:
          GT{}
        case True{} True{} <> t:
          Key.cmp(s, t)

def Key.lt(a: List<&2, Bool>, b: List<&2, Bool>) -> Bool:
  Cmp.is_lt(Key.cmp(a, b))

# a place after s
def Key.after(s: List<&2, Bool>) -> List<&2, Bool>:
  match s:
    case Nil{}:
      [True{}]
    case False{} <> _:
      [True{}]
    case True{} <> r:
      True{} <> Key.after(r)

# a place between 0 and t
def Key.midz(t: List<&2, Bool>) -> List<&2, Bool>:
  match t:
    case Nil{}:
      [True{}]
    case False{} <> r:
      False{} <> Key.midz(r)
    case True{} <> _:
      [False{}, True{}]

# a place between a and b (when a is before b)
def Key.mid(a: List<&2, Bool>, b: List<&2, Bool>) -> List<&2, Bool>:
  match a:
    case Nil{}:
      Key.midz(b)
    case x <> s:
      match x b:
        case _ Nil{}:
          [True{}]
        case False{} False{} <> t:
          False{} <> Key.mid(s, t)
        case False{} True{} <> _:
          False{} <> Key.after(s)
        case True{} False{} <> _:
          [True{}]
        case True{} True{} <> t:
          True{} <> Key.mid(s, t)

def Key.bit(+v: U32, +m: U32) -> Bool:
  U32.is_ne(U32.and(v, m), 0)

def Key.hexval(+c: U32) -> U32:
  Bool.pick(U32, Bool.and(U32.is_ge(c, 48), U32.is_le(c, 57)), (c - 48 : U32),
    Bool.pick(U32, Bool.and(U32.is_ge(c, 97), U32.is_le(c, 102)), (c - 87 : U32), 0))

def Key.nibble(+v: U32, rest: List<&2, Bool>) -> List<&2, Bool>:
  Key.bit(v, 8) <> Key.bit(v, 4) <> Key.bit(v, 2) <> Key.bit(v, 1) <> rest

# a key from its hex ("" is no key)
def Key.read(s: String) -> List<&2, Bool>:
  match s:
    case SNil{}:
      Nil{}
    case SCon{Chr{+c}, t}:
      Key.nibble(Key.hexval(c), Key.read(t))

def Key.b(b: Bool, +v: U32) -> U32:
  match b:
    case True{}:
      v
    case False{}:
      0

def Key.hex(a: Bool, b: Bool, c: Bool, d: Bool) -> Char:
  +v = {(Key.b(a, 8) + Key.b(b, 4) + Key.b(c, 2) + Key.b(d, 1) : U32) : U32}
  Chr{Bool.pick(U32, U32.is_lt(v, 10), (v + 48 : U32), (v + 87 : U32))}

def Key.show(bs: List<&2, Bool>) -> String:
  match bs:
    case Nil{}:
      SNil{}
    case a <> Nil{}:
      SCon{Key.hex(a, False{}, False{}, False{}), SNil{}}
    case a <> b <> Nil{}:
      SCon{Key.hex(a, b, False{}, False{}), SNil{}}
    case a <> b <> c <> Nil{}:
      SCon{Key.hex(a, b, c, False{}), SNil{}}
    case a <> b <> c <> d <> t:
      SCon{Key.hex(a, b, c, d), Key.show(t)}

# Manual order
# ------------
# Dragging a thread gives the hub the shelf's new order. Threads keep their
# keys where these already fall in order; the rest get a key between their
# neighbours (or after the one above). Threads past the moved one that have
# no key, with none below them keyed either, keep none: they already sit
# below the keyed ones, by recency.

type OKey is Data:
  OKey{id: String, key: String}

def Order.unkeyed(xs: List<&2, OKey>) -> Bool:
  match xs:
    case Nil{}:
      True{}
    case OKey{_, +k} <> t:
      Bool.and(String.is_empty(k), Order.unkeyed(t))

def Order.next(xs: List<&2, OKey>) -> List<&2, Bool>:
  match xs:
    case Nil{}:
      Nil{}
    case OKey{_, k} <> _:
      Key.read(k)

# what one thread's key becomes: skipped, kept, or the new key
type OStep is Data:
  OStep{skip: Bool, keep: Bool, key: List<&2, Bool>}

def Order.fresh(usable: Bool, +prev: List<&2, Bool>, +nxt: List<&2, Bool>) -> List<&2, Bool>:
  match usable:
    case True{}:
      Key.mid(prev, nxt)
    case False{}:
      Key.after(prev)

def Order.step(+k: List<&2, Bool>, +nxt: List<&2, Bool>, +prev: List<&2, Bool>, +skip: Bool) -> OStep:
  +keyed = Bool.not(List.is_empty(&2, Bool, k))
  +nkeyed = Bool.not(List.is_empty(&2, Bool, nxt))
  OStep{skip, Bool.and(Bool.and(keyed, Key.lt(prev, k)), Bool.or(Bool.not(nkeyed), Key.lt(k, nxt))),
    Order.fresh(Bool.and(nkeyed, Key.lt(prev, nxt)), prev, nxt)}

def Order.prev(o: OStep, +k: List<&2, Bool>, +prev: List<&2, Bool>) -> List<&2, Bool>:
  match o:
    case OStep{True{}, _, _}:
      prev
    case OStep{False{}, True{}, _}:
      k
    case OStep{False{}, False{}, n}:
      n

def Order.emit(o: OStep, +id: String, rest: List<&2, OKey>) -> List<&2, OKey>:
  match o:
    case OStep{False{}, False{}, n}:
      OKey{id, Key.show(n)} <> rest
    case _:
      rest

def Order.go(xs: List<&2, OKey>, +moved: String, +prev: List<&2, Bool>, +past: Bool) -> List<&2, OKey>:
  match xs:
    case Nil{}:
      Nil{}
    case OKey{+id, +k} <> +rest:
      +o = Order.step(Key.read(k), Order.next(rest), prev, Bool.and(Bool.and(past, String.is_empty(k)), Order.unkeyed(rest)))
      Order.emit(o, id, Order.go(rest, moved, Order.prev(o, Key.read(k), prev), Bool.or(past, String.eq(id, moved))))

# the key changes that put a shelf in this order (xs: its threads in the new
# order with their keys now; moved: the thread dragged)
def Order.plan(+xs: List<&2, OKey>, +moved: String) -> List<&2, OKey>:
  Order.go(xs, moved, Nil{}, False{})

def Order.remove(xs: List<&2, String>, +x: String) -> List<&2, String>:
  match xs:
    case Nil{}:
      Nil{}
    case +h <> +t:
      Bool.pick(List<&2, String>, String.eq(h, x), t, h <> Order.remove(t, x))

def Order.insert(i: Nat, xs: List<&2, String>, +x: String) -> List<&2, String>:
  match i xs:
    case 0n _:
      x <> xs
    case 1n+j h <> t:
      h <> Order.insert(j, t, x)
    case 1n+j Nil{}:
      [x]

def Order.index(xs: List<&2, String>, +x: String, +i: Nat) -> Nat:
  match xs:
    case Nil{}:
      i
    case +h <> t:
      Bool.pick(Nat, String.eq(h, x), i, Order.index(t, x, 1n+i))

# a shelf's ids with x moved to where the thread at index `to` was
def Order.move(+xs: List<&2, String>, +x: String, +to: Nat) -> List<&2, String>:
  Order.insert(to, Order.remove(xs, x), x)

# Sidebar order with keys and snoozes
# -----------------------------------
# Pinned and active: threads with a key first, by key; the rest by
# max(created, unsettled), newest first. Snoozed: the soonest to wake
# first. Settled: by settle time, newest first.

type Slot is Data:
  Slot{keyed: Bool, key: List<&2, Bool>, rank: Nat, t: Thread}

def Slot.before(a: Slot, b: Slot) -> Bool:
  match a b:
    case Slot{True{}, ka, _, _} Slot{True{}, kb, _, _}:
      Cmp.is_le(Key.cmp(ka, kb))
    case Slot{True{}, _, _, _} Slot{False{}, _, _, _}:
      True{}
    case Slot{False{}, _, _, _} Slot{True{}, _, _, _}:
      False{}
    case Slot{False{}, _, +ra, _} Slot{False{}, _, +rb, _}:
      Nat.is_ge(ra, rb)

def Slot.soon(a: Slot, b: Slot) -> Bool:
  match a b:
    case Slot{_, _, +ra, _} Slot{_, _, +rb, _}:
      Nat.is_le(ra, rb)

def Slot.keyed(+k: List<&2, Bool>, +rank: Nat, t: Thread) -> Slot:
  Slot{Bool.not(List.is_empty(&2, Bool, k)), k, rank, t}

def Slots.of(ts: List<&2, Thread>, +navs: Map<&2, Nav>, +section: String) -> List<&2, Slot>:
  match ts:
    case Nil{}:
      Nil{}
    case +t <> rest:
      Slot.keyed(Key.read(Nav.key(Nav.get(navs, Thread.id(t)), section)), Thread.rank(t), t) <> Slots.of(rest, navs, section)

def Slots.wake(ts: List<&2, Thread>, +navs: Map<&2, Nav>) -> List<&2, Slot>:
  match ts:
    case Nil{}:
      Nil{}
    case +t <> rest:
      Slot{False{}, Nil{}, Nav.until(Nav.get(navs, Thread.id(t))), t} <> Slots.wake(rest, navs)

def Slots.threads(xs: List<&2, Slot>) -> List<&2, Thread>:
  match xs:
    case Nil{}:
      Nil{}
    case Slot{_, _, _, t} <> rest:
      t <> Slots.threads(rest)

def Sidebar.order.at(s: Shelves, +navs: Map<&2, Nav>) -> Shelves:
  Shelves{p, a, st, h, sn} = s
  Shelves{Slots.threads(List.sort(~Slot, ~Slot.before, Slots.of(p, navs, "pinned"))),
    Slots.threads(List.sort(~Slot, ~Slot.before, Slots.of(a, navs, "active"))),
    List.sort(~Thread, ~Thread.settled_before, st), h,
    Slots.threads(List.sort(~Slot, ~Slot.soon, Slots.wake(sn, navs)))}

# a project's threads on their shelves, in sidebar order, at time now
def Sidebar.at(ts: List<&2, Thread>, +navs: Map<&2, Nav>, +now: Nat) -> Shelves:
  Sidebar.order.at(Sidebar.split.at(ts, navs, now), navs)

def Archive.keep(keep: Bool, t: Thread, rest: List<&2, Thread>) -> List<&2, Thread>:
  match keep:
    case True{}:
      t <> rest
    case False{}:
      rest

def Archive.go(ts: List<&2, Thread>) -> List<&2, Thread>:
  match ts:
    case Nil{}:
      Nil{}
    case Thread{id, p, ti, c, a, tu, pe, +ar, o, sa, ua, pi, en, br, wt, pr, mo, rm, im, se, +de} <> rest:
      Archive.keep(Bool.and(ar, Bool.not(de)), Thread{id, p, ti, c, a, tu, pe, ar, o, sa, ua, pi, en, br, wt, pr, mo, rm, im, se, de}, Archive.go(rest))

# the archived (not deleted) threads of a hidden shelf, newest first
def Archive.of(hidden: List<&2, Thread>) -> List<&2, Thread>:
  List.sort(~Thread, ~Thread.before, Archive.go(hidden))

# Sidebar status
# --------------
# The dot on a thread's row says the most urgent thing about it: an
# approval waiting > a question or plan waiting > a turn working (or
# starting: a message waits for its worktree) > a failed turn > ready (laws
# status_*). Nothing is ever queued.

type Status is Data:
  StApproval{}
  StInput{}
  StWorking{}
  StFailed{}
  StReady{}
  # the agent's turn is over and its subagents are still at work
  StWaiting{}

def Status.of.q(starting: Bool) -> Status:
  match starting:
    case True{}:
      StWorking{}
    case False{}:
      StReady{}

def Status.of.t(t: TurnState, starting: Bool) -> Status:
  match t:
    case Running{}:
      StWorking{}
    case Failed{}:
      StFailed{}
    case _:
      Status.of.q(starting)

def Status.of.i(input: Bool, t: TurnState, starting: Bool) -> Status:
  match input:
    case True{}:
      StInput{}
    case False{}:
      Status.of.t(t, starting)

def Status.of(approval: Bool, input: Bool, t: TurnState, starting: Bool) -> Status:
  match approval:
    case True{}:
      StApproval{}
    case False{}:
      Status.of.i(input, t, starting)

# how urgent a status is (5: most)
def Status.rank(s: Status) -> Nat:
  match s:
    case StApproval{}:
      5n
    case StInput{}:
      4n
    case StWorking{}:
      3n
    case StFailed{}:
      2n
    case StReady{}:
      0n
    case StWaiting{}:
      3n

def Status.name(s: Status) -> String:
  match s:
    case StApproval{}:
      "approval"
    case StInput{}:
      "input"
    case StWorking{}:
      "working"
    case StFailed{}:
      "failed"
    case StReady{}:
      "ready"
    case StWaiting{}:
      "waiting"

# a thread whose subagents are at work (any) waits on them, unless it
# already asks for more (approval, input, a turn working)
def Status.wait(any: Bool, s: Status) -> Status:
  match any s:
    case True{} StFailed{}:
      StWaiting{}
    case True{} StReady{}:
      StWaiting{}
    case _ _:
      s

# the same, except that a failed turn stays failed with work out: the
# person has to act on it
def Status.wait.f(any: Bool, s: Status) -> Status:
  match s:
    case StFailed{}:
      StFailed{}
    case _:
      Status.wait(any, s)

# Thread phase
# ------------
# The line at the end of a thread's timeline says what the thread is doing
# now: an approval waiting > a question or plan waiting > a turn working >
# child tasks still running after the turn (monitoring) > a failed turn >
# a finished turn > a stopped turn > nothing yet (laws
# phase_*).

type Phase is Data:
  PhApproval{}
  PhInput{}
  PhWorking{}
  PhMonitoring{n: Nat}
  PhFailed{}
  PhComplete{}
  PhStopped{}
  PhIdle{}

def Phase.of.t(t: TurnState, starting: Bool) -> Phase:
  match t starting:
    case Failed{} _:
      PhFailed{}
    case _ True{}:
      PhWorking{}
    case Completed{} False{}:
      PhComplete{}
    case Interrupted{} False{}:
      PhStopped{}
    case _ False{}:
      PhIdle{}

# (a failed turn says so before the work it left: the person has to act)
def Phase.of.w(+tasks: Nat, t: TurnState, starting: Bool) -> Phase:
  match t:
    case Running{}:
      PhWorking{}
    case Failed{}:
      PhFailed{}
    case _:
      Bool.pick(Phase, Nat.is_gt(tasks, 0n), PhMonitoring{tasks}, Phase.of.t(t, starting))

def Phase.of.i(input: Bool, +tasks: Nat, t: TurnState, starting: Bool) -> Phase:
  match input:
    case True{}:
      PhInput{}
    case False{}:
      Phase.of.w(tasks, t, starting)

# tasks: the thread's child tasks not yet finished
def Phase.of(approval: Bool, input: Bool, +tasks: Nat, t: TurnState, starting: Bool) -> Phase:
  match approval:
    case True{}:
      PhApproval{}
    case False{}:
      Phase.of.i(input, tasks, t, starting)

# the word for a phase (the web's class, the color's key)
def Phase.name(p: Phase) -> String:
  match p:
    case PhApproval{}:
      "approval"
    case PhInput{}:
      "input"
    case PhWorking{}:
      "working"
    case PhMonitoring{_}:
      "monitoring"
    case PhFailed{}:
      "failed"
    case PhComplete{}:
      "complete"
    case PhStopped{}:
      "stopped"
    case PhIdle{}:
      "idle"

def Phase.tasks(+n: Nat) -> String:
  Nat.show(n) ++ Bool.pick(String, Nat.is_eq(n, 1n), " task", " tasks")

# what the line says ("" for a thread with nothing to say)
def Phase.label(p: Phase) -> String:
  match p:
    case PhApproval{}:
      "Waiting for approval"
    case PhInput{}:
      "Waiting for input"
    case PhWorking{}:
      "Working…"
    case PhMonitoring{+n}:
      "Monitoring " ++ Phase.tasks(n)
    case PhFailed{}:
      "Failed"
    case PhComplete{}:
      "Complete"
    case PhStopped{}:
      "Stopped"
    case PhIdle{}:
      ""

# Subagents
# ---------
# The subagents, background shells and monitors an agent has at work
# (Claude's task_started, task_progress and task_notification lines), kept
# as the setting "bg.<thread>": one "id	doing" line each, or
# "id	doing	call	title	kind" (call: the tool call that started it;
# title: what it was asked to do; kind: Bg.kind's word for its task type).
# They can run on after the agent's own turn ends, so the thread shows it
# monitoring them.
#
# What each one does, step by step, goes to its own log: entries of the
# thread Bg.log(thread) (no thread id has '@'), off the thread's timeline,
# each an Act whose tone is the call it belongs to and whose kind is the
# tool it ran ("say" for what it wrote, "start:<kind>" and "end" for its
# bounds).

def Bg.key(+th: String) -> String:
  "bg." ++ th

def Bg.log(+th: String) -> String:
  "@sub:" ++ th

# a subagent's id or what it does, on one line with no tab
def Bg.clean(+t: String) -> String:
  String.join(String.split(String.join(String.split(t, '\t'), " "), '\n'), " ")

# a command or path kept whole on one tab-free line: "\e" then the text with
# backslash, tab, newline and return written \b \t \n \r (a row written
# before this reads as it was, cleaned; laws watch_target_*)
def Bg.enc(+t: String) -> String:
  "\\e" ++ String.join(String.split(String.join(String.split(String.join(String.split(String.join(String.split(t, '\\'), "\\b"), '\t'), "\\t"), '\n'), "\\n"), '\r'), "\\r")

def Bg.dec.piece(+p: String) -> String:
  Bool.pick(String, String.starts_with(p, "b"), "\\" ++ String.drop(p, 1n),
    Bool.pick(String, String.starts_with(p, "t"), "\t" ++ String.drop(p, 1n),
      Bool.pick(String, String.starts_with(p, "n"), "\n" ++ String.drop(p, 1n),
        Bool.pick(String, String.starts_with(p, "r"), "\r" ++ String.drop(p, 1n), "\\" ++ p))))

def Bg.dec.go(ps: List<&2, String>) -> String:
  match ps:
    case Nil{}:
      ""
    case +p <> t:
      Bg.dec.piece(p) ++ Bg.dec.go(t)

def Bg.dec.k(ps: List<&2, String>) -> String:
  match ps:
    case Nil{}:
      ""
    case +p <> t:
      p ++ Bg.dec.go(t)

def Bg.dec(+t: String) -> String:
  Bool.pick(String, String.starts_with(t, "\\e"), Bg.dec.k(String.split(String.drop(t, 2n), '\\')), t)

def Bg.line(+id: String, +doing: String) -> String:
  Bg.clean(id) ++ "\t" ++ Bg.clean(doing)

def Bg.keep(empty: Bool, +l: String, rest: List<&2, String>) -> List<&2, String>:
  match empty:
    case True{}:
      rest
    case False{}:
      l <> rest

def Bg.nonempty(ls: List<&2, String>) -> List<&2, String>:
  match ls:
    case Nil{}:
      Nil{}
    case +l <> t:
      Bg.keep(String.is_empty(l), l, Bg.nonempty(t))

def Bg.lines(+text: String) -> List<&2, String>:
  Bg.nonempty(String.split(text, '\n'))

def Bg.is(+l: String, +id: String) -> Bool:
  String.starts_with(l, Bg.clean(id) ++ "\t")

def Bg.has(ls: List<&2, String>, +id: String) -> Bool:
  match ls:
    case Nil{}:
      False{}
    case +l <> t:
      Bool.or(Bg.is(l, id), Bg.has(t, id))

def Bg.drop.k(hit: Bool, +l: String, rest: List<&2, String>) -> List<&2, String>:
  Bg.keep(hit, l, rest)

# the lines without subagent id
def Bg.drop(ls: List<&2, String>, +id: String) -> List<&2, String>:
  match ls:
    case Nil{}:
      Nil{}
    case +l <> t:
      Bg.drop.k(Bg.is(l, id), l, Bg.drop(t, id))

# field i of a line ("" past its end)
def Bg.field(+l: String, +i: Nat) -> String:
  Maybe.default(&2, String, List.head(&2, String, List.drop(&2, String, String.split(l, '\t'), i)), "")

# the fields after what it does, with their tab ("" for none)
def Bg.rest.k(+r: String) -> String:
  Bool.pick(String, String.is_empty(r), "", "\t" ++ r)

def Bg.rest(+l: String) -> String:
  Bg.rest.k(String.join(List.drop(&2, String, String.split(l, '\t'), 2n), "\t"))

def Bg.set.k(hit: Bool, +l: String, +id: String, +doing: String) -> String:
  match hit:
    case True{}:
      Bg.line(id, doing) ++ Bg.rest(l)
    case False{}:
      l

# what subagent id does now (a subagent not listed stays out)
def Bg.set(ls: List<&2, String>, +id: String, +doing: String) -> List<&2, String>:
  match ls:
    case Nil{}:
      Nil{}
    case +l <> t:
      Bg.set.k(Bg.is(l, id), l, id, doing) <> Bg.set(t, id, doing)

def Bg.add.k(has: Bool, +ls: List<&2, String>, +l: String) -> List<&2, String>:
  match has:
    case True{}:
      ls
    case False{}:
      List.append(&2, String, ls, [l])

# a subagent started (listed once, last)
def Bg.add(+ls: List<&2, String>, +id: String, +doing: String) -> List<&2, String>:
  Bg.add.k(Bg.has(ls, id), ls, Bg.line(id, doing))

# one started by a tool call, with what it was asked to do and its kind
def Bg.start(+ls: List<&2, String>, +id: String, +doing: String, +call: String, +title: String, +kind: String) -> List<&2, String>:
  Bg.add.k(Bg.has(ls, id), ls, Bg.line(id, doing) ++ "\t" ++ Bg.clean(call) ++ "\t" ++ Bg.clean(title) ++ "\t" ++ Bg.clean(kind))

# the word for a task type: "subagent", "shell", "workflow", "monitor"
def Bg.kind(+tt: String) -> String:
  Bool.pick(String, Bool.or(String.eq(tt, "local_agent"), Bool.or(String.eq(tt, "remote_agent"), String.eq(tt, "in_process_teammate"))), "subagent",
    Bool.pick(String, String.eq(tt, "local_bash"), "shell",
      Bool.pick(String, String.eq(tt, "local_workflow"), "workflow",
        Bool.pick(String, String.starts_with(tt, "monitor_"), "monitor", "task"))))

# a line's kind ("" for a line that names none)
def Bg.kind.of(+l: String) -> String:
  Bg.field(l, 4n)

# the Agent tool call that started a line's subagent ("" for none)
def Bg.call(+l: String) -> String:
  Bg.field(l, 2n)

# what it was asked to do ("" for none)
def Bg.title(+l: String) -> String:
  Bg.field(l, 3n)

def Bg.show(ls: List<&2, String>) -> String:
  String.join(ls, "\n")

def Bg.doing.of(ls: List<&2, String>) -> String:
  match ls:
    case Nil{}:
      ""
    case +l <> _:
      l

# what a line's subagent does (the text after its tab)
def Bg.doing(+l: String) -> String:
  Bg.doing.of(List.drop(&2, String, String.split(l, '\t'), 1n))

# a thread's subagents at work
def Bg.of(+s: State, +th: String) -> List<&2, String>:
  Bg.lines(Setting.get(s, Bg.key(th), ""))

# Ended rows: a task that finished (task_updated, or gone from Claude's
# background_tasks_changed list) whose outcome the agent has not been
# handed yet (its task_notification). Its doing reads Bg.ended.mark() and
# the status; it still counts as work until the notification, or the next
# turn's end, takes it.
def Bg.ended.mark() -> String:
  SCon{Chr{1}, SNil{}} ++ "ended:"

def Bg.ended(+l: String) -> Bool:
  String.starts_with(Bg.doing(l), Bg.ended.mark())

# how an ended row ended ("" for one at work)
def Bg.status(+l: String) -> String:
  Bool.pick(String, Bg.ended(l), String.drop(Bg.doing(l), String.length(Bg.ended.mark())), "")

def Bg.end.k(hit: Bool, +l: String, +id: String, +status: String) -> String:
  match hit:
    case True{}:
      Bg.line(id, Bg.ended.mark() ++ status) ++ Bg.rest(l)
    case False{}:
      l

# row id finished with status (one not listed, or already ended, is left)
def Bg.end(ls: List<&2, String>, +id: String, +status: String) -> List<&2, String>:
  match ls:
    case Nil{}:
      Nil{}
    case +l <> t:
      Bg.end.k(Bool.and(Bg.is(l, id), Bool.not(Bg.ended(l))), l, id, status) <> Bg.end(t, id, status)

# what row id does now, unless it ended
def Bg.progress(ls: List<&2, String>, +id: String, +doing: String) -> List<&2, String>:
  match ls:
    case Nil{}:
      Nil{}
    case +l <> t:
      Bg.set.k(Bool.and(Bg.is(l, id), Bool.not(Bg.ended(l))), l, id, doing) <> Bg.progress(t, id, doing)

# every row, ended or not: the work the thread still waits on
def Bg.live(ls: List<&2, String>) -> Nat:
  List.length(&2, String, ls)

# the rows still at work
def Bg.running(ls: List<&2, String>) -> Nat:
  match ls:
    case Nil{}:
      0n
    case +l <> t:
      Nat.add(Bool.pick(Nat, Bg.ended(l), 0n, 1n), Bg.running(t))

# the rows not ended
def Bg.unended(ls: List<&2, String>) -> List<&2, String>:
  match ls:
    case Nil{}:
      Nil{}
    case +l <> t:
      Bg.keep(Bg.ended(l), l, Bg.unended(t))

def Bg.in(ids: List<&2, String>, +id: String) -> Bool:
  match ids:
    case Nil{}:
      False{}
    case +x <> t:
      Bool.or(String.eq(Bg.clean(x), Bg.clean(id)), Bg.in(t, id))

def Bg.sync.k(gone: Bool, +l: String) -> String:
  match gone:
    case True{}:
      Bg.line(Bg.field(l, 0n), Bg.ended.mark() ++ "done") ++ Bg.rest(l)
    case False{}:
      l

# Claude's list of the tasks still open (background_tasks_changed): a row
# at work that is not on it has ended; none is ever added (the list does
# not say which a subagent owns)
def Bg.sync(ls: List<&2, String>, +open: List<&2, String>) -> List<&2, String>:
  match ls:
    case Nil{}:
      Nil{}
    case +l <> t:
      Bg.sync.k(Bool.and(Bool.not(Bg.ended(l)), Bool.not(Bg.in(open, Bg.field(l, 0n)))), l) <> Bg.sync(t, open)

# Readiness
# ---------
# One check for "this thread's work is done", used by everything that
# acts on it (alerts, thread_wait, settling, scratch expiry, worktree
# removal, the phone's island, delegated tasks, bot replies): no turn
# running, nothing queued, and nothing out of hand: child tasks not
# finished, the agent's background rows (ended ones included: their
# outcome is still owed) and watched work ("watch.<thread>", rows like
# Bg's) (laws work_*).

def Work.kids.go(ts: List<&2, Task>, +th: String) -> Nat:
  match ts:
    case Nil{}:
      0n
    case Task{_, +p, _, _, st, _, _, _, _} <> t:
      Nat.add(Bool.pick(Nat, Bool.and(String.eq(p, th), Bool.not(TaskState.terminal(st))), 1n, 0n), Work.kids.go(t, th))

# a thread's child tasks not yet finished
def Work.kids(+s: State, +th: String) -> Nat:
  match s:
    case State{_, _, _, _, +tk, _, _, _, _, _, _}:
      Work.kids.go(Map.values(&2, Task, tk), th)

def Work.watch.key(+th: String) -> String:
  "watch." ++ th

# the work Backplane watches for a thread
def Work.watch(+s: State, +th: String) -> List<&2, String>:
  Bg.lines(Setting.get(s, Work.watch.key(th), ""))

# what a thread's own rows hold (its background rows and watched work)
def Work.rows(+s: State, +th: String) -> Nat:
  Nat.add(Bg.live(Bg.of(s, th)), Bg.live(Work.watch(s, th)))

# how much work a thread still waits on (0: none)
def Work.busy(+s: State, +th: String) -> Nat:
  Nat.add(Work.kids(s, th), Work.rows(s, th))

def Work.ready.k(run: Bool, queued: Bool, +busy: Nat) -> Bool:
  match run queued:
    case False{} False{}:
      Nat.is_eq(busy, 0n)
    case _ _:
      False{}

# whether a thread is done: not running, not starting, nothing out
def Work.ready(+s: State, +th: String) -> Bool:
  Work.ready.k(State.running.k(State.thread.get(s, th)), Queue.starting(s, th), Work.busy(s, th))

def Work.kids.inc(r: Map<&2, Nat> & Nat, +p: String) -> Map<&2, Nat>:
  (m, n) = r
  Map.set(&2, Nat, m, p, 1n+n)

def Work.kids.add(hit: Bool, m: Map<&2, Nat>, +p: String) -> Map<&2, Nat>:
  match hit:
    case True{}:
      Work.kids.inc(Map.get(Nat, 0n, m, p), p)
    case False{}:
      m

def Work.kids.fold(ts: List<&2, Task>, m: Map<&2, Nat>) -> Map<&2, Nat>:
  match ts:
    case Nil{}:
      m
    case Task{_, +p, _, _, st, _, _, _, _} <> t:
      Work.kids.fold(t, Work.kids.add(Bool.not(TaskState.terminal(st)), m, p))

# every parent's unfinished child tasks, in one pass (for the tick)
def Work.kids.map(+s: State) -> Map<&2, Nat>:
  match s:
    case State{_, _, _, _, +tk, _, _, _, _, _, _}:
      Work.kids.fold(Map.values(&2, Task, tk), Map.new(&2, Nat))

# every thread's own rows (Work.rows) in one pass over the settings that hold
# them ("bg.<thread>", "watch.<thread>"), for the screens that ask of many
# threads: a lookup in this small map beats two in the settings
def Work.rows.id(+k: String, +pre: String) -> String:
  Bool.pick(String, String.starts_with(k, pre), String.drop(k, String.length(pre)), "")

def Work.rows.put.k(none: Bool, +s: State, +th: String, m: Map<&2, Nat>) -> Map<&2, Nat>:
  match none:
    case True{}:
      m
    case False{}:
      Map.set(&2, Nat, m, th, Work.rows(s, th))

def Work.rows.put(+s: State, +th: String, m: Map<&2, Nat>) -> Map<&2, Nat>:
  Work.rows.put.k(String.is_empty(th), s, th, m)

def Work.rows.go(ks: List<&2, String>, +s: State, m: Map<&2, Nat>) -> Map<&2, Nat>:
  match ks:
    case Nil{}:
      m
    case +k <> t:
      Work.rows.go(t, s, Work.rows.put(s, Work.rows.id(k, "bg."), Work.rows.put(s, Work.rows.id(k, "watch."), m)))

# every thread's rows by walking every setting (the specification of
# Work.rows.map; test/work_rows_test.bend holds them equal)
def Work.rows.map.all(+s: State) -> Map<&2, Nat>:
  Work.rows.go(Setting.keys(s), s, Map.new(&2, Nat))

def Work.rows.ids(ks: List<&2, String>, +n: Nat, +s: State, m: Map<&2, Nat>) -> Map<&2, Nat>:
  match ks:
    case Nil{}:
      m
    case +k <> t:
      Work.rows.ids(t, n, s, Work.rows.put(s, String.drop(k, n), m))

# Work.rows.map.all reading only the settings named "bg.*" and "watch.*"
# (Pfx.keys): the sidebar asks it every frame, and walking every setting
# (10k+ on a real log, each key sliced twice) grew each frame with history
def Work.rows.map(+s: State) -> Map<&2, Nat>:
  Work.rows.ids(Setting.keys.pre(s, "watch."), 6n, s, Work.rows.ids(Setting.keys.pre(s, "bg."), 3n, s, Map.new(&2, Nat)))

def Work.kids.at.k(r: Map<&2, Nat> & Nat) -> Nat:
  (_, n) = r
  n

# Work.busy with the children counted once for every thread (Work.kids.map)
def Work.busy.at(+m: Map<&2, Nat>, +s: State, +th: String) -> Nat:
  Nat.add(Work.kids.at.k(Map.get(Nat, 0n, m, th)), Work.rows(s, th))

# Work.busy.at with the rows from Work.rows.map (one pass), not two lookups in the settings
def Work.busy.m(+kids: Map<&2, Nat>, +rows: Map<&2, Nat>, +th: String) -> Nat:
  Nat.add(Work.kids.at.k(Map.get(Nat, 0n, kids, th)), Work.kids.at.k(Map.get(Nat, 0n, rows, th)))

# Lifecycle
# ---------
# A thread settles by itself once the work it owns is done (with the
# setting settle.done on, Lifecycle.on), and wakes
# (ThreadWoke) when real work starts on it again; hub.bend's Shelve.*
# applies these to every step and every tick (laws life_*). A thread works
# while a turn runs or waits in its queue, an ask waits on the person, or
# work it owns is out: delegated child tasks, background rows, watched
# work (Work.busy). Only a turn that completed settles: a failure or a stop
# stays for the person (Settle.auto's inactivity window still applies).

def Lifecycle.pending(t: Thread) -> Nat:
  match t:
    case Thread{_, _, _, _, _, _, pe, _, _, _, _, _, _, _, _, _, _, _, _, _, _}:
      pe

def Lifecycle.override(t: Thread) -> Override:
  match t:
    case Thread{_, _, _, _, _, _, _, _, o, _, _, _, _, _, _, _, _, _, _, _, _}:
      o

def Lifecycle.settled_at(t: Thread) -> Nat:
  match t:
    case Thread{_, _, _, _, _, _, _, _, _, sa, _, _, _, _, _, _, _, _, _, _, _}:
      sa

def Lifecycle.unsettled_at(t: Thread) -> Nat:
  match t:
    case Thread{_, _, _, _, _, _, _, _, _, _, ua, _, _, _, _, _, _, _, _, _, _}:
      ua

def Lifecycle.working.k(run: Bool, asks: Bool, queued: Bool, +busy: Nat) -> Bool:
  match run asks queued:
    case False{} False{} False{}:
      Nat.is_gt(busy, 0n)
    case _ _ _:
      True{}

# whether a thread works now (kids: Work.kids.map, every parent's children
# counted once)
def Lifecycle.working(+kids: Map<&2, Nat>, +s: State, +t: Thread) -> Bool:
  Lifecycle.working.k(State.running.k(t), Nat.is_gt(Lifecycle.pending(t), 0n), Queue.waiting(s, Thread.id(t)), Work.busy.at(kids, s, Thread.id(t)))

def Lifecycle.done.k(working: Bool, turn: TurnState, sh: Shelf, o: Override, snoozed: Bool) -> Bool:
  match working turn sh o snoozed:
    case False{} Completed{} Active{} NoOverride{} False{}:
      True{}
    case _ _ _ _ _:
      False{}

def Lifecycle.done.c(turn: TurnState, sh: Shelf, o: Override, snoozed: Bool, +kids: Map<&2, Nat>, +s: State, +t: Thread) -> Bool:
  match turn sh o snoozed:
    case Completed{} Active{} NoOverride{} False{}:
      Lifecycle.done.k(Lifecycle.working(kids, s, t), Completed{}, Active{}, NoOverride{}, False{})
    case _ _ _ _:
      False{}

# whether a thread settles by itself now: its last turn completed, nothing
# works (Lifecycle.working), it is on the active shelf with no override
# (pinned, hidden, settled or brought back by the person: left alone) and
# not snoozed. The cheap tests go first: the tick asks it of every thread
def Lifecycle.done(+kids: Map<&2, Nat>, +s: State, +t: Thread, snoozed: Bool) -> Bool:
  Lifecycle.done.c(Thread.turn(t), Shelf.of(t), Lifecycle.override(t), snoozed, kids, s, t)

# The "settle.done" setting (Settings, Threads): a done thread settles at
# once (Lifecycle.done, hub.bend's Shelve) only when "on". Off, the
# default, a finished thread stays on the active shelf, done, until the
# person settles it or Settle.auto's inactivity window passes. Waking is
# the same either way (laws life_*, life_off_*)
def Lifecycle.key() -> String:
  "settle.done"

def Lifecycle.on(+s: State) -> Bool:
  String.eq(Setting.get(s, Lifecycle.key(), "off"), "on")

# a step's wake: the thread went from idle to working, and it was settled
# (by itself or by the person) or brought back by the person (whose hold
# then gives way to the work, so the thread settles once it is done)
def Lifecycle.wake.k(was: Bool, now: Bool, sh: Shelf, o: Override) -> Bool:
  match was now sh o:
    case False{} True{} Settled{} _:
      True{}
    case False{} True{} Active{} ForcedActive{}:
      True{}
    case _ _ _ _:
      False{}

# a step's wake by new input: a message for the thread (the person's, or
# another thread's or bot's in its name) or one queued for it, while it
# works and is settled. New input is new work even while a turn runs, so a
# manual settle gives way to it, in the same second too; the turn's own
# events (a repeated running state) are not input and leave the settle
def Lifecycle.wake.fresh(fresh: Bool, now: Bool, sh: Shelf) -> Bool:
  match fresh now sh:
    case True{} True{} Settled{}:
      True{}
    case _ _ _:
      False{}

# whether a settle no longer holds against work: an auto-settle never
# does, a manual one only once a message came after it (Thread.touched
# moves unsettled_at past settled_at; a turn's own events never do)
def Lifecycle.lapsed(o: Override, +sa: Nat, +ua: Nat) -> Bool:
  match o:
    case ForcedSettled{}:
      Nat.is_gt(ua, sa)
    case _:
      True{}

def Lifecycle.stale.c(sh: Shelf, lapsed: Bool, +kids: Map<&2, Nat>, +s: State, +t: Thread) -> Bool:
  match sh lapsed:
    case Settled{} True{}:
      Lifecycle.working(kids, s, t)
    case _ _:
      False{}

# the tick's wake, for state left behind (a log from before this rule, a
# step that bypassed it): a settled thread that works, settled by itself
# or by the person before the message that started this work
def Lifecycle.stale(+kids: Map<&2, Nat>, +s: State, +t: Thread) -> Bool:
  Lifecycle.stale.c(Shelf.of(t), Lifecycle.lapsed(Lifecycle.override(t), Lifecycle.settled_at(t), Lifecycle.unsettled_at(t)), kids, s, t)

def Lifecycle.task.p(t: Task) -> String:
  match t:
    case Task{_, p, _, _, _, _, _, _, _}:
      p

def Lifecycle.task.k(r: Map<&2, Task> & Task) -> String:
  (_, t) = r
  Lifecycle.task.p(t)

# the thread that delegated a task ("" for none)
def Lifecycle.task.parent(+s: State, +id: String) -> String:
  match s:
    case State{_, _, _, _, +tk, _, _, _, _, _, _}:
      Lifecycle.task.k(Map.get(Task, Task.blank(id), tk, id))

# when an auto-settle is stamped: now, but always after the thread was
# last brought back (a thread settles only while settled_at passes
# unsettled_at, and a turn may start and end within one second)
def Lifecycle.at(+t: Thread, +now: Nat) -> Nat:
  Time.max(now, 1n+Lifecycle.unsettled_at(t))

def Work.row.title(+l: String) -> String:
  Bool.pick(String, String.is_empty(Bg.title(l)), Bg.doing(l), Bg.title(l))

def Work.row.titles(ls: List<&2, String>) -> List<&2, String>:
  match ls:
    case Nil{}:
      Nil{}
    case +l <> t:
      Work.row.title(l) <> Work.row.titles(t)

def Work.kid.titles(ts: List<&2, Task>, +s: State, +th: String) -> List<&2, String>:
  match ts:
    case Nil{}:
      Nil{}
    case Task{_, +p, +c, _, st, _, _, _, _} <> t:
      Bg.keep(Bool.not(Bool.and(String.eq(p, th), Bool.not(TaskState.terminal(st)))), "task " ++ c, Work.kid.titles(t, s, th))

# what a thread waits on, one line each (thread_wait's "work")
def Work.titles(+s: State, +th: String) -> List<&2, String>:
  match s:
    case State{_, _, _, _, +tk, _, _, _, _, _, _}:
      List.concat(&2, String, [Work.kid.titles(Map.values(&2, Task, tk), s, th), Work.row.titles(Bg.of(s, th)), Work.row.titles(Work.watch(s, th))])

# a scratch thread that has had its time and waits on no work (busy:
# Work.busy; law scratch_busy_kept)
def Scratch.expired.w(+now: Nat, +w: Nat, t: Thread, +busy: Nat) -> Bool:
  Bool.and(Scratch.expired(now, w, t), Nat.is_eq(busy, 0n))

# whether a thread's worktree goes now, never while it waits on work
# (law worktree_busy_kept)
def Worktree.drop.w(+s: State, +t: Thread, +now: Nat, +window: Nat, +busy: Nat) -> Bool:
  Bool.and(Worktree.drop(s, t, now, window), Nat.is_eq(busy, 0n))

def Work.turn.runs(t: TurnState) -> Bool:
  match t:
    case Running{}:
      True{}
    case _:
      False{}

# one thread: archived or deleted counts 0 and running 1 without reading
# its rows; only a live idle thread's rows are read (the busy count runs
# after steps that can move it, and walked every row of every thread)
def Work.threads.one(gone: Bool, run: Bool, +kids: Map<&2, Nat>, +s: State, +id: String) -> Nat:
  match gone run:
    case True{} _:
      0n
    case False{} True{}:
      1n
    case False{} False{}:
      Bool.pick(Nat, Nat.is_gt(Work.busy.at(kids, s, id), 0n), 1n, 0n)

def Work.threads.go(ts: List<&2, Thread>, +kids: Map<&2, Nat>, +s: State, n: Nat) -> Nat:
  match ts:
    case Nil{}:
      n
    case Thread{+id, _, _, _, _, turn, _, ar, _, _, _, _, _, _, _, _, _, _, _, _, de} <> t:
      Work.threads.go(t, kids, s, Nat.add(n, Work.threads.one(Bool.or(ar, de), Work.turn.runs(turn), kids, s, id)))

# how many threads run a turn or wait on work (an update waits for them)
def Work.threads(+s: State) -> Nat:
  match s:
    case State{_, +ts, _, _, _, _, _, _, _, _, _}:
      Work.threads.go(Map.values(&2, Thread, ts), Work.kids.map(s), s, 0n)
