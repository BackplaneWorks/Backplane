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

def History.plan(+input: J.Json) -> J.Json:
  +old = History.strings(J.Json.elems(J.Json.get_obj(input, "old")))
  +fresh = History.strings(J.Json.elems(J.Json.get_obj(input, "fresh")))
  J.Json.obj([J.KV{"ids", J.Json.arr(History.json(History.plan.rows(J.Json.get_bool(input, "page", False{}), old, fresh, History.strings(J.Json.elems(J.Json.get_obj(input, "keep"))), History.strings(J.Json.elems(J.Json.get_obj(input, "visible"))), J.Json.get_bool(input, "older", True{}))))},
    J.KV{"aliases", J.Json.arr(History.alias.json(HW.History.aliases(old, fresh)))}])

