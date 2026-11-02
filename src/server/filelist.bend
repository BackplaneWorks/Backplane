import Base
import ../core/pathrecords.bend as PR
import ../core/json.bend as J

# Read complete NUL records before applying the existing 3000-path bound.
# perl is available on both supported POSIX platforms; head -z is GNU-only.
def FileList.cmd(repo: Bool) -> String:
  match repo:
    case True{}:
      "git ls-files -z --cached --others --exclude-standard | perl -0ne 'print; last if ++$n == 3000'"
    case False{}:
      "find . -maxdepth 8 -type f -not -path '*/.*' -not -path '*/node_modules/*' -print0 2>/dev/null | perl -0ne 's{^\\./}{}; print; last if ++$n == 3000'"

def FileList.strs(paths: List<&2, String>) -> List<&2, J.Json>:
  match paths:
    case Nil{}: Nil{}
    case p <> rest: J.Str{p} <> FileList.strs(rest)

def FileList.result(text: String) -> J.Json:
  J.Json.arr(FileList.strs(PR.Paths.read(text)))
