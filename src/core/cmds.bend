import Base
import ./json.bend as J
import ./codec.bend as C

# The command ledger: which of the user lines a Claude process was sent it
# still owes an answer for, so a turn ends when its own command does.
#
# Each user line the hub writes carries a uuid (hub.bend's Claude.user.u).
# Claude Code (2.1.286 on) reports each one's `command_lifecycle`: queued,
# started, then completed or cancelled, and a command's `result` comes
# BEFORE its lifecycle completes. A turn Claude starts by itself (a
# background task's notification, a cron or wakeup) has no lifecycle and
# no echo: `task_notification` (or a bare `init`), its lines, its result.
# So (test/fixtures/claude/*.jsonl):
#
# - a turn ends when a command's lifecycle reaches completed or cancelled
#   and no other command is outstanding;
# - a result while another command is outstanding never ends the thread;
# - a notification, or an init with nothing outstanding, starts a visible
#   wake turn; its result ends it only when nothing is outstanding;
# - an old CLI (no lifecycle, its init's version older than Cmds.min())
#   keeps the old rule: verdict 0, hub.bend's Hub.agent_line.
#
# A command's st: 0 accepted (not yet written: its images are being read),
# 1 sent (or queued by the CLI), 2 started, 3 resulted, 4 a tomb (the user
# stopped it). The verdict for a line is computed before the line is seen
# (Cmds.verdict, then Cmds.see): 0 legacy, 1 quiet (it belongs to a stopped
# or foreign turn: nothing about the turn changes and nothing streams), 2
# stream (current work), 3 end ok, 4 end failed, 5 end stopped (the CLI
# cancelled the command itself), 6 a wake turn starts. Laws cmds_*.

type Cmd is Data:
  Cmd{uuid: String, st: Nat, err: Bool, text: String}

# mode: 0 not known yet, 1 lifecycle, 2 legacy
type Cmds is Data:
  Cmds{cs: List<&2, Cmd>, mode: Nat, wake: Bool}

def Cmds.new() -> Cmds:
  Cmds{Nil{}, 0n, False{}}

# the first Claude Code that reports command lifecycles
def Cmds.min() -> String:
  "2.1.286"

def Cmds.cs(c: Cmds) -> List<&2, Cmd>:
  match c:
    case Cmds{cs, _, _}:
      cs

def Cmds.mode(c: Cmds) -> Nat:
  match c:
    case Cmds{_, m, _}:
      m

def Cmds.wake(c: Cmds) -> Bool:
  match c:
    case Cmds{_, _, w}:
      w

def Cmd.uuid(c: Cmd) -> String:
  match c:
    case Cmd{u, _, _, _}:
      u

def Cmd.st(c: Cmd) -> Nat:
  match c:
    case Cmd{_, s, _, _}:
      s

# Versions
# --------
# (digits read by hand: Nat.read is slow for the laws to normalize)

def Cmds.digit(+c: U32) -> Bool:
  Bool.and(U32.is_ge(c, 48), U32.is_le(c, 57))

def Cmds.digits.k(d: Bool, on: Bool, +c: U32, +acc: Nat) -> Nat:
  match d on:
    case True{} True{}:
      Nat.add(Nat.mul(acc, 10n), U32.to_nat((c - 48 : U32)))
    case _ _:
      acc

def Cmds.on.k(d: Bool, on: Bool) -> Bool:
  match d:
    case True{}:
      on
    case False{}:
      False{}

# the leading digits of a dotted part ("286-beta" is 286)
def Cmds.digits(s: String, +on: Bool, +acc: Nat) -> Nat:
  match s:
    case SNil{}:
      acc
    case SCon{Chr{+c}, t}:
      Cmds.digits(t, Cmds.on.k(Cmds.digit(c), on), Cmds.digits.k(Cmds.digit(c), on, c, acc))

def Cmds.nums(ps: List<&2, String>) -> List<&2, Nat>:
  match ps:
    case Nil{}:
      Nil{}
    case p <> t:
      Cmds.digits(p, True{}, 0n) <> Cmds.nums(t)

def Cmds.part(ns: List<&2, Nat>, i: Nat) -> Nat:
  match ns i:
    case n <> _ 0n:
      n
    case _ <> t 1n+j:
      Cmds.part(t, j)
    case Nil{} _:
      0n

def Cmds.then(c: Cmp, rest: Cmp) -> Cmp:
  match c:
    case EQ{}:
      rest
    case _:
      c

def Cmds.ver.cmp(+a: List<&2, Nat>, +b: List<&2, Nat>) -> Cmp:
  Cmds.then(Nat.cmp(Cmds.part(a, 0n), Cmds.part(b, 0n)), Cmds.then(Nat.cmp(Cmds.part(a, 1n), Cmds.part(b, 1n)), Nat.cmp(Cmds.part(a, 2n), Cmds.part(b, 2n))))

# whether a `claude_code_version` reports command lifecycles
def Cmds.lifecycle(+v: String) -> Bool:
  Bool.not(Cmp.is_lt(Cmds.ver.cmp(Cmds.nums(String.split(String.trim(v), '.')), Cmds.nums(String.split(Cmds.min(), '.')))))

# Lines
# -----

def Cmds.type(+o: J.Json) -> String:
  J.Json.get_str(o, "type", "")

def Cmds.sub(+o: J.Json) -> String:
  J.Json.get_str(o, "subtype", "")

def Cmds.is(+o: J.Json, +t: String, +s: String) -> Bool:
  Bool.and(String.eq(Cmds.type(o), t), String.eq(Cmds.sub(o), s))

def Cmds.is_lc(+o: J.Json) -> Bool:
  String.eq(Cmds.type(o), "command_lifecycle")

# the mode a line shows: any lifecycle line means lifecycles; the first
# init decides by its version (every turn has an init before its result)
def Cmds.mode.init(init: Bool, +mode: Nat, +v: String) -> Nat:
  match init:
    case True{}:
      Bool.pick(Nat, Nat.is_eq(mode, 0n), Bool.pick(Nat, Cmds.lifecycle(v), 1n, 2n), mode)
    case False{}:
      mode

def Cmds.mode.of(lc: Bool, +mode: Nat, +o: J.Json) -> Nat:
  match lc:
    case True{}:
      1n
    case False{}:
      Cmds.mode.init(Cmds.is(o, "system", "init"), mode, J.Json.get_str(o, "claude_code_version", ""))

def Cmds.drop.k(hit: Bool, c: Cmd, rest: List<&2, Cmd>) -> List<&2, Cmd>:
  match hit:
    case True{}:
      rest
    case False{}:
      c <> rest

def Cmds.drop(cs: List<&2, Cmd>, +u: String) -> List<&2, Cmd>:
  match cs:
    case Nil{}:
      Nil{}
    case Cmd{+v, st, e, x} <> t:
      Cmds.drop.k(String.eq(v, u), Cmd{v, st, e, x}, Cmds.drop(t, u))

# An old CLI reports no lifecycle, so nothing would ever retire a command
# it was sent: in legacy mode a command is forgotten once written, and
# when a ledger finds itself legacy, the ones already written go (those
# still being prepared, and tombs, stay: the writer reads them)
def Cmds.keep.one(+st: Nat) -> Bool:
  Bool.or(Nat.is_eq(st, 0n), Nat.is_ge(st, 4n))

def Cmds.trim(cs: List<&2, Cmd>) -> List<&2, Cmd>:
  match cs:
    case Nil{}:
      Nil{}
    case Cmd{+v, +st, e, x} <> t:
      Cmds.drop.k(Bool.not(Cmds.keep.one(st)), Cmd{v, st, e, x}, Cmds.trim(t))

def Cmds.trim.k(legacy: Bool, cs: List<&2, Cmd>) -> List<&2, Cmd>:
  match legacy:
    case True{}:
      Cmds.trim(cs)
    case False{}:
      cs

def Cmds.mode.set(+m2: Nat, cs: List<&2, Cmd>, w: Bool) -> Cmds:
  Cmds{Cmds.trim.k(Nat.is_eq(m2, 2n), cs), m2, w}

def Cmds.mode.see(c: Cmds, +o: J.Json) -> Cmds:
  match c:
    case Cmds{cs, +m, w}:
      Cmds.mode.set(Cmds.mode.of(Cmds.is_lc(o), m, o), cs, w)

# The ledger
# ----------

def Cmds.outstanding.one(+st: Nat) -> Bool:
  Nat.is_lt(st, 4n)

# a command not yet answered (st 0..3), other than `but`
def Cmds.open(cs: List<&2, Cmd>, +but: String) -> Bool:
  match cs:
    case Nil{}:
      False{}
    case Cmd{+u, +st, _, _} <> t:
      Bool.or(Bool.and(Cmds.outstanding.one(st), Bool.not(String.eq(u, but))), Cmds.open(t, but))

def Cmds.started(cs: List<&2, Cmd>) -> Bool:
  match cs:
    case Nil{}:
      False{}
    case Cmd{_, +st, _, _} <> t:
      Bool.or(Nat.is_eq(st, 2n), Cmds.started(t))

def Cmds.find(cs: List<&2, Cmd>, +u: String) -> Maybe<&2, Cmd>:
  match cs:
    case Nil{}:
      None{}
    case Cmd{+v, +st, +e, +x} <> t:
      Bool.pick(Maybe<&2, Cmd>, String.eq(v, u), Some{Cmd{v, st, e, x}}, Cmds.find(t, u))

def Cmds.state.k(m: Maybe<&2, Cmd>) -> Nat:
  match m:
    case Some{Cmd{_, st, _, _}}:
      st
    case None{}:
      5n

# a command's st (5: not one of ours)
def Cmds.state(+c: Cmds, +u: String) -> Nat:
  Cmds.state.k(Cmds.find(Cmds.cs(c), u))

def Cmds.has(+c: Cmds, +u: String) -> Bool:
  Nat.is_lt(Cmds.state(c, u), 5n)

# the hub accepted a message (its line is written now, or once its images
# are read), and wrote it
def Cmds.accept(c: Cmds, +u: String) -> Cmds:
  match c:
    case Cmds{cs, m, w}:
      Cmds{List.append(&2, Cmd, cs, [Cmd{u, 0n, False{}, ""}]), m, w}

def Cmds.set.k(hit: Bool, +u: String, +st: Nat, +e: Bool, +x: String, +to: Nat) -> Cmd:
  match hit:
    case True{}:
      Cmd{u, to, e, x}
    case False{}:
      Cmd{u, st, e, x}

def Cmds.sent.go(cs: List<&2, Cmd>, +u: String) -> List<&2, Cmd>:
  match cs:
    case Nil{}:
      Nil{}
    case Cmd{+v, +st, +e, +x} <> t:
      Cmds.set.k(Bool.and(String.eq(v, u), Nat.is_eq(st, 0n)), v, st, e, x, 1n) <> Cmds.sent.go(t, u)

def Cmds.sent.k(legacy: Bool, cs: List<&2, Cmd>, +u: String) -> List<&2, Cmd>:
  match legacy:
    case True{}:
      Cmds.drop(cs, u)
    case False{}:
      Cmds.sent.go(cs, u)

# written to the process (an old CLI never answers it: forgotten)
def Cmds.sent(c: Cmds, +u: String) -> Cmds:
  match c:
    case Cmds{cs, +m, w}:
      Cmds{Cmds.sent.k(Nat.is_eq(m, 2n), cs, u), m, w}

def Cmds.in(xs: List<&2, String>, +u: String) -> Bool:
  match xs:
    case Nil{}:
      False{}
    case x <> t:
      Bool.or(String.eq(x, u), Cmds.in(t, u))

# the commands written but never started (the CLI holds them queued): a
# Stop cancels them unread, so the person is told which (hub.bend's
# Unread.note)
def Cmds.waiting.k(w: Bool, v: String, rest: List<&2, String>) -> List<&2, String>:
  match w:
    case True{}:
      v <> rest
    case False{}:
      rest

def Cmds.waiting.go(cs: List<&2, Cmd>) -> List<&2, String>:
  match cs:
    case Nil{}:
      Nil{}
    case Cmd{v, +st, _, _} <> t:
      Cmds.waiting.k(Nat.is_eq(st, 1n), v, Cmds.waiting.go(t))

def Cmds.waiting(c: Cmds) -> List<&2, String>:
  match c:
    case Cmds{cs, _, _}:
      Cmds.waiting.go(cs)

# a command started or with its result: one sent after it waits
def Cmds.ahead(cs: List<&2, Cmd>) -> Bool:
  match cs:
    case Nil{}:
      False{}
    case Cmd{_, +st, _, _} <> t:
      Bool.or(Bool.or(Nat.is_eq(st, 2n), Nat.is_eq(st, 3n)), Cmds.ahead(t))

def Cmds.unread.k(ahead: Bool, c: Cmds) -> List<&2, String>:
  match ahead:
    case True{}:
      Cmds.waiting(c)
    case False{}:
      Nil{}

# the commands the CLI holds behind the one it works on (or behind a turn
# it started by itself): the person's messages it has not read yet, shown
# so while they wait (hub.bend's Wait.*). The command that starts a turn
# is written a moment before it starts, with nothing ahead: never listed
def Cmds.unread(+c: Cmds) -> List<&2, String>:
  Cmds.unread.k(Bool.or(Cmds.wake(c), Cmds.ahead(Cmds.cs(c))), c)

def Cmds.stop.go(cs: List<&2, Cmd>, +still: List<&2, String>) -> List<&2, Cmd>:
  match cs:
    case Nil{}:
      Nil{}
    case Cmd{+v, +st, +e, +x} <> t:
      Cmds.set.k(Bool.and(Cmds.outstanding.one(st), Bool.not(Cmds.in(still, v))), v, st, e, x, 4n) <> Cmds.stop.go(t, still)

# the user's Stop: every unanswered command not still queued is a tomb (one
# not yet written never is), and a wake turn is over
def Cmds.stop(c: Cmds, +still: List<&2, String>) -> Cmds:
  match c:
    case Cmds{cs, m, _}:
      Cmds{Cmds.stop.go(cs, still), m, False{}}

def Cmds.still.go(cs: List<&2, Cmd>, +still: List<&2, String>) -> List<&2, Cmd>:
  match cs:
    case Nil{}:
      Nil{}
    case Cmd{+v, +st, +e, +x} <> t:
      Cmds.set.k(Bool.and(Nat.is_eq(st, 4n), Cmds.in(still, v)), v, st, e, x, 1n) <> Cmds.still.go(t, still)

# the interrupt's answer names the commands it left queued: not tombs
def Cmds.still(c: Cmds, +still: List<&2, String>) -> Cmds:
  match c:
    case Cmds{cs, m, w}:
      Cmds{Cmds.still.go(cs, still), m, w}

def Cmds.strs(xs: List<&2, J.Json>) -> List<&2, String>:
  match xs:
    case Nil{}:
      Nil{}
    case +x <> t:
      J.Json.as_str(Some{x}, "") <> Cmds.strs(t)

# a control_response's still_queued
def Cmds.queued(+o: J.Json) -> List<&2, String>:
  Cmds.strs(J.Json.elems(J.Json.get_obj(J.Json.get_obj(J.Json.get_obj(o, "response"), "response"), "still_queued")))

def Cmds.start.go(cs: List<&2, Cmd>, +u: String) -> List<&2, Cmd>:
  match cs:
    case Nil{}:
      Nil{}
    case Cmd{+v, +st, +e, +x} <> t:
      Cmds.set.k(String.eq(v, u), v, st, e, x, 2n) <> Cmds.start.go(t, u)

def Cmds.result.k(hit: Bool, +v: String, +st: Nat, +e: Bool, +x: String, +err: Bool, +text: String) -> Cmd:
  match hit:
    case True{}:
      Cmd{v, 3n, err, text}
    case False{}:
      Cmd{v, st, e, x}

# the oldest started command has its outcome
def Cmds.result.go(cs: List<&2, Cmd>, +err: Bool, +text: String, +done: Bool) -> List<&2, Cmd>:
  match cs:
    case Nil{}:
      Nil{}
    case Cmd{+v, +st, +e, +x} <> t:
      Cmds.result.k(Bool.and(Nat.is_eq(st, 2n), Bool.not(done)), v, st, e, x, err, text) <> Cmds.result.go(t, err, text, Bool.or(done, Nat.is_eq(st, 2n)))

def Cmds.see.lc(i: Nat, c: Cmds, +u: String) -> Cmds:
  match i c:
    case 1n Cmds{cs, m, w}:
      Cmds{Cmds.start.go(cs, u), m, w}
    case 2n Cmds{cs, m, w}:
      Cmds{Cmds.drop(cs, u), m, w}
    case 3n Cmds{cs, m, w}:
      Cmds{Cmds.drop(cs, u), m, w}
    case _ _:
      c

def Cmds.see.result(started: Bool, c: Cmds, +err: Bool, +text: String) -> Cmds:
  match started c:
    case True{} Cmds{cs, m, w}:
      Cmds{Cmds.result.go(cs, err, text, False{}), m, w}
    case False{} Cmds{cs, m, _}:
      Cmds{cs, m, False{}}

def Cmds.wake.set(on: Bool, c: Cmds) -> Cmds:
  match on c:
    case True{} Cmds{cs, m, _}:
      Cmds{cs, m, True{}}
    case False{} _:
      c

# what a line does to the ledger: 0 lifecycle, 1 notification, 2 init,
# 3 result, 4 the interrupt's answer
def Cmds.kind(+o: J.Json) -> Nat:
  Bool.pick(Nat, Cmds.is_lc(o), 0n,
    Bool.pick(Nat, Cmds.is(o, "system", "task_notification"), 1n,
      Bool.pick(Nat, Cmds.is(o, "system", "init"), 2n,
        Bool.pick(Nat, String.eq(Cmds.type(o), "result"), 3n,
          Bool.pick(Nat, String.eq(Cmds.type(o), "control_response"), 4n, 5n)))))

def Cmds.lc.state(+o: J.Json) -> Nat:
  C.Codec.pos(["queued", "started", "completed", "cancelled"], J.Json.get_str(o, "state", ""))

def Cmds.see.k(i: Nat, +c: Cmds, +o: J.Json) -> Cmds:
  match i:
    case 0n:
      Cmds.see.lc(Cmds.lc.state(o), c, J.Json.get_str(o, "command_uuid", ""))
    # (a notification while a command runs is folded into its turn: the
    # CLI says it when it hands it to the model)
    case 1n:
      Cmds.wake.set(Bool.not(Cmds.started(Cmds.cs(c))), c)
    case 2n:
      Cmds.wake.set(Bool.not(Cmds.open(Cmds.cs(c), "")), c)
    case 3n:
      Cmds.see.result(Cmds.started(Cmds.cs(c)), c, J.Json.get_bool(o, "is_error", False{}), J.Json.get_str(o, "result", ""))
    case 4n:
      Cmds.still(c, Cmds.queued(o))
    case _:
      c

# a line seen
def Cmds.see(c: Cmds, +o: J.Json) -> Cmds:
  Cmds.see.k(Cmds.kind(o), Cmds.mode.see(c, o), o)

# a line that can change which commands wait (Cmds.unread): a lifecycle,
# a result, the interrupt's answer
def Cmds.moves(+o: J.Json) -> Bool:
  Nat.is_lt(Cmds.kind(o), 5n)

# Verdicts
# --------

# a line of the work on hand streams; anything else is quiet
def Cmds.flow(+c: Cmds) -> Nat:
  Bool.pick(Nat, Bool.or(Cmds.started(Cmds.cs(c)), Cmds.wake(c)), 2n, 1n)

def Cmds.cmd.err(m: Maybe<&2, Cmd>) -> Bool:
  match m:
    case Some{Cmd{_, _, e, _}}:
      e
    case None{}:
      False{}

def Cmds.cmd.text(m: Maybe<&2, Cmd>) -> String:
  match m:
    case Some{Cmd{_, _, _, x}}:
      x
    case None{}:
      ""

# a command's lifecycle ended (`done`: completed, else cancelled)
def Cmds.v.end(done: Bool, +st: Nat, +others: Bool, +err: Bool) -> Nat:
  match done:
    case True{}:
      Bool.pick(Nat, Bool.or(Nat.is_ge(st, 4n), others), 1n, Bool.pick(Nat, err, 4n, 3n))
    case False{}:
      Bool.pick(Nat, Bool.or(Nat.is_ge(st, 4n), others), 1n, 5n)

def Cmds.v.lc(i: Nat, +c: Cmds, +run: Bool, +u: String) -> Nat:
  match i:
    # a tomb the CLI starts after all (or one the interrupt left queued,
    # starting after the Stop): the display follows the process
    case 1n:
      Bool.pick(Nat, Bool.and(Cmds.has(c, u), Bool.not(run)), 6n, 2n)
    case 2n:
      Cmds.v.end(True{}, Cmds.state(c, u), Bool.or(Cmds.open(Cmds.cs(c), u), Cmds.wake(c)), Cmds.cmd.err(Cmds.find(Cmds.cs(c), u)))
    case 3n:
      Cmds.v.end(False{}, Cmds.state(c, u), Cmds.open(Cmds.cs(c), u), False{})
    case _:
      2n

# a result: a started command's only records its outcome (the end waits
# for its lifecycle); a wake turn's ends the thread when nothing else is
# outstanding
def Cmds.v.result(+c: Cmds, +err: Bool) -> Nat:
  Bool.pick(Nat, Cmds.started(Cmds.cs(c)), 1n,
    Bool.pick(Nat, Cmds.wake(c), Bool.pick(Nat, Cmds.open(Cmds.cs(c), ""), 1n, Bool.pick(Nat, err, 4n, 3n)), 1n))

def Cmds.v.k(i: Nat, +c: Cmds, +run: Bool, +o: J.Json) -> Nat:
  match i:
    case 0n:
      Cmds.v.lc(Cmds.lc.state(o), c, run, J.Json.get_str(o, "command_uuid", ""))
    case 1n:
      Bool.pick(Nat, Bool.and(Bool.not(run), Bool.not(Cmds.started(Cmds.cs(c)))), 6n, Cmds.flow(c))
    case 2n:
      Bool.pick(Nat, Bool.and(Bool.not(run), Bool.not(Cmds.open(Cmds.cs(c), ""))), 6n, Cmds.flow(c))
    case 3n:
      Cmds.v.result(c, J.Json.get_bool(o, "is_error", False{}))
    case _:
      Cmds.flow(c)

def Cmds.v.mode(lc: Bool, +c: Cmds, +run: Bool, +o: J.Json) -> Nat:
  match lc:
    case True{}:
      Cmds.v.k(Cmds.kind(o), c, run, o)
    case False{}:
      0n

def Cmds.verdict.m(+c: Cmds, +run: Bool, +o: J.Json) -> Nat:
  Cmds.v.mode(Nat.is_eq(Cmds.mode(c), 1n), c, run, o)

# what a line does to the turn (run: the thread's turn is running), before
# the line is seen
def Cmds.verdict(c: Cmds, +run: Bool, +o: J.Json) -> Nat:
  Cmds.verdict.m(Cmds.mode.see(c, o), run, o)

# an ending's text: the result line's own, or what the command's result
# said when its lifecycle completes
def Cmds.text(+c: Cmds, +o: J.Json) -> String:
  Bool.pick(String, Cmds.is_lc(o), Cmds.cmd.text(Cmds.find(Cmds.cs(c), J.Json.get_str(o, "command_uuid", ""))), J.Json.get_str(o, "result", ""))

# what the wake turn's Act says: a notification's summary, else ""
def Cmds.title(+o: J.Json) -> String:
  J.Json.get_str(o, "summary", "")

# Ordered writes
# --------------
# A message with images is read off the hub before it is written; one sent
# after it waits, so Claude gets them in the order the hub took them. Held
# lines go out from the front as they are ready; a tomb's never does.

type Held is Data:
  Held{uuid: String, line: String, ready: Bool}

type Flush is Data:
  Flush{out: List<&2, String>, sent: List<&2, String>, rest: List<&2, Held>}

def Held.ready.go(hs: List<&2, Held>, +u: String, +line: String) -> List<&2, Held>:
  match hs:
    case Nil{}:
      Nil{}
    case Held{+v, +l, +r} <> t:
      Bool.pick(Held, String.eq(v, u), Held{v, line, True{}}, Held{v, l, r}) <> Held.ready.go(t, u, line)

# a held message's line is ready
def Held.ready(hs: List<&2, Held>, +u: String, +line: String) -> List<&2, Held>:
  Held.ready.go(hs, u, line)

def Held.push(hs: List<&2, Held>, +u: String, +line: String, +ready: Bool) -> List<&2, Held>:
  List.append(&2, Held, hs, [Held{u, line, ready}])

def Flush.out(f: Flush) -> List<&2, String>:
  match f:
    case Flush{o, _, _}:
      o

def Flush.sent(f: Flush) -> List<&2, String>:
  match f:
    case Flush{_, u, _}:
      u

def Flush.rest(f: Flush) -> List<&2, Held>:
  match f:
    case Flush{_, _, r}:
      r

# the walk so far: lines out and held ones (both last first), and whether
# a line not yet ready stopped it
type Walk is Data:
  Walk{out: List<&2, String>, sent: List<&2, String>, rest: List<&2, Held>, stop: Bool}

def Held.step(stop: Bool, tomb: Bool, ready: Bool, h: Held, +u: String, +line: String, w: Walk) -> Walk:
  match stop tomb ready w:
    case False{} True{} _ _:
      w
    case False{} False{} True{} Walk{o, us, r, s}:
      Walk{line <> o, u <> us, r, s}
    case _ _ _ Walk{o, us, r, _}:
      Walk{o, us, h <> r, True{}}

def Held.stopped(w: Walk) -> Bool:
  match w:
    case Walk{_, _, _, s}:
      s

def Held.walk(hs: List<&2, Held>, +c: Cmds, +w: Walk) -> Walk:
  match hs:
    case Nil{}:
      w
    case Held{+u, +l, +r} <> t:
      Held.walk(t, c, Held.step(Held.stopped(w), Nat.is_ge(Cmds.state(c, u), 4n), r, Held{u, l, r}, u, l, w))

def Held.done(w: Walk) -> Flush:
  match w:
    case Walk{o, us, r, _}:
      Flush{List.reverse(&2, String, o), List.reverse(&2, String, us), List.reverse(&2, Held, r)}

# the lines that may go now (a tomb's dropped), and what still waits
def Held.flush(hs: List<&2, Held>, +c: Cmds) -> Flush:
  Held.done(Held.walk(hs, c, Walk{Nil{}, Nil{}, Nil{}, False{}}))

def Held.uuids(hs: List<&2, Held>) -> List<&2, String>:
  match hs:
    case Nil{}:
      Nil{}
    case Held{u, _, _} <> t:
      u <> Held.uuids(t)

def Cmds.sent.all(us: List<&2, String>, c: Cmds) -> Cmds:
  match us:
    case Nil{}:
      c
    case +u <> t:
      Cmds.sent.all(t, Cmds.sent(c, u))

# Lines the laws are stated over
# ------------------------------

def Cmds.o(+line: String) -> J.Json:
  Maybe.default(&2, J.Json, J.Json.parse(line), J.Null{})

def Cmds.l.result() -> String:
  "{\"type\":\"result\",\"is_error\":false,\"result\":\"ok\"}"

def Cmds.l.failed() -> String:
  "{\"type\":\"result\",\"is_error\":true,\"subtype\":\"error_during_execution\"}"

def Cmds.l.note() -> String:
  "{\"type\":\"system\",\"subtype\":\"task_notification\",\"task_id\":\"b1\",\"summary\":\"done\"}"

def Cmds.l.init(+v: String) -> String:
  "{\"type\":\"system\",\"subtype\":\"init\",\"claude_code_version\":\"" ++ v ++ "\"}"

def Cmds.l.lc(+u: String, +state: String) -> String:
  "{\"type\":\"command_lifecycle\",\"command_uuid\":\"" ++ u ++ "\",\"state\":\"" ++ state ++ "\"}"

# a ledger in lifecycle mode with one command at st
def Cmds.one(+u: String, +st: Nat, +wake: Bool) -> Cmds:
  Cmds{[Cmd{u, st, False{}, ""}], 1n, wake}

# Kept across a restart
# ---------------------
# A Claude process the hub keeps through a restart in place (adopt.bend)
# keeps its ledger: written as JSON before the exec and read back after,
# so its lines are judged as before (law cmds_json_round).

def Cmd.json(c: Cmd) -> J.Json:
  match c:
    case Cmd{u, st, e, x}:
      J.Json.obj([J.KV{"u", J.Str{u}}, J.KV{"s", J.Json.nat(st)}, J.KV{"e", J.Json.bool(e)}, J.KV{"t", J.Str{x}}])

def Cmds.json.cs(cs: List<&2, Cmd>) -> List<&2, J.Json>:
  match cs:
    case Nil{}:
      Nil{}
    case c <> t:
      Cmd.json(c) <> Cmds.json.cs(t)

def Cmds.json(c: Cmds) -> J.Json:
  match c:
    case Cmds{cs, m, w}:
      J.Json.obj([J.KV{"cs", J.Json.arr(Cmds.json.cs(cs))}, J.KV{"m", J.Json.nat(m)}, J.KV{"w", J.Json.bool(w)}])

def Cmd.read(+o: J.Json) -> Cmd:
  Cmd{J.Json.get_str(o, "u", ""), J.Json.get_nat(o, "s", 0n), J.Json.get_bool(o, "e", False{}), J.Json.get_str(o, "t", "")}

def Cmds.read.cs(os: List<&2, J.Json>) -> List<&2, Cmd>:
  match os:
    case Nil{}:
      Nil{}
    case +o <> t:
      Cmd.read(o) <> Cmds.read.cs(t)

def Cmds.read(+o: J.Json) -> Cmds:
  Cmds{Cmds.read.cs(J.Json.elems(J.Json.get_obj(o, "cs"))), J.Json.get_nat(o, "m", 0n), J.Json.get_bool(o, "w", False{})}

# a ledger with a command started and one resulted (for the laws)
def Cmds.demo.kept() -> Cmds:
  Cmds{[Cmd{"u1", 2n, False{}, ""}, Cmd{"u2", 3n, True{}, "it \"failed\"\n"}], 1n, True{}}

# Stuck turns
# -----------
# A running turn whose ledger has no command at work any more (each one
# resulted or was stopped, and no wake turn runs) is over: only its last
# lifecycle line is missing (lost in a restart's handoff, say). hub.bend's
# Stuck.* ends it after a silence, the way its ending would have (laws
# stuck_*).

def Cmds.working(cs: List<&2, Cmd>) -> Bool:
  match cs:
    case Nil{}:
      False{}
    case Cmd{_, +st, _, _} <> t:
      Bool.or(Nat.is_lt(st, 3n), Cmds.working(t))

def Cmds.tomb(cs: List<&2, Cmd>) -> Bool:
  match cs:
    case Nil{}:
      False{}
    case Cmd{_, +st, _, _} <> t:
      Bool.or(Nat.is_eq(st, 4n), Cmds.tomb(t))

def Cmds.erred(cs: List<&2, Cmd>) -> Bool:
  match cs:
    case Nil{}:
      False{}
    case Cmd{_, +st, +e, _} <> t:
      Bool.or(Bool.and(Nat.is_eq(st, 3n), e), Cmds.erred(t))

# lifecycle mode, no wake turn, no command at work
def Cmds.idle(c: Cmds) -> Bool:
  match c:
    case Cmds{cs, m, w}:
      Bool.and(Nat.is_eq(m, 1n), Bool.and(Bool.not(w), Bool.not(Cmds.working(cs))))

# how such a turn ends, as its verdict would have said: 5 stopped (a
# command was stopped), 4 failed (one resulted in an error), else 3 ok
def Cmds.idle.end(c: Cmds) -> Nat:
  match c:
    case Cmds{+cs, _, _}:
      Bool.pick(Nat, Cmds.tomb(cs), 5n, Bool.pick(Nat, Cmds.erred(cs), 4n, 3n))

# the ledger once such a turn is ended
def Cmds.cleared(c: Cmds) -> Cmds:
  match c:
    case Cmds{_, m, _}:
      Cmds{Nil{}, m, False{}}
