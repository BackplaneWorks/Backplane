import Base
import ../../src/server/kicad.bend as K
import ../../src/core/json.bend as J
import ../../src/core/kicad-project.bend as KP

def env(name: String) -> IO(String):
  IO.bind(Result<&1, &1, U32 & String, String>, String, IO.get_env(name), r => IO.pure(String, Result.default(&1, &1, U32 & String, String, r, "")))

def files(f: K.Files) -> J.Json:
  match f:
    case K.Files{p, b, s, _}: J.Json.obj([J.KV{"project", J.Str{p}}, J.KV{"pcb", J.Str{b}}, J.KV{"schematic", J.Str{s}}])

def main() -> IO(Unit):
  do IO<Unit>:
    +cwd : String <- env("TEST_PROJECT_ROOT")
    text : String <- env("TEST_PROJECT_ARGS")
    +args : J.Json = Maybe.default(&2, J.Json, J.Json.parse(text), J.Null{})
    +root : String <- K.Files.root(cwd)
    +main : K.Files <- K.Files.read(cwd)
    +local : K.Files <- K.Files.read.local(cwd)
    +changed : Bool <- K.Files.changed(main, local)
    guard : String <- K.Files.write_guard(cwd, args, J.Json.get_str(args, "type", ""))
    resolved : J.Json <- K.Files.request(cwd, args, J.Json.get_str(args, "kind", "project"))
    IO.print(J.Json.show(J.Json.obj([J.KV{"root", J.Str{root}}, J.KV{"main", files(main)}, J.KV{"local", files(local)}, J.KV{"changed", J.Flag{changed}},
      J.KV{"prompt", J.Flag{KP.Project.choose(J.Json.get_str(args, "choice", ""), changed)}}, J.KV{"write_error", J.Str{guard}}, J.KV{"request", resolved}])))
