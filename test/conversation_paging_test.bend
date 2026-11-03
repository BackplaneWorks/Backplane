import Base
import ./check.bend as C
import ../src/core/json.bend as J
import ../src/core/model.bend as M
import ../src/core/thin.bend as T
import ../src/core/thin-wire.bend as W
import ../src/server/thin-body.bend as B
import ../src/server/thin-view.bend as V

def seed(+n: Nat, index: T.Index) -> T.Index:
  match n:
    case 0n: index
    case 1n+k: seed(k, T.Index.post(index, M.Msg{"m" ++ Nat.show(n), "t", M.Assistant{}, "Source", n}))

def request(limit: Nat) -> J.Json:
  J.Json.obj([J.KV{"target", V.Target.json("thread", "t")}, J.KV{"bodyPages", J.Num{"1"}}, J.KV{"viewEntries", J.Json.nat(limit)}])

def seen(ids: List<&2, String>, into: Map<&2, Bool>) -> Map<&2, Bool>:
  match ids:
    case Nil{}: into
    case id <> rest: seen(rest, Map.set(&2, Bool, into, id, True{}))

def cursor.value(c: Maybe<&2, Nat>) -> Nat:
  match c:
    case None{}: 0n
    case Some{n}: n

def walk(fuel: Nat, more: Bool, +index: T.Index, +before: Nat, +step: Nat, +limit: Nat, into: Map<&2, Bool>) -> Map<&2, Bool>:
  match fuel more:
    case 0n _: into
    case _ False{}: into
    case 1n+k True{}:
      +p = request(Bool.pick(Nat, Nat.is_eq(step, 0n), 32n, limit))
      +ids = B.Body.window.bounded(T.Index.page(index, "t", before, 32n), index, p)
      +window = V.Page.window(index, "t", p, "epoch", ids)
      +cursor = W.Cursor.read(J.Json.get_str(window, "older", ""), "epoch", V.Target.json("thread", "t"), "")
      walk(k, Maybe.is_some(&2, Nat, cursor), index, cursor.value(cursor), 1n+step, limit, seen(ids, into))

def main() -> IO(Unit):
  do IO<Unit>:
    +index : T.Index = seed(100n, T.Index.empty())
    +p : J.Json = request(8n)
    +ids : List<&2, String> = B.Body.window.bounded(T.Index.page(index, "t", 0n, 32n), index, p)
    +window : J.Json = V.Page.window(index, "t", p, "epoch", ids)
    C.Check.str("fitted page issues no speculative second cursor", J.Json.get_str(window, "olderNext", "missing"), "")
    C.Check.str("actual fitted page retains eight source rows", Nat.show(List.length(&2, String, ids)), "8")
    C.Check.str("total equals older plus shown plus newer", Nat.show(J.Json.get_nat(window, "total", 0n)), "100")
    C.Check.str("full window cannot promise a future fit", J.Json.get_str(V.Page.window(index, "t", request(32n), "epoch", T.Index.page(index, "t", 0n, 32n)), "olderNext", "missing"), "")
    C.Check.str("32 then 8 rows traverses every entry", Nat.show(List.length(&2, String, Map.keys(&2, Bool, walk(128n, True{}, index, 0n, 0n, 8n, Map.new(&2, Bool))))), "100")
    C.Check.str("32 then 2 rows traverses every entry", Nat.show(List.length(&2, String, Map.keys(&2, Bool, walk(128n, True{}, index, 0n, 0n, 2n, Map.new(&2, Bool))))), "100")
    C.Check.str("32 then 1 row traverses every entry", Nat.show(List.length(&2, String, Map.keys(&2, Bool, walk(128n, True{}, index, 0n, 0n, 1n, Map.new(&2, Bool))))), "100")
