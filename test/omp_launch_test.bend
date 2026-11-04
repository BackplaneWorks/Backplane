import Base
import ../src/server/omp-launch.bend as O
import ../src/core/json.bend as J

# The driver checks the exact production argv against the native smoke's
# contract, including optional model/effort and spaces/Unicode in file paths.
def jsons(xs: List<&2, String>) -> List<&2, J.Json>:
  match xs:
    case Nil{}: Nil{}
    case x <> rest: J.Str{x} <> jsons(rest)

def args(mcp: String, mode: String, sessions: String, files: String, model: String, effort: String) -> String:
  J.Json.show(J.Json.arr(jsons(O.OmpLaunch.args(mcp, mode, sessions, files, model, effort))))

def why(code: U32, msg: String) -> String:
  O.OmpLaunch.why(code, msg)
