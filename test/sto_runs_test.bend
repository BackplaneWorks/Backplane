import Base
import ./check.bend as C
import ../src/server/storedb.bend as SO

# Sto.lines.at reads one window per run of consecutive line numbers, in the
# order given: the runs Sto.runs finds

def show(rs: List<&2, SO.LRun>) -> String:
  match rs:
    case Nil{}:
      ""
    case SO.LRun{f, n} <> t:
      Nat.show(f) ++ "+" ++ Nat.show(n) ++ " " ++ show(t)

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("no numbers, no runs", show(SO.Sto.runs(Nil{})), "")
    C.Check.str("one number", show(SO.Sto.runs([7n])), "7+1 ")
    C.Check.str("consecutive numbers are one run", show(SO.Sto.runs([3n, 4n, 5n])), "3+3 ")
    C.Check.str("a gap starts a run", show(SO.Sto.runs([1n, 2n, 5n, 6n, 9n])), "1+2 5+2 9+1 ")
    C.Check.str("the order given is kept", show(SO.Sto.runs([9n, 10n, 2n, 3n])), "9+2 2+2 ")
    C.Check.str("a repeated number is its own run", show(SO.Sto.runs([4n, 4n])), "4+1 4+1 ")
