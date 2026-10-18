import Base
import ./check.bend as C
import ../src/core/model.bend as M
import ../src/core/codec.bend as K
import ../src/core/json.bend as J
import ../src/core/client.bend as U
import ../src/mobile/hubs.bend as H
import ../src/core/ubop.bend as UO
import ../src/mobile/app.bend as A

# ios-snappy tabs: the badges and routine rows the screen carries for the tab
# bar, and the thread list's view, which the device may choose itself
# (Backplane.local) over the hub's setting (laws tabs_*, local_view_*).

def items(cs: List<&2, M.Change>) -> List<&2, J.Json>:
  match cs:
    case Nil{}:
      Nil{}
    case c <> t:
      K.Change.encode(c) <> items(t)

def log(cs: List<&2, M.Change>) -> J.Json:
  J.Json.obj([J.KV{"t", J.Json.str("log")}, J.KV{"items", J.Json.arr(items(cs))}])

def now() -> Nat:
  200000n

# box: threads in several states, a bot at work promoted to uberbot, a
# room with news, a routine
def box() -> List<&2, M.Change>:
  [M.ProjectCreated{"p1", "One", "/a", 1n},
    M.ThreadCreated{"t1", "p1", "A", M.Local{}, "", "", "claude", "", 199000n}, M.TurnChanged{"t1", M.Running{}, 199001n}, M.AskOpened{"a1", "t1", "approval", "Bash", "ls", "", 199002n},
    M.ThreadCreated{"t4", "p1", "D", M.Local{}, "", "", "claude", "", 199300n}, M.TurnChanged{"t4", M.Running{}, 199301n},
    M.ThreadCreated{"t6", "p1", "F", M.Local{}, "", "", "claude", "", 199500n}, M.TurnChanged{"t6", M.Completed{}, 199501n},
    M.ThreadCreated{"bt1", "", "Kit", M.Local{}, "", "", "claude", "", 100n}, M.BotSet{"b1", "Kit", "bt1", 7n, "", 100n}, M.TurnChanged{"bt1", M.Running{}, 199900n},
    M.RoomSet{"r1", "ops", "b1", 150n}, M.RoomPosted{"m1", "r1", "b1", "hello", 0n, 199800n},
    M.RoutineSet{"rt1", "b1", "Digest", "0 9 * * *", "sum up", True{}, 160n},
    M.UberSet{"b1", "u1-a", 170n}]

# lap: its own thread at work, one waiting on a question
def lap() -> List<&2, M.Change>:
  [M.ProjectCreated{"p9", "Lap", "/lap", 50n},
    M.ThreadCreated{"t9", "p9", "Nine", M.Local{}, "", "", "claude", "", 199700n}, M.TurnChanged{"t9", M.Running{}, 199701n},
    M.ThreadCreated{"t8", "p9", "Eight", M.Local{}, "", "", "claude", "", 199600n}, M.AskOpened{"a8", "t8", "input", "", "?", "", 199601n}]

# the uberbot's replica on another machine (mirrored here as p2~b2): the
# machine mB tied it to the uid, and an exchange came in over link p2
def replica() -> List<&2, M.Change>:
  List.append(&2, M.Change, box(), [M.UberOp{"u1-a", "mB", 1n, "0000000100.000000.mB", "{\"k\":\"bind\",\"machine\":\"mB\",\"bot\":\"b2\",\"thread\":\"t2\",\"name\":\"Kit\"}", 100n},
    M.SettingSet{"uber.link.u1-a.mB", "p2", 101n}, M.ThreadCreated{"p2~t2", "", "Kit", M.Local{}, "", "", "claude", "", 100n}, M.BotSet{"p2~b2", "Kit", "p2~t2", 7n, "", 100n}])

def one(+cs: List<&2, M.Change>) -> H.Hubs:
  H.Hubs.after(H.Hubs.recv(H.Hubs.set(H.Hubs.init(now(), "c"), ["box:3787"]), "box:3787", log(cs)))

def two() -> H.Hubs:
  H.Hubs.after(H.Hubs.recv(
    H.Hubs.after(H.Hubs.recv(H.Hubs.set(H.Hubs.init(now(), "c"), ["lap:3787", "box:3787"]), "lap:3787", log(lap()))),
    "box:3787", log(box())))


def go(+h: H.Hubs, +a: String, +v: String) -> H.Hubs:
  H.Hubs.after(H.Hubs.act(h, a, v))

def sets(+h: H.Hubs) -> String:
  Bool.pick(String, String.starts_with(J.Json.show(J.Json.get_obj(H.Hubs.screen(h), "settings")), "{\"rows\""), "open", "shut")

# the labels of the settings rows marked local
def localrows.go(js: List<&2, J.Json>) -> String:
  match js:
    case Nil{}:
      ""
    case +j <> t:
      Bool.pick(String, J.Json.get_bool(j, "local", False{}), J.Json.get_str(j, "label", "") ++ ";", "") ++ localrows.go(t)

def notifyrows.go(js: List<&2, J.Json>) -> String:
  match js:
    case Nil{}:
      ""
    case +j <> t:
      Bool.pick(String, J.Json.get_bool(j, "notify", False{}), J.Json.get_str(j, "label", "") ++ ";", "") ++ notifyrows.go(t)

def notifyrows(+h: H.Hubs) -> String:
  notifyrows.go(J.Json.elems(J.Json.get_obj(J.Json.get_obj(H.Hubs.screen(h), "settings"), "rows")))

def localrows(+h: H.Hubs) -> String:
  localrows.go(J.Json.elems(J.Json.get_obj(J.Json.get_obj(H.Hubs.screen(h), "settings"), "rows")))

def tabs(+h: H.Hubs) -> J.Json:
  J.Json.get_obj(H.Hubs.screen(h), "tabs")

def badge(+h: H.Hubs, +tab: String) -> String:
  J.Json.show(J.Json.get_obj(J.Json.get_obj(tabs(h), tab), "badge"))

def strs(js: List<&2, J.Json>, +key: String) -> String:
  match js:
    case Nil{}:
      ""
    case j <> t:
      J.Json.get_str(j, key, "") ++ ";" ++ strs(t, key)

def routines(+h: H.Hubs, +field: String) -> String:
  strs(J.Json.elems(J.Json.get_obj(J.Json.get_obj(tabs(h), "routines"), "rows")), field)

def view(+h: H.Hubs) -> String:
  J.Json.get_str(H.Hubs.screen(h), "view", "")

# the hub says "active" (its setting), or not
def hubactive() -> List<&2, M.Change>:
  List.append(&2, M.Change, box(), [M.SettingSet{"sidebar.view", "active", 175n}])

# box with a one-time reminder (remind_person) that is not due yet
def reminded() -> List<&2, M.Change>:
  List.append(&2, M.Change, box(), [M.RoutineSet{"rt2", "b1", "Dentist", "", "call", True{}, 161n}, M.RoutinePlan{"rt2", 300000n, "", "remind", 162n}])

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("home badge: what needs you", badge(one(box()), "home"), "1")
    C.Check.str("bots badge: rooms with unread", badge(one(box()), "bots"), "1")
    C.Check.str("no uberbot, home badge still counts", badge(one(lap()), "home"), "1")
    C.Check.str("no rooms, no bots badge", badge(one(lap()), "bots"), "0")
    C.Check.str("two hubs: the home badges add up", badge(two(), "home"), "2")
    C.Check.str("routines: one row", routines(one(box()), "title"), "Digest;")
    C.Check.str("routines: opens its bot's tab", routines(one(box()), "value"), "box:3787|b1|routines;")
    C.Check.str("routines: its bot", routines(one(box()), "bot"), "Kit;")
    C.Check.str("routines: when", routines(one(box()), "sub"), "daily at 09:00 · Sat 3 Jan 09:00 UTC;")
    C.Check.str("routines: the hub's id on the row", routines(two(), "id"), "box:3787|rt1;")
    C.Check.str("routines: none on a hub without", routines(one(lap()), "id"), "")
    C.Check.str("no local view: the hub's, projects", view(one(box())), "projects")
    C.Check.str("no local view: the hub's, active", view(one(hubactive())), "active")
    C.Check.str("a local view wins over the hub's (active over projects)", view(H.Hubs.local(one(box()), "view", "active")), "active")
    C.Check.str("a local view wins over the hub's (projects over active)", view(H.Hubs.local(one(hubactive()), "view", "projects")), "projects")
    C.Check.str("an empty local view leaves it to the hub", view(H.Hubs.local(H.Hubs.local(one(hubactive()), "view", "projects"), "view", "")), "active")
    C.Check.str("a local view that is neither is ignored", view(H.Hubs.local(one(hubactive()), "view", "nonsense")), "active")
    C.Check.str("every hub takes the local view", view(H.Hubs.local(two(), "view", "active")), "active")
    C.Check.str("the local view survives a state loaded from the phone's copy (Hubs.calm)", view(H.Hubs.stale(H.Hubs.local(one(hubactive()), "view", "projects"))), "projects")
    C.Check.str("through the app's exports", J.Json.get_str(A.screenj(A.local(one(hubactive()), "view", "projects")), "view", ""), "projects")
    C.Check.str("hub settings: shut at first", sets(one(box())), "shut")
    C.Check.str("hub settings: opened for a hub", sets(go(one(box()), "hub-settings", "box:3787|on")), "open")
    C.Check.str("hub settings: opened twice stays open", sets(go(go(one(box()), "hub-settings", "box:3787|on"), "hub-settings", "box:3787|on")), "open")
    C.Check.str("hub settings: shut again", sets(go(go(one(box()), "hub-settings", "box:3787|on"), "hub-settings", "box:3787|off")), "shut")
    C.Check.str("hub settings: the focus follows the named hub", H.Hubs.cur(go(two(), "hub-settings", "lap:3787|on")), "lap:3787")
    C.Check.str("hub settings: Notify me for rows are marked notify", notifyrows(go(one(box()), "hub-settings", "box:3787|on")), "Notify me for;Needs me;Settled;Reminders;Everything;")
    C.Check.str("hub settings: device tab rows stay off the hub page", localrows(go(one(box()), "hub-settings", "box:3787|on")), "Theme;Language;Agents reply in my language;Thread list;Tour;")
    C.Check.str("routines: a recurring and a one-time reminder", routines(one(reminded()), "title"), "Dentist;Digest;")
    C.Check.str("routines: the one-time one is a reminder", routines(one(reminded()), "state"), "reminder;idle;")
    C.Check.str("routines: Sched's kind", routines(one(reminded()), "sched"), "reminder;routine;")
