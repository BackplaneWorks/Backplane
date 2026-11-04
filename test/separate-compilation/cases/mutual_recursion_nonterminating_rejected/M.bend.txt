import Base

def f(+n: Nat) -> Nat:
  g(n)

def g(+n: Nat) -> Nat:
  f(n)
