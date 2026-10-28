import Base
import ./model.bend as M
import ./live.bend as L

# A message sent while a reply streams (pure; server.bend's Hub.commit
# applies Steer.all to every step's changes before they are written, and
# Steer.keep to the live buffers after)
# ======================================================================
# The person's message to a Claude thread whose reply is streaming used to
# land above that reply (the reply is posted only when its text block
# ends), so the reply in progress jumped down and the thread read Q1, Q2,
# A1, A2. Now what streamed so far, up to its last whole line (Steer.head),
# is posted first as a reply of its own (the message's id with ":before"),
# then the message: it lands under the reply in progress, nothing above it
# moves, and the rest of the stream goes on under it (Steer.keep puts the
# unfinished line back in the buffer). When Claude posts the whole block,
# only what streamed after the cut is posted (the buffer then), so no text
# shows twice; a block that does not start with what was posted ahead is
# posted whole. A buffer cut at Live.cap() is never posted ahead.

def Steer.mark() -> String:
  ":before"

# a reply posted ahead of a message (Stamp.ends gives it no "Finished")
def Steer.ahead.is(+id: String) -> Bool:
  String.ends_with(id, Steer.mark())

# A turn stopped while its reply streams (Stop, or any interrupt): what
# streamed so far is posted as its reply in the same step, so the text
# the person was reading stays and gains its "Stopped" stamp, where it
# used to go blank until (or unless) the agent's own message landed. Its
# id ends ":stopped"; it ends its turn (Stamp.ends). Claude's message for
# it may still land after the Stop: like a reply after one posted ahead
# of a message, it adds only what streamed after (nothing, a stopped turn
# streams no more). A buffer cut at Live.cap() is not posted.
def Steer.stop.mark() -> String:
  ":stopped"

def Steer.stop.is(+id: String) -> Bool:
  String.ends_with(id, Steer.stop.mark())

# posted ahead of the agent's own message: for a message sent while it
# streamed, or as its turn stopped when no message of the person came
# since (user: one did; that reply was an earlier turn's)
def Steer.posted.is(+id: String, +user: Bool) -> Bool:
  Bool.or(Steer.ahead.is(id), Bool.and(Bool.not(user), Steer.stop.is(id)))

def Steer.cut(tr: Bool, cs: List<&2, String>) -> String:
  match tr:
    case True{}: ""
    case False{}: String.join(List.reverse(&2, String, cs), "")

def Steer.buf(b: L.Buf) -> String:
  match b:
    case L.Buf{_, cs, _, tr, _}: Steer.cut(tr, cs)

def Steer.text.k(hit: Bool, b: L.Buf, +rest: String) -> String:
  match hit:
    case True{}: Steer.buf(b)
    case False{}: rest

# what a thread has streamed so far ("" for none, or when its start was cut)
def Steer.text(bs: List<&2, L.Buf>, +thread: String) -> String:
  match bs:
    case Nil{}: ""
    case +b <> t: Steer.text.k(L.Live.is(b, thread), b, Steer.text(t, thread))

# all but the last, and the last
def Steer.init(+xs: List<&2, String>) -> List<&2, String>:
  List.reverse(&2, String, List.drop(&2, String, List.reverse(&2, String, xs), 1n))

def Steer.last(+xs: List<&2, String>) -> String:
  Maybe.default(&2, String, List.head(&2, String, List.reverse(&2, String, xs)), "")

# (String.split is native on the hub: a long reply is never walked a
# character at a time)
def Steer.at.k(+ps: List<&2, String>, +sep: String, +whole: String, head: Bool) -> String:
  match head:
    case True{}: Bool.pick(String, Nat.is_lt(List.length(&2, String, ps), 2n), whole, String.join(Steer.init(ps), sep) ++ sep)
    case False{}: Bool.pick(String, Nat.is_lt(List.length(&2, String, ps), 2n), "", Steer.last(ps))

def Steer.at(+text: String, head: Bool) -> String:
  Steer.at.k(String.split(text, '\n'), "\n", text, head)

# what is posted ahead: the text up to its last line break ("" when it has
# none: an unfinished first line is not cut), and what stays streaming
def Steer.head(+text: String) -> String:
  Bool.pick(String, String.contains(text, "\n"), Steer.at(text, True{}), "")

def Steer.tail(+text: String) -> String:
  Bool.pick(String, String.contains(text, "\n"), Steer.at(text, False{}), text)

def Steer.claude.t(t: M.Thread) -> Bool:
  match t:
    case M.Thread{_, _, _, _, _, _, _, _, _, _, _, _, _, _, _, p, _, _, _, _, _}: String.eq(p, "claude")

def Steer.claude.k(r: Map<&2, M.Thread> & M.Thread) -> Bool:
  (_, t) = r
  Steer.claude.t(t)

def Steer.claude(+s: M.State, +id: String) -> Bool:
  match s:
    case M.State{_, ts, _, _, _, _, _, _, _, _, _}: Steer.claude.k(Map.get(M.Thread, M.Thread.blank(id), ts, id))

# what was posted ahead since the agent's last whole reply (the thread's
# entries newest first: the person's messages are passed over, the replies
# posted ahead gathered, anything else ends it)
def Steer.ahead.on(e: M.Entry, +user: Bool) -> Bool:
  match e:
    case M.Msg{_, _, M.User{}, _, _}: True{}
    case M.Msg{+id, _, M.Assistant{}, _, _}: Steer.posted.is(id, user)
    case _: False{}

def Steer.ahead.add(e: M.Entry, +user: Bool, +acc: String) -> String:
  match e:
    case M.Msg{+id, _, M.Assistant{}, +text, _}: Bool.pick(String, Steer.posted.is(id, user), text ++ acc, acc)
    case _: acc

def Steer.asked(e: M.Entry) -> Bool:
  match e:
    case M.Msg{_, _, M.User{}, _, _}: True{}
    case _: False{}

def Steer.ahead.u(es: List<&2, M.Entry>, go: Bool, +user: Bool, +acc: String) -> String:
  match es go:
    case Nil{} _: acc
    case +e <> +t True{}: Steer.ahead.u(t, Steer.ahead.on(e, user), Bool.or(user, Steer.asked(e)), Steer.ahead.add(e, user, acc))
    case +e <> +t False{}: acc

def Steer.ahead(es: List<&2, M.Entry>, go: Bool, +acc: String) -> String:
  Steer.ahead.u(es, go, False{}, acc)

def Steer.user(go: Bool, +t: String, +m: String, +text: String, +at: Nat, +head: String) -> List<&2, M.Change>:
  match go:
    case True{}: [M.MessagePosted{t, m ++ Steer.mark(), M.Assistant{}, head, at}, M.MessagePosted{t, m, M.User{}, text, at}]
    case False{}: [M.MessagePosted{t, m, M.User{}, text, at}]

def Steer.whole(cut: Bool, +t: String, +m: String, +text: String, +at: Nat, +rest: String) -> List<&2, M.Change>:
  match cut:
    case True{}: Bool.pick(List<&2, M.Change>, String.is_empty(String.trim(rest)), Nil{}, [M.MessagePosted{t, m, M.Assistant{}, rest, at}])
    case False{}: [M.MessagePosted{t, m, M.Assistant{}, text, at}]

def Steer.gen.k(hit: Bool, b: L.Buf, +rest: Nat) -> Nat:
  match hit b:
    case True{} L.Buf{_, _, _, _, g}: g
    case False{} _: rest

# the thread's buffer's gen (each clearing bumps it: an id that is new)
def Steer.gen(bs: List<&2, L.Buf>, +thread: String) -> Nat:
  match bs:
    case Nil{}: 0n
    case +b <> t: Steer.gen.k(L.Live.is(b, thread), b, Steer.gen(t, thread))

def Steer.stopped(none: Bool, +t: String, +gen: Nat, +at: Nat, +text: String) -> List<&2, M.Change>:
  match none:
    case True{}: [M.TurnChanged{t, M.Interrupted{}, at}]
    case False{}: [M.MessagePosted{t, "live:" ++ t ++ ":" ++ Nat.show(gen) ++ Steer.stop.mark(), M.Assistant{}, text, at}, M.TurnChanged{t, M.Interrupted{}, at}]

# one change as written: the person's message into a streaming Claude
# reply with the reply so far ahead of it; a whole reply after that with
# only what streamed after the cut; a turn stopped with what it streamed
# posted first
def Steer.change(+s: M.State, +bs: List<&2, L.Buf>, +c: M.Change) -> List<&2, M.Change>:
  match c:
    case M.TurnChanged{+t, M.Interrupted{}, +at}:
      Steer.stopped(String.is_empty(String.trim(Steer.text(bs, t))), t, Steer.gen(bs, t), at, Steer.text(bs, t))
    case M.MessagePosted{+t, +m, M.User{}, +text, +at}:
      Steer.user(Bool.and(Steer.claude(s, t), Bool.not(String.is_empty(Steer.head(Steer.text(bs, t))))), t, m, text, at, Steer.head(Steer.text(bs, t)))
    case M.MessagePosted{+t, +m, M.Assistant{}, +text, +at}:
      Steer.whole(Bool.and(Bool.not(String.is_empty(Steer.ahead(M.State.mine(s, t), True{}, ""))),
          Bool.and(String.starts_with(text, Steer.ahead(M.State.mine(s, t), True{}, "")), Bool.not(Steer.ahead.is(m)))),
        t, m, text, at, Steer.text(bs, t))
    case _: [c]

# a step's changes as written (test/steer_test.bend): each user message
# sees the buffers as the changes before it left them (Live.ends: a reply
# posted ahead empties its thread's)
def Steer.all(cs: List<&2, M.Change>, +s: M.State, +bs: List<&2, L.Buf>) -> List<&2, M.Change>:
  match cs:
    case Nil{}: Nil{}
    case +c <> t: List.append(&2, M.Change, Steer.change(s, bs, c), Steer.all(t, s, L.Live.ends(Steer.change(s, bs, c), bs)))

def Steer.keep.k(ahead: Bool, +t: String, +old: List<&2, L.Buf>, +bs: List<&2, L.Buf>) -> List<&2, L.Buf>:
  match ahead:
    case True{}: Bool.pick(List<&2, L.Buf>, String.is_empty(Steer.tail(Steer.text(old, t))), bs, L.Live.add(bs, t, Steer.tail(Steer.text(old, t))))
    case False{}: bs

def Steer.keep.c(+c: M.Change, +old: List<&2, L.Buf>, bs: List<&2, L.Buf>) -> List<&2, L.Buf>:
  match c:
    case M.MessagePosted{+t, +m, M.Assistant{}, _, _}: Steer.keep.k(Steer.ahead.is(m), t, old, bs)
    case _: bs

# the buffers after the changes were written (bs: Live.ends of the old
# ones): a thread whose reply was posted ahead keeps streaming its
# unfinished line
def Steer.keep(cs: List<&2, M.Change>, +old: List<&2, L.Buf>, bs: List<&2, L.Buf>) -> List<&2, L.Buf>:
  match cs:
    case Nil{}: bs
    case +c <> t: Steer.keep(t, old, Steer.keep.c(c, old, bs))
