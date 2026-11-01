import Base
import ../../src/server/argv.bend as AV
import ../../src/server/storeio.bend as STO
import ../../src/server/db.bend as D

# `store_up_probe HOME`: opens HOME/store.db exactly as a hub's boot does
# (STO.Sto.boot: an in-place upgrade, a rebuild from the log, or neither)
# and stops, committing nothing else, so test/tools/store_upgrade_e2e.ts can
# compare a store upgraded in place with one rebuilt from the same log.
# Run: bend test/native/store_up_probe.bend HOME (its IO main)

def probe.k(xs: List<String>) -> IO(Unit):
  match xs:
    case home <> _:
      do IO<Unit>:
        o : STO.Opened <- STO.Sto.boot(home)
        c : String <- D.Db.close(STO.Opened.handle(o))
        IO.print("probe: " ++ c)
    case Nil{}:
      IO.print("usage: store_up_probe HOME")

def main() -> IO(Unit):
  IO.bind(List<String>, Unit, AV.Argv.args(), probe.k)
