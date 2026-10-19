import Base
import ../check.bend as C
import ../../src/server/argv.bend as AV
import ../../src/server/host.bend as H
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
  do IO<Unit>:
    # host.c's join helpers need its CBOR buffer in small native targets,
    # as in native/mailstore_test.bend. This opens no socket or hub.
    frame : List<&2, U32> <- H.Json.cbor_frame("null", "", "")
    C.Check.str("host effect linkage frame", Nat.show(List.length(&2, U32, frame)), "3")
    xs : List<String> <- AV.Argv.args()
    args(xs)
