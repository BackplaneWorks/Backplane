import Base
import ./check.bend as C
import ../src/core/model.bend as M
import ../src/core/omp.bend as OMP

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("omp approval supervised", OMP.Omp.approval(M.Supervised{}), "always-ask")
    C.Check.str("omp approval accept edits", OMP.Omp.approval(M.AcceptEdits{}), "write")
    C.Check.str("omp approval auto", OMP.Omp.approval(M.AutoMode{}), "write")
    C.Check.str("omp approval full access", OMP.Omp.approval(M.FullAccess{}), "yolo")
    C.Check.str("omp args", String.join(OMP.Omp.args(M.Supervised{}), " "), "acp --approval-mode always-ask")
    C.Check.str("omp levels", String.join(OMP.Omp.levels(), ","), "off,auto,low,medium,high,xhigh,max")
    C.Check.str("omp models keeps chat only", String.join(OMP.Omp.models("{\"models\":[{\"kind\":\"chat\",\"selector\":\"a/b\"},{\"kind\":\"embedding\",\"selector\":\"c/d\"}]}"), ","), "a/b")
    C.Check.str("omp models bad json", String.join(OMP.Omp.models("nope"), ","), "")
    C.Check.str("omp hint auth", OMP.Omp.hint("Not authenticated"), "Not authenticated (sign in once with `omp login`)")
    C.Check.str("omp hint login", OMP.Omp.hint("login required"), "login required (sign in once with `omp login`)")
    C.Check.str("omp hint plain", OMP.Omp.hint("tool failed"), "tool failed")
    C.Check.str("omp install", OMP.Omp.install(), "Install oh-my-pi with `curl -fsSL https://omp.sh/install | sh` or `bun install -g @oh-my-pi/pi-coding-agent`, then sign in with `omp login`.")
