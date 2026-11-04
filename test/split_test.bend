import Base
import ./check.bend as C
import ../src/core/client.bend as U

# The combined view (client.bend's Split): which panes are put away when
# one is flipped (the board and the 3D one at a time, one pane always
# stays), and which sheet its schematic shows.

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("at first: the schematic and the board", Bool.show(Bool.and(U.Split.shows("", "pcb"), Bool.not(U.Split.shows("", "3d")))), "True")
    C.Check.str("the 3D shown puts the board away", U.Split.flip("", "3d"), "pcb")
    C.Check.str("and the board shown puts the 3D away", U.Split.flip("pcb", "pcb"), "3d")
    C.Check.str("the 3D put away leaves the schematic", U.Split.flip("pcb", "3d"), "pcb 3d")
    C.Check.str("the board put away leaves the schematic", U.Split.flip("", "pcb"), "pcb 3d")
    C.Check.str("the schematic on its own", U.Split.flip("pcb", "sch"), "sch pcb")
    C.Check.str("the schematic put away leaves the board", U.Split.flip("", "sch"), "sch 3d")
    C.Check.str("the last pane shown stays", U.Split.flip("pcb 3d", "sch"), "pcb 3d")
    C.Check.str("so does any last one", U.Split.flip("sch pcb", "3d"), "sch pcb")
    C.Check.str("the 3D alone gives way to the board alone", U.Split.flip("sch pcb", "pcb"), "sch 3d")
    C.Check.str("never the board and the 3D together", Bool.show(U.Split.shows("3d", "3d")), "False")
    C.Check.str("a word that is no pane changes nothing", U.Split.flip("pcb", "x"), "pcb")
    C.Check.str("a pane is found by its word", Bool.show(U.Split.has("sch 3d", "3d")), "True")
    C.Check.str("not by a part of one", Bool.show(U.Split.has("sch", "s")), "False")
    C.Check.str("no sheet opened: the board's own schematic", U.Split.sheet.k("", "/p/boards/a/a.kicad_sch", True{}), "/p/boards/a/a.kicad_sch")
    C.Check.str("a sheet opened in the board's folder stays", U.Split.sheet.k("/p/boards/a/power.kicad_sch", "/p/boards/a/a.kicad_sch", True{}), "/p/boards/a/power.kicad_sch")
    C.Check.str("another board's sheet gives way to the board's own", U.Split.sheet.k("/p/boards/b/power.kicad_sch", "/p/boards/a/a.kicad_sch", True{}), "/p/boards/a/a.kicad_sch")
    C.Check.str("one board: the sheet opened stays", U.Split.sheet.k("/p/power.kicad_sch", "", False{}), "/p/power.kicad_sch")
    C.Check.str("one board, none opened: the project's own", U.Split.sheet.k("", "", False{}), "")
    return Unit{}
