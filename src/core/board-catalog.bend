import Base
import ./json.bend as J
import ./view-page.bend as P

# Recipient-local presentation only: source identity never grants file access.
# A closed selector carries no options. Open pages keep at most 16 matches
# plus the selected board, even when that board is outside the search/page.
type CatalogBuild is Data:
  CatalogBuild{seen: Map<&2, Bool>, ids: List<&2, String>}

def Catalog.unique.add(found: Bool, +x: String, seen: Map<&2, Bool>, acc: List<&2, String>) -> CatalogBuild:
  match found:
    case True{}: CatalogBuild{seen, acc}
    case False{}: CatalogBuild{Map.set(&2, Bool, seen, x, True{}), x <> acc}

# Map.has is computed by the caller. Destructure only this parameter, then
# pass its Bool field to a filled helper; never match a computed local.
def Catalog.unique.lookup(result: Map<&2, Bool> & Bool, x: String, acc: List<&2, String>) -> CatalogBuild:
  (seen, found) = result
  Catalog.unique.add(found, x, seen, acc)

def Catalog.unique.go(xs: List<&2, String>, build: CatalogBuild) -> List<&2, String>:
  match xs build:
    case Nil{} CatalogBuild{_, acc}: List.reverse(&2, String, acc)
    case +x <> rest CatalogBuild{seen, acc}:
      Catalog.unique.go(rest, Catalog.unique.lookup(Map.has(&2, Bool, seen, x), x, acc))

def Catalog.unique(xs: List<&2, String>) -> List<&2, String>:
  Catalog.unique.go(xs, CatalogBuild{Map.new(&2, Bool), Nil{}})

def Catalog.source(thread: String, root: String, ids: List<&2, String>) -> String:
  P.Page.scope("board-catalog\n" ++ thread ++ "\n" ++ root, ids)

def Catalog.state(text: String) -> J.Json:
  P.Page.parsed(text)

def Catalog.current(+state: J.Json, +source: String) -> Bool:
  String.eq(J.Json.get_str(state, "source", ""), source)

def Catalog.query(+state: J.Json, source: String) -> String:
  Bool.pick(String, Catalog.current(state, source), String.take(J.Json.get_str(state, "query", ""), 160n), "")

def Catalog.open(+state: J.Json, source: String) -> Bool:
  Bool.and(Catalog.current(state, source), J.Json.get_bool(state, "open", False{}))

def Catalog.filter(xs: List<&2, String>, +query: String) -> List<&2, String>:
  match xs:
    case Nil{}: Nil{}
    case +x <> rest:
      List.append(&2, String, Bool.pick(List<&2, String>, String.contains(String.to_lower(x), query), [x], Nil{}), Catalog.filter(rest, query))

def Catalog.command(source: String, open: Bool, query: String, offset: Nat) -> String:
  J.Json.show(J.Json.obj([J.KV{"source", J.Str{source}}, J.KV{"open", J.Flag{open}}, J.KV{"query", J.Str{String.take(query, 160n)}}, J.KV{"offset", J.Json.nat(Nat.min(offset, 10000000n))}]))

def Catalog.page(+ids: List<&2, String>, +selected: String, +source: String, +state: J.Json, limit: Nat) -> P.Page:
  +matches = Catalog.filter(ids, String.to_lower(String.trim(Catalog.query(state, source))))
  +all = List.append(&2, String, matches, Bool.pick(List<&2, String>, Bool.and(P.Page.has(ids, selected), Bool.not(P.Page.has(matches, selected))), [selected], Nil{}))
  P.Page.make(all, source, J.Json.show(J.Json.obj([J.KV{source, J.Json.nat(J.Json.get_nat(state, "offset", 0n))}])), Nat.max(1n, Nat.min(limit, 16n)), selected)
