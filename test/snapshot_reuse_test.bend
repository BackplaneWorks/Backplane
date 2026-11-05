import Base
import ./check.bend as C
import ../src/server/git.bend as G
import ../src/server/proc.bend as P

# A snapshot (git.bend's Git.snapshot: the work tree as a tree, through the
# thread's private index) reuses the index its last add -A left while the
# real index and the untracked files are as they were, so only changed
# files are read again. Every answer must equal a fresh seed's: after an
# edit of the same size in the same second, a new untracked file, a newly
# ignored one, and a revert.

def out(r: P.Ran) -> String:
  match r:
    case P.Ran{_, o}: String.trim(o)

def sh(+root: String, +cmd: String) -> IO(String):
  IO.bind(P.Ran, String, P.Proc.run("sh", ["-c", cmd], root), r => IO.pure(String, out(r)))

def setup() -> String:
  "d=$(mktemp -d) && cd \"$d\" && git init -q -b main r && cd r && printf 'one\\n' > a.txt && printf '*.log\\n' > .gitignore && git add . && git -c user.email=t@t -c user.name=t commit -q -m one && echo \"$d/r\""

# the same snapshot from a fresh seed (the private index and its key gone)
def fresh(+root: String) -> IO(String):
  do IO<String>:
    gone : String <- sh(root, "rm -f .git/backplane-*.index .git/backplane-*.index.key")
    G.Git.snapshot(root, "t")

def same(+name: String, +root: String) -> IO(Unit):
  do IO<Unit>:
    +kept : String <- G.Git.snapshot(root, "t")
    +key : String <- sh(root, "ls .git/backplane-*.index.key 2>/dev/null | wc -l")
    +f : String <- fresh(root)
    C.Check.str(name ++ ": kept index answers as a fresh seed", Bool.show(String.eq(kept, f)), "True")
    C.Check.str(name ++ ": its key is kept", key, "1")

def main() -> IO(Unit):
  do IO<Unit>:
    +root : String <- sh("/", setup())
    +t1 : String <- G.Git.snapshot(root, "t")
    +t2 : String <- G.Git.snapshot(root, "t")
    C.Check.str("nothing changed: the same tree", Bool.show(String.eq(t1, t2)), "True")
    e1 : String <- sh(root, "printf 'two\\n' > a.txt")
    +t3 : String <- G.Git.snapshot(root, "t")
    C.Check.str("an edit of the same size at once is seen", Bool.show(String.eq(t1, t3)), "False")
    same("after an edit", root)
    e2 : String <- sh(root, "printf 'new\\n' > new.txt; printf 'x\\n' > skip.log")
    +t4 : String <- G.Git.snapshot(root, "t")
    +has : String <- sh(root, "git ls-tree --name-only " ++ t4)
    C.Check.str("a new untracked file is in, an ignored one is not", has, ".gitignore\na.txt\nnew.txt")
    same("after a new file", root)
    e3 : String <- sh(root, "printf '*.log\\nnew.txt\\n' > .gitignore")
    +t5 : String <- G.Git.snapshot(root, "t")
    +has5 : String <- sh(root, "git ls-tree --name-only " ++ t5)
    C.Check.str("a file ignored since is out again", has5, ".gitignore\na.txt")
    same("after a newly ignored file", root)
    c0 : String <- G.Git.checkpoint.capture(root, "t", 0n)
    e4 : String <- sh(root, "printf 'three\\n' > a.txt; printf 'more\\n' > more.txt")
    r : String <- G.Git.checkpoint.revert(root, "t", 0n)
    +back : String <- sh(root, "cat a.txt; ls more.txt 2>/dev/null | wc -l")
    C.Check.str("revert put the checkpoint back", back, "two\n0")
    same("after a revert", root)
    gone : String <- sh("/", "rm -rf \"$(dirname " ++ root ++ ")\"")
    C.Check.str("cleaned", gone, "")
