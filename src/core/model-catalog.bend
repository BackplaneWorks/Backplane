import Base
import ./json.bend as J

# The model selector's data, independent of the UI and conversation state.
# One prepared value shares its parsed catalog across rows, labels and search.
type ModelCatalog is Data:
  ModelCatalog{models: List<&2, String>, labels: J.Json, search: Bool}

def ModelCatalog.models(c: ModelCatalog) -> List<&2, String>:
  match c:
    case ModelCatalog{ms, _, _}: ms

def ModelCatalog.labels(c: ModelCatalog) -> J.Json:
  match c:
    case ModelCatalog{_, labels, _}: labels

def ModelCatalog.search(c: ModelCatalog) -> Bool:
  match c:
    case ModelCatalog{_, _, search}: search

def Catalog.parse(info: String) -> J.Json:
  Maybe.default(&2, J.Json, J.Json.parse(info), J.Null{})

def Catalog.strs(xs: List<&2, J.Json>) -> List<&2, String>:
  match xs:
    case Nil{}: Nil{}
    case J.Str{s} <> t: s <> Catalog.strs(t)
    case _ <> t: Catalog.strs(t)

def Catalog.list(info: J.Json, provider: String) -> List<&2, String>:
  Catalog.strs(J.Json.elems(J.Json.get_obj(info, provider)))

def Catalog.menu.k(skip: Bool, +m: String, rest: List<&2, String>) -> List<&2, String>:
  match skip:
    case True{}: rest
    case False{}: m <> rest

def Catalog.menu.go(ms: List<&2, String>) -> List<&2, String>:
  match ms:
    case Nil{}: Nil{}
    case +m <> t: Catalog.menu.k(Bool.or(String.is_empty(m), String.eq(m, "default")), m, Catalog.menu.go(t))

def Catalog.menu(info: J.Json, provider: String) -> List<&2, String>:
  "" <> Catalog.menu.go(Catalog.list(info, provider))

def Catalog.about(info: J.Json, provider: String) -> J.Json:
  J.Json.get_obj(J.Json.get_obj(info, "about"), provider)

def Catalog.searchable() -> Nat:
  12n

def Catalog.cap() -> Nat:
  50n

def Catalog.words.all(ws: List<&2, String>, +hay: String) -> Bool:
  match ws:
    case Nil{}: True{}
    case +w <> t: Bool.and(Bool.or(String.is_empty(w), String.contains(hay, w)), Catalog.words.all(t, hay))

def Catalog.find.go(ms: List<&2, String>, +ws: List<&2, String>, +labels: J.Json) -> List<&2, String>:
  match ms:
    case Nil{}: Nil{}
    case +m <> t:
      Catalog.menu.k(Bool.not(Catalog.words.all(ws, String.to_lower(m ++ " " ++ J.Json.get_str(labels, Bool.pick(String, String.is_empty(m), "default", m), "")))), m, Catalog.find.go(t, ws, labels))

def Catalog.find(all: Bool, ms: List<&2, String>, +q: String, labels: J.Json) -> List<&2, String>:
  match all:
    case True{}: ms
    case False{}: Catalog.find.go(ms, String.split(String.to_lower(String.trim(q)), ' '), labels)

def Catalog.with(+cur: String, +ms: List<&2, String>) -> List<&2, String>:
  Bool.pick(List<&2, String>, Bool.or(String.is_empty(cur), List.contains(~String, ~String.eq, ms, cur)), ms, List.append(&2, String, ms, [cur]))

def Catalog.current(own: Bool, ms: List<&2, String>, current: String) -> List<&2, String>:
  match own:
    case True{}: Catalog.with(current, ms)
    case False{}: ms

def Catalog.prepare.given(+info: J.Json, +provider: String, current: String, +own: Bool, +q: String) -> ModelCatalog:
  +menu = Catalog.menu(info, provider)
  +labels = J.Json.get_obj(Catalog.about(info, provider), "labels")
  ModelCatalog{Catalog.current(Bool.and(own, String.is_empty(String.trim(q))),
    List.take(&2, String, Catalog.find(String.is_empty(String.trim(q)), menu, q, labels), Catalog.cap()), current),
    labels, Nat.is_gt(List.length(&2, String, menu), Catalog.searchable())}

def Catalog.prepare(info: String, provider: String, current: String, own: Bool, q: String) -> ModelCatalog:
  Catalog.prepare.given(Catalog.parse(info), provider, current, own, q)
