import Base
import ./check.bend as C
import ../src/core/model.bend as M
import ../src/core/hub.bend as B
import ../src/core/json.bend as J

# Worktrees made again (model.bend's Worktree.waits, hub.bend's Wt.asked,
# Wt.answered; laws wt_again_*): a message to a thread whose worktree was
# removed asks for it once, a message while that ask is out waits with it,
# and the worktree's arrival runs everything that waited, once.

def changes(st: B.Step) -> List<&2, M.Change>:
  match st:
    case B.Step{cs, _}: cs

def outs(st: B.Step) -> List<&2, B.Out>:
  match st:
    case B.Step{_, os}: os

def again.one(o: B.Out) -> Nat:
  match o:
    case B.Git{_, _, k, _, _, a, _}: Bool.pick(Nat, Bool.and(String.eq(k, "worktree"), String.eq(a, "again")), 1n, 0n)
    case _: 0n

# how many times the step asks for the worktree again
def agains(os: List<&2, B.Out>) -> Nat:
  match os:
    case Nil{}: 0n
    case o <> rest: Nat.add(again.one(o), agains(rest))

def turn(st: M.TurnState) -> String:
  match st:
    case M.Running{}: "running"
    case _: "turn"

def kinds.one(c: M.Change) -> String:
  match c:
    case M.MessagePosted{_, _, _, _, _}: "post"
    case M.TurnQueued{_, _, _, _}: "queued"
    case M.QueueTaken{_, _}: "taken"
    case M.TurnChanged{_, st, _}: turn(st)
    case M.WorktreeSet{_, _, _, _}: "worktree"
    case M.WorktreeSkipped{_, _}: "skipped"
    case M.WorktreeRemoved{_, _}: "removed"
    case M.SettingSet{k, v, _}: Bool.pick(String, String.starts_with(k, "wt.again."), "ask=" ++ v, "setting")
    case _: "other"

def kinds.go(cs: List<&2, M.Change>) -> List<&2, String>:
  match cs:
    case Nil{}: Nil{}
    case c <> rest: kinds.one(c) <> kinds.go(rest)

def kinds(cs: List<&2, M.Change>) -> String:
  String.join(kinds.go(cs), ",")

# the example: t1's turn completed at 5, its worktree removed at 6
def gone() -> M.State:
  B.Wt.demo(Nil{})

def post(+s: M.State, +now: Nat, +msg: String) -> B.Step:
  B.Hub.post(s, now, "t1", msg, M.User{}, "more")

def after(+s: M.State, st: B.Step) -> M.State:
  M.State.replay(changes(st), s)

# the first message, 1 s after the removal
def asked() -> M.State:
  after(gone(), post(gone(), 7n, "m2"))

# a second message while the ask is out
def waiting() -> M.State:
  after(asked(), post(asked(), 20n, "m3"))

def made() -> B.Step:
  B.Wt.ready(waiting(), 25n, 0, "", "t1", M.WorktreeSet{"t1", "b1", "/p/.backplane/worktrees/b1", 25n})

def waits(+s: M.State, +now: Nat) -> String:
  Bool.show(M.Worktree.waits(s, M.State.thread.get(s, "t1"), now))

# A live worktree remains usable even if an old recreation setting survives.
def healthy(ask: String) -> M.State:
  B.Wt.demo([M.WorktreeSet{"t1", "b1", "/p/.backplane/worktrees/b1", 7n}, M.TurnChanged{"t1", M.Running{}, 8n}, M.SettingSet{"wt.again.t1", ask, 190n}])

# The confirmed exceptional case: absent worktree and a fresh ask, despite
# an old running marker. Its original pending(activity) check has expired.
def fresh_missing() -> M.State:
  B.Wt.demo([M.TurnChanged{"t1", M.Running{}, 7n}, M.SettingSet{"wt.again.t1", "190", 190n}])

def content_texts(xs: List<&2, J.Json>) -> String:
  match xs:
    case Nil{}: ""
    case x <> rest: J.Json.get_str(x, "text", "") ++ content_texts(rest)

def send_texts(os: List<&2, B.Out>) -> String:
  match os:
    case Nil{}: ""
    case B.AgentSend{_, line} <> rest:
      content_texts(J.Json.elems(J.Json.get_obj(J.Json.get_obj(Maybe.default(&2, J.Json, J.Json.parse(line), J.Null{}), "message"), "content"))) ++ send_texts(rest)
    case _ <> rest: send_texts(rest)

def send_count(os: List<&2, B.Out>) -> Nat:
  match os:
    case Nil{}: 0n
    case B.AgentSend{_, _} <> rest: Nat.add(1n, send_count(rest))
    case _ <> rest: send_count(rest)

def ordered_first() -> M.State:
  after(gone(), B.Hub.post(gone(), 7n, "t1", "first", M.User{}, "first queued input"))

def ordered_waiting() -> M.State:
  after(ordered_first(), B.Hub.post(ordered_first(), 20n, "t1", "second", M.User{}, "second queued input"))

def ordered_ready(s: M.State, +at: Nat) -> B.Step:
  B.Wt.ready(s, at, 0, "", "t1", M.WorktreeSet{"t1", "b1", "/p/.backplane/worktrees/b1", at})

def exceptional_waiting() -> M.State:
  after(fresh_missing(), post(fresh_missing(), 200n, "exceptional"))

def exceptional_ready() -> M.State:
  after(exceptional_waiting(), ordered_ready(exceptional_waiting(), 201n))

def exceptional_done() -> B.Step:
  B.Queue.drain(exceptional_ready(), 202n, "t1", B.Step{[M.TurnChanged{"t1", M.Completed{}, 202n}], Nil{}})

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("the old guard read a just-ended turn as a worktree being made", Bool.show(M.Worktree.pending(M.State.thread.get(gone(), "t1"), 7n)), "True")
    C.Check.str("a just-ended turn's removed worktree is not being made", waits(gone(), 7n), "False")
    C.Check.str("a message 1 s after the removal asks for the worktree once", Nat.show(agains(outs(post(gone(), 7n, "m2")))), "1")
    C.Check.str("and records the ask, keeping the message queued", kinds(changes(post(gone(), 7n, "m2"))), "ask=7,post,queued")
    C.Check.str("a message 100 s after the turn ended asks too", Nat.show(agains(outs(post(gone(), 105n, "m2")))), "1")
    C.Check.str("a message from another thread asks too", Nat.show(agains(outs(B.Hub.inbox.id(gone(), 7n, "t9", "t1", M.User{}, "note", "t:t9:m1")))), "1")
    C.Check.str("while the ask is out the thread waits", waits(asked(), 20n), "True")
    C.Check.str("a second message then asks no second time", Nat.show(agains(outs(post(asked(), 20n, "m3")))), "0")
    C.Check.str("and waits in the queue with the first", kinds(changes(post(asked(), 20n, "m3"))), "post,queued")
    C.Check.str("the worktree comes: the queue runs once and the ask is cleared", kinds(changes(made())), "worktree,ask=,taken,running")
    C.Check.str("after it came nothing waits", waits(after(waiting(), made()), 30n), "False")
    C.Check.str("an ask never answered: after the wait the next message asks again", Nat.show(agains(outs(post(waiting(), 128n, "m4")))), "1")
    C.Check.str("worktree could not be made: the ask is cleared and the queue runs", kinds(changes(B.Wt.ready(waiting(), 25n, 0, "", "t1", M.WorktreeSkipped{"t1", 25n}))), "skipped,ask=,taken,running")
    C.Check.str("a removal clears an ask left behind", kinds(changes(B.Hub.agentm(5n, asked(), 30n, 0, "", J.Json.obj([J.KV{"thread", J.Str{"t1"}}]), "thread.unrooted"))), "removed,ask=")
    C.Check.str("a removal with no ask writes no extra line", kinds(changes(B.Hub.agentm(5n, gone(), 30n, 0, "", J.Json.obj([J.KV{"thread", J.Str{"t1"}}]), "thread.unrooted"))), "removed")
    C.Check.str("a new thread still waits for its first worktree", waits(M.State.replay([M.ProjectCreated{"p1", "p", "/p", 1n}, M.ThreadCreated{"t1", "p1", "x", M.Worktree{}, "", "", "claude", "", 1n}], M.State.empty()), 50n), "True")
    C.Check.str("a new thread's message waits for it, asking nothing", Nat.show(agains(outs(post(M.State.replay([M.ProjectCreated{"p1", "p", "/p", 1n}, M.ThreadCreated{"t1", "p1", "x", M.Worktree{}, "", "", "claude", "", 1n}], M.State.empty()), 50n, "m2")))), "0")
    C.Check.str("healthy running worktree has no recreation hold", waits(healthy(""), 200n), "False")
    C.Check.str("healthy running post is not held", Bool.show(B.Deliver.held(changes(post(healthy(""), 200n, "healthy")))), "False")
    C.Check.str("healthy running post delivers the current text now", send_texts(outs(post(healthy(""), 200n, "healthy"))), "more")
    C.Check.str("healthy running post leaves no queue", M.Queue.of(after(healthy(""), post(healthy(""), 200n, "healthy")), "t1"), "")
    C.Check.str("healthy path ignores even a fresh leftover ask", waits(healthy("190"), 200n), "False")
    C.Check.str("healthy path plus fresh ask still never holds input", Bool.show(B.Deliver.held(changes(post(healthy("190"), 200n, "fresh")))), "False")
    C.Check.str("healthy path plus fresh ask delivers once now", Nat.show(send_count(outs(post(healthy("190"), 200n, "fresh")))), "1")
    C.Check.str("healthy path plus fresh ask starts no recreation", Nat.show(agains(outs(post(healthy("190"), 200n, "fresh")))), "0")
    C.Check.str("exceptional absent path has the old running marker", Bool.show(B.Hub.running(B.Hub.thread(fresh_missing(), "t1"))), "True")
    C.Check.str("old activity guard expired in exceptional case", Bool.show(M.Worktree.pending(B.Hub.thread(fresh_missing(), "t1"), 200n)), "False")
    C.Check.str("exceptional absent path has a genuine fresh recreation", waits(fresh_missing(), 200n), "True")
    C.Check.str("exceptional post queues only while recreation is fresh", Bool.show(B.Deliver.held(changes(post(fresh_missing(), 200n, "exceptional")))), "True")
    C.Check.str("exceptional queued input launches no child", Nat.show(List.length(&2, B.Out, outs(post(fresh_missing(), 200n, "exceptional")))), "0")
    C.Check.str("ordered recreation queue retains both inputs", M.Queue.of(ordered_waiting(), "t1"), "first queued input\n\nsecond queued input")
    C.Check.str("readiness drains automatically in original order", send_texts(outs(ordered_ready(ordered_waiting(), 25n))), "first queued input\n\nsecond queued input")
    C.Check.str("readiness emits exactly one provider input", Nat.show(send_count(outs(ordered_ready(ordered_waiting(), 25n)))), "1")
    C.Check.str("readiness atomically clears the entire queue", M.Queue.of(after(ordered_waiting(), ordered_ready(ordered_waiting(), 25n)), "t1"), "")
    C.Check.str("repeated readiness emits no second provider input", Nat.show(send_count(outs(ordered_ready(after(ordered_waiting(), ordered_ready(ordered_waiting(), 25n)), 26n)))), "0")
    C.Check.str("repeated readiness cannot take the queue twice", kinds(changes(ordered_ready(after(ordered_waiting(), ordered_ready(ordered_waiting(), 25n)), 26n))), "worktree")
    C.Check.str("expired recreation ask no longer holds", waits(fresh_missing(), 310n), "False")
    C.Check.str("answered recreation ask no longer holds", waits(M.State.replay([M.SettingSet{"wt.again.t1", "", 201n}], fresh_missing()), 202n), "False")
    C.Check.str("readiness during old running marker preserves queued input", M.Queue.of(exceptional_ready(), "t1"), "more")
    C.Check.str("readiness during old running marker starts no concurrent input", Nat.show(send_count(outs(ordered_ready(exceptional_waiting(), 201n)))), "0")
    C.Check.str("completion automatically drains exceptional input without retry", send_texts(outs(exceptional_done())), "more")
    C.Check.str("completion clears exceptional queue atomically", M.Queue.of(after(exceptional_ready(), exceptional_done()), "t1"), "")
    C.Check.str("duplicate completion cannot redeliver exceptional input", Nat.show(send_count(outs(B.Queue.drain(after(exceptional_ready(), exceptional_done()), 203n, "t1", B.Step{[M.TurnChanged{"t1", M.Completed{}, 203n}], Nil{}})))), "0")
