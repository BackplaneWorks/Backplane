import Base
import ./check.bend as C
import ../src/core/model.bend as M
import ../src/core/client.bend as U
import ../src/core/tools.bend as TO

# The Ready to order bar's ×: put away for the project (the setting
# fab.min.<project>), a Buy it in the thread's header while it is away,
# and that Buy it brings the bar back (client.bend's Fab.min, Fab.mini)

def log() -> List<&2, M.Change>:
  [M.ProjectCreated{"p1", "lamp", "/tmp/lamp", 100n},
   M.ProjectCreated{"p2", "stand", "/tmp/stand", 200n},
   M.ThreadCreated{"a", "p1", "Shade", M.Local{}, "", "", "claude", "", 300n},
   M.ThreadCreated{"b", "p1", "Base", M.Local{}, "", "", "claude", "", 400n},
   M.ThreadCreated{"c", "p2", "Stand", M.Local{}, "", "", "claude", "", 500n},
   M.TurnChanged{"a", M.Completed{}, 600n},
   M.TurnChanged{"b", M.Completed{}, 700n},
   M.TurnChanged{"c", M.Completed{}, 800n},
   M.SettingSet{"order.enabled", "on", 900n}]

def ui(a: U.Act) -> U.Ui:
  match a:
    case U.Act{u, _}:
      u

def sent.all(cs: List<&2, U.Cmd>) -> String:
  match cs:
    case Nil{}:
      ""
    case U.Send{t} <> r:
      t ++ "\n" ++ sent.all(r)
    case _ <> r:
      sent.all(r)

def sent(a: U.Act) -> String:
  match a:
    case U.Act{_, cs}:
      sent.all(cs)

# a thread chosen, with the card a hub would have answered for it
def card(+u: U.Ui, +th: String) -> U.Ui:
  U.Ui.info_set(U.Ui.info_set(ui(U.Ui.act(u, "select", th)), "fab.buy:" ++ th, "Buy it · from $5.00"), "fab.ready:" ++ th, "Ready to order · Lamp")

def base() -> U.Ui:
  card(card(card(U.Ui.fold(log(), U.Ui.init(1000n)), "c"), "b"), "a")

def labels(ts: List<&2, TO.Tool>) -> String:
  match ts:
    case Nil{}:
      ""
    case TO.Tool{_, +label, _, +action, _, _, _, _} <> r:
      label ++ ":" ++ action ++ "|" ++ labels(r)

def hidden() -> U.Ui:
  ui(U.Ui.act(base(), "fab-hide", ""))

def go(+u: U.Ui, +th: String) -> U.Ui:
  ui(U.Ui.act(u, "select", th))

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("the bar shows", U.Fab.ready(base()), "Ready to order · Lamp")
    C.Check.str("no header Buy it while it shows", labels(U.Fab.mini.tools(base())), "")
    C.Check.str("× asks the hub to keep it away for the project", Bool.show(String.contains(sent(U.Ui.act(base(), "fab-hide", "")), "\"key\":\"fab.min.p1\",\"value\":\"1\"")), "True")
    C.Check.str("× puts it away at once", U.Fab.ready(hidden()), "")
    C.Check.str("the header has Buy it", labels(U.Fab.mini.tools(hidden())), "Buy it:fab-show|")
    C.Check.str("away in the project's other threads too", U.Fab.ready(go(hidden(), "b")), "")
    C.Check.str("another project keeps its bar", U.Fab.ready(go(hidden(), "c")), "Ready to order · Lamp")
    C.Check.str("another project has no header Buy it", labels(U.Fab.mini.tools(go(hidden(), "c"))), "")
    C.Check.str("the header's Buy it asks for the bar back", Bool.show(String.contains(sent(U.Ui.act(hidden(), "fab-show", "")), "\"key\":\"fab.min.p1\",\"value\":\"0\"")), "True")
    C.Check.str("and it is back", U.Fab.ready(ui(U.Ui.act(hidden(), "fab-show", ""))), "Ready to order · Lamp")
    C.Check.str("with no header Buy it", labels(U.Fab.mini.tools(ui(U.Ui.act(hidden(), "fab-show", "")))), "")
    C.Check.str("ordered: the header says so", labels(U.Fab.mini.tools(U.Ui.info_set(hidden(), "fab.again:a", "1"))), "Ordered:fab-show|")
