import Base
import ./check.bend as C
import ../src/core/cli.bend as CL
import ../src/core/onehub.bend as OH
import ../src/core/standin.bend as SI
import ../src/core/clipath.bend as CPA

def say(c: CL.Cli) -> String:
  match c:
    case CL.CliRun{}:
      "run"
    case CL.CliHelp{}:
      "help"
    case CL.CliVersion{}:
      "version"
    case CL.CliBad{why}:
      "bad: " ++ why

def got(g: OH.HubGot) -> String:
  match g:
    case OH.HubOurs{}:
      "ours"
    case OH.HubHeld{p, n}:
      "held " ++ p ++ "/" ++ n
    case OH.HubFail{w}:
      "fail " ++ w

def of(xs: List<&2, String>) -> String:
  say(CL.Cli.of(xs))

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("no flags runs", of(Nil{}), "run")
    C.Check.str("the test hubs' flags run", of(["--home", "/tmp/h", "--port", "3799", "--no-tailscale", "--foreground"]), "run")
    C.Check.str("every hub flag runs", of(["--web", "dist/web", "--host", "0.0.0.0", "--tailscale", "--tailscale-https", "--headless"]), "run")
    C.Check.str("--help", of(["--help"]), "help")
    C.Check.str("-h", of(["-h"]), "help")
    C.Check.str("--version", of(["--version"]), "version")
    C.Check.str("--version after flags", of(["--home", "/tmp/h", "--version"]), "version")
    C.Check.str("--help wins over what follows", of(["--help", "--bogus"]), "help")
    C.Check.str("an unknown flag", of(["--bogus"]), "bad: unknown flag --bogus")
    C.Check.str("an unknown flag after good ones", of(["--home", "/tmp/h", "--bogus"]), "bad: unknown flag --bogus")
    C.Check.str("an unknown flag before --version", of(["--bogus", "--version"]), "bad: unknown flag --bogus")
    C.Check.str("a single dash flag", of(["-v"]), "bad: unknown flag -v")
    C.Check.str("a word", of(["serve"]), "bad: unexpected argument serve")
    C.Check.str("a missing value", of(["--home"]), "bad: --home needs a value")
    C.Check.str("a flag for a value", of(["--home", "--version"]), "bad: --home needs a value")
    C.Check.str("an empty value", of(["--web", ""]), "bad: --web needs a value")
    C.Check.str("a port that is no number", of(["--port", "x1"]), "bad: --port wants a number, not x1")
    C.Check.str("a port too long", of(["--port", "123456"]), "bad: --port wants a number, not 123456")
    C.Check.str("--render", of(["--render", "a.step", "--size", "800x600"]), "run")
    C.Check.str("--render alone (render's usage)", of(["--render"]), "run")
    C.Check.str("--plot", of(["--plot", "1", "/a.kicad_pcb", "/i"]), "run")
    C.Check.str("--plot two", of(["--plot", "1", "/a.kicad_pcb"]), "run")
    C.Check.str("--plot short", of(["--plot", "1"]), "bad: --plot KIND FILE [INFO]")
    C.Check.str("--plot-tree", of(["--plot-tree", "/a.kicad_sch"]), "run")
    C.Check.str("--plot-tree short", of(["--plot-tree"]), "bad: --plot-tree FILE")
    C.Check.str("--plot-info", of(["--plot-info", "/a", "1", "2"]), "run")
    C.Check.str("--plot-info short", of(["--plot-info", "/a", "1"]), "bad: --plot-info FILE SIG INDEX")
    C.Check.str("--review bom", of(["--review", "bom", "", "/s.kicad_sch"]), "run")
    C.Check.str("--review checks", of(["--review", "checks", "/d", "c", "/h", "t"]), "run")
    C.Check.str("--review release", of(["--review", "release", "/d", "v1", "/h"]), "run")
    C.Check.str("--review short", of(["--review", "bom", "x"]), "bad: " ++ CL.Cli.review.why())
    C.Check.str("--review other", of(["--review", "x", "a", "b", "c", "d"]), "bad: " ++ CL.Cli.review.why())
    C.Check.str("--review alone", of(["--review"]), "bad: " ++ CL.Cli.review.why())
    C.Check.str("a child flag later is unknown", of(["--home", "/h", "--plot", "1", "f"]), "bad: unknown flag --plot")
    C.Check.str("runs", Bool.show(CL.Cli.runs(CL.Cli.of(["--bogus"]))), "False")
    C.Check.str("args drop the name", String.join(CL.Cli.args(["backplane", "--home", "/h"]), " "), "--home /h")
    C.Check.str("lock ours", got(OH.OneHub.got("ok")), "ours")
    C.Check.str("lock held", got(OH.OneHub.got("held 12 3799")), "held 12/3799")
    C.Check.str("lock held, file empty", got(OH.OneHub.got("held ")), "held ?/?")
    C.Check.str("lock failed", got(OH.OneHub.got("fail Permission denied")), "fail Permission denied")
    C.Check.str("lock answer unknown", got(OH.OneHub.got("what")), "fail what")
    C.Check.str("second hub says", OH.OneHub.says("/h", "12", "3799"), "a hub already serves /h (pid 12, port 3799)")
    C.Check.str("holder", OH.OneHub.holder(OH.OneHub.got("held 12 3799")), "12 3799")
    C.Check.str("holder ours", OH.OneHub.holder(OH.OneHub.got("ok")), "")
    C.Check.str("stand-ins first on PATH", SI.Standin.path("/f", "/b", "/usr/bin"), "/f:/b:/usr/bin")
    C.Check.str("stand-ins, no PATH", SI.Standin.path("/f", "/b", ""), "/f:/b")
    C.Check.str("the login PATH merges in behind the stand-ins", CPA.CliPath.merge(SI.Standin.path("/f", "/b", "/usr/bin"), "/u/.local/bin:/usr/bin", ["/u/.local/bin"]), "/f:/b:/usr/bin:/u/.local/bin")
    C.Check.str("stand-in provider list blocks OMP fallback", String.join(SI.Standin.names(), " "), "claude codex grok muse omp")
