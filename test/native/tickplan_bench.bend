import Base
import ../tickplan_test.bend as F
import ../check.bend as C
import ../../src/core/tickplan.bend as T
import ../../src/core/model.bend as M
import ../../src/core/hub.bend as B

# Run only on the coordinated build host. Fixture construction is outside
# the timers. Full serialized output equality also forces every planned item.
def bench(+n: Nat, +active: Nat) -> IO(Unit):
  do IO<Unit>:
    +s : M.State = F.fixture(n, active, Nat.div(n, 4n))
    t0 : Nat <- IO.now()
    +old : String = F.step(B.Hub.tick(s, 900000n))
    +t1 : Nat <- IO.now()
    +new : String = F.step(T.Tick.main(s, 900000n))
    t2 : Nat <- IO.now()
    IO.print("threads=" ++ Nat.show(n) ++ " active=" ++ Nat.show(active) ++ " old_ms=" ++ Nat.show(Nat.sub(t1, t0)) ++ " new_ms=" ++ Nat.show(Nat.sub(t2, t1)) ++ " equal=" ++ Bool.show(C.Check.same(old, new)))

def main() -> IO(Unit):
  do IO<Unit>:
    bench(128n, 12n)
    bench(512n, 12n)
    bench(1024n, 20n)
