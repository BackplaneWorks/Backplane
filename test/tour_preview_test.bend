import Base
import ./check.bend as C
import ../src/core/model.bend as M
import ../src/core/client.bend as U

def base() -> U.Ui:
  U.Ui.fold([M.ProjectCreated{"p", "Board", "/fixture", 1n}, M.ThreadCreated{"t", "p", "Thread", M.Local{}, "", "/fixture", "claude", "", 2n}], U.Ui.init(100n))

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("first-run tools step selects existing thread", U.Ui.sel(U.Tour.preview(3n, base())), "t")
    C.Check.str("other steps keep welcome screen", U.Ui.sel(U.Tour.preview(2n, base())), "")
    C.Check.str("no thread is created for the tour", U.Ui.sel(U.Tour.preview(3n, U.Ui.init(0n))), "")
    C.Check.str("existing selection stays", U.Ui.sel(U.Tour.preview(3n, U.Ui.select.k(base(), "t"))), "t")
    C.Check.str("tools step shows its examples", Bool.show(U.Tour.example(3n)), "True")
    C.Check.str("other steps need no example", Bool.show(U.Tour.example(2n)), "False")
