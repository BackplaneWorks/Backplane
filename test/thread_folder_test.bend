import Base
import ./check.bend as C
import ../src/core/model.bend as M
import ../src/core/hub.bend as H
import ../src/core/threadpath.bend as TP
import ../src/core/rview.bend as RV
import ../src/core/folder.bend as F

def fixture() -> M.State:
  M.State.replay([
    M.ProjectCreated{"p", "Fixture", "/fixture/project", 1n},
    M.ThreadCreated{"t", "p", "Active", M.Local{}, "", "", "claude", "", 2n},
    M.ThreadCreated{"archived", "p", "Archived", M.Local{}, "", "", "claude", "", 2n},
    M.ThreadArchived{"archived", True{}, 3n},
    M.ThreadCreated{"deleted", "p", "Deleted", M.Local{}, "", "", "claude", "", 2n},
    M.ThreadDeleted{"deleted", 3n},
    M.ThreadCreated{"wt", "p", "Worktree", M.Worktree{}, "branch", "/fixture/worktree", "claude", "", 2n},
    M.ProjectCreated{"removed", "Removed", "/fixture/removed", 1n}, M.ProjectRemoved{"removed", 3n},
    M.ThreadCreated{"removed-t", "removed", "Removed project", M.Local{}, "", "/fixture/removed-wt", "claude", "", 2n},
    M.ProjectCreated{"relative", "Relative", "relative", 1n},
    M.ThreadCreated{"relative-t", "relative", "Relative root", M.Local{}, "", "", "claude", "", 2n}], M.State.empty())

def root(id: String) -> String:
  Maybe.default(&2, String, H.Hub.root_for(fixture(), id), "REFUSED")

def refused(step: H.Step) -> Bool:
  match step:
    case H.Step{Nil{}, H.Send{_, text} <> Nil{}}: String.contains(text, "No folder")
    case _: False{}

def jobs() -> List<&2, String>:
  ["dir", "files", "status", "diff", "revert", "act", "act_gen", "checkpoint0", "checkpoint", "step", "mech", "render", "boards", "kicad", "kexport", "refs", "rv.resolve", "rv.bom", "rv.checks", "rv.release", "skills"]

def check.jobs(js: List<&2, String>, +id: String) -> IO(Unit):
  match js:
    case Nil{}: IO.pure(Unit, Unit{})
    case +job <> rest:
      do IO<Unit>:
        C.Check.str("refuse " ++ id ++ " " ++ job, Bool.show(refused(H.Hub.git(fixture(), 1, "1", job, id, "", ""))), "True")
        check.jobs(rest, id)

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("unknown has no folder", root("unknown"), "REFUSED")
    C.Check.str("empty has no folder", root(""), "REFUSED")
    C.Check.str("deleted has no folder", root("deleted"), "REFUSED")
    C.Check.str("removed project revokes worktree", root("removed-t"), "REFUSED")
    C.Check.str("relative project root refused", root("relative-t"), "REFUSED")
    C.Check.str("active keeps project folder", root("t"), "/fixture/project")
    C.Check.str("archive keeps folder", root("archived"), "/fixture/project")
    C.Check.str("worktree keeps its own folder", root("wt"), "/fixture/worktree")
    C.Check.str("deleted read root refused", RV.Rview.root("/home", fixture(), "deleted", ""), "")
    C.Check.str("deleted internal cwd refused", H.Hub.cwd(fixture(), H.Hub.thread(fixture(), "deleted")), "")
    C.Check.str("missing project home refused", root("project-home-missing"), "REFUSED")
    C.Check.str("project home keeps its root", root("project-home-p"), "/fixture/project")
    C.Check.str("render traversal refused", Bool.show(H.Hub.folder.path("/fixture/project", "../outside.step")), "False")
    C.Check.str("Windows absolute render outside root refused", Bool.show(H.Hub.folder.path("C:/fixture/project", "C:/outside/model.step")), "False")
    C.Check.str("render sibling prefix refused", Bool.show(H.Hub.folder.path("/fixture/project", "/fixture/project-other/a.step")), "False")
    C.Check.str("render in project supported", Bool.show(H.Hub.folder.path("/fixture/project", "/fixture/project/mech/a.step")), "True")
    C.Check.str("empty absolute folder refused", Bool.show(F.Folder.absolute("")), "False")
    C.Check.str("Windows drive-relative refused", Bool.show(F.Folder.absolute("C:project")), "False")
    C.Check.str("Windows native folder supported", Bool.show(F.Folder.absolute("C:/project")), "True")
    C.Check.str("missing-root checkpoint discards plan", Bool.show(refused(H.Hub.folder.check(H.Step{[M.TurnChanged{"unknown", M.Running{}, 4n}], [H.Git{0, "", "checkpoint", "unknown", "", "", ""}]}, 1, "1"))), "True")
    C.Check.str("missing-root terminal discards plan", Bool.show(refused(H.Hub.folder.check(H.Step{Nil{}, [H.TermOp{"open", "unknown", "", Nil{}, 80, 24}]}, 1, "1"))), "True")
    check.jobs(jobs(), "unknown")
    check.jobs(jobs(), "")
    check.jobs(jobs(), "deleted")
