import Base
import ./check.bend as C
import ../src/core/model.bend as M
import ../src/core/hub.bend as H
import ../src/server/git.bend as G
import ../src/server/proc.bend as P

# Shared worktrees and their self-heal. On 2026-10-30 a FazePro worktree
# shared by a thread and six it launched was removed three times by the
# tick (the launched ones were put away), and the thread's next start failed
# with "could not start claude: its folder ... is gone".

def drops(cs: List<&2, M.Change>) -> String:
  Nat.show(H.Wt.share.drops(H.Wt.share.demo(cs)))

def plan(m: Maybe<&2, H.Heal>) -> String:
  match m:
    case None{}: "none"
    case Some{H.Heal{r, w, b}}: r ++ " " ++ w ++ " " ++ b

def out(r: P.Ran) -> String:
  match r:
    case P.Ran{_, o}: String.trim(o)

# a repository with a commit, branch b1 checked out in a worktree under
# .backplane/worktrees, whose folder is then deleted outside git
def setup() -> String:
  "d=$(mktemp -d) && cd \"$d\" && git init -q -b main r && cd r && git -c user.email=t@t -c user.name=t commit -q --allow-empty -m one && git worktree add -q -b b1 .backplane/worktrees/w1 && rm -rf .backplane/worktrees/w1 && echo \"$d/r\""

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("a put-away thread's worktree stays while another on it stays", drops(Nil{}), "0")
    C.Check.str("it goes with the last thread on it", drops([M.ThreadSettled{"b", True{}, 3n}]), "2")
    C.Check.str("a deleted sharer keeps nothing", drops([M.ThreadDeleted{"b", 3n}]), "2")
    C.Check.str("an archived sharer keeps nothing", drops([M.ThreadArchived{"b", True{}, 3n}]), "2")
    C.Check.str("a worktree thread can be healed at its own path", plan(H.Wt.heal(H.Wt.share.demo(Nil{}), "b")), "/r /r/.backplane/worktrees/w1 b1")
    C.Check.str("a project-folder thread never is", plan(H.Wt.heal(H.Shelve.demo(Nil{}), "t1")), "none")
    made : P.Ran <- P.Proc.run("sh", ["-c", setup()], "/")
    +root : String = out(made)
    +wt : String = root ++ "/.backplane/worktrees/w1"
    gone : P.Ran <- P.Proc.run("sh", ["-c", "test -d \"$1\" && echo there || echo gone", "sh", wt], "/")
    C.Check.str("the folder was deleted", out(gone), "gone")
    healed : String <- G.Git.heal(root, wt, "b1")
    C.Check.str("the heal says so", healed, "worktree " ++ wt ++ " was gone; made again")
    head : P.Ran <- P.Proc.run("sh", ["-c", "git -C \"$1\" rev-parse --abbrev-ref HEAD", "sh", wt], "/")
    C.Check.str("back on its branch at its path", out(head), "b1")
    again : String <- G.Git.heal(root, wt, "b1")
    C.Check.str("a folder that is there is left alone", again, "")
    rm : P.Ran <- P.Proc.run("sh", ["-c", "rm -rf \"$(dirname \"$1\")\"", "sh", root], "/")
    return Unit{}
