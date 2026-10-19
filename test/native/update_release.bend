import Base
import ../../src/server/update.bend as UP
import ../../src/server/host.bend as H

# Compile the actual public updater installer path; tests use temp prefixes.
def run(xs: List<String>) -> IO(Unit):
  match xs:
    case _ <> src <> prefix <> Nil{}:
      IO.bind(String, Unit, UP.Update.swap(src, prefix, "fixture"), err => IO.print(err))
    case _:
      IO.print("usage: update_release PACKAGE PREFIX")

def main() -> IO(Unit):
  do IO<Unit>:
    # Include the CBOR effect used by the real hub. Its shared host.c helpers
    # need that feature enabled even in this smaller native updater harness.
    frame : List<&2, U32> <- H.Json.cbor_frame("{}", "", "")
    IO.bind(List<String>, Unit, IO.args(), run)
