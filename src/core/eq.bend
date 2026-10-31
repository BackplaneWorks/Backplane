import Base
import ./model.bend as M

# Exact, structural equality for values a cache compares against the ones
# it was made from. Each is true only for equal values (laws eq_*_sound),
# so nothing stale is ever reused.

def Eq.bool(a: Bool, b: Bool) -> Bool:
  match a b:
    case False{} False{}:
      True{}
    case True{} True{}:
      True{}
    case _ _:
      False{}

def Eq.chr(a: Char, b: Char) -> Bool:
  match a b:
    case Chr{x} Chr{y}:
      U32.is_eq(x, y)

# the verdict so far is carried, so every step is a tail call and the walk
# stops at the first difference: a frame per character faulted the hub's
# 64 MiB stack on macOS on a board's long plot chunks (test/eq_str_test.bend)
def Eq.str.go(a: String, b: String, ok: Bool) -> Bool:
  match a b ok:
    case SNil{} SNil{} True{}:
      True{}
    case SCon{x, xt} SCon{y, yt} True{}:
      Eq.str.go(xt, yt, Eq.chr(x, y))
    case _ _ _:
      False{}

def Eq.str(a: String, b: String) -> Bool:
  Eq.str.go(a, b, True{})

def Eq.role(a: M.Role, b: M.Role) -> Bool:
  match a b:
    case M.User{} M.User{}:
      True{}
    case M.Assistant{} M.Assistant{}:
      True{}
    case M.System{} M.System{}:
      True{}
    case _ _:
      False{}

def Eq.entry(a: M.Entry, b: M.Entry) -> Bool:
  match a b:
    case M.Msg{a0, a1, a2, a3, a4} M.Msg{b0, b1, b2, b3, b4}:
      Bool.and(Eq.str(a0, b0), Bool.and(Eq.str(a1, b1), Bool.and(Eq.role(a2, b2), Bool.and(Eq.str(a3, b3), Nat.is_eq(a4, b4)))))
    case M.Act{a0, a1, a2, a3, a4, a5} M.Act{b0, b1, b2, b3, b4, b5}:
      Bool.and(Eq.str(a0, b0), Bool.and(Eq.str(a1, b1), Bool.and(Eq.str(a2, b2), Bool.and(Eq.str(a3, b3), Bool.and(Eq.str(a4, b4), Nat.is_eq(a5, b5))))))
    case _ _:
      False{}
