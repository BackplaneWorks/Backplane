import Base
import ./check.bend as C
import ../src/core/client.bend as U
import ../src/core/model.bend as M

# Different boards can share one folder. Their exact board and root-sheet
# paths must win over a folder match when moving between viewer tabs.
def picked() -> U.Ui:
  U.Bd.picked(U.Ui.info_set(U.Bots.sel.set(U.Ui.fold([
    M.ProjectCreated{"p", "Boards", "/project", 1n},
    M.ThreadCreated{"t", "p", "Boards", M.Local{}, "", "", "claude", "", 2n}], U.Ui.init(3n)), "t"),
    "fab.boards:t", "boards/alpha.kicad_pcb\nboards/beta.kicad_pcb"), "boards/beta.kicad_pcb")

def board() -> U.Ui:
  U.Ui.view_put(picked(), "board", "/project/boards/beta.kicad_pcb")

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("exact board wins over shared folder", U.Bd.cur(["boards/alpha.kicad_pcb", "boards/beta.kicad_pcb"], "/project/boards/beta.kicad_pcb"), "boards/beta.kicad_pcb")
    C.Check.str("exact root sheet wins over shared folder", U.Bd.cur(["boards/alpha.kicad_pcb", "boards/beta.kicad_pcb"], "/project/boards/beta.kicad_sch"), "boards/beta.kicad_pcb")
    C.Check.str("schematic tab keeps picked board", U.View.kind.path(board(), "schematic"), "/project/boards/beta.kicad_sch")
    C.Check.str("3D tab keeps picked board", U.View.kind.path(board(), "3d"), "/project/boards/beta.kicad_pcb")
    C.Check.str("Split tab keeps picked board", U.View.kind.path(board(), "split"), "/project/boards/beta.kicad_pcb")
    C.Check.str("a root sheet returns to its board", U.View.kind.path(U.Ui.view_put(board(), "schematic", "/project/boards/beta.kicad_sch"), "board"), "/project/boards/beta.kicad_pcb")
    C.Check.str("a sub-sheet returns to the picked board", U.View.kind.path(U.Ui.view_put(board(), "schematic", "/project/boards/power.kicad_sch"), "board"), "/project/boards/beta.kicad_pcb")
    C.Check.str("a root sheet overrides a previous board", U.Bd.cur.at(["boards/alpha.kicad_pcb", "boards/beta.kicad_pcb"], "/project/boards/alpha.kicad_sch", "/project/boards/beta.kicad_pcb"), "boards/alpha.kicad_pcb")
    C.Check.str("a different folder does not inherit a board", U.Bd.cur.at(["boards/alpha.kicad_pcb", "other/beta.kicad_pcb"], "/project/other/power.kicad_sch", "/project/boards/alpha.kicad_pcb"), "other/beta.kicad_pcb")
    C.Check.str("a comparison keeps the exact board family", U.Bd.cur(["boards/alpha.kicad_pcb", "boards/beta.kicad_pcb"], "diff\n/project/boards/beta.kicad_pcb\n/project/boards/beta.kicad_pcb\n64"), "boards/beta.kicad_pcb")
