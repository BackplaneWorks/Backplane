import Base
import ./check.bend as C
import ./hist_test.bend as T
import ../src/core/hist.bend as H
import ../src/core/hier.bend as HR
import ../src/core/client.bend as U

def rows() -> List<&2, HR.Row>:
  [HR.Row{0n, "Root", H.Hist.git("/r/.git", "c1", "/r", "root.kicad_sch"), False{}},
   HR.Row{1n, "Child", H.Hist.git("/r/.git", "c1", "/r", "child.kicad_sch"), False{}}]

def ui() -> U.Ui:
  +u = U.Hs.set(U.Ui.view_put(T.base(), "schematic", H.Hist.git("/r/.git", "c1", "/r", "root.kicad_sch")), "hist.at", "step:c1")
  U.Ui.info_set(u, "sheets:" ++ U.Sch.want(u), HR.Hier.show(H.Hist.tree.live(rows())))

def main() -> IO(Unit):
  +child = T.act(ui(), "sch-go", "1\t/Child/\t")
  do IO<Unit>:
    C.Check.str("version hierarchy wire retains complete root path", HR.Hier.file(HR.Hier.read(HR.Hier.show(H.Hist.tree.live(rows()))), 0n), "/r/root.kicad_sch")
    C.Check.str("version hierarchy wire retains complete child path", HR.Hier.file(HR.Hier.read(HR.Hier.show(H.Hist.tree.live(rows()))), 1n), "/r/child.kicad_sch")
    C.Check.str("opening a wire-decoded child keeps its revision", T.path(child), H.Hist.git("/r/.git", "c1", "/r", "child.kicad_sch"))
    C.Check.str("highlighting the child keeps its file", H.Hist.family(T.path(T.act(child, "hist-diff", ""))), "/r/child.kicad_sch")
    C.Check.str("Live from the child returns to its live file", T.path(T.act(child, "hist-live", "")), "/r/child.kicad_sch")
