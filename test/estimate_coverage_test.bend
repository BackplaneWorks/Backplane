import Base
import ./check.bend as C
import ../src/core/json.bend as J
import ../src/core/order.bend as O

def part(+name: String) -> J.Json:
  J.Json.obj([J.KV{"name", J.Str{name}}, J.KV{"kind", J.Str{"pcb"}}, J.KV{"path", J.Str{"board.fab.json"}}])

def priced(+name: String) -> J.Json:
  J.Json.obj([J.KV{"name", J.Str{name}}, J.KV{"kind", J.Str{"pcb"}}, J.KV{"from", J.Num{"5"}}])

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("every part priced", O.Estimate.problems([part("a"), part("b")], [priced("a"), priced("b")]), "")
    C.Check.str("missing part named", Bool.show(String.contains(O.Estimate.problems([part("a"), part("b")], [priced("a")]), "No estimate for b")), "True")
    C.Check.str("bad part named", Bool.show(String.contains(O.Estimate.problems([part("a")], [part("a")]), "board.fab.json")), "True")
    C.Check.str("wrong row rejected", Bool.show(String.is_empty(O.Estimate.problems([part("a")], [priced("b")]))), "False")
    C.Check.str("extra rows rejected", Bool.show(String.is_empty(O.Estimate.problems([part("a")], [priced("a"), priced("b")]))), "False")
    C.Check.str("complete price retained", O.Estimate.label("from $5.00", ""), "from $5.00")
    C.Check.str("partial price withheld", O.Estimate.label("from $5.00", "No estimate for b"), "estimate incomplete")
