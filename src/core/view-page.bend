import Base
import ./model.bend as M
import ./json.bend as J
import ./sha256.bend as S
import ./utf8.bend as U8

# Read-only presentation positions, never canonical data or action authority.
# Scope hashes the complete ordered source; membership changes start anew.
# The private actor retains at most 32 bounded offsets, outside Session.info.
type Page is Data:
  Page{scope: String, total: Nat, start: Nat, limit: Nat, ids: List<&2, String>}

def Page.json(+ids: List<&2, String>) -> List<&2, J.Json>:
  match ids:
    case Nil{}: Nil{}
    case id <> rest: J.Str{id} <> Page.json(rest)

def Page.scope(+kind: String, +ids: List<&2, String>) -> String:
  S.Hex.encode(S.Sha256.hash(U8.Utf8.encode("backplane.view-list.v1\n" ++ kind ++ "\n" ++ J.Json.show(J.Json.arr(Page.json(ids))))))

def Page.has(+ids: List<&2, String>, +id: String) -> Bool:
  match ids:
    case Nil{}: False{}
    case h <> rest: Bool.or(String.eq(h, id), Page.has(rest, id))

def Page.parsed(+text: String) -> J.Json:
  Maybe.default(&2, J.Json, J.Json.parse(text), J.Json.obj(Nil{}))

def Page.make(+ids: List<&2, String>, +scope: String, +positions: String, +limit: Nat, +selected: String) -> Page:
  +total = List.length(&2, String, ids)
  +wanted = J.Json.get_nat(Page.parsed(positions), scope, 0n)
  +start = Nat.mul(Nat.div(Bool.pick(Nat, Nat.is_lt(wanted, total), wanted, Nat.sub(total, 1n)), limit), limit)
  +ordinary = List.take(&2, String, List.drop(&2, String, ids, start), limit)
  +extra = Bool.and(Bool.not(String.is_empty(selected)), Bool.and(Page.has(ids, selected), Bool.not(Page.has(ordinary, selected))))
  Page{scope, total, start, limit, List.append(&2, String, ordinary, Bool.pick(List<&2, String>, extra, [selected], Nil{}))}

def Page.all(+ids: List<&2, String>) -> Page:
  Page{"", List.length(&2, String, ids), 0n, List.length(&2, String, ids), ids}

def Page.on(+enabled: Bool, +ids: List<&2, String>, +kind: String, +positions: String, +limit: Nat, +selected: String) -> Page:
  match enabled:
    case False{}: Page.all(ids)
    case True{}: Page.make(ids, Page.scope(kind, ids), positions, Bool.pick(Nat, Nat.is_eq(limit, 0n), 1n, limit), selected)

def Page.limit(+kind: String, +budget: String) -> Nat:
  +normal = Bool.pick(Nat, String.eq(kind, "projects"), 8n, 16n)
  +small = M.Switch.num(String.take(budget, 2n), 0n)
  Bool.pick(Nat, Bool.and(Nat.is_gt(small, 0n), Nat.is_lt(small, normal)), small, normal)

def Page.of(+st: M.State, +kind: String, +ids: List<&2, String>, +selected: String) -> Page:
  Page.on(String.eq(M.Setting.get(st, "local.view.lazy", ""), "1"), ids, kind,
    M.Setting.get(st, "local.view.pages", "{}"), Page.limit(kind, M.Setting.get(st, "local.view.budget", "")), selected)

def Page.info.value(pair: Map<&2, String> & String) -> String:
  (_, value) = pair
  value

def Page.info.get(+m: Map<&2, String>, +key: String) -> String:
  Page.info.value(Map.get(String, "", m, key))

def Page.info(+info: Map<&2, String>, +kind: String, +ids: List<&2, String>, +selected: String) -> Page:
  Page.on(String.eq(Page.info.get(info, "local.view.lazy"), "1"), ids, kind,
    Page.info.get(info, "local.view.pages"), Page.limit(kind, Page.info.get(info, "local.view.budget")), selected)

def Page.ids(+p: Page) -> List<&2, String>:
  match p:
    case Page{_, _, _, _, ids}: ids

def Page.state(+on: Bool, +st: M.State, +info: Map<&2, String>) -> M.State:
  match on:
    case False{}: st
    case True{}: M.State.setting(M.State.setting(M.State.setting(st, "local.view.lazy", "1"), "local.view.pages", Page.info.get(info, "local.view.pages")), "local.view.budget", Page.info.get(info, "local.view.budget"))

# Positions may choose a read-only slice only. Their size cannot change the
# budget, owner, selected thread, draft, commands or canonical state.
def Page.positions.one(+same: Bool, +key: String, +value: String, +rest: J.Json) -> J.Json:
  match same:
    case True{}: rest
    case False{}: J.Field{key, J.Num{value}, rest}

def Page.positions.keep(+fields: J.Json, +scope: String, +left: Nat) -> J.Json:
  match fields left:
    case _ 0n: J.End{}
    case J.Field{+key, J.Num{value}, rest} 1n+n:
      Page.positions.one(String.eq(scope, key), key, value, Page.positions.keep(rest, scope, n))
    case _ _: J.End{}

# Defined separately so the shrinking walk never evaluates both branches.
def Page.position.text(+scope: String) -> Bool:
  Bool.and(Nat.is_eq(String.length(scope), 64n), Maybe.is_some(&2, List<&2, U32>, S.Hex.decode(scope)))

def Page.positions.fields(+j: J.Json, +scope: String) -> J.Json:
  match j:
    case J.Obj{fields}: Page.positions.keep(fields, scope, 31n)
    case _: J.End{}

def Page.positions.valid(+valid: Bool, +old: String, +scope: String, +offset: Nat) -> String:
  match valid:
    case False{}: old
    case True{}: J.Json.show(J.Obj{J.Field{scope, J.Json.nat(offset), Page.positions.fields(Page.parsed(old), scope)}})

def Page.positions(+old: String, +value: String) -> String:
  +p = Page.parsed(value)
  +scope = J.Json.get_str(p, "scope", "")
  +offset = J.Json.get_nat(p, "offset", 0n)
  Page.positions.valid(Bool.and(Nat.is_le(String.length(value), 256n), Bool.and(Page.position.text(scope), Nat.is_le(offset, 4294967295n))), old, scope, offset)
