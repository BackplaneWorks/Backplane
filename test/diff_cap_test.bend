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

# a commit adding 160 one-line files, and one renaming a file and changing
# a line among 160 changed files, with a name that looks like a pathspec
def setup.many() -> String:
  "d=$(mktemp -d) && cd \"$d\" && git init -q -b main r && cd r && git -c user.email=t@t -c user.name=t commit -q --allow-empty -m one && for i in $(seq 1 160); do echo $i > f$i.txt; done && git add . && git -c user.email=t@t -c user.name=t commit -q -m two && seq 1 50 > '*a b.txt' && git add . && git -c user.email=t@t -c user.name=t commit -q -m three && git mv '*a b.txt' 'a0 moved.txt' && echo 51 >> 'a0 moved.txt' && for i in $(seq 1 160); do echo x >> f$i.txt; done && git add . && git -c user.email=t@t -c user.name=t commit -q -m four && echo \"$d/r\""

# the same diff through the whole way (the cap over git's full output)
def whole(+root: String, +a: String, +b: String) -> IO(P.Ran):
  P.Proc.run("sh", ["-c", G.Git.safe.sh() ++ "bp_safe; git -c core.quotepath=false diff --no-color --no-ext-diff --no-textconv --src-prefix=a/ --dst-prefix=b/ -M \"$0\" \"$1\" -- | " ++ G.Git.diff.cap.awk.sh(), a, b], root)

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
    many : P.Ran <- P.Proc.run("sh", ["-c", setup.many()], "/")
    +root2 : String = String.trim(out(many))
    +m : P.Ran <- G.Git.run.capped(root2, G.Git.diff.args("HEAD~3", "HEAD~2"))
    C.Check.str("past 150 files they are counted, not listed", Nat.show(List.length(&2, D.DFile, D.Diff.parse(out(m)))) ++ " " ++ Bool.show(String.contains(out(m), "\\ 10 more changed files not shown here")), "150 True")
    +w : P.Ran <- whole(root2, "HEAD~3", "HEAD~2")
    C.Check.str("and exactly as the whole way said it", Bool.show(String.eq(out(m), out(w))), "True")
    +mv : P.Ran <- G.Git.run.capped(root2, G.Git.diff.args("HEAD~1", "HEAD"))
    +mvw : P.Ran <- whole(root2, "HEAD~1", "HEAD")
    C.Check.str("a rename among 161 changes is still a rename", Bool.show(String.contains(out(mv), "rename from *a b.txt\nrename to a0 moved.txt\n")), "True")
    C.Check.str("and the files kept are as the whole way's (length, count)", Nat.show(String.length(out(mv))) ++ " " ++ Bool.show(String.contains(out(mv), "\\ 11 more changed files not shown here")), Nat.show(String.length(out(mvw))) ++ " True")
    gone2 : P.Ran <- P.Proc.run("sh", ["-c", "rm -rf \"$(dirname \"$0\")\"", root2], "/")
    gone : P.Ran <- P.Proc.run("sh", ["-c", "rm -rf \"$(dirname \"$0\")\"", root], "/")
    C.Check.str("cleaned", code(gone), "0")
