import Base
import ../mailperf_test.bend as T
import ../../src/core/mail.bend as ML
import ../../src/server/perf.bend as P

# Keep this expression on one line so the original-source comparison can
# substitute the old Mail.list total expression without changing the fixture.
def total(b: ML.Box) -> Nat:
  ML.Box.count(b, "b0", "", "")

def totals(n: Nat, +b: ML.Box, acc: Nat) -> Nat:
  match n:
    case 0n: acc
    case 1n+k: totals(k, b, Nat.add(acc, total(b)))

def bench(+convs: Nat, +mails: Nat) -> IO(Unit):
  do IO<Unit>:
    t0 : U32 <- P.Perf.us()
    +b0 : ML.Box = T.seed(convs, ML.Box.empty())
    P.Perf.add("orderbench.seed.bot.rows", U32.from_nat(ML.Order.count(ML.Box.order_of(b0, "b0"))))
    t1 : U32 <- P.Perf.span("orderbench.seed" ++ Nat.show(convs), t0)
    +b : ML.Box = T.posts(mails, convs, b0)
    P.Perf.add("orderbench.fold.bot.rows", U32.from_nat(ML.Order.count(ML.Box.order_of(b, "b0"))))
    t2 : U32 <- P.Perf.span("orderbench.fold" ++ Nat.show(mails), t1)
    P.Perf.add("orderbench.total.results", U32.from_nat(totals(1000n, b, 0n)))
    t3 : U32 <- P.Perf.span("orderbench.total1000", t2)
    # All timings above hold a reference. This final release is separate.
    P.Perf.add("orderbench.dispose.rows", U32.from_nat(ML.Order.count(ML.Box.order_of(b, "b0"))))
    t4 : U32 <- P.Perf.span("orderbench.dispose", t3)
    text : String <- P.Perf.take()
    IO.print(text)

def main() -> IO(Unit): bench(1000n, 10000n)
