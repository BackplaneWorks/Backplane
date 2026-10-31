import Base
import ./check.bend as C
import ../src/web/terminal-cells.bend as V
import ../src/web/dom.bend as D
import ../src/web/presentation.bend as P
import ../src/core/json.bend as J

def nodes(s: String) -> String:
  J.Json.show(P.Node.write(D.El{"div", "", Nil{}, V.Cells.of(s)}))

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("ASCII stays one text node", Nat.show(List.length(&2, D.Node, V.Cells.of("ASCII only"))), "1")
    C.Check.str("Greek keeps one cell", Bool.show(String.contains(nodes("α"), "width:1ch")), "True")
    C.Check.str("emoji keeps two cells", Bool.show(String.contains(nodes("😀"), "width:2ch")), "True")
    C.Check.str("Japanese keeps two cells", Bool.show(String.contains(nodes("日"), "width:2ch")), "True")
    C.Check.str("mixed text order stays intact", Bool.show(String.contains(nodes("abαcd"), "ab")), "True")
