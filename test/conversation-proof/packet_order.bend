import Base
import ../../src/core/json.bend as J
import ./escaped_cost.bend as E
import ./json_bounds.bend as B
import ./packet_schema.bend as S

def go_mono(+a: J.Json, +b: J.Json, +first: Bool, +x: Nat, +y: Nat,
  values: {Nat.is_le(J.Json.bytes.go(a, first, 0n), J.Json.bytes.go(b, first, 0n)) == True{} : Bool},
  starts: {Nat.is_le(x, y) == True{} : Bool}) ->
  {Nat.is_le(J.Json.bytes.go(a, first, x), J.Json.bytes.go(b, first, y)) == True{} : Bool}:
  +ca = J.Json.bytes.go(a, first, 0n)
  +cb = J.Json.bytes.go(b, first, 0n)
  Equal.trans(Bool, Nat.is_le(J.Json.bytes.go(a, first, x), J.Json.bytes.go(b, first, y)), Nat.is_le(Nat.add(x, ca), Nat.add(y, cb)), True{},
    Equal.trans(Bool, Nat.is_le(J.Json.bytes.go(a, first, x), J.Json.bytes.go(b, first, y)), Nat.is_le(Nat.add(x, ca), J.Json.bytes.go(b, first, y)), Nat.is_le(Nat.add(x, ca), Nat.add(y, cb)),
      Equal.cong(Nat, Bool, n => Nat.is_le(n, J.Json.bytes.go(b, first, y)), J.Json.bytes.go(a, first, x), Nat.add(x, ca), B.at_zero(a, first, x)),
      Equal.cong(Nat, Bool, n => Nat.is_le(Nat.add(x, ca), n), J.Json.bytes.go(b, first, y), Nat.add(y, cb), B.at_zero(b, first, y))), E.sum_mono(x, ca, y, cb, starts, values))

def field_mono(+key: String, +av: J.Json, +bv: J.Json, +at: J.Json, +bt: J.Json, +first: Bool,
  vh: {Nat.is_le(J.Json.bytes(av), J.Json.bytes(bv)) == True{} : Bool},
  th: {Nat.is_le(J.Json.bytes.go(at, False{}, 0n), J.Json.bytes.go(bt, False{}, 0n)) == True{} : Bool}) ->
  {Nat.is_le(J.Json.bytes.go(J.Field{key, av, at}, first, 0n), J.Json.bytes.go(J.Field{key, bv, bt}, first, 0n)) == True{} : Bool}:
  +prefix = B.prefix(key, first)
  go_mono(at, bt, False{}, J.Json.bytes.go(av, True{}, prefix), J.Json.bytes.go(bv, True{}, prefix), th, go_mono(av, bv, True{}, prefix, prefix, vh, E.le_refl(prefix)))

def obj_mono(+a: J.Json, +b: J.Json,
  +h: {Nat.is_le(J.Json.bytes.go(a, True{}, 0n), J.Json.bytes.go(b, True{}, 0n)) == True{} : Bool}) ->
  {Nat.is_le(J.Json.bytes(J.Obj{a}), J.Json.bytes(J.Obj{b})) == True{} : Bool}:
  E.add_right_mono(J.Json.bytes.go(a, True{}, 0n), J.Json.bytes.go(b, True{}, 0n), 2n, h)

def result_fields(+rid: J.Json, +shown: J.Json) -> J.Json:
  J.Field{"shown", shown, J.Field{"controlId", rid, J.End{}}}

def result_mono(+a: J.Json, +b: J.Json, +shown: J.Json,
  +h: {Nat.is_le(J.Json.bytes(a), J.Json.bytes(b)) == True{} : Bool}) ->
  {Nat.is_le(J.Json.bytes(J.Obj{result_fields(a, shown)}), J.Json.bytes(J.Obj{result_fields(b, shown)})) == True{} : Bool}:
  obj_mono(result_fields(a, shown), result_fields(b, shown),
    field_mono("shown", shown, shown, J.Field{"controlId", a, J.End{}}, J.Field{"controlId", b, J.End{}}, True{}, E.le_refl(J.Json.bytes(shown)),
      field_mono("controlId", a, b, J.End{}, J.End{}, False{}, h, {==})))

def outer_tail(+rid: J.Json, +shown: J.Json) -> J.Json:
  J.Field{"id", rid, J.Field{"ok", J.Flag{True{}}, J.Field{"result", J.Obj{result_fields(rid, shown)}, J.End{}}}}

def envelope_mono(+a: J.Json, +b: J.Json, +shown: J.Json,
  +h: {Nat.is_le(J.Json.bytes(a), J.Json.bytes(b)) == True{} : Bool}) ->
  {Nat.is_le(J.Json.bytes(S.packet(a, shown)), J.Json.bytes(S.packet(b, shown))) == True{} : Bool}:
  obj_mono(J.Field{"t", J.Str{"reply"}, outer_tail(a, shown)}, J.Field{"t", J.Str{"reply"}, outer_tail(b, shown)},
    field_mono("t", J.Str{"reply"}, J.Str{"reply"}, outer_tail(a, shown), outer_tail(b, shown), True{}, {==},
      field_mono("id", a, b, J.Field{"ok", J.Flag{True{}}, J.Field{"result", J.Obj{result_fields(a, shown)}, J.End{}}}, J.Field{"ok", J.Flag{True{}}, J.Field{"result", J.Obj{result_fields(b, shown)}, J.End{}}}, False{}, h,
        field_mono("ok", J.Flag{True{}}, J.Flag{True{}}, J.Field{"result", J.Obj{result_fields(a, shown)}, J.End{}}, J.Field{"result", J.Obj{result_fields(b, shown)}, J.End{}}, False{}, {==},
          field_mono("result", J.Obj{result_fields(a, shown)}, J.Obj{result_fields(b, shown)}, J.End{}, J.End{}, False{}, result_mono(a, b, shown, h), {==})))))
