import Base
import ./i18n.bend as I
import ./msgs.bend as MSG
import ./locks.bend as LK
import ./model.bend as M
import ./codec.bend as C
import ./json.bend as J
import ./signal.bend as SG
import ./deploy.bend as DP
import ./bot.bend as BT
import ./far.bend as FR
import ./cron.bend as CR
import ./sched.bend as SC
import ./once.bend as O
import ./client.bend as U
import ./botui.bend as BU
import ./uber.bend as UB
import ./ubersync.bend as US

# The status plane (docs/uber.md)
# -------------------------------
# What the uberbot's Status tab shows, one value for every client (the
# window, the web and the phones draw it): what waits on the person, what
# runs, the bots, the rooms with news, the routines and the rest of the
# day's threads, here and on the owner's other machines (their threads and
# bots are in the client's state as <link>~<id>, far.bend).
#
# A row says what a click sends (act, val: the usual client action), the
# machine it is on ("" for this one) and its state: approval, question,
# failed, working, finished, idle or away for a thread (a lock or deploy
# row: working or waiting); the mood
# for a bot; unread for a room; working or idle for a routine. `at` is the
# last activity (a routine's: when it is next due, 0 for never).

type PRow is Data:
  PRow{kind: String, id: String, act: String, val: String, title: String, sub: String, machine: String, state: String, at: Nat, away: Bool}

# a machine: here (answering), away, or syncing (nothing has come yet)
type PMach is Data:
  PMach{name: String, state: String}

type Plane is Data:
  Plane{need: List<&2, PRow>, run: List<&2, PRow>, bots: List<&2, PRow>, rooms: List<&2, PRow>, routines: List<&2, PRow>, idle: List<&2, PRow>, idle_n: Nat, machines: List<&2, PMach>, locks: List<&2, PRow>}

def Plane.need(p: Plane) -> List<&2, PRow>:
  match p:
    case Plane{n, _, _, _, _, _, _, _, _}:
      n

def Plane.run(p: Plane) -> List<&2, PRow>:
  match p:
    case Plane{_, r, _, _, _, _, _, _, _}:
      r

def Plane.bots(p: Plane) -> List<&2, PRow>:
  match p:
    case Plane{_, _, b, _, _, _, _, _, _}:
      b

def Plane.rooms(p: Plane) -> List<&2, PRow>:
  match p:
    case Plane{_, _, _, r, _, _, _, _, _}:
      r

def Plane.routines(p: Plane) -> List<&2, PRow>:
  match p:
    case Plane{_, _, _, _, r, _, _, _, _}:
      r

def Plane.idle(p: Plane) -> List<&2, PRow>:
  match p:
    case Plane{_, _, _, _, _, i, _, _, _}:
      i

def Plane.idle_n(p: Plane) -> Nat:
  match p:
    case Plane{_, _, _, _, _, _, n, _, _}:
      n

def Plane.machines(p: Plane) -> List<&2, PMach>:
  match p:
    case Plane{_, _, _, _, _, _, _, m, _}:
      m

def Plane.locks(p: Plane) -> List<&2, PRow>:
  match p:
    case Plane{_, _, _, _, _, _, _, _, ls}:
      ls

# "held by <thread> 5m ago", or the process outside the hub that holds the flock
def Plane.lock.who(+l: I.Loc, o: Maybe<&2, LK.LockParty>, +title: String, +p: LK.LockParty, +now: Nat) -> String:
  match o:
    case Some{LK.LockParty{+pid, _, _, +cmd, _, _}}:
      MSG.T.plane.lock.outside(l, pid, cmd)
    case None{}:
      MSG.T.plane.lock.by(l, title, I.I18n.ago(l, now, LK.Lock.since(p)))

def Plane.lock.row(+l: I.Loc, +name: String, +p: LK.LockParty, +n: Nat, +ready: Bool, o: Maybe<&2, LK.LockParty>, +st: M.State, +now: Nat) -> PRow:
  +th = LK.Lock.thread(p)
  +title = Bool.pick(String, String.is_empty(th), LK.Lock.id(p), U.Active.title(U.Ui.thread(st, th)))
  PRow{"lock", name, Bool.pick(String, String.is_empty(th), "", "select"), th, MSG.T.plane.lock.row(l, name, Plane.lock.who(l, o, title, p, now), Nat.show(n)), LK.Lock.note(p), "", Bool.pick(String, ready, "working", "waiting"), 0n, False{}}

def Plane.lock.h(+lc: I.Loc, h: Maybe<&2, LK.LockParty>, +name: String, +l: LK.NamedLock, +st: M.State, +now: Nat, rest: List<&2, PRow>) -> List<&2, PRow>:
  match h:
    case None{}:
      rest
    case Some{p}:
      Plane.lock.row(lc, name, p, List.length(&2, LK.LockParty, LK.Lock.waiters(l)), LK.Lock.ready(l), LK.Lock.outside(l), st, now) <> rest

def Plane.lock.go(+l: I.Loc, names: List<&2, String>, +st: M.State, +now: Nat) -> List<&2, PRow>:
  match names:
    case Nil{}:
      Nil{}
    case +name <> t:
      Plane.lock.h(l, LK.Lock.holder(M.Locks.get(st, name)), name, M.Locks.get(st, name), st, now, Plane.lock.go(l, t, st, now))

def PRow.id(r: PRow) -> String:
  match r:
    case PRow{_, i, _, _, _, _, _, _, _, _}:
      i

# a row's title where a client shows it: a thread's stored "New thread"
# in the device's language (the agents' text keeps the stored title)
def Plane.title.shown(l: I.Loc, +kind: String, +title: String) -> String:
  Bool.pick(String, String.eq(kind, "thread"), U.Title.shown(l, title), title)

def PRow.title(r: PRow) -> String:
  match r:
    case PRow{_, _, _, _, t, _, _, _, _, _}:
      t

def PRow.state(r: PRow) -> String:
  match r:
    case PRow{_, _, _, _, _, _, _, s, _, _}:
      s

def PRow.at(r: PRow) -> Nat:
  match r:
    case PRow{_, _, _, _, _, _, _, _, a, _}:
      a

# the shown idle threads are those of the last day, at most this many
def Plane.idle.max() -> Nat:
  50n

def Plane.idle.age() -> Nat:
  86400n

# a routine counts as running for this long after it fires, while its bot works
def Plane.routine.window() -> Nat:
  900n

# Threads
# -------

def Plane.state(p: M.Phase) -> String:
  match p:
    case M.PhApproval{}:
      "approval"
    case M.PhInput{}:
      "question"
    case M.PhFailed{}:
      "failed"
    case M.PhWorking{}:
      "working"
    case M.PhMonitoring{_}:
      "working"
    case M.PhComplete{}:
      "finished"
    case M.PhStopped{}:
      "idle"
    case M.PhIdle{}:
      "idle"

# Display callers opt into the activity projection with decorated Ui/SIx.
# Legacy callers/laws retain their Phase protocol until they opt in.
def Plane.signal.state(+n: Nat) -> String:
  Bool.pick(String, SG.Signal.blocked(n), "approval", Bool.pick(String, SG.Signal.active(n), "working", Bool.pick(String, Nat.is_eq(n, 4n), "settled",
    Bool.pick(String, Nat.is_eq(n, 5n), "finished", "idle"))))

# The activity bit includes owned work. Only the thread's own Running turn
# means its agent is working; a completed coordinator can be waiting on work.
def Plane.signal.blocked(p: M.Phase) -> String:
  match p:
    case M.PhApproval{}: "approval"
    case M.PhInput{}: "question"
    case M.PhFailed{}: "failed"
    case _: "approval"

# What blocks a thread itself, "" when nothing does: an ask or pending tool
# (its phase), a failed turn, a sign-in it waits for or a held message.
def Plane.own.phase(p: M.Phase) -> String:
  match p:
    case M.PhApproval{}: "approval"
    case M.PhInput{}: "question"
    case M.PhFailed{}: "failed"
    case _: ""

def Plane.own.extra(+st: M.State, +id: String) -> String:
  Bool.pick(String, Bool.or(Bool.not(String.is_empty(M.Setting.get(st, "auth.wait." ++ id, ""))), SG.Signal.held(M.Queue.items(st, id))), "approval", "")

def Plane.own.k(+a: String, +st: M.State, +id: String) -> String:
  Bool.pick(String, String.is_empty(a), Plane.own.extra(st, id), a)

def Plane.own(+st: M.State, +phase: M.Phase, +id: String) -> String:
  Plane.own.k(Plane.own.phase(phase), st, id)

def Plane.own.of(+st: M.State, +id: String) -> String:
  Plane.own(st, U.Side.phase(st, U.Ui.thread(st, id)), id)

# Inherited blocks. signal.bend spreads a blocked bit from a launched thread
# or a delegated task up to its parent (the sidebar's dot). The plane counts
# the person's attention once, at its source. A launched thread is a row of
# its own, so its launcher does not count it again (law plane_launch_once).
# A delegated task's child shows only under its parent, so the parent stands
# for it, with the child's state (a question stays a question, a failure a
# failure), and its row opens that child (Plane.jump). Plane.via.src is the
# source thread, "" when every source is a row of its own.
def Plane.keep.text(yes: Bool, x: String, rest: List<&2, String>) -> List<&2, String>:
  match yes:
    case True{}: x <> rest
    case False{}: rest

def Plane.via.tasks(ts: List<&2, M.Task>, +launched: List<&2, String>) -> List<&2, String>:
  match ts:
    case Nil{}: Nil{}
    case M.Task{_, _, +child, _, state, _, _, _, _} <> rest:
      Plane.keep.text(Bool.and(Bool.not(M.TaskState.terminal(state)), Bool.not(M.Launch.has(launched, child))), child, Plane.via.tasks(rest, launched))

def Plane.via.kids(+st: M.State, +id: String) -> List<&2, String>:
  Plane.via.tasks(U.Tasks.of(st, id), M.Launch.of(st, id))

def Plane.via.at(+own: String, +c: String) -> String:
  Bool.pick(String, String.is_empty(own), "", c)

def Plane.via.hit(+found: String, +c: String, +st: M.State) -> String:
  Bool.pick(String, String.is_empty(found), Plane.via.at(Plane.own.of(st, c), c), found)

def Plane.via.next(none: Bool, +c: String, rest: List<&2, String>, +st: M.State) -> List<&2, String>:
  match none:
    case True{}: List.append(&2, String, rest, Plane.via.kids(st, c))
    case False{}: Nil{}

def Plane.via.go(fuel: Nat, ids: List<&2, String>, +found: String, +st: M.State) -> String:
  match fuel ids:
    case 0n _: found
    case 1n+_ Nil{}: found
    case 1n+k +c <> rest:
      +f = Plane.via.hit(found, c, st)
      Plane.via.go(k, Plane.via.next(String.is_empty(f), c, rest, st), f, st)

def Plane.via.src(+st: M.State, +id: String) -> String:
  Plane.via.go(64n, Plane.via.kids(st, id), "", st)

# not blocked: working, waiting on owned work, settled or idle
def Plane.signal.free(+n: Nat, +t: M.Thread) -> String:
  Bool.pick(String, SG.Signal.active(n), Bool.pick(String, M.State.running.k(t), "working", "waiting"), Plane.signal.state(Bool.pick(Nat, SG.Signal.blocked(n), 0n, n)))

def Plane.signal.via.k(none: Bool, +src: String, +n: Nat, +t: M.Thread, +st: M.State) -> String:
  match none:
    case True{}: Plane.signal.free(n, t)
    case False{}: Plane.own.of(st, src)

def Plane.signal.why(none: Bool, +own: String, +n: Nat, +t: M.Thread, +st: M.State) -> String:
  match none:
    case False{}: own
    case True{}: Plane.signal.via.k(String.is_empty(Plane.via.src(st, M.Thread.id(t))), Plane.via.src(st, M.Thread.id(t)), n, t, st)

def Plane.signal.current.k(blocked: Bool, +n: Nat, +t: M.Thread, +phase: M.Phase, +st: M.State) -> String:
  match blocked:
    case True{}: Plane.signal.why(String.is_empty(Plane.own(st, phase, M.Thread.id(t))), Plane.own(st, phase, M.Thread.id(t)), n, t, st)
    case False{}: Plane.signal.free(n, t)

def Plane.signal.current(+n: Nat, +t: M.Thread, +phase: M.Phase, +st: M.State) -> String:
  Bool.pick(String, SG.Signal.display.stalled(st, M.Thread.id(t)), "stalled", Plane.signal.current.k(SG.Signal.blocked(n), n, t, phase, st))

def Plane.signal.pick(present: Bool, +ix: U.SIx, +st: M.State, +t: M.Thread) -> String:
  match present:
    case True{}: Plane.signal.current(U.Side.signal.ix(ix, t), t, U.Side.phase.ix(ix, st, t), st)
    case False{}: Plane.state(U.Side.phase.ix(ix, st, t))

def Plane.signal(+ix: U.SIx, +st: M.State, +t: M.Thread) -> String:
  Bool.pick(String, SG.Signal.display.stalled(st, M.Thread.id(t)), "stalled", Plane.signal.pick(U.Side.signal.has(ix, M.Thread.id(t)), ix, st, t))

# how much a state waits on the person (0: not at all)
def Plane.rank(+s: String) -> Nat:
  Bool.pick(Nat, String.eq(s, "approval"), 8n, Bool.pick(Nat, String.eq(s, "question"), 7n, Bool.pick(Nat, String.eq(s, "failed"), 6n, Bool.pick(Nat, String.eq(s, "stalled"), 5n, 0n))))

# the section a state goes to: 0 need, 1 run, 2 the rest
def Plane.sec(+s: String) -> Nat:
  Bool.pick(Nat, Nat.is_gt(Plane.rank(s), 0n), 0n, Bool.pick(Nat, Bool.or(String.eq(s, "working"), String.eq(s, "waiting")), 1n, 2n))

# a thread of a machine that does not answer is not asking for anything
# A thread whose open asks are all questions or plans is asking, though the
# hub counts every ask as pending (the phase then says approval): the clients
# draw a question apart from an approval.
def Plane.asking(+s: String, +ix: U.SIx, +id: String) -> String:
  Bool.pick(String, Bool.and(String.eq(s, "approval"), Bool.and(Bool.not(U.Asks.kind(U.SIx.open(ix, id), "approval")),
    Bool.or(U.Asks.kind(U.SIx.open(ix, id), "input"), U.Asks.kind(U.SIx.open(ix, id), "plan")))), "question", s)

# what a thread row's click opens: the thread, or for one that needs the
# person only through a child task, that child (Plane.via.src)
def Plane.jump.to(+src: String, +id: String) -> String:
  Bool.pick(String, String.is_empty(src), id, src)

def Plane.jump.k(none: Bool, +st: M.State, +id: String) -> String:
  match none:
    case False{}: id
    case True{}: Plane.jump.to(Plane.via.src(st, id), id)

def Plane.jump.s(need: Bool, +st: M.State, +id: String) -> String:
  match need:
    case False{}: id
    case True{}: Plane.jump.k(String.is_empty(Plane.own.of(st, id)), st, id)

def Plane.jump(+st: M.State, +id: String, +state: String) -> String:
  Plane.jump.s(Nat.is_eq(Plane.sec(state), 0n), st, id)

def Plane.thread.k(+away: Bool, +id: String, +title: String, +sub: String, +machine: String, +phase: String, +at: Nat, +st: M.State) -> PRow:
  PRow{"thread", id, "select", Plane.jump(st, id, Bool.pick(String, away, "away", phase)), title, Bool.pick(String, away, sub, SG.Signal.display.note(I.En{}, st, id, sub)), machine, Bool.pick(String, away, "away", phase), at, away}

def Plane.thread.t(+t: M.Thread, +at: Nat, +pt: String, +u: U.Ui, +ix: U.SIx, +st: M.State) -> PRow:
  Plane.thread.k(U.Ui.far.away(u, M.Thread.id(t)), M.Thread.id(t), U.Active.title(t), pt, U.Ui.far.name(u, M.Thread.id(t)), Plane.asking(Plane.signal(ix, st, t), ix, M.Thread.id(t)), at, st)

def Plane.thread(r: U.ARow, +u: U.Ui, +ix: U.SIx, +st: M.State) -> PRow:
  match r:
    case U.ARow{_, +at, +t, _, +pt, _}:
      Plane.thread.t(t, at, pt, u, ix, st)

def Plane.keep(drop: Bool, x: PRow, rest: List<&2, PRow>) -> List<&2, PRow>:
  match drop:
    case True{}:
      rest
    case False{}:
      x <> rest

# the live threads of every project (settled ones, faded, are left out)
def Plane.threads(rs: List<&2, U.ARow>, +u: U.Ui, +ix: U.SIx, +st: M.State) -> List<&2, PRow>:
  match rs:
    case Nil{}:
      Nil{}
    case +r <> t:
      Plane.keep(U.ARow.faded(r), Plane.thread(r, u, ix, st), Plane.threads(t, u, ix, st))

# A bot's own thread waits on the person or runs like any other; it shows
# in those sections as the bot (the uberbot's opens its tab)
def Plane.bot.k(+away: Bool, +id: String, +name: String, +machine: String, +phase: String, +at: Nat, +uber: Bool) -> PRow:
  PRow{"bot", id, Bool.pick(String, uber, "uber", "bot"), Bool.pick(String, uber, "", id), name, "", machine, Bool.pick(String, away, "away", phase), at, away}

def Plane.bot.reason(r: PRow, +st: M.State, +th: String, lc: I.Loc) -> PRow:
  match r:
    case PRow{k, id, a, v, ti, +sub, ma, state, at, +away}:
      PRow{k, id, a, v, ti, Bool.pick(String, away, sub, SG.Signal.display.note(lc, st, th, sub)), ma, state, at, away}

def Plane.bot.t(+b: BT.Bot, +th: String, +u: U.Ui, +ix: U.SIx, +st: M.State) -> PRow:
  Plane.bot.reason(Plane.bot.k(U.Ui.far.away(u, BT.Bot.id(b)), BT.Bot.id(b), BT.Bot.name(b), U.Ui.far.name(u, BT.Bot.id(b)), Plane.asking(Plane.signal(ix, st, U.Ui.thread(st, th)), ix, th),
    U.Active.activity(U.Ui.thread(st, th)), UB.Uber.is(st, BT.Bot.id(b))), st, th, U.Lang.loc(u))

# only a bot that has a thread, and one that is not at rest, shows here
def Plane.bots.r(+row: PRow, rest: List<&2, PRow>) -> List<&2, PRow>:
  Plane.keep(Nat.is_gt(Plane.sec(PRow.state(row)), 1n), row, rest)

def Plane.bots.k(none: Bool, +b: BT.Bot, +u: U.Ui, +ix: U.SIx, +st: M.State, rest: List<&2, PRow>) -> List<&2, PRow>:
  match none:
    case True{}:
      rest
    case False{}:
      Plane.bots.r(Plane.bot.t(b, BT.Bot.thread(b), u, ix, st), rest)

def Plane.bots.go(bs: List<&2, BT.Bot>, +u: U.Ui, +ix: U.SIx, +st: M.State) -> List<&2, PRow>:
  match bs:
    case Nil{}:
      Nil{}
    case +b <> t:
      Plane.bots.k(String.is_empty(BT.Bot.thread(b)), b, u, ix, st, Plane.bots.go(t, u, ix, st))

# Ordering
# --------

# most urgent first, the newer first among equals (law plane_need_ranked)
def Plane.before(a: PRow, b: PRow) -> Bool:
  match a b:
    case PRow{_, _, _, _, _, _, _, +sa, +aa, _} PRow{_, _, _, _, _, _, _, +sb, +ab, _}:
      Bool.pick(Bool, Nat.is_eq(Plane.rank(sa), Plane.rank(sb)), Nat.is_ge(aa, ab), Nat.is_gt(Plane.rank(sa), Plane.rank(sb)))

def Plane.newer(a: PRow, b: PRow) -> Bool:
  Nat.is_ge(PRow.at(a), PRow.at(b))

def Plane.due.le(+a: Nat, +b: Nat) -> Bool:
  Bool.pick(Bool, Nat.is_eq(a, 0n), Nat.is_eq(b, 0n), Bool.or(Nat.is_eq(b, 0n), Nat.is_le(a, b)))

# a routine running first, then the soonest due (never, 0, last)
def Plane.due.before(a: PRow, b: PRow) -> Bool:
  match a b:
    case PRow{_, _, _, _, _, _, _, +sa, +aa, _} PRow{_, _, _, _, _, _, _, +sb, +ab, _}:
      Bool.pick(Bool, String.eq(sa, sb), Plane.due.le(aa, ab), String.eq(sa, "working"))

# the rows of a section (see Plane.sec), in the order they were given
def Plane.part(rs: List<&2, PRow>, +n: Nat) -> List<&2, PRow>:
  match rs:
    case Nil{}:
      Nil{}
    case +r <> t:
      Plane.keep(Bool.not(Nat.is_eq(Plane.sec(PRow.state(r)), n)), r, Plane.part(t, n))

# the threads (not bots) active in the last day
def Plane.recent(rs: List<&2, PRow>, +now: Nat) -> List<&2, PRow>:
  match rs:
    case Nil{}:
      Nil{}
    case +r <> t:
      Plane.keep(Bool.not(Nat.is_le(now, Nat.add(PRow.at(r), Plane.idle.age()))), r, Plane.recent(t, now))

# every thread row once, keyed by what it stands for
def Plane.ky(+r: PRow) -> O.Ky<PRow>:
  O.Ky{"t:" ++ PRow.id(r), r}

def Plane.kys(rs: List<&2, PRow>) -> List<&2, O.Ky<PRow>>:
  match rs:
    case Nil{}:
      Nil{}
    case +r <> t:
      Plane.ky(r) <> Plane.kys(t)

def Plane.bky(+r: PRow) -> O.Ky<PRow>:
  O.Ky{"b:" ++ PRow.id(r), r}

def Plane.bkys(rs: List<&2, PRow>) -> List<&2, O.Ky<PRow>>:
  match rs:
    case Nil{}:
      Nil{}
    case +r <> t:
      Plane.bky(r) <> Plane.bkys(t)

# the thread rows and the bots' own, before each is kept once
def Plane.all.src(+u: U.Ui, +ix: U.SIx, +now: Nat) -> List<&2, O.Ky<PRow>>:
  List.append(&2, O.Ky<PRow>, Plane.kys(Plane.threads(U.Active.rows.ix(U.Ui.state(u), ix, now, ""), u, ix, U.Ui.state(u))),
    Plane.bkys(Plane.bots.go(BT.World.bots(M.State.bots(U.Ui.state(u))), u, ix, U.Ui.state(u))))

# ... and kept once (law plane_each_once)
def Plane.all.keyed(+u: U.Ui, +ix: U.SIx, +now: Nat) -> List<&2, O.Ky<PRow>>:
  O.Once.of(PRow, Plane.all.src(u, ix, now))

# Bots
# ----

def Plane.bot.row(+l: I.Loc, r: BU.BRow, +st: M.State) -> PRow:
  match r:
    case BU.BRow{+i, +n, +th, _, +mood, +note, _, _, +pe, _}:
      PRow{"bot", i, BU.BU.act(BU.BRow{i, n, th, 0n, mood, note, False{}, False{}, pe, ""}), i, n, Bool.pick(String, BU.Mood.moving(mood), BU.Mood.note.loc(l, mood), ""), pe, mood,
        U.Active.activity(U.Ui.thread(st, th)), String.eq(mood, "away")}

def Plane.bot.rows(+l: I.Loc, rs: List<&2, BU.BRow>, +st: M.State) -> List<&2, PRow>:
  match rs:
    case Nil{}:
      Nil{}
    case +r <> t:
      Plane.bot.row(l, r, st) <> Plane.bot.rows(l, t, st)

# Rooms
# -----

def Plane.room.last(ps: List<&2, BT.Post>) -> Nat:
  match ps:
    case BT.Post{_, _, _, _, _, +at} <> _:
      at
    case Nil{}:
      0n

def Plane.room.k(n: Nat, +id: String, +name: String, +at: Nat, rest: List<&2, PRow>) -> List<&2, PRow>:
  match n:
    case 0n:
      rest
    case 1n+k:
      PRow{"room", id, "room", id, name, Nat.show(1n+k) ++ " unread", "", "unread", at, False{}} <> rest

def Plane.rooms.go(rs: List<&2, BT.Room>, +u: U.Ui) -> List<&2, PRow>:
  match rs:
    case Nil{}:
      Nil{}
    case +r <> t:
      Plane.room.k(U.Rooms.unread(u, BT.Room.id(r)), BT.Room.id(r), BT.Room.name(r), Plane.room.last(BT.World.posts(M.State.bots(U.Ui.state(u)), BT.Room.id(r), 1n)), Plane.rooms.go(t, u))

# Routines
# --------

# a busy bot's routine that fired within the window is running
def Plane.routine.run(busy: Bool, +last: Nat, +now: Nat) -> Bool:
  Bool.and(Bool.and(busy, Nat.is_gt(last, 0n)), Nat.is_le(now, Nat.add(last, Plane.routine.window())))

# a routine's row: what it does, and when it is next (in the person's zone)
# (a one-time routine's `when` is already its time: shown once)
def Plane.routine.sub(w: SC.SRow) -> String:
  match w:
    case SC.SRow{+kind, _, +when, +next, _, _, _, _}:
      when ++ Bool.pick(String, Bool.or(String.is_empty(next), Bool.not(String.eq(kind, "routine"))), "", " · " ++ next)

def Plane.routine.signal(+ix: U.SIx, +st: M.State, +t: M.Thread) -> Bool:
  Bool.pick(Bool, U.Side.signal.has(ix, M.Thread.id(t)), SG.Signal.active(U.Side.signal.ix(ix, t)),
    Nat.is_gt(Plane.sec(Plane.state(U.Side.phase.ix(ix, st, t))), 0n))

def Plane.routine.k(show: Bool, run: Bool, remind: Bool, +id: String, +bot: String, +name: String, +sub: String, +machine: String, +next: Nat, away: Bool, rest: List<&2, PRow>) -> List<&2, PRow>:
  match show:
    case True{}:
      PRow{"routine", id, "uber-bot", bot ++ "|routines", name, sub, machine, Bool.pick(String, run, "working", Bool.pick(String, remind, "reminder", "idle")), next, away} <> rest
    case False{}:
      rest

def Plane.routine.t(+l: I.Loc, +r: BT.Routine, +u: U.Ui, +ix: U.SIx, +now: Nat, rest: List<&2, PRow>) -> List<&2, PRow>:
  Plane.routine.k(Bool.and(BT.Routine.on(r), Bool.not(BT.Routine.done(r))),
    Plane.routine.run(Plane.routine.signal(ix, U.Ui.state(u), U.Ui.thread(U.Ui.state(u), BT.Bot.thread(BT.World.bot(M.State.bots(U.Ui.state(u)), BT.Routine.bot(r))))), BT.Routine.last(r), now),
    String.eq(BT.Routine.kind(r), "remind"), BT.Routine.id(r), BT.Routine.bot(r), BT.Routine.name(r), Plane.routine.sub(SC.Sched.row(l, U.Ui.state(u), r, now, 0n, True{})), U.Ui.far.name(u, BT.Routine.bot(r)),
    SC.Sched.next(U.Ui.state(u), r, now, 0n, True{}), U.Ui.far.away(u, BT.Routine.bot(r)), rest)

def Plane.routines.go(rs: List<&2, BT.Routine>, +l: I.Loc, +u: U.Ui, +ix: U.SIx, +now: Nat) -> List<&2, PRow>:
  match rs:
    case Nil{}:
      Nil{}
    case +r <> t:
      Plane.routine.t(l, r, u, ix, now, Plane.routines.go(t, l, u, ix, now))

# Machines
# --------

def Plane.mach.state(away: Bool, +head: Nat) -> String:
  Bool.pick(String, away, "away", Bool.pick(String, Nat.is_eq(head, 0n), "syncing", "here"))

def Plane.mach.name(+u: U.Ui, +id: String, +fallback: String) -> String:
  Bool.pick(String, String.is_empty(U.Ui.info(u, "far.name." ++ id)), fallback, U.Ui.info(u, "far.name." ++ id))

def Plane.machs(ps: List<&2, BT.Peer>, +u: U.Ui) -> List<&2, PMach>:
  match ps:
    case Nil{}:
      Nil{}
    case +p <> t:
      PMach{Plane.mach.name(u, BT.Peer.id(p), BT.Peer.name(p)), Plane.mach.state(String.eq(U.Ui.info(u, "far.away." ++ BT.Peer.id(p)), "1"), FR.Fr.head(U.Ui.state(u), BT.Peer.id(p)))} <> Plane.machs(t, u)

def Plane.machine.names(rs: List<&2, U.MRow>) -> List<&2, String>:
  match rs:
    case Nil{}:
      Nil{}
    case U.MRow{+n, _, _} <> t:
      n <> Plane.machine.names(t)

def Plane.machines.of(+u: U.Ui) -> List<&2, PMach>:
  PMach{Maybe.default(&2, String, List.head(&2, String, Plane.machine.names(U.Machines.rows(u))), MSG.T.machines.self(U.Lang.loc(u))), "here"} <> Plane.machs(BU.BU.peers.of(u), u)

# The plane
# ---------

def Plane.sort(rs: List<&2, PRow>) -> List<&2, PRow>:
  List.sort(~PRow, ~Plane.before, rs)

def Plane.deploy.k(+l: I.Loc, +line: String, +st: M.State) -> List<&2, PRow>:
  Bool.pick(List<&2, PRow>, String.is_empty(line), Nil{},
    [PRow{"deploy", "deploy", "deploy-cancel", "deploy", MSG.T.plane.deploy.title(l), MSG.T.plane.deploy.sub(l, DP.Deploy.blocking.line(DP.Deploy.count(line), DP.Deploy.locked(line))), "", "waiting", DP.Deploy.since(line), False{}}])

# a restart waiting for a safe point (core/deploy.bend): one row at the top
# of what runs; a click on it cancels the deploy
def Plane.deploy.rows(+l: I.Loc, +st: M.State) -> List<&2, PRow>:
  Plane.deploy.k(l, DP.Deploy.wait(st), st)

def Plane.of.loc(+l: I.Loc, +idle: List<&2, PRow>, +all: List<&2, PRow>, +u: U.Ui, +ix: U.SIx, +now: Nat) -> Plane:
  Plane{Plane.sort(Plane.part(all, 0n)), List.append(&2, PRow, Plane.deploy.rows(l, U.Ui.state(u)), List.sort(~PRow, ~Plane.newer, Plane.part(all, 1n))),
    Plane.bot.rows(l, BU.BU.rows(u), U.Ui.state(u)),
    List.sort(~PRow, ~Plane.newer, Plane.rooms.go(BT.World.rooms(M.State.bots(U.Ui.state(u))), u)),
    List.sort(~PRow, ~Plane.due.before, Plane.routines.go(BT.World.routines.all(M.State.bots(U.Ui.state(u))), l, u, ix, now)),
    List.take(&2, PRow, idle, Plane.idle.max()), List.length(&2, PRow, idle), Plane.machines.of(u), Plane.lock.go(l, M.Locks.names(U.Ui.state(u)), U.Ui.state(u), now)}

def Plane.of.i(+idle: List<&2, PRow>, +all: List<&2, PRow>, +u: U.Ui, +ix: U.SIx, +now: Nat) -> Plane:
  Plane.of.loc(U.Lang.loc(u), idle, all, u, ix, now)

def Plane.of.k(+all: List<&2, PRow>, +u: U.Ui, +ix: U.SIx, +now: Nat) -> Plane:
  Plane.of.i(Plane.sort(Plane.recent(Plane.part(all, 2n), now)), all, u, ix, now)

# A thread row is made only for a thread that shows: one that waits on the
# person or runs, or (when idle threads are wanted) one active in the last
# day. The rest are told apart by their state alone, which is cheap; a row's
# machine, title and project cost more, and a phone with 300 threads spent
# its time on the ones nobody sees.
def Plane.thread.t.s(r: U.ARow, +state: String, +u: U.Ui, +st: M.State) -> PRow:
  match r:
    case U.ARow{_, +at, +t, _, +pt, _}:
      Plane.thread.k(U.Ui.far.away(u, M.Thread.id(t)), M.Thread.id(t), U.Active.title(t), pt, U.Ui.far.name(u, M.Thread.id(t)), state, at, st)

def Plane.thread.st(r: U.ARow, +ix: U.SIx, +st: M.State) -> String:
  match r:
    case U.ARow{_, _, +t, _, _, _}:
      Plane.asking(Plane.signal(ix, st, t), ix, M.Thread.id(t))

def Plane.arow.at(r: U.ARow) -> Nat:
  match r:
    case U.ARow{_, +at, _, _, _, _}:
      at

def Plane.shows(+state: String, +at: Nat, +now: Nat, +idle: Bool) -> Bool:
  Bool.or(Nat.is_lt(Plane.sec(state), 2n), Bool.and(idle, Nat.is_le(now, Nat.add(at, Plane.idle.age()))))

def Plane.threads.m(show: Bool, r: U.ARow, +state: String, +u: U.Ui, +st: M.State, rest: List<&2, PRow>) -> List<&2, PRow>:
  match show:
    case True{}:
      Plane.thread.t.s(r, state, u, st) <> rest
    case False{}:
      rest

def Plane.threads.k(+r: U.ARow, +state: String, +u: U.Ui, +ix: U.SIx, +st: M.State, +idle: Bool, +now: Nat, rest: List<&2, PRow>) -> List<&2, PRow>:
  Plane.threads.m(Bool.and(Bool.not(U.ARow.faded(r)), Plane.shows(state, Plane.arow.at(r), now, idle)), r, state, u, st, rest)

def Plane.threads.f(rs: List<&2, U.ARow>, +u: U.Ui, +ix: U.SIx, +st: M.State, +idle: Bool, +now: Nat) -> List<&2, PRow>:
  match rs:
    case Nil{}:
      Nil{}
    case +r <> t:
      Plane.threads.k(r, Plane.thread.st(r, ix, st), u, ix, st, idle, now, Plane.threads.f(t, u, ix, st, idle, now))

# the live rows of the Active gather, unsorted and unfiltered: the plane
# orders its own sections, and the settled rows are not in it
def Plane.live(g: U.AParts) -> List<&2, U.ARow>:
  match g:
    case U.AParts{l, _}:
      l

def Plane.all.vals(+u: U.Ui, +ix: U.SIx, +now: Nat, +idle: Bool) -> List<&2, PRow>:
  List.append(&2, PRow, Plane.threads.f(Plane.live(U.Active.gather.ix(U.Side.rows.ix(U.Ui.state(u), ix, now), U.Ui.state(u), ix, now, U.AParts{Nil{}, Nil{}})), u, ix, U.Ui.state(u), idle, now),
    Plane.bots.go(BT.World.bots(M.State.bots(U.Ui.state(u))), u, ix, U.Ui.state(u)))

# the idle threads, only when they are wanted (the Status tab shows them)
def Plane.idle.of(want: Bool, +all: List<&2, PRow>, +now: Nat) -> List<&2, PRow>:
  match want:
    case True{}:
      Plane.sort(Plane.recent(Plane.part(all, 2n), now))
    case False{}:
      Nil{}

def Plane.of.f.k(+all: List<&2, PRow>, +u: U.Ui, +ix: U.SIx, +now: Nat, +idle: Bool) -> Plane:
  Plane.of.i(Plane.idle.of(idle, all, now), all, u, ix, now)

# the same plane without the keyed pass (the phones' screens); with no idle
# threads unless they are wanted (idle_n is then 0)
def Plane.of.f(+u: U.Ui, +ix: U.SIx, +now: Nat, +idle: Bool) -> Plane:
  Plane.of.f.k(Plane.all.vals(u, ix, now, idle), u, ix, now, idle)

def Plane.of.x(+u: U.Ui, +ix: U.SIx, +now: Nat) -> Plane:
  Plane.of.k(O.Once.vals(PRow, Plane.all.keyed(u, ix, now)), u, ix, now)

# the status plane as of `now` (the sidebar's index built once)
def Plane.of(+u: U.Ui, +now: Nat) -> Plane:
  Plane.of.x(u, U.Side.ix(U.Ui.state(u)), now)

# Cheap to compute, and different whenever the plane can be: the log's
# sequence, each machine's head and whether it answers, and the minute
def Plane.key.peers(ps: List<&2, BT.Peer>, +u: U.Ui) -> String:
  match ps:
    case Nil{}:
      ""
    case +p <> t:
      BT.Peer.id(p) ++ ":" ++ Nat.show(FR.Fr.head(U.Ui.state(u), BT.Peer.id(p))) ++ ":" ++ U.Ui.info(u, "far.away." ++ BT.Peer.id(p)) ++ ";" ++ Plane.key.peers(t, u)

def Plane.key(+u: U.Ui, +now: Nat) -> String:
  Nat.show(U.Ui.seq(u)) ++ "|" ++ Nat.show(Nat.div(now, 60n)) ++ "|" ++ Plane.key.peers(BU.BU.peers.of(u), u)

# the bots at work now (their mood: working or talking), for the strip
def Plane.working.k(hit: Bool, +n: Nat) -> Nat:
  match hit:
    case True{}:
      1n+n
    case False{}:
      n

def Plane.working.go(rs: List<&2, PRow>) -> Nat:
  match rs:
    case Nil{}:
      0n
    case +r <> t:
      Plane.working.k(Bool.or(String.eq(PRow.state(r), "work"), String.eq(PRow.state(r), "talk")), Plane.working.go(t))

def Plane.working(+p: Plane) -> Nat:
  Plane.working.go(Plane.bots(p))

# the sidebar row's words: "3 need you · 7 running" ("" when neither)
def Plane.counts.need(+l: I.Loc, +n: Nat) -> String:
  Bool.pick(String, Nat.is_eq(n, 0n), "", MSG.T.plane.count.need(l, Nat.show(n)))

def Plane.counts.run(+l: I.Loc, +n: Nat) -> String:
  Bool.pick(String, Nat.is_eq(n, 0n), "", MSG.T.plane.count.run(l, Nat.show(n)))

def Plane.counts.join(+a: String, +b: String) -> String:
  Bool.pick(String, String.is_empty(a), b, Bool.pick(String, String.is_empty(b), a, a ++ " · " ++ b))

# what a row in need asks of the person, in words: an approval, a question
# or a failed run ("" for any other state); every client shows it beside the
# title so a count can be read row by row
def Plane.why(+l: I.Loc, +state: String) -> String:
  Bool.pick(String, String.eq(state, "approval"), MSG.T.plane.why.approval(l), Bool.pick(String, String.eq(state, "question"), MSG.T.plane.why.question(l),
    Bool.pick(String, String.eq(state, "failed"), MSG.T.plane.why.failed(l), Bool.pick(String, String.eq(state, "stalled"), Bool.pick(String, String.eq(I.I18n.tag(l), "ja"), "進捗停止", "Stalled"), ""))))

# a failed run is told apart from an ask: "1 need you · 2 failed · 7 running"
# (need counts every row of the section; the words split it, law
# plane_counts_split)
def Plane.counts.failed(+l: I.Loc, +n: Nat) -> String:
  Bool.pick(String, Nat.is_eq(n, 0n), "", MSG.T.plane.count.failed(l, Nat.show(n)))

def Plane.counts.of(+l: I.Loc, +need: Nat, +failed: Nat, +run: Nat) -> String:
  Plane.counts.join(Plane.counts.join(Plane.counts.need(l, Nat.sub(need, failed)), Plane.counts.failed(l, failed)), Plane.counts.run(l, run))

def Plane.failed.k(hit: Bool, +n: Nat) -> Nat:
  match hit:
    case True{}: 1n+n
    case False{}: n

# the rows that are failed runs, not asks
def Plane.failed(rs: List<&2, PRow>) -> Nat:
  match rs:
    case Nil{}: 0n
    case +r <> t: Plane.failed.k(String.eq(PRow.state(r), "failed"), Plane.failed(t))

def Plane.counts(+l: I.Loc, +p: Plane) -> String:
  Plane.counts.of(l, List.length(&2, PRow, Plane.need(p)), Plane.failed(Plane.need(p)), List.length(&2, PRow, Plane.run(p)))

# ids in the three thread sections, for law plane_each_once
def Plane.ids(rs: List<&2, PRow>) -> List<&2, String>:
  match rs:
    case Nil{}:
      Nil{}
    case PRow{k, +i, _, _, _, _, _, _, _, _} <> t:
      i <> Plane.ids(t)

# what a click on the first row sends ("act val"), for law plane_hit_opens
def Plane.act.of(rs: List<&2, PRow>) -> String:
  match rs:
    case PRow{_, _, +a, +v, _, _, _, _, _, _} <> _:
      a ++ " " ++ v
    case Nil{}:
      ""

# the machine of the second row in a section (for tests)
def Plane.machine.of(rs: List<&2, PRow>) -> String:
  match rs:
    case _ <> PRow{_, _, _, _, _, _, +m, _, _, _} <> _:
      m
    case _:
      ""

# need, run and the rest are each in urgency order: no row waits on the
# person less than the one after it (law plane_need_ranked)
def Plane.ranked.head(a: PRow, t: List<&2, PRow>) -> Bool:
  match t:
    case Nil{}:
      True{}
    case +b <> _:
      Nat.is_ge(Plane.rank(PRow.state(a)), Plane.rank(PRow.state(b)))

def Plane.ranked(rs: List<&2, PRow>) -> Bool:
  match rs:
    case Nil{}:
      True{}
    case +a <> +t:
      Bool.and(Plane.ranked.head(a, t), Plane.ranked(t))

# what a click on a thread's row sends, "action value" (law plane_hit_opens)
def Plane.hit(r: PRow) -> String:
  match r:
    case PRow{_, _, +a, +v, _, _, _, _, _, _}:
      a ++ " " ++ v

# every thread id in need, run and idle
def Plane.threads.ids(+p: Plane) -> List<&2, String>:
  List.append(&2, String, Plane.ids(Plane.need(p)), List.append(&2, String, Plane.ids(Plane.run(p)), Plane.ids(Plane.idle(p))))

# a client for the laws: threads in every state, a bot at work, a room, a routine
def Plane.demo() -> U.Ui:
  U.Ui.far.st(U.Ui.init(200000n), M.State.replay([M.ProjectCreated{"p1", "One", "/a", 1n},
    M.ThreadCreated{"t1", "p1", "A", M.Local{}, "", "", "claude", "", 199000n}, M.TurnChanged{"t1", M.Running{}, 199001n}, M.AskOpened{"a1", "t1", "approval", "Bash", "ls", "", 199002n},
    M.ThreadCreated{"t2", "p1", "B", M.Local{}, "", "", "claude", "", 199100n}, M.AskOpened{"a2", "t2", "input", "", "?", "", 199101n},
    M.ThreadCreated{"t3", "p1", "C", M.Local{}, "", "", "claude", "", 199200n}, M.TurnChanged{"t3", M.Failed{}, 199201n},
    M.ThreadCreated{"t4", "p1", "D", M.Local{}, "", "", "claude", "", 199300n}, M.TurnChanged{"t4", M.Running{}, 199301n},
    M.ThreadCreated{"t6", "p1", "F", M.Local{}, "", "", "claude", "", 199500n}, M.TurnChanged{"t6", M.Completed{}, 199501n},
    M.ThreadCreated{"bt1", "", "Kit", M.Local{}, "", "", "claude", "", 100n}, M.BotSet{"b1", "Kit", "bt1", 7n, "", 100n}, M.TurnChanged{"bt1", M.Running{}, 199900n},
    M.RoomSet{"r1", "ops", "b1", 150n}, M.RoomPosted{"m1", "r1", "b1", "hello", 0n, 199800n},
    M.RoutineSet{"rt1", "b1", "Digest", "0 9 * * *", "sum up", True{}, 160n}], M.State.empty()))

# a client for the laws on inherited blocks: a launcher at work, a thread it
# launched whose turn failed, and a child task of the launcher asking a
# question (law plane_launch_once)
def Plane.demo.launch() -> U.Ui:
  U.Ui.far.st(U.Ui.init(200000n), M.State.replay([M.ProjectCreated{"p1", "One", "/a", 1n},
    M.ThreadCreated{"l", "p1", "Launcher", M.Local{}, "", "", "claude", "", 199000n}, M.TurnChanged{"l", M.Running{}, 199001n},
    M.ThreadCreated{"f", "p1", "Launched", M.Local{}, "", "", "claude", "", 199100n}, M.ThreadLaunched{"f", "l", True{}, 199100n}, M.TurnChanged{"f", M.Failed{}, 199101n},
    M.ThreadCreated{"c", "p1", "Child", M.Local{}, "", "", "claude", "", 199200n}, M.TaskDelegated{"k1", "l", "c", 1n, "claude", "", 199200n},
    M.AskOpened{"a1", "c", "input", "", "?", "", 199201n}], M.State.empty()))

# The count alone
# ---------------
# The phones' list shows only "3 need you · 7 running" of the plane, so it
# counts instead of building rows (no titles, machines, sorts or JSON). A
# thread's phase is the costly part (five lookups), so it is worked out only
# for a thread that can be need or run: its turn running or failed, an
# approval pending, or its id among those that have an open ask, an unfinished
# child task, something queued or background rows (Plane.hot, from the index). Every other
# thread's phase is complete, stopped or idle. Law plane_tally_counts.

type PTally is Data:
  PTally{need: Nat, run: Nat, failed: Nat}

def PTally.need(t: PTally) -> Nat:
  match t:
    case PTally{n, _, _}:
      n

def PTally.run(t: PTally) -> Nat:
  match t:
    case PTally{_, r, _}:
      r

def PTally.failed(t: PTally) -> Nat:
  match t:
    case PTally{_, _, f}:
      f

# a state counted into the tally: need (0), run (1), the rest (2) as Plane.sec
def Plane.tally.add(n: Nat, t: PTally) -> PTally:
  match n t:
    case 0n PTally{+a, +b, +f}:
      PTally{1n+a, b, f}
    case 1n PTally{+a, +b, +f}:
      PTally{a, 1n+b, f}
    case _ x:
      x

def Plane.tally.fail(hit: Bool, t: PTally) -> PTally:
  match hit t:
    case True{} PTally{+a, +b, +f}:
      PTally{a, b, 1n+f}
    case False{} x:
      x

# a row's state into the tally, a failed run counted apart too
def Plane.tally.state(+s: String, t: PTally) -> PTally:
  Plane.tally.fail(String.eq(s, "failed"), Plane.tally.add(Plane.sec(s), t))

def Plane.hot.put.k(none: Bool, +id: String, m: Map<&2, Bool>) -> Map<&2, Bool>:
  match none:
    case True{}:
      m
    case False{}:
      Map.set(&2, Bool, m, id, True{})

def Plane.hot.put(+id: String, m: Map<&2, Bool>) -> Map<&2, Bool>:
  Plane.hot.put.k(String.is_empty(id), id, m)

def Plane.hot.ids(ids: List<&2, String>, m: Map<&2, Bool>) -> Map<&2, Bool>:
  match ids:
    case Nil{}:
      m
    case +i <> t:
      Plane.hot.ids(t, Plane.hot.put(i, m))

def Plane.hot.signals(xs: List<&2, String>, m: Map<&2, Bool>) -> Map<&2, Bool>:
  match xs:
    case Nil{}: m
    case +k <> rest:
      Plane.hot.signals(rest, Plane.hot.put(Bool.pick(String, String.starts_with(k, "@signal:"), String.drop(k, 8n), k), m))

def Plane.hot.sx(ix: U.SIx, m: Map<&2, Bool>) -> Map<&2, Bool>:
  match ix:
    case U.SIx{_, +as, +ks, +wk}:
      Plane.hot.ids(Map.keys(&2, List<&2, M.Ask>, as), Plane.hot.ids(Map.keys(&2, Nat, ks), Plane.hot.signals(Map.keys(&2, Nat, wk), m)))

def Plane.hot.stalled(keys: List<&2, String>, +st: M.State, +m: Map<&2, Bool>) -> Map<&2, Bool>:
  match keys:
    case Nil{}: m
    case +key <> rest:
      +id = String.drop(key, 9n)
      Plane.hot.stalled(rest, st, Bool.pick(Map<&2, Bool>, SG.Signal.display.stalled(st, id), Plane.hot.put(id, m), m))

# the ids that can be need or run beyond what a thread's own turn and pending say
def Plane.hot(+st: M.State, ix: U.SIx) -> Map<&2, Bool>:
  Plane.hot.sx(ix, Plane.hot.stalled(M.Setting.keys.pre(st, "liveness."), st, Plane.hot.ids(M.Queue.threads(st), Map.new(&2, Bool))))

def Plane.turn.hot(t: M.TurnState) -> Bool:
  match t:
    case M.Running{}:
      True{}
    case M.Failed{}:
      True{}
    case _:
      False{}

def Plane.hot.get(r: Map<&2, Bool> & Bool) -> Bool:
  (_, b) = r
  b

def Plane.hot.has(m: Map<&2, Bool>, +id: String) -> Bool:
  Plane.hot.get(Map.get(Bool, False{}, m, id))

def Plane.maybe.k(own: Bool, m: Map<&2, Bool>, +id: String) -> Bool:
  match own:
    case True{}:
      True{}
    case False{}:
      Plane.hot.has(m, id)

# whether a thread can be need or run (its phase is worked out only then)
def Plane.maybe(t: M.Thread, m: Map<&2, Bool>) -> Bool:
  match t:
    case M.Thread{+id, _, _, _, _, +turn, +pending, _, _, _, _, _, _, _, _, _, _, _, _, _, _}:
      Plane.maybe.k(Bool.or(Plane.turn.hot(turn), Nat.is_gt(pending, 0n)), m, id)

def Plane.tally.t(r: Bool, +t: M.Thread, +ix: U.SIx, +st: M.State, acc: PTally) -> PTally:
  match r:
    case True{}:
      Plane.tally.state(Plane.asking(Plane.signal(ix, st, t), ix, M.Thread.id(t)), acc)
    case False{}:
      acc

def Plane.tally.ts(ts: List<&2, M.Thread>, +ix: U.SIx, +st: M.State, +m: Map<&2, Bool>, acc: PTally) -> PTally:
  match ts:
    case Nil{}:
      acc
    case +t <> r:
      Plane.tally.ts(r, ix, st, m, Plane.tally.t(Plane.maybe(t, m), t, ix, st, acc))

# the live shelves of every project, as Active.gather walks them
def Plane.tally.rows(rs: List<&2, U.SRow>, +ix: U.SIx, +st: M.State, +m: Map<&2, Bool>, acc: PTally) -> PTally:
  match rs:
    case Nil{}:
      acc
    case U.SRow{_, _, M.Shelves{+p, +a, _, _, _}} <> t:
      Plane.tally.rows(t, ix, st, m, Plane.tally.ts(a, ix, st, m, Plane.tally.ts(p, ix, st, m, acc)))

# the bots' own threads count as the plane's bot rows do
def Plane.tally.bots(rs: List<&2, PRow>, acc: PTally) -> PTally:
  match rs:
    case Nil{}:
      acc
    case +r <> t:
      Plane.tally.bots(t, Plane.tally.state(PRow.state(r), acc))

# what the plane would count as needing the person and running, from the
# sidebar's rows (Side.rows.ix, which the screen has made)
def Plane.tally(+u: U.Ui, +ix: U.SIx, +rows: List<&2, U.SRow>) -> PTally:
  Plane.tally.bots(Plane.bots.go(BT.World.bots(M.State.bots(U.Ui.state(u))), u, ix, U.Ui.state(u)),
    Plane.tally.rows(rows, ix, U.Ui.state(u), Plane.hot(U.Ui.state(u), ix), PTally{0n, 0n, 0n}))

def Plane.counts.t(+l: I.Loc, +t: PTally) -> String:
  Plane.counts.of(l, PTally.need(t), PTally.failed(t), PTally.run(t))

# a client for the count's laws: what each way to be need or run looks like
# (an open approval, a question, a failed turn, a pending tool, a turn at work,
# a message queued, background rows, an unfinished child task, a bot at
# work), and threads that are neither
def Plane.demo.rich() -> U.Ui:
  U.Ui.far.st(U.Ui.init(200000n), M.State.replay([M.ProjectCreated{"p1", "One", "/a", 1n},
    M.ThreadCreated{"t1", "p1", "A", M.Local{}, "", "", "claude", "", 199000n}, M.TurnChanged{"t1", M.Running{}, 199001n}, M.AskOpened{"a1", "t1", "approval", "Bash", "ls", "", 199002n},
    M.ThreadCreated{"t2", "p1", "B", M.Local{}, "", "", "claude", "", 199100n}, M.AskOpened{"a2", "t2", "input", "", "?", "", 199101n},
    M.ThreadCreated{"t3", "p1", "C", M.Local{}, "", "", "claude", "", 199200n}, M.TurnChanged{"t3", M.Failed{}, 199201n},
    M.ThreadCreated{"t4", "p1", "D", M.Local{}, "", "", "claude", "", 199300n}, M.TurnChanged{"t4", M.Running{}, 199301n},
    M.ThreadCreated{"t5", "p1", "E", M.Local{}, "", "", "claude", "", 199400n}, M.TurnQueued{"t5", "q1", "next", 199401n},
    M.ThreadCreated{"t6", "p1", "F", M.Local{}, "", "", "claude", "", 199500n}, M.TurnChanged{"t6", M.Completed{}, 199501n},
    M.ThreadCreated{"t7", "p1", "G", M.Local{}, "", "", "claude", "", 199510n}, M.TurnChanged{"t7", M.Interrupted{}, 199511n},
    M.ThreadCreated{"t8", "p1", "H", M.Local{}, "", "", "claude", "", 199520n}, M.SettingSet{"bg.t8", "k1\tsleeping", 199521n},
    M.ThreadCreated{"t9", "p1", "I", M.Local{}, "", "", "claude", "", 199530n}, M.ThreadCreated{"t10", "p1", "J", M.Local{}, "", "", "claude", "", 199531n},
    M.TaskDelegated{"k2", "t9", "t10", 1n, "claude", "", 199532n},
    M.ThreadCreated{"t11", "p1", "K", M.Local{}, "", "", "claude", "", 199540n}, M.PendingChanged{"t11", 1n, 199541n},
    M.ThreadCreated{"t12", "p1", "L", M.Local{}, "", "", "claude", "", 100n},
    M.ThreadCreated{"t13", "p1", "M", M.Local{}, "", "", "claude", "", 199550n}, M.SettingSet{"watch.t13", "w1\twatching", 199551n},
    M.ThreadCreated{"bt1", "", "Kit", M.Local{}, "", "", "claude", "", 100n}, M.BotSet{"b1", "Kit", "bt1", 7n, "", 100n}, M.TurnChanged{"bt1", M.Running{}, 199900n},
    M.ThreadCreated{"bt2", "", "Pip", M.Local{}, "", "", "claude", "", 101n}, M.BotSet{"b2", "Pip", "bt2", 8n, "", 101n}, M.AskOpened{"a3", "bt2", "approval", "Bash", "ls", "", 199901n}], M.State.empty()))

# the count made without rows is the count of the plane
def Plane.tally.same(+u: U.Ui, +now: Nat) -> Bool:
  String.eq(Plane.counts.t(U.Lang.loc(u), Plane.tally(u, U.Side.ix(U.Ui.state(u)), U.Side.rows.ix(U.Ui.state(u), U.Side.ix(U.Ui.state(u)), now))), Plane.counts(U.Lang.loc(u), Plane.of(u, now)))

# (the index made once: each Side.ix walks every thread)
def Plane.tally.words.ix(+u: U.Ui, +ix: U.SIx, +now: Nat) -> String:
  Plane.counts.t(U.Lang.loc(u), Plane.tally(u, ix, U.Side.rows.ix(U.Ui.state(u), ix, now)))

def Plane.tally.words(+u: U.Ui, +now: Nat) -> String:
  Plane.tally.words.ix(u, U.Side.ix(U.Ui.state(u)), now)

# The plane as text
# -----------------
# What an agent reads with plane_read (bothub.bend's tool, fmcp.bend for
# another machine): each section a heading and a line a row, need first, then
# run, bots, rooms with news, routines, and the idle threads' count (or the
# threads, when asked). A row starts with the id the thread tools take; `pre`
# is the link of the machine that has it ("" for this one), so an id read on
# another machine already works here (<link>~<id>).

def Plane.text.id(+pre: String, +id: String) -> String:
  Bool.pick(String, String.is_empty(pre), id, FR.Fr.id(pre, id))

# " · s", or nothing for an empty s
def Plane.text.part(+s: String) -> String:
  Bool.pick(String, String.is_empty(s), "", " · " ++ s)

def Plane.text.ago(+now: Nat, +at: Nat) -> String:
  Bool.pick(String, Nat.is_lt(Nat.sub(now, at), 60n), "just now", M.Time.ago(now, at) ++ " ago")

# the last activity of a row, or when a routine is next due (`at` of a routine)
def Plane.text.when(+kind: String, +at: Nat, +now: Nat) -> String:
  Bool.pick(String, Nat.is_eq(at, 0n), "",
    Bool.pick(String, String.eq(kind, "routine"), Bool.pick(String, Nat.is_gt(at, now), "next in " ++ M.Time.ago(at, now), "due now"), Plane.text.ago(now, at)))

def Plane.text.row(r: PRow, +pre: String, +now: Nat) -> String:
  match r:
    case PRow{+kind, +id, _, _, +title, +sub, +machine, +state, +at, _}:
      "- " ++ Plane.text.id(pre, id) ++ " · " ++ state ++ " · " ++ title ++ Plane.text.part(sub) ++ Plane.text.part(Bool.pick(String, String.is_empty(machine), "", "on " ++ machine))
        ++ Plane.text.part(Plane.text.when(kind, at, now)) ++ "\n"

def Plane.text.rows(rs: List<&2, PRow>, +pre: String, +now: Nat) -> String:
  match rs:
    case Nil{}:
      ""
    case +r <> t:
      Plane.text.row(r, pre, now) ++ Plane.text.rows(t, pre, now)

def Plane.text.sec.k(none: Bool, +head: String, +rs: List<&2, PRow>, +pre: String, +now: Nat) -> String:
  match none:
    case True{}:
      ""
    case False{}:
      head ++ " (" ++ Nat.show(List.length(&2, PRow, rs)) ++ ")\n" ++ Plane.text.rows(rs, pre, now)

def Plane.text.sec(+head: String, +rs: List<&2, PRow>, +pre: String, +now: Nat) -> String:
  Plane.text.sec.k(List.is_empty(&2, PRow, rs), head, rs, pre, now)

def Plane.text.machs(ms: List<&2, PMach>) -> List<&2, String>:
  match ms:
    case Nil{}:
      Nil{}
    case PMach{+n, +s} <> t:
      (n ++ " (" ++ s ++ ")") <> Plane.text.machs(t)

# (only when there are other machines)
def Plane.text.machines.k(many: Bool, ms: List<&2, PMach>) -> String:
  match many:
    case True{}:
      "Machines: " ++ String.join(Plane.text.machs(ms), ", ") ++ "\n"
    case False{}:
      ""

def Plane.text.idle.k(shown: Bool, +n: Nat, rs: List<&2, PRow>, +pre: String, +now: Nat) -> String:
  match shown:
    case True{}:
      Plane.text.sec("Idle in the last day", rs, pre, now)
    case False{}:
      Bool.pick(String, Nat.is_eq(n, 0n), "", "Idle in the last day: " ++ Nat.show(n) ++ " (plane_read with idle: true lists them).\n")

def Plane.text(+p: Plane, +pre: String, +name: String, +now: Nat, +idle: Bool) -> String:
  "Status plane of " ++ name ++ "\n"
    ++ Bool.pick(String, Bool.and(List.is_empty(&2, PRow, Plane.need(p)), List.is_empty(&2, PRow, Plane.run(p))), "Nothing needs you and nothing is running.\n", "")
    ++ Plane.text.sec("Needs you", Plane.need(p), pre, now) ++ Plane.text.sec("Running", Plane.run(p), pre, now)
    ++ Plane.text.sec("Bots", Plane.bots(p), pre, now) ++ Plane.text.sec("Rooms with unread posts", Plane.rooms(p), pre, now)
    ++ Plane.text.sec("Locks", Plane.locks(p), pre, now) ++ Plane.text.sec("Routines", Plane.routines(p), pre, now) ++ Plane.text.idle.k(idle, Plane.idle_n(p), Plane.idle(p), pre, now)
    ++ Plane.text.machines.k(Nat.is_gt(List.length(&2, PMach, Plane.machines(p)), 1n), Plane.machines(p))

# where the needle first starts in the text (the text's length when it does not)
def Plane.text.at(t: String, +needle: String, +i: Nat, hit: Bool) -> Nat:
  match t hit:
    case _ True{}:
      i
    case SNil{} False{}:
      i
    case SCon{_, +rest} False{}:
      Plane.text.at(rest, needle, 1n+i, String.starts_with(rest, needle))

def Plane.text.pos(+t: String, +needle: String) -> Nat:
  Plane.text.at(t, needle, 0n, String.starts_with(t, needle))

# the latest and the earliest start of the rows of these ids ("- <id> ·")
def Plane.text.last(rs: List<&2, PRow>, +t: String, +acc: Nat) -> Nat:
  match rs:
    case Nil{}:
      acc
    case PRow{_, +i, _, _, _, _, _, _, _, _} <> r:
      Plane.text.last(r, t, M.Time.max(acc, Plane.text.pos(t, "- " ++ i ++ " ·")))

def Plane.text.first(rs: List<&2, PRow>, +t: String, +acc: Nat) -> Nat:
  match rs:
    case Nil{}:
      acc
    case PRow{_, +i, _, _, _, _, _, _, _, _} <> r:
      Plane.text.first(r, t, Bool.pick(Nat, Nat.is_le(acc, Plane.text.pos(t, "- " ++ i ++ " ·")), acc, Plane.text.pos(t, "- " ++ i ++ " ·")))

# every need row is before every run row in the text (law plane_text_need_first)
def Plane.text.need_first(+p: Plane, +now: Nat) -> Bool:
  Nat.is_lt(Plane.text.last(Plane.need(p), Plane.text(p, "", "x", now, True{}), 0n), Plane.text.first(Plane.run(p), Plane.text(p, "", "x", now, True{}), String.length(Plane.text(p, "", "x", now, True{}))))

# Where the conversation is held
# ------------------------------
# The uberbot's conversation runs on one machine, the lease holder (ubersync.bend).
# The Status strip says which ("Home: desk") and, on a machine that has the
# uberbot but not the conversation, offers "Make this the home" (the action
# uber-home, bots.uber_home). Whether to offer it is decided here, for every
# client (law plane_home_button).

type PHome is Data:
  PHome{on: Bool, name: String, here: Bool}

def PHome.on(h: PHome) -> Bool:
  match h:
    case PHome{o, _, _}:
      o

def PHome.here(h: PHome) -> Bool:
  match h:
    case PHome{_, _, he}:
      he

def PHome.name(h: PHome) -> String:
  match h:
    case PHome{_, n, _}:
      n

def Plane.home.label(+l: I.Loc) -> String:
  MSG.T.plane.home.make(l)

def Plane.home.peer(ps: List<&2, BT.Peer>, +link: String) -> String:
  match ps:
    case Nil{}:
      ""
    case +p <> t:
      Bool.pick(String, String.eq(BT.Peer.id(p), link), BT.Peer.name(p), Plane.home.peer(t, link))

# this machine's name, as the machine switcher has it
def Plane.machine.this(+u: U.Ui) -> String:
  Maybe.default(&2, String, List.head(&2, String, Plane.machine.names(U.Machines.rows(u))), MSG.T.machines.self(U.Lang.loc(u)))

# the holder's name: the name its link carries here, else the peer's
def Plane.home.other(+u: U.Ui, +link: String) -> String:
  Plane.mach.name(u, link, Bool.pick(String, String.is_empty(Plane.home.peer(BU.BU.peers.of(u), link)), MSG.T.plane.machine.other(U.Lang.loc(u)), Plane.home.peer(BU.BU.peers.of(u), link)))

def Plane.home.of(here: Bool, +u: U.Ui) -> String:
  match here:
    case True{}:
      Plane.machine.this(u)
    case False{}:
      Plane.home.other(u, US.Ub.link(U.Ui.state(u), US.Ub.holder(U.Ui.state(u))))

def Plane.home(+u: U.Ui) -> PHome:
  PHome{US.Ub.on(U.Ui.state(u)), Plane.home.of(US.Ub.holder.me(U.Ui.state(u)), u), US.Ub.holder.me(U.Ui.state(u))}

# the offer: an uberbot here, the conversation held elsewhere
def Plane.home.button(+h: PHome) -> Bool:
  Bool.and(PHome.on(h), Bool.not(PHome.here(h)))

# "Home: desk" ("" with no uberbot here)
def Plane.home.line(+l: I.Loc, +h: PHome) -> String:
  Bool.pick(String, PHome.on(h), MSG.T.plane.home.line(l, PHome.name(h)), "")

# the line and the offer, for the laws and tests
def Plane.home.words(+u: U.Ui) -> String:
  Plane.home.line(U.Lang.loc(u), Plane.home(u)) ++ Bool.pick(String, Plane.home.button(Plane.home(u)), " | " ++ Plane.home.label(U.Lang.loc(u)), "")

# The list's plane
# ----------------
# What the phones' home list draws (need, run, bots, rooms, routines; no idle
# threads) from the index and the sidebar's rows the screen has made: a
# thread's phase and row are worked out only for a thread that can be need
# or run (Plane.maybe). Law plane_list_same.

def Plane.list.row(+state: String, +t: M.Thread, +pt: String, +u: U.Ui, rest: List<&2, PRow>) -> List<&2, PRow>:
  Plane.keep(Nat.is_gt(Plane.sec(state), 1n), Plane.thread.k(U.Ui.far.away(u, M.Thread.id(t)), M.Thread.id(t), U.Active.title(t), pt, U.Ui.far.name(u, M.Thread.id(t)), state, U.Active.activity(t), U.Ui.state(u)), rest)

def Plane.list.keep(r: Bool, +t: M.Thread, +pt: String, +ix: U.SIx, +st: M.State, +u: U.Ui, rest: List<&2, PRow>) -> List<&2, PRow>:
  match r:
    case True{}:
      Plane.list.row(Plane.asking(Plane.signal(ix, st, t), ix, M.Thread.id(t)), t, pt, u, rest)
    case False{}:
      rest

def Plane.list.ts(ts: List<&2, M.Thread>, +pt: String, +ix: U.SIx, +st: M.State, +m: Map<&2, Bool>, +u: U.Ui, rest: List<&2, PRow>) -> List<&2, PRow>:
  match ts:
    case Nil{}:
      rest
    case +t <> r:
      Plane.list.keep(Plane.maybe(t, m), t, pt, ix, st, u, Plane.list.ts(r, pt, ix, st, m, u, rest))

def Plane.list.rows(rs: List<&2, U.SRow>, +ix: U.SIx, +st: M.State, +m: Map<&2, Bool>, +u: U.Ui) -> List<&2, PRow>:
  match rs:
    case Nil{}:
      Nil{}
    case U.SRow{_, M.Project{_, +ti, _, _, _}, M.Shelves{+p, +a, _, _, _}} <> t:
      Plane.list.ts(p, ti, ix, st, m, u, Plane.list.ts(a, ti, ix, st, m, u, Plane.list.rows(t, ix, st, m, u)))

def Plane.of.l(+u: U.Ui, +ix: U.SIx, +rows: List<&2, U.SRow>, +now: Nat) -> Plane:
  Plane.of.i(Nil{}, List.append(&2, PRow, Plane.list.rows(rows, ix, U.Ui.state(u), Plane.hot(U.Ui.state(u), ix), u), Plane.bots.go(BT.World.bots(M.State.bots(U.Ui.state(u))), u, ix, U.Ui.state(u))), u, ix, now)

# the list's need and run rows are the plane's, in the same order
def Plane.list.ids(+p: Plane) -> String:
  String.join(Plane.ids(Plane.need(p)), ",") ++ "|" ++ String.join(Plane.ids(Plane.run(p)), ",")

def Plane.list.same(+u: U.Ui, +now: Nat) -> Bool:
  String.eq(Plane.list.ids(Plane.of.l(u, U.Side.ix(U.Ui.state(u)), U.Side.rows.ix(U.Ui.state(u), U.Side.ix(U.Ui.state(u)), now), now)), Plane.list.ids(Plane.of(u, now)))

# Current visible plane, sharing the same projection as sidebar and timeline.
# The old Phase API remains decodable/testable for older clients.
def Plane.signals.bot(r: PRow, +u: U.Ui) -> PRow:
  match r:
    case PRow{k, +id, a, v, ti, sub, ma, +state, at, +away}:
      +thread = BT.Bot.thread(BT.World.bot(M.State.bots(U.Ui.state(u)), id))
      PRow{k, id, a, v, ti, sub, ma, Bool.pick(String, away, state, Plane.signal.current(SG.Signal.cached(U.Ui.infos(u), thread), U.Ui.thread(U.Ui.state(u), thread), U.Side.phase(U.Ui.state(u), U.Ui.thread(U.Ui.state(u), thread)), U.Ui.state(u))), at, away}

def Plane.signals.bots.rows(rs: List<&2, PRow>, +u: U.Ui) -> List<&2, PRow>:
  match rs:
    case Nil{}: Nil{}
    case r <> rest: Plane.signals.bot(r, u) <> Plane.signals.bots.rows(rest, u)

def Plane.signals.bots(p: Plane, +u: U.Ui) -> Plane:
  match p:
    case Plane{ne, ru, bots, ro, rt, id, n, ma, lo}:
      Plane{ne, ru, Plane.signals.bots.rows(bots, u), ro, rt, id, n, ma, lo}

# Show identities/titles, never a watched command or inferred progress. Ended
# rows may still await delivery; they are deliberately absent from active work.
def Plane.await.rows(xs: List<&2, String>, +watch: Bool) -> List<&2, String>:
  match xs:
    case Nil{}: Nil{}
    case +r <> rest:
      Plane.keep.text(Bool.and(Bool.not(M.Bg.ended(r)), Bool.or(Bool.not(watch), SG.Signal.watch([r]))),
        M.Bg.field(r, 0n) ++ " (" ++ M.Bg.title(r) ++ ": " ++ M.Bg.doing(r) ++ ")", Plane.await.rows(rest, watch))

def Plane.await.children(ids: List<&2, String>, +u: U.Ui) -> List<&2, String>:
  match ids:
    case Nil{}: Nil{}
    case +id <> rest:
      Plane.keep.text(SG.Signal.active(SG.Signal.cached(U.Ui.infos(u), id)), id ++ " (" ++ U.Active.title(U.Ui.thread(U.Ui.state(u), id)) ++ ")", Plane.await.children(rest, u))

def Plane.await.tasks(ts: List<&2, M.Task>, +launched: List<&2, String>) -> List<&2, String>:
  match ts:
    case Nil{}: Nil{}
    case M.Task{_, _, +child, _, state, _, _, _, _} <> rest:
      Plane.keep.text(Bool.and(Bool.not(M.TaskState.terminal(state)), Bool.not(M.Launch.has(launched, child))), child, Plane.await.tasks(rest, launched))

def Plane.await.work(+u: U.Ui, +th: String) -> List<&2, String>:
  +st = U.Ui.state(u)
  +launched = M.Launch.of(st, th)
  List.concat(&2, String, [Plane.await.rows(M.Bg.of(st, th), False{}), Plane.await.rows(M.Work.watch(st, th), True{}),
    Plane.await.children(List.append(&2, String, launched, Plane.await.tasks(U.Tasks.of(st, th), launched)), u)])

def Plane.await.wakes(ids: List<&2, String>, +st: M.State) -> List<&2, String>:
  match ids:
    case Nil{}: Nil{}
    case +id <> rest:
      +o = M.Wake.get(st, id)
      (id ++ " (" ++ J.Json.get_str(o, "kind", "") ++ " " ++ J.Json.get_str(o, "target", "") ++ ")") <> Plane.await.wakes(rest, st)

def Plane.observed.sub(+sub: String, +th: String, +u: U.Ui) -> String:
  +st = U.Ui.state(u)
  +jp = String.eq(I.I18n.tag(U.Lang.loc(u)), "ja")
  +work = String.join(Plane.await.work(u, th), "; ")
  +wake = String.join(Plane.await.wakes(M.Wake.ids(st, "wakes.by." ++ th), st), "; ")
  sub ++ " · " ++ Bool.pick(String, jp, "エージェント: ", "agent: ") ++ C.Turn.name(M.Thread.turn(U.Ui.thread(st, th)))
    ++ Bool.pick(String, String.is_empty(work), "", Bool.pick(String, jp, " · 待機中: ", " · work: ") ++ work)
    ++ Bool.pick(String, String.is_empty(wake), "", Bool.pick(String, jp, " · 通知待ち: ", " · wake on: ") ++ wake)

def Plane.observed.sub.if(show: Bool, +sub: String, th: String, u: U.Ui) -> String:
  match show:
    case True{}: Plane.observed.sub(sub, th, u)
    case False{}: sub

def Plane.observed.row(r: PRow, +u: U.Ui) -> PRow:
  match r:
    case PRow{+k, +id, a, v, ti, +sub, ma, state, at, +away}:
      +th = Bool.pick(String, String.eq(k, "bot"), BT.Bot.thread(BT.World.bot(M.State.bots(U.Ui.state(u)), id)), id)
      PRow{k, id, a, v, ti, Plane.observed.sub.if(Bool.and(Bool.not(away), Bool.or(String.eq(k, "thread"), String.eq(k, "bot"))), sub, th, u), ma, state, at, away}

def Plane.observed.rows(rs: List<&2, PRow>, +u: U.Ui) -> List<&2, PRow>:
  match rs:
    case Nil{}: Nil{}
    case r <> rest: Plane.observed.row(r, u) <> Plane.observed.rows(rest, u)

def Plane.observed(p: Plane, +u: U.Ui) -> Plane:
  match p:
    case Plane{ne, ru, bots, ro, rt, id, n, ma, lo}:
      Plane{Plane.observed.rows(ne, u), Plane.observed.rows(ru, u), Plane.observed.rows(bots, u), ro, rt, Plane.observed.rows(id, u), n, ma, lo}

def Plane.signals.with(+u: U.Ui, +now: Nat, idle: Bool) -> Plane:
  Plane.observed(Plane.signals.bots(Plane.of.f(u, U.Side.signals.ix(U.Side.ix(U.Ui.state(u)), u), now, idle), u), u)

def Plane.signals.of(u: U.Ui, now: Nat, idle: Bool) -> Plane:
  Plane.signals.with(U.Side.with_signals(u), now, idle)

# Hub reads and all display clients use the same current activity projection.
# Retained ended rows and queued/unknown watches never make a completed agent
# work. The persisted turn, delivery queue and legacy Phase API stay unchanged.
def Plane.of.st(+st: M.State, +now: Nat) -> Plane:
  Plane.signals.of(U.Ui.online(U.Ui.far.st(U.Ui.init(now), st), True{}), now, True{})

def Plane.read(+st: M.State, +now: Nat, +pre: String, +name: String, +idle: Bool) -> String:
  Plane.text(Plane.of.st(st, now), pre, name, now, idle)

# the need rows of the current plane, "id=state" in order (laws, tests)
def Plane.need.states.go(rs: List<&2, PRow>) -> List<&2, String>:
  match rs:
    case Nil{}: Nil{}
    case +r <> t: (PRow.id(r) ++ "=" ++ PRow.state(r)) <> Plane.need.states.go(t)

def Plane.need.states(u: U.Ui, now: Nat) -> String:
  String.join(Plane.need.states.go(Plane.need(Plane.signals.of(u, now, True{}))), ",")
