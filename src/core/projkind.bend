import Base

# Hardware projects and software projects (docs/git.md "Hardware projects").
# A board, a schematic or a CAD part cannot be merged, so a hardware project
# keeps one copy of its design: every thread works in the project's folder,
# where the project's page and viewers look. A software project gives each
# thread a worktree. Pure: the server reads the folder (kicad.bend's
# Files.read) and runs the git (git.bend's Git.hardware, Git.outside).

def Pk.marker.in(ms: List<&2, String>, +n: String) -> Bool:
  match ms:
    case Nil{}:
      False{}
    case +m <> t:
      Bool.or(String.eq(n, m), Pk.marker.in(t, n))

# a file or folder at a project's root that says it builds software
def Pk.marker(+n: String) -> Bool:
  Pk.marker.in(["package.json", "Cargo.toml", "go.mod", "pyproject.toml", "setup.py", "requirements.txt", "CMakeLists.txt", "Makefile", "makefile",
    "meson.build", "build.zig", "pom.xml", "build.gradle", "build.gradle.kts", "west.yml", "platformio.ini", "Gemfile", "deno.json", "mix.exs",
    "Package.swift", "SConstruct", "src"], n)

# a software build among a root's names
def Pk.build(ns: List<&2, String>) -> Bool:
  match ns:
    case Nil{}:
      False{}
    case +n <> t:
      Bool.or(Pk.marker(n), Pk.build(t))

# KiCad's project, board or schematic among a folder's names
def Pk.kicad(ns: List<&2, String>) -> Bool:
  match ns:
    case Nil{}:
      False{}
    case +n <> t:
      Bool.or(Bool.or(String.ends_with(n, ".kicad_pro"), String.ends_with(n, ".kicad_pcb")), Bool.or(String.ends_with(n, ".kicad_sch"), Pk.kicad(t)))

def Pk.mech(ns: List<&2, String>) -> Bool:
  match ns:
    case Nil{}:
      False{}
    case +n <> t:
      Bool.or(String.eq(n, "mech"), Pk.mech(t))

def Pk.auto(build: Bool, +design: Bool) -> Bool:
  match build:
    case True{}:
      False{}
    case False{}:
      design

# whether a folder is a hardware project: .backplane.json's "kind" when it
# says (said), else a design (KiCad at the root, or found below it, or a
# mech folder) with no software build at the root. Everything else is
# software, which is what a project was before this rule.
def Pk.hardware(said: String, +ns: List<&2, String>, +found: String) -> Bool:
  match said:
    case "hardware":
      True{}
    case "software":
      False{}
    case _:
      Pk.auto(Pk.build(ns), Bool.or(Bool.or(Pk.kicad(ns), Pk.mech(ns)), Bool.not(String.is_empty(found))))

# whether a new thread stays in the project's folder rather than getting a
# worktree: in a hardware project, unless a worktree was asked for by name
def Pk.stay(hw: Bool, +mode: String) -> Bool:
  match hw:
    case True{}:
      Bool.not(String.eq(mode, "worktree"))
    case False{}:
      False{}

# the note the thread gets, after "working in the project folder: "
def Pk.why() -> String:
  "this is a hardware project, so its threads share one copy of the design"

# the repository's own files a hardware project's clone gets (.git/info, so
# nothing tracked changes). KiCad's droppings stay out of status, and a
# design file never takes a line merge: a conflict keeps one whole side
# rather than writing conflict markers into a board.
def Pk.exclude() -> List<&2, String>:
  ["/.backplane/", "*.kicad_prl", "fp-info-cache", "*-backups/", "*.lck", "_autosave-*"]

def Pk.attributes() -> List<&2, String>:
  ["*.kicad_pcb -merge", "*.kicad_sch -merge", "*.kicad_pro -merge", "*.FCStd binary"]

# A copy of the design the project's page does not show: this folder's
# uncommitted work (place "here") or a thread worktree that differs from
# it (place "wt"). From git.bend's Git.outside.sh, a line each:
# place, branch, path, changed files, commits ahead, commits behind, when
# saved, the first few files, tab separated. Counts stay text: they are
# only shown.
type PkRow is Data:
  PkRow{place: String, branch: String, path: String, files: String, ahead: String, behind: String, saved: String, names: String}

def Pk.row(fs: List<&2, String>) -> List<&2, PkRow>:
  match fs:
    case +pl <> +b <> +p <> +n <> +a <> +h <> +t <> +ns <> _:
      [PkRow{pl, b, p, n, a, h, t, ns}]
    case _:
      Nil{}

def Pk.rows.go(ls: List<&2, String>) -> List<&2, PkRow>:
  match ls:
    case Nil{}:
      Nil{}
    case +l <> t:
      List.append(&2, PkRow, Pk.row(String.split(l, '\t')), Pk.rows.go(t))

def Pk.rows(+text: String) -> List<&2, PkRow>:
  Pk.rows.go(String.lines(text))

def Pk.count(+n: String, +one: String, +many: String) -> String:
  n ++ " " ++ Bool.pick(String, String.eq(n, "1"), one, many)

# what a row differs by, in words; a count of 0 says nothing
def Pk.parts(+n: String, +a: String, +h: String) -> List<&2, String>:
  List.append(&2, String, List.append(&2, String,
    Bool.pick(List<&2, String>, String.eq(n, "0"), Nil{}, [Pk.count(n, "file not committed", "files not committed")]),
    Bool.pick(List<&2, String>, String.eq(a, "0"), Nil{}, [Pk.count(a, "commit not in this folder", "commits not in this folder")])),
    Bool.pick(List<&2, String>, String.eq(h, "0"), Nil{}, [Pk.count(h, "commit behind", "commits behind")]))

def Pk.place(+pl: String, +b: String) -> String:
  Bool.pick(String, String.eq(pl, "here"), "This folder (" ++ b ++ ")", b)

def Pk.saved(+t: String) -> String:
  Bool.pick(String, String.is_empty(t), "", " Saved " ++ t ++ ".")

def Pk.line(r: PkRow) -> String:
  match r:
    case PkRow{+pl, +b, +p, +n, +a, +h, +t, +ns}:
      "**" ++ Pk.place(pl, b) ++ "**: " ++ String.join(Pk.parts(n, a, h), ", ") ++ "." ++ Pk.saved(t) ++ "\n`" ++ ns ++ "`" ++ Bool.pick(String, String.eq(pl, "here"), "", " in `" ++ p ++ "`")

def Pk.lines(rs: List<&2, PkRow>) -> List<&2, String>:
  match rs:
    case Nil{}:
      Nil{}
    case r <> t:
      Pk.line(r) <> Pk.lines(t)

# the project page's panel (project-home.bend's Home.outside): "" when every
# copy of the design is this folder's committed one
def Pk.panel(+rs: List<&2, PkRow>) -> String:
  String.join(Pk.lines(rs), "\n\n")

# A hardware project's rules for an agent: a thread in the project's
# folder shares it, one in a worktree (made before the rule, or by name) is
# told its files are not what the person sees. rows: the copies of the
# design that differ from the folder (git.bend's Git.outside).
def Pk.prompt.others(+rows: String) -> String:
  Bool.pick(String, String.is_empty(rows), "", "\n\nThe project's page does not show these copies of the design, which differ from this folder:\n\n" ++ rows ++ "\n\nOne of them may be the current design and this folder an old draft. Before editing a design here, ask the person which is current unless they already said.")

def Pk.prompt.here(+root: String, +rows: String) -> String:
  "<backplane_hardware_project>This is a hardware project. Boards, schematics and CAD parts cannot be merged, so it keeps one copy of them, in `" ++ root ++ "`, and every thread works here, with no worktree of its own. Save in place: the project's page and viewers show exactly the saved files.\n"
  ++ "- One writer per document: before editing a board, schematic or CAD part, call lock_acquire with the name `doc:<its path from the root>`; if it is queued, end your turn and wait for the grant. lock_release it once your work is saved and committed. Reading needs no lock.\n"
  ++ "- Commit at each milestone on the branch checked out here, adding only your own paths (`git add <paths>`). Other threads share this folder: never use `git add -A`, `git stash`, `git reset --hard`, `git clean`, or switch branches. For a variant, copy the files under `extra/` and label them, or ask the person.\n"
  ++ "- A `.kicad_pro` that changed only because KiCad saved it is noise: leave it out of your commits, and do not let it hold up other work.\n"
  ++ "- If this project is really software, `\"kind\": \"software\"` in `.backplane.json` gives its threads worktrees again; say so to the person rather than setting it yourself."
  ++ Pk.prompt.others(rows) ++ "</backplane_hardware_project>"

def Pk.prompt.worktree(+root: String, +cwd: String, +rows: String) -> String:
  "<backplane_hardware_project>This is a hardware project, and this thread works in a worktree, `" ++ cwd ++ "`. The project's own folder is `" ++ root ++ "`: its page and the viewers show that folder, not your files here, and new threads work there. KiCad reads default to that folder too. To work on this thread's copy, pass `thread_copy:true` on every `kicad_ipc` and `viewer_show` call, reads included. Pass `main:true` only when you mean to write the project's folder, and never just to get past a refusal: a refusal means this copy and the folder differ, so ask the person which is current. Finish the step you are on, commit it on this thread's branch and bring the design home, then say so to the person. Boards, schematics and CAD parts cannot be merged by line: take each file whole from one side. If a file changed both here and in the project's folder, ask the person which is current; never overwrite the folder's uncommitted files."
  ++ Pk.prompt.others(rows) ++ "</backplane_hardware_project>"

def Pk.prompt(here: Bool, +root: String, +cwd: String, +rows: List<&2, PkRow>) -> String:
  match here:
    case True{}:
      Pk.prompt.here(root, Pk.panel(rows))
    case False{}:
      Pk.prompt.worktree(root, cwd, Pk.panel(rows))
