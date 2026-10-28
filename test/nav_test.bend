import Base
import ./check.bend as C
import ../src/core/model.bend as M
import ../src/core/codec.bend as K
import ../src/core/hub.bend as H
import ../src/core/json.bend as J
import ../src/core/client.bend as U

# Sidebar and navigation: order keys, manual order, snooze shelves, the
# status dot, and the hub's reorder and snooze methods.

def ids(ts: List<&2, M.Thread>) -> String:
  String.join(U.Side.ids(ts), ",")

def shelves(s: M.Shelves) -> String:
  match s:
    case M.Shelves{p, a, st, h, sn}:
      "p:" ++ ids(p) ++ " a:" ++ ids(a) ++ " z:" ++ ids(sn) ++ " s:" ++ ids(st) ++ " h:" ++ ids(h)

def hidden(s: M.Shelves) -> List<&2, M.Thread>:
  match s:
    case M.Shelves{_, _, _, h, _}:
      h

def okeys(ks: List<&2, M.OKey>) -> String:
  match ks:
    case Nil{}:
      ""
    case M.OKey{i, k} <> t:
      i ++ "=" ++ k ++ " " ++ okeys(t)

def flag(x: Bool) -> String:
  match x:
    case True{}:
      "T"
    case False{}:
      "F"

def lt(a: String, b: String) -> String:
  flag(M.Key.lt(M.Key.read(a), M.Key.read(b)))

def mid(a: String, b: String) -> String:
  M.Key.show(M.Key.mid(M.Key.read(a), M.Key.read(b)))

def after(a: String) -> String:
  M.Key.show(M.Key.after(M.Key.read(a)))

# a project with three threads made at 100, 200, 300
def base() -> List<&2, M.Change>:
  [M.ProjectCreated{"p1", "P", "/tmp/p", 50n},
   M.ThreadCreated{"a", "p1", "A", M.Local{}, "", "", "claude", "", 100n},
   M.ThreadCreated{"b", "p1", "B", M.Local{}, "", "", "claude", "", 200n},
   M.ThreadCreated{"c", "p1", "C", M.Local{}, "", "", "claude", "", 300n}]

# base() and two projects made at 400 and 500, each with a thread (so all
# three are live, and only their order keys and activity place them)
def three() -> List<&2, M.Change>:
  List.append(&2, M.Change, base(), [M.ProjectCreated{"p2", "Q", "/tmp/q", 400n}, M.ThreadCreated{"x", "p2", "X", M.Local{}, "", "", "claude", "", 400n},
    M.ProjectCreated{"p3", "R", "/tmp/r", 500n}, M.ThreadCreated{"y", "p3", "Y", M.Local{}, "", "", "claude", "", 500n}])

def pside(es: List<&2, M.Change>) -> String:
  String.join(U.Side.projects(U.Ui.fold(es, U.Ui.init(1000n))), ",")

def side(es: List<&2, M.Change>, +now: Nat) -> String:
  +st = M.State.replay(es, M.State.empty())
  shelves(M.Sidebar.at(U.Ui.threads(st), M.State.navs(st), now))

def changes.go(cs: List<&2, M.Change>) -> List<&2, String>:
  match cs:
    case Nil{}:
      Nil{}
    case c <> t:
      J.Json.show(K.Change.encode(c)) <> changes.go(t)

def changes(st: H.Step) -> String:
  String.join(changes.go(H.Queue.changes(st)), " ")

def sent(a: U.Act) -> String:
  match a:
    case U.Act{_, U.Send{t} <> _}:
      t
    case _:
      ""

def draft(a: U.Act) -> String:
  match a:
    case U.Act{u, _}:
      U.Ui.draft_text(u)

def act_ui(a: U.Act) -> U.Ui:
  match a:
    case U.Act{u, _}:
      u

def ui() -> U.Ui:
  U.Ui.fold(base(), U.Ui.init(1000n))

def main() -> IO(Unit):
  +st = M.State.replay(base(), M.State.empty())
  do IO<Unit>:
    C.Check.str("key after empty", after(""), "8")
    C.Check.str("key after 8", after("8"), "c")
    C.Check.str("key after f", after("f"), "f8")
    C.Check.str("key mid 4 8", mid("4", "8"), "6")
    C.Check.str("key mid top", mid("", "8"), "4")
    C.Check.str("key mid deep", mid("8", "88"), "84")
    C.Check.str("key lt", lt("4", "6") ++ lt("6", "8") ++ lt("8", "80") ++ lt("80", "8") ++ lt("", "01"), "TTFFT")
    C.Check.str("key read show", M.Key.show(M.Key.read("a3f")), "a3f")
    C.Check.str("order to top", okeys(M.Order.plan([M.OKey{"c", ""}, M.OKey{"a", ""}, M.OKey{"b", ""}], "c")), "c=8 ")
    C.Check.str("order between", okeys(M.Order.plan([M.OKey{"a", "4"}, M.OKey{"b", ""}, M.OKey{"c", "8"}], "b")), "b=6 ")
    C.Check.str("order to bottom", okeys(M.Order.plan([M.OKey{"b", ""}, M.OKey{"c", ""}, M.OKey{"a", ""}], "a")), "b=8 c=c a=e ")
    C.Check.str("order keeps", okeys(M.Order.plan([M.OKey{"a", "4"}, M.OKey{"b", "8"}], "a")), "")
    C.Check.str("order swap", okeys(M.Order.plan([M.OKey{"b", "8"}, M.OKey{"a", "4"}], "b")), "b=2 ")
    C.Check.str("order move", String.join(M.Order.move(["a", "b", "c", "d"], "a", 2n), ","), "b,c,a,d")
    C.Check.str("order move up", String.join(M.Order.move(["a", "b", "c", "d"], "d", 1n), ","), "a,d,b,c")
    C.Check.str("sidebar plain", side(base(), 1000n), "p: a:c,b,a z: s: h:")
    C.Check.str("sidebar keyed first", side(List.append(&2, M.Change, base(), [M.ThreadOrdered{"a", "active", "8", 400n}]), 1000n), "p: a:a,c,b z: s: h:")
    C.Check.str("sidebar snoozed", side(List.append(&2, M.Change, base(), [M.ThreadSnoozed{"b", 5000n, 400n}, M.ThreadSnoozed{"c", 3000n, 400n}]), 1000n), "p: a:a z:c,b s: h:")
    C.Check.str("sidebar wakes", side(List.append(&2, M.Change, base(), [M.ThreadSnoozed{"b", 5000n, 400n}]), 5000n), "p: a:c,b,a z: s: h:")
    C.Check.str("sidebar unsnoozed", side(List.append(&2, M.Change, base(), [M.ThreadSnoozed{"b", 5000n, 400n}, M.ThreadSnoozed{"b", 0n, 500n}]), 1000n), "p: a:c,b,a z: s: h:")
    C.Check.str("sidebar pinned snoozed", side(List.append(&2, M.Change, base(), [M.ThreadPinned{"a", True{}, 400n}, M.ThreadSnoozed{"a", 5000n, 400n}]), 1000n), "p: a:c,b z:a s: h:")
    C.Check.str("sidebar archived", ids(M.Archive.of(hidden(M.Sidebar.at(U.Ui.threads(M.State.replay(List.append(&2, M.Change, base(), [M.ThreadArchived{"a", True{}, 400n}, M.ThreadArchived{"b", True{}, 400n}, M.ThreadDeleted{"b", 500n}]), M.State.empty())), Map.new(&2, M.Nav), 1000n)))), "a")
    C.Check.str("settle snoozed", flag(M.Settle.auto.at(900000n, 259200n, H.Hub.thread(st, "a"), 900001n)), "F")
    C.Check.str("settle woken fresh", flag(M.Settle.auto.at(900002n, 259200n, H.Hub.thread(st, "a"), 900001n)), "F")
    C.Check.str("settle woken stale", flag(M.Settle.auto.at(1200000n, 259200n, H.Hub.thread(st, "a"), 900001n)), "T")
    C.Check.str("settle never snoozed", flag(M.Settle.auto.at(900000n, 259200n, H.Hub.thread(st, "a"), 0n)), "T")
    C.Check.str("tick skips snoozed", changes(H.Hub.tick(M.State.replay(List.append(&2, M.Change, base(), [M.ThreadSnoozed{"a", 900001n, 400n}]), M.State.empty()), 900000n)),
      "{\"$\":\"ThreadSettled\",\"id\":\"b\",\"manual\":false,\"at\":900000} {\"$\":\"ThreadSettled\",\"id\":\"c\",\"manual\":false,\"at\":900000}")
    C.Check.str("tomorrow east", Nat.show(M.Snooze.tomorrow(1728043200n, "+0200")), "1728111600")
    C.Check.str("tomorrow west", Nat.show(M.Snooze.tomorrow(1728043200n, "-0530")), "1728138600")
    C.Check.str("status approval", M.Status.name(M.Status.of(True{}, True{}, M.Running{}, True{})), "approval")
    C.Check.str("status input", M.Status.name(M.Status.of(False{}, True{}, M.Running{}, True{})), "input")
    C.Check.str("status working", M.Status.name(M.Status.of(False{}, False{}, M.Running{}, True{})), "working")
    C.Check.str("status failed", M.Status.name(M.Status.of(False{}, False{}, M.Failed{}, True{})), "failed")
    C.Check.str("status starting", M.Status.name(M.Status.of(False{}, False{}, M.Completed{}, True{})), "working")
    C.Check.str("status ready", M.Status.name(M.Status.of(False{}, False{}, M.Interrupted{}, False{})), "ready")
    C.Check.str("status of thread", M.Status.name(U.Side.status(M.State.replay(List.append(&2, M.Change, base(), [M.AskOpened{"k1", "a", "input", "", "?", "", 400n}]), M.State.empty()), H.Hub.thread(st, "a"))), "input")
    C.Check.str("phase monitoring", M.Phase.label(M.Phase.of(False{}, False{}, 2n, M.Completed{}, False{})), "Monitoring 2 tasks")
    C.Check.str("phase monitoring one", M.Phase.label(M.Phase.of(False{}, False{}, 1n, M.Interrupted{}, False{})), "Monitoring 1 task")
    C.Check.str("phase complete", M.Phase.label(M.Phase.of(False{}, False{}, 0n, M.Completed{}, False{})), "Complete")
    C.Check.str("phase stopped", M.Phase.label(M.Phase.of(False{}, False{}, 0n, M.Interrupted{}, False{})), "Stopped")
    C.Check.str("phase idle", M.Phase.label(M.Phase.of(False{}, False{}, 0n, M.Idle{}, False{})), "")
    C.Check.str("phase of thread", M.Phase.name(U.Side.phase(M.State.replay(List.append(&2, M.Change, base(), [M.TaskDelegated{"t1", "a", "b", 1n, "", "", 400n}]), M.State.empty()), H.Hub.thread(st, "a"))), "monitoring")
    C.Check.str("codec snoozed", J.Json.show(K.Change.encode(Maybe.default(&2, M.Change, K.Change.decode(K.Change.encode(M.ThreadSnoozed{"a", 7n, 3n})), M.ThreadDeleted{"", 0n}))), "{\"$\":\"ThreadSnoozed\",\"id\":\"a\",\"until\":7,\"at\":3}")
    C.Check.str("codec ordered", J.Json.show(K.Change.encode(Maybe.default(&2, M.Change, K.Change.decode(K.Change.encode(M.ThreadOrdered{"a", "pinned", "8", 3n})), M.ThreadDeleted{"", 0n}))), "{\"$\":\"ThreadOrdered\",\"id\":\"a\",\"section\":\"pinned\",\"key\":\"8\",\"at\":3}")
    C.Check.str("hub snooze 1h", changes(H.Hub.request_text(st, 1000n, 1, "{\"id\":1,\"m\":\"thread.snooze\",\"p\":{\"thread\":\"a\",\"for\":\"1h\"}}")), "{\"$\":\"ThreadSnoozed\",\"id\":\"a\",\"until\":4600,\"at\":1000}")
    C.Check.str("hub unsnooze", changes(H.Hub.request_text(st, 1000n, 1, "{\"id\":1,\"m\":\"thread.snooze\",\"p\":{\"thread\":\"a\",\"for\":\"off\"}}")), "{\"$\":\"ThreadSnoozed\",\"id\":\"a\",\"until\":0,\"at\":1000}")
    # archiving wakes a snoozed thread (an archived one waits on no clock); an
    # awake one or one whose snooze has ended is archived alone
    C.Check.str("hub archive wakes snoozed", changes(H.Hub.request_text(M.State.replay(List.append(&2, M.Change, base(), [M.ThreadSnoozed{"a", 5000n, 400n}]), M.State.empty()), 1000n, 1, "{\"id\":1,\"m\":\"thread.archive\",\"p\":{\"thread\":\"a\"}}")),
      "{\"$\":\"ThreadArchived\",\"id\":\"a\",\"archived\":true,\"at\":1000} {\"$\":\"ThreadSnoozed\",\"id\":\"a\",\"until\":0,\"at\":1000} {\"$\":\"WorkSet\",\"thread\":\"a\",\"kind\":\"watch\",\"rows\":\"\",\"at\":1000}")
    C.Check.str("hub archive awake", changes(H.Hub.request_text(st, 1000n, 1, "{\"id\":1,\"m\":\"thread.archive\",\"p\":{\"thread\":\"a\"}}")), "{\"$\":\"ThreadArchived\",\"id\":\"a\",\"archived\":true,\"at\":1000} {\"$\":\"WorkSet\",\"thread\":\"a\",\"kind\":\"watch\",\"rows\":\"\",\"at\":1000}")
    C.Check.str("hub archive snooze over", changes(H.Hub.request_text(M.State.replay(List.append(&2, M.Change, base(), [M.ThreadSnoozed{"a", 900n, 400n}]), M.State.empty()), 1000n, 1, "{\"id\":1,\"m\":\"thread.archive\",\"p\":{\"thread\":\"a\"}}")), "{\"$\":\"ThreadArchived\",\"id\":\"a\",\"archived\":true,\"at\":1000} {\"$\":\"WorkSet\",\"thread\":\"a\",\"kind\":\"watch\",\"rows\":\"\",\"at\":1000}")
    C.Check.str("archived snoozed leaves its shelf", side(List.append(&2, M.Change, base(), [M.ThreadSnoozed{"a", 5000n, 400n}, M.ThreadArchived{"a", True{}, 500n}, M.ThreadSnoozed{"a", 0n, 500n}]), 1000n), "p: a:c,b z: s: h:a")
    C.Check.str("hub reorder", changes(H.Hub.request_text(st, 1000n, 1, "{\"id\":1,\"m\":\"thread.reorder\",\"p\":{\"section\":\"active\",\"thread\":\"a\",\"ids\":[\"a\",\"c\",\"b\"]}}")), "{\"$\":\"ThreadOrdered\",\"id\":\"a\",\"section\":\"active\",\"key\":\"8\",\"at\":1000}")
    C.Check.str("hub reorder settled", changes(H.Hub.request_text(st, 1000n, 1, "{\"id\":1,\"m\":\"thread.reorder\",\"p\":{\"section\":\"settled\",\"thread\":\"a\",\"ids\":[\"a\"]}}")), "")
    C.Check.str("client visible", String.join(U.Side.visible(ui()), ","), "c,b,a")
    C.Check.str("client jump", U.Side.nth(U.Side.visible(ui()), 1n), "b")
    C.Check.str("client move up", sent(U.Ui.act(ui(), "move", "b|up")), "{\"id\":0,\"m\":\"thread.reorder\",\"p\":{\"section\":\"active\",\"thread\":\"b\",\"ids\":[\"b\",\"c\",\"a\"]}}")
    C.Check.str("client move top stays", sent(U.Ui.act(ui(), "move", "c|up")), "")
    C.Check.str("client drop", sent(U.Ui.act(ui(), "reorder", "a|c")), "{\"id\":0,\"m\":\"thread.reorder\",\"p\":{\"section\":\"active\",\"thread\":\"a\",\"ids\":[\"a\",\"c\",\"b\"]}}")
    C.Check.str("projects by activity", pside(three()), "p3,p2,p1")
    C.Check.str("projects keyed first", pside(List.append(&2, M.Change, three(), [M.ProjectOrdered{"p1", "8", 600n}])), "p1,p3,p2")
    C.Check.str("projects by key", pside(List.append(&2, M.Change, three(), [M.ProjectOrdered{"p1", "8", 600n}, M.ProjectOrdered{"p2", "4", 600n}])), "p2,p1,p3")
    C.Check.str("projects key dropped", pside(List.append(&2, M.Change, three(), [M.ProjectOrdered{"p1", "8", 600n}, M.ProjectOrdered{"p1", "", 700n}])), "p3,p2,p1")
    C.Check.str("codec project ordered", J.Json.show(K.Change.encode(Maybe.default(&2, M.Change, K.Change.decode(K.Change.encode(M.ProjectOrdered{"p1", "8", 3n})), M.ThreadDeleted{"", 0n}))), "{\"$\":\"ProjectOrdered\",\"id\":\"p1\",\"key\":\"8\",\"at\":3}")
    C.Check.str("hub project reorder", changes(H.Hub.request_text(M.State.replay(three(), M.State.empty()), 1000n, 1, "{\"id\":1,\"m\":\"project.reorder\",\"p\":{\"project\":\"p1\",\"ids\":[\"p1\",\"p3\",\"p2\"]}}")), "{\"$\":\"ProjectOrdered\",\"id\":\"p1\",\"key\":\"8\",\"at\":1000}")
    C.Check.str("client project drop", sent(U.Ui.act(U.Ui.fold(three(), U.Ui.init(1000n)), "reorder-project", "p1|p3")), "{\"id\":0,\"m\":\"project.reorder\",\"p\":{\"project\":\"p1\",\"ids\":[\"p1\",\"p3\",\"p2\"]}}")
    C.Check.str("client project drop on thread", sent(U.Ui.act(U.Ui.fold(three(), U.Ui.init(1000n)), "reorder-project", "p3|b")), "{\"id\":0,\"m\":\"project.reorder\",\"p\":{\"project\":\"p3\",\"ids\":[\"p2\",\"p1\",\"p3\"]}}")
    C.Check.str("client project drop on itself", sent(U.Ui.act(U.Ui.fold(three(), U.Ui.init(1000n)), "reorder-project", "p1|a")), "")
    C.Check.str("client mention", draft(U.Ui.act(act_ui(U.Ui.act(ui(), "select", "a")), "mention", "src/x.bend")), "@src/x.bend ")
