import Base
import ./check.bend as C
import ../src/server/thin-signals.bend as K
import ../src/server/thin-signal-text.bend as X

def yes(b: Bool) -> String:
  Bool.pick(String, b, "yes", "no")

def scalar(key: String) -> String:
  yes(K.SignalCache.bounded(K.SignalCache.build("epoch", 1n, MLeaf{key, 7n})))

def text(key: String) -> String:
  yes(X.SignalTextCache.bounded(X.SignalTextCache.build("epoch", 1n, True{}, MLeaf{key, 7n})))

# Exactly 504 key bytes + eight scalar bytes, with distinct fixed-width ids.
def full(+n: Nat, m: Map<&2, Nat>) -> Map<&2, Nat>:
  match n:
    case 0n: m
    case 1n+k:
      +id = Nat.show(k)
      full(k, Map.set(&2, Nat, m, String.repeat("x", 500n) ++ String.repeat("0", Nat.sub(4n, String.length(id))) ++ id, 1n))

def wide(+n: Nat, m: Map<&2, Nat>) -> Map<&2, Nat>:
  match n:
    case 0n: m
    case 1n+k: wide(k, Map.set(&2, Nat, m, String.repeat("x", 600n) ++ Nat.show(k), 1n))

def many(+n: Nat, m: Map<&2, Nat>) -> Map<&2, Nat>:
  match n:
    case 0n: m
    case 1n+k: many(k, Map.set(&2, Nat, m, Nat.show(k), 1n))

def values(s: String) -> String:
  yes(K.CacheBudget.strings([("key", s)], 8193n, 524288n, True{}))

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("1024-byte scalar key retained wholly", scalar(String.repeat("x", 1024n)), "yes")
    C.Check.str("1025-byte scalar key is not retained", scalar(String.repeat("x", 1025n)), "no")
    C.Check.str("1024 UTF8 bytes of astral scalar key retained", scalar(String.repeat("😀", 256n)), "yes")
    C.Check.str("1025 UTF8 bytes of scalar key rejected", scalar(String.repeat("😀", 256n) ++ "x"), "no")
    C.Check.str("text namespace counts toward 1024-byte key", text(String.repeat("x", 1016n)), "yes")
    C.Check.str("text namespace exceeding 1024-byte key rejected", text(String.repeat("x", 1017n)), "no")
    C.Check.str("1024-byte derived value retained wholly", values(String.repeat("x", 1024n)), "yes")
    C.Check.str("1025-byte derived value rejected", values(String.repeat("x", 1025n)), "no")
    C.Check.str("1024 UTF8 byte derived value retained", values(String.repeat("😀", 256n)), "yes")
    C.Check.str("multibyte derived value overflow rejected", values(String.repeat("😀", 256n) ++ "x"), "no")
    C.Check.str("512KiB exact scalar charge including eight-byte values fits", yes(K.SignalCache.bounded(K.SignalCache.build("epoch", 1n, full(1024n, Map.new(&2, Nat))))), "yes")
    C.Check.str("scalar byte overflow rejects below entry cap", yes(K.SignalCache.bounded(K.SignalCache.build("epoch", 1n, full(1025n, Map.new(&2, Nat))))), "no")
    C.Check.str("text byte overflow rejects below entry cap", yes(X.SignalTextCache.bounded(X.SignalTextCache.build("epoch", 1n, True{}, wide(900n, Map.new(&2, Nat))))), "no")
    C.Check.str("8192 signals plus text online marker fit", yes(X.SignalTextCache.bounded(X.SignalTextCache.build("epoch", 1n, True{}, many(8192n, Map.new(&2, Nat))))), "yes")
    C.Check.str("8194 text entries exceed entry bound", yes(X.SignalTextCache.bounded(X.SignalTextCache.build("epoch", 1n, True{}, many(8193n, Map.new(&2, Nat))))), "no")
