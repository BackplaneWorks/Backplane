import Base
import ../../src/server/argv.bend as A
import ../../src/server/filebytes.bend as F

def bytes(bs: List<&2, U32>) -> List<&2, String>:
  match bs:
    case Nil{}: Nil{}
    case b <> t: U32.show(b) <> bytes(t)

def show(r: Result<&2, &2, String, List<&2, U32>>) -> IO(Unit):
  match r:
    case Done{bs}:
      IO.print("ok:" ++ String.join(bytes(bs), ","))
    case Fail{_}:
      IO.print("error")

def args(xs: List<String>) -> IO(Unit):
  match xs:
    case path <> Nil{}:
      IO.bind(Result<&2, &2, String, List<&2, U32>>, Unit, F.FileBytes.read(path), show)
    case _:
      IO.die(Unit, 2, "usage: file_bytes PATH")

def main() -> IO(Unit):
  IO.bind(List<String>, Unit, A.Argv.args(), args)
