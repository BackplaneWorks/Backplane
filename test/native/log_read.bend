import Base
import ../../src/server/argv.bend as A
import ../../src/server/logread.bend as L

# A small native entry for the strict history reader. Each failing case
# runs in its own process, so its fatal status and diagnostic can be checked.
#   build/log_read PATH
#   build/log_read --count PATH
#   build/log_read --short-read PATH

def Probe.words(bs: List<&2, U32>, acc: List<&2, String>) -> String:
  match bs:
    case Nil{}:
      String.join(List.reverse(&2, String, acc), ",")
    case b <> t:
      Probe.words(t, U32.show(b) <> acc)

def Probe.show(+bs: List<&2, U32>) -> IO(Unit):
  do IO<Unit>:
    IO.print("read " ++ Nat.show(L.LogRead.count(bs, 0n)))
    IO.print(Probe.words(bs, Nil{}))

def Probe.count(bs: List<&2, U32>) -> IO(Unit):
  IO.print("read " ++ Nat.show(L.LogRead.count(bs, 0n)))

def Probe.args(xs: List<String>) -> IO(Unit):
  match xs:
    case "--short-read" <> +path <> Nil{}:
      IO.bind(List<&2, U32>, Unit,
        L.LogRead.checked(L.LogRead.complete(4, 3n), path, 4, 3n, [65, 66, 67]), bs => Probe.show(bs))
    case "--count" <> +path <> Nil{}:
      IO.bind(List<&2, U32>, Unit, L.LogRead.bytes(path), Probe.count)
    case +path <> Nil{}:
      IO.bind(List<&2, U32>, Unit, L.LogRead.bytes(path), bs => Probe.show(bs))
    case _:
      IO.die(Unit, 2, "usage: log_read [--count | --short-read] PATH")

def main() -> IO(Unit):
  IO.bind(List<String>, Unit, A.Argv.args(), Probe.args)
