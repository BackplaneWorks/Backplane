import Base

def even(+n: Nat) -> Nat:
  match n:
    case 0n:
      1n
    case 1n+p:
      odd(p)

def odd(+n: Nat) -> Nat:
  match n:
    case 0n:
      0n
    case 1n+p:
      even(p)
