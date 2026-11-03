import Base
import ./model.bend as M
import ./far.bend as FR

# Projects whose folder is gone (moved, renamed or deleted while Backplane
# was not looking). The hub looks once a minute (server.bend's RootJob) and
# keeps the ids of the missing ones in the setting "projects.missing", as
# " id1 id2 " (empty when none), written only when that set changes; every
# client reads it from the state it already has, so the sidebar and the
# thread can say so before a message fails with "its folder ... is gone".

def Roots.key() -> String:
  "projects.missing"

def Roots.arg.k(skip: Bool, +id: String, +root: String, rest: List<&2, String>) -> List<&2, String>:
  match skip:
    case True{}: rest
    case False{}: (id ++ "\t" ++ root) <> rest

# what to look for, "<id>\t<root>" each: this machine's projects that are
# not removed and have a folder (not one elsewhere, not the scratch one)
def Roots.args.go(ps: List<&2, M.Project>) -> List<&2, String>:
  match ps:
    case Nil{}: Nil{}
    case M.Project{+id, _, +root, _, rm} <> t:
      Roots.arg.k(Bool.or(rm, Bool.or(FR.Fr.is(id), Bool.or(M.Scratch.is(id), String.is_empty(root)))), id, root, Roots.args.go(t))

def Roots.args(s: M.State) -> List<&2, String>:
  match s:
    case M.State{ps, _, _, _, _, _, _, _, _, _, _}:
      Roots.args.go(Map.values(&2, M.Project, ps))

# the shell that answers the ids of the folders not there, one a line
# (a path that exists but is not a folder counts as missing too)
def Roots.sh() -> String:
  "for a; do id=${a%%\t*}; d=${a#*\t}; [ -d \"$d\" ] || printf '%s\\n' \"$id\"; done; true"

def Roots.join(ls: List<&2, String>) -> String:
  match ls:
    case Nil{}: ""
    case +l <> t: Bool.pick(String, String.is_empty(String.trim(l)), "", String.trim(l) ++ " ") ++ Roots.join(t)

# the setting's value for that answer: " a b " ("" for none)
def Roots.value(+out: String) -> String:
  +ids = Roots.join(String.lines(out))
  Bool.pick(String, String.is_empty(ids), "", " " ++ ids)

# whether the hub last found this project's folder missing
def Roots.missing(s: M.State, +id: String) -> Bool:
  String.contains(M.Setting.get(s, Roots.key(), ""), " " ++ id ++ " ")

# Pointing a project at the folder's new place ("Use this folder" on the
# missing notice; hub.bend's project.move): the project is logged again
# under its id with the new folder (State.put_project replaces it), the
# threads that named a folder under the old one (worktrees, rarely: most
# follow the project's) are moved with it, and the project leaves
# "projects.missing" at once (the next look puts it back if the new folder
# is not there either). A git worktree moved this way needs `git worktree
# repair` in the project once (said in the answer).

def Roots.clean(+path: String) -> String:
  +t = String.trim(path)
  Bool.pick(String, Bool.and(String.ends_with(t, "/"), Nat.is_gt(String.length(t), 1n)), String.take(t, Nat.sub(String.length(t), 1n)), t)

def Roots.keep(ws: List<&2, String>, +id: String) -> String:
  match ws:
    case Nil{}: ""
    case +w <> t: Bool.pick(String, Bool.or(String.is_empty(w), String.eq(w, id)), "", w ++ " ") ++ Roots.keep(t, id)

def Roots.without(+v: String, +id: String) -> String:
  +rest = Roots.keep(String.split(v, ' '), id)
  Bool.pick(String, String.is_empty(rest), "", " " ++ rest)

def Roots.wt.k(under: Bool, +th: String, +branch: String, +old: String, +new: String, +wt: String, +now: Nat, rest: List<&2, M.Change>) -> List<&2, M.Change>:
  match under:
    case True{}: M.WorktreeSet{th, branch, new ++ String.drop(wt, String.length(old)), now} <> rest
    case False{}: rest

def Roots.wts(ts: List<&2, M.Thread>, +pid: String, +old: String, +new: String, +now: Nat) -> List<&2, M.Change>:
  match ts:
    case Nil{}: Nil{}
    case M.Thread{+th, +p, _, _, _, _, _, _, _, _, _, _, _, +branch, +wt, _, _, _, _, _, _} <> t:
      Roots.wt.k(Bool.and(String.eq(p, pid), String.starts_with(wt, old ++ "/")), th, branch, old, new, wt, now, Roots.wts(t, pid, old, new, now))

def Roots.project.got(r: Map<&2, M.Project> & M.Project) -> M.Project:
  (_, p) = r
  p

def Roots.threads(s: M.State) -> List<&2, M.Thread>:
  match s:
    case M.State{_, ts, _, _, _, _, _, _, _, _, _}: Map.values(&2, M.Thread, ts)

def Roots.find(s: M.State, +id: String) -> M.Project:
  match s:
    case M.State{ps, _, _, _, _, _, _, _, _, _, _}: Roots.project.got(Map.get(M.Project, M.Project.blank(id), ps, id))

def Roots.project.k(p: M.Project, +s: M.State, +path: String, +now: Nat) -> List<&2, M.Change>:
  match p:
    case M.Project{+id, +title, +root, _, False{}}:
      M.ProjectCreated{id, title, path, now} <> List.append(&2, M.Change, Roots.wts(Roots.threads(s), id, root, path, now), [M.SettingSet{Roots.key(), Roots.without(M.Setting.get(s, Roots.key(), ""), id), now}])
    case _: Nil{}

# what project.move logs: nothing for an unknown or removed project, or a
# path that is not absolute (the folder picker's ~ is not known here)
def Roots.move(+s: M.State, +id: String, +path: String, +now: Nat) -> List<&2, M.Change>:
  Bool.pick(List<&2, M.Change>, Bool.or(Bool.not(String.starts_with(Roots.clean(path), "/")), Bool.or(FR.Fr.is(id), M.Scratch.is(id))), Nil{},
    Roots.project.k(Roots.find(s, id), s, Roots.clean(path), now))
