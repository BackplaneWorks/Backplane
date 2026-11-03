import Base
import ../src/core/outbox.bend as O

# Emitted JS property driver exercises the exact hub credit reducer.
def initial() -> O.Cr:
  O.Cr{7, 99n, 3n, 500n, 20n, False{}, True{}, 5n, 6n, 7n, True{}, ["left"]}

def report(c: O.Cr, gen: U32, n: Nat, bytes: Nat) -> O.Cr:
  O.Cr.drained(c, gen, n, bytes)
