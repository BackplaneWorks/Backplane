import Base
import ../src/core/folder.bend as F
import ../src/server/spawn.bend as S
import ./check.bend as C

# The driver executes these production scripts with literal path arguments.
def path_script() -> String:
  "set -e; " ++ F.Folder.path.sh() ++ "printf '%s\\n' \"$d\""

def resolve_script() -> String:
  "set -e; " ++ F.Folder.path.sh() ++ F.Folder.dir.sh() ++ F.Folder.native.sh() ++ "printf '%s\\n' \"$d\""

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("drive root slash survives", F.Folder.root.fin("C:"), "C:/")
    C.Check.str("lowercase drive root slash survives", F.Folder.root.fin("c:"), "c:/")
    C.Check.str("POSIX root survives", F.Folder.root.fin(""), "/")
    C.Check.str("UNC share stays absolute", F.Folder.root.fin("//server/share"), "//server/share")
    C.Check.str("missing cwd names the directory", S.Spawn.why(Fail{(2, "No such file or directory")}, "/tmp/branch only", "No such file or directory"), "working directory does not exist: /tmp/branch only (No such file or directory)")
    C.Check.str("existing cwd preserves executable error", S.Spawn.why(Done{(1, (0, 0))}, "/tmp", "No such file or directory"), "No such file or directory")
    C.Check.str("file cwd names the file", S.Spawn.why(Done{(0, (0, 0))}, "/tmp/a.txt", "Not a directory"), "working directory is not a folder: /tmp/a.txt (Not a directory)")
    C.Check.str("inaccessible cwd is not called missing", S.Spawn.why(Fail{(13, "Permission denied")}, "/private", "Permission denied"), "cannot access working directory: /private (Permission denied; Permission denied)")
