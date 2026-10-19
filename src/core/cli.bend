import Base

# The command line, strict. Both mains (src/app/main.bend and
# src/server/main.bend) read it with Cli.of as the very first step, before
# any other IO: an unknown flag, or a flag without its value, used to be
# skipped (server.bend's Main.args), so `backplane --help` or `--version`
# started a second hub on the live home. Now it prints the usage to stderr
# and exits 2; --help/-h prints it and exits 0; --version prints the
# version and exits 0. None of them opens the home, binds a port or starts
# anything. Laws cli_*; test test/cli_test.bend.
#
# The flags a hub takes (every one passed today by scripts/, deploy/,
# test/tools, test/native, the systemd unit and the hub's own restarts):
#   --home DIR --port N --host ADDR --web DIR      (values)
#   --no-tailscale --tailscale --tailscale-https   (tailnet)
#   --headless --foreground                        (the window app's)
# The children the hub runs of its own binary come first and keep their
# own arguments: --review (review.bend), --plot, --plot-tree, --plot-info
# (plot.bend), --render (render.bend).

type Cli is Data:
  CliRun{}
  CliHelp{}
  CliVersion{}
  CliBad{why: String}

# where the walk is: taking flags, waiting for a flag's value, or decided
type CliSt is Data:
  CliWalk{}
  CliNeed{flag: String}
  CliStop{c: Cli}

def Cli.usage() -> String:
  "usage: backplane [--home DIR] [--port N] [--host ADDR] [--web DIR] [--no-tailscale | --tailscale | --tailscale-https] [--headless] [--foreground]\n       backplane --render MODEL... [--size WxH]\n       backplane --help | --version"

# what one flag is, alone: 0 unknown, 1 takes a value, 2 stands alone,
# 3 help, 4 version
def Cli.kind(h: String) -> U32:
  match h:
    case "--home":
      1
    case "--port":
      1
    case "--host":
      1
    case "--web":
      1
    case "--no-tailscale":
      2
    case "--tailscale":
      2
    case "--tailscale-https":
      2
    case "--headless":
      2
    case "--foreground":
      2
    case "--help":
      3
    case "-h":
      3
    case "--version":
      4
    case _:
      0

def Cli.unknown(+h: String) -> String:
  Bool.pick(String, String.starts_with(h, "-"), "unknown flag " ++ h, "unexpected argument " ++ h)

def Cli.step.k(k: U32, +h: String) -> CliSt:
  match k:
    case 1:
      CliNeed{h}
    case 2:
      CliWalk{}
    case 3:
      CliStop{CliHelp{}}
    case 4:
      CliStop{CliVersion{}}
    case _:
      CliStop{CliBad{Cli.unknown(h)}}

def Cli.step(+h: String) -> CliSt:
  Cli.step.k(Cli.kind(h), h)

def Cli.digit(+c: U32) -> Bool:
  Bool.and(U32.is_ge(c, 48), U32.is_le(c, 57))

# every character a digit (and at least one)
def Cli.digits.go(s: String, ok: Bool) -> Bool:
  match s:
    case SNil{}:
      ok
    case SCon{Chr{c}, t}:
      Cli.digits.go(t, Bool.and(ok, Cli.digit(c)))

def Cli.digits(+s: String) -> Bool:
  Bool.and(Bool.not(String.is_empty(s)), Cli.digits.go(s, True{}))

# a port is a number of at most five digits
def Cli.port.ok(+v: String) -> Bool:
  Bool.and(Cli.digits(v), Nat.is_le(String.length(v), 5n))

def Cli.val.port(ok: Bool, +v: String) -> CliSt:
  match ok:
    case True{}:
      CliWalk{}
    case False{}:
      CliStop{CliBad{"--port wants a number, not " ++ v}}

def Cli.val.k(missing: Bool, +f: String, +v: String) -> CliSt:
  match missing:
    case True{}:
      CliStop{CliBad{f ++ " needs a value"}}
    case False{}:
      Bool.pick(CliSt, String.eq(f, "--port"), Cli.val.port(Cli.port.ok(v), v), CliWalk{})

# a flag's value: one that looks like a flag (or is empty) is missing
def Cli.val(+f: String, +v: String) -> CliSt:
  Cli.val.k(Bool.or(String.is_empty(v), String.starts_with(v, "-")), f, v)

# the hub's flags, left to right; the first verdict stops the walk
def Cli.go(xs: List<&2, String>, st: CliSt) -> Cli:
  match xs st:
    case Nil{} CliWalk{}:
      CliRun{}
    case Nil{} CliNeed{f}:
      CliBad{f ++ " needs a value"}
    case Nil{} CliStop{c}:
      c
    case +h <> t CliWalk{}:
      Cli.go(t, Cli.step(h))
    case +h <> t CliNeed{+f}:
      Cli.go(t, Cli.val(f, h))
    case +h <> +t CliStop{c}:
      c

# the children's arguments (a child's own parser reads them)
def Cli.least(+n: Nat, +t: List<&2, String>, +what: String) -> Cli:
  Bool.pick(Cli, Nat.is_ge(List.length(&2, String, t), n), CliRun{}, CliBad{what})

def Cli.review.why() -> String:
  "--review bom OLD NEW | --review checks DIR COMMIT HOME TRIGGER | --review release DIR TAG HOME"

def Cli.review.k(+s: String, +r: List<&2, String>) -> Cli:
  Bool.pick(Cli, String.eq(s, "bom"), Cli.least(2n, r, Cli.review.why()),
    Bool.pick(Cli, String.eq(s, "checks"), Cli.least(4n, r, Cli.review.why()),
      Bool.pick(Cli, String.eq(s, "release"), Cli.least(3n, r, Cli.review.why()), CliBad{Cli.review.why()})))

def Cli.review(t: List<&2, String>) -> Cli:
  match t:
    case Nil{}:
      CliBad{Cli.review.why()}
    case +s <> r:
      Cli.review.k(s, r)

# 0 none, 1 --review, 2 --plot, 3 --plot-tree, 4 --plot-info, 5 --render
def Cli.child(h: String) -> U32:
  match h:
    case "--review":
      1
    case "--plot":
      2
    case "--plot-tree":
      3
    case "--plot-info":
      4
    case "--render":
      5
    case _:
      0

def Cli.first(k: U32, +h: String, t: List<&2, String>) -> Cli:
  match k:
    case 0:
      Cli.go(h <> t, CliWalk{})
    case 1:
      Cli.review(t)
    case 2:
      Cli.least(2n, t, "--plot KIND FILE [INFO]")
    case 3:
      Cli.least(1n, t, "--plot-tree FILE")
    case 4:
      Cli.least(3n, t, "--plot-info FILE SIG INDEX")
    case _:
      CliRun{}

# the decision for the program's arguments (after its name)
def Cli.of(xs: List<&2, String>) -> Cli:
  match xs:
    case Nil{}:
      CliRun{}
    case +h <> t:
      Cli.first(Cli.child(h), h, t)

# whether the program goes on to run (a hub, a window or a child)
def Cli.runs(c: Cli) -> Bool:
  match c:
    case CliRun{}:
      True{}
    case _:
      False{}

def Cli.copy(xs: List<String>) -> List<&2, String>:
  match xs:
    case Nil{}:
      Nil{}
    case x <> t:
      x <> Cli.copy(t)

# Bend 2.0.36's IO.args() starts with the program's name
def Cli.args(xs: List<String>) -> List<&2, String>:
  match xs:
    case Nil{}:
      Nil{}
    case _ <> t:
      Cli.copy(t)
