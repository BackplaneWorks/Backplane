import Base
import ./check.bend as C
import ../src/core/wire-retention.bend as W

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("stored wire cache releases past payloads", Nat.show(List.length(&2, String, W.WireRetention.keep(True{}, ["one", "two", "three"]))), "0")
    C.Check.str("memory reference retains ordered payloads", String.join(W.WireRetention.keep(False{}, ["one", "two", "three"]), ","), "one,two,three")
    C.Check.str("queued wire count defines durable prefix", Nat.show(W.WireRetention.durable(103n, ["public-103", "public-102", "public-101"])), "100")
    C.Check.str("first queued row follows durable prefix", String.join(W.WireRetention.pending(103n, 100n, ["public-103", "public-102", "public-101"]), ","), "public-101,public-102,public-103")
    C.Check.str("resume inside pending suffix neither duplicates nor skips", String.join(W.WireRetention.pending(103n, 101n, ["public-103", "public-102", "public-101"]), ","), "public-102,public-103")
    C.Check.str("caught up pending cursor has no rows", String.join(W.WireRetention.pending(103n, 103n, ["public-103", "public-102", "public-101"]), ","), "")
    C.Check.str("empty queue keeps advertised durable head", Nat.show(W.WireRetention.durable(100n, Nil{})), "100")
