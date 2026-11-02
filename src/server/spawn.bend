import Base
import ./host.bend as H

# Preserve executable errors when cwd exists; explain directory errors with
# the exact path, including worktrees whose project subdirectory is absent.
def Spawn.why(st: Result<&1, &1, U32 & String, U32 & U32 & U32>, +cwd: String, +msg: String) -> String:
  match st:
    case Done{(1, _)}:
      msg
    case Done{_}:
      "working directory is not a folder: " ++ cwd ++ " (" ++ msg ++ ")"
    case Fail{(2, _)}:
      "working directory does not exist: " ++ cwd ++ " (" ++ msg ++ ")"
    case Fail{(_, reason)}:
      "cannot access working directory: " ++ cwd ++ " (" ++ reason ++ "; " ++ msg ++ ")"

def Spawn.message(e: U32 & String) -> String:
  (_, msg) = e
  msg

def Spawn.explained(st: Result<&1, &1, U32 & String, U32 & U32 & U32>, cwd: String, e: U32 & String) -> Result<&1, &1, U32 & String, U32 & Socket>:
  (code, msg) = e
  Fail{(code, Spawn.why(st, cwd, msg))}

def Spawn.failed(empty: Bool, +cwd: String, e: U32 & String) -> IO(Result<&1, &1, U32 & String, U32 & Socket>):
  match empty:
    case True{}:
      IO.pure(Result<&1, &1, U32 & String, U32 & Socket>, Fail{e})
    case False{}:
      IO.bind(Result<&1, &1, U32 & String, U32 & U32 & U32>, Result<&1, &1, U32 & String, U32 & Socket>, H.FS.stat(cwd), st => IO.pure(Result<&1, &1, U32 & String, U32 & Socket>, Spawn.explained(st, cwd, e)))

def Spawn.checked(r: Result<&1, &1, U32 & String, U32 & Socket>, +cwd: String) -> IO(Result<&1, &1, U32 & String, U32 & Socket>):
  match r:
    case Done{started}:
      IO.pure(Result<&1, &1, U32 & String, U32 & Socket>, Done{started})
    case Fail{e}:
      Spawn.failed(String.is_empty(cwd), cwd, e)
