import Base
import ./json.bend as J

# Git has verified the linked checkout and returned the corresponding main
# project folder. An ordinary folder stays exactly where the caller put it.
def Project.root(linked: Bool, cwd: String, main: String) -> String:
  Bool.pick(String, linked, main, cwd)

def Project.absolute(+path: String) -> Bool:
  Bool.or(String.starts_with(path, "/"), Bool.or(String.starts_with(path, "\\\\"), String.starts_with(String.drop(path, 1n), ":")))

# Only absolute thread configuration entries override the main configuration.
# A relative entry copied into a worktree must never redirect discovery there.
def Project.setting(+main: J.Json, +thread: J.Json, +key: String) -> J.KV:
  +value = J.Json.get_str(thread, key, "")
  J.KV{key, J.Str{Bool.pick(String, Project.absolute(value), value, J.Json.get_str(main, key, ""))}}

def Project.config(+main: J.Json, +thread: J.Json) -> J.Json:
  J.Json.obj([Project.setting(main, thread, "project"), Project.setting(main, thread, "pcb"), Project.setting(main, thread, "schematic")])

def Project.design(+path: String) -> Bool:
  Bool.or(String.ends_with(path, ".kicad_pro"), Bool.or(String.ends_with(path, ".kicad_pcb"), String.ends_with(path, ".kicad_sch")))

# Explicit thread-copy operations and absolute configuration overrides retain
# their paths. Other KiCad paths inside the linked project map to main.
def Project.path(+copy: Bool, +cwd: String, +main: String, +path: String) -> String:
  Bool.pick(String, Bool.not(Project.design(path)), path,
    Bool.pick(String, Project.absolute(path),
      Bool.pick(String, Bool.and(Bool.not(copy), String.starts_with(path, cwd ++ "/")), main ++ String.drop(path, String.length(cwd)), path),
      Bool.pick(String, copy, cwd, main) ++ "/" ++ path))

# A deliberate main choice skips the prompt; a thread copy is opt-in only.
def Project.choose(+choice: String, changed: Bool) -> Bool:
  Bool.and(changed, Bool.not(Bool.or(String.eq(choice, "main"), String.eq(choice, "thread"))))
