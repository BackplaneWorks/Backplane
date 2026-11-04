import Base
import ../core/model.bend as M

# Sequence-keyed authentication waiting cache. Scans and policy are pure.
# The small scanner bodies match the unchanged core/hub implementation.

def Rec.threads(s: M.State) -> List<&2, M.Thread>:
  match s:
    case M.State{_, ts, _, _, _, _, _, _, _, _, _}:
      Map.values(&2, M.Thread, ts)

def Auth.waiting(+s: M.State, +th: String) -> Bool:
  Bool.not(String.is_empty(M.Setting.get(s, "auth.wait." ++ th, "")))

def Auth.any(ts: List<&2, M.Thread>, +s: M.State) -> Bool:
  match ts:
    case Nil{}: False{}
    case t <> rest: Bool.or(Auth.waiting(s, M.Thread.id(t)), Auth.any(rest, s))

def Auth.kept.key(+st: M.State) -> String:
  Nat.show(1n+M.State.seq(st)) ++ " "

def Auth.kept.k(hit: Bool, +kept: String, +st: M.State) -> Bool:
  match hit:
    case True{}:
      String.ends_with(kept, " 1")
    case False{}:
      Auth.any(Rec.threads(st), st)

def Auth.kept(+kept: String, +st: M.State) -> Bool:
  Auth.kept.k(String.starts_with(kept, Auth.kept.key(st)), kept, st)
