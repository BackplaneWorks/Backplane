import Base
import ./check.bend as C
import ../src/core/refresh.bend as RF
import ../src/server/plot.bend as PV

# Same-size nanosecond edits, atomic identity changes, unchanged quiet,
# and changes to either live side of a comparison use the shared decision.
def yes(b: Bool) -> String:
  match b:
    case True{}: "yes"
    case False{}: "no"

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("subsecond same-size save", yes(RF.Revision.changed(184, "1:2:184:100:2:100:2", 184, "1:2:184:100:1:100:1")), "yes")
    C.Check.str("atomic replacement preserves mtime", yes(RF.Revision.changed(184, "1:3:184:100:1:100:1", 184, "1:2:184:100:1:100:1")), "yes")
    C.Check.str("unchanged revision stays quiet", yes(RF.Revision.changed(184, "1:2:184:100:1:100:1", 184, "1:2:184:100:1:100:1")), "no")
    C.Check.str("size changes", yes(RF.Revision.changed(185, "same", 184, "same")), "yes")
    C.Check.str("old comparison side changes", yes(RF.Revision.changed(1, "old2|new1", 1, "old1|new1")), "yes")
    C.Check.str("new comparison side changes", yes(RF.Revision.changed(1, "old1|new2", 1, "old1|new1")), "yes")
