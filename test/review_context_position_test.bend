import Base
import ./check.bend as C
import ./design_context_test.bend as F
import ../src/core/client.bend as U

def main() -> IO(Unit):
  +u = U.Ui.view_put(F.pending_board(), "review", "bom")
  do IO<Unit>:
    C.Check.str("BOM reading scope keeps beta before its card", U.Rp.place(u), "[\"bom\",\"boards/beta/beta.kicad_pcb\",\"\",\"\"]")
