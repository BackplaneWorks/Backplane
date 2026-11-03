import Base
import ../../src/core/json.bend as J
import ../../src/core/thin.bend as T
import ../../src/web/dom.bend as D
import ./clip_bounds.bend as C
import ../../src/server/conversation-frame.bend as F

# Proofs directly about the verbatim frozen production functions. Integration
# changes only F's module import to the owner's actual conversation-frame.
def text_bound(+s: String, +left: Nat) -> {Nat.is_le(J.Json.bytes.esc(F.Frame.text(s, left), 0n), left) == True{} : Bool}:
  C.clip_expression(s, left)

def part_bound(+s: String, +left: Nat, h: {Nat.is_le(6n, left) == True{} : Bool}) ->
  {Nat.is_le(J.Json.bytes.esc(F.Part.text(F.Part.take(s, left)), 0n), left) == True{} : Bool}:
  C.part_fits(s, left, h)

def part_32768(+s: String) -> {Nat.is_le(J.Json.bytes.esc(F.Part.text(F.Part.take(s, 32768n)), 0n), 32768n) == True{} : Bool}:
  part_bound(s, 32768n, {==})

def part_lossless(+s: String, +left: Nat) -> {F.Part.text(F.Part.take(s, left)) ++ F.Part.rest(F.Part.take(s, left)) == s : String}:
  C.take_drop(s, Nat.max(1n, Nat.div(left, 6n)))

def part_progress(+c: Char, +tail: String, +left: Nat) -> {String.is_empty(F.Part.text(F.Part.take(SCon{c, tail}, left))) == False{} : Bool}:
  C.max_take_nonempty(c, tail, Nat.div(left, 6n))

def rows_count(ids: List<&2, String>, +index: T.Index) -> {List.length(&2, D.Node, F.Frame.rows(ids, index)) == List.length(&2, String, ids) : Nat}:
  match ids:
    case Nil{}: {==}
    case id <> +rest:
      Equal.cong(Nat, Nat, x => 1n+x, List.length(&2, D.Node, F.Frame.rows(rest, index)), List.length(&2, String, rest), rows_count(rest, index))

def rows_empty(ids: List<&2, String>, index: T.Index) -> {List.is_empty(&2, D.Node, F.Frame.rows(ids, index)) == List.is_empty(&2, String, ids) : Bool}:
  match ids:
    case Nil{}: {==}
    case _ <> _: {==}
