import Base
import ./check.bend as C
import ../src/core/i18n.bend as I
import ../src/core/plot-error.bend as PE
import ../src/core/msgs.bend as MSG

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("missing file in Japanese", PE.PlotError.text(I.Ja{}, "@bp.file-missing:board.kicad_pcb"), MSG.T.viewer.file.missing(I.Ja{}, "board.kicad_pcb"))
    C.Check.str("unread version in Japanese", PE.PlotError.text(I.Ja{}, "@bp.file-unread"), MSG.T.viewer.file.unread(I.Ja{}))
    C.Check.str("other diagnostics stay intact", PE.PlotError.text(I.Ja{}, "broken triangle"), "broken triangle")
