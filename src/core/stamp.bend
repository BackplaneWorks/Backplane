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
# "Finished", the last turn's last reply while the thread waits on
# work it started (to 2: "Waiting since", never "Finished" before it is)
# and the last turn's when it failed (to 3: "Failed")
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

# the newest reply after the person's newest message, entries newest
# first ("" for none)
def Stamp.newest(es: List<&2, M.Entry>) -> String:
  match es:
    case Nil{}:
      ""
    case M.Msg{_, _, M.User{}, _, _} <> t:
      ""
    case M.Msg{+id, _, M.Assistant{}, _, _} <> t:
      id
    case _ <> t:
      Stamp.newest(t)

def Stamp.live.k(+id: String, ends: Map<&2, Nat>) -> Map<&2, Nat>:
  Stamp.close(String.is_empty(id), id, 0n, ends)

# while a turn runs, its newest reply so far is kept in ends with 0: it is
# no turn's end (its stamp is its time), but its time stays in sight
# (Stamp.mid)
def Stamp.live(running: Bool, +es: List<&2, M.Entry>, ends: Map<&2, Nat>) -> Map<&2, Nat>:
  match running:
    case True{}:
      Stamp.live.k(Stamp.newest(List.reverse(&2, M.Entry, es)), ends)
    case False{}:
      ends

# entries oldest first; while the thread's turn runs its last turn has not
# ended (its newest reply is kept with 0, Stamp.live)
def Stamp.ends(+es: List<&2, M.Entry>, +running: Bool) -> Map<&2, Nat>:
  Stamp.live(running, es, Stamp.ends.go(List.reverse(&2, M.Entry, es), Bool.not(running), "", Map.new(&2, Nat)))

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

def Stamp.end.k(t: M.TurnState) -> Nat:
  match t:
    case M.Interrupted{}:
      1n
    case M.Failed{}:
      3n
    case _:
      0n

# how the thread's last turn ended, as the stops' kinds: 1 stopped (Stop,
# an interrupt), 3 failed (an error ended it), 0 otherwise
def Stamp.end(t: M.Thread) -> Nat:
  match t:
    case M.Thread{_, _, _, _, _, turn, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _}:
      Stamp.end.k(turn)

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

# A failed turn keeps "Failed" the same way: when an error ends a turn the
# hub sets "failed.<message>" for the message that started it
# (Stamp.fail.mark, from hub.bend's Ended.out.p), so a retry or the next
# message does not turn it into "Finished".
def Stamp.fail.key(+msg: String) -> String:
  "failed." ++ msg

def Stamp.fail.mark.k(+msg: String, +now: Nat) -> List<&2, M.Change>:
  Bool.pick(List<&2, M.Change>, String.is_empty(msg), Nil{}, [M.SettingSet{Stamp.fail.key(msg), "1", now}])

# what the hub logs as an error ends a thread's turn (failed: it did)
def Stamp.fail.mark(failed: Bool, +st: M.State, +th: String, +now: Nat) -> List<&2, M.Change>:
  match failed:
    case True{}:
      Stamp.fail.mark.k(Stamp.turn(st, th), now)
    case False{}:
      Nil{}

def Stamp.stops.close(stopped: Bool, +held: String, stops: Map<&2, Nat>) -> Map<&2, Nat>:
  match stopped:
    case True{}:
      Map.set(&2, Nat, stops, held, 1n)
    case False{}:
      stops

def Stamp.marked(+st: M.State, +msg: String) -> Bool:
  String.eq(M.Setting.get(st, Stamp.stop.key(msg), ""), "1")

# how the turn a message started ended, by its marks: 1 stopped, 3 failed,
# 0 neither
def Stamp.mark.kind(+st: M.State, +msg: String) -> Nat:
  Bool.pick(Nat, Stamp.marked(st, msg), 1n, Bool.pick(Nat, String.eq(M.Setting.get(st, Stamp.fail.key(msg), ""), "1"), 3n, 0n))

# the last turn, stopped (its message may not be marked: a log from before
# the marks, or an interrupt that marks none) and over
def Stamp.stops.last(stopped: Bool, +es: List<&2, M.Entry>, stops: Map<&2, Nat>) -> Map<&2, Nat>:
  match stopped:
    case True{}:
      Stamp.stops.close(Bool.not(String.is_empty(Stamp.last.go(List.reverse(&2, M.Entry, es)))), Stamp.last.go(List.reverse(&2, M.Entry, es)), stops)
    case False{}:
      stops

def Stamp.ended.put(found: Bool, +kind: Nat, +held: String, stops: Map<&2, Nat>) -> Map<&2, Nat>:
  match found:
    case True{}:
      Map.set(&2, Nat, stops, held, kind)
    case False{}:
      stops

def Stamp.stops.at(+kind: Nat, +held: String, stops: Map<&2, Nat>) -> Map<&2, Nat>:
  Stamp.ended.put(Bool.and(Nat.is_gt(kind, 0n), Bool.not(String.is_empty(held))), kind, held, stops)

# entries newest first, walked as Stamp.ends.go: at a person's message the
# reply held ends its turn, stopped or failed when the message is marked
def Stamp.stops.go(es: List<&2, M.Entry>, +st: M.State, +open: Bool, +held: String, stops: Map<&2, Nat>) -> Map<&2, Nat>:
  match es:
    case Nil{}:
      stops
    case M.Msg{+id, _, M.User{}, _, _} <> t:
      Stamp.stops.go(t, st, True{}, "", Stamp.stops.at(Stamp.mark.kind(st, id), held, stops))
    case M.Msg{+id, _, M.Assistant{}, _, _} <> t:
      Stamp.stops.go(t, st, Bool.and(open, STR.Steer.ahead.is(id)), Bool.pick(String, STR.Steer.ahead.is(id), held, Stamp.held(open, held, id)), stops)
    case _ <> t:
      Stamp.stops.go(t, st, open, held, stops)

# the stopped turns' last replies on a page (entries oldest first)
def Stamp.stops(+es: List<&2, M.Entry>, +st: M.State, +running: Bool) -> Map<&2, Nat>:
  Stamp.stops.go(List.reverse(&2, M.Entry, es), st, Bool.not(running), "", Map.new(&2, Nat))

# the last turn, over, ended as kind says (Stamp.end: 1 stopped, its
# message maybe unmarked as Stamp.stops.last; 3 failed; 0 changes nothing)
def Stamp.ended.last(+kind: Nat, +es: List<&2, M.Entry>, stops: Map<&2, Nat>) -> Map<&2, Nat>:
  +id = Stamp.last.go(List.reverse(&2, M.Entry, es))
  Stamp.ended.put(Bool.and(Nat.is_gt(kind, 0n), Bool.not(String.is_empty(id))), kind, id, stops)

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

# the same by how the thread's last turn ended (end: Stamp.end), so a
# failed turn's last reply says "Failed"
def Stamps.of.e(+es: List<&2, M.Entry>, +st: M.State, +now: Nat, +running: Bool, +end: Nat) -> Stamps:
  Stamps.at.s(SCH.Sched.person(st, 0n, True{}), now, Stamp.ends(es, running),
    Stamp.ended.last(Bool.pick(Nat, running, 0n, end), es, Stamp.stops(es, st, running)))

# the same while the thread waits on work it started (waiting)
def Stamps.of.w(+es: List<&2, M.Entry>, +st: M.State, +now: Nat, +running: Bool, +end: Nat, waiting: Bool) -> Stamps:
  Stamps.at.s(SCH.Sched.person(st, 0n, True{}), now, Stamp.ends(es, running),
    Stamp.waits.last(Bool.and(waiting, Bool.not(running)), es, Stamp.ended.last(Bool.pick(Nat, running, 0n, end), es, Stamp.stops(es, st, running))))

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

def Stamp.fail.k(+l: I.Loc, known: Bool, +clock: String, +start: Nat, +at: Nat) -> String:
  match known:
    case True{}:
      MSG.T.stamp.failed.took(l, clock, I.I18n.took(l, Nat.sub(at, start)))
    case False{}:
      MSG.T.stamp.failed(l, clock)

# a failed turn's last reply says "Failed 14:12 · 22s"
def Stamp.reply.f(failed: Bool, +l: I.Loc, +start: Nat, +clock: String, +at: Nat) -> String:
  match failed:
    case True{}:
      Bool.pick(String, Nat.is_eq(start, 0n), clock, Stamp.fail.k(l, Nat.is_gt(start, 1n), clock, start, at))
    case False{}:
      Stamp.reply(l, start, clock, at)

# the stopped turn's last reply says "Stopped 14:12 · 4s"
def Stamp.reply.s(stopped: Bool, +kind: Nat, +l: I.Loc, +start: Nat, +clock: String, +at: Nat) -> String:
  match stopped:
    case True{}:
      Stamp.stop.k(l, Nat.is_gt(start, 1n), clock, start, at)
    case False{}:
      Stamp.reply.f(Nat.is_eq(kind, 3n), l, start, clock, at)

# a turn's last reply by how the turn ended (stops' value: 0 finished,
# 1 stopped, 2 waiting on work it started: "Waiting since 14:12 · 7m 3s",
# 3 failed)
def Stamp.reply.w(waits: Bool, +kind: Nat, +l: I.Loc, +start: Nat, +clock: String, +at: Nat) -> String:
  match waits:
    case True{}:
      Bool.pick(String, Nat.is_eq(start, 0n), clock, Stamp.wait.k(l, Nat.is_gt(start, 1n), clock, start, at))
    case False{}:
      Stamp.reply.s(Nat.is_eq(kind, 1n), kind, l, start, clock, at)

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

# Messages not read yet
# ---------------------
# A person's message sent while the agent works waits until the command
# ahead of it ends (the hub keeps the waiting ones in the setting
# wait.<thread>, hub.bend's Wait.*). While its thread runs it says so:
# "not read yet · 06:13", a step dimmer (view.bend's class unread); it
# reads as any other the moment the agent picks it up (laws wait_*).

def Stamp.waits(+ids: String, +id: String) -> Bool:
  String.contains(" " ++ ids ++ " ", " " ++ id ++ " ")

def Stamp.unread.k(none: Bool, +s: M.State, +th: String, +ids: String, +id: String) -> Bool:
  match none:
    case True{}:
      False{}
    case False{}:
      Bool.and(Stamp.waits(ids, id), M.State.running.k(M.State.thread.get(s, th)))

def Stamp.unread.ids(+s: M.State, +th: String, +id: String, +ids: String) -> Bool:
  Stamp.unread.k(String.is_empty(ids), s, th, ids, id)

def Stamp.unread(+s: M.State, e: M.Entry) -> Bool:
  match e:
    case M.Msg{+id, +th, M.User{}, _, _}:
      Stamp.unread.ids(s, th, id, M.Setting.get(s, "wait." ++ th, ""))
    case _:
      False{}

def Stamp.unread.text(unread: Bool, +l: I.Loc, +stamp: String) -> String:
  match unread:
    case True{}:
      MSG.T.stamp.unread(l, stamp)
    case False{}:
      stamp

# Replies inside a turn
# ---------------------
# A turn of several short replies would read as a column of the same
# time. A reply that does not end its turn (and is not the newest of a
# running one) is "mid"; every client shows every message's time
# (Stamp.shown). The person's message and the turn's Finished/Stopped/Waiting stamp
# always show (laws stamp_mid_*).

def Stamp.mid.k(r: Map<&2, Nat> & Bool) -> Bool:
  (_, yes) = r
  Bool.not(yes)

def Stamp.mid(+s: Stamps, e: M.Entry) -> Bool:
  match s e:
    case Stamps{_, _, +ends, _} M.Msg{+id, _, M.Assistant{}, _, _}:
      Stamp.mid.k(Map.has(&2, Nat, ends, id))
    case _ _:
      False{}

# what a client with no hover shows under an entry: Stamp.text.loc, for
# every message, replies inside a turn included
def Stamp.shown(+l: I.Loc, +s: Stamps, +e: M.Entry) -> String:
  Stamp.text.loc(l, s, e)
