import Base
import ../../src/core/outbox.bend as O
import ../../src/server/perf.bend as P
import ../check.bend as C

def seed(+n: Nat, xs: List<&2, String>) -> List<&2, String>:
  match n:
    case 0n: xs
    case 1n+k: seed(k, Nat.show(k) <> xs)

def window(more: Bool, xs: List<&2, String>, n: Nat, at: Nat) -> List<&2, String>:
  match more:
    case True{}: O.Join.window(xs, n, at)
    case False{}: Nil{}

# The two List.length calls in the retained Boot.k.rest implementation.
def old(+xs: List<&2, String>, +at: Nat) -> List<&2, String>:
  window(Nat.is_lt(at, List.length(&2, String, xs)), xs, List.length(&2, String, xs), at)

def cached(+xs: List<&2, String>, +n: Nat, +at: Nat) -> List<&2, String>:
  window(Nat.is_lt(at, n), xs, n, at)

def run_old(fuel: Nat, +xs: List<&2, String>, +at: Nat, acc: Nat) -> Nat:
  match fuel:
    case 0n: acc
    case 1n+k: run_old(k, xs, at, Nat.add(acc, List.length(&2, String, old(xs, at))))

def run_cached(fuel: Nat, +xs: List<&2, String>, +n: Nat, +at: Nat, acc: Nat) -> Nat:
  match fuel:
    case 0n: acc
    case 1n+k: run_cached(k, xs, n, at, Nat.add(acc, List.length(&2, String, cached(xs, n, at))))

def main() -> IO(Unit):
  do IO<Unit>:
    +xs : List<&2, String> = seed(50000n, Nil{})
    C.Check.str("cached near-head window is identical", String.join(cached(xs, 50000n, 49968n), ","), String.join(old(xs, 49968n), ","))
    C.Check.str("cached end cutoff is identical", String.join(cached(xs, 50000n, 50000n), ","), String.join(old(xs, 50000n), ","))
    t0 : U32 <- P.Perf.us()
    P.Perf.add("bootbench.old.items", U32.from_nat(run_old(1000n, xs, 49968n, 0n)))
    t1 : U32 <- P.Perf.span("bootbench.old.1000windows", t0)
    P.Perf.add("bootbench.cached.items", U32.from_nat(run_cached(1000n, xs, 50000n, 49968n, 0n)))
    t2 : U32 <- P.Perf.span("bootbench.cached.1000windows", t1)
    P.Perf.add("bootbench.retained.lines", U32.from_nat(List.length(&2, String, xs)))
    t3 : U32 <- P.Perf.span("bootbench.dispose", t2)
    text : String <- P.Perf.take()
    IO.print(text)
