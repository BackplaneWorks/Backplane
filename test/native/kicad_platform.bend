import Base
import ../../src/server/argv.bend as AV
import ../../src/server/host.bend as H
import ../../src/server/proc.bend as P
import ../../src/server/kicad.bend as K
import ../../src/core/json.bend as J

# The production runtime selector and transaction, with no hub or providers.
# Fixture digests are arguments only in this test entry point.
def ran(r: P.Ran) -> IO(Unit):
  match r:
    case P.Ran{0, out}: IO.print(out)
    case P.Ran{_, out}: IO.die(Unit, 1, out)

def installed(error: String) -> IO(Unit):
  match error:
    case "": IO.print("installed")
    case _: IO.die(Unit, 1, error)

def args(xs: List<String>) -> IO(Unit):
  match xs:
    case "discover" <> data <> exe <> Nil{}:
      IO.bind(K.Rt, Unit, K.Rt.discover("", "", exe, data), rt => IO.print(J.Json.show(K.Rt.json(rt))))
    case "refresh" <> data <> exe <> Nil{}:
      IO.bind(K.Rt, Unit, K.Rt.refresh(data, "", exe, True{}, K.Rt.empty()), rt => IO.print(J.Json.show(K.Rt.json(rt))))
    case "boot" <> data <> exe <> Nil{}:
      IO.bind(K.Rt, Unit, K.Rt.boot(data, "", exe), rt => IO.print(J.Json.show(K.Rt.json(rt))))
    case "resolve" <> data <> exe <> Nil{}:
      do IO<Unit>:
        explicit : String <- K.Rt.env("BACKPLANE_KICAD_CLI")
        rt : K.Rt <- K.Rt.resolve(explicit, exe, data)
        IO.print(J.Json.show(K.Rt.json(rt)))
    case "install" <> data <> Nil{}:
      IO.bind(String, Unit, K.KiCad.install(data), IO.print)
    case "transaction" <> dir <> base <> name <> sha <> ver <> Nil{}:
      IO.bind(String, Unit, K.KiCad.install.transaction(dir, base, name, sha, ver, "linux-x86_64"), installed)
    case _:
      IO.die(Unit, 2, "usage: kicad_platform discover|refresh|boot|resolve DATA EXE | install DATA | transaction DIR BASE NAME SHA VERSION")

def main() -> IO(Unit):
  do IO<Unit>:
    frame : List<&2, U32> <- H.Json.cbor_frame("null", "", "")
    xs : List<String> <- AV.Argv.args()
    args(xs)
