import Base
import ../../src/server/kicad.bend as K
import ../../src/core/json.bend as J
import ../check.bend as C

def main() -> IO(Unit):
  do IO<Unit>:
    +home : String <- K.Rt.env("TEST_KICAD_CACHE_HOME")
    +setting : String <- K.Rt.env("TEST_KICAD_CACHE_SETTING")
    +rt : K.Rt = K.Rt.set(K.Rt{"C:/Program Files/KiCad/bin/kicad-cli.exe", True{}, "10.99.0-bp.6", "fixture", J.Null{}}, "setting", J.Str{setting})
    +expected : String = J.Json.show(K.Rt.json(rt))
    K.Rt.save(home, rt)
    +actual : String <- K.Io.text(home ++ "/kicad-health.json")
    C.Check.str("native saved JSON bytes", actual, expected)
    C.Check.str("native saved Windows setting", J.Json.get_str(Maybe.default(&2, J.Json, J.Json.parse(actual), J.Null{}), "setting", "missing"), setting)
