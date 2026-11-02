import Base
import ../../src/server/allowanceio.bend as A
import ../../src/server/argv.bend as V

def read(xs: List<String>) -> IO(Unit):
  match xs:
    case path <> _:
      IO.bind(String, Unit, A.AllowanceIO.read(path), IO.print)
    case Nil{}:
      IO.print("cache path required")

def main() -> IO(Unit):
  IO.bind(List<String>, Unit, V.Argv.args(), read)
