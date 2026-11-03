import Base
import ../../src/core/json.bend as J
import ./escaped_cost.bend as E

# Translation of the actual serializer's accumulator; no alternative size
# function is assumed equivalent to Json.bytes.
def raw_acc(+s: String, +a: Nat, +b: Nat) ->
  {J.Json.bytes.str(s, Nat.add(a, b)) == Nat.add(a, J.Json.bytes.str(s, b)) : Nat}:
  match s:
    case SNil{}: {==}
    case SCon{Chr{+c}, +rest}:
      +cost = J.Json.bytes.raw(c)
      Equal.trans(Nat, J.Json.bytes.str(rest, Nat.add(Nat.add(a, b), cost)), J.Json.bytes.str(rest, Nat.add(a, Nat.add(b, cost))), Nat.add(a, J.Json.bytes.str(rest, Nat.add(b, cost))),
        Equal.cong(Nat, Nat, n => J.Json.bytes.str(rest, n), Nat.add(Nat.add(a, b), cost), Nat.add(a, Nat.add(b, cost)), E.add_assoc(a, b, cost)), raw_acc(rest, a, Nat.add(b, cost)))

def sep_acc(first: Bool, +a: Nat, +b: Nat) ->
  {J.Json.bytes.sep(first, Nat.add(a, b)) == Nat.add(a, J.Json.bytes.sep(first, b)) : Nat}:
  match first:
    case True{}: {==}
    case False{}: E.add_assoc(a, b, 1n)

def go_acc(+j: J.Json, +first: Bool, +a: Nat, +b: Nat) ->
  {J.Json.bytes.go(j, first, Nat.add(a, b)) == Nat.add(a, J.Json.bytes.go(j, first, b)) : Nat}:
  match j:
    case J.Null{}: E.add_assoc(a, b, 4n)
    case J.Flag{flag}:
      match flag:
        case True{}: E.add_assoc(a, b, 4n)
        case False{}: E.add_assoc(a, b, 5n)
    case J.Num{s}: raw_acc(s, a, b)
    case J.Str{s}:
      Equal.trans(Nat, J.Json.bytes.esc(s, Nat.add(Nat.add(a, b), 2n)), J.Json.bytes.esc(s, Nat.add(a, Nat.add(b, 2n))), Nat.add(a, J.Json.bytes.esc(s, Nat.add(b, 2n))),
        Equal.cong(Nat, Nat, n => J.Json.bytes.esc(s, n), Nat.add(Nat.add(a, b), 2n), Nat.add(a, Nat.add(b, 2n)), E.add_assoc(a, b, 2n)), E.esc_acc(s, a, Nat.add(b, 2n)))
    case J.Arr{xs}:
      Equal.trans(Nat, Nat.add(J.Json.bytes.go(xs, True{}, Nat.add(a, b)), 2n), Nat.add(Nat.add(a, J.Json.bytes.go(xs, True{}, b)), 2n), Nat.add(a, Nat.add(J.Json.bytes.go(xs, True{}, b), 2n)),
        Equal.cong(Nat, Nat, n => Nat.add(n, 2n), J.Json.bytes.go(xs, True{}, Nat.add(a, b)), Nat.add(a, J.Json.bytes.go(xs, True{}, b)), go_acc(xs, True{}, a, b)), E.add_assoc(a, J.Json.bytes.go(xs, True{}, b), 2n))
    case J.Obj{fs}:
      Equal.trans(Nat, Nat.add(J.Json.bytes.go(fs, True{}, Nat.add(a, b)), 2n), Nat.add(Nat.add(a, J.Json.bytes.go(fs, True{}, b)), 2n), Nat.add(a, Nat.add(J.Json.bytes.go(fs, True{}, b), 2n)),
        Equal.cong(Nat, Nat, n => Nat.add(n, 2n), J.Json.bytes.go(fs, True{}, Nat.add(a, b)), Nat.add(a, J.Json.bytes.go(fs, True{}, b)), go_acc(fs, True{}, a, b)), E.add_assoc(a, J.Json.bytes.go(fs, True{}, b), 2n))
    case J.Item{+h, +t}:
      +base = J.Json.bytes.sep(first, b)
      +value = J.Json.bytes.go(h, True{}, base)
      Equal.trans(Nat, J.Json.bytes.go(t, False{}, J.Json.bytes.go(h, True{}, J.Json.bytes.sep(first, Nat.add(a, b)))),
        J.Json.bytes.go(t, False{}, J.Json.bytes.go(h, True{}, Nat.add(a, base))), Nat.add(a, J.Json.bytes.go(t, False{}, value)),
        Equal.cong(Nat, Nat, n => J.Json.bytes.go(t, False{}, J.Json.bytes.go(h, True{}, n)), J.Json.bytes.sep(first, Nat.add(a, b)), Nat.add(a, base), sep_acc(first, a, b)),
        Equal.trans(Nat, J.Json.bytes.go(t, False{}, J.Json.bytes.go(h, True{}, Nat.add(a, base))), J.Json.bytes.go(t, False{}, Nat.add(a, value)), Nat.add(a, J.Json.bytes.go(t, False{}, value)),
          Equal.cong(Nat, Nat, n => J.Json.bytes.go(t, False{}, n), J.Json.bytes.go(h, True{}, Nat.add(a, base)), Nat.add(a, value), go_acc(h, True{}, a, base)), go_acc(t, False{}, a, value)))
    case J.Field{+k, +v, +t}:
      +base = Nat.add(J.Json.bytes.esc(k, Nat.add(J.Json.bytes.sep(first, b), 2n)), 1n)
      +value = J.Json.bytes.go(v, True{}, base)
      +prefix = J.Json.bytes.sep(first, b)
      +keycost = J.Json.bytes.esc(k, Nat.add(prefix, 2n))
      +keyshift = Equal.trans(Nat, Nat.add(J.Json.bytes.esc(k, Nat.add(J.Json.bytes.sep(first, Nat.add(a, b)), 2n)), 1n), Nat.add(J.Json.bytes.esc(k, Nat.add(Nat.add(a, prefix), 2n)), 1n), Nat.add(a, base),
        Equal.cong(Nat, Nat, n => Nat.add(J.Json.bytes.esc(k, Nat.add(n, 2n)), 1n), J.Json.bytes.sep(first, Nat.add(a, b)), Nat.add(a, prefix), sep_acc(first, a, b)),
        Equal.trans(Nat, Nat.add(J.Json.bytes.esc(k, Nat.add(Nat.add(a, prefix), 2n)), 1n), Nat.add(J.Json.bytes.esc(k, Nat.add(a, Nat.add(prefix, 2n))), 1n), Nat.add(a, base),
          Equal.cong(Nat, Nat, n => Nat.add(J.Json.bytes.esc(k, n), 1n), Nat.add(Nat.add(a, prefix), 2n), Nat.add(a, Nat.add(prefix, 2n)), E.add_assoc(a, prefix, 2n)),
          Equal.trans(Nat, Nat.add(J.Json.bytes.esc(k, Nat.add(a, Nat.add(prefix, 2n))), 1n), Nat.add(Nat.add(a, keycost), 1n), Nat.add(a, base),
            Equal.cong(Nat, Nat, n => Nat.add(n, 1n), J.Json.bytes.esc(k, Nat.add(a, Nat.add(prefix, 2n))), Nat.add(a, keycost), E.esc_acc(k, a, Nat.add(prefix, 2n))), E.add_assoc(a, keycost, 1n))))
      Equal.trans(Nat, J.Json.bytes.go(t, False{}, J.Json.bytes.go(v, True{}, Nat.add(J.Json.bytes.esc(k, Nat.add(J.Json.bytes.sep(first, Nat.add(a, b)), 2n)), 1n))), J.Json.bytes.go(t, False{}, J.Json.bytes.go(v, True{}, Nat.add(a, base))), Nat.add(a, J.Json.bytes.go(t, False{}, value)),
        Equal.cong(Nat, Nat, n => J.Json.bytes.go(t, False{}, J.Json.bytes.go(v, True{}, n)), Nat.add(J.Json.bytes.esc(k, Nat.add(J.Json.bytes.sep(first, Nat.add(a, b)), 2n)), 1n), Nat.add(a, base), keyshift),
        Equal.trans(Nat, J.Json.bytes.go(t, False{}, J.Json.bytes.go(v, True{}, Nat.add(a, base))), J.Json.bytes.go(t, False{}, Nat.add(a, value)), Nat.add(a, J.Json.bytes.go(t, False{}, value)),
          Equal.cong(Nat, Nat, n => J.Json.bytes.go(t, False{}, n), J.Json.bytes.go(v, True{}, Nat.add(a, base)), Nat.add(a, value), go_acc(v, True{}, a, base)), go_acc(t, False{}, a, value)))
    case J.End{}: {==}

def go_zero(+j: J.Json, +n: Nat) -> {J.Json.bytes.go(j, True{}, n) == Nat.add(n, J.Json.bytes(j)) : Nat}:
  Equal.trans(Nat, J.Json.bytes.go(j, True{}, n), J.Json.bytes.go(j, True{}, Nat.add(n, 0n)), Nat.add(n, J.Json.bytes(j)),
    Equal.cong(Nat, Nat, x => J.Json.bytes.go(j, True{}, x), n, Nat.add(n, 0n), Equal.sym(Nat, Nat.add(n, 0n), n, E.add_zero(n))), go_acc(j, True{}, n, 0n))
