import Base
import ./model.bend as M

# Context handed from one thread to another: a fork's start, a switch to
# another provider, a fork's work brought back to its source. It is a
# budgeted selection of the source thread's own messages and tool lines,
# never an agent-written summary: short conversations go whole; longer ones
# keep their newest items that fit the budget, plus the first request, and
# say how many earlier items were left out and where to read them
# (thread_read). One item longer than Hand.cap() is cut, and says so.
#
# The selection travels as a queued item of the target thread (its message
# id starts with Hand.prefix()), so it goes ahead of the next message the
# user sends there and the user's own words are never shortened to make
# room (laws hand_*).

def Hand.prefix() -> String:
  "handoff:"

# whether a queued item is handed-over context rather than a message
def Hand.is(+msg: String) -> Bool:
  String.starts_with(msg, Hand.prefix())

# the most characters of history one handoff carries
def Hand.budget() -> Nat:
  16000n

# the most characters of one item
def Hand.cap() -> Nat:
  4000n

def Hand.cut.k(long: Bool, +t: String, +n: Nat) -> String:
  match long:
    case True{}:
      String.take(t, n) ++ " ...[" ++ Nat.show(Nat.sub(String.length(t), n)) ++ " more characters]"
    case False{}:
      t

def Hand.cut(+t: String, +n: Nat) -> String:
  Hand.cut.k(Nat.is_gt(String.length(t), n), t, n)

def Hand.role(r: M.Role) -> String:
  match r:
    case M.User{}:
      "User"
    case M.Assistant{}:
      "Assistant"
    case M.System{}:
      "Note"

# one entry as a line of the handoff
def Hand.line(e: M.Entry) -> String:
  match e:
    case M.Msg{_, _, r, +text, _}:
      Hand.role(r) ++ ": " ++ Hand.cut(text, Hand.cap())
    case M.Act{_, _, +tone, +kind, +summary, _}:
      "Tool " ++ kind ++ Bool.pick(String, String.eq(tone, "error"), " (failed)", "") ++ ": " ++ Hand.cut(summary, 300n)

def Hand.mine.k(hit: Bool, e: M.Entry, rest: List<&2, M.Entry>) -> List<&2, M.Entry>:
  match hit:
    case True{}:
      e <> rest
    case False{}:
      rest

def Hand.of(e: M.Entry) -> String:
  match e:
    case M.Msg{_, t, _, _, _}:
      t
    case M.Act{_, t, _, _, _, _}:
      t

# a thread's entries, newest first (as the state keeps them)
def Hand.mine(es: List<&2, M.Entry>, +th: String) -> List<&2, M.Entry>:
  match es:
    case Nil{}:
      Nil{}
    case +e <> t:
      Hand.mine.k(String.eq(Hand.of(e), th), e, Hand.mine(t, th))

def Hand.user(e: M.Entry) -> Bool:
  match e:
    case M.Msg{_, _, M.User{}, _, _}:
      True{}
    case _:
      False{}

# what fits: the newest entries whose lines fit in the budget, oldest
# first, and how many older ones were left out. Once one does not fit, all
# older ones are left out too (the kept history has no holes).
type Sel is Data:
  Sel{left: Nat, lines: List<&2, String>, full: Bool, omitted: Nat}

def Hand.put(full: Bool, fits: Bool, +line: String, +left: Nat, ls: List<&2, String>, +om: Nat) -> Sel:
  match full fits:
    case False{} True{}:
      Sel{Nat.sub(left, 1n+String.length(line)), line <> ls, False{}, om}
    case _ _:
      Sel{left, ls, True{}, 1n+om}

def Hand.step(a: Sel, +e: M.Entry) -> Sel:
  match a:
    case Sel{+left, ls, full, +om}:
      Hand.put(full, Nat.is_le(1n+String.length(Hand.line(e)), left), Hand.line(e), left, ls, om)

# newest first in, oldest first out
def Hand.go(es: List<&2, M.Entry>, a: Sel) -> Sel:
  match es:
    case Nil{}:
      a
    case e <> t:
      Hand.go(t, Hand.step(a, e))

# the thread's first request (its oldest user message), for when the
# selection leaves it out; newest first in, so the last user message seen
def Hand.first(es: List<&2, M.Entry>, +acc: String) -> String:
  match es:
    case Nil{}:
      acc
    case +e <> t:
      Hand.first(t, Bool.pick(String, Hand.user(e), Hand.line(e), acc))

def Hand.head(+omitted: Nat, +first: String, +th: String) -> List<&2, String>:
  Bool.pick(List<&2, String>, Nat.is_eq(omitted, 0n), Nil{},
    [Hand.cut(first, 2000n), "[" ++ Nat.show(omitted) ++ " earlier items left out; thread_read with threadId " ++ th ++ " has them all]"])

# what was left out: every item but the lines kept (total: how many items
# the thread has, Hand.total)
def Hand.body(s: Sel, +total: Nat, +first: String, +th: String) -> String:
  match s:
    case Sel{_, +ls, _, _}:
      String.join(List.append(&2, String, Hand.head(Nat.sub(total, List.length(&2, String, ls)), first, th), ls), "\n\n")

# the items of a run, without its count (a hub's state ends a thread's run
# with one, model.bend's Win.count)
def Hand.real(es: List<&2, M.Entry>) -> List<&2, M.Entry>:
  match es:
    case Nil{}:
      Nil{}
    case +e <> t:
      Hand.mine.k(Bool.not(M.Win.is_count(e)), e, Hand.real(t))

def Hand.total.go(es: List<&2, M.Entry>, +n: Nat) -> Nat:
  match es:
    case Nil{}:
      n
    case +e <> t:
      Hand.total.go(t, Bool.pick(Nat, M.Win.is_count(e), M.Win.at([e], 0n), 1n+n))

# how many items a run stands for: its count when it has one (a hub's
# window holds the newest only), else its length
def Hand.total(es: List<&2, M.Entry>) -> Nat:
  Hand.total.go(es, 0n)

# the handoff text: why it is here, the selection, and where it ends
def Hand.text(+why: String, +es: List<&2, M.Entry>, +th: String) -> String:
  "[Backplane] " ++ why ++ "\n\n" ++ Hand.body(Hand.go(Hand.real(es), Sel{Hand.budget(), Nil{}, False{}, 0n}), Hand.total(es), Hand.first(Hand.real(es), ""), th) ++
    "\n\n[End of the handed-over context. The user's message follows.]"

def Hand.title(t: M.Thread) -> String:
  match t:
    case M.Thread{_, _, ti, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _}:
      ti

# a fork's start: the source thread's conversation so far
def Hand.fork(+es: List<&2, M.Entry>, +src: String, +title: String) -> String:
  Hand.text("This thread is a fork of \"" ++ title ++ "\" (" ++ src ++ "). You continue its work from where it stands; its conversation so far, oldest first:",
    Hand.mine(es, src), src)

# a fork's work brought back to its source: what happened in the fork
def Hand.back(+es: List<&2, M.Entry>, +fork: String, +title: String) -> String:
  Hand.text("Work done in a fork of this thread, \"" ++ title ++ "\" (" ++ fork ++ "), brought back here. Its files stay on its own branch; what was said and done there, oldest first:",
    Hand.mine(es, fork), fork)

def Hand.note(+note: String) -> String:
  Bool.pick(String, String.is_empty(note), "", " The user's note with it: " ++ note)

# a conversation given away (/give, bothub.bend's Give): what was said and
# done in the thread given from, with the user's note
def Hand.give(+es: List<&2, M.Entry>, +src: String, +title: String, +note: String) -> String:
  Hand.text("The user hands you the conversation of the thread \"" ++ title ++ "\" (" ++ src ++ ")." ++ Hand.note(note) ++ " Its conversation, oldest first:",
    Hand.mine(es, src), src)

# a side question (/btw): the thread so far, then the question, for the
# writing model to answer beside the running turn
def Hand.btw(+es: List<&2, M.Entry>, +th: String, +title: String, +q: String) -> String:
  Hand.text("The user asks a side question about the thread \"" ++ title ++ "\" (" ++ th ++ ") while its agent works. Its conversation so far, oldest first:",
    Hand.mine(es, th), th) ++ "\n\n" ++ q

# the newest n of a list (newest first in)
def Hand.newest(n: Nat, es: List<&2, M.Entry>) -> List<&2, M.Entry>:
  match n es:
    case 0n _:
      Nil{}
    case _ Nil{}:
      Nil{}
    case 1n+k e <> t:
      e <> Hand.newest(k, t)

# the newest n, counted as n (the window may hold fewer)
def Hand.newest.c(+n: Nat, +es: List<&2, M.Entry>, +th: String) -> List<&2, M.Entry>:
  List.append(&2, M.Entry, Hand.newest(n, es), [M.Win.count(th, n)])

# a provider taking a thread up: what happened since it last took part
# (seen: how many of the thread's entries it had seen; 0 for a provider
# new to the thread, which then gets it all)
def Hand.switch(+es: List<&2, M.Entry>, +th: String, +from: String, +to: String, +seen: Nat) -> String:
  Hand.text(Bool.pick(String, Nat.is_eq(seen, 0n),
      "This thread was worked on with " ++ from ++ " until now; you (" ++ to ++ ") take it over. Its conversation so far, oldest first:",
      "This thread went on with " ++ from ++ " since you (" ++ to ++ ") last took part. What happened meanwhile, oldest first:"),
    Hand.newest.c(Nat.sub(Hand.total(Hand.mine(es, th)), seen), Hand.real(Hand.mine(es, th)), th), th)
