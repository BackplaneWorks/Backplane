import Base
import ./check.bend as C
import ../src/web/view.bend as V
import ../src/web/dom.bend as D
import ../src/core/i18n.bend as I
import ../src/core/model.bend as M
import ../src/core/signal.bend as SG

def attrs(n: D.Node) -> List<&2, D.Attr>:
  match n:
    case D.El{_, _, as, _}: as
    case _: Nil{}

def base() -> M.State:
  M.State.replay([M.ProjectCreated{"p", "Project", "/tmp/sidebar-finished", 1n},
    M.ThreadCreated{"t", "p", "Finished handoff", M.Local{}, "", "", "codex", "", 1n},
    M.TurnChanged{"t", M.Completed{}, 2n}], M.State.empty())

def name(+s: M.State) -> String:
  SG.Signal.name(SG.Signal.at(SG.Signal.index(s), "t"))

def word(lc: I.Loc, n: String) -> String:
  V.View.attr(attrs(V.View.dot(lc, n)), "data-status-label")

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("completed retained thread says finished", name(base()), "finished")
    C.Check.str("finished is the green dot, no word beside it", word(I.En{}, name(base())), "")
    C.Check.str("finished keeps descriptive accessible label", V.View.attr(attrs(V.View.dot(I.En{}, "finished")), "aria-label"), "Finished: ready for you to look at")
    +running : M.State = M.State.apply(base(), M.TurnChanged{"t", M.Running{}, 3n})
    C.Check.str("running keeps activity signal", name(running), "active")
    +monitor : M.State = M.State.apply(base(), M.WorkSet{"t", "watch", "w\twatching\t\tReal monitor\twatch\trun\tcmd\t-\t0", 3n})
    C.Check.str("completed with owned work remains waiting", name(monitor), "waiting")
    +settled : M.State = M.State.apply(base(), M.ThreadSettled{"t", True{}, 3n})
    C.Check.str("settled differs from finished", name(settled), "settled")
