import Base
import ../src/core/model-catalog.bend as MC
import ../src/core/json.bend as J

# Frozen original catalog pipeline; deliberately independent of the new helper.
def Old.strs(xs: List<&2, J.Json>) -> List<&2, String>:
  match xs:
    case Nil{}:
      Nil{}
    case J.Str{s} <> t:
      s <> Old.strs(t)
    case _ <> t:
      Old.strs(t)

def Old.list(+info: String, +provider: String) -> List<&2, String>:
  Old.strs(J.Json.elems(J.Json.get_obj(Maybe.default(&2, J.Json, J.Json.parse(info), J.Null{}), provider)))

def Old.menu.k(skip: Bool, +m: String, rest: List<&2, String>) -> List<&2, String>:
  match skip:
    case True{}:
      rest
    case False{}:
      m <> rest

def Old.menu.go(ms: List<&2, String>) -> List<&2, String>:
  match ms:
    case Nil{}:
      Nil{}
    case +m <> t:
      Old.menu.k(Bool.or(String.is_empty(m), String.eq(m, "default")), m, Old.menu.go(t))

def Old.menu(+info: String, +provider: String) -> List<&2, String>:
  "" <> Old.menu.go(Old.list(info, provider))

def Old.about(+info: String, +provider: String) -> J.Json:
  J.Json.get_obj(J.Json.get_obj(Maybe.default(&2, J.Json, J.Json.parse(info), J.Null{}), "about"), provider)

def Old.searchable() -> Nat:
  12n

def Old.search(+ms: List<&2, String>) -> Bool:
  Nat.is_gt(List.length(&2, String, ms), Old.searchable())

def Old.words.all(ws: List<&2, String>, +hay: String) -> Bool:
  match ws:
    case Nil{}:
      True{}
    case +w <> t:
      Bool.and(Bool.or(String.is_empty(w), String.contains(hay, w)), Old.words.all(t, hay))

def Old.find.go(ms: List<&2, String>, +ws: List<&2, String>, +labels: J.Json) -> List<&2, String>:
  match ms:
    case Nil{}:
      Nil{}
    case +m <> t:
      Old.menu.k(Bool.not(Old.words.all(ws, String.to_lower(m ++ " " ++ J.Json.get_str(labels, Bool.pick(String, String.is_empty(m), "default", m), "")))), m, Old.find.go(t, ws, labels))

def Old.find.k(all: Bool, +ms: List<&2, String>, +q: String, +info: String, +prov: String) -> List<&2, String>:
  match all:
    case True{}:
      ms
    case False{}:
      Old.find.go(ms, String.split(String.to_lower(String.trim(q)), ' '), J.Json.get_obj(Old.about(info, prov), "labels"))

def Old.find(+ms: List<&2, String>, +q: String, +info: String, +prov: String) -> List<&2, String>:
  Old.find.k(String.is_empty(String.trim(q)), ms, q, info, prov)

def Old.cap() -> Nat:
  50n

def Old.shown(+info: String, +prov: String, +q: String) -> List<&2, String>:
  List.take(&2, String, Old.find(Old.menu(info, prov), q, info, prov), Old.cap())

def Old.with(+cur: String, +ms: List<&2, String>) -> List<&2, String>:
  Bool.pick(List<&2, String>, Bool.or(String.is_empty(cur), List.contains(~String, ~String.eq, ms, cur)), ms, List.append(&2, String, ms, [cur]))

def Old.view.strs(ms: List<&2, String>) -> List<&2, J.Json>:
  match ms:
    case Nil{}:
      Nil{}
    case +m <> t:
      J.Str{m} <> Old.view.strs(t)

def Old.view.go(ms: List<&2, String>, +labels: J.Json) -> List<&2, J.KV>:
  match ms:
    case Nil{}:
      Nil{}
    case +m <> t:
      J.KV{Bool.pick(String, String.is_empty(m), "default", m), J.Str{J.Json.get_str(labels, Bool.pick(String, String.is_empty(m), "default", m), "")}} <> Old.view.go(t, labels)

def Old.view(+info: String, +prov: String, +ms: List<&2, String>) -> String:
  J.Json.show(J.Json.obj([J.KV{prov, J.Json.arr(Old.view.strs(ms))},
    J.KV{"about", J.Json.obj([J.KV{prov, J.Json.obj([J.KV{"labels", J.Json.obj(Old.view.go(ms, J.Json.get_obj(Old.about(info, prov), "labels")))}])}])}]))


def strs(ms: List<&2, String>) -> List<&2, J.Json>:
  match ms:
    case Nil{}: Nil{}
    case m <> rest: J.Str{m} <> strs(rest)

def labels(ms: List<&2, String>, +metadata: J.Json) -> List<&2, J.Json>:
  match ms:
    case Nil{}: Nil{}
    case +m <> rest: J.Str{J.Json.get_str(metadata, Bool.pick(String, String.is_empty(m), "default", m), "")} <> labels(rest, metadata)

def summary(+ms: List<&2, String>, metadata: J.Json, search: Bool) -> String:
  J.Json.show(J.Json.obj([J.KV{"models", J.Json.arr(strs(ms))}, J.KV{"labels", J.Json.arr(labels(ms, metadata))}, J.KV{"search", J.Flag{search}}]))

def current(own: Bool, ms: List<&2, String>, cur: String) -> List<&2, String>:
  match own:
    case True{}: Old.with(cur, ms)
    case False{}: ms

def old(+info: String, +provider: String, cur: String, +own: String, +q: String) -> String:
  +ms = current(Bool.and(String.eq(own, "yes"), String.is_empty(String.trim(q))), Old.shown(info, provider, q), cur)
  summary(ms, J.Json.get_obj(Old.about(Old.view(info, provider, ms), provider), "labels"), Old.search(Old.menu(info, provider)))

def fresh.result(+prepared: MC.ModelCatalog) -> String:
  summary(MC.ModelCatalog.models(prepared), MC.ModelCatalog.labels(prepared), MC.ModelCatalog.search(prepared))

def fresh(info: String, provider: String, cur: String, own: String, q: String) -> String:
  fresh.result(MC.Catalog.prepare(info, provider, cur, String.eq(own, "yes"), q))
