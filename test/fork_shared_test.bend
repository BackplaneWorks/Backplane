import Base
import ./check.bend as C
import ../src/core/model.bend as M
import ../src/core/hub.bend as H

# A fork whose source has no worktree of its own (a project that is not a
# git repository, or local mode) works in the same folder: it says so.

def state(+env: M.EnvMode, +wt: String) -> M.State:
  M.State.replay([M.ProjectCreated{"p", "P", "/r", 1n}, M.ThreadCreated{"a", "p", "Shield layout", env, "", wt, "claude", "", 1n}], M.State.empty())

def notes(cs: List<&2, M.Change>) -> String:
  match cs:
    case Nil{}: ""
    case M.ActivityLogged{th, _, _, "git", +sum, _} <> t: th ++ ": " ++ sum ++ "\n" ++ notes(t)
    case _ <> t: notes(t)

def forked(+s: M.State) -> String:
  notes(H.Queue.changes(H.Hub.fork(s, 5n, 0, "1", "a")))

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("a fork of a thread in the project folder says it shares it", forked(state(M.Worktree{}, "")), "t5-2: working in the project folder with \"Shield layout\" (no worktree of its own): its edits change the same files\n")
    C.Check.str("and of a thread in local mode", Bool.show(String.contains(forked(state(M.Local{}, "")), "its edits change the same files")), "True")
    C.Check.str("a fork of a thread with its own worktree says nothing", forked(state(M.Worktree{}, "/r/.backplane/worktrees/a")), "")
