import Base

# What a hub on a copy of a real log (BACKPLANE_COPY=1, core/copy.bend)
# refuses to run: anything that reaches the network or another machine.
# Kept apart from copy.bend so proc.bend and botnet.bend need no hub types.

# whether xs holds s
def Copy.has(xs: List<&2, String>, +s: String) -> Bool:
  match xs:
    case Nil{}:
      False{}
    case x <> t:
      Bool.or(String.eq(x, s), Copy.has(t, s))

# commands that reach the network (or another machine)
def Copy.net.git(+args: List<&2, String>) -> Bool:
  Bool.or(Bool.or(Copy.has(args, "push"), Copy.has(args, "fetch")),
    Bool.or(Bool.or(Copy.has(args, "pull"), Copy.has(args, "clone")), Copy.has(args, "ls-remote")))

def Copy.net.k(git: Bool, +cmd: String, +args: List<&2, String>) -> Bool:
  match git:
    case True{}:
      Copy.net.git(args)
    case False{}:
      Copy.has(["curl", "wget", "gh", "ssh", "scp", "rsync", "tailscale", "nc"], cmd)

def Copy.net(+cmd: String, +args: List<&2, String>) -> Bool:
  Copy.net.k(String.eq(cmd, "git"), cmd, args)

# whether a copy hub refuses to run cmd: a network command, or git's
def Copy.refused(copy: Bool, +cmd: String, +args: List<&2, String>) -> Bool:
  match copy:
    case False{}:
      False{}
    case True{}:
      Copy.net(cmd, args)

def Copy.refusal(+cmd: String) -> String:
  "backplane: a hub on a copied log (BACKPLANE_COPY=1) does not run " ++ cmd

