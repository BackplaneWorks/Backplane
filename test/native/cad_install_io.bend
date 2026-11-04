import Base
import ../../src/server/jsloop.bend as LOOP
import ../../src/server/cad-install.bend as FX
import ../../src/core/cad-install.bend as CI
import ../../src/core/json.bend as J
import ../check.bend as C

def env(name: String) -> IO(String):
  do IO<String>:
    r : Result<&1, &1, U32 & String, String> <- IO.get_env(name)
    return Result.default(&1, &1, U32 & String, String, r, "")

def watch(fuel: Nat, busy: Bool, +home: String, +attempt: String, +phase: String, +wanted: String) -> IO(Unit):
  match fuel busy:
    case 0n _: C.Check.str("compiled install terminated", "timeout", wanted)
    case 1n+rest False{}: C.Check.str("compiled install terminated", phase, wanted)
    case 1n+rest True{}:
      do IO<Unit>:
        +text : String <- FX.CadFx.next(home, "kicad", attempt)
        +next : String = J.Json.get_str(Maybe.default(&2, J.Json, J.Json.parse(text), J.Null{}), "phase", "failed")
        watch(rest, CI.Cad.busy(CI.Cad.phase(next)), home, attempt, next, wanted)

def cancel(yes: Bool, home: String, attempt: String) -> IO(Unit):
  match yes:
    case True{}: FX.CadFx.cancel(home, "kicad", attempt)
    case False{}: IO.pure(Unit, Unit{})

def main() -> IO(Unit):
  do IO<Unit>:
    loop : String <- LOOP.Rt.loop()
    C.Check.str("production async loop", loop, "bun")
    +home : String <- env("TEST_CAD_HOME")
    spec : String <- env("TEST_CAD_SPEC")
    +wanted : String <- env("TEST_CAD_WANTED")
    reserved : Bool <- FX.CadFx.reserve(home, "kicad", "1")
    C.Check.str("compiled immediate reserve", Bool.show(reserved), "True")
    +attempt : String <- FX.CadFx.start(home, "kicad", "1", spec)
    text : String <- FX.CadFx.next(home, "kicad", attempt)
    cancel(String.eq(wanted, "cancelled"), home, attempt)
    watch(60n, True{}, home, attempt, "download", wanted)
