import Base
import ../src/core/http.bend as W

# An upload's head taken by the production Http.take, in the emitted
# JavaScript (test/tools/http_take.mjs runs each on a small stack):
# POST /attach writes the bytes that came with the request's head, which
# can be a whole image, and List.take kept a frame per byte, faulting the
# hub's 64 MiB stack on macOS whenever an image was attached.

# n bytes of b in front of acc, one tail call each
def body(n: Nat, +b: U32, acc: List<&2, U32>) -> List<&2, U32>:
  match n:
    case 0n:
      acc
    case 1n+k:
      body(k, b, b <> acc)

# a length in tail calls (the test's own, so only Att.head is measured)
def len(xs: List<&2, U32>, n: Nat) -> Nat:
  match xs:
    case Nil{}:
      n
    case _ <> t:
      len(t, Nat.add(n, 1n))

# the length of the first `take` bytes of an n-byte body
def head_len(n: Nat, take: Nat) -> Nat:
  len(W.Http.take(body(n, 7, Nil{}), take), 0n)

def show(bs: List<&2, U32>, acc: String) -> String:
  match bs:
    case Nil{}:
      acc
    case b <> t:
      show(t, acc ++ U32.show(b) ++ ",")

# the first two of [1, 2, 3, 4], in order
def order() -> String:
  show(W.Http.take([1, 2, 3, 4], 2n), "")
