import Base

# Shared, deterministic input generation. test/gpui/bend/ts/gen.ts mirrors it
# exactly, so Bend-native, Bend-JS and plain TS run the same input.

# a hash of i (two rounds of minstd), below 2^31-1; Nat is immediate only below 2^48
def K.h1(+x: Nat) -> Nat:
  Nat.mod(Nat.mul(x, 48271n), 2147483647n)

def K.h(+i: Nat) -> Nat:
  K.h1(K.h1(Nat.add(i, 12345n)))

def K.mod(+a: Nat, +b: Nat) -> Nat:
  Nat.mod(a, b)

def K.div(+a: Nat, +b: Nat) -> Nat:
  Nat.div(a, b)

def K.is(+a: Nat, +b: Nat) -> Bool:
  Nat.is_eq(a, b)

def K.b(c: Bool) -> Nat:
  match c:
    case True{}: 1n
    case False{}: 0n

def K.id(+p: String, +i: Nat) -> String:
  p ++ Nat.show(i)

# the order-dependent digest every run prints
def K.mix(+acc: Nat, +v: Nat) -> Nat:
  Nat.mod(Nat.add(Nat.mul(acc, 31337n), v), 4294967291n)

def K.digits(s: String, +acc: Nat) -> Nat:
  match s:
    case SNil{}: acc
    case SCon{Chr{+c}, tail}:
      K.digits(tail, Nat.add(Nat.mul(acc, 10n), Nat.sub(U32.to_nat(c), 48n)))

type Args is Data:
  Args{a: Nat, b: Nat, c: Nat}

# the program name first, then up to three numbers
def K.parse(xs: List<&1, String>, a: Nat, b: Nat, c: Nat, k: Nat) -> Args:
  match xs k:
    case Nil{} _: Args{a, b, c}
    case x <> rest 0n: K.parse(rest, a, b, c, 1n)
    case x <> rest 1n: K.parse(rest, K.digits(x, 0n), b, c, 2n)
    case x <> rest 2n: K.parse(rest, a, K.digits(x, 0n), c, 3n)
    case x <> rest 3n+_: K.parse(rest, a, b, K.digits(x, 0n), 4n)

def K.args() -> IO(Args):
  do IO<Args>:
    xs : List<&1, String> <- IO.args()
    return K.parse(xs, 0n, 0n, 0n, 0n)

def K.Args.a(x: Args) -> Nat:
  match x:
    case Args{a, _, _}: a

def K.Args.b(x: Args) -> Nat:
  match x:
    case Args{_, b, _}: b

def K.Args.c(x: Args) -> Nat:
  match x:
    case Args{_, _, c}: c

# total ms over `inner` runs as per-run ms with 4 decimals
def K.dig(+x: Nat, +d: Nat) -> String:
  Nat.show(Nat.mod(Nat.div(x, d), 10n))

def K.fmt.k(+x: Nat) -> String:
  Nat.show(Nat.div(x, 10000n)) ++ "." ++ K.dig(x, 1000n) ++ K.dig(x, 100n) ++ K.dig(x, 10n) ++ K.dig(x, 1n)

def K.fmt(+total: Nat, +inner: Nat) -> String:
  K.fmt.k(Nat.div(Nat.mul(total, 10000n), inner))
