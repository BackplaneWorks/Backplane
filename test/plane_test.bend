import Base
import ../src/core/i18n.bend as I
import ./check.bend as C
import ../src/core/model.bend as M
import ../src/core/client.bend as U
import ../src/core/far.bend as FR
import ../src/core/botui.bend as BU
import ../src/core/plane.bend as PL
import ../src/core/ubersync.bend as UBS
import ../src/core/ubop.bend as UO
import ../src/core/json.bend as J
import ../src/mobile/plane.bend as MP

# The status plane: its sections, their order, the counts line and the
# rooms' unread (laws plane_*).

def now() -> Nat:
  200000n

# threads in every state, two bots (one working), a room with news, two
# routines and a thread long idle
def demo() -> List<&2, M.Change>:
  [M.ProjectCreated{"p1", "One", "/a", 1n},
    M.ThreadCreated{"t1", "p1", "A", M.Local{}, "", "", "claude", "", 199000n}, M.TurnChanged{"t1", M.Running{}, 199001n}, M.AskOpened{"a1", "t1", "approval", "Bash", "ls", "", 199002n},
    M.ThreadCreated{"t2", "p1", "B", M.Local{}, "", "", "claude", "", 199100n}, M.AskOpened{"a2", "t2", "input", "", "?", "", 199101n},
    M.ThreadCreated{"t3", "p1", "C", M.Local{}, "", "", "claude", "", 199200n}, M.TurnChanged{"t3", M.Failed{}, 199201n},
    M.ThreadCreated{"t4", "p1", "D", M.Local{}, "", "", "claude", "", 199300n}, M.TurnChanged{"t4", M.Running{}, 199301n},
    M.ThreadCreated{"t5", "p1", "E", M.Local{}, "", "", "claude", "", 199400n}, M.TurnQueued{"t5", "q1", "next", 199401n},
    M.ThreadCreated{"t6", "p1", "F", M.Local{}, "", "", "claude", "", 199500n}, M.TurnChanged{"t6", M.Completed{}, 199501n},
    M.ThreadCreated{"t7", "p1", "G", M.Local{}, "", "", "claude", "", 100n},
    M.ThreadCreated{"bt1", "", "Kit", M.Local{}, "", "", "claude", "", 100n}, M.BotSet{"b1", "Kit", "bt1", 7n, "", 100n}, M.TurnChanged{"bt1", M.Running{}, 199900n},
    M.ThreadCreated{"bt2", "", "Pip", M.Local{}, "", "", "claude", "", 101n}, M.BotSet{"b2", "Pip", "bt2", 8n, "", 101n},
    M.RoomSet{"r1", "ops", "b1\nb2", 150n}, M.RoomPosted{"m1", "r1", "b1", "hello", 0n, 199800n}, M.RoomPosted{"m2", "r1", "you", "hi", 0n, 199801n},
    M.RoutineSet{"rt1", "b1", "Digest", "0 9 * * *", "sum up", True{}, 160n}, M.RoutineFired{"rt1", 199850n},
    M.RoutineSet{"rt2", "b2", "Poll", "*/15 * * * *", "poll", True{}, 161n}]

def ui(+extra: List<&2, M.Change>) -> U.Ui:
  U.Ui.far.st(U.Ui.init(now()), M.State.replay(List.append(&2, M.Change, demo(), extra), M.State.empty()))

# a thread on the owner's machine "desk" (link L), at work
def far(+away: Bool) -> U.Ui:
  U.Ui.info_set(U.Ui.info_set(U.Ui.far.st(ui([M.ProjectCreated{"L~p1", "Board", "/b", 2n}, M.ThreadCreated{"L~t1", "L~p1", "Far", M.Local{}, "", "", "claude", "", 199600n}, M.TurnChanged{"L~t1", M.Running{}, 199601n}]), FR.Fr.mark(U.Ui.state(ui([M.ProjectCreated{"L~p1", "Board", "/b", 2n}, M.ThreadCreated{"L~t1", "L~p1", "Far", M.Local{}, "", "", "claude", "", 199600n}, M.TurnChanged{"L~t1", M.Running{}, 199601n}])), "L", "desk", away)),
    "far.away.L", Bool.pick(String, away, "1", "")), "far.name.L", "desk")

def machines(rs: List<&2, PL.PMach>) -> String:
  match rs:
    case Nil{}:
      ""
    case PL.PMach{n, s} <> t:
      n ++ ":" ++ s ++ "," ++ machines(t)
    case Nil{}:
      ""

def row(m: Maybe<&2, BU.BRow>) -> String:
  match m:
    case Some{BU.BRow{_, n, _, _, _, _, _, _, _, _}}:
      n
    case None{}:
      "none"

def subs(rs: List<&2, PL.PRow>) -> String:
  match rs:
    case Nil{}:
      ""
    case PL.PRow{_, _, _, _, _, s, _, _, _, _} <> t:
      s ++ "," ++ subs(t)

def ids(rs: List<&2, PL.PRow>) -> String:
  match rs:
    case Nil{}:
      ""
    case r <> t:
      PL.PRow.id(r) ++ "," ++ ids(t)

def states(rs: List<&2, PL.PRow>) -> String:
  match rs:
    case Nil{}:
      ""
    case r <> t:
      PL.PRow.state(r) ++ "," ++ states(t)

def cmds(a: U.Act) -> List<&2, U.Cmd>:
  match a:
    case U.Act{_, cs}:
      cs

# the text of every request a step sent
def sends.of(cs: List<&2, U.Cmd>) -> String:
  match cs:
    case U.Send{t} <> r:
      t ++ "|" ++ sends.of(r)
    case _ <> r:
      sends.of(r)
    case Nil{}:
      ""

def main() -> IO(Unit):
  do IO<Unit>:
    +p : PL.Plane = PL.Plane.of(ui(Nil{}), now())
    C.Check.str("need", ids(PL.Plane.need(p)), "t1,t2,t3,")
    C.Check.str("need states", states(PL.Plane.need(p)), "approval,question,failed,")
    C.Check.str("a question the hub counts as pending is still a question", states(PL.Plane.need(PL.Plane.of(ui([M.PendingChanged{"t2", 1n, 199102n}]), now()))), "approval,question,failed,")
    C.Check.str("run, newest first", ids(PL.Plane.run(p)), "b1,t4,")
    C.Check.str("run states", states(PL.Plane.run(p)), "working,working,")
    C.Check.str("idle: the last day only", ids(PL.Plane.idle(p)), "t6,t5,")
    C.Check.str("idle count", Nat.show(PL.Plane.idle_n(p)), "2")
    C.Check.str("bots", ids(PL.Plane.bots(p)), "b1,b2,")
    C.Check.str("rooms with news", ids(PL.Plane.rooms(p)), "r1,")
    C.Check.str("a running routine first", ids(PL.Plane.routines(p)), "rt1,rt2,")
    C.Check.str("routine states", states(PL.Plane.routines(p)), "working,idle,")
    C.Check.str("counts", PL.Plane.counts(I.En{}, p), "3 need you · 2 running")
    # routines-cal: once, reminders and done routines; the next time is in the person's zone
    +pr : PL.Plane = PL.Plane.of(ui([M.SettingSet{"person.tz", "UTC", 1n}, M.RoutineSet{"rt3", "b2", "Stand", "", "stand up", True{}, 199000n}, M.RoutinePlan{"rt3", 203600n, "person", "remind", 199000n},
      M.RoutineSet{"rt4", "b2", "Old", "", "x", True{}, 199000n}, M.RoutinePlan{"rt4", 199500n, "person", "", 199000n}, M.RoutineFired{"rt4", 199500n}, M.RoutineDone{"rt4", 199500n}]), now())
    C.Check.str("a reminder is a routine row in the reminder state, a done one is gone", ids(PL.Plane.routines(pr)), "rt1,rt3,rt2,")
    C.Check.str("states with a reminder", states(PL.Plane.routines(pr)), "working,reminder,idle,")
    C.Check.str("a reminder row says when, in your time", subs(PL.Plane.routines(pr)), "daily at 09:00 · Sat 3 Jan 09:00 UTC,Sat 3 Jan 08:33 UTC · in 1h,every 15 minutes · Sat 3 Jan 07:45 UTC,")
    C.Check.str("the count without rows", PL.Plane.tally.words(ui(Nil{}), now()), "3 need you · 2 running")
    C.Check.str("every way to be need or run", PL.Plane.tally.words(PL.Plane.demo.rich(), 200000n), PL.Plane.counts(I.En{}, PL.Plane.of(PL.Plane.demo.rich(), 200000n)))
    C.Check.str("... is what it is", PL.Plane.counts(I.En{}, PL.Plane.of(PL.Plane.demo.rich(), 200000n)), "5 need you · 5 running")
    C.Check.str("a thread elsewhere counts", PL.Plane.tally.words(far(False{}), now()), "3 need you · 3 running")
    C.Check.str("the plane as text, need first", PL.Plane.text(p, "", "this machine", now(), False{}),
      "Status plane of this machine\nNeeds you (3)\n- t1 · approval · A · One · 16m ago\n- t2 · question · B · One · 15m ago\n- t3 · failed · C · One · 13m ago\nRunning (2)\n- b1 · working · Kit · 1m ago\n- t4 · working · D · One · 11m ago\nBots (2)\n- b1 · work · Kit · working · 1m ago\n- b2 · sleep · Pip · 2d ago\nRooms with unread posts (1)\n- r1 · unread · ops · 1 unread · 3m ago\nRoutines (2)\n- rt1 · working · Digest · daily at 09:00 · Sat 3 Jan 09:00 UTC · next in 1h\n- rt2 · idle · Poll · every 15 minutes · Sat 3 Jan 07:45 UTC · next in 11m\nIdle in the last day: 2 (plane_read with idle: true lists them).\n")
    C.Check.str("need before run in the text", Bool.show(PL.Plane.text.need_first(p, now())), "True")
    C.Check.str("ids of another machine carry its link", Bool.show(String.contains(PL.Plane.text(p, "L", "desk", now(), True{}), "- L~t1 ")) ++ Bool.show(String.contains(PL.Plane.text(p, "L", "desk", now(), True{}), "- L~t6 ")), "TrueTrue")
    C.Check.str("a hub reads its own plane", PL.Plane.read(M.State.replay(demo(), M.State.empty()), now(), "", "x", False{}), PL.Plane.text(PL.Plane.of(U.Ui.far.st(U.Ui.init(now()), M.State.replay(demo(), M.State.empty())), now()), "", "x", now(), False{}))
    C.Check.str("home: no uberbot, no line", PL.Plane.home.words(ui(Nil{})), "")
    C.Check.str("home: the lease is held elsewhere, the holder unnamed", PL.Plane.home.words(U.Ui.far.st(U.Ui.init(now()), UBS.Ub.demo.away())), "Home: another machine | Make this the home")
    C.Check.str("home: named by its link", PL.Plane.home.words(U.Ui.info_set(U.Ui.far.st(U.Ui.init(now()), M.State.replay([M.SettingSet{"uber.link.u1-x.mB", "L", 5n}], UBS.Ub.demo.away())), "far.name.L", "desk")), "Home: desk | Make this the home")
    C.Check.str("home: here, no offer", PL.Plane.home.words(U.Ui.far.st(U.Ui.init(now()), M.State.replay([UBS.Ub.demo.op("mA", 1n, 1002n, UO.Ub.b.lease("mA", 3n))], UBS.Ub.demo.away()))), "Home: This machine")
    C.Check.str("home: no lease yet, here", PL.Plane.home.words(U.Ui.far.st(U.Ui.init(now()), UBS.Ub.demo.a())), "Home: This machine")
    C.Check.str("uber-home asks the hub", Bool.show(String.contains(sends.of(cmds(U.Ui.act(ui(Nil{}), "uber-home", ""))), "bots.uber_home")), "True")
    C.Check.str("the list's plane has the plane's need and run rows", Bool.show(PL.Plane.list.same(ui(Nil{}), now())) ++ Bool.show(PL.Plane.list.same(PL.Plane.demo.rich(), now())), "TrueTrue")
    C.Check.str("nothing to count", PL.Plane.counts(I.En{}, PL.Plane.of(U.Ui.far.st(U.Ui.init(now()), M.State.empty()), now())), "")
    C.Check.str("unread", Nat.show(U.Rooms.unread(ui(Nil{}), "r1")), "1")
    C.Check.str("seen", Nat.show(U.Rooms.unread(ui([M.SettingSet{"room.seen.r1", "199800", 199900n}]), "r1")), "0")
    C.Check.str("no room news once seen", ids(PL.Plane.rooms(PL.Plane.of(ui([M.SettingSet{"room.seen.r1", "199800", 199900n}]), now()))), "")
    C.Check.str("a thread elsewhere runs", ids(PL.Plane.run(PL.Plane.of(far(False{}), now()))), "b1,L~t1,t4,")
    C.Check.str("with its machine", PL.Plane.machine.of(PL.Plane.run(PL.Plane.of(far(False{}), now()))), "desk")
    C.Check.str("away it asks for nothing", ids(PL.Plane.idle(PL.Plane.of(far(True{}), now()))), "L~t1,t6,t5,")
    C.Check.str("machines", machines(PL.Plane.machines(PL.Plane.of(ui([M.PeerSet{"L", "desk", "http://desk:3787", False{}, 5n}]), now()))), "This machine:here,desk:syncing,")
    C.Check.str("the uberbot is not among the bots", ids(PL.Plane.bots(PL.Plane.of(ui([M.UberSet{"b2", "u1-a", 5n}]), now()))), "b1,")
    C.Check.str("it has its own row", row(BU.BU.uber(ui([M.UberSet{"b2", "u1-a", 5n}]))), "Pip")
    C.Check.str("without the keyed pass, the same plane", J.Json.show(MP.MPlane.json(PL.Plane.of.f(far(False{}), U.Side.ix(U.Ui.state(far(False{}))), now(), True{}), now(), U.Ui.state(far(False{})))), J.Json.show(MP.MPlane.json(PL.Plane.of(far(False{}), now()), now(), U.Ui.state(far(False{})))))
    C.Check.str("a thread opens by select", PL.Plane.act.of(PL.Plane.need(p)), "select t1")
    C.Check.str("the key changes with the minute", Bool.pick(String, String.eq(PL.Plane.key(ui(Nil{}), now()), PL.Plane.key(ui(Nil{}), 200090n)), "same", "differs"), "differs")
    C.Check.str("and with the log", Bool.pick(String, String.eq(PL.Plane.key(ui(Nil{}), now()), PL.Plane.key(ui([M.SettingSet{"x", "y", 3n}]), now())), "same", "differs"), "differs")
