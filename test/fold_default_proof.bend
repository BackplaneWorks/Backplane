import Base
import ../src/core/model.bend as M
import ../src/core/i18n.bend as I
import ../src/core/fold.bend as F

law default_closed:
  for +lc: I.Loc
  for +es: List<&2, M.Entry>
  for +last: Bool
  for +running: Bool
  {F.Fold.grp.roles(lc, F.GRun{es}, last, Map.new(&2, String), running) == F.Fold.run.roles(lc, es, False{}) : List<&2, F.Role>}

def default_closed(lc, es, last, running): {==}
