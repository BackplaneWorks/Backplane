import Base
import ./check.bend as C
import ../src/core/projkind.bend as K
import ../src/core/project-home.bend as PH
import ../src/core/json.bend as J

# Hardware projects: what counts as one, whether a thread stays in the
# folder, the copies of a design the project's page does not show, and what
# an agent is told. docs/git.md "Hardware projects".

def hw(+said: String, +ns: List<&2, String>, +found: String) -> String:
  Bool.show(K.Pk.hardware(said, ns, found))

def two() -> String:
  "wt\tbackplane-753e\t/p/.backplane/worktrees/backplane-753e\t2\t1\t7\t2026-11-03 14:02\tb.kicad_pcb`, `b.kicad_sch\nhere\tmain\t/p\t1\t0\t0\t2026-11-04 09:30\tmech/case.py\n"

def has(+text: String, +part: String) -> String:
  Bool.show(String.contains(text, part))

def panels(+text: String) -> Nat:
  List.length(&2, PH.Panel, PH.Home.panels(J.Json.obj([J.KV{"outside", J.Str{text}}])))

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("declared hardware beats a build", hw("hardware", ["Cargo.toml"], ""), "True")
    C.Check.str("declared software beats a board", hw("software", ["b.kicad_pro"], ""), "False")
    C.Check.str("an unknown kind decides from the files", hw("firmware", ["b.kicad_pcb"], ""), "True")
    C.Check.str("a board at the root", hw("", ["README.md", "b.kicad_sch"], ""), "True")
    C.Check.str("a board below the root", hw("", ["boards"], "boards/a/a.kicad_pro"), "True")
    C.Check.str("a mech folder", hw("", ["mech"], ""), "True")
    C.Check.str("a build and a board stay software", hw("", ["pyproject.toml", "b.kicad_pro"], ""), "False")
    C.Check.str("a src folder is a build", hw("", ["src"], "boards/a/a.kicad_pro"), "False")
    C.Check.str("an empty folder is software", hw("", Nil{}, ""), "False")
    C.Check.str("a new thread stays", Bool.show(K.Pk.stay(True{}, "auto")), "True")
    C.Check.str("a worktree by name is kept", Bool.show(K.Pk.stay(True{}, "worktree")), "False")
    C.Check.str("software never stays", Bool.show(K.Pk.stay(False{}, "auto")), "False")
    C.Check.str("two rows", Nat.show(List.length(&2, K.PkRow, K.Pk.rows(two()))), "2")
    C.Check.str("a bad line is no row", Nat.show(List.length(&2, K.PkRow, K.Pk.rows("nonsense\n"))), "0")
    C.Check.str("worktree in words", has(K.Pk.panel(K.Pk.rows(two())), "**backplane-753e**: 2 files not committed, 1 commit not in this folder, 7 commits behind. Saved 2026-11-03 14:02.\n`b.kicad_pcb`, `b.kicad_sch` in `/p/.backplane/worktrees/backplane-753e`"), "True")
    C.Check.str("the folder in words", has(K.Pk.panel(K.Pk.rows(two())), "**This folder (main)**: 1 file not committed. Saved 2026-11-04 09:30.\n`mech/case.py`"), "True")
    C.Check.str("no saved time, no sentence", K.Pk.panel(K.Pk.rows("wt\tb\t/w\t1\t0\t0\t\tgone.kicad_pcb\n")), "**b**: 1 file not committed.\n`gone.kicad_pcb` in `/w`")
    C.Check.str("exclude keeps the worktree line", has(String.join(K.Pk.exclude(), "\n"), "/.backplane/"), "True")
    C.Check.str("boards take no line merge", has(String.join(K.Pk.attributes(), "\n"), "*.kicad_pcb -merge"), "True")
    C.Check.str("the page shows other copies first", Nat.show(panels("**b**")), "1")
    C.Check.str("and nothing without them", Nat.show(panels("")), "0")
    C.Check.str("in the folder: one copy", has(K.Pk.prompt(True{}, "/p", "/p", Nil{}), "keeps one copy of them, in `/p`"), "True")
    C.Check.str("in the folder: the document lock", has(K.Pk.prompt(True{}, "/p", "/p", Nil{}), "`doc:<its path from the root>`"), "True")
    C.Check.str("in the folder: no shared-folder git", has(K.Pk.prompt(True{}, "/p", "/p", Nil{}), "never use `git add -A`"), "True")
    C.Check.str("no copies, no question", has(K.Pk.prompt(True{}, "/p", "/p", Nil{}), "ask the person which is current"), "False")
    C.Check.str("copies: ask before editing a draft", has(K.Pk.prompt(True{}, "/p", "/p", K.Pk.rows(two())), "ask the person which is current"), "True")
    C.Check.str("copies are named", has(K.Pk.prompt(True{}, "/p", "/p", K.Pk.rows(two())), "backplane-753e"), "True")
    C.Check.str("in a worktree: its files are not shown", has(K.Pk.prompt(False{}, "/p", "/p/.backplane/worktrees/w", Nil{}), "not your files here"), "True")
    C.Check.str("in a worktree: its own copy on every call", has(K.Pk.prompt(False{}, "/p", "/p/.backplane/worktrees/w", Nil{}), "pass `thread_copy:true` on every `kicad_ipc` and `viewer_show` call, reads included"), "True")
    C.Check.str("in a worktree: a refusal is a question", has(K.Pk.prompt(False{}, "/p", "/p/.backplane/worktrees/w", Nil{}), "never just to get past a refusal"), "True")
    C.Check.str("in a worktree: whole files", has(K.Pk.prompt(False{}, "/p", "/p/.backplane/worktrees/w", Nil{}), "take each file whole from one side"), "True")
    C.Check.str("in a worktree: not told it shares the folder", has(K.Pk.prompt(False{}, "/p", "/p/.backplane/worktrees/w", Nil{}), "never use `git add -A`"), "False")
