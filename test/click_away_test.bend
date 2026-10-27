import Base
import ./check.bend as C
import ../src/core/client.bend as U
import ../src/core/model.bend as M
import ../src/web/workspace.bend as WS

# Leaving a full pane by picking something else, and the rail's toggles
# (client.bend's Want.thread.k and Rail.*, workspace.bend's WS.key).

def base() -> U.Ui:
  U.Ui.select.k(U.Ui.fold([M.ProjectCreated{"p", "Project", "/fixture", 1n},
    M.ThreadCreated{"current", "p", "Current thread", M.Local{}, "", "/fixture", "codex", "", 1n},
    M.ThreadCreated{"other", "p", "Other thread", M.Local{}, "", "/fixture", "codex", "", 1n}], U.Ui.init(2n)), "current")

def t_ui(a: U.Act) -> U.Ui:
  match a:
    case U.Act{u, _}: u

def flag(+u: U.Ui, +k: String) -> String:
  Bool.show(U.Ui.flag(u, k))

# New thread pressed in Settings, the hub's answer, then the thread
def made(+u: U.Ui) -> U.Ui:
  U.Ui.recv(U.Ui.recv(t_ui(U.Ui.new_thread(U.Ui.flag_on(u, "settings"), "p")), U.Made.made("changes", "t9")), U.Made.demo("0", "t9"))

# the viewer open on a kind, for the selected thread
def shown(+u: U.Ui, +kind: String) -> U.Ui:
  U.Ui.view_put(u, kind, "")

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("new thread from Settings: the thread opens", U.Ui.sel(made(base())), "t9")
    C.Check.str("new thread from Settings: Settings goes", flag(made(base()), "settings"), "False")
    C.Check.str("new thread: Settings untouched while it is made", flag(t_ui(U.Ui.new_thread(U.Ui.flag_on(base(), "settings"), "p")), "settings"), "True")
    C.Check.str("a wanted thread arriving leaves Settings", flag(U.Want.thread.k(True{}, U.Ui.flag_on(base(), "settings"), "other"), "settings"), "False")
    C.Check.str("a thread from Settings still leaves it", flag(t_ui(U.Ui.act(U.Ui.flag_on(base(), "settings"), "select", "other")), "settings"), "False")
    C.Check.str("rail lights nothing while the viewer is shut", U.Rail.tab(base()), "")
    C.Check.str("rail lights the view on show", U.Rail.tab(shown(base(), "schematic")), "schematic")
    C.Check.str("rail lights Files for an image", U.Rail.tab.of(True{}, "image", "/p/a.png"), "files")
    C.Check.str("rail lights Mech for a part in 3D", U.Rail.tab.of(True{}, "3d", "/p/mech/case.step"), "mech")
    C.Check.str("rail lights 3D for the board's model", U.Rail.tab.of(True{}, "3d", ""), "3d")
    C.Check.str("the view on show: its button shuts", U.Rail.act("board", "board", "view-kind"), "panel")
    C.Check.str("the view on show: no value", U.Rail.value("board", "board", "board"), "")
    C.Check.str("another view: its own action", U.Rail.act("board", "schematic", "view-kind"), "view-kind")
    C.Check.str("another view: its own value", U.Rail.value("board", "schematic", "schematic"), "schematic")
    C.Check.str("viewer shut: its own action", U.Rail.act("", "board", "view-kind"), "view-kind")
    C.Check.str("Board pressed on the board shuts the viewer", Bool.show(U.Ui.panel(t_ui(U.Ui.act(shown(base(), "board"), U.Rail.act(U.Rail.tab(shown(base(), "board")), "board", "view-kind"), U.Rail.value(U.Rail.tab(shown(base(), "board")), "board", "board"))))), "False")
    C.Check.str("Board pressed on the schematic shows the board", U.Rail.tab(t_ui(U.Ui.act(shown(base(), "schematic"), U.Rail.act(U.Rail.tab(shown(base(), "schematic")), "board", "view-kind"), U.Rail.value(U.Rail.tab(shown(base(), "schematic")), "board", "board")))), "board")
    C.Check.str("ctrl+D on the diff shuts the viewer", Bool.show(U.Ui.panel(t_ui(WS.WS.act("ws-key", shown(base(), "diff"), "diff")))), "False")
    C.Check.str("ctrl+P on the files shuts the viewer", Bool.show(U.Ui.panel(t_ui(WS.WS.act("ws-key", shown(base(), "files"), "files")))), "False")
    C.Check.str("ctrl+P on the board shows the files", U.Rail.tab(t_ui(WS.WS.act("ws-key", shown(base(), "board"), "files"))), "files")
    C.Check.str("Commands opens the palette", flag(t_ui(WS.WS.act("ws-palette-open", base(), "")), "palette"), "True")
    C.Check.str("Commands again shuts it", flag(t_ui(WS.WS.act("ws-palette-open", t_ui(WS.WS.act("ws-palette-open", base(), "")), "")), "palette"), "False")
    C.Check.str("the open palette's button shuts it", U.Rail.palette(True{}), "ws-palette-close")
