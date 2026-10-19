import Base
import ../server/telemetry.bend as TI
import ./server.bend as S
import ../core/prompt.bend as Q
import ../core/cli.bend as CL
import ./plot.bend as PV
import ./review.bend as RVC

# `backplane-serve`: the hub and the web server, no window.

def Args.back(xs: List<&2, String>) -> List<String>:
  match xs:
    case Nil{}:
      Nil{}
    case x <> t:
      x <> Args.back(t)

def start.k(+held: String, +user: String, +exe: String, +home: String, +xs: List<&2, String>) -> IO(Unit):
  do IO<Unit>:
    S.Lock.refuse(String.is_empty(held), held, home)
    b : S.Boot <- S.Main.prepare(user, exe)
    S.Main.boot(S.Main.config(user, exe, Args.back(xs), b), True{}, i => IO.pure(Unit, Unit{}))

# the home's lock is taken before anything else is found: a second hub on
# a home ends at once (server.bend's Lock.*)
def start(args: List<String>) -> IO(Unit):
  do IO<Unit>:
    +xs : List<&2, String> = S.Names.copy(args)
    +user : String <- S.Env.get("HOME", "/tmp")
    +exe : String <- S.Sys.exe_dir.io()
    +c0 : S.Cfg = S.Main.where(user, Args.back(xs))
    +held : String <- S.Lock.take(S.Cfg.home(c0), S.Cfg.port(c0))
    start.k(held, user, exe, S.Cfg.home(c0), xs)

def main.go(u: Unit) -> IO(Unit):
  do IO<Unit>:
    args : List<String> <- IO.args()
    RVC.Rvc.main(args, a2 => TI.Tio.main(a2, a0 => PV.Pv.main(a0, a => start(a))))

# (`--plot KIND FILE` is the hub's own board reader, src/server/plot.bend;
# `--review bom OLD NEW` design review's, src/server/review.bend)
# The command line is read strictly before anything else (core/cli.bend):
# --help, --version or a bad flag never reach a hub.
def main() -> IO(Unit):
  do IO<Unit>:
    a0 : List<String> <- IO.args()
    S.Main.cli(CL.Cli.of(CL.Cli.args(a0)), u => main.go(u))
