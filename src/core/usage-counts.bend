import Base
import ./json.bend as J

# Fixed positions are shared by the local projection and the wire allowlist.
# No caller supplies field names or arbitrary JSON to the serializer.
def Uc.names() -> List<&2, String>:
  ["threads_created", "user_messages", "assistant_messages", "bot_messages", "bot_wakes", "input_tokens", "cache_read_tokens", "cache_write_tokens", "output_tokens", "usage_turns"]

# Keep the original ten-field contract available for legacy laws/readers.
def Uc.all_names() -> List<&2, String>:
  List.append(&2, String, Uc.names(), ["boards_created", "boards_ordered"])

def Uc.zero() -> List<&2, Nat>:
  [0n, 0n, 0n, 0n, 0n, 0n, 0n, 0n, 0n, 0n, 0n, 0n]

def Uc.num.go(s: String, n: Nat) -> Nat:
  match s:
    case SNil{}: n
    case SCon{Chr{+c}, t}: Uc.num.go(t, Nat.add(Nat.mul(n, 10n), U32.to_nat((c - 48 : U32))))

def Uc.digits(s: String) -> Bool:
  match s:
    case SNil{}: True{}
    case SCon{Chr{+c}, t}: Bool.and(Bool.and(U32.is_ge(c, 48), U32.is_le(c, 57)), Uc.digits(t))

def Uc.order(pair: (String & String) & Cmp) -> Bool:
  ((_, _), cmp) = pair
  Cmp.is_le(cmp)

def Uc.within(+s: String) -> Bool:
  Bool.or(Nat.is_lt(String.length(s), 15n), Bool.and(Nat.is_eq(String.length(s), 15n), Uc.order(String.cmp(s, "281474976710655"))))

def Uc.cap.k(ok: Bool, n: Nat) -> Nat:
  match ok:
    case True{}: n
    case False{}: Uc.num.go("281474976710655", 0n)

def Uc.cap(+n: Nat) -> Nat:
  Uc.cap.k(Uc.within(Nat.show(n)), n)

def Uc.num.k(ok: Bool, s: String) -> Nat:
  match ok:
    case True{}: Uc.num.go(s, 0n)
    case False{}: 0n

def Uc.num(+s: String) -> Nat:
  Uc.num.k(Bool.and(Uc.within(s), Uc.digits(s)), s)

def Uc.read.go(names: List<&2, String>, parts: List<&2, String>) -> List<&2, Nat>:
  match names parts:
    case Nil{} _: Nil{}
    case _ <> ns p <> ps: Uc.num(p) <> Uc.read.go(ns, ps)
    case _ <> ns Nil{}: 0n <> Uc.read.go(ns, Nil{})

def Uc.read(s: String) -> List<&2, Nat>:
  Uc.read.go(Uc.all_names(), String.split(s, '/'))

def Uc.strings(xs: List<&2, Nat>) -> List<&2, String>:
  match xs:
    case Nil{}: Nil{}
    case n <> tail: Nat.show(n) <> Uc.strings(tail)

def Uc.show(xs: List<&2, Nat>) -> String:
  String.join(Uc.strings(xs), "/")

def Uc.sum.k(over: Bool, a: Nat, b: Nat, limit: Nat) -> Nat:
  match over:
    case True{}: limit
    case False{}: Nat.add(a, b)

def Uc.sum.with(+limit: Nat, +a: Nat, +b: Nat) -> Nat:
  Uc.sum.k(Nat.is_gt(b, Nat.sub(limit, a)), a, b, limit)

# Test the remaining headroom before adding: the runtime itself cannot
# represent a Nat above 2^48-1, even briefly before a clamp.
def Uc.sum(a: Nat, b: Nat) -> Nat:
  Uc.sum.with(Uc.num.go("281474976710655", 0n), a, b)

def Uc.add(xs: List<&2, Nat>, at: Nat, n: Nat) -> List<&2, Nat>:
  match xs at:
    case Nil{} _: Nil{}
    case h <> t 0n: Uc.sum(h, n) <> t
    case h <> t 1n+k: h <> Uc.add(t, k, n)

def Uc.fields(names: List<&2, String>, xs: List<&2, Nat>) -> List<&2, J.KV>:
  match names xs:
    case name <> ns n <> tail: J.KV{name, J.Json.nat(Uc.cap(n))} <> Uc.fields(ns, tail)
    case _ _: Nil{}

def Uc.json(xs: List<&2, Nat>) -> J.Json:
  J.Json.obj(Uc.fields(Uc.all_names(), xs))
