import Base
import ../../src/server/argv.bend as AV
import ../../src/server/storefar.bend as SFL

# `store_far_probe HOME`: the far_line refill (storefar.bend's Fl.child) on
# HOME's store, as the hub's child runs it. Run: bend
# test/native/store_far_probe.bend HOME (test/tools/store_far_line_e2e.ts)

def probe.k(xs: List<String>) -> IO(Unit):
  match xs:
    case home <> _:
      SFL.Fl.child(home)
    case Nil{}:
      IO.print("usage: store_far_probe HOME")

def main() -> IO(Unit):
  IO.bind(List<String>, Unit, AV.Argv.args(), probe.k)
