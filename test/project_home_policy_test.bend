import Base
import ./check.bend as C
import ../src/core/json.bend as J
import ../src/core/project-home.bend as P

def row(kind: String, path: String) -> J.Json:
  J.Json.obj([J.KV{"kind", J.Str{kind}}, J.KV{"path", J.Str{path}}])

def contents() -> J.Json:
  J.Json.obj([J.KV{"artifacts", J.Json.arr([row("board", "/p/boards/logic.kicad_pcb"), row("schematic", "/p/drawings/control.kicad_sch")])}])

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("project home round trip", P.Home.project(P.Home.owner("board-project")), "board-project")
    C.Check.str("3D uses exact saved board", P.Home.view_path(contents(), "3d"), "/p/boards/logic.kicad_pcb")
    C.Check.str("Board uses same saved board", P.Home.view_path(contents(), "board"), "/p/boards/logic.kicad_pcb")
    C.Check.str("Split anchors same saved board", P.Home.view_path(contents(), "split"), "/p/boards/logic.kicad_pcb")
    C.Check.str("schematic keeps a different saved filename", P.Home.view_path(contents(), "schematic"), "/p/drawings/control.kicad_sch")
    C.Check.str("Split needs both documents", P.Home.view_path(J.Json.obj([J.KV{"artifacts", J.Json.arr([row("board", "/p/logic.kicad_pcb")])}]), "split"), "")
    C.Check.str("software has no invented board", P.Home.view_path(J.Json.obj([]), "3d"), "")
    C.Check.str("spec project has no invented schematic", P.Home.view_path(J.Json.obj([J.KV{"artifacts", J.Json.arr([row("spec", "/p/docs/SPEC.md")])}]), "schematic"), "")
    C.Check.str("absent source has no panel", Nat.show(List.length(&2, P.Panel, P.Home.panel("board", "", "Board", ""))), "0")
