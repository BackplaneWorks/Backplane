import Base
import ./check.bend as C
import ../src/core/http.bend as W

# HTTP and WebSocket bodies far longer than Bun's stack holds as plain
# recursion (core/http.bend's Http.count, Ws.unmask.go, Ws.unmask.take.go):
# a 500 KB response is measured, a 300 KB masked frame decodes to itself.

def bytes.go(+n: Nat, acc: List<&2, U32>) -> List<&2, U32>:
  match n:
    case 0n:
      acc
    case 1n+k:
      bytes.go(k, U32.from_nat(Nat.mod(k, 251n)) <> acc)

def same.go(a: List<&2, U32>, b: List<&2, U32>, ok: Bool) -> Bool:
  match a b:
    case Nil{} Nil{}:
      ok
    case x <> xt y <> yt:
      same.go(xt, yt, Bool.and(ok, U32.is_eq(x, y)))
    case _ _:
      False{}

def payload(g: Maybe<&2, W.Got>) -> List<&2, U32>:
  match g:
    case Some{W.Got{W.Frame{_, _, p}, _}}:
      p
    case None{}:
      Nil{}

# the header's text: the bytes before the first blank line, as chars
def head.go(bs: List<&2, U32>, acc: String) -> String:
  match bs:
    case Nil{}:
      String.reverse(acc)
    case 13 <> 10 <> 13 <> 10 <> _:
      String.reverse(acc)
    case b <> t:
      head.go(t, SCon{Chr{b}, acc})

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("a 500 KB response's length", Bool.show(String.contains(head.go(W.Http.response("200 OK", "text/plain", bytes.go(500000n, []), ""), ""), "Content-Length: 500000")), "True")
    C.Check.str("a 300 KB body counted", Nat.show(W.Http.count(bytes.go(300000n, []))), "300000")
    C.Check.str("a 300 KB masked frame decodes to itself", Bool.show(same.go(payload(W.Ws.decode(W.Ws.encode_masked(2, bytes.go(300000n, [])))), bytes.go(300000n, []), True{})), "True")
