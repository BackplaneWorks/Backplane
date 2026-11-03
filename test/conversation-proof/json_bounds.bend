import Base
import ../../src/core/json.bend as J
import ./escaped_cost.bend as E
import ./json_cost.bend as C

# Compositional bounds for the actual accumulator, including field punctuation.
def at_zero(+j: J.Json, +first: Bool, +n: Nat) ->
  {J.Json.bytes.go(j, first, n) == Nat.add(n, J.Json.bytes.go(j, first, 0n)) : Nat}:
  Equal.trans(Nat, J.Json.bytes.go(j, first, n), J.Json.bytes.go(j, first, Nat.add(n, 0n)), Nat.add(n, J.Json.bytes.go(j, first, 0n)),
    Equal.cong(Nat, Nat, x => J.Json.bytes.go(j, first, x), n, Nat.add(n, 0n), Equal.sym(Nat, Nat.add(n, 0n), n, E.add_zero(n))), C.go_acc(j, first, n, 0n))

def at_bound(+j: J.Json, +first: Bool, +n: Nat, +cap: Nat,
  fits: {Nat.is_le(J.Json.bytes.go(j, first, 0n), cap) == True{} : Bool}) ->
  {Nat.is_le(J.Json.bytes.go(j, first, n), Nat.add(n, cap)) == True{} : Bool}:
  Equal.trans(Bool, Nat.is_le(J.Json.bytes.go(j, first, n), Nat.add(n, cap)), Nat.is_le(Nat.add(n, J.Json.bytes.go(j, first, 0n)), Nat.add(n, cap)), True{},
    Equal.cong(Nat, Bool, x => Nat.is_le(x, Nat.add(n, cap)), J.Json.bytes.go(j, first, n), Nat.add(n, J.Json.bytes.go(j, first, 0n)), at_zero(j, first, n)), E.add_left_mono(n, J.Json.bytes.go(j, first, 0n), cap, fits))

def string_bound(+s: String, +cap: Nat, fits: {Nat.is_le(J.Json.bytes.esc(s, 0n), cap) == True{} : Bool}) ->
  {Nat.is_le(J.Json.bytes(J.Str{s}), Nat.add(2n, cap)) == True{} : Bool}:
  Equal.trans(Bool, Nat.is_le(J.Json.bytes.esc(s, 2n), Nat.add(2n, cap)), Nat.is_le(Nat.add(2n, J.Json.bytes.esc(s, 0n)), Nat.add(2n, cap)), True{},
    Equal.cong(Nat, Bool, x => Nat.is_le(x, Nat.add(2n, cap)), J.Json.bytes.esc(s, 2n), Nat.add(2n, J.Json.bytes.esc(s, 0n)), E.esc_acc(s, 2n, 0n)), E.add_left_mono(2n, J.Json.bytes.esc(s, 0n), cap, fits))

def container_bound(+xs: J.Json, +cap: Nat, fits: {Nat.is_le(J.Json.bytes.go(xs, True{}, 0n), cap) == True{} : Bool}) ->
  {Nat.is_le(Nat.add(J.Json.bytes.go(xs, True{}, 0n), 2n), Nat.add(cap, 2n)) == True{} : Bool}:
  E.add_right_mono(J.Json.bytes.go(xs, True{}, 0n), cap, 2n, fits)

def prefix(+key: String, first: Bool) -> Nat:
  Nat.add(J.Json.bytes.esc(key, Nat.add(J.Json.bytes.sep(first, 0n), 2n)), 1n)

def field_bound(+key: String, +v: J.Json, +tail: J.Json, +first: Bool, +vcap: Nat, +tcap: Nat,
  vh: {Nat.is_le(J.Json.bytes(v), vcap) == True{} : Bool},
  th: {Nat.is_le(J.Json.bytes.go(tail, False{}, 0n), tcap) == True{} : Bool}) ->
  {Nat.is_le(J.Json.bytes.go(J.Field{key, v, tail}, first, 0n), Nat.add(Nat.add(prefix(key, first), vcap), tcap)) == True{} : Bool}:
  +p = prefix(key, first)
  +cost = J.Json.bytes.go(v, True{}, p)
  E.le_trans(J.Json.bytes.go(tail, False{}, cost), Nat.add(cost, tcap), Nat.add(Nat.add(p, vcap), tcap),
    at_bound(tail, False{}, cost, tcap, th), E.add_right_mono(cost, Nat.add(p, vcap), tcap, at_bound(v, True{}, p, vcap, vh)))

def item_bound(+v: J.Json, +tail: J.Json, +first: Bool, +vcap: Nat, +tcap: Nat,
  vh: {Nat.is_le(J.Json.bytes(v), vcap) == True{} : Bool},
  th: {Nat.is_le(J.Json.bytes.go(tail, False{}, 0n), tcap) == True{} : Bool}) ->
  {Nat.is_le(J.Json.bytes.go(J.Item{v, tail}, first, 0n), Nat.add(Nat.add(J.Json.bytes.sep(first, 0n), vcap), tcap)) == True{} : Bool}:
  +p = J.Json.bytes.sep(first, 0n)
  +cost = J.Json.bytes.go(v, True{}, p)
  E.le_trans(J.Json.bytes.go(tail, False{}, cost), Nat.add(cost, tcap), Nat.add(Nat.add(p, vcap), tcap),
    at_bound(tail, False{}, cost, tcap, th), E.add_right_mono(cost, Nat.add(p, vcap), tcap, at_bound(v, True{}, p, vcap, vh)))
