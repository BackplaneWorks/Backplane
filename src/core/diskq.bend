import Base
import ./model.bend as M
import ./hub.bend as B

# Disk full: steps kept while the log cannot be written
# ------------------------------------------------------
# A step whose append fails (no space, a read-only disk) is not dropped: its
# log is cut back as before, its changes are folded into the hub's state
# (clients carry on) and it waits here: its disk lines, its changes and its
# lines for clients, in order, with the state before the first step that
# waits. While any wait, every later step waits behind them. A flush appends
# them all at once and commits the store from that state (storeio.bend's
# Sto.commit), as one commit of all of them would; it is tried after each
# failure's backoff (2 s doubling to 60 s), with every commit once that has
# passed, and at the tick.
#
# A step that waits runs only its local effects at once (what clients see,
# memory, streamed text: Dq.local); every effect that acts outside the hub
# (an agent started or written to, a send, a push or alert, a deploy, git
# that writes, work launched or stopped: the rest) waits with it and runs,
# in order, after its flush (law dq_no_external_before_written). So a hub
# that stops while steps wait loses their lines but nothing outside heard
# of them; replayed after a restart, nothing runs twice.
#
# At Dq.soft() lines new turns are refused (the person is told why); an
# agent's tool call is refused while any line waits (its answer would say
# done about what is not yet saved); at Dq.hard() a step is refused as a
# failed write was before. Nothing taken while there is room is dropped
# (laws dq_*).

# (lines, cs and outs are kept newest first: taking a step costs its own
# size, not the queue's; a flush reverses them once)
type Dq is Data:
  Dq{lines: List<&2, String>, n: Nat, pre: M.State, cs: List<&2, M.Change>, fresh: List<&2, String>, from: Nat, outs: List<&2, B.Out>, next: Nat, wait: Nat, err: String}

def Dq.empty() -> Dq:
  Dq{Nil{}, 0n, M.State.empty(), Nil{}, Nil{}, 0n, Nil{}, 0n, 0n, ""}

def Dq.soft() -> Nat:
  5000n

def Dq.hard() -> Nat:
  50000n

def Dq.n(q: Dq) -> Nat:
  match q:
    case Dq{_, n, _, _, _, _, _, _, _, _}:
      n

def Dq.some(q: Dq) -> Bool:
  Nat.is_gt(Dq.n(q), 0n)

# new turns are refused (output of running ones is still taken)
def Dq.full(q: Dq) -> Bool:
  Nat.is_ge(Dq.n(q), Dq.soft())

# a step of k lines can still be taken
def Dq.room(q: Dq, +k: Nat) -> Bool:
  Nat.is_le(Nat.add(Dq.n(q), k), Dq.hard())

def Dq.rev(xs: List<&2, String>, acc: List<&2, String>) -> List<&2, String>:
  match xs:
    case Nil{}:
      acc
    case x <> t:
      Dq.rev(t, x <> acc)

def Dq.revc(xs: List<&2, M.Change>, acc: List<&2, M.Change>) -> List<&2, M.Change>:
  match xs:
    case Nil{}:
      acc
    case x <> t:
      Dq.revc(t, x <> acc)

def Dq.revo(xs: List<&2, B.Out>, acc: List<&2, B.Out>) -> List<&2, B.Out>:
  match xs:
    case Nil{}:
      acc
    case x <> t:
      Dq.revo(t, x <> acc)

# Effects
# -------

# what a waiting step may run at once: what clients see, memory, streamed
# text, reads; everything else acts outside the hub and waits for the flush
def Dq.local(o: B.Out) -> Bool:
  match o:
    case B.Send{_, _}: True{}
    case B.Broadcast{_}: True{}
    case B.Watch{_, _, _, _, _, _, _}: True{}
    case B.ModelsRefresh{}: True{}
    case B.PlotInfo{_, _, _, _, _}: True{}
    case B.DeskLook{_, _, _}: True{}
    case B.Live{_, _}: True{}
    case B.ToolOpen{_, _}: True{}
    case B.ToolShut{_, _}: True{}
    case B.ToolsGone{_}: True{}
    case B.TermSnap{_, _}: True{}
    case B.TermOp{_, _, _, _, _, _}: True{}
    case B.TermKill{_}: True{}
    case B.Fs{_, _, _, _}: True{}
    case B.MailRead{_, _, _, _, _}: True{}
    case B.AgentStop{_}: True{}
    case B.DeployProbe{}: True{}
    case B.WakePoll{_, _}: True{}
    case _: False{}

# the effects split so far (each newest first)
type DqSp is Data:
  DqSp{now: List<&2, B.Out>, held: List<&2, B.Out>}

def Dq.split.step(local: Bool, o: B.Out, sp: DqSp) -> DqSp:
  match local sp:
    case True{} DqSp{now, held}:
      DqSp{o <> now, held}
    case False{} DqSp{now, held}:
      DqSp{now, o <> held}

def Dq.split.fin(sp: DqSp) -> List<&2, B.Out> & List<&2, B.Out>:
  match sp:
    case DqSp{now, held}:
      (List.reverse(&2, B.Out, now), List.reverse(&2, B.Out, held))

def Dq.split.go(os: List<&2, B.Out>, sp: DqSp) -> List<&2, B.Out> & List<&2, B.Out>:
  match os:
    case Nil{}:
      Dq.split.fin(sp)
    case +o <> t:
      Dq.split.go(t, Dq.split.step(Dq.local(o), o, sp))

# a waiting step's effects: those it runs now, those that wait (both in order)
def Dq.split(os: List<&2, B.Out>) -> List<&2, B.Out> & List<&2, B.Out>:
  Dq.split.go(os, DqSp{Nil{}, Nil{}})

# Taking a step
# -------------

# a step behind those waiting: its disk lines (oldest first), its changes,
# its lines for clients (newest first) and the effects it holds back; the
# first step that waits also keeps the state before it (`st`) and where the
# far push it skipped starts (`seq`)
def Dq.push.k(+first: Bool, q: Dq, +st: M.State, +seq: Nat, ls: List<&2, String>, +k: Nat, cs: List<&2, M.Change>, fr: List<&2, String>, held: List<&2, B.Out>) -> Dq:
  match q:
    case Dq{lines, n, pre, qcs, qfr, from, outs, next, wait, err}:
      Dq{Dq.rev(ls, lines), Nat.add(n, k), Bool.pick(M.State, first, st, pre), Dq.revc(cs, qcs), List.append(&2, String, fr, qfr),
        Bool.pick(Nat, first, seq, from), Dq.revo(held, outs), next, wait, err}

def Dq.push(+q: Dq, +st: M.State, +seq: Nat, +ls: List<&2, String>, cs: List<&2, M.Change>, fr: List<&2, String>, held: List<&2, B.Out>) -> Dq:
  Dq.push.k(Bool.not(Dq.some(q)), q, st, seq, ls, List.length(&2, String, ls), cs, fr, held)

# a waiting step's effects that act outside, held behind those already held
def Dq.hold(q: Dq, held: List<&2, B.Out>) -> Dq:
  match q:
    case Dq{lines, n, pre, cs, fr, from, outs, next, wait, err}:
      Dq{lines, n, pre, cs, fr, from, Dq.revo(held, outs), next, wait, err}

# the person is told when steps start to wait and again every 500 lines
# (not at every line an agent streams)
def Dq.tell(+q: Dq, +k: Nat) -> Bool:
  Bool.or(Nat.is_eq(Dq.n(q), k), Bool.not(Nat.is_eq(Nat.div(Dq.n(q), 500n), Nat.div(Nat.sub(Dq.n(q), k), 500n))))

# what a flush appends: every line waiting, oldest first
def Dq.out(q: Dq) -> List<&2, String>:
  match q:
    case Dq{lines, _, _, _, _, _, _, _, _, _}:
      List.reverse(&2, String, lines)

# the state the flush's store commit starts from
def Dq.pre(q: Dq) -> M.State:
  match q:
    case Dq{_, _, pre, _, _, _, _, _, _, _}:
      pre

# after a flush: the effects held back, oldest first
def Dq.held(q: Dq) -> List<&2, B.Out>:
  match q:
    case Dq{_, _, _, _, _, _, outs, _, _, _}:
      List.reverse(&2, B.Out, outs)

# after a flush: the changes written, oldest first (the far push, the
# uberbot's sync)
def Dq.changes(q: Dq) -> List<&2, M.Change>:
  match q:
    case Dq{_, _, _, cs, _, _, _, _, _, _}:
      List.reverse(&2, M.Change, cs)

# after a flush: the lines for clients written (newest first) and the
# number the first of them follows
def Dq.fresh(q: Dq) -> List<&2, String>:
  match q:
    case Dq{_, _, _, _, fr, _, _, _, _, _}:
      fr

def Dq.from(q: Dq) -> Nat:
  match q:
    case Dq{_, _, _, _, _, from, _, _, _, _}:
      from

# Trying again
# ------------

# a flush is due
def Dq.due(q: Dq, +now: Nat) -> Bool:
  match q:
    case Dq{_, n, _, _, _, _, _, next, _, _}:
      Bool.and(Nat.is_gt(n, 0n), Nat.is_ge(now, next))

# the next wait after a failure: 2 s, doubling, at most 60 s
def Dq.backoff(+w: Nat) -> Nat:
  Bool.pick(Nat, Nat.is_eq(w, 0n), 2n, Bool.pick(Nat, Nat.is_ge(Nat.mul(w, 2n), 60n), 60n, Nat.mul(w, 2n)))

# a write or a flush failed at `now`, and why
def Dq.failed(q: Dq, +now: Nat, +why: String) -> Dq:
  match q:
    case Dq{lines, n, pre, cs, fr, from, outs, _, +wait, _}:
      Dq{lines, n, pre, cs, fr, from, outs, Nat.add(now, Dq.backoff(wait)), Dq.backoff(wait), why}

def Dq.err(q: Dq) -> String:
  match q:
    case Dq{_, _, _, _, _, _, _, _, _, err}:
      err

# Words
# -----

# what every client is told while lines wait (and when they are saved)
def Dq.say(+q: Dq) -> String:
  "Disk full: Backplane can't save; " ++ Nat.show(Dq.n(q)) ++ " changes waiting (" ++ Dq.err(q) ++ "). They are saved once there is space; a restart now would lose them."

def Dq.saved() -> String:
  "Disk space is back: every waiting change is saved."

def Dq.refused() -> String:
  "Disk full: Backplane can't save, so no new turn starts until there is space."

def Dq.refused.tool() -> String:
  "Disk full: Backplane can't save this now, so nothing was done; try again once there is space."

# Examples the laws read
def Dq.demo.step(+st: M.State, +ls: List<&2, String>, os: List<&2, B.Out>) -> Dq:
  Dq.push(Dq.failed(Dq.empty(), 100n, "No space left on device"), st, 7n, ls, Nil{}, Nil{}, os)

def Dq.demo() -> Dq:
  Dq.push(Dq.push(Dq.failed(Dq.empty(), 100n, "No space left on device"), M.State.empty(), 7n, ["a", "b"], Nil{}, Nil{}, Nil{}), M.State.empty(), 9n, ["c"], Nil{}, Nil{}, Nil{})

# a step's effects: one client message, an agent started, a stream relay,
# a bot reply, a deploy
def Dq.demo.outs() -> List<&2, B.Out>:
  [B.Send{1, "x"}, B.AgentStart{"t1", "/w", "claude", "", "", M.FullAccess{}, M.DefaultMode{}}, B.Live{"t1", "d"}, B.BotReply{"t1", "to", "text", "done"}, B.DeployGo{""}]
