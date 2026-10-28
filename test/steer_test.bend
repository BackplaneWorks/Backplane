import Base
import ./check.bend as C
import ../src/core/model.bend as M
import ../src/core/live.bend as L
import ../src/core/steer.bend as S
import ../src/core/stamp.bend as ST

# A message sent while a Claude reply streams (core/steer.bend): what
# streamed so far is posted ahead of it, up to its last line break, and
# the whole reply that follows keeps only what streamed after the cut.

def cs.role(r: M.Role) -> String:
  match r:
    case M.User{}: "U"
    case M.Assistant{}: "A"
    case _: "?"

def cs.one(c: M.Change) -> String:
  match c:
    case M.MessagePosted{_, m, r, text, _}: m ++ "|" ++ cs.role(r) ++ "|" ++ text
    case _: "other"

def cs.show(xs: List<&2, M.Change>) -> String:
  match xs:
    case Nil{}: ""
    case c <> t: cs.one(c) ++ ";" ++ cs.show(t)

def made(+q1: List<&2, M.Change>) -> M.State:
  M.State.replay(List.append(&2, M.Change, [M.ThreadCreated{"t1", "", "Steer", M.Local{}, "", "", "claude", "", 1n},
    M.ThreadCreated{"t2", "", "Codex", M.Local{}, "", "", "codex", "", 1n},
    M.MessagePosted{"t1", "a0", M.Assistant{}, "earlier", 2n}, M.MessagePosted{"t1", "q1", M.User{}, "first", 3n},
    M.MessagePosted{"t2", "q1b", M.User{}, "first", 3n}], q1), M.State.empty())

def bufs(+t: String, +text: String) -> List<&2, L.Buf>:
  L.Live.add(L.Live.add(Nil{}, t, String.take(text, 4n)), t, String.drop(text, 4n))

def q2(+t: String) -> List<&2, M.Change>:
  [M.MessagePosted{t, "q2", M.User{}, "steer it", 5n}]

def keep(+text: String) -> String:
  S.Steer.text(S.Steer.keep(S.Steer.all(q2("t1"), made(Nil{}), bufs("t1", text)), bufs("t1", text), L.Live.ends(S.Steer.all(q2("t1"), made(Nil{}), bufs("t1", text)), bufs("t1", text))), "t1")

def ahead() -> List<&2, M.Change>:
  [M.MessagePosted{"t1", "q2:before", M.Assistant{}, "one\ntwo\n", 5n}, M.MessagePosted{"t1", "q2", M.User{}, "steer it", 5n}]

def whole(+text: String, +buf: String) -> String:
  cs.show(S.Steer.all([M.MessagePosted{"t1", "a1", M.Assistant{}, text, 6n}], made(ahead()), bufs("t1", buf)))

def got(r: Map<&2, Nat> & Nat) -> Nat:
  (_, n) = r
  n

# when the turn a reply ends started (0: it ends none)
def ends(+es: List<&2, M.Entry>, +id: String) -> String:
  Nat.show(got(Map.get(Nat, 0n, ST.Stamp.ends(es, False{}), id)))

def turn() -> List<&2, M.Entry>:
  [M.Msg{"q1", "t1", M.User{}, "first", 3n}, M.Msg{"q2:before", "t1", M.Assistant{}, "one\n", 5n}, M.Msg{"q2", "t1", M.User{}, "steer it", 5n}, M.Msg{"a1", "t1", M.Assistant{}, "rest", 6n}]

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("the reply so far goes ahead, up to its last line break", cs.show(S.Steer.all(q2("t1"), made(Nil{}), bufs("t1", "one\ntwo\nthr"))), "q2:before|A|one\ntwo\n;q2|U|steer it;")
    C.Check.str("nothing streaming: the message alone", cs.show(S.Steer.all(q2("t1"), made(Nil{}), Nil{})), "q2|U|steer it;")
    C.Check.str("an unfinished first line is not cut", cs.show(S.Steer.all(q2("t1"), made(Nil{}), bufs("t1", "one and"))), "q2|U|steer it;")
    C.Check.str("only Claude's threads", cs.show(S.Steer.all(q2("t2"), made(Nil{}), bufs("t2", "one\ntwo\nthr"))), "q2|U|steer it;")
    C.Check.str("the unfinished line keeps streaming", keep("one\ntwo\nthr"), "thr")
    C.Check.str("the whole reply after the cut: only the rest", whole("one\ntwo\nthree\nfour", "three\nfour"), "a1|A|three\nfour;")
    C.Check.str("a whole reply that does not start with it: whole", whole("other\ntext", "text"), "a1|A|other\ntext;")
    C.Check.str("nothing after the cut: nothing more", whole("one\ntwo\n", ""), "")
    C.Check.str("no cut before: the whole reply", cs.show(S.Steer.all([M.MessagePosted{"t1", "a1", M.Assistant{}, "all of it", 6n}], made(Nil{}), bufs("t1", "all of it"))), "a1|A|all of it;")
    C.Check.str("a reply posted ahead ends no turn", ends(turn(), "q2:before"), "0")
    C.Check.str("the reply after the message ends its turn", ends(turn(), "a1"), "5")
