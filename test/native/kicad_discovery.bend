import Base
import ../../src/server/kicad.bend as K
import ../../src/core/json.bend as J
import ../check.bend as C

def chosen(+root: String, +cli: String, source: String) -> IO(Unit):
  do IO<Unit>:
    +rt : K.Rt <- K.Rt.discover("", "", root ++ "/app", root ++ "/home")
    C.Check.str("filesystem selected CLI", K.Rt.cli(rt), cli)
    C.Check.str("filesystem selected source", J.Json.get_str(K.Rt.health(rt), "source", ""), source)

def main() -> IO(Unit):
  do IO<Unit>:
    +root : String <- K.Rt.env("TEST_KICAD_ROOT")
    +wanted : String <- K.Rt.env("TEST_KICAD_EXPECTED")
    source : String <- K.Rt.env("TEST_KICAD_SOURCE")
    +old : String <- K.Rt.executable(root ++ "/old/bin/kicad-cli")
    C.Check.str("filesystem exe suffix lookup", old, root ++ "/old/bin/kicad-cli.exe")
    missing : String <- K.Rt.executable(root ++ "/absent/bin/kicad-cli")
    C.Check.str("missing stays explicit", missing, root ++ "/absent/bin/kicad-cli")
    posix : String <- K.Rt.executable(root ++ "/posix/bin/kicad-cli")
    C.Check.str("unsuffixed CLI takes precedence", posix, root ++ "/posix/bin/kicad-cli")
    chosen(root, wanted, source)
