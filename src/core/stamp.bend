import Base
import ./i18n.bend as I
import ./msgs.bend as MSG
import ./model.bend as M
import ./cron.bend as CR
import ./tz.bend as TZ
import ./sched.bend as SCH
import ./steer.bend as STR

# When messages were sent (pure)
# ===============================
# Every client shows under a message the time it was sent, in the person's
# zone: "14:05" today, "Mon 13 Oct 14:05" before. A turn's last reply, once
# the turn is over, says when it finished and how long the turn took
# ("Finished 14:12 · 7m 3s"). A turn is a person's message and what came
# after it, up to their next one. A page that starts mid-turn has no
# message to count from, so its last reply says only when it finished.

# today: the day it is now on the person's wall clock (days since the
# epoch), worked out once a page; stops: the replies that end a turn the
# person stopped (to 1), which say "Stopped" where the others say
# "Finished", and the last turn's last reply while the thread waits on
# work it started (to 2: "Waiting since", never "Finished" before it is)
type Stamps is Data:
  Stamps{rule: TZ.TzRule, today: Nat, ends: Map<&2, Nat>, stops: Map<&2, Nat>}

# Turns' last replies
# -------------------
# ends: each finished turn's last reply (its id) to when the turn started
# (1 when the page does not hold its start). Entries newest first: `open`
# while the reply that ends the turn is still to be found, `held` that
# reply until the message that started its turn comes.

def Stamp.close(empty: Bool, +held: String, +at: Nat, ends: Map<&2, Nat>) -> Map<&2, Nat>:
  match empty:
    case True{}:
      ends
    case False{}:
      Map.set(&2, Nat, ends, held, at)

def Stamp.held(open: Bool, +held: String, +id: String) -> String:
  match open:
    case True{}:
      id
    case False{}:
      held

# (a reply posted ahead of a message sent while it streamed, steer.bend's,
# ends no turn: the agent went on with it after the message)
def Stamp.ends.go(es: List<&2, M.Entry>, +open: Bool, +held: String, ends: Map<&2, Nat>) -> Map<&2, Nat>:
  match es:
    case Nil{}:
      Stamp.close(String.is_empty(held), held, 1n, ends)
    case M.Msg{_, _, M.User{}, _, +at} <> t:
      Stamp.ends.go(t, True{}, "", Stamp.close(String.is_empty(held), held, at, ends))
    case M.Msg{+id, _, M.Assistant{}, _, _} <> t:
      Stamp.ends.go(t, Bool.and(open, STR.Steer.ahead.is(id)), Bool.pick(String, STR.Steer.ahead.is(id), held, Stamp.held(open, held, id)), ends)
    case _ <> t:
      Stamp.ends.go(t, open, held, ends)

# entries oldest first; while the thread's turn runs its last turn has not
# ended
def Stamp.ends(+es: List<&2, M.Entry>, +running: Bool) -> Map<&2, Nat>:
  Stamp.ends.go(List.reverse(&2, M.Entry, es), Bool.not(running), "", Map.new(&2, Nat))

def Stamps.at.s(+r: TZ.TzRule, +now: Nat, ends: Map<&2, Nat>, stops: Map<&2, Nat>) -> Stamps:
  Stamps{r, Nat.div(TZ.Tz.wall(r, now), 86400n), ends, stops}

def Stamps.at(+r: TZ.TzRule, +now: Nat, ends: Map<&2, Nat>) -> Stamps:
  Stamps.at.s(r, now, ends, Map.new(&2, Nat))

# the last turn's last reply, entries newest first ("" when the newest is
# the person's: that turn has no reply)
def Stamp.last.go(es: List<&2, M.Entry>) -> String:
  match es:
    case Nil{}:
      ""
    case M.Msg{_, _, M.User{}, _, _} <> t:
      ""
    case M.Msg{+id, _, M.Assistant{}, _, _} <> t:
      id
    case _ <> t:
      Stamp.last.go(t)

def Stamp.interrupted(t: M.TurnState) -> Bool:
  match t:
    case M.Interrupted{}:
      True{}
    case _:
      False{}

# the thread's last turn was stopped (Stop, an interrupt)
def Stamp.stopped(t: M.Thread) -> Bool:
  match t:
    case M.Thread{_, _, _, _, _, turn, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _}:
      Stamp.interrupted(turn)

# Stopped turns
# -------------
# A turn keeps its own ending after the next one starts: when a turn is
# stopped the hub sets "stopped.<message>" for the person's message that
# started it (Stamp.stop.mark, from hub.bend's Retire.turn.f), a setting
# like any other, so every client has it and it outlives a restart. A
# reply still landing after the Stop is in the same turn, so it is the
# message, not the reply, that is marked.

def Stamp.stop.key(+msg: String) -> String:
  "stopped." ++ msg

# a person's message of the thread (its id), "" for any other entry
def Stamp.user(e: M.Entry, +th: String) -> String:
  match e:
    case M.Msg{+id, +t, M.User{}, _, _}:
      Bool.pick(String, String.eq(t, th), id, "")
    case _:
      ""

# entries newest first; n: how many more to look at (0 once found)
def Stamp.turn.go(es: List<&2, M.Entry>, +th: String, n: Nat, +found: String) -> String:
  match es n:
    case +_ 0n:
      found
    case Nil{} 1n+_:
      found
    case +e <> t 1n+k:
      Stamp.turn.go(t, th, Bool.pick(Nat, String.is_empty(Stamp.user(e, th)), k, 0n), Stamp.user(e, th))

# the person's message that started the thread's turn ("" for none among
# its newest 5000 entries)
def Stamp.turn(+st: M.State, +th: String) -> String:
  Stamp.turn.go(M.State.mine(st, th), th, 5000n, "")

def Stamp.stop.mark.k(+msg: String, +now: Nat) -> List<&2, M.Change>:
  Bool.pick(List<&2, M.Change>, String.is_empty(msg), Nil{}, [M.SettingSet{Stamp.stop.key(msg), "1", now}])

# what the hub logs as it stops a thread's turn
def Stamp.stop.mark(+st: M.State, +th: String, +now: Nat) -> List<&2, M.Change>:
  Stamp.stop.mark.k(Stamp.turn(st, th), now)

def Stamp.stops.close(stopped: Bool, +held: String, stops: Map<&2, Nat>) -> Map<&2, Nat>:
  match stopped:
    case True{}:
      Map.set(&2, Nat, stops, held, 1n)
    case False{}:
      stops

def Stamp.marked(+st: M.State, +msg: String) -> Bool:
  String.eq(M.Setting.get(st, Stamp.stop.key(msg), ""), "1")

# entries newest first, walked as Stamp.ends.go: at a person's message the
# reply held ends its turn, stopped when the message is marked
def Stamp.stops.go(es: List<&2, M.Entry>, +st: M.State, +open: Bool, +held: String, stops: Map<&2, Nat>) -> Map<&2, Nat>:
  match es:
    case Nil{}:
      stops
    case M.Msg{+id, _, M.User{}, _, _} <> t:
      Stamp.stops.go(t, st, True{}, "", Stamp.stops.close(Bool.and(Bool.not(String.is_empty(held)), Stamp.marked(st, id)), held, stops))
    case M.Msg{+id, _, M.Assistant{}, _, _} <> t:
      Stamp.stops.go(t, st, Bool.and(open, STR.Steer.ahead.is(id)), Bool.pick(String, STR.Steer.ahead.is(id), held, Stamp.held(open, held, id)), stops)
    case _ <> t:
      Stamp.stops.go(t, st, open, held, stops)

# the stopped turns' last replies on a page (entries oldest first)
def Stamp.stops(+es: List<&2, M.Entry>, +st: M.State, +running: Bool) -> Map<&2, Nat>:
  Stamp.stops.go(List.reverse(&2, M.Entry, es), st, Bool.not(running), "", Map.new(&2, Nat))

# the last turn, stopped (its message may not be marked: a log from before
# the marks, or an interrupt that marks none) and over
def Stamp.stops.last(stopped: Bool, +es: List<&2, M.Entry>, stops: Map<&2, Nat>) -> Map<&2, Nat>:
  match stopped:
    case True{}:
      Stamp.stops.close(Bool.not(String.is_empty(Stamp.last.go(List.reverse(&2, M.Entry, es)))), Stamp.last.go(List.reverse(&2, M.Entry, es)), stops)
    case False{}:
      stops

# the last turn's last reply says "Waiting since" while the thread waits
# on work it started (signal.bend's waiting), unless it was stopped
def Stamp.kind.at(r: Map<&2, Nat> & Nat) -> Nat:
  (_, v) = r
  v

def Stamp.waits.put(free: Bool, +id: String, stops: Map<&2, Nat>) -> Map<&2, Nat>:
  match free:
    case True{}:
      Map.set(&2, Nat, stops, id, 2n)
    case False{}:
      stops

def Stamp.waits.k(+id: String, +stops: Map<&2, Nat>) -> Map<&2, Nat>:
  Stamp.waits.put(Bool.and(Bool.not(String.is_empty(id)), Nat.is_eq(Stamp.kind.at(Map.get(Nat, 0n, stops, id)), 0n)), id, stops)

def Stamp.waits.last(waiting: Bool, +es: List<&2, M.Entry>, stops: Map<&2, Nat>) -> Map<&2, Nat>:
  match waiting:
    case True{}:
      Stamp.waits.k(Stamp.last.go(List.reverse(&2, M.Entry, es)), stops)
    case False{}:
      stops

# what a client needs to stamp a page of entries: the person's zone, the
# time now (seconds), the turns' ends and which of them were stopped
# (stopped: the thread's last turn was)
def Stamps.of.s(+es: List<&2, M.Entry>, +st: M.State, +now: Nat, +running: Bool, +stopped: Bool) -> Stamps:
  Stamps.at.s(SCH.Sched.person(st, 0n, True{}), now, Stamp.ends(es, running),
    Stamp.stops.last(Bool.and(stopped, Bool.not(running)), es, Stamp.stops(es, st, running)))

# the same while the thread waits on work it started (waiting)
def Stamps.of.w(+es: List<&2, M.Entry>, +st: M.State, +now: Nat, +running: Bool, +stopped: Bool, waiting: Bool) -> Stamps:
  Stamps.at.s(SCH.Sched.person(st, 0n, True{}), now, Stamp.ends(es, running),
    Stamp.waits.last(Bool.and(waiting, Bool.not(running)), es, Stamp.stops.last(Bool.and(stopped, Bool.not(running)), es, Stamp.stops(es, st, running))))

def Stamps.of(+es: List<&2, M.Entry>, +st: M.State, +now: Nat, +running: Bool) -> Stamps:
  Stamps.of.s(es, st, now, running, False{})

# Showing
# -------

def Stamp.hm(+w: Nat) -> String:
  CR.Cron.hm(Nat.div(Nat.mod(w, 86400n), 3600n), Nat.div(Nat.mod(w, 3600n), 60n))

def Stamp.clock.k(today: Bool, +w: Nat) -> String:
  match today:
    case True{}:
      Stamp.hm(w)
    case False{}:
      TZ.Tz.show.dt(w)

# "14:05" on the day it is now, "Mon 13 Oct 14:05" on another
def Stamp.clock(+r: TZ.TzRule, +today: Nat, +at: Nat) -> String:
  +w = TZ.Tz.wall(r, at)
  Stamp.clock.k(Nat.is_eq(Nat.div(w, 86400n), today), w)

def Stamp.done.k(+l: I.Loc, known: Bool, +clock: String, +start: Nat, +at: Nat) -> String:
  match known:
    case True{}:
      MSG.T.stamp.finished.took(l, clock, I.I18n.took(l, Nat.sub(at, start)))
    case False{}:
      MSG.T.stamp.finished(l, clock)

# a reply: when it came, and for a turn's last one when the turn finished
# (start 0: not the last)
def Stamp.reply(+l: I.Loc, +start: Nat, +clock: String, +at: Nat) -> String:
  Bool.pick(String, Nat.is_eq(start, 0n), clock, Stamp.done.k(l, Nat.is_gt(start, 1n), clock, start, at))

def Stamp.stop.k(+l: I.Loc, known: Bool, +clock: String, +start: Nat, +at: Nat) -> String:
  match known:
    case True{}:
      MSG.T.stamp.stopped.took(l, clock, I.I18n.took(l, Nat.sub(at, start)))
    case False{}:
      MSG.T.stamp.stopped(l, clock)

def Stamp.wait.k(+l: I.Loc, known: Bool, +clock: String, +start: Nat, +at: Nat) -> String:
  match known:
    case True{}:
      MSG.T.stamp.waiting.took(l, clock, I.I18n.took(l, Nat.sub(at, start)))
    case False{}:
      MSG.T.stamp.waiting(l, clock)

# the stopped turn's last reply says "Stopped 14:12 · 4s"
def Stamp.reply.s(stopped: Bool, +l: I.Loc, +start: Nat, +clock: String, +at: Nat) -> String:
  match stopped:
    case True{}:
      Stamp.stop.k(l, Nat.is_gt(start, 1n), clock, start, at)
    case False{}:
      Stamp.reply(l, start, clock, at)

# a turn's last reply by how the turn ended (stops' value: 0 finished,
# 1 stopped, 2 waiting on work it started: "Waiting since 14:12 · 7m 3s")
def Stamp.reply.w(waits: Bool, +kind: Nat, +l: I.Loc, +start: Nat, +clock: String, +at: Nat) -> String:
  match waits:
    case True{}:
      Bool.pick(String, Nat.is_eq(start, 0n), clock, Stamp.wait.k(l, Nat.is_gt(start, 1n), clock, start, at))
    case False{}:
      Stamp.reply.s(Nat.is_eq(kind, 1n), l, start, clock, at)

def Stamp.reply.k(+kind: Nat, +l: I.Loc, +start: Nat, +clock: String, +at: Nat) -> String:
  Stamp.reply.w(Nat.is_eq(kind, 2n), kind, l, start, clock, at)

def Stamp.start(r: Map<&2, Nat> & Nat) -> Nat:
  (_, v) = r
  v

def Stamp.has(r: Map<&2, Nat> & Bool) -> Bool:
  (_, yes) = r
  yes

# what shows under an entry: a message's time, nothing for the rest
def Stamp.text.loc(+l: I.Loc, +s: Stamps, e: M.Entry) -> String:
  match s e:
    case Stamps{+r, +today, _, _} M.Msg{_, _, M.User{}, _, +at}:
      Stamp.clock(r, today, at)
    case Stamps{+r, +today, +ends, +stops} M.Msg{+id, _, M.Assistant{}, _, +at}:
      Stamp.reply.k(Stamp.start(Map.get(Nat, 0n, stops, id)), l, Stamp.start(Map.get(Nat, 0n, ends, id)), Stamp.clock(r, today, at), at)
    case _ _:
      ""

def Stamp.text(+s: Stamps, e: M.Entry) -> String:
  Stamp.text.loc(I.En{}, s, e)
