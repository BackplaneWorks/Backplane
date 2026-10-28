import Base
import ./check.bend as C
import ../src/core/model.bend as M
import ../src/core/client.bend as U

# The simpler sidebar: quiet projects sink and fold away, the active view
# puts what waits on the person first and fades the recently settled, and
# a row's menu acts on it (Delete and Remove ask first).

def now() -> Nat:
  1000000n

def log() -> List<&2, M.Change>:
  [M.ProjectCreated{"p1", "board", "/tmp/board", 100n},
   M.ProjectCreated{"p2", "enclosure", "/tmp/enclosure", 200n},
   M.ProjectCreated{"p3", "firmware", "/tmp/firmware", 300n},
   M.ProjectCreated{"p4", "case", "/tmp/case", 998000n},
   M.ThreadCreated{"a", "p1", "Power sequencing", M.Local{}, "", "", "claude", "", 900000n},
   M.ThreadCreated{"b", "p1", "Pin swap", M.Local{}, "", "", "claude", "", 900100n},
   M.ThreadCreated{"c", "p1", "Pick a regulator", M.Local{}, "", "", "claude", "", 900200n},
   M.ThreadCreated{"d", "p1", "Silk cleanup", M.Local{}, "", "", "claude", "", 900300n},
   M.ThreadCreated{"e", "p1", "Old spike", M.Local{}, "", "", "claude", "", 400n},
   M.ThreadCreated{"f", "p2", "Wall bracket", M.Local{}, "", "", "claude", "", 999000n},
   M.TurnChanged{"a", M.Running{}, 999100n},
   M.TurnChanged{"b", M.Completed{}, 999200n},
   M.AskOpened{"q1", "c", "input", "AskUserQuestion", "Which one?", "", 999300n},
   M.ThreadSettled{"d", True{}, 999000n},
   M.ThreadSettled{"e", True{}, 500n},
   M.ThreadSettled{"f", True{}, 999500n}]

def ui(a: U.Act) -> U.Ui:
  match a:
    case U.Act{u, _}:
      u

def sent.go(cs: List<&2, U.Cmd>) -> String:
  match cs:
    case Nil{}:
      ""
    case U.Send{t} <> _:
      t
    case _ <> r:
      sent.go(r)

def sent(a: U.Act) -> String:
  match a:
    case U.Act{_, cs}:
      sent.go(cs)

def copied.go(cs: List<&2, U.Cmd>) -> String:
  match cs:
    case Nil{}:
      ""
    case U.Copy{t} <> _:
      t
    case _ <> r:
      copied.go(r)

def copied(a: U.Act) -> String:
  match a:
    case U.Act{_, cs}:
      copied.go(cs)

def base() -> U.Ui:
  U.Ui.fold(log(), U.Ui.init(now()))

# b made as a fork of a
def forked() -> U.Ui:
  U.Ui.fold(List.append(&2, M.Change, log(), [M.SettingSet{"fork.b", "a", 999600n}]), U.Ui.init(now()))

def st() -> M.State:
  M.State.replay(log(), M.State.empty())

def act(+u: U.Ui, +a: String, +v: String) -> U.Ui:
  ui(U.Ui.act(u, a, v))

def pids(+u: U.Ui) -> String:
  String.join(U.Side.projects(u), ",")

def labels(xs: List<&2, U.MItem>) -> List<&2, String>:
  match xs:
    case Nil{}:
      Nil{}
    case U.MItem{l, _, _, _} <> t:
      l <> labels(t)

def menu(+u: U.Ui) -> String:
  String.join(labels(U.Menu.items(u, False{})), ",")

def active(+q: String) -> String:
  String.join(U.Active.ids(U.Active.rows(st(), now(), q)), ",")

# the active view as a client shows it, the settled fold as the Ui has it
def shown(+u: U.Ui, +q: String) -> String:
  String.join(U.Active.ids(U.Active.folded(U.Active.open(U.Side.open(u), q), U.Active.parts(U.Active.rows(U.Side.st(u), U.Side.now(u), q)))), ",")

def faded(rs: List<&2, U.ARow>) -> List<&2, String>:
  match rs:
    case Nil{}:
      Nil{}
    case +r <> t:
      Bool.show(U.ARow.faded(r)) <> faded(t)

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("live, quiet, older folded", pids(base()), "p1,p2,p4")
    C.Check.str("older unfolds", pids(act(base(), "toggle-settled", "old-projects")), "p1,p2,p4,p3")
    C.Check.str("search reaches older", pids(act(base(), "bfield", "pq\u{1f}firm")), "p3")
    C.Check.str("view default", U.Side.view(st()), "projects")
    C.Check.str("view setting", U.Side.view(M.State.replay([M.SettingSet{"sidebar.view", "active", 1n}], st())), "active")
    C.Check.str("waits on you first, recent settled last", active(""), "c,b,a,f,d")
    C.Check.str("settled faded", String.join(faded(U.Active.rows(st(), now(), "")), ","), "False,False,False,True,True")
    C.Check.str("active search by project", active("enclosure"), "f")
    C.Check.str("active search by title", active("regul"), "c")
    C.Check.str("settled folded", shown(base(), ""), "c,b,a")
    C.Check.str("settled unfolds", shown(act(base(), "side-settled", ""), ""), "c,b,a,f,d")
    C.Check.str("settled fold shuts again", shown(act(act(base(), "side-settled", ""), "side-settled", ""), ""), "c,b,a")
    C.Check.str("search shows settled", shown(base(), "enclosure"), "f")
    C.Check.str("settled count", Nat.show(U.Active.faded_n(U.Active.parts(U.Active.rows(st(), now(), "")))), "2")
    C.Check.str("view swaps at once", U.Side.view(U.Side.st(act(base(), "setting", "sidebar.view=active"))), "active")
    C.Check.str("view swap still sent", sent(U.Ui.act(base(), "setting", "sidebar.view=active")), "{\"id\":0,\"m\":\"setting.set\",\"p\":{\"key\":\"sidebar.view\",\"value\":\"active\"}}")
    C.Check.str("view swap keeps the sequence", Nat.show(U.Ui.seq(act(base(), "setting", "sidebar.view=active"))), Nat.show(U.Ui.seq(base())))
    C.Check.str("menu opens", U.Ui.scratch(act(base(), "row-menu", "d"), "@menu"), "t:d")
    C.Check.str("same again shuts", U.Ui.scratch(act(act(base(), "row-menu", "d"), "row-menu", "d"), "@menu"), "")
    C.Check.str("settled menu", menu(act(base(), "row-menu", "d")), "New subthread,Fork thread,Bring back,Rename,Archive,Delete,Copy link")
    C.Check.str("open menu", menu(act(base(), "row-menu", "a")), "New subthread,Fork thread,Pin,Rename,Settle,Snooze 1 hour,Snooze until tomorrow,Snooze 1 week,Archive,Delete,Copy link")
    # the row menu offers what the header's ▾ does: a fork of the row's own thread, and a fork's way back
    C.Check.str("fork from a row", sent(U.Ui.act(act(base(), "row-menu", "b"), "fork", "b")), "{\"id\":0,\"m\":\"thread.fork\",\"p\":{\"thread\":\"b\"}}")
    C.Check.str("fork shuts", U.Ui.scratch(act(act(base(), "row-menu", "b"), "fork", "b"), "@menu"), "")
    C.Check.str("the header's fork is the open thread's", sent(U.Ui.act(act(base(), "select", "c"), "fork", "")), "{\"id\":1,\"m\":\"thread.fork\",\"p\":{\"thread\":\"c\"}}")
    C.Check.str("a fork's menu", menu(act(forked(), "row-menu", "b")), "New subthread,Fork thread,Merge into source thread,Pin,Rename,Settle,Snooze 1 hour,Snooze until tomorrow,Snooze 1 week,Archive,Delete,Copy link")
    C.Check.str("back from a row", sent(U.Ui.act(act(forked(), "row-menu", "b"), "back", "b")), "{\"id\":0,\"m\":\"thread.back\",\"p\":{\"thread\":\"b\"}}")
    # the open thread keeps its place: its project stays live for an hour
    # after that thread settled, and a shut fold still shows its row
    C.Check.str("held: settled within the hour", U.Side.cur.held(st(), "f", now()), "p2")
    C.Check.str("not held: settled long ago", U.Side.cur.held(st(), "e", now()) ++ "|" ++ U.Side.cur.held(st(), "", now()), "|")
    C.Check.str("open thread's project held live", pids(act(base(), "select", "f")), "p2,p1,p4")
    C.Check.str("held keeps the groups' order", String.join(U.Side.pids(U.SGroups.all(U.Side.groups.at(U.Side.rows(st(), now()), now(), ""))), ","), String.join(U.Side.pids(U.Side.rows(st(), now())), ","))
    C.Check.str("shut fold shows the open row", String.join(U.Side.ids(U.Side.sel(U.Ui.threads(st()), "d")), ",") ++ "|" ++ String.join(U.Side.ids(U.Side.sel(U.Ui.threads(st()), "")), ","), "d|")
    C.Check.str("rename opens on the title", U.Ui.scratch(act(base(), "row-rename", "d"), "@ren") ++ "|" ++ U.Ui.scratch(act(base(), "row-rename", "d"), "@f.title"), "d|" ++ U.Active.title(U.Ui.thread(st(), "d")))
    C.Check.str("rename sends the edited title", sent(U.Ui.act(act(act(base(), "row-rename", "d"), "bfield", "title\u{1f}  Enclosure v2 "), "rename-save", "")), "{\"id\":0,\"m\":\"thread.rename\",\"p\":{\"thread\":\"d\",\"title\":\"Enclosure v2\"}}")
    C.Check.str("a phone's rename names the thread", sent(U.Ui.act(base(), "rename-save", "f\u{1f}Fixture")), "{\"id\":0,\"m\":\"thread.rename\",\"p\":{\"thread\":\"f\",\"title\":\"Fixture\"}}")
    C.Check.str("a blank title is not sent", sent(U.Ui.act(act(act(base(), "row-rename", "d"), "bfield", "title\u{1f}   "), "rename-save", "")), "")
    C.Check.str("cancel leaves it", U.Ui.scratch(act(act(base(), "row-rename", "d"), "rename-no", ""), "@ren"), "")
    C.Check.str("delete asks, menu stays", menu(act(act(base(), "row-menu", "d"), "row-delete", "d")), "New subthread,Fork thread,Bring back,Rename,Archive,Click again to delete,Copy link")
    C.Check.str("delete twice deletes", sent(U.Ui.act(act(act(base(), "row-menu", "d"), "row-delete", "d"), "row-delete", "d")), "{\"id\":0,\"m\":\"thread.delete\",\"p\":{\"thread\":\"d\"}}")
    C.Check.str("deleted shuts", U.Ui.scratch(act(act(act(base(), "row-menu", "d"), "row-delete", "d"), "row-delete", "d"), "@menu"), "")
    C.Check.str("pin sends", sent(U.Ui.act(act(base(), "row-menu", "a"), "row-pin", "a")), "{\"id\":0,\"m\":\"thread.pin\",\"p\":{\"thread\":\"a\"}}")
    C.Check.str("pin shuts", U.Ui.scratch(act(act(base(), "row-menu", "a"), "row-pin", "a"), "@menu"), "")
    C.Check.str("bring back sends", sent(U.Ui.act(act(base(), "row-menu", "d"), "row-unsettle", "d")), "{\"id\":0,\"m\":\"thread.unsettle\",\"p\":{\"thread\":\"d\"}}")
    C.Check.str("other action shuts", U.Ui.scratch(act(act(base(), "row-menu", "a"), "toggle-settled", "p1"), "@menu"), "")
    C.Check.str("project menu", menu(act(base(), "proj-menu", "p1")), "New thread,Intent…,Remove project,Copy link")
    C.Check.str("rich project menu", String.join(labels(U.Menu.items(act(base(), "proj-menu", "p1"), True{})), ","), "New thread,Intent…,Colour and icon,Remove project,Copy link")
    C.Check.str("remove asks", menu(act(act(base(), "proj-menu", "p1"), "proj-remove", "p1")), "New thread,Intent…,Click again to remove,Copy link")
    C.Check.str("menu title", U.Menu.title(act(base(), "proj-menu", "p2")), "enclosure")
    C.Check.str("file menu", menu(act(act(base(), "select", "a"), "file-menu", "src/x.bend:12")), "Open,Show in folder,Copy path,Copy link")
    C.Check.str("file menu title", U.Menu.title(act(base(), "file-menu", "src/x.bend:12")), "src/x.bend:12")
    # (the first action on a thread also asks for its card, Fab.sync: one request id before it)
    C.Check.str("file open in the thread's folder", sent(U.Ui.act(act(base(), "select", "a"), "file-open", "src/x.bend:12")), "{\"id\":1,\"m\":\"file.open\",\"p\":{\"path\":\"/tmp/board/src/x.bend\",\"how\":\"open\"}}")
    C.Check.str("file reveal", sent(U.Ui.act(act(act(base(), "select", "a"), "file-menu", "/etc/hosts"), "file-reveal", "/etc/hosts")), "{\"id\":1,\"m\":\"file.open\",\"p\":{\"path\":\"/etc/hosts\",\"how\":\"reveal\"}}")
    C.Check.str("file reveal shuts", U.Ui.scratch(act(act(act(base(), "select", "a"), "file-menu", "/etc/hosts"), "file-reveal", "/etc/hosts"), "@menu"), "")
    C.Check.str("copy path", copied(U.Ui.act(act(base(), "select", "b"), "file-copy", "./README.md")), "/tmp/board/README.md")
    C.Check.str("close disarms", U.Ui.scratch(act(act(act(base(), "row-menu", "d"), "row-delete", "d"), "menu-close", ""), "@del"), "")
    C.Check.str("enter opens first active", U.Ui.sel(act(act(U.Ui.fold(List.append(&2, M.Change, log(), [M.SettingSet{"sidebar.view", "active", 1n}]), U.Ui.init(now())), "bfield", "pq\u{1f}pin swap"), "proj-go", "")), "b")
    C.Check.str("palette: a thread names its project", U.Palette.place(st(), "select", "a"), "board")
    C.Check.str("palette: a command names none", U.Palette.place(st(), "new-thread", "p1"), "")
