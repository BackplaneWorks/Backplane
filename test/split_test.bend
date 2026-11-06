import Base
import ./check.bend as C
import ../src/core/client.bend as U

# The combined view (client.bend's Split): which panes are put away when
# one is flipped (each on its own on a desktop; on a phone the board and
# the 3D one at a time; one pane always stays), and which sheet its
# schematic shows.

def main() -> IO(Unit):
  do IO<Unit>:
    # a desktop: the three at once, each on its own
    C.Check.str("desktop at first: all three", Bool.show(Bool.and(U.Split.shows(False{}, "", "sch"), Bool.and(U.Split.shows(False{}, "", "pcb"), U.Split.shows(False{}, "", "3d")))), "True")
    C.Check.str("desktop: the 3D put away", U.Split.flip(False{}, "", "3d"), "3d")
    C.Check.str("desktop: and brought back beside the board", U.Split.flip(False{}, "3d", "3d"), "")
    C.Check.str("desktop: the board put away keeps the 3D", U.Split.flip(False{}, "", "pcb"), "pcb")
    C.Check.str("desktop: put away in the panes' order", U.Split.flip(False{}, "3d", "sch"), "sch 3d")
    C.Check.str("desktop: the last pane shown stays", U.Split.flip(False{}, "pcb 3d", "sch"), "pcb 3d")
    C.Check.str("desktop: so does any last one", U.Split.flip(False{}, "sch 3d", "pcb"), "sch 3d")
    C.Check.str("desktop: a phone's choice shows the schematic and the 3D", Bool.show(Bool.and(U.Split.shows(False{}, "pcb", "3d"), Bool.not(U.Split.shows(False{}, "pcb", "pcb")))), "True")
    # a phone: the board and the 3D one at a time
    C.Check.str("phone at first: the schematic and the board", Bool.show(Bool.and(U.Split.shows(True{}, "", "pcb"), Bool.not(U.Split.shows(True{}, "", "3d")))), "True")
    C.Check.str("phone: the 3D shown puts the board away", U.Split.flip(True{}, "", "3d"), "pcb")
    C.Check.str("phone: and the board shown puts the 3D away", U.Split.flip(True{}, "pcb", "pcb"), "3d")
    C.Check.str("phone: the 3D put away leaves the schematic", U.Split.flip(True{}, "pcb", "3d"), "pcb 3d")
    C.Check.str("phone: the board put away leaves the schematic", U.Split.flip(True{}, "", "pcb"), "pcb 3d")
    C.Check.str("phone: the schematic on its own", U.Split.flip(True{}, "pcb", "sch"), "sch pcb")
    C.Check.str("phone: the schematic put away leaves the board", U.Split.flip(True{}, "", "sch"), "sch 3d")
    C.Check.str("phone: the last pane shown stays", U.Split.flip(True{}, "pcb 3d", "sch"), "pcb 3d")
    C.Check.str("phone: so does any last one", U.Split.flip(True{}, "sch pcb", "3d"), "sch pcb")
    C.Check.str("phone: the 3D alone gives way to the board alone", U.Split.flip(True{}, "sch pcb", "pcb"), "sch 3d")
    C.Check.str("phone: never the board and the 3D together", Bool.show(U.Split.shows(True{}, "3d", "3d")), "False")
    C.Check.str("a word that is no pane changes nothing", U.Split.flip(False{}, "pcb", "x"), "pcb")
    C.Check.str("a pane is found by its word", Bool.show(U.Split.has("sch 3d", "3d")), "True")
    C.Check.str("not by a part of one", Bool.show(U.Split.has("sch", "s")), "False")
    C.Check.str("no sheet opened: the board's own schematic", U.Split.sheet.k("", "/p/boards/a/a.kicad_sch", True{}), "/p/boards/a/a.kicad_sch")
    C.Check.str("a sheet opened in the board's folder stays", U.Split.sheet.k("/p/boards/a/power.kicad_sch", "/p/boards/a/a.kicad_sch", True{}), "/p/boards/a/power.kicad_sch")
    C.Check.str("another board's sheet gives way to the board's own", U.Split.sheet.k("/p/boards/b/power.kicad_sch", "/p/boards/a/a.kicad_sch", True{}), "/p/boards/a/a.kicad_sch")
    C.Check.str("one board: the sheet opened stays", U.Split.sheet.k("/p/power.kicad_sch", "", False{}), "/p/power.kicad_sch")
    C.Check.str("one board, none opened: the project's own", U.Split.sheet.k("", "", False{}), "")
    return Unit{}
