import Base
import ./i18n.bend as I
import ./msgs.bend as MSG
import ./model.bend as M
import ./cron.bend as CR
import ./tz.bend as TZ
import ./sched.bend as SCH

# When messages were sent (pure)
# ===============================
# Every client shows under a message the time it was sent, in the person's
# zone: "14:05" today, "Mon 13 Oct 14:05" before. A turn's last reply, once
# the turn is over, says when it finished and how long the turn took
# ("Finished 14:12 · 7m 3s"). A turn is a person's message and what came
# after it, up to their next one. A page that starts mid-turn has no
# message to count from, so its last reply says only when it finished.

# today: the day it is now on the person's wall clock (days since the
# epoch), worked out once a page
type Stamps is Data:
  Stamps{rule: TZ.TzRule, today: Nat, ends: Map<&2, Nat>}

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

def Stamp.ends.go(es: List<&2, M.Entry>, open: Bool, +held: String, ends: Map<&2, Nat>) -> Map<&2, Nat>:
  match es:
    case Nil{}:
      Stamp.close(String.is_empty(held), held, 1n, ends)
    case M.Msg{_, _, M.User{}, _, +at} <> t:
      Stamp.ends.go(t, True{}, "", Stamp.close(String.is_empty(held), held, at, ends))
    case M.Msg{+id, _, M.Assistant{}, _, _} <> t:
      Stamp.ends.go(t, False{}, Stamp.held(open, held, id), ends)
    case _ <> t:
      Stamp.ends.go(t, open, held, ends)

# entries oldest first; while the thread's turn runs its last turn has not
# ended
def Stamp.ends(+es: List<&2, M.Entry>, +running: Bool) -> Map<&2, Nat>:
  Stamp.ends.go(List.reverse(&2, M.Entry, es), Bool.not(running), "", Map.new(&2, Nat))

def Stamps.at(+r: TZ.TzRule, +now: Nat, ends: Map<&2, Nat>) -> Stamps:
  Stamps{r, Nat.div(TZ.Tz.wall(r, now), 86400n), ends}

# what a client needs to stamp a page of entries: the person's zone, the
# time now (seconds) and the turns' ends
def Stamps.of(+es: List<&2, M.Entry>, +st: M.State, +now: Nat, +running: Bool) -> Stamps:
  Stamps.at(SCH.Sched.person(st, 0n, True{}), now, Stamp.ends(es, running))

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

def Stamp.start(r: Map<&2, Nat> & Nat) -> Nat:
  (_, v) = r
  v

# what shows under an entry: a message's time, nothing for the rest
def Stamp.text.loc(+l: I.Loc, +s: Stamps, e: M.Entry) -> String:
  match s e:
    case Stamps{+r, +today, _} M.Msg{_, _, M.User{}, _, +at}:
      Stamp.clock(r, today, at)
    case Stamps{+r, +today, +ends} M.Msg{+id, _, M.Assistant{}, _, +at}:
      Stamp.reply(l, Stamp.start(Map.get(Nat, 0n, ends, id)), Stamp.clock(r, today, at), at)
    case _ _:
      ""

def Stamp.text(+s: Stamps, e: M.Entry) -> String:
  Stamp.text.loc(I.En{}, s, e)
