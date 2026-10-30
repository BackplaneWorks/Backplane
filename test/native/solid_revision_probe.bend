import Base
import ../../src/server/plot.bend as PV
import ../../src/server/argv.bend as AV
import ../../src/server/proc.bend as PR

# Real viewer source/cache functions, without a hub, event store or CAD export.
def revision.show(sm: U32 & String) -> String:
  (sz, rev) = sm
  U32.show(sz) ++ "|" ++ rev

def revision.run(+path: String) -> IO(Unit):
  do IO<Unit>:
    stamp : U32 & String <- PV.Pv.stamp.of(path)
    step : String <- PV.Pv.step.out(path)
    board : String <- PV.Pv.board.out(path, False{})
    bare : String <- PV.Pv.board.out(path, True{})
    IO.print(revision.show(stamp))
    IO.print(step)
    IO.print(board)
    IO.print(bare)

def revision.result(r: PR.Ran) -> IO(Unit):
  match r:
    case PR.Ran{code, _}: IO.print(U32.show(code))

def revision.dispatch(args: List<String>) -> IO(Unit):
  match args:
    case "promote" <> code <> path <> _:
      IO.bind(PR.Ran, Unit, PV.Pv.step.ran(PR.Ran{Maybe.default(&2, U32, U32.read(code), 3), "export"}, path), revision.result)
    case path <> _: revision.run(path)
    case _: IO.die(Unit, 2, "expected source path or promote CODE PATH")

def main() -> IO(Unit):
  IO.bind(List<String>, Unit, AV.Argv.args(), revision.dispatch)
