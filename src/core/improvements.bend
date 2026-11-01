import Base

# A stable, client-generated creation key survives a lost reply or reconnect.
# It grants no provider, permission or environment override.
def Improvements.hex(s: String) -> Bool:
  match s:
    case SNil{}: True{}
    case SCon{Chr{+c}, t}:
      Bool.and(Bool.or(Bool.and(U32.is_ge(c, 48), U32.is_le(c, 57)), Bool.and(U32.is_ge(c, 97), U32.is_le(c, 102))), Improvements.hex(t))

def Improvements.width(ok: Bool, s: String) -> Bool:
  match ok:
    case True{}: Improvements.hex(s)
    case False{}: False{}

def Improvements.key(+s: String) -> Bool:
  Improvements.width(Nat.is_eq(String.length(s), 32n), s)

def Improvements.thread(key: String) -> String:
  "improvement-" ++ key
