import Base
import ./check.bend as C
import ../src/core/model.bend as M
import ../src/core/codec.bend as K
import ../src/core/json.bend as J
import ../src/core/far.bend as FR
import ../src/core/hub.bend as H
import ../src/core/mcp.bend as MC
import ../src/core/fmcp.bend as F
import ../src/core/client.bend as U
import ../src/core/subs.bend as SB
import ../src/core/notice.bend as NO

# Launched threads report back (hub.bend's Launch.tell): t0 launches t1
# with thread_launch, t1's turn ends, asks or fails, and t0 hears it as a
# message, once, through its inbox.

def tags(cs: List<&2, M.Change>) -> String:
  match cs:
    case Nil{}:
      ""
    case c <> t:
      J.Json.get_str(K.Change.encode(c), "$", "") ++ " " ++ tags(t)

def cs(st: H.Step) -> List<&2, M.Change>:
  H.Queue.changes(st)

def res.cs(r: MC.Res) -> List<&2, M.Change>:
  match r:
    case MC.Res{_, _, H.Step{c, _}}:
      c

def res.body(r: MC.Res) -> String:
  match r:
    case MC.Res{_, b, _}:
      J.Json.show(b)

# the text t0 is sent by a step about t1 (the first message to t0)
def sent.k(xs: List<&2, M.Change>) -> String:
  match xs:
    case Nil{}:
      ""
    case M.MessagePosted{+t, _, _, +text, _} <> rest:
      Bool.pick(String, String.eq(t, "t0"), text, sent.k(rest))
    case M.TurnQueued{+t, _, +text, _} <> rest:
      Bool.pick(String, String.eq(t, "t0"), text, sent.k(rest))
    case _ <> rest:
      sent.k(rest)

def sent.to.k(xs: List<&2, M.Change>, +th: String) -> String:
  match xs:
    case Nil{}:
      ""
    case M.MessagePosted{+t, _, _, +text, _} <> rest:
      Bool.pick(String, String.eq(t, th), text, sent.to.k(rest, th))
    case _ <> rest:
      sent.to.k(rest, th)

def sent.to(xs: List<&2, M.Change>, +th: String) -> String:
  sent.to.k(xs, th)

def told(+s: M.State, xs: List<&2, M.Change>) -> String:
  sent.k(cs(H.Launch.tell(s, 9n, "t1", H.Step{xs, Nil{}})))

def words(rs: List<&2, SB.SubRow>) -> String:
  match rs:
    case Nil{}:
      ""
    case r <> t:
      SB.Subs.word(r) ++ ";" ++ words(t)

def base() -> M.State:
  M.State.replay([M.ProjectCreated{"p1", "p", "/p", 1n}, M.ThreadCreated{"t0", "p1", "boss", M.Local{}, "", "", "claude", "", 1n}], M.State.empty())

def launch(+s: M.State, +args: String) -> MC.Res:
  MC.Mcp.launch(s, 5n, "t0", Maybe.default(&2, J.Json, J.Json.parse(args), J.Null{}))

# a bot (its thread "tb", no project) and two project threads
def bots() -> M.State:
  M.State.replay([M.ProjectCreated{"p1", "p", "/p", 1n}, M.ThreadCreated{"tb", "", "Bub", M.Local{}, "", "", "claude", "", 1n},
    M.BotSet{"b1", "Bub", "tb", 0n, "", 1n}, M.ThreadCreated{"tx", "p1", "other", M.Local{}, "", "", "claude", "", 1n}], M.State.empty())

def tool.pos(+tool: String) -> Nat:
  K.Codec.pos(MC.Mcp.names(), tool)

def call(+s: M.State, +thread: String, +tool: String, +args: String) -> MC.Res:
  MC.Mcp.call.k(tool.pos(tool), s, 20n, thread, J.Null{}, Maybe.default(&2, J.Json, J.Json.parse(args), J.Null{}))

def ok(r: MC.Res) -> String:
  match r:
    case MC.Res{o, _, _}:
      Bool.show(o)

def long() -> String:
  String.repeat("x", 3000n)

def main() -> IO(Unit):
  do IO<Unit>:
    +s : M.State = base()
    +r : MC.Res = launch(s, "{\"prompt\":\"route the clock\",\"title\":\"clock\"}")
    C.Check.str("launch records the launcher", tags(res.cs(r)), "ThreadCreated ThreadLaunched MessagePosted TurnQueued ")
    +s1 : M.State = M.State.replay(res.cs(r), s)
    C.Check.str("the launched thread knows its launcher", M.Launch.by(s1, "t5-2"), "t0")
    C.Check.str("it reports by default", Bool.show(M.Launch.reports(s1, "t5-2")), "True")
    C.Check.str("the launcher lists it", String.join(M.Launch.of(s1, "t0"), ","), "t5-2")
    C.Check.str("the answer says reports come", Bool.show(String.contains(res.body(r), "Its reports will come to you")), "True")
    +q : MC.Res = launch(s, "{\"prompt\":\"route the clock\",\"report\":false}")
    C.Check.str("report false launches it quiet", Bool.show(M.Launch.reports(M.State.replay(res.cs(q), s), "t5-2")), "False")
    # thread_update turns them off and on
    +off : MC.Res = MC.Mcp.update.r(2n, s1, 6n, "t0", "t5-2", "")
    C.Check.str("thread_update report false", Bool.show(M.Launch.reports(M.State.replay(res.cs(off), s1), "t5-2")), "False")
    C.Check.str("the launcher stays", M.Launch.by(M.State.replay(res.cs(off), s1), "t5-2"), "t0")
    C.Check.str("and on again", Bool.show(M.Launch.reports(M.State.replay(res.cs(MC.Mcp.update.r(1n, M.State.replay(res.cs(off), s1), 7n, "t0", "t5-2", "")), M.State.replay(res.cs(off), s1)), "t5-2")), "True")
    C.Check.str("a thread nobody launched has no reports to change", Bool.show(String.contains(res.body(MC.Mcp.update.r(2n, s1, 6n, "t0", "t0", "")), "nobody launched")), "True")
    C.Check.str("a title still renames", tags(res.cs(MC.Mcp.update.r(2n, s1, 6n, "t0", "t5-2", "New"))), "ThreadRenamed ")
    # the event round-trips through the log
    C.Check.str("ThreadLaunched decodes", Bool.show(Maybe.is_some(&2, M.Change, K.Change.decode(K.Change.encode(M.ThreadLaunched{"t1", "a~t0", False{}, 3n})))), "True")
    C.Check.str("an old ThreadLaunched without report reports", J.Json.show(K.Change.encode(Maybe.default(&2, M.Change, K.Change.decode(Maybe.default(&2, J.Json, J.Json.parse("{\"$\":\"ThreadLaunched\",\"id\":\"t1\",\"by\":\"t0\",\"at\":3}"), J.Null{})), M.TurnChanged{"x", M.Idle{}, 0n}))),
      "{\"$\":\"ThreadLaunched\",\"id\":\"t1\",\"by\":\"t0\",\"report\":true,\"at\":3}")
    # on another machine's mirror the launcher is its own thread again
    C.Check.str("a mirror names the launcher", J.Json.show(K.Change.encode(Maybe.default(&2, M.Change, FR.Fr.change("L", M.ThreadLaunched{"t1", "L~t0", True{}, 3n}), M.TurnChanged{"x", M.Idle{}, 0n}))),
      "{\"$\":\"ThreadLaunched\",\"id\":\"L~t1\",\"by\":\"t0\",\"report\":true,\"at\":3}")
    # the reports
    +d : M.State = H.Lr.demo(True{})
    C.Check.str("a turn end", told(d, H.Lr.demo.end()), "[Backplane] Launched thread \"work\" (t1) completed:\n\ndone it")
    C.Check.str("a failure", told(d, [M.TurnChanged{"t1", M.Failed{}, 9n}]), "[Backplane] Launched thread \"work\" (t1) failed:\n\ndone it")
    C.Check.str("an ask", told(d, [M.AskOpened{"q1", "t1", "question", "AskUserQuestion", "which board?", "", 9n}]),
      "[Backplane] Launched thread \"work\" (t1) needs the person: question (AskUserQuestion): which board?\nIt waits until they answer in that thread.")
    C.Check.str("a question in the reply", told(H.Lr.demo.k(True{}, False{}, False{}, [M.MessagePosted{"t1", "a2", M.Assistant{}, "Shall I go on?", 3n}]), H.Lr.demo.end()),
      "[Backplane] Launched thread \"work\" (t1) finished its turn and waits for an answer:\n\nShall I go on?")
    C.Check.str("a long reply is kept whole", Bool.show(String.contains(told(H.Lr.demo.k(True{}, False{}, False{}, [M.MessagePosted{"t1", "a2", M.Assistant{}, long(), 3n}]), H.Lr.demo.end()), long())), "True")
    C.Check.str("a wake on a thread carries a long reply whole", Bool.show(String.contains(H.Wake.thread.text(H.Lr.demo.k(True{}, False{}, False{}, [M.MessagePosted{"t1", "a2", M.Assistant{}, long(), 3n}]), "t1", H.Lr.demo.end()), long())), "True")
    C.Check.str("not cut: no thread_read hint", Bool.show(String.contains(told(H.Lr.demo.k(True{}, False{}, False{}, [M.MessagePosted{"t1", "a2", M.Assistant{}, long(), 3n}]), H.Lr.demo.end()), "cut short")), "False")
    C.Check.str("the report starts the launcher's turn", tags(cs(H.Launch.tell(d, 9n, "t1", H.Step{H.Lr.demo.end(), Nil{}}))), "TurnChanged MessagePosted MessageFrom TurnChanged ")
    C.Check.str("its id names the thread", Bool.show(String.contains(J.Json.show(K.Change.encode(Maybe.default(&2, M.Change, List.head(&2, M.Change, List.drop(&2, M.Change, cs(H.Launch.tell(d, 9n, "t1", H.Step{H.Lr.demo.end(), Nil{}})), 1n)), M.TurnChanged{"x", M.Idle{}, 0n}))), "t:t1:lr-")), "True")
    C.Check.str("a busy launcher hears it at once", tags(cs(H.Launch.tell(H.Lr.demo.k(True{}, False{}, True{}, Nil{}), 9n, "t1", H.Step{H.Lr.demo.end(), Nil{}}))), "TurnChanged MessagePosted MessageFrom TurnChanged ")
    C.Check.str("muted: nothing", told(H.Lr.demo(False{}), H.Lr.demo.end()), "")
    C.Check.str("work out: nothing yet", told(H.Lr.demo.k(True{}, False{}, False{}, [M.WorkSet{"t1", "bg", "a\tx", 2n}]), H.Lr.demo.end()), "")
    C.Check.str("work out, failed: told", Bool.show(String.is_empty(told(H.Lr.demo.k(True{}, False{}, False{}, [M.WorkSet{"t1", "bg", "a\tx", 2n}]), [M.TurnChanged{"t1", M.Failed{}, 9n}]))), "False")
    C.Check.str("already said: nothing", told(H.Lr.demo.k(True{}, False{}, False{}, [M.MessagePosted{"t0", M.Link.id("t1", "m5"), M.User{}, "here", 2n}]), H.Lr.demo.end()), "")
    C.Check.str("an earlier report is not its words", Bool.show(String.is_empty(told(H.Lr.demo.k(True{}, False{}, False{}, [M.MessagePosted{"t0", H.Lr.msg("t1", "r1"), M.User{}, "[Backplane] ...", 2n}]), H.Lr.demo.end()))), "False")
    C.Check.str("a step that ends nothing", told(d, [M.MessagePosted{"t1", "a2", M.Assistant{}, "more", 9n}]), "")
    C.Check.str("the person's stop is told", Bool.show(String.contains(sent.k(cs(H.Retire.stop(d, 9n, "t1", Nil{}))), "was interrupted")), "True")
    C.Check.str("the launcher's own stop is not", sent.k(cs(H.Retire.stop.f(d, 9n, "t1", "t0", Nil{}))), "")
    C.Check.str("an agent exit is told", Bool.show(String.contains(sent.k(cs(H.Hub.agent_gone(d, 9n, "t1", 1))), "Launched thread")), "True")
    C.Check.str("in order behind a busy launcher", Nat.show(H.Lr.demo.twice()), "2")
    C.Check.str("the loop's bound stops them", Bool.show(H.Lr.demo.loop.on()), "False")
    C.Check.str("the last note says so", Bool.show(String.contains(sent.k(cs(H.Launch.tell(H.Lr.demo.loop(), 9n, "t1", H.Step{H.Lr.demo.end(), Nil{}}))), "Reports from this thread stop here")), "True")
    C.Check.str("the person's message resets the count", Bool.show(String.contains(sent.k(cs(H.Launch.tell(H.Lr.demo.k(True{}, False{}, False{}, [M.SettingSet{H.Lr.n.key("t1"), "19", 1n}]), 9n, "t1", H.Step{H.Lr.demo.end(), Nil{}}))), "stop here")), "False")
    # a launcher elsewhere: the outbox
    +fd : M.State = H.Lr.demo.k(True{}, False{}, False{}, [M.ThreadLaunched{"t1", "L~t9", True{}, 2n}])
    +fs : H.Step = H.Launch.tell(fd, 9n, "t1", H.Step{H.Lr.demo.end(), Nil{}})
    C.Check.str("far: into the outbox", tags(cs(fs)), "TurnChanged SettingSet ")
    C.Check.str("far: the outbox line", M.Setting.get(M.State.replay(cs(fs), fd), H.Lr.far.key(), ""), "r9-9\tL~t9\tt1\t\"[Backplane] Launched thread \\\"work\\\" (t1) completed:\\n\\ndone it\"")
    C.Check.str("far: sent", Bool.show(String.is_empty(M.Setting.get(M.State.replay(cs(H.Lr.far.sent(M.State.replay(cs(fs), fd), 10n, "r9-9")), M.State.replay(cs(fs), fd)), H.Lr.far.key(), ""))), "True")
    C.Check.str("far: the tick sends again", Nat.show(List.length(&2, H.Out, H.Step.outs_of(H.Lr.far.tick(M.State.replay(cs(fs), fd))))), "1")
    C.Check.str("far: taken once", Nat.show(F.Fm.report.twice()), "1")
    C.Check.str("far: the request", F.Fm.request("L", "launch_report", FR.Fr.id("L", "t1"), J.Json.obj([J.KV{"threadId", J.Str{"L~t9"}}, J.KV{"key", J.Str{"k"}}, J.KV{"text", J.Str{"x"}}])),
      "{\"id\":0,\"m\":\"mcp.call\",\"p\":{\"tool\":\"launch_report\",\"from\":\"L~t1\",\"args\":{\"threadId\":\"t9\",\"key\":\"k\",\"text\":\"x\"}}}")
    C.Check.str("far: a remote call", Bool.show(F.Fm.remote("launch_report")), "True")
    C.Check.str("far: a local tool stays here", Bool.show(String.eq(F.Fm.plan.name(F.Fm.plan.o(Maybe.default(&2, J.Json, J.Json.parse("{\"method\":\"tools/call\",\"params\":{\"name\":\"delegate_task\",\"arguments\":{\"project\":\"x~p\"}}}"), J.Null{}), d, Nil{}, "t0")), "here")), "True")
    # a bot launches a project thread: it reports to the bot's thread,
    # which belongs to no project
    +b : M.State = bots()
    +br : MC.Res = MC.Mcp.launch(b, 5n, "tb", Maybe.default(&2, J.Json, J.Json.parse("{\"project\":\"p1\",\"prompt\":\"check the LDO\",\"title\":\"ldo\"}"), J.Null{}))
    +b1 : M.State = M.State.replay(List.concat(&2, M.Change, [res.cs(br), [M.WorktreeSkipped{"t5-4", 6n}, M.QueueTaken{"t5-4", 6n}, M.TurnChanged{"t5-4", M.Running{}, 6n}, M.MessagePosted{"t5-4", "a9", M.Assistant{}, "LDO is fine", 7n}]]), b)
    C.Check.str("bot: the child's launcher is the bot's thread", M.Launch.by(b1, "t5-4"), "tb")
    +bs : H.Step = H.Launch.tell(b1, 9n, "t5-4", H.Step{[M.TurnChanged{"t5-4", M.Completed{}, 9n}], Nil{}})
    C.Check.str("bot: the report reaches the bot's thread", tags(cs(bs)), "TurnChanged SettingSet MessagePosted MessageFrom TurnChanged ")
    C.Check.str("bot: in the bot's thread", sent.to(cs(bs), "tb"), "[Backplane] Launched thread \"ldo\" (t5-4) completed:\n\nLDO is fine")
    C.Check.str("bot: the link row names the bot", U.Tasks.up.label(b1, "t5-4", "tb"), "← launched by Bub")
    C.Check.str("bot: the child may thread_send the bot", ok(call(b1, "t5-4", "thread_send", "{\"threadId\":\"tb\",\"text\":\"done\"}")), "True")
    C.Check.str("bot: another project thread may not", ok(call(b1, "tx", "thread_send", "{\"threadId\":\"tb\",\"text\":\"hi\"}")), "False")
    +ub : M.State = M.State.replay([M.UberSet{"b1", "u1", 8n}], b1)
    C.Check.str("uber: any thread may thread_send the uberbot", ok(call(ub, "tx", "thread_send", "{\"threadId\":\"tb\",\"text\":\"hi\"}")), "True")
    C.Check.str("uber: across machines too", Bool.show(H.Lr.may(ub, "L~t7", "tb")), "True")
    C.Check.str("a launcher may thread_send its launched thread", Bool.show(H.Lr.may(b1, "tb", "t5-4")), "True")
    # the clients
    C.Check.str("links back", U.Tasks.up(d, "t1"), "t0")
    C.Check.str("its words", U.Tasks.up.label(d, "t1", "t0"), "← launched by boss")
    C.Check.str("the launcher lists it", words(SB.Subs.launched(d, "t0")), "running;")
    C.Check.str("no Stop on a launched row", SB.Subs.cancel(Maybe.default(&2, SB.SubRow, List.head(&2, SB.SubRow, SB.Subs.launched(d, "t0")), SB.SubRow{"", True{}, "", "", "", "", False{}, False{}, 0n, Nil{}})), "")
    C.Check.str("a report is chatter for a bot", Bool.show(NO.Turn.msg(NO.Turn{False{}, "[Backplane] Launched thread \"work\" (t1) completed.", 0n})), "True")
