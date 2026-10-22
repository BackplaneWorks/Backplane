import Base
import ./json.bend as J
import ./model.bend as M
import ./codec.bend as C
import ./utf8.bend as UT

# The event log read at start-up in one pass: each line is parsed once,
# folded into the read model and kept (as clients may see it) for the
# log sent to them. Laws store_state_is_replay (the state is the replay
# of the log's changes) and store_load_append (a log read in two parts
# is the log read whole).

type Loaded is Data:
  Loaded{st: M.State, wire: List<&2, String>}

def Loaded.st(a: Loaded) -> M.State:
  match a:
    case Loaded{st, _}:
      st

# the lines clients may see, newest first
def Loaded.wire(a: Loaded) -> List<&2, String>:
  match a:
    case Loaded{_, ws}:
      ws

def Store.apply(m: Maybe<&2, M.Change>, st: M.State) -> M.State:
  match m:
    case Some{c}:
      M.State.apply(st, c)
    case None{}:
      st

# one line, parsed as m: a line that does not parse is left out
def Store.step(m: Maybe<&2, J.Json>, l: String, a: Loaded) -> Loaded:
  match m a:
    case Some{+o} Loaded{st, ws}:
      Loaded{Store.apply(C.Change.decode(o), st), C.Change.wire_line(o, l) <> ws}
    case None{} _:
      a

def Store.load(ls: List<&2, String>, a: Loaded) -> Loaded:
  match ls:
    case Nil{}:
      a
    case +l <> t:
      Store.load(t, Store.step(J.Json.parse(l), l, a))

# How many lines a log holds: List.length, counted in a tail call. Base's
# List.length adds after its recursive call, so on the JS target a log of
# a few hundred thousand lines overflows the stack (law store_size_is_length).
def Store.size.go(ls: List<&2, String>, n: Nat) -> Nat:
  match ls:
    case Nil{}:
      n
    case _ <> t:
      Store.size.go(t, 1n+n)

def Store.size(ls: List<&2, String>) -> Nat:
  Store.size.go(ls, 0n)

# The log's changes in order (what Store.load folds; for the laws)
# ----------------------------------------------------------------

def Store.put(m: Maybe<&2, M.Change>, rest: List<&2, M.Change>) -> List<&2, M.Change>:
  match m:
    case Some{c}:
      c <> rest
    case None{}:
      rest

def Store.change(m: Maybe<&2, J.Json>, rest: List<&2, M.Change>) -> List<&2, M.Change>:
  match m:
    case Some{+o}:
      Store.put(C.Change.decode(o), rest)
    case None{}:
      rest

def Store.changes(ls: List<&2, String>) -> List<&2, M.Change>:
  match ls:
    case Nil{}:
      Nil{}
    case +l <> t:
      Store.change(J.Json.parse(l), Store.changes(t))

# Writing
# -------
# A step's lines go to the log in one write (the server's Hub.commit), so a
# crash tears at most the step being written; Task.repair (hub.bend)
# finishes a delivery torn that way. A log whose last line was torn ends
# without a newline: the next write starts on a line of its own, so the
# fragment stays one malformed line that Store.step leaves out (laws
# commit_text_joins, store_torn_tail_resyncs).

# a step's lines, each ended by a newline
def Store.text(ls: List<&2, String>) -> String:
  match ls:
    case Nil{}:
      ""
    case l <> t:
      l ++ "\n" ++ Store.text(t)

def Store.last.go(bs: List<&2, U32>, +last: U32) -> U32:
  match bs:
    case Nil{}:
      last
    case +b <> t:
      Store.last.go(t, b)

# what to write before a log's first new line: a newline when its last
# byte (read at start-up) is not one
def Store.tail_fix(bs: List<&2, U32>) -> String:
  match bs:
    case Nil{}:
      ""
    case +b <> t:
      Bool.pick(String, U32.is_eq(Store.last.go(t, b), 10), "", "\n")

# What the hub's EvLog.read effect answers for a log of these bytes (the
# text that closes off a torn tail, then its whole lines, as utf8.bend's
# Lines and Utf8.decode read them), made by the effect without a List cell per
# byte (test/evlog_read_test.bend compares the two)
# (the tail's fix first, then the lines: one List, so it can be kept)
def Store.read.lines(r: UT.Lines & List<&2, String>) -> List<&2, String>:
  (_, ls) = r
  ls

def Store.read.of(+bs: List<&2, U32>) -> List<&2, String>:
  Store.tail_fix(bs) <> Store.read.lines(UT.Lines.feed(UT.Lines.new(), bs))

# the lines of what Store.read.of answers
def Store.read.body(r: List<&2, String>) -> List<&2, String>:
  match r:
    case Nil{}:
      Nil{}
    case _ <> ls:
      ls

# the tail's fix of what Store.read.of answers
def Store.read.tail(r: List<&2, String>) -> String:
  match r:
    case Nil{}:
      ""
    case t <> _:
      t

# a failed write may have left a cut line: the log is closed off with a
# newline before anything else is written, and the failure says when that
# did not work either
def Store.fail.msg(err: String, repaired: Bool) -> String:
  match repaired:
    case True{}:
      err
    case False{}:
      err ++ "; the log's tail could not be repaired"

# A write that fails is taken back: the log is cut to the length it had
# before the step (Store.trunc.args, run by the server), so no complete line
# of it stays on disk unfolded and no torn one is joined by a later write.
# When that cannot be done the hub holds the length and the reason
# (Store.hold) and commits nothing until the cut works; its error says so.
def Store.hold(len: String, err: String) -> String:
  len ++ "\t" ++ err

def Store.hold.first(+ps: List<&2, String>) -> String:
  match ps:
    case n <> _:
      n
    case Nil{}:
      ""

def Store.hold.rest(+ps: List<&2, String>) -> String:
  match ps:
    case _ <> t:
      String.join(t, "\t")
    case Nil{}:
      ""

def Store.hold.len(+h: String) -> String:
  Store.hold.first(String.split(h, '\t'))

def Store.hold.err(+h: String) -> String:
  Store.hold.rest(String.split(h, '\t'))

def Store.trunc.args(len: String, path: String) -> List<&2, String>:
  ["-s", len, path]

def Store.held.msg(+h: String) -> String:
  "the log could not be written and its failed write could not be taken back (" ++ Store.hold.err(h) ++ "); nothing is saved until it is"

# a line cut by a crash (the laws' example)
def Store.torn.demo() -> String:
  "{\"$\":\"ProjectRem"
