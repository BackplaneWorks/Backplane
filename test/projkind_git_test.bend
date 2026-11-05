import Base
import ./check.bend as C
import ../src/server/proc.bend as P
import ../src/server/git.bend as G
import ../src/server/kicad.bend as KF
import ../src/core/projkind.bend as K

# A hardware project's clone and the copies of its design outside the
# folder, against a real repository: a worktree with uncommitted board work,
# one with a commit the folder lacks, one that only re-saved its .kicad_pro
# (noise), one that is clean. docs/git.md "Hardware projects".

def setup.sh() -> String:
  String.join(["set -e", "cd \"$1\"", "git init -q -b main p", "cd p", "git config user.email t@t", "git config user.name t",
    "printf '(kicad_pcb)\\n' > b.kicad_pcb", "printf '{}\\n' > b.kicad_pro", "mkdir mech", "echo x > mech/case.py", "git add .", "git commit -qm one",
    "for w in dirty ahead noise clean; do git worktree add -q .backplane/worktrees/$w -b $w; done",
    "printf '(new)\\n' > .backplane/worktrees/dirty/b.kicad_pcb",
    "mkdir .backplane/worktrees/dirty/sub", "echo n > '.backplane/worktrees/dirty/sub/new board.kicad_sch'",
    "cd .backplane/worktrees/ahead", "printf '(c)\\n' > b.kicad_pcb", "git add .", "git commit -qm c", "cd ../../..",
    "printf '{\"a\":1}\\n' > .backplane/worktrees/noise/b.kicad_pro",
    "echo y > c.txt", "git add c.txt", "git commit -qm two"], "\n")

# a software repository whose thread made the first board in its worktree
def first.sh() -> String:
  String.join(["set -e", "cd \"$1\"", "git init -q -b main s", "cd s", "git config user.email t@t", "git config user.name t", "echo x > README.md", "git add .", "git commit -qm one",
    "git worktree add -q .backplane/worktrees/w -b w", "printf '{}\\n' > .backplane/worktrees/w/first.kicad_pro"], "\n")

# .backplane.json's "kind" against what the folder holds
def kinds.sh() -> String:
  String.join(["set -e", "cd \"$1\"", "mkdir soft hard", "printf '{}\\n' > soft/b.kicad_pro", "printf '{\"kind\":\"software\"}\\n' > soft/.backplane.json",
    "echo x > hard/Cargo.toml", "printf '{\"kind\":\"hardware\"}\\n' > hard/.backplane.json"], "\n")

def out(r: P.Ran) -> String:
  match r:
    case P.Ran{_, o}:
      String.trim(o)

def has(+text: String, +part: String) -> String:
  Bool.show(String.contains(text, part))

def sh(+dir: String, +script: String) -> IO(String):
  IO.bind(P.Ran, String, P.Proc.run("sh", ["-c", script], dir), r => IO.pure(String, out(r)))

def count(+text: String) -> String:
  Nat.show(List.length(&2, K.PkRow, K.Pk.rows(text)))

def go(+t: String) -> IO(Unit):
  do IO<Unit>:
    +p : String = t ++ "/p"
    made : P.Ran <- P.Proc.run("sh", ["-c", setup.sh(), "sh", t], "/")
    C.Check.str("the repository is made", out(made), "")
    first : String <- G.Git.hardware(p)
    second : String <- G.Git.hardware(p)
    C.Check.str("set up quietly", first ++ second, "")
    +ex : String <- sh(p, "cat .git/info/exclude")
    +at : String <- sh(p, "cat .git/info/attributes")
    C.Check.str("droppings excluded", has(ex, "fp-info-cache"), "True")
    C.Check.str("worktrees excluded", has(ex, "/.backplane/"), "True")
    dups : String <- sh(p, "cat .git/info/exclude .git/info/attributes | grep -v '^#' | sort | uniq -d | wc -l | tr -d ' '")
    C.Check.str("each line once", dups, "0")
    C.Check.str("no line merge of a board", has(at, "*.kicad_pcb -merge"), "True")
    +rows : String <- G.Git.outside(p)
    C.Check.str("noise and clean copies are left out", count(rows), "2")
    C.Check.str("uncommitted work, with its new file", has(K.Pk.panel(K.Pk.rows(rows)), "**dirty**: 2 files not committed, 1 commit behind."), "True")
    C.Check.str("a name with a space", has(K.Pk.panel(K.Pk.rows(rows)), "sub/new board.kicad_sch"), "True")
    C.Check.str("a commit the folder lacks", has(K.Pk.panel(K.Pk.rows(rows)), "**ahead**: 1 commit not in this folder, 1 commit behind."), "True")
    _ : String <- sh(p, "printf '(dirty)\\n' > b.kicad_pcb")
    +now : String <- G.Git.outside(p)
    C.Check.str("the folder's own uncommitted board comes first", has(K.Pk.panel(K.Pk.rows(now)), "**This folder (main)**: 1 file not committed."), "True")
    _ : String <- sh(p, "git checkout -q b.kicad_pcb; git checkout -q -b side; printf '(side)\\n' > b.kicad_pcb; git commit -qam s; git checkout -q main; printf '(main)\\n' > b.kicad_pcb; git commit -qam m; git merge side >/dev/null 2>&1; true")
    +merged : String <- sh(p, "cat b.kicad_pcb")
    C.Check.str("a conflict keeps one whole side", merged, "(main)")
    outside : String <- G.Git.outside("/")
    C.Check.str("outside a repository, nothing", outside, "")
    _ : String <- G.Git.hardware(t)
    +left : String <- sh(t, "ls -A")
    C.Check.str("no repository: nothing is written", left, "p")
    _ : P.Ran <- P.Proc.run("sh", ["-c", first.sh(), "sh", t], "/")
    +wt : String = t ++ "/s/.backplane/worktrees/w"
    here : KF.Files <- KF.Files.read.local(wt)
    main : KF.Files <- KF.Files.read.local(t ++ "/s")
    C.Check.str("a worktree's first board is seen in its own folder", Bool.show(KF.Files.hardware(here)), "True")
    C.Check.str("but not in the project's, so detection reads both", Bool.show(KF.Files.hardware(main)), "False")
    _ : P.Ran <- P.Proc.run("sh", ["-c", kinds.sh(), "sh", t], "/")
    soft : KF.Files <- KF.Files.read.local(t ++ "/soft")
    hard : KF.Files <- KF.Files.read.local(t ++ "/hard")
    C.Check.str("kind software overrules a board", Bool.show(KF.Files.hardware(soft)), "False")
    C.Check.str("kind hardware overrules a build", Bool.show(KF.Files.hardware(hard)), "True")
    _ : P.Ran <- P.Proc.run("rm", ["-rf", t], "/")
    return Unit{}

def main() -> IO(Unit):
  IO.bind(P.Ran, Unit, P.Proc.run("mktemp", ["-d"], "/"), r => go(out(r)))
