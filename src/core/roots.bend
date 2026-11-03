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
