import Base
import ../src/core/outbox.bend as O

# The counter join is the real writer-credit function, not event replay.
def pick_succ(k: Bool, +a: Nat, +b: Nat) -> {Bool.pick(Nat, k, 1n+a, 1n+b) == 1n+Bool.pick(Nat, k, a, b) : Nat}:
  match k:
    case False{}: {==}
    case True{}: {==}

def most_max(+a: Nat, +b: Nat) -> {O.Cr.most(a, b) == Nat.max(a, b) : Nat}:
  match a b:
    case 0n 0n: {==}
    case 0n 1n+_: {==}
    case 1n+_ 0n: {==}
    case 1n+p 1n+q:
      Equal.trans(Nat, O.Cr.most(1n+p, 1n+q), 1n+O.Cr.most(p, q), 1n+Nat.max(p, q),
        pick_succ(Nat.is_ge(p, q), p, q), Equal.cong(Nat, Nat, x => 1n+x, O.Cr.most(p, q), Nat.max(p, q), most_max(p, q)))

def max_zero(+a: Nat) -> {Nat.max(a, 0n) == a : Nat}:
  match a:
    case 0n: {==}
    case 1n+_: {==}

def max_self(+a: Nat) -> {Nat.max(a, a) == a : Nat}:
  match a:
    case 0n: {==}
    case 1n+p: Equal.cong(Nat, Nat, x => 1n+x, Nat.max(p, p), p, max_self(p))

def max_commute(+a: Nat, +b: Nat) -> {Nat.max(a, b) == Nat.max(b, a) : Nat}:
  match a b:
    case 0n _: Equal.sym(Nat, Nat.max(b, 0n), b, max_zero(b))
    case 1n+_ 0n: {==}
    case 1n+p 1n+q: Equal.cong(Nat, Nat, x => 1n+x, Nat.max(p, q), Nat.max(q, p), max_commute(p, q))

def max_absorb(+a: Nat, +b: Nat) -> {Nat.max(Nat.max(a, b), b) == Nat.max(a, b) : Nat}:
  match a b:
    case 0n _: max_self(b)
    case 1n+_ 0n: {==}
    case 1n+p 1n+q: Equal.cong(Nat, Nat, x => 1n+x, Nat.max(Nat.max(p, q), q), Nat.max(p, q), max_absorb(p, q))

def max_assoc(+a: Nat, +b: Nat, +c: Nat) -> {Nat.max(Nat.max(a, b), c) == Nat.max(a, Nat.max(b, c)) : Nat}:
  match a b c:
    case 0n _ _: {==}
    case 1n+_ 0n _: {==}
    case 1n+_ 1n+_ 0n: {==}
    case 1n+p 1n+q 1n+r:
      Equal.cong(Nat, Nat, x => 1n+x, Nat.max(Nat.max(p, q), r), Nat.max(p, Nat.max(q, r)), max_assoc(p, q, r))

def max_swap(+a: Nat, +b: Nat, +c: Nat) -> {Nat.max(Nat.max(a, b), c) == Nat.max(Nat.max(a, c), b) : Nat}:
  Equal.trans(Nat, Nat.max(Nat.max(a, b), c), Nat.max(a, Nat.max(b, c)), Nat.max(Nat.max(a, c), b), max_assoc(a, b, c),
    Equal.trans(Nat, Nat.max(a, Nat.max(b, c)), Nat.max(a, Nat.max(c, b)), Nat.max(Nat.max(a, c), b),
      Equal.cong(Nat, Nat, x => Nat.max(a, x), Nat.max(b, c), Nat.max(c, b), max_commute(b, c)),
      Equal.sym(Nat, Nat.max(Nat.max(a, c), b), Nat.max(a, Nat.max(c, b)), max_assoc(a, c, b))))

def most_nested(+a: Nat, +b: Nat, +c: Nat) -> {O.Cr.most(O.Cr.most(a, b), c) == Nat.max(Nat.max(a, b), c) : Nat}:
  Equal.trans(Nat, O.Cr.most(O.Cr.most(a, b), c), Nat.max(O.Cr.most(a, b), c), Nat.max(Nat.max(a, b), c), most_max(O.Cr.most(a, b), c),
    Equal.cong(Nat, Nat, x => Nat.max(x, c), O.Cr.most(a, b), Nat.max(a, b), most_max(a, b)))

def most_absorb(+a: Nat, +b: Nat) -> {O.Cr.most(O.Cr.most(a, b), b) == O.Cr.most(a, b) : Nat}:
  Equal.trans(Nat, O.Cr.most(O.Cr.most(a, b), b), Nat.max(Nat.max(a, b), b), O.Cr.most(a, b), most_nested(a, b, b),
    Equal.trans(Nat, Nat.max(Nat.max(a, b), b), Nat.max(a, b), O.Cr.most(a, b), max_absorb(a, b), Equal.sym(Nat, O.Cr.most(a, b), Nat.max(a, b), most_max(a, b))))

def most_swap(+a: Nat, +b: Nat, +c: Nat) -> {O.Cr.most(O.Cr.most(a, b), c) == O.Cr.most(O.Cr.most(a, c), b) : Nat}:
  Equal.trans(Nat, O.Cr.most(O.Cr.most(a, b), c), Nat.max(Nat.max(a, b), c), O.Cr.most(O.Cr.most(a, c), b), most_nested(a, b, c),
    Equal.trans(Nat, Nat.max(Nat.max(a, b), c), Nat.max(Nat.max(a, c), b), O.Cr.most(O.Cr.most(a, c), b), max_swap(a, b, c),
      Equal.sym(Nat, O.Cr.most(O.Cr.most(a, c), b), Nat.max(Nat.max(a, c), b), most_nested(a, c, b))))

def duplicate(+c: O.Cr, +n: Nat, +by: Nat) -> {O.Cr.drained.k(True{}, O.Cr.drained.k(True{}, c, n, by), n, by) == O.Cr.drained.k(True{}, c, n, by) : O.Cr}:
  match c:
    case O.Cr{+g, +s, +a, +b, +k, +nv, +bt, +at, +fl, +fq, +w, +rs}:
      Equal.trans(O.Cr,
        O.Cr{g, s, O.Cr.most(O.Cr.most(a, n), n), b, O.Cr.most(O.Cr.most(k, by), by), nv, bt, at, fl, fq, w, rs},
        O.Cr{g, s, O.Cr.most(a, n), b, O.Cr.most(O.Cr.most(k, by), by), nv, bt, at, fl, fq, w, rs},
        O.Cr{g, s, O.Cr.most(a, n), b, O.Cr.most(k, by), nv, bt, at, fl, fq, w, rs},
        Equal.cong(Nat, O.Cr, x => O.Cr{g, s, x, b, O.Cr.most(O.Cr.most(k, by), by), nv, bt, at, fl, fq, w, rs},
          O.Cr.most(O.Cr.most(a, n), n), O.Cr.most(a, n), most_absorb(a, n)),
        Equal.cong(Nat, O.Cr, x => O.Cr{g, s, O.Cr.most(a, n), b, x, nv, bt, at, fl, fq, w, rs},
          O.Cr.most(O.Cr.most(k, by), by), O.Cr.most(k, by), most_absorb(k, by)))

def commute(+c: O.Cr, +n: Nat, +by: Nat, +m: Nat, +more: Nat) -> {O.Cr.drained.k(True{}, O.Cr.drained.k(True{}, c, n, by), m, more) == O.Cr.drained.k(True{}, O.Cr.drained.k(True{}, c, m, more), n, by) : O.Cr}:
  match c:
    case O.Cr{+g, +s, +a, +b, +k, +nv, +bt, +at, +fl, +fq, +w, +rs}:
      Equal.trans(O.Cr,
        O.Cr{g, s, O.Cr.most(O.Cr.most(a, n), m), b, O.Cr.most(O.Cr.most(k, by), more), nv, bt, at, fl, fq, w, rs},
        O.Cr{g, s, O.Cr.most(O.Cr.most(a, m), n), b, O.Cr.most(O.Cr.most(k, by), more), nv, bt, at, fl, fq, w, rs},
        O.Cr{g, s, O.Cr.most(O.Cr.most(a, m), n), b, O.Cr.most(O.Cr.most(k, more), by), nv, bt, at, fl, fq, w, rs},
        Equal.cong(Nat, O.Cr, x => O.Cr{g, s, x, b, O.Cr.most(O.Cr.most(k, by), more), nv, bt, at, fl, fq, w, rs},
          O.Cr.most(O.Cr.most(a, n), m), O.Cr.most(O.Cr.most(a, m), n), most_swap(a, n, m)),
        Equal.cong(Nat, O.Cr, x => O.Cr{g, s, O.Cr.most(O.Cr.most(a, m), n), b, x, nv, bt, at, fl, fq, w, rs},
          O.Cr.most(O.Cr.most(k, by), more), O.Cr.most(O.Cr.most(k, more), by), most_swap(k, by, more)))
