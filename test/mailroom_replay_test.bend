import Base
import ./native/mailroom_replay.bend as RP
import ../src/app/layout.bend as L
import ../src/app/bots.bend as B

# Fast independent output replay; native repeats it with system faces.
def main() -> IO(Unit):
  do IO<Unit>:
    c : RP.Counts <- RP.replay(RP.cases(), L.Ty.none(), B.Bv.wraps.none(), RP.Counts{0n, 0n, 0n, 0n})
    RP.report(c)
