import Base

# A Linux package builds the app once and installs the same executable at
# both names. Only this exact basename selects the server's entry point.
# Inspect argv[0], rather than /proc/self/exe: callers may use PATH, and a
# running executable may have been replaced before it restarts.
def Exe.serve(+name: String) -> Bool:
  Bool.or(String.eq(name, "backplane-serve"), String.ends_with(name, "/backplane-serve"))

def Exe.server(args: List<String>) -> Bool:
  match args:
    case Nil{}:
      False{}
    case name <> _:
      Exe.serve(name)

# Where the hub's own binary is. Linux reports /proc/self/exe as
# "<path> (deleted)" once an install or update has replaced the file by
# rename while the hub keeps running.

# the binary was replaced under the running hub
def Exe.gone(+raw: String) -> Bool:
  String.ends_with(raw, " (deleted)")

def Exe.path.k(gone: Bool, +raw: String) -> String:
  match gone:
    case True{}:
      String.take(raw, Nat.sub(String.length(raw), 10n))
    case False{}:
      raw

# the installed binary's path: raw without a trailing " (deleted)"
def Exe.path(+raw: String) -> String:
  Exe.path.k(Exe.gone(raw), raw)

def Exe.child.k(gone: Bool, raw: String) -> String:
  match gone:
    case True{}:
      "/proc/self/exe"
    case False{}:
      raw

# the program a child of this same build runs: a replaced binary is still run
# through /proc/self/exe, so the hub and its child stay one version
def Exe.child(+raw: String) -> String:
  Exe.child.k(Exe.gone(raw), raw)
