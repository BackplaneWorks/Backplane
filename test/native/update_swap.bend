import Base
import ../../src/server/argv.bend as AV
import ../../src/server/update.bend as U

# Calls the production updater's file swap only. The harness supplies
# temporary source and install directories; no release is downloaded and
# no hub, installed binary, or provider is started.
def done(error: String) -> IO(Unit):
  match error:
    case SNil{}:
      IO.print("ok update swap")
    case other:
      IO.die(Unit, 1, other)

def args(xs: List<String>) -> IO(Unit):
  match xs:
    case src <> dst <> tag <> Nil{}:
      IO.bind(String, Unit, U.Update.swap(src, dst, tag), done)
    case _:
      IO.die(Unit, 2, "usage: update_swap SOURCE INSTALL TAG")

def main() -> IO(Unit):
  IO.bind(List<String>, Unit, AV.Argv.args(), args)
