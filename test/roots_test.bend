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

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("looks for this machine's live projects", String.join(R.Roots.args(st([])), "|"), "p1\t/work/here|p2\t/work/gone")
    C.Check.str("the shell's answer as the setting", R.Roots.value("p2\n") ++ "|" ++ R.Roots.value("p1\np2\n") ++ "|" ++ R.Roots.value("") ++ "|" ++ R.Roots.value("\n"), " p2 | p1 p2 ||")
    C.Check.str("nothing known: none missing", Bool.show(R.Roots.missing(st([]), "p2")), "False")
    C.Check.str("the hub found p2 gone", Bool.show(R.Roots.missing(st([M.SettingSet{R.Roots.key(), " p2 ", 9n}]), "p2")) ++ Bool.show(R.Roots.missing(st([M.SettingSet{R.Roots.key(), " p2 ", 9n}]), "p1")), "TrueFalse")
    C.Check.str("an id is not a prefix of another", Bool.show(R.Roots.missing(st([M.SettingSet{R.Roots.key(), " p22 ", 9n}]), "p2")), "False")
    C.Check.str("found again: none missing", Bool.show(R.Roots.missing(st([M.SettingSet{R.Roots.key(), " p2 ", 9n}, M.SettingSet{R.Roots.key(), "", 10n}]), "p2")), "False")
