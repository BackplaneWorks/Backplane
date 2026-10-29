import Base
import ./check.bend as C
import ../src/core/model.bend as M
import ../src/core/tz.bend as Z
import ../src/core/stamp.bend as S

# Times under messages (core/stamp.bend): when each was sent, in the
# person's zone, and when a turn finished on its last reply.

# Tue 13 Oct 2026 09:00:00 UTC
def t0() -> Nat:
  1791882000n

def at(+s: Nat) -> Nat:
  Nat.add(t0(), s)

def u(+id: String, +s: Nat) -> M.Entry:
  M.Msg{id, "t", M.User{}, "hi", at(s)}

def a(+id: String, +s: Nat) -> M.Entry:
  M.Msg{id, "t", M.Assistant{}, "ok", at(s)}

def act(+id: String, +s: Nat) -> M.Entry:
  M.Act{id, "t", "tool", "Bash", "ls", at(s)}

# two turns: the first over, the second with a reply so far
def es() -> List<&2, M.Entry>:
  [u("u1", 0n), a("a1", 30n), act("x1", 40n), a("a2", 433n), act("x2", 440n), u("u2", 1000n), a("a3", 1010n)]

def ss(+es: List<&2, M.Entry>, +r: Z.TzRule, +now: Nat, +running: Bool) -> S.Stamps:
  S.Stamps.at(r, now, S.Stamp.ends(es, running))

# the stamp of entry i
def st(+es: List<&2, M.Entry>, +r: Z.TzRule, +now: Nat, +running: Bool, +i: Nat) -> String:
  S.Stamp.text(ss(es, r, now, running), Maybe.default(&2, M.Entry, List.get(&2, M.Entry, es, i), act("none", 0n)))

# the last turn stopped (the person's Stop): its last reply says so
def ss.stop(+es: List<&2, M.Entry>, +r: Z.TzRule, +now: Nat) -> S.Stamps:
  S.Stamps.at.s(r, now, S.Stamp.ends(es, False{}), Map.set(&2, Nat, Map.new(&2, Nat), S.Stamp.last.go(List.reverse(&2, M.Entry, es)), 1n))

def st.stop(+es: List<&2, M.Entry>, +r: Z.TzRule, +now: Nat, +i: Nat) -> String:
  S.Stamp.text(ss.stop(es, r, now), Maybe.default(&2, M.Entry, List.get(&2, M.Entry, es, i), act("none", 0n)))

# a thread whose changes are cs (the stopped marks are settings)
def state(cs: List<&2, M.Change>) -> M.State:
  M.State.replay(cs, M.State.empty())

# the stamp of entry i as a client makes it: the state's marks, and
# whether the thread's last turn is stopped now
def st.of(+es: List<&2, M.Entry>, +s: M.State, +running: Bool, +stopped: Bool, +i: Nat) -> String:
  S.Stamp.text(S.Stamps.of.s(es, s, at(3600n), running, stopped), Maybe.default(&2, M.Entry, List.get(&2, M.Entry, es, i), act("none", 0n)))

# the same while the thread waits on work it started
def st.w(+es: List<&2, M.Entry>, +s: M.State, +running: Bool, +stopped: Bool, +waiting: Bool, +i: Nat) -> String:
  S.Stamp.text(S.Stamps.of.w(es, s, at(3600n), running, stopped, waiting), Maybe.default(&2, M.Entry, List.get(&2, M.Entry, es, i), act("none", 0n)))

# a thread's entries as the hub posts them, for Stamp.stop.mark
def posted(es: List<&2, M.Entry>) -> List<&2, M.Change>:
  match es:
    case Nil{}:
      Nil{}
    case M.Msg{id, th, r, x, t} <> rest:
      M.MessagePosted{th, id, r, x, t} <> posted(rest)
    case M.Act{id, th, tone, k, x, t} <> rest:
      M.ActivityLogged{th, id, tone, k, x, t} <> posted(rest)

def marks(cs: List<&2, M.Change>) -> String:
  match cs:
    case Nil{}:
      ""
    case M.SettingSet{k, v, _} <> rest:
      k ++ "=" ++ v ++ " " ++ marks(rest)
    case _ <> rest:
      marks(rest)

# three turns: the first stopped, the second finished, the third stopped
# with a reply that landed after the Stop
def es3() -> List<&2, M.Entry>:
  [u("u1", 0n), a("a1", 5n), u("u2", 60n), a("a2", 70n), u("u3", 120n), a("a3", 130n), a("a4", 131n)]

def marked() -> M.State:
  state([M.SettingSet{"stopped.u1", "1", 6n}, M.SettingSet{"stopped.u3", "1", 130n}])

def utc() -> Z.TzRule:
  Z.Tz.utc0()

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("a message sent today shows its time", st(es(), utc(), at(3600n), True{}, 0n), "09:00")
    C.Check.str("a reply mid-turn shows its time", st(es(), utc(), at(3600n), True{}, 1n), "09:00")
    C.Check.str("a tool call shows nothing", st(es(), utc(), at(3600n), True{}, 2n), "")
    C.Check.str("a finished turn's last reply: when, and how long", st(es(), utc(), at(3600n), True{}, 3n), "Finished 09:07 · 7m 13s")
    C.Check.str("the turn still running has not finished", st(es(), utc(), at(3600n), True{}, 6n), "09:16")
    C.Check.str("once it stops, it has", st(es(), utc(), at(3600n), False{}, 6n), "Finished 09:16 · 10s")
    C.Check.str("a page from mid-turn: when it finished alone", st([a("a1", 30n), a("a2", 433n), u("u2", 1000n)], utc(), at(3600n), False{}, 1n), "Finished 09:07")
    C.Check.str("only a turn's last reply finishes it", st([a("a1", 30n), a("a2", 433n), u("u2", 1000n)], utc(), at(3600n), False{}, 0n), "09:00")
    C.Check.str("another day shows the date", st(es(), utc(), at(172800n), True{}, 0n), "Tue 13 Oct 09:00")
    C.Check.str("in the person's zone", st(es(), Z.Tz.fixed(True{}, 10800n), at(3600n), True{}, 0n), "12:00")
    C.Check.str("a stopped turn's last reply says Stopped", st.stop(es(), utc(), at(3600n), 6n), "Stopped 09:16 · 10s")
    C.Check.str("the turns before it still say Finished", st.stop(es(), utc(), at(3600n), 3n), "Finished 09:07 · 7m 13s")
    C.Check.str("a stop with no reply yet marks nothing", S.Stamp.last.go(List.reverse(&2, M.Entry, [u("u1", 0n), a("a1", 30n), u("u2", 60n)])), "")
    C.Check.str("a turn with no reply finishes nothing", st([u("u1", 0n), act("x1", 5n), u("u2", 60n)], utc(), at(3600n), False{}, 0n), "09:00")
    C.Check.str("a stopped turn still says Stopped once the next has started", st.of(es3(), marked(), True{}, False{}, 1n), "Stopped 09:00 · 5s")
    C.Check.str("and once the next has finished", st.of(es3(), marked(), False{}, False{}, 1n), "Stopped 09:00 · 5s")
    C.Check.str("a finished turn between them says Finished", st.of(es3(), marked(), False{}, False{}, 3n), "Finished 09:01 · 10s")
    C.Check.str("a reply landing after the Stop ends its stopped turn", st.of(es3(), marked(), False{}, False{}, 6n), "Stopped 09:02 · 11s")
    # a turn over while work it started is out: the last reply waits, never finishes
    C.Check.str("waiting on subagents: the last reply says Waiting since", st.w(es(), state(Nil{}), False{}, False{}, True{}, 6n), "Waiting since 09:16 · 10s")
    C.Check.str("waiting: the turns before still say Finished", st.w(es(), state(Nil{}), False{}, False{}, True{}, 3n), "Finished 09:07 · 7m 13s")
    C.Check.str("done waiting: Finished", st.w(es(), state(Nil{}), False{}, False{}, False{}, 6n), "Finished 09:16 · 10s")
    C.Check.str("a running turn does not wait", st.w(es(), state(Nil{}), True{}, False{}, True{}, 6n), "09:16")
    C.Check.str("a stopped turn stays Stopped while work is out", st.w(es3(), marked(), False{}, True{}, True{}, 6n), "Stopped 09:02 · 11s")
    C.Check.str("the reply before it is not the turn's last", st.of(es3(), marked(), False{}, False{}, 5n), "09:02")
    C.Check.str("an unmarked last turn the thread stopped says Stopped", st.of(es(), state(Nil{}), False{}, True{}, 6n), "Stopped 09:16 · 10s")
    C.Check.str("with no mark, the turns before it say Finished", st.of(es(), state(Nil{}), False{}, True{}, 3n), "Finished 09:07 · 7m 13s")
    C.Check.str("the turn still running says nothing yet", st.of(es3(), marked(), True{}, False{}, 6n), "09:02")
    C.Check.str("a Stop marks the message that started the turn", marks(S.Stamp.stop.mark(state(posted(es3())), "t", at(140n))), "stopped.u3=1 ")
    C.Check.str("a reply sent to another thread is not its message", marks(S.Stamp.stop.mark(state(posted([u("u1", 0n), a("a1", 5n), M.Msg{"x", "t2", M.User{}, "hi", at(6n)}])), "t", at(10n))), "stopped.u1=1 ")
    C.Check.str("a turn with no message of the person marks nothing", marks(S.Stamp.stop.mark(state(posted([a("a1", 5n)])), "t", at(10n))), "")
