import Base
import ../core/json.bend as J

# Diagnostic structure only: keys/reasons are fixed and values are byte
# counts. No paths, message text, draft contents or recipient identities.
def Capacity.region.hit(+node: J.Json, key: String) -> Bool:
  Bool.and(String.eq(J.Json.get_str(node, "type", ""), "element"), String.eq(J.Json.get_str(node, "key", ""), key))

def Capacity.region.go(node: J.Json, +key: String, hit: Bool) -> Nat:
  match node hit:
    case +value True{}: J.Json.bytes(value)
    case J.Obj{fields} False{}: Capacity.region.go(fields, key, False{})
    case J.Field{_, +value, rest} False{}:
      Nat.add(Capacity.region.go(value, key, Capacity.region.hit(value, key)), Capacity.region.go(rest, key, False{}))
    case J.Arr{items} False{}: Capacity.region.go(items, key, False{})
    case J.Item{+value, rest} False{}:
      Nat.add(Capacity.region.go(value, key, Capacity.region.hit(value, key)), Capacity.region.go(rest, key, False{}))
    case _ _: 0n

def Capacity.region(+node: J.Json, +key: String) -> Nat:
  Capacity.region.go(node, key, Capacity.region.hit(node, key))

def Capacity.report(+result: J.Json, capacity: Bool, logical: Nat) -> J.Json:
  +shown = J.Json.get_obj(result, "shown")
  J.Json.obj([J.KV{"reason", J.Str{Bool.pick(String, capacity, "fixed-view-bytes", "actor-capacity")}},
    J.KV{"logicalReplyBytes", J.Json.nat(logical)}, J.KV{"budgetBytes", J.Num{"229376"}},
    J.KV{"viewerBytes", J.Json.nat(Capacity.region(shown, "viewer"))},
    J.KV{"sidebarBytes", J.Json.nat(Capacity.region(shown, "sidebar"))},
    J.KV{"timelineBytes", J.Json.nat(Capacity.region(shown, "timeline"))},
    J.KV{"shownBytes", J.Json.nat(J.Json.bytes(shown))},
    J.KV{"sessionBytes", J.Json.nat(J.Json.bytes(J.Json.get_obj(result, "session")))},
    J.KV{"windowBytes", J.Json.nat(J.Json.bytes(J.Json.get_obj(result, "window")))},
    J.KV{"cmdsBytes", J.Json.nat(J.Json.bytes(Maybe.default(&2, J.Json, J.Json.get(result, "cmds"), J.Null{})))}])
