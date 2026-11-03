import Base
import ../../src/core/json.bend as J
import ./escaped_cost.bend as E

# Arithmetic over the actual Base.Nat.divmod.go implementation. Six phases
# encode its remaining/used counters; they are proof states, not a new divider.
type Phase is Data:
  P0{}
  P1{}
  P2{}
  P3{}
  P4{}
  P5{}

def remaining(p: Phase) -> Nat:
  match p:
    case P0{}: 5n
    case P1{}: 4n
    case P2{}: 3n
    case P3{}: 2n
    case P4{}: 1n
    case P5{}: 0n

def used(p: Phase) -> Nat:
  match p:
    case P0{}: 0n
    case P1{}: 1n
    case P2{}: 2n
    case P3{}: 3n
    case P4{}: 4n
    case P5{}: 5n

def quotient(n: Nat, m: Nat, d: Nat, r: Nat) -> Nat:
  Pair.fst(Nat, Nat, Nat.divmod.go(n, m, d, r))

def carry_budget(+d: Nat, +n: Nat) -> {Nat.add(Nat.mul(1n+d, 6n), n) == Nat.add(Nat.mul(d, 6n), 6n+n) : Nat}:
  +base = Nat.mul(d, 6n)
  Equal.trans(Nat, Nat.add(Nat.add(6n, base), n), Nat.add(Nat.add(base, 6n), n), Nat.add(base, 6n+n),
    Equal.cong(Nat, Nat, x => Nat.add(x, n), Nat.add(6n, base), Nat.add(base, 6n), E.add_comm(6n, base)), E.add_assoc(base, 6n, n))

def quota(+n: Nat, +phase: Phase, +d: Nat) ->
  {Nat.is_le(Nat.mul(quotient(n, remaining(phase), d, used(phase)), 6n), Nat.add(Nat.mul(d, 6n), Nat.add(used(phase), n))) == True{} : Bool}:
  match n:
    case 0n:
      +base = Nat.mul(d, 6n)
      Equal.trans(Bool, Nat.is_le(base, Nat.add(base, Nat.add(used(phase), 0n))), Nat.is_le(base, Nat.add(base, used(phase))), True{},
        Equal.cong(Nat, Bool, x => Nat.is_le(base, Nat.add(base, x)), Nat.add(used(phase), 0n), used(phase), E.add_zero(used(phase))), E.le_add_right(base, used(phase)))
    case 1n+rest:
      match phase:
        case P0{}: quota(rest, P1{}, d)
        case P1{}: quota(rest, P2{}, d)
        case P2{}: quota(rest, P3{}, d)
        case P3{}: quota(rest, P4{}, d)
        case P4{}: quota(rest, P5{}, d)
        case P5{}:
          +q = Nat.mul(quotient(rest, 5n, 1n+d, 0n), 6n)
          Equal.trans(Bool, Nat.is_le(q, Nat.add(Nat.mul(d, 6n), 6n+rest)), Nat.is_le(q, Nat.add(Nat.mul(1n+d, 6n), rest)), True{},
            Equal.cong(Nat, Bool, x => Nat.is_le(q, x), Nat.add(Nat.mul(d, 6n), 6n+rest), Nat.add(Nat.mul(1n+d, 6n), rest),
              Equal.sym(Nat, Nat.add(Nat.mul(1n+d, 6n), rest), Nat.add(Nat.mul(d, 6n), 6n+rest), carry_budget(d, rest))), quota(rest, P0{}, 1n+d))

def div_six(+n: Nat) -> {Nat.is_le(Nat.mul(Nat.div(n, 6n), 6n), n) == True{} : Bool}:
  quota(n, P0{}, 0n)

def pick_six(k: Bool, +a: Nat, +b: Nat,
  ha: {Nat.is_le(a, 6n) == True{} : Bool}, hb: {Nat.is_le(b, 6n) == True{} : Bool}) -> {Nat.is_le(Bool.pick(Nat, k, a, b), 6n) == True{} : Bool}:
  match k:
    case True{}: ha
    case False{}: hb

def chr_slow(+c: U32) -> {Nat.is_le(J.Json.bytes.chr.slow(c), 6n) == True{} : Bool}:
  pick_six(U32.is_lt(c, 32), Bool.pick(Nat, Bool.or(U32.is_eq(c, 10), Bool.or(U32.is_eq(c, 13), U32.is_eq(c, 9))), 2n, 6n),
    Bool.pick(Nat, Bool.or(U32.is_eq(c, 34), U32.is_eq(c, 92)), 2n, Bool.pick(Nat, U32.is_lt(c, 128), 1n, Bool.pick(Nat, U32.is_lt(c, 2048), 2n, Bool.pick(Nat, U32.is_lt(c, 65536), 3n, 4n)))),
    pick_six(Bool.or(U32.is_eq(c, 10), Bool.or(U32.is_eq(c, 13), U32.is_eq(c, 9))), 2n, 6n, {==}, {==}),
    pick_six(Bool.or(U32.is_eq(c, 34), U32.is_eq(c, 92)), 2n, Bool.pick(Nat, U32.is_lt(c, 128), 1n, Bool.pick(Nat, U32.is_lt(c, 2048), 2n, Bool.pick(Nat, U32.is_lt(c, 65536), 3n, 4n))), {==},
      pick_six(U32.is_lt(c, 128), 1n, Bool.pick(Nat, U32.is_lt(c, 2048), 2n, Bool.pick(Nat, U32.is_lt(c, 65536), 3n, 4n)), {==},
        pick_six(U32.is_lt(c, 2048), 2n, Bool.pick(Nat, U32.is_lt(c, 65536), 3n, 4n), {==}, pick_six(U32.is_lt(c, 65536), 3n, 4n, {==}, {==})))))

def chr_fast(plain: Bool, +c: U32) -> {Nat.is_le(J.Json.bytes.chr.k(plain, c), 6n) == True{} : Bool}:
  match plain:
    case True{}: {==}
    case False{}: chr_slow(c)

def chr_six(+c: U32) -> {Nat.is_le(J.Json.bytes.chr(c), 6n) == True{} : Bool}:
  chr_fast(Bool.and(U32.is_ge(c, 32), Bool.and(U32.is_lt(c, 128), Bool.and(U32.is_ne(c, 34), U32.is_ne(c, 92)))), c)

def take_cost(+s: String, +n: Nat) -> {Nat.is_le(J.Json.bytes.esc(String.take(s, n), 0n), Nat.mul(n, 6n)) == True{} : Bool}:
  match s n:
    case SNil{} _: E.zero_le(Nat.mul(n, 6n))
    case SCon{_, _} 0n: {==}
    case SCon{Chr{+c}, +rest} 1n+k:
      +tail = String.take(rest, k)
      Equal.trans(Bool, Nat.is_le(J.Json.bytes.esc(SCon{Chr{c}, tail}, 0n), Nat.mul(1n+k, 6n)),
        Nat.is_le(Nat.add(J.Json.bytes.chr(c), J.Json.bytes.esc(tail, 0n)), Nat.add(6n, Nat.mul(k, 6n))), True{},
        Equal.cong(Nat, Bool, x => Nat.is_le(x, Nat.mul(1n+k, 6n)), J.Json.bytes.esc(SCon{Chr{c}, tail}, 0n), Nat.add(J.Json.bytes.chr(c), J.Json.bytes.esc(tail, 0n)), E.esc_cons(c, tail)),
        E.sum_mono(J.Json.bytes.chr(c), J.Json.bytes.esc(tail, 0n), 6n, Nat.mul(k, 6n), chr_six(c), take_cost(rest, k)))

# This expression is the exact production Frame.text body. The integration
# theorem will name that production def through the frozen source import.
def clip_expression(+s: String, +left: Nat) ->
  {Nat.is_le(J.Json.bytes.esc(String.take(s, Nat.div(left, 6n)), 0n), left) == True{} : Bool}:
  E.le_trans(J.Json.bytes.esc(String.take(s, Nat.div(left, 6n)), 0n), Nat.mul(Nat.div(left, 6n), 6n), left, take_cost(s, Nat.div(left, 6n)), div_six(left))

def take_drop(+s: String, +n: Nat) -> {String.take(s, n) ++ String.drop(s, n) == s : String}:
  match s n:
    case SNil{} _: {==}
    case SCon{_, _} 0n: {==}
    case SCon{+c, +rest} 1n+k:
      Equal.cong(String, String, x => SCon{c, x}, String.take(rest, k) ++ String.drop(rest, k), rest, take_drop(rest, k))

def le_max_left(+a: Nat, +b: Nat) -> {Nat.is_le(a, Nat.max(a, b)) == True{} : Bool}:
  match a b:
    case 0n _: E.zero_le(b)
    case 1n+_ 0n: E.le_refl(a)
    case 1n+x 1n+y: le_max_left(x, y)

def le_max_right(+a: Nat, +b: Nat) -> {Nat.is_le(b, Nat.max(a, b)) == True{} : Bool}:
  match a b:
    case 0n _: E.le_refl(b)
    case 1n+_ 0n: E.zero_le(a)
    case 1n+x 1n+y: le_max_right(x, y)

def max_eq(+a: Nat, +b: Nat, h: {Nat.is_le(a, b) == True{} : Bool}) -> {Nat.max(a, b) == b : Nat}:
  match a b:
    case 0n _: {==}
    case 1n+_ 0n: Empty.absurd({Nat.max(a, 0n) == 0n : Nat}, E.false_true(h))
    case 1n+x 1n+y: Equal.cong(Nat, Nat, n => 1n+n, Nat.max(x, y), y, max_eq(x, y, h))

def quotient_from(+q: Nat, +left: Nat, h: {Nat.div(left, 6n) == q : Nat}) -> {Nat.is_le(Nat.mul(q, 6n), left) == True{} : Bool}:
  Equal.trans(Bool, Nat.is_le(Nat.mul(q, 6n), left), Nat.is_le(Nat.mul(Nat.div(left, 6n), 6n), left), True{},
    Equal.cong(Nat, Bool, x => Nat.is_le(Nat.mul(x, 6n), left), q, Nat.div(left, 6n), Equal.sym(Nat, Nat.div(left, 6n), q, h)), div_six(left))

def part_expression.k(+s: String, +left: Nat, +q: Nat, h: {Nat.div(left, 6n) == q : Nat}) ->
  {Nat.is_le(J.Json.bytes.esc(String.take(s, Nat.max(1n, q)), 0n), Nat.max(6n, left)) == True{} : Bool}:
  match q:
    case 0n:
      E.le_trans(J.Json.bytes.esc(String.take(s, 1n), 0n), 6n, Nat.max(6n, left), take_cost(s, 1n), le_max_left(6n, left))
    case 1n+p:
      E.le_trans(J.Json.bytes.esc(String.take(s, 1n+p), 0n), Nat.mul(1n+p, 6n), Nat.max(6n, left), take_cost(s, 1n+p),
        E.le_trans(Nat.mul(1n+p, 6n), left, Nat.max(6n, left), quotient_from(1n+p, left, h), le_max_right(6n, left)))

def part_expression(+s: String, +left: Nat) ->
  {Nat.is_le(J.Json.bytes.esc(String.take(s, Nat.max(1n, Nat.div(left, 6n))), 0n), Nat.max(6n, left)) == True{} : Bool}:
  part_expression.k(s, left, Nat.div(left, 6n), {==})

def part_fits(+s: String, +left: Nat, h: {Nat.is_le(6n, left) == True{} : Bool}) ->
  {Nat.is_le(J.Json.bytes.esc(String.take(s, Nat.max(1n, Nat.div(left, 6n))), 0n), left) == True{} : Bool}:
  +cost = J.Json.bytes.esc(String.take(s, Nat.max(1n, Nat.div(left, 6n))), 0n)
  Equal.trans(Bool, Nat.is_le(cost, left), Nat.is_le(cost, Nat.max(6n, left)), True{},
    Equal.cong(Nat, Bool, x => Nat.is_le(cost, x), left, Nat.max(6n, left), Equal.sym(Nat, Nat.max(6n, left), left, max_eq(6n, left, h))), part_expression(s, left))

def max_take_nonempty(+c: Char, +tail: String, q: Nat) -> {String.is_empty(String.take(SCon{c, tail}, Nat.max(1n, q))) == False{} : Bool}:
  match q:
    case 0n: {==}
    case 1n+_: {==}
