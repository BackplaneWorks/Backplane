import Base

# Stand-in agents never fall back to real ones. A test hub is given its
# stand-ins as a folder, BACKPLANE_AGENTS=<dir>. The hub then puts <dir>
# first on PATH and a folder of blocks second (<home>/standin-blocks): for
# each provider CLI <dir> has no runnable file for, a block that says so on
# stderr and exits 127. A turn on it fails with that message (the agent's
# stderr is its failure, server.bend's Proc.errtail), and the real CLI
# later on PATH (~/.local/bin, and the login shell's PATH that
# core/clipath.bend merges in after the hub's own) is never reached. A
# <dir> that is not a folder stops the hub at start. The CLIs present
# (server.bend's Models.present) do not count a block. Laws standin_*;
# test test/cli_test.bend, end to end bun test/tools/onehub_e2e.ts.

# the CLIs looked up on PATH (Antigravity runs from <home>/antigravity)
def Standin.names() -> List<&2, String>:
  ["claude", "codex", "grok", "muse", "omp"]

def Standin.mark() -> String:
  "BACKPLANE-STANDIN-MISSING"

def Standin.blocks(+home: String) -> String:
  home ++ "/standin-blocks"

# $1 the stand-ins' folder, $2 the blocks' folder, $3 the mark, then the
# CLIs: makes the blocks and prints the stand-ins' folder as an absolute
# path; exits 1 with why when it cannot
def Standin.sh() -> String:
  "d=$1; b=$2; m=$3; shift 3; [ -d \"$d\" ] || { echo \"BACKPLANE_AGENTS=$d is not a folder\"; exit 1; }; "
    ++ "d=$(cd \"$d\" && pwd -P) || exit 1; rm -rf \"$b\"; mkdir -p \"$b\" || { echo \"cannot make $b\"; exit 1; }; "
    ++ "for c in \"$@\"; do [ -f \"$d/$c\" ] && [ -x \"$d/$c\" ] && continue; "
    ++ "printf '#!/bin/sh\\n# %s\\necho \"backplane: %s has no runnable %s (BACKPLANE_AGENTS); the %s on PATH is never used\" >&2\\nexit 127\\n' \"$m\" \"$d\" \"$c\" \"$c\" > \"$b/$c\" && chmod 755 \"$b/$c\" || { echo \"cannot write $b/$c\"; exit 1; }; "
    ++ "done; printf '%s' \"$d\""

def Standin.args(+dir: String, +blocks: String) -> List<&2, String>:
  List.append(&2, String, ["-c", Standin.sh(), "sh", dir, blocks, Standin.mark()], Standin.names())

# the hub's PATH: the stand-ins, the blocks, then what it had
def Standin.path(+dir: String, +blocks: String, +cur: String) -> String:
  dir ++ ":" ++ blocks ++ Bool.pick(String, String.is_empty(cur), "", ":" ++ cur)

# which of the CLIs are on PATH, "claude codex " style; a block is none
def Standin.present.sh() -> String:
  "for c in " ++ String.join(Standin.names(), " ") ++ "; do p=$(command -v $c 2>/dev/null) || continue; head -c 256 \"$p\" 2>/dev/null | grep -q "
    ++ Standin.mark() ++ " && continue; printf '%s ' $c; done; true"
