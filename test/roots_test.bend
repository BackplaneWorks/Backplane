import Base
import ./check.bend as C
import ../src/core/model.bend as M
import ../src/core/roots.bend as R

# Projects whose folder is gone (core/roots.bend): what the hub looks for,
# the setting it keeps, and what clients read from it.

def log() -> List<&2, M.Change>:
  [M.ProjectCreated{"p1", "here", "/work/here", 1n},
   M.ProjectCreated{"p2", "gone", "/work/gone", 2n},
   M.ProjectCreated{"p3", "removed", "/work/removed", 3n},
   M.ProjectRemoved{"p3", 4n},
   M.ProjectCreated{"link~p9", "elsewhere", "/far/p9", 5n}]

def st(es: List<&2, M.Change>) -> M.State:
  M.State.replay(List.append(&2, M.Change, log(), es), M.State.empty())

# a worktree thread under the gone folder, a plain thread, one elsewhere
def moving() -> M.State:
  st([M.ThreadCreated{"t1", "p2", "Plain", M.Local{}, "", "", "claude", "", 6n},
      M.ThreadCreated{"t2", "p2", "Worktree", M.Local{}, "backplane/x", "/work/gone/.backplane/worktrees/x", "claude", "", 7n},
      M.ThreadCreated{"t3", "p1", "Other", M.Local{}, "", "/work/gone2/y", "claude", "", 8n},
      M.SettingSet{R.Roots.key(), " p2 p9 ", 9n}])

def show(cs: List<&2, M.Change>) -> String:
  match cs:
    case Nil{}: ""
    case M.ProjectCreated{id, t, r, _} <> rest: "project " ++ id ++ " " ++ t ++ " " ++ r ++ "; " ++ show(rest)
    case M.WorktreeSet{id, b, w, _} <> rest: "worktree " ++ id ++ " " ++ b ++ " " ++ w ++ "; " ++ show(rest)
    case M.SettingSet{k, v, _} <> rest: "set " ++ k ++ "=[" ++ v ++ "]; " ++ show(rest)
    case _ <> rest: "other; " ++ show(rest)

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("looks for this machine's live projects", String.join(R.Roots.args(st([])), "|"), "p1\t/work/here|p2\t/work/gone")
    C.Check.str("the shell's answer as the setting", R.Roots.value("p2\n") ++ "|" ++ R.Roots.value("p1\np2\n") ++ "|" ++ R.Roots.value("") ++ "|" ++ R.Roots.value("\n"), " p2 | p1 p2 ||")
    C.Check.str("nothing known: none missing", Bool.show(R.Roots.missing(st([]), "p2")), "False")
    C.Check.str("the hub found p2 gone", Bool.show(R.Roots.missing(st([M.SettingSet{R.Roots.key(), " p2 ", 9n}]), "p2")) ++ Bool.show(R.Roots.missing(st([M.SettingSet{R.Roots.key(), " p2 ", 9n}]), "p1")), "TrueFalse")
    C.Check.str("an id is not a prefix of another", Bool.show(R.Roots.missing(st([M.SettingSet{R.Roots.key(), " p22 ", 9n}]), "p2")), "False")
    C.Check.str("move: the project, its worktree threads, and no longer missing", show(R.Roots.move(moving(), "p2", " /work/new-gone/ ", 20n)),
      "project p2 gone /work/new-gone; worktree t2 backplane/x /work/new-gone/.backplane/worktrees/x; set projects.missing=[ p9 ]; ")
    C.Check.str("move: then the state says the new folder and not missing", Bool.show(R.Roots.missing(M.State.replay(R.Roots.move(moving(), "p2", "/work/new", 20n), moving()), "p2")), "False")
    C.Check.str("move: unknown, removed, relative or elsewhere: nothing", show(R.Roots.move(moving(), "nope", "/x", 20n)) ++ show(R.Roots.move(moving(), "p3", "/x", 20n)) ++ show(R.Roots.move(moving(), "p2", "~/x", 20n)) ++ show(R.Roots.move(moving(), "link~p9", "/x", 20n)), "")
    C.Check.str("the last missing one leaves an empty setting", R.Roots.without(" p2 ", "p2"), "")
    C.Check.str("found again: none missing", Bool.show(R.Roots.missing(st([M.SettingSet{R.Roots.key(), " p2 ", 9n}, M.SettingSet{R.Roots.key(), "", 10n}]), "p2")), "False")
