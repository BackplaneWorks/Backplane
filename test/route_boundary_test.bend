import Base
import ./check.bend as C
import ../src/core/model.bend as M
import ../src/core/hub.bend as H
import ../src/core/rview.bend as RV

# Route boundaries (synthetic paths only; nothing here touches the disk):
# review.bom's versions name a repository and a commit that reach git, and
# kicad.watch's folder and files reach the viewers' parsers.

def yn(b: Bool) -> String:
  Bool.pick(String, b, "yes", "no")

def fixture() -> M.State:
  M.State.replay([
    M.ProjectCreated{"p", "Fixture", "/fixture/project", 1n},
    M.ProjectCreated{"gone", "Gone", "/fixture/gone", 2n},
    M.ProjectRemoved{"gone", 3n},
    M.ThreadCreated{"t", "p", "Active", M.Local{}, "", "/fixture/project/.backplane/worktrees/t", "claude", "", 4n}], M.State.empty())

def repo(g: String, c: String) -> String:
  "git\t" ++ g ++ "\t" ++ c ++ "\t2\t/fixture/project/a.kicad_sch"

def ok(s: M.State, root: String, path: String) -> String:
  yn(H.Watch.ok(s, "/home/h", "board", root, path, "", ""))

def main() -> IO(Unit):
  do IO<Unit>:
    +s : M.State = fixture()
    C.Check.str("repository folder of a git dir", RV.Rview.top("/fixture/project/.git"), "/fixture/project")
    C.Check.str("repository folder of a worktree record", RV.Rview.top("/fixture/project/.git/worktrees/t"), "/fixture/project")
    C.Check.str("folder of the history outside git", RV.Rview.top("/fixture/project/.backplane/history.git"), "/fixture/project")
    C.Check.str("no folder for any other directory", "[" ++ RV.Rview.top("/etc") ++ "]", "[]")
    C.Check.str("own repository", yn(RV.Rview.repo.ok(repo("/fixture/project/.git", "abc123"), "/fixture/project")), "yes")
    C.Check.str("own repository from its worktree", yn(RV.Rview.repo.ok(repo("/fixture/project/.git/worktrees/t", "abc123"), "/fixture/project/.backplane/worktrees/t")), "yes")
    C.Check.str("another repository", yn(RV.Rview.repo.ok(repo("/other/.git", "abc123"), "/fixture/project")), "no")
    C.Check.str("a sibling folder with the same prefix", yn(RV.Rview.repo.ok(repo("/fixture/proj/.git", "abc123"), "/fixture/project")), "no")
    C.Check.str("a commit word that is an option", yn(RV.Rview.repo.ok(repo("/fixture/project/.git", "--output=x"), "/fixture/project")), "no")
    C.Check.str("an empty commit", yn(RV.Rview.repo.ok(repo("/fixture/project/.git", ""), "/fixture/project")), "no")
    C.Check.str("a git dir that climbs", yn(RV.Rview.repo.ok(repo("/fixture/project/../../other/.git", "abc123"), "/fixture/project")), "no")
    C.Check.str("no folder, no repository", yn(RV.Rview.repo.ok(repo("/fixture/project/.git", "abc123"), "")), "no")
    C.Check.str("a live file names none", yn(RV.Rview.repo.ok("/fixture/project/a.kicad_sch", "/fixture/project")), "yes")
    C.Check.str("no version names none", yn(RV.Rview.repo.ok("", "/fixture/project")), "yes")
    C.Check.str("watch stop needs no folder", yn(H.Watch.ok(s, "/home/h", "", "", "", "", "")), "yes")
    C.Check.str("watch a project's own board", ok(s, "/fixture/project", ""), "yes")
    C.Check.str("watch a file of the project", ok(s, "/fixture/project", "/fixture/project/hw/a.kicad_pcb"), "yes")
    C.Check.str("watch a thread's worktree", ok(s, "/fixture/project/.backplane/worktrees/t", ""), "yes")
    C.Check.str("watch a mirrored thread's cache", ok(s, "/home/h/farfs/beta/p", ""), "yes")
    C.Check.str("watch a vendor model shown elsewhere", ok(s, "/fixture/project", "/vendors/lib/part.STEP"), "yes")
    C.Check.str("watch an unknown folder", ok(s, "/etc", ""), "no")
    C.Check.str("watch a removed project", ok(s, "/fixture/gone", ""), "no")
    C.Check.str("watch a sibling folder with the same prefix", ok(s, "/fixture/project-other", ""), "no")
    C.Check.str("watch a folder that climbs out", ok(s, "/fixture/project/../../etc", ""), "no")
    C.Check.str("watch no folder", ok(s, "", ""), "no")
    C.Check.str("watch a relative folder", ok(s, "fixture/project", ""), "no")
    C.Check.str("watch a file that is no design or model", ok(s, "/fixture/project", "/etc/passwd"), "no")
    C.Check.str("watch a file that climbs", ok(s, "/fixture/project", "/fixture/project/../x.step"), "no")
    C.Check.str("watch a project file of any kind", ok(s, "/fixture/project", "/fixture/project/notes.txt"), "yes")
    IO.pure(Unit, Unit{})
