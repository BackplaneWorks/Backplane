import Base
import ../src/core/eq.bend as Q

def verdict(same: Bool) -> U32:
  match same:
    case True{}:
      1
    case False{}:
      0

def equal(a: String, b: String) -> U32:
  verdict(Q.Eq.str(a, b))
