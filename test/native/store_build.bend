import Base
import ../../src/server/argv.bend as A
import ../../src/server/stobuild.bend as S

# HOME/store.db made from HOME/events.jsonl, streamed a chunk at a time
# (src/server/stobuild.bend). JS only: bend test/native/store_build.bend
# HOME [CHUNK_BYTES]. Never run it on a live home. Run it with the stack
# scripts/test.sh gives Bun (BUN_JSC_maxPerThreadStackUsage=536870912):
# a long message's facts recurse deeper than Bun's default allows.

def Main.chunk(xs: List<String>) -> U32:
  match xs:
    case _ <> c <> _:
      U32.from_nat(Maybe.default(&2, Nat, Nat.read(c), 1048576n))
    case _:
      1048576

def Main.show(b: S.Built) -> String:
  match b:
    case S.Built{seq, off, ord}:
      "lines=" ++ Nat.show(seq) ++ " applied_off=" ++ Nat.show(off) ++ " entries=" ++ Nat.show(Nat.sub(ord, 1n))

def Main.run(+home: String, +chunk: U32) -> IO(Unit):
  do IO<Unit>:
    t0 : Nat <- IO.now()
    b : S.Built <- S.Stb.build(home, chunk)
    t1 : Nat <- IO.now()
    IO.print(Main.show(b) ++ " ms=" ++ Nat.show(Nat.sub(t1, t0)))

def Main.go(xs: List<String>) -> IO(Unit):
  match xs:
    case Nil{}:
      IO.die(Unit, 2, "usage: store_build HOME [CHUNK_BYTES]")
    case +home <> rest:
      Main.run(home, Main.chunk(home <> rest))

def main() -> IO(Unit):
  IO.bind(List<String>, Unit, A.Argv.args(), Main.go)
