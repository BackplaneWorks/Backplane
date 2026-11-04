import Base
import ./check.bend as C
import ../src/server/thin-signals.bend as S
import ../src/server/thin-signal-text.bend as T

def yes(b: Bool) -> String:
  Bool.pick(String, b, "yes", "no")

# Production-sized catalog, with actual-shaped thread identifiers. No private
# data or saved fixture is needed to catch the old 8192-entry retention cliff.
def ids(+n: Nat, acc: Map<&2, Nat>) -> Map<&2, Nat>:
  match n:
    case 0n: acc
    case 1n+k:
      ids(k, Map.set(&2, Nat, acc, "t1793648028-" ++ Nat.show(1200000n+k), 0n))

def main() -> IO(Unit):
  do IO<Unit>:
    +signals : Map<&2, Nat> = ids(10841n, Map.new(&2, Nat))
    +scalar : S.SignalCache = S.SignalCache.build("epoch", 17n, signals)
    +text : T.SignalTextCache = T.SignalTextCache.build("epoch", 17n, True{}, signals)
    C.Check.str("10841-thread scalar projection can be retained", yes(S.SignalCache.bounded(scalar)), "yes")
    C.Check.str("10841-thread scalar projection hits unchanged epoch and revision", yes(S.SignalCache.hit(scalar, "epoch", 17n)), "yes")
    C.Check.str("10841-thread scalar projection rejects different revision", yes(S.SignalCache.hit(scalar, "epoch", 18n)), "no")
    C.Check.str("10841-thread text projection can be retained within original byte budget", yes(T.SignalTextCache.bounded(text)), "yes")
    C.Check.str("10841-thread text projection hits unchanged inputs", yes(T.SignalTextCache.hit(text, "epoch", 17n, True{})), "yes")
    C.Check.str("10841-thread text projection rejects recipient visibility change", yes(T.SignalTextCache.hit(text, "epoch", 17n, False{})), "no")
