import Base
import ../../src/core/json.bend as J

# Independent arithmetic for the production Frame.text.go proof.
# No clipping implementation is duplicated here.
def impossible(b: Bool) -> Type:
  match b:
    case False{}: Unit
    case True{}: Empty

def false_true(h: {False{} == True{} : Bool}) -> Empty:
  %h : impossible(_)
  Unit{}

def add_zero(+n: Nat) -> {Nat.add(n, 0n) == n : Nat}:
  match n:
    case 0n: {==}
    case 1n+p: Equal.cong(Nat, Nat, x => 1n+x, Nat.add(p, 0n), p, add_zero(p))

def add_assoc(+a: Nat, +b: Nat, +c: Nat) -> {Nat.add(Nat.add(a, b), c) == Nat.add(a, Nat.add(b, c)) : Nat}:
  match a:
    case 0n: {==}
    case 1n+p:
      Equal.cong(Nat, Nat, x => 1n+x, Nat.add(Nat.add(p, b), c), Nat.add(p, Nat.add(b, c)), add_assoc(p, b, c))

def zero_le(+n: Nat) -> {Nat.is_le(0n, n) == True{} : Bool}:
  match n:
    case 0n: {==}
    case 1n+_: {==}

# Exact prefix cost plus a bounded tail fits the original remaining budget.
def spend(+cost: Nat, +left: Nat, +tail: Nat,
  enough: {Nat.is_le(cost, left) == True{} : Bool},
  fits: {Nat.is_le(tail, Nat.sub(left, cost)) == True{} : Bool}) ->
  {Nat.is_le(Nat.add(cost, tail), left) == True{} : Bool}:
  match cost left:
    case 0n 0n: fits
    case 0n 1n+_: fits
    case 1n+_ 0n:
      Empty.absurd({Nat.is_le(Nat.add(cost, tail), 0n) == True{} : Bool}, false_true(enough))
    case 1n+c 1n+l: spend(c, l, tail, enough, fits)

# Json.bytes.esc is an accumulator walk, not a recursive sum. This bridge
# lets a structural text induction use its exact production cost function.
def esc_acc(+s: String, +a: Nat, +b: Nat) ->
  {J.Json.bytes.esc(s, Nat.add(a, b)) == Nat.add(a, J.Json.bytes.esc(s, b)) : Nat}:
  match s:
    case SNil{}: {==}
    case SCon{Chr{+c}, +rest}:
      +cost = J.Json.bytes.chr(c)
      Equal.trans(Nat,
        J.Json.bytes.esc(rest, Nat.add(Nat.add(a, b), cost)),
        J.Json.bytes.esc(rest, Nat.add(a, Nat.add(b, cost))),
        Nat.add(a, J.Json.bytes.esc(rest, Nat.add(b, cost))),
        Equal.cong(Nat, Nat, x => J.Json.bytes.esc(rest, x), Nat.add(Nat.add(a, b), cost), Nat.add(a, Nat.add(b, cost)), add_assoc(a, b, cost)),
        esc_acc(rest, a, Nat.add(b, cost)))

def esc_cons(+c: U32, +rest: String) ->
  {J.Json.bytes.esc(SCon{Chr{c}, rest}, 0n) == Nat.add(J.Json.bytes.chr(c), J.Json.bytes.esc(rest, 0n)) : Nat}:
  +cost = J.Json.bytes.chr(c)
  Equal.trans(Nat, J.Json.bytes.esc(rest, cost), J.Json.bytes.esc(rest, Nat.add(cost, 0n)), Nat.add(cost, J.Json.bytes.esc(rest, 0n)),
    Equal.cong(Nat, Nat, x => J.Json.bytes.esc(rest, x), cost, Nat.add(cost, 0n), Equal.sym(Nat, Nat.add(cost, 0n), cost, add_zero(cost))),
    esc_acc(rest, cost, 0n))

def esc_prepend_fits(+c: U32, +rest: String, +left: Nat,
  enough: {Nat.is_le(J.Json.bytes.chr(c), left) == True{} : Bool},
  fits: {Nat.is_le(J.Json.bytes.esc(rest, 0n), Nat.sub(left, J.Json.bytes.chr(c))) == True{} : Bool}) ->
  {Nat.is_le(J.Json.bytes.esc(SCon{Chr{c}, rest}, 0n), left) == True{} : Bool}:
  Equal.trans(Bool,
    Nat.is_le(J.Json.bytes.esc(SCon{Chr{c}, rest}, 0n), left),
    Nat.is_le(Nat.add(J.Json.bytes.chr(c), J.Json.bytes.esc(rest, 0n)), left), True{},
    Equal.cong(Nat, Bool, x => Nat.is_le(x, left), J.Json.bytes.esc(SCon{Chr{c}, rest}, 0n), Nat.add(J.Json.bytes.chr(c), J.Json.bytes.esc(rest, 0n)), esc_cons(c, rest)),
    spend(J.Json.bytes.chr(c), left, J.Json.bytes.esc(rest, 0n), enough, fits))

def le_refl(+n: Nat) -> {Nat.is_le(n, n) == True{} : Bool}:
  match n:
    case 0n: {==}
    case 1n+p: le_refl(p)

def add_succ(+a: Nat, +b: Nat) -> {1n+Nat.add(a, b) == Nat.add(a, 1n+b) : Nat}:
  match a:
    case 0n: {==}
    case 1n+p: Equal.cong(Nat, Nat, x => 1n+x, 1n+Nat.add(p, b), Nat.add(p, 1n+b), add_succ(p, b))

def add_comm(+a: Nat, +b: Nat) -> {Nat.add(a, b) == Nat.add(b, a) : Nat}:
  match a:
    case 0n: Equal.sym(Nat, Nat.add(b, 0n), b, add_zero(b))
    case 1n+p:
      Equal.trans(Nat, 1n+Nat.add(p, b), 1n+Nat.add(b, p), Nat.add(b, 1n+p),
        Equal.cong(Nat, Nat, x => 1n+x, Nat.add(p, b), Nat.add(b, p), add_comm(p, b)), add_succ(b, p))

def le_trans(+a: Nat, +b: Nat, +c: Nat,
  ab: {Nat.is_le(a, b) == True{} : Bool}, bc: {Nat.is_le(b, c) == True{} : Bool}) -> {Nat.is_le(a, c) == True{} : Bool}:
  match a b c:
    case 0n _ _: zero_le(c)
    case 1n+_ 0n _: Empty.absurd({Nat.is_le(a, c) == True{} : Bool}, false_true(ab))
    case 1n+_ 1n+_ 0n: Empty.absurd({Nat.is_le(a, 0n) == True{} : Bool}, false_true(bc))
    case 1n+x 1n+y 1n+z: le_trans(x, y, z, ab, bc)

def le_add_right(+a: Nat, +b: Nat) -> {Nat.is_le(a, Nat.add(a, b)) == True{} : Bool}:
  match a:
    case 0n: zero_le(b)
    case 1n+p: le_add_right(p, b)

def le_add_left(+a: Nat, +b: Nat) -> {Nat.is_le(b, Nat.add(a, b)) == True{} : Bool}:
  Equal.trans(Bool, Nat.is_le(b, Nat.add(a, b)), Nat.is_le(b, Nat.add(b, a)), True{},
    Equal.cong(Nat, Bool, x => Nat.is_le(b, x), Nat.add(a, b), Nat.add(b, a), add_comm(a, b)), le_add_right(b, a))

def add_right_mono(+a: Nat, +b: Nat, +c: Nat, h: {Nat.is_le(a, b) == True{} : Bool}) -> {Nat.is_le(Nat.add(a, c), Nat.add(b, c)) == True{} : Bool}:
  match a b:
    case 0n _: le_add_left(b, c)
    case 1n+_ 0n: Empty.absurd({Nat.is_le(Nat.add(a, c), c) == True{} : Bool}, false_true(h))
    case 1n+x 1n+y: add_right_mono(x, y, c, h)

def add_left_mono(+base: Nat, +a: Nat, +b: Nat, h: {Nat.is_le(a, b) == True{} : Bool}) -> {Nat.is_le(Nat.add(base, a), Nat.add(base, b)) == True{} : Bool}:
  match base:
    case 0n: h
    case 1n+p: add_left_mono(p, a, b, h)

def sum_mono(+a: Nat, +b: Nat, +c: Nat, +d: Nat,
  ac: {Nat.is_le(a, c) == True{} : Bool}, bd: {Nat.is_le(b, d) == True{} : Bool}) -> {Nat.is_le(Nat.add(a, b), Nat.add(c, d)) == True{} : Bool}:
  le_trans(Nat.add(a, b), Nat.add(c, b), Nat.add(c, d), add_right_mono(a, c, b, ac), add_left_mono(c, b, d, bd))
