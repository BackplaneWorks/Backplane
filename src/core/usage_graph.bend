import Base
import ./model.bend as M
import ./usage_page.bend as UP
import ./json.bend as J

# Capability arrays are the existing machine provider availability source.
# Never infer a provider from a model name or fabricate unsupported collectors.
def Ug.enabled.go(ps: List<&2, String>, +caps: J.Json) -> List<&2, String>:
  match ps:
    case Nil{}: Nil{}
    case +p <> rest:
      List.append(&2, String, Bool.pick(List<&2, String>, List.is_empty(&2, J.Json, J.Json.elems(J.Json.get_obj(caps, p))), Nil{}, [p]), Ug.enabled.go(rest, caps))

def Ug.enabled(info: String) -> List<&2, String>:
  Ug.enabled.go(["claude", "codex", "grok", "muse", "antigravity", "opencode", "pi", "omp"], Maybe.default(&2, J.Json, J.Json.parse(info), J.Null{}))

def Ug.selected(+ps: List<&2, String>, +wanted: String) -> String:
  Bool.pick(String, M.Tok.has(ps, wanted), wanted, Maybe.default(&2, String, List.head(&2, String, ps), ""))

# Switch.num's second argument is a digit accumulator, not a fallback.
# Starting at one would parse "01" as 101 and move the range back 100 days.
def Ug.month.offset(now: Nat) -> Nat:
  Nat.sub(M.Switch.num(String.drop(M.Tok.day(now), 8n), 0n), 1n)

def Ug.start(+now: Nat, +period: String) -> Nat:
  +midnight = Nat.mul(Nat.div(now, 86400n), 86400n)
  Nat.sub(midnight, Nat.mul(Bool.pick(Nat, String.eq(UP.Up.period(period), "today"), 0n, Ug.month.offset(now)), 86400n))

def Ug.count(+now: Nat, +period: String) -> Nat:
  Nat.add(Nat.div(Nat.sub(now, Ug.start(now, period)), 86400n), 1n)

type UgDay is Data:
  UgDay{date: String, total: String}

def Ug.days.go(n: Nat, +at: Nat, +st: M.State, +provider: String) -> List<&2, UgDay>:
  match n:
    case 0n: Nil{}
    case 1n+k:
      UgDay{M.Tok.day(at), M.Setting.get(st, M.Tok.pre(st) ++ "dv." ++ M.Tok.day(at) ++ "|" ++ provider, "")} <> Ug.days.go(k, Nat.add(at, 86400n), st, provider)

def Ug.days(+st: M.State, +now: Nat, +period: String, provider: String) -> List<&2, UgDay>:
  Ug.days.go(Ug.count(now, period), Ug.start(now, period), st, provider)

def Ug.max(ds: List<&2, UgDay>, +acc: Nat) -> Nat:
  match ds:
    case Nil{}: acc
    case UgDay{_, v} <> rest:
      +n = UP.Up.tokens(v)
      Ug.max(rest, Bool.pick(Nat, Nat.is_gt(n, acc), n, acc))

def Ug.sum(ds: List<&2, UgDay>, acc: Nat) -> Nat:
  match ds:
    case Nil{}: acc
    case UgDay{_, v} <> rest: Ug.sum(rest, Nat.add(acc, UP.Up.tokens(v)))

def Ug.known(+v: String) -> Bool:
  Bool.or(Nat.is_gt(M.Tok.part(v, 8n), 0n), Nat.is_gt(UP.Up.tokens(v), 0n))

def Ug.reading(+v: String) -> String:
  Bool.pick(String, String.is_empty(v), "No readings", Bool.pick(String, Ug.known(v), Nat.show(UP.Up.tokens(v)) ++ " recorded tokens · " ++ UP.Up.coverage(v), "Token readings unknown · " ++ UP.Up.coverage(v)))

def Ug.coverage(+st: M.State) -> String:
  +since = M.Setting.get(st, M.Tok.pre(st) ++ "daily.since", "")
  Bool.pick(String, String.is_empty(since), "Daily provider readings have not started.", "Daily provider readings since " ++ M.Tok.day(M.Switch.num(since, 0n)) ++ " (UTC); earlier days may be missing.") ++ " Missing days are unknown, not zero. Only recorded turns on this machine."

type UgProject is Data:
  UgProject{id: String, title: String, tokens: Nat}

def UgProject.tokens(p: UgProject) -> Nat:
  match p:
    case UgProject{_, _, n}: n

def Ug.project.title(ps: List<&2, M.Project>, +id: String) -> String:
  match ps:
    case Nil{}: Bool.pick(String, String.is_empty(id), "Unknown project", "Unavailable project")
    case M.Project{+pid, +title, _, _, _} <> rest:
      # A catalog lookup only; missing/deleted identities remain in the ranking.
      Bool.pick(String, String.eq(pid, id), title, Ug.project.title(rest, id))

# Bounded insertion keeps at most five entries throughout aggregation.
def Ug.higher(+a: UgProject, +b: UgProject) -> UgProject:
  Bool.pick(UgProject, Nat.is_ge(UgProject.tokens(a), UgProject.tokens(b)), a, b)

def Ug.lower(+a: UgProject, +b: UgProject) -> UgProject:
  Bool.pick(UgProject, Nat.is_ge(UgProject.tokens(a), UgProject.tokens(b)), b, a)

def Ug.insert(ps: List<&2, UgProject>, +p: UgProject) -> List<&2, UgProject>:
  match ps:
    case Nil{}: [p]
    case +h <> rest: Ug.higher(h, p) <> Ug.insert(rest, Ug.lower(h, p))

def Ug.project.add(have: Bool, +id: String, +v: String, +catalog: List<&2, M.Project>, acc: List<&2, UgProject>) -> List<&2, UgProject>:
  match have:
    case False{}: acc
    case True{}: List.take(&2, UgProject, Ug.insert(acc, UgProject{id, Ug.project.title(catalog, id), UP.Up.tokens(v)}), 5n)

def Ug.projects.go(keys: List<&2, String>, +st: M.State, +catalog: List<&2, M.Project>, +pre: String, +suffix: String, acc: List<&2, UgProject>) -> List<&2, UgProject>:
  match keys:
    case Nil{}: acc
    case +key <> rest:
      +v = M.Setting.get(st, key, "")
      +id = String.take(String.drop(key, String.length(pre)), Nat.sub(String.length(String.drop(key, String.length(pre))), String.length(suffix)))
      Ug.projects.go(rest, st, catalog, pre, suffix, Ug.project.add(Bool.and(String.ends_with(key, suffix), Nat.is_gt(UP.Up.tokens(v), 0n)), id, v, catalog, acc))

def Ug.projects(+st: M.State, catalog: List<&2, M.Project>, +now: Nat, +period: String) -> List<&2, UgProject>:
  +pre = M.Tok.pre(st) ++ Bool.pick(String, String.eq(UP.Up.period(period), "today"), "dp." ++ M.Tok.day(now) ++ "|", "p.")
  +suffix = Bool.pick(String, String.eq(UP.Up.period(period), "today"), "", "|" ++ M.Tok.month(now))
  Ug.projects.go(M.Setting.keys.pre(st, pre), st, catalog, pre, suffix, Nil{})
