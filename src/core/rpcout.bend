import Base

# Kept requests use canonical decimal request numbers as map keys. A
# map walks those keys as text: "10" comes before "9". Reconnects and
# pending messages need creation order, so sort active keys by their
# digit count and then their text, without reading the request bodies.
# This row is temporary; the clients keep the existing map representation.
type RpcOutItem is Data:
  RpcOutItem{key: String, digits: Nat, text: String}

def RpcOutItem.before.k(c: Cmp, a: String, b: String) -> Bool:
  match c:
    case LT{}:
      True{}
    case EQ{}:
      String.is_le(a, b)
    case GT{}:
      False{}

def RpcOutItem.before(a: RpcOutItem, b: RpcOutItem) -> Bool:
  match a b:
    case RpcOutItem{ak, an, _} RpcOutItem{bk, bn, _}:
      RpcOutItem.before.k(Nat.cmp(an, bn), ak, bk)

# Older saved phone state may still hold empty entries for answered
# requests. Drop them before counting digits or sorting.
def RpcOut.keep(empty: Bool, +key: String, text: String, rest: List<&2, RpcOutItem>) -> List<&2, RpcOutItem>:
  match empty:
    case True{}:
      rest
    case False{}:
      RpcOutItem{key, String.length(key), text} <> rest

def RpcOut.collect(m: Map<&2, String>, acc: List<&2, RpcOutItem>) -> List<&2, RpcOutItem>:
  match m:
    case MTip{}:
      acc
    case MLeaf{key, +text}:
      RpcOut.keep(String.is_empty(text), key, text, acc)
    case MNode{_, lo, hi}:
      RpcOut.collect(lo, RpcOut.collect(hi, acc))

def RpcOut.texts(xs: List<&2, RpcOutItem>, acc: List<&2, String>) -> List<&2, String>:
  match xs:
    case Nil{}:
      List.reverse(&2, String, acc)
    case RpcOutItem{_, _, text} <> rest:
      RpcOut.texts(rest, text <> acc)

# Outstanding requests only, oldest first. Gaps in the request numbers
# cost no work, and each payload is returned exactly as it was kept.
def RpcOut.values(m: Map<&2, String>) -> List<&2, String>:
  RpcOut.texts(List.sort(~RpcOutItem, ~RpcOutItem.before, RpcOut.collect(m, Nil{})), Nil{})
