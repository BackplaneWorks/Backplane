import Base

@unsafe
def f(+n: Nat) -> Nat:
  g(n)

@unsafe
def g(+n: Nat) -> Nat:
  f(n)

def h() -> {1n == 1n : Nat}:
  {==}
