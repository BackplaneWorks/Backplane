import Base
import ../../src/core/perf.bend as PF
import ../../src/server/perf.bend as PS

# The performance overlay's text from real readings of a busy process
# (30 window rows, 7 hub rows), timed.
#   scripts/build-js.sh test/native/hud_bench.bend build/hud_bench
#   build/hud_bench

def fill(+n: Nat, +k: U32) -> IO(Unit):
  match n:
    case 0n:
      IO.pure(Unit, Unit{})
    case 1n+m:
      do IO<Unit>:
        +t0 : U32 <- PS.Perf.us()
        t1 : U32 <- PS.Perf.span("ui.msg.k" ++ Nat.show(Nat.mod(m, 30n)), (t0 - (k * U32.from_nat(Nat.mod(m, 97n)) : U32) : U32))
        t2 : U32 <- PS.Perf.span("hub.k" ++ Nat.show(Nat.mod(m, 7n)), (t0 - (k * 13 : U32) : U32))
        fill(m, k)

def main() -> IO(Unit):
  do IO<Unit>:
    fill(3000n, 401)
    +base : String <- PS.Perf.take()
    fill(3000n, 901)
    +cur : String <- PS.Perf.take()
    +t0 : Nat <- IO.now()
    +h : String = PF.Hud.text(cur, base, Nat.sub(t0, 3000n), t0)
    +t1 : Nat <- IO.now()
    IO.print(h)
    IO.print("overlay text: " ++ Nat.show(Nat.sub(t1, t0)) ++ " ms")
