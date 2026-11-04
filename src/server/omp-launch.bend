import Base

# Prompt contents never enter argv (CreateProcessW has a 32,767 UTF-16
# command-line limit). The caller masks them before preparing private,
# independent files, and keeps those files until the turn has exited.
def OmpLaunch.prepare(home: String, prompt: String, extension: String) -> IO(Result<&1, &1, U32 & String, String>):
  import "./effects/omp-launch.js"

def OmpLaunch.remove(dir: String) -> IO(Unit):
  import "./effects/omp-launch.js"

def OmpLaunch.why(code: U32, msg: String) -> String:
  match code:
    case 2:
      "Oh My Pi CLI or a required launch path was not found (ENOENT); check omp on the hub's PATH and its working directory"
    case 5:
      "Oh My Pi launch failed (EIO); check its executable and permissions on the working directory and private launch files"
    case _:
      msg

def OmpLaunch.opt(+flag: String, +value: String) -> List<&2, String>:
  Bool.pick(List<&2, String>, String.is_empty(value), Nil{}, [flag, value])

def OmpLaunch.args(+mcp: String, +mode: String, +sessions: String, +files: String, +model: String, +effort: String) -> List<&2, String>:
  List.concat(&2, String, [["BACKPLANE_OMP_MCP=" ++ mcp, "omp", "--mode", "rpc-ui", "--approval-mode", mode,
    "--session-dir", sessions, "--continue", "-e", files ++ "/backplane.js", "--append-system-prompt", files ++ "/system-prompt.txt"],
    OmpLaunch.opt("--model", model), OmpLaunch.opt("--thinking", effort)])
