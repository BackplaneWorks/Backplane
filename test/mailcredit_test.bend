import Base
import ./check.bend as C
import ../src/server/server.bend as S
import ../src/core/outbox.bend as O
import ../src/core/http.bend as W

def charged(w: S.Wire) -> Nat:
  match w:
    case S.WCharged{_, n}: n
    case _: 0n

def offered(n: Nat, +units: Nat, c: O.Cr) -> O.Cr:
  match n:
    case 0n: c
    case 1n+k: offered(k, units, O.Cr.offer(c, units))

def balance(c: O.Cr) -> Nat:
  match c:
    case O.Cr{_, _, _, bytes, backed, _, _, _, _, _, _, _}: Nat.sub(bytes, backed)

def replies(n: Nat, c: O.Cr, total: Nat, +text: String) -> O.Cr:
  match n:
    case 0n: c
    case 1n+k:
      +next = O.Cr.offer(c, S.Wire.text.charge(text))
      +paid = Nat.add(total, S.Wire.text.charge(text))
      replies(k, O.Cr.drained(next, 7, O.Cr.sent(next), paid), paid, text)

def main() -> IO(Unit):
  do IO<Unit>:
    +text : String = "{\"t\":\"reply\",\"id\":123,\"ok\":true,\"text\":\"日本語 hé🙂\"}"
    +n : Nat = S.Wire.text.charge(text)
    C.Check.str("text charge differs from compressed frame", Bool.show(Nat.is_gt(n, List.length(&2, U32, W.Ws.cbor(text)))), "True")
    C.Check.str("Unicode original offered charge carried", Nat.show(charged(S.Wire.charged(False{}, S.WText{text}, n))), Nat.show(n))
    C.Check.str("zero offered charge carried", Nat.show(charged(S.Wire.charged(False{}, S.WText{text}, 0n))), "0")
    C.Check.str("variable offered charge carried", Nat.show(charged(S.Wire.charged(False{}, S.WText{text}, 17n))), "17")
    C.Check.str("native text stays uncharged", Nat.show(charged(S.Wire.charged(True{}, S.WText{text}, n))), "0")
    +blocked : O.Cr = offered(64n, 100000n, O.Cr.new(7, False{}, False{}, 0n))
    C.Check.str("credit can be exhausted", Bool.show(O.Cr.room(blocked)), "False")
    +paid : O.Cr = O.Cr.drained(blocked, 7, 64n, 6400000n)
    C.Check.str("success recovers room", Bool.show(O.Cr.room(paid)), "True")
    C.Check.str("cumulative report is not refunded twice", Nat.show(balance(O.Cr.offer(O.Cr.drained(paid, 7, 64n, 6400000n), 100000n))), "100000")
    +c : O.Cr = replies(50000n, O.Cr.new(7, False{}, False{}, 0n), 0n, "{\"t\":\"reply\",\"id\":123,\"ok\":true}")
    C.Check.str("50k RPCs leave no phantom byte credit", Nat.show(balance(c)), "0")
    C.Check.str("a fully drained RPC client can catch up", Bool.show(O.Cr.room(c)), "True")
