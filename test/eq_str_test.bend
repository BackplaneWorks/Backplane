import Base
import ./check.bend as C
import ../src/core/eq.bend as Q

# Eq.str over strings as long as a board's plot chunks: the hub compares a
# chunk's data with what a client holds, and a frame per character faulted
# the hub's 64 MiB stack on macOS whenever a board view opened.

# n copies of c in front of acc, one tail call each
def run(n: Nat, +c: Char, acc: String) -> String:
  match n:
    case 0n:
      acc
    case 1n+k:
      run(k, c, SCon{c, acc})

def yes(b: Bool) -> String:
  match b:
    case True{}:
      "true"
    case False{}:
      "false"

def main() -> IO(Unit):
  do IO<Unit>:
    +long : String = run(2000000n, 'a', "")
    C.Check.str("two million equal characters", yes(Q.Eq.str(long, run(2000000n, 'a', ""))), "true")
    C.Check.str("the last character differs", yes(Q.Eq.str(long, run(1999999n, 'a', "b"))), "false")
    C.Check.str("one is longer", yes(Q.Eq.str(long, run(2000001n, 'a', ""))), "false")
    C.Check.str("the first character differs", yes(Q.Eq.str(SCon{'b', long}, SCon{'a', long})), "false")
    C.Check.str("empty", yes(Q.Eq.str("", "")), "true")
    C.Check.str("short", yes(Q.Eq.str("ab", "ab")), "true")
    C.Check.str("short unequal", yes(Q.Eq.str("ab", "ac")), "false")
