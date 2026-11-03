import Base
import ./check.bend as C
import ../src/server/git.bend as G
import ../src/server/proc.bend as P
import ../src/core/diff.bend as D

# The diffs the hub hands on are capped (git.bend's Git.diff.cap): a file
# whose changes are too big keeps its header and one meta line with its
# counts, so Diff still lists it. A thread that regenerated STEP models had
# a 165 MB diff; the hub grew to 12 GB putting it in one reply.

def out(r: P.Ran) -> String:
  match r:
    case P.Ran{_, o}: o

def code(r: P.Ran) -> String:
  match r:
    case P.Ran{c, _}: U32.show(c)

# a repository whose last commit changes a line of small.txt and adds a
# 20000-line big.txt (~400 KB)
def setup() -> String:
  "d=$(mktemp -d) && cd \"$d\" && git init -q -b main r && cd r && printf 'a\\nb\\nc\\n' > small.txt && git add . && git -c user.email=t@t -c user.name=t commit -q -m one && printf 'a\\nB\\nc\\n' > small.txt && seq 1 20000 | sed 's/^/line number /' > big.txt && git add . && git -c user.email=t@t -c user.name=t commit -q -m two && echo \"$d/r\""

def names(fs: List<&2, D.DFile>) -> String:
  match fs:
    case Nil{}: ""
    case D.DFile{_, n, st, _} <> t: n ++ ":" ++ st ++ " " ++ names(t)

def main() -> IO(Unit):
  do IO<Unit>:
    made : P.Ran <- P.Proc.run("sh", ["-c", setup()], "/")
    +root : String = String.trim(out(made))
    +r : P.Ran <- G.Git.run.capped(root, G.Git.diff.args("HEAD~1", "HEAD"))
    +d : String = out(r)
    C.Check.str("git's exit status", code(r), "0")
    C.Check.str("a small file's change is shown", Bool.show(String.contains(d, "-b\n+B\n")), "True")
    C.Check.str("a big file's lines are not", Bool.show(String.contains(d, "+line number 19999")), "False")
    C.Check.str("it says how big instead", Bool.show(String.contains(d, "\n\\ Too big to show here: +20000 -0 lines\n")), "True")
    C.Check.str("the diff stays small", Bool.show(Nat.is_lt(String.length(d), 4000n)), "True")
    C.Check.str("Diff still lists both files", names(D.Diff.parse(d)), "big.txt:added small.txt:modified ")
    +bad : P.Ran <- G.Git.run.capped(root, G.Git.diff.args("nosuchref", "HEAD"))
    C.Check.str("a failing git still fails", Bool.show(Bool.not(String.eq(code(bad), "0"))), "True")
    gone : P.Ran <- P.Proc.run("sh", ["-c", "rm -rf \"$(dirname \"$0\")\"", root], "/")
    C.Check.str("cleaned", code(gone), "0")
