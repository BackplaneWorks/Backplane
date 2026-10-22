import Base
import ./model.bend as M
import ./client.bend as U

# When a thin view hears view.changed (pure; the hub's Hub.thin.notify).
# Each step's changes and streamed deltas say which threads they touch
# (U.Touch.of: "*" for anything not clearly one thread's own). A view hears
# a change to its own thread (or to everything) at once, at most every
# Pace.urgent() ms; a change elsewhere (a sidebar row, at most) at most
# every Pace.soft() ms. What a view was not told yet is owed, and a timer
# (the hub's ThinFlush) tells it once its gap has passed: nothing is
# dropped, only gathered. Before, every delta of every thread re-rendered
# every view, back to back (fast hub R1).
type Touch is Data:
  Touch{all: Bool, threads: List<&2, String>}

def Touch.none() -> Touch:
  Touch{False{}, Nil{}}

def Touch.add(t: Touch, +th: String) -> Touch:
  match t:
    case Touch{all, ts}:
      Touch{Bool.or(all, String.eq(th, "*")), th <> ts}

def Touch.adds(ths: List<&2, String>, t: Touch) -> Touch:
  match ths:
    case Nil{}:
      t
    case +th <> rest:
      Touch.adds(rest, Touch.add(t, th))

# what a step's committed changes touch
def Touch.changes(cs: List<&2, M.Change>, t: Touch) -> Touch:
  match cs:
    case Nil{}:
      t
    case c <> rest:
      Touch.changes(rest, Touch.adds(U.Touch.of(c), t))

def Touch.member(ts: List<&2, String>, +th: String, hit: Bool) -> Bool:
  match ts hit:
    case _ True{}:
      True{}
    case Nil{} False{}:
      False{}
    case +x <> rest False{}:
      Touch.member(rest, th, String.eq(x, th))

def Touch.any(t: Touch) -> Bool:
  match t:
    case Touch{all, ts}:
      Bool.or(all, Bool.not(List.is_empty(&2, String, ts)))

def Touch.own(t: Touch, +th: String) -> Bool:
  match t:
    case Touch{all, ts}:
      Bool.or(all, Bool.and(Touch.member(ts, th, False{}), Bool.not(String.is_empty(th))))

def Touch.level.k(own: Bool, any: Bool) -> Nat:
  match own any:
    case True{} _:
      2n
    case False{} True{}:
      1n
    case False{} False{}:
      0n

# 2: the view's own thread (th, "" for a view of no thread) or everything;
# 1: something else; 0: nothing
def Touch.level(+t: Touch, +th: String) -> Nat:
  Touch.level.k(Touch.own(t, th), Touch.any(t))

# ms between notices of a view's own thread and of anything else
def Pace.urgent() -> Nat:
  50n

def Pace.soft() -> Nat:
  500n

# when the view was last told (ms) and what it is owed (a level)
type Pace is Data:
  Pace{last: Nat, owed: Nat}

def Pace.fresh() -> Pace:
  Pace{0n, 0n}

# send: tell the view now; pace: its state after; wait: ms until it is due
# (0 when nothing is owed)
type Paced is Data:
  Paced{send: Bool, pace: Pace, wait: Nat}

def Pace.gap(+level: Nat) -> Nat:
  Bool.pick(Nat, Nat.is_ge(level, 2n), Pace.urgent(), Pace.soft())

def Pace.due(due: Bool, +last: Nat, +owed: Nat, +now: Nat) -> Paced:
  match due:
    case True{}:
      Paced{True{}, Pace{now, 0n}, 0n}
    case False{}:
      Paced{False{}, Pace{last, owed}, Nat.sub(Pace.gap(owed), Nat.sub(now, last))}

def Pace.owed(none: Bool, +last: Nat, +owed: Nat, +now: Nat) -> Paced:
  match none:
    case True{}:
      Paced{False{}, Pace{last, 0n}, 0n}
    case False{}:
      Pace.due(Nat.is_ge(Nat.sub(now, last), Pace.gap(owed)), last, owed, now)

# a view's step: level is what this step touched for it (0 for a timer's)
def Pace.step(p: Pace, +now: Nat, +level: Nat) -> Paced:
  match p:
    case Pace{+last, +owed}:
      +o = Bool.pick(Nat, Nat.is_ge(owed, level), owed, level)
      Pace.owed(Nat.is_eq(o, 0n), last, o, now)

def Paced.send(p: Paced) -> Bool:
  match p:
    case Paced{s, _, _}:
      s

def Paced.pace(p: Paced) -> Pace:
  match p:
    case Paced{_, pace, _}:
      pace

def Paced.wait(p: Paced) -> Nat:
  match p:
    case Paced{_, _, w}:
      w

# the sooner of two waits, 0 being none
def Wait.min(+a: Nat, +b: Nat) -> Nat:
  Bool.pick(Nat, Nat.is_eq(a, 0n), b, Bool.pick(Nat, Nat.is_eq(b, 0n), a, Bool.pick(Nat, Nat.is_le(a, b), a, b)))

# the hub's notice state: what this step touched, each view's pace (by
# client id), and when the flush timer is set to fire (ms, 0 for none)
type Notes is Data:
  Notes{touch: Touch, paces: Map<&2, Pace>, flush: Nat}

def Notes.empty() -> Notes:
  Notes{Touch.none(), Map.new(&2, Pace), 0n}

def Notes.touch(n: Notes, +th: String) -> Notes:
  match n:
    case Notes{t, ps, f}:
      Notes{Touch.add(t, th), ps, f}

def Notes.changes(n: Notes, cs: List<&2, M.Change>) -> Notes:
  match n:
    case Notes{t, ps, f}:
      Notes{Touch.changes(cs, t), ps, f}

def Notes.touched(n: Notes) -> Touch:
  match n:
    case Notes{t, _, _}:
      t

def Notes.paces(n: Notes) -> Map<&2, Pace>:
  match n:
    case Notes{_, ps, _}:
      ps

def Notes.flush(n: Notes) -> Nat:
  match n:
    case Notes{_, _, f}:
      f

def Pace.get.k(p: Map<&2, Pace> & Pace) -> Pace:
  (_, pace) = p
  pace

def Pace.of(+ps: Map<&2, Pace>, +key: String) -> Pace:
  Pace.get.k(Map.get(Pace, Pace.fresh(), ps, key))

# a timer is wanted for a wait (ms from now) unless one already fires by then
def Notes.timer(+flush: Nat, +now: Nat, +wait: Nat) -> Bool:
  Bool.and(Nat.is_gt(wait, 0n), Bool.or(Nat.is_eq(flush, 0n), Nat.is_gt(flush, Nat.add(now, wait))))
