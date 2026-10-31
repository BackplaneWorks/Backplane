import Base
import ../core/vt.bend as VT
import ./dom.bend as D

# Fallback fonts need not advance by the VT grid's cell width (Greek and
# emoji often do not). Keep ASCII runs cheap and give other glyphs their
# emulator width, so later text and the caret share the same grid.
def Cells.ascii(text: String) -> List<&2, D.Node>:
  match text:
    case SNil{}: Nil{}
    case _: [D.Txt{text}]

type TermCells is Data:
  TermCells{text: String, nodes: List<&2, D.Node>}

def Cells.step(ascii: Bool, +cp: U32, cells: TermCells) -> TermCells:
  match ascii cells:
    case True{} TermCells{text, nodes}: TermCells{text ++ SCon{Chr{cp}, SNil{}}, nodes}
    case False{} TermCells{text, nodes}:
      TermCells{"", D.El{"span", "", [D.At{"class", "term-cell"}, D.At{"style", "display:inline-block;width:" ++ U32.show(VT.Vt.width(cp)) ++ "ch"}],
        [D.Txt{SCon{Chr{cp}, SNil{}}}]} <> List.append(&2, D.Node, Cells.ascii(text), nodes)}

def Cells.done(cells: TermCells) -> List<&2, D.Node>:
  match cells:
    case TermCells{text, nodes}: List.reverse(&2, D.Node, List.append(&2, D.Node, Cells.ascii(text), nodes))

def Cells.walk(s: String, cells: TermCells) -> List<&2, D.Node>:
  match s:
    case SNil{}: Cells.done(cells)
    case SCon{Chr{+cp}, rest}: Cells.walk(rest, Cells.step(U32.is_lt(cp, 128), cp, cells))

def Cells.of(s: String) -> List<&2, D.Node>:
  Cells.walk(s, TermCells{"", Nil{}})
