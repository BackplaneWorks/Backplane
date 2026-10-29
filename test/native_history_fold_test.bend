import Base
import ./check.bend as C
import ../src/core/json.bend as J
import ../src/core/history-window.bend as H
import ../src/mobile/history.bend as N

def fold(+id: String, value: String, open: Bool) -> J.Json:
  J.Json.obj([J.KV{"id", J.Str{id}}, J.KV{"kind", J.Str{"fold"}}, J.KV{"value", J.Str{value}}, J.KV{"open", J.Flag{open}}])

def input(+old: List<&2, String>, fresh: List<&2, String>, rows: List<&2, J.Json>, freshRows: List<&2, J.Json>) -> J.Json:
  J.Json.obj([J.KV{"old", J.Json.arr(N.History.json(old))}, J.KV{"fresh", J.Json.arr(N.History.json(fresh))},
    J.KV{"keep", J.Json.arr(N.History.json(old))}, J.KV{"visible", J.Json.arr(N.History.json(old))},
    J.KV{"page", J.Flag{True{}}}, J.KV{"older", J.Flag{True{}}},
    J.KV{"oldRows", J.Json.arr(rows)}, J.KV{"freshRows", J.Json.arr(freshRows)}])

def ids(result: J.Json) -> String:
  String.join(N.History.strings(J.Json.elems(J.Json.get_obj(result, "ids"))), ",")

def alias(result: J.Json) -> String:
  J.Json.get_str(Maybe.default(&2, J.Json, List.head(&2, J.Json, J.Json.elems(J.Json.get_obj(result, "aliases"))), J.Null{}), "to", "")

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("page-end keeps previously proved canonical fold identity", ids(N.History.plan(input(
      ["user", "fold:item_9", "ride"], ["older", "user", "fold:item_6"],
      [fold("fold:item_9", "1item_6", False{})], [fold("fold:item_6", "1item_6", False{})]))), "older,user,fold:item_9,ride")
    C.Check.str("page-end aliases to stable displayed key", alias(N.History.plan(input(
      ["user", "fold:item_9", "ride"], ["older", "user", "fold:item_6"],
      [fold("fold:item_9", "0item_6", True{})], [fold("fold:item_6", "1item_6", False{})]))), "fold:item_9")
    C.Check.str("a new unrelated trailing fold gets no remembered alias", alias(N.History.plan(input(
      ["user", "fold:item_9", "ride"], ["older", "user", "fold:other"],
      [fold("fold:item_9", "1item_6", False{})], [fold("fold:other", "1other", False{})]))), "")
    C.Check.str("unchanged canonical head does not establish another identity", alias(N.History.plan(input(
      ["user", "fold:item_9", "ride"], ["older", "user", "fold:item_6"],
      [fold("fold:item_9", "1item_9", False{})], [fold("fold:item_6", "1item_6", False{})]))), "")
    C.Check.str("mismatched incoming canonical action rejects remembered alias", alias(N.History.plan(input(
      ["user", "fold:item_9", "ride"], ["older", "user", "fold:item_6"],
      [fold("fold:item_9", "1item_6", False{})], [fold("fold:item_6", "1unrelated", False{})]))), "")
    C.Check.str("wrong retained toggle does not establish lineage", alias(N.History.plan(input(
      ["user", "fold:item_9", "ride"], ["older", "user", "fold:item_6"],
      [fold("fold:item_9", "0item_6", False{})], [fold("fold:item_6", "1item_6", False{})]))), "")
    C.Check.str("missing open marker does not establish lineage", N.History.fold.head(J.Json.obj([
      J.KV{"id", J.Str{"fold:item_9"}}, J.KV{"kind", J.Str{"fold"}}, J.KV{"value", J.Str{"1item_6"}}])), "")
    C.Check.str("a non-fold cannot establish canonical lineage", N.History.fold.head(J.Json.obj([
      J.KV{"id", J.Str{"fold:item_9"}}, J.KV{"kind", J.Str{"assistant"}}, J.KV{"value", J.Str{"1item_6"}}, J.KV{"open", J.Flag{False{}}}])), "")
    C.Check.str("ambiguous prior targets reject all remembered aliases", Nat.show(List.length(&2, H.Alias, H.History.aliases.using(
      ["fold:a", "fold:b"], ["fold:new"], [H.Alias{"fold:new", "fold:a"}, H.Alias{"fold:new", "fold:b"}]))), "0")
    C.Check.str("lineage to an absent displayed owner is inert", Nat.show(List.length(&2, H.Alias, H.History.aliases.using(
      ["fold:other"], ["fold:new"], [H.Alias{"fold:new", "fold:old"}]))), "0")
    C.Check.str("two distinct incoming fold heads cannot collapse together", Nat.show(List.length(&2, H.Alias, H.History.aliases.using(
      ["fold:old"], ["fold:new", "fold:old"], [H.Alias{"fold:new", "fold:old"}]))), "0")
    C.Check.str("page applies exactly the returned alias plan with no cascading rename", ids(N.History.plan(input(
      ["fold:a", "fold:d", "x"], ["fold:b", "fold:c", "x"],
      [fold("fold:a", "1a", False{}), fold("fold:d", "1d", False{})],
      [fold("fold:b", "1b", False{}), fold("fold:c", "1c", False{})]))), "fold:b,fold:a,fold:d,x")
