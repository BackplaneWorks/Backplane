import Base
import ../core/json.bend as J
import ../core/history-window.bend as HW

def History.capacity(+input: J.Json) -> Bool:
  HW.History.within(J.Json.get_nat(input, "rows", 0n), J.Json.get_nat(input, "bytes", 0n))

def History.strings(xs: List<&2, J.Json>) -> List<&2, String>:
  match xs:
    case Nil{}: Nil{}
    case J.Str{s} <> rest: s <> History.strings(rest)
    case _ <> rest: History.strings(rest)

def History.json(xs: List<&2, String>) -> List<&2, J.Json>:
  match xs:
    case Nil{}: Nil{}
    case id <> rest: J.Str{id} <> History.json(rest)

def History.alias.json(xs: List<&2, HW.Alias>) -> List<&2, J.Json>:
  match xs:
    case Nil{}: Nil{}
    case HW.Alias{from, to} <> rest:
      J.Json.obj([J.KV{"from", J.Str{from}}, J.KV{"to", J.Str{to}}]) <> History.alias.json(rest)

def History.plan.rows(page: Bool, old: List<&2, String>, fresh: List<&2, String>, keep: List<&2, String>, visible: List<&2, String>, older: Bool) -> List<&2, String>:
  match page:
    case True{}: HW.History.page(old, fresh, keep, visible, older)
    case False{}: HW.History.refresh(old, fresh, keep, visible, older)

def History.viewport(+input: J.Json) -> Bool:
  HW.History.viewport(History.strings(J.Json.elems(J.Json.get_obj(input, "ids"))), J.Json.get_str(input, "first", ""),
    J.Json.get_str(input, "last", ""), J.Json.get_str(input, "id", ""))

# Mob.fold emits id=fold:<head> and value=<toggle><head>. Only a previously
# retained/renamed fold has different displayed and canonical heads. That
# action identity survives even when the next bounded page clips its tail.
def History.fold.toggle(open: J.Json) -> String:
  match open:
    case J.Flag{True{}}: "0"
    case J.Flag{False{}}: "1"
    case _: ""

def History.fold.head(+row: J.Json) -> String:
  +id = J.Json.get_str(row, "id", "")
  +value = J.Json.get_str(row, "value", "")
  +toggle = History.fold.toggle(J.Json.get_obj(row, "open"))
  Bool.pick(String, Bool.and(String.eq(J.Json.get_str(row, "kind", ""), "fold"),
    Bool.and(String.starts_with(id, "fold:"), Bool.and(Nat.is_gt(String.length(id), 5n),
      Bool.and(Nat.is_eq(String.length(toggle), 1n), Bool.and(String.starts_with(value, toggle), Nat.is_gt(String.length(value), 1n)))))), "fold:" ++ String.drop(value, 1n), "")

def History.fold.current(rows: List<&2, J.Json>, +from: String) -> Bool:
  match rows:
    case Nil{}: False{}
    case +row <> rest:
      Bool.or(Bool.and(String.eq(J.Json.get_str(row, "id", ""), from), String.eq(History.fold.head(row), from)), History.fold.current(rest, from))

def History.lineage(rows: List<&2, J.Json>, +fresh: List<&2, J.Json>) -> List<&2, HW.Alias>:
  match rows:
    case Nil{}: Nil{}
    case +row <> rest:
      +from = History.fold.head(row)
      HW.History.aliases.k(History.fold.current(fresh, from), from, J.Json.get_str(row, "id", ""), History.lineage(rest, fresh))

def History.plan(+input: J.Json) -> J.Json:
  +old = History.strings(J.Json.elems(J.Json.get_obj(input, "old")))
  +fresh = History.strings(J.Json.elems(J.Json.get_obj(input, "fresh")))
  +aliases = HW.History.aliases.using(old, fresh, History.lineage(J.Json.elems(J.Json.get_obj(input, "oldRows")), J.Json.elems(J.Json.get_obj(input, "freshRows"))))
  J.Json.obj([J.KV{"ids", J.Json.arr(History.json(History.plan.rows(J.Json.get_bool(input, "page", False{}), old, HW.History.renamed(fresh, aliases), History.strings(J.Json.elems(J.Json.get_obj(input, "keep"))), History.strings(J.Json.elems(J.Json.get_obj(input, "visible"))), J.Json.get_bool(input, "older", True{}))))},
    J.KV{"aliases", J.Json.arr(History.alias.json(aliases))}])

