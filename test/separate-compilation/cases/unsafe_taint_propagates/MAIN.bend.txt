import Base

@unsafe
def f(+n: Nat) -> Nat:
  g(n)

@unsafe
def g(+n: Nat) -> Nat:
  f(n)

def k() -> Nat:
  f(1n)
