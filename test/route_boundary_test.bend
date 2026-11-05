import Base
import ./check.bend as C
import ../src/core/model.bend as M
import ../src/core/watchpol.bend as WP

# Route boundaries, the pure part (core/watchpol.bend): which folder and
# which sources kicad.watch / kicad.warm may name. Synthetic strings only;
# nothing touches the disk, so no file here can be a link out of its folder.
# What git says about a version's repository is tested on real temporary
# repositories by test/tools/ident_fs.sh.

def yn(b: Bool) -> String:
  Bool.pick(String, b, "yes", "no")

def fixture() -> M.State:
  M.State.replay([
    M.ProjectCreated{"p", "Fixture", "/fixture/project", 1n},
    M.ProjectCreated{"gone", "Gone", "/fixture/gone", 2n},
    M.ProjectRemoved{"gone", 3n},
    M.ThreadCreated{"t", "p", "Active", M.Local{}, "", "", "claude", "", 4n},
    M.ThreadCreated{"u", "p", "Other", M.Local{}, "", "", "claude", "", 4n},
    M.ThreadCreated{"wt", "p", "Worktree", M.Worktree{}, "branch", "/fixture/worktree", "claude", "", 5n},
    M.ThreadCreated{"dead", "p", "Deleted", M.Worktree{}, "branch", "/fixture/dead-wt", "claude", "", 5n},
    M.ThreadDeleted{"dead", 6n},
    M.ThreadCreated{"old", "p", "Archived", M.Local{}, "", "", "claude", "", 4n},
    M.ThreadArchived{"old", True{}, 7n},
    M.SettingSet{"viewer.src:t", "/vendors/lib/part.step", 8n}], M.State.empty())

def w(s: M.State, thread: String, root: String, path: String) -> String:
  yn(WP.Wp.ok(s, "/home/h", "board", thread, root, path, "", "", True{}))

def x(s: M.State, thread: String, root: String, path: String, tree: String, sheet: String) -> String:
  yn(WP.Wp.ok(s, "/home/h", "board", thread, root, path, tree, sheet, True{}))

def v(commit: String, rel: String) -> String:
  "git\t/fixture/project/.git\t" ++ commit ++ "\t16\t/fixture/project/" ++ rel

def main() -> IO(Unit):
  do IO<Unit>:
    +s : M.State = fixture()
    C.Check.str("a stop names nothing", yn(WP.Wp.ok(s, "/home/h", "", "", "/etc", "/etc/x", "", "", True{})), "yes")
    C.Check.str("a thread's own folder", w(s, "t", "/fixture/project", ""), "yes")
    C.Check.str("a file of it", w(s, "t", "/fixture/project", "/fixture/project/hw/a.kicad_pcb"), "yes")
    C.Check.str("another folder for that thread", w(s, "t", "/etc", ""), "no")
    C.Check.str("a worktree thread's folder", w(s, "wt", "/fixture/worktree", ""), "yes")
    C.Check.str("its project's root is not its folder", w(s, "wt", "/fixture/project", ""), "no")
    C.Check.str("a file of the project is not a file of the worktree", w(s, "wt", "/fixture/worktree", "/fixture/project/a.kicad_pcb"), "no")
    C.Check.str("a deleted thread has no folder", w(s, "dead", "/fixture/dead-wt", ""), "no")
    C.Check.str("nor does its former folder as nobody's", w(s, "", "/fixture/dead-wt", ""), "no")
    C.Check.str("an archived thread keeps its folder", w(s, "old", "/fixture/project", ""), "yes")
    C.Check.str("an unknown thread has none", w(s, "nobody", "/fixture/project", ""), "no")
    C.Check.str("no thread: a live project's root", w(s, "", "/fixture/project", ""), "yes")
    C.Check.str("no thread: a live thread's folder", w(s, "", "/fixture/worktree", ""), "yes")
    C.Check.str("no thread: a removed project", w(s, "", "/fixture/gone", ""), "no")
    C.Check.str("no thread: inside a project is not a root", w(s, "", "/fixture/project/sub", ""), "no")
    C.Check.str("no thread: above a project is not a root", w(s, "", "/fixture", ""), "no")
    C.Check.str("no thread: a sibling with the same prefix", w(s, "", "/fixture/project-other", ""), "no")
    C.Check.str("no folder", w(s, "", "", ""), "no")
    C.Check.str("a relative folder", w(s, "", "fixture/project", ""), "no")
    C.Check.str("a file that climbs out", w(s, "t", "/fixture/project", "/fixture/project/../x.kicad_pcb"), "no")
    C.Check.str("a file of a sibling folder", w(s, "t", "/fixture/project", "/fixture/project-other/a.kicad_pcb"), "no")
    C.Check.str("what the agent showed, for its thread", w(s, "t", "/fixture/project", "/vendors/lib/part.step"), "yes")
    C.Check.str("not for another thread", w(s, "u", "/fixture/project", "/vendors/lib/part.step"), "no")
    C.Check.str("not with no thread", w(s, "", "/fixture/project", "/vendors/lib/part.step"), "no")
    C.Check.str("not another file of that folder", w(s, "t", "/fixture/project", "/vendors/lib/other.step"), "no")
    C.Check.str("not a design file nobody showed", w(s, "t", "/fixture/project", "/elsewhere/x.kicad_pcb"), "no")
    C.Check.str("a file that is no model", w(s, "t", "/fixture/project", "/etc/passwd"), "no")
    C.Check.str("the tree is judged too", x(s, "t", "/fixture/project", "", "/etc/x.kicad_sch", ""), "no")
    C.Check.str("and the sheet", x(s, "t", "/fixture/project", "", "", "/etc/x.kicad_sch"), "no")
    C.Check.str("all three in the folder", x(s, "t", "/fixture/project", "/fixture/project/a.kicad_pcb", "/fixture/project/r.kicad_sch", "/fixture/project/s.kicad_sch"), "yes")
    C.Check.str("a version of a design file", w(s, "t", "/fixture/project", v("abc123", "a.kicad_sch")), "yes")
    C.Check.str("a version in a subfolder", w(s, "t", "/fixture/project", v("abc123", "hw/a.kicad_pcb")), "yes")
    C.Check.str("a commit that is an option", w(s, "t", "/fixture/project", v("--output=x", "a.kicad_sch")), "no")
    C.Check.str("a commit that is no hash", w(s, "t", "/fixture/project", v("main", "a.kicad_sch")), "no")
    C.Check.str("a version of a hidden file", w(s, "t", "/fixture/project", v("abc123", ".env")), "no")
    C.Check.str("a version of a file that is no design", w(s, "t", "/fixture/project", v("abc123", "notes.txt")), "no")
    C.Check.str("a version that climbs", w(s, "t", "/fixture/project", v("abc123", "../a.kicad_sch")), "no")
    C.Check.str("a relative git dir", w(s, "t", "/fixture/project", "git\t.git\tabc123\t16\t/fixture/project/a.kicad_sch"), "no")
    C.Check.str("a git dir that climbs", w(s, "t", "/fixture/project", "git\t/fixture/project/../../x/.git\tabc123\t16\t/fixture/project/a.kicad_sch"), "no")
    C.Check.str("a comparison of two versions", w(s, "t", "/fixture/project", "diff\n" ++ v("abc123", "a.kicad_sch") ++ "\n" ++ v("def456", "a.kicad_sch") ++ "\n3"), "yes")
    C.Check.str("a comparison with an empty old side", w(s, "t", "/fixture/project", "diff\n\n" ++ v("def456", "a.kicad_sch") ++ "\n"), "yes")
    C.Check.str("a comparison with an empty new side", w(s, "t", "/fixture/project", "diff\n" ++ v("abc123", "a.kicad_sch") ++ "\n\n"), "yes")
    C.Check.str("a comparison with a live side", w(s, "t", "/fixture/project", "diff\n" ++ v("abc123", "a.kicad_pcb") ++ "\n/fixture/project/a.kicad_pcb\n"), "yes")
    C.Check.str("a comparison with a bad side", w(s, "t", "/fixture/project", "diff\n" ++ v("abc123", "a.kicad_sch") ++ "\n" ++ v("--x", "a.kicad_sch") ++ "\n"), "no")
    C.Check.str("a comparison with an outside live side", w(s, "t", "/fixture/project", "diff\n" ++ v("abc123", "a.kicad_sch") ++ "\n/etc/x.kicad_sch\n"), "no")
    C.Check.str("a comparison of comparisons", w(s, "t", "/fixture/project", "diff\ndiff\n\n\n\n" ++ v("abc123", "a.kicad_sch") ++ "\n"), "no")
    C.Check.str("a mirrored thread's cache, its link live", yn(WP.Wp.ok(s, "/home/h", "board", "beta~x", "/home/h/farfs/beta/p", "", "", "", True{})), "yes")
    C.Check.str("a file of that cache", yn(WP.Wp.ok(s, "/home/h", "board", "beta~x", "/home/h/farfs/beta/p", "/home/h/farfs/beta/p/a.kicad_pcb", "", "", True{})), "yes")
    C.Check.str("a mirrored thread's cache, its link gone", yn(WP.Wp.ok(s, "/home/h", "board", "beta~x", "/home/h/farfs/beta/p", "", "", "", False{})), "no")
    C.Check.str("another link's cache", yn(WP.Wp.ok(s, "/home/h", "board", "beta~x", "/home/h/farfs/gamma/p", "", "", "", True{})), "no")
    C.Check.str("the cache of no thread", w(s, "", "/home/h/farfs/beta/p", ""), "no")
    C.Check.str("versions to prove: one", Nat.show(List.length(&2, String, WP.Wp.gits([v("abc123", "a.kicad_sch"), "", "/fixture/project/a.kicad_pcb"]))), "1")
    C.Check.str("versions to prove: both sides", Nat.show(List.length(&2, String, WP.Wp.gits(["diff\n" ++ v("abc123", "a.kicad_sch") ++ "\n" ++ v("def456", "a.kicad_sch") ++ "\n", v("abc123", "r.kicad_sch")]))), "3")
    C.Check.str("versions to prove: none", Nat.show(List.length(&2, String, WP.Wp.gits(["", "/fixture/project/a.kicad_pcb", "diff\n\n\n"]))), "0")
    IO.pure(Unit, Unit{})
